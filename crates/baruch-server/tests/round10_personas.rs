// SPDX-License-Identifier: AGPL-3.0-or-later
//! Personas wirklich (Runde 10, „personas“): Aufträge an Personas über RPC, Formular (ohne JavaScript),
//! Strom (SSE) und MCP, Kontextaufbau, Rollen-Prompts, Lektor-Wahl und Einfall an eine Persona.
//!
//! Alle Tests nutzen `baruch_server::assist::mock::ScriptedModel` hinter `AppState::open_with` – kein
//! Netz, keine GPU, kein echter Modell-Server. Die Texte sind frei erfunden (ein kleines Bienenbuch).

use crate::common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::{ModelFailure, Role, prompts};
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use tower::ServiceExt;

const BEES: &str = "#set document(title: \"Das Bienenbuch\", author: \"Mara Wabe\")\n#set heading(numbering: \"1.1\")\n\n\
= Frühjahr\n\nDie Völker erwachen im März.\n\n\
== Ausflug\n\nDie ersten Sammlerinnen fliegen an warmen Tagen zum Weidenkätzchen.\n\n\
== Pollen\n\nGelbe Höschen an den Beinen zeigen, dass das Volk brütet.\n\n\
= Sommer\n\nDer Honigraum füllt sich langsam.\n";

struct Bees {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    /// Schlüssel von Frühjahr, Ausflug, Pollen, Sommer.
    keys: Vec<i64>,
}

async fn with_model(model: ScriptedModel) -> Bees {
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
    let report = server.import(BEES, "bienen.typ").await;
    let work = report.work;
    let updated = server.rpc("work.update", json!({ "work": work, "audience": "Hobby-Imker" })).await;
    assert!(updated["result"].is_object(), "Zielgruppe gesetzt: {updated}");
    let keys = server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Bees { server, model, work, keys }
}

async fn bees(steps: impl IntoIterator<Item = Step>) -> Bees {
    with_model(ScriptedModel::with(steps)).await
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

fn system(call: &baruch_server::assist::ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::System).map(|m| m.content.as_str()).unwrap()
}

fn user(call: &baruch_server::assist::ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::User).map(|m| m.content.as_str()).unwrap()
}

async fn posts(b: &Bees, node: usize) -> Vec<baruch_core::Post> {
    b.server.state.store.load_work(b.work).await.unwrap().nodes[node].posts.clone()
}

// ---- RPC ------------------------------------------------------------------------------------------

#[tokio::test]
async fn asking_a_persona_stores_the_question_and_the_answer_with_the_right_authors() {
    let b = bees([Step::words("Kritisch: „zeigen, dass das Volk brütet“ ist zu sicher.\n- Schritt: prüfen.")]).await;
    let answer = b
        .server
        .rpc(
            "persona.ask",
            json!({ "work": b.work, "key": b.keys[2], "persona": "@lektor", "question": "Stimmt das?" }),
        )
        .await;
    let r = result(&answer);
    assert_eq!(r["post"]["author"], "@lektor", "Antwort als Beitrag der Persona: {r}");
    assert_eq!(
        r["post"]["body"], "Kritisch: „zeigen, dass das Volk brütet“ ist zu sicher.\n- Schritt: prüfen.",
        "Text wie geliefert"
    );
    assert_eq!(r["question_post"]["author"], "Autor", "die Frage steht als Beitrag des Menschen davor");
    assert_eq!(r["model"], "google/gemma-4-26b-a4b", "tatsächliches Modell aus der Antwort");
    assert_eq!(r["truncated"], false, "großer Kontext: nichts gekürzt");
    assert_eq!(r["url"], format!("/?work={}&key={}", b.work, b.keys[2]), "Adresse in Key-Form");
    let stored = posts(&b, 2).await;
    assert_eq!(stored.len(), 2, "Frage und Antwort gespeichert");
    assert_eq!((stored[0].author.as_str(), stored[0].body.as_str()), ("Autor", "Stimmt das?"), "Frage zuerst");
    assert_eq!(stored[1].author, "@lektor", "dann die Antwort");
    let calls = b.model.calls();
    assert_eq!(calls.len(), 1, "genau ein Modellaufruf");
    assert_eq!(calls[0].model, "google/gemma-4-26b-a4b", "Modell der Persona");
    assert_eq!(calls[0].max_tokens, 700, "ask: 700 Tokens");
    assert_eq!(calls[0].temperature, Some(0.4), "Temperatur 0,4");
    assert_eq!(calls[0].label, format!("@lektor ask key {}", b.keys[2]), "Bezeichnung ohne Text");
}

#[tokio::test]
async fn the_context_has_the_shown_version_the_chapter_and_ideas_only_for_the_lector() {
    let b =
        bees([Step::words("Kritisch: eins.\n- Schritt: prüfen."), Step::words("Kritisch: zwei.\n- Schritt: prüfen.")])
            .await;
    // Zweite Fassung von „Pollen“, dann die erste zeigen (rev 1).
    let saved = b
        .server
        .rpc(
            "revision.save",
            json!({ "work": b.work, "key": b.keys[2], "body": "== Pollen\n\nNeu: Bienenbrot im Wabenrand.\n" }),
        )
        .await;
    assert!(saved["result"]["rev"] == 2, "zweite Fassung aktuell: {saved}");
    let idea = b
        .server
        .rpc(
            "idea.create",
            json!({ "work": b.work, "key": b.keys[2], "words": ["Bienenbrot"], "note": "Gärung erklären" }),
        )
        .await;
    assert!(idea["result"]["idea"]["id"].is_number(), "Einfall mit Bezug: {idea}");
    b.server.rpc("idea.create", json!({ "work": b.work, "words": ["Schwarmzeit"], "note": "Mai oder Juni?" })).await;
    b.server.rpc("idea.create", json!({ "work": b.work, "key": b.keys[0], "words": ["Märzbecher"] })).await;

    let lector = b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[2], "rev": 1 })).await;
    assert_eq!(result(&lector)["post"]["author"], "@lektor", "review ohne persona: der Lektor");
    let exeget = b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[2], "persona": "exeget" })).await;
    assert_eq!(result(&exeget)["post"]["author"], "@exeget", "Handle ohne @ angenommen");
    let calls = b.model.calls();
    let first = user(&calls[0]);
    assert!(first.contains("Gelbe Höschen an den Beinen"), "gezeigte Fassung 1, nicht die aktuelle: {first}");
    assert!(!first.contains("Bienenbrot im Wabenrand"), "die aktuelle Fassung 2 steht nicht drin: {first}");
    assert!(first.contains("(Fassung 1)"), "Nummer der gezeigten Fassung: {first}");
    assert!(first.contains("Kapitel: 1 Frühjahr"), "Kapitelumfeld: Kapitel: {first}");
    // Seit Runde 15 steht das ganze Kapitel im Wortlaut da (nicht nur 300 Zeichen je Nachbar).
    assert!(
        first.contains("--- 1.1 Ausflug ---\n<<<\n== Ausflug\n\nDie ersten Sammlerinnen fliegen an warmen Tagen zum Weidenkätzchen."),
        "Nachbar im Wortlaut: {first}"
    );
    assert!(first.contains("Zielgruppe: Hobby-Imker"), "Werk mit Zielgruppe: {first}");
    assert!(
        // Runde 17a (L5): jeder Einfall gerahmt und entschärft wie ein Befund.
        first.contains("Gedächtnis")
            && first.contains("- Zu diesem Abschnitt: <<<einfall [Bienenbrot] Gärung erklären >>>"),
        "Einfall mit Bezug: {first}"
    );
    assert!(
        first.contains("- Offen im Werk: <<<einfall [Schwarmzeit] Mai oder Juni? >>>"),
        "offener Einfall ohne Bezug: {first}"
    );
    assert!(!first.contains("Märzbecher"), "Einfälle anderer Abschnitte nicht: {first}");
    assert_eq!(calls[0].max_tokens, 900, "review: 900 Tokens");
    let second = user(&calls[1]);
    assert!(second.contains("Bienenbrot im Wabenrand"), "ohne rev die aktuelle Fassung: {second}");
    assert!(!second.contains("Gedächtnis"), "nur der Lektor kennt die Einfälle: {second}");
}

