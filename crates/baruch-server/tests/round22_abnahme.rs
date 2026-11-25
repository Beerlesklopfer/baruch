// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 22 (Abnahme, Besitz Integrator): Ende zu Ende über die Grenzen der drei Aufgaben hinweg – Fortschreiben,
//! Verlauf und Rückgängig, neue Fassung mit Grund (fassungen), Rückfrage „Befund n scheint erledigt“ (befund-rueckfrage),
//! Wasserzeichen „Entwurf“, Austausch Unterversion 8 mit neuem Teil als Antrag und `<kN>`-Regel beim Re-Import
//! (spiegel-entwurf), Archiv (befund-rueckfrage) und die Verdrahtung des Integrators (Seitenzweig gleich nach einem
//! Vorschlag, Menü „Als Entwurf herunterladen“, `post.repeat` ohne doppelte Rückfrage).
//!
//! Alle Werke frei erfunden („Die Salzwiese“, „Das Fährhaus am Deich“, „Rast“). Modelle nur über `MockEliezer` auf
//! 127.0.0.1:0 unter `common::model_queue`, PDFs unter `common::PDF_SERIAL`; kein Eliezer, kein Netz. Jede Zusicherung
//! mit Begründung. Alles über RPC bzw. Formulare ohne JavaScript.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_llm::mock::{MockEliezer, Reply};
use baruch_server::assist::eliezer::EliezerPort;
use baruch_server::assist::{Mode, Order, OrderFrame, PostLinks, Scope, prompts};
use baruch_store::FindingStatus;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

const MEADOW: &str = "#set document(title: \"Die Salzwiese\", author: \"Merle Watt\")\n#set heading(numbering: \"1.\")\n\n\
= Flut\n\nAm Morgen steht das Wasser bis an die Pfähle. Die Schafe drängen sich auf der Warft, und der Wind \
treibt Schaum über die Priele.\n\n\
= Ebbe\n\nAm Abend liegt der Schlick offen, und die Austernfischer suchen nach Würmern.\n";

const MEADOW_TITLE: &str = "Die Salzwiese";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn form_escape(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn readable(page: &str) -> String {
    page.replace("&#x2f;", "/").replace("&amp;", "&").replace("&quot;", "\"")
}

/// Ein Wort vor das erste Ankerwort („Wasser“, „Sturm“, „Schlick“) setzen: weit unter 20 % (kleine Korrektur).
fn touched(body: &str, word: &str) -> String {
    let anchor = ["Wasser", "Sturm", "Schlick"].into_iter().find(|a| body.contains(a)).expect("Testtext mit Ankerwort");
    body.replacen(anchor, &format!("{word} {anchor}"), 1)
}

fn commits(dir: &Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|n| n.trim().parse().unwrap())
}

fn last_message(dir: &Path) -> Option<String> {
    common::git(dir, &["log", "-1", "--format=%s%n%b"])
}

async fn node(server: &common::Server, work: i64, key: i64) -> Value {
    result(&server.rpc("node.get", json!({ "work": work, "key": key })).await).clone()
}

fn current(node: &Value) -> Value {
    node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").cloned().expect("aktuelle Fassung")
}

async fn save(server: &common::Server, work: i64, key: i64, body: &str, based_on: u64) -> Value {
    let answer = server
        .rpc("outline.save", json!({ "work": work, "key": key, "body": body, "based_on": based_on, "mode": "auto" }))
        .await;
    result(&answer).clone()
}

/// Das Alter einer Fassung zurückstellen (Zeitgrenze ohne Schlafen): `revision.created` ist per Trigger
/// unveränderlich – Trigger kurz herausnehmen und wörtlich wieder anlegen (wie SKILLS.md, `age_revision`).
async fn age_revision(server: &common::Server, key: i64, number: u64) {
    let pool = server.state.store.pool();
    let trigger: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'revision_origin_immutable'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("DROP TRIGGER revision_origin_immutable").execute(&mut *conn).await.unwrap();
    sqlx::query("UPDATE revision SET created = '2000-01-01T00:00:00.000Z' WHERE node_id = ? AND number = ?")
        .bind(key)
        .bind(number as i64)
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query(&trigger).execute(&mut *conn).await.unwrap();
}

async fn meadow(server: &common::Server) -> (i64, i64, std::path::PathBuf) {
    let report = server.import(MEADOW, "salzwiese.typ").await;
    let key = server.state.store.load_work(report.work).await.unwrap().nodes[0].key;
    (report.work, key, server.mirror_dir_of(&report.slug))
}

