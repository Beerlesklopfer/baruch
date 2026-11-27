// SPDX-License-Identifier: AGPL-3.0-or-later
//! Acceptance of Round 30 (contract §7) with an invented work, ScriptedModel or MockEliezer, without network.
//!
//! Owner: integrator. Created empty by the interface R30-S1.
//!
//! Here is what only works together after the three branches have been merged: search/replace and cross-references
//! (suchen-verweise) on a work whose sections are reviewed by personas (chat-loesungen), whose answers the chat renders
//! structured (chat-bedienung) – with the finding blocks from the real `finding_spans`, the jump labels from the real
//! `check_marker` and the exchange round trip u5 with the real `remap_keys`. The „Küstenbuch“ is freely invented: 3
//! parts, 12 chapters, 48 sections, 6 personas (default). Models only through `ScriptedModel`, which records every
//! call; no network request.

use crate::common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::context;
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::port::{ModelCall, Role};
use baruch_server::search::{SearchQuery, SearchRange};
use baruch_typst::xref::{find_xrefs, xref_snippet};
use serde_json::{Value, json};
use tower::ServiceExt;

const PARTS: [(&str, [&str; 4]); 3] = [
    ("Vom Meer", ["Einleitung", "Strand", "Düne", "Watt"]),
    ("Vom Hafen", ["Mole", "Kai", "Werft", "Speicher"]),
    ("Vom Land", ["Deich", "Marsch", "Geest", "Heimweg"]),
];

/// The coast book: in every chapter and section „Leuchtturm“ in the running text (60 places in 60 sections); the
/// „Hafen“ additionally carries a Bible quotation with „Leuchtturm“ (stays when replacing), the „Brücke“ the text for
/// the personas, „Rückschau“ the phrase „die Einleitung“ (possible cross-reference).
fn book() -> String {
    let mut out =
        String::from("#set document(title: \"Küstenbuch\", author: \"Ida Kai\")\n#set heading(numbering: \"1.1\")\n");
    let mut n = 0;
    for (part, chapters) in PARTS {
        out.push_str(&format!("#heading(level: 1, numbering: none)[{part}] <baruch-part>\n\nVorrede.\n\n"));
        for chapter in chapters {
            out.push_str(&format!("= {chapter}\n\nDer Leuchtturm im Kapitel {chapter}.\n\n"));
            for _ in 0..4 {
                n += 1;
                let section = match n {
                    1 => "== Hafen\n\nDer Leuchtturm blinkt. Wie geschrieben steht: „Ein Leuchtturm ist mein Licht“ \
                          (Ps 119,105).\n\n"
                        .to_owned(),
                    2 => "== Brücke\n\nDie Brücke trägt drei Laternen über den Fluss. Der Leuchtturm wacht.\n\n"
                        .to_owned(),
                    3 => "== Rückschau\n\nWie die Einleitung sagt, steht der Leuchtturm fest.\n\n".to_owned(),
                    4 => "== Wachturm\n\nAuf dem Wachturm sieht man den Leuchtturm.\n\n".to_owned(),
                    _ => format!("== Abschnitt {n}\n\nDer Leuchtturm {n} steht am Ufer.\n\n"),
                };
                out.push_str(&section);
            }
        }
    }
    out
}

struct Coast {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    slug: String,
    keys: HashMap<String, i64>,
}

async fn coast(steps: impl IntoIterator<Item = Step>) -> Coast {
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
    let report = server.import(&book(), "kueste.typ").await;
    let keys = server
        .state
        .store
        .load_work(report.work)
        .await
        .unwrap()
        .nodes
        .iter()
        .map(|n| (n.title.clone(), n.key))
        .collect();
    Coast { server, model, work: report.work, slug: report.slug, keys }
}

impl Coast {
    fn key(&self, title: &str) -> i64 {
        *self.keys.get(title).unwrap_or_else(|| panic!("kein Punkt {title}"))
    }

    async fn ok(&self, method: &str, params: Value) -> Value {
        let mut params = params;
        params["work"] = json!(self.work);
        let answer = self.server.rpc(method, params).await;
        assert!(answer.get("error").is_none_or(Value::is_null), "{method}: {answer}");
        answer["result"].clone()
    }

    async fn node(&self, key: i64) -> baruch_core::Node {
        let work = self.server.state.store.load_work(self.work).await.unwrap();
        work.nodes.into_iter().find(|n| n.key == key).unwrap()
    }

    async fn body(&self, key: i64) -> String {
        let node = self.node(key).await;
        node.revision(node.current_number().unwrap()).unwrap().body.clone()
    }

