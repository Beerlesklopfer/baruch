// SPDX-License-Identifier: AGPL-3.0-or-later
//! Eine Gliederung im Baum (Vertrag Runde 5, Aufgabe „tree“): Seiten als HTML-Ausschnitte mit und ohne
//! JavaScript, Verschieben behält die Ebene (RPC und Formular), Umschalten bringt die Ebene mit
//! (Antwort und Spiegel), römische Kapitelnummern aus einer Nummerierungsfunktion der Vorlage und die
//! Absprachen im TypeScript (am Quelltext, ohne Browser). Echter Datenordner im Temp-Ordner,
//! Gartenbuch aus `tests/data`, sonst frei erfundene Texte.

use crate::common;

use axum::http::StatusCode;
use serde_json::{Value, json};

/// Gartenbuch importieren: Frühling (0), Aussaat (1), Tiefe (2), Sommer (3).
async fn garden() -> (common::Server, i64, std::path::PathBuf) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let dir = server.mirror_dir_of(&report.slug);
    (server, report.work, dir)
}

fn titles(loaded: &Value) -> Vec<String> {
    loaded["result"]["work"]["nodes"].as_array().unwrap().iter().map(|n| n["title"].as_str().unwrap().into()).collect()
}

fn commits(dir: &std::path::Path) -> Option<u64> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

/// Das Stück HTML der linken Spalte (Gliederung).
fn tree_pane(html: &str) -> &str {
    let start = html.find("id=\"pane-tree\"").expect("linke Spalte vorhanden");
    let rest = &html[start..];
    &rest[..rest.find("</nav>").expect("Ende der linken Spalte")]
}

// ---- Seiten ------------------------------------------------------------------------------------

#[tokio::test]
async fn tree_entries_open_the_editor_even_from_the_preview() {
    let (server, work, _) = garden().await;
    let (status, html) = server.get(&format!("/?work={work}&node=1&view=preview")).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    let tree = tree_pane(&html);
    let k = key_at(&server, work, 3).await;
    assert!(
        tree.contains(&format!("href=\"/?work={work}&key={k}\" data-nav")),
        "der Eintrag „Sommer“ zeigt auf den Editor (ohne view=preview), über den Schlüssel: {tree}"
    );
    assert!(!tree.contains("view=preview\" data-nav"), "kein Baumeintrag verlinkt die Vorschau: {tree}");
    assert!(tree.contains("name=\"view\" value=\"preview\""), "die Formulare kehren weiter in die Vorschau zurück");
}

