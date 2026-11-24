// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Import im Server (Vertrag B1.5, Abschnitt 4): neues Werk in SQLite, eigener Git-Spiegel mit
//! Commit „Import aus <datei>“, Duplikatschutz, Export; dazu die Befehle `import` (mit `--title`) und
//! `export` und das Hochladen über `POST /werke/import` (Runde 4, multipart, ohne JavaScript).
//! Echte SQLite-Datei und echtes Git-Repo im Temp-Ordner, Testdatei `tests/data/gartenbuch.typ`.

use crate::common;

use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::rpc;
use serde_json::json;

use common::GARDEN;

#[tokio::test]
async fn importing_creates_a_work_with_its_own_mirror_and_import_commit() {
    let server = common::open().await;
    let report = server.import_garden().await;
    assert_eq!(
        (report.title.as_str(), report.slug.as_str(), report.nodes),
        ("Das Gartenbuch", "das-gartenbuch", 4),
        "Titel aus #set document, vier Überschriften: {report:?}"
    );
    assert!(report.commit.is_some() && report.mirror.is_none(), "Spiegel muss geschrieben sein: {report:?}");

    let dir = server.mirror_dir_of(&report.slug);
    let main =
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(dir.join("main.typ")).unwrap()).into_owned();
    assert!(main.starts_with("// Gartenbuch für die Server-Tests"), "main.typ beginnt mit der Werkvorlage: {main}");
    assert!(
        main.ends_with(
            "#include \"abschnitte/000-fruehling.typ\"\n#include \"abschnitte/001-aussaat.typ\"\n\
        #include \"abschnitte/002-tiefe.typ\"\n#include \"abschnitte/003-sommer.typ\"\n"
        ),
        "nach der Vorlage die Abschnitte in Gliederungsreihenfolge: {main}"
    );
    let template = server.state.store.work_template(report.work).await.unwrap();
    assert!(main.starts_with(&template), "main.typ = Werkvorlage + #include wie bisher: {main}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(dir.join("vorlage.typ")).unwrap()).into_owned(),
        template,
        "vorlage.typ trägt die Werkvorlage, damit #let-Funktionen wie #beet in den Abschnitten gelten"
    );
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(dir.join("abschnitte/001-aussaat.typ")).unwrap())
            .into_owned(),
        "#import \"../vorlage.typ\": *\n== Aussaat\n\n#beet[Radieschen & Möhren.]\n\n",
        "Abschnittsdatei = Import der Vorlage + Revision 1"
    );
    for entry in std::fs::read_dir(dir.join("abschnitte")).unwrap() {
        let text =
            baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(entry.unwrap().path()).unwrap()).into_owned();
        assert!(text.starts_with("#import \"../vorlage.typ\": *\n"), "jede Abschnittsdatei importiert die Vorlage");
    }
    let node = server.rpc("node.get", json!({ "work": report.work, "node": 1 })).await;
    assert_eq!(
        node["result"]["revisions"][0]["body"], "== Aussaat\n\n#beet[Radieschen & Möhren.]\n\n",
        "die Revision in SQLite bleibt ohne Import-Zeile"
    );

    let Some(log) = common::git(&dir, &["log", "--format=%s|%an"]) else {
        eprintln!("git fehlt, Commit-Prüfung übersprungen");
        return;
    };
    assert_eq!(log, "Import aus gartenbuch.typ|Autor\n", "genau ein Commit des Menschen für den Import: {log}");
    assert_eq!(
        common::git(&dir, &["rev-parse", "HEAD"]).unwrap().trim(),
        report.commit.as_deref().unwrap(),
        "gemeldeter Commit ist HEAD"
    );
}

#[tokio::test]
async fn importing_the_same_file_twice_is_refused() {
    let server = common::open().await;
    let first = server.import_garden().await;
    let err = baruch_server::import::import_typst(&server.state, GARDEN, "nochmal.typ").await.unwrap_err();
    assert_eq!(err.code, rpc::REFUSED, "schon importiert ist ein fachlicher Fehler: {err:?}");
    assert!(
        err.message.contains("schon importiert") && err.message.contains(&first.slug),
        "Meldung nennt das vorhandene Werk: {}",
        err.message
    );
    let works = server.rpc("work.list", json!({})).await;
    assert_eq!(works["result"]["works"].as_array().unwrap().len(), 2, "kein drittes Werk: {works}");
}

