// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 22 (finding question, ownership befund-rueckfrage): personas ask about findings that are done, status proposal
//! Done, „Ja, erledigt“/„Nein, noch offen“, state k.
//!
//! Models only through the mock `MockEliezer` on 127.0.0.1:0 (never Eliezer, never the network), tests with models
//! under `common::model_queue`. The „Fährhaus“ and all answers of the personas are freely invented.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_llm::mock::{MockEliezer, Reply};
use baruch_server::assist::eliezer::EliezerPort;
use baruch_server::assist::{Mode, Order, OrderFrame, PostLinks, Scope, prompts};
use baruch_store::{FindingStatus, IdeaOrigin, StatusChange};
use serde_json::{Value, json};

use crate::common;

const BOOK: &str = "#set document(title: \"Das Fährhaus\", author: \"Lene Ufer\")\n#set heading(numbering: \"1.\")\n\n\
= Am Steg\n\nDer Fährmann zieht das graue Seil durch die Rolle. Nebel liegt über dem Fluss.\n\n\
= Die Überfahrt\n\nDrüben wartet niemand. Nur ein Hund bellt am Ufer.\n";

struct Ferry {
    server: common::Server,
    eliezer: MockEliezer,
    work: i64,
    slug: String,
    keys: Vec<i64>,
}

impl Ferry {
    async fn commits(&self) -> usize {
        common::git(&self.server.mirror_dir_of(&self.slug), &["rev-list", "--count", "HEAD"])
            .map_or(0, |c| c.trim().parse().unwrap())
    }

    async fn last_commit(&self) -> String {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "-1", "--format=%an%n%s%n%b"]).unwrap_or_default()
    }

    /// Check job for `persona` for the first section (like „Prüfen“ in the user interface or MCP review).
    async fn review(&self, persona: &str, reply: &str) -> baruch_server::assist::Outcome {
        self.eliezer.push(Reply::text(&[reply]));
        baruch_server::assist::run(&self.server.state, &order(self.work, self.keys[0], persona, Mode::Review), None)
            .await
            .expect("Prüfen gelingt")
    }

    /// Last call sent to the mock: (system, user).
    fn last_call(&self) -> (String, String) {
        let body = self.eliezer.chat_bodies().pop().expect("ein Aufruf ging an den Nachbau");
        messages(&body)
    }

    async fn form(&self, uri: &str, form: String) -> (StatusCode, Option<String>, String) {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form))
            .unwrap();
        let (status, headers, body) = self.server.send(request).await;
        (status, headers.get(header::LOCATION).map(|l| l.to_str().unwrap().to_owned()), body)
    }

    async fn page(&self, uri: &str) -> String {
        let (status, html) = self.server.get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
        html.replace("&#x2f;", "/").replace("&amp;", "&").replace("&quot;", "\"")
    }
}

