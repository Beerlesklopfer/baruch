// SPDX-License-Identifier: AGPL-3.0-or-later
//! Findings: splitting, similarity, status, mirror, exchange u4 (Round 28, task befunde, contract R28 §1).
//!
//! Garden book from `tests/data/gartenbuch.typ` (freely invented), answer texts of the personas freely invented as
//! well. No model: persona answers are created as posts through the store and split with `findings::ingest` (the way
//! `assist::save` and `threads::answer` have done it since S1). What only works after the merge (status forms of the
//! page, B references in the RFC) is checked by the integrator in `round28_abnahme.rs`.

use baruch_core::Post;
use baruch_server::findings::parse::split_answer;
use baruch_server::findings::{self, FindingKind, FindingStatus, IngestOrigin, IngestSource};
use serde_json::{Value, json};

use crate::common;

/// Garden book with stable keys by position (Frühling, Aussaat, Tiefe, Sommer) and mirror directory.
async fn garden() -> (common::Server, i64, Vec<i64>, std::path::PathBuf) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let keys: Vec<i64> = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? ORDER BY seq")
        .bind(report.work)
        .fetch_all(server.state.store.pool())
        .await
        .unwrap();
    let dir = server.mirror_dir_of(&report.slug);
    (server, report.work, keys, dir)
}

fn post(author: &str, body: &str) -> Post {
    Post { id: 0, created: String::new(), author: author.into(), body: body.into(), sources: vec![], refs: vec![] }
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn commit_count(dir: &std::path::Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

fn last_message(dir: &std::path::Path) -> Option<String> {
    common::git(dir, &["log", "-1", "--format=%an <%ae>%n%B"])
}

/// Create a post of a persona in the section `seq` (key `key`) and split it; returns (post, report).
async fn answer(
    server: &common::Server,
    work: i64,
    seq: usize,
    key: i64,
    persona: &str,
    text: &str,
) -> (i64, findings::IngestReport) {
    let id = server.state.store.add_post(work, seq, &post(persona, text)).await.unwrap();
    let source =
        IngestSource { work, origin: IngestOrigin::Post { id, key }, persona, text, summary: false, manual: false };
    (id, findings::ingest(&server.state, &source).await.unwrap())
}

/// Rows of a post as text (for „byte-gleich vorher/nachher“).
async fn post_row(server: &common::Server, table: &str, id: i64) -> String {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT quote(author) || '|' || quote(body) || '|' || quote(sources) || '|' || quote(created) || '|' || \
         quote(position) || '|' || quote(edited) || '|' || quote(uuid) FROM {table} WHERE id = ?"
    )))
    .bind(id)
    .fetch_one(server.state.store.pool())
    .await
    .unwrap()
}

/// Invented review answer with everything the splitter has to know.
const REVIEW: &str = "\
**Was trägt**
- Der Einstieg mit dem Beet trägt die Szene.

**Was fehlt**
1. Ein Satz zur Erntezeit.
2. Die Sorte der Möhren.
   Sie steht nur im Vorwort.

**Was stört**
Kritisch: „immer im März“ verallgemeinert. (nicht im Text belegt)
Kritisch: „Tiefe“ passt nicht zur Überschrift.

(Antwort abgeschnitten: Längengrenze erreicht)

(Kontext gekürzt)
";

#[test]
fn a_review_splits_into_the_expected_findings_and_small_talk_into_none() {
    let found = split_answer(REVIEW);
    let summary: Vec<(u32, FindingKind, &str, &str, &str)> =
        found.iter().map(|f| (f.position, f.kind, f.body.as_str(), f.quote.as_str(), f.quote_check.as_str())).collect();
    assert_eq!(
        summary,
        vec![
            (0, FindingKind::Praise, "Der Einstieg mit dem Beet trägt die Szene.", "", ""),
            (1, FindingKind::Hint, "Ein Satz zur Erntezeit.", "", ""),
            (2, FindingKind::Hint, "Die Sorte der Möhren.\nSie steht nur im Vorwort.", "", ""),
            (3, FindingKind::Critical, "„immer im März“ verallgemeinert.", "immer im März", "(nicht im Text belegt)"),
            (4, FindingKind::Critical, "„Tiefe“ passt nicht zur Überschrift.", "Tiefe", ""),
        ],
        "Gliederung, Aufzählung, Marken, Zitat und Prüfvermerk; Vermerke von Baruch sind keine Befunde (Vertrag §1.3)"
    );
    assert!(split_answer("Gern geschehen! Das klingt nach einem schönen Frühling.").is_empty(), "Small Talk");
    let refs = split_answer("- Sammlung: [B4] und [B9] meinen dasselbe Beet.");
    assert_eq!(refs[0].refs, vec![4, 9], "Verweise [B<id>] landen in refs (Vertrag §1.7)");
}

