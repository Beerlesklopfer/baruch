// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 16c, outline requests:
//! - User decision 2026-11-08: declining and „veraltet“ are recorded for **every** kind of request (rename, move,
//!   set_level, set_numbered, delete, add) as exactly one empty mirror commit „Gliederung: Antrag N abgelehnt“ or
//!   „… veraltet“ with `Baruch-Decision` and `Baruch-Requested-By` – through RPC, through the form without JavaScript
//!   and through the consent dialog. „Später entscheiden“ stays unchanged.
//! - Preview of the request "new part": per item kind and number unambiguous („Teil …“, „Kapitel …“) and before → after
//!   for chapters taken over, on `/gliederung` (without JavaScript) and in the dialog.
//! - Times on `/gliederung`: column „Dauer“ with time mark, at the top total/target time with a warning – only for
//!   sermons or a set target time.
//!
//! All works and texts freely invented; requests are created directly in the store (as through MCP, origin
//! `@mcp:pruefer`).

use crate::common;

use axum::http::StatusCode;
use baruch_store::{AssistantAdd, IdeaOrigin, NewPoint, OutlineOp, Place, TimingSettings};
use serde_json::{Value, json};

const COAST: &str = "= Ebbe\n\nDas Wasser weicht.\n\n= Flut\n\nDas Wasser kommt.\n\n= Brandung\n\nGischt am Fels.\n";

const ASKER: &str = "@mcp:pruefer";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

fn origin() -> IdeaOrigin {
    IdeaOrigin::Mcp { handle: ASKER.to_owned() }
}

struct Coast {
    server: common::Server,
    work: i64,
    slug: String,
}

async fn coast() -> Coast {
    let server = common::open().await;
    let report = server.import(COAST, "kueste.typ").await;
    Coast { server, work: report.work, slug: report.slug }
}

impl Coast {
    async fn key(&self, title: &str) -> i64 {
        let loaded = self.server.state.store.load_work(self.work).await.unwrap();
        loaded.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("kein Punkt „{title}“")).key
    }

    async fn seq(&self, title: &str) -> usize {
        let loaded = self.server.state.store.load_work(self.work).await.unwrap();
        loaded.nodes.iter().find(|n| n.title == title).unwrap().id
    }

    async fn titles(&self) -> Vec<String> {
        let loaded = self.server.state.store.load_work(self.work).await.unwrap();
        loaded.nodes.iter().map(|n| n.title.clone()).collect()
    }

    /// Subject lines in the mirror (newest first); `None` without a git program.
    fn subjects(&self) -> Option<Vec<String>> {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "--format=%s"])
            .map(|text| text.lines().map(str::to_owned).collect())
    }

    fn last_commit(&self) -> Option<String> {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "-1", "--format=%an%n%B"])
    }

    fn last_commit_files(&self) -> Option<String> {
        common::git(&self.server.mirror_dir_of(&self.slug), &["show", "--stat", "--format=", "HEAD"])
    }

    /// File a request of kind `kind` (as through MCP); id of the request.
    async fn request(&self, kind: &str) -> i64 {
        let store = &self.server.state.store;
        let (ebb, flood, surf) = (self.key("Ebbe").await, self.key("Flut").await, self.key("Brandung").await);
        let op = match kind {
            "rename" => OutlineOp::Rename { key: flood, title: "Hochwasser".into() },
            "move" => OutlineOp::Move { key: surf, target_key: ebb, place: Place::Before },
            "set_level" => OutlineOp::SetLevel { key: surf, level: 2 },
            "set_numbered" => OutlineOp::SetNumbered { key: ebb, numbered: false },
            "delete" => OutlineOp::Delete { key: surf },
            "add" => {
                let point = NewPoint { title: "Vom Wasser".into(), level: Some(0) };
                let target = self.seq("Flut").await;
                let added = store
                    .outline_add_by_assistant(
                        self.work,
                        Some(target),
                        Place::Before,
                        &[point],
                        &origin(),
                        "nachbau",
                        "",
                    )
                    .await
                    .unwrap();
                return match added {
                    AssistantAdd::Requested(request) => request.id,
                    AssistantAdd::Added(_) => panic!("ein Teil vor „Flut“ übernimmt Kapitel und wird ein Antrag"),
                };
            }
            other => panic!("unbekannte Antragsart {other}"),
        };
        store.outline_request(self.work, op, &origin(), "zur Probe").await.unwrap().id
    }

    /// The item that the request `kind` concerns (its title makes it outdated on renaming).
    fn subject_of(kind: &str) -> &'static str {
        match kind {
            "rename" | "add" => "Flut",
            "set_numbered" => "Ebbe",
            _ => "Brandung",
        }
    }
}

const KINDS: [&str; 6] = ["rename", "move", "set_level", "set_numbered", "delete", "add"];

