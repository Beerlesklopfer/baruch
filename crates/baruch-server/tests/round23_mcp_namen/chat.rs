// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23 (mcp-namen), Chat und Assistenten: Rückmeldungen „erledigt“ in `persona.chat` und MCP `ask_persona`,
//! eigene Befunde nach dem Kürzen, ein Relay-Weg, ↻ Wiederholen mit Relay-Rahmen und Herkunft, Zitatzeilen,
//! `[B<id>]` nur aus der Nachricht.
//!
//! Modell: `ScriptedModel` (Nachbau, zeichnet jeden Aufruf auf) – kein Eliezer, kein Netz. Texte frei erfunden.

use super::*;

use baruch_server::assist::mock::Step;
use baruch_server::assist::{context, prompts};
use baruch_store::FindingStatus;

fn order(m: &Mill, key: usize, persona: &str, mode: baruch_server::assist::Mode) -> baruch_server::assist::Order {
    baruch_server::assist::Order {
        work: m.work,
        key: m.keys[key],
        persona: persona.into(),
        mode,
        text: String::new(),
        rev: None,
        asker: None,
        post_question: false,
        sources: vec![],
        web: false,
        scope: baruch_server::assist::Scope::Section,
        frame: Default::default(),
        links: Default::default(),
        basis: None,
    }
}

async fn chat(m: &Mill, key: usize, message: &str) -> Value {
    let answer = m.server.rpc("persona.chat", json!({ "work": m.work, "key": m.keys[key], "message": message })).await;
    result(&answer).clone()
}

fn authors(posts: &[baruch_core::Post]) -> Vec<&str> {
    posts.iter().map(|p| p.author.as_str()).collect()
}

// ---- done_proposals, done_notes, all_done ----------------------------------------------------------------------------

#[tokio::test]
async fn ask_persona_reports_done_claims_like_the_rpc_method() {
    let _turn = common::model_queue(module_path!()).await;
    let m = mill([]).await;
    let own = finding(&m, 0, None, 0, "@nathan", "„alten Wehr“ bleibt ohne Bild.").await;
    let foreign = finding(&m, 0, None, 1, "@lektor", "„staut sich“ wirkt schwer.").await;
    let client = local(&m, "Prüfer").await;
    m.model.push(Step::words(&format!(
        "Erledigt: Befund {own} (Stelle „alten Wehr“)\nErledigt: Befund {foreign}\n\n{}",
        prompts::ALL_DONE_SENTENCE
    )));
    let answer = ok(
        &m,
        &client,
        "ask_persona",
        json!({ "work": m.work, "key": m.keys[0], "persona": "@nathan", "mode": "review" }),
    )
    .await;
    assert_eq!(answer["done_proposals"], json!([own]), "der eigene Befund bekommt den Statusvorschlag: {answer}");
    let notes = answer["done_notes"].as_array().expect("done_notes ist eine Liste");
    assert!(
        notes.len() == 1 && notes[0].as_str().unwrap().contains(&format!("[B{foreign}]")),
        "fremder vermerkt: {answer}"
    );
    assert_eq!(answer["all_done"], true, "die Persona sagt, alle ihre Befunde scheinen erledigt: {answer}");
    let pending = m.server.state.store.finding_get(m.work, own).await.unwrap().pending.expect("Rückfrage am Befund");
    assert_eq!(pending.status, FindingStatus::Done, "vorgeschlagen ist „erledigt“; entscheiden kann nur der Mensch");

    // Ohne Rückmeldung fehlen die drei Felder ganz (nur gesetzt, wenn nicht leer bzw. true).
    m.model.push(Step::words("Kritisch: „alten Wehr“ bleibt blass.\n- Schritt: ein Bild ergänzen."));
    let plain = ok(
        &m,
        &client,
        "ask_persona",
        json!({ "work": m.work, "key": m.keys[0], "persona": "@nathan", "mode": "review" }),
    )
    .await;
    for field in ["done_proposals", "done_notes", "all_done"] {
        assert!(plain.get(field).is_none(), "{field} fehlt ohne Rückmeldung: {plain}");
    }
}

