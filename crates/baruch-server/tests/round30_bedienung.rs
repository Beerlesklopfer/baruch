// SPDX-License-Identifier: AGPL-3.0-or-later
//! Chat-Bedienung (Runde 30c): Wiederholen, Löschen, Anrede, Relay, gegliederte Beiträge, Sprungmarken-Links.
//!
//! Besitz: chat-bedienung. Kein Netz, kein Eliezer: Modelle nur über `ScriptedModel` (zeichnet jeden Aufruf auf).
//! Alle Texte frei erfunden (ein kleines „Brückenbuch“ mit Leuchtturm, Wachturm und Brücke).

use crate::common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::port::ModelCall;
use baruch_store::{FindingFilter, FindingKind, FindingStatus, IdeaOrigin, NewFinding};
use serde_json::{Value, json};

const BRIDGE: &str = "#set document(title: \"Das Brückenbuch\", author: \"Ida Lorenz\")\n\n\
= Der Leuchtturm\n\nDas Licht kreist über dem Wasser und zählt die Wellen.\n\n\
== Der Wachturm\n\nOben steht ein Fernrohr aus Messing.\n\n\
== Die Brücke\n\nDie Brücke trägt drei Laternen über den Fluss.\n";

struct Bridge {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    slug: String,
    /// Schlüssel von Leuchtturm, Wachturm, Brücke.
    keys: Vec<i64>,
}

async fn bridge(steps: impl IntoIterator<Item = Step>) -> Bridge {
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
    let report = server.import(BRIDGE, "bruecke.typ").await;
    let keys = server.state.store.load_work(report.work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Bridge { server, model, work: report.work, slug: report.slug, keys }
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

/// Alle Texte eines Modellaufrufs (System und Nutzer) hintereinander.
fn all_text(call: &ModelCall) -> String {
    call.messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n")
}

async fn posts(b: &Bridge, key: usize) -> Vec<baruch_core::Post> {
    let loaded = b.server.state.store.load_work(b.work).await.unwrap();
    loaded.nodes.iter().find(|n| n.key == b.keys[key]).unwrap().posts.clone()
}

fn authors(posts: &[baruch_core::Post]) -> Vec<&str> {
    posts.iter().map(|p| p.author.as_str()).collect()
}

async fn add_post(b: &Bridge, key: usize, author: &str, body: &str) -> i64 {
    let seq = b.server.state.store.node_seq(b.work, b.keys[key]).await.unwrap();
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: author.into(),
        body: body.into(),
        sources: vec![],
        refs: vec![],
    };
    b.server.state.store.add_post(b.work, seq, &post).await.unwrap()
}

async fn page(b: &Bridge, key: usize, rev: Option<u32>) -> String {
    let rev = rev.map_or(String::new(), |r| format!("&rev={r}"));
    let (status, html) = b.server.get(&format!("/?work={}&key={}{rev}", b.work, b.keys[key])).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite: {html}");
    html
}

async fn commits(b: &Bridge) -> usize {
    common::git(&b.server.mirror_dir_of(&b.slug), &["rev-list", "--count", "HEAD"])
        .map_or(0, |c| c.trim().parse().unwrap())
}

async fn last_commit(b: &Bridge) -> String {
    common::git(&b.server.mirror_dir_of(&b.slug), &["log", "-1", "--format=%s%n%b"]).unwrap_or_default()
}

async fn chat(b: &Bridge, key: usize, message: &str) -> Value {
    let answer = b.server.rpc("persona.chat", json!({ "work": b.work, "key": b.keys[key], "message": message })).await;
    result(&answer).clone()
}

// ---- Anrede ---------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn an_address_with_the_name_calls_the_persona_but_the_name_mid_sentence_does_not() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge([Step::words("Was trägt: die Brücke mit ihren Laternen.")]).await;
    let answer = chat(&b, 2, "Lektor, was meinst du?").await;
    assert!(answer["errors"].as_array().unwrap().is_empty(), "keine Fehler: {answer}");
    assert_eq!(authors(&posts(&b, 2).await), ["Autor", "@lektor"], "„Lektor, …“ ruft den Lektor wie @lektor");
    assert_eq!(b.model.calls().len(), 1, "genau ein Modellaufruf");
    assert_eq!(b.model.calls()[0].persona, "@lektor", "an den Lektor");

    chat(&b, 2, "Ich glaube, der Lektor meint das anders.").await;
    assert_eq!(authors(&posts(&b, 2).await), ["Autor", "@lektor", "Autor"], "Fließtext ruft niemanden");
    assert_eq!(b.model.calls().len(), 1, "kein weiterer Modellaufruf für „der Lektor meint“");

    let html = page(&b, 2, None).await;
    assert!(
        html.contains("<div class=\"post-basis tiny\" title=\"Grundlage dieser Antwort (von Baruch, nicht vom Modell)\">geprüft: aktuelle Fassung 1</div>"),
        "Kopfzeile „geprüft: …“ aus den Metadaten (PostMeta.basis_label): {html}"
    );
}

