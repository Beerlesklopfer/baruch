// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typesetting settings, data (Round 29a, contract §8.1): RPC work.typeset_get/set/reset/adopt_rule, mirror commits
//! with trailer, MCP typeset_settings (read-only), exchange across two instances.
//!
//! Owner: satz-daten. Work freely invented (`tests/data/gartenbuch.typ`); every assertion with its reason. The check
//! goes through the real ways: RPC like the user interface, Git mirror in the temp directory, MCP through `/mcp` with a
//! loopback peer, exchange through `work.exchange_export`/`work.exchange_import` (XSD of minor version 7 since Round
//! 31).

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::rpc::{INVALID_PARAMS, REFUSED};
use baruch_server::typeset_settings::{ADOPT_NOTE, no_rule};
use baruch_typst::typeset::{CANONICAL_CHAPTER_BREAK, FIELDS, TypesetSettings, rules};
use serde_json::{Value, json};

use crate::common;

/// Example of the contract §2 (all sixteen fields).
/// The fields from Round 29 that [`full`] sets; the three R31 fields (index and glossary) come after them and enter
/// the exchange only with minor version 7 (R31 glossar-daten), therefore `full` does not set them.
fn r29_fields() -> &'static [(&'static str, &'static str)] {
    &FIELDS[..16]
}

