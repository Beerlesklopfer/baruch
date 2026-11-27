// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23a: leftovers from Round 22 – `revision.update` on an amended revision creates a step in the history
//! (fast-forwarding and exchange stay intact), an outdated state is rejected, the confirmation page passes the save
//! mode on, a rejected save without JavaScript keeps the text, `ui.render` without `work` shows the work from the
//! cookie. Work freely invented.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

use crate::common;

const WEBEREI: &str = "#set document(title: \"Die Weberei am Kanal\", author: \"Tilda Rohn\")\n\n\
= Kettfaden\n\nAm Morgen spannte Tilda die Kette. Jeder Faden lief durch das Geschirr und das Blatt, und erst am \
Mittag stand der Webstuhl bereit; das Schiffchen lag noch unberührt auf der Bank.\n\n\
= Schussfaden\n\nDer Schuss war blau gefärbt. Er kam aus dem Kessel im Hof und trocknete drei Tage unter dem Dach.\n";

fn ok(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none_or(Value::is_null), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn weberei(server: &common::Server) -> (i64, Vec<i64>) {
    let work = server.import(WEBEREI, "weberei.typ").await.work;
    let keys = server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    (work, keys)
}

async fn current(server: &common::Server, work: i64, node: usize) -> (u32, String) {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let n = &loaded.nodes[node];
    let rev = &n.revisions[n.current_index().expect("aktuelle Fassung")];
    (rev.number, rev.body.clone())
}

async fn export_xml(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    ok(&answer)["xml"].as_str().expect("xml in der Antwort").to_owned()
}

async fn import_xml(server: &common::Server, xml: &str, mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": mode }),
        )
        .await;
    ok(&answer).clone()
}

#[tokio::test]
async fn revision_update_on_an_amended_version_writes_a_step_and_exchange_and_fast_forward_still_work() {
    let a = common::open().await;
    let (work, keys) = weberei(&a).await;
    let (rev, body) = current(&a, work, 0).await;

    // Amend in the editor (state 1) …
    let amended = body.replacen("Mittag", "frühen Mittag", 1);
    let saved = a
        .rpc(
            "revision.save",
            json!({ "work": work, "key": keys[0], "body": amended, "based_on": rev, "based_on_amend": 0, "mode": "auto" }),
        )
        .await;
    assert_eq!(ok(&saved)["saved"], "amended", "Testvoraussetzung: der Editor schreibt fort: {saved}");
    assert_eq!(ok(&saved)["amend_seq"], 1, "Stand 1");

    // … then change through the tree: one step in the history, state 2.
    let over_tree = amended.replacen("unberührt", "ganz unberührt", 1);
    let updated = a
        .rpc(
            "revision.update",
            json!({ "work": work, "key": keys[0], "rev": rev, "body": over_tree, "based_on_amend": 1 }),
        )
        .await;
    let updated = ok(&updated);
    assert_eq!(updated["changed"], true, "geändert: {updated}");
    assert_eq!(updated["amend_seq"], 2, "die Antwort nennt den neuen Stand: {updated}");
    let history = a.rpc("revision.amends", json!({ "work": work, "key": keys[0], "rev": rev })).await;
    let steps = ok(&history)["steps"].as_array().unwrap().clone();
    assert_eq!(steps.len(), 2, "Fortschreibung und Ändern über den Baum stehen beide im Verlauf: {history}");
    assert_eq!(steps[1]["kind"], "amend", "der Schritt aus dem Baum hat die Art amend: {history}");
    assert_eq!(steps[1]["body_before"], steps[0]["body_after"], "lückenlose Kette der Schritte: {history}");
    assert_eq!(steps[1]["body_after"], json!(over_tree), "Text nachher ist der neue: {history}");

    // Exchange: export and read in anew in a second instance.
    let out_a = export_xml(&a, work).await;
    assert!(out_a.contains("<fortschreibung "), "die Schritte stehen in der Austauschdatei");
    let b = common::open().await;
    let work_b = import_xml(&b, &out_a, "new").await["work"].as_i64().expect("Werk in B angelegt");
    let (rev_b, body_b) = current(&b, work_b, 0).await;
    assert_eq!(body_b, over_tree, "B hat den Text nach dem Ändern über den Baum");
    let keys_b: Vec<i64> = b.state.store.load_work(work_b).await.unwrap().nodes.iter().map(|n| n.key).collect();
    let history_b = b.rpc("revision.amends", json!({ "work": work_b, "key": keys_b[0], "rev": rev_b })).await;
    assert_eq!(
        ok(&history_b)["steps"].as_array().unwrap().len(),
        2,
        "B hat beide Schritte gelesen (das Lesen bricht nicht an dem Schritt aus dem Baum): {history_b}"
    );
    assert_eq!(ok(&history_b)["amend_seq"], 2, "B steht auf Stand 2");

    // Fast-forwarding: A changes once more through the tree (state 3); B syncs and follows, without
    // „abweichende Fassung“.
    let third = over_tree.replacen("auf der Bank", "auf der Fensterbank", 1);
    let again = a
        .rpc("revision.update", json!({ "work": work, "key": keys[0], "rev": rev, "body": third, "based_on_amend": 2 }))
        .await;
    assert_eq!(ok(&again)["amend_seq"], 3, "A steht auf Stand 3: {again}");
    let merged = import_xml(&b, &export_xml(&a, work).await, "merge").await;
    let report = merged.to_string();
    assert!(
        report.contains("vorgespult von Stand 2 auf Stand 3"),
        "B spult die Fassung vor (die Kette ist lückenlos, auch mit Schritten aus dem Baum): {report}"
    );
    assert_eq!(current(&b, work_b, 0).await, (rev_b, third), "dieselbe Fassung in B trägt den Text von Stand 3");
    let revisions = b.state.store.load_work(work_b).await.unwrap().nodes[0].revisions.len();
    assert_eq!(revisions, 1, "es ist keine „abweichende Fassung“ dazugekommen");
}

