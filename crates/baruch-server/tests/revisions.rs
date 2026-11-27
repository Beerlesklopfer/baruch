// SPDX-License-Identifier: AGPL-3.0-or-later
//! Revision tree through RPC, `/act` and user interface (contract Round 3, sections 3 and 4): switching, changing,
//! deleting, tree and saving on a base; every switch, change and deletion as exactly one commit in the Git mirror; gaps
//! in the number sequence; tree view and preview with `pick`. Real data folder (SQLite, library, Git mirror) in the
//! temp folder.

use crate::common;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::rpc;
use serde_json::{Value, json};

/// Mirror file of a section (`abschnitte/NNN-…`).
fn section_file(dir: &Path, seq: usize) -> PathBuf {
    let prefix = format!("{seq:03}-");
    std::fs::read_dir(dir.join("abschnitte"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(&prefix))
        .unwrap_or_else(|| panic!("Datei für Abschnitt {seq} fehlt"))
}

fn commit_count(dir: &Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|n| n.trim().parse().unwrap())
}

/// Subject, author, committer, body and trailers of the last commit.
struct LastCommit {
    subject: String,
    author: String,
    committer: String,
    message: String,
    trailers: Vec<String>,
    files: Vec<String>,
}

fn last_commit(dir: &Path) -> Option<LastCommit> {
    let head = common::git(dir, &["log", "-1", "--format=%s%n%an <%ae>%n%cn <%ce>"])?;
    let mut lines = head.lines();
    let subject = lines.next().unwrap().to_owned();
    let author = lines.next().unwrap().to_owned();
    let committer = lines.next().unwrap().to_owned();
    let message = common::git(dir, &["log", "-1", "--format=%B"]).unwrap();
    let trailers = common::git(dir, &["log", "-1", "--format=%(trailers:only,unfold)"])
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let files = common::git(dir, &["show", "--name-only", "--format=", "HEAD"])
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    Some(LastCommit { subject, author, committer, message, trailers, files })
}

fn revisions(node: &Value) -> Vec<(u64, String)> {
    node["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["number"].as_u64().unwrap(), r["status"].as_str().unwrap().to_owned()))
        .collect()
}

async fn node(server: &common::Server, id: usize) -> Value {
    server.rpc("node.get", json!({ "node": id })).await["result"].clone()
}

/// Introduction (section 2) in the sample work: 1 superseded, 2 current, 3 proposal by @lektor.
const INTRO_V1_LINE: &str = "Es richtet sich an Betroffene und an Gemeinden.";
const INTRO_V2_LINE: &str = "Es richtet sich an Betroffene, an Seelsorger und an Gemeinden.";

// ---- Switching -----------------------------------------------------------------------------------

#[tokio::test]
async fn switching_to_an_older_version_makes_exactly_one_commit_with_trailers() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let before = commit_count(&dir);
    let answer = server.rpc("revision.set_current", json!({ "node": 2, "rev": 1 })).await;
    assert_eq!(answer["result"]["rev"], 1, "Antwort nennt die neue aktuelle Fassung: {answer}");
    assert!(answer["result"]["commit"].is_string(), "Umschalten wird committet: {answer}");
    assert_eq!(
        revisions(&node(&server, 2).await),
        [(1, "current".into()), (2, "superseded".into()), (3, "proposed".into())],
        "genau eine aktuelle Fassung, der Vorschlag bleibt offen"
    );
    let file =
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(section_file(&dir, 2)).unwrap()).into_owned();
    assert!(file.contains(INTRO_V1_LINE), "Spiegel zeigt die umgeschaltete Fassung: {file}");

    let Some(count) = before else {
        eprintln!("git fehlt – Commit nicht geprüft");
        return;
    };
    assert_eq!(commit_count(&dir).unwrap(), count + 1, "genau ein Commit");
    let log = last_commit(&dir).unwrap();
    assert!(log.subject.ends_with("1 Einleitung: auf Fassung 1 umgeschaltet"), "Betreff: {}", log.subject);
    assert_eq!(log.author, "Autor <autor@baruch.invalid>", "Autor ist der Autor der Fassung (Mensch)");
    assert_eq!(log.committer, "Autor <autor@baruch.invalid>", "Committer ist der Mensch");
    assert_eq!(
        log.trailers,
        ["Baruch-Node: 2", "Baruch-Revision: 1", "Baruch-Action: switched"],
        "Trailer beim Umschalten auf eine eigene Fassung"
    );

    let again = server.rpc("revision.set_current", json!({ "node": 2, "rev": 1 })).await;
    assert_eq!(again["error"]["code"], rpc::REFUSED, "schon aktuell → fachlich abgelehnt: {again}");
    assert_eq!(
        again["error"]["message"], "Diese Fassung ist schon aktuell.",
        "Umschalten auf die aktuelle Fassung wird abgelehnt"
    );
    assert_eq!(commit_count(&dir).unwrap(), count + 1, "Ablehnung committet nicht");
    let unknown = server.rpc("revision.set_current", json!({ "node": 2, "rev": 9 })).await;
    assert_eq!(unknown["error"]["code"], rpc::INVALID_PARAMS, "unbekannte Fassung: {unknown}");
}

