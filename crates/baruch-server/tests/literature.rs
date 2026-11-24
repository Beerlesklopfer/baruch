// SPDX-License-Identifier: AGPL-3.0-or-later
//! Literatur als Popup und eigene Seiten (`/literatur`, `/quelle`), Hervorhebungen über RPC und
//! Formular (`POST /literatur/act`), Zitat als Vorschlag-Revision. Echter Datenordner im Temp-Ordner.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_library::{HighlightAuthor, HighlightKind, NewHighlight, NewTitle};
use serde_json::{Value, json};

/// Beispieltitel `luther1912` und sein Zitat 1. Kor 13,4: (Titel-id, Zitat-id).
async fn luther(server: &common::Server) -> (i64, i64) {
    let answer = server.rpc("library.list", json!(null)).await;
    let titles = answer["result"]["titles"].as_array().unwrap_or_else(|| panic!("keine Titel: {answer}"));
    let t = titles.iter().find(|t| t["key"] == "luther1912").expect("Lutherbibel fehlt");
    let q = t["quotes"].as_array().unwrap().iter().find(|q| q["page"] == "1. Kor 13,4").expect("1. Kor 13,4 fehlt");
    (t["id"].as_i64().unwrap(), q["id"].as_i64().unwrap())
}

/// Fester Schlüssel (`key`) des Abschnitts an Stelle `seq` im Werk `work`.
async fn key_of(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

/// `highlight.add` über RPC; gibt die Hervorhebung zurück.
async fn add(server: &common::Server, params: Value) -> Value {
    let answer = server.rpc("highlight.add", params).await;
    let highlight = answer["result"]["highlight"].clone();
    assert!(highlight.is_object(), "Hervorheben scheiterte: {answer}");
    highlight
}

/// Ausschnitt `from` … `to` (erstes Vorkommen danach).
fn between<'a>(html: &'a str, from: &str, to: &str) -> &'a str {
    let start = html.find(from).unwrap_or_else(|| panic!("{from:?} fehlt: {html}"));
    let rest = &html[start..];
    &rest[..rest.find(to).unwrap_or(rest.len())]
}

/// `href` des ersten Treffers einer Art (Maskierung von `&` zurückgenommen).
fn hit_link(html: &str, kind: &str) -> String {
    let item = between(html, &format!("<li class=\"library-hit\" data-kind=\"{kind}\""), "</li>");
    let href = item.split("href=\"").nth(1).and_then(|s| s.split('"').next()).unwrap();
    href.replace("&amp;", "&")
}

