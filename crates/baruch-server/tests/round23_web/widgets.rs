// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23a: Verhaltenstests der Widgets im Node-Nachbau (ohne Browser). Der DOM-Nachbau kommt aus
//! `round18_satzbild.rs` (`run_harness`), die Szenen stehen in `widgets.mjs`. Geprüft wird Verhalten der
//! ausgelieferten, übersetzten Skripte (`baruch_server::SCRIPTS`): Einstellungen (Alt+, und
//! `baruch:settings-changed` nur nach Erfolg), Editor (Knopf-Weitergabe über `requestSubmit(submitter)`, Sperre
//! während des Speicherns, Hinweis danach), Befunde, Dialog „Verweis“ (Runde 30a), `sameMarkBehind`, `markdownTitle`
//! im Gleichlauf mit dem Server, EPUB-Vorschau und Zitat-Werkzeug. Fehlt `node`, meldet jeder Test „ÜBERSPRUNGEN“.
//! Alle Texte frei erfunden.

use serde_json::{Value, json};

use crate::common;
use crate::round18_satzbild::run_harness;

const SCENES: &str = include_str!("widgets.mjs");

/// Szene ausführen; `None` (mit Meldung), wenn `node` fehlt.
fn scene(name: &str, scenario: Value) -> Option<Value> {
    let Some(seen) = run_harness(SCENES, &scenario) else {
        eprintln!("ÜBERSPRUNGEN: node fehlt – Verhaltenstest „{name}“ nicht gelaufen.");
        return None;
    };
    assert_eq!(seen["errors"], json!([]), "{name}: keine Fehler im Nachbau: {seen}");
    Some(seen)
}

fn ok(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none_or(Value::is_null), "RPC-Fehler: {answer}");
    &answer["result"]
}

// ---- Einstellungen -------------------------------------------------------------------------------------------------

#[test]
fn the_settings_popup_opens_with_alt_comma_and_reports_a_change_only_after_success() {
    let frame = |alert: &str| {
        format!(
            "<html><body><div data-settings-frame data-page=\"darstellung\">{alert}<form method=\"post\" \
             action=\"/einstellungen/darstellung\"><input name=\"theme\" value=\"dark\"><button \
             type=\"submit\">Speichern</button></form></div></body></html>"
        )
    };
    let scenario = json!({
        "scene": "settings", "submits": 3,
        "pages": [
            // Öffnen mit Alt+, lädt den Rahmen der Seite.
            { "status": 200, "body": frame("") },
            // 1. Speichern mit Erfolg (303 gefolgt, Adresse mit ok-Code).
            { "status": 200, "url": "http://127.0.0.1:8765/einstellungen/darstellung?ok=gespeichert", "body": frame("") },
            // 2. Abgelehnt: 303 auf eine Adresse mit Fehlercode (Status 200 nach der Weiterleitung).
            { "status": 200, "url": "http://127.0.0.1:8765/einstellungen/verbindungen?fehler=name",
              "body": frame("<p role=\"alert\">Der Name ist schon vergeben.</p>") },
            // 3. Abgelehnt: 422 mit der Meldung auf der Seite.
            { "status": 422, "body": frame("<p role=\"alert\">Sicherung bitte bestätigen.</p>") },
        ],
    });
    let Some(seen) = scene("Einstellungen", scenario) else { return };
    assert_eq!(seen["openBefore"], false, "vor Alt+, ist der Dialog zu: {seen}");
    assert_eq!(seen["openAfterAltComma"], true, "Alt+, öffnet den Dialog: {seen}");
    assert_eq!(seen["altCommaPrevented"], true, "die Taste ist verbraucht (kein Komma im Feld darunter): {seen}");
    assert_eq!(seen["plainCommaPrevented"], false, "ein Komma ohne Alt bleibt ein Komma: {seen}");
    assert_eq!(
        seen["firstFetch"], "http://127.0.0.1:8765/einstellungen/daten?fragment=1",
        "geladen wird der Rahmen der Seite des Links (data-settings-page), als Fragment: {seen}"
    );
    assert_eq!(seen["framePage"], "darstellung", "der Rahmen aus der Antwort steht im Dialog: {seen}");

    let steps = seen["steps"].as_array().unwrap();
    assert_eq!(steps[0]["posted"]["method"], "POST", "das Formular geht per fetch POST ab: {seen}");
    assert_eq!(steps[0]["posted"]["body"], "theme=dark", "mit seinen Feldern: {seen}");
    assert_eq!(
        steps[0]["events"],
        json!([{ "page": "darstellung" }]),
        "nach dem Erfolg genau ein baruch:settings-changed mit der Seite: {seen}"
    );
    assert_eq!(steps[0]["theme"], "dark", "die Darstellung setzt das Thema sofort: {seen}");
    assert_eq!(
        steps[1]["events"],
        json!([]),
        "nach einer Weiterleitung auf ?fehler=… kommt kein baruch:settings-changed (vorher: ja): {seen}"
    );
    assert_eq!(steps[1]["alert"], "Der Name ist schon vergeben.", "die Meldung der Seite steht im Dialog: {seen}");
    assert_eq!(steps[2]["events"], json!([]), "nach 422 kommt kein baruch:settings-changed: {seen}");
    assert_eq!(steps[2]["alert"], "Sicherung bitte bestätigen.", "die Meldung der abgelehnten Seite steht da: {seen}");
    assert_eq!(steps[2]["hasForm"], true, "und das Formular bleibt zum Korrigieren: {seen}");
}