fn messages(body: &Value) -> (String, String) {
    let list = body["messages"].as_array().expect("messages im Aufruf");
    let pick = |role: &str| {
        list.iter()
            .filter(|m| m["role"] == role)
            .map(|m| m["content"].as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    };
    (pick("system"), pick("user"))
}

fn order(work: i64, key: i64, persona: &str, mode: Mode) -> Order {
    Order {
        work,
        key,
        persona: persona.into(),
        mode,
        text: String::new(),
        rev: None,
        asker: None,
        post_question: false,
        sources: vec![],
        web: false,
        scope: Scope::Section,
        frame: OrderFrame::default(),
        links: PostLinks::default(),
        basis: None,
    }
}

async fn ferry() -> Ferry {
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
    let report = server.import(BOOK, "faehrhaus.typ").await;
    let keys = server.state.store.load_work(report.work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Ferry { server, eliezer, work: report.work, slug: report.slug, keys }
}

/// First check by @nathan with exactly one critical finding (with solution, so that nothing is requested again).
const FIRST: &str =
    "Kritisch: „graue Seil“ ist zu blass für den Morgen.\n- statt: „graue Seil“\n- besser: „nasse Seil“";

async fn nathan_finding(f: &Ferry) -> i64 {
    let outcome = f.review("@nathan", FIRST).await;
    assert_eq!(outcome.findings.len(), 1, "die erste Prüfung ergibt genau einen Befund: {outcome:?}");
    outcome.findings[0]
}

#[tokio::test]
async fn a_recheck_shows_the_persona_its_own_open_findings_and_the_rule() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let (system, user) = {
        let id = nathan_finding(&f).await;
        let (system, user) = f.last_call();
        assert!(!user.contains(prompts::OWN_FINDINGS_TITLE), "die erste Prüfung kennt noch keine eigenen Befunde");
        assert!(!system.contains(prompts::RECHECK_RULE), "ohne eigene Befunde keine Regel im System");
        // A foreign open finding (@lektor) does not belong in the block of the own ones.
        let lektor = f.review("@lektor", "Hinweis: „Nebel“ könnte einen zweiten Satz tragen.").await;
        assert_eq!(lektor.findings.len(), 1, "der Lektor hat seinen eigenen Befund");
        f.review("@nathan", "Kritisch: „bellt“ wirkt verloren.\n- Schritt: den Hund früher nennen.").await;
        let (system, user) = f.last_call();
        assert!(user.contains(&format!("[B{id}]")), "die eigene Befundnummer steht im Kontext: {user}");
        (system, user.replace(&format!("[B{}] zu", lektor.findings[0]), "FREMD"))
    };
    let block = user.split(prompts::OWN_FINDINGS_TITLE).nth(1).expect("Block „Deine offenen Befunde“ im Kontext");
    let block = block.split("\n\n").next().unwrap();
    assert!(block.contains("<<<befund"), "die eigenen Befunde stehen gerahmt als Daten: {block}");
    assert!(block.contains("ist zu blass für den Morgen"), "der Text des Befunds steht im Block: {block}");
    assert!(block.contains("graue Seil"), "das Zitat des Befunds steht im Block: {block}");
    assert!(!block.contains("FREMD") && !block.contains("Nebel"), "Befunde anderer Personas nicht im eigenen Block");
    assert!(system.contains(prompts::RECHECK_RULE), "die Regel „Erneute Prüfung“ steht im System: {system}");
    assert!(system.contains("Erledigt: Befund n"), "die Regel nennt die Form der Rückmeldung");
}

#[tokio::test]
async fn a_done_line_becomes_a_status_proposal_instead_of_a_new_finding() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let id = nathan_finding(&f).await;
    let commits = f.commits().await;
    let reply = format!(
        "- **Erledigt:** Befund {id} (Stelle „nasse Seil“)\nKritisch: „bellt am Ufer“ bleibt ohne Folge.\n- Schritt: den Hund später wieder aufnehmen."
    );
    let outcome = f.review("@nathan", &reply).await;
    assert_eq!(outcome.done_proposals, vec![id], "aus der Rückmeldung wird ein Statusvorschlag: {outcome:?}");
    assert!(outcome.done_notes.is_empty(), "nichts übergangen: {:?}", outcome.done_notes);
    assert_eq!(outcome.findings.len(), 1, "nur der neue kritische Punkt ist ein Befund, nicht die Rückmeldung");
    let fresh = f.server.state.store.finding_get(f.work, outcome.findings[0]).await.unwrap();
    assert!(fresh.body.contains("bellt am Ufer"), "der neue Befund ist der kritische Punkt: {}", fresh.body);
    let finding = f.server.state.store.finding_get(f.work, id).await.unwrap();
    assert_eq!(finding.status, FindingStatus::Open, "der Befund bleibt offen, bis der Mensch entscheidet");
    let pending = finding.pending.expect("offene Rückfrage am Befund");
    assert_eq!(pending.status, FindingStatus::Done, "vorgeschlagen ist „erledigt ohne Änderung“");
    assert_eq!(pending.actor, IdeaOrigin::Persona { handle: "@nathan".into() }, "Herkunft: die Persona");
    assert_eq!(
        pending.note,
        format!("Befund {id} scheint erledigt (Stelle „nasse Seil“) – als erledigt markieren?"),
        "Notiz nach Vertrag §2.8"
    );
    // Checking itself does not commit (post and findings are in the next snapshot); the proposal is exactly one commit
    // as in Round 28.
    assert_eq!(f.commits().await, commits + 1, "genau ein Commit für den Vorschlag");
    let last = f.last_commit().await;
    assert!(last.contains("Baruch-Action: finding-propose"), "Commit des Vorschlags: {last}");
    assert!(last.contains("Persona: @nathan"), "Trailer Persona: {last}");
}