#[tokio::test]
async fn switching_to_a_proposal_is_accepting_it() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let answer = server.rpc("revision.set_current", json!({ "node": 2, "rev": 3 })).await;
    assert!(answer["result"]["commit"].is_string(), "{answer}");
    let Some(log) = last_commit(&dir) else { return };
    assert!(
        log.subject.ends_with(": Vorschlag 3 übernommen"),
        "Umschalten auf einen Vorschlag = Übernehmen: {}",
        log.subject
    );
    assert!(log.author.ends_with("(@lektor) <lektor@persona.baruch.invalid>"), "Autor ist die Persona: {}", log.author);
    assert!(log.trailers.contains(&"Persona: @lektor".to_owned()), "Persona-Trailer: {:?}", log.trailers);
    assert!(
        !log.trailers.iter().any(|t| t.starts_with("Baruch-Action")),
        "Übernehmen ohne Baruch-Action: {:?}",
        log.trailers
    );
}

#[tokio::test]
async fn a_broken_mirror_is_reported_and_the_switch_stays() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    std::fs::remove_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join(".git"), "kaputt").unwrap();
    let answer = server.rpc("revision.set_current", json!({ "node": 2, "rev": 1 })).await;
    assert!(answer["result"]["mirror"].is_string(), "Spiegelfehler gemeldet: {answer}");
    assert!(answer["result"].get("commit").is_none(), "kein Commit trotz Fehler: {answer}");
    assert_eq!(node(&server, 2).await["revisions"][0]["status"], "current", "die Datenbank ist die Quelle");
}

// ---- Changing ------------------------------------------------------------------------------------

#[tokio::test]
async fn updating_an_older_version_logs_an_empty_commit_with_the_diff() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let before_count = commit_count(&dir);
    let mirrored =
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(section_file(&dir, 2)).unwrap()).into_owned();
    let body =
        "= Einleitung\n\nDieses Buch handelt vom Quittenbaum.\nEs richtet sich an Betroffene und an Gemeinden.\n";
    let answer =
        server.rpc("revision.update", json!({ "node": 2, "rev": 1, "body": body, "note": "neu & <besser>" })).await;
    assert_eq!(answer["result"]["changed"], true, "{answer}");
    assert_eq!(answer["result"]["rev"], 1, "Antwort nennt die geänderte Fassung: {answer}");
    assert!(answer["result"]["commit"].is_string(), "Änderung wird protokolliert: {answer}");
    let n = node(&server, 2).await;
    assert_eq!(n["revisions"][0]["body"], body, "Text geändert");
    assert_eq!(n["revisions"][0]["note"], "neu & <besser>", "Notiz geändert");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(section_file(&dir, 2)).unwrap()).into_owned(),
        mirrored,
        "Spiegeldatei unverändert"
    );

    // Full-text search (migration 0104): the new text, no longer the old one.
    let hits = server.rpc("library.search", json!({ "query": "Quittenbaum", "all_revisions": true })).await;
    let hits = hits["result"]["hits"].as_array().unwrap().clone();
    assert!(hits.iter().any(|h| h["node"] == 2 && h["rev"] == 1), "neuer Text im Index: {hits:?}");
    let old = server
        .rpc(
            "library.search",
            json!({ "query": "Abhängigkeit Gemeinden", "kinds": ["revision"], "all_revisions": true }),
        )
        .await;
    let old: Vec<Value> = old["result"]["hits"].as_array().unwrap().clone();
    assert!(!old.iter().any(|h| h["node"] == 2 && h["rev"] == 1), "alter Text nicht mehr im Index: {old:?}");

    let Some(count) = before_count else { return };
    assert_eq!(commit_count(&dir).unwrap(), count + 1, "genau ein Commit");
    let log = last_commit(&dir).unwrap();
    assert!(log.subject.ends_with("1 Einleitung: Fassung 1 geändert"), "Betreff: {}", log.subject);
    assert!(log.files.is_empty(), "leerer Protokoll-Commit (allow_empty): {:?}", log.files);
    assert_eq!(log.author, "Autor <autor@baruch.invalid>", "geändert hat der Mensch");
    assert_eq!(log.committer, "Autor <autor@baruch.invalid>", "Committer ist der Mensch");
    assert!(log.message.contains("- Dieses Buch handelt von Abhängigkeit."), "Zeilen-Diff vorher: {}", log.message);
    assert!(log.message.contains("+ Dieses Buch handelt vom Quittenbaum."), "Zeilen-Diff nachher: {}", log.message);
    assert!(
        log.message.contains("Notiz vorher: –") && log.message.contains("Notiz nachher: neu & <besser>"),
        "{}",
        log.message
    );
    assert_eq!(log.trailers, ["Baruch-Node: 2", "Baruch-Revision: 1", "Baruch-Action: updated"], "Trailer");
    assert_eq!(common::git(&dir, &["status", "--porcelain"]).unwrap(), "", "Spiegel sauber");
}