#[tokio::test]
async fn ingest_stores_findings_once_changes_no_post_and_never_takes_the_write_lock() {
    let (server, work, keys, _) = garden().await;
    let id = server.state.store.add_post(work, 1, &post("@lektor", REVIEW)).await.unwrap();
    let before = post_row(&server, "post", id).await;
    let source = IngestSource {
        work,
        origin: IngestOrigin::Post { id, key: keys[1] },
        persona: "@lektor",
        text: REVIEW,
        summary: false,
        manual: false,
    };
    // With the write lock held: ingest must not take it (the caller in assist::save holds it).
    let held = server.state.write_lock.lock().await;
    let report = tokio::time::timeout(std::time::Duration::from_secs(10), findings::ingest(&server.state, &source))
        .await
        .expect("ingest verklemmt sich nicht an der gehaltenen Schreibsperre")
        .unwrap();
    drop(held);
    assert_eq!(report.created.len(), 5, "fünf Befunde aus der Prüfantwort: {:?}", report.created);
    assert!(report.created.iter().all(|f| f.key == Some(keys[1]) && f.post == Some(id)), "Abschnitt und Beitrag");
    let again = findings::ingest(&server.state, &source).await.unwrap();
    assert!(again.created.is_empty() && again.attached.is_empty(), "zweiter Aufruf legt nichts an (wiederholbar)");
    assert_eq!(post_row(&server, "post", id).await, before, "der Beitrag ist byte-gleich (Vertrag §1.3)");
    let counts = server.state.store.finding_open_counts(work).await.unwrap();
    assert_eq!(counts.get(&keys[1]), Some(&4), "offen zählen vier: Lob zählt nicht (Vertrag §1.5)");

    // Thread post: without a section.
    let (thread, _) = server.state.store.thread_create(work, "Autor", "Wann säen?").await.unwrap();
    let reply =
        server.state.store.thread_add_post(work, thread.id, "@lektor", "Hinweis: erst nach Frost.", &[]).await.unwrap();
    let before = post_row(&server, "chat_thread_post", reply.id).await;
    let source = IngestSource {
        work,
        origin: IngestOrigin::ThreadPost { id: reply.id, thread: thread.id },
        persona: "@lektor",
        text: "Hinweis: erst nach Frost.",
        summary: false,
        manual: false,
    };
    let report = findings::ingest(&server.state, &source).await.unwrap();
    assert_eq!(report.created.len(), 1, "ein Hinweis aus dem Faden");
    assert_eq!(
        (report.created[0].key, report.created[0].thread, report.created[0].thread_post),
        (None, Some(thread.id), Some(reply.id)),
        "Fadenbefund ohne Abschnitt"
    );
    assert_eq!(post_row(&server, "chat_thread_post", reply.id).await, before, "Fadenbeitrag byte-gleich");
    // Converted manually: without a mark one hint with the whole text.
    let plain = server.state.store.add_post(work, 0, &post("Autor", "Mehr Kompost unter die Beete.")).await.unwrap();
    let manual = IngestSource {
        work,
        origin: IngestOrigin::Post { id: plain, key: keys[0] },
        persona: "Autor",
        text: "Mehr Kompost unter die Beete.",
        summary: false,
        manual: true,
    };
    let report = findings::ingest(&server.state, &manual).await.unwrap();
    assert_eq!(report.created.len(), 1, "manuell: genau ein Befund");
    assert_eq!(
        (report.created[0].kind, report.created[0].body.as_str()),
        (FindingKind::Hint, "Mehr Kompost unter die Beete."),
        "ganzer Text als Hinweis"
    );
}

