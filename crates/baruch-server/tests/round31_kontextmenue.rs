// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 31c (kontextmenue): Kontextmenü im Satzbild und Quelltext, Glossar- und Registerseite, Formular „an der
//! Markierung“ ohne JavaScript, Menüband „Register und Glossar“, Registermarken im Satzbild, Satz-Einstellungen für
//! Register und Glossar.
//!
//! Die Seiten nutzen nur `crate::glossary` (glossar-daten) und `crate::index_marks` (register-satz). Seit dem
//! Zusammenführen gibt es beide Körper: Die Tests verlangen den Weg über das Formular bis zur Anzeige; die Zweige aus
//! der Schnittstellenzeit, die ein „noch nicht umgesetzt“ als richtig gelten ließen, sind entfernt (Nachbesserung R31,
//! wie in R27) – fiele ein RPC-Körper auf einen Stub zurück, wird der Test rot ([`implemented`]). Abgelehnte Formulare
//! (422 mit erhaltenen Eingaben) prüfen echte Fehler (unlesbare Bibelstelle, eingebaute Register-Kennung). Reine
//! Funktionen der Widgets laufen unter Node (fehlt `node`, „ÜBERSPRUNGEN“). Kein Netz, kein Modell. Werke frei erfunden.

use crate::common;

use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::settings::work_typeset::{form_from_settings, settings_from_form};
use baruch_typst::index::{IndexMark, mark_source, parse_mark};
use baruch_typst::typeset::{GlossaryPosition, IndexPages, TypesetSettings};
use serde_json::{Value, json};

/// Erfundenes Werk: zwei Kapitel, im zweiten schon eine Registermarke.
fn book(tag: &str) -> String {
    format!(
        "#set text(lang: \"de\")\n\n= Vom Warten\n\nDie Langmut der Gärtnerin ist groß. Sie wartet auf den Regen.\n\n\
         = Vom Ernten\n\nDie Ernte{} kommt im Herbst.\n\n// Test: {tag}\n",
        mark_source(&IndexMark { sub: "im Herbst".into(), ..IndexMark::new("stichwort", "Ernte") })
    )
}

async fn import(server: &common::Server, tag: &str) -> (i64, Vec<(String, i64, u32)>) {
    let report = server.import(&book(tag), &format!("{tag}.typ")).await;
    let work = server.state.store.load_work(report.work).await.unwrap();
    (report.work, work.nodes.iter().map(|n| (n.title.clone(), n.key, n.current_number().unwrap_or(1))).collect())
}

fn key_of(keys: &[(String, i64, u32)], title: &str) -> (i64, u32) {
    keys.iter()
        .find(|(t, _, _)| t == title)
        .map(|(_, k, r)| (*k, *r))
        .unwrap_or_else(|| panic!("kein Abschnitt {title}: {keys:?}"))
}

async fn post(server: &common::Server, uri: &str, form: &str) -> (StatusCode, Option<String>, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap();
    let (status, headers, body) = server.send(request).await;
    (status, headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned()), body)
}

/// Die RPC-Methode ist umgesetzt: Sie antwortet ohne Fehler (lesende Sonden ändern nichts). Ein Rückfall auf einen
/// Stub („noch nicht umgesetzt“) oder ein anderer Fehler macht den Test rot, statt einen zweiten, milden Zweig zu prüfen.
async fn implemented(server: &common::Server, method: &str, params: Value) {
    let answer = server.rpc(method, params).await;
    assert!(answer.get("error").is_none(), "{method} ist umgesetzt und antwortet ohne Fehler: {answer}");
}

/// Zahl der Fassungen, aktuelle Fassung und ihr Text des Abschnitts `key`.
async fn section_state(server: &common::Server, work: i64, key: i64) -> (usize, u32, String) {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = loaded.nodes.iter().find(|n| n.key == key).unwrap();
    let current = node.current_number().unwrap();
    (node.revisions.len(), current, node.revision(current).unwrap().body.clone())
}

/// Ausschnitt zwischen `from` und dem nächsten `to` (oder Ende).
fn cut<'a>(page: &'a str, from: &str, to: &str) -> &'a str {
    let start = page.find(from).unwrap_or_else(|| panic!("{from} fehlt: {page}"));
    let end = page[start..].find(to).map_or(page.len(), |e| start + e);
    &page[start..end]
}

// ---- Editor: Link ohne JavaScript, Menüband, Satzbild ------------------------------------------------------------