#[tokio::test]
async fn revision_update_with_an_outdated_stand_is_refused_over_rpc() {
    let server = common::open().await;
    let (work, keys) = weberei(&server).await;
    let (rev, body) = current(&server, work, 1).await;
    let first = body.replacen("blau", "tiefblau", 1);
    let done = server.rpc("revision.update", json!({ "work": work, "key": keys[1], "rev": rev, "body": first })).await;
    assert_eq!(ok(&done)["amend_seq"], 1, "auf der aktuellen Fassung entsteht Stand 1 (auch ohne Angabe des Stands)");

    // A form that still shows state 0 (second tab).
    let late = body.replacen("drei Tage", "vier Tage", 1);
    let refused = server
        .rpc("revision.update", json!({ "work": work, "key": keys[1], "rev": rev, "body": late, "based_on_amend": 0 }))
        .await;
    assert_eq!(refused["error"]["code"], -32000, "veralteter Stand → -32000 (StoreError::Stale): {refused}");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(message.starts_with("Veraltet: "), "die Meldung beginnt mit „Veraltet: “: {message}");
    assert_eq!(current(&server, work, 1).await.1, first, "der neuere Text bleibt stehen");

    // The form „Fassung ändern“ sends the state along.
    let (_, page) = server.get(&format!("/?work={work}&key={}", keys[1])).await;
    let form = &page[page.find("class=\"rev-edit-form\"").expect("Formular „Fassung ändern“")..];
    let form = &form[..form.find("</form>").unwrap()];
    assert!(
        form.contains("name=\"based_on_amend\" value=\"1\""),
        "das Formular trägt den Stand, auf dem sein Textfeld beruht: {form}"
    );
}