#[tokio::test]
async fn foreign_unknown_decided_and_doubled_numbers_are_skipped_and_noted() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let own = nathan_finding(&f).await;
    let decided = f.review("@nathan", "Kritisch: „Drüben“ ist unklar.\n- Schritt: das Ufer nennen.").await.findings[0];
    f.server
        .state
        .store
        .finding_set_status(f.work, decided, &StatusChange::Rejected { reason: "so gewollt".into() }, "Lene")
        .await
        .unwrap();
    let foreign = f.review("@lektor", "Hinweis: „Nebel“ trägt die Stimmung.").await.findings[0];
    let reply = format!(
        "Erledigt: Befund {foreign}\nErledigt: [B999999]\nErledigt: Befund {decided}\nErledigt: Befund {own} (Stelle „nasse Seil“)\n\n{}",
        prompts::ALL_DONE_SENTENCE
    );
    let outcome = f.review("@nathan", &reply).await;
    assert_eq!(outcome.done_proposals, vec![own], "nur der eigene, offene Befund bekommt die Rückfrage");
    let notes = outcome.done_notes.join("\n");
    assert!(notes.contains(&format!("[B{foreign}]")) && notes.contains("@lektor"), "fremder Befund vermerkt: {notes}");
    assert!(notes.contains("[B999999]") && notes.contains("nicht"), "unbekannte Nummer vermerkt: {notes}");
    assert!(
        notes.contains(&format!("[B{decided}]")) && notes.contains("entschieden"),
        "entschiedener vermerkt: {notes}"
    );
    let lektor = f.server.state.store.finding_get(f.work, foreign).await.unwrap();
    assert!(lektor.pending.is_none(), "ein fremder Befund bekommt keine Rückfrage von @nathan");
    // The same feedback once more: no second open proposal.
    let again = f.review("@nathan", &format!("Erledigt: Befund {own}\n\n{}", prompts::ALL_DONE_SENTENCE)).await;
    assert!(again.done_proposals.is_empty(), "kein doppelter offener Vorschlag: {again:?}");
    assert!(
        again.done_notes.join(" ").contains("schon offen"),
        "der doppelte steht im Vermerk: {:?}",
        again.done_notes
    );
    let history = f.server.state.store.finding_history(f.work, own).await.unwrap();
    assert_eq!(history.iter().filter(|e| e.proposed).count(), 1, "genau ein Vorschlag im Verlauf");
}

#[tokio::test]
async fn all_done_gives_no_finding_and_waives_nathans_mandatory_finding() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let id = nathan_finding(&f).await;
    let calls = f.eliezer.chat_bodies().len();
    let reply = format!("Erledigt: Befund {id} (Stelle „nasse Seil“)\n\n{}", prompts::ALL_DONE_SENTENCE);
    let outcome = f.review("@nathan", &reply).await;
    assert_eq!(f.eliezer.chat_bodies().len(), calls + 1, "keine Nachforderung des Pflichtbefunds");
    assert!(outcome.findings.is_empty(), "kein neuer Befund: {:?}", outcome.findings);
    assert!(outcome.all_done, "die Antwort sagt ausdrücklich: alle erledigt");
    let post = outcome.post.expect("der Beitrag bleibt sichtbar");
    assert!(post.body.contains(prompts::ALL_DONE_SENTENCE), "der Satz steht im Beitrag: {}", post.body);
    assert!(!post.body.contains(prompts::CRITICAL_MISSING_NOTE), "kein Hinweis „Pflichtbefund fehlt“: {}", post.body);
    let html = f.page(&format!("/?work={}&key={}", f.work, f.keys[0])).await;
    assert!(html.contains("Alle meine Befunde zu diesem Abschnitt scheinen erledigt"), "Beitrag im Abschnitts-Chat");
}

#[tokio::test]
async fn all_done_without_own_findings_does_not_waive_the_mandatory_finding() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let calls = f.eliezer.chat_bodies().len();
    f.eliezer.push(Reply::text(&[prompts::ALL_DONE_SENTENCE]));
    f.eliezer.push(Reply::text(&[prompts::ALL_DONE_SENTENCE]));
    let outcome = baruch_server::assist::run(&f.server.state, &order(f.work, f.keys[0], "@nathan", Mode::Review), None)
        .await
        .unwrap();
    assert_eq!(f.eliezer.chat_bodies().len(), calls + 2, "ohne eigene Befunde gilt der Pflichtbefund: Nachforderung");
    assert!(!outcome.all_done, "„alle erledigt“ ohne eigene Befunde zählt nicht");
    assert!(
        outcome.post.unwrap().body.contains(prompts::CRITICAL_MISSING_NOTE),
        "der Hinweis „Pflichtbefund fehlt“ bleibt"
    );
}

