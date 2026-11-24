// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 17c „Prüfanzeige“: Ergebnis der Wortlautprüfung im Editor (RPC `bible.marks`, Seite
//! `/werke/{id}/pruefung`, Widget `scripture-check`), Lesen in der Zählung des Moduls über die Abbildung
//! (`bibles::read_passage`), verschlüsselte SWORD-Module (RPC `bible.cipher_key_set/_delete`, Formular unter
//! Einstellungen → Quellen), BZIP2/XZ, Lizenzvermerk und Kommentar-Widget. Alle Bibeln schreibt der Test selbst
//! (`baruch_sword::fixture`, frei erfundener Text, Temp-Ordner); verschlüsselt wird mit einem frei erfundenen
//! Testschlüssel. Kein echtes Modul, kein Modell, kein Netz.

use crate::common;

use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_scripture::versification::mapping::{self, Mapped, VerseRef};
use baruch_scripture::versification::{german, versification};
use baruch_sword::fixture::{Block, Compression, Fixture, Layout, write};
use serde_json::{Value, json};

/// Frei erfundener Testschlüssel; darf nie in einer Antwort, Seite oder Meldung stehen.
const TEST_KEY: &str = "Kieselamsel-Testschluessel-7f3a9";

const SERMON: &str = "#set document(title: \"Kieselpredigt\", author: \"Lena Fluss\")\n\n\
= Am Bach\n\n\
Wie es heißt: „Der Kiesel lag am Bach, und die Amsel sang.“ (Joh 3,16)\n\n\
Falsch erinnert: „Der Stein lag am Bach“ (Joh 3,16). Ohne Zitat Ps 23,1.\n";

/// Erfundene Testbibel in deutscher Zählung.
fn kiesel(layout: Layout) -> Fixture {
    Fixture::new("Kiesel", layout, german())
        .conf("Description", "Erfundene Kieselbibel")
        .conf("DistributionLicense", "Public Domain")
        .conf("Copyright", "frei erfunden für Tests")
        .verse("John", 3, 16, "Der Kiesel lag am Bach, und die Amsel sang.")
        .verse("Ps", 23, 1, "Ein Lied vom stillen Wasser.")
}

/// Erfundene Testbibel in KJV-Zählung: Maleachi hat dort vier Kapitel (Mal 4,1–6 = deutsch Mal 3,19–24).
fn english() -> Fixture {
    let kjv = versification("KJV").unwrap();
    let mut f = Fixture::new("Englisch", Layout::ZText(Block::Chapter), kjv)
        .conf("Description", "Erfundene Bibel mit KJV-Zählung")
        .conf("DistributionLicense", "Creative Commons: BY 4.0")
        .verse("John", 3, 16, "Der Kiesel lag am Bach, und die Amsel sang.");
    for v in 1..=6 {
        f = f.verse("Mal", 4, v, &format!("Erfundener Schlussvers {v}."));
    }
    f
}

/// Verschlüsselte Testbibel (zText, mit Sapphire II und dem Testschlüssel selbst verschlüsselt).
fn sealed() -> Fixture {
    Fixture::new("Siegel", Layout::ZText(Block::Book), german())
        .conf("Description", "Erfundene verschlüsselte Bibel")
        .conf("DistributionLicense", "Copyrighted; Freischaltung beim Herausgeber")
        .verse("John", 3, 16, "Der Kiesel lag versiegelt am Bach.")
        .enciphered(TEST_KEY)
}

fn modules_dir(dir: &Path, modules: &[Fixture]) -> String {
    let root = dir.join("sword");
    for m in modules {
        write(&root, m).unwrap();
    }
    root.display().to_string()
}

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn err(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_object(), "{method} sollte scheitern: {answer}");
    answer["error"].clone()
}

/// Server mit eigenem Modulordner, der Kieselpredigt und gewählter Bibel `bible` (falls gegeben).
async fn setup(modules: &[Fixture], bible: Option<&str>) -> (common::Server, tempfile::TempDir, i64, i64) {
    let server = common::open().await;
    let dir = tempfile::tempdir().unwrap();
    let root = modules_dir(dir.path(), modules);
    ok(&server, "bible.set_dirs", json!({ "dirs": [root] })).await;
    let work = server.import(SERMON, "kiesel.typ").await.work;
    let loaded = ok(&server, "work.get", json!({ "work": work })).await;
    let key = loaded["work"]["nodes"][0]["key"].as_i64().unwrap();
    if let Some(name) = bible {
        ok(&server, "bible.set_work", json!({ "work": work, "module": name })).await;
    }
    (server, dir, work, key)
}

// ---- Lesen über die Abbildung ---------------------------------------------------------------------