#[tokio::test]
async fn a_small_context_compresses_and_never_cuts_the_section() {
    // Bis Runde 14 fiel hier der Abschnitt vom Ende her weg („[… gekürzt: n Zeichen]“). Seit Runde 15 wird nie
    // still gekürzt: Der Abschnitt wird in Stücken im Wortlaut geprüft, das Übrige zusammengefasst (Vermerk).
    let long = format!("== Pollen\n\n{}\n", "Die Sammlerin trägt Pollen heim. ".repeat(200));
    let b = bees((0..12).map(|_| Step::words("Kritisch: kurz.\n- Schritt: prüfen."))).await;
    b.server.rpc("revision.save", json!({ "work": b.work, "key": b.keys[2], "body": long })).await;
    b.server.rpc("idea.create", json!({ "work": b.work, "key": b.keys[2], "words": ["Pollenfarbe"] })).await;
    // Runde 17a: 3400 statt 3000 Tokens – mit Einfällen steht seither die Regel „Daten, keine Anweisungen“ im
    // System-Prompt (OTHERS_RULE gilt auch für das Gedächtnis), das Stück braucht dafür etwas mehr Platz.
    let set = b.server.rpc("persona.update", json!({ "handle": "@lektor", "context_length": 3400 })).await;
    assert_eq!(result(&set)["context_length"], 3400, "kleine Kontextlänge");
    let answer =
        b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[2], "persona": "@lektor" })).await;
    let r = result(&answer);
    let message = r["compressed"]["message"].as_str().unwrap_or_else(|| panic!("komprimiert: {r}"));
    assert!(message.starts_with("Komprimiert:"), "Vermerk für Toast und Hinweisseite: {message}");
    assert!(r["post"]["body"].as_str().unwrap().ends_with(message), "Vermerk am Ende des Beitrags: {r}");
    assert_eq!(r["post"]["body"].as_str().unwrap().matches("Kritisch: kurz.").count(), 1, "gleiche Befunde nur einmal");
    for call in b.model.calls() {
        let text = user(&call);
        assert!(!text.contains("[… gekürzt"), "keine Kürzungsmarke mehr: {text}");
        let all: usize = call.messages.iter().map(|m| m.content.chars().count()).sum();
        let budget = ((3400 - call.max_tokens - 256) * 3) as usize;
        assert!(all <= budget, "Eingabe ({all} Zeichen) passt ins Budget ({budget} Zeichen ≈ Tokens·3)");
    }
}

#[tokio::test]
async fn a_forbidden_opening_is_asked_again_once_and_then_the_first_sentence_goes() {
    let b = bees([
        Step::words("Als Assistent sehe ich hier Gutes. Kritisch: zu kurz.\n- Schritt: prüfen."),
        Step::words("Gerne! Kritisch: „langsam“ fehlt der Beleg.\n- Schritt: prüfen."),
    ])
    .await;
    let answer =
        b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[3], "persona": "@nathan" })).await;
    assert_eq!(
        result(&answer)["post"]["body"],
        "Kritisch: „langsam“ fehlt der Beleg.\n- Schritt: prüfen.",
        "erster Satz der zweiten Antwort entfernt"
    );
    let calls = b.model.calls();
    assert_eq!(calls.len(), 2, "genau ein zweiter Aufruf");
    assert!(!system(&calls[0]).contains(prompts::RETRY_NOTE), "erster Aufruf ohne Zusatz");
    assert!(system(&calls[1]).contains(prompts::RETRY_NOTE), "zweiter Aufruf mit Zusatzanweisung");

    // Runde 16a: „Kritisch:“ zählt nur am Zeilenanfang, darum steht der Befund in einer eigenen Zeile.
    let b = bees([Step::words("Natürlichkeit ist hier ein gutes Wort.\nKritisch: nichts weiter.\n- Schritt: prüfen.")])
        .await;
    let answer =
        b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[3], "persona": "@nathan" })).await;
    assert!(result(&answer)["post"]["body"].as_str().unwrap().starts_with("Natürlichkeit"), "nur ganze Wörter zählen");
    assert_eq!(b.model.calls().len(), 1, "erlaubter Anfang: kein zweiter Aufruf");
}

