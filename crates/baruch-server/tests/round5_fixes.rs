// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nacharbeit nach Runde 4 (Vertrag Runde 5, Aufgabe r4): Größengrenze von `work.import` über RPC,
//! Schutzzweige von `works::move_to_trash`, keine neue Saat nach dem Löschen aller Werke. Echter
//! Datenordner im Temp-Ordner.

use crate::common;

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::import::MAX_SOURCE_BYTES;
use baruch_server::rpc;
use baruch_server::works::{self, RPC_BODY_LIMIT, TRASH_DIR, move_to_trash};
use serde_json::json;

const DATE: &str = "2026-11-05";

// ---- work.import über RPC: bis 16 MiB Text -----------------------------------------------------

/// Typst-Text mit genau `len` Bytes: eine Überschrift, dann Zeilen aus wenigen Wörtern (der
/// Volltextindex bleibt klein), am Ende mit Leerzeichen aufgefüllt.
fn typst_of_size(len: usize) -> String {
    let mut text = String::from("= Das große Getriebe\n\n");
    let line = "Das Zahnrad dreht sich langsam.\n";
    while text.len() + line.len() <= len {
        text.push_str(line);
    }
    text.push_str(&" ".repeat(len - text.len()));
    assert_eq!(text.len(), len, "Testaufbau: genau {len} Bytes");
    text
}

#[test]
fn the_rpc_body_limit_leaves_room_for_a_fully_escaped_import() {
    assert_eq!(MAX_SOURCE_BYTES, 16 * 1024 * 1024, "der Import nimmt höchstens 16 MiB Text");
    assert_eq!(
        RPC_BODY_LIMIT,
        2 * 16 * 1024 * 1024 + 64 * 1024,
        "JSON-Maskierung kann den Text verdoppeln (\\n, \\\", \\\\), dazu 64 KiB Rahmen"
    );
    let worst = "\"\\\n".repeat(1000);
    let escaped = serde_json::to_string(&worst).unwrap();
    assert!(
        escaped.len() <= 2 * worst.len() + 2,
        "selbst lauter Sonderzeichen wird höchstens doppelt so lang: {} → {}",
        worst.len(),
        escaped.len()
    );
}

#[tokio::test]
async fn work_import_over_rpc_takes_16_mib_and_refuses_one_byte_more() {
    let server = common::open().await;
    let exact = typst_of_size(MAX_SOURCE_BYTES as usize);
    let answer = works::call(&server.state, "work.import", json!({ "file_name": "getriebe.typ", "source": exact }))
        .await
        .unwrap_or_else(|e| panic!("16 MiB werden angenommen: {e}"));
    let work = answer["work"].as_i64().expect("Antwort nennt das neue Werk");
    assert_eq!(answer["nodes"], 1, "ein Kapitel angelegt: {}", answer["slug"]);
    let exported =
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(work).await.unwrap()).into_owned();
    assert_eq!(exported.len(), MAX_SOURCE_BYTES as usize, "der ganze Text ist angekommen");

    let over = typst_of_size(MAX_SOURCE_BYTES as usize + 1);
    let err = works::call(&server.state, "work.import", json!({ "file_name": "groesser.typ", "source": over }))
        .await
        .expect_err("ein Byte mehr wird abgelehnt");
    assert_eq!(err.code, rpc::INVALID_PARAMS, "zu groß → -32602: {}", err.message);
    assert!(err.message.contains("größer als 16 MiB"), "deutsche Meldung nennt die Grenze: {}", err.message);
    assert_eq!(server.state.store.works().await.unwrap().len(), 2, "nur das Beispielwerk und der 16-MiB-Import");
}

// ---- move_to_trash: jeder Schutzzweig ----------------------------------------------------------

fn mirror_with_file(data: &Path, slug: &str) {
    let dir = data.join("works").join(slug);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("main.typ"), "= Probe\n").unwrap();
}

#[test]
fn without_a_mirror_folder_nothing_is_moved() {
    let data = tempfile::tempdir().unwrap();
    assert_eq!(move_to_trash(data.path(), "nie-gespiegelt", DATE), Ok(None), "kein Ordner → Ok(None)");
    assert!(!data.path().join("works").join(TRASH_DIR).exists(), "ohne Ordner entsteht auch kein Archiv");
}

#[test]
fn a_taken_archive_name_gets_dash_two_then_dash_three() {
    let data = tempfile::tempdir().unwrap();
    for (expected, round) in [("probe-2026-11-05", 1), ("probe-2026-11-05-2", 2), ("probe-2026-11-05-3", 3)] {
        mirror_with_file(data.path(), "probe");
        let moved = move_to_trash(data.path(), "probe", DATE).unwrap();
        assert_eq!(
            moved.as_deref(),
            Some(format!("works/{TRASH_DIR}/{expected}").as_str()),
            "Runde {round}: freier Name im Archiv"
        );
        let archived = data.path().join("works").join(TRASH_DIR).join(expected).join("main.typ");
        assert!(archived.is_file(), "Runde {round}: der Inhalt liegt im Archiv: {}", archived.display());
        assert!(!data.path().join("works/probe").exists(), "Runde {round}: der Spiegelordner ist weg");
    }
}