#[tokio::test]
async fn a_kjv_module_reads_german_references_through_the_mapping() {
    let (server, _dir, work, _key) = setup(&[english()], Some("Englisch")).await;
    let text = "„Erfundener Schlussvers 1. Erfundener Schlussvers 2. Erfundener Schlussvers 3. Erfundener \
                Schlussvers 4. Erfundener Schlussvers 5. Erfundener Schlussvers 6.“ (Mal 3,19–24) und „Der Kiesel \
                lag am Bach“ (Joh 3,16).";
    let checked = ok(&server, "scripture.check", json!({ "work": work, "text": text })).await;
    let mal = &checked["refs"][0];
    let joh = &checked["refs"][1];
    // Joh 3 hat in beiden Zählungen 36 Verse: gleiche Stelle, kein module_ref.
    assert_eq!(joh["wording_code"], "match", "Joh 3,16 gleich gezählt: {checked}");
    assert_eq!(joh["module_ref"], Value::Null, "gleiche Stelle: kein Hinweis auf die Zählung des Moduls: {checked}");
    // Die Erwartung kommt aus der Abbildung selbst (R17a liefert die Regeltabelle; bis dahin ist die Abbildung
    // vorläufig und Mal 3,19 nicht abbildbar). So prüft der Test, dass die Wortlautprüfung genau die Verse liest,
    // die map_range nennt – und mit der Regeltabelle den Vertrag „Mal 3,19–24 → Mal 4,1–6 (KJV)“.
    let kjv = versification("KJV").unwrap();
    match mapping::map_range(german(), kjv, "Mal", (3, 19), (3, 24), baruch_sword::MAX_PASSAGE_VERSES) {
        Ok(mapped) => {
            assert_eq!(
                mal["wording_code"], "match",
                "Mal 3,19–24 wird als Mal 4,1–6 gelesen und verglichen: {checked}"
            );
            assert_eq!(mapped.verses.first().map(VerseRef::label).as_deref(), Some("Mal 4,1"), "Abbildung: {mapped:?}");
            assert_eq!(mal["module_ref"], "Mal 4,1–6 (KJV)", "gelesene Stelle in der Zählung des Moduls: {checked}");
            let looked = ok(&server, "bible.lookup", json!({ "work": work, "ref": "Mal 3,19–24" })).await;
            assert_eq!(looked["refs"][0]["module_ref"], "Mal 4,1–6 (KJV)", "bible.lookup ebenso: {looked}");
            assert_eq!(looked["refs"][0]["verses"].as_array().map(Vec::len), Some(6), "sechs Verse: {looked}");
            assert_eq!(
                looked["refs"][0]["verses"][0]["text"], "Erfundener Schlussvers 1.",
                "Text aus Mal 4,1: {looked}"
            );
        }
        Err(reason) => {
            assert_eq!(mal["wording_code"], "versification_mismatch", "nicht abbildbar bleibt ehrlich: {checked}");
            let label = mal["wording"].as_str().unwrap();
            assert!(label.starts_with("Versifikation passt nicht"), "Grund wie die Abbildung: {label}");
            assert!(label.contains(&reason), "Grund der Abbildung wörtlich: {label} / {reason}");
            assert!(label.contains("Englisch"), "Modul genannt: {label}");
            let looked = ok(&server, "bible.lookup", json!({ "work": work, "ref": "Mal 3,19–24" })).await;
            assert_eq!(looked["refs"][0]["status"], "versification_mismatch", "bible.lookup ebenso: {looked}");
        }
    }
}

#[tokio::test]
async fn an_unmappable_verse_stays_a_versification_mismatch_with_a_reason() {
    // Einen deutschen Vers suchen, den die Abbildung nicht nach KJV bringt (mit Regeltabelle seltener).
    let kjv = versification("KJV").unwrap();
    let mut unmapped = None;
    'search: for (book, chapters) in german().books() {
        for (c, &n) in chapters.iter().enumerate() {
            for v in 1..=n {
                let at = VerseRef { book, chapter: (c + 1) as u16, verse: v };
                if let Mapped::Unmapped { reason } = mapping::map_verse(german(), kjv, at) {
                    unmapped = Some((at, reason));
                    break 'search;
                }
            }
        }
    }
    let Some((at, reason)) = unmapped else {
        // Deckt die Regeltabelle jeden deutschen Vers ab, gibt es nichts zu prüfen – das ist dann der Befund.
        eprintln!("Jeder deutsche Vers lässt sich nach KJV abbilden; kein nicht abbildbarer Vers zum Prüfen.");
        return;
    };
    let (server, _dir, work, _key) = setup(&[english()], Some("Englisch")).await;
    let text = format!("„Irgendein Wortlaut“ ({})", at.label());
    let checked = ok(&server, "scripture.check", json!({ "work": work, "text": text })).await;
    let r = &checked["refs"][0];
    assert_eq!(r["wording_code"], "versification_mismatch", "{} nicht abbildbar: {checked}", at.label());
    let label = r["wording"].as_str().unwrap();
    assert!(label.contains(&reason) && label.contains("KJV"), "Grund mit beiden Zählungen: {label}");
    assert_eq!(r["bible_text"], Value::Null, "kein geratener Bibeltext: {checked}");
}