// ---- Relay ----------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_persona_addressing_another_at_the_start_of_a_line_gets_exactly_one_relay_answer() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge([
        Step::words("Was trägt: die Brücke.\n@schrift: Stimmt die Stelle Ps 23,1 hier?"),
        Step::words("Die Stelle ist richtig angegeben."),
    ])
    .await;
    chat(&b, 2, "@lektor Wie wirkt die Brücke?").await;
    let list = posts(&b, 2).await;
    assert_eq!(authors(&list), ["Autor", "@lektor", "@schrift"], "@schrift antwortet auf die Anrede des Lektors");
    let calls = b.model.calls();
    assert_eq!(calls.len(), 2, "Lektor und eine Relay-Antwort");
    let (lector_post, relay_post) = (list[1].id, list[2].id);
    assert!(
        all_text(&calls[1]).contains(&format!("@lektor hat dich in Beitrag {lector_post} direkt angesprochen")),
        "der Auftrag der Relay-Antwort kommt von Baruch und nennt den Beitrag: {}",
        all_text(&calls[1])
    );
    let meta = b.server.state.store.post_meta(b.work, &[relay_post]).await.unwrap().remove(&relay_post).unwrap();
    assert_eq!(meta.relay_from.as_deref(), Some("@lektor"), "Relay von der anredenden Persona");
    assert_eq!(meta.reply_to, Some(lector_post), "antwortet auf den Beitrag des Lektors");
    let html = page(&b, 2, None).await;
    assert!(
        html.contains(&format!("antwortet auf <a href=\"#post-{lector_post}\">Beitrag {lector_post}</a> von @lektor")),
        "sichtbar „antwortet auf Beitrag n von @…“: {html}"
    );
}

#[tokio::test]
async fn the_relay_keeps_one_stage_three_answers_each_persona_once_and_is_off_in_an_rfc() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge([
        Step::words("Bitte prüft:\n@schrift: die Stelle\n@exeget: den Zusammenhang\n@belege: die Quelle\n@hoerer: die Wirkung\n@lektor: mich selbst"),
        Step::words("@nathan: bitte du weiter."),
        Step::words("Der Zusammenhang trägt."),
        Step::words("Keine Quelle nötig."),
    ])
    .await;
    chat(&b, 2, "@lektor Wie wirkt die Brücke?").await;
    let personas: Vec<String> = b.model.calls().iter().map(|c| c.persona.clone()).collect();
    assert_eq!(
        personas,
        ["@lektor", "@schrift", "@exeget", "@belege"],
        "höchstens drei Relay-Antworten (@hoerer nicht mehr), der Lektor nicht an sich selbst, und eine Relay-Antwort \
         ruft niemanden (@nathan nicht)"
    );
    let list = posts(&b, 2).await;
    let metas = b.server.state.store.post_meta(b.work, &list.iter().map(|p| p.id).collect::<Vec<_>>()).await.unwrap();
    for post in &list[2..] {
        assert_eq!(metas[&post.id].relay_from.as_deref(), Some("@lektor"), "{}: Relay vom Lektor", post.author);
    }

    // RFC: jede aktive Persona genau einmal, Anreden in den Antworten rufen niemanden.
    let (others, lector) = baruch_server::assist::chat::rfc_personas(&b.server.state).await.unwrap();
    let expected = others.len() + usize::from(lector.is_some());
    for _ in 0..expected {
        // Integrator Runde 30: mit Lösungszeile, sonst fordert die Lösungspflicht (chat-loesungen) einmal nach und
        // verbraucht einen weiteren Aufruf, der hier fälschlich als Relay zählte.
        b.model
            .push(Step::words("Kritisch: „drei Laternen“ wirkt schwer.\n- Schritt: prüfen.\n@hoerer: Was meinst du?"));
    }
    let before = b.model.calls().len();
    chat(&b, 2, "RFC Wie wirkt die Brücke?").await;
    assert_eq!(b.model.calls().len() - before, expected, "beim RFC kein Relay: genau ein Aufruf je aktiver Persona");
}

