// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 22 (revisions, ownership fassungen): amending in the editor, new revision with reason, history, undo, setting
//! „Fassungen“, alternatives in the reading flow and MCP `revision_amends` – through RPC, forms without JavaScript,
//! delivered pages and `/mcp`, with a real data folder (SQLite, Git mirror) and a freely invented work. No model, no
//! network.

use crate::common;

use std::net::SocketAddr;
use std::path::Path;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const BOOK: &str = "#set document(title: \"Der Leuchtturm von Halmsand\", author: \"Ina Kessler\")\n\n\
= Die Ankunft\n\nAls die Fähre anlegte, stand der Leuchtturm schon im Abendlicht. Die Wärterin winkte vom Geländer, \
und ein kalter Wind trug den Geruch von Tang über die Mole.\n\n\
= Die Nacht\n\nIn der Nacht drehte sich das Feuer ruhig über dem Wasser, und niemand im Dorf schlief.\n";

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn error(answer: &Value) -> (i64, String) {
    let e = answer.get("error").unwrap_or_else(|| panic!("Fehler erwartet: {answer}"));
    (e["code"].as_i64().unwrap(), e["message"].as_str().unwrap().to_owned())
}

/// Create the work; return value: (work, key of the first section, mirror folder).
async fn setup(server: &common::Server) -> (i64, i64, std::path::PathBuf) {
    let report = server.import(BOOK, "leuchtturm.typ").await;
    let node = server.rpc("node.get", json!({ "work": report.work, "node": 0 })).await;
    let key = result(&node)["key"].as_i64().unwrap();
    (report.work, key, server.mirror_dir_of(&report.slug))
}

async fn body_of(server: &common::Server, work: i64, key: i64) -> String {
    let node = server.rpc("node.get", json!({ "work": work, "key": key })).await;
    let node = result(&node);
    let current = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap();
    current["body"].as_str().unwrap().to_owned()
}

async fn revisions_of(server: &common::Server, work: i64, key: i64) -> usize {
    let node = server.rpc("node.get", json!({ "work": work, "key": key })).await;
    result(&node)["revisions"].as_array().unwrap().len()
}

/// Small addition (one word before „Leuchtturm“), far below 20 %.
fn touched(body: &str, word: &str) -> String {
    let changed = body.replacen("Leuchtturm", &format!("{word} Leuchtturm"), 1);
    assert_ne!(changed, body, "Testtext enthält „Leuchtturm“");
    changed
}

fn subject(dir: &Path) -> Option<String> {
    common::git(dir, &["log", "-1", "--format=%s"]).map(|s| s.trim().to_owned())
}

fn trailers(dir: &Path) -> Option<String> {
    common::git(dir, &["log", "-1", "--format=%(trailers:only,unfold)"])
}

fn commit_count(dir: &Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|n| n.trim().parse().unwrap())
}

async fn save(server: &common::Server, work: i64, key: i64, body: &str, based_on: u32, mode: Option<&str>) -> Value {
    let mut params = json!({ "work": work, "key": key, "body": body, "based_on": based_on });
    if let Some(mode) = mode {
        params["mode"] = json!(mode);
    }
    server.rpc("outline.save", params).await
}