#[tokio::test]
async fn repeat_and_rfc_are_rechecks_too() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let id = nathan_finding(&f).await;
    let first = f.server.state.store.finding_get(f.work, id).await.unwrap().post.expect("Befund aus einem Beitrag");
    // ↻ Wiederholen (post.repeat) of the post with the finding.
    f.eliezer.push(Reply::text(&[FIRST]));
    let answer = f.server.rpc("post.repeat", json!({ "work": f.work, "id": first })).await;
    assert!(answer["error"].is_null(), "Wiederholen gelingt: {answer}");
    let (system, user) = f.last_call();
    assert!(user.contains(prompts::OWN_FINDINGS_TITLE) && user.contains(&format!("[B{id}]")), "Wiederholen: {user}");
    assert!(system.contains(prompts::RECHECK_RULE), "Wiederholen: Regel im System");
    // RFC: all active personas; only @nathan has open findings of his own.
    let (others, lector) = baruch_server::assist::chat::rfc_personas(&f.server.state).await.unwrap();
    let before = f.eliezer.chat_bodies().len();
    for _ in others.iter().chain(lector.iter()) {
        f.eliezer.push(Reply::text(&["Kritisch: „Hund“ bleibt stumm.\n- Schritt: ihn bellen lassen."]));
    }
    let answer =
        f.server.rpc("persona.chat", json!({ "work": f.work, "key": f.keys[0], "message": "RFC bitte" })).await;
    assert!(answer["error"].is_null(), "RFC gelingt: {answer}");
    let bodies: Vec<(String, String)> = f.eliezer.chat_bodies()[before..].iter().map(messages).collect();
    let with_own: Vec<&(String, String)> =
        bodies.iter().filter(|(_, u)| u.contains(prompts::OWN_FINDINGS_TITLE)).collect();
    assert!(!with_own.is_empty(), "beim RFC sieht @nathan seine offenen Befunde");
    assert!(
        with_own.iter().all(|(s, u)| s.contains(prompts::RECHECK_RULE) && u.contains(&format!("[B{id}]"))),
        "mit Regel und Nummer"
    );
    assert!(
        bodies.len() > with_own.len(),
        "Personas ohne eigene offene Befunde bekommen keinen Block ({} von {})",
        with_own.len(),
        bodies.len()
    );
}