#[tokio::test]
async fn a_proposal_is_exactly_one_proposed_with_the_heading_of_the_shown_version() {
    let b = bees([
        Step::words("```typ\n= Anderer Titel\n\nDie Völker erwachen schon im Februar.\n```"),
        Step::words("= Frühjahr\n\nDie Völker erwachen im März.\n"),
    ])
    .await;
    let before = b.server.state.store.load_work(b.work).await.unwrap().nodes[0].revisions.clone();
    let answer = b
        .server
        .rpc(
            "persona.propose",
            json!({ "work": b.work, "key": b.keys[0], "persona": "@lektor", "instruction": "früher" }),
        )
        .await;
    let r = result(&answer);
    assert_eq!(r["status"], "proposed", "{r}");
    assert_eq!(r["author"], "@lektor", "Autor ist die Persona");
    let rev = r["rev"].as_u64().unwrap() as u32;
    assert_eq!(r["url"], format!("/?work={}&key={}&rev={rev}", b.work, b.keys[0]), "Adresse des Vorschlags");
    let node = b.server.state.store.load_work(b.work).await.unwrap().nodes[0].clone();
    assert_eq!(node.revisions.len(), before.len() + 1, "genau ein Vorschlag mehr");
    let added = node.revision(rev).unwrap();
    assert_eq!(added.status, baruch_core::RevisionStatus::Proposed, "nur proposed, nie current");
    assert_eq!(
        added.body, "= Frühjahr\n\nDie Völker erwachen schon im Februar.\n",
        "Codeblock weg, Überschrift erzwungen"
    );
    assert_eq!(added.note, "Vorschlag von @lektor", "Notiz");
    assert!(
        matches!(&added.author, baruch_core::Author::Assistant { persona, model } if persona == "@lektor" && model == "google/gemma-4-26b-a4b"),
        "Herkunft: Persona und Modell: {:?}",
        added.author
    );
    assert_eq!(node.current_number(), Some(1), "die aktuelle Fassung bleibt");
    assert!(user(&b.model.calls()[0]).contains("Auftrag: früher"), "Anweisung im Auftrag");
    assert!(user(&b.model.calls()[0]).contains("Überschriftszeile, unverändert: = Frühjahr"), "Überschrift vorgegeben");

    let same = b.server.rpc("persona.propose", json!({ "work": b.work, "key": b.keys[0], "persona": "@lektor" })).await;
    assert_eq!(error_code(&same), -32000, "gleich der aktuellen → ehrlicher Fehler: {same}");
    let node = b.server.state.store.load_work(b.work).await.unwrap().nodes[0].clone();
    assert_eq!(node.revisions.len(), before.len() + 1, "kein weiterer Vorschlag");
}

#[tokio::test]
async fn chat_posts_the_human_message_then_each_mentioned_active_persona_once() {
    let b = bees([
        Step::words("Kritisch: Weide blüht früher.\n- Schritt: prüfen."),
        Step::words("Kritisch: Satz zwei.\n- Schritt: prüfen."),
    ])
    .await;
    b.server.rpc("persona.update", json!({ "handle": "@nathan", "active": false })).await;
    let answer = b
        .server
        .rpc(
            "persona.chat",
            json!({ "work": b.work, "key": b.keys[1], "message": "@Exeget und @lektor, dann @exeget, @nathan?" }),
        )
        .await;
    let r = result(&answer);
    let authors: Vec<&str> = r["posts"].as_array().unwrap().iter().map(|p| p["author"].as_str().unwrap()).collect();
    assert_eq!(authors, ["Autor", "@exeget", "@lektor"], "Mensch, dann je erwähnte aktive Persona einmal: {r}");
    assert_eq!(b.model.calls().len(), 2, "die inaktive @nathan antwortet nicht");
    assert_eq!(b.model.calls()[0].max_tokens, 500, "chat: 500 Tokens");
    assert!(
        user(&b.model.calls()[1]).contains("@exeget: <<<beitrag Kritisch: Weide blüht früher. - Schritt: prüfen. >>>"),
        "der zweite sieht den Verlauf (seit Runde 16a gerahmt als Daten)"
    );

    let none =
        b.server.rpc("persona.chat", json!({ "work": b.work, "key": b.keys[1], "message": "Nur eine Notiz." })).await;
    assert_eq!(result(&none)["posts"].as_array().unwrap().len(), 1, "ohne Erwähnung nur der Beitrag");
    assert_eq!(b.model.calls().len(), 2, "kein Modellaufruf");
}

#[tokio::test]
async fn an_answer_cut_at_the_length_limit_is_marked_and_a_cut_proposal_is_not_saved() {
    let b = bees([
        Step::cut("Kritisch: der zweite Satz bricht"),
        Step::cut("= Frühjahr\n\nDie Völker erwachen im"),
        Step::words("Kritisch: vollständig.\n- Schritt: prüfen."),
    ])
    .await;
    let review =
        b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[2], "persona": "@lektor" })).await;
    let r = result(&review);
    assert_eq!(r["cut"], true, "finish_reason „length“ → cut: {r}");
    assert_eq!(
        r["post"]["body"],
        format!("Kritisch: der zweite Satz bricht …\n\n{}", baruch_server::assist::CUT_NOTE),
        "der Beitrag sagt, dass er unvollständig ist"
    );

    // Ein abgeschnittener Vorschlag wäre ein Abschnitt ohne Ende; übernähme ihn der Mensch, fehlte der Rest.
    let before = b.server.state.store.load_work(b.work).await.unwrap().nodes[0].revisions.len();
    let propose =
        b.server.rpc("persona.propose", json!({ "work": b.work, "key": b.keys[0], "persona": "@lektor" })).await;
    assert_eq!(error_code(&propose), -32000, "abgeschnittener Vorschlag → ehrlicher Fehler: {propose}");
    assert!(propose["error"]["message"].as_str().unwrap().contains("Längengrenze"), "Meldung nennt den Grund");
    let after = b.server.state.store.load_work(b.work).await.unwrap().nodes[0].revisions.len();
    assert_eq!(after, before, "kein Vorschlag angelegt");

    let ask = b
        .server
        .rpc("persona.ask", json!({ "work": b.work, "key": b.keys[2], "persona": "@lektor", "question": "Und?" }))
        .await;
    assert_eq!(result(&ask)["cut"], false, "finish_reason „stop“ → nicht abgeschnitten");
    assert!(!result(&ask)["post"]["body"].as_str().unwrap().contains("abgeschnitten"), "kein Hinweis");
}