/// Checks: exactly one new commit with subject, decision and requester, without a changed file.
fn assert_one_protocol_commit(c: &Coast, before: Option<Vec<String>>, kind: &str, id: i64, word: &str, code: &str) {
    let (Some(before), Some(after)) = (before, c.subjects()) else { return };
    let mut expected = vec![format!("Gliederung: Antrag {id} {word}")];
    expected.extend(before);
    assert_eq!(after, expected, "{kind}: genau ein Protokoll-Commit „{word}“, sonst nichts");
    let commit = c.last_commit().unwrap();
    assert!(commit.contains(&format!("Baruch-Decision: {code}")), "{kind}: Entscheidung im Spiegel: {commit}");
    assert!(commit.contains(&format!("Baruch-Requested-By: {ASKER}")), "{kind}: Antragsteller im Spiegel: {commit}");
    assert!(commit.contains(&format!("Baruch-Request: {id}")), "{kind}: Nummer des Antrags im Spiegel: {commit}");
    assert_eq!(c.last_commit_files().as_deref().map(str::trim), Some(""), "{kind}: der Commit ändert keine Datei");
}

#[tokio::test]
async fn rejecting_any_kind_of_request_over_rpc_writes_exactly_one_empty_commit() {
    for kind in KINDS {
        let c = coast().await;
        let id = c.request(kind).await;
        let shown = c.titles().await;
        let before = c.subjects();
        let decided =
            c.server.rpc("outline.request_decide", json!({ "work": c.work, "id": id, "decision": "reject" })).await;
        let answer = result(&decided);
        assert_eq!(answer["request"]["decision"], "rejected", "{kind}: abgelehnt: {decided}");
        assert!(answer["outline"].is_null(), "{kind}: keine Gliederungsänderung: {decided}");
        assert!(
            answer["commit"].is_string() || answer["mirror"].is_string(),
            "{kind}: die Antwort trägt commit (oder mirror bei einem Spiegelfehler): {decided}"
        );
        assert_eq!(c.titles().await, shown, "{kind}: Ablehnen ändert die Gliederung nicht");
        assert_one_protocol_commit(&c, before, kind, id, "abgelehnt", "rejected");
    }
}

#[tokio::test]
async fn a_stale_request_of_any_kind_writes_exactly_one_empty_commit() {
    for kind in KINDS {
        let c = coast().await;
        let id = c.request(kind).await;
        // Meanwhile the human renames the item concerned: the request is outdated.
        let subject = Coast::subject_of(kind);
        let key = c.key(subject).await;
        let renamed = c.server.rpc("outline.rename", json!({ "work": c.work, "key": key, "title": "Umbenannt" })).await;
        result(&renamed);
        let shown = c.titles().await;
        let before = c.subjects();
        let decided =
            c.server.rpc("outline.request_decide", json!({ "work": c.work, "id": id, "decision": "confirm" })).await;
        let answer = result(&decided);
        assert_eq!(answer["request"]["decision"], "stale", "{kind}: veraltet statt ausgeführt: {decided}");
        assert!(
            answer["request"]["message"].as_str().unwrap_or_default().contains("heißt inzwischen"),
            "{kind}: der Grund steht in der Antwort: {decided}"
        );
        assert_eq!(c.titles().await, shown, "{kind}: ein veralteter Antrag ändert nichts");
        assert_one_protocol_commit(&c, before, kind, id, "veraltet", "stale");
    }
}

#[tokio::test]
async fn rejecting_without_javascript_writes_the_protocol_commit_and_returns_303() {
    let c = coast().await;
    let id = c.request("set_level").await;
    let before = c.subjects();
    let form = format!("method=outline.request_decide&work={}&id={id}&decision=reject&view=gliederung", c.work);
    let (status, location) = c.server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne JavaScript: 303 statt Klartext");
    assert!(location.unwrap_or_default().ends_with("#antraege"), "zurück zu den Anträgen");
    assert_one_protocol_commit(&c, before, "set_level (Formular)", id, "abgelehnt", "rejected");
    let (_, page) = c.server.get(&format!("/gliederung?work={}", c.work)).await;
    assert!(
        page.contains(&format!("Antrag {id}:")) && page.contains("abgelehnt"),
        "unter „Zuletzt entschieden“: {page}"
    );
}