fn full() -> Value {
    json!({"chapter_start":"right_page","part_page":{"side":"right","blank_back":true},"page_format":{"kind":"a5"},
           "margins":{"inside_mm":20.0,"outside_mm":15.0,"top_mm":18.0,"bottom_mm":22.0,"binding_mm":5.0},
           "font":"Libertinus Serif","font_size_pt":10.5,"leading_em":0.65,"paragraph":{"kind":"indent","em":1.0},
           "justify":true,"hyphenate":true,"lang":"de",
           "page_numbers":{"position":"bottom_outside","start":"first_chapter"},"running_head":"chapter",
           "footnotes":{"numbering":"arabic","restart":"per_chapter"},"widows_orphans":true,"xref_with_page":true})
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

fn commits(server: &common::Server, slug: &str) -> usize {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map_or(0, |n| n.trim().parse().unwrap())
}

fn last_commit(server: &common::Server, slug: &str) -> String {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%an <%ae>|%cn%n%B"]).unwrap_or_default()
}

async fn garden() -> (common::Server, i64, String) {
    let server = common::open().await;
    let report = server.import_garden().await;
    (server, report.work, report.slug)
}

async fn get(server: &common::Server, work: i64) -> Value {
    result(&server.rpc("work.typeset_get", json!({ "work": work })).await).clone()
}

async fn tombstones(server: &common::Server) -> Vec<String> {
    sqlx::query_scalar("SELECT uuid FROM uuid_tombstone WHERE kind = 'work_typeset' ORDER BY uuid")
        .fetch_all(server.state.store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn typeset_get_without_settings_names_every_field_and_has_no_rules() {
    let (server, work, _) = garden().await;
    let view = get(&server, work).await;
    assert_eq!(view["work"], work, "Werk der Antwort");
    assert_eq!(view["set"], false, "ohne Zeile keine Satz-Einstellungen");
    for key in ["uuid", "updated", "updated_by", "template_rule"] {
        assert!(view[key].is_null(), "{key}: null ohne Einstellungen bzw. ohne Vorlagenregel: {view}");
    }
    assert_eq!(view["settings"], json!({}), "keine gesetzten Felder");
    assert_eq!(view["rules"], "", "kein Regelblock ohne Einstellungen (byte-gleicher Export)");
    assert_eq!(view["conflicts"], json!([]), "keine Konflikte ohne gesetzte Felder");
    assert_eq!(view["warnings"], json!([]), "keine Hinweise");
    let fields = view["fields"].as_array().unwrap();
    assert_eq!(fields.len(), FIELDS.len(), "alle sechzehn Felder für die Oberfläche");
    assert_eq!(fields[0], json!({ "id": "chapter_start", "label": "Kapitelanfang" }), "id und Bezeichnung");
    // Without `work` the default work (smallest id = sample work); an unknown work is a parameter error.
    let default = result(&server.rpc("work.typeset_get", json!({})).await).clone();
    assert_eq!(default["work"], server.state.default_work().await.unwrap(), "Vorgabewerk ohne work");
    assert_eq!(error_code(&server.rpc("work.typeset_get", json!({ "work": 9999 })).await), INVALID_PARAMS);
}

#[tokio::test]
async fn typeset_set_saves_commits_once_with_trailer_and_body_and_skips_equal_values() {
    let (server, work, slug) = garden().await;
    let before = commits(&server, &slug);
    let saved = result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": full() })).await).clone();
    assert_eq!(saved["work"], work, "Werk der Antwort");
    assert_eq!(saved["settings"], full(), "kanonische Einstellungen zurück (schon normalisiert)");
    let ids: Vec<&str> = r29_fields().iter().map(|(id, _)| *id).collect();
    assert_eq!(saved["changed"], json!(ids), "alle gesetzten Felder geändert, in FIELDS-Reihenfolge");
    assert!(saved["commit"].is_string(), "ein Commit: {saved}");
    assert_eq!(commits(&server, &slug), before + 1, "genau ein Commit je Änderung");
    let message = last_commit(&server, &slug);
    assert!(message.starts_with("Autor <autor@baruch.invalid>|Autor\n"), "Autor und Committer der Mensch: {message}");
    assert!(
        message.contains("Satz: Kapitelanfang, Teil-Seite, Seitenformat, Ränder,"),
        "Betreff nennt die Bezeichnungen: {message}"
    );
    assert!(message.contains("Kapitelanfang: aus der Werkvorlage → neue rechte Seite"), "Rumpf vorher → nachher");
    assert!(message.contains("Grundschrift: aus der Werkvorlage → „Libertinus Serif“"), "Rumpf je Feld: {message}");
    assert!(message.contains("Baruch-Action: typeset-set"), "Trailer: {message}");

    let view = get(&server, work).await;
    assert_eq!(view["set"], true, "gespeichert");
    assert_eq!(view["settings"], full(), "gelesen wie gespeichert");
    assert_eq!(view["updated_by"], "Autor", "geändert vom Menschen");
    assert_eq!(view["uuid"].as_str().map(str::len), Some(36), "uuid der Zeile");
    let settings: TypesetSettings = serde_json::from_value(full()).unwrap();
    assert_eq!(view["rules"], rules(&settings), "erzeugter Regelblock aus baruch_typst::typeset::rules");
    // main.typ in the mirror carries the rule block through snapshot::head (no mirror file of its own).
    let main = std::fs::read_to_string(server.mirror_dir_of(&slug).join("main.typ")).unwrap();
    let block = rules(&settings);
    assert!(main.contains(&block), "main.typ enthält den Block „// Baruch: Satz“: {main}");
    let snapshot = server.state.store.mirror_snapshot(work).await.unwrap();
    assert_eq!(snapshot.iter().find(|f| f.path == "main.typ").unwrap().contents, main, "Spiegel = Datenbankstand");

    // Same values: nothing changed, no commit.
    let same = result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": full() })).await).clone();
    assert_eq!(same["changed"], json!([]), "gleiche Werte → changed leer");
    assert!(same["commit"].is_null(), "ohne Änderung kein Commit: {same}");
    assert_eq!(commits(&server, &slug), before + 1, "weiterhin ein Commit");

    // Only „Verweis mit Seitenzahl“: no file changes, nevertheless exactly one commit (allow_empty).
    let mut xref = full();
    xref["xref_with_page"] = json!(false);
    let saved = result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": xref })).await).clone();
    assert_eq!(saved["changed"], json!(["xref_with_page"]), "nur dieses Feld");
    assert_eq!(commits(&server, &slug), before + 2, "Protokoll-Commit ohne Dateiänderung");
    let message = last_commit(&server, &slug);
    assert!(message.contains("Satz: Verweis mit Seitenzahl geändert"), "Betreff: {message}");
    assert!(message.contains("Verweis mit Seitenzahl: mit Seitenzahl → ohne Seitenzahl"), "Rumpf: {message}");
}

#[tokio::test]
async fn invalid_or_unknown_settings_are_parameter_errors_and_change_nothing() {
    let (server, work, slug) = garden().await;
    let before = commits(&server, &slug);
    let mut bad = full();
    bad["font_size_pt"] = json!(40.0);
    bad["margins"]["inside_mm"] = json!(1.0);
    bad["lang"] = json!("Deutsch");
    let answer = server.rpc("work.typeset_set", json!({ "work": work, "settings": bad })).await;
    assert_eq!(error_code(&answer), INVALID_PARAMS, "ungültig → -32602");
    let message = answer["error"]["message"].as_str().unwrap();
    for part in ["Satz-Einstellungen:", "Schriftgröße", "Rand innen", "Sprache", "; "] {
        assert!(message.contains(part), "alle Meldungen („Feld: Meldung; …“), auch {part}: {message}");
    }
    let unknown = server.rpc("work.typeset_set", json!({ "work": work, "settings": { "farbe": "rot" } })).await;
    assert_eq!(error_code(&unknown), INVALID_PARAMS, "unbekannte Felder → -32602: {unknown}");
    let missing = server.rpc("work.typeset_set", json!({ "work": work })).await;
    assert_eq!(error_code(&missing), INVALID_PARAMS, "settings fehlt → -32602");
    assert_eq!(get(&server, work).await["set"], false, "nichts gespeichert");
    assert_eq!(commits(&server, &slug), before, "kein Commit");
}

#[tokio::test]
async fn reset_commits_once_leaves_a_tombstone_and_does_nothing_without_settings() {
    let (server, work, slug) = garden().await;
    let nothing = result(&server.rpc("work.typeset_reset", json!({ "work": work })).await).clone();
    assert_eq!(nothing["changed"], json!([]), "ohne Einstellungen ändert Zurücksetzen nichts");
    assert!(nothing["commit"].is_null(), "und committet nichts");
    result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": full() })).await);
    let uuid = get(&server, work).await["uuid"].as_str().unwrap().to_owned();
    let before = commits(&server, &slug);
    let reset = result(&server.rpc("work.typeset_reset", json!({ "work": work })).await).clone();
    assert_eq!(reset["work"], work, "Werk der Antwort");
    assert_eq!(reset["changed"].as_array().unwrap().len(), r29_fields().len(), "alle gesetzten Felder zurück: {reset}");
    assert!(reset["commit"].is_string(), "ein Commit: {reset}");
    assert_eq!(commits(&server, &slug), before + 1, "genau ein Commit");
    let message = last_commit(&server, &slug);
    assert!(message.contains("Satz: Einstellungen zurückgesetzt"), "Betreff: {message}");
    assert!(message.contains("Kolumnentitel: Kapitel → aus der Werkvorlage"), "Rumpf vorher → nachher: {message}");
    assert!(message.contains("Baruch-Action: typeset-reset"), "Trailer: {message}");
    assert_eq!(tombstones(&server).await, vec![uuid], "Grabstein mit der uuid der Zeile");
    let view = get(&server, work).await;
    assert_eq!((view["set"].clone(), view["rules"].clone()), (json!(false), json!("")), "wieder ohne Einstellungen");
    let main = std::fs::read_to_string(server.mirror_dir_of(&slug).join("main.typ")).unwrap();
    assert!(!main.contains("// Baruch: Satz"), "main.typ ohne Regelblock: {main}");
}

#[tokio::test]
async fn adopting_the_canonical_rule_moves_it_into_the_settings_with_one_commit() {
    let (server, work, slug) = garden().await;
    let template = server.state.store.work_template(work).await.unwrap();
    let with_rule = format!("{template}{CANONICAL_CHAPTER_BREAK}\n");
    result(&server.rpc("work.update", json!({ "work": work, "template": with_rule })).await);
    // One field already set: the remaining fields stay on adoption.
    let keep = json!({ "font_size_pt": 11.0 });
    result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": keep })).await);
    let view = get(&server, work).await;
    let rule = &view["template_rule"];
    assert_eq!(rule["chapter_start"], "right_page", "kanonische Regel erkannt: {view}");
    assert_eq!(rule["text"], CANONICAL_CHAPTER_BREAK, "Text der Zeile");
    let line = with_rule.lines().position(|l| l == CANONICAL_CHAPTER_BREAK).unwrap() + 1;
    assert_eq!(rule["line"], line, "Zeile 1-basiert");

    let before = commits(&server, &slug);
    let revisions_before = server.state.store.template_revisions(work).await.unwrap().len();
    // Round 23a: over RPC only with explicit confirmation.
    let unconfirmed = server.rpc("work.typeset_adopt_rule", json!({ "work": work })).await;
    assert_eq!(unconfirmed["error"]["code"], -32602, "ohne confirm: true → -32602: {unconfirmed}");
    assert_eq!(unconfirmed["error"]["message"], "confirm: true erwartet", "mit dem festen Wortlaut: {unconfirmed}");
    let adopted =
        result(&server.rpc("work.typeset_adopt_rule", json!({ "work": work, "confirm": true })).await).clone();
    assert_eq!(adopted["chapter_start"], "right_page", "Wert aus der Regel");
    assert_eq!(adopted["changed"], json!(["chapter_start"]), "nur der Kapitelanfang");
    let rev = adopted["template_rev"].as_u64().unwrap() as u32;
    assert!(adopted["commit"].is_string(), "ein Commit: {adopted}");
    assert_eq!(commits(&server, &slug), before + 1, "Einstellungen und Vorlage in genau einem Commit");
    let message = last_commit(&server, &slug);
    assert!(
        message.contains(&format!("Satz: Kapitelanfang aus der Werkvorlage übernommen (Vorlagenfassung {rev})")),
        "Betreff mit Fassung: {message}"
    );
    assert!(message.contains("Baruch-Action: typeset-adopt"), "Trailer: {message}");
    assert_eq!(server.state.store.work_template(work).await.unwrap(), template, "Zeile aus der Vorlage entfernt");
    let revisions = server.state.store.template_revisions(work).await.unwrap();
    assert_eq!(revisions.len(), revisions_before + 1, "eine neue Vorlagenfassung");
    let last = revisions.last().unwrap();
    assert_eq!((last.number, last.note.as_str()), (rev, ADOPT_NOTE), "Nummer und Notiz der neuen Fassung");
    let after = get(&server, work).await;
    assert_eq!(after["settings"], json!({ "chapter_start": "right_page", "font_size_pt": 11.0 }), "übrige bleiben");
    assert!(after["template_rule"].is_null(), "keine Regel mehr in der Vorlage");
    let vorlage = std::fs::read_to_string(server.mirror_dir_of(&slug).join("vorlage.typ")).unwrap();
    assert!(!vorlage.contains(CANONICAL_CHAPTER_BREAK), "vorlage.typ im selben Commit ohne die Zeile");

    // Without a recognised rule: rejected, nothing changed.
    let again = server.rpc("work.typeset_adopt_rule", json!({ "work": work, "confirm": true })).await;
    assert_eq!(error_code(&again), REFUSED, "keine Regel → -32000");
    let no_rule = baruch_server::i18n::sync_scope(baruch_server::i18n::Lang::De, no_rule);
    assert_eq!(again["error"]["message"], no_rule, "Meldung des Vertrags §6.1");
    assert_eq!(commits(&server, &slug), before + 1, "kein weiterer Commit");
    assert_eq!(server.state.store.template_revisions(work).await.unwrap().len(), revisions_before + 1, "keine Fassung");
}

#[tokio::test]
async fn an_unknown_font_is_named_in_the_warnings() {
    let (server, work, _) = garden().await;
    let settings = json!({ "font": "Erfundene Grotesk Probe" });
    result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": settings })).await);
    let view = get(&server, work).await;
    let warnings = view["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w == "Schrift „Erfundene Grotesk Probe“ fehlt; Typst setzt mit der eingebetteten Schrift"),
        "Hinweis auf die fehlende Schrift (Vertrag §6.1): {view}"
    );
    // An embedded font is not missing.
    let families = tokio::task::spawn_blocking(baruch_render::font_families).await.unwrap();
    let embedded = families.iter().find(|f| f.embedded).expect("Typst bringt Schriften mit");
    result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": { "font": embedded.name } })).await);
    assert_eq!(get(&server, work).await["warnings"], json!([]), "{} ist vorhanden", embedded.name);
}

async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
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
async fn the_mcp_tool_reads_like_typeset_get_and_there_is_no_writing_tool() {
    let (server, work, _) = garden().await;
    let template = server.state.store.work_template(work).await.unwrap();
    result(
        &server
            .rpc("work.update", json!({ "work": work, "template": format!("{template}{CANONICAL_CHAPTER_BREAK}\n") }))
            .await,
    );
    result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": full() })).await);
    let (headers, init) = mcp(
        &server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Satzprobe", "version": "1.0" } } }),
    )
    .await;
    assert!(init["result"].is_object(), "initialize: {init}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let (_, list) = mcp(&server, Some(&session), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let typeset: Vec<&Value> = tools
        .iter()
        .filter(|t| t["name"].as_str().is_some_and(|n| n.contains("typeset") || n.contains("satz")))
        .collect();
    assert_eq!(typeset.len(), 1, "genau ein Satz-Werkzeug, kein schreibendes (Vertrag §6.2): {typeset:?}");
    assert_eq!(typeset[0]["name"], "typeset_settings", "das lesende Werkzeug");
    let description = typeset[0]["description"].as_str().unwrap();
    assert!(description.contains("Read-only"), "Beschreibung sagt „nur lesend“: {description}");
    assert!(!description.contains("Stub"), "kein Stub mehr: {description}");

    let (_, answer) = mcp(
        &server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "typeset_settings", "arguments": { "work": work } } }),
    )
    .await;
    assert_eq!(answer["result"]["isError"], false, "typeset_settings gelingt: {answer}");
    let read = &answer["result"]["structuredContent"];
    let mut expected = get(&server, work).await;
    expected.as_object_mut().unwrap().remove("fields");
    assert_eq!(read, &expected, "wie work.typeset_get ohne fields");
    assert_eq!(read["settings"], full(), "Einstellungen gelesen");
    assert_eq!(read["template_rule"]["chapter_start"], "right_page", "Vorlagenregel gemeldet");
    assert!(read["conflicts"].is_array(), "Konflikte als Liste");
}

async fn export_xml(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    result(&answer)["xml"].as_str().unwrap().to_owned()
}

async fn import(server: &common::Server, xml: &str, mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": mode }),
        )
        .await;
    result(&answer).clone()
}

