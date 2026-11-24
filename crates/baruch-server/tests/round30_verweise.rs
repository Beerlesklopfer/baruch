// SPDX-License-Identifier: AGPL-3.0-or-later
//! Querverweise und Sprung-Infrastruktur (Runde 30a): Sprungmarken `<kN>` in Export und Spiegel, `xref.*`,
//! Gliederungsnennungen (`outline_mentions`), mögliche Verweise und Umwandeln durch den Menschen, kaputte Verweise in
//! der Druckprüfung, Seite `/verweise` ohne JavaScript, Prüfung von Sprungmarken (`check_marker`) und die reinen
//! Funktionen von `jump.ts`/`xref.ts` unter Node. Alle Werke und Texte frei erfunden; kein Modell, kein Netz.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::xref::{JumpTarget, MarkerCheck, check_marker, outline_mentions};
use baruch_typst::xref::{find_xrefs, xref_snippet};
use serde_json::{Value, json};

/// Ein Werk mit Vorwort, Einleitung, drei Kapiteln (drittes mit Abschnitt) und zwei gleichnamigen Abschnitten
/// „Rückblick“ (nicht eindeutig).
const BRIDGE: &str = "#set heading(numbering: \"1.\")\n\
#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>\n\nKurz vorab.\n\n\
= Einleitung\n\nHier beginnt das Brückenbuch.\n\n== Rückblick\n\nErster Blick zurück.\n\n\
= Der Leuchtturm\n\nWie die Einleitung sagt, steht der Turm. Im dritten Kapitel mehr dazu, siehe auch Kapitel III \
und das Vorwort. Ein „Wachturm“ kommt später; der Rückblick bleibt offen.\n\n\
= Der Wachturm\n\nAuf dem Wachturm.\n\n== Rückblick\n\nZweiter Blick zurück.\n\n\
== Am Ufer\n\nAbschnitt 3.2 erwähnt sich nicht selbst.\n";

struct Bridge {
    server: common::Server,
    work: i64,
    slug: String,
}

async fn bridge() -> Bridge {
    let server = common::open().await;
    let report = server.import(BRIDGE, "bruecke.typ").await;
    Bridge { server, work: report.work, slug: report.slug }
}

impl Bridge {
    async fn key(&self, title: &str) -> i64 {
        let work = self.server.state.store.load_work(self.work).await.unwrap();
        work.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("{title}")).key
    }

    async fn ok(&self, method: &str, params: Value) -> Value {
        let mut params = params;
        params["work"] = json!(self.work);
        let answer = self.server.rpc(method, params).await;
        assert!(answer["error"].is_null(), "{method}: {answer}");
        answer["result"].clone()
    }

    async fn body(&self, key: i64) -> String {
        let work = self.server.state.store.load_work(self.work).await.unwrap();
        let node = work.nodes.iter().find(|n| n.key == key).unwrap();
        node.revision(node.current_number().unwrap()).unwrap().body.clone()
    }

    fn subjects(&self) -> Vec<String> {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "--format=%s"])
            .map(|log| log.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }
}