/// MCP-Anfrage vom eigenen Rechner (ohne Token nur Loopback).
async fn mcp(server: &common::Server, method: &str, params: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP {method} antwortet mit 200: {body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn an_author_amends_three_times_takes_one_back_and_gets_new_versions_only_with_a_reason() {
    let server = common::open().await;
    let (work, key, dir) = meadow(&server).await;
    let original = current(&node(&server, work, key).await)["body"].as_str().unwrap().to_owned();
    let start = commits(&dir);

    // Dreimal eine kleine Korrektur auf der eigenen Fassung: eine Fassung, drei Fortschreibungen, drei Commits.
    let mut texts = vec![original.clone()];
    for (i, word) in ["hohe", "graue", "kalte"].into_iter().enumerate() {
        let next = touched(texts.last().unwrap(), word);
        let saved = save(&server, work, key, &next, 1).await;
        assert_eq!(
            (saved["saved"].as_str(), saved["rev"].as_u64(), saved["amend_seq"].as_u64()),
            (Some("amended"), Some(1), Some(i as u64 + 1)),
            "Speichern {} schreibt Fassung 1 fort: {saved}",
            i + 1
        );
        if let Some(n) = start {
            assert_eq!(commits(&dir), Some(n + i as u32 + 1), "jede Fortschreibung ist genau ein Spiegel-Commit");
            let message = last_message(&dir).unwrap();
            assert!(
                message.contains(&format!("Fassung 1 fortgeschrieben (Stand {})", i + 1))
                    && message.contains("Baruch-Action: amended"),
                "Betreff und Trailer der Fortschreibung: {message}"
            );
        }
        texts.push(next);
    }
    let shown = node(&server, work, key).await;
    assert_eq!(shown["revisions"].as_array().unwrap().len(), 1, "keine neue Fassung durch kleine Korrekturen");
    assert_eq!(current(&shown)["amend_seq"], 3, "node.get trägt den Stand 3");

    // Verlauf: drei Schritte mit Text vorher/nachher, auch als Seite ohne JavaScript.
    let amends = result(&server.rpc("revision.amends", json!({ "work": work, "key": key, "rev": 1 })).await).clone();
    let steps = amends["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3, "drei Schritte im Verlauf: {amends}");
    for (i, step) in steps.iter().enumerate() {
        assert_eq!(
            (step["body_before"].as_str(), step["body_after"].as_str()),
            (Some(texts[i].as_str()), Some(texts[i + 1].as_str())),
            "Schritt {} speichert Text vorher und nachher",
            i + 1
        );
    }
    let history = format!("/werke/{work}/fassungen/verlauf?key={key}&rev=1");
    let (status, page) = server.get(&history).await;
    assert_eq!(status, StatusCode::OK, "Verlaufsseite lädt");
    assert!(page.contains("Bis hier zurück") && page.contains("Stand 3"), "Verlauf mit Knopf „Bis hier zurück“");

    // Rückgängig über das Formular (ohne JavaScript): Stand vor Schritt 3 als neuer Schritt 4, ein Commit.
    let form = format!("method=revision.amend_undo&work={work}&key={key}&rev=1&seq=3");
    let (status, location) = server.post_form("/werke/fassungen/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular „Bis hier zurück“ leitet weiter");
    assert!(location.unwrap_or_default().contains("fassungen/verlauf"), "zurück auf den Verlauf");
    let shown = current(&node(&server, work, key).await);
    assert_eq!(shown["amend_seq"], 4, "Rücknahme ist ein neuer Schritt (Stand 4), nichts gelöscht");
    assert_eq!(shown["body"].as_str(), Some(texts[2].as_str()), "Text wie vor Schritt 3 (Stand 2)");
    if let Some(n) = start {
        assert_eq!(commits(&dir), Some(n + 4), "die Rücknahme ist genau ein Commit");
        assert!(last_message(&dir).unwrap().contains("Baruch-Action: amend-undone"), "Trailer der Rücknahme");
    }

    // „Als neue Fassung speichern“ ohne JavaScript (as_new=1 neben dem versteckten mode=auto): neue Nummer.
    let form = format!(
        "method=outline.save&work={work}&key={key}&based_on=1&mode=auto&as_new=1&body={}",
        form_escape(&touched(&texts[2], "nasse"))
    );
    let (status, location) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Editor-Formular leitet weiter");
    assert!(location.unwrap().ends_with("&saved=new-2"), "neue Fassung 2 auf Wunsch");

    // Große Änderung: neue Fassung mit Grund large_change.
    let rewritten =
        "= Flut\n\nNachts kam der Sturm. Niemand hörte die Glocke am Siel, bis die Lampen im Dorf flackerten \
                     und der Deichgraf an jede Tür klopfte.\n"
            .to_owned();
    let saved = save(&server, work, key, &rewritten, 2).await;
    assert_eq!(
        (saved["saved"].as_str(), saved["rev"].as_u64(), saved["reason"].as_str()),
        (Some("new"), Some(3), Some("large_change")),
        "große Änderung ergibt eine neue Fassung mit Grund: {saved}"
    );
    assert_eq!(saved["reason_label"], "große Änderung", "deutscher Grund");

    // Zeitabstand: Einstellung „Fassungen“ über das Formular (5 Minuten), Fassung 3 gealtert → time_elapsed.
    let (status, location) = server
        .post_form(
            "/einstellungen/werk/fassungen",
            &format!("work={work}&amend=1&max_change_percent=20&new_after_minutes=5"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular Einstellungen → Werk → Fassungen leitet weiter");
    assert!(location.unwrap().contains("#fassungen"), "zurück zum Block Fassungen");
    let policy = result(&server.rpc("revision.policy_get", json!({ "work": work })).await).clone();
    assert_eq!(policy["new_after_minutes"], 5, "Einstellung gespeichert: {policy}");
    age_revision(&server, key, 3).await;
    let saved = save(&server, work, key, &touched(&rewritten, "spät"), 3).await;
    assert_eq!(
        (saved["saved"].as_str(), saved["rev"].as_u64(), saved["reason"].as_str()),
        (Some("new"), Some(4), Some("time_elapsed")),
        "nach der Zeitgrenze entsteht eine neue Fassung: {saved}"
    );
    // Fortschreiben aus: jede Speicherung eine neue Fassung (policy_off).
    result(&server.rpc("revision.policy_set", json!({ "work": work, "amend": false })).await);
    let body = current(&node(&server, work, key).await)["body"].as_str().unwrap().to_owned();
    let saved = save(&server, work, key, &touched(&body, "leise"), 4).await;
    assert_eq!(saved["reason"], "policy_off", "Fortschreiben ausgeschaltet: neue Fassung mit Grund: {saved}");
}

#[tokio::test]
async fn on_a_persona_version_saving_always_gives_a_new_version_and_the_proposal_keeps_its_own() {
    let server = common::open().await;
    let (work, key, dir) = meadow(&server).await;
    let original = current(&node(&server, work, key).await)["body"].as_str().unwrap().to_owned();
    // Vorschlag über MCP (propose_revision): der Seitenzweig entsteht gleich (Integrator, ohne Revisions-Commit).
    let proposed_text = touched(&original, "brackige");
    let answer = mcp(
        &server,
        "tools/call",
        json!({ "name": "propose_revision",
                "arguments": { "work": work, "key": key, "body": proposed_text, "note": "Farbe" } }),
    )
    .await;
    assert!(answer["result"]["isError"] != json!(true), "Vorschlag über MCP gelingt: {answer}");
    let proposal = current(&node(&server, work, key).await);
    assert_eq!(proposal["number"], 1, "der Vorschlag ändert die aktuelle Fassung nicht");
    if common::git(&dir, &["--version"]).is_some() {
        let refs = common::git(&dir, &["for-each-ref", "--format=%(refname)", "refs/revisions/"]).unwrap();
        assert!(
            refs.lines().any(|r| r == format!("refs/revisions/{key}/2")),
            "Seitenzweig des neuen Vorschlags sofort im Spiegel: {refs}"
        );
    }

    // Der Mensch übernimmt den Vorschlag; das nächste Speichern im Editor ist eine neue Fassung (not_own).
    result(&server.rpc("revision.accept", json!({ "work": work, "key": key, "rev": 2 })).await);
    let saved = save(&server, work, key, &touched(&proposed_text, "eigene"), 2).await;
    assert_eq!(
        (saved["saved"].as_str(), saved["rev"].as_u64(), saved["reason"].as_str()),
        (Some("new"), Some(3), Some("not_own")),
        "auf einer Persona-Fassung entsteht immer eine neue Fassung: {saved}"
    );
    let shown = node(&server, work, key).await;
    let persona = shown["revisions"].as_array().unwrap().iter().find(|r| r["number"] == 2).unwrap().clone();
    assert_eq!(persona["body"].as_str(), Some(proposed_text.as_str()), "der Vorschlag bleibt eine eigene Fassung");
    assert_eq!(persona["amend_seq"], 0, "der Vorschlag wurde nie fortgeschrieben");
    // Danach ist die Fassung wieder die des Menschen: kleine Korrektur schreibt fort.
    let mine = current(&shown)["body"].as_str().unwrap().to_owned();
    let saved = save(&server, work, key, &touched(&mine, "kühle"), 3).await;
    assert_eq!(saved["saved"], "amended", "die eigene neue Fassung lässt sich fortschreiben: {saved}");
}

/// Datenordner mit dem Nachbau des Modell-Backends (nie Eliezer).
async fn with_mock() -> (common::Server, MockEliezer) {
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
    result(&server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": eliezer.url() } })).await);
    (server, eliezer)
}

const FERRY: &str = "#set document(title: \"Das Fährhaus am Deich\", author: \"Jule Priel\")\n\
#set heading(numbering: \"1.\")\n\n= Am Steg\n\nDer Fährmann zieht das graue Seil durch die Rolle. Nebel liegt über \
dem Fluss, und am anderen Ufer brennt eine Laterne.\n\n= Drüben\n\nNiemand wartet am Anleger.\n";

#[tokio::test]
async fn a_persona_rechecks_after_the_correction_asks_whether_its_finding_is_done_and_the_human_confirms() {
    let _turn = common::model_queue(module_path!()).await;
    let (server, eliezer) = with_mock().await;
    let report = server.import(FERRY, "faehrhaus-deich.typ").await;
    let work = report.work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[0].key;
    // Erste Prüfung durch @nathan: ein kritischer Befund (mit Lösung, damit nichts nachgefordert wird).
    eliezer.push(Reply::text(&["Kritisch: „graue Seil“ ist zu blass.\n- statt: „graue Seil“\n- besser: „nasse Seil“"]));
    let order = Order {
        work,
        key,
        persona: "@nathan".into(),
        mode: Mode::Review,
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
    };
    let outcome = baruch_server::assist::run(&server.state, &order, None).await.expect("Prüfen gelingt");
    assert_eq!(outcome.findings.len(), 1, "ein Befund aus der ersten Prüfung");
    let id = outcome.findings[0];
    let post = server.state.store.finding_get(work, id).await.unwrap().post.expect("Befund aus einem Beitrag");

    // Der Autor setzt den Befund um – kleine Korrektur, also Fortschreibung der eigenen Fassung.
    let body = current(&node(&server, work, key).await)["body"].as_str().unwrap().replace("graue Seil", "nasse Seil");
    assert_eq!(save(&server, work, key, &body, 1).await["saved"], "amended", "Korrektur schreibt Fassung 1 fort");

    // ↻ Wiederholen: @nathan sieht den eigenen offenen Befund und meldet ihn erledigt, ohne neue Kritik.
    let reply = format!("Erledigt: Befund {id} (Stelle „nasse Seil“)\n\n{}", prompts::ALL_DONE_SENTENCE);
    eliezer.push(Reply::text(&[reply.as_str()]));
    let answer = server.rpc("post.repeat", json!({ "work": work, "id": post })).await;
    let repeated = result(&answer).clone();
    let last = eliezer.chat_bodies().pop().unwrap().to_string();
    assert!(last.contains(&format!("[B{id}]")), "die Persona sah ihren offenen Befund im Kontext");
    // Integrator: posts::repeat schlägt einen Befund mit offener Rückfrage nicht noch einmal vor.
    assert_eq!(repeated["proposed_done"], json!([]), "keine zweite Rückfrage über die Wiederholung: {repeated}");
    assert_eq!(repeated["notes"], json!([]), "kein Vermerk „schon offen“: {repeated}");
    let finding = server.state.store.finding_get(work, id).await.unwrap();
    let pending = finding.pending.clone().expect("Rückfrage am Befund");
    assert_eq!(pending.status, FindingStatus::Done, "vorgeschlagen ist „erledigt“");
    assert_eq!(
        pending.note,
        format!("Befund {id} scheint erledigt (Stelle „nasse Seil“) – als erledigt markieren?"),
        "Rückfrage im Wortlaut des Vertrags"
    );
    let filter = baruch_store::FindingFilter { keys: vec![key], limit: Some(100), ..Default::default() };
    let all = server.state.store.finding_list(work, &filter).await.unwrap();
    assert_eq!(all.len(), 1, "keine neue Kritik zum selben Punkt: {all:?}");

    // Arbeitsseite ohne JavaScript: „Ja, erledigt“ / „Nein, noch offen“; der Mensch bestätigt per Formular.
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    let page = readable(&page);
    assert!(page.contains("Ja, erledigt") && page.contains("Nein, noch offen"), "beide Knöpfe sichtbar");
    let back = form_escape(&format!("/?work={work}&key={key}"));
    let form = format!("method=finding.decide&work={work}&entry={}&confirm=1&back={back}", pending.id);
    let (status, _) = server.post_form("/werke/befunde/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular „Ja, erledigt“ leitet weiter");
    let finding = server.state.store.finding_get(work, id).await.unwrap();
    assert_eq!(finding.status, FindingStatus::Done, "der Mensch hat den Befund als erledigt markiert");
    assert!(finding.pending.is_none(), "die Rückfrage ist beantwortet");
}

/// Text eines PDFs je Seite ohne jeden Leerraum (pdftotext trennt Seiten mit \x0c); `None` ohne pdftotext.
fn pdf_pages(bytes: &[u8]) -> Option<Vec<String>> {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("probe.pdf");
    std::fs::write(&file, bytes).unwrap();
    let output = std::process::Command::new("pdftotext").arg(&file).arg("-").output().ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .split('\x0c')
            .map(|p| p.chars().filter(|c| !c.is_whitespace()).collect::<String>())
            .filter(|p| !p.is_empty())
            .collect()
    })
}

async fn pdf(server: &common::Server, method: &str, params: Value) -> (String, Vec<u8>) {
    let mut params = params;
    params["inline"] = json!(true);
    let answer = server.rpc(method, params).await;
    let r = result(&answer);
    (r["sha256"].as_str().unwrap().to_owned(), baruch_store::decode_base64(r["pdf_base64"].as_str().unwrap()).unwrap())
}

async fn get_bytes(server: &common::Server, uri: &str) -> (StatusCode, Vec<u8>) {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    (status, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

fn epub_entry(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(&mut archive.by_name(name).unwrap(), &mut text).unwrap();
    text
}

async fn typeset_settings(server: &common::Server, work: i64) -> Value {
    result(&server.rpc("work.typeset_get", json!({ "work": work })).await)["settings"].clone()
}

async fn step_uuids(server: &common::Server, work: i64, key: i64) -> Vec<String> {
    let r = result(&server.rpc("revision.amends", json!({ "work": work, "key": key, "rev": 1 })).await).clone();
    r["steps"].as_array().unwrap().iter().map(|x| x["uuid"].as_str().unwrap().to_owned()).collect()
}

async fn current_texts(server: &common::Server, work: i64) -> Vec<String> {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes.iter().map(|n| n.current_index().map(|i| n.revisions[i].body.clone()).unwrap_or_default()).collect()
}

#[tokio::test]
async fn the_draft_watermark_with_date_reaches_every_output_as_asked_and_nothing_changes_without_it() {
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    // Erfundenes Werk, lang genug für mehrere Seiten (die Messung „auf jeder Seite“ braucht mehr als eine).
    let mut source = String::from("#set document(title: \"Deichtage\", author: \"Ole Siel\")\n\n");
    for (i, title) in ["Morgen", "Mittag", "Abend", "Nacht"].iter().enumerate() {
        source.push_str(&format!("= {title}\n\n"));
        for j in 0..12 {
            source.push_str(&format!(
                "Absatz {i}.{j}: Der Deichläufer geht die Kante entlang, zählt die Pfähle und notiert, wo das Gras \
                 nach der letzten Flut kahl geblieben ist. Möwen folgen ihm bis zum Siel.\n\n"
            ));
        }
    }
    let work = server.import(&source, "deichtage.typ").await.work;
    let (plain, _) = pdf(&server, "work.pdf", json!({ "work": work })).await;

    // „Als Entwurf herunterladen“ ohne Einstellung: Menü-Eintrag und Download, die Einstellung bleibt leer.
    let (_, page) = server.get(&format!("/?work={work}")).await;
    assert!(
        // Runde 33 (Menüband): „Als Entwurf herunterladen“ (Menü Werk ▾) heißt im Reiter Buch, Gruppe Ausgabe, kurz
        // „Entwurf“; Adresse und download-Attribut sind dieselben, das „herunterladen“ steht im title.
        page.contains(&format!("<a class=\"btn\" href=\"/werke/{work}/pdf?entwurf=1\" download title=\"Das ganze Werk als PDF mit Wasserzeichen „Entwurf“ herunterladen (Einstellung bleibt unverändert)\">Entwurf</a>")),
        "Reiter Buch bietet „Entwurf“ herunterladen (Link, ohne JavaScript)"
    );
    let (status, download) = get_bytes(&server, &format!("/werke/{work}/pdf?entwurf=1")).await;
    assert_eq!(status, StatusCode::OK, "Download als Entwurf");
    assert_ne!(baruch_render::sha256_hex(&download), plain, "der Download trägt das Wasserzeichen");
    assert_eq!(typeset_settings(&server, work).await, json!({}), "der Download ändert die Einstellung nicht");

    // Einstellung „Entwurf“ mit Datum in der Fußzeile.
    result(
        &server
            .rpc(
                "work.typeset_set",
                json!({ "work": work, "settings": { "draft": {
                    "text": "ENTWURF", "date": true, "opacity_percent": 20, "placement": "footer" } } }),
            )
            .await,
    );
    let stored = typeset_settings(&server, work).await;
    let today = baruch_server::typeset::draft_today();
    let (by_setting, bytes) = pdf(&server, "work.pdf", json!({ "work": work })).await;
    assert_ne!(by_setting, plain, "Buch-PDF nach Einstellung mit Wasserzeichen");
    match pdf_pages(&bytes) {
        Some(pages) => {
            assert!(pages.len() > 1, "mehrere Seiten gemessen: {}", pages.len());
            for (i, text) in pages.iter().enumerate() {
                assert!(
                    text.contains("ENTWURF") && text.contains(&format!("Stand{today}")),
                    "Seite {} trägt „ENTWURF“ und „Stand {today}“: {text}",
                    i + 1
                );
            }
        }
        None => eprintln!("pdftotext fehlt: Messung je Seite übersprungen"),
    }
    let (_, notes) = pdf(&server, "notes.pdf", json!({ "work": work })).await;
    if let Some(pages) = pdf_pages(&notes) {
        assert!(pages.iter().all(|p| p.contains("ENTWURF")), "jeder Stichwortzettel trägt das Wasserzeichen");
    }
    let preview = result(&server.rpc("preview.typst", json!({ "work": work })).await).clone();
    assert_eq!(preview["draft"], true, "Satz-Vorschau nach Einstellung mit Wasserzeichen");
    let (status, page) = server.get(&format!("/werke/{work}/satz")).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite lädt");
    assert!(page.contains("Ohne Wasserzeichen herunterladen"), "bei eingeschaltetem Entwurf auch „ohne“ herunterladen");

    // Umschlag: nicht nach Einstellung (ohne draft.cover), auf Wunsch mit Warnung der Druckprüfung.
    let codes = |answer: &Value| -> Vec<String> {
        result(answer)["checks"].as_array().unwrap().iter().map(|c| c["code"].as_str().unwrap().to_owned()).collect()
    };
    let cover = server.rpc("work.cover_pdf", json!({ "work": work })).await;
    assert_eq!(result(&cover)["draft"], false, "Umschlag ohne Wasserzeichen, solange draft.cover fehlt");
    assert!(!codes(&cover).contains(&"draft".to_owned()), "ohne Wasserzeichen keine Warnung");
    let cover = server.rpc("work.cover_pdf", json!({ "work": work, "draft": true })).await;
    assert!(codes(&cover).contains(&"draft".to_owned()), "Umschlag auf Wunsch mit Wasserzeichen: Druckprüfung warnt");

    // EPUB nach Einstellung: Titelseite und Titel-Metadatum.
    let epub = server.rpc("work.epub", json!({ "work": work, "inline": true })).await;
    let epub = baruch_store::decode_base64(result(&epub)["epub_base64"].as_str().unwrap()).unwrap();
    assert!(epub_entry(&epub, "OEBPS/content.opf").contains("(Entwurf)</dc:title>"), "dc:title mit „(Entwurf)“");
    assert!(epub_entry(&epub, "OEBPS/text/title.xhtml").contains("Entwurf: ENTWURF"), "Hinweis auf der Titelseite");

    // Ohne Wasserzeichen trotz Einstellung: byte-gleich zum Werk ohne Einstellung; die Einstellung bleibt.
    let (without, _) = pdf(&server, "work.pdf", json!({ "work": work, "draft": false })).await;
    assert_eq!(without, plain, "draft:false byte-gleich zum PDF ohne Einstellung");
    let export =
        result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].as_str().unwrap().to_owned();
    assert!(!export.contains("ENTWURF") && !export.contains("foreground"), "Export bleibt ohne Wasserzeichen");
    assert_eq!(typeset_settings(&server, work).await, stored, "kein Download ändert die Einstellung");
}

async fn exchange_import(server: &common::Server, xml: &str, mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": mode }),
        )
        .await;
    result(&answer).clone()
}

async fn exchange_export(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    result(&answer)["xml"].as_str().unwrap().to_owned()
}

fn comparable(xml: &str) -> String {
    let re = regex::Regex::new(r#" schluessel="\d+""#).unwrap();
    re.replace_all(xml, "").lines().filter(|l| !l.trim_start().starts_with("<ausgabe ")).collect::<Vec<_>>().join("\n")
}

async fn uuid_of(server: &common::Server, sql: &str, id: i64) -> String {
    sqlx::query_scalar(sql).bind(id).fetch_one(server.state.store.pool()).await.unwrap()
}

#[tokio::test]
async fn two_throwaway_instances_carry_amends_the_watermark_and_a_new_part_as_a_request_with_its_identity() {
    let a = common::open().await;
    let (work_a, key_a, _) = meadow(&a).await;
    let original = current(&node(&a, work_a, key_a).await)["body"].as_str().unwrap().to_owned();
    let first = touched(&original, "hohe");
    save(&a, work_a, key_a, &first, 1).await;
    save(&a, work_a, key_a, &touched(&first, "graue"), 1).await;
    result(
        &a.rpc(
            "work.typeset_set",
            json!({ "work": work_a, "settings": { "draft": { "text": "VORABZUG", "opacity_percent": 15,
                    "placement": "diagonal" } } }),
        )
        .await,
    );
    let xml = exchange_export(&a, work_a).await;
    assert!(
        xml.contains("version=\"9\"") && xml.contains("<fortschreibung "),
        "Unterversion 9 (seit Runde 34) mit Fortschreibungen"
    );

    // B liest ein: Stand 2 mit denselben Schritten (gleiche uuids), Entwurf in den Satz-Einstellungen.
    let b = common::open().await;
    let work_b = exchange_import(&b, &xml, "new").await["work"].as_i64().unwrap();
    let key_b = b.state.store.load_work(work_b).await.unwrap().nodes[0].key;
    assert_eq!(current(&node(&b, work_b, key_b).await)["amend_seq"], 2, "B hat Fassung 1 im Stand 2");
    assert_eq!(
        step_uuids(&b, work_b, key_b).await,
        step_uuids(&a, work_a, key_a).await,
        "Schritte mit gleichen Kennungen"
    );
    let draft = result(&b.rpc("work.typeset_get", json!({ "work": work_b })).await)["settings"]["draft"].clone();
    assert_eq!(draft["text"], "VORABZUG", "der Entwurf reist mit: {draft}");
    assert_eq!(comparable(&exchange_export(&b, work_b).await), comparable(&xml), "A → B → Ausgabe byte-gleich");

    // A schreibt weiter fort und legt einen Teil vor das erste Kapitel; B gleicht ab.
    let body = current(&node(&a, work_a, key_a).await)["body"].as_str().unwrap().to_owned();
    save(&a, work_a, key_a, &touched(&body, "kalte"), 1).await;
    let seq = a.state.store.node_seq(work_a, key_a).await.unwrap();
    result(
        &a.rpc(
            "outline.add",
            json!({ "work": work_a, "target": seq, "place": "before", "title": "Küste", "level": 0 }),
        )
        .await,
    );
    let first_a = &a.state.store.load_work(work_a).await.unwrap().nodes[0];
    assert_eq!(first_a.kind, baruch_core::NodeKind::Part, "in A legt der Mensch den Teil direkt an");
    let part_a = first_a.key;
    let part_uuid = uuid_of(&a, "SELECT uuid FROM node WHERE id = ?", part_a).await;
    let xml = exchange_export(&a, work_a).await;
    let merged = exchange_import(&b, &xml, "merge").await;
    assert_eq!(current(&node(&b, work_b, key_b).await)["amend_seq"], 3, "B spult Fassung 1 auf Stand 3 vor: {merged}");
    let requests = merged["requests"].as_array().cloned().unwrap_or_default();
    assert_eq!(requests.len(), 1, "der neue Teil über vorhandenen Kapiteln ist ein Antrag: {merged}");
    assert!(
        !b.state.store.load_work(work_b).await.unwrap().nodes.iter().any(|n| n.kind == baruch_core::NodeKind::Part),
        "ohne Zustimmung ist nichts angelegt"
    );
    // Zustimmen ohne JavaScript: Formular auf der Gliederungsseite.
    let (_, page) = b.get(&format!("/gliederung?work={work_b}")).await;
    assert!(page.contains("mit Kennung und"), "die Gliederung nennt den Antrag mit Kennung");
    let decided =
        b.rpc("outline.request_decide", json!({ "work": work_b, "id": requests[0], "decision": "confirm" })).await;
    result(&decided);
    let loaded = b.state.store.load_work(work_b).await.unwrap();
    let part_b = loaded.nodes.iter().find(|n| n.kind == baruch_core::NodeKind::Part).expect("Teil angelegt");
    assert_eq!(uuid_of(&b, "SELECT uuid FROM node WHERE id = ?", part_b.key).await, part_uuid, "gleiche Kennung");
    let again = exchange_import(&b, &xml, "merge").await;
    let total: u64 = again["added"].as_object().unwrap().values().filter_map(Value::as_u64).sum();
    assert_eq!(total, 0, "erneuter Abgleich bringt nichts: {again}");
    assert_eq!(again["requests"], json!([]), "und keinen Antrag");
}

#[tokio::test]
async fn an_amended_work_comes_back_from_the_archive_with_its_current_texts() {
    let server = common::open().await;
    let (work, key, _) = meadow(&server).await;
    let original = current(&node(&server, work, key).await)["body"].as_str().unwrap().to_owned();
    save(&server, work, key, &touched(&original, "hohe"), 1).await;
    let before = current_texts(&server, work).await;
    let deleted = server.rpc("work.delete", json!({ "work": work, "confirm_title": MEADOW_TITLE })).await;
    let slug = result(&deleted)["slug"].as_str().unwrap().to_owned();
    let (status, page) = server.get("/werke/archiv").await;
    assert_eq!(status, StatusCode::OK, "Archivseite lädt");
    assert!(page.contains(MEADOW_TITLE), "das gelöschte Werk steht im Archiv");
    let (status, location) =
        server.post_form("/werke/archiv/wiederherstellen", &format!("slug={slug}&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Wiederherstellen ohne JavaScript leitet weiter");
    let restored: i64 = location.unwrap().trim_start_matches("/?work=").parse().unwrap();
    assert_eq!(
        current_texts(&server, restored).await,
        before,
        "die fortgeschriebenen Texte kommen als Fassung 1 zurück"
    );
    assert!(server.mirror_dir_of(&slug).join("main.typ").is_file(), "Spiegel zurück unter works/<slug>");
}

#[tokio::test]
async fn the_jump_label_rule_of_an_unnumbered_title_with_code_survives_export_and_import() {
    let server = common::open().await;
    let source =
        "#set heading(numbering: \"1.\")\n\n= Anfang\n\nText.\n\n= #emph[Rast]\n\nPause.\n\n= #emph[Rast]\n\nWeiter.\n";
    let work = server.import(source, "rast.typ").await.work;
    let loaded = server.state.store.load_work(work).await.unwrap();
    let pause = loaded.nodes.iter().find(|n| n.revisions.iter().any(|r| r.body.contains("Pause"))).unwrap();
    result(
        &server
            .rpc("outline.set_numbered", json!({ "work": work, "node": pause.id, "key": pause.key, "numbered": false }))
            .await,
    );
    let export =
        result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].as_str().unwrap().to_owned();
    assert!(export.contains(&format!("#show <k{}>: set heading(numbering: none)", pause.key)), "Regel im Export");

    // Re-Import in einer zweiten Instanz: der Block verschwindet aus der Vorlage, der Abschnitt ist „ohne Nummer“.
    let other = common::open().await;
    let again = other.import(&export, "rast-export.typ").await.work;
    let numbered: Vec<i64> = sqlx::query_scalar("SELECT numbered FROM node WHERE work_id = ? ORDER BY seq")
        .bind(again)
        .fetch_all(other.state.store.pool())
        .await
        .unwrap();
    assert_eq!(numbered, vec![1, 0, 1], "nur der mittlere „Rast“ kommt ohne Nummer zurück");
    let template: String = sqlx::query_scalar("SELECT template FROM work WHERE id = ?")
        .bind(again)
        .fetch_one(other.state.store.pool())
        .await
        .unwrap();
    assert!(!template.contains("über die Sprungmarke"), "kein toter Regelblock in der Vorlage: {template}");
    let export2 =
        result(&other.rpc("work.export", json!({ "work": again })).await)["typst"].as_str().unwrap().to_owned();
    let key2 = other.state.store.load_work(again).await.unwrap().nodes[1].key;
    assert!(
        export2.contains(&format!("#show <k{key2}>: set heading(numbering: none)")),
        "der zweite Export setzt die Regel mit dem neuen Schlüssel: {export2}"
    );
}