#[cfg(unix)]
#[test]
fn a_trash_folder_that_is_a_symlink_is_refused_and_the_mirror_stays() {
    let data = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    mirror_with_file(data.path(), "probe");
    std::os::unix::fs::symlink(elsewhere.path(), data.path().join("works").join(TRASH_DIR)).unwrap();
    let err = move_to_trash(data.path(), "probe", DATE).expect_err(".geloescht als Verknüpfung");
    assert!(err.contains("symbolische Verknüpfung"), "Meldung nennt den Grund: {err}");
    assert!(data.path().join("works/probe/main.typ").is_file(), "der Spiegel bleibt, wo er ist");
    assert_eq!(std::fs::read_dir(elsewhere.path()).unwrap().count(), 0, "nichts landet am Ziel der Verknüpfung");
}

#[cfg(unix)]
#[test]
fn a_mirror_or_works_folder_that_is_a_symlink_is_refused() {
    let data = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("main.typ"), "fremd").unwrap();
    std::fs::create_dir_all(data.path().join("works")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), data.path().join("works/probe")).unwrap();
    let err = move_to_trash(data.path(), "probe", DATE).expect_err("Spiegel als Verknüpfung");
    assert!(err.contains("symbolische Verknüpfung"), "Meldung nennt den Grund: {err}");
    assert!(elsewhere.path().join("main.typ").is_file(), "das Ziel der Verknüpfung bleibt unberührt");
    assert!(!data.path().join("works").join(TRASH_DIR).exists(), "kein Archiv angelegt");

    let other = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), other.path().join("works")).unwrap();
    let err = move_to_trash(other.path(), "probe", DATE).expect_err("works/ als Verknüpfung");
    assert!(err.contains("symbolische Verknüpfung"), "auch works/ selbst darf keine Verknüpfung sein: {err}");
}

#[test]
fn an_invalid_slug_is_refused_before_touching_the_disk() {
    let data = tempfile::tempdir().unwrap();
    mirror_with_file(data.path(), "probe");
    for slug in ["", "../probe", "Probe", "probe/../x", "-probe", "probe-", "a--b", &"x".repeat(49)] {
        let err = move_to_trash(data.path(), slug, DATE).expect_err("ungültiger slug");
        assert!(err.contains("ungültiger Ordnername"), "{slug:?}: Meldung nennt den Grund: {err}");
    }
    assert!(data.path().join("works/probe/main.typ").is_file(), "kein gültiger Spiegel wurde angefasst");
    assert!(!data.path().join("works").join(TRASH_DIR).exists(), "kein Archiv angelegt");
}

#[tokio::test]
async fn the_delete_page_tells_that_the_folder_name_stays_reserved() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let (status, html) = server.get(&format!("/werke/{}/loeschen", garden.work)).await;
    assert_eq!(status, StatusCode::OK, "die Löschseite lädt ohne JavaScript: {html}");
    assert!(
        html.contains("Der Ordnername <code>das-gartenbuch</code> bleibt danach reserviert"),
        "der Mensch erfährt vorher, dass der slug reserviert bleibt: {html}"
    );
    assert!(html.contains("<code>das-gartenbuch-2</code>"), "und wie ein neues Werk dann heißt: {html}");
}

// ---- Saat: nach dem Löschen aller Werke kein Neuanfang -----------------------------------------

#[tokio::test]
async fn after_deleting_every_work_a_restart_does_not_seed_and_the_page_leads_to_the_list() {
    let server = common::open().await;
    let sample = server.state.work;
    let title = server.state.store.works().await.unwrap()[0].2.clone();
    let deleted = server.rpc("work.delete", json!({ "work": sample, "confirm_title": title })).await;
    assert!(deleted["result"]["next"].is_null(), "danach gibt es kein Werk mehr: {deleted}");

    // Neustart auf demselben Datenordner.
    let state = baruch_server::AppState::open(server.path()).await.expect("Neustart gelingt");
    assert!(state.store.works().await.unwrap().is_empty(), "kein neues Beispielwerk nach dem Neustart");
    let app = baruch_server::app(state);
    let response = tower::ServiceExt::oneshot(app, Request::get("/").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER, "ohne Werke leitet / weiter");
    assert_eq!(
        response.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/werke"),
        "und zwar auf die Werkliste"
    );
}