#[tokio::test]
async fn updating_the_current_version_changes_the_mirror_file() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let body = "= Einleitung\n\nKorrigiert.\n";
    let answer = server.rpc("revision.update", json!({ "node": 2, "rev": 2, "body": body })).await;
    assert_eq!(answer["result"]["changed"], true, "{answer}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(section_file(&dir, 2)).unwrap()).into_owned(),
        body,
        "Spiegeldatei trägt den neuen Text"
    );
    assert_eq!(revisions(&node(&server, 2).await)[1], (2, "current".into()), "Status bleibt aktuell");
    if let Some(log) = last_commit(&dir) {
        assert_eq!(log.files.len(), 1, "der Commit ändert die Datei des Abschnitts: {:?}", log.files);
        assert!(log.files[0].starts_with("abschnitte/002-"), "{:?}", log.files);
    }

    let same = server.rpc("revision.update", json!({ "node": 2, "rev": 2, "body": body })).await;
    assert_eq!(same["result"]["changed"], false, "gleicher Text: nichts geändert: {same}");
    assert!(same["result"].get("commit").is_none(), "ohne Änderung kein Commit: {same}");
    let empty = server.rpc("revision.update", json!({ "node": 2, "rev": 2, "body": "  " })).await;
    assert_eq!(empty["error"]["code"], rpc::REFUSED, "leerer Text abgelehnt: {empty}");
    assert_eq!(empty["error"]["message"], "Eine Fassung braucht Text.", "leerer Text wird abgelehnt");
}

// ---- Deleting ------------------------------------------------------------------------------------

#[tokio::test]
async fn deleting_a_middle_version_reparents_and_logs_the_full_text() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    server.rpc("revision.set_current", json!({ "node": 2, "rev": 3 })).await;
    let id: i64 = sqlx::query_scalar(
        "SELECT r.id FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = 1 AND n.seq = 2 AND r.number = 2",
    )
    .fetch_one(server.state.store.pool())
    .await
    .unwrap();
    let before_count = commit_count(&dir);
    let answer = server.rpc("revision.delete", json!({ "node": 2, "rev": 2, "confirm": true })).await;
    let result = &answer["result"];
    assert_eq!(result["deleted"], 2, "{answer}");
    assert_eq!(result["rev"], 3, "Antwort nennt die aktuelle Fassung: {answer}");
    assert_eq!(result["reparented"], json!([3]), "Fassung 3 hing an 2: {answer}");
    assert!(result["commit"].is_string(), "Löschen wird protokolliert: {answer}");
    let tree = server.rpc("revision.tree", json!({ "node": 2 })).await;
    let parents: Vec<(Value, Value)> = tree["result"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["number"].clone(), r["based_on"].clone()))
        .collect();
    assert_eq!(parents, [(json!(1), Value::Null), (json!(3), json!(1))], "3 hängt jetzt an 1");
    let indexed: i64 = sqlx::query_scalar("SELECT count(*) FROM search_fts WHERE kind = 'revision' AND row_id = ?")
        .bind(id)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(indexed, 0, "die gelöschte Fassung ist nicht mehr im Index");

    let Some(count) = before_count else { return };
    assert_eq!(commit_count(&dir).unwrap(), count + 1, "genau ein Commit");
    let log = last_commit(&dir).unwrap();
    assert!(log.subject.ends_with("1 Einleitung: Fassung 2 gelöscht"), "Betreff: {}", log.subject);
    assert!(log.files.is_empty(), "die gelöschte Fassung stand nicht im Spiegel: {:?}", log.files);
    assert!(log.message.contains("Gelöschter Text:\n= Einleitung\n\n"), "{}", log.message);
    assert!(log.message.contains(INTRO_V2_LINE), "vollständiger alter Text im Commit: {}", log.message);
    for part in ["Autor: Autor", "Status: ersetzt", "Elternteil: Fassung 1", "Umgehängt: 3 → Fassung 1"] {
        assert!(log.message.contains(part), "{part} fehlt: {}", log.message);
    }
    assert_eq!(log.trailers, ["Baruch-Node: 2", "Baruch-Revision: 2", "Baruch-Action: deleted"], "Trailer");
}

#[tokio::test]
async fn deleting_the_current_version_is_refused_with_a_german_message() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let before = node(&server, 2).await;
    let head = common::git(&dir, &["rev-parse", "HEAD"]);
    let answer = server.rpc("revision.delete", json!({ "node": 2, "rev": 2, "confirm": true })).await;
    assert_eq!(answer["error"]["code"], rpc::REFUSED, "{answer}");
    assert_eq!(
        answer["error"]["message"], "Die aktuelle Fassung lässt sich nicht löschen; erst auf eine andere umschalten.",
        "deutsche Meldung"
    );
    assert_eq!(node(&server, 2).await, before, "nichts geändert");
    assert_eq!(common::git(&dir, &["rev-parse", "HEAD"]), head, "kein Commit");
}

