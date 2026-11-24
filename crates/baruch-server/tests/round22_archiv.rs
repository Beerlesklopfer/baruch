// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 22 (Archiv, Besitz befund-rueckfrage): archivierte Werke auflisten und wiederherstellen, Spiegel der Einfälle ohne Werk.
//!
//! Echter Datenordner im Temp-Verzeichnis (SQLite, Git-Spiegel), kein Modell. Das „Inselbuch“ ist frei erfunden.

use axum::http::StatusCode;
use baruch_server::works::TRASH_DIR;
use baruch_store::{IdeaOrigin, NewIdea};
use serde_json::{Value, json};

use crate::common;

const ISLAND: &str = "#set document(title: \"Das Inselbuch\", author: \"Tade Möwe\")\n#set heading(numbering: \"1.\")\n\n\
= Ankunft\n\nDas Postschiff legt nur mittwochs an.\n\n\
== Der Kai\n\nAuf dem Kai stapeln sich Kisten mit Zwieback.\n\n\
= Winter\n\nIm Januar friert die Bucht zu.\n";

const TITLE: &str = "Das Inselbuch";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn current_texts(server: &common::Server, work: i64) -> Vec<String> {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes.iter().map(|n| n.current_index().map(|i| n.revisions[i].body.clone()).unwrap_or_default()).collect()
}

/// Inselbuch anlegen, einen Abschnitt ändern (über RPC wie der Editor) und löschen. Gibt Server, slug, Texte zurück.
async fn deleted_island() -> (common::Server, String, Vec<String>) {
    let server = common::open().await;
    let report = server.import(ISLAND, "inselbuch.typ").await;
    let answer = server
        .rpc(
            "revision.save",
            json!({ "work": report.work, "node": 2, "body": "= Winter\n\nIm Januar friert die Bucht zu. Die Kinder laufen bis zur Tonne.\n" }),
        )
        .await;
    result(&answer);
    let texts = current_texts(&server, report.work).await;
    let answer = server.rpc("work.delete", json!({ "work": report.work, "confirm_title": TITLE })).await;
    assert_eq!(result(&answer)["slug"], report.slug.as_str(), "gelöscht: {answer}");
    (server, report.slug, texts)
}

