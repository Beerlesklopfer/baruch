// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 31a (glossar-daten): Glossar und eigene Register über RPC `glossary.*` und `index.registers/register_*/
//! proposals/decide` (je Änderung genau ein Spiegel-Commit mit `Baruch-Action`, Autor Mensch bzw. Persona/Client bei
//! Vorschlägen, Bibelstellen mit baruch_scripture geprüft und normalisiert), Spiegeldatei `glossar.md`, MCP
//! `glossary_list`/`glossary_propose`/`index_propose` (nur Vorschläge, nie Buchtext, `node` statt `key` → isError),
//! `glossary.suggest` über die Lektor-Regel (nur Nachbau `ScriptedModel`, nie Eliezer) und der Austausch in
//! Unterversion 7 zwischen zwei Instanzen (abgleichen statt doppeln, XSD u7).
//!
//! Besitz: glossar-daten. Kein Netz, kein Eliezer. Werke und Texte frei erfunden.

use crate::common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::rpc::{INVALID_PARAMS, REFUSED};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

/// Zahl der Commits im Spiegel.
fn commits(dir: &Path) -> usize {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map_or(0, |n| n.trim().parse().unwrap())
}

/// Autor, Betreff und Rumpf samt Trailern des letzten Commits.
fn last_commit(dir: &Path) -> String {
    common::git(dir, &["log", "-1", "--format=%an%n%s%n%b"]).unwrap_or_default()
}

async fn garden() -> (common::Server, i64, std::path::PathBuf) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let dir = server.mirror_dir_of(&report.slug);
    (server, report.work, dir)
}

async fn first_key(server: &common::Server, work: i64) -> i64 {
    server.state.store.load_work(work).await.unwrap().nodes[0].key
}

