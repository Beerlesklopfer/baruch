// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 16c, cue sheet: `notes.get` (cards with time mark from the time keeper, cues by a fixed rule and Bible
//! passages), `notes.pdf`, the page `/werke/{id}/stichwortzettel` (without JavaScript), the download
//! `/werke/{id}/stichwortzettel.pdf` (409/422/503 like `/werke/{id}/pdf`), the menu entry and the command line
//! `export … --stichwort DATEI`. The sermon is freely invented; PDFs only under `common::PDF_SERIAL`.

use crate::common;

use std::process::Command;
use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::pdf::notes::{NotesOptions, write_notes_file};
use baruch_store::TimingSettings;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common::PDF_SERIAL as SERIAL;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// A paragraph whose first sentence is longer than 160 characters.
const LONG: &str = "Wer unterwegs ist und nichts mitnimmt außer dem, was er am Morgen in die Tasche gesteckt hat, \
                    merkt bald, wie schwer ein leerer Magen wiegt und wie leicht ein geteiltes Stück Brot macht, \
                    wenn es jemand anbietet. Danach mehr.";

/// Invented sermon: introduction with a marking of its own, „Das Brot“ with highlights and duplicate Bible passages,
/// „Der Weg“ without both (first sentence per paragraph, more than six paragraphs, one sentence too long).
fn sermon() -> String {
    let mut text = String::from(
        "= Einleitung\n\n// Stichwort: Bäcker um vier Uhr\n// Stichwort: <b>fett</b> & #Raute\n\
         Der Bäcker in unserer Straße steht um vier Uhr auf. Er knetet, bevor wir wach sind.\n\n\
         = Das Brot\n\nJesus sagt: *Ich bin das Brot des Lebens* (Joh 6,35). Brot ist #strong[Gabe, nicht Lohn].\n\n\
         Johannes 6,35 steht da noch einmal, und Mt 6,11 bittet um das tägliche Brot.\n\n= Der Weg\n\n",
    );
    text.push_str(LONG);
    text.push_str("\n\n");
    for i in 1..=7 {
        text.push_str(&format!("Absatz {i} beginnt hier. Und geht weiter.\n\n"));
    }
    text.push_str("Lk 24,30 erzählt vom Brechen des Brots.\n\n= Schluss\n\nGeht hin und teilt. Amen.\n");
    text
}

struct Sermon {
    server: common::Server,
    work: i64,
    slug: String,
}

async fn sermon_work() -> Sermon {
    let server = common::open().await;
    let report = server.import(&sermon(), "brot-am-wegrand.typ").await;
    sqlx::query("UPDATE work SET kind = 'sermon' WHERE id = ?")
        .bind(report.work)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    Sermon { server, work: report.work, slug: report.slug }
}

fn card<'a>(notes: &'a Value, title: &str) -> &'a Value {
    notes["cards"].as_array().unwrap().iter().find(|c| c["title"] == title).unwrap_or_else(|| panic!("Karte {title}"))
}

fn strings(value: &Value) -> Vec<String> {
    value.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()).collect()
}

async fn get_bytes(server: &common::Server, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec();
    (status, headers, bytes)
}

async fn body_text(response: axum::response::Response) -> (StatusCode, String) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

// ---- notes.get --------------------------------------------------------------------------------------

