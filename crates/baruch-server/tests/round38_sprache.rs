// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 38 (task mcp_rpc): work language end to end and English for MCP and RPC.
//!
//! - RPC `work.language` / `work.language_set`, `language` in `work.get`, `work.meta`, `work.list`; the form
//!   `POST /einstellungen/werk/sprache` without JavaScript, behind `same_origin`; both closed for a lector.
//! - MCP: `list_works` and `outline` carry `language`; `initialize` and `tools/list` contain no umlauts.
//! - Prompts: English with the language rule; the markers follow the work language; the recognition takes German and
//!   English markers whatever the work language is.
//!
//! No Eliezer: the model is `assist::mock::ScriptedModel`. All texts are invented.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::{WorkKind, WorkLanguage};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::{Role, lang, prompts};
use serde_json::{Value, json};

use crate::common;
use crate::round35_rollen::{code, denied_by_role, form_as, location, rpc_as, stage};

const PIER: &str = "#set document(title: \"The Pier\", author: \"Tilda Rohr\")\n#set heading(numbering: \"1.1\")\n\n\
= Low Tide\n\nThe rope always creaks when the ferry leaves.\n\n\
== Lantern\n\nA lantern hangs at the end of the pier.\n";

struct Pier {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    keys: Vec<i64>,
}

async fn pier(steps: impl IntoIterator<Item = Step>) -> Pier {
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
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let work = server.import(PIER, "pier.typ").await.work;
    let keys = server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Pier { server, model, work, keys }
}

fn system(call: &baruch_server::assist::ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::System).map(|m| m.content.as_str()).unwrap()
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unexpected RPC error: {answer}");
    &answer["result"]
}

async fn mcp(server: &common::Server, message: Value) -> Value {
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "the MCP endpoint answers a loopback client: {body}");
    serde_json::from_str(&body).unwrap()
}

async fn tool(server: &common::Server, name: &str, arguments: Value) -> Value {
    let answer = mcp(
        server,
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": name, "arguments": arguments } }),
    )
    .await;
    assert_eq!(answer["result"]["isError"], false, "{name} succeeds: {answer}");
    answer["result"]["structuredContent"].clone()
}

const UMLAUTS: [char; 7] = ['ä', 'ö', 'ü', 'Ä', 'Ö', 'Ü', 'ß'];

#[tokio::test]
async fn the_work_language_is_read_and_set_over_rpc_and_shown_by_the_work_methods() {
    let server = common::open().await;
    let work = server.import(PIER, "pier.typ").await.work;

    let read = server.rpc("work.language", json!({ "work": work })).await;
    assert_eq!(result(&read), &json!({ "work": work, "language": "de" }), "an imported work starts German");

    let set = server.rpc("work.language_set", json!({ "work": work, "language": "en" })).await;
    assert_eq!(
        result(&set),
        &json!({ "work": work, "language": "en", "changed": true }),
        "setting another language answers with the new language and changed: true"
    );
    let again = server.rpc("work.language_set", json!({ "work": work, "language": "en" })).await;
    assert_eq!(result(&again)["changed"], false, "the same language again changes nothing and says so");

    for bad in [json!("fr"), json!("EN"), json!(" en"), json!(""), json!(7), Value::Null] {
        let answer = server.rpc("work.language_set", json!({ "work": work, "language": bad })).await;
        assert_eq!(code(&answer), Some(-32602), "{bad}: only the exact codes de and en are valid parameters: {answer}");
        assert_eq!(
            answer["error"]["message"], "language: allowed values are \"de\" and \"en\"",
            "the message names the allowed values, in English (protocol error)"
        );
    }
    let missing = server.rpc("work.language_set", json!({ "language": "de" })).await;
    assert_eq!(code(&missing), Some(-32602), "work.language_set demands the work explicitly: {missing}");
    assert_eq!(
        result(&server.rpc("work.language", json!({ "work": work })).await)["language"],
        "en",
        "the refused calls changed nothing"
    );

    let get = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(result(&get)["language"], "en", "work.get carries the language beside work, lector and cover");
    let meta = server.rpc("work.meta", json!({ "work": work })).await;
    assert_eq!(result(&meta)["work"]["language"], "en", "work.meta carries the language in the work data");
    let list = server.rpc("work.list", json!({})).await;
    let listed = result(&list)["works"].as_array().unwrap();
    let languages: Vec<(i64, &str)> =
        listed.iter().map(|w| (w["id"].as_i64().unwrap(), w["language"].as_str().unwrap())).collect();
    assert!(languages.contains(&(work, "en")), "work.list carries the language of the English work: {languages:?}");
    assert!(
        languages.iter().filter(|(id, _)| *id != work).all(|(_, language)| *language == "de"),
        "the sample work stays German: {languages:?}"
    );

    let (status, _) = server.post_form("/act", &format!("method=work.language_set&work={work}&language=de")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "/act does not know work.language_set (only the form of its own)");
    assert_eq!(
        server.state.store.work_language(work).await.unwrap(),
        WorkLanguage::En,
        "and the refused /act form changed nothing"
    );
}