/// Werk mit sieben langen Beiträgen „Alt-0“ … „Alt-6“ im Chat von „Pollen“, dann die Nachricht an @lektor;
/// gibt den Modellaufruf zurück. `context`: Kontextlänge des Lektors.
async fn chat_after_long_history(context: Option<u32>) -> (baruch_server::assist::ModelCall, Value) {
    let b = bees([Step::words("Kritisch: kurz.\n- Schritt: prüfen.")]).await;
    for n in 0..7 {
        let message = format!("Alt-{n} {}", "Wabe ".repeat(118));
        let posted =
            b.server.rpc("persona.chat", json!({ "work": b.work, "key": b.keys[2], "message": message })).await;
        assert_eq!(result(&posted)["posts"].as_array().unwrap().len(), 1, "nur der Beitrag, kein Modellaufruf");
    }
    if let Some(context) = context {
        let set = b.server.rpc("persona.update", json!({ "handle": "@lektor", "context_length": context })).await;
        assert_eq!(result(&set)["context_length"], context, "Kontextlänge gesetzt");
    }
    let message = json!({ "work": b.work, "key": b.keys[2], "message": "@lektor Neueste Frage zur Farbe?" });
    let answer = b.server.rpc("persona.chat", message).await;
    let r = result(&answer).clone();
    let calls = b.model.calls();
    assert_eq!(calls.len(), 1, "genau ein Modellaufruf");
    (calls[0].clone(), r)
}

#[tokio::test]
async fn a_short_context_drops_the_oldest_chat_posts_and_keeps_the_newest() {
    // Erst mit großem Kontext messen, wie lang die Eingabe ungekürzt ist, dann denselben Verlauf mit einer
    // Kontextlänge, der rund 1500 Zeichen fehlen: Das Kapitelumfeld (~100 Zeichen) reicht nicht, also fallen
    // Chatbeiträge (je ~600 Zeichen) – drei, und zwar die ältesten.
    let (full, _) = chat_after_long_history(None).await;
    let text = user(&full);
    assert!(text.contains("Alt-0 ") && text.contains("Neueste Frage"), "ungekürzt ist alles drin: {text}");
    let total: usize = full.messages.iter().map(|m| m.content.chars().count()).sum();
    // Runde 30b: Mit großem Kontext stehen zusätzlich die Gliederung mit Schlüsseln und die Regel für Sprungmarken
    // im Aufruf – aber nur, solange Platz ist; beim knappen Kontext fallen sie zuerst weg. Gemessen wird darum ohne sie.
    let outline_start = text.find("\n\nGliederung (").expect("bei großem Kontext steht die Gliederung da");
    let outline_end = text[outline_start + 2..].find("\n\n").map_or(text.len(), |n| outline_start + 2 + n);
    let outline_len = text[outline_start..outline_end].chars().count();
    let total = total - outline_len - baruch_server::assist::context::JUMP_RULE.chars().count() - 2;
    let context = u32::try_from((total - 1500) / 3).unwrap() + 500 + 256;
    let (short, r) = chat_after_long_history(Some(context)).await;
    let text = user(&short);
    assert_eq!(r["truncated"], true, "gekürzt: {r}");
    assert!(!text.contains("[… gekürzt"), "der Abschnitt selbst bleibt ganz: {text}");
    for gone in ["Alt-0 ", "Alt-1 ", "Alt-2 "] {
        assert!(!text.contains(gone), "die ältesten Beiträge fallen zuerst ({gone}): {text}");
    }
    for kept in ["Alt-3 ", "Alt-6 ", "Autor: <<<beitrag @lektor Neueste Frage zur Farbe?"] {
        assert!(text.contains(kept), "jüngere Beiträge und die Nachricht selbst bleiben ({kept}): {text}");
    }
}

#[tokio::test]
async fn unknown_inactive_and_failing_personas_are_refused_and_save_nothing() {
    let b = bees([Step::Fail(ModelFailure::Busy("Warteschlange voll".into()))]).await;
    b.server.rpc("persona.update", json!({ "handle": "@hoerer", "active": false })).await;
    for (persona, why) in [("@niemand", "unbekannt"), ("@hoerer", "inaktiv")] {
        let answer = b
            .server
            .rpc("persona.ask", json!({ "work": b.work, "key": b.keys[1], "persona": persona, "question": "?" }))
            .await;
        assert_eq!(error_code(&answer), -32602, "{why} → -32602: {answer}");
    }
    let by_node =
        b.server.rpc("persona.ask", json!({ "work": b.work, "node": 1, "persona": "@lektor", "question": "?" })).await;
    assert_eq!(error_code(&by_node), -32602, "node wird für Schreibendes abgelehnt: {by_node}");
    let long = b
        .server
        .rpc(
            "persona.ask",
            json!({ "work": b.work, "key": b.keys[1], "persona": "@lektor", "question": "x".repeat(4001) }),
        )
        .await;
    assert_eq!(error_code(&long), -32602, "Frage über 4000 Zeichen: {long}");
    let busy = b
        .server
        .rpc("persona.ask", json!({ "work": b.work, "key": b.keys[1], "persona": "@lektor", "question": "Wann?" }))
        .await;
    assert_eq!(error_code(&busy), -32000, "Fehler des Backends → -32000: {busy}");
    assert!(busy["error"]["message"].as_str().unwrap().contains("Warteschlange voll"), "ehrliche Meldung: {busy}");
    assert!(posts(&b, 1).await.is_empty(), "nichts gespeichert, auch nicht die Frage");

    let plain = common::open().await;
    let work = plain.state.default_work().await.unwrap();
    let key = plain.state.store.load_work(work).await.unwrap().nodes[2].key;
    let none = plain.rpc("persona.review", json!({ "work": work, "key": key, "persona": "@lektor" })).await;
    assert_eq!(error_code(&none), -32000, "ohne Backend: {none}");
    assert!(none["error"]["message"].as_str().unwrap().contains("Kein Modell-Backend angebunden"), "ehrlich: {none}");
}

#[tokio::test]
async fn the_write_lock_is_free_while_the_model_works() {
    let b = bees([Step::Wait {
        before: vec!["Kritisch: ".into()],
        then: Box::new(Step::words("langsam.\n- Schritt: prüfen.")),
    }])
    .await;
    let app = b.server.app.clone();
    let (work, key) = (b.work, b.keys[3]);
    let asking = tokio::spawn(async move {
        common::rpc(app, "persona.ask", json!({ "work": work, "key": key, "persona": "@lektor", "question": "Wie?" }))
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), b.model.started()).await.expect("Modellaufruf beginnt");
    let save = tokio::time::timeout(
        Duration::from_secs(5),
        b.server.rpc("revision.save", json!({ "work": b.work, "key": b.keys[0], "body": "= Frühjahr\n\nAnders.\n" })),
    )
    .await
    .expect("Speichern läuft durch, während das Modell arbeitet (keine Schreibsperre gehalten)");
    assert_eq!(save["result"]["rev"], 2, "gespeichert: {save}");
    b.model.release();
    let answer = tokio::time::timeout(Duration::from_secs(5), asking).await.unwrap().unwrap();
    assert_eq!(
        result(&answer)["post"]["body"],
        "Kritisch: langsam.\n- Schritt: prüfen.",
        "danach gespeichert: {answer}"
    );
}

