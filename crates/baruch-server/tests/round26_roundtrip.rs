// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 26: Hin- und Rückweg zwischen zwei Instanzen ohne Verlust und wiederholbarer Abgleich. Ausgegeben wird über
//! RPC `work.exchange_export` (R26b), eingelesen über `work.exchange_import` (R26c). Ausgangswerk ist die
//! handgeschriebene, frei erfundene Prüfdatei des Lesers.
//!
//! Seit der Integration (export und import zusammengeführt) laufen alle Tests vollständig. Einen Ausweg für einen
//! Stub gibt es nicht mehr: Antwortet die Ausgabe mit einem Fehler, scheitert der Test hart.

use serde_json::{Value, json};

use crate::common;

const WERK: &str = include_str!("../../baruch-exchange/tests/data/import/werk.baruch.xml");

fn sha(bytes: &[u8]) -> String {
    baruch_exchange::sha256_hex(bytes)
}

async fn import(server: &common::Server, bytes: &[u8], mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(bytes), "sha256": sha(bytes), "mode": mode }),
        )
        .await;
    assert!(answer["error"].is_null(), "Einlesen ({mode}) scheitert: {answer}");
    answer["result"].clone()
}

/// Ausgabe eines Werks als Bytes (`format` "xml" oder "baruch"); jeder Fehler lässt den Test scheitern.
async fn export(server: &common::Server, work: i64, format: &str) -> Vec<u8> {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": format })).await;
    let result = &answer["result"];
    assert!(answer["error"].is_null(), "Ausgabe scheitert (kein Ausweg für einen Stub mehr): {answer}");
    let bytes = match format {
        "xml" => result["xml"].as_str().expect("xml in der Antwort").as_bytes().to_vec(),
        _ => baruch_store::decode_base64(result["data"].as_str().expect("data in der Antwort")).unwrap(),
    };
    assert_eq!(result["sha256"], sha(&bytes), "sha256 der Antwort passt zu den Bytes");
    bytes
}

/// XML ohne `<ausgabe …/>` (Zeit und Instanz der Ausgabe unterscheiden sich naturgemäß).
fn without_output(xml: &[u8]) -> String {
    let text = String::from_utf8(xml.to_vec()).expect("UTF-8");
    let Some(start) = text.find("<ausgabe") else { return text };
    let end = start + text[start..].find("/>").expect("<ausgabe/> geschlossen") + 2;
    format!("{}{}", &text[..start], &text[end..])
}

#[tokio::test]
async fn a_to_b_as_new_and_back_loses_nothing() {
    let a = common::open().await;
    let work_a = import(&a, WERK.as_bytes(), "new").await["work"].as_i64().unwrap();
    let xml_a = export(&a, work_a, "xml").await;
    let zip_a = export(&a, work_a, "baruch").await;

    // B liest die XML-Ausgabe, C den Container: beide geben dasselbe aus wie A (bis auf <ausgabe>).
    let b = common::open().await;
    let work_b = import(&b, &xml_a, "new").await["work"].as_i64().unwrap();
    let xml_b = export(&b, work_b, "xml").await;
    assert_eq!(without_output(&xml_b), without_output(&xml_a), "Rückweg über XML ohne Verlust");
    let c = common::open().await;
    let work_c = import(&c, &zip_a, "new").await["work"].as_i64().unwrap();
    let xml_c = export(&c, work_c, "xml").await;
    assert_eq!(without_output(&xml_c), without_output(&xml_a), "Rückweg über den Container ohne Verlust");

    // Die Ausgabe ist gültig nach dem XSD (xmllint; fehlt es, ehrlich übersprungen).
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.baruch.xml");
    std::fs::write(&file, &xml_a).unwrap();
    let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u8.xsd");
    match std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&file)
        .output()
    {
        Ok(output) => assert!(output.status.success(), "XSD: {}", String::from_utf8_lossy(&output.stderr)),
        Err(e) => eprintln!("ÜBERSPRUNGEN: xmllint nicht gefunden ({e})"),
    }
}