#[tokio::test]
async fn notes_get_applies_the_three_rules_with_limits_and_unique_refs() {
    let s = sermon_work().await;
    let answer = s.server.rpc("notes.get", json!({ "work": s.work })).await;
    let notes = result(&answer);
    assert_eq!(notes["kind"], "sermon", "Werkart der Predigt: {notes}");
    assert_eq!(notes["format_choices"], json!(["a5", "karten"]), "die beiden Formate");
    let titles: Vec<&str> = notes["cards"].as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(
        titles,
        ["Einleitung", "Das Brot", "Der Weg", "Schluss"],
        "je Gliederungspunkt eine Karte, in Reihenfolge"
    );

    let intro = card(notes, "Einleitung");
    assert_eq!(intro["rule"], "marker", "eigene Markierung geht vor: {intro}");
    assert_eq!(
        strings(&intro["points"]),
        ["Bäcker um vier Uhr", "<b>fett</b> & #Raute"],
        "je Kommentarzeile „// Stichwort:“ ein Punkt, wörtlich"
    );

    let bread = card(notes, "Das Brot");
    assert_eq!(bread["rule"], "strong", "ohne Markierung die Hervorhebungen: {bread}");
    assert_eq!(strings(&bread["points"]), ["Ich bin das Brot des Lebens", "Gabe, nicht Lohn"], "*…* und #strong[…]");
    assert_eq!(
        strings(&bread["refs"]),
        ["Joh 6,35", "Mt 6,11"],
        "Joh 6,35 und Johannes 6,35 sind dieselbe Stelle: einheitlich geschrieben und nur einmal"
    );

    let way = card(notes, "Der Weg");
    assert_eq!(way["rule"], "first_sentence", "ohne beides der erste Satz je Absatz: {way}");
    let points = strings(&way["points"]);
    assert_eq!(points.len(), 6, "höchstens sechs Punkte, obwohl der Abschnitt neun Absätze hat: {points:?}");
    assert!(points[0].chars().count() <= 160, "höchstens 160 Zeichen: {}", points[0].chars().count());
    assert!(points[0].ends_with('…') && points[0].starts_with("Wer unterwegs ist"), "gekürzt mit „…“: {}", points[0]);
    assert_eq!(points[1], "Absatz 1 beginnt hier.", "nur der erste Satz des Absatzes");
    assert_eq!(strings(&way["refs"]), ["Lk 24,30"], "Bibelstellen auch aus Absätzen jenseits der sechs Punkte");

    for c in notes["cards"].as_array().unwrap() {
        for field in ["key", "number", "title", "start_clock", "clock", "points", "refs", "rule"] {
            assert!(!c[field].is_null(), "jede Karte hat {field}: {c}");
        }
    }
}

#[tokio::test]
async fn deeply_nested_markup_and_footnotes_do_not_break_the_notes() {
    // Review Round 16: 60 000 levels of #a[…] made the stack overflow (the whole server aborted), and highlights in
    // footnotes or work orders landed as cues on the card.
    let server = common::open().await;
    let deep = format!(
        "= Tiefe\n\nEin Satz zum Anfang.\n\n{}x{}\n\n= Fußnote\n\nDer Hirte sucht. #footnote[Vgl. *Kommentar*]\n\n\
         Dann kommt er heim. #arbeit[*TODO* prüfen]\n",
        "#a[".repeat(60_000),
        "]".repeat(60_000)
    );
    let report = server.import(&deep, "tief.typ").await;
    let answer = server.rpc("notes.get", json!({ "work": report.work })).await;
    let notes = result(&answer);
    assert_eq!(
        strings(&card(notes, "Tiefe")["points"]),
        ["Ein Satz zum Anfang."],
        "tiefe Verschachtelung: kein Absturz, der verschachtelte Rest liefert nichts"
    );
    let footnote = card(notes, "Fußnote");
    assert_eq!(footnote["rule"], "first_sentence", "Hervorhebungen in Fußnote und Arbeitsauftrag zählen nicht");
    assert_eq!(strings(&footnote["points"]), ["Der Hirte sucht.", "Dann kommt er heim."], "die ersten Sätze bleiben");
    let (status, _, page) = get_bytes(&server, &format!("/werke/{}/stichwortzettel", report.work)).await;
    assert_eq!(status, StatusCode::OK, "auch die Seite antwortet");
    assert!(!String::from_utf8_lossy(&page).contains("TODO"), "kein Arbeitsauftrag auf der Vorschau");
}

#[tokio::test]
async fn the_clocks_of_the_notes_are_those_of_the_timer() {
    let s = sermon_work().await;
    let target = TimingSettings { words_per_minute: 90, target_seconds: Some(20 * 60) };
    s.server.state.store.set_timing_settings(s.work, target).await.unwrap();
    let notes = s.server.rpc("notes.get", json!({ "work": s.work })).await;
    let notes = result(&notes);
    let timing = baruch_server::timing::work_timing(&s.server.state, s.work).await.unwrap();
    let timing = baruch_server::timing::timing_json(&timing);
    assert_eq!(notes["total_clock"], timing["clock"], "Gesamtdauer wie im Zeitwächter (work.timing)");
    assert_eq!(notes["target_clock"], "20:00", "Zielzeit als Uhr");
    assert_eq!(notes["target_clock"], timing["target_clock"], "Zielzeit wie im Zeitwächter");
    let sections = timing["sections"].as_array().unwrap();
    let cards = notes["cards"].as_array().unwrap();
    assert_eq!(cards.len(), sections.len(), "je Abschnitt eine Karte und eine Zeit");
    for (c, t) in cards.iter().zip(sections) {
        assert_eq!(c["key"], t["key"], "gleiche Reihenfolge wie work.timing");
        assert_eq!(c["start_clock"], t["start_clock"], "Zeitmarke aus dem Zeitwächter: {c}");
        assert_eq!(c["clock"], t["clock"], "Dauer aus dem Zeitwächter: {c}");
    }
    assert_ne!(cards[1]["start_clock"], "0:00", "der zweite Abschnitt beginnt nach der Einleitung");
}