#[tokio::test]
async fn export_gives_back_the_imported_file_byte_for_byte() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let answer = server.rpc("work.export", json!({ "work": report.work })).await;
    assert_eq!(
        answer["result"]["typst"].as_str().map(|t| baruch_typst::xref::strip_key_labels(t).into_owned()).as_deref(),
        Some(GARDEN),
        "Export = Eingabe bis auf die Sprungmarken (Runde 30a): {answer}"
    );
    let none =
        server.rpc("revision.save", json!({ "work": report.work, "node": 3, "body": "= Sommer\n\nRegen.\n" })).await;
    assert!(none["result"]["commit"].is_string(), "Speichern im importierten Werk committet in dessen Spiegel: {none}");
    let changed = server.rpc("work.export", json!({ "work": report.work })).await;
    assert!(
        baruch_typst::xref::strip_key_labels(changed["result"]["typst"].as_str().unwrap())
            .ends_with("= Sommer\n\nRegen.\n"),
        "Export zeigt den neuen Stand"
    );
}

#[tokio::test]
async fn import_file_refuses_files_that_are_not_utf8_or_have_no_headings() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let latin1 = dir.path().join("alt.typ");
    std::fs::write(&latin1, b"= Gr\xf6\xdfe\n").unwrap();
    let err = baruch_server::import::import_file(&server.state, &latin1).await.unwrap_err();
    assert_eq!(err.code, rpc::INVALID_PARAMS, "kein UTF-8: {err:?}");
    assert!(err.message.contains("UTF-8"), "Meldung nennt den Grund: {}", err.message);

    let plain = dir.path().join("ohne.typ");
    std::fs::write(&plain, "Nur Text.\n").unwrap();
    let err = baruch_server::import::import_file(&server.state, &plain).await.unwrap_err();
    assert_eq!(err.code, rpc::INVALID_PARAMS, "ohne Überschrift: {err:?}");
    assert!(err.message.starts_with("Import: keine Überschrift"), "Meldung nennt den Grund: {}", err.message);

    let missing = baruch_server::import::import_file(&server.state, &dir.path().join("fehlt.typ")).await.unwrap_err();
    assert_eq!(missing.code, rpc::INVALID_PARAMS, "fehlende Datei: {missing:?}");
    assert!(missing.message.contains("lässt sich nicht lesen"), "deutsche Meldung: {}", missing.message);

    let folder = dir.path().join("ordner.typ");
    std::fs::create_dir(&folder).unwrap();
    let err = baruch_server::import::import_file(&server.state, &folder).await.unwrap_err();
    assert!(err.message.ends_with("ist keine Datei"), "ein Ordner mit Endung .typ ist keine Datei: {}", err.message);
}

/// Eine FIFO ohne Schreiber: `File::open` würde blockieren, bis jemand schreibt. Der Import prüft vorher
/// mit `metadata`, dass es eine reguläre Datei ist, und lehnt sofort ab. `mkfifo` aus coreutils legt
/// die FIFO an (ohne zusätzliche Crate); fehlt das Programm, wird der Test übersprungen.
#[cfg(unix)]
#[tokio::test]
async fn importing_a_fifo_does_not_block_and_says_it_is_no_file() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("rohr.typ");
    match Command::new("mkfifo").arg(&fifo).status() {
        Ok(status) if status.success() => {}
        other => {
            eprintln!("mkfifo nicht verfügbar ({other:?}) – FIFO-Test übersprungen");
            return;
        }
    }
    use std::os::unix::fs::FileTypeExt;
    assert!(std::fs::metadata(&fifo).unwrap().file_type().is_fifo(), "Testaufbau: rohr.typ ist eine FIFO");

    let import = baruch_server::import::import_file(&server.state, &fifo);
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), import).await;
    let err = result.expect("Import einer FIFO blockiert (kein Ergebnis nach 10 s)").unwrap_err();
    assert_eq!(err.code, rpc::INVALID_PARAMS, "FIFO ist ein Fehler in den Angaben: {err:?}");
    assert_eq!(err.message, format!("{} ist keine Datei", fifo.display()), "deutsche Meldung nennt den Pfad");
    let works = server.rpc("work.list", json!({})).await;
    assert_eq!(works["result"]["works"].as_array().unwrap().len(), 1, "nichts importiert: {works}");
}