// ---- Wiederholen ----------------------------------------------------------------------------------------------------

#[tokio::test]
async fn repeating_asks_the_same_against_the_shown_revision_and_proposes_vanished_findings_as_done() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge([
        // Integrator Runde 30: der kritische Punkt mit Lösung (Lösungspflicht, chat-loesungen), sonst würde einmal
        // nachgefordert und der zweite Schritt verbraucht.
        Step::words("- Kritisch: „drei Laternen“ wirken schwer.\n  Schritt: eine Laterne streichen.\n- Hinweis: „über den Fluss“ ist blass."),
        Step::words("- Hinweis: „über den Fluss“ ist blass."),
    ])
    .await;
    let answer = chat(&b, 2, "@lektor Wie wirkt die Brücke?").await;
    let old = answer["posts"][1]["id"].as_i64().expect("Beitrag des Lektors");
    let filter = FindingFilter { post: Some(old), ..Default::default() };
    let findings = b.server.state.store.finding_list(b.work, &filter).await.unwrap();
    let critical = findings.iter().find(|f| f.kind == FindingKind::Critical).expect("kritischer Befund").id;
    let hint = findings.iter().find(|f| f.kind == FindingKind::Hint).expect("Hinweis").id;

    let saved = b
        .server
        .rpc(
            "revision.save",
            json!({ "work": b.work, "key": b.keys[2], "body": "== Die Brücke\n\nDie Brücke trägt zwei Laternen über den Fluss.\n" }),
        )
        .await;
    assert_eq!(result(&saved)["rev"], 2, "der Mensch hat Fassung 2 gespeichert: {saved}");

    let repeated = b.server.rpc("post.repeat", json!({ "work": b.work, "id": old, "rev": 2 })).await;
    let repeated = result(&repeated);
    let new = repeated["post"]["id"].as_i64().expect("neuer Beitrag");
    assert_eq!(repeated["repeat_of"], old, "der neue Beitrag nennt den alten");
    assert_eq!(repeated["proposed_done"], json!([critical]), "nur der Befund, der nicht wiederkommt: {repeated}");
    let calls = b.model.calls();
    assert_eq!(calls.len(), 2, "die Wiederholung ist ein Modellaufruf");
    assert_eq!(calls[1].persona, "@lektor", "dieselbe Persona");
    let text = all_text(&calls[1]);
    assert!(text.contains("Wie wirkt die Brücke?"), "dieselbe Frage: {text}");
    assert!(text.contains("zwei Laternen"), "gegen die jetzt gezeigte Fassung 2: {text}");

    let meta = b.server.state.store.post_meta(b.work, &[new]).await.unwrap().remove(&new).unwrap();
    assert_eq!(meta.repeat_of, Some(old), "repeat_of gespeichert");
    assert_eq!(meta.basis_rev, Some(2), "geprüft: Fassung 2");
    assert_eq!(
        meta.request.as_ref().map(|r| r.text.as_str()),
        Some("@lektor Wie wirkt die Brücke?"),
        "der Auftrag (die ganze Nachricht des Menschen) bleibt gleich"
    );
    assert!(posts(&b, 2).await.iter().any(|p| p.id == old), "der alte Beitrag bleibt");

    let done = b.server.state.store.finding_get(b.work, critical).await.unwrap();
    let pending = done.pending.expect("Vorschlag am verschwundenen Befund");
    assert_eq!(pending.status, FindingStatus::Done, "als „erledigt“ vorgeschlagen");
    assert_eq!(done.status, FindingStatus::Open, "nur vorgeschlagen – entscheiden kann nur der Mensch");
    assert!(
        b.server.state.store.finding_get(b.work, hint).await.unwrap().pending.is_none(),
        "der wiederkehrende Befund bleibt ohne Vorschlag"
    );
    let html = page(&b, 2, Some(2)).await;
    assert!(
        html.contains(&format!("Wiederholung von <a href=\"#post-{old}\">Beitrag {old}</a> nach Fassung 2")),
        "sichtbar „Wiederholung von Beitrag n nach Fassung m“: {html}"
    );
}