#[tokio::test]
async fn the_confirm_page_passes_the_mode_on_and_the_feedback_names_the_changed_heading() {
    let server = common::open().await;
    let (work, keys) = weberei(&server).await;
    let (rev, body) = current(&server, work, 0).await;
    // Without JavaScript: new heading in the text → confirmation prompt „Teilen und speichern“.
    let split = format!("{body}\n= Leinen\n\nDas Leinen kam aus dem Tal.\n");
    let fields = |extra: &str| {
        format!(
            "method=outline.save&work={work}&key={}&based_on={rev}&based_on_amend=0&mode=auto&body={}{extra}",
            keys[0],
            urlencode(&split)
        )
    };
    let request = Request::post("/gliederung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(fields("")))
        .unwrap();
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Rückfrageseite statt Speichern: {page}");
    assert!(page.contains("Teilen und speichern"), "die Seite fragt nach dem Teilen: {page}");
    assert!(
        page.contains("<input type=\"hidden\" name=\"mode\" value=\"auto\">"),
        "die Rückfrage reicht den Speichermodus weiter (vorher fehlte er): {page}"
    );

    // Confirm like the button of the confirmation page: with `mode` from the hidden field.
    let (status, location) = server.post_form("/gliederung/act", &fields("&split=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert und weitergeleitet");
    let location = location.expect("Ziel der Weiterleitung");
    assert!(
        location.contains("&saved=new-") && location.ends_with("-heading_changed"),
        "die Rückmeldung nennt die neue Fassung und den Grund heading_changed: {location}"
    );
    let (_, shown) = server.get(&location).await;
    assert!(
        shown.contains("Überschrift"),
        "die Arbeitsseite zeigt die Rückmeldung in Worten (Grund: Überschrift geändert): {}",
        &shown[..shown.len().min(400)]
    );

    // „Als neue Fassung speichern“ on the confirmation prompt: the mode new passes likewise.
    let (rev2, body2) = current(&server, work, 1).await;
    let split2 = format!("{body2}\n= Wolle\n\nDie Wolle roch nach Regen.\n");
    let as_new = format!(
        "method=outline.save&work={work}&key={}&based_on={rev2}&based_on_amend=0&mode=auto&as_new=1&body={}",
        keys[1],
        urlencode(&split2)
    );
    let request = Request::post("/gliederung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(as_new))
        .unwrap();
    let (_, _, page) = server.send(request).await;
    assert!(
        page.contains("<input type=\"hidden\" name=\"mode\" value=\"new\">"),
        "mit „Als neue Fassung speichern“ trägt die Rückfrage den Modus new: {page}"
    );
}

#[tokio::test]
async fn a_refused_save_without_javascript_keeps_the_text_on_the_page() {
    let server = common::open().await;
    let (work, keys) = weberei(&server).await;
    let (rev, _) = current(&server, work, 0).await;
    // Inadmissible heading level: a chapter (=) gets a sub-subheading as its first line.
    let bad = "==== Kettfaden\n\nDer lange Text mit <Zeichen> & „Anführungen“, der nicht verloren gehen darf.\n";
    let form = format!(
        "method=outline.save&work={work}&key={}&based_on={rev}&based_on_amend=0&mode=auto&body={}",
        keys[0],
        urlencode(bad)
    );
    let request = Request::post("/gliederung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let (status, headers, page) = server.send(request).await;
    assert!(status.is_client_error(), "abgelehnt bleibt abgelehnt (Status {status})");
    assert!(
        headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/html")),
        "die Antwort ist eine Seite, kein Klartext mehr"
    );
    assert!(page.contains("role=\"alert\">Nicht gespeichert: "), "die Meldung steht auf der Seite: {page}");
    assert!(
        page.contains("Der lange Text mit &lt;Zeichen&gt; &amp; „Anführungen“, der nicht verloren gehen darf."),
        "der eingegebene Text steht (maskiert) im Textfeld zum erneuten Speichern: {page}"
    );
    assert!(
        page.contains("<textarea id=\"outline-retry-body\" name=\"body\"")
            && page.contains("action=\"/gliederung/act\"")
            && page.contains("name=\"mode\" value=\"auto\""),
        "das Formular schickt ihn mit denselben Angaben wieder an /gliederung/act: {page}"
    );
    assert_eq!(current(&server, work, 0).await.0, rev, "gespeichert wurde nichts");
}

#[tokio::test]
async fn ui_render_without_work_draws_the_work_of_the_cookie() {
    let server = common::open().await;
    let (work, _) = weberei(&server).await;
    let default = server.state.default_work().await.unwrap();
    assert_ne!(work, default, "Testvoraussetzung: das Werk aus dem Cookie ist nicht das Vorgabewerk");
    let call = |cookie: Option<String>| {
        let mut request = Request::post("/rpc").header(header::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        request
            .body(Body::from(json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.render", "params": {} }).to_string()))
            .unwrap()
    };
    let (_, _, text) = server.send(call(Some(format!("baruch_work={work}")))).await;
    let answer: Value = serde_json::from_str(&text).unwrap();
    let url = ok(&answer)["url"].as_str().unwrap();
    assert!(
        url.contains(&format!("work={work}")),
        "„Zurück“ auf die Adresse `/` zeichnet das Werk aus dem Cookie, nicht das Vorgabewerk: {url}"
    );
    let (_, _, text) = server.send(call(None)).await;
    let answer: Value = serde_json::from_str(&text).unwrap();
    assert!(
        ok(&answer)["url"].as_str().unwrap().contains(&format!("work={default}")),
        "ohne Cookie bleibt es beim Vorgabewerk: {answer}"
    );
    let (_, _, text) = server.send(call(Some("baruch_work=987654".to_owned()))).await;
    let answer: Value = serde_json::from_str(&text).unwrap();
    assert!(
        ok(&answer)["url"].as_str().unwrap().contains(&format!("work={default}")),
        "ein Cookie mit unbekanntem Werk fällt auf das Vorgabewerk zurück: {answer}"
    );
}

fn urlencode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