/// Page with resolved escapes in addresses (minijinja escapes `/` as `&#x2f;`, `&` as `&amp;`).
fn readable(page: &str) -> String {
    page.replace("&#x2f;", "/").replace("&amp;", "&")
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

#[tokio::test]
async fn editor_saves_in_auto_mode_amend_the_version_with_one_mirror_commit_each() {
    let server = common::open().await;
    let (work, key, dir) = setup(&server).await;
    let original = body_of(&server, work, key).await;
    let before = commit_count(&dir);

    let first = touched(&original, "alte");
    let answer = save(&server, work, key, &first, 1, Some("auto")).await;
    let r = result(&answer);
    assert_eq!(
        (r["saved"].as_str(), r["amend_seq"].as_u64(), r["rev"].as_u64()),
        (Some("amended"), Some(1), Some(1)),
        "kleine Änderung an der eigenen Fassung 1: fortgeschrieben zu Stand 1: {r}"
    );
    assert!(r.get("reason").is_none(), "fortgeschrieben: kein Grund für eine neue Fassung: {r}");
    assert!(r["commit"].is_string(), "Spiegel-Commit gemeldet: {r}");
    if let Some(n) = before {
        assert_eq!(commit_count(&dir), Some(n + 1), "genau ein Commit für die Fortschreibung");
        assert_eq!(
            subject(&dir).as_deref(),
            Some("1 Die Ankunft: Fassung 1 fortgeschrieben (Stand 1)"),
            "Betreff nennt Fassung und Stand"
        );
        let t = trailers(&dir).unwrap();
        assert!(t.contains("Baruch-Action: amended") && t.contains("Baruch-Amend: 1"), "Trailer: {t}");
    }
    let second = touched(&first, "weiße");
    let r = save(&server, work, key, &second, 1, Some("auto")).await;
    assert_eq!(result(&r)["amend_seq"].as_u64(), Some(2), "zweite Fortschreibung: Stand 2");
    if let Some(n) = before {
        assert_eq!(commit_count(&dir), Some(n + 2), "zweiter Schritt, zweiter Commit");
    }
    assert_eq!(revisions_of(&server, work, key).await, 1, "keine neue Fassung");
    assert_eq!(body_of(&server, work, key).await, second, "Text fortgeschrieben");
    let tree = server.rpc("revision.tree", json!({ "work": work, "node": 0 })).await;
    let infos = result(&tree)["revisions"].as_array().cloned().unwrap_or_default();
    assert!(
        infos.iter().any(|i| i["number"] == 1 && i["amend_seq"] == 2) || tree.to_string().contains("\"amend_seq\":2"),
        "revision.tree trägt amend_seq: {tree}"
    );

    // Without mode: new revision as before (old clients).
    let r = save(&server, work, key, &touched(&second, "hohe"), 1, None).await;
    assert_eq!((result(&r)["saved"].as_str(), result(&r)["rev"].as_u64()), (Some("new"), Some(2)), "fehlt = new");
    assert!(result(&r).get("reason").is_none(), "ohne Modus auto kein Grund");
    // Unknown mode: -32602.
    let (code, message) = error(&save(&server, work, key, "x", 2, Some("immer")).await);
    assert_eq!(code, -32602, "unbekannter Modus ist ein Parameterfehler: {message}");
    assert!(message.contains("„auto“"), "Meldung nennt die erlaubten Werte: {message}");
}

#[tokio::test]
async fn a_new_version_in_auto_mode_names_its_reason_and_revision_save_takes_the_mode_too() {
    let server = common::open().await;
    let (work, key, _dir) = setup(&server).await;
    let original = body_of(&server, work, key).await;

    // revision.save with mode auto amends as well.
    let r = server
        .rpc("revision.save", json!({ "work": work, "key": key, "body": touched(&original, "alte"), "mode": "auto" }))
        .await;
    assert_eq!(
        (result(&r)["saved"].as_str(), result(&r)["amend_seq"].as_u64()),
        (Some("amended"), Some(1)),
        "revision.save mit mode auto schreibt fort"
    );

    // Accepted proposal of a persona: the next save is a new revision (not_own).
    let proposal =
        server.state.store.add_proposal(work, 0, &touched(&original, "fremde"), "@lektor", "mock", "").await.unwrap();
    let r = server.rpc("revision.accept", json!({ "work": work, "key": key, "rev": proposal })).await;
    result(&r);
    let body = body_of(&server, work, key).await;
    let r = save(&server, work, key, &touched(&body, "eigene"), proposal, Some("auto")).await;
    let r = result(&r);
    assert_eq!((r["saved"].as_str(), r["reason"].as_str()), (Some("new"), Some("not_own")), "Grund not_own: {r}");
    assert_eq!(r["reason_label"].as_str(), Some("aktuelle Fassung stammt nicht vom Menschen"), "deutscher Grund");

    // Edited on another revision: not_on_current (also through revision.save).
    let r = server
        .rpc(
            "revision.save",
            json!({ "work": work, "key": key, "body": touched(&original, "kühle"), "based_on": 1, "mode": "auto" }),
        )
        .await;
    assert_eq!(result(&r)["reason"].as_str(), Some("not_on_current"), "Basis ≠ aktuell: {r}");
}

#[tokio::test]
async fn history_and_undo_over_rpc_record_one_commit_and_keep_every_step() {
    let server = common::open().await;
    let (work, key, dir) = setup(&server).await;
    let original = body_of(&server, work, key).await;
    let first = touched(&original, "alte");
    save(&server, work, key, &first, 1, Some("auto")).await;
    save(&server, work, key, &touched(&first, "weiße"), 1, Some("auto")).await;

    let amends = server.rpc("revision.amends", json!({ "work": work, "key": key, "rev": 1 })).await;
    let a = result(&amends);
    assert_eq!(
        (a["key"].as_i64(), a["rev"].as_u64(), a["amend_seq"].as_u64()),
        (Some(key), Some(1), Some(2)),
        "Verlauf von Fassung 1 nach zwei Fortschreibungen: {a}"
    );
    let steps = a["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2, "zwei Schritte: {a}");
    assert_eq!(steps[0]["body_before"].as_str(), Some(original.as_str()), "Text vorher im ersten Schritt");
    assert_eq!(steps[0]["kind"], "amend", "Art amend");
    assert_eq!(steps[0]["author"], "Autor", "Autor ist der Mensch");

    let before = commit_count(&dir);
    let undo = server.rpc("revision.amend_undo", json!({ "work": work, "key": key, "rev": 1, "seq": 1 })).await;
    let u = result(&undo);
    assert_eq!(u["amend_seq"].as_u64(), Some(3), "Rücknahme ist Schritt 3: {u}");
    assert_eq!(body_of(&server, work, key).await, original, "Text wie vor Schritt 1");
    if let Some(n) = before {
        assert_eq!(commit_count(&dir), Some(n + 1), "genau ein Commit für die Rücknahme");
        assert_eq!(
            subject(&dir).as_deref(),
            Some("1 Die Ankunft: Fortschreibung von Fassung 1 zurückgenommen (Stand 3)"),
            "Betreff der Rücknahme"
        );
        let t = trailers(&dir).unwrap();
        assert!(t.contains("Baruch-Action: amend-undone") && t.contains("Baruch-Amend: 3"), "Trailer: {t}");
    }
    let amends = server.rpc("revision.amends", json!({ "work": work, "key": key, "rev": 1 })).await;
    let steps = result(&amends)["steps"].as_array().unwrap().clone();
    assert_eq!(steps.len(), 3, "kein Schritt gelöscht");
    assert_eq!((steps[2]["kind"].as_str(), steps[2]["undo_of"].as_u64()), (Some("undo"), Some(1)), "undo_of = 1");

    let (code, message) =
        error(&server.rpc("revision.amend_undo", json!({ "work": work, "key": key, "rev": 1, "seq": 9 })).await);
    assert_eq!(code, -32000, "unbekannter Schritt: fachlich abgelehnt");
    assert!(message.contains("Schritt"), "deutsche Meldung: {message}");
    let (code, _) = error(&server.rpc("revision.amends", json!({ "work": work, "key": key, "rev": 77 })).await);
    assert_ne!(code, 0, "unbekannte Fassung abgelehnt");

    // Not through /act: there the new methods are unknown.
    for method in ["revision.amend_undo", "revision.policy_set"] {
        let form = format!("method={method}&work={work}&key={key}&rev=1&seq=1&amend=0");
        let (status, _) = server.post_form("/act", &form).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method} geht nicht über /act");
    }
}

#[tokio::test]
async fn the_policy_is_read_set_checked_and_recorded_in_the_mirror() {
    let server = common::open().await;
    let (work, _key, dir) = setup(&server).await;
    let got = server.rpc("revision.policy_get", json!({ "work": work })).await;
    let g = result(&got);
    assert_eq!(
        (g["amend"].as_bool(), g["max_change_percent"].as_u64(), g["new_after_minutes"].as_u64()),
        (Some(true), Some(100), Some(0)),
        "Vorgabe seit 2026-11-27: an, ohne Größen- und Zeitgrenze (100 %, 0 min): {g}"
    );
    assert_eq!(g["defaults"]["max_change_percent"].as_u64(), Some(100), "Vorgaben mitgeliefert");

    let set = server.rpc("revision.policy_set", json!({ "work": work, "max_change_percent": 40 })).await;
    let s = result(&set);
    assert_eq!(s["changed"], json!(["max_change_percent"]), "geändertes Feld: {s}");
    if common::git(&dir, &["status"]).is_some() {
        assert_eq!(subject(&dir).as_deref(), Some("Fassungen: größte Änderung geändert"), "Betreff der Einstellung");
        let t = trailers(&dir).unwrap();
        assert!(t.contains("Baruch-Action: revision-policy-set"), "Trailer: {t}");
        let body = common::git(&dir, &["log", "-1", "--format=%b"]).unwrap();
        assert!(body.contains("größte Änderung: 100 % → 40 %"), "Rumpf vorher → nachher: {body}");
    }
    let unchanged = server.rpc("revision.policy_set", json!({ "work": work, "max_change_percent": 40 })).await;
    assert!(result(&unchanged)["changed"].as_array().unwrap().is_empty(), "nichts geändert");
    assert!(result(&unchanged).get("commit").is_none(), "ohne Änderung kein Commit");
    let (code, message) =
        error(&server.rpc("revision.policy_set", json!({ "work": work, "max_change_percent": 0 })).await);
    assert_eq!(code, -32602, "Grenze verletzt: Parameterfehler");
    assert!(message.contains("1 bis 100"), "Meldung nennt die Grenzen: {message}");
    let (code, _) = error(&server.rpc("revision.policy_set", json!({ "work": work, "new_after_minutes": 2000 })).await);
    assert_eq!(code, -32602, "über 1440 Minuten abgelehnt");
}

#[tokio::test]
async fn without_javascript_the_editor_amends_or_saves_anew_and_reports_it_in_the_head() {
    let server = common::open().await;
    let (work, key, _dir) = setup(&server).await;
    let (status, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite mit Editor lädt");
    assert!(page.contains(r#"<input type="hidden" name="mode" value="auto">"#), "verstecktes mode=auto im Editor");
    assert!(
        page.contains(r#"name="as_new" value="1""#) && page.contains("Als neue Fassung speichern"),
        "zweiter Knopf „Als neue Fassung speichern“ mit as_new=1"
    );
    assert!(page.contains("Speichern (Strg+S) schreibt Fassung 1 fort"), "Hinweis nennt Fortschreiben und Stand");

    let original = body_of(&server, work, key).await;
    let form = format!(
        "method=outline.save&work={work}&key={key}&based_on=1&mode=auto&body={}",
        form_escape(&touched(&original, "alte"))
    );
    let (status, location) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet weiter");
    let location = location.unwrap();
    assert!(location.ends_with("&saved=amended-1"), "Rückmeldung in der Adresse: {location}");
    let (_, page) = server.get(&location).await;
    assert!(page.contains("Gespeichert: fortgeschrieben (Stand 1)."), "Rückmeldung im Editor-Kopf ohne JavaScript");
    let page = readable(&page);
    assert!(
        page.contains(&format!("/werke/{work}/fassungen/verlauf?key={key}&rev=1")) && page.contains("Stand 1</a>"),
        "Kopf zeigt „Stand 1“ mit Link auf den Verlauf"
    );

    let form = format!(
        "method=outline.save&work={work}&key={key}&based_on=1&mode=auto&as_new=1&body={}",
        form_escape(&touched(&original, "neue"))
    );
    let (_, location) = server.post_form("/gliederung/act", &form).await;
    let location = location.unwrap();
    // as_new=1 → mode „new“ (takes precedence over the hidden mode=auto); a reason exists only in mode „auto“ (§4).
    assert!(location.ends_with("&saved=new-2"), "as_new=1 → neue Fassung 2 ohne Grund: {location}");
    let (_, page) = server.get(&location).await;
    assert!(page.contains("Gespeichert: neue Fassung 2."), "Rückmeldung nennt die neue Nummer");

    // New revision in mode „auto“ with reason: heading changed.
    let body = body_of(&server, work, key).await.replacen("= Die Ankunft", "= Die späte Ankunft", 1);
    let form = format!("method=outline.save&work={work}&key={key}&based_on=2&mode=auto&body={}", form_escape(&body));
    let (_, location) = server.post_form("/gliederung/act", &form).await;
    let location = location.unwrap();
    assert!(location.ends_with("&saved=new-3-heading_changed"), "Grund in der Adresse: {location}");
    let (_, page) = server.get(&location).await;
    assert!(
        page.contains("Gespeichert: neue Fassung 3 (Überschrift oder Gliederung geändert)."),
        "Rückmeldung nennt Nummer und Grund"
    );
    let (_, page) = server.get(&format!("/?work={work}&key={key}&saved=kaputt-%3Cb%3E")).await;
    // Round 39a: the widget catalog in the page head carries the widgets' own „Gespeichert: …“ texts.
    let shown = match page.split_once("id=\"baruch-i18n\">") {
        Some((head, rest)) => format!("{head}{}", &rest[rest.find("</script>").unwrap()..]),
        None => page.clone(),
    };
    assert!(!shown.contains("Gespeichert:") && !shown.contains("<b>"), "ungültige Rückmeldung wird übergangen");
}

#[tokio::test]
async fn the_history_page_shows_each_step_and_takes_back_without_javascript() {
    let server = common::open().await;
    let (work, key, _dir) = setup(&server).await;
    let original = body_of(&server, work, key).await;
    save(&server, work, key, &touched(&original, "alte"), 1, Some("auto")).await;
    let url = format!("/werke/{work}/fassungen/verlauf?key={key}&rev=1");
    let (status, page) = server.get(&url).await;
    assert_eq!(status, StatusCode::OK, "Verlauf erreichbar: {page}");
    assert!(page.contains("Verlauf von Fassung 1") && page.contains("Stand 1"), "Überschrift und Stand");
    assert!(page.contains(r#"<ins class="d-add">alte "#), "Wort-Unterschied: neues Wort markiert");
    assert!(page.contains("Bis hier zurück") && page.contains(r#"action="/werke/fassungen/act""#), "Formular da");
    assert!(page.contains(r#"name="seq" value="1""#), "Formular nennt den Schritt");

    let form = format!("method=revision.amend_undo&work={work}&key={key}&rev=1&seq=1");
    let (status, location) = server.post_form("/werke/fassungen/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Rücknahme leitet weiter");
    assert_eq!(location.as_deref(), Some(format!("{url}&ok=1").as_str()), "zurück zum Verlauf mit Hinweis");
    assert_eq!(body_of(&server, work, key).await, original, "Text zurückgenommen");
    let (_, page) = server.get(&format!("{url}&ok=1")).await;
    assert!(page.contains("Zurückgenommen:") && page.contains("Rücknahme"), "Hinweis und neuer Schritt sichtbar");

    // On a revision that is not current: message on the page, no form.
    save(&server, work, key, &touched(&original, "neue"), 1, Some("new")).await;
    let (status, page) = server.post_form("/werke/fassungen/act", &form).await;
    assert_eq!(status, StatusCode::CONFLICT, "abgelehnt mit 409: {page:?}");
    let (_, page) = server.get(&url).await;
    assert!(
        !page.contains(r#"action="/werke/fassungen/act""#),
        "auf der ersetzten Fassung kein Formular „Bis hier zurück“"
    );
    assert!(page.contains("nur auf der aktuellen Fassung des Menschen"), "Hinweis, warum nicht");
}

#[tokio::test]
async fn settings_work_has_the_revisions_block_and_its_form_saves_the_policy() {
    let server = common::open().await;
    let (work, _key, _dir) = setup(&server).await;
    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Einstellungen → Werk lädt");
    assert!(page.contains(r#"id="fassungen""#) && page.contains("<h3>Fassungen</h3>"), "Block „Fassungen“ mit Anker");
    assert!(page.contains(r#"action="/einstellungen/werk/fassungen""#), "Formular ohne JavaScript");
    assert!(
        page.contains(r#"name="max_change_percent""#) && page.contains(r#"value="100""#),
        "Vorgabe 100 % (seit 2026-11-27 keine Grenze)"
    );

    let (status, location) = server
        .post_form("/einstellungen/werk/fassungen", &format!("work={work}&max_change_percent=50&new_after_minutes=0"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular ohne JavaScript leitet nach dem Speichern weiter");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=fassungen#fassungen").as_str()),
        "Rücksprung zum Block"
    );
    let policy = server.state.store.revision_policy(work).await.unwrap();
    assert_eq!(
        (policy.amend, policy.max_change_percent, policy.new_after_minutes),
        (false, 50, 0),
        "fehlendes Kästchen = aus, Werte gespeichert"
    );
    let (_, page) = server.get(&format!("/einstellungen/werk?work={work}&ok=fassungen")).await;
    assert!(page.contains("Die Einstellung „Fassungen“ ist gespeichert."), "Hinweis nach dem Speichern");

    let (_, location) =
        server.post_form("/einstellungen/werk/fassungen", &format!("work={work}&amend=1&max_change_percent=0")).await;
    assert!(location.unwrap().contains("fassungenfehler=1#fassungen"), "Grenze verletzt: Fehler-Rücksprung");
    assert_eq!(server.state.store.revision_policy(work).await.unwrap(), policy, "nichts geändert");
    let (_, page) = server.get(&format!("/einstellungen/werk?work={work}&fassungenfehler=1")).await;
    assert!(page.contains("ließ sich nicht speichern"), "Fehlermeldung im Block");
}

#[tokio::test]
async fn a_section_with_other_versions_shows_them_in_the_reading_flow_and_switches_without_javascript() {
    let server = common::open().await;
    let (work, key, _dir) = setup(&server).await;
    let original = body_of(&server, work, key).await;
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(!page.contains("weitere Fassung"), "ohne weitere Fassungen kein Hinweis");

    save(&server, work, key, &touched(&original, "neue"), 1, Some("new")).await;
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(page.contains("weitere Fassung: 1"), "Hinweis mit Anzahl im Editor-Kopf");
    let readable_page = readable(&page);
    assert!(readable_page.contains(&format!("/?work={work}&key={key}&rev=1\">Fassung 1</a>")), "Link auf ?rev=1");
    assert!(
        page.contains(r#"id="rev-alt-1" method="post" action="/act""#)
            && page.contains(r#"<input type="hidden" name="method" value="revision.set_current">"#),
        "Formular „aktuell machen“ an /act (im Akkordeon „Fassungen“)"
    );
    let ui = server.rpc("ui.render", json!({ "work": work, "key": key })).await;
    let editor = result(&ui)["editor"].as_str().unwrap().to_owned();
    assert!(
        editor.contains(r#"form="rev-alt-1""#)
            && editor.contains("aktuell machen")
            && editor.contains("weitere Fassung: 1"),
        "Editor-Kopf: Hinweis und Knopf „aktuell machen“, der auf das Formular im Akkordeon zeigt"
    );
    assert!(page.contains(r#"data-alternatives="1""#) && page.contains(">+1</span>"), "Abzeichen im Baum");
    let (_, preview) = server.get(&format!("/?work={work}&key={key}&view=preview")).await;
    assert!(preview.contains("weitere Fassung: 1</a>"), "Hinweis auch im Lesefluss der Vorschau");

    // An open proposal does not count as a further revision.
    server.state.store.add_proposal(work, 0, &touched(&original, "fremde"), "@lektor", "mock", "").await.unwrap();
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(page.contains("weitere Fassung: 1"), "Vorschläge zählen nicht mit");

    let form = format!("method=revision.set_current&work={work}&key={key}&rev=1");
    let (status, _) = server.post_form("/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "aktuell machen über /act");
    assert_eq!(body_of(&server, work, key).await, original, "Fassung 1 ist wieder aktuell");
}

#[tokio::test]
async fn proposals_show_the_amend_state_of_their_base() {
    let server = common::open().await;
    let (work, key, _dir) = setup(&server).await;
    let original = body_of(&server, work, key).await;
    let first = touched(&original, "alte");
    save(&server, work, key, &first, 1, Some("auto")).await;
    save(&server, work, key, &touched(&first, "weiße"), 1, Some("auto")).await;
    let body = body_of(&server, work, key).await;
    let proposal =
        server.state.store.add_proposal(work, 0, &touched(&body, "fremde"), "@lektor", "mock", "").await.unwrap();
    let (_, page) = server.get(&format!("/?work={work}&key={key}&rev={proposal}")).await;
    assert!(page.contains("beruht auf Fassung 1, Stand 2"), "Editor-Kopf und Lesefluss nennen den Stand der Basis");
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(page.contains("beruht auf Fassung 1, Stand 2") || page.contains("beruht auf Stand 2"), "Fassungsbaum");
}

/// PM close R22 (review): a proposal on state 0 whose base was amended afterwards is called „veraltet“ – otherwise it
/// would look as if it were based on the current text.
#[tokio::test]
async fn a_proposal_on_an_older_amend_state_is_marked_as_outdated() {
    let server = common::open().await;
    let (work, key, _dir) = setup(&server).await;
    let original = body_of(&server, work, key).await;
    let proposal =
        server.state.store.add_proposal(work, 0, &touched(&original, "fremde"), "@lektor", "mock", "").await.unwrap();
    let (_, page) = server.get(&format!("/?work={work}&key={key}&rev={proposal}")).await;
    assert!(!page.contains("veraltet: Fassung"), "Basis unverändert: kein Hinweis „veraltet“");
    let answer = save(&server, work, key, &touched(&original, "alte"), 1, Some("auto")).await;
    assert_eq!(result(&answer)["saved"].as_str(), Some("amended"), "der Mensch schreibt Fassung 1 fort: {answer}");
    let (_, page) = server.get(&format!("/?work={work}&key={key}&rev={proposal}")).await;
    assert!(
        page.contains("veraltet: Fassung 1 ist inzwischen bei Stand 1"),
        "Vorschlag auf Stand 0 nach Fortschreibung der Basis als veraltet markiert"
    );
}

/// MCP request from the own machine (without token only loopback).
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
async fn mcp_reads_the_amend_history_only() {
    let server = common::open().await;
    let (work, key, _dir) = setup(&server).await;
    let original = body_of(&server, work, key).await;
    save(&server, work, key, &touched(&original, "alte"), 1, Some("auto")).await;
    let tools = mcp(&server, "tools/list", json!({})).await;
    let tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "revision_amends")
        .cloned()
        .expect("revision_amends in der Werkzeugliste");
    let description = tool["description"].as_str().unwrap();
    assert!(!description.contains("STUB"), "Beschreibung ohne STUB-Vermerk: {description}");
    assert!(description.contains("read-only") && description.contains("not instructions"), "{description}");

    let answer = mcp(
        &server,
        "tools/call",
        json!({ "name": "revision_amends", "arguments": { "work": work, "key": key, "rev": 1 } }),
    )
    .await;
    let text = answer["result"]["content"][0]["text"].as_str().unwrap_or_default().to_owned();
    assert!(answer["result"]["isError"] != json!(true), "kein Werkzeugfehler: {answer}");
    let data: Value = serde_json::from_str(&text).unwrap_or_else(|_| answer["result"]["structuredContent"].clone());
    assert_eq!(data["amend_seq"].as_u64(), Some(1), "Stand 1: {data}");
    assert_eq!(data["steps"][0]["body_before"].as_str(), Some(original.as_str()), "Text vorher geliefert");
    // Through MCP there is no undoing.
    assert!(
        !tools.to_string().contains("amend_undo") && !tools.to_string().contains("policy_set"),
        "kein schreibendes Werkzeug für Fortschreibungen"
    );
}

#[tokio::test]
async fn a_stale_editor_tab_saves_a_new_version_instead_of_overwriting_an_amend_without_javascript_and_over_rpc() {
    // Finding (follow-up fix R22): two tabs open revision 1 (state 0). A saves (state 1), then B on its old text. The
    // form carries the loaded state (`based_on_amend`); B becomes a new revision with reason
    // „nicht auf der aktuellen Fassung bearbeitet“, and the text of A is preserved as revision 1.
    let server = common::open().await;
    let (work, key, _dir) = setup(&server).await;
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(
        page.contains(r#"<input type="hidden" name="based_on_amend" value="0">"#),
        "Editor schickt den geladenen Stand mit (verstecktes Feld)"
    );
    let original = body_of(&server, work, key).await;
    let tab_a = touched(&original, "weiße");
    let tab_b = touched(&original, "rote");
    let form = |body: &str| {
        format!(
            "method=outline.save&work={work}&key={key}&based_on=1&based_on_amend=0&mode=auto&body={}",
            form_escape(body)
        )
    };
    let (_, location) = server.post_form("/gliederung/act", &form(&tab_a)).await;
    assert!(location.unwrap().ends_with("&saved=amended-1"), "A schreibt fort");
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(
        page.contains(r#"<input type="hidden" name="based_on_amend" value="1">"#),
        "nach dem Fortschreiben trägt der Editor Stand 1"
    );
    let (_, location) = server.post_form("/gliederung/act", &form(&tab_b)).await;
    let location = location.unwrap();
    assert!(location.ends_with("&saved=new-2-not_on_current"), "B wird neue Fassung 2 mit Grund: {location}");
    assert_eq!(revisions_of(&server, work, key).await, 2, "beide Texte bleiben als Fassungen");
    let node = server.rpc("node.get", json!({ "work": work, "key": key })).await;
    let first = result(&node)["revisions"].as_array().unwrap().iter().find(|r| r["number"] == 1).cloned().unwrap();
    assert_eq!(first["body"].as_str(), Some(tab_a.as_str()), "der Text von A ist nicht verschwunden");

    // Through RPC (revision.save) likewise: outdated state → new revision.
    let answer = server
        .rpc(
            "revision.save",
            json!({ "work": work, "key": key, "body": touched(&tab_b, "kleine"), "based_on": 2, "based_on_amend": 5, "mode": "auto" }),
        )
        .await;
    let answer = result(&answer);
    assert_eq!(
        (answer["saved"].as_str(), answer["reason"].as_str()),
        (Some("new"), Some("not_on_current")),
        "{answer}"
    );
}