#[tokio::test]
async fn an_old_post_without_request_repeats_the_previous_human_message_or_offers_no_button() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge([Step::words("Der Wachturm steht fest.")]).await;
    add_post(&b, 1, "Autor", "Wie wirkt der Wachturm?").await;
    let old = add_post(&b, 1, "@exeget", "Gut gebaut.").await;
    let html = page(&b, 1, None).await;
    assert!(html.contains("aria-label=\"Antwort von @exeget wiederholen\""), "Rückfall: ↻ auch ohne Auftrag: {html}");

    // Ohne JavaScript: Formular an /beitraege/act, danach 303 an den neuen Beitrag.
    let form = format!("method=post.repeat&work={}&key={}&id={old}&rev=1", b.work, b.keys[1]);
    let (status, location) = b.server.post_form("/beitraege/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular → 303");
    let location = location.unwrap();
    let list = posts(&b, 1).await;
    let new = list.last().unwrap();
    assert!(location.ends_with(&format!("#post-{}", new.id)), "zurück an den neuen Beitrag: {location}");
    assert_eq!(new.author, "@exeget", "dieselbe Persona");
    assert!(all_text(&b.model.calls()[0]).contains("Wie wirkt der Wachturm?"), "die vorangehende Frage des Menschen");

    let lonely = add_post(&b, 0, "@hoerer", "Schön.").await;
    let html = page(&b, 0, None).await;
    assert!(!html.contains("Antwort von @hoerer wiederholen"), "ohne Auftrag und ohne Frage davor kein ↻: {html}");
    let refused = b.server.rpc("post.repeat", json!({ "work": b.work, "id": lonely })).await;
    assert_eq!(error_code(&refused), -32000, "ehrlich abgelehnt: {refused}");
    let human = list.iter().find(|p| p.author == "Autor").unwrap().id;
    let refused = b.server.rpc("post.repeat", json!({ "work": b.work, "id": human })).await;
    assert_eq!(error_code(&refused), -32000, "Beiträge des Menschen wiederholt niemand: {refused}");
}

// ---- Löschen --------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_delete_cross_is_visible_with_a_link_without_javascript_and_deleting_makes_one_commit() {
    let b = bridge([]).await;
    let id = add_post(&b, 2, "@lektor", "Zu löschen.").await;
    let html = page(&b, 2, None).await;
    assert!(
        html.contains(&format!(
            "href=\"/beitrag?work={}&amp;id={id}#loeschen\" data-post-delete data-work=\"{}\" aria-label=\"Beitrag löschen\"",
            b.work, b.work
        )),
        "✕ direkt sichtbar, ohne JavaScript ein Link auf die Bestätigungsseite: {html}"
    );
    let (_, single) = b.server.get(&format!("/beitrag?work={}&id={id}", b.work)).await;
    assert!(single.contains("id=\"loeschen\""), "Ziel des Links: Abschnitt „Löschen“: {single}");
    assert!(single.contains("name=\"method\" value=\"post.delete\""), "mit Formular an /beitraege/act");
    let before = commits(&b).await;
    let form = format!("method=post.delete&work={}&key={}&id={id}&confirm=1", b.work, b.keys[2]);
    assert_eq!(b.server.post_form("/beitraege/act", &form).await.0, StatusCode::SEE_OTHER, "gelöscht, 303");
    assert!(posts(&b, 2).await.iter().all(|p| p.id != id), "der Beitrag ist fort");
    assert_eq!(commits(&b).await, before + 1, "genau ein Commit");
    assert!(last_commit(&b).await.starts_with(&format!("Beitrag {id}: gelöscht")), "Lösch-Commit");
}