#[tokio::test]
async fn a_work_without_template_keeps_its_mirror_without_vorlage_typ() {
    let server = common::open().await;
    let report = server.import("= Eins\n\nText.\n", "ohne-vorlage.typ").await;
    let dir = server.mirror_dir_of(&report.slug);
    assert!(!dir.join("vorlage.typ").exists(), "ohne Präambel keine vorlage.typ");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(dir.join("abschnitte/000-eins.typ")).unwrap())
            .into_owned(),
        "= Eins\n\nText.\n",
        "ohne Vorlage keine Import-Zeile"
    );
    let sample = server.state.store.work_slug(server.state.work).await.unwrap();
    let sample_dir = server.mirror_dir_of(&sample);
    assert!(sample_dir.join("main.typ").is_file(), "Spiegel des Beispielwerks angelegt");
    assert!(!sample_dir.join("vorlage.typ").exists(), "das Beispielwerk hat keine Vorlage und keine vorlage.typ");
}

#[tokio::test]
async fn import_file_takes_the_name_from_the_path() {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Herbstlaub.typ");
    std::fs::write(&path, "= Blätter\n\nSie fallen.\n").unwrap();
    let report = baruch_server::import::import_file(&server.state, &path).await.unwrap();
    assert_eq!(
        (report.title.as_str(), report.slug.as_str()),
        ("Herbstlaub", "herbstlaub"),
        "ohne #set document Titel aus dem Dateinamen"
    );
    let Some(log) = common::git(&server.mirror_dir_of("herbstlaub"), &["log", "-1", "--format=%s"]) else { return };
    assert_eq!(log, "Import aus Herbstlaub.typ\n", "Commit nennt die Datei");
}