#[tokio::test]
async fn the_settings_page_offers_the_language_and_the_form_saves_it_without_javascript() {
    let server = common::open().await;
    let work = server.import(PIER, "pier.typ").await.work;

    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "the settings page of the work renders");
    let block = page.split("id=\"sprache\"").nth(1).expect("the block id=\"sprache\" is on the page");
    let block = block.split("id=\"zeit\"").next().expect("the block stands directly before id=\"zeit\"");
    assert!(block.contains("<h3>Werksprache</h3>"), "German heading of the block: {block}");
    assert!(
        block.contains("<form method=\"post\" action=\"/einstellungen/werk/sprache\">"),
        "a plain form, usable without JavaScript: {block}"
    );
    assert!(
        block.contains(&format!("<input type=\"hidden\" name=\"work\" value=\"{work}\">")),
        "the form names the work: {block}"
    );
    assert!(
        block.contains("<option value=\"de\" selected>Deutsch</option>")
            && block.contains("<option value=\"en\">Englisch</option>"),
        "the select offers Deutsch and Englisch, the current language (German) is selected: {block}"
    );
    assert!(block.contains(">Speichern</button>"), "the button is labelled Speichern: {block}");
    assert!(
        block.contains(
            "In dieser Sprache antworten die Assistenten. Die Sprache der Oberfläche ändert sich dadurch nicht."
        ),
        "the sentence explains what the work language does and does not change: {block}"
    );
    assert!(
        page.contains("<h3>Predigtzeit</h3>") && page.contains("Gliederungsschema"),
        "the user interface stays German: the neighbouring blocks keep their German headings"
    );

    let (status, target) = server.post_form("/einstellungen/werk/sprache", &format!("work={work}&language=en")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "the form answers with a redirect (303)");
    assert_eq!(
        target.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=sprache#sprache").as_str()),
        "back to the block with the notice code"
    );
    assert_eq!(server.state.store.work_language(work).await.unwrap(), WorkLanguage::En, "the language is stored");
    let (_, page) = server.get(&format!("/einstellungen/werk?work={work}&ok=sprache")).await;
    assert!(page.contains("Die Werksprache ist gespeichert."), "the page confirms in German");
    assert!(
        page.contains("<option value=\"en\" selected>Englisch</option>")
            && page.contains("<option value=\"de\">Deutsch</option>"),
        "now Englisch is selected"
    );

    let (status, target) = server.post_form("/einstellungen/werk/sprache", &format!("work={work}&language=fr")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "an invalid value also redirects");
    assert_eq!(
        target.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&sprachfehler=1#sprache").as_str()),
        "back to the block with the error code"
    );
    assert_eq!(server.state.store.work_language(work).await.unwrap(), WorkLanguage::En, "nothing changed");
    let (_, page) = server.get(&format!("/einstellungen/werk?work={work}&sprachfehler=1")).await;
    assert!(page.contains("Die Werksprache ließ sich nicht speichern"), "the page shows the German error in the block");

    // Behind same_origin: a form posted from a foreign web page is refused before the handler runs.
    let request = Request::post("/einstellungen/werk/sprache")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::from(format!("work={work}&language=de")))
        .unwrap();
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a foreign origin must not change the work language");
    assert_eq!(server.state.store.work_language(work).await.unwrap(), WorkLanguage::En, "and it did not");
}