#[test]
fn saved_tells_success_from_refusal_by_status_and_error_codes_in_the_address() {
    let call = |ok: bool, url: &str| json!(["saved", { "ok": ok, "url": url }]);
    let scenario = json!({
        "scene": "pure", "module": "settings.js",
        "calls": [
            call(true, "http://127.0.0.1:8765/einstellungen/werk?ok=gespeichert&work=3"),
            call(true, "http://127.0.0.1:8765/einstellungen/verbindungen?fehler=name"),
            call(true, "/einstellungen/werk?zeitfehler=1&work=3"),
            call(true, "/einstellungen/werk?satzbildfehler=1"),
            call(true, "/einstellungen/assistenten?modellfehler=leer"),
            call(false, "/einstellungen/daten"),
            call(true, "/einstellungen/werk?q=fehlerfrei"),
        ],
    });
    let Some(seen) = scene("saved", scenario) else { return };
    assert_eq!(
        seen["values"],
        json!([true, false, false, false, false, false, true]),
        "Erfolg nur bei 2xx ohne Parameter, dessen Name auf „fehler“ endet (ein Wert „fehlerfrei“ zählt nicht)"
    );
}

// ---- Editor --------------------------------------------------------------------------------------------------------

const TOEPFEREI: &str = "= Ton\n\nDer Ton ruht drei Tage im Keller, bevor er auf die Scheibe kommt.\n\n\
= Glasur\n\nDie Glasur wird zweimal gebrannt.\n";