#[tokio::test]
async fn glossary_rpc_writes_with_exactly_one_commit_each_and_checks_bible_references() {
    let (server, work, dir) = garden().await;
    let start = commits(&dir);
    let added = server
        .rpc(
            "glossary.add",
            json!({ "work": work, "term": "Möhre", "short": "Wurzelgemüse mit orangem Rübenkörper.",
                    "long": "Wird im Frühling gesät.\nErnte im Herbst.", "variants": ["Möhren", "Mohrrübe"],
                    "source": "Gartenkalender (erfunden)", "bible_refs": ["ps 1,3", "Ps 1,3"] }),
        )
        .await;
    let term = result(&added)["term"].clone();
    let normalized = baruch_scripture::parse("ps 1,3")[0].normalized.clone().unwrap();
    assert_eq!(term["bible_refs"], json!([normalized]), "Bibelstelle mit baruch_scripture normalisiert, ohne Doppel");
    assert_eq!(term["status"], "accepted", "der Mensch legt angenommene Begriffe an");
    assert_eq!(term["origin"], "Autor", "Herkunft ist der Mensch");
    assert_eq!(term["anchor"], format!("baruch-gl-{}", term["uuid"].as_str().unwrap()), "Anker für Satz und EPUB");
    assert!(result(&added)["commit"].is_string(), "Antwort nennt den Commit: {added}");
    assert_eq!(commits(&dir), start + 1, "genau ein Spiegel-Commit");
    let log = last_commit(&dir);
    assert!(log.starts_with("Autor\nGlossar: „Möhre“ angelegt"), "Autor Mensch, Betreff: {log}");
    assert!(log.contains("Baruch-Action: glossary-add"), "Trailer Baruch-Action: {log}");
    let md = std::fs::read_to_string(dir.join("glossar.md")).expect("glossar.md im Spiegel");
    assert!(md.contains("## Möhre") && md.contains("Varianten: Möhren, Mohrrübe"), "Spiegeldatei: {md}");

    // Unlesbare Bibelstelle und Doppel: abgelehnt, kein Commit.
    for (params, why) in [
        (json!({ "work": work, "term": "Beet", "bible_refs": ["Hesekiel 99,1"] }), "Kapitel gibt es nicht"),
        (json!({ "work": work, "term": "Beet", "bible_refs": ["Gartenbuch 1,1"] }), "unbekanntes Buch"),
        (json!({ "work": work, "term": " MÖHRE " }), "Doppel über term_key"),
        (json!({ "work": work, "short": "ohne Begriff" }), "term fehlt"),
    ] {
        let answer = server.rpc("glossary.add", params).await;
        assert_eq!(error_code(&answer), INVALID_PARAMS, "{why}: INVALID_PARAMS: {answer}");
    }
    assert_eq!(commits(&dir), start + 1, "abgelehnt: kein Commit");

    // Nachschlagen über Begriff und Varianten; Liste mit Filter.
    for word in ["möhre", "Mohrrübe", "MÖHREN"] {
        let found = result(&server.rpc("glossary.lookup", json!({ "work": work, "word": word })).await).clone();
        assert_eq!(found["terms"].as_array().unwrap().len(), 1, "„{word}“ findet die Möhre: {found}");
    }
    let id = term["id"].as_i64().unwrap();
    let got = result(&server.rpc("glossary.get", json!({ "work": work, "id": id })).await).clone();
    assert_eq!(got["term"]["long"], "Wird im Frühling gesät.\nErnte im Herbst.", "glossary.get liest alles");
    assert_eq!(error_code(&server.rpc("glossary.get", json!({ "work": work, "id": 9999 })).await), INVALID_PARAMS);
    assert_eq!(
        error_code(&server.rpc("glossary.list", json!({ "work": work, "status": "neu" })).await),
        INVALID_PARAMS,
        "unbekannter Status"
    );

    // Ändern: ein Commit mit vorher/nachher.
    let updated = server
        .rpc(
            "glossary.update",
            json!({ "work": work, "id": id, "term": "Möhre", "short": "Karotte.", "variants": ["Möhren"] }),
        )
        .await;
    assert_eq!(result(&updated)["term"]["short"], "Karotte.", "geändert");
    assert_eq!(result(&updated)["term"]["updated_by"], "Autor", "updated_by = Mensch");
    assert_eq!(commits(&dir), start + 2, "ein Commit für die Änderung");
    let log = last_commit(&dir);
    assert!(log.contains("Baruch-Action: glossary-update") && log.contains("Vorher:"), "Rumpf vorher/nachher: {log}");

    // Löschen nur mit Bestätigung; der Commit trägt den ganzen Begriff (wiederherstellbar).
    let unconfirmed = server.rpc("glossary.delete", json!({ "work": work, "id": id })).await;
    assert_eq!(error_code(&unconfirmed), INVALID_PARAMS, "ohne confirm abgelehnt");
    let deleted = server.rpc("glossary.delete", json!({ "work": work, "id": id, "confirm": true })).await;
    assert_eq!(result(&deleted)["deleted"], id, "gelöscht");
    let log = last_commit(&dir);
    assert!(
        log.contains("Baruch-Action: glossary-delete") && log.contains("Kurz: Karotte."),
        "Begriff im Commit: {log}"
    );
    assert_eq!(commits(&dir), start + 3, "drei Änderungen, drei Commits");
    assert!(!dir.join("glossar.md").exists(), "ohne Begriffe keine Spiegeldatei mehr");
}