#[tokio::test]
async fn the_tree_offers_the_context_menu_and_hides_the_fallback_links_with_javascript() {
    let (server, work, _) = garden().await;
    let (_, html) = server.get(&format!("/?work={work}&node=1")).await;
    let tree = tree_pane(&html);
    // Kontextmenü: verborgen (ohne JavaScript unsichtbar), mit allen Aktionen auf Deutsch.
    assert!(tree.contains("<div class=\"outline-menu\" role=\"menu\""), "Kontextmenü im Baum: {tree}");
    assert!(tree.contains("hidden data-outline-menu"), "das Menü ist ohne JavaScript verborgen: {tree}");
    for (action, label) in [
        ("rename", "Umbenennen <kbd>F2</kbd>"),
        ("add-before", "Neuer Punkt davor"),
        ("add-after", "Neuer Punkt danach"),
        ("add-into", "Neuer Punkt darunter"),
        ("promote", "Höher stufen"),
        ("demote", "Tiefer stufen"),
        ("delete", "Löschen… <kbd>Entf</kbd>"),
    ] {
        assert!(tree.contains(&format!("data-action=\"{action}\"")), "Menüpunkt {action} fehlt: {tree}");
        assert!(tree.contains(label), "Beschriftung „{label}“ fehlt: {tree}");
    }
    assert!(tree.contains("data-outline-menu-open"), "Knopf „⋯“ öffnet das Menü auch per Maus: {tree}");
    assert!(tree.contains("class=\"tool js-only\""), "der Knopf „⋯“ erscheint nur mit JavaScript: {tree}");
    // Rückfall-Links nur ohne JavaScript (CSS `.js .nojs-only { display: none }`).
    let links: Vec<&str> = tree.match_indices("href=\"/gliederung").map(|(i, _)| &tree[..i]).collect();
    assert!(!links.is_empty(), "ohne JavaScript bleiben die Links auf /gliederung: {tree}");
    for before in links {
        let tag = &before[before.rfind('<').unwrap()..];
        assert!(tag.contains("nojs-only"), "jeder Link auf /gliederung ist nur ohne JavaScript sichtbar: {tag}");
    }
    let css = include_str!("../static/outline.css");
    assert!(css.contains(".js .nojs-only { display: none !important; }"), "mit JavaScript verborgen");
    assert!(css.contains("html:not(.js) .js-only { display: none !important; }"), "ohne JavaScript verborgen");
    // Ohne JavaScript stehen die Werkzeuge immer da; sie bekommen eine eigene Zeile, sonst verdrängen sie mit dem
    // Abzeichen „k offen“ den Titel (R28, Sichtprüfung „2.. 4 offen“). Der Titel behält eine Mindestbreite, Abzeichen
    // rutschen darunter.
    assert!(css.contains("html:not(.js) .outline-row { flex-wrap: wrap; }"), "Zeile darf umbrechen: {css}");
    assert!(
        css.contains("html:not(.js) .outline-row > .outline-tools { flex-basis: 100%;"),
        "Werkzeuge ohne JavaScript in eigener Zeile: {css}"
    );
    assert!(
        css.contains(".outline-row > .tree-entry > .tree-label { flex: 1 1 8em; }"),
        "Titel mit Mindestbreite: {css}"
    );
    // Die Formulare ↑ ↓ ← → bleiben als Rückfall ohne JavaScript (an /gliederung/act).
    for id in ["o-down-0", "o-up-3", "o-left-1", "o-right-1"] {
        assert!(tree.contains(&format!("id=\"{id}\"")), "Formular {id} bleibt: {tree}");
    }
    assert!(tree.contains("action=\"/gliederung/act\""), "Formulare gehen ohne JavaScript an /gliederung/act");
    // Angaben für Menü und Löschen am Eintrag.
    assert!(tree.contains("data-level=\"1\" data-title=\"Frühling\""), "Ebene und Titel am Eintrag: {tree}");
    assert!(tree.contains(" data-children"), "Punkte mit Unterpunkten sind markiert (Löschen gesperrt): {tree}");
    assert!(tree.contains(" data-promote") && tree.contains(" data-demote"), "Umstufen möglich markiert: {tree}");
}

#[tokio::test]
async fn the_numbering_form_sits_in_the_head_of_the_tree_and_works_without_javascript() {
    let (server, work, dir) = garden().await;
    let (_, html) = server.get(&format!("/?work={work}&node=1")).await;
    let tree = tree_pane(&html);
    assert!(tree.contains("<form class=\"numbering-form\" method=\"post\" action=\"/gliederung/act\""), "{tree}");
    assert!(tree.contains("value=\"outline.set_numbering\""), "das Formular stellt die Nummerierung ein: {tree}");
    assert!(tree.contains("name=\"numbering\" value=\"vorlage\""), "Vorgabe: aus der Werkvorlage: {tree}");
    // Runde 7: der Name des passenden Schemas („1.1“ = Dezimal), das Muster nur im title.
    assert!(tree.contains("Nummern: Dezimal (DIN 1421)</summary>"), "der Kopf nennt das passende Schema: {tree}");
    assert!(tree.contains("Gliederungsschema: Aus der Werkvorlage, Muster 1.1)"), "das Muster im title: {tree}");
    assert!(tree.find("numbering-form").unwrap() < tree.find("data-widget=\"outline\"").unwrap(), "im Kopf");
    // Ohne JavaScript: Formular an /gliederung/act, danach zurück zum Werk.
    let before = commits(&dir);
    let (status, location) = server
        .post_form("/gliederung/act", &format!("method=outline.set_numbering&work={work}&node=1&numbering=I.1"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Nummerierung über das Formular gelingt");
    assert!(location.unwrap().starts_with(&format!("/?work={work}&key=")), "zurück zum Werk (Key-Form)");
    let (_, html) = server.get(&format!("/?work={work}&node=1")).await;
    assert!(html.contains("<span class=\"num\">II</span>Sommer"), "Kapitel jetzt römisch: {}", tree_pane(&html));
    assert!(tree_pane(&html).contains("name=\"numbering\" value=\"I.1\""), "das Feld zeigt das Muster");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "die Nummerierung ist genau ein Commit");
    }
    // „keine“ = keine Nummern.
    server
        .post_form("/gliederung/act", &format!("method=outline.set_numbering&work={work}&node=1&numbering=keine"))
        .await;
    let (_, html) = server.get(&format!("/?work={work}&node=1")).await;
    assert!(tree_pane(&html).contains("Nummern: keine"), "keine Nummern: {}", tree_pane(&html));
}