#[tokio::test]
async fn a_repeated_rejected_finding_is_attached_and_a_summary_points_to_its_origins() {
    let (server, work, keys, _) = garden().await;
    let (_, first) =
        answer(&server, work, 3, keys[3], "@lektor", "Hinweis: Der Sommer braucht mehr Hitze im Text.").await;
    let rejected = &first.created[0];
    let rpc = server
        .rpc(
            "finding.set_status",
            json!({ "work": work, "id": rejected.id, "status": "rejected", "reason": "Der Sommer bleibt mild." }),
        )
        .await;
    result(&rpc);
    // Another persona reports the same once more: attached, not open.
    let (_, second) =
        answer(&server, work, 3, keys[3], "@nathan", "Kritisch: Der Sommer braucht mehr Hitze im Text!").await;
    assert!(second.created.is_empty(), "nichts Neues: {:?}", second.created);
    assert_eq!(second.attached.len(), 1, "angehängt an den verworfenen Befund");
    assert_eq!(second.attached[0].similar_to, Some(rejected.id), "similar_to = Original (Vertrag §1.4)");
    let counts = server.state.store.finding_open_counts(work).await.unwrap();
    assert_eq!(counts.get(&keys[3]), None, "der angehängte zählt nicht als offen (Vertrag §1.5)");
    // Something different is new and open.
    let (_, third) = answer(&server, work, 3, keys[3], "@nathan", "Hinweis: Die Bohnen fehlen ganz.").await;
    assert_eq!(third.created.len(), 1, "anderer Text: neuer Befund");
    // Summary of the lector in the RFC: [B<id>] become summary_of.
    let id = server
        .state
        .store
        .add_post(
            work,
            3,
            &post("@lektor", &format!("- Jetzt wichtig: [B{}] und [B{}].", rejected.id, third.created[0].id)),
        )
        .await
        .unwrap();
    let text = format!("- Jetzt wichtig: [B{}] und [B{}].", rejected.id, third.created[0].id);
    let source = IngestSource {
        work,
        origin: IngestOrigin::Post { id, key: keys[3] },
        persona: "@lektor",
        text: &text,
        summary: true,
        manual: false,
    };
    let report = findings::ingest(&server.state, &source).await.unwrap();
    let summary = &report.created[0];
    assert_eq!(summary.kind, FindingKind::Hint, "Sammelbefund ist ein Hinweis (Vertrag §1.6)");
    assert_eq!(
        summary.summary_of,
        vec![rejected.uuid.clone(), third.created[0].uuid.clone()],
        "summary_of = UUIDs der verwiesenen Befunde"
    );
}

#[tokio::test]
async fn backfill_splits_persona_posts_exactly_once_and_leaves_every_post_unchanged() {
    let (server, work, keys, _) = garden().await;
    let store = &server.state.store;
    let persona = store.add_post(work, 0, &post("@lektor", "Hinweis: Das Beet ist zu schmal.")).await.unwrap();
    let human = store.add_post(work, 0, &post("Autor", "Hinweis: Das ist meine Notiz.")).await.unwrap();
    let mcp = store.add_post(work, 0, &post("@mcp:probe", "Hinweis: vom Client.")).await.unwrap();
    let (thread, _) = store.thread_create(work, "Autor", "Frage zum Beet").await.unwrap();
    let reply = store.thread_add_post(work, thread.id, "@nathan", "Kritisch: „Beet“ ist unklar.", &[]).await.unwrap();
    let rows_before = [
        post_row(&server, "post", persona).await,
        post_row(&server, "post", human).await,
        post_row(&server, "post", mcp).await,
        post_row(&server, "chat_thread_post", reply.id).await,
    ];
    let created = findings::backfill(&server.state).await.unwrap();
    assert_eq!(created, 2, "nur die Persona-Beiträge (Lektor im Abschnitt, Nathan im Faden)");
    let all = baruch_store::FindingFilter { include_praise: true, include_attached: true, ..Default::default() };
    let list = store.finding_list(work, &all).await.unwrap();
    let origins: Vec<(Option<i64>, Option<i64>, &str)> =
        list.iter().map(|f| (f.post, f.thread_post, f.persona.as_str())).collect();
    assert_eq!(
        origins,
        vec![(Some(persona), None, "@lektor"), (None, Some(reply.id), "@nathan")],
        "Beiträge des Menschen und von MCP-Clients werden übergangen"
    );
    assert_eq!(list[0].key, Some(keys[0]), "Abschnitt des Beitrags");
    let rows_after = [
        post_row(&server, "post", persona).await,
        post_row(&server, "post", human).await,
        post_row(&server, "post", mcp).await,
        post_row(&server, "chat_thread_post", reply.id).await,
    ];
    assert_eq!(rows_after, rows_before, "alle Beiträge byte-gleich");
    let marker: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'findings_backfill'")
        .fetch_optional(store.pool())
        .await
        .unwrap();
    assert_eq!(marker.as_deref(), Some("1"), "Merker in store_meta");
    store.add_post(work, 0, &post("@lektor", "Hinweis: noch einer.")).await.unwrap();
    assert_eq!(findings::backfill(&server.state).await.unwrap(), 0, "einmal je Datenbank: der zweite Lauf tut nichts");
}