#[tokio::test]
async fn custom_registers_over_rpc_with_one_commit_each_and_builtin_ones_locked() {
    let (server, work, dir) = garden().await;
    let list = result(&server.rpc("index.registers", json!({ "work": work })).await).clone();
    let slugs: Vec<(&str, bool)> = list["registers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["slug"].as_str().unwrap(), r["builtin"].as_bool().unwrap()))
        .collect();
    assert_eq!(slugs, [("stichwort", true), ("personen", true), ("bibel", true)], "eingebaute Register");
    let start = commits(&dir);
    let added =
        server.rpc("index.register_add", json!({ "work": work, "slug": "pflanzen", "title": "Pflanzen" })).await;
    assert_eq!(result(&added)["register"]["slug"], "pflanzen", "angelegt");
    assert!(last_commit(&dir).contains("Baruch-Action: register-add"), "Commit register-add");
    let renamed = server
        .rpc("index.register_rename", json!({ "work": work, "slug": "pflanzen", "title": "Pflanzen & Kräuter" }))
        .await;
    assert_eq!(result(&renamed)["register"]["title"], "Pflanzen & Kräuter", "umbenannt");
    let log = last_commit(&dir);
    assert!(log.contains("Baruch-Action: register-rename") && log.contains("vorher „Pflanzen“"), "Commit: {log}");
    let regs = baruch_server::glossary::registers(&server.state, work).await.unwrap();
    assert_eq!(regs.last().map(|r| r.slug.as_str()), Some("pflanzen"), "registers() hängt eigene an");
    for slug in ["stichwort", "bibel"] {
        let answer = server.rpc("index.register_add", json!({ "work": work, "slug": slug, "title": "X" })).await;
        assert_eq!(error_code(&answer), INVALID_PARAMS, "{slug} eingebaut: {answer}");
    }
    assert_eq!(
        error_code(&server.rpc("index.register_delete", json!({ "work": work, "slug": "pflanzen" })).await),
        INVALID_PARAMS,
        "Löschen braucht confirm"
    );
    let deleted =
        server.rpc("index.register_delete", json!({ "work": work, "slug": "pflanzen", "confirm": true })).await;
    assert_eq!(result(&deleted)["deleted"], "pflanzen", "gelöscht");
    assert!(last_commit(&dir).contains("Baruch-Action: register-delete"), "Commit register-delete");
    assert_eq!(commits(&dir), start + 3, "drei Änderungen, drei Commits");
}

async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
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

async fn mcp_session(server: &common::Server) -> String {
    let (headers, init) = mcp(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Glossarprobe", "version": "1.0" } } }),
    )
    .await;
    assert!(init["result"].is_object(), "initialize: {init}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let (_, answer) = mcp(
        server,
        Some(session),
        json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": { "name": name, "arguments": arguments } }),
    )
    .await;
    answer
}

#[tokio::test]
async fn mcp_only_proposes_and_the_human_decides() {
    let (server, work, dir) = garden().await;
    let session = mcp_session(&server).await;
    let (_, list) = mcp(&server, Some(&session), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    for name in ["glossary_list", "glossary_propose", "index_propose"] {
        let entry = list["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap().clone();
        let description = entry["description"].as_str().unwrap();
        assert!(
            !description.contains("noch nicht umgesetzt") && !description.starts_with("STUB"),
            "{name}: kein Stub mehr: {description}"
        );
    }

    // glossary_propose: nur ein Vorschlag, Herkunft der Client; Autor des Commits ist der Client.
    let start = commits(&dir);
    let proposed = tool(
        &server,
        &session,
        "glossary_propose",
        json!({ "work": work, "term": "Radieschen", "short": "Kleine rote Rübe.", "bible_refs": ["Röm 3,24"],
                "reason": "kommt im Text vor" }),
    )
    .await;
    assert_eq!(proposed["result"]["isError"], false, "glossary_propose gelingt: {proposed}");
    let content = &proposed["result"]["structuredContent"];
    assert_eq!(content["status"], "proposed", "nur Vorschlag (Vertrag §3.3)");
    assert_eq!(content["origin"], "@mcp:glossarprobe", "Herkunft der Client");
    let id = content["id"].as_i64().unwrap();
    assert_eq!(content["url"], format!("/werke/{work}/glossar#begriff-{id}"), "Adresse der Glossarseite");
    assert_eq!(commits(&dir), start + 1, "ein Commit");
    let log = last_commit(&dir);
    assert!(log.contains("Baruch-Action: glossary-propose") && log.contains("Persona: @mcp:glossarprobe"), "{log}");
    assert!(log.starts_with("MCP-Client (@mcp:glossarprobe)"), "Autor ist der Client: {log}");
    let again = tool(&server, &session, "glossary_propose", json!({ "work": work, "term": "radieschen" })).await;
    assert_eq!(again["result"]["isError"], true, "schon vorgeschlagen → isError: {again}");
    let as_persona =
        tool(&server, &session, "glossary_propose", json!({ "work": work, "term": "Gießkanne", "persona": "@lektor" }))
            .await;
    assert_eq!(as_persona["result"]["structuredContent"]["origin"], "@lektor", "Herkunft nach acting_persona");

    // glossary_list liest nur: Vorschläge sind nicht nachschlagbar, aber in der Liste.
    let listed = tool(&server, &session, "glossary_list", json!({ "work": work, "status": "proposed" })).await;
    assert_eq!(listed["result"]["structuredContent"]["terms"].as_array().unwrap().len(), 2, "zwei Vorschläge");
    let looked = tool(&server, &session, "glossary_list", json!({ "work": work, "word": "Radieschen" })).await;
    assert!(looked["result"]["structuredContent"]["terms"].as_array().unwrap().is_empty(), "nur angenommene");
    let before = commits(&dir);
    tool(&server, &session, "glossary_list", json!({ "work": work })).await;
    assert_eq!(commits(&dir), before, "Lesen schreibt nichts");

    // Der Mensch entscheidet: übernehmen (Commit mit Baruch-Proposer) bzw. verwerfen (Grabstein).
    let accepted = server.rpc("glossary.decide", json!({ "work": work, "id": id, "accept": true })).await;
    assert_eq!(result(&accepted)["term"]["status"], "accepted", "übernommen");
    let log = last_commit(&dir);
    assert!(
        log.starts_with("Autor\n")
            && log.contains("glossary-accept")
            && log.contains("Baruch-Proposer: @mcp:glossarprobe"),
        "Mensch übernimmt, Antragsteller genannt: {log}"
    );
    let kanne = as_persona["result"]["structuredContent"]["id"].as_i64().unwrap();
    let rejected = server.rpc("glossary.decide", json!({ "work": work, "id": kanne, "accept": false })).await;
    assert_eq!(result(&rejected)["deleted"], kanne, "verworfen = gelöscht");
    assert!(last_commit(&dir).contains("Baruch-Action: glossary-reject"), "Commit glossary-reject");

    // index_propose: node statt key → -32602 (Runde 23, wie propose_revision/add_post); mit key ein Registervorschlag, der Text bleibt.
    let key = first_key(&server, work).await;
    let node_refused =
        tool(&server, &session, "index_propose", json!({ "work": work, "node": 0, "key": key, "word": "Beete" })).await;
    assert_eq!(
        node_refused["error"]["code"], -32602,
        "node → Argumentfehler -32602 wie bei propose_revision (Runde 23, vorher isError): {node_refused}"
    );
    let revisions = server.state.store.load_work(work).await.unwrap().nodes[0].revisions.len();
    let index = tool(
        &server,
        &session,
        "index_propose",
        json!({ "work": work, "key": key, "word": "Beete", "term": "Beet", "sub": "umgraben", "occurrence": 1,
                "reason": "Leitwort" }),
    )
    .await;
    assert_eq!(index["result"]["isError"], false, "index_propose gelingt: {index}");
    let content = &index["result"]["structuredContent"];
    assert_eq!(content["status"], "open", "Registervorschlag offen");
    let proposal = content["id"].as_i64().unwrap();
    assert_eq!(content["url"], format!("/werke/{work}/register#vorschlag-{proposal}"), "Adresse der Registerseite");
    assert_eq!(content["proposal"]["mark"], json!({ "register": "stichwort", "term": "Beet", "sub": "umgraben" }));
    assert_eq!(
        server.state.store.load_work(work).await.unwrap().nodes[0].revisions.len(),
        revisions,
        "nie eine Textänderung über MCP"
    );
    let log = last_commit(&dir);
    assert!(log.contains("Baruch-Action: index-propose") && log.contains("Baruch-Node: "), "Commit: {log}");
    let missing = tool(&server, &session, "index_propose", json!({ "work": work, "key": key, "word": "Kaktus" })).await;
    assert_eq!(missing["result"]["isError"], true, "Wort nicht im Text → isError: {missing}");

    let open = result(&server.rpc("index.proposals", json!({ "work": work })).await).clone();
    assert_eq!(open["proposals"][0]["id"], proposal, "index.proposals zeigt den offenen Vorschlag: {open}");
    assert_eq!(open["proposals"][0]["origin"], "@mcp:glossarprobe", "Herkunft");

    // Übernahme durch den Menschen: neue Fassung mit Marke.
    let decided = server.rpc("index.decide", json!({ "work": work, "id": proposal, "accept": true })).await;
    let answer = result(&decided);
    assert_eq!(answer["proposal"]["status"], "accepted", "übernommen: {answer}");
    assert_eq!(answer["marks"], 1, "eine Marke");
    let log = last_commit(&dir);
    assert!(log.starts_with("Autor\n") && log.contains("Baruch-Action: index-accept"), "Commit: {log}");
    let all = result(&server.rpc("index.proposals", json!({ "work": work, "status": "all" })).await).clone();
    assert_eq!(all["proposals"].as_array().unwrap().len(), 1, "status all zeigt auch entschiedene");
}

#[tokio::test]
async fn suggest_asks_the_lector_through_the_model_port_and_only_proposes() {
    let _turn = common::model_queue(module_path!()).await;
    let model = ScriptedModel::with([Step::words(
        "Begriff | kurze Erklärung\n- Möhre | Wurzelgemüse.\n2. Radieschen – kleine rote Rübe\n* Beet: Fläche im Garten",
    )]);
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model.clone()),
    )
    .await
    .expect("Datenordner");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let report = server.import_garden().await;
    let work = report.work;
    result(&server.rpc("glossary.add", json!({ "work": work, "term": "Beet" })).await);
    let key = first_key(&server, work).await;
    let answer = server.rpc("glossary.suggest", json!({ "work": work, "key": key })).await;
    let out = result(&answer);
    assert_eq!(out["persona"], "@lektor", "Lektor nach Einstellung (Vorgabe fest @lektor)");
    let proposed: Vec<&str> = out["proposed"].as_array().unwrap().iter().map(|t| t["term"].as_str().unwrap()).collect();
    assert_eq!(proposed, ["Möhre", "Radieschen"], "nur Vorschläge aus der Antwort: {out}");
    assert!(out["proposed"].as_array().unwrap().iter().all(|t| t["status"] == "proposed" && t["origin"] == "@lektor"));
    let skipped = out["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "Beet steht schon im Glossar: {out}");
    assert!(skipped[0]["reason"].as_str().unwrap().contains("schon im Glossar"), "Grund genannt: {out}");
    let calls = model.calls();
    assert_eq!(calls.len(), 1, "genau ein Modellaufruf");
    assert_eq!(calls[0].persona, "@lektor", "über den Port der Persona");
    let user = &calls[0].messages.last().unwrap().content;
    assert!(user.contains("Die Beete werden umgegraben."), "Text des Abschnitts im Auftrag: {user}");
    assert!(user.contains("Schon im Glossar") && user.contains("Beet"), "bekannte Begriffe genannt: {user}");
    let accepted = server.state.store.glossary(work, Some(baruch_store::GlossaryStatus::Accepted)).await.unwrap();
    assert_eq!(accepted.len(), 1, "nichts wird vom Modell angenommen");
}

#[tokio::test]
async fn suggest_reports_a_missing_model_honestly_and_writes_nothing() {
    // AppState::open hat NoModel: ehrlicher Fehler, kein Vorschlag, kein Commit.
    let (server, work, dir) = garden().await;
    let start = commits(&dir);
    let answer = server.rpc("glossary.suggest", json!({ "work": work })).await;
    assert_eq!(error_code(&answer), REFUSED, "Fehler des Modells als REFUSED: {answer}");
    assert!(answer["error"]["message"].as_str().unwrap().contains("Modell"), "Grund genannt: {answer}");
    assert!(server.state.store.glossary(work, None).await.unwrap().is_empty(), "nichts angelegt");
    assert_eq!(commits(&dir), start, "kein Commit");
}

/// Formular-POST an `uri` → Status, Ziel der Weiterleitung, Seite.
async fn post_form(server: &common::Server, uri: &str, form: &str) -> (StatusCode, Option<String>, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap();
    let (status, headers, body) = server.send(request).await;
    (status, headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned()), body)
}

#[tokio::test]
async fn suggest_is_a_form_on_the_glossary_and_marking_pages_without_javascript() {
    // Befund nach R31: „Begriffe vorschlagen lassen“ gab es nur als RPC. Jetzt ein Formular (ohne JavaScript) auf der
    // Glossarseite (ganzes Werk) und auf der Seite „an der Markierung“ (Abschnitt); Ergebnis nur Vorschläge.
    let _turn = common::model_queue(module_path!()).await;
    let model = ScriptedModel::with([
        Step::words("Möhre | Wurzelgemüse.\nBeet | Fläche im Garten"),
        Step::words("Spaten | Werkzeug zum Umgraben"),
    ]);
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model.clone()),
    )
    .await
    .expect("Datenordner");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let work = server.import_garden().await.work;
    let key = first_key(&server, work).await;
    result(&server.rpc("glossary.add", json!({ "work": work, "term": "Beet" })).await);

    let (status, page) = server.get(&format!("/werke/{work}/glossar")).await;
    assert_eq!(status, StatusCode::OK);
    let form = &page[page.find("data-glossary-suggest").expect("Formular auf der Glossarseite")..];
    let form = &form[..form.find("</form>").unwrap()];
    assert!(
        form.contains("name=\"action\" value=\"suggest\"") && form.contains("Begriffe vorschlagen lassen"),
        "Knopf „Begriffe vorschlagen lassen“ als POST-Formular: {form}"
    );
    assert!(page.contains(&format!("action=\"/werke/{work}/glossar/act\" data-glossary-suggest")), "Ziel glossar/act");

    let (status, location, body) = post_form(&server, &format!("/werke/{work}/glossar/act"), "action=suggest").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Erfolg → 303 (Post/Redirect/Get): {body}");
    let location = location.expect("Weiterleitung");
    assert_eq!(
        location,
        format!("/werke/{work}/glossar?ok=suggest&neu=1&uebersprungen=1#vorschlaege"),
        "Möhre neu, Beet übersprungen (schon im Glossar); Anker auf die Vorschläge"
    );
    let (_, page) = server.get(location.split('#').next().unwrap()).await;
    assert!(page.contains("Der Lektor hat 1 Begriff vorgeschlagen"), "Hinweis mit Zahl: {page}");
    assert!(page.contains("1 übersprungen"), "übersprungene genannt: {page}");
    assert!(page.contains("Vorschlag von @lektor"), "Möhre steht gekennzeichnet bei den Vorschlägen: {page}");
    let calls = model.calls();
    assert_eq!(calls.len(), 1, "ein Modellaufruf über den Port (Nachbau)");
    assert!(calls[0].label.ends_with("work"), "ohne key das ganze Werk: {}", calls[0].label);

    // Seite „an der Markierung“: Formular mit dem Abschnitt.
    let (_, page) = server.get(&format!("/werke/{work}/markierung?key={key}")).await;
    let form = &page[page.find("class=\"marking-suggest\"").expect("Formular an der Markierung")..];
    let form = &form[..form.find("</form>").unwrap()];
    assert!(
        form.contains("value=\"suggest\"") && form.contains(&format!("name=\"key\" value=\"{key}\"")),
        "Abschnitt im Formular: {form}"
    );
    let (status, location, _) =
        post_form(&server, &format!("/werke/{work}/glossar/act"), &format!("action=suggest&key={key}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(location.unwrap().contains("ok=suggest&neu=1&uebersprungen=0"), "Spaten neu");
    assert!(model.calls()[1].label.ends_with(&format!("key {key}")), "Auftrag für den Abschnitt");
    let accepted = server.state.store.glossary(work, Some(baruch_store::GlossaryStatus::Accepted)).await.unwrap();
    assert_eq!(accepted.len(), 1, "nichts wird vom Modell angenommen (nur Beet vom Menschen)");

    let (status, _, page) = post_form(&server, &format!("/werke/{work}/glossar/act"), "action=suggest&key=x").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unlesbarer Abschnitt → 422: {page}");
    assert_eq!(model.calls().len(), 2, "kein Modellaufruf bei unlesbarer Eingabe");
}

#[tokio::test]
async fn the_suggest_form_reports_a_missing_model_on_the_page() {
    // AppState::open hat NoModel: 422 mit der ehrlichen Meldung auf der Glossarseite, nichts angelegt, kein Commit.
    let (server, work, dir) = garden().await;
    let start = commits(&dir);
    let (status, _, page) = post_form(&server, &format!("/werke/{work}/glossar/act"), "action=suggest").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Fehler des Modells → 422: {page}");
    assert!(page.contains("data-glossary-error") && page.contains("Modell"), "Meldung auf der Seite: {page}");
    assert!(server.state.store.glossary(work, None).await.unwrap().is_empty(), "nichts angelegt");
    assert_eq!(commits(&dir), start, "kein Commit");
}

async fn export_xml(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    result(&answer)["xml"].as_str().unwrap().to_owned()
}

async fn import(server: &common::Server, xml: &str, mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": mode }),
        )
        .await;
    result(&answer).clone()
}