    fn subjects(&self) -> Vec<String> {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "--format=%s"])
            .map(|log| log.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    async fn page(&self, uri: &str) -> String {
        let (status, html) = self.server.get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
        html
    }

    async fn posts(&self, key: i64) -> Vec<baruch_core::Post> {
        self.node(key).await.posts
    }
}

fn user(call: &ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::User).map(|m| m.content.as_str()).unwrap()
}

fn authors(posts: &[baruch_core::Post]) -> Vec<&str> {
    posts.iter().map(|p| p.author.as_str()).collect()
}

// ---- §7.1 Search and replace -------------------------------------------------------------------------------------

#[tokio::test]
async fn regex_search_matches_the_page_and_replacing_fifty_sections_is_one_commit_and_one_undo() {
    let c = coast([]).await;
    let harbour = c.key("Hafen");

    // Regex across the work: RPC and the page /suche without JavaScript show the same hits.
    let found = c.ok("search.find", json!({ "q": "Leuchtturm \\d+", "re": true, "range": "work" })).await;
    let total = found["total"].as_u64().unwrap() as usize;
    assert_eq!(total, 44, "„Der Leuchtturm n“ steht in 48 Abschnitten außer den vier benannten: {found}");
    assert_eq!(found["hits"].as_array().unwrap().len(), total, "nicht gekürzt");
    let html = c.page(&format!("/suche?work={}&q=Leuchtturm+%5Cd%2B&re=1&range=work", c.work)).await;
    assert!(html.contains("data-search-results"), "Trefferliste ohne JavaScript");
    assert_eq!(html.matches("<mark>").count(), total, "die Seite zeigt genau die Treffer der RPC");

    // Time limit: without remaining time the search aborts with timed_out (contract §3.1, 2000 ms per call).
    let query = SearchQuery { q: "Leucht.+".into(), regex: true, range: SearchRange::Work, ..SearchQuery::default() };
    let late = baruch_server::search::find_within(&c.server.state, c.work, &query, Duration::ZERO).await.unwrap();
    assert!(late.timed_out && late.hits.is_empty(), "abgelaufene Zeitgrenze: timed_out, keine Treffer");

    // Preview across the whole work: 12 chapters and 48 sections, the quotation in the Hafen does not count.
    let preview =
        c.ok("search.preview", json!({ "q": "Leuchtturm", "replacement": "Wachturm", "range": "work" })).await;
    let sections = preview["sections"].as_array().unwrap().clone();
    assert_eq!(sections.len(), 60, "jedes Kapitel und jeder Abschnitt hat eine Stelle im Fließtext");
    let hafen = sections.iter().find(|s| s["key"] == json!(harbour)).unwrap();
    assert_eq!(hafen["hits"].as_array().unwrap().len(), 1, "im Hafen nur der Fließtext, nicht das Bibelzitat");
    // Replace exactly 50: the Hafen and 49 more.
    let mut picks: Vec<Value> = vec![json!({ "key": harbour, "base_rev": 1, "skip": [] })];
    picks.extend(
        sections
            .iter()
            .filter(|s| s["key"] != json!(harbour))
            .take(49)
            .map(|s| json!({ "key": s["key"], "base_rev": s["base_rev"], "skip": [] })),
    );
    let before = c.subjects();
    let done = c
        .ok(
            "search.replace",
            json!({ "q": "Leuchtturm", "replacement": "Wachturm", "range": "work", "sections": picks, "confirm": true }),
        )
        .await;
    assert_eq!(done["changed"].as_array().unwrap().len(), 50, "50 neue Fassungen: {done}");
    let after = c.subjects();
    assert_eq!(after.len(), before.len() + 1, "genau ein Commit für alle 50 Abschnitte");
    assert_eq!(after[0], "Ersetzt „Leuchtturm“ → „Wachturm“ in 50 Abschnitten", "Commit-Text nach §3.1");
    let work = c.server.state.store.load_work(c.work).await.unwrap();
    let human = work
        .nodes
        .iter()
        .filter(|n| n.current_number() == Some(2))
        .filter(|n| matches!(&n.revision(2).unwrap().author, baruch_core::Author::Human { .. }))
        .count();
    assert_eq!(human, 50, "jede neue Fassung stammt vom Menschen");
    let hafen_text = c.body(harbour).await;
    assert!(hafen_text.contains("Der Wachturm blinkt."), "Fließtext ersetzt: {hafen_text}");
    assert!(hafen_text.contains("„Ein Leuchtturm ist mein Licht“"), "das Bibelzitat bleibt unverändert: {hafen_text}");

    // Undo without JavaScript through /suche/act: one commit, all back at revision 1.
    let batch = done["batch"]["id"].as_i64().unwrap();
    let (status, location) = c
        .server
        .post_form(
            "/suche/act",
            &format!("method=search.undo&work={}&batch={batch}&confirm=1&q=Leuchtturm&range=work", c.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Rückgängig per Formular: 303");
    assert!(location.unwrap().starts_with(&format!("/suche?work={}", c.work)), "zurück auf /suche");
    let last = c.subjects();
    assert_eq!(last.len(), after.len() + 1, "Rückgängig ist genau ein Commit");
    assert_eq!(last[0], "Ersetzen rückgängig: „Leuchtturm“ → „Wachturm“ (50 Abschnitte)", "Commit-Text");
    let work = c.server.state.store.load_work(c.work).await.unwrap();
    assert!(
        work.nodes.iter().filter(|n| n.revisions.len() > 1).all(|n| n.current_number() == Some(1)),
        "alle 50 wieder auf Fassung 1"
    );
}

// ---- §7.2 Cross-references ----------------------------------------------------------------------------------------

async fn pdf_bytes(c: &Coast) -> Vec<u8> {
    let _serial = common::PDF_SERIAL.lock().await;
    let response = c
        .server
        .app
        .clone()
        .oneshot(Request::get(format!("/werke/{}/pdf", c.work)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Buch-PDF lässt sich herunterladen");
    to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()
}

#[tokio::test]
async fn a_reference_becomes_an_internal_pdf_link_with_page_and_survives_import_and_exchange() {
    let c = coast([]).await;
    let intro = c.key("Einleitung");
    let watch = c.key("Wachturm");

    // Search the target through xref.targets, insert a reference with page number and save as the human.
    let targets = c.ok("xref.targets", json!({ "q": "Einl" })).await;
    let target = targets["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["key"] == json!(intro))
        .unwrap_or_else(|| panic!("Einleitung unter den Zielen: {targets}"))
        .clone();
    assert!(target["snippet"].as_str().unwrap().contains(&format!("#link(<k{intro}>)")), "Verweisform: {target}");
    let body = format!(
        "== Wachturm\n\nAuf dem Wachturm sieht man den Leuchtturm. Siehe {}.\n",
        xref_snippet(intro, "die Einleitung", true)
    );
    let saved = c.ok("revision.save", json!({ "key": watch, "based_on": 1, "body": body })).await;
    assert_eq!(saved["rev"], 2, "neue Fassung vom Menschen: {saved}");

    // The export carries the jump labels; the database does not.
    let export = c.ok("work.export", json!({})).await["typst"].as_str().unwrap().to_owned();
    assert!(export.contains(&format!("= Einleitung <k{intro}>")), "Export mit Sprungmarke an der Überschrift");
    assert!(!c.body(intro).await.contains("<k"), "der Text in der Datenbank bleibt ohne Sprungmarke");

    // PDF: internal link and „S. n“ (pdftotext).
    let pdf = pdf_bytes(&c).await;
    let raw = String::from_utf8_lossy(&pdf);
    assert!(raw.contains("/Link") && (raw.contains("/GoTo") || raw.contains("/Dest")), "interner Link im PDF");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kueste.pdf");
    std::fs::write(&path, &pdf).unwrap();
    match std::process::Command::new("pdftotext").arg(&path).arg("-").output() {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).replace('\u{a0}', " ");
            let at = text.find("die Einleitung, S.").unwrap_or_else(|| panic!("Verweistext im PDF: {text}"));
            let page = text[at + "die Einleitung, S.".len()..].trim_start();
            assert!(page.starts_with(|ch: char| ch.is_ascii_digit()), "Seitenzahl hinter „S.“: {}", &page[..20]);
        }
        _ => eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (nur der Link im PDF geprüft)"),
    }

    // Importing the export as a new work: the reference points to the new Einleitung.
    let report = c.server.import(&export.replace("Küstenbuch", "Küstenbuch zwei"), "kueste-zwei.typ").await;
    let copy = c.server.state.store.load_work(report.work).await.unwrap();
    let new_intro = copy.nodes.iter().find(|n| n.title == "Einleitung").unwrap().key;
    let new_watch = copy.nodes.iter().find(|n| n.title == "Wachturm").unwrap();
    let uses = find_xrefs(&new_watch.revisions[0].body);
    assert_ne!(new_intro, intro, "neues Werk, neue Schlüssel");
    assert_eq!(uses.len(), 1, "ein Verweis: {}", new_watch.revisions[0].body);
    assert_eq!(uses[0].key, new_intro, "umgeschlüsselt auf die neue Einleitung");
    assert!(uses[0].page, "mit Seitenzahl-Zusatz");

    // Exchange u5 into a second instance (keys are already assigned there): reference re-keyed.
    let xml = c.ok("work.exchange_export", json!({ "format": "xml" })).await["xml"].as_str().unwrap().to_owned();
    let other = common::open().await;
    other.import_garden().await;
    let imported = other
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": "new" }),
        )
        .await;
    assert!(imported["error"].is_null(), "Austausch einlesen: {imported}");
    let there = other.state.store.load_work(imported["result"]["work"].as_i64().unwrap()).await.unwrap();
    let there_intro = there.nodes.iter().find(|n| n.title == "Einleitung").unwrap().key;
    let there_watch = there.nodes.iter().find(|n| n.title == "Wachturm").unwrap();
    let body = &there_watch.revision(there_watch.current_number().unwrap()).unwrap().body;
    assert_ne!(there_intro, intro, "andere Instanz, anderer Schlüssel der Einleitung");
    assert_eq!(find_xrefs(body)[0].key, there_intro, "Austausch u5 schlüsselt den Verweis um: {body}");

    // Possible cross-references: „die Einleitung“ in the Rückschau; convert without JavaScript, one new revision.
    let review = c.key("Rückschau");
    let candidates = c.ok("xref.candidates", json!({ "key": review })).await;
    let list = candidates["candidates"].as_array().unwrap();
    let index = list.iter().position(|x| x["text"] == "die Einleitung").expect("„die Einleitung“ ist ein Kandidat");
    assert_eq!(list[index]["target_key"], json!(intro), "Ziel ist die Einleitung");
    let (status, location) = c
        .server
        .post_form(
            "/verweise/act",
            &format!("method=xref.convert&work={}&key={review}&rev=1&picks={index}&confirm=1&scope={review}", c.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Umwandeln per Formular: 303 ({location:?})");
    assert_eq!(c.node(review).await.current_number(), Some(2), "neue Fassung vom Menschen");
    assert_eq!(find_xrefs(&c.body(review).await)[0].key, intro, "Verweis eingesetzt");

    // A broken reference is listed in the print check of the typesetting page.
    let broken = format!(
        "== Brücke\n\nDie Brücke trägt drei Laternen. Siehe {}.\n",
        xref_snippet(999_999, "das Nirgendwo", false)
    );
    c.ok("revision.save", json!({ "key": c.key("Brücke"), "based_on": 1, "body": broken })).await;
    let html = c.page(&format!("/werke/{}/satz", c.work)).await;
    assert!(html.contains("Kaputter Querverweis k999999"), "Druckprüfung nennt den kaputten Verweis");
    let page = c.page(&format!("/verweise?work={}", c.work)).await;
    assert!(page.contains("data-xref-broken"), "auch auf /verweise");
}

// ---- §7.3 RFC, solutions, variants --------------------------------------------------------------------------------

const EXEGET: &str = "Kritisch: „drei Laternen“ wirkt schwer.\n- statt: „drei Laternen“\n- besser: „zwei Laternen“\n\
Kritisch: „über den Fluss“ ist blass.\n- statt: „über den Fluss“\n- besser: „über den stillen Fluss“";
const NATHAN: &str = "Kritisch: Die Brücke bleibt ohne Bild.";
const LECTOR: &str = "Jetzt wichtig: die Laternen.\n\nMein Vorschlag:\n1. statt „drei Laternen“ besser „zwei Laternen“\n\
2. statt „Der Leuchtturm wacht“ besser „Der Leuchtturm hält Wache“";

fn variant(summary: &str, findings: &[i64], sentence: &str) -> Step {
    let refs: Vec<String> = findings.iter().map(|f| format!("[B{f}]")).collect();
    Step::words(&format!(
        "Zeile: {summary}\nBefunde: {}\nText:\n== Brücke\n\nDie Brücke trägt {sentence} Der Leuchtturm wacht.\n",
        refs.join(" ")
    ))
}

#[tokio::test]
async fn the_rfc_checks_one_basis_asks_for_missing_solutions_and_solutions_become_proposals_and_variants() {
    let _turn = common::model_queue(module_path!()).await;
    let c = coast([]).await;
    let state = &c.server.state;
    let bridge = c.key("Brücke");
    let (others, lector) = baruch_server::assist::chat::rfc_personas(state).await.unwrap();
    assert_eq!(lector.as_deref(), Some("@lektor"), "fester Lektor (Vorgabe)");
    assert_eq!(others.len(), 5, "sechs aktive Personas: {others:?}");
    for handle in &others {
        match handle.as_str() {
            "@exeget" => c.model.push(Step::words(EXEGET)),
            // Nathan stays without a solution twice: demanded once more, after that the note.
            "@nathan" => {
                c.model.push(Step::words(NATHAN));
                c.model.push(Step::words(NATHAN));
            }
            _ => c.model.push(Step::words("Hinweis: Die Brücke trägt.")),
        }
    }
    c.model.push(Step::words(LECTOR));
    let answer = c.ok("persona.chat", json!({ "key": bridge, "message": "RFC zur Brücke" })).await;
    assert_eq!(answer["rfc"], true, "RFC erkannt: {answer}");
    assert!(answer["errors"].as_array().unwrap().is_empty(), "alle antworten: {answer}");
    let calls = c.model.calls();
    assert_eq!(calls.len(), others.len() + 2, "je Persona ein Aufruf, Nathan einmal nachgefordert, der Lektor zuletzt");
    for call in &calls[..calls.len() - 1] {
        assert!(!user(call).contains(context::RFC_TASK), "nur der Lektor sammelt und kommentiert fremde Befunde");
    }
    assert!(user(calls.last().unwrap()).contains(context::RFC_TASK), "der Lektor schließt mit „Mein Vorschlag:“");

    // Every critical line has a solution or the note; all posts name the same review basis.
    let posts = c.posts(bridge).await;
    let nathan = posts.iter().find(|p| p.author == "@nathan").unwrap();
    assert!(nathan.body.contains("(ohne Lösungsvorschlag)"), "Vermerk nach der Nachforderung: {}", nathan.body);
    let persona_ids: Vec<i64> = posts.iter().filter(|p| p.author.starts_with('@')).map(|p| p.id).collect();
    assert_eq!(persona_ids.len(), 6, "sechs Persona-Beiträge");
    let meta = state.store.post_meta(c.work, &persona_ids).await.unwrap();
    for id in &persona_ids {
        assert_eq!(meta[id].basis_label, "geprüft: aktuelle Fassung 1", "Beitrag {id}: dieselbe Prüfgrundlage");
    }
    let html = c.page(&format!("/?work={}&key={bridge}", c.work)).await;
    assert!(
        html.matches(">geprüft: aktuelle Fassung 1</div>").count() >= 6,
        "der Kopf „geprüft: …“ steht sichtbar an jedem Persona-Beitrag"
    );

    // Lector „Mein Vorschlag“ → exactly one summary proposal.
    let proposals = answer["proposals"].as_array().unwrap();
    assert_eq!(proposals.len(), 1, "ein Sammel-Vorschlag: {answer}");
    let node = c.node(bridge).await;
    let rev = node.revision(proposals[0]["rev"].as_u64().unwrap() as u32).unwrap();
    assert!(rev.note.starts_with("Mein Vorschlag von @lektor (Beitrag "), "Notiz: {}", rev.note);
    assert!(rev.body.contains("zwei Laternen") && rev.body.contains("hält Wache"), "beide Lösungen: {}", rev.body);

    // „Als Vorschlag übernehmen“ with two solutions from the post of the exegete, without JavaScript.
    let exeget = posts.iter().find(|p| p.author == "@exeget").unwrap().id;
    assert!(html.contains("Als Vorschlag übernehmen"), "Knopf am Befund-Block bzw. an der Lösungsliste");
    let (status, location) = c
        .server
        .post_form(
            "/loesungen/act",
            &format!("method=solution.adopt&work={}&key={bridge}&post={exeget}&rev=1&picks=0&picks=1", c.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular → 303");
    assert!(location.unwrap().starts_with(&format!("/?work={}&key={bridge}&rev=", c.work)), "zur neuen Fassung");
    let node = c.node(bridge).await;
    let adopted = node.revisions.iter().find(|r| r.note == format!("Lösung aus Beitrag {exeget}")).expect("Vorschlag");
    assert_eq!(adopted.status, baruch_core::RevisionStatus::Proposed, "nur ein Vorschlag");
    assert!(adopted.body.contains("zwei Laternen über den stillen Fluss"), "beide Ersetzungen: {}", adopted.body);
    let filter = baruch_store::FindingFilter { post: Some(exeget), ..Default::default() };
    let findings: Vec<i64> = state.store.finding_list(c.work, &filter).await.unwrap().iter().map(|f| f.id).collect();
    assert_eq!(findings.len(), 2, "zwei Befunde des Exegeten");
    for id in &findings {
        let f = state.store.finding_get(c.work, *id).await.unwrap();
        assert_eq!(f.pending.and_then(|p| p.adopted_rev), Some(adopted.number), "Statusvorschlag „übernommen in …“");
    }

    // „Lektor, … Varianten“ in the chat → 2 sibling proposals with a label.
    c.model.push(variant("knapp", &findings, "zwei Laternen über den Fluss."));
    c.model.push(variant("gründlich", &findings, "zwei Laternen über den stillen Fluss."));
    let chatted =
        c.ok("persona.chat", json!({ "key": bridge, "message": "Lektor, setze die Lösungen als Varianten um." })).await;
    assert_eq!(chatted["proposals"].as_array().unwrap().len(), 2, "zwei Varianten aus dem Chat: {chatted}");

    // solution.variants count 3 → A/B/C on the same base; comparison side by side; discard the rest.
    for (summary, sentence) in [("eins", "eine Laterne."), ("zwei", "zwei Laternen."), ("drei", "keine Laterne.")] {
        c.model.push(variant(summary, &findings, sentence));
    }
    let three = c.ok("solution.variants", json!({ "key": bridge, "findings": findings, "count": 3 })).await;
    let labels: Vec<&str> =
        three["variants"].as_array().unwrap().iter().map(|v| v["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["A", "B", "C"], "drei Varianten mit Etikett: {three}");
    let group = three["group"].as_str().unwrap().to_owned();
    let seq = state.store.node_seq(c.work, bridge).await.unwrap();
    let members = state.store.variant_group(c.work, seq, &group).await.unwrap();
    let tree = state.store.revision_tree(c.work).await.unwrap();
    let infos = &tree.iter().find(|n| n.node == seq).unwrap().revisions;
    for (rev, _) in &members {
        let info = infos.iter().find(|i| i.number == *rev).unwrap();
        assert_eq!(info.based_on, Some(1), "Variante {rev}: gleiche Basis");
    }
    let compare = c.page(&format!("/varianten?work={}&key={bridge}&group={group}", c.work)).await;
    assert!(compare.contains("value=\"solution.reject_siblings\""), "„übrige verwerfen“ ohne JavaScript");
    let a = members[0].0;
    let (status, _) = c
        .server
        .post_form(
            "/loesungen/act",
            &format!("method=solution.reject_siblings&work={}&key={bridge}&rev={a}&confirm=1", c.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular → 303");
    let node = c.node(bridge).await;
    for (rev, _) in &members[1..] {
        assert_eq!(
            node.revision(*rev).unwrap().status,
            baruch_core::RevisionStatus::Rejected,
            "Geschwister {rev} verworfen"
        );
    }
    assert_eq!(node.revision(a).unwrap().status, baruch_core::RevisionStatus::Proposed, "A bleibt Vorschlag");
}

// ---- §7.4 Chat ----------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_chat_addresses_relays_lives_repeats_deletes_and_sets_safe_blocks() {
    let _turn = common::model_queue(module_path!()).await;
    let c = coast([]).await;
    let bridge = c.key("Brücke");
    let lector_text = format!(
        "Was trägt: die Brücke.\n\
         - Kritisch: „drei Laternen“ wirken schwer.\n  statt: „drei Laternen“\n  besser: „zwei Laternen“\n\
         - Hinweis: Siehe [[key:{bridge}|„über den Fluss“]] und [[key:{bridge}|„über das Meer“]].\n\
         @schrift: Passt hier Ps 23,1?"
    );
    c.model.push(Step::words(&lector_text));
    c.model.push(Step::words("Die Stelle passt."));

    // „Lektor, …“ without @ calls the lector; its line „@schrift: …“ leads to exactly one answer from @schrift.
    let chatted = c.ok("persona.chat", json!({ "key": bridge, "message": "Lektor, was meinst du?" })).await;
    assert!(chatted["errors"].as_array().unwrap().is_empty(), "keine Fehler: {chatted}");
    let posts = c.posts(bridge).await;
    assert_eq!(authors(&posts), ["Autor", "@lektor", "@schrift"], "Lektor und eine Relay-Antwort");
    assert_eq!(c.model.calls().len(), 2, "genau zwei Modellaufrufe");
    let (lector, relay) = (posts[1].id, posts[2].id);
    let meta = c.server.state.store.post_meta(c.work, &[relay]).await.unwrap().remove(&relay).unwrap();
    assert_eq!((meta.relay_from.as_deref(), meta.reply_to), (Some("@lektor"), Some(lector)), "Relay-Metadaten");

    // Structured blocks, checked jump label as a link, unchecked one as text, finding block with solution.
    let html = c.page(&format!("/?work={}&key={bridge}", c.work)).await;
    assert!(
        html.contains(&format!("antwortet auf <a href=\"#post-{lector}\">Beitrag {lector}</a> von @lektor")),
        "„antwortet auf Beitrag n“ sichtbar"
    );
    assert!(html.contains("<ul class=\"post-list\">"), "Listen werden <ul>");
    assert!(html.contains("<div class=\"fb fb-kind-critical"), "der kritische Punkt ist ein eigener Befund-Block");
    assert!(html.contains("besser „zwei Laternen“"), "die Lösung steht im Block");
    assert!(html.contains("Als Vorschlag übernehmen"), "Knopf am Befund-Block");
    assert!(
        html.contains(&format!("key={bridge}&amp;rev=1&amp;find=%C3%BCber%20den%20Fluss"))
            && html.contains("data-jump-origin=\"chat\""),
        "[[key:N|„Zitat“]] mit bestandener Prüfung wird ein Sprung-Link"
    );
    assert!(html.contains("<span class=\"jump-unchecked\""), "ungültige Sprungmarke bleibt Text");
    assert!(!html.contains("find=%C3%BCber%20das%20Meer"), "kein Link für das fehlende Zitat");
    let block = &html[html.find("<div class=\"fb fb-kind-critical").unwrap()..];
    let block = &block[..block.find("</form>").unwrap_or(block.len())];
    assert_eq!(block.matches("wirken schwer").count(), 1, "der Befundtext steht im Block genau einmal");

    // <script> from the message of the human is escaped (no persona addressed, no model call).
    c.ok("persona.chat", json!({ "key": bridge, "message": "Notiz <script>alert(1)</script>" })).await;
    let html = c.page(&format!("/?work={}&key={bridge}", c.work)).await;
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;") && !html.contains("<script>alert"), "maskiert");
    assert_eq!(c.model.calls().len(), 2, "die Notiz ruft niemanden");

    // Live: post.since reports „@lektor schreibt …“ and after that the new post.
    c.model.push(Step::Wait { before: vec![], then: Box::new(Step::words("Hinweis: Die Brücke hält.")) });
    let app = c.server.app.clone();
    let (work, key) = (c.work, bridge);
    let running = tokio::spawn(async move {
        common::rpc(app, "persona.chat", json!({ "work": work, "key": key, "message": "@lektor Noch einmal?" })).await
    });
    c.model.started().await;
    let mut typing = false;
    for _ in 0..200 {
        let since = c.ok("post.since", json!({ "key": bridge, "after": relay })).await;
        if since["typing"].as_array().unwrap().iter().any(|t| t["persona"] == "@lektor") {
            typing = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(typing, "während des Auftrags: „@lektor schreibt …“");
    c.model.release();
    assert!(running.await.unwrap()["error"].is_null(), "Chat gelingt");
    let since = c.ok("post.since", json!({ "key": bridge, "after": relay })).await;
    let fresh: Vec<&str> = since["posts"].as_array().unwrap().iter().map(|p| p["author"].as_str().unwrap()).collect();
    assert_eq!(fresh, ["Autor", "Autor", "@lektor"], "neue Beiträge ohne Neuladen: Notiz, Frage, Antwort");

    // ↻ after a correction: repeat_of and the proposal „erledigt?“ for the critical finding that disappeared.
    let filter = baruch_store::FindingFilter { post: Some(lector), ..Default::default() };
    let critical = c
        .server
        .state
        .store
        .finding_list(c.work, &filter)
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.kind == baruch_store::FindingKind::Critical)
        .expect("kritischer Befund des Lektors")
        .id;
    let fixed = "== Brücke\n\nDie Brücke trägt zwei Laternen über den Fluss. Der Leuchtturm wacht.\n";
    c.ok("revision.save", json!({ "key": bridge, "based_on": 1, "body": fixed })).await;
    c.model.push(Step::words("- Hinweis: Siehe „über den Fluss“."));
    let (status, location) = c
        .server
        .post_form("/beitraege/act", &format!("method=post.repeat&work={}&key={bridge}&id={lector}&rev=2", c.work))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "↻ ohne JavaScript: 303");
    let repeated = c.posts(bridge).await.last().unwrap().id;
    assert!(location.unwrap().ends_with(&format!("#post-{repeated}")), "zurück an den neuen Beitrag");
    let meta = c.server.state.store.post_meta(c.work, &[repeated]).await.unwrap().remove(&repeated).unwrap();
    assert_eq!(meta.repeat_of, Some(lector), "repeat_of");
    let f = c.server.state.store.finding_get(c.work, critical).await.unwrap();
    assert_eq!(f.pending.map(|p| p.status), Some(baruch_store::FindingStatus::Done), "„erledigt?“ vorgeschlagen");

    // ✕ deletes with exactly one commit.
    let before = c.subjects().len();
    let (status, _) = c
        .server
        .post_form("/beitraege/act", &format!("method=post.delete&work={}&key={bridge}&id={relay}&confirm=1", c.work))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "✕ ohne JavaScript: 303");
    assert!(c.posts(bridge).await.iter().all(|p| p.id != relay), "gelöscht");
    assert_eq!(c.subjects().len(), before + 1, "genau ein Commit");
}

// ---- §7.5 Without JavaScript: new writing forms reject a foreign origin ----------------------------------------

#[tokio::test]
async fn every_new_form_refuses_a_foreign_origin() {
    let c = coast([]).await;
    for (uri, form) in [
        ("/suche/act", format!("method=search.undo&work={}&batch=1&confirm=1", c.work)),
        ("/verweise/act", format!("method=xref.convert&work={}&key=1&rev=1&picks=0&confirm=1", c.work)),
        ("/loesungen/act", format!("method=solution.adopt&work={}&key=1&post=1&picks=0", c.work)),
        ("/faden/act", format!("method=thread.repeat&work={}&thread=1&id=1", c.work)),
        ("/beitraege/act", format!("method=post.repeat&work={}&key=1&id=1", c.work)),
    ] {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ORIGIN, "https://fremd.example")
            .body(Body::from(form))
            .unwrap();
        let (status, _, _) = c.server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}: fremder Origin → 403 (guard::same_origin)");
    }
}

// ---- Wiring by the integrator: MCP `ask_persona` with `basis`, `post_delete {thread}` own ones only --------------

async fn mcp(c: &Coast, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = c.server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

async fn tool(c: &Coast, session: &str, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": name, "arguments": arguments } });
    mcp(c, Some(session), message).await.1["result"].clone()
}

/// Content of an MCP tool answer as JSON.
fn content(result: &Value) -> Value {
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap_or(Value::Null)
}

#[tokio::test]
async fn mcp_ask_persona_names_the_basis_and_post_delete_removes_only_own_thread_posts() {
    let _turn = common::model_queue(module_path!()).await;
    let c = coast([Step::words("Hinweis: Die Brücke trägt."), Step::words("Hallo zurück.")]).await;
    let (headers, init) = mcp(
        &c,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "probe", "version": "1.0" } } }),
    )
    .await;
    assert!(init["result"].is_object(), "initialize: {init}");
    let hint = init["result"]["instructions"].as_str().unwrap_or_default();
    assert!(!hint.contains("noch Stubs"), "der Hinweis nennt text_search/solution_variants nicht mehr als Stub");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();

    // ask_persona: the reviewed revision is in the answer as `basis` (as with the RPC method).
    let asked = tool(
        &c,
        &session,
        "ask_persona",
        json!({ "work": c.work, "key": c.key("Brücke"), "persona": "@lektor", "mode": "review" }),
    )
    .await;
    assert_ne!(asked["isError"], true, "ask_persona gelingt: {asked}");
    let asked = content(&asked);
    assert_eq!(asked["basis"]["rev"], 1, "Prüfgrundlage Fassung 1: {asked}");
    assert_eq!(asked["basis"]["label"], "geprüft: aktuelle Fassung 1", "Kopfzeile wie im Beitrag: {asked}");

    // Thread without a section: the human asks, the lector answers; MCP may delete only its own thread posts.
    let posted = c.ok("thread.post", json!({ "message": "@lektor Hallo?" })).await;
    let thread = posted["thread"].as_i64().unwrap_or_else(|| panic!("Faden: {posted}"));
    let read = c.server.state.store.thread_get(c.work, thread).await.unwrap();
    let human = read.entries.iter().find(|p| p.author == "Autor").unwrap().id;
    let foreign = tool(&c, &session, "post_delete", json!({ "work": c.work, "id": human, "thread": thread })).await;
    assert_eq!(foreign["isError"], true, "fremder Fadenbeitrag: isError: {foreign}");
    assert!(foreign["content"][0]["text"].as_str().unwrap().contains("Nur eigene"), "ehrliche Meldung: {foreign}");
    let own =
        tool(&c, &session, "chat", json!({ "work": c.work, "thread": thread, "message": "Notiz vom Client." })).await;
    assert_ne!(own["isError"], true, "chat in den Faden: {own}");
    let read = c.server.state.store.thread_get(c.work, thread).await.unwrap();
    let mine =
        read.entries.iter().find(|p| p.author == "@mcp:probe").unwrap_or_else(|| panic!("{:?}", read.entries)).id;
    let deleted = tool(&c, &session, "post_delete", json!({ "work": c.work, "id": mine, "thread": thread })).await;
    assert_ne!(deleted["isError"], true, "eigener Fadenbeitrag: gelöscht: {deleted}");
    let read = c.server.state.store.thread_get(c.work, thread).await.unwrap();
    assert!(read.entries.iter().all(|p| p.id != mine), "der eigene Beitrag ist fort");
    assert!(read.entries.iter().any(|p| p.id == human), "der fremde bleibt");
}