#[tokio::test]
async fn persona_chat_reports_done_claims_of_an_rfc() {
    let _turn = common::model_queue(module_path!()).await;
    let m = mill([]).await;
    let own = finding(&m, 0, None, 0, "@nathan", "„alten Wehr“ bleibt ohne Bild.").await;
    let (others, lector) = baruch_server::assist::chat::rfc_personas(&m.server.state).await.unwrap();
    assert!(others.contains(&"@nathan".to_owned()), "@nathan ist aktiv und antwortet beim RFC: {others:?}");
    for handle in &others {
        // Beim RFC prüft jede Persona erneut und sieht ihre eigenen offenen Befunde.
        m.model.push(Step::words(&if handle == "@nathan" {
            format!("Erledigt: Befund {own} (Stelle „alten Wehr“)\n\n{}", prompts::ALL_DONE_SENTENCE)
        } else {
            "Hinweis: „staut sich“ trägt den Anfang.".to_owned()
        }));
    }
    if lector.is_some() {
        m.model.push(Step::words("Was trägt: „alten Wehr“ setzt den Ton."));
    }
    let answer = chat(&m, 0, "RFC Wie steht der Anfang jetzt?").await;
    assert!(answer["errors"].as_array().unwrap().is_empty(), "keine Persona scheitert: {answer}");
    assert_eq!(answer["done_proposals"], json!([own]), "Zahlenliste wie bei persona.ask: {answer}");
    assert_eq!(answer["all_done"], true, "eine Persona meldete alle ihre Befunde als erledigt: {answer}");
    assert!(answer.get("done_notes").is_none(), "nichts übergangen, also kein Feld: {answer}");

    let quiet = mill([Step::words("Was trägt: der Stein.")]).await;
    let plain = chat(&quiet, 1, "@lektor Wie wirkt der Mahlgang?").await;
    for field in ["done_proposals", "done_notes", "all_done"] {
        assert!(plain.get(field).is_none(), "{field} fehlt in einem Chat ohne Rückmeldung: {plain}");
    }
}

// ---- own_findings nach dem Kürzen -------------------------------------------------------------------------------------

