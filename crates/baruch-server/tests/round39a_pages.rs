// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39a (integrator): every HTML page of the router in English and in German. The GET routes come from the
//! source text (`common::registered_routes`), so a page added later is walked by itself. Each page is fetched with
//! `Accept-Language: en` and `de`; with a work whose user data is plain ASCII, the English page must carry no raw
//! catalog key and no German letter (umlauts, sharp s, German opening quotes) in its visible text and its text
//! attributes – a German letter there is interface text that did not go through a catalog. The German page carries
//! no raw key either.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::i18n::{self, CATALOGS};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// A small work in plain ASCII: every German letter on an English page is then interface text.
const PLAIN: &str = "#set document(title: \"The Plain Book\", author: \"Ann Example\")\n\n= Spring\n\nThe beds are dug.\n\n\
                     == Sowing\n\nRadish and carrots.\n\n= Summer\n\nWater, water, water.\n";

/// Routes that are not pages of their own (a leading `=` matches the whole pattern, otherwise a part of it): files,
/// downloads, typeset outputs (long, process-wide PDF mark), the reader frames of the EPUB preview and the static
/// files. Each with its reason.
const NOT_PAGES: &[(&str, &str)] = &[
    ("/static/", "static files, no catalog text"),
    ("/datei", "image bytes"),
    ("/vorschau/", "typeset preview files"),
    ("/epub/leser/", "EPUB reader frame: book content in the work language (Round 39b)"),
    (".baruch", "exchange file download"),
    (".xml", "exchange file download"),
    (".md", "mind map download (book content)"),
    (".mm", "mind map download (book content)"),
    (".pdf", "PDF output (book content, typesets)"),
    (".svg", "picture output (book content)"),
    (".xmind", "mind map download (book content)"),
    (".docx", "Word download (book content)"),
    ("/karte/bild", "picture of the mind map (book content)"),
    ("/satz", "typesets the whole work (book content); its page frame is covered by round39a_templates_1/2"),
    ("=/werke/{id}/pdf", "PDF output (book content)"),
    ("=/werke/{id}/epub", "EPUB download (book content)"),
    ("/gliederung/antraege/", "needs an open outline request"),
    ("/bilder/antraege/", "needs an open image request"),
    ("/bilder/pfad", "JSON helper of the image dialog"),
    ("/fassungen/verlauf", "fragment of the revision history, needs a section (covered in round22_fassungen)"),
    ("/ueber/lizenz", "the AGPL licence text, English by nature"),
    ("/quelle/multigpt", "asks MultiGPT (no outside requests in tests)"),
    ("/beenden", "quit page; checked in round39a_templates_3"),
];

/// The known German pieces that stay on English pages this round, each with its reason (contract R39a §3.3, open
/// items of the tasks; listed in `docs/Backlog.md`, "Round 39a – integrated"). Matched as a substring of the text.
const KNOWN_GERMAN: &[(&str, &str)] = &[
    // User data of a new data folder (not interface text): the sample work, the stored personas, the sample Bibles.
    ("Beispielwerk: Leise Abhängigkeit", "user data: the sample work of a new data folder"),
    ("Aufbau, Klarheit, Stil", "user data: stored persona description"),
    ("Quellen und Fußnoten prüfen", "user data: stored persona description"),
    ("Liest als Zielgruppe", "user data: stored persona description"),
    ("Hörer", "user data: stored persona name"),
    ("You are @", "user data: stored persona prompt, quotes the German markers it asks for"),
    ("Schlachter 2000", "user data: name of the sample SWORD module"),
    ("Die Liebe ist langmütig", "user data: Bible wording of the sample library"),
    ("Und stellet euch nicht", "user data: Bible wording of the sample library"),
    ("Nun aber bleibt Glaube", "user data: Bible wording of the sample library"),
    ("Baruch, dem Sohn Nerias", "user data: Bible wording of the sample library"),
    ("Röm 12,2", "user data: reference in the sample library"),
    // Book-output names quoted on purpose (they are what the file or the mirror contains).
    ("Werk gelöscht", "mirror commit text, quoted as it is written (Round 39b)"),
    ("Überschrift 1–3", "Word style names in the exported file (Round 39b)"),
    ("(ü → ue)", "English sentence that shows the transcription of an umlaut"),
    // Labels from other crates: since Round 39b (rest_literals, integration) translated by the server (numbering.rs,
    // typeset.rs ui_labels, cover_layout.rs, findings/mod.rs, epub_preview.rs, epub_check.rs).
    // Only the warning form "<profile name>: <message>" is excused; a bare German reader name (Kindle profiles) on an
    // English page is still found (epub_preview::reader_name must translate it; review finding of Round 39b).
    (
        "über EPUB): ",
        "reader warnings of baruch-epub check.rs (`reader_warnings`, crate messages with the profile name)",
    ),
    // Server modules left open by messages_2 / excluded by messages_1 (reports R39a).
    ("Kein Einbettungsmodell angebunden", "src/rag/**: not converted (messages_2 open)"),
    ("Für dieses Werk nicht gewählt", "src/rag/mod.rs: not converted (messages_2 open)"),
    ("Keine Einbettung angebunden", "src/rag/neighbors.rs: not converted (messages_2 open)"),
    ("Wolke – leer", "src/idea_map/bild/mindmap.rs: SVG shared with the picture export (messages_1 excluded)"),
];

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC error: {answer}");
    &answer["result"]
}