#[tokio::test]
async fn a_change_on_b_arrives_on_a_as_revision_and_request_and_a_second_pass_adds_nothing() {
    let a = common::open().await;
    let work_a = import(&a, WERK.as_bytes(), "new").await["work"].as_i64().unwrap();
    let zip_a = export(&a, work_a, "baruch").await;
    let b = common::open().await;
    let work_b = import(&b, &zip_a, "new").await["work"].as_i64().unwrap();

    // Auf B ändert der Mensch: neue Fassung von „Die Speichen“ und „Das Rad“ wieder mit Nummer.
    let loaded = b.state.store.load_work(work_b).await.unwrap();
    let spokes = loaded.nodes.iter().find(|n| n.title == "Die Speichen").unwrap();
    let wheel = loaded.nodes.iter().find(|n| n.title == "Das Rad").unwrap();
    // Überschrift mit der Ebene des Punkts (Abschnitt = `==`); die Prüfdatei schreibt `===`, der Store verlangt beim
    // Speichern des Menschen die passende Ebene.
    b.state.store.save_human(work_b, spokes.id, "== Die Speichen\n\nVierzehn Speichen.\n", "Autor").await.unwrap();
    b.state.store.outline_set_numbered(work_b, wheel.id, true, "Autor").await.unwrap();

    let zip_b = export(&b, work_b, "baruch").await;
    let current_before: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT r.node_id, r.number FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ? AND r.status = 'current'",
    )
    .bind(work_a)
    .fetch_all(a.state.store.pool())
    .await
    .unwrap();
    let merged = import(&a, &zip_b, "merge").await;
    assert_eq!(merged["added"]["revisions"], 1, "die neue Fassung von B kommt als weitere Fassung: {merged}");
    let requests = a.state.store.outline_requests(work_a, true).await.unwrap();
    assert!(
        requests.iter().any(|r| matches!(r.op, baruch_store::OutlineOp::SetNumbered { numbered: true, .. })),
        "„mit Nummer“ kommt als Antrag: {requests:?}"
    );
    let current_after: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT r.node_id, r.number FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ? AND r.status = 'current'",
    )
    .bind(work_a)
    .fetch_all(a.state.store.pool())
    .await
    .unwrap();
    assert_eq!(current_after, current_before, "„aktuell“ bleibt auf A unverändert");

    let again = import(&a, &zip_b, "merge").await;
    let total: u64 = again["added"].as_object().unwrap().values().map(|v| v.as_u64().unwrap()).sum();
    assert_eq!(total, 0, "ein zweiter Durchgang legt nichts doppelt an: {again}");
    assert!(again["requests"].as_array().unwrap().is_empty(), "keine neuen Anträge");
}