#[tokio::test]
async fn the_editor_forwards_the_pressed_button_locks_while_saving_and_shows_the_notice() {
    let server = common::open().await;
    let work = server.import(TOEPFEREI, "toepferei-23.typ").await.work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[0].key;
    // Echte Bereiche des Servers; das Textfeld (Quelltext) ist der Weg, den auch das Satzbild beim Speichern nimmt.
    let rendered = server.rpc("ui.render", json!({ "work": work, "key": key })).await;
    let panes = ok(&rendered).clone();
    let fresh = json!({ "result": { "tree": "<p>Baum nach dem Speichern</p>", "editor": "<p>Editor nach dem Speichern</p>",
        "assistants": "<p>Assistenten nach dem Speichern</p>", "url": format!("/?work={work}&key={key}&rev=2"),
        "title": "Ton – Baruch" } });
    let base = json!({
        "scene": "editor", "url": format!("http://127.0.0.1:8765/?work={work}&key={key}"),
        "tree": panes["tree"], "editor": panes["editor"], "assistants": panes["assistants"],
    });

    // „Als neue Fassung speichern“: der Knopf geht über requestSubmit(submitter) mit (as_new → mode: "new").
    let mut scenario = base.clone();
    scenario["button"] = json!("as_new");
    scenario["rpc"] = json!({
        "outline.check": { "result": { "valid": true, "split": [], "merge": false } },
        "outline.save": { "result": { "node": 0, "key": key, "rev": 2, "saved": "new", "reason": "forced",
                                      "reason_label": "auf Wunsch" } },
        "ui.render": fresh,
    });
    let Some(seen) = scene("Editor: Als neue Fassung speichern", scenario) else { return };
    assert_eq!(
        seen["buttonsFound"],
        json!({ "save": true, "asNew": true, "asNewOutsideForm": true }),
        "das ausgelieferte HTML hat beide Knöpfe; „Als neue Fassung“ steht außerhalb des Formulars (form=…): {seen}"
    );
    assert_eq!(seen["saveDisabledAtStart"], true, "ohne Änderung ist „Speichern“ gesperrt: {seen}");
    assert_eq!(seen["dirtyAfterInput"]["saveDisabled"], false, "nach dem Tippen ist „Speichern“ frei: {seen}");
    let calls = seen["calls"].as_array().unwrap();
    let methods: Vec<&str> = calls.iter().map(|c| c["method"].as_str().unwrap()).collect();
    assert_eq!(
        methods,
        ["outline.check", "outline.save", "ui.render"],
        "prüfen, speichern, neu zeichnen – und trotz zweitem Druck nur EIN outline.save (Codereview web-1): {seen}"
    );
    let save = &calls[1];
    assert_eq!(save["mode"], "new", "der gedrückte Knopf kam beim Arbeitsbereich an: mode „new“ statt „auto“: {seen}");
    assert_eq!(save["asNew"], Value::Null, "as_new selbst geht nicht an den Server: {seen}");
    assert!(
        save["body"].as_str().unwrap().ends_with("\nEin neuer Satz vom Test.\n"),
        "gespeichert wird der getippte Text: {seen}"
    );
    assert_eq!(save["basedOnAmend"], 0, "mit dem Stand der Grundlage aus dem Formular: {seen}");
    assert_eq!(seen["inertRightAfterPress"], true, "sofort nach dem Druck ist der Editor gesperrt (inert): {seen}");
    assert_eq!(seen["busyRightAfterPress"], "true", "und als beschäftigt gemeldet (aria-busy): {seen}");
    assert!(
        calls.iter().all(|c| c["editorInert"] == true),
        "während prüfen, speichern und neu zeichnen bleibt der Editor gesperrt – nichts Getipptes geht verloren: {seen}"
    );
    assert_eq!(seen["inertAfter"], false, "danach ist die Sperre aufgehoben: {seen}");
    assert_eq!(seen["busyAfter"], Value::Null, "aria-busy ist entfernt: {seen}");
    assert_eq!(
        seen["notices"],
        json!([{ "text": "Gespeichert: neue Fassung 2 (auf Wunsch).", "role": "status" }]),
        "nach dem Speichern steht der Hinweis in Worten da (role=status): {seen}"
    );
    assert_eq!(seen["alerts"], json!([]), "kein Fehlerhinweis: {seen}");
    assert_eq!(seen["editorAfter"], "<p>Editor nach dem Speichern</p>", "der Bereich ist neu gezeichnet: {seen}");
    assert_eq!(seen["pushed"][0]["how"], "push", "die Adresse wandert in den Verlauf: {seen}");

    // „Speichern“ ohne Namen: Modus auto bleibt, Rückmeldung „fortgeschrieben (Stand k)“.
    let mut scenario = base.clone();
    scenario["button"] = json!("save");
    scenario["rpc"] = json!({
        "outline.check": { "result": { "valid": true, "split": [], "merge": false } },
        "outline.save": { "result": { "node": 0, "key": key, "rev": 1, "saved": "amended", "amend_seq": 1 } },
        "ui.render": fresh,
    });
    let seen = scene("Editor: Speichern", scenario).expect("node war eben noch da");
    let save = &seen["calls"][1];
    assert_eq!(save["mode"], "auto", "der gewöhnliche Knopf lässt den Modus des Formulars (auto): {seen}");
    assert_eq!(
        seen["notices"][0]["text"], "Gespeichert: fortgeschrieben (Stand 1).",
        "Hinweis nach dem Fortschreiben: {seen}"
    );

    // Abgelehnt (ungültige Überschriften): kein Speichern, der Text bleibt, die Sperre geht wieder auf.
    let mut scenario = base;
    scenario["button"] = json!("save");
    scenario["rpc"] = json!({
        "outline.check": { "result": { "valid": false, "split": [], "merge": false,
                                       "message": "Diese Überschriften ergeben keine gültige Gliederung." } },
    });
    let seen = scene("Editor: abgelehnt", scenario).expect("node war eben noch da");
    let methods: Vec<&str> = seen["calls"].as_array().unwrap().iter().map(|c| c["method"].as_str().unwrap()).collect();
    assert_eq!(methods, ["outline.check"], "nach der Ablehnung wird nicht gespeichert: {seen}");
    assert_eq!(seen["inertAfter"], false, "die Sperre ist wieder aufgehoben, der Autor kann weiterschreiben: {seen}");
    assert!(
        seen["areaValue"].as_str().is_some_and(|text| text.ends_with("\nEin neuer Satz vom Test.\n")),
        "der getippte Text steht noch im Textfeld des Editors: {seen}"
    );
    assert_eq!(seen["notices"], json!([]), "kein „Gespeichert“: {seen}");
}