/// Programm `baruch-server` mit Datenordner `data` aufrufen: (Exit-Code, stdout, stderr).
fn cli(data: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_baruch-server"))
        .args(args)
        .arg("--data")
        .arg(data)
        .env_remove("BARUCH_DATA")
        .output()
        .expect("baruch-server lässt sich nicht starten");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn the_help_names_port_8765_and_the_local_data_folder_as_defaults() {
    let data = tempfile::tempdir().unwrap();
    let (code, out, err) = cli(data.path(), &["--help"]);
    assert_eq!(code, 0, "--help endet mit Exit 0: {err}");
    assert!(out.contains("Vorgabe 127.0.0.1:8765"), "Vorgabe-Adresse ist 127.0.0.1:8765: {out}");
    assert_eq!(out.matches("Vorgabe 127.0.0.1:").count(), 1, "genau eine Vorgabe-Adresse, keine alte daneben: {out}");
    assert!(out.contains("%LOCALAPPDATA%"), "Windows nimmt Local statt Roaming: {out}");
}

#[test]
fn the_import_and_export_commands_round_trip_and_refuse_duplicates() {
    let data = tempfile::tempdir().unwrap();
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/gartenbuch.typ");

    let (code, out, err) = cli(data.path(), &["import", file]);
    assert_eq!(code, 0, "Import muss gelingen: {out}\n{err}");
    assert!(out.contains("Werk: Das Gartenbuch"), "Ausgabe nennt das Werk: {out}");
    assert!(out.contains("Ordnername: das-gartenbuch"), "Ausgabe nennt den Ordner: {out}");
    assert!(out.contains("Abschnitte: 4"), "Ausgabe nennt die Abschnitte: {out}");
    assert!(out.contains("Commit: "), "Ausgabe nennt den Commit: {out}");
    assert!(data.path().join("works/das-gartenbuch/main.typ").is_file(), "Spiegelordner angelegt");
    assert!(data.path().join("works/das-gartenbuch/vorlage.typ").is_file(), "Werkvorlage als vorlage.typ im Spiegel");

    let (code, out, err) = cli(data.path(), &["export", "das-gartenbuch"]);
    assert_eq!(code, 0, "Export über den Ordnernamen: {err}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&out),
        GARDEN,
        "Export auf stdout = Eingabe bis auf die Sprungmarken"
    );

    let (code, _, err) = cli(data.path(), &["import", file]);
    assert_eq!(code, 1, "zweiter Import endet mit Exit 1");
    assert!(err.contains("schon importiert"), "Meldung auf stderr: {err}");

    let (code, _, err) = cli(data.path(), &["export", "gibt-es-nicht"]);
    assert_eq!(code, 1, "unbekanntes Werk endet mit Exit 1");
    assert!(err.contains("unbekanntes Werk"), "Meldung auf stderr: {err}");
}

#[test]
fn the_import_command_refuses_a_taken_title_and_takes_another_with_title() {
    let data = tempfile::tempdir().unwrap();
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/gartenbuch.typ");
    let (code, _, err) = cli(data.path(), &["import", file]);
    assert_eq!(code, 0, "erster Import gelingt: {err}");

    // Andere Datei mit demselben Titel in #set document.
    let other = data.path().join("garten-neu.typ");
    std::fs::write(&other, GARDEN.replace("Gießen, gießen, gießen.", "Ernten.")).unwrap();
    let other = other.to_str().unwrap();
    let (code, out, err) = cli(data.path(), &["import", other]);
    assert_eq!(code, 1, "vergebener Titel endet mit Exit 1: {out}");
    assert_eq!(
        err.trim_end(),
        "Es gibt schon ein Werk „Das Gartenbuch“ (das-gartenbuch). Mit --title \"…\" einen anderen Titel wählen.",
        "Meldung auf stderr nennt das Werk und --title"
    );
    let (code, out, err) = cli(data.path(), &["import", other, "--title", "Das Gartenbuch, Band 2"]);
    assert_eq!(code, 0, "mit --title gelingt der Import: {err}");
    assert!(out.contains("Werk: Das Gartenbuch, Band 2"), "Ausgabe nennt den eigenen Titel: {out}");
    assert!(out.contains("Ordnername: das-gartenbuch-band-2"), "slug aus dem eigenen Titel: {out}");

    let (code, _, err) = cli(data.path(), &["import", file, "--title", "Noch ein Titel"]);
    assert_eq!(code, 1, "dieselbe Datei bleibt abgelehnt, auch mit freiem Titel");
    assert!(err.contains("schon importiert"), "Meldung „schon importiert“ geht vor: {err}");
    let (_, help, _) = cli(data.path(), &["--help"]);
    assert!(help.contains("--title"), "Hilfe nennt --title: {help}");
}

// ---- Hochladen über POST /werke/import ---------------------------------------------------------

const BOUNDARY: &str = "baruch-test-grenze";

/// Multipart-Anfrage wie ein Browserformular: Feld `title`, dann Feld `file`.
fn upload(file_name: &str, content: &[u8], title: &str) -> Request<Body> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n{title}\r\n").as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/werke/import")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn uploading_a_typst_file_creates_a_work_and_opens_it() {
    let server = common::open().await;
    let (status, headers, _) = server.send(upload("gartenbuch.typ", GARDEN.as_bytes(), "")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Erfolg leitet weiter");
    let location = headers[header::LOCATION].to_str().unwrap().to_owned();
    let work: i64 =
        location.strip_prefix("/?work=").and_then(|w| w.parse().ok()).expect("Weiterleitung auf /?work=<id>");
    let meta = server.rpc("work.meta", json!({ "work": work })).await;
    assert_eq!(meta["result"]["work"]["title"], "Das Gartenbuch", "leerer Titel: aus #set document: {meta}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(
            server.rpc("work.export", json!({ "work": work })).await["result"]["typst"].as_str().unwrap()
        ),
        GARDEN,
        "Inhalt unverändert"
    );
    let Some(log) = common::git(&server.mirror_dir_of("das-gartenbuch"), &["log", "--format=%s"]) else { return };
    assert_eq!(log, "Import aus gartenbuch.typ\n", "Spiegel-Commit wie beim Import über die Kommandozeile");
}