#[tokio::test]
async fn a_deleted_work_comes_back_from_the_archive_with_the_same_current_texts() {
    let (server, slug, texts) = deleted_island().await;
    assert!(!server.mirror_dir_of(&slug).exists(), "nach dem Löschen liegt der Spiegel im Archiv");

    let archives = server.rpc("work.archives", json!({})).await;
    let list = result(&archives)["archives"].as_array().unwrap().clone();
    let entry = list.iter().find(|a| a["slug"] == slug.as_str()).expect("das gelöschte Werk steht in der Liste");
    assert_eq!(entry["title"], TITLE, "Titel aus work_retired");
    assert_eq!(entry["restorable"], true, "wiederherstellbar: {entry}");
    let folder = entry["folder"].as_str().expect("Archivordner gefunden").to_owned();
    assert!(folder.starts_with(&format!("works/{TRASH_DIR}/{slug}-")), "Archivordner nach Vertrag: {folder}");

    let (status, page) = server.get("/werke/archiv").await;
    assert_eq!(status, StatusCode::OK, "Archivseite lädt: {page}");
    assert!(page.contains(TITLE) && page.contains("Wiederherstellen"), "Archivseite listet das Werk mit Knopf");
    assert!(
        page.contains("action=\"/werke/archiv/wiederherstellen\"")
            && page.contains(&format!("name=\"slug\" value=\"{slug}\""))
            && page.contains("name=\"confirm\""),
        "Formular ohne JavaScript mit Bestätigung: {page}"
    );
    let (_, works) = server.get("/werke").await;
    assert!(works.contains("/werke/archiv"), "die Werkliste verweist auf das Archiv");

    let (status, location) =
        server.post_form("/werke/archiv/wiederherstellen", &format!("slug={slug}&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Wiederherstellen leitet weiter");
    let location = location.expect("Weiterleitung");
    let work: i64 = location.trim_start_matches("/?work=").parse().expect("Weiterleitung auf das Werk");
    assert_eq!(current_texts(&server, work).await, texts, "gleiche aktuelle Texte wie vor dem Löschen");
    let meta = result(&server.rpc("work.meta", json!({ "work": work })).await)["work"].clone();
    assert_eq!((meta["slug"].as_str(), meta["title"].as_str()), (Some(slug.as_str()), Some(TITLE)), "slug und Titel");

    let dir = server.mirror_dir_of(&slug);
    assert!(dir.join("main.typ").is_file(), "der Ordner ist zurück unter works/<slug>");
    assert!(!server.path().join(&folder).exists(), "der Archivordner ist weg (verschoben, nicht kopiert)");
    if let Some(log) = common::git(&dir, &["log", "--format=%s"]) {
        let subjects: Vec<&str> = log.lines().collect();
        assert_eq!(subjects[0], format!("Werk wiederhergestellt: {TITLE}"), "ein Commit „Werk wiederhergestellt“");
        assert!(subjects.contains(&format!("Werk gelöscht: {TITLE}").as_str()), "die alte Historie bleibt: {log}");
        let body = common::git(&dir, &["log", "-1", "--format=%b"]).unwrap();
        assert!(body.contains("Baruch-Action: work-restored"), "Trailer: {body}");
    }
    let archives = server.rpc("work.archives", json!({})).await;
    assert!(
        !result(&archives)["archives"].as_array().unwrap().iter().any(|a| a["slug"] == slug.as_str()),
        "nach dem Wiederherstellen nicht mehr im Archiv"
    );
}

#[tokio::test]
async fn restoring_needs_the_confirmation_and_is_neither_on_act_nor_for_unknown_slugs() {
    let (server, slug, _) = deleted_island().await;
    let (status, location) = server.post_form("/werke/archiv/wiederherstellen", &format!("slug={slug}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Kästchen nichts wiederhergestellt");
    assert!(location.is_none(), "keine Weiterleitung");
    let (status, _) = server.post_form("/act", &format!("method=work.restore&slug={slug}")).await;
    assert_ne!(status, StatusCode::SEE_OTHER, "work.restore gibt es nicht über /act (nur der Mensch, eigene Seite)");
    let unknown = server.rpc("work.restore", json!({ "slug": "gibt-es-nicht" })).await;
    assert_eq!(unknown["error"]["code"], -32000, "unbekannter slug → REFUSED: {unknown}");
    assert!(unknown["error"]["message"].as_str().unwrap().contains("nicht im Archiv"), "deutsche Meldung: {unknown}");
    let archives = server.rpc("work.archives", json!({})).await;
    assert!(
        result(&archives)["archives"].as_array().unwrap().iter().any(|a| a["slug"] == slug.as_str()),
        "das Werk steht weiter im Archiv"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_in_the_archive_folder_is_refused_and_nothing_changes() {
    let (server, slug, _) = deleted_island().await;
    let archives = server.rpc("work.archives", json!({})).await;
    let folder = result(&archives)["archives"][0]["folder"].as_str().unwrap().to_owned();
    let dir = server.path().join(&folder);
    let section = std::fs::read_dir(dir.join("abschnitte")).unwrap().next().unwrap().unwrap().path();
    let outside = server.path().join("fremd.typ");
    std::fs::write(&outside, "= Fremd\n\nNicht aus dem Archiv.\n").unwrap();
    std::fs::remove_file(&section).unwrap();
    std::os::unix::fs::symlink(&outside, &section).unwrap();
    let answer = server.rpc("work.restore", json!({ "slug": slug })).await;
    assert_eq!(answer["error"]["code"], -32000, "symbolische Verknüpfung → REFUSED: {answer}");
    assert!(answer["error"]["message"].as_str().unwrap().contains("symbolische Verknüpfung"), "Grund: {answer}");
    assert!(server.state.store.work_list().await.unwrap().iter().all(|w| w.slug != slug), "kein Werk angelegt");
    assert!(dir.exists(), "der Archivordner bleibt, wo er ist");
}

#[tokio::test]
async fn deleting_a_work_mirrors_its_ideas_as_ideas_without_work_at_once() {
    let server = common::open().await;
    let report = server.import(ISLAND, "inselbuch.typ").await;
    let answer = server
        .rpc(
            "idea.create",
            json!({ "work": report.work, "words": "Leuchtfeuer", "note": "Die Laterne am Kai flackert." }),
        )
        .await;
    result(&answer);
    let ideas = server.path().join("einfaelle").join("einfaelle.md");
    assert!(!ideas.exists(), "vorher gibt es keine Einfälle ohne Werk");
    let answer = server.rpc("work.delete", json!({ "work": report.work, "confirm_title": TITLE })).await;
    assert!(result(&answer)["ideas_commit"].is_string(), "das Löschen führt den Spiegel nach: {answer}");
    let text = std::fs::read_to_string(&ideas).expect("einfaelle.md im Spiegel der Einfälle ohne Werk");
    assert!(text.contains("Leuchtfeuer") && text.contains("Laterne"), "der Einfall steht sofort darin: {text}");
    if let Some(log) = common::git(&server.path().join("einfaelle"), &["log", "-1", "--format=%s%n%b"]) {
        assert!(log.contains("Einfälle ohne Werk nach dem Löschen von „Das Inselbuch“"), "Betreff: {log}");
        assert!(log.contains("Baruch-Action: ideas-synced"), "Trailer: {log}");
    }
}

#[tokio::test]
async fn the_start_catches_up_the_ideas_without_work_and_makes_no_empty_commit() {
    let server = common::open().await;
    let words = vec!["Treibholz".to_owned()];
    let new = NewIdea {
        work: None,
        key: None,
        words: &words,
        note: "Am Strand liegt ein Ruder.",
        quote: "",
        source: "",
        bible: "",
        origin: IdeaOrigin::Human { name: "Autor".into() },
    };
    // Direkt in der Datenbank (ohne Spiegel), wie nach einem Absturz zwischen Datenbank und Spiegel.
    server.state.store.idea_create(&new).await.unwrap();
    let dir = server.path().join("einfaelle");
    assert!(!dir.join("einfaelle.md").exists(), "der Spiegel kennt den Einfall noch nicht");
    baruch_server::mirror::sync_all(&server.state).await;
    let text = std::fs::read_to_string(dir.join("einfaelle.md")).expect("beim Start nachgeführt");
    assert!(text.contains("Treibholz"), "der Einfall steht nach dem Start im Spiegel: {text}");
    if let Some(count) = common::git(&dir, &["rev-list", "--count", "HEAD"]) {
        baruch_server::mirror::sync_all(&server.state).await;
        let again = common::git(&dir, &["rev-list", "--count", "HEAD"]).unwrap();
        assert_eq!(count, again, "ohne Änderung kein leerer Commit");
    }
}

#[tokio::test]
async fn without_ideas_without_work_the_start_creates_no_folder() {
    let server = common::open().await;
    baruch_server::mirror::sync_all(&server.state).await;
    assert!(!server.path().join("einfaelle").exists(), "kein leeres Repo für Einfälle ohne Werk");
}

#[tokio::test]
async fn a_work_without_template_comes_back_without_template() {
    let server = common::open().await;
    let source = "= Ebbe\n\nDas Watt glänzt.\n\n= Flut\n\nDas Wasser kommt leise.\n";
    let report = server.import(source, "gezeiten.typ").await;
    let texts = current_texts(&server, report.work).await;
    let title = report.title.clone();
    result(&server.rpc("work.delete", json!({ "work": report.work, "confirm_title": title })).await);
    let restored = server.rpc("work.restore", json!({ "slug": report.slug })).await;
    let work = result(&restored)["work"].as_i64().expect("wiederhergestellt");
    assert_eq!(current_texts(&server, work).await, texts, "gleiche Texte, auch ohne Werkvorlage");
    let meta = result(&server.rpc("work.meta", json!({ "work": work })).await)["work"].clone();
    assert_eq!(meta["template"], "", "die Kopfzeile „// Baruch: …“ des Spiegels wird keine Werkvorlage: {meta}");
    assert!(!server.mirror_dir_of(&report.slug).join("vorlage.typ").exists(), "weiter ohne vorlage.typ");
}