#[tokio::test]
async fn deleting_over_rpc_needs_confirm_true() {
    let server = common::open().await;
    let dir = server.mirror_dir();
    let before = node(&server, 2).await;
    let head = common::git(&dir, &["rev-parse", "HEAD"]);
    for (params, case) in [
        (json!({ "node": 2, "rev": 1 }), "ohne confirm"),
        (json!({ "node": 2, "rev": 1, "confirm": false }), "confirm: false"),
        (json!({ "node": 2, "rev": 1, "confirm": "1" }), "confirm als Text statt true"),
    ] {
        let answer = server.rpc("revision.delete", params).await;
        assert_eq!(answer["error"]["code"], rpc::INVALID_PARAMS, "{case}: ungültige Parameter: {answer}");
        assert_eq!(
            answer["error"]["message"], "Löschen bitte bestätigen (confirm).",
            "{case}: deutsche Meldung mit dem Feldnamen"
        );
    }
    assert_eq!(node(&server, 2).await, before, "ohne Bestätigung bleibt Fassung 1 erhalten");
    assert_eq!(common::git(&dir, &["rev-parse", "HEAD"]), head, "ohne Bestätigung kein Commit");
    let answer = server.rpc("revision.delete", json!({ "node": 2, "rev": 1, "confirm": true })).await;
    assert_eq!(answer["result"]["deleted"], 1, "mit confirm: true wird gelöscht wie bisher: {answer}");
}

#[tokio::test]
async fn deleting_over_act_needs_the_checked_box() {
    let server = common::open().await;
    let before = node(&server, 2).await;
    let request = Request::post("/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("method=revision.delete&work=1&node=2&rev=1"))
        .unwrap();
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Kästchen lehnt der Server ab: {body}");
    assert_eq!(body, "Löschen bitte bestätigen.", "deutsche Meldung");
    let (status, _) = server.post_form("/act", "method=revision.delete&work=1&node=2&rev=1&confirm=nein").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "nur 1 oder on gelten als angehakt");
    assert_eq!(node(&server, 2).await, before, "die Fassung bleibt, solange nicht bestätigt ist");
    let (status, location) = server.post_form("/act", "method=revision.delete&work=1&node=2&rev=1&confirm=on").await;
    assert!(status.is_redirection(), "mit confirm=on (Browser-Vorgabe für Kästchen) gelöscht: {status}");
    let k = key_at(&server, 1, 2).await;
    assert_eq!(location, Some(format!("/?work=1&key={k}&rev=2")), "danach zur aktuellen Fassung");
    let numbers: Vec<u64> = revisions(&node(&server, 2).await).iter().map(|r| r.0).collect();
    assert_eq!(numbers, [2, 3], "Fassung 1 ist gelöscht");
}

#[tokio::test]
async fn an_unknown_number_after_a_deletion_lists_the_existing_ones() {
    let server = common::open().await;
    // Introduction: 1, 2 (current), 3; save revision 4, delete 3 → 1, 2, 4.
    server.rpc("revision.save", json!({ "node": 2, "body": "= Einleitung\n\nVierte.\n" })).await;
    let deleted = server.rpc("revision.delete", json!({ "node": 2, "rev": 3, "confirm": true })).await;
    assert_eq!(deleted["result"]["deleted"], 3, "Fassung 3 gelöscht: {deleted}");
    let answer = server.rpc("revision.set_current", json!({ "node": 2, "rev": 3 })).await;
    assert_eq!(answer["error"]["code"], rpc::INVALID_PARAMS, "unbekannte Nummer: {answer}");
    assert_eq!(
        answer["error"]["message"], "Fassung 3 gibt es nicht; vorhanden: 1, 2, 4",
        "die Meldung nennt die Nummern mit Lücke statt eines Bereichs"
    );
}

#[test]
fn the_workspace_script_confirms_deleting_and_leaves_alt_shift_to_the_outline() {
    let script = include_str!("../web/src/workspace.ts");
    assert!(script.contains("params.confirm = true;"), "angehaktes Kästchen wird zu confirm: true");
    assert!(
        script.contains("if (!event.altKey || event.shiftKey ||"),
        "Alt+←/→ blättert nur ohne Umschalt (Alt+Umschalt gehört der Gliederung)"
    );
    assert!(script.contains("\"baruch:navigate\""), "hört auf das Ereignis baruch:navigate");
    assert!(script.contains("detail?.url"), "nimmt die Adresse aus detail.url");
    assert!(script.contains("void navigate(url)"), "das Ereignis zeichnet über navigate (ui.render) neu");
}