#[tokio::test]
async fn thread_posts_are_deleted_by_the_human_with_a_commit_and_by_mcp_only_when_own() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge([Step::words("Hallo zurück.")]).await;
    let posted = b.server.rpc("thread.post", json!({ "work": b.work, "message": "@lektor Hallo?" })).await;
    let posted = result(&posted);
    let thread = posted["thread"].as_i64().unwrap();
    let human = posted["posts"][0]["id"].as_i64().unwrap();
    let lector = posted["posts"][1]["id"].as_i64().unwrap();

    let html = b.server.get(&format!("/faden?work={}&thread={thread}", b.work)).await.1;
    assert!(html.contains("data-thread-delete") && html.contains("aria-label=\"Beitrag löschen\""), "✕ im Faden");
    assert!(html.contains("name=\"method\" value=\"thread.post_delete\""), "ohne JavaScript ein Formular");

    let unconfirmed =
        b.server.rpc("thread.post_delete", json!({ "work": b.work, "thread": thread, "id": lector })).await;
    assert_eq!(error_code(&unconfirmed), -32602, "ohne confirm abgelehnt");
    let before = commits(&b).await;
    let deleted = b
        .server
        .rpc("thread.post_delete", json!({ "work": b.work, "thread": thread, "id": lector, "confirm": true }))
        .await;
    assert_eq!(result(&deleted)["deleted"]["author"], "@lektor", "der Mensch löscht jeden Beitrag: {deleted}");
    assert_eq!(commits(&b).await, before + 1, "genau ein Commit über mirror::record_request");
    let message = last_commit(&b).await;
    assert!(message.starts_with(&format!("Fadenbeitrag {lector}: gelöscht")), "Betreff: {message}");
    assert!(message.contains("Hallo zurück."), "der Text bleibt im Protokoll wiederherstellbar: {message}");

    let mcp = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    let foreign = baruch_server::threads::delete_post_as(&b.server.state, b.work, thread, human, mcp.clone(), true)
        .await
        .unwrap_err();
    assert!(foreign.message.contains("Nur eigene"), "MCP löscht keine fremden Beiträge: {}", foreign.message);
    let own = b.server.state.store.thread_add_post(b.work, thread, "@mcp:probe", "Mein Beitrag.", &[]).await.unwrap();
    baruch_server::threads::delete_post_as(&b.server.state, b.work, thread, own.id, mcp, true)
        .await
        .expect("eigene Beiträge darf MCP löschen");

    let form = format!("method=thread.post_delete&work={}&thread={thread}&id={human}", b.work);
    let (status, location) = b.server.post_form("/faden/act", &form).await;
    assert_eq!((status, location), (StatusCode::OK, None), "ohne Kästchen: Hinweis auf derselben Seite");
    let request = Request::post("/faden/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("{form}&confirm=1")))
        .unwrap();
    assert_eq!(b.server.send(request).await.0, StatusCode::FORBIDDEN, "fremder Origin → 403");
    let still = b.server.state.store.thread_get(b.work, thread).await.unwrap();
    assert!(still.entries.iter().any(|p| p.id == human), "nach 403 ist nichts gelöscht");
    let (status, _) = b.server.post_form("/faden/act", &format!("{form}&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit Kästchen gelöscht, 303");
    let after = b.server.state.store.thread_get(b.work, thread).await.unwrap();
    assert!(after.entries.iter().all(|p| p.id != human), "der Beitrag des Menschen ist fort");
}

#[tokio::test]
async fn in_threads_addresses_relay_once_and_repeating_marks_the_new_post() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge([
        Step::words("Was trägt: der Titel.\nSchrift, prüfe bitte Ps 23,1."),
        Step::words("Die Stelle stimmt."),
        Step::words("Was trägt: der Titel trägt weiter."),
    ])
    .await;
    let posted =
        b.server.rpc("thread.post", json!({ "work": b.work, "message": "Lektor: Wie wirkt der Titel?" })).await;
    let posted = result(&posted);
    let thread = posted["thread"].as_i64().unwrap();
    let names: Vec<&str> = posted["posts"].as_array().unwrap().iter().map(|p| p["author"].as_str().unwrap()).collect();
    assert_eq!(names, ["Autor", "@lektor", "@schrift"], "Anrede ruft den Lektor, seine Anrede „Schrift,“ das Relay");
    let lector = posted["posts"][1]["id"].as_i64().unwrap();
    let relay = posted["posts"][2]["id"].as_i64().unwrap();
    let meta = b.server.state.store.thread_post_meta(b.work, &[relay]).await.unwrap().remove(&relay).unwrap();
    assert_eq!((meta.relay_from.as_deref(), meta.reply_to), (Some("@lektor"), Some(lector)), "Relay-Metadaten");
    let html = b.server.get(&format!("/faden?work={}&thread={thread}", b.work)).await.1;
    assert!(
        html.contains(&format!("antwortet auf <a href=\"#beitrag-{lector}\">Beitrag {lector}</a> von @lektor")),
        "sichtbar im Faden: {html}"
    );
    assert!(html.contains("aria-label=\"Antwort von @lektor wiederholen\""), "↻ am Persona-Beitrag: {html}");

    let repeated = b.server.rpc("thread.repeat", json!({ "work": b.work, "thread": thread, "id": lector })).await;
    let new = result(&repeated)["post"]["id"].as_i64().unwrap();
    assert_eq!(b.model.calls().len(), 3, "die Wiederholung ist ein Aufruf (ohne neues Relay)");
    assert!(all_text(&b.model.calls()[2]).contains("Wie wirkt der Titel?"), "dieselbe Nachricht des Menschen");
    let meta = b.server.state.store.thread_post_meta(b.work, &[new]).await.unwrap().remove(&new).unwrap();
    assert_eq!(meta.repeat_of, Some(lector), "repeat_of im Faden");
    let html = b.server.get(&format!("/faden?work={}&thread={thread}", b.work)).await.1;
    assert!(html.contains(&format!("Wiederholung von <a href=\"#beitrag-{lector}\">")), "sichtbar: {html}");
}