#[tokio::test]
async fn own_findings_shed_by_the_budget_are_no_longer_counted_as_in_the_order() {
    let m = mill([]).await;
    let mut ids = Vec::new();
    for n in 0..context::MAX_OWN_FINDINGS {
        let body =
            format!("Befund Nummer {n}: {}", "das Wehr braucht an dieser Stelle ein genaueres Bild. ".repeat(10));
        ids.push(finding(&m, 0, None, u32::try_from(n).unwrap(), "@nathan", &body).await);
    }
    let review = order(&m, 0, "@nathan", baruch_server::assist::Mode::Review);
    let mut persona = context::active_persona(&m.server.state, "@nathan").await.unwrap();
    let full = context::prepare(&m.server.state, &review, &persona).await.unwrap();
    assert_eq!(full.own_findings, ids, "mit reichlich Kontext stehen alle zwanzig eigenen Befunde im Auftrag");

    // Kontext schrittweise verkleinern, bis ein Teil der eigenen Befunde fällt (unabhängig von den festen Größen).
    let mut cut = None;
    while persona.context_length > 600 {
        persona.context_length -= 200;
        match context::prepare(&m.server.state, &review, &persona).await {
            Ok(p) if !p.own_findings.is_empty() && p.own_findings.len() < ids.len() => {
                cut = Some(p);
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let cut = cut.expect("es gibt eine Kontextlänge, bei der nur ein Teil der eigenen Befunde bleibt");
    assert!(cut.truncated, "die Kürzung ist vermerkt");
    let text = all_text(&cut.call);
    let kept = cut.own_findings.len();
    assert_eq!(cut.own_findings, ids[ids.len() - kept..], "es bleiben die jüngsten (gekürzt wird von vorn)");
    for id in &ids {
        let shown = text.contains(&format!("[B{id}] zu"));
        assert_eq!(
            shown,
            cut.own_findings.contains(id),
            "Befund {id}: own_findings nennt genau die Befunde, die im Aufruf stehen (gezeigt: {shown})"
        );
    }
}

// ---- ein Relay-Weg ------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn mcp_and_the_surface_share_one_relay_stage() {
    let _turn = common::model_queue(module_path!()).await;
    // Oberfläche: nur die Anrede am Zeilenanfang ruft; eine Erwähnung mitten im Satz nicht.
    let m = mill([
        Step::words("Das trägt. Vielleicht weiß @exeget mehr dazu.\n@schrift: Stimmt Ps 23,1 hier?"),
        Step::words("Die Stelle stimmt."),
    ])
    .await;
    chat(&m, 1, "@lektor Wie wirkt der Mahlgang?").await;
    let called: Vec<String> = m.model.calls().iter().map(|c| c.persona.clone()).collect();
    assert_eq!(called, ["@lektor", "@schrift"], "Oberfläche: Anrede ruft, bloße Erwähnung nicht");

    // MCP `chat`: dieselbe Stufe, dazu jede Erwähnung – Anrede zuerst, alle gerahmt, eine Stufe.
    let m = mill([
        Step::words("Das trägt. Vielleicht weiß @exeget mehr dazu.\n@schrift: Stimmt Ps 23,1 hier?"),
        Step::words("Die Stelle stimmt. @nathan, was sagst du?"),
        Step::words("Der Zusammenhang trägt."),
    ])
    .await;
    let client = local(&m, "Relay Probe").await;
    let answer = ok(
        &m,
        &client,
        "chat",
        json!({ "work": m.work, "key": m.keys[1], "message": "@lektor Wie wirkt der Mahlgang?" }),
    )
    .await;
    let calls = m.model.calls();
    let called: Vec<&str> = calls.iter().map(|c| c.persona.as_str()).collect();
    assert_eq!(
        called,
        ["@lektor", "@schrift", "@exeget"],
        "MCP: Anrede und Erwähnung, Relay-Antworten rufen niemanden"
    );
    assert_eq!(answer["relayed"], json!(["@schrift", "@exeget"]), "die Antwort nennt, wer über das Relay antwortete");
    let list = posts(&m, 1).await;
    assert_eq!(authors(&list), ["@mcp:relay-probe", "@lektor", "@schrift", "@exeget"], "vier Beiträge im Chat");
    let metas = m.server.state.store.post_meta(m.work, &[list[2].id, list[3].id]).await.unwrap();
    for (post, call) in [(&list[2], &calls[1]), (&list[3], &calls[2])] {
        assert_eq!(metas[&post.id].relay_from.as_deref(), Some("@lektor"), "{}: Relay vom Lektor", post.author);
        assert!(
            all_text(call).contains(context::RELAY_TITLE),
            "{}: die Antwort des Lektors steht gerahmt im Aufruf",
            post.author
        );
        let request = metas[&post.id].request.clone().expect("Auftrag gespeichert");
        assert_eq!(request.web, Some(false), "über MCP nie Websuche – auch im gespeicherten Auftrag");
        assert_eq!(request.asker.as_deref(), Some("@lektor"), "Fragender des Relay-Auftrags ist die anredende Persona");
    }
    let source = include_str!("../../src/mcp/tools.rs");
    assert!(!source.contains("fn relay_section"), "den zweiten Relay-Weg in mcp/tools.rs gibt es nicht mehr");
}

// ---- ↻ Wiederholen --------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn repeating_a_relay_post_sends_the_frame_again_and_old_posts_repeat_as_before() {
    let _turn = common::model_queue(module_path!()).await;
    let m = mill([
        Step::words("Das Mahlen trägt.\n@schrift: Stimmt Ps 23,1 hier?"),
        Step::words("Die Stelle stimmt."),
        Step::words("Die Stelle stimmt weiterhin."),
        Step::words("Ohne Rahmen wiederholt."),
    ])
    .await;
    chat(&m, 1, "@lektor Wie wirkt der Mahlgang?").await;
    let list = posts(&m, 1).await;
    let (lector_post, relay_post) = (list[1].id, list[2].id);

    let repeated = m.server.rpc("post.repeat", json!({ "work": m.work, "id": relay_post })).await;
    let new = result(&repeated)["post"]["id"].as_i64().unwrap();
    let calls = m.model.calls();
    let again = all_text(&calls[2]);
    assert!(again.contains(context::RELAY_TITLE), "die Wiederholung trägt den Relay-Rahmen: {again}");
    assert!(again.contains(context::RELAY_TASK), "und den Zusatz zum Auftrag wie beim ersten Mal");
    assert!(
        again.contains(&format!("@lektor hat dich in Beitrag {lector_post} direkt angesprochen")),
        "der Auftragstext ist derselbe"
    );
    let metas = m.server.state.store.post_meta(m.work, &[relay_post, new]).await.unwrap();
    assert_eq!(metas[&new].repeat_of, Some(relay_post), "Wiederholung von …");
    assert_eq!(metas[&new].relay_from.as_deref(), Some("@lektor"), "auch die Wiederholung ist ein Relay-Beitrag");
    let (first, second) = (metas[&relay_post].request.clone().unwrap(), metas[&new].request.clone().unwrap());
    assert_eq!(second.asker, first.asker, "die Herkunft bleibt");
    assert_eq!((first.web, second.web), (Some(true), Some(true)), "aus der Oberfläche war die Websuche zugelassen");
    assert!(second.relay.is_some(), "der Rahmen ist wieder gespeichert (eine zweite Wiederholung trägt ihn auch)");

    // Ein Beitrag aus der Zeit vor Runde 23: ohne die drei Felder wiederholt er wie bisher.
    sqlx::query("UPDATE post SET request = json_remove(request, '$.relay', '$.asker', '$.web') WHERE id = ?")
        .bind(relay_post)
        .execute(m.server.state.store.pool())
        .await
        .unwrap();
    let old = m.server.rpc("post.repeat", json!({ "work": m.work, "id": relay_post })).await;
    let old_new = result(&old)["post"]["id"].as_i64().unwrap();
    let plain = all_text(&m.model.calls()[3]);
    assert!(!plain.contains(context::RELAY_TITLE), "alte Zeile ohne Felder: kein Rahmen, wie bisher");
    let meta = m.server.state.store.post_meta(m.work, &[old_new]).await.unwrap().remove(&old_new).unwrap();
    assert_eq!(meta.relay_from, None, "ohne Rahmen kein Relay-Beitrag");
    let request = meta.request.unwrap();
    assert_eq!(
        (request.web, request.asker),
        (Some(true), None),
        "wie bisher: Websuche zugelassen, Fragender der Mensch"
    );
}

#[tokio::test]
async fn repeating_an_answer_to_an_mcp_client_keeps_its_origin_and_never_searches_the_web() {
    let _turn = common::model_queue(module_path!()).await;
    let m = mill([Step::words("Das Wehr trägt."), Step::words("Das Wehr trägt weiter.")]).await;
    let client = local(&m, "Fragesteller").await;
    ok(&m, &client, "chat", json!({ "work": m.work, "key": m.keys[0], "message": "@lektor Wie wirkt das Wehr?" }))
        .await;
    let answer = posts(&m, 0).await[1].id;
    let repeated = m.server.rpc("post.repeat", json!({ "work": m.work, "id": answer })).await;
    let new = result(&repeated)["post"]["id"].as_i64().unwrap();
    let request = m.server.state.store.post_meta(m.work, &[new]).await.unwrap().remove(&new).unwrap().request.unwrap();
    assert_eq!(request.web, Some(false), "der Auftrag kam über MCP: auch die Wiederholung sucht nicht im Netz");
    assert_eq!(request.asker.as_deref(), Some("@mcp:fragesteller"), "die Herkunft geht nicht verloren");
    assert!(
        all_text(&m.model.calls()[1]).contains("@mcp:fragesteller"),
        "der wiederholte Auftrag nennt weiter den Client als Fragenden"
    );
}

// ---- Zitatzeilen ----------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn quoted_lines_call_nobody_and_make_no_rfc() {
    let _turn = common::model_queue(module_path!()).await;
    let m = mill([Step::words("Die Stelle stimmt.")]).await;
    let answer = chat(&m, 1, "> @lektor: @alle sollten das prüfen.\n> RFC wäre gut.\n\nDanke, das genügt mir.").await;
    assert_eq!(answer["rfc"], false, "„> @alle“ in einer Zitatzeile ist kein RFC: {answer}");
    assert_eq!(m.model.calls().len(), 0, "„> @lektor“ und „> @alle“ rufen keine Persona");
    assert_eq!(authors(&posts(&m, 1).await), ["Autor"], "nur der Beitrag des Menschen steht im Chat");

    chat(&m, 1, "> @lektor: Schreibe das neu.\n\n@schrift Stimmt Ps 23,1 hier?").await;
    let calls = m.model.calls();
    assert_eq!(calls.len(), 1, "außerhalb des Zitats gilt die Erwähnung weiter");
    assert_eq!(calls[0].persona, "@schrift", "gerufen wird nur, wen der Mensch selbst anspricht");
    assert_eq!(
        m.server.state.store.load_work(m.work).await.unwrap().nodes[1].revisions.len(),
        1,
        "das Schreibverb im Zitat macht keinen Schreibauftrag (kein Vorschlag)"
    );
}

