// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rendered pages: scaffold, scripts, style and the literature in the popup of the header without JavaScript.

use crate::common;

use axum::http::StatusCode;

#[tokio::test]
async fn the_page_escapes_and_serves_scripts_and_style() {
    let server = common::open().await;
    let (status, html) = server.get("/?node=2").await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite lädt");
    assert!(html.contains("data-widget=\"workspace\""), "der Arbeitsbereich ist ein Widget");
    assert!(!baruch_server::SCRIPTS.is_empty(), "keine Widgets übersetzt");
    assert!(baruch_server::SCRIPTS.iter().any(|(name, _)| *name == "library.js"), "Widget library fehlt");
    assert!(baruch_server::SCRIPTS.iter().any(|(name, _)| *name == "literature.js"), "Widget literature fehlt");
    let (status, _) = server.get("/static/js/main.js").await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    let (status, css) = server.get("/static/app.css").await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    assert!(css.contains(".library"), "library.css ist nicht an app.css angehängt");
}

#[tokio::test]
async fn the_literature_list_shows_titles_without_javascript() {
    let server = common::open().await;
    let (status, html) = server.get("/?node=2").await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    assert!(
        html.contains("<h2 class=\"pane-title\" id=\"literature-title\">Literatur</h2>"),
        "Überschrift des Literatur-Popups fehlt"
    );
    assert!(html.contains("data-widget=\"library\""), "Widget-Wurzel fehlt");
    assert!(html.contains("class=\"library-search\""), "Suchformular fehlt");
    assert!(html.contains("<details class=\"library-title\""), "keine Titel in der Liste");
    assert!(html.contains("Lutherbibel 1912"), "Beispieltitel fehlt");
    assert!(!html.contains("Noch keine Titel."), "Beispielbibliothek ist leer");
    assert!(!html.contains("(folgt)"), "alter Platzhalter steht noch da");
    assert!(html.contains("name=\"node\" value=\"2\""), "Suche behält den Abschnitt");
}

#[tokio::test]
async fn searching_from_the_literature_column_lists_work_and_library_hits() {
    let server = common::open().await;
    let (status, html) = server.get("/?node=2&lq=langm%C3%BCtig").await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    assert!(html.contains("Treffer für „langmütig“"), "Trefferliste fehlt: {html}");
    assert!(html.contains("data-kind=\"revision\""), "kein Treffer im Werk");
    assert!(html.contains("data-kind=\"quote\""), "kein Treffer in der Bibliothek");
    // The link is literally in the template, only values are inserted.
    let k = key_at(&server, 1, 4).await;
    assert!(
        html.contains(&format!("href=\"/?work=1&amp;key={k}&amp;rev=")),
        "Link auf 2.1 Erste Anzeichen (über den Schlüssel) fehlt: {html}"
    );
    assert!(html.contains("href=\"/quelle?title="), "Bibliotheks-Treffer öffnet die Dokumentansicht: {html}");

    let (_, none) = server.get("/?lq=Quargelbrot").await;
    assert!(none.contains("Keine Treffer."), "Leerfall fehlt");
}

/// `href` of the first work hit (HTML escaping of `&` undone).
fn first_work_hit_link(html: &str) -> String {
    let start = html
        .find("<li class=\"library-hit\" data-kind=\"revision\"")
        .unwrap_or_else(|| panic!("kein Werk-Treffer: {html}"));
    let rest = &html[start..];
    let href = rest.find("href=\"").map(|i| &rest[i + 6..]).unwrap();
    href[..href.find('"').unwrap()].replace("&amp;", "&")
}

#[tokio::test]
async fn a_hit_link_keeps_work_and_query_so_the_hit_list_stays() {
    let server = common::open().await;
    let (_, html) = server.get("/?node=2&lq=langm%C3%BCtig").await;
    let link = first_work_hit_link(&html);
    let k = key_at(&server, 1, 4).await;
    assert!(
        link.starts_with(&format!("/?work=1&key={k}&rev=")),
        "Link trägt Werk, Schlüssel des Abschnitts und Revision: {link}"
    );
    assert!(
        link.ends_with("&lq=langm%C3%BCtig#fundstelle"),
        "Link trägt die Suche (urlencode) und den Anker der Fundstelle: {link}"
    );
    // Follow the link: the page shows the section and the same hit list.
    let (status, page) = server.get(&link).await;
    assert_eq!(status, StatusCode::OK, "Treffer-Link führt auf eine Fehlerseite: {page}");
    assert!(page.contains("Treffer für „langmütig“"), "Trefferliste nach dem Klick weg: {page}");
    assert!(page.contains("value=\"langmütig\""), "Suchfeld nach dem Klick leer");
}