// ---- notes.pdf and download --------------------------------------------------------------------------

#[tokio::test]
async fn notes_pdf_and_the_download_give_a_named_pdf_attachment() {
    let _serial = SERIAL.lock().await;
    let s = sermon_work().await;
    let answer = s.server.rpc("notes.pdf", json!({ "work": s.work, "format": "karten", "inline": true })).await;
    let r = result(&answer);
    let name = format!("{}-stichwortzettel.pdf", s.slug);
    assert_eq!(r["file_name"], name, "Dateiname <slug>-stichwortzettel.pdf");
    assert_eq!(r["font_pt"], 14, "Karten: Vorgabe 14 pt");
    assert_eq!(r["url"], format!("/werke/{}/stichwortzettel.pdf?format=karten&schrift=14", s.work), "Adresse");
    assert!(r["pages"].as_u64().unwrap() >= 4, "Karten: je Abschnitt mindestens eine Karte: {r}");
    let inline = baruch_store::decode_base64(r["pdf_base64"].as_str().expect("Base64 mit inline")).unwrap();
    assert!(inline.starts_with(b"%PDF"), "inline sind PDF-Bytes");
    assert_eq!(r["sha256"], baruch_render::sha256_hex(&inline), "sha256 gehört zu den Bytes");
    let plain = s.server.rpc("notes.pdf", json!({ "work": s.work })).await;
    assert!(result(&plain).get("pdf_base64").is_none(), "ohne inline keine Bytes in der Antwort");
    assert_eq!(result(&plain)["format"], "a5", "Vorgabe A5");
    assert_eq!(result(&plain)["font_pt"], 16, "A5: Vorgabe 16 pt");

    let (status, headers, bytes) =
        get_bytes(&s.server, &format!("/werke/{}/stichwortzettel.pdf?format=a5&schrift=20", s.work)).await;
    assert_eq!(status, StatusCode::OK, "Herunterladen gelingt");
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf", "MIME-Typ PDF");
    assert_eq!(headers[header::CONTENT_DISPOSITION], format!("attachment; filename=\"{name}\""), "als Anhang");
    assert!(bytes.starts_with(b"%PDF"), "PDF-Bytes beginnen mit %PDF");

    for (params, why) in [
        (json!({ "work": s.work, "format": "a6" }), "unbekanntes Format"),
        (json!({ "work": s.work, "font_pt": 40 }), "Schrift über 28 pt"),
        (json!({ "work": s.work, "font_pt": 13 }), "Schrift unter 14 pt"),
        (json!({ "work": s.work, "inline": "ja" }), "inline nur true/false"),
    ] {
        let answer = s.server.rpc("notes.pdf", params).await;
        assert_eq!(answer["error"]["code"], -32602, "{why} → -32602: {answer}");
    }
    let (status, html) = s.server.get(&format!("/werke/{}/stichwortzettel.pdf?schrift=40", s.work)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "falsche Schriftgröße → 422 als Seite");
    assert!(
        // Round 33: the header is the ribbon (header.band instead of header.topbar)
        html.contains("<header class=\"band\"") && html.contains("von 14 bis 28"),
        "Fehlerseite mit Grund: {html}"
    );
}