#[tokio::test]
async fn bible_lookup_takes_truncated_from_the_passage_text() {
    let (server, _dir, work, _key) = setup(&[kiesel(Layout::RawText)], Some("Kiesel")).await;
    // 1. Mose 1–16 hat 398 Verse, dazu 17,1–2: genau 400 – nicht gekürzt; bis 17,27 sind es 425 – gekürzt.
    let exact = ok(&server, "bible.lookup", json!({ "work": work, "ref": "1. Mose 1,1–17,2" })).await;
    assert_eq!(
        exact["refs"][0]["verses"].as_array().map(Vec::len),
        Some(400),
        "genau 400 Verse: {}",
        exact["refs"][0]["message"]
    );
    assert_eq!(exact["refs"][0]["truncated"], false, "genau an der Grenze ist nichts gekürzt");
    let long = ok(&server, "bible.lookup", json!({ "work": work, "ref": "1. Mose 1,1–17,27" })).await;
    assert_eq!(long["refs"][0]["verses"].as_array().map(Vec::len), Some(400), "höchstens 400 Verse");
    assert_eq!(long["refs"][0]["truncated"], true, "425 Verse: gekürzt (aus PassageText, nicht aus der Länge)");
}

// ---- bible.marks und die Prüfseite ------------------------------------------------------------------