/// Integration R26: Umbenennen auf B legt dort eine Fassung mit der neuen Überschrift an. Auf A kommt diese Fassung
/// mit (nicht aktuell), und nach F6 (b) wird daraus kein Antrag „Umbenennen“, sondern der Hinweis „Umschalten genügt“
/// mit Link; ein auf B angelegter Einfall kommt dazu. Gelöscht oder umgeschaltet wird auf A nichts.
#[tokio::test]
async fn a_rename_on_b_arrives_as_revision_with_a_hint_and_a_new_idea_arrives_too() {
    let a = common::open().await;
    let work_a = import(&a, WERK.as_bytes(), "new").await["work"].as_i64().unwrap();
    let zip_a = export(&a, work_a, "baruch").await;
    let b = common::open().await;
    let work_b = import(&b, &zip_a, "new").await["work"].as_i64().unwrap();

    let loaded = b.state.store.load_work(work_b).await.unwrap();
    let wheel = loaded.nodes.iter().find(|n| n.title == "Das Rad").unwrap();
    let renamed = b.rpc("outline.rename", json!({ "work": work_b, "node": wheel.id, "title": "Das große Rad" })).await;
    assert!(renamed["error"].is_null(), "Umbenennen auf B: {renamed}");
    let idea = b.rpc("idea.create", json!({ "work": work_b, "words": "Nabe", "note": "Die Nabe hält alles." })).await;
    assert!(idea["error"].is_null(), "Einfall auf B: {idea}");
    let zip_b = export(&b, work_b, "baruch").await;

    let ideas_before: i64 = sqlx::query_scalar("SELECT count(*) FROM idea WHERE work_id = ?")
        .bind(work_a)
        .fetch_one(a.state.store.pool())
        .await
        .unwrap();
    let merged = import(&a, &zip_b, "merge").await;
    assert_eq!(merged["added"]["revisions"], 1, "die Fassung mit der neuen Überschrift kommt mit: {merged}");
    assert_eq!(merged["added"]["ideas"], 1, "der Einfall von B kommt dazu: {merged}");
    let hint = merged["deviations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["label"].as_str().is_some_and(|l| l.contains("Das große Rad")))
        .unwrap_or_else(|| panic!("Abweichung zum neuen Titel fehlt: {merged}"));
    assert_eq!(hint["action"], "hinweis", "eine passende Fassung ist da: Umschalten genügt, kein Antrag: {hint}");
    assert!(hint["label"].as_str().unwrap().contains("Umschalten genügt"), "der Hinweis sagt, was zu tun ist: {hint}");
    let requests = a.state.store.outline_requests(work_a, true).await.unwrap();
    assert!(
        !requests.iter().any(|r| matches!(r.op, baruch_store::OutlineOp::Rename { .. })),
        "kein zusätzlicher Antrag „Umbenennen“ (er legte beim Bestätigen eine doppelte Fassung an): {requests:?}"
    );
    let loaded_a = a.state.store.load_work(work_a).await.unwrap();
    assert!(
        loaded_a.nodes.iter().any(|n| n.title == "Das Rad"),
        "auf A heißt der Punkt weiter „Das Rad“ (nichts umgeschaltet)"
    );
    let ideas_after: i64 = sqlx::query_scalar("SELECT count(*) FROM idea WHERE work_id = ?")
        .bind(work_a)
        .fetch_one(a.state.store.pool())
        .await
        .unwrap();
    assert_eq!(ideas_after, ideas_before + 1, "genau ein Einfall mehr, keiner verloren");
}

/// Befund Nachbesserung R26: Ein gelöschtes Werk hinterlässt seine Einfälle als Einfälle ohne Werk (Migration 0011,
/// `ON DELETE SET NULL`) mit derselben Kennung. Die eigene Sicherung zurückzuspielen darf daran nicht scheitern: Das
/// neue Werk bekommt diese Einfälle wieder, statt das Einlesen mit „gibt es hier schon in einem anderen Werk“
/// abzulehnen.
#[tokio::test]
async fn a_deleted_work_comes_back_from_its_own_file_and_gets_its_ideas_back() {
    let a = common::open().await;
    let work = import(&a, WERK.as_bytes(), "new").await["work"].as_i64().unwrap();
    let idea = a.rpc("idea.create", json!({ "work": work, "words": "Nabe", "note": "Die Nabe hält alles." })).await;
    assert!(idea["error"].is_null(), "Einfall anlegen: {idea}");
    let xml = export(&a, work, "xml").await;
    let ideas_before: Vec<String> = sqlx::query_scalar("SELECT uuid FROM idea WHERE work_id = ? ORDER BY uuid")
        .bind(work)
        .fetch_all(a.state.store.pool())
        .await
        .unwrap();
    assert!(!ideas_before.is_empty(), "das Werk hat Einfälle (sonst prüfte der Test den Fall nicht)");

    let title = a.state.store.load_work(work).await.unwrap().title;
    a.state.store.delete_work(work, &title).await.unwrap();
    let orphans: i64 = sqlx::query_scalar("SELECT count(*) FROM idea WHERE work_id IS NULL")
        .fetch_one(a.state.store.pool())
        .await
        .unwrap();
    assert_eq!(orphans as usize, ideas_before.len(), "Vorbedingung: die Einfälle bleiben als Einfälle ohne Werk");

    let back = import(&a, &xml, "new").await;
    let restored = back["work"].as_i64().unwrap();
    assert_eq!(back["added"]["ideas"].as_u64(), Some(ideas_before.len() as u64), "alle Einfälle kommen zurück: {back}");
    let ideas_after: Vec<String> = sqlx::query_scalar("SELECT uuid FROM idea WHERE work_id = ? ORDER BY uuid")
        .bind(restored)
        .fetch_all(a.state.store.pool())
        .await
        .unwrap();
    assert_eq!(ideas_after, ideas_before, "dieselben Kennungen gehören wieder zum Werk");
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM idea").fetch_one(a.state.store.pool()).await.unwrap();
    assert_eq!(total as usize, ideas_before.len(), "wieder zugeordnet, nicht doppelt angelegt");
}

/// Zahl der Paare (Punkt, Text), die in einem Werk mehr als einmal vorkommen.
async fn same_text_pairs(server: &common::Server, work: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT r.node_id FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ? \
         GROUP BY r.node_id, r.body HAVING count(*) > 1)",
    )
    .bind(work)
    .fetch_one(server.state.store.pool())
    .await
    .unwrap()
}

