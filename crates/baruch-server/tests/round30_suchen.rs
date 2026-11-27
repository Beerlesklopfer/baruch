// SPDX-License-Identifier: AGPL-3.0-or-later
//! Search and replace in the work (Round 30a): `search.find`/`preview`/`replace`/`undo`, page `/suche` without
//! JavaScript, MCP `text_search` (read-only) and the key bindings (`keyAction` from `search_keys.ts` under Node). All
//! works and texts freely invented; no model, no network.

use crate::common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::search::{SearchQuery, SearchRange};
use serde_json::{Value, json};

/// Work with two parts, two chapters each and sections below them (`sections` per chapter), „Leuchtturm“ everywhere; in
/// the first section a Bible quotation with „Leuchtturm“ in quotation marks, a footnote, Typst code and an escaped
/// `\#`.
fn coast(sections: usize) -> String {
    let mut out = String::from("#set heading(numbering: \"1.1\")\n");
    let mut n = 0;
    for part in ["Vom Meer", "Vom Land"] {
        out.push_str(&format!("#heading(level: 1, numbering: none)[{part}] <baruch-part>\n\n"));
        for chapter in 0..2 {
            out.push_str(&format!("= Kapitel {part} {chapter}\n\nDer Leuchtturm im Kapitel.\n\n"));
            for _ in 0..sections {
                n += 1;
                if n == 1 {
                    out.push_str(
                        "== Hafen\n\nDer Leuchtturm blinkt. Wie geschrieben steht: „Ein Leuchtturm ist mein Licht“ \
                         (Ps 119,105).#footnote[Der Leuchtturm in der Fußnote.] Sprache C\\# und `Leuchtturm` roh.\n\
                         #let leuchtturm = 1\n\n",
                    );
                } else {
                    out.push_str(&format!("== Abschnitt {n}\n\nDer Leuchtturm {n} steht am Ufer.\n\n"));
                }
            }
        }
    }
    out
}

struct Coast {
    server: common::Server,
    work: i64,
    slug: String,
}

async fn open(sections: usize) -> Coast {
    let server = common::open().await;
    let report = server.import(&coast(sections), "kueste.typ").await;
    Coast { server, work: report.work, slug: report.slug }
}