/// Text des Abschnitts 4 (2.1 Erste Anzeichen) in der aktuellen Fassung und deren Nummer.
async fn current(server: &common::Server) -> (u64, String) {
    let node = server.rpc("node.get", json!({ "node": 4 })).await;
    let rev = node["result"]["revisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["status"] == "current")
        .unwrap_or_else(|| panic!("keine aktuelle Fassung: {node}"))
        .clone();
    (rev["number"].as_u64().unwrap(), rev["body"].as_str().unwrap().to_owned())
}

#[tokio::test]
async fn the_workspace_has_no_literature_column_but_a_header_link_and_a_dialog() {
    let server = common::open().await;
    let (status, html) = server.get("/?node=2&lq=langm%C3%BCtig").await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite mit Suche lädt: {html}");
    let tree = between(&html, "id=\"pane-tree\"", "</nav>");
    assert!(!tree.contains("data-widget=\"library\""), "linke Spalte zeigt noch die Literatur: {tree}");
    assert!(!tree.contains("Literatur"), "linke Spalte nennt die Literatur noch: {tree}");
    let head = between(&html, "<header class=\"topbar\">", "</header>");
    let k = key_of(&server, 1, 2).await;
    assert!(
        head.contains(&format!("href=\"/literatur?work=1&key={k}&rev=")) && head.contains("&amp;lq=langm%C3%BCtig\""),
        "Literatur-Link ohne JavaScript trägt Werk, Schlüssel des Abschnitts (Runde 8) und Suche: {head}"
    );
    assert!(head.contains(">Literatur</a>"), "Schaltfläche heißt „Literatur“: {head}");
    let dialog = between(&html, "<dialog", "</dialog>");
    assert!(
        dialog.contains("id=\"literature\" data-widget=\"literature\""),
        "Popup ist ein <dialog> mit Widget literature: {dialog}"
    );
    assert!(dialog.contains("data-widget=\"library\""), "Popup enthält die Literatur: {dialog}");
    assert!(dialog.contains("<form method=\"dialog\">"), "Popup schließt sich ohne eigenes Skript: {dialog}");
    assert!(dialog.contains("value=\"langmütig\""), "Popup zeigt die Suche der Adresse: {dialog}");
}

#[tokio::test]
async fn the_literature_page_lists_all_kinds_of_hits_without_javascript() {
    let server = common::open().await;
    let (title, quote) = luther(&server).await;
    let posted = server.rpc("post.add", json!({ "node": 4, "body": "Wie langmütig darf Liebe sein?" })).await;
    assert!(posted.get("error").is_none(), "Beitrag scheiterte: {posted}");
    let book = NewTitle {
        key: "geduld2020".into(),
        kind: "book".into(),
        title: "Langmütig leben".into(),
        authors: "Muster, Max".into(),
        year: Some(2020),
        publisher: String::new(),
        place: String::new(),
        url: String::new(),
        note: String::new(),
    };
    baruch_library::add_title(server.state.store.pool(), &book).await.unwrap();
    let h = add(&server, json!({ "title": title, "quote": quote, "text": "langmütig und freundlich", "kind": "core" }))
        .await;

    let (status, html) = server.get("/literatur?node=4&lq=langm%C3%BCtig").await;
    assert_eq!(status, StatusCode::OK, "die Literatur-Seite lädt mit Suche: {html}");
    assert!(
        html.contains("<form action=\"/literatur\" method=\"get\" class=\"library-search\""),
        "Suchformular: {html}"
    );
    for kind in ["revision", "post", "title", "quote", "highlight"] {
        assert!(html.contains(&format!("data-kind=\"{kind}\"")), "kein Treffer der Art {kind}: {html}");
    }
    assert!(
        html.contains("<mark>langmütig</mark>") || html.contains("<mark>Langmütig</mark>"),
        "Treffer markiert: {html}"
    );
    assert!(html.contains("class=\"library-highlights\""), "Titel zeigt seine Hervorhebungen: {html}");
    assert!(html.contains("<blockquote class=\"library-quote\""), "Titel zeigt seine Zitate: {html}");
    assert!(html.contains(&format!("#hervorhebung-{}", h["id"])), "Hervorhebung in der Titelliste verlinkt: {html}");
    let k = key_of(&server, 1, 4).await;
    assert!(
        html.contains(&format!("href=\"/?work=1&amp;key={k}&amp;lq=langm%C3%BCtig\" data-back")),
        "Link zurück zum Werk mit Schlüssel des Abschnitts und Suche: {html}"
    );
    assert!(!html.contains("{{") && !html.contains("{%"), "Platzhalter übrig: {html}");
}

#[tokio::test]
async fn the_search_is_kept_in_every_link_and_control_characters_give_no_hits() {
    let server = common::open().await;
    let (status, html) = server.get("/literatur?node=4&lq=langm%C3%BCtig%20%26%20100%25%2B%23x").await;
    assert_eq!(status, StatusCode::OK, "Sonderzeichen in der Suche ergeben keine Fehlerseite: {html}");
    let encoded = "lq=langm%C3%BCtig%20%26%20100%25%2B%23x";
    let k = key_of(&server, 1, 4).await;
    assert!(
        html.contains(&format!("href=\"/?work=1&amp;key={k}&amp;{encoded}\"")),
        "Rückweg trägt den Schlüssel und die Suche kodiert: {html}"
    );
    let sources = html.matches("href=\"/quelle?").count();
    assert!(sources > 0, "keine Links auf Quellen: {html}");
    assert_eq!(
        html.matches(&format!("&amp;{encoded}")).count(),
        sources + 1,
        "jeder Quellen-Link und der Rückweg tragen die Suche, % & + # kodiert: {html}"
    );

    for uri in ["/literatur?lq=%00", "/literatur?node=2&lq=%01%00%7F"] {
        let (status, html) = server.get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: NUL darf keine Fehlerseite ergeben: {html}");
        assert!(html.contains("Keine Treffer."), "{uri}: leere Trefferliste erwartet: {html}");
    }
    let (status, _) = server.get("/literatur?work=99").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk");
}

#[tokio::test]
async fn hit_links_open_the_editor_or_the_source_view_at_the_place_found() {
    let server = common::open().await;
    let (title, quote) = luther(&server).await;
    let first = add(&server, json!({ "title": title, "quote": quote, "start": 4, "end": 9, "text": "Liebe", "kind": "quote", "note": "langmütig" })).await;
    let other = add(&server, json!({ "title": title, "text": "Baruch, der Schreiber", "kind": "check" })).await;
    let (h, o) = (first["id"].as_i64().unwrap(), other["id"].as_i64().unwrap());
    let k = key_of(&server, 1, 4).await;
    let (_, html) = server.get("/literatur?node=4&lq=langm%C3%BCtig").await;

    let revision = hit_link(&html, "revision");
    assert!(
        revision.starts_with(&format!("/?work=1&key={k}&rev=")) && revision.ends_with("&lq=langm%C3%BCtig#fundstelle"),
        "Werk-Treffer öffnet den Editor an der Fundstelle, über den Schlüssel (node_key): {revision}"
    );
    let source = hit_link(&html, "quote");
    assert!(
        source.starts_with(&format!(
            "/quelle?title={title}&quote={quote}&work=1&key={k}&lq=langm%C3%BCtig#zitat-{quote}"
        )),
        "Zitat-Treffer öffnet die Dokumentansicht am Zitat und trägt den Schlüssel des Abschnitts: {source}"
    );
    let marked = hit_link(&html, "highlight");
    assert_eq!(
        marked,
        format!("/quelle?title={title}&quote={quote}&highlight={h}&work=1&key={k}&lq=langm%C3%BCtig#hervorhebung-{h}"),
        "Hervorhebungs-Treffer öffnet die Dokumentansicht an der Hervorhebung"
    );

    // Dem Link folgen (ohne Anker, den schickt kein Browser mit).
    let (status, page) = server.get(marked.split('#').next().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "der Link führt auf eine Dokumentansicht: {page}");
    assert!(
        page.contains(&format!("id=\"hervorhebung-{h}\" class=\"highlight kind-quote target\"")),
        "Ziel markiert: {page}"
    );
    assert!(
        page.contains(&format!("id=\"hervorhebung-{o}\" class=\"highlight kind-check\"")),
        "alle Hervorhebungen sichtbar: {page}"
    );
    assert!(
        page.contains("<blockquote class=\"highlight-text\"><mark>Liebe</mark>"),
        "Text des Ziels in <mark>: {page}"
    );
    assert!(
        page.contains(&format!("id=\"zitat-{quote}\" class=\"source-quote target\"")),
        "Zitat des Ziels markiert: {page}"
    );
    assert!(
        page.contains(&format!(
            "Die <mark class=\"hl kind-quote target\" data-highlight=\"{h}\">Liebe</mark> ist langmütig"
        )),
        "Bereich der Hervorhebung im Zitattext markiert: {page}"
    );
    // Runde 13: Der Volltext kommt über MultiGPT; eine Quelle ohne Verknüpfung sagt das ehrlich (kein Stub mehr).
    assert!(
        page.contains("data-fulltext=\"none\"") && page.contains("nicht mit MultiGPT verknüpft"),
        "Quelle ohne Verknüpfung: ehrlicher Hinweis statt Volltext: {page}"
    );
    assert!(
        !include_str!("../src/literature.rs").contains(&format!("// {}: Dokumentansicht", "STUB")),
        "der Stub-Vermerk zum Volltext ist mit Runde 13 entfallen"
    );
    assert!(page.contains("action=\"/literatur/act\""), "Formular ohne JavaScript unter der Dokumentansicht: {page}");
    assert!(
        page.contains(&format!("href=\"/?work=1&amp;key={k}&amp;lq=langm%C3%BCtig\" data-back")),
        "Rückweg zum Werk über den Schlüssel: {page}"
    );

    let (status, _) = server.get("/quelle?title=9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekannte Quelle");
    let (status, _) = server.get("/quelle").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "Quelle fehlt");
}