/// Befund Nachbesserung R26: B bearbeitet eine gemeinsame Fassung an Ort und Stelle, dann wird im Wechsel abgeglichen.
/// Beide Texte kommen auf beiden Seiten an (nichts verloren), aber kein Punkt bekommt denselben Text zweimal, und
/// spätestens der dritte Durchgang legt nichts mehr an.
#[tokio::test]
async fn merging_back_and_forth_after_an_in_place_edit_creates_no_duplicate_texts() {
    let a = common::open().await;
    let work_a = import(&a, WERK.as_bytes(), "new").await["work"].as_i64().unwrap();
    let b = common::open().await;
    let work_b = import(&b, &export(&a, work_a, "baruch").await, "new").await["work"].as_i64().unwrap();
    let baseline = same_text_pairs(&a, work_a).await;

    // Eine Fassung auf B an Ort und Stelle ändern (Überschrift bleibt, eine Zeile kommt dazu).
    // `update_revision` nimmt die Stelle (`seq`) des Punkts, nicht seine id.
    let (node, number, old_body, uuid): (i64, i64, String, String) = sqlx::query_as(
        "SELECT n.seq, r.number, r.body, r.uuid FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ? \
         ORDER BY n.seq, r.number LIMIT 1",
    )
    .bind(work_b)
    .fetch_one(b.state.store.pool())
    .await
    .unwrap();
    let new_body = format!("{}\nEin Nachtrag auf B.\n", old_body.trim_end());
    let edit = baruch_store::RevisionEdit { body: Some(&new_body), note: None };
    b.state.store.update_revision(work_b, node as usize, number as u32, &edit, "Autor").await.unwrap().unwrap();

    let mut added = Vec::new();
    for pass in 0..5 {
        let (from, from_work, to) = if pass % 2 == 0 { (&b, work_b, &a) } else { (&a, work_a, &b) };
        let merged = import(to, &export(from, from_work, "baruch").await, "merge").await;
        added.push(merged["added"]["revisions"].as_u64().unwrap());
    }
    assert_eq!(&added[3..], &[0, 0], "nach dem Austausch beider Texte kommt nichts mehr dazu: {added:?}");
    for (server, work, side) in [(&a, work_a, "A"), (&b, work_b, "B")] {
        assert_eq!(same_text_pairs(server, work).await, baseline, "{side}: kein Punkt hat denselben Text doppelt");
        let bodies: Vec<String> = sqlx::query_scalar(
            "SELECT r.body FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ? AND n.uuid = \
             (SELECT n2.uuid FROM revision r2 JOIN node n2 ON n2.id = r2.node_id WHERE r2.uuid = ? LIMIT 1)",
        )
        .bind(work)
        .bind(&uuid)
        .fetch_all(server.state.store.pool())
        .await
        .unwrap();
        // Runde 23: `revision.update` legt einen Schritt im Verlauf an; die Gegenseite spult vor. Der alte Text
        // steht dann im Verlauf der Fortschreibungen statt als eigene Fassung.
        let history: Vec<String> = sqlx::query_scalar(
            "SELECT a.body_before FROM revision_amend a JOIN revision r ON r.id = a.revision_id \
             JOIN node n ON n.id = r.node_id WHERE n.work_id = ?",
        )
        .bind(work)
        .fetch_all(server.state.store.pool())
        .await
        .unwrap();
        assert!(
            bodies.contains(&old_body) || history.contains(&old_body),
            "{side}: der alte Text ist da (als Fassung oder im Verlauf der Fortschreibungen – nichts verloren)"
        );
        assert!(bodies.contains(&new_body), "{side}: der neue Text ist da");
    }
}

/// Befund Nachbesserung R26: Ein Instanzname mit `<`, `>` oder Zeilenumbruch in der Datei darf den Commit im Spiegel
/// nicht verhindern (Git lehnt diese Zeichen im Autor ab); sonst fehlte das Protokoll mit Herkunft.
#[tokio::test]
async fn an_instance_name_with_angle_brackets_and_newline_still_reaches_the_mirror() {
    let evil = WERK.replace(
        r#"instanzname="Prüfrechner""#,
        &format!(r#"instanzname="Prüf &lt;rechner&gt;&#10;zweite Zeile{}""#, "x".repeat(200)),
    );
    assert_ne!(evil, WERK, "Vorbedingung: der Instanzname wurde ersetzt");
    let a = common::open().await;
    let answer = import(&a, evil.as_bytes(), "new").await;
    assert!(answer["mirror"].is_null(), "kein Fehler im Spiegel: {answer}");
    assert!(answer["commit"].is_string(), "das Einlesen ist ein Commit im Spiegel: {answer}");
}