#[test]
fn outline_changes_and_other_forms_ask_before_dropping_unsaved_editor_text() {
    // Without a browser only checkable on the source text: every way that redraws the areas asks first.
    let workspace = include_str!("../web/src/workspace.ts");
    assert!(
        workspace.contains("if (detail?.asked === true || leaveEditor()) void navigate(url);"),
        "baruch:navigate fragt bei ungespeichertem Text, außer der Absender hat schon gefragt"
    );
    assert!(
        workspace.contains("if (!form.matches(\"[data-widget=editor]\") && !leaveEditor()) return;"),
        "Formulare außer dem Editor (Gliederung ↑ ↓ ← →, Umschalten, Beitrag) fragen vor dem Absenden"
    );
    let outline = include_str!("../web/src/outline.ts");
    assert!(
        outline.contains("if (!leaveEditor()) return false;"),
        "Ziehen und Ablegen und Alt+Umschalt+Pfeil fragen vor dem RPC-Aufruf, nicht erst danach"
    );
    assert!(outline.contains("asked: true"), "nach der Rückfrage meldet outline.ts das Neuzeichnen als gefragt");
    let widgets = include_str!("../web/src/widgets.ts");
    assert!(
        widgets.contains("export function leaveEditor(): boolean"),
        "eine gemeinsame Rückfrage für alle Widgets statt Kopien"
    );
}

// ---- Tree and saving on a base -------------------------------------------------------------------

#[tokio::test]
async fn the_tree_answers_for_a_section_a_chapter_and_the_whole_work() {
    let server = common::open().await;
    let section = server.rpc("revision.tree", json!({ "node": 2 })).await;
    let revs = section["result"]["revisions"].as_array().unwrap();
    assert_eq!(revs.len(), 3, "{section}");
    assert_eq!(revs[1]["current"], true, "Fassung 2 ist aktuell: {section}");
    assert_eq!(revs[2]["based_on"], 2, "Vorschlag beruht auf 2");
    assert_eq!(revs[2]["author"]["persona"], "@lektor", "Autor mit Persona und Modell: {section}");
    assert!(revs[0]["created"].is_string() && revs[0]["edited"].is_null(), "Zeit, nie geändert: {section}");

    let chapter = server.rpc("revision.tree", json!({ "node": 4, "chapter": true })).await;
    let nodes: Vec<Value> =
        chapter["result"]["sections"].as_array().unwrap().iter().map(|s| s["node"].clone()).collect();
    assert_eq!(nodes, [json!(3), json!(4), json!(5)], "Kapitel 2 mit 2.1 und 2.2: {chapter}");
    assert_eq!(chapter["result"]["sections"][1]["current"], 2, "aktuelle Fassung von 2.1");
    assert_eq!(chapter["result"]["sections"][1]["title"], "Erste Anzeichen", "das Kapitel enthält seinen Abschnitt");

    let all = server.rpc("revision.tree", json!({})).await;
    assert_eq!(all["result"]["sections"].as_array().unwrap().len(), 11, "ohne node: alle Abschnitte");
    let bad = server.rpc("revision.tree", json!({ "node": 99 })).await;
    assert_eq!(bad["error"]["code"], rpc::INVALID_PARAMS, "unbekannter Abschnitt: {bad}");
}

#[tokio::test]
async fn saving_on_a_shown_version_takes_it_as_parent() {
    let server = common::open().await;
    let answer =
        server.rpc("revision.save", json!({ "node": 2, "body": "= Einleitung\n\nAuf 1.\n", "based_on": 1 })).await;
    assert_eq!(answer["result"]["rev"], 4, "{answer}");
    let tree = server.rpc("revision.tree", json!({ "node": 2 })).await;
    let new = &tree["result"]["revisions"][3];
    assert_eq!((new["number"].clone(), new["based_on"].clone()), (json!(4), json!(1)), "Elternteil = gezeigte Fassung");
    assert_eq!(new["current"], true, "sofort aktuell");
    let plain = server.rpc("revision.save", json!({ "node": 4, "body": "== Erste Anzeichen\n\nOhne Basis.\n" })).await;
    let tree = server.rpc("revision.tree", json!({ "node": 4 })).await;
    let new = tree["result"]["revisions"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(new["number"], plain["result"]["rev"], "{plain}");
    assert_eq!(new["based_on"], 2, "ohne based_on: die aktuelle Fassung");
    let unknown = server.rpc("revision.save", json!({ "node": 2, "body": "x", "based_on": 42 })).await;
    assert_eq!(unknown["error"]["code"], rpc::INVALID_PARAMS, "unbekannte Basis: {unknown}");
}

// ---- Without JavaScript through /act -------------------------------------------------------------

#[tokio::test]
async fn the_same_actions_work_without_javascript_and_redirect() {
    let server = common::open().await;
    let (status, location) = server.post_form("/act", "method=revision.set_current&work=1&node=2&rev=1").await;
    assert!(status.is_redirection(), "Weiterleitung: {status}");
    let k = key_at(&server, 1, 2).await;
    assert_eq!(location, Some(format!("/?work=1&key={k}&rev=1")), "zurück zur umgeschalteten Fassung");
    assert_eq!(node(&server, 2).await["revisions"][0]["status"], "current", "Fassung 1 ist jetzt aktuell");

    let form = "method=revision.update&work=1&node=2&rev=3&body=%3D+Einleitung%0A%0AGe%C3%A4ndert.%0A&note=von+Hand&view=preview&lq=Liebe";
    let (status, location) = server.post_form("/act", form).await;
    assert!(status.is_redirection(), "{status}");
    assert_eq!(location, Some(format!("/?work=1&key={k}&rev=3&view=preview&lq=Liebe")), "Ansicht und Suche bleiben");
    let n = node(&server, 2).await;
    assert_eq!(n["revisions"][2]["body"], "= Einleitung\n\nGeändert.\n", "Text über das Formular geändert");
    assert_eq!(n["revisions"][2]["note"], "von Hand", "die Notiz wurde geändert");

    let (status, location) = server
        .post_form("/act", "method=revision.save&work=1&node=2&based_on=3&body=%3D+Einleitung%0A%0AEigen.%0A")
        .await;
    assert!(status.is_redirection(), "{status}");
    assert_eq!(location, Some(format!("/?work=1&key={k}&rev=4")), "neue Fassung 4");
    let tree = server.rpc("revision.tree", json!({ "node": 2 })).await;
    assert_eq!(tree["result"]["revisions"][3]["based_on"], 3, "based_on aus dem versteckten Feld: {tree}");

    let (status, location) = server.post_form("/act", "method=revision.delete&work=1&node=2&rev=2&confirm=1").await;
    assert!(status.is_redirection(), "{status}");
    assert_eq!(location, Some(format!("/?work=1&key={k}&rev=4")), "nach dem Löschen zur aktuellen Fassung");
    assert_eq!(revisions(&node(&server, 2).await).iter().map(|r| r.0).collect::<Vec<_>>(), [1, 3, 4], "2 gelöscht");

    let (status, _) = server.post_form("/act", "method=revision.delete&work=1&node=2&rev=4&confirm=1").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "aktuelle Fassung über /act nicht löschbar");
}