/// All catalog keys (both languages share them).
fn all_keys() -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for (code, _, source) in CATALOGS.iter() {
        if *code == "en" {
            keys.extend(i18n::catalog_entries(source).into_iter().map(|(key, _)| key));
        }
    }
    keys
}

/// Visible text and text attributes of a page: without `<script>`, `<style>`, comments and the widget catalog.
fn visible(html: &str) -> Vec<String> {
    let cut = Regex::new(r"(?s)<script\b.*?</script>|<style\b.*?</style>|<!--.*?-->").unwrap();
    let html = cut.replace_all(html, " ");
    let mut out = Vec::new();
    let attribute = Regex::new(r#"\s(?:title|aria-label|placeholder|alt|label|data-confirm)="([^"]*)""#).unwrap();
    for found in attribute.captures_iter(&html) {
        out.push(found[1].to_owned());
    }
    let tag = Regex::new(r"(?s)<[^>]*>").unwrap();
    for piece in tag.split(&html) {
        let piece = piece.split_whitespace().collect::<Vec<_>>().join(" ");
        if !piece.is_empty() {
            out.push(piece);
        }
    }
    out
}

/// The raw keys left in a page (outside scripts): candidates `a.b(.c)*` that are catalog keys.
fn raw_keys(html: &str, keys: &BTreeSet<String>) -> Vec<String> {
    let candidate = Regex::new(r"[a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+").unwrap();
    let mut found = BTreeSet::new();
    for text in visible(html) {
        for m in candidate.find_iter(&text) {
            if keys.contains(m.as_str()) {
                found.insert(m.as_str().to_owned());
            }
        }
    }
    found.into_iter().collect()
}

/// The pieces of an English page with a German letter, without the known exceptions.
fn german_pieces(html: &str) -> Vec<String> {
    let german = Regex::new("[äöüÄÖÜß„‚]").unwrap();
    visible(html)
        .into_iter()
        .filter(|piece| german.is_match(piece))
        .filter(|piece| !KNOWN_GERMAN.iter().any(|(known, _)| piece.contains(known)))
        .collect()
}

/// The page without the table of MCP clients seen (`/einstellungen/mcp`): the clients live process-wide (`mcp`
/// sessions), so the names other tests of this binary gave their clients (German words) appear there – user
/// data, not interface text.
fn without_seen_clients(html: &str) -> String {
    let title = i18n::t(i18n::Lang::En, "settings.mcp.clients_title", &[]);
    let Some(at) = html.find(&format!(">{title}</h4>")) else { return html.to_owned() };
    match html[at..].find("</table>") {
        Some(end) if html[at..at + end].contains("<table") => format!("{}{}", &html[..at], &html[at + end..]),
        _ => html.to_owned(),
    }
}

/// Fills the placeholders of a route pattern; `None` for a route that is not a page (`NOT_PAGES`).
fn uri_of(pattern: &str, ids: &Ids) -> Option<String> {
    let skipped = |part: &str| match part.strip_prefix('=') {
        Some(exact) => pattern == exact,
        None => pattern.contains(part),
    };
    if NOT_PAGES.iter().any(|(part, _)| skipped(part)) {
        return None;
    }
    let mut uri = pattern.to_owned();
    if uri.starts_with("/einfaelle/{id}") {
        uri = uri.replace("{id}", &ids.idea.to_string());
    }
    uri = uri.replace("{id}", &ids.work.to_string()).replace("{name}", &ids.asset);
    if uri.contains('{') {
        return None;
    }
    let query = format!("work={}&key={}", ids.work, ids.key);
    Some(if uri.contains('?') { format!("{uri}&{query}") } else { format!("{uri}?{query}") })
}

struct Ids {
    work: i64,
    key: i64,
    idea: i64,
    asset: String,
}

async fn plain_work(server: &common::Server) -> Ids {
    let work = server.import(PLAIN, "plain.typ").await.work;
    let nodes = server.rpc("work.get", json!({ "work": work })).await;
    let key = result(&nodes)["work"]["nodes"][1]["key"].as_i64().unwrap();
    let svg =
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "house.svg", "content": svg, "encoding": "svg",
                    "path": "/bilder/house.svg", "caption": "The house", "alt": "A house" }),
        )
        .await;
    let asset = result(&added)["name"].as_str().unwrap().to_owned();
    let idea = server.rpc("idea.create", json!({ "work": work, "key": key, "words": "Arbor", "note": "Roses." })).await;
    let idea = result(&idea)["idea"]["id"].as_i64().unwrap();
    Ids { work, key, idea, asset }
}