impl Coast {
    async fn key_of(&self, title: &str) -> i64 {
        let work = self.server.state.store.load_work(self.work).await.unwrap();
        work.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("{title} fehlt")).key
    }

    async fn find(&self, params: Value) -> Value {
        let mut params = params;
        params["work"] = json!(self.work);
        let answer = self.server.rpc("search.find", params).await;
        assert!(answer["error"].is_null(), "search.find: {answer}");
        answer["result"].clone()
    }

    async fn body_of(&self, key: i64) -> String {
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

fn matched(result: &Value) -> Vec<String> {
    result["hits"].as_array().unwrap().iter().map(|h| h["matched"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test]
async fn find_honours_case_whole_word_and_regex() {
    let coast = open(2).await;
    let all = coast.find(json!({ "q": "leuchtturm", "range": "work" })).await;
    let insensitive = all["total"].as_u64().unwrap();
    assert!(insensitive > 8, "ohne Groß/klein werden alle Leuchttürme gefunden: {insensitive}");
    let exact = coast.find(json!({ "q": "leuchtturm", "range": "work", "case": true })).await;
    assert_eq!(
        exact["total"], 0,
        "mit Groß/klein findet „leuchtturm“ nur kleingeschriebene (`#let leuchtturm` ist Code)"
    );
    let word = coast.find(json!({ "q": "turm", "range": "work", "word": true })).await;
    assert_eq!(word["total"], 0, "„turm“ als ganzes Wort steht nirgends (nur in Leuchtturm)");
    let part = coast.find(json!({ "q": "turm", "range": "work" })).await;
    assert_eq!(part["total"], all["total"], "ohne ganzes Wort steckt „turm“ in jedem Leuchtturm");
    let re = coast.find(json!({ "q": r"Leuchtturm (\d+)", "range": "work", "re": true })).await;
    assert!(
        matched(&re).iter().all(|m| m.starts_with("Leuchtturm ") && m[11..].chars().all(|c| c.is_ascii_digit())),
        "regulärer Ausdruck mit Gruppe: {re}"
    );
    let bad =
        coast.server.rpc("search.find", json!({ "work": coast.work, "q": "(", "re": true, "range": "work" })).await;
    assert_eq!(bad["error"]["code"], -32602, "ungültiger Ausdruck ist ein Parameterfehler: {bad}");
}

#[tokio::test]
async fn ranges_cover_section_chapter_part_from_picked_work_and_all() {
    let coast = open(2).await;
    let harbour = coast.key_of("Hafen").await;
    let chapter = coast.key_of("Kapitel Vom Meer 0").await;
    let second_part = coast.key_of("Vom Land").await;
    let count = |v: &Value| v["total"].as_u64().unwrap();
    let section = coast.find(json!({ "q": "Leuchtturm", "range": "section", "key": harbour })).await;
    assert!(section["hits"].as_array().unwrap().iter().all(|h| h["key"] == json!(harbour)), "nur der Abschnitt");
    assert!(section["range_label"].as_str().unwrap().starts_with("Abschnitt"), "Bereich benannt: {section}");
    let chap = coast.find(json!({ "q": "Leuchtturm", "range": "chapter", "key": harbour })).await;
    assert_eq!(
        count(&chap),
        count(&coast.find(json!({ "q": "Leuchtturm", "range": "chapter", "key": chapter })).await),
        "Kapitel eines Abschnitts = das Kapitel selbst samt Unterpunkten"
    );
    assert!(count(&chap) > count(&section), "das Kapitel umfasst mehr als den Abschnitt");
    let part = coast.find(json!({ "q": "Leuchtturm", "range": "part", "key": second_part })).await;
    let work = coast.find(json!({ "q": "Leuchtturm", "range": "work" })).await;
    assert!(count(&part) < count(&work) && count(&part) > 0, "ein Teil ist ein Teil des Werks");
    assert!(
        part["hits"].as_array().unwrap().iter().all(|h| h["key"] != json!(harbour)),
        "der Hafen liegt im ersten Teil"
    );
    let from = coast.find(json!({ "q": "Leuchtturm", "range": "from", "key": second_part })).await;
    assert_eq!(count(&from), count(&part), "ab dem zweiten (letzten) Teil bis zum Ende = der zweite Teil");
    let picked = coast.find(json!({ "q": "Leuchtturm", "range": "picked", "sel": [harbour, chapter] })).await;
    let keys: std::collections::BTreeSet<i64> =
        picked["hits"].as_array().unwrap().iter().map(|h| h["key"].as_i64().unwrap()).collect();
    assert_eq!(keys, [chapter, harbour].into_iter().collect(), "Auswahl: genau die angehakten Punkte");
    let all = coast.find(json!({ "q": "Leuchtturm", "range": "all" })).await;
    assert!(count(&all) >= count(&work), "alle Werke enthalten dieses Werk");
    let missing = coast.server.rpc("search.find", json!({ "work": coast.work, "q": "x", "range": "chapter" })).await;
    assert_eq!(missing["error"]["code"], -32602, "Kapitel ohne Abschnitt (key) geht nicht: {missing}");
}

#[tokio::test]
async fn hits_carry_kinds_byte_and_utf16_offsets_and_typst_escapes() {
    let coast = open(1).await;
    let harbour = coast.key_of("Hafen").await;
    let body = coast.body_of(harbour).await;
    let found = coast.find(json!({ "q": "Leuchtturm", "range": "section", "key": harbour, "kinds": ["text", "heading", "footnote", "quote", "code"] })).await;
    let kinds: Vec<&str> = found["hits"].as_array().unwrap().iter().map(|h| h["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["text", "quote", "footnote", "code", "code"],
        "Fließtext, Zitat, Fußnote, Raw, Codezeile: {found}"
    );
    let default = coast.find(json!({ "q": "Leuchtturm", "range": "section", "key": harbour })).await;
    let kinds: Vec<&str> = default["hits"].as_array().unwrap().iter().map(|h| h["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["text", "quote", "footnote"], "Vorgabe beim Suchen: ohne Code");
    for hit in found["hits"].as_array().unwrap() {
        let (s, e) = (hit["start"].as_u64().unwrap() as usize, hit["end"].as_u64().unwrap() as usize);
        assert_eq!(&body[s..e], hit["matched"].as_str().unwrap(), "Byte-Stellen treffen den Text");
        let units: Vec<u16> = body.encode_utf16().collect();
        let (us, ue) = (hit["u16_start"].as_u64().unwrap() as usize, hit["u16_end"].as_u64().unwrap() as usize);
        assert_eq!(
            String::from_utf16(&units[us..ue]).unwrap(),
            hit["matched"].as_str().unwrap(),
            "UTF-16-Stellen ebenso"
        );
        let url = hit["url"].as_str().unwrap();
        let find = hit["matched"].as_str().unwrap();
        assert_eq!(url, format!("/?work={}&key={harbour}&find={find}&at={s}", coast.work), "Sprung-Adresse");
    }
    let quote = &found["hits"][1];
    assert!(quote["before"].as_str().unwrap().ends_with("„Ein "), "Umfeld davor: {quote}");
    assert!(quote["after"].as_str().unwrap().starts_with(" ist mein"), "Umfeld dahinter: {quote}");
    // Typst special characters: „C#“ finds the escaped form „C\#“.
    let sharp = coast.find(json!({ "q": "C#", "range": "section", "key": harbour })).await;
    assert_eq!(matched(&sharp), ["C\\#"], "`#` findet auch `\\#`");
    // Unicode: umlauts and accents case-insensitively, UTF-16 counts the emoji twice.
    let accents = "= Café\n\nÄPFEL im Café 😀 und äpfel.\n";
    let report = coast.server.import(accents, "cafe.typ").await;
    let cafe = coast.server.state.store.load_work(report.work).await.unwrap().nodes[0].key;
    let answer = coast
        .server
        .rpc("search.find", json!({ "work": report.work, "q": "äpfel", "range": "section", "key": cafe }))
        .await;
    let hits = answer["result"]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 2, "Groß/klein gilt auch für Umlaute: {answer}");
    let last = &hits[1];
    let prefix = &accents[..last["start"].as_u64().unwrap() as usize];
    assert_eq!(last["u16_start"].as_u64().unwrap() as usize, prefix.encode_utf16().count(), "UTF-16 hinter dem Emoji");
    assert!(last["u16_start"].as_u64().unwrap() < last["start"].as_u64().unwrap(), "Bytes zählen Umlaute doppelt");
    let cafe_hits = coast
        .server
        .rpc("search.find", json!({ "work": report.work, "q": "CAFÉ", "range": "section", "key": cafe }))
        .await;
    let kinds: Vec<&str> =
        cafe_hits["result"]["hits"].as_array().unwrap().iter().map(|h| h["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["heading", "text"], "Akzent ohne Groß/klein, Überschrift als eigene Art: {cafe_hits}");
}

#[tokio::test]
async fn the_shown_revision_and_unsaved_text_are_searched_in_the_section() {
    let coast = open(1).await;
    let harbour = coast.key_of("Hafen").await;
    let node = coast.server.state.store.node_seq(coast.work, harbour).await.unwrap();
    let body = coast.body_of(harbour).await;
    coast.server.state.store.save_human(coast.work, node, &body.replace("blinkt", "leuchtet"), "Ida").await.unwrap();
    let current = coast.find(json!({ "q": "blinkt", "range": "section", "key": harbour })).await;
    assert_eq!(current["total"], 0, "die aktuelle Fassung 2 blinkt nicht mehr");
    let shown =
        coast.find(json!({ "q": "blinkt", "range": "section", "key": harbour, "rev": "shown", "shown_rev": 1 })).await;
    assert_eq!(shown["total"], 1, "die gezeigte Fassung 1 schon");
    assert!(shown["hits"][0]["url"].as_str().unwrap().contains("&rev=1&"), "Sprung in die gezeigte Fassung");
    let unsaved = coast
        .find(json!({ "q": "Möwe", "range": "section", "key": harbour, "text": "== Hafen\n\nEine Möwe, noch eine Möwe.\n" }))
        .await;
    assert_eq!(unsaved["total"], 2, "ungespeicherter Editortext wird durchsucht");
    let selection = coast
        .find(json!({ "q": "Möwe", "range": "selection", "key": harbour, "start": 10, "end": 19,
                       "text": "== Hafen\n\nEine Möwe, noch eine Möwe.\n" }))
        .await;
    assert_eq!(selection["total"], 1, "in der Auswahl (UTF-16 10–19) nur die erste Möwe: {selection}");
    let refused = coast
        .server
        .rpc("search.find", json!({ "work": coast.work, "q": "x", "range": "work", "rev": "shown", "shown_rev": 1 }))
        .await;
    assert_eq!(refused["error"]["code"], -32602, "rev=shown nur im Abschnitt oder in der Auswahl: {refused}");
}

#[tokio::test]
async fn the_time_limit_and_the_regex_limits_hold() {
    let coast = open(1).await;
    let long = format!("= Wüste\n\n{}!\n", "a".repeat(60_000));
    let report = coast.server.import(&long, "wueste.typ").await;
    let started = Instant::now();
    let answer = coast
        .server
        .rpc("search.find", json!({ "work": report.work, "q": "(a|aa)+$", "re": true, "range": "work" }))
        .await;
    assert!(answer["error"].is_null(), "katastrophaler Ausdruck läuft (linear im Crate regex): {answer}");
    assert!(started.elapsed() < Duration::from_secs(5), "und bleibt in der Zeit: {:?}", started.elapsed());
    let query = SearchQuery { q: "a+".into(), regex: true, range: SearchRange::Work, ..SearchQuery::default() };
    let late =
        baruch_server::search::find_within(&coast.server.state, report.work, &query, Duration::ZERO).await.unwrap();
    assert!(late.timed_out, "nach Ablauf der Zeitgrenze: timed_out");
    assert!(late.hits.is_empty(), "nichts gefunden, weil keine Zeit blieb");
    let huge = coast
        .server
        .rpc(
            "search.find",
            json!({ "work": report.work, "q": "(?:(?:a{100}){100}){100}", "re": true, "range": "work" }),
        )
        .await;
    assert_eq!(huge["error"]["code"], -32602, "zu großer Ausdruck (size_limit): {huge}");
    assert!(huge["error"]["message"].as_str().unwrap().contains("zu groß"), "Meldung: {huge}");
    let long_q = coast.server.rpc("search.find", json!({ "work": report.work, "q": "x".repeat(501) })).await;
    assert_eq!(long_q["error"]["code"], -32602, "Suchtext höchstens 500 Zeichen: {long_q}");
    let limited = coast.find(json!({ "q": "Leuchtturm", "range": "work", "limit": 2 })).await;
    assert_eq!(limited["hits"].as_array().unwrap().len(), 2, "höchstens limit Treffer");
    assert_eq!(limited["truncated"], true, "gekürzt vermerkt");
    assert!(limited["total"].as_u64().unwrap() > 2, "gezählt wird weiter");
}

#[tokio::test]
async fn replacing_in_fifty_sections_makes_one_commit_and_spares_the_bible_quote() {
    // 2 parts × 2 chapters × 12 sections: 48 sections and 4 chapters with hits in the running text = 52.
    let coast = open(12).await;
    let harbour = coast.key_of("Hafen").await;
    let before = coast.subjects();
    let preview = coast
        .server
        .rpc(
            "search.preview",
            json!({ "work": coast.work, "q": "Leuchtturm", "replacement": "Wachturm", "range": "work" }),
        )
        .await;
    let sections = preview["result"]["sections"].as_array().unwrap().clone();
    assert_eq!(sections.len(), 52, "Vorschau: Kapitel und Abschnitte mit Treffern im Fließtext: {}", sections.len());
    let first = sections.iter().find(|s| s["key"] == json!(harbour)).unwrap();
    assert_eq!(
        first["hits"].as_array().unwrap().len(),
        2,
        "im Hafen: Fließtext und Fußnote, nicht das Zitat, nicht Code"
    );
    assert!(
        first["diff_html"]
            .as_str()
            .unwrap()
            .contains("<del data-hit=\"0\">Leuchtturm</del><ins data-hit=\"0\">Wachturm</ins>"),
        "Diff"
    );
    // Two sections are deselected so that exactly 50 are replaced.
    let picks: Vec<Value> =
        sections.iter().skip(2).map(|s| json!({ "key": s["key"], "base_rev": s["base_rev"], "skip": [] })).collect();
    let unconfirmed = coast
        .server
        .rpc("search.replace", json!({ "work": coast.work, "q": "Leuchtturm", "replacement": "Wachturm", "range": "work", "sections": picks }))
        .await;
    assert_eq!(unconfirmed["error"]["code"], -32602, "ohne confirm: nichts ersetzt: {unconfirmed}");
    let done = coast
        .server
        .rpc(
            "search.replace",
            json!({ "work": coast.work, "q": "Leuchtturm", "replacement": "Wachturm", "range": "work",
                                        "sections": picks, "confirm": true }),
        )
        .await;
    let result = &done["result"];
    assert_eq!(result["changed"].as_array().unwrap().len(), 50, "50 neue Fassungen: {done}");
    assert!(result["commit"].is_string(), "ein Commit im Spiegel: {done}");
    let after = coast.subjects();
    assert_eq!(after.len(), before.len() + 1, "genau ein Commit für alle 50 Abschnitte");
    assert_eq!(after[0], "Ersetzt „Leuchtturm“ → „Wachturm“ in 50 Abschnitten", "Commit-Text nach Vertrag §3.1");
    let work = coast.server.state.store.load_work(coast.work).await.unwrap();
    let human = work
        .nodes
        .iter()
        .filter(|n| n.current_number() == Some(2))
        .filter(|n| matches!(&n.revision(2).unwrap().author, baruch_core::Author::Human { .. }))
        .count();
    assert_eq!(human, 50, "jede neue Fassung vom Menschen");
    let changed_key = result["changed"][0]["key"].as_i64().unwrap();
    assert!(coast.body_of(changed_key).await.contains("Wachturm"), "ersetzt");
    assert!(
        coast.body_of(harbour).await.contains("„Ein Leuchtturm ist mein Licht“"),
        "abgewählt und Zitat ohnehin unberührt"
    );

    // Undo: one commit, all back at revision 1.
    let batch = result["batch"]["id"].as_i64().unwrap();
    let undone = coast.server.rpc("search.undo", json!({ "work": coast.work, "batch": batch, "confirm": true })).await;
    assert_eq!(undone["result"]["restored"].as_array().unwrap().len(), 50, "alle 50 zurück: {undone}");
    let last = coast.subjects();
    assert_eq!(last.len(), after.len() + 1, "Rückgängig ist genau ein Commit");
    assert_eq!(last[0], "Ersetzen rückgängig: „Leuchtturm“ → „Wachturm“ (50 Abschnitte)", "Commit-Text");
    assert!(coast.body_of(changed_key).await.contains("Leuchtturm"), "wieder der alte Text");
}

#[tokio::test]
async fn the_bible_quote_stays_by_default_and_changed_sections_are_skipped() {
    let coast = open(1).await;
    let harbour = coast.key_of("Hafen").await;
    let other = coast.key_of("Abschnitt 2").await;
    let preview = coast
        .server
        .rpc("search.preview", json!({ "work": coast.work, "q": "Leuchtturm", "replacement": "Wachturm", "range": "picked", "sel": [harbour, other] }))
        .await;
    let sections = preview["result"]["sections"].as_array().unwrap().clone();
    // The second section is saved by the human after the preview.
    let node = coast.server.state.store.node_seq(coast.work, other).await.unwrap();
    let body = coast.body_of(other).await;
    coast.server.state.store.save_human(coast.work, node, &format!("{body}Neu.\n"), "Ida").await.unwrap();
    let picks: Vec<Value> =
        sections.iter().map(|s| json!({ "key": s["key"], "base_rev": s["base_rev"], "skip": [] })).collect();
    let done = coast
        .server
        .rpc(
            "search.replace",
            json!({ "work": coast.work, "q": "Leuchtturm", "replacement": "Wachturm", "range": "picked",
                                        "sel": [harbour, other], "sections": picks, "confirm": true }),
        )
        .await;
    let result = &done["result"];
    assert_eq!(result["changed"].as_array().unwrap().len(), 1, "nur der Hafen: {done}");
    assert_eq!(result["skipped"][0]["key"], json!(other), "der geänderte Abschnitt wird übersprungen");
    assert!(result["skipped"][0]["reason"].as_str().unwrap().contains("inzwischen geändert"), "mit Grund: {done}");
    let harbour_body = coast.body_of(harbour).await;
    assert!(harbour_body.contains("Der Wachturm blinkt."), "Fließtext ersetzt");
    assert!(harbour_body.contains("„Ein Leuchtturm ist mein Licht“"), "Bibelzitat bleibt standardmäßig");
    assert!(harbour_body.contains("#footnote[Der Wachturm in der Fußnote.]"), "Fußnote ersetzt");
    assert!(harbour_body.contains("`Leuchtturm`") && harbour_body.contains("#let leuchtturm"), "Code bleibt");
    let refused = coast
        .server
        .rpc(
            "search.replace",
            json!({ "work": coast.work, "q": "x", "replacement": "y", "range": "all", "confirm": true }),
        )
        .await;
    assert_eq!(refused["error"]["code"], -32602, "Ersetzen nie über alle Werke: {refused}");
    let long = coast
        .server
        .rpc(
            "search.preview",
            json!({ "work": coast.work, "q": "x", "replacement": "y".repeat(2001), "range": "work" }),
        )
        .await;
    assert_eq!(long["error"]["code"], -32602, "Ersatz höchstens 2000 Zeichen: {long}");
}

#[tokio::test]
async fn a_heading_follows_its_new_title_but_never_changes_level() {
    let coast = open(1).await;
    let harbour = coast.key_of("Hafen").await;
    let done = coast
        .server
        .rpc(
            "search.replace",
            json!({ "work": coast.work, "q": "Hafen", "replacement": "Anleger", "range": "section",
                                        "key": harbour, "confirm": true }),
        )
        .await;
    assert_eq!(done["result"]["changed"].as_array().unwrap().len(), 1, "Titel ersetzt: {done}");
    let work = coast.server.state.store.load_work(coast.work).await.unwrap();
    assert!(work.nodes.iter().any(|n| n.key == harbour && n.title == "Anleger"), "Gliederung folgt dem Titel");
    let regex = coast
        .server
        .rpc(
            "search.replace",
            json!({ "work": coast.work, "q": "steht am Ufer", "replacement": "steht\n= Neu", "re": true,
                                        "range": "work", "confirm": true }),
        )
        .await;
    let skipped = regex["result"]["skipped"].as_array().unwrap();
    assert!(!skipped.is_empty(), "eine neue Überschrift durch Ersetzen wird übersprungen: {regex}");
    assert!(skipped[0]["reason"].as_str().unwrap().contains("Überschrift"), "mit Grund: {regex}");
}

#[tokio::test]
async fn the_search_page_works_without_javascript() {
    let coast = open(1).await;
    let harbour = coast.key_of("Hafen").await;
    let (status, html) = coast.server.get(&format!("/suche?work={}&q=Leuchtturm&range=work", coast.work)).await;
    assert_eq!(status, StatusCode::OK, "Seite lädt: {html}");
    assert!(html.contains("data-search-results"), "Trefferliste ohne JavaScript");
    assert!(html.contains("<mark>Leuchtturm</mark>"), "Treffer markiert");
    assert!(
        html.contains(&format!("href=\"/?work={}&amp;key={harbour}&amp;find=Leuchtturm&amp;at=", coast.work)),
        "Sprung-Link"
    );
    assert!(html.contains("data-jump"), "Sprung-Links tragen data-jump");
    assert!(html.contains("name=\"sel\""), "eigene Häkchenliste der Gliederung");
    assert!(html.contains("value=\"work\" checked"), "Bereich aus der Adresse");
    let (_, preview) = coast
        .server
        .get(&format!(
            "/suche?work={}&q=Leuchtturm&replacement=Wachturm&range=picked&sel={harbour}&vorschau=1",
            coast.work
        ))
        .await;
    assert!(preview.contains("data-search-preview"), "Vorschau: {preview}");
    assert!(preview.contains("<del data-hit=\"0\">Leuchtturm</del>"), "Diff je Abschnitt");
    assert!(preview.contains(&format!("name=\"pick\" value=\"{harbour}:1\" checked")), "Abschnitt abwählbar");
    assert!(preview.contains("Alle ersetzen"), "Knopf");
    // Replace without the check box „Ja, ersetzen“: back with a hint, nothing changed.
    let base = format!(
        "work={}&q=Leuchtturm&replacement=Wachturm&range=picked&sel={harbour}&pick={harbour}%3A1&count_{harbour}=2&keep={harbour}%3A0",
        coast.work
    );
    let (status, location) = coast.server.post_form("/suche/act", &format!("method=search.replace&{base}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303");
    assert!(location.unwrap().contains("fehler="), "ohne Bestätigung ein Hinweis");
    let (status, location) =
        coast.server.post_form("/suche/act", &format!("method=search.replace&{base}&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303 nach dem Ersetzen");
    let location = location.unwrap();
    assert!(
        location.starts_with(&format!("/suche?work={}&q=Leuchtturm&range=picked&sel={harbour}", coast.work)),
        "Bereich bleibt in der Adresse: {location}"
    );
    assert!(location.contains("&batch="), "Ergebnis mit Nummer: {location}");
    let body = coast.body_of(harbour).await;
    assert!(body.contains("Der Wachturm blinkt."), "erste Stelle ersetzt");
    assert!(body.contains("#footnote[Der Leuchtturm in der Fußnote.]"), "zweite Stelle abgewählt (keep fehlt)");
    let (_, done) = coast.server.get(&location).await;
    assert!(done.contains("data-search-batch"), "Ergebnis auf der Seite");
    assert!(done.contains("Rückgängig"), "Knopf Rückgängig");
    assert!(done.contains(&format!("/werke/{}/pruefung?key={harbour}", coast.work)), "Hinweis auf die Wortlautprüfung");
    let batch = location.split("&batch=").nth(1).unwrap();
    let (status, location) = coast
        .server
        .post_form(
            "/suche/act",
            &format!("method=search.undo&batch={batch}&confirm=1&work={}&q=Leuchtturm&range=work", coast.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303 nach Rückgängig");
    assert!(location.unwrap().contains("&undone="), "Rückgängig vermerkt");
    assert!(coast.body_of(harbour).await.contains("Der Leuchtturm blinkt."), "zurück");
}

#[tokio::test]
async fn the_replace_preview_is_bounded_and_never_starts_for_a_foreign_page() {
    // Finding of the review: GET /suche?vorschau=1 collected all hits including the replacement (up to 2000 characters
    // per hit) without a limit – an <img> of a foreign page could fill the memory that way.
    let coast = open(1).await;
    let wide = format!("= Weite\n\n{}\n", "Meer ".repeat(4000));
    let report = coast.server.import(&wide, "weite.typ").await;
    let long = "W".repeat(2000);
    let url = format!("/suche?work={}&q=.&re=1&replacement={long}&range=work&vorschau=1", report.work);
    let request = Request::get(&url).header("sec-fetch-site", "cross-site").body(Body::empty()).unwrap();
    let (status, _, body) = coast.server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "Vorschau auf Anfrage einer fremden Seite: 403: {body}");
    // From Baruch itself: 20 000 hits × 2000 characters would be 40 MB of replacement – above the budget, honestly
    // rejected.
    let started = Instant::now();
    let (status, html) = coast.server.get(&url).await;
    assert_eq!(status, StatusCode::OK, "die Seite antwortet");
    assert!(html.contains("Zu viele Ersetzungen auf einmal"), "Hinweis statt Speicherflut");
    assert!(!html.contains("data-search-preview"), "keine (halbe) Vorschau");
    assert!(started.elapsed() < Duration::from_secs(10), "und schnell: {:?}", started.elapsed());
    let rpc = coast
        .server
        .rpc(
            "search.preview",
            json!({ "work": report.work, "q": ".", "re": true, "replacement": long, "range": "work" }),
        )
        .await;
    assert_eq!(rpc["error"]["code"], -32000, "auch über RPC abgelehnt: {rpc}");
    // Below the budget but more than 2000 places in one section: no section by halves, preview noted as truncated.
    let (_, html) =
        coast.server.get(&format!("/suche?work={}&q=e&replacement=a&range=work&vorschau=1", report.work)).await;
    assert!(html.contains("data-search-preview-truncated"), "gekürzt vermerkt: {html}");
    assert!(!html.contains("name=\"pick\""), "der Abschnitt mit 4000 Stellen wird nicht halb angeboten");
}

#[tokio::test]
async fn the_search_page_keeps_selection_and_shown_revision_and_offers_both() {
    // Finding of the review: range „Markierung“ and revision „gezeigte“ could not be chosen on /suche and were lost on
    // submitting.
    let coast = open(1).await;
    let harbour = coast.key_of("Hafen").await;
    let seq = coast.server.state.store.node_seq(coast.work, harbour).await.unwrap();
    let body = coast.body_of(harbour).await;
    coast.server.state.store.save_human(coast.work, seq, &body.replace("blinkt", "leuchtet"), "Ida").await.unwrap();
    let (_, html) = coast
        .server
        .get(&format!("/suche?work={}&key={harbour}&q=blinkt&range=section&rev=shown&shown_rev=1", coast.work))
        .await;
    assert!(html.contains("data-search-rev"), "Fassung wählbar, wenn ein Abschnitt gewählt ist");
    assert!(html.contains("name=\"rev\" value=\"shown\" checked"), "gezeigte Fassung aus der Adresse gewählt");
    assert!(html.contains("gezeigte Fassung 1"), "mit Nummer");
    assert!(html.contains("<mark>blinkt</mark>"), "gesucht wird in der gezeigten Fassung 1 (aktuell steht „leuchtet“)");
    assert!(
        html.contains("<input type=\"hidden\" name=\"shown_rev\" value=\"1\">"),
        "das Suchformular trägt die gezeigte Fassung weiter"
    );
    let (_, current) = coast
        .server
        .get(&format!("/suche?work={}&key={harbour}&q=blinkt&range=section&rev=current&shown_rev=1", coast.work))
        .await;
    assert!(!current.contains("<mark>blinkt</mark>"), "aktuelle Fassung: kein Treffer mehr");
    // Selection: only the selected part (UTF-16) is searched, start/end stay in the form.
    let start = body.find("Leuchtturm blinkt").unwrap();
    let start16 = body[..start].encode_utf16().count();
    let end16 = start16 + "Leuchtturm blinkt".encode_utf16().count();
    let (_, sel) = coast
        .server
        .get(&format!(
            "/suche?work={}&key={harbour}&q=Leuchtturm&range=selection&start={start16}&end={end16}&rev=shown&shown_rev=1",
            coast.work
        ))
        .await;
    assert!(sel.contains("value=\"selection\" checked"), "Bereich „Markierung im Editor“ angeboten und gewählt");
    assert!(sel.contains(&format!("Zeichen {start16}–{end16}")), "mit der Stelle");
    assert_eq!(sel.matches("<mark>Leuchtturm</mark>").count(), 1, "nur der Treffer in der Markierung");
    assert!(sel.contains(&format!("name=\"start\" value=\"{start16}\"")), "start bleibt im Formular");
    assert!(sel.contains(&format!("name=\"end\" value=\"{end16}\"")), "end bleibt im Formular");
    let (_, plain) = coast.server.get(&format!("/suche?work={}&q=Leuchtturm&range=work", coast.work)).await;
    assert!(!plain.contains("value=\"selection\""), "ohne Markierung in der Adresse kein solcher Bereich");
    assert!(!plain.contains("data-search-rev"), "ohne Abschnitt keine Fassungswahl");
}

#[tokio::test]
async fn search_act_from_a_foreign_origin_is_refused() {
    let coast = open(1).await;
    let request = Request::post("/suche/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("origin", "https://fremd.example")
        .body(Body::from(format!("method=search.undo&batch=1&confirm=1&work={}", coast.work)))
        .unwrap();
    let (status, _, _) = coast.server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Herkunft: 403 (guard::same_origin)");
}

// ---- MCP ------------------------------------------------------------------------------------------------------

async fn mcp_post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

#[tokio::test]
async fn mcp_text_search_only_reads_and_keeps_its_limits() {
    let coast = open(1).await;
    let (headers, answer) = mcp_post(
        &coast.server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "Kompass", "version": "1" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let (_, tools) =
        mcp_post(&coast.server, Some(&session), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tool =
        tools["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "text_search").unwrap().clone();
    let description = tool["description"].as_str().unwrap();
    assert!(!description.contains("Stub"), "kein Stub mehr: {description}");
    assert!(description.contains("read-only"), "nur lesend: {description}");
    let schema = tool["inputSchema"]["properties"].as_object().unwrap();
    assert!(!schema.contains_key("replacement"), "kein Schreibweg im Schema");
    let before = coast.subjects();
    let (_, answer) = mcp_post(
        &coast.server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "text_search", "arguments": { "work": coast.slug, "q": "Leuchtturm", "limit": 3, "replacement": "Wachturm" } } }),
    )
    .await;
    assert_eq!(answer["result"]["isError"], false, "Suche gelingt: {answer}");
    let content = &answer["result"]["structuredContent"];
    assert_eq!(content["hits"].as_array().unwrap().len(), 3, "limit gilt: {content}");
    assert_eq!(content["truncated"], true, "gekürzt vermerkt");
    assert!(content["hits"][0]["url"].as_str().unwrap().starts_with("/?work="), "Treffer mit Adresse");
    assert_eq!(coast.subjects(), before, "ein fremdes Feld „replacement“ ändert nichts: kein Commit");
    let (_, answer) = mcp_post(
        &coast.server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": { "name": "text_search", "arguments": { "work": coast.work, "q": "x", "range": "all" } } }),
    )
    .await;
    assert!(
        answer["error"].is_object() || answer["result"]["isError"] == true,
        "range=all gibt es über MCP nicht: {answer}"
    );
}

// ---- Key bindings (Node) --------------------------------------------------------------------------------------

/// `keyAction` from the delivered `search_keys.js` under Node; per case `[taste, kontext]` → action or null.
fn key_actions(cases: Value) -> Value {
    let (_, module) = baruch_server::SCRIPTS
        .iter()
        .find(|(name, _)| *name == "search_keys.js")
        .expect("search_keys.js wird ausgeliefert");
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("search_keys.mjs"), module).unwrap();
    let script = format!(
        "import {{ keyAction }} from './search_keys.mjs';\nconst cases = {cases};\n\
         console.log(JSON.stringify(cases.map(([e, c]) => keyAction(Object.assign({{ctrlKey:false,shiftKey:false,altKey:false,metaKey:false}}, e), c))));\n"
    );
    std::fs::write(dir.path().join("probe.mjs"), script).unwrap();
    let out = match std::process::Command::new("node").arg("probe.mjs").current_dir(dir.path()).output() {
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
fn key_bindings_follow_vs_code() {
    let ws = json!({ "inEditor": true, "inSearch": false, "onWorkspace": true });
    let bar = json!({ "inEditor": false, "inSearch": true, "onWorkspace": true, "searchOpen": true });
    let open = json!({ "inEditor": true, "inSearch": false, "onWorkspace": true, "searchOpen": true });
    let chat = json!({ "inEditor": false, "inSearch": false, "onWorkspace": true, "inChat": true });
    let away = json!({ "inEditor": false, "inSearch": false, "onWorkspace": false });
    let cases = json!([
        [{ "key": "f", "code": "KeyF", "ctrlKey": true }, ws],
        [{ "key": "h", "code": "KeyH", "ctrlKey": true }, ws],
        [{ "key": "f", "code": "KeyF", "ctrlKey": true }, chat],
        [{ "key": "F", "code": "KeyF", "ctrlKey": true, "shiftKey": true }, ws],
        [{ "key": "H", "code": "KeyH", "ctrlKey": true, "shiftKey": true }, away],
        [{ "key": "Enter" }, bar],
        [{ "key": "F3" }, open],
        [{ "key": "Enter", "shiftKey": true }, bar],
        [{ "key": "F3", "shiftKey": true }, open],
        [{ "key": "!", "code": "Digit1", "ctrlKey": true, "shiftKey": true }, bar],
        [{ "key": "Enter", "ctrlKey": true, "altKey": true }, open],
        [{ "key": "c", "code": "KeyC", "altKey": true }, bar],
        [{ "key": "w", "code": "KeyW", "altKey": true }, bar],
        [{ "key": "r", "code": "KeyR", "altKey": true }, bar],
        [{ "key": "c", "code": "KeyC", "altKey": true }, open],
        [{ "key": "Escape" }, bar],
        [{ "key": "Enter" }, ws],
        [{ "key": "f", "code": "KeyF", "ctrlKey": true }, away],
        [{ "key": "f", "code": "KeyF", "metaKey": true }, ws],
        [{ "key": "l", "code": "KeyL", "altKey": true }, bar],
        [{ "key": "l", "code": "KeyL", "altKey": true }, ws],
    ]);
    let actions = key_actions(cases);
    if actions.is_null() {
        return;
    }
    assert_eq!(
        actions,
        json!([
            "find-section",
            "replace-section",
            null,
            "find-work",
            "replace-work",
            "next",
            "next",
            "prev",
            "prev",
            "replace-one",
            "replace-all",
            "toggle-case",
            "toggle-word",
            "toggle-regex",
            null,
            "close",
            null,
            null,
            "find-section",
            "toggle-selection",
            null
        ]),
        "Strg+F/H im Abschnitt (nicht im Chat, nicht außerhalb der Arbeitsseite), Strg+Umschalt+F/H im Werk, Enter/F3 \
         vor, Umschalt zurück, Strg+Umschalt+1 und Strg+Alt+Enter ersetzen, Alt+C/W/R nur in der Leiste, Escape \
         schließt, Enter im Editor bleibt dem Editor, Cmd wie Strg"
    );
}