#[tokio::test]
async fn a_lector_can_neither_read_nor_set_the_language_and_the_route_is_closed() {
    let s = stage().await;
    let server = &s.server;
    for (method, params) in [
        ("work.language", json!({ "work": s.granted })),
        ("work.language_set", json!({ "work": s.granted, "language": "en" })),
    ] {
        let answer = rpc_as(server, &s.erika, method, params.clone()).await;
        assert!(
            answer.get("error").is_some(),
            "{method} is not in LEKTOR_RPC: a lector is refused even for a work granted to her: {answer}"
        );
        let answer = rpc_as(server, &s.author, method, params).await;
        assert!(answer.get("error").is_none(), "{method}: the author account may call it: {answer}");
    }
    assert_eq!(
        server.state.store.work_language(s.granted).await.unwrap(),
        WorkLanguage::En,
        "precondition for the next step: the author's call set English"
    );
    let (status, headers, _) =
        form_as(server, &s.erika, "/einstellungen/werk/sprache", &format!("work={}&language=de", s.granted)).await;
    assert!(
        denied_by_role(&headers) || status == StatusCode::FORBIDDEN,
        "the route is not in LEKTOR_ROUTES: {status} {}",
        location(&headers)
    );
    assert_eq!(
        server.state.store.work_language(s.granted).await.unwrap(),
        WorkLanguage::En,
        "the lector's form changed nothing"
    );
}

#[tokio::test]
async fn mcp_speaks_english_and_carries_the_work_language() {
    let server = common::open().await;
    let work = server.import(PIER, "pier.typ").await.work;
    server.state.store.set_work_language(work, WorkLanguage::En).await.unwrap();

    let init = mcp(
        &server,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "round38", "version": "1" } } }),
    )
    .await;
    let instructions = init["result"]["instructions"].as_str().expect("initialize returns instructions");
    assert!(
        !instructions.contains(UMLAUTS),
        "the instructions are English: no umlaut or sharp s. First offender: {:?}",
        instructions.char_indices().find(|(_, c)| UMLAUTS.contains(c))
    );
    for needle in ["only the human can accept", "propose_revision", "Work language", "See docs/MCP.md."] {
        assert!(instructions.contains(needle), "the instructions still state the rule or name: {needle}");
    }

    let list = mcp(&server, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tools = list["result"]["tools"].as_array().expect("tools/list returns the tools");
    assert!(tools.len() > 60, "all tools are listed: {}", tools.len());
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let text = tool.to_string();
        assert!(
            !text.contains(UMLAUTS),
            "{name}: description and parameters are English, without umlaut or sharp s: {:?}",
            text.chars()
                .collect::<Vec<_>>()
                .windows(30)
                .find(|w| w.iter().any(|c| UMLAUTS.contains(c)))
                .map(|w| w.iter().collect::<String>())
        );
        assert!(tool["description"].as_str().is_some_and(|d| d.len() > 20), "{name} keeps a real description");
    }
    for request_tool in
        ["outline_rename", "outline_move", "outline_delete", "asset_replace", "asset_delete", "cover_layout_set"]
    {
        let description = tools.iter().find(|t| t["name"] == request_tool).unwrap()["description"].as_str().unwrap();
        assert!(
            description.starts_with("Request:"),
            "{request_tool} changes nothing immediately and says so first: {description}"
        );
    }

    let works = tool(&server, "list_works", json!({})).await;
    let languages: Vec<(i64, &str)> = works["works"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| (w["id"].as_i64().unwrap(), w["language"].as_str().expect("every work carries language")))
        .collect();
    assert!(languages.contains(&(work, "en")), "list_works names the language of the English work: {languages:?}");
    assert!(languages.iter().any(|(_, language)| *language == "de"), "and of the German sample work");
    let outline = tool(&server, "outline", json!({ "work": work })).await;
    assert_eq!(outline["language"], "en", "outline carries the language at the top level");
    assert!(outline["nodes"].as_array().is_some_and(|n| n.len() == 2), "beside the nodes as before");

    // Protocol errors are English, the codes unchanged.
    let unknown = mcp(
        &server,
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "no_such_tool" } }),
    )
    .await;
    assert_eq!(unknown["error"]["code"], -32602, "an unknown tool stays invalid params");
    assert_eq!(unknown["error"]["message"], "unknown tool no_such_tool", "with an English message");
    let method = mcp(&server, json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/remove" })).await;
    assert_eq!(method["error"]["code"], -32601, "an unknown method stays method not found");
    assert_eq!(method["error"]["message"], "unknown method tools/remove", "with an English message");
    let node = mcp(
        &server,
        json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call",
                "params": { "name": "add_post", "arguments": { "work": work, "node": 0, "body": "x" } } }),
    )
    .await;
    assert_eq!(node["error"]["code"], -32602, "writing tools still refuse node");
    assert!(
        node["error"]["message"].as_str().is_some_and(|m| m.contains("node (position) is not accepted; pass key")),
        "and say in English what to pass instead: {node}"
    );
    // No tool sets the language: it is reserved for the human.
    assert!(
        tools.iter().all(|t| !t["name"].as_str().unwrap().contains("language")),
        "MCP has no tool for the work language"
    );
}