/// The pages to walk: every GET route (with the eight settings pages for `/einstellungen/{seite}`).
fn pages(ids: &Ids) -> Vec<String> {
    let mut uris = Vec::new();
    for (method, pattern) in common::registered_routes() {
        if method != "GET" {
            continue;
        }
        if pattern == "/einstellungen/{seite}" {
            for page in ["darstellung", "werk", "assistenten", "verbindungen", "quellen", "mcp", "daten", "konten"] {
                uris.push(format!("/einstellungen/{page}?work={}", ids.work));
            }
            continue;
        }
        if let Some(uri) = uri_of(&pattern, ids) {
            uris.push(uri);
        }
    }
    uris
}

#[tokio::test]
async fn every_page_renders_in_english_and_german_without_raw_keys_and_english_without_german() {
    let server = common::open().await;
    // Only the plain work: the sample work of a new data folder (if any) would bring German user data.
    let ids = plain_work(&server).await;
    let keys = all_keys();
    let uris = pages(&ids);
    assert!(uris.len() > 40, "the router has more than 40 page routes, walked only {}", uris.len());
    let mut problems: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut walked = 0;
    for uri in &uris {
        for lang in ["en", "de"] {
            let request = common::with_lang(Request::get(uri.as_str()).body(Body::empty()).unwrap(), lang);
            let (status, headers, html) = server.send(request).await;
            assert!(!status.is_server_error(), "{uri} in {lang}: no server error, got {status}: {html}");
            let is_html = headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/html"));
            if status != StatusCode::OK || !is_html {
                continue;
            }
            walked += 1;
            for key in raw_keys(&html, &keys) {
                problems.entry(format!("{uri} [{lang}]")).or_default().push(format!("raw key {key}"));
            }
            if lang == "en" {
                for piece in german_pieces(&without_seen_clients(&html)) {
                    problems.entry(format!("{uri} [en]")).or_default().push(format!("German: {piece}"));
                }
            }
        }
    }
    assert!(walked > 60, "most routes are HTML pages in both languages, only {walked} answered with a page");
    let report: Vec<String> =
        problems.iter().map(|(page, list)| format!("{page}\n    {}", list.join("\n    "))).collect();
    assert!(problems.is_empty(), "pages with raw keys or German in English:\n{}", report.join("\n"));
}