#[tokio::test]
async fn hit_titles_are_open_and_hits_are_marked_without_javascript() {
    let server = common::open().await;
    let (_, html) = server.get("/?node=2&lq=langm%C3%BCtig").await;
    assert!(html.contains("<mark>"), "Treffer im Ausschnitt nicht als <mark> ausgezeichnet: {html}");
    assert!(!html.contains("[langmütig]"), "alte Klammermarken stehen noch da");
    let start = html
        .find("<li class=\"library-hit\" data-kind=\"quote\"")
        .unwrap_or_else(|| panic!("kein Zitat-Treffer: {html}"));
    let key = html[start..].split("Zitat · @").nth(1).and_then(|s| s.split('<').next()).unwrap().to_owned();
    assert!(
        html.contains(&format!("id=\"lit-{key}\" data-key=\"{key}\" open>")),
        "Titel {key} mit Zitat-Treffer ist nicht aufgeklappt: {html}"
    );
    let (_, plain) = server.get("/?node=2").await;
    let start = plain.find("data-widget=\"library\"").expect("Literatur fehlt");
    let section = &plain[start..start + plain[start..].find("</section>").unwrap()];
    assert!(!section.contains(" open>"), "ohne Suche bleibt jeder Titel zugeklappt: {section}");
}

#[tokio::test]
async fn markup_in_a_revision_is_escaped_in_the_hit_snippet() {
    let server = common::open().await;
    let body = "== Erste Anzeichen\n\n<script>alert(1)</script> Quargelbrot #emph[Quargelbrot]\n";
    let saved = server.rpc("revision.save", serde_json::json!({ "node": 4, "body": body })).await;
    assert!(saved.get("error").is_none(), "Speichern scheiterte: {saved}");
    let (status, html) = server.get("/?node=4&lq=Quargelbrot").await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    assert!(!html.contains("<script>alert"), "Revisionstext darf nicht als HTML durchkommen: {html}");
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "maskiertes <script> fehlt: {html}");
    assert!(html.contains("#emph[<mark>Quargelbrot</mark>]"), "Typst-Klammern bleiben, Treffer markiert: {html}");
}

#[tokio::test]
async fn a_query_with_nul_shows_no_hits_instead_of_an_error_page() {
    let server = common::open().await;
    for uri in ["/?lq=%00", "/?node=2&lq=%01%00%7F"] {
        let (status, html) = server.get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: NUL darf keine Fehlerseite ergeben: {html}");
        assert!(html.contains("Keine Treffer."), "{uri}: leere Trefferliste erwartet: {html}");
    }
    let (status, html) = server.get("/?lq=langm%C3%BCtig%00").await;
    assert_eq!(status, StatusCode::OK, "NUL am Wortende: {html}");
    assert!(html.contains("data-kind=\"revision\""), "Wort vor dem NUL trifft weiter: {html}");
}

#[tokio::test]
async fn the_tree_numbers_parts_roman_and_leaves_the_preface_without_number() {
    let server = common::open().await;
    let (status, html) = server.get("/?node=2").await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    for (number, title) in [
        ("I", "Teil I – Erkennen"),
        ("1", "Einleitung"),
        ("2.1", "Erste Anzeichen"),
        ("II", "Teil II – Lösen"),
        ("4.1", "Der Text"),
    ] {
        assert!(
            html.contains(&format!("<span class=\"num\">{number}</span>{title}")),
            "{number} {title} fehlt in der Gliederung: {html}"
        );
    }
    assert!(html.contains("<span class=\"tree-label\">Vorwort</span>"), "Vorwort ohne Nummer");
    assert!(html.contains("data-level=\"0\" data-title=\"Teil I – Erkennen\""), "Teil auf Ebene 0");
    assert!(html.contains("title=\"Kapitel\""), "Ebene als Hinweis an der Zeile");
    assert!(!html.contains("id=\"o-left-1\""), "Teile lassen sich nicht umstufen");
    assert!(
        html.contains("id=\"o-down-1\"") && html.contains("name=\"target\" value=\"6\""),
        "Teil I ↓ hinter Teil II"
    );
}

#[tokio::test]
async fn an_escaped_heading_shows_its_resolved_title() {
    let server = common::open().await;
    let report = server.import("#set document(title: \"Masken\")\n= Anna\\'s \\#1\n\nText.\n", "masken.typ").await;
    let (status, html) = server.get(&format!("/?work={}&node=0", report.work)).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("<span class=\"num\">1</span>Anna&#x27;s #1</span>"), "Titel aufgelöst und maskiert: {html}");
    assert!(html.contains("= Anna\\&#x27;s \\#1"), "die Überschriftszeile im Editor bleibt wörtlich: {html}");
}

/// Fixed key (`Node::key`) of the section at position `seq`: generated addresses carry it instead of the position
/// (`/?work=W&key=K`, contract Round 8).
async fn key_at(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}