// ---- User interface ------------------------------------------------------------------------------

#[tokio::test]
async fn the_switcher_highlights_the_current_version_and_offers_the_buttons() {
    let server = common::open().await;
    let (_, current) = server.get("/?work=1&node=2&rev=2").await;
    assert!(current.contains("<span class=\"counter is-current\""), "aktuelle Fassung hervorgehoben: {current}");
    assert!(current.contains(">2/3</span>"), "Stelle/Anzahl: {current}");
    assert!(
        current.contains("<button class=\"btn danger\" type=\"submit\" disabled>Löschen</button>"),
        "Löschen bei der aktuellen Fassung gesperrt: {current}"
    );
    assert!(current.contains("Die aktuelle Fassung lässt sich nicht löschen"), "Hinweis beim gesperrten Knopf");
    assert!(!current.contains("Diese Fassung verwenden"), "die aktuelle Fassung braucht keinen Umschaltknopf");
    assert!(current.contains("<input type=\"hidden\" name=\"based_on\" value=\"2\">"), "Speichern trägt die Basis");
    assert!(current.contains("<summary class=\"btn\">Fassung ändern</summary>"), "Ändern als <details>-Formular");
    assert!(current.contains("value=\"revision.update\""), "Formular für revision.update");

    let (_, older) = server.get("/?work=1&node=2&rev=1").await;
    assert!(older.contains("<span class=\"counter\""), "nicht aktuell: ohne Hervorhebung: {older}");
    assert!(older.contains(">aktuell: Fassung 2</a>"), "Link auf die aktuelle Fassung: {older}");
    assert!(older.contains("Diese Fassung verwenden"), "Umschaltknopf: {older}");
    assert!(older.contains("value=\"revision.set_current\""), "Formular für revision.set_current");
    assert!(older.contains("name=\"confirm\" value=\"1\" required"), "Pflicht-Kontrollkästchen vor dem Löschen");
    assert!(!older.contains("type=\"submit\" disabled>Löschen"), "Löschen ist hier frei");

    let (_, proposal) = server.get("/?work=1&node=2&rev=3").await;
    assert!(
        proposal.contains(">Übernehmen</button>") && proposal.contains(">Verwerfen</button>"),
        "Vorschlag im Lesefluss mit Übernehmen und Verwerfen"
    );
    assert!(!proposal.contains("Diese Fassung verwenden"), "beim Vorschlag heißt Umschalten „Übernehmen“");
    assert!(proposal.contains("Unterschied zu Fassung 2 (aktuell)"), "Diff gegen die Basis: {proposal}");
}