#[tokio::test]
async fn a_numbering_function_in_the_template_gives_roman_chapter_numbers() {
    let server = common::open().await;
    // Frei erfundene Vorlage: eine eigene Funktion zählt Kapitel römisch.
    let source = "#set document(title: \"Römisch gezählt\", author: \"Test\")\n\
                  #let kapitelzahl(..teile) = numbering(\"I\", ..teile)\n\
                  #set heading(numbering: kapitelzahl)\n\n\
                  = Anfang\n\nEins.\n\n= Mitte\n\nZwei.\n\n= Schluss\n\nDrei.\n";
    let report = server.import(source, "roemisch.typ").await;
    let (status, html) = server.get(&format!("/?work={}&node=0", report.work)).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    let tree = tree_pane(&html);
    for (number, title) in [("I", "Anfang"), ("II", "Mitte"), ("III", "Schluss")] {
        assert!(
            tree.contains(&format!("<span class=\"num\">{number}</span>{title}")),
            "Kapitel {title} trägt die römische Nummer {number}: {tree}"
        );
    }
    assert!(tree.contains("Nummern: I"), "der Kopf zeigt das Muster aus der Vorlage: {tree}");
}

// ---- Verschieben behält die Ebene ---------------------------------------------------------------

#[tokio::test]
async fn moving_keeps_the_level_over_rpc_and_without_javascript() {
    let (server, work, _) = garden().await;
    let refusal = "Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).";
    for (node, target, place, why) in [
        (2, 3, "into", "Unterabschnitt unter ein Kapitel"),
        (1, 3, "after", "Abschnitt hinter ein Kapitel"),
        (3, 1, "into", "Kapitel unter einen Abschnitt"),
        (3, 0, "into", "Kapitel unter ein Kapitel"),
    ] {
        let answer =
            server.rpc("outline.move", json!({ "work": work, "node": node, "target": target, "place": place })).await;
        assert_eq!(answer["error"]["code"], -32000, "{why}: abgelehnt: {answer}");
        assert_eq!(answer["error"]["message"], refusal, "{why}: die Meldung nennt den Weg: {answer}");
    }
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(titles(&loaded), ["Frühling", "Aussaat", "Tiefe", "Sommer"], "nichts verschoben");
    // Ohne JavaScript: 409 mit derselben Meldung.
    let (status, _) = server
        .post_form("/gliederung/act", &format!("method=outline.move&work={work}&node=2&target=3&place=into"))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "das Formular wird mit 409 abgelehnt");
    // Gleiche Ebene geht: Sommer vor Frühling.
    let ok = server.rpc("outline.move", json!({ "work": work, "node": 3, "target": 0, "place": "before" })).await;
    assert!(ok.get("error").is_none(), "Kapitel vor Kapitel: {ok}");
    // Das Urteil im Browser sagt dasselbe (am Quelltext geprüft).
    let script = include_str!("../web/src/outline.ts");
    assert!(script.contains(refusal), "judge() zeigt dieselbe Meldung wie der Server");
    assert!(
        script.contains("place === \"into\" ? targetLevel + 1 === level : targetLevel === level"),
        "judge() urteilt nach derselben Regel: davor/danach gleiche Ebene, darunter eine höher"
    );
}

