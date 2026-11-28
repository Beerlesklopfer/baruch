// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 38, acceptance of the integrator: the parts of the round together.
//!
//! - One journey: a new work is German, the author sets it to English with the plain form, the exchange file carries
//!   the language to a second instance, a file as older versions wrote it becomes a German work.
//! - The interfaces speak English (MCP `initialize`, an RPC protocol error with its old code) while the user interface
//!   stays German, also for an English work.
//! - A persona of an English work is told the language rule; a German work with a German answer behaves as before.
//! - The tools of the round are in place: allow list in the documented form, CHANGELOG section, honest list of what
//!   is still German.
//!
//! No Eliezer: the model is `assist::mock::ScriptedModel`. All texts are invented.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::WorkLanguage;
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::{Role, lang};
use serde_json::{Value, json};

use crate::common;

const UMLAUTS: [char; 7] = ['ä', 'ö', 'ü', 'Ä', 'Ö', 'Ü', 'ß'];

const ORCHARD: &str = "#set document(title: \"The Orchard\", author: \"Mara Lind\")\n#set heading(numbering: \"1.1\")\n\n\
= First Frost\n\nThe apples always fall before anyone is ready.\n\n\
== Ladder\n\nA ladder leans against the oldest tree.\n";

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unexpected RPC error: {answer}");
    &answer["result"]
}

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo(path)).unwrap_or_else(|e| panic!("{path} is readable: {e}"))
}

async fn import_xml(server: &common::Server, xml: &str) -> Value {
    server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": "new" }),
        )
        .await
}

#[tokio::test]
async fn the_work_language_travels_from_the_form_through_the_exchange_file_to_a_second_instance() {
    let a = common::open().await;
    let created = a.rpc("work.create", json!({ "title": "The Orchard", "author": "Mara Lind" })).await;
    let work = result(&created)["work"]["id"].as_i64().or_else(|| result(&created)["work"].as_i64());
    let work = work.unwrap_or_else(|| panic!("work.create names the new work: {created}"));
    assert_eq!(
        result(&a.rpc("work.language", json!({ "work": work })).await)["language"],
        "de",
        "a newly created work is German: that is the default of migration 0055"
    );

    // The author switches with the plain form – no JavaScript, no RPC.
    let (status, target) = a.post_form("/einstellungen/werk/sprache", &format!("work={work}&language=en")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "the form answers with a redirect back to the page");
    assert!(
        target.as_deref().is_some_and(|t| t.ends_with("ok=sprache#sprache")),
        "the redirect leads back to the block of the work language: {target:?}"
    );
    assert_eq!(
        result(&a.rpc("work.language", json!({ "work": work })).await)["language"],
        "en",
        "what the form stored is what RPC reads"
    );

    // The exchange file carries the language to another instance.
    let exported = a.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    let xml = result(&exported)["xml"].as_str().expect("the XML export returns the file").to_owned();
    assert!(xml.contains("version=\"10\""), "the file is of sub-version 10");
    assert!(xml.contains("<angaben sprache=\"en\""), "the work language stands at <angaben>");
    let b = common::open().await;
    let imported = import_xml(&b, &xml).await;
    let work_b = result(&imported)["work"].as_i64().unwrap();
    assert_eq!(
        b.state.store.work_language(work_b).await.unwrap(),
        WorkLanguage::En,
        "the second instance creates the work in the language of the file"
    );
    let again = b.rpc("work.exchange_export", json!({ "work": work_b, "format": "xml" })).await;
    assert!(
        result(&again)["xml"].as_str().unwrap().contains("<angaben sprache=\"en\""),
        "and writes it out again: the round trip keeps the language"
    );

    // A file as every version up to sub-version 9 wrote it: no attribute → German work.
    let old = xml.replacen("version=\"10\"", "version=\"9\"", 1).replacen(" sprache=\"en\"", "", 1);
    assert!(!old.contains("sprache=\"en\"") && old.contains("version=\"9\""), "precondition: an older file");
    let c = common::open().await;
    let imported = import_xml(&c, &old).await;
    let work_c = result(&imported)["work"].as_i64().unwrap();
    assert_eq!(
        c.state.store.work_language(work_c).await.unwrap(),
        WorkLanguage::De,
        "an older exchange file has no work language and becomes a German work"
    );

    // A value outside de/en is refused as a whole, nothing is created.
    let broken = xml.replacen(" sprache=\"en\"", " sprache=\"fr\"", 1);
    let d = common::open().await;
    let before = result(&d.rpc("work.list", json!({})).await)["works"].as_array().unwrap().len();
    let refused = import_xml(&d, &broken).await;
    let message = refused["error"]["message"].as_str().unwrap_or_else(|| panic!("an error is expected: {refused}"));
    assert!(message.contains("@sprache") && message.contains("„fr“"), "the error names attribute and value: {message}");
    assert_eq!(
        result(&d.rpc("work.list", json!({})).await)["works"].as_array().unwrap().len(),
        before,
        "the refused file created no work"
    );
}