#[tokio::test]
async fn rpc_protocol_errors_are_english_and_keep_their_codes() {
    let server = common::open().await;
    let unknown = server.rpc("no.such_method", json!({})).await;
    assert_eq!(code(&unknown), Some(-32601), "unknown method: code unchanged");
    assert_eq!(unknown["error"]["message"], "unknown method no.such_method", "English message");

    let raw = |body: &'static str| {
        Request::post("/rpc").header(header::CONTENT_TYPE, "application/json").body(Body::from(body)).unwrap()
    };
    let (_, _, text) = server.send(raw("{ not json")).await;
    let parse: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(code(&parse), Some(-32700), "unreadable JSON stays a parse error: {parse}");
    assert!(
        !parse["error"]["message"].as_str().unwrap().contains(UMLAUTS),
        "the parse error carries no German text: {parse}"
    );
    let (_, _, text) = server.send(raw("7")).await;
    let invalid: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(code(&invalid), Some(-32600), "a number is no request: {invalid}");
    let wrong = server.rpc("work.meta", json!({ "work": "the first" })).await;
    assert_eq!(code(&wrong), Some(-32602), "a parameter of the wrong type stays invalid params: {wrong}");
    assert!(
        wrong["error"]["message"].as_str().is_some_and(|m| !m.is_empty() && !m.contains(UMLAUTS)),
        "the parameter check answers without German text: {wrong}"
    );
    let key = server.rpc("node.get", json!({ "key": "first" })).await;
    assert_eq!(code(&key), Some(-32602), "a key that is no number stays invalid params: {key}");
    assert_eq!(key["error"]["message"], "key: stable key (number) expected", "with an English message");
}

#[tokio::test]
async fn the_prompt_prescribes_the_language_and_the_markers_of_the_work() {
    // Unit level: default prompts of every known persona in both languages.
    for handle in ["@lektor", "@exeget", "@nathan", "@hoerer", "@schrift", "@belege", "@eigene"] {
        for kind in [WorkKind::Book, WorkKind::Sermon] {
            let en = prompts::effective_prompt_in(handle, "Own role", None, kind, WorkLanguage::En);
            let de = prompts::effective_prompt_in(handle, "Own role", None, kind, WorkLanguage::De);
            assert!(en.ends_with(&lang::language_rule(WorkLanguage::En)), "{handle}: the English rule stands last");
            assert!(de.ends_with(&lang::language_rule(WorkLanguage::De)), "{handle}: the German rule stands last");
            assert!(en.starts_with("You are @"), "{handle}: the prompt itself is English: {en}");
            assert!(de.starts_with("You are @"), "{handle}: also for a German work the prompt text is English: {de}");
            assert!(
                en.contains("\"Critical: …\"") && en.contains("- instead of: ") && en.contains("- better: "),
                "{handle}: an English work prescribes the English markers of the solution duty: {en}"
            );
            assert!(
                !en.contains("Kritisch:") && !en.contains("statt:") && !en.contains("besser:"),
                "{handle}: an English work names no German marker: {en}"
            );
            assert!(
                de.contains("\"Kritisch: …\"") && de.contains("- statt: ") && de.contains("- besser: "),
                "{handle}: a German work prescribes the German markers: {de}"
            );
            assert!(
                !de.contains("Critical:") && !de.contains("instead of:"),
                "{handle}: a German work names no English marker: {de}"
            );
        }
    }
    let lector = prompts::default_prompt_in("@lektor", "", WorkKind::Book, WorkLanguage::En);
    for marker in
        ["\"What works:\"", "\"Suggestion: …\"", "\"Important now:\"", "\"Can wait:\"", "(not backed by the text)"]
    {
        assert!(lector.contains(marker), "the lector of an English work is told the marker {marker}: {lector}");
    }
    let lector = prompts::default_prompt_in("@lektor", "", WorkKind::Book, WorkLanguage::De);
    for marker in
        ["\"Was trägt:\"", "\"Vorschlag: …\"", "\"Jetzt wichtig:\"", "\"Kann warten:\"", "(nicht im Text belegt)"]
    {
        assert!(lector.contains(marker), "the lector of a German work is told the marker {marker}: {lector}");
    }
    // A stored prompt stays untouched, the rule comes below it.
    let stored = prompts::effective_prompt_in("@lektor", "", Some("Sei knapp."), WorkKind::Book, WorkLanguage::En);
    assert_eq!(
        stored,
        format!("Sei knapp.\n\n{}", lang::language_rule(WorkLanguage::En)),
        "a stored prompt is kept literally, with the language rule of the work below"
    );
    assert_eq!(
        prompts::effective_prompt("@lektor", "", Some("Sei knapp.")),
        "Sei knapp.",
        "the prompt shown for editing carries no language rule (it would be saved into the prompt)"
    );
}