#[tokio::test]
async fn highlight_rpc_adds_lists_updates_and_deletes() {
    let server = common::open().await;
    let (title, quote) = luther(&server).await;
    let h =
        add(&server, json!({ "title": title, "quote": quote, "text": "langmütig", "kind": "core", "note": "Kern" }))
            .await;
    assert_eq!(h["status"], "active", "Mensch → aktiv: {h}");
    assert_eq!(h["author"]["kind"], "human", "Urheber ist der Mensch: {h}");
    assert_eq!(h["locator"], "1. Kor 13,4", "Fundstelle aus dem Zitat: {h}");
    let id = h["id"].as_i64().unwrap();

    let list = server.rpc("highlight.list", json!({ "title": title })).await;
    assert_eq!(list["result"]["highlights"].as_array().map(Vec::len), Some(1), "Liste je Titel: {list}");
    let changed =
        server.rpc("highlight.update", json!({ "highlight": id, "kind": "contradiction", "note": "anders" })).await;
    assert_eq!(changed["result"]["highlight"]["kind"], "contradiction", "Art geändert: {changed}");
    assert_eq!(changed["result"]["highlight"]["note"], "anders", "Notiz geändert: {changed}");

    // Persona-Vorschlag: bleibt Vorschlag.
    let proposal = NewHighlight {
        title_id: title,
        quote_id: None,
        locator: String::new(),
        start: None,
        end: None,
        text: "Vorschlag der Lektorin".into(),
        kind: HighlightKind::Check,
        note: String::new(),
        author: HighlightAuthor::Persona { handle: "@lektor".into(), model: "qwen3:4b".into() },
    };
    let pid = baruch_library::add_highlight(server.state.store.pool(), &proposal).await.unwrap();
    let refused = server.rpc("highlight.update", json!({ "highlight": pid, "status": "active" })).await;
    assert_eq!(refused["error"]["code"], -32000, "Persona-Hervorhebung wird nicht aktiv: {refused}");
    let rejected = server.rpc("highlight.update", json!({ "highlight": pid, "status": "rejected" })).await;
    assert_eq!(rejected["result"]["highlight"]["status"], "rejected", "Vorschlag ablehnen geht: {rejected}");

    for (params, why) in [
        (json!({ "title": title, "text": "  ", "kind": "core" }), "leerer Text"),
        (json!({ "title": title, "text": "x", "kind": "core", "start": 5, "end": 1 }), "Bereich rückwärts"),
        (
            json!({ "title": title, "quote": quote, "text": "ganz anderer Text", "kind": "core", "start": 0, "end": 9999 }),
            "Ende hinter dem Zitat",
        ),
        (
            json!({ "title": title, "quote": quote, "text": "ganz anderer Text", "kind": "core", "start": 0, "end": 3 }),
            "Text ungleich dem Ausschnitt",
        ),
        (json!({ "title": title, "text": "x", "kind": "unsinn" }), "unbekannte Art"),
        (json!({ "title": 9999, "text": "x", "kind": "core" }), "unbekannter Titel"),
    ] {
        let answer = server.rpc("highlight.add", params).await;
        assert_eq!(answer["error"]["code"], -32602, "{why}: ungültige Parameter erwartet: {answer}");
    }

    let deleted = server.rpc("highlight.delete", json!({ "highlight": id })).await;
    assert_eq!(deleted["result"]["deleted"], id, "gelöscht: {deleted}");
    let again = server.rpc("highlight.delete", json!({ "highlight": id })).await;
    assert_eq!(again["error"]["code"], -32602, "zweites Löschen: unbekannte Hervorhebung: {again}");
    let unknown = server.rpc("highlight.nothing", json!({})).await;
    assert_eq!(unknown["error"]["code"], -32601, "unbekannte Methode der Literatur: {unknown}");
}