#[tokio::test]
async fn a_stale_proposal_is_diffed_against_its_base() {
    let server = common::open().await;
    // The human saves a revision 4 on 2; proposal 3 is still based on 2.
    server.rpc("revision.save", json!({ "node": 2, "body": "= Einleitung\n\nGanz neu.\n" })).await;
    let ui = server.rpc("ui.render", json!({ "node": 2, "rev": 3 })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    // Round 7: the proposal is a variant in the reading flow (fragments/proposals.html), the head names the base.
    assert!(
        editor.contains("Unterschied zu Fassung 2 · gezeigt</span>"),
        "Basis ist 2, nicht die aktuelle 4 (ohne „(aktuell)“): {editor}"
    );
    assert!(!editor.contains("Ganz neu."), "die neue eigene Fassung erscheint nicht im Diff des alten Vorschlags");
}

#[tokio::test]
async fn after_deleting_a_middle_version_numbers_not_positions_are_used() {
    let server = common::open().await;
    // Introduction: 1, 2 (current), 3 → switch to 3, delete 2: 1, 3.
    server.rpc("revision.set_current", json!({ "node": 2, "rev": 3 })).await;
    server.rpc("revision.delete", json!({ "node": 2, "rev": 2, "confirm": true })).await;

    let ui = server.rpc("ui.render", json!({ "node": 2, "rev": 3 })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    let k = key_at(&server, 1, 2).await;
    assert_eq!(ui["result"]["url"], format!("/?work=1&key={k}&rev=3"), "Adresse trägt Schlüssel und Nummer: {ui}");
    assert!(editor.contains(">2/2</span>"), "Stelle 2 von 2: {editor}");
    assert!(editor.contains("<span class=\"counter is-current\""), "3 ist aktuell und hervorgehoben");
    assert!(editor.contains("&rev=1\" title=\"Vorige Fassung"), "zurück blättert auf Nummer 1, nicht 2: {editor}");
    // The forms of the revision (switching, changing, deleting) are in the accordion „Fassungen“ on the right.
    let assistants = ui["result"]["assistants"].as_str().unwrap();
    assert!(assistants.contains("<input type=\"hidden\" name=\"rev\" value=\"3\">"), "Formulare tragen die Nummer");

    let ui = server.rpc("ui.render", json!({ "node": 2, "rev": 1 })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    assert!(
        editor.contains(">1/2</span>") && editor.contains("&rev=3\" title=\"Nächste Fassung"),
        "vor auf 3: {editor}"
    );
    assert!(editor.contains(INTRO_V1_LINE), "Fassung 1 im Editor");

    let gone = server.rpc("ui.render", json!({ "node": 2, "rev": 2 })).await;
    assert_eq!(
        gone["result"]["url"],
        format!("/?work=1&key={k}&rev=3"),
        "gelöschte Nummer führt auf die aktuelle Fassung"
    );

    // shown_current: unchanged text answers with the number of the current revision (3), not position + 1.
    let body = node(&server, 2).await["revisions"][1]["body"].as_str().unwrap().to_owned();
    let same = server.rpc("revision.save", json!({ "node": 2, "body": body })).await;
    assert_eq!(same["result"]["rev"], 3, "{same}");
    let (status, page) = server.get("/?work=1&node=2&rev=3").await;
    assert_eq!(status, StatusCode::OK, "Seite mit Lücke lädt: {page}");
}

#[tokio::test]
async fn the_tree_view_indents_by_parent_and_escapes_authors() {
    let server = common::open().await;
    let store = &server.state.store;
    store.add_proposal(1, 2, "= Einleitung\n\nZweiter Ast.\n", "@x<b>", "m&<i>", "").await.unwrap();
    store.save_human_on(1, 2, "= Einleitung\n\nAuf der Wurzel.\n", "Autor", 1).await.unwrap().unwrap();
    let ui = server.rpc("ui.render", json!({ "node": 2, "rev": 5 })).await;
    // The tree view is in the accordion „Fassungen“ of the right column (wish 2026-11-06).
    let editor = ui["result"]["assistants"].as_str().unwrap();
    let section = editor.split("revtree-chapter").next().unwrap();
    // Tree: 1 (root) → 2 → 3, 4 (both on 2); 1 → 5 (current).
    for (rev, depth) in [(1, 0), (2, 1), (3, 2), (4, 2), (5, 1)] {
        let needle = format!("style=\"--depth:{depth}\" data-rev=\"{rev}\"");
        assert!(section.contains(&needle), "Fassung {rev} mit Tiefe {depth} fehlt: {section}");
    }
    // Only the tree list: before it are the forms of the revision shown.
    let list = &section[section.find("revtree-list").expect("Baumliste im Akkordeon")..];
    let order: Vec<usize> = [1, 2, 3, 4, 5]
        .iter()
        .map(|r| list.find(&format!("data-rev=\"{r}\"")).expect("jede Fassung steht in der Baumliste"))
        .collect();
    assert!(order[3] < order[4] && order[2] < order[3], "Kinder direkt unter dem Elternteil: {order:?}");
    assert!(section.contains("is-current shown\" style=\"--depth:1\" data-rev=\"5\""), "aktuelle markiert: {section}");
    assert!(section.contains("@lektor (google&#x2f;gemma-4-26b-a4b)"), "Persona und Modell: {section}");
    assert!(section.contains("@x&lt;b&gt; (m&amp;&lt;i&gt;)"), "Autor maskiert: {section}");
    assert!(!editor.contains("<b>") && !editor.contains("<i>"), "kein rohes HTML aus Autorennamen");
    assert!(section.contains("in der Vorschau probieren"), "Link zum Probieren");
    assert!(section.contains("&amp;view=preview&amp;pick=2:1"), "pick-Link für Fassung 1: {section}");

    let ui = server.rpc("ui.render", json!({ "node": 4 })).await;
    let editor = ui["result"]["assistants"].as_str().unwrap();
    let chapter = editor.split("revtree-chapter").nth(1).expect("Kapitelansicht fehlt");
    assert!(
        chapter.contains("Das heimliche Muster") && chapter.contains("Fromme Begründungen"),
        "Kapitel 2 ganz: {chapter}"
    );
    assert!(chapter.contains("aktuell: Fassung 2"), "aktuelle Fassung je Abschnitt: {chapter}");
}

#[tokio::test]
async fn the_preview_sets_a_picked_version_without_switching() {
    let server = common::open().await;
    let ui = server.rpc("ui.render", json!({ "node": 2, "rev": 2, "view": "preview", "pick": "2:1" })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    assert!(editor.contains(&format!("<p>{INTRO_V1_LINE}</p>")), "Vorschau zeigt die gewählte Fassung 1: {editor}");
    assert!(!editor.contains(&format!("<p>{INTRO_V2_LINE}</p>")), "nicht die aktuelle Fassung 2");
    assert!(editor.contains("Probeweise gesetzt"), "Hinweis auf die Probe: {editor}");
    let k = key_at(&server, 1, 2).await;
    assert_eq!(
        ui["result"]["url"],
        format!("/?work=1&key={k}&rev=2&view=preview&pick=2:1"),
        "pick (weiter <seq>:<rev>) steht in der Adresse, nach view"
    );
    assert_eq!(node(&server, 2).await["revisions"][1]["status"], "current", "die Markierung bleibt bei Fassung 2");

    let (status, page) = server.get("/?work=1&node=2&view=preview&pick=2:1").await;
    assert_eq!(status, StatusCode::OK, "die Vorschau mit pick ist auch ohne JavaScript eine Seite: {page}");
    assert!(page.contains(&format!("<p>{INTRO_V1_LINE}</p>")), "auch ohne JavaScript: {page}");

    let bad = server.rpc("ui.render", json!({ "node": 2, "view": "preview", "pick": "2:99,x,7" })).await;
    let editor = bad["result"]["editor"].as_str().unwrap();
    assert!(editor.contains(&format!("<p>{INTRO_V2_LINE}</p>")), "ungültige Einträge übergangen: {editor}");
    assert!(!bad["result"]["url"].as_str().unwrap().contains("pick"), "ungültiges pick fällt aus der Adresse: {bad}");
}

// ---- MCP ------------------------------------------------------------------------------------------

#[tokio::test]
async fn mcp_offers_no_switching_editing_or_deleting() {
    let server = common::open().await;
    let mcp = |message: Value| {
        let mut request = Request::post("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(message.to_string()))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        request
    };
    let (status, _, body) = server.send(mcp(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let list: Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    // Ideas (idea_*, Round 7) and posts (post_*, Round 8) are no revisions; `idea_update`, `idea_delete` and
    // `post_delete` do not count. Images (asset_*, Round 11) are no revisions of sections; `asset_delete` only files a
    // request.
    for forbidden in ["set_current", "update", "delete", "switch", "accept", "reject", "save"] {
        assert!(
            !names.iter().any(|n| !n.starts_with("idea_")
                && !n.starts_with("post_")
                && !n.starts_with("outline_")
                && !n.starts_with("asset_")
                && n.contains(forbidden)),
            "MCP-Werkzeug mit „{forbidden}“ gefunden: {names:?}"
        );
    }
    let (_, _, body) = server
        .send(mcp(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "set_current", "arguments": { "node": 2, "rev": 1 } } })))
        .await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["error"]["code"], rpc::INVALID_PARAMS, "unbekanntes Werkzeug: {answer}");

    // The fixed key instead of the position (from Round 5 on MCP rejects `node`).
    let key = node(&server, 2).await["key"].clone();
    assert!(key.is_i64(), "node.get nennt den festen Schlüssel: {key}");
    let (_, _, body) = server
        .send(mcp(json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "propose_revision", "arguments": { "work": 1, "key": key, "body": "= Einleitung\n\nPer MCP.\n" } } })))
        .await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["isError"], false, "{answer}");
    let n = node(&server, 2).await;
    let last = n["revisions"].as_array().unwrap().last().unwrap();
    assert_eq!(last["status"], "proposed", "MCP legt nur Vorschläge an");
    assert_eq!(common::currents(&n), 1, "die Markierung bleibt");
}

/// Fixed key (`Node::key`) of the section at position `seq`: generated addresses carry it instead of the position
/// (`/?work=W&key=K`, contract Round 8).
async fn key_at(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}