fn xmllint_u6(xml: &str) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("werk.baruch.xml");
    std::fs::write(&file, xml).unwrap();
    let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u10.xsd");
    match std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&file)
        .output()
    {
        Ok(output) => {
            assert!(output.status.success(), "XSD u6: {}", String::from_utf8_lossy(&output.stderr))
        }
        Err(e) => eprintln!("ÜBERSPRUNGEN: xmllint nicht gefunden ({e})"),
    }
}

#[tokio::test]
async fn two_instances_exchange_set_change_and_reset_without_silent_loss() {
    // A sets, B imports anew: all fields including the identifier come along.
    let (a, work_a, _) = garden().await;
    result(&a.rpc("work.typeset_set", json!({ "work": work_a, "settings": full() })).await);
    let xml = export_xml(&a, work_a).await;
    assert!(
        // Round 34 (mindmap_daten): the writer now writes minor version 9 (tree of the ideas).
        xml.contains("version=\"10\"") && xml.contains("<satz uuid="),
        "Ausgabe in Unterversion 9 (seit Runde 34) mit <satz>"
    );
    xmllint_u6(&xml);
    let b = common::open().await;
    let imported = import(&b, &xml, "new").await;
    let work_b = imported["work"].as_i64().unwrap();
    assert_eq!(imported["added"]["typeset"], 1, "Satz-Einstellungen im Bericht: {imported}");
    let view_a = get(&a, work_a).await;
    let view_b = get(&b, work_b).await;
    for key in ["settings", "uuid", "updated", "updated_by", "rules"] {
        assert_eq!(view_b[key], view_a[key], "{key} auf B wie auf A");
    }

    // A changes: B merges – nothing silently overwritten, a hint names the field and both states.
    let mut changed = full();
    changed["running_head"] = json!("work_title");
    result(&a.rpc("work.typeset_set", json!({ "work": work_a, "settings": changed })).await);
    let merged = import(&b, &export_xml(&a, work_a).await, "merge").await;
    let hint = merged["deviations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "satz")
        .unwrap_or_else(|| panic!("Hinweis zur Abweichung: {merged}"))
        .clone();
    assert_eq!(hint["action"], "hinweis", "nur ein Hinweis");
    assert!(hint["label"].as_str().unwrap().contains("Kolumnentitel"), "Feld genannt: {hint}");
    assert!(hint["there"].as_str().unwrap().contains("work_title"), "Stand der Datei im Hinweis: {hint}");
    assert_eq!(get(&b, work_b).await["settings"], full(), "B behält seinen Stand");

    // B resets (tombstone); a further merge does not bring the settings back.
    result(&b.rpc("work.typeset_reset", json!({ "work": work_b })).await);
    let merged = import(&b, &export_xml(&a, work_a).await, "merge").await;
    assert!(
        merged["deleted_here"].as_array().unwrap().iter().any(|d| d["kind"] == "satz"),
        "unter „hier gelöscht“: {merged}"
    );
    assert_eq!(get(&b, work_b).await["set"], false, "B bleibt zurückgesetzt");

    // A resets: the output carries the tombstone satz, a third instance takes it over without settings.
    let uuid_a = view_a["uuid"].as_str().unwrap().to_owned();
    result(&a.rpc("work.typeset_reset", json!({ "work": work_a })).await);
    let xml = export_xml(&a, work_a).await;
    assert!(xml.contains(&format!("<eintrag uuid=\"{uuid_a}\" art=\"satz\"")), "Grabstein satz in der Ausgabe");
    assert!(!xml.contains("<satz "), "keine Satz-Einstellungen mehr");
    xmllint_u6(&xml);
    let c = common::open().await;
    let work_c = import(&c, &xml, "new").await["work"].as_i64().unwrap();
    assert_eq!(get(&c, work_c).await["set"], false, "C ohne Einstellungen");
    assert_eq!(tombstones(&c).await, vec![uuid_a.clone()], "C kennt den Grabstein (kommt nie wieder)");
    // And B, which has already reset the settings of A itself, gets no error.
    let merged = import(&b, &xml, "merge").await;
    assert!(merged["refusals"].as_array().unwrap().is_empty(), "nichts abgelehnt: {merged}");
}