#[tokio::test]
async fn citing_a_highlight_adds_a_proposal_and_leaves_the_current_text() {
    let server = common::open().await;
    let (title, quote) = luther(&server).await;
    let (number, body) = current(&server).await;
    let h = add(
        &server,
        json!({ "title": title, "quote": quote, "text": "Liebe [ist] #langmütig *sehr* @x", "kind": "quote" }),
    )
    .await;
    let k = key_of(&server, 1, 4).await;
    for (params, why) in [
        (json!({ "highlight": h["id"], "node": 4 }), "node (Stelle) wird nicht angenommen; key mitgeben"),
        (json!({ "highlight": h["id"], "node": 4, "key": k }), "node (Stelle) wird nicht angenommen; key mitgeben"),
        (json!({ "highlight": h["id"] }), "key fehlt"),
    ] {
        let refused = server.rpc("highlight.cite", params.clone()).await;
        assert_eq!(refused["error"]["code"], -32602, "{params}: -32602 statt eines Vorschlags: {refused}");
        let message = refused["error"]["message"].as_str().unwrap_or_default();
        assert!(message.contains(why), "{params}: deutscher Hinweis „{why}“: {message}");
    }
    let gone = server.rpc("highlight.cite", json!({ "highlight": h["id"], "key": 99_999 })).await;
    assert_eq!(gone["error"]["code"], -32000, "unbekannter Schlüssel ist ein fachlicher Fehler: {gone}");
    let node = server.rpc("node.get", json!({ "node": 4 })).await;
    assert_eq!(node["result"]["revisions"].as_array().unwrap().len(), 2, "keine Ablehnung legte etwas an: {node}");

    let cited = server.rpc("highlight.cite", json!({ "highlight": h["id"], "key": k })).await;
    let rev = cited["result"]["rev"].as_u64().unwrap_or_else(|| panic!("Zitieren scheiterte: {cited}"));
    assert_eq!(
        (cited["result"]["node"].as_i64(), cited["result"]["key"].as_i64()),
        (Some(4), Some(k)),
        "Antwort nennt Stelle und Schlüssel: {cited}"
    );

    let node = server.rpc("node.get", json!({ "node": 4 })).await;
    let revisions = node["result"]["revisions"].as_array().unwrap();
    let proposal =
        revisions.iter().find(|r| r["number"] == rev).unwrap_or_else(|| panic!("Vorschlag {rev} fehlt: {node}"));
    assert_eq!(proposal["status"], "proposed", "Zitat ist ein Vorschlag: {proposal}");
    assert_eq!(
        proposal["author"],
        json!({ "kind": "assistant", "persona": "@literatur", "model": "baruch-zitat" }),
        "Urheber @literatur mit Modell baruch-zitat"
    );
    assert_eq!(proposal["note"], "Zitat aus @luther1912", "Notiz nennt den Schlüssel");
    let text = proposal["body"].as_str().unwrap();
    assert!(text.starts_with(&body), "Vorschlag beginnt mit dem bisherigen Text: {text}");
    assert!(
        text.ends_with(
            "#quote(block: true, attribution: [@luther1912[1\\. Kor 13,4]])[Liebe \\[ist\\] \\#langmütig \\*sehr\\* \\@x]\n"
        ),
        "gültiges Typst-Zitat mit maskierten Sonderzeichen am Ende: {text}"
    );
    assert_eq!(current(&server).await, (number, body), "aktuelle Fassung bleibt unberührt");
    assert_eq!(common::currents(&node["result"]), 1, "genau eine aktuelle Fassung");
}