#[tokio::test]
async fn export_and_mirror_carry_key_labels_while_the_database_does_not() {
    let b = bridge().await;
    let intro = b.key("Einleitung").await;
    let preface = b.key("Vorwort").await;
    let export = b.ok("work.export", json!({})).await["typst"].as_str().unwrap().to_owned();
    assert!(export.contains(&format!("= Einleitung <k{intro}>\n")), "Überschrift mit Sprungmarke: {export}");
    assert!(
        export.contains(&format!("<baruch-unnumbered>\n#metadata(\"baruch-key\") <k{preface}>\n")),
        "ohne Nummer: Ankerzeile darunter: {export}"
    );
    assert!(!b.body(intro).await.contains("<k"), "die Datenbank bleibt ohne Sprungmarke");
    let main = std::fs::read_dir(b.server.mirror_dir_of(&b.slug).join("abschnitte"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .find(|text| text.contains("= Einleitung"))
        .unwrap();
    assert!(main.contains(&format!("= Einleitung <k{intro}>")), "Spiegel trägt die Sprungmarke: {main}");
}

#[tokio::test]
async fn targets_and_resolve_find_outline_points() {
    let b = bridge().await;
    let intro = b.key("Einleitung").await;
    let targets = b.ok("xref.targets", json!({ "q": "einl" })).await;
    let list = targets["targets"].as_array().unwrap();
    assert_eq!(list.len(), 1, "Suche im Baum: {targets}");
    assert_eq!(list[0]["key"], json!(intro), "Schlüssel");
    assert_eq!(list[0]["label"], format!("k{intro}"), "Sprungmarke");
    assert_eq!(list[0]["level"], 1, "Ebene Kapitel");
    assert_eq!(list[0]["snippet"], xref_snippet(intro, "Einleitung", false), "fertiger Verweis");
    let all = b.ok("xref.targets", json!({ "limit": 3 })).await;
    assert_eq!(all["targets"].as_array().unwrap().len(), 3, "limit");
    let resolved = b.ok("xref.resolve", json!({ "label": format!("<k{intro}>") })).await;
    assert_eq!(resolved["title"], "Einleitung", "Label aufgelöst: {resolved}");
    assert_eq!(resolved["url"], format!("/?work={}&key={intro}", b.work), "Adresse");
    let bad = b.server.rpc("xref.resolve", json!({ "work": b.work, "label": "k999999" })).await;
    assert_eq!(bad["error"]["code"], -32602, "unbekannte Sprungmarke: {bad}");
}

#[tokio::test]
async fn outline_mentions_are_recognised_only_when_unique() {
    let b = bridge().await;
    let work = b.server.state.store.load_work(b.work).await.unwrap();
    let key = |t: &str| work.nodes.iter().find(|n| n.title == t).unwrap().key;
    let text = "Wie die Einleitung sagt, steht der Turm. Im dritten Kapitel mehr dazu, siehe auch Kapitel III und das \
                Vorwort. Ein „Wachturm“ kommt später; der Rückblick bleibt offen. Abschnitt 3.2 auch.";
    let found: Vec<(String, i64)> = outline_mentions(&work, text).into_iter().map(|m| (m.text, m.key)).collect();
    assert_eq!(
        found,
        vec![
            ("die Einleitung".to_owned(), key("Einleitung")),
            ("dritten Kapitel".to_owned(), key("Der Wachturm")),
            ("Kapitel III".to_owned(), key("Der Wachturm")),
            ("das Vorwort".to_owned(), key("Vorwort")),
            ("Abschnitt 3.2".to_owned(), key("Am Ufer")),
        ],
        "Einleitung mit Artikel, drittes Kapitel (Zählung ohne das Vorwort ohne Nummer), römisch, Vorwort, Nummer; \
         „Wachturm“ ist nur Teil eines Titels („Der Wachturm“), „Rückblick“ gibt es zweimal (nicht eindeutig)"
    );
}

#[tokio::test]
async fn candidates_are_converted_only_after_confirmation_with_one_commit() {
    let b = bridge().await;
    let tower = b.key("Der Leuchtturm").await;
    let candidates = b.ok("xref.candidates", json!({ "key": tower })).await;
    let list = candidates["candidates"].as_array().unwrap().clone();
    let texts: Vec<&str> = list.iter().map(|c| c["text"].as_str().unwrap()).collect();
    assert!(texts.contains(&"die Einleitung"), "„die Einleitung“ ist ein möglicher Verweis: {candidates}");
    let first = &list[0];
    assert_eq!(
        first["replacement"],
        xref_snippet(first["target_key"].as_i64().unwrap(), "die Einleitung", false),
        "Ersatz"
    );
    let whole = b.ok("xref.candidates", json!({})).await;
    assert!(whole["candidates"].as_array().unwrap().len() >= list.len(), "ohne key: das ganze Werk");
    let unconfirmed =
        b.server.rpc("xref.convert", json!({ "work": b.work, "key": tower, "rev": 1, "picks": [0] })).await;
    assert_eq!(unconfirmed["error"]["code"], -32602, "ohne confirm nichts: {unconfirmed}");
    assert_eq!(b.body(tower).await.matches("#link").count(), 0, "noch kein Verweis");
    let before = b.subjects();
    let done = b.ok("xref.convert", json!({ "key": tower, "rev": 1, "picks": [0, 1], "confirm": true })).await;
    assert_eq!(done["converted"], 2, "zwei umgewandelt: {done}");
    assert_eq!(done["rev"], 2, "neue Fassung 2");
    let body = b.body(tower).await;
    let uses = find_xrefs(&body);
    assert_eq!(uses.len(), 2, "zwei Verweise im Text: {body}");
    assert_eq!(uses[0].text, "die Einleitung", "Linktext bleibt der Text");
    let after = b.subjects();
    assert_eq!(after.len(), before.len() + 1, "genau ein Commit");
    let work = b.server.state.store.load_work(b.work).await.unwrap();
    let node = work.nodes.iter().find(|n| n.key == tower).unwrap();
    assert!(matches!(node.revision(2).unwrap().author, baruch_core::Author::Human { .. }), "Fassung vom Menschen");
    let again = b.ok("xref.candidates", json!({ "key": tower })).await;
    assert!(
        !again["candidates"].as_array().unwrap().iter().any(|c| c["text"] == "die Einleitung"),
        "ein vorhandener Verweis ist kein möglicher mehr"
    );
}

#[tokio::test]
async fn broken_references_show_in_the_typesetting_check_and_the_typesetting_still_works() {
    let b = bridge().await;
    let tower = b.key("Der Leuchtturm").await;
    let intro = b.key("Einleitung").await;
    let node = b.server.state.store.node_seq(b.work, tower).await.unwrap();
    let body = format!(
        "= Der Leuchtturm\n\nSiehe {} und {}.\n",
        xref_snippet(intro, "die Einleitung", true),
        xref_snippet(999_999, "das Nirgendwo", false)
    );
    b.server.state.store.save_human(b.work, node, &body, "Ida").await.unwrap();
    let broken = b.ok("xref.broken", json!({})).await;
    let list = broken["broken"].as_array().unwrap();
    assert_eq!(list.len(), 1, "genau der Verweis ins Nirgendwo ist kaputt: {broken}");
    assert_eq!(list[0]["label"], "k999999", "Sprungmarke");
    assert_eq!(list[0]["text"], "das Nirgendwo", "Linktext");
    let (status, html) = b.server.get(&format!("/werke/{}/satz", b.work)).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite lädt");
    assert!(
        html.contains("typeset-xref") && html.contains("Kaputter Querverweis k999999"),
        "Druckprüfung nennt ihn: {html}"
    );
    assert!(!html.contains("does not exist"), "der Satz scheitert nicht am kaputten Verweis (im Satz nur Text)");
    let prepared = baruch_server::typeset::prepare(&b.server.state, b.work).await.unwrap();
    assert!(
        prepared.source.main.contains(&format!("#link(<k{intro}>)[die Einleitung, S.~")),
        "gültiger Verweis bleibt"
    );
    assert!(!prepared.source.main.contains("<k999999>"), "kaputter Verweis im Satz nur als Text");
    assert!(prepared.source.main.contains("und das Nirgendwo."), "sein Linktext bleibt");
    let (_, page) = b.server.get(&format!("/verweise?work={}", b.work)).await;
    assert!(page.contains("data-xref-broken") && page.contains("k999999"), "auch auf /verweise: {page}");
}

#[tokio::test]
async fn the_xref_page_converts_without_javascript_and_refuses_foreign_origins() {
    let b = bridge().await;
    let tower = b.key("Der Leuchtturm").await;
    let (status, html) = b.server.get(&format!("/verweise?work={}&key={tower}", b.work)).await;
    assert_eq!(status, StatusCode::OK, "Seite lädt");
    assert!(html.contains("„die Einleitung“"), "möglicher Verweis gelistet: {html}");
    assert!(html.contains("name=\"picks\" value=\"0\" checked"), "Häkchen je Verweis");
    assert!(html.contains("value=\"xref.convert\""), "Formular ohne JavaScript");
    let (status, location) = b
        .server
        .post_form(
            "/verweise/act",
            &format!("method=xref.convert&work={}&key={tower}&rev=1&picks=0&scope={tower}", b.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303");
    assert!(location.unwrap().contains("fehler="), "ohne Häkchen „Ja, umwandeln“: Hinweis, nichts geändert");
    let (status, location) = b
        .server
        .post_form(
            "/verweise/act",
            &format!("method=xref.convert&work={}&key={tower}&rev=1&picks=0&confirm=1&scope={tower}", b.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303 nach dem Umwandeln");
    assert_eq!(location.unwrap(), format!("/verweise?work={}&key={tower}&ok=1", b.work), "zurück mit Ergebnis");
    assert_eq!(find_xrefs(&b.body(tower).await).len(), 1, "ein Verweis eingesetzt");
    let request = Request::post("/verweise/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("origin", "https://fremd.example")
        .body(Body::from(format!("method=xref.convert&work={}&key={tower}&rev=2&picks=0&confirm=1", b.work)))
        .unwrap();
    let (status, _, _) = b.server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Herkunft: 403");
}

#[tokio::test]
async fn check_marker_checks_key_and_quote() {
    let b = bridge().await;
    let work = b.server.state.store.load_work(b.work).await.unwrap();
    let intro = b.key("Einleitung").await;
    match check_marker(b.work, &work, intro, None, None) {
        MarkerCheck::Linked { target, label } => {
            assert_eq!(label, "1. Einleitung", "Beschriftung „Nummer Titel“");
            assert_eq!(target, JumpTarget { work: b.work, key: intro, rev: None, find: None, at: None }, "Ziel");
        }
        other => panic!("Schlüssel geprüft: {other:?}"),
    }
    match check_marker(b.work, &work, intro, Some("„beginnt   das Brückenbuch“"), None) {
        MarkerCheck::Linked { target, label } => {
            assert_eq!(label, "Zitat", "Zitat gefunden (Leerraum vereinheitlicht)");
            assert!(target.find.is_some(), "Sprung markiert das Zitat");
        }
        other => panic!("Zitat geprüft: {other:?}"),
    }
    assert_eq!(
        check_marker(b.work, &work, intro, Some("erfundenes Zitat"), None),
        MarkerCheck::QuoteMissing { key: intro, quote: "erfundenes Zitat".into() },
        "fehlendes Zitat"
    );
    assert_eq!(
        check_marker(b.work, &work, 999_999, None, None),
        MarkerCheck::UnknownKey { key: 999_999 },
        "erfundener Schlüssel"
    );
}

#[tokio::test]
async fn importing_the_export_as_a_new_work_remaps_the_references() {
    let b = bridge().await;
    let tower = b.key("Der Leuchtturm").await;
    let intro = b.key("Einleitung").await;
    let node = b.server.state.store.node_seq(b.work, tower).await.unwrap();
    let body = format!("= Der Leuchtturm\n\nSiehe {}.\n", xref_snippet(intro, "die Einleitung", false));
    b.server.state.store.save_human(b.work, node, &body, "Ida").await.unwrap();
    let export =
        b.ok("work.export", json!({})).await["typst"].as_str().unwrap().replace("Brückenbuch", "Brückenbuch zwei");
    let report = b.server.import(&export, "bruecke-zwei.typ").await;
    let copy = b.server.state.store.load_work(report.work).await.unwrap();
    let new_intro = copy.nodes.iter().find(|n| n.title == "Einleitung").unwrap().key;
    let new_tower = copy.nodes.iter().find(|n| n.title == "Der Leuchtturm").unwrap();
    let text = &new_tower.revisions[0].body;
    assert_ne!(new_intro, intro, "neues Werk, neue Schlüssel");
    assert_eq!(find_xrefs(text)[0].key, new_intro, "der Verweis zeigt auf die neue Einleitung: {text}");
    assert!(!text.contains(&format!("<k{}>", new_tower.key)), "die eigene Sprungmarke ist beim Einlesen entfernt");
}

// ---- reine Funktionen unter Node ------------------------------------------------------------------------------

/// Alle ausgelieferten Module in einen Ordner (ES-Module) und `expr` mit `import * as m from './<module>'` auswerten.
fn node(module: &str, expr: &str) -> Value {
    let dir = tempfile::tempdir().unwrap();
    for (name, source) in baruch_server::SCRIPTS {
        std::fs::write(dir.path().join(name), source).unwrap();
    }
    std::fs::write(dir.path().join("package.json"), "{\"type\": \"module\"}").unwrap();
    std::fs::write(
        dir.path().join("probe.js"),
        format!("import * as m from './{module}';\nconsole.log(JSON.stringify({expr}));\n"),
    )
    .unwrap();
    let out = match std::process::Command::new("node").arg("probe.js").current_dir(dir.path()).output() {
        Ok(out) => out,
        Err(_) => {
            eprintln!("ÜBERSPRUNGEN: node fehlt");
            return Value::Null;
        }
    };
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn the_jump_history_goes_back_and_forward() {
    let result = node(
        "jump.js",
        "(() => { let s = m.emptyState(); s = m.pushJump(s, {url: '/a'}); s = m.pushJump(s, {url: '/b', origin: 'chat'}); \
          const back = m.popJump(s, '/c'); const back2 = m.popJump(back.state, '/b'); const fwd = m.forwardJump(back2.state, '/a'); \
          const many = Array.from({length: 60}).reduce((st, _, i) => m.pushJump(st, {url: '/' + i}), m.emptyState()); \
          return [back.entry, back2.entry, fwd.entry, m.popJump(m.emptyState(), '/x'), many.back.length, \
                  m.pushJump(back.state, {url: '/z'}).forward.length]; })()",
    );
    if result.is_null() {
        return;
    }
    assert_eq!(
        result,
        json!([{ "url": "/b", "origin": "chat" }, { "url": "/a" }, { "url": "/b" }, null, 50, 0]),
        "Zurück zur letzten Ausgangsstelle (mit Herkunft Chat), noch einmal zurück, Vorwärts wiederholt, leerer Verlauf → \
         null, höchstens 50, ein neuer Sprung löscht „Vorwärts“"
    );
}

#[test]
fn find_and_at_are_read_and_located_in_utf16() {
    let result = node(
        "jump.js",
        "[m.findOf('/?work=2&key=5&find=Licht%20und%20Wasser&at=17'), m.findOf('/?work=2&key=5'), \
          m.byteToUtf16('aä😀b', 1), m.byteToUtf16('aä😀b', 3), m.byteToUtf16('aä😀b', 7), \
          m.locate('Licht  und\\nWasser … Licht und Wasser', 'Licht und Wasser', 20), m.locate('abc', 'x', 0)]",
    );
    if result.is_null() {
        return;
    }
    assert_eq!(
        result,
        json!([{ "text": "Licht und Wasser", "at": 17 }, null, 1, 2, 4, [20, 36], null]),
        "find/at aus der Adresse, Bytes → UTF-16 (ä zwei Bytes, Emoji vier Bytes/zwei Einheiten), die Fundstelle \
         nahe `at` (Leerraum beliebig), nicht gefunden → null"
    );
}

#[test]
fn the_xref_dialog_builds_the_snippet_and_finds_keys_for_ctrl_click() {
    let result = node(
        "xref.js",
        "[m.snippet(5, m.escapeText('die *Einleitung* #1'), false), m.snippet(5, 'x', true), \
          m.keyAt('Siehe #link(<k12>)[die [Einleitung]] und <k7>.', 15), m.keyAt('Siehe #link(<k12>)[die Einleitung] und <k7>.', 40), \
          m.keyAt('kein Verweis', 3)]",
    );
    if result.is_null() {
        return;
    }
    assert_eq!(
        result,
        json!([
            "#link(<k5>)[die \\*Einleitung\\* \\#1]",
            "#link(<k5>)[x, S.~#context counter(page).at(<k5>).first()]",
            12,
            7,
            null
        ]),
        "Form aus §3.3 mit maskiertem Linktext; Strg+Klick im Quelltext findet den Schlüssel im Verweis und an <kN>"
    );
}