#[tokio::test]
async fn the_editor_offers_the_marking_link_without_javascript_and_the_ribbon_group() {
    let server = common::open().await;
    let (work, keys) = import(&server, "editor-link").await;
    let (key, rev) = key_of(&keys, "Vom Warten");
    let (status, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite lädt");
    let link = format!("data-ctx-open href=\"/werke/{work}/markierung?key={key}&amp;rev={rev}\"");
    assert!(
        page.contains(&link),
        "ohne JavaScript führt „Markierung …“ zum Formular mit Abschnitt und Fassung: {page}"
    );
    assert!(page.contains(">Markierung …</a>"), "deutsche Beschriftung des Links");
    let status_line = cut(&page, "<div class=\"editor-status\"", "</div>\n</div>");
    assert!(status_line.contains("data-ctx-open"), "der Link steht in der Statuszeile des Editors (nicht im Menüband)");
    // Runde 33 (Menüband): Die Leiste hat zwei Teile; die Gruppe steht im Teil „einfuegen“ als Gruppe des Bands
    // (<details class="band-group">, Name für Hilfsmittel an div.band-items), das Widget sitzt auf dem Rahmen darum.
    let ribbon = cut(&page, "data-edit-toolbar hidden>", "<div class=\"search-bar\"");
    let group = cut(ribbon, "<div class=\"ribbon-group ribbon-index\"", "</details>");
    assert!(
        group.starts_with("<div class=\"ribbon-group ribbon-index\" data-widget=\"context-menu\">")
            && group.contains("<div class=\"band-items\" role=\"group\" aria-label=\"Register und Glossar\">"),
        "Gruppe des Menübands mit Widget und Namen für Hilfsmittel: {group}"
    );
    assert!(
        ribbon[..ribbon.find("<div class=\"ribbon-group ribbon-index\"").unwrap()]
            .contains("data-band-part=\"einfuegen\""),
        "die Gruppe gehört zum Reiter Einfügen: {ribbon}"
    );
    for (action, label) in [
        ("glossary_add", ">In Glossar</button>"),
        ("index_add", ">In Register</button>"),
        ("glossary_lookup", ">Nachschlagen</button>"),
    ] {
        assert!(
            group.contains(&format!("data-ctx-action=\"{action}\"")) && group.contains(label),
            "Menüband-Knopf {action} mit deutscher Beschriftung: {group}"
        );
    }
    assert!(
        group.contains("data-index-toggle aria-expanded=\"false\" aria-controls=\"ribbon-register-list\"")
            && group.contains("id=\"ribbon-register-list\" data-index-entries")
            && group.matches("<details").count() == 1,
        "„Register“ als Knopf mit aria-expanded und Liste der Einträge (kein eigenes details; Runde 33: das einzige \
         details hier ist die Gruppe des Bands selbst): {group}"
    );
}

#[tokio::test]
async fn the_sheet_shows_marks_quietly_and_serialize_writes_them_canonically() {
    let server = common::open().await;
    let (work, _) = import(&server, "satzbild-marke").await;
    let body = format!("Die Ernte{} kommt.\n", mark_source(&IndexMark::new("stichwort", "Ernte")));
    let parsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": body })).await;
    let html = parsed["result"]["html"].as_str().unwrap_or_else(|| panic!("wysiwyg.parse: {parsed}"));
    assert!(
        html.contains("<span class=\"ws-index-mark\" contenteditable=\"false\" role=\"img\"")
            && html.contains("aria-label=\"Registereintrag im Stichwortregister: Ernte\""),
        "die Marke kommt als dezentes, nicht tippbares Element mit Beschreibung ins Satzbild: {html}"
    );
    // So schickt wysiwyg.ts eine geänderte Marke (ein Absatz neu, Feld sub dazu).
    let edits = json!([{ "replaces": 0, "block": { "kind": "paragraph", "inlines": [
        { "t": "text", "v": "Die Ernte" },
        { "t": "index_mark", "mark": { "register": "stichwort", "term": "Ernte", "sub": "Äpfel" } },
        { "t": "text", "v": " kommt." }
    ] } }]);
    let answer = server.rpc("wysiwyg.serialize", json!({ "work": work, "body": body, "edits": edits })).await;
    let expected = format!(
        "Die Ernte{} kommt.\n",
        mark_source(&IndexMark { sub: "Äpfel".into(), ..IndexMark::new("stichwort", "Ernte") })
    );
    assert_eq!(answer["result"]["body"], json!(expected), "der Server schreibt die Marke kanonisch: {answer}");
    let (_, js) = server.get("/static/js/wysiwyg.js").await;
    assert!(
        js.contains("classList.contains(\"ws-index-mark\")") && js.contains("t: \"index_mark\""),
        "das Widget liest Marken als index_mark (nicht als Insel) zurück"
    );
    assert!(!js.contains("context_menu.js"), "kein Import zwischen Satzbild und Kontextmenü (nur baruch:ws-command)");
}

// ---- Seite „an der Markierung“ ------------------------------------------------------------------------------------

#[tokio::test]
async fn the_marking_page_offers_the_three_actions_as_forms() {
    let server = common::open().await;
    let (work, keys) = import(&server, "markierung").await;
    let (key, rev) = key_of(&keys, "Vom Warten");
    let (status, page) = server.get(&format!("/werke/{work}/markierung?key={key}&rev={rev}&wort=Langmut")).await;
    assert_eq!(status, StatusCode::OK, "Seite lädt ohne JavaScript: {page}");
    assert!(!page.contains("STUB") && !page.contains("data-marking-stub"), "kein Gerüst mehr: {page}");
    assert!(page.contains("<strong>Vom Warten</strong>, Fassung"), "nennt Abschnitt und Fassung: {page}");
    let glossary = cut(&page, "id=\"glossar\"", "</section>");
    assert!(
        glossary.contains(&format!("action=\"/werke/{work}/glossar/act\""))
            && glossary.contains("name=\"action\" value=\"add\"")
            && glossary.contains("name=\"term\" maxlength=\"200\" required value=\"Langmut\""),
        "„In Glossar aufnehmen“: Formular mit vorbelegtem Begriff: {glossary}"
    );
    assert!(
        glossary.contains(&format!("name=\"back\" value=\"/werke/{work}/markierung?key={key}&amp;wort=Langmut\"")),
        "nach Erfolg zurück an die Markierung (aktuelle Fassung): {glossary}"
    );
    let register = cut(&page, "id=\"register\"", "</section>");
    for needle in [
        format!("action=\"/werke/{work}/register/act\""),
        "name=\"action\" value=\"mark\"".to_owned(),
        format!("name=\"key\" value=\"{key}\""),
        format!("name=\"rev\" value=\"{rev}\""),
        "name=\"word\" maxlength=\"200\" required value=\"Langmut\"".to_owned(),
        "<option value=\"stichwort\" selected>Stichwortregister</option>".to_owned(),
        "<option value=\"personen\">Personenregister</option>".to_owned(),
        "name=\"sub\"".to_owned(),
        "name=\"see\"".to_owned(),
        "name=\"also\"".to_owned(),
    ] {
        assert!(register.contains(&needle), "„In Register aufnehmen“ enthält {needle}: {register}");
    }
    let lookup = cut(&page, "id=\"nachschlagen\"", "</section>");
    assert!(
        lookup.contains(&format!("method=\"get\" action=\"/werke/{work}/glossar\""))
            && lookup.contains("name=\"nachschlagen\" required value=\"Langmut\""),
        "„Im Glossar nachschlagen“ als GET-Formular: {lookup}"
    );
    // Registereinträge des Abschnitts: Liste mit Entfernen oder die ehrliche Meldung von index.entries.
    let entries = cut(&page, "id=\"eintraege\"", "</section>");
    implemented(&server, "index.entries", json!({ "work": work, "key": key })).await;
    assert!(!entries.contains("data-marking-entries-error"), "keine Fehlermeldung statt der Liste: {entries}");
    assert!(entries.contains("Keine Registereinträge."), "im ersten Kapitel steht keine Marke: {entries}");
}

#[tokio::test]
async fn the_marking_page_names_missing_sections_and_versions_and_defaults_to_the_current_one() {
    let server = common::open().await;
    let (work, keys) = import(&server, "markierung-fehlt").await;
    let (key, rev) = key_of(&keys, "Vom Ernten");
    let (status, page) = server.get(&format!("/werke/{work}/markierung")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "ohne key 400: {page}");
    assert!(page.contains("Der Link braucht key"), "verständliche Meldung: {page}");
    let (status, page) = server.get(&format!("/werke/{work}/markierung?key=999999")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekannter Abschnitt 404");
    assert!(page.contains("Abschnitt mit Schlüssel 999999 gibt es nicht"), "Meldung nennt den Schlüssel: {page}");
    let (status, _) = server.get(&format!("/werke/{work}/markierung?key={key}&rev=77")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekannte Fassung 404");
    let (status, page) = server.get(&format!("/werke/{work}/markierung?key={key}")).await;
    assert_eq!(status, StatusCode::OK, "ohne rev gilt die aktuelle Fassung");
    assert!(page.contains(&format!("name=\"rev\" value=\"{rev}\"")), "aktuelle Fassung im Formular: {page}");
    let (status, _) = server.get("/werke/987654/markierung?key=1").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk 404");
}

// ---- Glossarseite -------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_glossary_page_has_every_form_and_shows_the_honest_state_of_the_data() {
    let server = common::open().await;
    let (work, _) = import(&server, "glossar-seite").await;
    let (status, page) = server.get(&format!("/werke/{work}/glossar?begriff=Langmut")).await;
    assert_eq!(status, StatusCode::OK, "Glossarseite lädt ohne JavaScript");
    assert!(!page.contains("data-glossary-stub"), "kein Gerüst mehr");
    assert!(page.contains("data-widget=\"glossary\""), "Komfort-Widget (Filter) sitzt auf der Seite");
    assert!(
        page.contains("<p class=\"glossary-filter\" hidden>"),
        "das Suchfeld des Widgets ist ohne JavaScript verborgen (es filtert nur mit JavaScript)"
    );
    let new = cut(&page, "id=\"neu\"", "</section>");
    assert!(
        new.contains("name=\"action\" value=\"add\"")
            && new.contains("name=\"term\" maxlength=\"200\" required value=\"Langmut\"")
            && new.contains("name=\"variants\"")
            && new.contains("name=\"bible_refs\""),
        "Formular „Neuer Begriff“ mit vorbelegtem Begriff (begriff=…), Varianten und Bibelstellen je Zeile: {new}"
    );
    assert!(
        page.contains(&format!("method=\"get\" action=\"/werke/{work}/glossar\" role=\"search\"")),
        "Nachschlagen als GET-Formular"
    );
    implemented(&server, "glossary.list", json!({ "work": work })).await;
    assert!(!page.contains("data-glossary-load-error"), "das Glossar lässt sich lesen: {page}");
    assert!(page.contains("Begriffe (0)"), "neues Werk ohne Begriffe: {page}");
}

#[tokio::test]
async fn glossary_forms_redirect_with_a_hint_or_answer_422_with_the_inputs() {
    let server = common::open().await;
    let (work, _) = import(&server, "glossar-formulare").await;
    let uri = format!("/werke/{work}/glossar/act");
    // Vor jedem Aufruf geprüft (unabhängig von glossar-daten): Löschen nur mit Häkchen, unbekannte Aktion.
    let (status, _, page) = post(&server, &uri, "action=delete&id=1").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Löschen ohne Häkchen → 422");
    assert!(page.contains("Löschen bitte mit dem Häkchen bestätigen"), "Meldung beim Formular: {page}");
    let (status, _, _) = post(&server, &uri, "action=raten").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannte Aktion → 400");
    let (status, _, page) = post(&server, &uri, "action=update&term=X").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Ändern ohne Begriff-id → 422");
    assert!(page.contains("Begriff fehlt (Feld id)"), "Meldung nennt das fehlende Feld: {page}");

    let form = "action=add&term=Langmut&short=Geduld+mit+anderen&variants=langm%C3%BCtig%0AGeduld&bible_refs=Gal+5%2C22\
                &back=%2Fwerke%2F{work}%2Fmarkierung%3Fkey%3D1";
    let form = form.replace("{work}", &work.to_string());
    let (status, location, page) = post(&server, &uri, &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Erfolg → 303: {page}");
    let location = location.expect("Weiterleitung");
    assert!(
        location.starts_with(&format!("/werke/{work}/markierung?key=1&ok=add#begriff-")),
        "zurück an die Markierung, Hinweis vor dem Anker des neuen Begriffs: {location}"
    );
    let (_, page) = server.get(&format!("/werke/{work}/glossar?ok=add")).await;
    assert!(page.contains("Begriff ins Glossar aufgenommen."), "Hinweis nach Erfolg: {page}");
    assert!(page.contains("<h3 class=\"glossary-term-title\">Langmut</h3>"), "Begriff in der Liste: {page}");
    assert!(page.contains("auch: langmütig, Geduld"), "Varianten je Zeile übernommen: {page}");

    // Ein echter Fehler des RPC (unbekannte Bibelstelle): 422 mit der Meldung, die Eingaben bleiben erhalten.
    let (status, _, page) = post(
        &server,
        &uri,
        "action=add&term=Sanftmut&short=Milde+im+Umgang&variants=sanftm%C3%BCtig%0AMilde&bible_refs=Hesekiel+99%2C1",
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unlesbare Bibelstelle → 422 statt stiller Weiterleitung");
    assert!(page.contains("data-glossary-error") && page.contains("Hesekiel"), "Meldung nennt die Stelle: {page}");
    let new = cut(&page, "id=\"neu\"", "</section>");
    assert!(
        new.contains("value=\"Sanftmut\"") && new.contains("Milde im Umgang") && new.contains("sanftmütig\nMilde"),
        "die Eingaben bleiben erhalten (Begriff, Erklärung, Varianten je Zeile): {new}"
    );
    assert!(
        server.state.store.glossary(work, None).await.unwrap().iter().all(|t| t.term != "Sanftmut"),
        "nichts gespeichert"
    );
}

// ---- Registerseite ------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_register_page_lists_every_register_with_anchors_and_management_forms() {
    let server = common::open().await;
    let (work, keys) = import(&server, "register-seite").await;
    let (key, rev) = key_of(&keys, "Vom Ernten");
    let (status, page) = server.get(&format!("/werke/{work}/register?key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Registerseite lädt ohne JavaScript");
    assert!(!page.contains("noch nicht umgesetzt (R31 kontextmenue)"), "kein Gerüst der eigenen Aufgabe mehr: {page}");
    for (slug, title) in
        [("stichwort", "Stichwortregister"), ("personen", "Personenregister"), ("bibel", "Bibelstellenregister")]
    {
        assert!(page.contains(&format!("id=\"register-{slug}\"")), "Anker #register-{slug}: {page}");
        assert!(
            page.contains(&format!("<a href=\"#register-{slug}\">{title}</a>")),
            "Inhaltsverzeichnis nennt {title}"
        );
        assert!(
            page.contains(&format!("{title}</span> <span class=\"muted tiny\">eingebaut ({slug})")),
            "eingebaut: {slug}"
        );
    }
    let manage = cut(&page, "id=\"eigene\"", "</section>");
    assert!(
        manage.contains("name=\"action\" value=\"register_add\"")
            && manage.contains("pattern=\"[a-z][a-z0-9\\-]{0,31}\"")
            && manage.contains("Noch 13 eigene Register möglich"),
        "eigene Register anlegen (Kennung, Titel, höchstens 13): {manage}"
    );
    assert!(!manage.contains("value=\"register_delete\""), "eingebaute Register lassen sich nicht löschen: {manage}");
    let section = cut(&page, "id=\"abschnitt\"", "</section>");
    assert!(
        section.contains("Vom Ernten") && section.contains(&format!("name=\"rev\" value=\"{rev}\"")),
        "mit key: der Abschnitt mit Formular „Marke setzen“ auf seiner aktuellen Fassung: {section}"
    );
    implemented(&server, "index.overview", json!({ "work": work })).await;
    assert!(!page.contains("data-index-overview-error"), "die Übersicht lässt sich lesen: {page}");
    let keyword = cut(&page, "id=\"register-stichwort\"", "</section>");
    assert!(keyword.contains("Ernte ›</span> im Herbst"), "Unterstichwort eingerückt unter dem Stichwort: {keyword}");
    // Seit der Sichtprüfung R31 nennt der Verweis Nummer und Titel des Abschnitts statt „k<Schlüssel>“.
    assert!(
        keyword.contains(&format!("key={key}\">2 Vom Ernten</a>")),
        "Eintrag verweist auf seinen Abschnitt (Nummer und Titel): {keyword}"
    );
    implemented(&server, "index.proposals", json!({ "work": work })).await;
    assert!(page.contains("Keine offenen Registervorschläge."), "neues Werk ohne Vorschläge: {page}");
}

#[tokio::test]
async fn register_forms_check_their_inputs_and_keep_them_on_422() {
    let server = common::open().await;
    let (work, keys) = import(&server, "register-formulare").await;
    let (key, rev) = key_of(&keys, "Vom Warten");
    let uri = format!("/werke/{work}/register/act");
    let (status, _, page) = post(
        &server,
        &uri,
        &format!("action=mark&key={key}&rev={rev}&word=Langmut&occurrence=dritte&register=stichwort&sub=Geduld"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unlesbare Fundstelle → 422");
    assert!(page.contains("Fundstelle: „dritte“"), "Meldung nennt die Eingabe: {page}");
    let section = cut(&page, "id=\"abschnitt\"", "</section>");
    assert!(
        section.contains("value=\"Langmut\"") && section.contains("value=\"Geduld\""),
        "die Eingaben stehen wieder im Formular des Abschnitts: {section}"
    );
    let (status, _, page) =
        post(&server, &uri, &format!("action=mark&key={key}&rev={rev}&word=Langmut&register=Orte")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültige Register-Kennung → 422");
    assert!(page.contains("keine gültige Kennung"), "Meldung von IndexMark::validate: {page}");
    let (status, _, page) = post(&server, &uri, "action=register_delete&slug=orte").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Löschen ohne Häkchen → 422");
    assert!(page.contains("Löschen bitte mit dem Häkchen bestätigen"), "Meldung: {page}");
    let (status, _, _) = post(&server, &uri, "action=raten").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannte Aktion → 400");

    let (status, location, page) =
        post(&server, &uri, &format!("action=mark&key={key}&rev={rev}&word=Langmut&term=Geduld&sub=Gärtnerin")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Marke gesetzt → 303: {page}");
    assert_eq!(location.as_deref(), Some(format!("/werke/{work}/register?key={key}&ok=mark").as_str()), "zurück");
    let marked = IndexMark { sub: "Gärtnerin".into(), ..IndexMark::new("stichwort", "Geduld") };
    let (revisions, rev_now, current) = section_state(&server, work, key).await;
    assert!(current.contains(&mark_source(&marked)), "neue Fassung des Menschen mit der Marke: {current}");

    // Dasselbe Formular noch einmal (Doppelklick, Zurück-Taste): keine zweite, gleiche Marke, keine neue Fassung,
    // sondern 422 mit verständlicher Meldung (Befund nach R31).
    let (status, _, page) = post(
        &server,
        &uri,
        &format!("action=mark&key={key}&rev={rev_now}&word=Langmut&term=Geduld&sub=G%C3%A4rtnerin"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "schon markiert → 422: {page}");
    assert!(page.contains("schon diese Registermarke"), "Meldung erklärt es: {page}");
    let (revisions_after, _, body_after) = section_state(&server, work, key).await;
    assert_eq!(revisions_after, revisions, "keine neue Fassung");
    assert_eq!(body_after, current, "Text unverändert, also keine zweite Marke");

    let (status, _, _) = post(&server, &uri, "action=register_add&slug=orte&title=Ortsregister").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Register angelegt → 303");
    // Ein echter Fehler (eingebaute Kennung): 422, Eingaben bleiben.
    let (status, _, page) = post(&server, &uri, "action=register_add&slug=stichwort&title=Schlagworte").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "eingebaute Kennung → 422: {page}");
    let add = cut(&page, "class=\"index-register-add\"", "</form>");
    assert!(add.contains("value=\"stichwort\"") && add.contains("value=\"Schlagworte\""), "Eingaben bleiben: {add}");
}

#[tokio::test]
async fn every_new_post_refuses_a_foreign_origin() {
    let server = common::open().await;
    let (work, _) = import(&server, "herkunft").await;
    for (uri, form) in [
        (format!("/werke/{work}/glossar/act"), "action=add&term=Fremd"),
        (format!("/werke/{work}/register/act"), "action=register_add&slug=fremd&title=Fremd"),
    ] {
        let request = Request::post(&uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::HOST, "127.0.0.1:8765")
            .header(header::ORIGIN, "https://evil.example")
            .body(Body::from(form))
            .unwrap();
        let (status, _, _) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri} mit fremder Herkunft → 403 (guard::same_origin)");
    }
}

// ---- Satz-Einstellungen -------------------------------------------------------------------------------------------

fn pairs(form: &[(&str, &str)]) -> Vec<(String, String)> {
    form.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

#[test]
fn the_r31_fields_survive_the_form_round_trip_in_their_order() {
    let s = TypesetSettings {
        glossary_page: Some(GlossaryPosition::BeforeIndexes),
        index_pages: Some(IndexPages {
            registers: vec!["personen".into(), "orte".into(), "stichwort".into()],
            columns: 3,
        }),
        glossary_first_mention: Some(false),
        ..Default::default()
    };
    let form = form_from_settings(&s);
    let registers: Vec<&str> = form.iter().filter(|(k, _)| k == "index_register").map(|(_, v)| v.as_str()).collect();
    assert_eq!(registers, ["personen", "orte", "stichwort"], "je Register ein Häkchenfeld, in Satzreihenfolge");
    assert_eq!(
        settings_from_form(&form).expect("gültig"),
        s,
        "Rundreise verliert nichts, auch nicht „ausdrücklich aus“"
    );
    let defaults = settings_from_form(&pairs(&[("index_register", "stichwort"), ("index_columns", "")])).unwrap();
    assert_eq!(
        defaults.index_pages,
        Some(IndexPages { registers: vec!["stichwort".into()], columns: 2 }),
        "Spalten leer = 2"
    );
    let none =
        settings_from_form(&pairs(&[("index_columns", "3"), ("glossary_page", ""), ("glossary_first_mention", "")]))
            .unwrap();
    assert!(none.is_empty(), "ohne Häkchen keine Registerseiten, leere Wahl = keine Glossarseite: {none:?}");
}

#[test]
fn invalid_r31_values_report_the_messages_of_validate() {
    let errors = settings_from_form(&pairs(&[
        ("glossary_page", "mittendrin"),
        ("index_register", "stichwort"),
        ("index_register", "stichwort"),
        ("index_columns", "5"),
        ("glossary_first_mention", "vielleicht"),
    ]))
    .expect_err("mehrere Fehler");
    let reference = TypesetSettings {
        index_pages: Some(IndexPages { registers: vec!["stichwort".into(), "stichwort".into()], columns: 5 }),
        ..Default::default()
    }
    .validate()
    .unwrap_err();
    for expected in &reference {
        assert!(
            errors.iter().any(|e| e.field == expected.field && e.message == expected.message),
            "Meldung von TypesetSettings::validate „{}“ kommt auch aus dem Formular: {errors:?}",
            expected.message
        );
    }
    assert!(
        errors.iter().any(|e| e.field == "glossary_page" && e.message == "Glossarseite: unbekannte Wahl „mittendrin“"),
        "unbekannte Wahl mit Bezeichnung: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.field == "glossary_first_mention" && e.message.contains("unbekannte Wahl „vielleicht“")),
        "Schalter mit unbekanntem Wert: {errors:?}"
    );
    let bad =
        settings_from_form(&pairs(&[("index_register", "Orte!"), ("index_columns", "zwei")])).expect_err("ungültig");
    assert!(
        bad.iter()
            .any(|e| e.field == "index_pages.registers"
                && e.message.contains("„Orte!“ ist keine gültige Register-Kennung")),
        "ungültige Kennung: {bad:?}"
    );
    assert!(
        bad.iter().any(|e| e.field == "index_pages.columns" && e.message == "Registerseiten: 1 bis 3 Spalten"),
        "keine Zahl → dieselbe Meldung wie ein Wert außerhalb der Grenzen: {bad:?}"
    );
}

#[tokio::test]
async fn the_typeset_block_offers_register_and_glossary_and_shows_errors_at_the_field() {
    let server = common::open().await;
    let (work, _) = import(&server, "satz-register").await;
    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Einstellungsseite lädt");
    let group = cut(&page, "<fieldset class=\"typeset-index\">", "</fieldset>");
    assert!(group.contains("<legend class=\"small\">Register und Glossar</legend>"), "eigene Gruppe: {group}");
    for label in ["Glossarseite", "Registerseiten", "Erste Erwähnung im Glossar auszeichnen"] {
        assert!(group.contains(label), "Bezeichnung „{label}“ aus FIELDS: {group}");
    }
    for option in [
        "<option value=\"\" selected>keine</option>",
        "<option value=\"before_indexes\">vor den Registern</option>",
        "<option value=\"after_indexes\">nach den Registern</option>",
    ] {
        assert!(group.contains(option), "Glossarseite aus/vor/nach den Registern: {option}: {group}");
    }
    for slug in ["stichwort", "personen", "bibel"] {
        assert!(
            group.contains(&format!("<input type=\"checkbox\" name=\"index_register\" value=\"{slug}\">")),
            "Häkchen je Register aus glossary::registers ({slug}), ohne Einstellung nicht gewählt: {group}"
        );
    }
    assert!(
        group.contains("<option value=\"2\" selected>2</option>") && group.contains("<option value=\"3\">3</option>"),
        "Spalten 1–3, Vorgabe 2"
    );
    assert!(
        !group.contains("aus der Werkvorlage"),
        "für Register und Glossar gibt es keine Regel der Werkvorlage (die Zählung der Vorlagen-Wahlen bleibt bei 11)"
    );
    // Ungültige Spaltenzahl: 422, Meldung von validate beim Feld, Eingaben bleiben.
    let (status, _, page) = post(
        &server,
        "/einstellungen/werk/satz",
        &format!("work={work}&action=save&index_register=personen&index_register=stichwort&index_columns=4"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültig → 422");
    let group = cut(&page, "<fieldset class=\"typeset-index\">", "</fieldset>");
    assert!(
        group.contains("data-typeset-error=\"index_pages\">Registerseiten: 1 bis 3 Spalten</span>"),
        "Meldung von TypesetSettings::validate beim Feld: {group}"
    );
    let personen = group.find("value=\"personen\" checked").expect("Personen gewählt");
    let stichwort = group.find("value=\"stichwort\" checked").expect("Stichwort gewählt");
    assert!(personen < stichwort, "die gewählten Register stehen in der eingegebenen Reihenfolge vorn: {group}");
    // Gültig speichern und wieder zeigen.
    let (status, location, page) = post(
        &server,
        "/einstellungen/werk/satz",
        &format!(
            "work={work}&action=save&glossary_page=after_indexes&index_register=personen&index_register=stichwort\
             &index_columns=1&glossary_first_mention=on"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert → 303: {page}");
    assert!(location.unwrap().ends_with("&ok=satz#satz"), "zurück zum Block");
    let stored = server.rpc("work.typeset_get", json!({ "work": work })).await;
    let settings: TypesetSettings = serde_json::from_value(stored["result"]["settings"].clone())
        .unwrap_or_else(|e| panic!("Einstellungen lesbar ({e}): {stored}"));
    assert_eq!(settings.glossary_page, Some(GlossaryPosition::AfterIndexes), "Glossarseite gespeichert");
    assert_eq!(
        settings.index_pages,
        Some(IndexPages { registers: vec!["personen".into(), "stichwort".into()], columns: 1 }),
        "Register in Reihenfolge und Spalten gespeichert"
    );
    assert_eq!(settings.glossary_first_mention, Some(true), "erste Erwähnung gespeichert");
    let (_, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    let group = cut(&page, "<fieldset class=\"typeset-index\">", "</fieldset>");
    assert!(group.contains("<option value=\"after_indexes\" selected>"), "gespeicherte Glossarseite gewählt: {group}");
    assert!(
        group.find("value=\"personen\" checked").unwrap() < group.find("value=\"stichwort\" checked").unwrap()
            && group.contains("<input type=\"checkbox\" name=\"index_register\" value=\"bibel\">"),
        "gespeicherte Register gewählt und vorn, das übrige nicht: {group}"
    );
    assert!(group.contains("<option value=\"on\" selected>ja</option>"), "erste Erwähnung gewählt: {group}");
}

// ---- Widgets unter Node -------------------------------------------------------------------------------------------

/// Ruft einen Ausdruck mit den ausgelieferten Modulen unter Node auf; `None`, wenn `node` fehlt (dann „ÜBERSPRUNGEN“).
fn node(module: &str, expr: &str) -> Option<Value> {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("ÜBERSPRUNGEN: node fehlt");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    for (name, source) in baruch_server::SCRIPTS {
        std::fs::write(dir.path().join(name.replace(".js", ".mjs")), source.replace(".js\"", ".mjs\"")).unwrap();
    }
    let script =
        format!("import * as m from './{}';\nconsole.log(JSON.stringify({expr}));\n", module.replace(".js", ".mjs"));
    std::fs::write(dir.path().join("probe.mjs"), script).unwrap();
    let out = Command::new("node").arg("probe.mjs").current_dir(dir.path()).output().expect("node läuft");
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    Some(serde_json::from_slice(&out.stdout).unwrap())
}

#[test]
fn the_menu_follows_the_aria_menu_pattern() {
    let Some(items) = node("context_menu.js", "[m.menuItems(false), m.menuItems(true)]") else { return };
    let labels = |list: &Value| list.as_array().unwrap().iter().map(|i| i["label"].clone()).collect::<Vec<_>>();
    assert_eq!(
        labels(&items[0]),
        [json!("In Glossar aufnehmen …"), json!("In Register aufnehmen …"), json!("Im Glossar nachschlagen")],
        "die drei Aktionen in fester Reihenfolge"
    );
    assert_eq!(
        labels(&items[1])[3..],
        [json!("Registereintrag bearbeiten …"), json!("Registereintrag entfernen")],
        "auf einer Marke zusätzlich bearbeiten und entfernen"
    );
    let keys = node(
        "context_menu.js",
        "[m.opensMenu({key: 'ContextMenu', shiftKey: false}), m.opensMenu({key: 'F10', shiftKey: true}), \
          m.opensMenu({key: 'F10', shiftKey: false}), m.opensMenu({key: 'F10', shiftKey: true, ctrlKey: true}), \
          m.opensMenu({key: 'a', shiftKey: false})]",
    )
    .unwrap();
    assert_eq!(
        keys,
        json!([true, true, false, false, false]),
        "Kontextmenü-Taste und Umschalt+F10 öffnen, sonst nichts"
    );
    let moves = node(
        "context_menu.js",
        "['ArrowDown', 'ArrowUp', 'Home', 'End', 'Escape', 'Tab', 'Enter', ' ', 'x'].map((k) => m.menuKey(k, 0, 3))\
         .concat([m.menuKey('ArrowDown', 2, 3)])",
    )
    .unwrap();
    assert_eq!(
        moves,
        json!([
            { "index": 1 }, { "index": 2 }, { "index": 0 }, { "index": 2 },
            { "close": true }, { "close": true }, { "activate": true }, { "activate": true }, null, { "index": 0 }
        ]),
        "Pfeile laufen um, Pos1/Ende springen, Escape und Tab schließen, Enter/Leertaste lösen aus, Fremdes gehört nicht dem Menü"
    );
}

#[test]
fn the_widget_writes_and_reads_marks_exactly_like_the_server() {
    let marks = [
        IndexMark::new("stichwort", "Gnade"),
        IndexMark {
            sub: "im Alltag".into(),
            see: "Huld".into(),
            also: "Liebe".into(),
            ..IndexMark::new("personen", "Rut")
        },
        IndexMark::new("orte-und-wege", "„Zitat“ mit \"Anführung\" und \\ Schrägstrich"),
        IndexMark { sort: "01001".into(), ..IndexMark::new("bibel", "Gen 1,1") },
    ];
    let js_marks = serde_json::to_string(&marks).unwrap();
    let Some(sources) = node("ws_command.js", &format!("{js_marks}.map((x) => m.markSource(m.cleanMark(x)))")) else {
        return;
    };
    let expected: Vec<String> = marks.iter().map(mark_source).collect();
    assert_eq!(sources, json!(expected), "markSource (TS) = mark_source (Rust), auch mit Maskierungen");
    let labels = node("ws_command.js", &format!("{js_marks}.map((x) => m.markLabel(m.cleanMark(x)))")).unwrap();
    let expected: Vec<String> = marks.iter().map(baruch_wysiwyg::index_mark_label).collect();
    assert_eq!(
        labels,
        json!(expected),
        "markLabel (TS) = index_mark_label (Rust): gleiche Beschreibung nach dem Bearbeiten"
    );
    let probes = [
        mark_source(&marks[1]),
        r#"#metadata(( register : "stichwort" ,  term: "Wort" ))<baruch-idx>"#.to_owned(),
        r#"#metadata((term: "Wort", register: "stichwort"))<baruch-idx>"#.to_owned(),
        r#"#metadata((register: "stichwort", term: "Wort")) <baruch-idx>"#.to_owned(),
        r#"#metadata((register: "stichwort", term: "A\u{9}B"))<baruch-idx>"#.to_owned(),
        r#"#metadata((register: "Stichwort", term: "Wort"))<baruch-idx>"#.to_owned(),
        r#"#metadata((register: "stichwort", term: "Ä\u{e4}"))<baruch-idx> danach"#.to_owned(),
    ];
    let js = serde_json::to_string(&probes).unwrap();
    let read = node("ws_command.js", &format!("{js}.map((t) => m.parseMarkAt(t, 0)?.mark ?? null)")).unwrap();
    let expected: Vec<Value> = probes
        .iter()
        .map(|p| parse_mark(p).map_or(Value::Null, |(mark, _)| serde_json::to_value(mark).unwrap()))
        .collect();
    assert_eq!(
        read,
        json!(expected),
        "parseMarkAt (TS) liest dieselben Marken wie parse_mark (Rust), Fremdes bleibt null"
    );
    let bad = [
        IndexMark::new("Orte", "x"),
        IndexMark::new("stichwort", "  "),
        IndexMark::new("stichwort", &"a".repeat(201)),
        IndexMark { sub: "zwei\nZeilen".into(), ..IndexMark::new("stichwort", "x") },
    ];
    let messages = node(
        "ws_command.js",
        &format!("{}.map((x) => m.validateMark(m.cleanMark(x)))", serde_json::to_string(&bad).unwrap()),
    )
    .unwrap();
    let expected: Vec<String> = bad.iter().map(|m| m.validate().unwrap_err().to_string()).collect();
    assert_eq!(messages, json!(expected), "validateMark (TS) meldet wie IndexMark::validate (Rust)");
}

#[test]
fn a_mark_in_the_source_lands_directly_behind_the_word_and_never_in_a_heading() {
    let mark = r#"{register: "stichwort", term: "Langmut"}"#;
    let text = "= Vom Warten\n\nDie Langmut ist groß. // Notiz Langmut\n";
    let Some(result) = node(
        "ws_command.js",
        &format!(
            "(() => {{ const t = {text:?}; const at = t.indexOf('Langmut'); return [\
               m.markInSource(t, at, at + 7, {mark}), m.markInSource(t, at + 3, at + 3, {mark}), \
               m.markInSource(t, at - 1, at + 8, {mark}), m.markInSource(t, 4, 4, {mark}), \
               m.markInSource(t, t.lastIndexOf('Langmut'), t.lastIndexOf('Langmut') + 7, {mark}), \
               m.markInSource(t, 0, 0, {{register: 'x y', term: 'z'}})]; }})()"
        ),
    ) else {
        return;
    };
    let source = mark_source(&IndexMark::new("stichwort", "Langmut"));
    let marked = format!("= Vom Warten\n\nDie Langmut{source} ist groß. // Notiz Langmut\n");
    assert_eq!(result[0]["text"], json!(marked), "ausgewähltes Wort: Marke direkt dahinter");
    assert_eq!(result[1]["text"], json!(marked), "Cursor im Wort: Marke hinter das ganze Wort");
    assert_eq!(result[2]["text"], json!(marked), "Leerraum am Rand der Auswahl zählt nicht");
    assert_eq!(result[0]["insert"], json!(source), "eingefügt wird genau mark_source");
    assert!(
        result[3]["error"].as_str().unwrap().contains("Überschriftszeile"),
        "nie in einer Überschrift: {}",
        result[3]
    );
    assert!(result[4]["error"].as_str().unwrap().contains("Kommentar"), "nie in einem Kommentar: {}", result[4]);
    assert!(result[5]["error"].as_str().unwrap().contains("keine gültige Kennung"), "ungültige Marke: {}", result[5]);
    let found = node(
        "ws_command.js",
        &format!("m.findMarks({:?}).map((f) => [f.start, f.end, f.mark.term])", format!("A{source} b{source}")),
    )
    .unwrap();
    let one = source.encode_utf16().count();
    assert_eq!(
        found,
        json!([[1, 1 + one, "Langmut"], [3 + one, 3 + 2 * one, "Langmut"]]),
        "alle Marken mit Lage (UTF-16)"
    );
    let inside = node("ws_command.js", &format!("m.markRefusal({:?}, 5)", format!("A{source}"))).unwrap();
    assert_eq!(inside, json!("Hier steht schon eine Registermarke."), "nie in einer anderen Marke");
    // Nachbesserung R31: dieselbe Marke nicht zweimal hinter dasselbe Wort, auch nicht hinter einer anderen Marke;
    // eine andere Marke (anderes Unterstichwort) bleibt erlaubt.
    let other = mark_source(&IndexMark { sub: "Geduld".into(), ..IndexMark::new("stichwort", "Langmut") });
    let twice = node(
        "ws_command.js",
        &format!(
            "(() => {{ const a = {:?}; const b = {:?}; return [m.markInSource(a, 4, 4, {mark}),                m.markInSource(b, 4, 4, {mark}), m.markInSource(a, 4, 4, {{register: 'stichwort', term: 'Langmut',                sub: 'Geduld'}}).insert]; }})()",
            format!("Die Langmut{source} ist groß."),
            format!("Die Langmut{other}{source} ist groß.")
        ),
    )
    .unwrap();
    assert_eq!(twice[0]["error"], json!("Dieses Wort trägt diese Registermarke schon; nichts geändert."), "{twice}");
    assert_eq!(twice[1]["error"], twice[0]["error"], "auch als zweite Marke der Kette: {twice}");
    assert_eq!(twice[2], json!(other), "andere Marke an derselben Stelle wird gesetzt: {twice}");
}

#[test]
fn dialog_fields_become_rpc_parameters_and_register_lines() {
    let Some(result) = node(
        "context_menu.js",
        "[m.markFromFields({term: ' Geduld ', sub: '', register: '', see: 'Langmut', also: ''}), \
          m.markFromFields({term: '', register: 'stichwort'}), \
          m.glossaryParams({term: 'Langmut', short: ' kurz ', long: 'lang', variants: 'langmütig\\n\\n Geduld ', \
                            bible_refs: 'Gal 5,22', source: ''}), \
          m.glossaryParams({term: '  '}), \
          m.entryLine({register: 'personen', term: 'Rut', sub: 'Moab', also: 'Noomi'}), \
          m.entryLine({register: 'orte', term: 'Bethlehem'}, {orte: 'Ortsregister'}), \
          m.termUrl(3, 9)]",
    ) else {
        return;
    };
    assert_eq!(
        result[0],
        json!({ "mark": { "register": "stichwort", "term": "Geduld", "see": "Langmut" } }),
        "getrimmt, Register-Vorgabe Stichwort, leere Zusätze fehlen"
    );
    assert_eq!(result[1], json!({ "error": "Stichwort: darf nicht leer sein." }), "Meldung wie IndexMark::validate");
    assert_eq!(
        result[2],
        json!({ "term": "Langmut", "short": "kurz", "long": "lang", "variants": ["langmütig", "Geduld"],
                "bible_refs": ["Gal 5,22"], "source": "" }),
        "Parameter von glossary.add: Varianten und Bibelstellen je Zeile"
    );
    assert_eq!(
        result[3],
        json!({ "error": "Begriff: darf nicht leer sein." }),
        "leerer Begriff geht nicht an den Server"
    );
    assert_eq!(result[4], json!("Personenregister: Rut › Moab (siehe auch: Noomi)"), "Zeile der Liste „Register“");
    assert_eq!(result[5], json!("Ortsregister: Bethlehem"), "eigenes Register mit seinem Titel");
    assert_eq!(result[6], json!("/werke/3/glossar#begriff-9"), "Popup verweist auf den Anker des Begriffs");
}

#[test]
fn the_widget_reads_the_r31_fields_exactly_like_the_server() {
    let forms: [&[(&str, &str)]; 3] = [
        &[
            ("glossary_page", "before_indexes"),
            ("index_register", "personen"),
            ("index_register", "stichwort"),
            ("index_columns", "3"),
            ("glossary_first_mention", "on"),
        ],
        &[("index_register", " bibel "), ("index_columns", ""), ("glossary_first_mention", "off")],
        &[("glossary_page", ""), ("index_columns", "2"), ("glossary_first_mention", "")],
    ];
    for form in forms {
        let js = serde_json::to_string(&form.iter().map(|(k, v)| [*k, *v]).collect::<Vec<_>>()).unwrap();
        let Some(widget) = node("typeset_settings.js", &format!("m.formToSettings({js})")) else { return };
        assert_eq!(widget["errors"], json!([]), "keine Fehler im Widget: {widget}");
        let read: TypesetSettings = serde_json::from_value(widget["settings"].clone()).expect("gültiges JSON");
        let server = settings_from_form(&pairs(form)).expect("gültig");
        assert_eq!(read, server, "formToSettings und settings_from_form lesen die R31-Felder gleich: {widget}");
    }
    let bad = node(
        "typeset_settings.js",
        r#"m.formToSettings([["index_register", "stichwort"], ["index_columns", "zwei"]]).settings.index_pages"#,
    )
    .unwrap();
    assert_eq!(
        bad,
        json!({ "registers": ["stichwort"], "columns": 0 }),
        "keine Zahl → 0, dann meldet der Server die Grenze"
    );
}

#[test]
fn the_glossary_filter_ignores_case_and_accents() {
    let Some(result) = node(
        "glossary.js",
        "[m.matches('', 'Langmut'), m.matches('LANG', 'Langmut geduldig'), m.matches('mut ged', 'Langmut geduldig'), \
          m.matches('gnade', 'Langmut'), m.matches('ubel', 'Übel'), m.fold('  Äpfel  und\\tBirnen ')]",
    ) else {
        return;
    };
    assert_eq!(
        result,
        json!([true, true, true, false, true, "apfel und birnen"]),
        "leerer Filter passt, Groß/klein und Umlaute egal, jedes Wort muss vorkommen"
    );
}

#[tokio::test]
async fn the_widgets_are_delivered_without_stubs_and_loaded_by_main() {
    let server = common::open().await;
    for (file, widget) in [("context_menu.js", "context-menu"), ("glossary.js", "glossary")] {
        let (status, js) = server.get(&format!("/static/js/{file}")).await;
        assert_eq!(status, StatusCode::OK, "{file} wird ausgeliefert");
        assert!(js.contains(&format!("register(\"{widget}\"")), "Widget {widget} registriert");
        assert!(!js.contains("STUB"), "kein Gerüst mehr in {file}");
    }
    let (_, js) = server.get("/static/js/context_menu.js").await;
    for needle in
        ["\"role\", \"menu\"", "\"role\", \"menuitem\"", "\"glossary.add\"", "\"glossary.lookup\"", "showModal"]
    {
        assert!(js.contains(needle), "Kontextmenü: {needle}");
    }
    assert!(!js.contains("wysiwyg.js"), "kein Import von wysiwyg.ts (nur baruch:ws-command)");
    let (_, css) = server.get("/static/app.css").await;
    assert!(css.contains(".ws-sheet .ws-index-mark::before") && css.contains(".ctx-menu"), "Stile ausgeliefert");
}