#[tokio::test]
async fn uploads_that_are_no_typst_too_large_or_taken_are_refused_with_a_page() {
    let server = common::open().await;
    let (status, _, html) = server.send(upload("notizen.txt", b"= Eins\n", "")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "andere Endung → 422");
    assert!(html.contains("nur .typ lässt sich importieren"), "Fehlerseite nennt den Grund: {html}");
    assert!(html.contains("enctype=\"multipart/form-data\""), "Fehlerseite zeigt das Formular wieder");

    let mut big = b"= Riesig\n".to_vec();
    big.resize(16 * 1024 * 1024 + 1, b'x');
    let (status, _, html) = server.send(upload("riesig.typ", &big, "")).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "eine Datei über 16 MiB → 413");
    assert!(html.contains("größer als 16 MiB"), "deutsche Meldung: {html}");
    big.resize(17 * 1024 * 1024, b'x');
    let (status, _, _) = server.send(upload("riesig.typ", &big, "")).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "eine Anfrage über der Grenze von 16 MiB + 64 KiB → 413");

    server.import_garden().await;
    let other = GARDEN.replace("Gießen, gießen, gießen.", "Ernten.");
    let (status, _, html) = server.send(upload("garten-neu.typ", other.as_bytes(), "")).await;
    assert_eq!(status, StatusCode::CONFLICT, "doppelter Titel → 409");
    assert!(
        html.contains("Es gibt schon ein Werk „Das Gartenbuch“ (das-gartenbuch)."),
        "Hinweis nennt das Werk: {html}"
    );
    assert!(html.contains("anderen Titel"), "Hinweis, einen anderen Titel zu wählen: {html}");
    let (status, _, html) = server.send(upload("kopie.typ", GARDEN.as_bytes(), "Ganz neu")).await;
    assert_eq!(status, StatusCode::CONFLICT, "dieselbe Datei noch einmal → 409");
    assert!(html.contains("schon importiert"), "Meldung „schon importiert“: {html}");
    let (status, headers, _) = server.send(upload("garten-neu.typ", other.as_bytes(), "Gartenbuch, Band 2")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit eigenem Titel gelingt der Upload: {headers:?}");
    let works = server.rpc("work.list", json!({})).await;
    assert_eq!(works["result"]["works"].as_array().unwrap().len(), 3, "nur die gelungenen Importe sind Werke: {works}");
}

#[tokio::test]
async fn an_upload_from_a_foreign_page_is_refused() {
    let server = common::open().await;
    let mut request = upload("gartenbuch.typ", GARDEN.as_bytes(), "");
    request.headers_mut().insert(header::HOST, "127.0.0.1:8765".parse().unwrap());
    request.headers_mut().insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Origin → 403");
    let works = server.rpc("work.list", json!({})).await;
    assert_eq!(works["result"]["works"].as_array().unwrap().len(), 1, "nichts importiert: {works}");
}

// ---- Bilder beim Import und Export --out (Vertrag Runde 6, Abschnitte 4d und 4e) ---------------

/// Frei erfundenes SVG.
const HOUSE: &str = "<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\">\
                     <path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";