#[tokio::test]
async fn literature_render_returns_the_popup_content() {
    let server = common::open().await;
    let (title, _) = luther(&server).await;
    add(&server, json!({ "title": title, "text": "Geduld ist langmütig", "kind": "core" })).await;
    let answer = server.rpc("literature.render", json!({ "node": 4, "lq": "langmütig" })).await;
    let html = answer["result"]["html"].as_str().unwrap_or_else(|| panic!("kein HTML: {answer}"));
    assert_eq!(answer["result"]["lq"], "langmütig", "Suche kommt zurück: {answer}");
    assert!(html.contains("data-widget=\"library\""), "Inhalt des Popups: {html}");
    assert!(html.contains("data-kind=\"highlight\""), "Hervorhebungen werden gefunden: {html}");
    let k = key_of(&server, 1, 4).await;
    assert!(html.contains(&format!("name=\"key\" value=\"{k}\"")), "Suchformular behält den festen Schlüssel: {html}");
    assert!(
        !html.contains("name=\"node\""),
        "mit Schlüssel trägt das GET-Formular keine Stelle mehr (seine Adresse wäre sonst node=…): {html}"
    );
    assert!(
        html.contains(&format!("&amp;work=1&amp;key={k}")) && !html.contains("&amp;node="),
        "Links zur Dokumentansicht tragen den Schlüssel, keine Stelle: {html}"
    );
    assert!(html.contains("class=\"library-highlights\""), "Titel mit Hervorhebungen: {html}");
    let empty = server.rpc("literature.render", json!({ "lq": "\u{0}" })).await;
    assert!(
        empty["result"]["html"].as_str().is_some_and(|h| h.contains("Keine Treffer.")),
        "NUL ergibt eine leere Trefferliste: {empty}"
    );
}