#[tokio::test]
async fn idea_process_as_persona_asks_the_lector_and_marks_the_idea_done() {
    let b = bees([Step::words("Kritisch: Bienenbrot gehört in den Abschnitt.\n- Schritt: prüfen.")]).await;
    let created = b
        .server
        .rpc(
            "idea.create",
            json!({ "work": b.work, "key": b.keys[2], "words": ["Bienenbrot"], "note": "Bienenbrot erwähnen?" }),
        )
        .await;
    let id = created["result"]["idea"]["id"].as_i64().unwrap();
    let processed = b.server.rpc("idea.process", json!({ "id": id, "as": "persona" })).await;
    let r = result(&processed);
    assert_eq!(r["idea"]["status"], "done", "verarbeitet: {r}");
    assert_eq!(r["idea"]["done_as"], "persona", "als Persona-Auftrag");
    let post_id = r["post_id"].as_i64().unwrap();
    assert_eq!(r["idea"]["done_ref"], format!("key {}, Beitrag {post_id}", b.keys[2]), "Verweis auf den Beitrag");
    let stored = posts(&b, 2).await;
    let last = stored.last().unwrap();
    assert_eq!(last.author, "@lektor", "Vorgabe: der Lektor antwortet");
    assert_eq!(last.sources, [format!("Einfall {id}")], "Quelle nennt den Einfall");
    assert!(user(&b.model.calls()[0]).contains("Bienenbrot erwähnen?"), "der Einfall ist die Frage");
}

// ---- Formular und Strom ------------------------------------------------------------------------

#[tokio::test]
async fn the_forms_work_without_javascript_and_redirect_to_the_key_address() {
    let b = bees([
        Step::words("Kritisch: gut belegt fehlt.\n- Schritt: prüfen."),
        Step::words("= Sommer\n\nDer Honigraum füllt sich im Juli.\n"),
    ])
    .await;
    let form = format!(
        "method=persona.review&work={}&key={}&persona=%40nathan&rev=1&view=edit&instruction=",
        b.work, b.keys[3]
    );
    let (status, location) = b.server.post_form("/assistenten/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Prüfen ohne JavaScript → 303");
    assert_eq!(location.as_deref(), Some(format!("/?work={}&key={}&rev=1", b.work, b.keys[3]).as_str()), "Key-Adresse");
    assert_eq!(posts(&b, 3).await.last().unwrap().author, "@nathan", "Befunde als Beitrag");
    let form = format!(
        "method=persona.propose&work={}&key={}&persona=%40lektor&rev=1&view=preview&lq=Honig",
        b.work, b.keys[3]
    );
    let (status, location) = b.server.post_form("/assistenten/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Vorschlag ohne JavaScript → 303");
    assert_eq!(
        location.as_deref(),
        Some(format!("/?work={}&key={}&rev=2&view=preview&lq=Honig", b.work, b.keys[3]).as_str()),
        "zum neuen Vorschlag, Ansicht und Suche bleiben"
    );
    let (status, _) = b
        .server
        .post_form(
            "/assistenten/act",
            &format!("method=persona.ask&work={}&key={}&persona=%40lektor&question=x", b.work, b.keys[3]),
        )
        .await;
    // Runde 15: statt 422 die Hinweisseite (200) mit der Meldung und dem Link zurück (tests/round15_auftraege.rs).
    assert_eq!(status, StatusCode::OK, "Skript zu Ende → ehrliche Hinweisseite statt 422");
    let (status, _) = b.server.post_form("/assistenten/act", "method=persona.set_prompt").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "nur Aufträge über dieses Formular");
    let request = Request::post("/assistenten/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "http://evil.example")
        .body(Body::from(form))
        .unwrap();
    let (status, ..) = b.server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremder Origin → 403");
}

/// Ereignisse aus einem SSE-Text: (Name, Daten).
fn events(text: &str) -> Vec<(String, Value)> {
    text.split("\n\n")
        .filter_map(|block| {
            let name = block.lines().find_map(|l| l.strip_prefix("event:")).map(|n| n.trim().to_owned())?;
            let data: String = block.lines().filter_map(|l| l.strip_prefix("data:")).map(str::trim_start).collect();
            Some((name, serde_json::from_str(&data).unwrap()))
        })
        .collect()
}