#[tokio::test]
async fn the_question_shows_yes_and_no_and_the_human_decides_with_a_form() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let id = nathan_finding(&f).await;
    let reply = format!("Erledigt: Befund {id} (Stelle „nasse Seil“)\n\n{}", prompts::ALL_DONE_SENTENCE);
    f.review("@nathan", &reply).await;
    let entry = f.server.state.store.finding_get(f.work, id).await.unwrap().pending.unwrap().id;
    let workspace = f.page(&format!("/?work={}&key={}", f.work, f.keys[0])).await;
    for (place, html) in [
        ("Arbeitsseite", &workspace),
        ("Befundseite", &f.page(&format!("/werke/{}/befunde?befund={id}", f.work)).await),
    ] {
        assert!(html.contains("Rückfrage"), "{place}: Rückfrage sichtbar");
        assert!(
            html.contains("scheint erledigt (Stelle „nasse Seil“) – als erledigt markieren?"),
            "{place}: Frage der Persona"
        );
        assert!(html.contains("Ja, erledigt") && html.contains("Nein, noch offen"), "{place}: beide Knöpfe");
        assert!(
            html.contains(&format!("name=\"entry\" value=\"{entry}\"")) && html.contains("value=\"finding.decide\""),
            "{place}: Formulare an finding.decide ohne JavaScript"
        );
    }
    assert!(workspace.contains("data-finding-question"), "Kasten der Rückfrage für das Widget");
    // The finding in the chat post (finding_block) carries the question as well.
    assert!(workspace.contains("fb-question"), "Befundkarte im Chat zeigt die Rückfrage");

    // „Nein, noch offen“ without JavaScript: the finding stays open, the proposal is declined.
    let commits = f.commits().await;
    let back = format!("/?work={}&key={}", f.work, f.keys[0]);
    let form = format!("method=finding.decide&work={}&entry={entry}&confirm=0&back={}", f.work, urlencoding(&back));
    let (status, location, body) = f.form("/werke/befunde/act", form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet weiter: {body}");
    assert!(location.unwrap_or_default().starts_with("/?work="), "zurück zur Arbeitsseite");
    let finding = f.server.state.store.finding_get(f.work, id).await.unwrap();
    assert_eq!(finding.status, FindingStatus::Open, "„Nein, noch offen“ lässt den Befund offen");
    assert!(finding.pending.is_none(), "die Rückfrage ist beantwortet");
    assert_eq!(f.commits().await, commits + 1, "Entscheidung = genau ein Commit");
    assert!(f.last_commit().await.contains("Baruch-Action: finding-decide"), "Commit der Entscheidung");

    // New question, then „Ja, erledigt“.
    f.review("@nathan", &reply).await;
    let entry = f.server.state.store.finding_get(f.work, id).await.unwrap().pending.expect("neue Rückfrage").id;
    let form = format!("method=finding.decide&work={}&entry={entry}&confirm=1&back={}", f.work, urlencoding(&back));
    let (status, _, body) = f.form("/werke/befunde/act", form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet weiter: {body}");
    let finding = f.server.state.store.finding_get(f.work, id).await.unwrap();
    assert_eq!(finding.status, FindingStatus::Done, "„Ja, erledigt“ setzt den Status (Mensch)");
    assert!(finding.reason.contains("laut @nathan umgesetzt"), "Begründung aus der Rückmeldung: {}", finding.reason);
    let (s, last) = (f.commits().await, f.last_commit().await);
    assert!(s > commits + 1 && last.contains("Baruch-Action: finding-decide"), "wieder ein Commit: {last}");
}

#[tokio::test]
async fn an_adopted_finding_shows_version_and_stand_in_card_list_and_matrix() {
    let f = ferry().await;
    let seq = f.server.state.store.node_seq(f.work, f.keys[0]).await.unwrap();
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: "Hinweis: „Nebel“ trägt die Stimmung.".into(),
        sources: vec![],
        refs: vec![],
    };
    let post_id = f.server.state.store.add_post(f.work, seq, &post).await.unwrap();
    let source = baruch_server::findings::IngestSource {
        work: f.work,
        origin: baruch_server::findings::IngestOrigin::Post { id: post_id, key: f.keys[0] },
        persona: "@lektor",
        text: &post.body,
        summary: false,
        manual: false,
    };
    let id = baruch_server::findings::ingest(&f.server.state, &source).await.unwrap().created[0].id;
    // State by SQL (the task fassungen builds the amending; here only the state of the revision).
    sqlx::query("UPDATE revision SET amend_seq = 2 WHERE node_id = ? AND number = 1")
        .bind(f.keys[0])
        .execute(f.server.state.store.pool())
        .await
        .unwrap();
    let answer =
        f.server.rpc("finding.set_status", json!({ "work": f.work, "id": id, "status": "adopted", "rev": 1 })).await;
    assert!(answer["error"].is_null(), "übernommen setzen: {answer}");
    assert_eq!(answer["result"]["finding"]["adopted_amend"], 2, "RPC liefert adopted_amend: {answer}");
    let list = f.server.rpc("finding.list", json!({ "work": f.work, "status": "adopted" })).await;
    assert_eq!(list["result"]["findings"][0]["adopted_amend"], 2, "finding.list liefert adopted_amend: {list}");
    let detail = f.page(&format!("/werke/{}/befunde?befund={id}", f.work)).await;
    assert!(detail.contains("übernommen in Fassung 1 · Stand 2"), "Karte auf der Befundseite nennt den Stand");
    let overview = f.page(&format!("/werke/{}/befunde?status=adopted", f.work)).await;
    assert!(overview.contains("· Stand 2"), "Verlauf je Kapitel (Matrix) nennt den Stand");
    let workspace = f.page(&format!("/?work={}&key={}", f.work, f.keys[0])).await;
    assert!(workspace.contains("übernommen in Fassung 1 · Stand 2"), "Akkordeon „Befunde“ nennt den Stand");
}