fn xmllint_u7(xml: &str) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("werk.baruch.xml");
    std::fs::write(&file, xml).unwrap();
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u7.xsd");
    match std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&file)
        .output()
    {
        Ok(output) => assert!(output.status.success(), "XSD u7: {}", String::from_utf8_lossy(&output.stderr)),
        Err(e) => eprintln!("ÜBERSPRUNGEN: xmllint nicht gefunden ({e})"),
    }
}

#[tokio::test]
async fn two_instances_exchange_glossary_and_registers_without_doubling() {
    let (a, work_a, _) = garden().await;
    result(
        &a.rpc(
            "glossary.add",
            json!({ "work": work_a, "term": "Möhre", "variants": ["Möhren"], "bible_refs": ["Ps 1,3"] }),
        )
        .await,
    );
    let rueben = result(&a.rpc("glossary.add", json!({ "work": work_a, "term": "Rübe" })).await)["term"]["id"].clone();
    result(&a.rpc("glossary.delete", json!({ "work": work_a, "id": rueben, "confirm": true })).await);
    result(&a.rpc("index.register_add", json!({ "work": work_a, "slug": "pflanzen", "title": "Pflanzen" })).await);
    result(
        &a.rpc(
            "work.typeset_set",
            json!({ "work": work_a, "settings": {
        "glossary_page": "after_indexes", "index_pages": { "registers": ["stichwort", "pflanzen"], "columns": 2 },
        "glossary_first_mention": true } }),
        )
        .await,
    );
    let xml = export_xml(&a, work_a).await;
    for needle in [
        // Runde 22 (Vertrag §6.5, spiegel-entwurf): Der Schreiber schreibt jetzt Unterversion 8.
        "version=\"8\"",
        "<glossar>",
        "<wort>Möhre</wort>",
        "kennung=\"pflanzen\"",
        "art=\"glossar-begriff\"",
        "glossarseite=\"nach-registern\"",
        "registerseiten=\"stichwort pflanzen\"",
    ] {
        assert!(xml.contains(needle), "{needle} in der Ausgabe: {xml}");
    }
    xmllint_u7(&xml);

    // B liest neu ein: Begriffe, Register, Satz-Felder und Grabsteine kommen mit.
    let b = common::open().await;
    let imported = import(&b, &xml, "new").await;
    let work_b = imported["work"].as_i64().unwrap();
    assert_eq!(
        (imported["added"]["glossary_terms"].clone(), imported["added"]["registers"].clone()),
        (json!(1), json!(1))
    );
    let terms_a = a.state.store.glossary(work_a, None).await.unwrap();
    let terms_b = b.state.store.glossary(work_b, None).await.unwrap();
    assert_eq!(
        terms_b.iter().map(|t| (&t.uuid, &t.term, &t.variants)).collect::<Vec<_>>(),
        terms_a.iter().map(|t| (&t.uuid, &t.term, &t.variants)).collect::<Vec<_>>(),
        "gleiche Begriffe mit gleichen Kennungen"
    );
    let typeset_b = b.state.store.work_typeset(work_b).await.unwrap().unwrap().settings;
    assert_eq!(typeset_b.glossary_first_mention, Some(true), "Satz-Felder der Runde 31 mitgenommen");
    let graves: i64 = sqlx::query_scalar("SELECT count(*) FROM uuid_tombstone WHERE kind = 'glossary_term'")
        .fetch_one(b.state.store.pool())
        .await
        .unwrap();
    assert_eq!(graves, 1, "Grabstein des gelöschten Begriffs mitgenommen");

    // A ergänzt, B gleicht ab: nur das Neue kommt dazu, nichts doppelt; ein zweiter Abgleich bringt nichts.
    result(&a.rpc("glossary.add", json!({ "work": work_a, "term": "Kresse" })).await);
    let merged = import(&b, &export_xml(&a, work_a).await, "merge").await;
    assert_eq!(merged["added"]["glossary_terms"], 1, "nur Kresse: {merged}");
    assert_eq!(merged["added"]["registers"], 0, "Register bekannt");
    let merged = import(&b, &export_xml(&a, work_a).await, "merge").await;
    assert_eq!(merged["added"]["glossary_terms"], 0, "zweiter Abgleich bringt nichts: {merged}");
    assert_eq!(b.state.store.glossary(work_b, None).await.unwrap().len(), 2, "Möhre und Kresse, keine Doppel");
}