#[tokio::test]
async fn an_english_work_is_asked_in_english_markers_and_english_answers_are_recognised() {
    let _turn = common::model_queue(module_path!()).await;
    let b = pier([
        // First answer of the auditor without the mandatory finding → one retry.
        Step::words("The section reads well."),
        Step::words("Critical: \"always creaks\" claims more than the section shows.\n- instead of: \"always\"\n- better: \"often\""),
    ])
    .await;
    b.server.state.store.set_work_language(b.work, WorkLanguage::En).await.unwrap();
    let answer =
        b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[0], "persona": "@nathan" })).await;
    let body = result(&answer)["post"]["body"].as_str().unwrap().to_owned();
    let calls = b.model.calls();
    assert_eq!(calls.len(), 2, "the missing \"Critical:\" was asked for exactly once");
    let first = system(&calls[0]);
    assert!(
        first.contains(&lang::language_rule(WorkLanguage::En)),
        "the system prompt of an English work ends with the English language rule: {first}"
    );
    assert!(
        first.contains("mark it with \"Critical:\"") && !first.contains("Kritisch:"),
        "the auditor is told the English marker only: {first}"
    );
    assert!(
        system(&calls[1]).contains(&prompts::critical_retry_note(WorkLanguage::En))
            && system(&calls[1]).contains("start its line with \"Critical:\""),
        "the retry names the marker of the work language"
    );
    assert!(body.starts_with("Critical: \"always creaks\""), "the second answer is stored: {body}");
    assert!(
        !body.contains(prompts::CRITICAL_MISSING_NOTE)
            && !body.contains(prompts::CRITICAL_MISSING_NOTE_EN)
            && !body.contains(lang::EN.no_solution_note)
            && !body.contains(lang::EN.unbacked_note),
        "the English finding counts as the mandatory finding, its solution as a solution, its quote as backed: {body}"
    );
    let findings = b.server.rpc("finding.list", json!({ "work": b.work })).await;
    let listed = result(&findings)["findings"].as_array().cloned().unwrap_or_default();
    assert!(
        listed.iter().any(|f| f["kind"] == "critical" && f["body"].as_str().is_some_and(|t| t.contains("claims more"))),
        "the findings parser reads the English marker as a critical finding: {findings}"
    );
}

#[tokio::test]
async fn notes_of_baruch_follow_the_work_language_and_german_works_stay_as_they_were() {
    let _turn = common::model_queue(module_path!()).await;
    // English work: a quote that is not in the text and a critical point without a solution, twice (retry).
    let unbacked = "Critical: \"the gulls scream\" is a worn image.";
    let b = pier([Step::words(unbacked), Step::words(unbacked)]).await;
    b.server.state.store.set_work_language(b.work, WorkLanguage::En).await.unwrap();
    let answer =
        b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[0], "persona": "@nathan" })).await;
    let body = result(&answer)["post"]["body"].as_str().unwrap();
    assert!(
        body.contains("(not backed by the text)") && body.contains("(no solution proposed)"),
        "in an English work Baruch writes its notes in English: {body}"
    );
    assert!(
        !body.contains("(nicht im Text belegt)") && !body.contains("(ohne Lösungsvorschlag)"),
        "and not in German: {body}"
    );

    // German work: the same flow with a German answer, literally as before Round 38.
    let unbacked = "Kritisch: „die Möwen schreien“ ist ein abgegriffenes Bild.";
    let b = pier([Step::words(unbacked), Step::words(unbacked)]).await;
    let answer =
        b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[0], "persona": "@nathan" })).await;
    let body = result(&answer)["post"]["body"].as_str().unwrap();
    assert_eq!(
        body,
        "Kritisch: „die Möwen schreien“ ist ein abgegriffenes Bild. (nicht im Text belegt) (ohne Lösungsvorschlag)",
        "a German work gets the German notes in the old wording and order"
    );
    let calls = b.model.calls();
    assert!(
        system(&calls[0]).contains(&lang::language_rule(WorkLanguage::De))
            && system(&calls[0]).contains("mark it with \"Kritisch:\""),
        "the prompt of a German work is English too, but prescribes German and the German markers"
    );
    assert!(
        system(&calls[1]).contains(prompts::SOLUTION_RETRY_NOTE),
        "the retry for the missing solution names the German form"
    );

    // German work, English answer: the recognition takes both languages whatever the work language is.
    let b =
        pier([Step::words("Critical: \"always creaks\" generalises.\n- instead of: \"always\"\n- better: \"often\"")])
            .await;
    let answer =
        b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[0], "persona": "@nathan" })).await;
    let body = result(&answer)["post"]["body"].as_str().unwrap();
    assert_eq!(b.model.calls().len(), 1, "an English finding in a German work needs no retry: {body}");
    assert!(
        !body.contains("(ohne Lösungsvorschlag)") && !body.contains(prompts::CRITICAL_MISSING_NOTE),
        "its English solution lines count as the solution: {body}"
    );
}

