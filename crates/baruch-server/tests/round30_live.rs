// SPDX-License-Identifier: AGPL-3.0-or-later
//! Live in the chat (Round 30c): post.since, „schreibt …“, errors of failed model calls, thread.since and the pure
//! functions of the widgets (chat.ts, mention.ts) under Node.
//!
//! Owner: chat-bedienung. No network, no Eliezer: models only through `ScriptedModel`. The display „schreibt …“ and the
//! errors are process-wide (per work and section); assertions therefore use `find`/`any` on own entries, never on the
//! whole list (SKILLS.md).

use crate::common;

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::port::ModelFailure;
use serde_json::{Value, json};

const LIGHT: &str = "#set document(title: \"Das Leuchtfeuer\", author: \"Ida Lorenz\")\n\n\
= Das Feuer\n\nDas Feuer brennt oben im Turm.\n\n\
== Die Wache\n\nDer Wächter zählt die Schiffe.\n";

struct Light {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    keys: Vec<i64>,
}

async fn light(model: ScriptedModel) -> Light {
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
    let report = server.import(LIGHT, "leuchtfeuer.typ").await;
    let keys = server.state.store.load_work(report.work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Light { server, model, work: report.work, keys }
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

#[tokio::test]
async fn post_since_shows_who_is_typing_and_then_delivers_the_new_post_as_html() {
    let _turn = common::model_queue(module_path!()).await;
    let model = ScriptedModel::with([Step::Wait {
        before: vec![],
        then: Box::new(Step::words("- Das Feuer trägt.\n- Der Wächter wirkt **müde**.")),
    }]);
    let l = light(model).await;
    let (work, key) = (l.work, l.keys[1]);
    let app = l.server.app.clone();
    let running = tokio::spawn(async move {
        common::rpc(app, "persona.chat", json!({ "work": work, "key": key, "message": "@lektor Wie wirkt die Wache?" }))
            .await
    });
    l.model.started().await;
    // The post of the human is there immediately; the lector is still writing.
    let mut seen = None;
    for _ in 0..200 {
        let since = l.server.rpc("post.since", json!({ "work": work, "key": key, "after": 0 })).await;
        let since = result(&since).clone();
        if let Some(entry) = since["typing"].as_array().unwrap().iter().find(|t| t["persona"] == "@lektor") {
            seen = Some((since.clone(), entry["since"].as_str().unwrap().to_owned()));
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let (during, started) = seen.expect("während des Auftrags meldet post.since „@lektor schreibt …“");
    let human = during["posts"].as_array().unwrap();
    assert_eq!(human.len(), 1, "nur der Beitrag des Menschen ist schon da: {during}");
    assert!(human[0]["html"].as_str().unwrap().contains("Wie wirkt die Wache?"), "als HTML des Makros: {during}");
    let human_id = human[0]["id"].as_i64().unwrap();

    l.model.release();
    let answer = running.await.unwrap();
    assert!(answer.get("error").is_none(), "Chat gelingt: {answer}");
    let after = l.server.rpc("post.since", json!({ "work": work, "key": key, "after": human_id })).await;
    let after = result(&after);
    let posts = after["posts"].as_array().unwrap();
    assert_eq!(posts.len(), 1, "nur der neue Beitrag nach dem des Menschen: {after}");
    assert_eq!(posts[0]["author"], "@lektor", "die Antwort des Lektors");
    let html = posts[0]["html"].as_str().unwrap();
    for part in [
        &format!("id=\"post-{}\"", posts[0]["id"]),
        // Integrator Round 30: with the real `finding_spans` (chat-loesungen) every list item of the answer is a
        // finding block of its own (segment `finding`), still rendered as a list inside it.
        "<ul class=\"post-list\"><li>Der Wächter wirkt <strong>müde</strong>.</li></ul>",
        "class=\"post-finding\" data-finding=",
        "<div class=\"fb fb-kind-hint",
        "aria-label=\"Beitrag löschen\"",
        "aria-label=\"Antwort von @lektor wiederholen\"",
        "geprüft: aktuelle Fassung 1",
    ] {
        assert!(html.contains(part), "{part} fehlt im Beitrag (wie im Abschnitts-Chat): {html}");
    }
    assert!(
        !after["typing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["persona"] == "@lektor" && t["since"] == started.as_str()),
        "nach dem Auftrag ist der Lektor ausgetragen: {after}"
    );
    let none = l.server.rpc("post.since", json!({ "work": work, "key": key, "after": posts[0]["id"] })).await;
    assert!(result(&none)["posts"].as_array().unwrap().is_empty(), "danach nichts Neues");
}

#[tokio::test]
async fn a_failed_model_call_is_reported_by_post_since() {
    let _turn = common::model_queue(module_path!()).await;
    let model = ScriptedModel::with([Step::Fail(ModelFailure::Failed("Leuchtfeuer-Probe: Modell ausgefallen".into()))]);
    let l = light(model).await;
    let answer = l
        .server
        .rpc("persona.chat", json!({ "work": l.work, "key": l.keys[0], "message": "@lektor Brennt das Feuer?" }))
        .await;
    assert_eq!(result(&answer)["errors"][0]["persona"], "@lektor", "der Chat meldet den Fehler: {answer}");
    let since = l.server.rpc("post.since", json!({ "work": l.work, "key": l.keys[0], "after": 0 })).await;
    let failures = result(&since)["failures"].as_array().unwrap().clone();
    let failure = failures
        .iter()
        .find(|f| f["message"].as_str().unwrap_or_default().contains("Leuchtfeuer-Probe"))
        .unwrap_or_else(|| panic!("Fehler des Modellaufrufs fehlt: {failures:?}"));
    assert_eq!(failure["persona"], "@lektor", "mit Persona");
    assert!(failure["at"].as_str().unwrap().ends_with('Z'), "mit Zeit (der Browser zeigt jeden einmal)");
}

#[tokio::test]
async fn thread_since_delivers_new_thread_posts_as_html() {
    let _turn = common::model_queue(module_path!()).await;
    let l = light(ScriptedModel::with([Step::words("Die Wache hält.")])).await;
    let posted = l.server.rpc("thread.post", json!({ "work": l.work, "message": "@lektor Hält die Wache?" })).await;
    let posted = result(&posted);
    let thread = posted["thread"].as_i64().unwrap();
    let first = posted["posts"][0]["id"].as_i64().unwrap();
    let since = l.server.rpc("thread.since", json!({ "work": l.work, "thread": thread, "after": first })).await;
    let since = result(&since);
    let posts = since["posts"].as_array().unwrap();
    assert_eq!(posts.len(), 1, "die Antwort nach dem ersten Beitrag: {since}");
    let html = posts[0]["html"].as_str().unwrap();
    assert!(html.contains(&format!("id=\"beitrag-{}\"", posts[0]["id"])), "Anker des Fadens: {html}");
    assert!(html.contains("Die Wache hält."), "der Text: {html}");
    assert!(since["typing"].is_array() && since["failures"].is_array(), "Felder typing und failures: {since}");
}

/// Calls an expression with the delivered modules under Node (all scripts in the same directory so that the imports
/// resolve) and returns the result as JSON.
fn node(module: &str, expr: &str) -> Value {
    let dir = tempfile::tempdir().unwrap();
    for (name, source) in baruch_server::SCRIPTS {
        std::fs::write(dir.path().join(name.replace(".js", ".mjs")), source.replace(".js\"", ".mjs\"")).unwrap();
    }
    let script =
        format!("import * as m from './{}';\nconsole.log(JSON.stringify({expr}));\n", module.replace(".js", ".mjs"));
    std::fs::write(dir.path().join("probe.mjs"), script).unwrap();
    let out = Command::new("node").arg("probe.mjs").current_dir(dir.path()).output().expect("node läuft (wie für tsc)");
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn the_live_line_names_who_is_typing_and_shows_each_failure_once() {
    assert_eq!(node("chat.js", "m.typingLine([{persona: '@lektor'}])"), json!("@lektor schreibt …"), "eine Persona");
    assert_eq!(
        node("chat.js", "m.typingLine([{persona: '@lektor'}, {persona: '@schrift'}, {persona: '@lektor'}])"),
        json!("@lektor und @schrift schreiben …"),
        "mehrere ohne Doppelte"
    );
    assert_eq!(node("chat.js", "m.typingLine([])"), json!(""), "niemand schreibt: leer (Anzeige verborgen)");
    assert_eq!(
        node(
            "chat.js",
            "(() => { const seen = new Set(); const f = [{persona: '@a', at: '1', message: 'x'}]; \
             return [m.freshFailures(f, seen).length, m.freshFailures(f, seen).length]; })()"
        ),
        json!([1, 0]),
        "jeder Fehler erscheint nur einmal"
    );
    assert_eq!(
        node("chat.js", "m.deleteQuestion(7, '@lektor')"),
        json!("Beitrag 7 von @lektor löschen? Der Spiegel hält ihn im Lösch-Commit fest."),
        "Rückfrage vor dem Löschen nennt Beitrag und Autor"
    );
}

#[test]
fn the_completion_offers_the_address_with_name_at_the_start_of_a_sentence() {
    let personas = "[{handle: '@lektor', name: 'Lektor', role: '', style: ''}, {handle: '@exeget', name: 'Exeget', role: '', style: ''}]";
    assert_eq!(node("mention.js", "m.addressAt('Le', 2)"), json!({ "start": 0, "query": "Le" }), "Beitragsanfang");
    assert_eq!(
        node("mention.js", "m.addressAt('Gut. Ex', 7)"),
        json!({ "start": 5, "query": "Ex" }),
        "Satzanfang nach dem Punkt"
    );
    assert_eq!(node("mention.js", "m.addressAt('Das Le', 6)"), Value::Null, "im Fließtext keine Anrede");
    assert_eq!(node("mention.js", "m.addressAt('L', 1)"), Value::Null, "erst ab zwei Buchstaben");
    assert_eq!(
        node("mention.js", &format!("m.filterAddresses({personas}, 'lek').map(p => p.handle)")),
        json!(["@lektor"]),
        "Name ohne Groß/klein"
    );
    assert_eq!(
        node("mention.js", "m.applyAddress('Le was meinst du?', 0, 2, 'Lektor')"),
        json!({ "text": "Lektor, was meinst du?", "caret": 8 }),
        "Anrede „Lektor, “ übernommen"
    );
    assert_eq!(
        node("mention.js", "m.applyMention('@le', 0, 3, '@lektor')"),
        json!({ "text": "@lektor ", "caret": 8 }),
        "die Form @handle bleibt wie bisher"
    );
}