// ---- Befunde -------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_finding_status_form_goes_over_rpc_and_shows_errors_at_the_form() {
    let server = common::open().await;
    let work = server.import(TOEPFEREI, "toepferei-befund-23.typ").await.work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[0].key;
    let body = "Hinweis: „drei Tage“ belegen.";
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: body.into(),
        sources: vec![],
        refs: vec![],
    };
    let post = server.state.store.add_post(work, 0, &post).await.unwrap();
    let finding = server
        .state
        .store
        .finding_insert(&baruch_store::NewFinding {
            work,
            key: Some(key),
            post: Some(post),
            thread_post: None,
            position: 0,
            persona: "@lektor",
            kind: baruch_store::FindingKind::Hint,
            body,
            quote: "",
            quote_check: "",
            similar_to: None,
            summary_of: &[],
        })
        .await
        .unwrap();
    let rendered = server.rpc("ui.render", json!({ "work": work, "key": key })).await;
    let assistants = ok(&rendered)["assistants"].clone();
    let url = format!("http://127.0.0.1:8765/?work={work}&key={key}");
    let scenario = json!({
        "scene": "findings", "url": url, "assistants": assistants, "status": "rejected",
        "reason": "Ist im Vorwort schon belegt.",
        // Erster Versuch scheitert am Server, der zweite gelingt.
        "rpc": { "finding.set_status": [
            { "error": { "code": -32000, "message": "Der Befund ist inzwischen entschieden." } },
            { "result": { "finding": { "id": finding.id } } },
        ] },
    });
    let Some(seen) = scene("Befunde", scenario) else { return };
    assert!(seen["forms"].as_u64().unwrap() >= 3, "das Akkordeon liefert die Statusformulare des Befunds: {seen}");
    assert_eq!(seen["formMethod"], "finding.set_status", "gewählt ist das Formular „verworfen“: {seen}");
    assert_eq!(seen["first"]["prevented"], true, "das Widget übernimmt das Absenden (kein Seitenwechsel): {seen}");
    assert_eq!(seen["first"]["disabledDuring"], true, "der Knopf ist während des Aufrufs gesperrt: {seen}");
    assert_eq!(seen["first"]["disabledAfter"], false, "nach dem Fehler ist er wieder frei: {seen}");
    assert_eq!(
        seen["errorAfterFirst"], "Der Befund ist inzwischen entschieden.",
        "die Meldung des Servers steht als Hinweis am Formular: {seen}"
    );
    assert_eq!(
        seen["navigationsAfterFirst"], 0,
        "nach dem Fehler wird nicht neu gezeichnet (der Entwurf bleibt): {seen}"
    );
    let calls = seen["calls"].as_array().unwrap();
    assert_eq!(calls.len(), 2, "zwei Aufrufe: {seen}");
    assert_eq!(
        calls[1]["params"],
        json!({ "work": work, "id": finding.id, "status": "rejected", "reason": "Ist im Vorwort schon belegt." }),
        "Zahlen als Zahlen, die Begründung als Text, `method` und `back` gehören nicht zur RPC: {seen}"
    );
    assert_eq!(
        seen["navigations"],
        json!([{ "url": url }]),
        "nach dem Erfolg zeichnet die Arbeitsseite neu (baruch:navigate mit der gezeigten Adresse): {seen}"
    );
}