#[tokio::test]
async fn busy_typst_errors_and_timeouts_give_409_422_and_503() {
    let _serial = SERIAL.lock().await;
    let s = sermon_work().await;
    let running = baruch_server::pdf::claim(s.work).expect("niemand erzeugt gerade ein PDF für dieses Werk");
    let (status, _, body) = get_bytes(&s.server, &format!("/werke/{}/stichwortzettel.pdf", s.work)).await;
    assert_eq!(status, StatusCode::CONFLICT, "läuft schon → 409");
    assert!(String::from_utf8(body).unwrap().contains("entsteht gerade ein PDF"), "ehrliche Meldung auf der Seite");
    let answer = s.server.rpc("notes.pdf", json!({ "work": s.work })).await;
    assert_eq!(answer["error"]["message"], baruch_server::pdf::BUSY, "über RPC dieselbe Meldung: {answer}");
    drop(running);

    let headers = HeaderMap::new();
    let broken = NotesOptions { append: "\n#panic(\"Probe\")\n".into(), ..NotesOptions::default() };
    let response = baruch_server::pdf::notes::notes_response(&s.server.state, &headers, s.work, &broken).await;
    let (status, html) = body_text(response).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Satzfehler → 422");
    assert!(html.contains("Kein PDF (Stichwortzettel): Der Satz scheitert"), "die Seite sagt, warum: {html}");
    assert!(!html.starts_with("%PDF"), "nie eine halbe Datei");

    let slow = NotesOptions { limit: Duration::ZERO, ..NotesOptions::default() };
    let response = baruch_server::pdf::notes::notes_response(&s.server.state, &headers, s.work, &slow).await;
    let (status, html) = body_text(response).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "Zeit abgelaufen → 503");
    assert!(html.contains("Typst brauchte länger als 0 s"), "die Meldung nennt das Limit: {html}");
    // The timed-out run finishes computing in the background and holds the work for that long.
    let freed = async {
        loop {
            if let Some(claim) = baruch_server::pdf::claim(s.work) {
                break claim;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    drop(tokio::time::timeout(Duration::from_secs(60), freed).await.expect("der Hintergrundlauf endet"));
}

// ---- Page and menu -----------------------------------------------------------------------------------

#[tokio::test]
async fn the_page_offers_format_choice_preview_and_download_without_javascript() {
    let s = sermon_work().await;
    let (status, html) = s.server.get(&format!("/werke/{}/stichwortzettel?format=karten&schrift=18", s.work)).await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt: {html}");
    assert!(
        html.contains(&format!(
            "<form class=\"notes-form\" method=\"get\" action=\"/werke/{}/stichwortzettel\">",
            s.work
        )),
        "Formatwahl als GET-Formular ohne JavaScript: {html}"
    );
    assert!(html.contains("value=\"karten\" checked"), "das gewählte Format ist markiert");
    assert!(html.contains("name=\"schrift\" value=\"18\" min=\"14\" max=\"28\""), "Schriftgröße 14–28 mit Wert");
    assert!(
        html.contains(&format!("href=\"/werke/{}/stichwortzettel.pdf?format=karten&amp;schrift=18\"", s.work)),
        "Link zum PDF mit den gewählten Einstellungen: {html}"
    );
    assert!(html.contains(">PDF herunterladen</a>"), "Knopf „PDF herunterladen“");
    assert!(html.contains("&lt;b&gt;fett&lt;") && !html.contains("<b>fett</b>"), "Stichpunkte maskiert: {html}");
    assert!(html.contains("Ich bin das Brot des Lebens"), "Vorschau der Karten");
    assert!(html.contains("Regel: Hervorhebung") && html.contains("Regel: eigene Markierung"), "Regel auf Deutsch");
    assert!(html.contains("Bibelstellen: Joh 6,35 · Mt 6,11"), "Bibelstellen der Karte");
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Reste von Platzhaltern");
    // Round 33 (ribbon): the entry was in the menu Werk ▾ and is now in the tab Buch (group Ausgabe) – still a link to
    // the page without download.
    let item = format!("<a class=\"btn\" href=\"/werke/{}/stichwortzettel\"", s.work);
    let at = html.find(&item).expect("Eintrag „Stichwortzettel …“ im Reiter Buch");
    let tag = &html[at..at + html[at..].find('>').unwrap()];
    assert!(!tag.contains(" download"), "der Menüeintrag führt zur Seite, kein download-Attribut: {tag}");
    assert!(html[at..].starts_with(&item) && html[at..].contains(">Stichwortzettel …</a>"), "Text des Eintrags");

    let (status, html) = s.server.get(&format!("/werke/{}/stichwortzettel?format=quer", s.work)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unbekanntes Format → 422 mit Seite");
    assert!(html.contains("Format „quer“ gibt es nicht"), "deutsche Meldung: {html}");
    let (status, _) = s.server.get("/werke/999/stichwortzettel").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk");
}

// ---- Command line -----------------------------------------------------------------------------------

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_baruch-server")).args(args).output().expect("baruch-server startet")
}

#[tokio::test]
async fn write_notes_file_writes_never_overwrites_and_leaves_nothing_on_errors() {
    let _serial = SERIAL.lock().await;
    let s = sermon_work().await;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("zettel.pdf");
    let pdf = write_notes_file(&s.server.state, s.work, &target, NotesOptions::default()).await.unwrap();
    let written = std::fs::read(&target).unwrap();
    assert!(written.starts_with(b"%PDF"), "eine PDF-Datei entsteht");
    assert_eq!(written, pdf.bytes, "genau die Bytes des Ergebnisses");
    let again = write_notes_file(&s.server.state, s.work, &target, NotesOptions::default()).await.unwrap_err();
    assert!(again.contains("nichts überschrieben"), "vorhandene Datei bleibt: {again}");
    assert_eq!(std::fs::read(&target).unwrap(), written, "unverändert");
    let failed = dir.path().join("kaputt.pdf");
    let broken = NotesOptions { append: "\n#panic(\"Probe\")\n".into(), ..NotesOptions::default() };
    let error = write_notes_file(&s.server.state, s.work, &failed, broken).await.unwrap_err();
    assert!(error.contains("Der Satz scheitert"), "Satzfehler mit Meldung: {error}");
    assert!(!failed.exists(), "bei einem Satzfehler entsteht keine Datei");
}

#[tokio::test]
async fn the_cli_exports_notes_and_refuses_conflicting_or_existing_targets() {
    let _serial = SERIAL.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("daten");
    let slug = {
        let state = baruch_server::AppState::open(&data).await.unwrap();
        let report = baruch_server::import::import_typst(&state, &sermon(), "brot.typ").await.unwrap();
        state.store.pool().close().await;
        report.slug
    };
    let data_arg = data.to_str().unwrap();
    let target = dir.path().join("karten.pdf");
    let target_arg = target.to_str().unwrap();
    let out =
        cli(&["export", &slug, "--stichwort", target_arg, "--format", "karten", "--schrift", "16", "--data", data_arg]);
    assert!(out.status.success(), "export --stichwort gelingt: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let written = std::fs::read(&target).unwrap();
    assert!(written.starts_with(b"%PDF"), "gültiges PDF geschrieben");
    assert!(
        stdout.starts_with(&format!("Stichwortzettel geschrieben: {target_arg} ("))
            && stdout.contains(" Seiten, ")
            && stdout.contains(&format!("{} Bytes)", written.len())),
        "Ausgabe mit Seiten und Bytes: {stdout}"
    );

    let again = cli(&["export", &slug, "--stichwort", target_arg, "--data", data_arg]);
    assert_eq!(again.status.code(), Some(1), "vorhandene Datei → Exit 1");
    assert!(String::from_utf8_lossy(&again.stderr).contains("nichts überschrieben"), "mit Grund");
    assert_eq!(std::fs::read(&target).unwrap(), written, "die vorhandene Datei bleibt unverändert");

    let other = dir.path().join("neu.pdf");
    let other_arg = other.to_str().unwrap();
    for (extra, why) in [
        (vec!["--pdf", other_arg], "--stichwort und --pdf"),
        (vec!["--out", other_arg], "--stichwort und --out"),
        (vec!["--format", "quer"], "unbekanntes Format"),
        (vec!["--schrift", "50"], "Schrift außerhalb 14–28"),
    ] {
        let mut args = vec!["export", slug.as_str(), "--stichwort", other_arg];
        args.extend(extra);
        args.extend(["--data", data_arg]);
        let out = cli(&args);
        assert_eq!(out.status.code(), Some(2), "{why} → Exit 2: {}", String::from_utf8_lossy(&out.stderr));
    }
    let out = cli(&["export", &slug, "--format", "a5", "--data", data_arg]);
    assert_eq!(out.status.code(), Some(2), "--format ohne --stichwort → Exit 2");
    assert!(!other.exists(), "keiner der abgelehnten Aufrufe schreibt eine Datei");
}