#[tokio::test]
async fn rejecting_in_the_consent_dialog_writes_the_protocol_commit_and_later_stays_a_plain_close() {
    let c = coast().await;
    let id = c.request("add").await;
    let (status, page) = c.server.get("/werke").await;
    assert_eq!(status, StatusCode::OK, "die Werkliste lädt mit offenem Antrag");
    let dialog = &page[page.find("<dialog class=\"consent-dialog\"").expect("Dialog auf der Seite")..];
    let dialog = &dialog[..dialog.find("</dialog>").unwrap()];
    assert!(
        dialog.contains("<form class=\"consent-later\" method=\"dialog\"><button class=\"btn\" type=\"submit\">Später entscheiden</button></form>"),
        "„Später entscheiden“ schließt nur den Dialog, unverändert: {dialog}"
    );
    // The buttons of the dialog are ordinary forms to /gliederung/act; it sends exactly these fields.
    for field in [
        "name=\"method\" value=\"outline.request_decide\"".to_owned(),
        format!("name=\"work\" value=\"{}\"", c.work),
        format!("name=\"id\" value=\"{id}\""),
        "name=\"decision\" value=\"reject\"".to_owned(),
    ] {
        assert!(dialog.contains(&field), "Feld {field} im Dialogformular: {dialog}");
    }
    let before = c.subjects();
    let form = format!("method=outline.request_decide&work={}&id={id}&view=gliederung&decision=reject", c.work);
    let (status, _) = c.server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "der Dialog-Knopf ohne JavaScript: 303");
    assert_one_protocol_commit(&c, before, "add (Dialog)", id, "abgelehnt", "rejected");
    let script = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "consent.js").expect("Widget consent.js");
    assert!(script.1.contains("outline.request_decide"), "mit JavaScript schickt das Widget dieselbe RPC-Methode");
    assert!(!script.1.contains("setTimeout"), "kein Zeitablauf: Schließen bleibt Schließen");
}

// ---- Preview "new part" -------------------------------------------------------------------------

/// The preview list of a request from the HTML (between `<ol class="request-preview-list` and `</ol>`).
fn preview_list<'a>(html: &'a str, from: &str) -> &'a str {
    let start = html.find(from).unwrap_or_else(|| panic!("{from} fehlt: {html}"));
    let list = &html[start..];
    let list = &list[list.find("<ol class=\"request-preview-list").expect("Vorschau-Liste")..];
    &list[..list.find("</ol>").unwrap()]
}

#[tokio::test]
async fn the_new_part_preview_names_kind_and_number_and_before_after_for_adopted_chapters() {
    let c = coast().await;
    let id = c.request("add").await;
    let listed = c.server.rpc("outline.requests", json!({ "work": c.work, "preview": true })).await;
    let preview = result(&listed)["requests"][0]["preview"].as_array().unwrap().clone();
    let labels: Vec<(String, String)> = preview
        .iter()
        .map(|p| (p["title"].as_str().unwrap().to_owned(), p["label"].as_str().unwrap().to_owned()))
        .collect();
    let kinds: Vec<&str> = labels.iter().map(|(_, l)| l.split(' ').next().unwrap()).collect();
    assert_eq!(kinds, ["Kapitel", "Teil", "Kapitel", "Kapitel"], "jede Zeile nennt ihre Art: {labels:?}");
    let expected: Vec<(String, String)> =
        [("Ebbe", "Kapitel 1"), ("Vom Wasser", "Teil I"), ("Flut", "Kapitel 2"), ("Brandung", "Kapitel 3")]
            .iter()
            .map(|(t, l)| ((*t).to_owned(), (*l).to_owned()))
            .collect();
    assert_eq!(labels, expected, "Teile zählen eigen (I), Kapitel über Teile hinweg fortlaufend (Vorgabeschema)");
    let distinct: std::collections::HashSet<&String> = labels.iter().map(|(_, l)| l).collect();
    assert_eq!(distinct.len(), labels.len(), "Art und Nummer sind je Punkt eindeutig: {labels:?}");
    let flood = preview.iter().find(|p| p["title"] == "Flut").unwrap();
    assert_eq!(flood["mark"], "adopted", "Flut wandert unter den neuen Teil: {flood}");
    assert!(flood["before_label"].as_str().unwrap().starts_with("Kapitel"), "vorher mit Art: {flood}");
    assert_eq!(flood["changed_label"], true, "übernommene Kapitel zeigen vorher → nachher: {flood}");
    let part = preview.iter().find(|p| p["title"] == "Vom Wasser").unwrap();
    assert!(part["before_label"].is_null(), "der neue Teil hat kein Vorher: {part}");

    // Without JavaScript on /gliederung (in <details>) …
    let (status, page) = c.server.get(&format!("/gliederung?work={}", c.work)).await;
    assert_eq!(status, StatusCode::OK, "die Gliederungsseite lädt");
    let list = preview_list(&page, &format!("id=\"antrag-{id}\""));
    let (_, part_label) = labels.iter().find(|(t, _)| t == "Vom Wasser").unwrap();
    assert!(
        list.contains(&format!("<span class=\"num preview-label\">{part_label}</span> Vom Wasser")),
        "der Teil mit Art und Nummer: {list}"
    );
    let flood_before = flood["before_label"].as_str().unwrap();
    let flood_after = flood["label"].as_str().unwrap();
    assert!(
        list.contains(&format!("(vorher {flood_before} → nachher {flood_after})")),
        "vorher → nachher für das übernommene Kapitel: {list}"
    );
    // … and in the dialog on every page.
    let (_, works) = c.server.get("/werke").await;
    let dialog = preview_list(&works, "<dialog class=\"consent-dialog\"");
    assert!(dialog.contains(&format!("{part_label}</span> Vom Wasser")), "Dialog mit Art und Nummer: {dialog}");
    assert!(
        dialog.contains(&format!("(vorher {flood_before} → nachher {flood_after})")),
        "Dialog mit vorher → nachher"
    );
}

