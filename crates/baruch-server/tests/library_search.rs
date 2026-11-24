// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC `library.list` und `library.search`, das Werkzeug des Lektors, auf der echten Datenbank
//! (Kernmigration aus baruch-store, danach Bibliothek).

use crate::common;

use serde_json::json;

#[tokio::test]
async fn library_search_finds_langmuetig_in_work_and_library() {
    let server = common::open().await;
    let answer = server.rpc("library.search", json!({ "query": "langmütig" })).await;
    let hits = answer["result"]["hits"].as_array().unwrap_or_else(|| panic!("keine Treffer: {answer}"));
    let revision =
        hits.iter().find(|h| h["kind"] == "revision").unwrap_or_else(|| panic!("kein Werk-Treffer: {answer}"));
    assert_eq!(revision["node"], 4, "„langmütig“ steht in 2.1 Erste Anzeichen");
    assert_eq!(revision["work"], 1, "Vorgabe ist das aktuelle Werk");
    assert!(
        revision["label"].as_str().unwrap().contains("Erste Anzeichen"),
        "Bezeichnung nennt den Abschnitt: {revision}"
    );
    assert!(revision["snippet"].as_str().unwrap().contains("<mark>langmütig</mark>"), "Marken fehlen: {revision}");
    let quote = hits.iter().find(|h| h["kind"] == "quote").unwrap_or_else(|| panic!("kein Zitat-Treffer: {answer}"));
    assert!(quote["key"].as_str().is_some_and(|k| !k.is_empty()), "Zitierschlüssel fehlt: {quote}");

    // Ohne Umlaut und nur Bibliothek.
    let folded = server.rpc("library.search", json!({ "query": "langmutig", "kinds": ["quote"] })).await;
    let folded = folded["result"]["hits"].as_array().unwrap();
    assert!(
        !folded.is_empty() && folded.iter().all(|h| h["kind"] == "quote"),
        "ohne Umlaut gefunden, und kinds lässt nur Zitate durch: {folded:?}"
    );
}

#[tokio::test]
async fn a_saved_revision_is_found_right_away() {
    let server = common::open().await;
    server.rpc("revision.save", json!({ "node": 4, "body": "== Erste Anzeichen\n\nQuargelbrot und Langmut.\n" })).await;
    let answer = server.rpc("library.search", json!({ "query": "Quargelbrot" })).await;
    let hits = answer["result"]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "genau die neue aktuelle Revision: {answer}");
    assert_eq!(hits[0]["node"], 4, "der Treffer liegt im gespeicherten Abschnitt 2.1: {answer}");
}

#[tokio::test]
async fn a_search_for_nul_is_an_invalid_parameter_not_an_internal_error() {
    let server = common::open().await;
    for query in ["\u{0}", "\u{1}\u{0}\u{7f}"] {
        let answer = server.rpc("library.search", json!({ "query": query })).await;
        assert_eq!(answer["error"]["code"], -32602, "{query:?}: leere Suche ist -32602, nicht -32603: {answer}");
        assert!(answer.get("result").is_none(), "{query:?}: Fehler und Ergebnis zugleich: {answer}");
    }
    let answer = server.rpc("library.search", json!({ "query": "lang\u{0}mütig" })).await;
    assert!(answer["result"]["hits"].is_array(), "NUL mitten im Wort darf die Suche nicht abbrechen: {answer}");
}

#[tokio::test]
async fn snippets_are_html_with_escaped_text_and_marked_hits() {
    let server = common::open().await;
    let body = "== Erste Anzeichen\n\n<b>fett</b> & Quargelbrot #emph[Quargelbrot]\n";
    server.rpc("revision.save", json!({ "node": 4, "body": body })).await;
    let answer = server.rpc("library.search", json!({ "query": "Quargelbrot" })).await;
    let snippet = answer["result"]["hits"][0]["snippet"].as_str().unwrap_or_else(|| panic!("kein Treffer: {answer}"));
    assert!(snippet.contains("&lt;b&gt;fett&lt;/b&gt; &amp;"), "Text im Ausschnitt maskiert: {snippet}");
    assert!(
        snippet.contains("<mark>Quargelbrot</mark> #emph[<mark>Quargelbrot</mark>]"),
        "Treffer als <mark>, Klammern bleiben: {snippet}"
    );
}

#[tokio::test]
async fn library_list_returns_titles_with_quotes() {
    let server = common::open().await;
    let answer = server.rpc("library.list", json!(null)).await;
    let titles = answer["result"]["titles"].as_array().unwrap_or_else(|| panic!("library.list ohne Titel: {answer}"));
    assert!(titles.len() >= 2, "Beispielbibliothek fehlt: {answer}");
    let luther = titles.iter().find(|t| t["title"].as_str().unwrap().contains("Lutherbibel")).unwrap();
    assert!(
        luther["key"].is_string() && luther["quotes"].as_array().is_some_and(|q| !q.is_empty()),
        "die Lutherbibel hat einen Zitierschlüssel und Zitate: {luther}"
    );
}

#[tokio::test]
async fn library_search_finds_highlights_with_their_quote_and_id() {
    let server = common::open().await;
    let list = server.rpc("library.list", json!(null)).await;
    let luther =
        list["result"]["titles"].as_array().unwrap().iter().find(|t| t["key"] == "luther1912").unwrap().clone();
    let quote = luther["quotes"][0]["id"].clone();
    let added = server
        .rpc("highlight.add", json!({ "title": luther["id"], "quote": quote, "text": "Quargelperle", "kind": "core" }))
        .await;
    let id = added["result"]["highlight"]["id"].clone();
    assert!(id.is_i64(), "Hervorheben scheiterte: {added}");
    let answer = server.rpc("library.search", json!({ "query": "Quargelperle", "kinds": ["highlight"] })).await;
    let hits = answer["result"]["hits"].as_array().unwrap_or_else(|| panic!("keine Treffer: {answer}"));
    assert_eq!(hits.len(), 1, "genau die Hervorhebung: {answer}");
    assert_eq!(hits[0]["kind"], "highlight", "Art highlight: {answer}");
    assert_eq!(hits[0]["highlight_id"], id, "Treffer nennt die Hervorhebung: {answer}");
    assert_eq!(hits[0]["quote_id"], quote, "Treffer nennt das Zitat, in dem sie liegt: {answer}");
    assert_eq!(hits[0]["key"], "luther1912", "Zitierschlüssel der Quelle: {answer}");

    let all = server.rpc("library.search", json!({ "query": "langmütig", "kinds": ["quote"] })).await;
    let q = &all["result"]["hits"][0];
    assert!(
        q["quote_id"].is_i64() && q["highlight_id"].is_null(),
        "Zitat-Treffer: quote_id gesetzt, highlight_id null: {q}"
    );
}