/// Percent-encoding for form values (only what occurs in an address).
fn urlencoding(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[tokio::test]
async fn mcp_ask_persona_review_is_a_recheck_and_proposes_done() {
    use axum::extract::ConnectInfo;
    use std::net::SocketAddr;
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let id = nathan_finding(&f).await;
    let reply = format!("Erledigt: [B{id}] (Stelle „nasse Seil“)\n\n{}", prompts::ALL_DONE_SENTENCE);
    f.eliezer.push(Reply::text(&[&reply]));
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "ask_persona",
                    "arguments": { "work": f.work, "key": f.keys[0], "persona": "@nathan", "mode": "review" } } })
            .to_string(),
        ))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50_000))));
    let (status, _, text) = f.server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {text}");
    let answer: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(answer["result"]["isError"], false, "ask_persona review gelingt: {text}");
    let (system, user) = f.last_call();
    assert!(user.contains(prompts::OWN_FINDINGS_TITLE), "MCP review sieht die eigenen offenen Befunde: {user}");
    assert!(system.contains(prompts::RECHECK_RULE), "MCP review: Regel im System");
    let pending = f.server.state.store.finding_get(f.work, id).await.unwrap().pending.expect("Rückfrage über MCP");
    assert_eq!(pending.status, FindingStatus::Done, "Statusvorschlag „erledigt“ auch auf dem MCP-Weg");
}

/// PM close R22 (review): feedback on an own open finding from another section is passed over – the persona has not
/// seen its text in this job.
#[tokio::test]
async fn a_done_claim_for_a_finding_of_another_section_is_skipped() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let own = nathan_finding(&f).await;
    // With „Kritisch:“, so that @nathan does not request a mandatory finding again (one call to the mock).
    let reply = format!(
        "Erledigt: Befund {own} (Stelle „Hund“)\n\nKritisch: „Drüben“ bleibt vage.\n- Schritt: das Ufer nennen."
    );
    f.eliezer.push(Reply::text(&[&reply]));
    let outcome = baruch_server::assist::run(&f.server.state, &order(f.work, f.keys[1], "@nathan", Mode::Review), None)
        .await
        .expect("Prüfen des zweiten Abschnitts gelingt");
    assert!(
        outcome.done_proposals.is_empty(),
        "Befund aus Abschnitt 1 bekommt beim Prüfen von Abschnitt 2 keine Rückfrage"
    );
    let notes = outcome.done_notes.join("\n");
    assert!(
        notes.contains(&format!("[B{own}]")) && notes.contains("nicht in diesem Auftrag"),
        "übergangen mit Grund: {notes}"
    );
    let finding = f.server.state.store.finding_get(f.work, own).await.unwrap();
    assert!(finding.pending.is_none(), "kein Statusvorschlag am Befund aus dem anderen Abschnitt");
}

/// Follow-up fix 0.9.0 (user 2026-11-24: „Der Befund lässt sich nirgends anspringen“): the quotation of a finding and
/// „zur Stelle“ lead with a jump address (`find`) to the place in the section – in the accordion and on the findings
/// page, without JavaScript as an ordinary link.
#[tokio::test]
async fn the_quote_of_a_finding_jumps_to_its_place_in_the_section() {
    let f = ferry().await;
    let id = nathan_finding(&f).await;
    let jump = format!("/?work={}&key={}&find=graue%20Seil", f.work, f.keys[0]);
    let workspace = f.page(&format!("/?work={}&key={}", f.work, f.keys[0])).await;
    let card = workspace.split(&format!("id=\"befund-{id}\"")).nth(1).expect("Karte des Befunds im Akkordeon");
    let card = card.split("</article>").next().unwrap();
    assert!(
        card.contains(&format!(
            "<a href=\"{jump}\" data-jump title=\"Stelle im Abschnitt anspringen\">„graue Seil“</a>"
        )),
        "das Zitat ist ein Sprung-Link an die Stelle: {card}"
    );
    assert!(
        card.contains(&format!("href=\"{jump}\" data-jump")) && card.contains(">zur Stelle</a>"),
        "„zur Stelle“ im Kopf: {card}"
    );
    let detail = f.page(&format!("/werke/{}/befunde?befund={id}", f.work)).await;
    assert!(detail.contains(&format!("href=\"{jump}\" data-jump")), "auch auf der Befundseite anspringbar");
}