#[tokio::test]
async fn forms_without_javascript_highlight_cite_search_and_answer_the_persona_stub() {
    let server = common::open().await;
    let (title, quote) = luther(&server).await;
    let k = key_of(&server, 1, 4).await;

    let form = format!(
        "action=highlight&work=1&node=4&title={title}&quote={quote}&start=4&end=9&text=Liebe&kind=core&lq=langm%C3%BCtig"
    );
    let (status, location) = server.post_form("/literatur/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Weiterleitung nach dem Hervorheben");
    let location = location.unwrap();
    let list = server.rpc("highlight.list", json!({})).await;
    let id = list["result"]["highlights"][0]["id"].as_i64().unwrap_or_else(|| panic!("nichts angelegt: {list}"));
    assert_eq!(
        location,
        format!(
            "/quelle?title={title}&work=1&key={k}&quote={quote}&highlight={id}&lq=langm%C3%BCtig#hervorhebung-{id}"
        ),
        "zurück zur Dokumentansicht an der neuen Hervorhebung, mit dem Schlüssel des Abschnitts"
    );
    assert_eq!(list["result"]["highlights"][0]["start"], 4, "Bereich aus dem Formular: {list}");

    let (status, location) = server
        .post_form("/literatur/act", &format!("action=cite&work=1&node=4&key={k}&title={title}&text=Glaube+%23x"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Zitieren leitet zum neuen Vorschlag weiter");
    let location = location.unwrap();
    assert!(location.starts_with(&format!("/?work=1&key={k}&rev=")), "weiter zum Vorschlag im Editor: {location}");
    let rev: u64 = location.rsplit('=').next().unwrap().parse().unwrap();
    let node = server.rpc("node.get", json!({ "node": 4 })).await;
    let proposal = node["result"]["revisions"].as_array().unwrap().iter().find(|r| r["number"] == rev).unwrap();
    assert!(proposal["body"].as_str().unwrap().contains("[Glaube \\#x]"), "Zitat im Vorschlag: {proposal}");

    // Runde 15: „An Persona geben“ ist umgesetzt; ohne Abschnitt zeigt eine Hinweisseite (200) „Abschnitt wählen“.
    let request = axum::http::Request::post("/literatur/act")
        .header(axum::http::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(format!("action=persona&title={title}&text=Liebe")))
        .unwrap();
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "ohne Abschnitt: Hinweisseite statt Fehler");
    assert!(page.contains("Abschnitt wählen"), "Hinweis „Abschnitt wählen“: {page}");
    assert!(!include_str!("../src/literature.rs").contains("STUB: Persona-Auftrag"), "Stub-Vermerk entfernt");

    let (status, location) =
        server.post_form("/literatur/act", "action=similar&node=4&text=Die+Liebe+ist+langm%C3%BCtig").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Ähnliches suchen leitet zur Literatur mit Suche weiter");
    assert_eq!(
        location.as_deref(),
        Some(format!("/literatur?work=1&key={k}&lq=Die%20Liebe%20langm%C3%BCtig").as_str()),
        "Ähnliches suchen: die längsten Wörter als Suche"
    );

    let (status, _) =
        server.post_form("/literatur/act", &format!("action=update&work=1&highlight={id}&kind=check&note=Neu")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Ändern über das Formular");
    let h = server.rpc("highlight.list", json!({})).await;
    let changed = h["result"]["highlights"].as_array().unwrap().iter().find(|x| x["id"] == id).unwrap().clone();
    assert_eq!(
        (changed["kind"].as_str(), changed["note"].as_str()),
        (Some("check"), Some("Neu")),
        "Art und Notiz geändert: {changed}"
    );

    let (status, _) = server.post_form("/literatur/act", &format!("action=delete&highlight={id}")).await;
    assert_eq!(status, StatusCode::CONFLICT, "Löschen ohne Kästchen wird abgelehnt");
    let (status, location) =
        server.post_form("/literatur/act", &format!("action=delete&highlight={id}&confirm=on")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit angehaktem Kästchen wird gelöscht und weitergeleitet");
    assert!(location.unwrap().contains("notice=deleted"), "Hinweis nach dem Löschen");

    let (status, _) = server.post_form("/literatur/act", &format!("action=highlight&title={title}&text=+")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "leerer Text");
    let (status, _) = server.post_form("/literatur/act", "action=nonsense").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unbekannte Aktion");
}

#[tokio::test]
async fn citing_without_a_section_is_refused_and_creates_nothing() {
    let server = common::open().await;
    let (title, quote) = luther(&server).await;
    let before: Vec<Value> = {
        let work = server.rpc("work.get", json!({})).await;
        work["result"]["work"]["nodes"].as_array().unwrap().iter().map(|n| n["revisions"].clone()).collect()
    };
    for form in [
        format!("action=cite&work=1&title={title}&quote={quote}&start=4&end=9&text=Liebe"),
        format!("action=cite&work=1&node=&title={title}&text=Liebe"),
        format!("action=cite&work=1&node=999&title={title}&text=Liebe"),
        // Die Stelle allein genügt nicht mehr (Vertrag Runde 5): Ohne Schlüssel kein Zitat.
        format!("action=cite&work=1&node=4&title={title}&text=Liebe"),
        format!("action=cite&work=1&node=4&key=&title={title}&text=Liebe"),
    ] {
        let request = Request::post("/literatur/act")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.clone()))
            .unwrap();
        let (status, _, body) = server.send(request).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{form}: Zitieren ohne Abschnitt wird abgelehnt");
        assert!(body.contains("Abschnitt fehlt."), "{form}: deutsche Meldung: {body}");
    }
    let (status, gone) =
        server.post_form("/literatur/act", &format!("action=cite&work=1&key=99999&title={title}&text=Liebe")).await;
    assert_eq!(status, StatusCode::CONFLICT, "ein Schlüssel, den es nicht gibt, wird abgelehnt: {gone:?}");
    let after: Vec<Value> = {
        let work = server.rpc("work.get", json!({})).await;
        work["result"]["work"]["nodes"].as_array().unwrap().iter().map(|n| n["revisions"].clone()).collect()
    };
    assert_eq!(after, before, "kein Vorschlag angelegt, auch nicht in Abschnitt 0");
    let list = server.rpc("highlight.list", json!({})).await;
    assert_eq!(
        list["result"]["highlights"].as_array().map(Vec::len),
        Some(0),
        "auch keine halb angelegte Hervorhebung: {list}"
    );
}

#[tokio::test]
async fn a_highlight_that_differs_only_in_whitespace_is_accepted_over_rpc() {
    let server = common::open().await;
    let (title, _) = luther(&server).await;
    let quote_text = "Die Liebe ist\nlangmütig  und\u{a0}freundlich.";
    let quote =
        baruch_library::add_quote(server.state.store.pool(), title, quote_text, "1. Kor 13,4", "").await.unwrap();
    let h = add(
        &server,
        json!({ "title": title, "quote": quote, "start": 4, "end": 39, "text": "Liebe ist langmütig und freundlich",
                "kind": "core" }),
    )
    .await;
    assert_eq!(
        h["text"], "Liebe ist\nlangmütig  und\u{a0}freundlich",
        "gespeichert ist der genaue Zitatausschnitt: {h}"
    );
    let other = server
        .rpc(
            "highlight.add",
            json!({ "title": title, "quote": quote, "start": 4, "end": 39, "text": "Liebe ist geduldig und freundlich",
                    "kind": "core" }),
        )
        .await;
    assert_eq!(other["error"]["code"], -32602, "inhaltlich anderer Text bleibt TextMismatch: {other}");
}

#[test]
fn the_context_menu_maps_the_selection_by_dom_ranges() {
    let script = include_str!("../web/src/literature.ts");
    assert!(script.contains("text: range.toString(),"), "Text wörtlich aus dem Bereich, nicht selection.toString()");
    assert!(
        script.contains("const end = offset(range.endContainer, range.endOffset);"),
        "Ende aus dem DOM-Bereich statt Anfang + Länge der zusammengefassten Auswahl"
    );
}

#[tokio::test]
async fn a_literature_form_from_a_foreign_origin_is_refused() {
    let server = common::open().await;
    let (title, _) = luther(&server).await;
    let request = Request::post("/literatur/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("action=highlight&title={title}&text=fremd&kind=core")))
        .unwrap();
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Seite darf keine Hervorhebung anlegen");
    let list = server.rpc("highlight.list", json!({})).await;
    assert_eq!(list["result"]["highlights"].as_array().map(Vec::len), Some(0), "nichts angelegt: {list}");
}

#[tokio::test]
async fn the_widgets_are_registered_and_shipped() {
    let script = |name: &str| {
        baruch_server::SCRIPTS
            .iter()
            .find(|(file, _)| *file == name)
            .map(|(_, body)| *body)
            .unwrap_or_else(|| panic!("{name} fehlt in SCRIPTS"))
    };
    let literature = script("literature.js");
    assert!(literature.contains("register(\"literature\""), "Widget literature registriert");
    assert!(literature.contains("register(\"source\""), "Widget source (Kontextmenü) registriert");
    assert!(literature.contains("showModal"), "Popup als modaler <dialog>");
    assert!(literature.contains("\"literature.render\""), "Popup lädt über literature.render");
    assert!(literature.contains("event.altKey") && literature.contains("!== \"l\""), "Alt+L öffnet das Popup");
    assert!(!literature.contains("STUB:"), "Runde 15: kein Stub mehr im Kontextmenü");
    assert!(literature.contains("\"literature.to_persona\""), "An Persona geben über RPC literature.to_persona");
    assert!(script("main.js").contains("import \"./literature.js\""), "main.js lädt das Widget");
    let server = common::open().await;
    let (status, _) = server.get("/static/js/literature.js").await;
    assert_eq!(status, StatusCode::OK, "Skript wird ausgeliefert");
}

// ---- Zitieren über den festen Schlüssel (Vertrag Runde 5) --------------------------------------

#[tokio::test]
async fn the_source_page_carries_the_key_and_citing_from_an_old_tab_lands_in_the_right_section() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let work = garden.work;
    let (title, quote) = luther(&server).await;
    let summer = key_of(&server, work, 3).await;
    // Die Dokumentansicht wurde geöffnet, als „Sommer“ an Stelle 3 stand.
    let (status, page) = server.get(&format!("/quelle?title={title}&quote={quote}&work={work}&node=3")).await;
    assert_eq!(status, StatusCode::OK, "Dokumentansicht lädt: {page}");
    assert!(
        page.contains(&format!("data-key=\"{summer}\"")),
        "die Seite trägt den Schlüssel für das Kontextmenü: {page}"
    );
    let form = between(&page, "data-source-form", "</form>");
    assert!(
        form.contains(&format!("name=\"key\" value=\"{summer}\"")),
        "das Formular für markierten Text schickt den Schlüssel mit: {form}"
    );
    let highlight =
        add(&server, json!({ "title": title, "quote": quote, "start": 4, "end": 9, "text": "Liebe", "kind": "quote" }))
            .await;
    let (_, page) =
        server.get(&format!("/quelle?title={title}&highlight={}&work={work}&node=3", highlight["id"])).await;
    let marks = between(&page, "<ol class=\"highlights\">", "</ol>");
    let cite_form = between(marks, "value=\"cite\"", "</form>");
    assert!(
        cite_form.contains(&format!("name=\"key\" value=\"{summer}\"")),
        "„Als Zitat“ schickt den Schlüssel mit: {cite_form}"
    );

    // In einem anderen Tab stellt der Mensch um: „Sommer“ nach vorn, an Stelle 3 steht jetzt „Tiefe“.
    let moved = server.rpc("outline.move", json!({ "work": work, "node": 3, "target": 0, "place": "before" })).await;
    assert!(moved.get("error").is_none(), "Umstellen gelingt: {moved}");
    let (status, location) = server
        .post_form(
            "/literatur/act",
            &format!("action=cite&work={work}&node=3&key={summer}&title={title}&highlight={}", highlight["id"]),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Zitieren aus dem alten Tab gelingt");
    assert!(
        location.as_deref().unwrap_or_default().starts_with(&format!("/?work={work}&key={summer}&rev=2")),
        "weiter zu „Sommer“ über seinen Schlüssel (er steht jetzt an Stelle 0): {location:?}"
    );
    let summer_now = server.rpc("node.get", json!({ "work": work, "key": summer })).await;
    assert_eq!(summer_now["result"]["title"], "Sommer", "Testaufbau: der Schlüssel gehört zu „Sommer“: {summer_now}");
    assert_eq!(
        summer_now["result"]["revisions"][1]["status"], "proposed",
        "der Zitat-Vorschlag steht bei „Sommer“: {summer_now}"
    );
    let deep = server.rpc("node.get", json!({ "work": work, "node": 3 })).await;
    assert_eq!(deep["result"]["title"], "Tiefe", "an Stelle 3 steht jetzt „Tiefe“: {deep}");
    assert_eq!(deep["result"]["revisions"].as_array().unwrap().len(), 1, "„Tiefe“ bekam keinen Vorschlag: {deep}");

    // Eine Adresse mit Schlüssel und veralteter Stelle zeigt den Abschnitt des Schlüssels.
    let (_, page) = server.get(&format!("/literatur?work={work}&node=3&key={summer}")).await;
    assert!(
        page.contains(&format!("href=\"/?work={work}&amp;key={summer}\" data-back")),
        "der Schlüssel geht vor der veralteten Stelle (Rückweg zu „Sommer“): {page}"
    );
}

#[test]
fn literature_stubs_name_an_open_round_not_the_finished_stage_b2() {
    // B2 (Personas wirklich) ist mit Runde 10 erledigt; ein Stub muss ehrlich die Runde nennen, die noch
    // aussteht (AGENTS.md „Stub antwortet ehrlich … mit Stufe“).
    let texts = [
        baruch_server::literature::PICK_SECTION,
        baruch_server::literature::NOT_LINKED,
        include_str!("../templates/source.html"),
        include_str!("../web/src/literature.ts"),
    ];
    for text in texts {
        assert!(!text.contains("Stufe B2") && !text.contains("(B2)"), "kein Verweis auf die erledigte Stufe B2");
        assert!(!text.contains("Literatur (Rest)"), "Runde 15: „an Persona geben“ ist umgesetzt, kein Verweis mehr");
    }
    assert!(
        !baruch_server::literature::NOT_LINKED.contains("Noch nicht umgesetzt"),
        "Runde 13: Der Volltext ist umgesetzt; der Hinweis ohne Verknüpfung ist kein Stub"
    );
}