// ---- [B<id>] nur aus der Nachricht ---------------------------------------------------------------------------------------

#[tokio::test]
async fn a_write_order_proposes_status_only_for_findings_named_in_the_message() {
    let _turn = common::model_queue(module_path!()).await;
    let m = mill([]).await;
    let named = finding(&m, 1, None, 0, "@nathan", "„langsam“ bremst den Satz.").await;
    let smuggled = finding(&m, 1, None, 1, "@nathan", "„zählt die Körner“ ist unklar.").await;
    m.model.push(Step::words(&format!(
        "== Der Mahlgang\n\nDer Stein dreht sich und zählt die Körner. // löst [B{smuggled}]\n"
    )));
    let answer = chat(&m, 1, &format!("@lektor Schreibe den Abschnitt straffer und löse [B{named}].")).await;
    assert_eq!(answer["proposals"].as_array().unwrap().len(), 1, "der Schreibauftrag wird ein Vorschlag: {answer}");
    let store = &m.server.state.store;
    let pending =
        store.finding_get(m.work, named).await.unwrap().pending.expect("Statusvorschlag zum genannten Befund");
    assert_eq!(pending.status, FindingStatus::Adopted, "„übernommen in Fassung n“ für den Befund aus der Nachricht");
    assert!(
        store.finding_get(m.work, smuggled).await.unwrap().pending.is_none(),
        "[B<id>] aus dem Modelltext des Vorschlags erzeugt keinen Statusvorschlag"
    );
}