/// Round 36 (note from 0.9.0): since the critique cycle every directly addressed persona got its own open findings into
/// the context – even with a plain question that has nothing to do with checking. Now only with a check job
/// (`chat::is_check_order`); RFC, „Prüfen“ and ↻ stay renewed checking.
#[tokio::test]
async fn a_plain_question_to_an_addressed_persona_carries_no_own_findings_but_a_check_order_does() {
    let _turn = common::model_queue(module_path!()).await;
    let f = ferry().await;
    let id = nathan_finding(&f).await;
    // The first call to the mock that the message triggers (a repeated request would come after it).
    let ask = async |message: &str| -> (String, String) {
        let before = f.eliezer.chat_bodies().len();
        f.eliezer.push(Reply::text(&["Kritisch: „Rolle“ knarrt nicht.\n- Schritt: das Knarren hörbar machen."]));
        let params = json!({ "work": f.work, "key": f.keys[0], "message": message });
        let answer = f.server.rpc("persona.chat", params).await;
        assert!(answer["error"].is_null(), "Chat „{message}“ gelingt: {answer}");
        let bodies = f.eliezer.chat_bodies();
        assert!(bodies.len() > before, "die Nachricht „{message}“ ruft die Persona");
        messages(&bodies[before])
    };
    let own = format!("[B{id}]");

    let (system, user) = ask("@nathan wie breit ist der Fluss an dieser Stelle?").await;
    assert!(user.contains("wie breit ist der Fluss"), "Vorbedingung: der Aufruf trägt die Frage: {user}");
    assert!(
        !user.contains(prompts::OWN_FINDINGS_TITLE) && !user.contains(&own),
        "eine schlichte Frage zieht die eigenen offenen Befunde nicht in den Kontext: {user}"
    );
    assert!(!system.contains(prompts::RECHECK_RULE), "und nicht die Regel des erneuten Prüfens ins System");

    let (system, user) = ask("@nathan prüf den Abschnitt bitte noch einmal").await;
    assert!(
        user.contains(prompts::OWN_FINDINGS_TITLE) && user.contains(&own),
        "ein Prüfauftrag an die Persona bleibt erneutes Prüfen mit den eigenen offenen Befunden: {user}"
    );
    assert!(system.contains(prompts::RECHECK_RULE), "samt Regel im System");

    let (_, user) = ask("@nathan kannst du das bitte überprüfen?").await;
    assert!(user.contains(prompts::OWN_FINDINGS_TITLE), "auch als Frage formuliert ist es ein Prüfauftrag: {user}");

    let (_, user) = ask(&format!("@nathan was meinst du mit {own}?")).await;
    assert!(
        user.contains(prompts::OWN_FINDINGS_TITLE),
        "wer einen Befund nennt, fragt nach den Befunden – die Persona braucht sie: {user}"
    );

    let (_, user) = ask("> bitte prüfen\n@nathan wie heißt der Hund am Ufer?").await;
    assert!(
        !user.contains(prompts::OWN_FINDINGS_TITLE),
        "ein Prüfwort in einer Zitatzeile macht die Frage nicht zum Prüfauftrag: {user}"
    );
}

#[test]
fn check_orders_are_recognised_by_word_starts_and_finding_numbers() {
    use baruch_server::assist::chat::is_check_order;
    for message in [
        "@nathan prüf den Vorschlag",
        "Bitte PRÜFE das noch einmal, @nathan",
        "@nathan kannst du das überprüfen?",
        "@nathan Review bitte",
        "@nathan check das Zitat",
        "@nathan kontrolliere die Zahlen",
        "@nathan ist [B12] noch offen?",
    ] {
        assert!(is_check_order(message), "„{message}“ ist ein Prüfauftrag");
    }
    for message in [
        "@nathan wie alt war der Fährmann?",
        "@nathan das hast du schon geprüft, danke",
        "@pruefer wie spät ist es?",
        "> prüf das\n@nathan danke",
        "@nathan [B] und [Bx] sind keine Nummern",
        "",
    ] {
        assert!(!is_check_order(message), "„{message}“ ist kein Prüfauftrag");
    }
}