fn stream_request(body: Value) -> Request<Body> {
    Request::post("/assistenten/stream")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn the_stream_sends_start_deltas_and_done_in_order_and_saves() {
    let text = "Kritisch: Die Höschen sind kein sicheres Zeichen für Brut.\n- Schritt: prüfen.";
    let b = bees([Step::words(text)]).await;
    let request = stream_request(
        json!({ "method": "persona.ask", "work": b.work, "key": b.keys[2], "persona": "@exeget", "question": "Sicher?" }),
    );
    let (status, headers, body) = b.server.send(request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/event-stream"), "SSE");
    let got = events(&body);
    let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names.first(), Some(&"start"), "zuerst start: {names:?}");
    assert_eq!(names.last(), Some(&"done"), "zuletzt done: {names:?}");
    assert!(
        names[1..names.len() - 1].iter().all(|n| *n == "delta") && names.len() > 4,
        "dazwischen nur Deltas: {names:?}"
    );
    assert_eq!(got[0].1["persona"], "@exeget", "start nennt die Persona");
    let streamed: String = got.iter().filter(|(n, _)| n == "delta").map(|(_, d)| d["text"].as_str().unwrap()).collect();
    assert_eq!(streamed, text, "Wort für Wort die ganze Antwort");
    let done = &got.last().unwrap().1;
    assert_eq!(done["kind"], "post", "Beitrag: {done}");
    assert_eq!(done["post"]["author"], "@exeget", "{done}");
    assert_eq!(done["url"], format!("/?work={}&key={}", b.work, b.keys[2]), "Adresse für baruch:navigate");
    assert_eq!(posts(&b, 2).await.last().unwrap().body, text, "gespeichert");

    let (status, ..) = b
        .server
        .send(
            Request::post("/assistenten/stream")
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "nur JSON");
    let failed = b.server.send(stream_request(json!({ "method": "persona.ask", "work": b.work, "key": b.keys[2], "persona": "@lektor", "question": "Noch?" }))).await;
    let failed_events = events(&failed.2);
    assert_eq!(failed_events.last().map(|(n, _)| n.as_str()), Some("error"), "Fehler als Ereignis error: {}", failed.2);
}

#[tokio::test]
async fn dropping_the_stream_cancels_the_model_call_and_saves_nothing() {
    let b = bees([Step::Hang]).await;
    let request =
        stream_request(json!({ "method": "persona.review", "work": b.work, "key": b.keys[2], "persona": "@lektor" }));
    let response = b.server.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "der Strom beginnt (200), bevor das Modell antwortet");
    let mut body = response.into_body().into_data_stream();
    let first = tokio::time::timeout(Duration::from_secs(5), body.next()).await.expect("start kommt").unwrap().unwrap();
    assert!(String::from_utf8_lossy(&first).contains("event: start"), "erst start: {first:?}");
    assert_eq!(b.model.cancelled(), 0, "noch läuft der Aufruf");
    drop(body);
    for _ in 0..100 {
        if b.model.cancelled() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(b.model.cancelled(), 1, "Trennen des Clients bricht den Modellaufruf ab");
    assert!(posts(&b, 2).await.is_empty(), "nichts gespeichert");
}

// ---- MCP ----------------------------------------------------------------------------------------

async fn mcp(b: &Bees, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = b.server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

async fn session(b: &Bees, name: &str) -> String {
    let (headers, answer) = mcp(
        b,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": name, "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn tool(b: &Bees, session: &str, name: &str, arguments: Value) -> Value {
    mcp(b, Some(session), json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": name, "arguments": arguments } }))
        .await
        .1["result"]
        .clone()
}

#[tokio::test]
async fn mcp_ask_persona_asks_reviews_and_proposes_with_the_client_as_asker() {
    let b = bees([
        Step::words("Kritisch: März ist früh.\n- Schritt: prüfen."),
        Step::words("Kritisch: Befund.\n- Schritt: prüfen."),
        Step::words("= Sommer\n\nDer Honigraum füllt sich im Juli.\n"),
        Step::Fail(ModelFailure::Unreachable("Eliezer schläft".into())),
    ])
    .await;
    let s = session(&b, "Probe").await;
    let asked = tool(
        &b,
        &s,
        "ask_persona",
        json!({ "work": b.work, "key": b.keys[0], "persona": "@lektor", "question": "Früh?" }),
    )
    .await;
    assert_eq!(asked["isError"], false, "{asked}");
    let r = &asked["structuredContent"];
    assert_eq!((r["mode"].as_str(), r["model"].as_str()), (Some("ask"), Some("google/gemma-4-26b-a4b")), "{r}");
    assert_eq!(r["text"], "Kritisch: März ist früh.\n- Schritt: prüfen.", "Text der Antwort");
    assert!(r["post_id"].is_number() && r["url"].is_string() && r["truncated"] == false, "{r}");
    let stored = posts(&b, 0).await;
    assert_eq!(
        (stored[0].author.as_str(), stored[1].author.as_str()),
        ("@mcp:probe", "@lektor"),
        "Frage vom Client, Antwort der Persona"
    );

    let reviewed = tool(
        &b,
        &s,
        "ask_persona",
        json!({ "work": b.work, "key": b.keys[3], "persona": "@nathan", "mode": "review" }),
    )
    .await;
    assert_eq!(reviewed["structuredContent"]["mode"], "review", "{reviewed}");
    assert_eq!(posts(&b, 3).await.len(), 1, "review: nur der Befund, keine Frage");
    let proposed = tool(
        &b,
        &s,
        "ask_persona",
        json!({ "work": b.work, "key": b.keys[3], "persona": "@lektor", "mode": "propose" }),
    )
    .await;
    let p = &proposed["structuredContent"];
    assert_eq!((p["status"].as_str(), p["rev"].as_u64()), (Some("proposed"), Some(2)), "nur ein Vorschlag: {proposed}");
    let node = b.server.state.store.load_work(b.work).await.unwrap().nodes[3].clone();
    assert_eq!(node.current_number(), Some(1), "die aktuelle Fassung bleibt");

    let failed =
        tool(&b, &s, "ask_persona", json!({ "work": b.work, "key": b.keys[3], "persona": "@lektor", "question": "?" }))
            .await;
    assert_eq!(failed["isError"], true, "Backend-Fehler → isError: {failed}");
    assert!(failed["content"][0]["text"].as_str().unwrap().contains("Eliezer schläft"), "ehrliche Meldung: {failed}");
    let by_node = mcp(
        &b,
        Some(&s),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": { "name": "ask_persona", "arguments": { "work": b.work, "node": 3, "persona": "@lektor" } } }),
    )
    .await
    .1;
    assert_eq!(by_node["error"]["code"], -32602, "node wird abgelehnt: {by_node}");
}

#[tokio::test]
async fn mcp_chat_calls_at_most_three_mentioned_personas_and_never_relays() {
    let b = bees([
        Step::words("Kritisch: @nathan sollte das prüfen.\n- Schritt: prüfen."),
        Step::words("Kritisch: zwei.\n- Schritt: prüfen."),
        Step::words("Kritisch: drei.\n- Schritt: prüfen."),
    ])
    .await;
    let s = session(&b, "Probe").await;
    let chat = tool(
        &b,
        &s,
        "chat",
        json!({ "work": b.work, "key": b.keys[1], "message": "@exeget @hoerer @schrift @belege bitte" }),
    )
    .await;
    assert_eq!(chat["isError"], false, "{chat}");
    let authors: Vec<&str> =
        chat["structuredContent"]["posts"].as_array().unwrap().iter().map(|p| p["author"].as_str().unwrap()).collect();
    assert_eq!(authors, ["@mcp:probe", "@exeget", "@hoerer", "@schrift"], "Client, dann höchstens drei Personas");
    assert_eq!(
        b.model.calls().len(),
        3,
        "@nathan in einer Antwort ruft keine weitere Persona (Relay-Grenze: drei Antworten erreicht)"
    );
    // Runde 15: ohne key ein Faden ohne Abschnitt (kein Stub mehr); das Skript ist aufgebraucht, darum scheitert
    // die Antwort ehrlich, der Beitrag des Clients steht im Faden.
    let thread = tool(&b, &s, "chat", json!({ "work": b.work, "message": "@lektor ohne Abschnitt" })).await;
    assert_eq!(thread["isError"], false, "ohne key: Faden ohne Abschnitt statt Stub: {thread}");
    assert!(thread["structuredContent"]["thread"].is_number(), "die Antwort nennt den Faden: {thread}");

    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "http://evil.example")
        .body(Body::from(
            json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "chat", "arguments": { "work": b.work, "key": b.keys[1], "message": "@lektor" } } })
            .to_string(),
        ))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, ..) = b.server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremder Origin → 403");
}