#[tokio::test]
async fn the_interfaces_speak_english_and_the_user_interface_stays_german() {
    let server = common::open().await;
    let work = server.import(ORCHARD, "orchard.typ").await.work;
    server.state.store.set_work_language(work, WorkLanguage::En).await.unwrap();

    // MCP: instructions and tool descriptions.
    let mcp = |message: Value| {
        let mut request = Request::post("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(message.to_string()))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40038))));
        request
    };
    let (status, _, body) = server
        .send(mcp(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": { "name": "abnahme38", "version": "1" } } })))
        .await;
    assert_eq!(status, StatusCode::OK, "a loopback client may initialize without accounts: {body}");
    let init: Value = serde_json::from_str(&body).unwrap();
    let instructions = init["result"]["instructions"].as_str().expect("initialize returns instructions");
    assert!(!instructions.contains(UMLAUTS), "the instructions carry no German letter: {instructions}");
    assert!(
        instructions.contains(" the ") && instructions.contains("propose_revision"),
        "they are English prose and still name the tools: {instructions}"
    );
    let (_, _, body) = server.send(mcp(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }))).await;
    let list: Value = serde_json::from_str(&body).unwrap();
    let tools = list["result"]["tools"].as_array().expect("tools/list returns the tools");
    let german: Vec<&str> =
        tools.iter().filter(|t| t.to_string().contains(UMLAUTS)).filter_map(|t| t["name"].as_str()).collect();
    assert!(german.is_empty(), "no tool describes itself with German letters: {german:?}");
    let outline = tools.iter().find(|t| t["name"] == "outline").expect("the tool outline exists as before");
    assert!(
        outline["description"].as_str().is_some_and(|d| d.contains("outline") || d.contains("Outline")),
        "the description of outline is English: {outline}"
    );

    // RPC: a protocol error is English, its code is the old one.
    let unknown = server.rpc("werk.gibt_es_nicht", json!({})).await;
    assert_eq!(unknown["error"]["code"], -32601, "the code of an unknown method did not change: {unknown}");
    assert_eq!(unknown["error"]["message"], "unknown method werk.gibt_es_nicht", "the text is English");

    // The user interface is German as before, although the work is English.
    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "the settings page renders without accounts");
    assert!(page.contains("<html lang=\"de\""), "the page declares German");
    for needle in ["<h3>Werksprache</h3>", "<option value=\"en\" selected>Englisch</option>", ">Speichern</button>"] {
        assert!(page.contains(needle), "the settings page shows the German text {needle}");
    }
    let (status, page) = server.get(&format!("/?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "the work page renders without accounts");
    assert!(page.contains("<html lang=\"de\""), "the work page of an English work still declares German");
    assert!(page.contains("Einstellungen"), "and keeps its German menu");
    assert!(page.contains("The apples always fall"), "the text of the work is shown unchanged");

    // A message that the user interface shows as well stays German over RPC (boundary of contract §4.4).
    let refused = server.rpc("work.create", json!({ "title": "The Orchard" })).await;
    let message = refused["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("a second work of the same title is refused: {refused}"));
    assert!(
        message.starts_with("Es gibt schon ein Werk"),
        "the refusal of a duplicate title is the German sentence the form shows too: {message}"
    );
}

#[tokio::test]
async fn a_persona_is_told_the_work_language_and_a_german_work_behaves_as_before() {
    let _turn = common::model_queue(module_path!()).await;
    let open = |steps: Vec<Step>| async move {
        let model = ScriptedModel::with(steps);
        let data = tempfile::tempdir().unwrap();
        let state = baruch_server::AppState::open_with(
            data.path(),
            Arc::new(baruch_server::connections::MemorySecrets::default()),
            Arc::new(model.clone()),
        )
        .await
        .expect("the data directory opens");
        let state = Arc::new(state);
        let server = common::Server { app: common::router(state.clone()), state, data };
        let work = server.import(ORCHARD, "orchard.typ").await.work;
        let key = server.state.store.load_work(work).await.unwrap().nodes[0].key;
        (server, model, work, key)
    };
    let system = |call: &baruch_server::assist::ModelCall| {
        call.messages.iter().find(|m| m.role == Role::System).map(|m| m.content.clone()).unwrap()
    };

    // German work, German answer with finding and solution: one call, stored literally, read as a critical finding.
    let german =
        "Kritisch: „always fall“ behauptet mehr, als der Abschnitt zeigt.\n- statt: „always“\n- besser: „often“";
    let (server, model, work, key) = open(vec![Step::words(german)]).await;
    let answer = server.rpc("persona.review", json!({ "work": work, "key": key, "persona": "@nathan" })).await;
    assert_eq!(result(&answer)["post"]["body"], german, "the German answer is stored without any note of Baruch");
    let calls = model.calls();
    assert_eq!(calls.len(), 1, "finding and solution are recognised at once: no retry");
    assert!(
        system(&calls[0]).contains(&lang::language_rule(WorkLanguage::De)),
        "the prompt of a German work carries the rule to answer in German"
    );
    assert!(system(&calls[0]).contains("\"Kritisch:\""), "and prescribes the German marker");
    let findings = server.rpc("finding.list", json!({ "work": work })).await;
    assert!(
        result(&findings)["findings"].as_array().is_some_and(|f| f.iter().any(|f| f["kind"] == "critical")),
        "the findings parser reads „Kritisch:“ as before: {findings}"
    );

    // The same work set to English through RPC: the next call carries the English rule and markers.
    let (server, model, work, key) = open(vec![Step::words(
        "Critical: \"always fall\" claims more than the section shows.\n- instead of: \"always\"\n- better: \"often\"",
    )])
    .await;
    result(&server.rpc("work.language_set", json!({ "work": work, "language": "en" })).await);
    result(&server.rpc("persona.review", json!({ "work": work, "key": key, "persona": "@nathan" })).await);
    let calls = model.calls();
    assert_eq!(calls.len(), 1, "the English finding with its solution needs no retry");
    let prompt = system(&calls[0]);
    assert!(
        prompt.contains(&lang::language_rule(WorkLanguage::En)) && prompt.contains("Always answer in English"),
        "the prompt of an English work carries the rule to answer in English: {prompt}"
    );
    assert!(
        prompt.contains("\"Critical:\"") && !prompt.contains("Kritisch:"),
        "and names the English marker only: {prompt}"
    );
}

#[test]
fn the_tools_and_records_of_the_round_are_in_place() {
    // Allow list: every entry has the three documented fields and names a reason.
    let allow = read("scripts/comment-umlaut-allow.txt");
    let entries: Vec<&str> = allow.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).collect();
    assert!(!entries.is_empty(), "the round reported exceptions; the allow list carries them");
    for entry in &entries {
        let fields: Vec<&str> = entry.split('\t').collect();
        assert_eq!(fields.len(), 3, "an entry is glob, substring and reason, separated by TAB: {entry:?}");
        assert!(fields.iter().all(|f| !f.trim().is_empty()), "no field of an entry is empty: {entry:?}");
        assert!(!fields[2].contains(UMLAUTS), "the reason is written in English: {entry:?}");
    }
    assert!(
        read("scripts/ci-selftest.sh").contains("check-comments-only.py --selftest"),
        "the self-test of the comment check runs with the CI self-test"
    );

    // CHANGELOG: the section of the round names every part, under the heading the release tests look for.
    let changelog = read("CHANGELOG.md");
    let unreleased = changelog.split("## Unveröffentlicht").nth(1).expect("the heading of unreleased changes stays");
    let section = unreleased
        .split("### English as the primary language (Round 38)")
        .nth(1)
        .expect("the section of Round 38 stands under the unreleased changes");
    let section = section.split("\n### ").next().unwrap();
    for needle in ["Work language", "`work.language_set`", "sub-version 10", "MCP server", "Comments", "still German"] {
        assert!(section.contains(needle), "the CHANGELOG section of Round 38 names: {needle}");
    }

    // Honest record: the backlog names what the round did not finish.
    let backlog = read("docs/Backlog.md");
    for needle in
        ["SKILLS.md", "docs/MCP.md", "docs/RPC.md", "kommentare_store", "kommentare_rest", "assist/context.rs"]
    {
        assert!(backlog.contains(needle), "docs/Backlog.md names the open part of Round 38: {needle}");
    }
    assert!(
        read("crates/baruch-store/migrations/0055_work_language.sql").contains("CHECK (language IN ('de', 'en'))"),
        "migration 0055 is the one of the contract"
    );
    assert!(
        !repo("crates/baruch-store/migrations")
            .read_dir()
            .unwrap()
            .flatten()
            .any(|e| { e.file_name().to_string_lossy().starts_with("0056") }),
        "0056 was reserved for the integrator and stays unused"
    );
}