#[tokio::test]
async fn bible_marks_lists_states_and_byte_spans_of_the_shown_revision() {
    let (server, _dir, work, key) = setup(&[kiesel(Layout::ZText(Block::Chapter))], Some("Kiesel")).await;
    let marks = ok(&server, "bible.marks", json!({ "work": work, "key": key })).await;
    let loaded = ok(&server, "work.get", json!({ "work": work })).await;
    let body = loaded["work"]["nodes"][0]["revisions"]
        .as_array()
        .and_then(|r| r.iter().find(|r| r["status"] == "current"))
        .and_then(|r| r["body"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("aktuelle Fassung: {loaded}"));
    let list = marks["marks"].as_array().unwrap();
    let states: Vec<&str> = list.iter().map(|m| m["state"].as_str().unwrap()).collect();
    assert_eq!(states, ["stimmt", "weicht_ab", "ohne_zitat"], "je Stelle der Zustand: {marks}");
    assert_eq!(marks["counts"], json!({ "stimmt": 1, "weicht_ab": 1, "andere": 1 }), "Zählung: {marks}");
    assert_eq!(marks["rev"], 1, "Vorgabe: die aktuelle Fassung: {marks}");
    assert_eq!(marks["title"], "Am Bach", "Abschnitt genannt: {marks}");
    assert_eq!(marks["bible"]["module"], "Kiesel", "Modul genannt: {marks}");
    assert_eq!(marks["notice"], "Public Domain; frei erfunden für Tests", "Lizenzvermerk der conf: {marks}");
    for m in list {
        let [s, e] = [m["ref_span"][0].as_u64().unwrap() as usize, m["ref_span"][1].as_u64().unwrap() as usize];
        assert!(
            body[s..e].contains(m["ref"].as_str().unwrap().split(' ').next().unwrap()),
            "ref_span zeigt auf die Stelle: {m}"
        );
        if let Some(span) = m["quote_span"].as_array() {
            let (s, e) = (span[0].as_u64().unwrap() as usize, span[1].as_u64().unwrap() as usize);
            assert_eq!(&body[s..e], m["quote"].as_str().unwrap(), "quote_span ist der Byte-Bereich des Zitats");
        }
    }
    assert_eq!(list[2]["quote_span"], Value::Null, "ohne Zitat kein Bereich: {marks}");
    assert_eq!(list[1]["diff_line"], "Der [−Kiesel] [+Stein] lag am Bach", "Unterschiede: {marks}");

    // scripture.check bekommt dieselben Felder additiv.
    let checked = ok(&server, "scripture.check", json!({ "work": work, "text": body })).await;
    assert_eq!(checked["refs"][0]["quote_span"], list[0]["quote_span"], "quote_span auch in scripture.check");
    assert!(checked["refs"][0].get("module_ref").is_some_and(Value::is_null), "module_ref da (null): {checked}");
    assert!(
        checked["refs"][0]["wording_code"].is_string() && checked["refs"][0]["span"].is_array(),
        "alte Felder bleiben"
    );

    // Eine neue Fassung: rev wählt die gezeigte; die Spans gelten für deren Text.
    let saved = ok(&server, "revision.save", json!({ "work": work, "key": key, "body": "= Am Bach\n\nNur noch: „Ein Lied vom stillen Wasser“ (Ps 23,1).\n" })).await;
    let new_rev = saved["rev"].as_u64().unwrap();
    let now = ok(&server, "bible.marks", json!({ "work": work, "key": key })).await;
    assert_eq!(now["rev"], new_rev, "Vorgabe folgt der neuen aktuellen Fassung: {now}");
    assert_eq!(now["marks"][0]["state"], "stimmt", "Ps 23,1 stimmt: {now}");
    let old = ok(&server, "bible.marks", json!({ "work": work, "key": key, "rev": 1 })).await;
    assert_eq!(old["marks"].as_array().map(Vec::len), Some(3), "rev=1 zeigt die alte Fassung: {old}");
    let missing = err(&server, "bible.marks", json!({ "work": work, "key": key, "rev": 99 })).await;
    assert_eq!(missing["code"], -32602, "unbekannte Fassung: {missing}");
}

#[tokio::test]
async fn bible_marks_without_a_bible_is_honestly_unchecked() {
    let (server, _dir, work, key) = setup(&[], None).await;
    let marks = ok(&server, "bible.marks", json!({ "work": work, "key": key })).await;
    assert_eq!(marks["bible"], Value::Null, "keine Bibel: {marks}");
    assert!(marks["marks"].as_array().unwrap().iter().all(|m| m["state"] == "ungeprueft"), "alles ungeprüft: {marks}");
    assert_eq!(marks["counts"]["andere"], 3, "drei Stellen, keine bestätigt: {marks}");
}

#[tokio::test]
async fn the_check_page_lists_the_results_without_javascript() {
    let (server, _dir, work, key) = setup(&[kiesel(Layout::RawText)], Some("Kiesel")).await;
    let (status, html) = server.get(&format!("/werke/{work}/pruefung?key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Seite lädt: {html}");
    assert!(html.contains("Zitate prüfen: Am Bach"), "Überschrift mit Abschnitt: {html}");
    assert!(html.contains("<table class=\"check-list\">"), "Liste als Tabelle (ohne JavaScript lesbar)");
    assert!(html.contains("data-state=\"stimmt\"") && html.contains("data-state=\"weicht_ab\""), "Zustände je Zeile");
    assert!(html.contains("„Der Stein lag am Bach“"), "Zitat sichtbar (maskiert): {html}");
    assert!(html.contains("Der [−Kiesel] [+Stein] lag am Bach"), "Unterschiede sichtbar");
    assert!(
        html.contains("<code>Kiesel</code>") && html.contains("<strong data-versification>German</strong>"),
        "Modul und Versifikation"
    );
    assert!(html.contains("Lizenz (laut conf): Public Domain; frei erfunden für Tests"), "Lizenzvermerk sichtbar");
    assert!(
        html.contains(&format!("href=\"/?work={work}&amp;key={key}&amp;rev=1\""))
            || html.contains(&format!("/?work={work}&key={key}&rev=1")),
        "Zurück zum Abschnitt: {html}"
    );
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Reste von Platzhaltern");
    let (status, old) = server.get(&format!("/werke/{work}/pruefung?key={key}&rev=1")).await;
    assert_eq!(status, StatusCode::OK, "mit rev: {old}");
    assert!(old.contains("(Fassung 1)"), "gezeigte Fassung genannt");
    let (status, _) = server.get(&format!("/werke/{work}/pruefung")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "ohne key: ehrlicher Hinweis statt Absturz");
    let (status, _) = server.get(&format!("/werke/{}/pruefung?key={key}", work + 999)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk");
    let (status, none) = server.get(&format!("/werke/{work}/pruefung?key={key}&rev=x")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "rev keine Zahl: {none}");
}

#[tokio::test]
async fn the_editor_links_to_the_check_page_and_the_widget_never_touches_the_saved_text() {
    let (server, _dir, work, key) = setup(&[kiesel(Layout::RawText)], Some("Kiesel")).await;
    let (status, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite: {page}");
    let link = format!("href=\"/werke/{work}/pruefung?key={key}&amp;rev=1\"");
    assert!(
        page.contains(&link) && page.contains(">Zitate prüfen</a>"),
        "Link „Zitate prüfen“ in der Statuszeile: {page}"
    );
    let widget = page.find("data-widget=\"scripture-check\"").expect("Widget eingehängt");
    let form_start = page.find("id=\"editor-form\"").expect("Editor-Formular");
    let form_end = form_start + page[form_start..].find("</form>").expect("Formularende");
    assert!(widget > form_end, "das Widget steht außerhalb des Formulars: nichts davon wird abgeschickt");
    let status_line = page.find("class=\"editor-status\"").expect("Statuszeile");
    assert!(widget > status_line, "das Widget steht in der Statuszeile");

    // Das Widget markiert nur über die CSS Custom Highlight API: keine DOM-Änderung im Satzbild.
    let ts = include_str!("../web/src/scripture_check.ts");
    assert!(
        ts.contains("CSS as unknown as { highlights")
            && ts.contains("\"bibel-stimmt\"")
            && ts.contains("\"bibel-weicht\""),
        "Highlight-Namen laut Vertrag"
    );
    for forbidden in [
        "surroundContents",
        "insertNode",
        "splitText",
        "execCommand",
        ".innerHTML",
        "extractContents",
        "deleteContents",
    ] {
        assert!(!ts.contains(forbidden), "scripture_check.ts ändert das Satzbild nicht ({forbidden})");
    }
    let css = include_str!("../static/pruefung.css");
    assert!(
        css.contains("::highlight(bibel-stimmt)") && css.contains("::highlight(bibel-weicht)"),
        "Stil der Markierungen"
    );

    // Speichern bleibt unverändert: Das Satzbild ohne Eingabe gibt den Text byte-gleich zurück.
    let loaded = ok(&server, "work.get", json!({ "work": work })).await;
    let body = loaded["work"]["nodes"][0]["revisions"][0]["body"].as_str().unwrap().to_owned();
    let parsed = ok(&server, "wysiwyg.parse", json!({ "work": work, "body": body })).await;
    let keep: Vec<Value> = (0..parsed["blocks"].as_array().unwrap().len()).map(|i| json!({ "keep": i })).collect();
    let same = ok(&server, "wysiwyg.serialize", json!({ "work": work, "body": body, "edits": keep })).await;
    assert_eq!(same["body"], body.as_str(), "ohne Eingabe byte-gleich (die Markierung ist keine Eingabe)");

    // Ereignis baruch:serialize-text (Vertrag §2.6): Quelltext ohne Speichern, touched und Textfeld bleiben.
    let ws = include_str!("../web/src/wysiwyg.ts");
    let start = ws.find("async function serializeText").expect("serializeText in wysiwyg.ts");
    let end = start + ws[start..].find("\n}\n").expect("Ende von serializeText");
    let function = &ws[start..end];
    assert!(
        !function.contains("touched =") && !function.contains("area.value =") && !function.contains("dispatchEvent"),
        "serializeText ändert nichts: {function}"
    );
    assert!(ws.contains("\"baruch:serialize-text\""), "Hörer am Editor-Formular");
}

// ---- Verschlüsselte Module ------------------------------------------------------------------------

/// Kein Testschlüssel im Text (Antworten, Seiten, Meldungen).
fn no_key(what: &str, text: &str) {
    assert!(!text.contains(TEST_KEY), "{what}: der Schlüssel steht nie in Antworten oder Seiten");
}

#[tokio::test]
async fn an_enciphered_module_is_locked_until_the_right_key_is_stored() {
    let (server, _dir, work, _key) = setup(&[sealed()], None).await;
    let listed = ok(&server, "bible.modules", json!({ "work": work })).await;
    let siegel = listed["modules"].as_array().unwrap().iter().find(|m| m["module"] == "Siegel").cloned().unwrap();
    assert_eq!(siegel["cipher"], "locked", "ohne Schlüssel gesperrt: {siegel}");
    assert_eq!(siegel["key_stored"], false, "kein Schlüssel hinterlegt: {siegel}");
    assert_eq!(siegel["supported"], true, "das Format ist lesbar; nur der Schlüssel fehlt: {siegel}");
    assert_eq!(siegel["notice"], "Copyrighted; Freischaltung beim Herausgeber", "Lizenzvermerk: {siegel}");
    let refused = err(&server, "bible.lookup", json!({ "work": work, "module": "Siegel", "ref": "Joh 3,16" })).await;
    assert!(refused["message"].as_str().unwrap().contains("verschlüsselt"), "kein Text, ehrlicher Hinweis: {refused}");

    let wrong = err(&server, "bible.cipher_key_set", json!({ "module": "Siegel", "key": "falsch-geraten" })).await;
    assert_eq!(wrong["code"], -32000, "falscher Schlüssel abgelehnt: {wrong}");
    assert_eq!(wrong["message"], "Schlüssel passt nicht zum Modul; nichts gespeichert.", "Meldung laut Vertrag");
    assert_eq!(server.state.secrets.get("sword-cipher:siegel").unwrap(), None, "nichts gespeichert");
    let empty = err(&server, "bible.cipher_key_set", json!({ "module": "Siegel", "key": "  " })).await;
    assert_eq!(empty["code"], -32602, "leerer Schlüssel: {empty}");
    let plain = err(&server, "bible.cipher_key_set", json!({ "module": "Gibtsnicht", "key": "x" })).await;
    assert_eq!(plain["code"], -32602, "unbekanntes Modul: {plain}");

    let set = ok(&server, "bible.cipher_key_set", json!({ "module": "siegel", "key": TEST_KEY })).await;
    assert_eq!(set, json!({ "module": "Siegel", "cipher": "unlocked" }), "Antwort laut Vertrag, ohne Schlüssel");
    assert_eq!(
        server.state.secrets.get("sword-cipher:siegel").unwrap().as_deref(),
        Some(TEST_KEY),
        "im Schlüsselbund-Konto sword-cipher:siegel"
    );
    let read = ok(&server, "bible.lookup", json!({ "work": work, "module": "Siegel", "ref": "Joh 3,16" })).await;
    assert_eq!(read["refs"][0]["verses"][0]["text"], "Der Kiesel lag versiegelt am Bach.", "jetzt lesbar: {read}");
    assert_eq!(read["bible"]["cipher"], "unlocked", "entsperrt: {read}");
    let listed = ok(&server, "bible.modules", json!({ "work": work })).await;
    let siegel = listed["modules"].as_array().unwrap().iter().find(|m| m["module"] == "Siegel").cloned().unwrap();
    assert_eq!((siegel["cipher"].as_str(), siegel["key_stored"].as_bool()), (Some("unlocked"), Some(true)), "{siegel}");
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Siegel" })).await;
    let checked = ok(
        &server,
        "scripture.check",
        json!({ "work": work, "text": "„Der Kiesel lag versiegelt am Bach“ (Joh 3,16)" }),
    )
    .await;
    assert_eq!(checked["refs"][0]["wording_code"], "match", "Wortlautprüfung mit entsperrtem Modul: {checked}");

    let unconfirmed = err(&server, "bible.cipher_key_delete", json!({ "module": "Siegel" })).await;
    assert_eq!(unconfirmed["code"], -32602, "Löschen braucht confirm: true: {unconfirmed}");
    let deleted = ok(&server, "bible.cipher_key_delete", json!({ "module": "Siegel", "confirm": true })).await;
    assert_eq!(deleted, json!({ "module": "Siegel", "cipher": "locked" }), "wieder gesperrt");
    assert_eq!(server.state.secrets.get("sword-cipher:siegel").unwrap(), None, "aus dem Schlüsselbund entfernt");
    let again = ok(&server, "scripture.check", json!({ "work": work, "text": "„Der Kiesel“ (Joh 3,16)" })).await;
    assert_eq!(again["refs"][0]["wording_code"], "unchecked", "ohne Schlüssel kein Wortlaut: {again}");
    assert!(again["bible"]["problem"].as_str().unwrap_or("").contains("verschlüsselt"), "Grund genannt: {again}");

    for value in [listed, set, read, checked, deleted, again, wrong] {
        no_key("RPC-Antwort", &value.to_string());
    }
}

#[tokio::test]
async fn cipher_keys_are_human_only_and_never_shown() {
    let (server, _dir, work, key) = setup(&[sealed()], None).await;
    ok(&server, "bible.cipher_key_set", json!({ "module": "Siegel", "key": TEST_KEY })).await;
    ok(&server, "bible.set_work", json!({ "work": work, "module": "Siegel" })).await;
    // Nicht über MCP: kein Werkzeug setzt oder löscht einen Schlüssel.
    let names: Vec<&str> = baruch_server::mcp::tools::TOOLS.iter().map(|t| t.name).collect();
    assert!(!names.iter().any(|n| n.contains("cipher")), "kein MCP-Werkzeug für Schlüssel: {names:?}");
    assert!(
        baruch_server::bibles::WRITES.contains(&"bible.cipher_key_set")
            && baruch_server::bibles::WRITES.contains(&"bible.cipher_key_delete"),
        "beide nur Mensch"
    );
    let tool = baruch_server::mcp::bibles::call(&server.state, "bible_modules", json!({ "work": work })).await.unwrap();
    no_key("MCP bible_modules", &tool.to_string());
    assert!(tool.to_string().contains("\"cipher\":\"unlocked\""), "MCP sieht nur den Stand: {tool}");
    let checked =
        baruch_server::mcp::scripture::call(&server.state, json!({ "work": work, "key": key })).await.unwrap();
    no_key("MCP scripture_check", &checked.to_string());
    // Nicht über /act.
    let request = Request::post("/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("method=bible.cipher_key_delete&work={work}&module=Siegel&confirm=1")))
        .unwrap();
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "/act kennt keine Schlüssel-Methoden");
    assert!(server.state.secrets.get("sword-cipher:siegel").unwrap().is_some(), "Schlüssel blieb");
    // Seiten zeigen nur „hinterlegt“.
    for uri in [
        format!("/einstellungen/quellen?work={work}"),
        format!("/werke/{work}/pruefung?key={key}"),
        format!("/literatur/bibel?work={work}&key={key}&module=Siegel&ref=Joh+3%2C16"),
        format!("/?work={work}&key={key}"),
    ] {
        let (status, html) = server.get(&uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {html}");
        no_key(&uri, &html);
    }
    let (_, sources) = server.get(&format!("/einstellungen/quellen?work={work}")).await;
    assert!(sources.contains("Schlüssel hinterlegt"), "Anzeige „hinterlegt“: {sources}");
}

#[tokio::test]
async fn the_cipher_form_on_the_sources_page_works_without_javascript() {
    let (server, _dir, work, _key) = setup(&[sealed()], None).await;
    let (status, html) = server.get(&format!("/einstellungen/quellen?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Seite Quellen: {html}");
    assert!(
        html.contains("value=\"bible.cipher_key_set\"") && html.contains("type=\"password\" name=\"key\""),
        "Formular mit Passwortfeld"
    );
    assert!(html.contains("gesperrt – Schlüssel fehlt") && html.contains("data-cipher=\"locked\""), "Stand sichtbar");
    assert!(html.contains("Copyrighted; Freischaltung beim Herausgeber"), "Lizenzvermerk sichtbar");
    assert!(!html.contains("value=\"bible.cipher_key_delete\""), "ohne Schlüssel nichts zu löschen");

    let (status, to) = server
        .post_form(
            "/einstellungen/quellen/act",
            &format!("method=bible.cipher_key_set&work={work}&module=Siegel&key=falsch"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303 wie die übrigen Formulare");
    let to = to.unwrap();
    assert!(to.contains("fehler=bibel_schluessel_falsch"), "falscher Schlüssel: {to}");
    no_key("Weiterleitung", &to);
    let (_, page) = server.get(&to).await;
    assert!(page.contains("Der Schlüssel passt nicht zum Modul; nichts gespeichert."), "Meldung auf der Seite: {page}");

    let form = format!("method=bible.cipher_key_set&work={work}&module=Siegel&key={TEST_KEY}");
    let (status, to) = server.post_form("/einstellungen/quellen/act", &form).await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "das Formular ohne JavaScript leitet nach dem richtigen Schlüssel weiter"
    );
    let to = to.unwrap();
    assert!(to.contains("ok=bibel_schluessel"), "hinterlegt: {to}");
    no_key("Weiterleitung", &to);
    let (_, page) = server.get(&to).await;
    no_key("Seite nach dem Speichern", &page);
    assert!(page.contains("value=\"bible.cipher_key_delete\""), "jetzt mit Formular zum Entfernen");
    assert!(page.contains("data-cipher=\"unlocked\""), "entsperrt");

    let (_, to) = server
        .post_form("/einstellungen/quellen/act", &format!("method=bible.cipher_key_delete&work={work}&module=Siegel"))
        .await;
    assert!(to.unwrap().contains("fehler=bestaetigen"), "ohne Kästchen nichts entfernt");
    assert!(server.state.secrets.get("sword-cipher:siegel").unwrap().is_some(), "Schlüssel blieb");
    let (_, to) = server
        .post_form(
            "/einstellungen/quellen/act",
            &format!("method=bible.cipher_key_delete&work={work}&module=Siegel&confirm=1"),
        )
        .await;
    assert!(to.unwrap().contains("ok=bibel_schluessel_entfernt"), "entfernt");
    assert_eq!(server.state.secrets.get("sword-cipher:siegel").unwrap(), None, "aus dem Schlüsselbund entfernt");
}

// ---- Kompression, Lizenz, Kommentar-Widget ----------------------------------------------------------

#[tokio::test]
async fn bzip2_and_xz_bibles_are_checked_by_the_server() {
    for (name, compression) in [("Bzwei", Compression::Bzip2), ("Xzett", Compression::Xz)] {
        let f = Fixture::new(name, Layout::ZText(Block::Chapter), german()).compression(compression).verse(
            "John",
            3,
            16,
            "Der Kiesel lag am Bach, und die Amsel sang.",
        );
        let (server, _dir, work, _key) = setup(&[f], Some(name)).await;
        let checked =
            ok(&server, "scripture.check", json!({ "work": work, "text": "„Der Kiesel lag am Bach“ (Joh 3,16)" }))
                .await;
        assert_eq!(checked["refs"][0]["wording_code"], "match", "{name}: entpackt und verglichen: {checked}");
    }
}

#[tokio::test]
async fn the_licence_notice_is_visible_on_every_bible_page() {
    let (server, _dir, work, key) = setup(&[kiesel(Layout::RawText)], Some("Kiesel")).await;
    let notice = "Public Domain; frei erfunden für Tests";
    let (_, sources) = server.get(&format!("/einstellungen/quellen?work={work}")).await;
    assert!(sources.contains(notice), "Einstellungen → Quellen");
    let (_, bible) = server.get(&format!("/literatur/bibel?work={work}&key={key}&module=Kiesel")).await;
    assert!(
        bible.contains(&format!("Lizenz (laut conf): <span data-notice>{notice}</span>")),
        "/literatur/bibel: {bible}"
    );
    let (_, check) = server.get(&format!("/werke/{work}/pruefung?key={key}")).await;
    assert!(check.contains(&format!("Lizenz (laut conf): {notice}")), "Prüfseite");
    let checked = ok(&server, "scripture.check", json!({ "work": work, "text": "Joh 3,16" })).await;
    assert_eq!(checked["bible"]["license"], notice, "scripture.check: {checked}");
    assert_eq!(checked["bible"]["notice"], notice, "scripture.check (additiv notice): {checked}");
    let marks = ok(&server, "bible.marks", json!({ "work": work, "key": key })).await;
    assert_eq!(marks["notice"], notice, "bible.marks: {marks}");
}

#[tokio::test]
async fn the_commentary_panel_has_one_details_per_module_and_unified_group_titles() {
    let commentary = |name: &str| {
        Fixture::new(name, Layout::RawCom, german())
            .conf("Description", "Erfundener Kommentar")
            .conf("DistributionLicense", "Public Domain")
            .verse("Ps", 23, 1, "Erfundene Auslegung zum stillen Wasser.")
    };
    let (server, _dir, work, _key) =
        setup(&[kiesel(Layout::RawText), commentary("Bachkommentar"), commentary("Wiesenkommentar")], None).await;
    ok(
        &server,
        "bible.set_work_sources",
        json!({ "work": work, "commentaries": ["Bachkommentar", "Wiesenkommentar"], "lexicons": [] }),
    )
    .await;
    let html = baruch_server::bibles::commentary_panel_html(&server.state, work, "Ps 23,1").await;
    assert!(
        html.starts_with("<details class=\"commentaries\" data-widget=\"commentaries\">"),
        "Panel mit Widget: {html}"
    );
    assert_eq!(
        html.matches("<details class=\"commentary\"").count(),
        2,
        "je Modul ein eigenes <details> (ohne JavaScript klappbar): {html}"
    );
    assert!(
        html.contains("Kommentar · Bachkommentar") && html.contains("Kommentar · Wiesenkommentar"),
        "Überschrift „Kommentar · Modul“"
    );
    let hits = baruch_server::bibles::commentary_hits_html(&server.state, work, "stillen Wasser", "").await;
    assert!(
        hits.contains("hit-group-title\">Kommentar · Bachkommentar<"),
        "Treffergruppe wie „Bibel · Modul“ gestaltet: {hits}"
    );

    let ts = include_str!("../web/src/commentaries.ts");
    assert!(ts.contains("alle aufklappen") && ts.contains("alle zuklappen"), "Knöpfe zum Auf-/Zuklappen");
    // Jeder Zugriff (window.localStorage) steht in einem offenen try-Block; Erwähnungen im Kommentar zählen nicht.
    assert!(ts.contains("window.localStorage"), "Zustand in localStorage gemerkt");
    for (i, _) in ts.match_indices("window.localStorage") {
        let before = &ts[..i];
        assert!(
            before.rfind("try {").is_some_and(|t| before[t..].matches('}').count() <= before[t..].matches('{').count()),
            "localStorage nur in try/catch (Stelle {i})"
        );
    }
    let main = include_str!("../web/src/main.ts");
    assert!(
        main.contains("import \"./commentaries.js\";") && main.contains("import \"./scripture_check.js\";"),
        "beide Widgets geladen"
    );
    let css = include_str!("../static/literature_bibles.css");
    assert!(css.contains(".hit-group-title"), "einheitlicher Stil der Gruppenüberschriften");
}