#[tokio::test]
async fn set_status_refuses_bad_changes_and_writes_one_commit_with_the_trailers() {
    let (server, work, keys, dir) = garden().await;
    let (_, report) = answer(&server, work, 1, keys[1], "@lektor", "Hinweis: Die Reihen enger setzen.").await;
    let id = report.created[0].id;
    let before = commit_count(&dir);
    for (params, why) in [
        (json!({ "status": "rejected" }), "verworfen ohne Begründung"),
        (json!({ "status": "rejected", "reason": "   " }), "verworfen mit leerer Begründung"),
        (json!({ "status": "adopted", "rev": 99 }), "übernommen mit fehlender Fassung"),
        (json!({ "status": "adopted" }), "übernommen ohne Fassung"),
        (json!({ "status": "deferred" }), "verschoben ohne Ziel"),
        (json!({ "status": "deferred", "target_key": keys[0], "target_idea": 1 }), "verschoben mit zwei Zielen"),
        (json!({ "status": "vergessen" }), "unbekannter Status"),
    ] {
        let mut params = params;
        params["work"] = json!(work);
        params["id"] = json!(id);
        let answer = server.rpc("finding.set_status", params).await;
        assert_eq!(answer["error"]["code"], -32602, "{why}: -32602 erwartet: {answer}");
        assert_eq!(commit_count(&dir), before, "{why}: kein Commit");
    }
    let unchanged = server.state.store.finding_get(work, id).await.unwrap();
    assert_eq!((unchanged.status, unchanged.reason.as_str()), (FindingStatus::Open, ""), "nichts geändert");
    assert!(server.state.store.finding_history(work, id).await.unwrap().is_empty(), "kein Verlaufseintrag");

    let done = server
        .rpc("finding.set_status", json!({ "work": work, "id": id, "status": "deferred", "target_key": keys[3] }))
        .await;
    let done = result(&done);
    assert_eq!(done["finding"]["status"], "deferred", "neuer Status: {done}");
    assert_eq!(done["finding"]["status_label"], "verschoben", "deutsche Bezeichnung über label: {done}");
    assert_eq!(done["before"]["status"], "open", "vorher: {done}");
    assert_eq!(done["finding"]["deferred"]["key"], keys[3], "Ziel aufgelöst: {done}");
    assert!(done["commit"].is_string(), "Antwort trägt den Commit: {done}");
    if let (Some(before), Some(after)) = (before, commit_count(&dir)) {
        assert_eq!(after, before + 1, "genau ein Commit je Statusänderung");
    }
    if let Some(message) = last_message(&dir) {
        assert!(message.starts_with("Autor <autor@baruch.invalid>"), "Autor ist der Mensch: {message}");
        assert!(message.contains(&format!("Befund {id}: verschoben nach")), "Betreff: {message}");
        assert!(message.contains(&format!("Baruch-Finding: {id}")), "Trailer Baruch-Finding: {message}");
        assert!(message.contains("Baruch-Action: finding-status"), "Trailer Baruch-Action: {message}");
        assert!(message.contains(&format!("Baruch-Node: {}", keys[1])), "Trailer Baruch-Node: {message}");
        assert!(message.contains("Die Reihen enger setzen."), "der Befund steht im Rumpf: {message}");
    }
    let history = server.state.store.finding_history(work, id).await.unwrap();
    assert_eq!(history.len(), 1, "ein Verlaufseintrag");
    assert!(!history[0].proposed, "gesetzt vom Menschen, kein Vorschlag");
    // Over MCP there is no way to set the status (only proposals).
    let tools = baruch_server::mcp::tools::list();
    let names: Vec<&str> = tools["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.iter().all(|n| !n.contains("set_status") && *n != "finding_decide"), "kein MCP-Werkzeug: {names:?}");
}

#[tokio::test]
async fn proposals_never_reject_never_double_and_the_human_decides_with_one_commit_each() {
    let (server, work, keys, dir) = garden().await;
    let (_, report) = answer(&server, work, 0, keys[0], "@lektor", "Hinweis: Den Frühling früher beginnen.").await;
    let id = report.created[0].id;
    let lektor = baruch_store::IdeaOrigin::Persona { handle: "@lektor".into() };
    let before = commit_count(&dir);
    for change in [baruch_store::StatusChange::Rejected { reason: "nein".into() }, baruch_store::StatusChange::Open] {
        let err = baruch_server::i18n::scope(
            baruch_server::i18n::Lang::De,
            findings::rpc::propose(&server.state, work, id, &change, &lektor, ""),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, -32602, "Assistenten verwerfen und öffnen nie: {err:?}");
        assert!(err.message.starts_with("Nur der Mensch kann"), "ehrliche Meldung: {}", err.message);
    }
    assert_eq!(commit_count(&dir), before, "abgelehnte Vorschläge: kein Commit");
    let change = baruch_store::StatusChange::Adopted { rev: 1 };
    let proposed =
        findings::rpc::propose(&server.state, work, id, &change, &lektor, "Fassung 1 setzt es um.").await.unwrap();
    assert!(proposed["commit"].is_string(), "ein Commit für den Vorschlag: {proposed}");
    if let Some(message) = last_message(&dir) {
        assert!(message.contains("Baruch-Action: finding-propose"), "{message}");
        assert!(message.contains("Persona: @lektor"), "Persona bei Vorschlägen: {message}");
        assert!(message.contains("Fassung 1 setzt es um."), "Notiz im Rumpf: {message}");
    }
    assert!(
        findings::rpc::propose(&server.state, work, id, &change, &lektor, "").await.is_err(),
        "derselbe offene Vorschlag nicht doppelt"
    );
    let entry = proposed["entry"]["id"].as_i64().unwrap();
    let decided = server.rpc("finding.decide", json!({ "work": work, "entry": entry, "confirm": true })).await;
    let decided = result(&decided);
    assert_eq!(decided["finding"]["status"], "adopted", "bestätigt: {decided}");
    assert_eq!(decided["finding"]["adopted_rev"], 1, "in Fassung 1: {decided}");
    assert_eq!(decided["entry"]["decision"], "confirmed", "{decided}");
    if let Some(message) = last_message(&dir) {
        assert!(message.contains("Baruch-Action: finding-decide"), "{message}");
        assert!(message.contains("Baruch-Proposer: @lektor"), "Antragsteller genannt: {message}");
        assert!(message.starts_with("Autor <"), "entschieden hat der Mensch: {message}");
    }
    if let (Some(before), Some(after)) = (before, commit_count(&dir)) {
        assert_eq!(after, before + 2, "je ein Commit für Vorschlag und Entscheidung");
    }
    // Rejected and outdated.
    let (_, other) = answer(&server, work, 0, keys[0], "@nathan", "Hinweis: Mehr Regen im Frühling.").await;
    let other = other.created[0].id;
    let done = baruch_store::StatusChange::Done { reason: String::new() };
    let nathan = baruch_store::IdeaOrigin::Persona { handle: "@nathan".into() };
    let first = findings::rpc::propose(&server.state, work, other, &done, &nathan, "").await.unwrap();
    let no =
        server.rpc("finding.decide", json!({ "work": work, "entry": first["entry"]["id"], "confirm": false })).await;
    assert_eq!(result(&no)["finding"]["status"], "open", "confirm=false verwirft den Vorschlag, der Befund bleibt");
    assert_eq!(result(&no)["entry"]["decision"], "rejected", "{no}");
    let second = findings::rpc::propose(&server.state, work, other, &done, &nathan, "").await.unwrap();
    result(
        &server
            .rpc("finding.set_status", json!({ "work": work, "id": other, "status": "rejected", "reason": "nein" }))
            .await,
    );
    let stale =
        server.rpc("finding.decide", json!({ "work": work, "entry": second["entry"]["id"], "confirm": true })).await;
    assert_eq!(result(&stale)["entry"]["decision"], "stale", "inzwischen geändert → veraltet: {stale}");
    assert_eq!(result(&stale)["finding"]["status"], "rejected", "der Befund bleibt verworfen");
    let list = server.rpc("finding.list", json!({ "work": work, "status": "rejected,adopted" })).await;
    assert_eq!(result(&list)["total"], 2, "Filter nach Status (Liste als Text „a,b“): {list}");
    let got = server.rpc("finding.get", json!({ "work": work, "id": other })).await;
    assert_eq!(result(&got)["history"].as_array().unwrap().len(), 3, "Vorschlag, Vorschlag, Setzen: {got}");
}

#[tokio::test]
async fn the_mirror_has_befunde_md_only_with_findings_and_one_commit_per_change() {
    let (server, work, keys, dir) = garden().await;
    let snapshot = server.state.store.mirror_snapshot(work).await.unwrap();
    assert!(!snapshot.iter().any(|f| f.path == "befunde.md"), "ohne Befunde keine befunde.md");
    let (_, report) = answer(&server, work, 2, keys[2], "@lektor", "Kritisch: „zwei Finger tief“ ist zu genau.").await;
    let id = report.created[0].id;
    let before = commit_count(&dir);
    result(
        &server
            .rpc(
                "finding.set_status",
                json!({ "work": work, "id": id, "status": "done", "reason": "steht so im Saatbuch" }),
            )
            .await,
    );
    result(&server.rpc("finding.set_status", json!({ "work": work, "id": id, "status": "adopted", "rev": 1 })).await);
    if let (Some(before), Some(after)) = (before, commit_count(&dir)) {
        assert_eq!(after, before + 2, "zwei Statusänderungen, zwei Commits");
    }
    let Ok(text) = std::fs::read_to_string(dir.join("befunde.md")) else {
        panic!("befunde.md fehlt im Spiegel nach einem Commit");
    };
    for needle in [
        format!("### [B{id}] kritisch · übernommen · @lektor"),
        "- Zitat: „zwei Finger tief“".to_owned(),
        "- Übernommen in Fassung 1".to_owned(),
        "Autor setzt erledigt ohne Änderung – steht so im Saatbuch".to_owned(),
    ] {
        assert!(text.contains(&needle), "{needle} in befunde.md:\n{text}");
    }
    let again = server.state.store.mirror_snapshot(work).await.unwrap();
    let again = again.iter().find(|f| f.path == "befunde.md").unwrap();
    assert_eq!(again.contents, text, "deterministisch: der Schnappschuss gibt dieselbe Datei");
}

async fn exchange_import(server: &common::Server, bytes: &[u8], mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(bytes), "sha256": baruch_exchange::sha256_hex(bytes), "mode": mode }),
        )
        .await;
    result(&answer).clone()
}