// ---- Umschalten bringt die Ebene mit ------------------------------------------------------------

#[tokio::test]
async fn accepting_and_switching_answer_with_the_outline_and_write_one_commit() {
    let (server, work, dir) = garden().await;
    // Ein Vorschlag für Sommer; der Mensch ändert ihn danach zu einer Abschnittsüberschrift.
    let proposal = server
        .state
        .store
        .add_proposal(work, 3, "= Sommer\n\nVorschlag.\n", "@lektor", "probe", "Probe")
        .await
        .unwrap();
    let changed = server
        .rpc(
            "revision.update",
            json!({ "work": work, "node": 3, "rev": proposal, "body": "== Sommer\n\nVorschlag.\n" }),
        )
        .await;
    assert_eq!(changed["result"]["changed"], true, "ein Vorschlag lässt sich vom Menschen ändern: {changed}");
    let before = commits(&dir);
    let accepted = server.rpc("revision.accept", json!({ "work": work, "node": 3, "rev": proposal })).await;
    assert!(accepted.get("error").is_none(), "Übernehmen mit anderer Ebene gelingt: {accepted}");
    assert_eq!(accepted["result"]["outline"]["action"], "level", "die Antwort trägt die Gliederung: {accepted}");
    assert_eq!(accepted["result"]["outline"]["detail"], "„Sommer“ Ebene 1 → 2", "{accepted}");
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    let sommer = &loaded["result"]["work"]["nodes"][3];
    assert_eq!(sommer["kind"], "section", "Sommer ist jetzt Abschnitt: {sommer}");
    assert_eq!(sommer["parent"], 0, "unter Frühling: {sommer}");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "Übernehmen mit Ebenenwechsel ist genau ein Commit");
        let log = common::git(&dir, &["log", "-1", "--format=%s%n%b"]).unwrap();
        assert!(log.contains(&format!("Vorschlag {proposal} übernommen · Ebene 1 → 2")), "Betreff: {log}");
        assert!(log.contains("Baruch-Action: switched"), "auch Übernehmen mit Ebene trägt den Trailer: {log}");
    }
    // Zurück auf Fassung 1 (Kapitel): wieder ein Commit, Gliederung folgt.
    let back = server.rpc("revision.set_current", json!({ "work": work, "node": 3, "rev": 1 })).await;
    assert_eq!(back["result"]["outline"]["detail"], "„Sommer“ Ebene 2 → 1", "{back}");
    // Gleiche Ebene: keine Gliederungsantwort, aber weiter genau ein Commit je Umschaltung.
    let save = server.rpc("revision.save", json!({ "work": work, "node": 3, "body": "= Sommer\n\nNeu.\n" })).await;
    assert!(save.get("error").is_none(), "{save}");
    let before = commits(&dir);
    let plain = server.rpc("revision.set_current", json!({ "work": work, "node": 3, "rev": 1 })).await;
    assert!(plain.get("error").is_none(), "{plain}");
    assert!(plain["result"].get("outline").is_none(), "gleiche Ebene: keine Gliederungsantwort: {plain}");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "auch ohne Ebenenwechsel genau ein Commit");
    }
}

#[tokio::test]
async fn a_version_with_several_headings_is_refused_with_a_hint_to_the_editor() {
    let (server, work, _) = garden().await;
    server.rpc("revision.save", json!({ "work": work, "node": 3, "body": "= Sommer\n\nNeu.\n" })).await;
    let edit = json!({ "work": work, "node": 3, "rev": 1, "body": "= Sommer\n\nA.\n\n= Herbst\n\nB.\n" });
    assert_eq!(server.rpc("revision.update", edit).await["result"]["changed"], true, "alte Fassung frei änderbar");
    let refused = server.rpc("revision.set_current", json!({ "work": work, "node": 3, "rev": 1 })).await;
    assert_eq!(refused["error"]["code"], -32000, "weitere Überschriften: abgelehnt: {refused}");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(message.contains("„Herbst“") && message.contains("im Editor"), "Grund und Hinweis: {message}");
    let node = server.rpc("node.get", json!({ "work": work, "node": 3 })).await;
    assert_eq!(common::currents(&node["result"]), 1, "genau eine aktuelle Fassung");
    assert_eq!(node["result"]["revisions"][1]["status"], "current", "Fassung 2 bleibt aktuell: {node}");
}