// ---- Dialog „Verweis“ (Runde 30a) ----------------------------------------------------------------------------------

#[test]
fn the_xref_dialog_searches_picks_and_inserts_at_the_caret_in_the_source() {
    let text = "Vorher  nachher.";
    let scenario = json!({
        "scene": "xref", "work": 4, "text": text, "caret": 7, "query": "Brun", "pick": 1,
        "rpc": {
            "work.typeset_get": { "result": { "settings": { "xref_with_page": true } } },
            "xref.targets": { "result": { "targets": [
                { "key": 7, "label": "k7", "number": "1", "title": "Am Hof", "level": 1, "snippet": "" },
                { "key": 9, "label": "k9", "number": "1.1", "title": "Am Brunnen", "level": 2, "snippet": "" },
            ] } },
        },
    });
    let Some(seen) = scene("Verweis", scenario) else { return };
    assert_eq!(seen["dialogOpen"], true, "baruch:xref-open am Editor öffnet den Dialog: {seen}");
    assert_eq!(seen["pageCheckedFromWork"], true, "das Häkchen „mit Seitenzahl“ folgt der Werk-Einstellung: {seen}");
    assert_eq!(
        seen["options"],
        json!(["1 Am Hof", "1.1 Am Brunnen"]),
        "die Ziele aus xref.targets stehen zur Wahl: {seen}"
    );
    let calls = seen["calls"].as_array().unwrap();
    assert_eq!(
        calls.last().unwrap()["params"],
        json!({ "work": 4, "q": "Brun", "limit": 50 }),
        "das Tippen im Suchfeld fragt (verzögert) mit dem Suchwort nach: {seen}"
    );
    assert_eq!(seen["selected"], json!(["false", "true"]), "das gewählte Ziel ist als gewählt ausgezeichnet: {seen}");
    assert_eq!(seen["textAfterPick"], "Am Brunnen", "der Linktext übernimmt den Titel des Ziels: {seen}");
    assert_eq!(
        seen["areaAfter"], "Vorher #link(<k9>)[Am Brunnen, S.~#context counter(page).at(<k9>).first()] nachher.",
        "„Einfügen“ setzt den Verweis mit Seitenzahl an die Cursorstelle im Quelltext: {seen}"
    );
    assert!(
        seen["areaAfter"].as_str().unwrap().contains(seen["previewAfter"].as_str().unwrap()),
        "eingefügt ist genau, was die Vorschau des Dialogs zeigte: {seen}"
    );
    assert_eq!(seen["inputs"], 1, "das Einfügen meldet sich wie Tippen (ein input-Ereignis): {seen}");
    assert_eq!(seen["dialogGone"], true, "der Dialog ist danach entfernt: {seen}");
}

// ---- Registermarke -------------------------------------------------------------------------------------------------