#[test]
fn every_recognition_takes_german_and_english_markers() {
    for line in ["Kritisch: zu kurz.", "**Critical:** too short.", "- critical: too short.", "2. Critical: x"] {
        assert!(prompts::critical_line(line), "{line:?} is a critical finding at the start of a line");
        assert!(prompts::has_critical(line), "{line:?} satisfies the mandatory finding");
    }
    for line in ["Uncritical: fine.", "This is critical: no.", "Unkritisch: gut."] {
        assert!(!prompts::critical_line(line), "{line:?} is no finding: the marker must open the line");
    }
    assert!(
        prompts::critical_finding("- II. Application: Critical: \"all\" generalises."),
        "the English marker also counts behind a short part label, like the German one"
    );
    for sentence in [lang::DE.all_done_sentence, lang::EN.all_done_sentence] {
        assert!(
            prompts::all_done(&format!("Done: finding 3\n{sentence}")),
            "{sentence:?} waives the mandatory finding"
        );
    }
    let cleaned = prompts::strip_critical_lines(
        "= Low Tide\n\nWhat works: the rope.\nSuggestion: shorten.\nCritical: too long.\nImportant now: [B1]\n\
         Can wait: [B2]\nWas trägt: das Seil.\nThe ferry leaves.",
    );
    assert_eq!(
        cleaned, "= Low Tide\n\nThe ferry leaves.",
        "lines with markers of either language are conversation, not book text, and fall out of a proposal"
    );
    for opening in ["Sure, here it is.", "Certainly! The text", "As an AI I think", "Gerne! Der Text", "Natürlich."] {
        assert!(prompts::forbidden_opening(opening), "{opening:?} is a forbidden opening in either language");
    }
    for opening in ["Surely the ferry leaves.", "Natürlichkeit ist ein gutes Wort.", "Certain things stay."] {
        assert!(
            !prompts::forbidden_opening(opening),
            "{opening:?} only starts with the same letters: whole words count"
        );
    }
    assert_eq!(prompts::missing_solutions("Critical: x.\n- Step: delete the paragraph."), 0, "Step: is a solution");
    assert_eq!(prompts::missing_solutions("Critical: x.\n- Schritt: streichen."), 0, "Schritt: is a solution");
    assert_eq!(prompts::missing_solutions("Critical: x.\nStep by step it grows."), 1, "a sentence is no solution");
    assert_eq!(
        prompts::mark_missing_solutions_in("Critical: x. (ohne Lösungsvorschlag)", WorkLanguage::En),
        "Critical: x. (no solution proposed)",
        "a note in the other language is replaced, not doubled"
    );
    assert_eq!(
        prompts::mark_unbacked_criticals_in("Critical: \"gulls\" is worn.", "The rope creaks.", WorkLanguage::De),
        "Critical: \"gulls\" is worn. (nicht im Text belegt)",
        "the note follows the work language, the recognition of the finding does not depend on it"
    );
    assert_eq!(
        lang::of_prompt(&prompts::effective_prompt_in("@x", "", None, WorkKind::Book, WorkLanguage::En)),
        WorkLanguage::En,
        "the language of a call is read from its rule"
    );
    assert_eq!(lang::of_prompt("Sei knapp."), WorkLanguage::De, "a prompt without the rule counts as German");
}