// ---- Times on /gliederung ------------------------------------------------------------------------

/// Invented sermon; „Das Suchen“ is long enough (over 60 words) to exceed a target time of one minute at 60 words per
/// minute.
fn sermon() -> String {
    let long = "Er lässt die neunundneunzig stehen und geht dem einen nach, durch Dornen und Geröll. ".repeat(6);
    format!(
        "= Einleitung\n\nEin Hirte zählt am Abend seine Schafe. Eins fehlt.\n\n= Das Suchen\n\n{long}\n\n\
         = Schluss\n\nSo sucht Gott jeden. Amen.\n"
    )
}

#[tokio::test]
async fn the_outline_page_shows_durations_and_the_target_warning_for_sermons() {
    let server = common::open().await;
    let work = server.import(&sermon(), "hirte.typ").await.work;
    sqlx::query("UPDATE work SET kind = 'sermon' WHERE id = ?")
        .bind(work)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let timing =
        baruch_server::timing::timing_json(&baruch_server::timing::work_timing(&server.state, work).await.unwrap());
    assert_eq!(timing["shown"], true, "Voraussetzung: Predigten zeigen die Zeiten");

    let (status, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "die Gliederungsseite der Predigt lädt");
    assert!(page.contains("<th>Dauer</th>"), "Spalte „Dauer“ bei Predigten: {page}");
    let total = timing["clock"].as_str().unwrap();
    assert!(page.contains(&format!("Gesamtzeit <strong>{total}</strong>")), "Gesamtzeit wie work.timing: {page}");
    for section in timing["sections"].as_array().unwrap() {
        let cell = format!(
            "{} <span class=\"muted small\">ab {}</span>",
            section["clock"].as_str().unwrap(),
            section["start_clock"].as_str().unwrap()
        );
        assert!(page.contains(&cell), "Dauer und Zeitmarke je Punkt aus dem Zeitwächter ({cell}): {page}");
    }
    // Round 39a: the widget catalog in the page head carries every widget text; only the shown page counts.
    let shown = match page.split_once("id=\"baruch-i18n\">") {
        Some((head, rest)) => format!("{head}{}", &rest[rest.find("</script>").unwrap()..]),
        None => page.clone(),
    };
    assert!(!shown.contains("Zielzeit"), "ohne Zielzeit keine Zielzeit-Angabe");

    // Target time 1 minute at 60 words per minute: over 60 words exceed it.
    let slow = TimingSettings { words_per_minute: 60, target_seconds: Some(60) };
    server.state.store.set_timing_settings(work, slow).await.unwrap();
    let timing =
        baruch_server::timing::timing_json(&baruch_server::timing::work_timing(&server.state, work).await.unwrap());
    assert_eq!(timing["status"], "over", "Voraussetzung: über 60 Wörter dauern bei 60 Wörtern je Minute über 1:00");
    let (_, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert!(page.contains("Zielzeit <strong>1:00</strong>"), "Zielzeit als Text: {page}");
    let over = timing["over_clock"].as_str().unwrap();
    assert!(
        page.contains(&format!("Warnung: Zielzeit um {over} überschritten")),
        "Warnung als Text ohne JavaScript: {page}"
    );
}

#[tokio::test]
async fn books_without_a_target_show_no_duration_column_but_with_one_they_do() {
    let c = coast().await;
    let (_, page) = c.server.get(&format!("/gliederung?work={}", c.work)).await;
    assert!(!page.contains("<th>Dauer</th>") && !page.contains("Gesamtzeit"), "Buch ohne Zielzeit: keine Zeiten");
    c.server
        .state
        .store
        .set_timing_settings(c.work, TimingSettings { words_per_minute: 110, target_seconds: Some(60) })
        .await
        .unwrap();
    let (_, page) = c.server.get(&format!("/gliederung?work={}", c.work)).await;
    assert!(page.contains("<th>Dauer</th>"), "mit Zielzeit zeigt auch ein Buch die Spalte „Dauer“: {page}");
    assert!(page.contains("im Rahmen"), "neun Wörter bleiben unter einer Minute: {page}");
}