#[test]
fn same_mark_behind_sees_the_chain_of_marks_directly_at_the_caret() {
    let grace = json!({ "register": "stichwort", "term": "Gnade" });
    let paul = json!({ "register": "personen", "term": "Paulus" });
    let scenario = json!({
        "scene": "marks",
        // Im Absatz: „Die Gnade“ ‹Marke Gnade› ‹leerer Text› ‹Marke Paulus› „ trägt.“
        "present": [grace, paul],
        "asked": [
            ["end-of-text", grace], ["end-of-text", paul], ["end-of-text", { "register": "stichwort", "term": "Huld" }],
            ["inside-text", grace], ["element-offset", paul], ["after-marks", grace],
        ],
    });
    let Some(seen) = scene("sameMarkBehind", scenario) else { return };
    assert_eq!(
        seen["values"],
        json!([true, true, false, false, true, false]),
        "am Ende des Worts: dieselbe Marke direkt dahinter (auch die zweite der Kette, über leeren Text hinweg) → \
         true; andere Marke → false; mitten im Wort → false; Stelle als Kind-Index vor der Kette → true; hinter den \
         Marken → false"
    );
}

// ---- markdownTitle im Gleichlauf mit dem Server --------------------------------------------------------------------

#[test]
fn markdown_title_agrees_with_the_server_side_converter() {
    let files = [
        "---\ntitle: \"Der Obstgarten\"\nauthor: Lene\n---\n\n# Eins\n\nText.\n",
        "---\ntitle: 'Zwischen den Zeilen'\n---\nText.\n",
        "---\ntitle:   Ohne Anführung  \n---\nText.\n",
        "---\nauthor: Niemand\n---\nText ohne Titel im Kopf.\n",
        "# Nur eine Überschrift\n\nKein Kopf.\n",
        "---\ntitle:\n---\nLeerer Titel.\n",
        "---\ntitle: Erster\ntitle: Zweiter\n---\nZwei Titel.\n",
        "---\ntitle : Mit Leerraum vor dem Doppelpunkt\n---\nText.\n",
        "---\n  title: eingerückt zählt nicht\n---\nText.\n",
        "---\ntitle: \"'Doppelt gefasst'\"\n---\nText.\n",
        "\u{feff}---\r\ntitle: Mit BOM und CRLF\r\n---\r\nText.\r\n",
        "---\ntitle: Ende mit Punkten\n...\nText.\n",
        "Text davor\n---\ntitle: Nicht am Anfang\n---\n",
    ];
    let scenario = json!({
        "scene": "pure", "module": "works.js",
        "calls": files.iter().map(|f| json!(["markdownTitle", f])).collect::<Vec<_>>(),
    });
    let Some(seen) = scene("markdownTitle", scenario) else { return };
    for (file, widget) in files.iter().zip(seen["values"].as_array().unwrap()) {
        let server = baruch_server::markdown_import::convert::to_typst(file, "Ersatz").title;
        assert_eq!(
            widget.as_str().map(str::to_owned),
            server,
            "Titelvorschlag des Widgets und vergebener Titel des Servers stimmen überein für {file:?}"
        );
    }
    assert_eq!(seen["values"][0], "Der Obstgarten", "Gegenprobe: der Titel wird wirklich gelesen");
    assert_eq!(seen["values"][6], "Zweiter", "Gegenprobe: bei zwei Titeln gilt in beiden der letzte");
}

// ---- EPUB-Vorschau -------------------------------------------------------------------------------------------------

#[test]
fn the_epub_preview_does_not_reload_on_arrow_keys_in_a_select() {
    let Some(seen) = scene("EPUB-Vorschau", json!({ "scene": "epub-preview" })) else { return };
    assert_eq!(
        seen["steps"],
        json!([
            ["nach drei Pfeiltasten", 0],
            ["nach Eingabetaste", 1],
            ["nach Wahl mit der Maus", 2],
            ["nach Kästchen", 3],
            ["aufgeklappt: Pfeil, Eingabetaste, change", 4],
        ]),
        "Pfeiltasten in der Auswahlliste schicken nicht ab (WCAG 3.2.2); erst die Eingabetaste, eine Wahl mit der \
         Maus oder ein Kästchen lädt neu – je genau einmal"
    );
}

// ---- Menüband ------------------------------------------------------------------------------------------------------