/// CRC-32 (PNG), bitweise.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Gültiges PNG mit einem grauen Pixel, im Test erzeugt (keine Binärdatei im Repo).
fn tiny_png(gray: u8) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let raw = [0u8, gray];
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    let mut zlib = vec![0x78, 0x01, 0x01, 0x02, 0x00, 0xFD, 0xFF];
    zlib.extend_from_slice(&raw);
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Erfundenes Buch, das sechs Bilder nennt: zwei liegen im Ordner, die übrigen sind außerhalb, ein
/// Symlink hinaus, fehlend oder kein Bild.
const HILLS: &str = "// Hügelbuch für die Bild-Tests\n#set document(title: \"Das Hügelbuch\", author: \"Ida Feder\")\n\
                     #let bild(p) = figure(image(p))\n\n= Der Hang\n\n#figure(image(\"/bilder/haus.svg\"), caption: [Das Haus])\n\n\
                     Text am Hang.\n\n== Die Wiese\n\n#image(\"bilder/punkt.png\", width: 2cm)\n\
                     #image(\"../draussen.svg\")\n#image(\"bilder/link.svg\")\n#image(\"bilder/fehlt.png\")\n\
                     #image(\"bilder/notiz.png\")\n// #image(\"bilder/kommentar.png\")\n";

/// Ordner `<tmp>/projekt` mit `huegel.typ` und den Bildern; daneben `draussen.svg`.
fn hills_folder() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("projekt");
    std::fs::create_dir_all(project.join("bilder")).unwrap();
    std::fs::write(project.join("huegel.typ"), HILLS).unwrap();
    std::fs::write(project.join("bilder/haus.svg"), HOUSE).unwrap();
    std::fs::write(project.join("bilder/punkt.png"), tiny_png(128)).unwrap();
    std::fs::write(project.join("bilder/notiz.png"), "Nur eine Notiz, kein Bild.\n").unwrap();
    std::fs::write(project.join("bilder/kommentar.png"), tiny_png(1)).unwrap();
    std::fs::write(tmp.path().join("draussen.svg"), HOUSE).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(tmp.path().join("draussen.svg"), project.join("bilder/link.svg")).unwrap();
    (tmp, project)
}

#[tokio::test]
async fn importing_a_file_reads_its_images_from_its_folder_and_skips_the_rest_with_a_reason() {
    let server = common::open().await;
    let (_tmp, project) = hills_folder();
    let report = baruch_server::import::import_file(&server.state, &project.join("huegel.typ")).await.unwrap();
    assert_eq!(report.assets, ["haus.svg", "punkt.png"], "die zwei Bilder im Ordner sind angelegt: {report:?}");
    let reasons: Vec<(&str, &str)> = report.skipped.iter().map(|s| (s.path.as_str(), s.reason.as_str())).collect();
    let reason = |path: &str| reasons.iter().find(|(p, _)| *p == path).map(|(_, r)| *r).unwrap_or("nicht übersprungen");
    assert!(reason("../draussen.svg").contains("außerhalb"), "Pfad aus dem Ordner hinaus: {reasons:?}");
    #[cfg(unix)]
    assert!(reason("bilder/link.svg").contains("außerhalb"), "Symlink hinaus: {reasons:?}");
    assert!(reason("bilder/fehlt.png").contains("nicht gefunden"), "fehlende Datei: {reasons:?}");
    assert!(reason("bilder/notiz.png").contains("kein unterstütztes Bild"), "Nicht-Bild: {reasons:?}");
    assert!(!reasons.iter().any(|(p, _)| p.contains("kommentar")), "auskommentierte Bilder zählen nicht: {reasons:?}");

    let work = report.work;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(work).await.unwrap()).into_owned(),
        HILLS,
        "Vorlage und Text bleiben byte-gleich"
    );
    let list = server.state.store.asset_list(work).await.unwrap();
    let paths: Vec<(&str, Vec<String>)> = list.iter().map(|a| (a.name.as_str(), a.paths.clone())).collect();
    assert_eq!(
        paths,
        [("haus.svg", vec!["/bilder/haus.svg".to_owned()]), ("punkt.png", vec!["bilder/punkt.png".to_owned()])],
        "asset_path bildet die Pfade so ab, wie sie im Text stehen"
    );
    let dir = server.mirror_dir_of(&report.slug);
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(dir.join("bilder/haus.svg")).unwrap())
            .into_owned(),
        HOUSE,
        "SVG als Text im Spiegel"
    );
    assert!(dir.join("bilder/punkt.png.b64").is_file(), "PNG als .b64 im Spiegel");
    if let Some(log) = common::git(&dir, &["log", "--format=%s"]) {
        assert_eq!(log, "Import aus huegel.typ\n", "Werk und Bilder in einem Import-Commit: {log}");
    }
}