// ---- TypeScript (am Quelltext, ohne Browser) ----------------------------------------------------

#[test]
fn the_outline_script_waits_for_before_save_and_asks_before_every_write() {
    let script = include_str!("../web/src/outline.ts");
    let dispatch = script
        .find("form.dispatchEvent(new CustomEvent(\"baruch:before-save\", { detail: { wait } }));")
        .expect("checkThenSubmit löst baruch:before-save mit detail.wait aus");
    let check = script.find("async function checkThenSubmit").expect("checkThenSubmit vorhanden");
    let read = check + script[check..].find("new FormData(form)").expect("liest das Formular");
    assert!(dispatch < read, "das Ereignis kommt vor dem Lesen des Formulars (Satzbild schreibt ins Textfeld)");
    assert!(script.contains("await Promise.all(wait);"), "es wartet auf alle Promises");
    // Jede schreibende Baumaktion geht über act(): erst leaveEditor(), dann RPC, dann baruch:navigate.
    let act = &script[script.find("async function act(").unwrap()..];
    let act = &act[..act.find("\n}\n").unwrap()];
    let ask = act.find("if (!leaveEditor()) return false;").expect("act fragt zuerst");
    let rpc = act.find("await call<Answer>(method").expect("act ruft RPC");
    assert!(ask < rpc, "leaveEditor() vor dem RPC-Aufruf");
    assert!(act.contains("navigate(`/?work=${work}&${place}`, true)"), "danach baruch:navigate, gefragt");
    assert!(act.contains("`key=${result.key}`"), "mit dem Schlüssel aus der Antwort (Runde 8)");
    assert!(script.contains("detail: { url, asked }"), "das Ereignis trägt url und asked");
    for method in [
        "outline.rename",
        "outline.add",
        "outline.delete",
        "outline.set_level",
        "outline.move",
        "outline.set_numbering",
    ] {
        assert!(script.contains(&format!("\"{method}\"")), "{method} läuft über act()");
        assert_eq!(
            script.matches(&format!("call<Answer>(\"{method}\"")).count(),
            0,
            "{method} ruft RPC nicht an act() vorbei"
        );
    }
    assert!(script.contains("confirm: true"), "Löschen schickt confirm: true");
    assert!(script.contains("if (!confirm(`„${title}“ mit allen Fassungen"), "Löschen fragt mit confirm()");
    // Tasten: F2/Entf/Kontextmenü nur im Baum, Alt+Umschalt+Pfeile außer in Eingabefeldern.
    assert!(
        script.contains("event.key === \"ContextMenu\" || (event.key === \"F10\" && event.shiftKey)"),
        "das Kontextmenü öffnet auch mit der Kontextmenü-Taste und Umschalt+F10"
    );
    assert!(script.contains("if (event.key === \"F2\") startRename(root, item);"), "F2 benennt um");
    assert!(script.contains("else if (event.key === \"Delete\") askDelete(root, item);"), "Entf löscht");
    assert!(script.contains("root.addEventListener(\"dblclick\""), "Doppelklick benennt um");
    assert!(
        script.contains("el.isContentEditable || el.matches(\"textarea, input, select\")"),
        "die Wache für Eingabefelder bleibt"
    );
    assert!(script.contains("\"outline.rename\", { node: nodeOf(item), key: keyOf(item), title: next }"), "Enter");
    assert!(script.contains("event.key === \"Escape\""), "Esc bricht ab");
}

/// Fester Schlüssel (`Node::key`) des Abschnitts an Stelle `seq`: Erzeugte Adressen tragen ihn statt
/// der Stelle (`/?work=W&key=K`, Vertrag Runde 8).
async fn key_at(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}