#[test]
fn arrow_keys_move_through_the_toolbar_with_wrap_around() {
    let call = |key: &str, index: i64, count: i64| json!(["toolbarTarget", key, index, count]);
    let scenario = json!({
        "scene": "pure", "module": "ribbon.js",
        "calls": [
            call("ArrowRight", 0, 5), call("ArrowRight", 4, 5), call("ArrowLeft", 0, 5), call("ArrowLeft", 3, 5),
            call("Home", 3, 5), call("End", 1, 5), call("Tab", 1, 5), call("ArrowRight", -1, 5), call("ArrowRight", 0, 0),
        ],
    });
    let Some(seen) = scene("Menüband: Pfeiltasten", scenario) else { return };
    assert_eq!(
        seen["values"],
        json!([1, 0, 4, 2, 0, 4, null, null, null]),
        "→ und ← wandern mit Umlauf, Pos1/Ende springen an den Rand; Tab bleibt dem Browser; ohne Fokus in der \
         Leiste oder ohne Knöpfe bewegt sich nichts"
    );
}

// ---- Zitat-Werkzeug ------------------------------------------------------------------------------------------------

#[test]
fn the_quote_button_inserts_the_block_with_one_blank_line_around_and_a_line_for_the_reference() {
    let snippet = baruch_server::ribbon::QUOTE_SNIPPET;
    let call =
        |before: &str, selected: &str, after: &str| json!(["insertion", snippet, "quote", before, selected, after]);
    let scenario = json!({
        "scene": "pure", "module": "ribbon.js",
        "calls": [
            call("Der Satz davor.", "", "Der Satz danach.\n"),
            call("Der Satz davor.\n", "", "\nDer Satz danach.\n"),
            call("Der Satz davor.\n\n", "", "\n\nDer Satz danach.\n"),
            call("", "", ""),
            call("Absatz.\n\n", "Wer sät, wartet.", ""),
            ["caretIn", snippet, "quote"],
        ],
    });
    let Some(seen) = scene("Zitat", scenario) else { return };
    let values = seen["values"].as_array().unwrap();
    let block = "#quote(block: true)[]\n– ";
    assert_eq!(
        values[0],
        json!({ "text": format!("\n\n{block}\n\n"), "caret": 2 + "#quote(block: true)[".len() }),
        "mitten im Text: je eine Leerzeile davor und danach, darunter die Zeile für die Stellenangabe; Cursor im Rumpf"
    );
    assert_eq!(
        values[1]["text"],
        format!("\n{block}\n"),
        "ein vorhandenes Zeilenende zählt mit: zusammen genau eine Leerzeile davor und danach"
    );
    assert_eq!(values[2]["text"], block, "stehen die Leerzeilen schon da, kommt keine doppelte dazu");
    assert_eq!(values[3]["text"], format!("{block}\n"), "am Anfang eines leeren Texts steht nichts davor");
    assert_eq!(
        values[4]["text"], "#quote(block: true)[Wer sät, wartet.]\n– \n",
        "eine Auswahl wandert in den Rumpf des Zitats"
    );
    assert_eq!(values[5], "#quote(block: true)[".len(), "der Cursor steht im Rumpf, nicht in der Zeile der Angabe");

    // Was der Knopf einfügt, liest das Satzbild als Zitatblock und eigenen Absatz – keine Insel.
    let inserted = format!("Der Satz davor.{}Der Satz danach.\n", values[0]["text"].as_str().unwrap());
    let doc = baruch_wysiwyg::parse(&inserted);
    assert!(
        doc.blocks.iter().any(|b| matches!(b.kind, baruch_wysiwyg::BlockKind::Quote { block: true, .. })),
        "das Eingefügte ist im Satzbild ein Zitatblock: {:#?}",
        doc.blocks
    );
    assert!(
        !doc.blocks.iter().any(|b| b.kind == baruch_wysiwyg::BlockKind::Island),
        "und nichts davon eine Code-Insel: {:#?}",
        doc.blocks
    );
}