// ---- Rollen-Prompts und Lektor -------------------------------------------------------------------

#[tokio::test]
async fn every_default_prompt_carries_the_hard_rules_with_a_counter_example() {
    let b = bees([]).await;
    for handle in ["@lektor", "@exeget", "@schrift", "@belege", "@nathan", "@hoerer"] {
        let got = b.server.rpc("persona.prompt_get", json!({ "handle": handle })).await;
        let prompt = result(&got)["prompt"].as_str().unwrap().to_owned();
        assert_eq!(result(&got)["custom"], false, "{handle}: Vorgabe");
        for rule in [
            "Deutsch",
            "„Als Assistent“",
            "„Als KI“",
            "„Als Sprachmodell“",
            "„Gerne“",
            "„Natürlich“",
            "Falsch:",
            "Richtig:",
            "Erfinde keine Quellen",
            "wörtliches Zitat",
        ] {
            assert!(prompt.contains(rule), "{handle}: Regel {rule:?} fehlt: {prompt}");
        }
        let critical = matches!(handle, "@lektor" | "@exeget" | "@nathan");
        assert_eq!(
            prompt.contains("„Kritisch:“"),
            critical,
            "{handle}: kritischer Befund nur bei Lektor, Exeget, Nathan"
        );
        assert!(prompt.contains("Bibelverse") || prompt.contains("Bibelzitate"), "{handle}: Regel zu Bibelzitaten");
    }
    let lektor = result(&b.server.rpc("persona.prompt_get", json!({ "handle": "@lektor" })).await)["prompt"]
        .as_str()
        .unwrap()
        .to_owned();
    // Der Kontext enthält nur Abschnitt, Kapitelumfeld und Einfälle: Der Prompt darf keine Verweise auf
    // Stellen verlangen, die das Modell nie sieht (sonst lädt er zum Erfinden ein).
    assert!(
        lektor.contains("nicht das ganze Werk") && !lektor.contains("Du kennst das ganze Werk"),
        "Lektor-Prompt sagt, was er sieht: {lektor}"
    );
    let nathan = result(&b.server.rpc("persona.prompt_get", json!({ "handle": "@nathan" })).await)["prompt"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(nathan.contains("nie frei") && nathan.contains("„unbedenklich“"), "Nathan gibt nie frei: {nathan}");
    let schrift = result(&b.server.rpc("persona.prompt_get", json!({ "handle": "schrift" })).await)["prompt"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        schrift.contains("Wortlaut ungeprüft") && schrift.contains("nie"),
        "Schrift bestätigt nie Wortlaut: {schrift}"
    );
    let belege = result(&b.server.rpc("persona.prompt_get", json!({ "handle": "@belege" })).await)["prompt"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        belege.contains("„nicht belegt“") && belege.contains("„Angabe prüfen“") && belege.contains("nicht pauschal"),
        "{belege}"
    );
    let hoerer = result(&b.server.rpc("persona.prompt_get", json!({ "handle": "@hoerer" })).await)["prompt"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(hoerer.contains("Zielgruppe") && hoerer.contains("Ich-Perspektive"), "Hörer liest als Leser: {hoerer}");
}

#[tokio::test]
async fn a_saved_prompt_replaces_the_default_and_can_be_reset() {
    let b =
        bees([Step::words("Kritisch: eins.\n- Schritt: prüfen."), Step::words("Kritisch: zwei.\n- Schritt: prüfen.")])
            .await;
    let set = b
        .server
        .rpc("persona.set_prompt", json!({ "handle": "@exeget", "prompt": "Nur Bienen. Kritisch: immer." }))
        .await;
    assert_eq!(result(&set)["custom"], true, "{set}");
    assert_eq!(result(&set)["prompt"], "Nur Bienen. Kritisch: immer.", "wirksamer Prompt");
    assert!(result(&set)["default"].as_str().unwrap().contains("Exeget"), "Vorgabe bleibt abrufbar");
    b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[1], "persona": "@exeget" })).await;
    assert!(system(&b.model.calls()[0]).starts_with("Nur Bienen."), "gespeicherter Prompt ersetzt die Vorgabe");
    let list = b.server.rpc("persona.list", json!({})).await;
    let exeget =
        result(&list)["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@exeget").cloned().unwrap();
    assert_eq!(exeget["prompt_custom"], true, "persona.list nennt den eigenen Prompt");
    for (bad, why) in [(json!(""), "leer"), (json!("x".repeat(8001)), "über 8000"), (json!(3), "keine Zahl")] {
        let refused = b.server.rpc("persona.set_prompt", json!({ "handle": "@exeget", "prompt": bad })).await;
        assert!(refused.get("error").is_some(), "{why} abgelehnt: {refused}");
    }
    let reset = b.server.rpc("persona.set_prompt", json!({ "handle": "@exeget", "prompt": null })).await;
    assert_eq!(result(&reset)["custom"], false, "zurück zur Vorgabe");
    b.server.rpc("persona.review", json!({ "work": b.work, "key": b.keys[1], "persona": "@exeget" })).await;
    assert!(system(&b.model.calls()[1]).starts_with("Du bist @exeget"), "wieder die Vorgabe");

    let page = format!("method=persona.set_prompt&handle=%40exeget&prompt=Eigener+Text&work={}", b.work);
    let (status, location) = b.server.post_form("/einstellungen/personas/act", &page).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular speichert");
    assert!(location.unwrap().contains("ok=prompt"), "Hinweis");
    let (status, location) =
        b.server.post_form("/einstellungen/personas/act", "method=persona.set_prompt&handle=%40exeget&reset=1").await;
    assert_eq!(
        (status, location.unwrap().contains("ok=vorgabe")),
        (StatusCode::SEE_OTHER, true),
        "„Vorgabe“ setzt zurück"
    );
    let (status, _) =
        b.server.post_form("/einstellungen/personas/act", "method=persona.set_prompt&handle=%40exeget&prompt=+").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "leerer Prompt → 422 mit Seite");
}