async fn exchange_export(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    result(&answer)["xml"].as_str().unwrap().to_owned()
}

fn without_output(text: &str) -> String {
    let start = text.find("<ausgabe").unwrap();
    let end = start + text[start..].find("/>").unwrap() + 2;
    format!("{}{}", &text[..start], &text[end..])
}

/// Findings of a work as comparable rows (without ids and times of the instance).
async fn finding_rows(server: &common::Server, work: i64) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT f.uuid || '|' || f.kind || '|' || f.status || '|' || f.reason || '|' || coalesce(f.adopted_revision, '') \
         || '|' || (SELECT count(*) FROM finding_status s WHERE s.finding_id = f.id) FROM finding f WHERE f.work_id = ? \
         ORDER BY f.uuid",
    )
    .bind(work)
    .fetch_all(server.state.store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn findings_and_old_post_versions_travel_between_two_instances_in_version_four() {
    let (a, work_a, keys, _) = garden().await;
    let (post_a, report) = answer(&a, work_a, 1, keys[1], "@lektor", REVIEW).await;
    let ids: Vec<i64> = report.created.iter().map(|f| f.id).collect();
    result(
        &a.rpc(
            "finding.set_status",
            json!({ "work": work_a, "id": ids[3], "status": "rejected", "reason": "Absicht" }),
        )
        .await,
    );
    result(&a.rpc("finding.set_status", json!({ "work": work_a, "id": ids[1], "status": "adopted", "rev": 1 })).await);
    let lektor = baruch_store::IdeaOrigin::Persona { handle: "@lektor".into() };
    findings::rpc::propose(
        &a.state,
        work_a,
        ids[2],
        &baruch_store::StatusChange::Done { reason: String::new() },
        &lektor,
        "",
    )
    .await
    .unwrap();
    // An edited post of the human (the editing itself is built by chat-auftrag; here the rows as they come into being).
    let own = a.state.store.add_post(work_a, 0, &post("Autor", "Erste Fassung meiner Notiz.")).await.unwrap();
    sqlx::query(
        "INSERT INTO post_version (post_id, body, valid_from, replaced, edited_by) \
         SELECT id, body, created, '2026-11-09T10:00:00.000Z', 'Autor' FROM post WHERE id = ?",
    )
    .bind(own)
    .execute(a.state.store.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE post SET body = 'Zweite Fassung.', edited = '2026-11-09T10:00:00.000Z' WHERE id = ?")
        .bind(own)
        .execute(a.state.store.pool())
        .await
        .unwrap();

    let xml_a = exchange_export(&a, work_a).await;
    for needle in
        // Round 34 (mindmap_daten): the writer now writes minor version 9 (tree of the ideas).
        [
            "version=\"10\"",
            "<befunde>",
            "art=\"kritisch\" status=\"verworfen\"",
            "<alte-fassung ",
            "vorschlag=\"true\"",
        ]
    {
        assert!(xml_a.contains(needle), "{needle} in der Ausgabe:\n{xml_a}");
    }
    // XSD u4 (xmllint; if it is missing, honestly skipped).
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.baruch.xml");
    std::fs::write(&file, &xml_a).unwrap();
    let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u10.xsd");
    match std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&file)
        .output()
    {
        Ok(output) => assert!(output.status.success(), "XSD u4: {}", String::from_utf8_lossy(&output.stderr)),
        Err(e) => eprintln!("ÜBERSPRUNGEN: xmllint nicht gefunden ({e})"),
    }

    let b = common::open().await;
    let imported = exchange_import(&b, xml_a.as_bytes(), "new").await;
    let work_b = imported["work"].as_i64().unwrap();
    assert_eq!(
        finding_rows(&b, work_b).await,
        finding_rows(&a, work_a).await,
        "gleiche Befunde, Status, Fassungen, Verlauf"
    );
    let xml_b = exchange_export(&b, work_b).await;
    assert_eq!(without_output(&xml_b), without_output(&xml_a), "Rückweg ohne Verlust (bis auf <ausgabe>)");
    let versions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM post_version").fetch_one(b.state.store.pool()).await.unwrap();
    assert_eq!(versions, 1, "die alte Fassung des Beitrags kam mit");
    let counts_a = a.state.store.finding_open_counts(work_a).await.unwrap();
    let counts_b = b.state.store.finding_open_counts(work_b).await.unwrap();
    assert_eq!(counts_a.values().sum::<usize>(), counts_b.values().sum::<usize>(), "gleich viele offene Befunde");
    // Merging the same file brings nothing twice.
    let merged = exchange_import(&b, xml_a.as_bytes(), "merge").await;
    assert_eq!(finding_rows(&b, work_b).await.len(), report.created.len(), "kein Befund doppelt: {merged}");
    let _ = post_a;
}