// ---- Gegliederte Beiträge und Links ---------------------------------------------------------------------------------

#[tokio::test]
async fn posts_are_set_as_safe_markdown_and_only_checked_marks_become_links() {
    let b = bridge([]).await;
    let finding = b
        .server
        .state
        .store
        .finding_insert(&NewFinding {
            work: b.work,
            key: Some(b.keys[2]),
            post: None,
            thread_post: None,
            position: 0,
            persona: "@nathan",
            kind: FindingKind::Critical,
            body: "„drei Laternen“ wirken schwer.",
            quote: "drei Laternen",
            quote_check: "",
            similar_to: None,
            summary_of: &[],
        })
        .await
        .unwrap()
        .id;
    let (k1, k2) = (b.keys[1], b.keys[2]);
    let body = format!(
        "Was trägt:\n- erster Punkt mit **fett**\n- zweiter mit [[key:{k2}|„drei Laternen“]], [[key:{k2}|„grüne Laternen“]] \
         und [[key:999999]]\n\n<script>alert(1)</script> <a href=\"javascript:alert(1)\">x</a>\nWie „Fernrohr aus Messing“ \
         oben. Siehe Ps 23,1, Einfall 7, Befund {finding} und Befund 999999.\n\n{}",
        baruch_server::scripture::MARK_OTHERS
    );
    add_post(&b, 2, "@exeget", &body).await;
    let html = page(&b, 2, None).await;
    let w = b.work;
    for (part, why) in [
        ("<h4 class=\"post-h\">Was trägt:</h4>", "Zwischenüberschrift"),
        ("<ul class=\"post-list\"><li>erster Punkt mit <strong>fett</strong></li>", "echte Liste mit fett"),
        ("&lt;script&gt;alert(1)&lt;/script&gt;", "Skript nur maskiert"),
        ("&lt;a href=&quot;javascript:alert(1)&quot;&gt;x&lt;/a&gt;", "rohes <a> nur maskiert"),
        (
            &format!(
                // Integrator Runde 30: `check_marker` (suchen-verweise) liefert beim wörtlich gefundenen Zitat auch die
                // Byte-Stelle `at`; die Sprung-Adresse trägt sie darum hinter `find`.
                "href=\"/?work={w}&amp;key={k2}&amp;rev=1&amp;find=drei%20Laternen&amp;at="
            ),
            "geprüfte Marke → Sprung",
        ),
        (
            "<span class=\"jump-unchecked\" title=\"Sprungmarke ungeprüft: das Zitat steht nicht in Abschnitt",
            "Zitat fehlt → ungeprüft",
        ),
        (
            "Sprungmarke ungeprüft: Abschnitt 999999 gibt es in diesem Werk nicht\">key:999999</span>",
            "unbekannter Schlüssel",
        ),
        (
            &format!("href=\"/?work={w}&amp;key={k1}&amp;find=Fernrohr%20aus%20Messing\" data-jump"),
            "Zitat im Werk gefunden → Sprung",
        ),
        (&format!("class=\"bible-ref\" href=\"/literatur/bibel?work={w}&amp;ref="), "Bibelstelle in die Bibelansicht"),
        ("<a class=\"idea-ref\" href=\"/einfaelle/7\"", "Einfall n → Link"),
        (&format!("<a class=\"finding-ref\" href=\"/werke/{w}/befunde?befund={finding}\""), "Befund n → Link"),
        ("<footer class=\"post-foot small muted\">", "Schlusshinweis als Fußzeile"),
        ("<pre class=\"post-plain\" hidden>Was trägt:\n- erster Punkt mit fett", "Kopieren liefert Klartext"),
    ] {
        assert!(html.contains(part), "{why}: {part} fehlt in {html}");
    }
    assert!(!html.contains("<script>alert") && !html.contains("href=\"javascript:"), "kein rohes HTML aus dem Beitrag");
    assert!(!html.contains("befund=999999"), "unbekannter Befund wird kein Link");
    assert!(!html.contains(&format!("key={k2}&amp;rev=1&amp;find=gr")), "ungeprüfte Marke wird kein Link");

    let (_, history) = b.server.get(&format!("/beitraege?work={w}&key={k2}")).await;
    assert!(history.contains("<ul class=\"post-list\">") && history.contains("data-jump"), "auch auf /beitraege");
    let id = posts(&b, 2).await[0].id;
    let (_, single) = b.server.get(&format!("/beitrag?work={w}&id={id}")).await;
    assert!(single.contains("<ul class=\"post-list\">"), "auch auf /beitrag: {single}");
    assert!(single.contains("- erster Punkt mit fett"), "das Feld zum Kopieren hat den Klartext: {single}");
    assert!(!single.contains("<script>alert"), "kein rohes HTML auf /beitrag");
}

#[tokio::test]
async fn thread_posts_are_set_as_markdown_too() {
    let b = bridge([]).await;
    let (summary, _) = b.server.state.store.thread_create(b.work, "Autor", "Liste?").await.unwrap();
    b.server
        .state
        .store
        .thread_add_post(b.work, summary.id, "@lektor", "- eins\n- zwei <b>nicht fett</b>", &[])
        .await
        .unwrap();
    let html = b.server.get(&format!("/faden?work={}&thread={}", b.work, summary.id)).await.1;
    assert!(
        html.contains("<ul class=\"post-list\"><li>eins</li><li>zwei &lt;b&gt;nicht fett&lt;/b&gt;</li></ul>"),
        "Liste im Faden, HTML maskiert: {html}"
    );
}