#[tokio::test]
async fn the_lector_setting_is_fixed_by_default_and_shown_everywhere() {
    let b = bees([]).await;
    let got = b.server.rpc("persona.lector_get", json!({})).await;
    assert_eq!(result(&got), &json!({ "mode": "fixed", "handle": "@lektor", "lector": "@lektor" }), "Vorgabe");
    b.server.rpc("persona.update", json!({ "handle": "@nathan", "context_length": 900000 })).await;
    let work = b.server.rpc("work.get", json!({ "work": b.work })).await;
    assert_eq!(result(&work)["lector"], "@lektor", "fest: größerer Kontext ändert nichts (work.get)");
    assert_eq!(result(&work)["work"]["lector_choice"], json!({ "mode": "fixed", "handle": "@lektor" }), "Wahl im Werk");

    let (status, location) =
        b.server.post_form("/einstellungen/personas/act", "method=persona.set_lector&mode=auto&handle=%40lektor").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne JavaScript gespeichert");
    assert!(location.unwrap().contains("ok=lektor"), "Hinweis");
    let list = b.server.rpc("persona.list", json!({})).await;
    assert_eq!(
        (result(&list)["lector"].as_str(), result(&list)["lector_mode"].as_str()),
        (Some("@nathan"), Some("auto")),
        "{list}"
    );
    let s = session(&b, "Probe").await;
    let personas = tool(&b, &s, "list_personas", json!({})).await;
    assert_eq!(personas["structuredContent"]["lector"], "@nathan", "MCP list_personas folgt: {personas}");
    assert_eq!(personas["structuredContent"]["lector_mode"], "auto", "mit Modus");
    let (_, page) = b.server.get(&format!("/?work={}", b.work)).await;
    assert!(
        page.contains("title=\"Lektor nach größtem Kontext (aktive Persona)\""),
        "Abzeichen im Assistenten-Bereich"
    );

    let set = b.server.rpc("persona.set_lector", json!({ "mode": "fixed", "handle": "hoerer" })).await;
    assert_eq!(result(&set), &json!({ "mode": "fixed", "handle": "@hoerer", "lector": "@hoerer" }), "{set}");
    let (_, page) = b.server.get(&format!("/?work={}", b.work)).await;
    assert!(page.contains("title=\"Lektor fest gewählt (Einstellungen → Assistenten)\""), "Abzeichen: fest gewählt");
    let (_, settings) = b.server.get("/einstellungen/assistenten").await;
    assert!(
        settings.contains("Lektor: <strong>Hörer</strong>") && settings.contains("fest gewählt"),
        "Einstellungsseite"
    );
    for (params, why) in [
        (json!({ "mode": "fixed", "handle": "@niemand" }), "unbekannt"),
        (json!({ "mode": "fest" }), "Modus"),
        (json!({ "mode": "fixed" }), "ohne handle"),
    ] {
        let refused = b.server.rpc("persona.set_lector", params).await;
        assert_eq!(error_code(&refused), -32602, "{why} → -32602: {refused}");
    }
    assert_eq!(
        result(&b.server.rpc("persona.lector_get", json!({})).await)["handle"],
        "@hoerer",
        "abgelehnt ändert nichts"
    );
}

#[tokio::test]
async fn the_settings_page_offers_the_model_list_and_warns_about_unknown_alias_and_mixed_models() {
    let model = ScriptedModel::new()
        .with_models(&["google/gemma-4-26b-a4b", "qwen3-4b-q4-k-m"])
        .with_alias("qwen3:4b", "qwen3-4b-q4-k-m");
    let b = with_model(model).await;
    // Runde 17a (L4): Beim bloßen Laden fragt die Seite keine Gegenstelle; die Liste kommt mit „Modellliste abfragen“.
    let (_, plain) = b.server.get("/einstellungen/assistenten").await;
    assert!(plain.contains("data-models-pending"), "ohne Abfrage nur der Hinweis „noch nicht abgefragt“: {plain}");
    let (status, page) = b.server.get("/einstellungen/assistenten?modelle=abfragen").await;
    assert_eq!(status, StatusCode::OK, "Einstellungsseite Assistenten lädt (auch mit Alias und Mischmodellen)");
    assert!(
        page.contains("<datalist id=\"baruch-models\">") && page.contains("<option value=\"qwen3-4b-q4-k-m\">"),
        "Liste aus dem Port: {page}"
    );
    assert!(page.contains("list=\"baruch-models\""), "Modellfeld mit Auswahl (Freitext bleibt)");
    assert!(page.contains("Alter Name: Alias → qwen3-4b-q4-k-m"), "Alias-Hinweis für qwen3:4b");
    assert!(page.contains("Modell „gpt-4.1-mini“ ist unbekannt bei Eliezer."), "Warnung unbekannt");
    assert!(page.contains("verschiedene Modelle") && page.contains("VRAM"), "Hinweis mehrere Modelle");
    assert!(
        page.contains("name=\"method\" value=\"persona.set_lector\"") && page.contains("Versuch: fest @lektor"),
        "Lektor-Wahl als Versuch"
    );
    // Runde 11: ein Formular je Persona (action=save) mit dem Rollen-Prompt als großem Textfeld.
    assert!(
        page.contains("<textarea class=\"persona-prompt-text\" name=\"prompt\"")
            && page.contains("Vorgabe wiederherstellen"),
        "Rollen-Prompt editierbar"
    );
    assert!(!page.contains("wirken erst ab B2") && !page.contains("Noch nicht umgesetzt"), "keine STUB-Hinweise mehr");
}

#[tokio::test]
async fn the_workspace_offers_review_proposal_and_persona_chat_without_javascript() {
    let b = bees([]).await;
    let (_, page) = b.server.get(&format!("/?work={}&key={}", b.work, b.keys[2])).await;
    assert!(
        page.contains(
            "<form class=\"assist-form\" method=\"post\" action=\"/assistenten/act\" data-widget=\"assist\">"
        ),
        "Prüfen/Vorschlag-Formular: {page}"
    );
    assert!(
        page.contains("value=\"persona.review\">Prüfen</button>") || page.contains("value=\"persona.review\" title="),
        "Knopf Prüfen"
    );
    assert!(page.contains("name=\"method\" value=\"persona.propose\""), "Knopf Vorschlag");
    assert!(!page.contains("folgt mit den Backends (B2)"), "keine gesperrten Knöpfe mehr");
    assert!(page.contains("<option value=\"@lektor\" selected>"), "Vorgabe der Auswahl: der Lektor");
    let chat = page.split("value=\"persona.chat\"").nth(1).expect("Chat-Eingabe an persona.chat");
    let chat = &chat[..chat.find("</form>").unwrap()];
    assert!(
        chat.contains("placeholder=\"@persona fragen …\"") && chat.contains("name=\"message\""),
        "Chat-Eingabe: {chat}"
    );
    assert!(
        page.contains("/static/js/assist.js") || baruch_server::SCRIPTS.iter().any(|(n, _)| *n == "assist.js"),
        "Widget assist übersetzt"
    );
}