#[tokio::test]
async fn image_paths_with_dot_parts_are_imported_found_and_exported_and_parent_dirs_are_skipped() {
    // Runde 7: `./bilder/beet.svg` wurde vorher übersprungen; jetzt gilt die Normalform
    // (`baruch_store::normalize_asset_path`), nur `..` bleibt verboten.
    let server = common::open().await;
    let tmp = tempfile::tempdir().unwrap();
    let source = "= Der Garten\n\n#image(\"./bilder/beet.svg\")\n\n#image(\"bilder//zaun.svg\")\n\n\
                  #image(\"bilder/./tor.svg\")\n\n#image(\"bilder/../bilder/haus.svg\")\n\nText im Garten.\n";
    std::fs::create_dir_all(tmp.path().join("bilder")).unwrap();
    std::fs::write(tmp.path().join("garten.typ"), source).unwrap();
    for name in ["beet.svg", "zaun.svg", "tor.svg", "haus.svg"] {
        std::fs::write(tmp.path().join("bilder").join(name), HOUSE).unwrap();
    }
    let works_before = server.state.store.works().await.unwrap().len();
    let report = baruch_server::import::import_file(&server.state, &tmp.path().join("garten.typ"))
        .await
        .expect("ein ungültiger Bildpfad darf den Import nicht abbrechen");
    assert_eq!(report.assets, ["beet.svg", "zaun.svg", "tor.svg"], "./ und // sind erlaubt: {report:?}");
    let skipped: Vec<(&str, &str)> = report.skipped.iter().map(|s| (s.path.as_str(), s.reason.as_str())).collect();
    assert_eq!(skipped.len(), 1, "nur der Pfad mit .. ist übersprungen: {skipped:?}");
    assert_eq!(skipped[0].0, "bilder/../bilder/haus.svg", "der Pfad wie im Text: {skipped:?}");
    assert!(skipped[0].1.contains("'..' ist nicht erlaubt"), "der Grund nennt die Pfadregel: {skipped:?}");
    assert_eq!(server.state.store.works().await.unwrap().len(), works_before + 1, "das Werk ist angelegt");
    let work = report.work;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(work).await.unwrap()).into_owned(),
        source,
        "der Text bleibt byte-gleich"
    );

    // Die Bilderseite findet das Bild unter jeder Schreibweise, aber nicht mit `..`.
    for query in ["./bilder/beet.svg", ".%2Fbilder%2Fbeet.svg", "bilder/beet.svg", "%2Fbilder%2Fbeet.svg"] {
        let (status, svg) = server.get(&format!("/werke/{work}/bilder/pfad?p={query}")).await;
        assert_eq!((status, svg.as_str()), (StatusCode::OK, HOUSE), "?p={query} liefert das Bild");
    }
    let (status, _) = server.get(&format!("/werke/{work}/bilder/pfad?p=..%2Fbilder%2Fbeet.svg")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "mit .. gibt es kein Bild");

    // Im Satzbild erscheint das Bild mit der Adresse in Normalform, und die Adresse liefert es.
    let node = server.rpc("node.get", json!({ "work": work, "node": 0 })).await;
    let body = node["result"]["revisions"][0]["body"].as_str().unwrap().to_owned();
    let parsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await;
    let html = parsed["result"]["html"].as_str().unwrap();
    let src = format!("/werke/{work}/bilder/pfad?p=bilder%2Fbeet.svg");
    assert!(html.contains(&format!("<img class=\"ws-image\" src=\"{src}\"")), "Bild im Satzbild: {html}");
    let (status, svg) = server.get(&src).await;
    assert_eq!((status, svg.as_str()), (StatusCode::OK, HOUSE), "die Adresse im Satzbild liefert das Bild");

    // export --out schreibt die Bilder an ihren Pfad in Normalform.
    let target = tempfile::tempdir().unwrap();
    let written = baruch_server::import::export_to_dir(&server.state.store, work, &target.path().join("aus"))
        .await
        .expect("Export in einen neuen Ordner");
    assert_eq!(written.len(), 4, "Typst-Datei und drei Bilder: {written:?}");
    for name in ["beet.svg", "zaun.svg", "tor.svg"] {
        let path = target.path().join("aus/bilder").join(name);
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(&path).unwrap()).into_owned(),
            HOUSE,
            "bilder/{name} byte-gleich"
        );
    }
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(target.path().join("aus/garten.typ")).unwrap())
            .into_owned(),
        source,
        "Text byte-gleich"
    );
}

#[test]
fn export_out_writes_the_typst_file_and_all_images_byte_for_byte() {
    let data = tempfile::tempdir().unwrap();
    let (_tmp, project) = hills_folder();
    let file = project.join("huegel.typ");
    let (code, out, err) = cli(data.path(), &["import", file.to_str().unwrap()]);
    assert_eq!(code, 0, "Import mit Bildern gelingt: {out}\n{err}");
    assert!(out.contains("Bilder: 2"), "Ausgabe nennt die Zahl der Bilder: {out}");
    assert!(
        err.contains("Bild übersprungen: bilder/fehlt.png (nicht gefunden"),
        "je übersprungenem Pfad eine Zeile auf stderr: {err}"
    );

    let (code, stdout, err) = cli(data.path(), &["export", "das-huegelbuch"]);
    assert_eq!(code, 0, "Export ohne --out: {err}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&stdout),
        HILLS,
        "ohne --out wie bisher der Text auf stdout (bis auf die Sprungmarken)"
    );

    let target = tempfile::tempdir().unwrap();
    let out_dir = target.path().join("zurueck");
    let (code, out, err) = cli(data.path(), &["export", "das-huegelbuch", "--out", out_dir.to_str().unwrap()]);
    assert_eq!(code, 0, "Export mit --out gelingt: {out}\n{err}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(out_dir.join("huegel.typ")).unwrap()).as_bytes(),
        HILLS.as_bytes(),
        "Typst-Datei unter ihrem Importnamen"
    );
    for path in ["bilder/haus.svg", "bilder/punkt.png"] {
        assert_eq!(
            std::fs::read(out_dir.join(path)).unwrap(),
            std::fs::read(project.join(path)).unwrap(),
            "{path} byte-gleich zur Quelle"
        );
    }
    let mut written: Vec<String> = walk(&out_dir);
    written.sort();
    assert_eq!(written, ["bilder/haus.svg", "bilder/punkt.png", "huegel.typ"], "nur Werk und gelesene Bilder");

    std::fs::write(out_dir.join("huegel.typ"), "eigene Änderung\n").unwrap();
    let (code, _, err) = cli(data.path(), &["export", "das-huegelbuch", "--out", out_dir.to_str().unwrap()]);
    assert_eq!(code, 1, "vorhandene Zieldatei → Exit 1");
    assert!(err.contains("gibt es schon"), "Meldung nennt den Grund: {err}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(out_dir.join("huegel.typ")).unwrap())
            .into_owned(),
        "eigene Änderung\n",
        "nichts überschrieben"
    );
    let (_, help, _) = cli(data.path(), &["--help"]);
    assert!(help.contains("--out"), "Hilfe nennt --out: {help}");
}

/// Dateien unter `dir`, relativ und mit `/`.
fn walk(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(folder) = stack.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap().to_str().unwrap().replace('\\', "/");
                out.push(rel);
            }
        }
    }
    out
}

#[tokio::test]
async fn uploading_and_work_import_bring_no_images() {
    let server = common::open().await;
    let answer =
        server.rpc("work.import", json!({ "file_name": "huegel.typ", "source": HILLS.replace("Hang", "Berg") })).await;
    let work = answer["result"]["work"].as_i64().expect("work.import gelingt");
    assert_eq!(answer["result"]["assets"], json!([]), "work.import legt keine Bilder an: {answer}");
    assert!(server.state.store.asset_list(work).await.unwrap().is_empty(), "keine Bildzeilen");
}
