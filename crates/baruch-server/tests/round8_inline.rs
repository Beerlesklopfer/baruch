// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 8, Aufgabe „inline“: Vorschläge an ihrer Stelle im Lesefluss (Vertrag 2a–2c), Satzbild als
//! Vorgabe schon im Server-HTML (2d) und die Linien in Persona-Farbe (2e, Test in `personas.rs`).
//!
//! Aufbau: Der Mensch speichert die Einleitung des Beispielwerks (Stelle 2) als Fassung 4 mit drei
//! Absätzen (frei erfundener Gartentext). Darauf schlagen vor: @lektor (5) und @nathan (6) je eine
//! Änderung des mittleren Absatzes, @lektor (7) einen neuen ersten Absatz und das Streichen des
//! letzten (zwei Stücke). Der alte Vorschlag 3 des Beispiels bleibt offen.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const SHOWN: &str =
    "= Einleitung\n\nIm Garten blüht der Flieder.\n\nAnna gießt die Beete am Abend.\n\nDer Zaun braucht neue Farbe.\n";
const MIDDLE_LEKTOR: &str = "= Einleitung\n\nIm Garten blüht der Flieder.\n\nAnna gießt die Beete früh am Morgen.\n\nDer Zaun braucht neue Farbe.\n";
const MIDDLE_NATHAN: &str = "= Einleitung\n\nIm Garten blüht der Flieder.\n\nAnna gießt die Beete, wenn es nicht regnet.\n\nDer Zaun braucht neue Farbe.\n";
const START_AND_END: &str =
    "= Einleitung\n\nEin Vorwort über den Garten.\n\nIm Garten blüht der Flieder.\n\nAnna gießt die Beete am Abend.\n";

struct Setup {
    server: common::Server,
    key: i64,
}

impl Setup {
    async fn new() -> Self {
        let server = common::open().await;
        let saved = server.rpc("revision.save", json!({ "node": 2, "body": SHOWN })).await;
        assert_eq!(saved["result"]["rev"], 4, "der Mensch speichert Fassung 4: {saved}");
        let store = &server.state.store;
        for (body, persona, number) in
            [(MIDDLE_LEKTOR, "@lektor", 5), (MIDDLE_NATHAN, "@nathan", 6), (START_AND_END, "@lektor", 7)]
        {
            let rev = store.add_proposal(1, 2, body, persona, "probe-1", "").await.unwrap();
            assert_eq!(rev, number, "Vorschlag {number} von {persona} auf Fassung 4");
        }
        let node = server.rpc("node.get", json!({ "node": 2 })).await;
        let key = node["result"]["key"].as_i64().expect("key der Einleitung");
        Setup { server, key }
    }

    async fn page(&self, uri: &str, cookie: Option<&str>) -> String {
        let mut request = Request::get(uri);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        let (status, _, html) = self.server.send(request.body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
        html
    }
}

/// Der Lesefluss `section.pv-flow` bis `</section>`.
fn flow(html: &str) -> &str {
    let at = html.find("<section class=\"pv-flow\"").unwrap_or_else(|| panic!("Lesefluss fehlt: {html}"));
    &html[at..at + html[at..].find("</section>").unwrap()]
}

/// Alle `<details …>…</details>` eines Ausschnitts.
fn all_details(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("<details class=\"pv") {
        let end = at + rest[at..].find("</details>").unwrap() + "</details>".len();
        out.push(&rest[at..end]);
        rest = &rest[end..];
    }
    out
}

/// Das `<details>`, zu dem die Stelle `at` (sein `data-pv`) gehört.
fn details_at(html: &str, at: usize) -> &str {
    let start = html[..at].rfind("<details class=\"pv").expect("data-pv steht an einem <details>");
    &html[start..at + html[at..].find("</details>").unwrap()]
}

/// Stellen der Stücke eines Vorschlags im Ausschnitt.
fn places(html: &str, key: i64, rev: u32) -> Vec<usize> {
    html.match_indices(&format!("data-pv=\"{key}-{rev}\"")).map(|(at, _)| at).collect()
}

fn block_at(html: &str, b: usize) -> usize {
    html.find(&format!("<pre class=\"pv-src\" data-b=\"{b}\">")).unwrap_or_else(|| panic!("Block {b} fehlt: {html}"))
}

/// Wie oft `needle` vorkommt, ohne dass gleich eine Ziffer folgt („Fassung 4“ zählt nicht in „Fassung 42“).
fn count_exact(haystack: &str, needle: &str) -> usize {
    haystack
        .match_indices(needle)
        .filter(|(at, _)| !haystack[at + needle.len()..].starts_with(|c: char| c.is_ascii_digit()))
        .count()
}

#[tokio::test]
async fn each_proposal_stands_directly_at_the_block_it_changes_inserts_or_removes() {
    let setup = Setup::new().await;
    let key = setup.key;
    let html = setup.page("/?work=1&node=2&rev=4", None).await;
    let flow = flow(&html);
    assert!(
        flow.starts_with(&format!(
            "<section class=\"pv-flow\" data-widget=\"proposals\" data-key=\"{key}\" data-rev=\"4\""
        )),
        "Lesefluss der gezeigten Fassung 4 mit festem Schlüssel: {flow}"
    );
    let editor_end = html.find("<form class=\"editor-form\"").map(|at| at + html[at..].find("</form>").unwrap());
    assert!(
        editor_end < html.find("<section class=\"pv-flow\""),
        "der Lesefluss steht unter dem Textfeld (nach dem Formular)"
    );
    for b in 0..4 {
        block_at(flow, b);
    }
    assert!(
        flow.contains("<pre class=\"pv-src\" data-b=\"2\">Anna gießt die Beete am Abend.\n\n</pre>"),
        "die gezeigte Fassung blockweise als Quelltext, wörtlich: {flow}"
    );

    // Mittlerer Absatz (Block 2): Lektor (5) und Nathan (6) direkt dahinter, vor Block 3, untereinander.
    let (lektor, nathan) = (places(flow, key, 5), places(flow, key, 6));
    assert_eq!((lektor.len(), nathan.len()), (1, 1), "je ein Stück");
    assert!(
        block_at(flow, 2) < lektor[0] && lektor[0] < nathan[0] && nathan[0] < block_at(flow, 3),
        "beide an genau dieser Stelle, Lektor vor Nathan (Persona-Reihenfolge): {flow}"
    );
    let first = details_at(flow, lektor[0]);
    assert!(
        first.contains(&format!("data-pv=\"{key}-5\" data-pv-at=\"2\" data-pv-gap=\"3\" data-pv-kind=\"changed\"")),
        "geändert: at = Block 2, steht vor Block 3: {first}"
    );
    assert!(
        first.contains("<span class=\"added\">+ Anna gießt die Beete früh am Morgen.</span>")
            && first.contains("<span class=\"removed\">− Anna gießt die Beete am Abend.</span>"),
        "Zeilen-Diff nur des Stücks: {first}"
    );
    assert!(!first.contains("Flieder") && !first.contains("Zaun"), "unveränderte Nachbarn nicht im Stück: {first}");

    // Vorschlag 7: neuer erster Absatz (vor Block 1) und gestrichener letzter (Block 3, am Ende).
    let seven = places(flow, key, 7);
    assert_eq!(seven.len(), 2, "zwei Stücke eines Vorschlags: {flow}");
    assert!(
        block_at(flow, 0) < seven[0] && seven[0] < block_at(flow, 1),
        "eingefügt: zwischen Überschrift und erstem Absatz"
    );
    assert!(block_at(flow, 3) < seven[1], "entfernt: direkt hinter dem gestrichenen letzten Block");
    let parts = [details_at(flow, seven[0])];
    assert!(
        parts[0].contains("data-pv-at=\"1\" data-pv-gap=\"1\" data-pv-kind=\"inserted\""),
        "eingefügt vor Block 1: {}",
        parts[0]
    );
    assert!(parts[0].contains("· Teil 1&#x2f;2</span>"), "Teil 1 von 2 im Kopf (minijinja maskiert /): {}", parts[0]);
    let last = details_at(flow, seven[1]);
    assert!(last.contains("data-pv-at=\"3\" data-pv-gap=\"4\" data-pv-kind=\"removed\""), "entfernt: Block 3: {last}");
    assert!(last.contains("· Teil 2&#x2f;2</span>"), "Teil 2 von 2: {last}");
    assert!(last.contains("<span class=\"removed\">− Der Zaun braucht neue Farbe.</span>"), "das Gestrichene: {last}");

    // Satzbild der Stücke (für wysiwyg.ts), die Liste unter dem Editor ist fort.
    assert!(
        first.contains("<div class=\"pv-satz ws-sheet\"><div class=\"ws-diff-doc\" data-kind=\"changed\">"),
        "das Stück trägt sein Satzbild (für wysiwyg.ts im Satzbild) mit der Art „geändert“: {first}"
    );
    assert!(
        parts[0].contains("<div class=\"ws-diff-doc\" data-kind=\"inserted\"><div class=\"ws-ins\">"),
        "{}",
        parts[0]
    );
    assert!(!html.contains("pv-list"), "keine Liste mehr unter dem Editor");
    assert!(
        all_details(&html[..html.find("<section class=\"pv-flow\"").unwrap()]).is_empty(),
        "kein Stück vor dem Lesefluss"
    );
}

#[tokio::test]
async fn the_head_appears_exactly_once_per_insertion() {
    let setup = Setup::new().await;
    let html = setup.page("/?work=1&node=2&rev=4", None).await;
    let details = all_details(flow(&html));
    assert_eq!(details.len(), 5, "3 (alt), 5, 6 und zwei Stücke von 7: {details:#?}");
    for item in &details {
        let rev = item.split("data-pv=\"").nth(1).and_then(|r| r.split('"').next()).unwrap();
        let rev = rev.split('-').nth(1).unwrap();
        let who = if item.contains("@nathan") { "Nathan (@nathan)" } else { "Lektor (@lektor)" };
        assert_eq!(item.matches(who).count(), 1, "„{who}“ genau einmal: {item}");
        assert_eq!(count_exact(item, &format!("Fassung {rev}")), 1, "„Fassung {rev}“ genau einmal: {item}");
        assert!(!item.contains("unbekannt"), "kein zweiter Kopf „… unbekannt“: {item}");
        assert_eq!(item.matches("<summary").count(), 1, "eine Linie je Stück");
        assert!(
            !item.contains("title=\"Lektor") && !item.contains("title=\"Nathan"),
            "der Kopf steht nicht noch im title"
        );
    }
    let old = details.iter().find(|d| d.contains(&format!("data-pv=\"{}-3\"", setup.key))).unwrap();
    assert!(
        old.contains("Lektor (@lektor) · Fassung 3 · google&#x2f;gemma-4-26b-a4b · auf Fassung 2</span>"),
        "Basis 2 weicht von der gezeigten Fassung 4 ab und steht im Kopf: {old}"
    );
    let five = details.iter().find(|d| d.contains(&format!("data-pv=\"{}-5\"", setup.key))).unwrap();
    assert!(
        five.contains("Lektor (@lektor) · Fassung 5 · probe-1</span>"),
        "auf der gezeigten Fassung: ohne Basis: {five}"
    );
}

#[tokio::test]
async fn folding_a_proposal_folds_all_its_parts_and_accepting_by_its_form_is_one_commit() {
    let setup = Setup::new().await;
    let key = setup.key;
    let cookie = format!("baruch_folded={key}-7");
    let html = setup.page("/?work=1&node=2&rev=4", Some(&cookie)).await;
    let flow = flow(&html);
    for item in all_details(flow) {
        let folded = item.contains(&format!("data-pv=\"{key}-7\""));
        let tag = &item[..item.find('>').unwrap() + 1];
        assert_eq!(!tag.ends_with(" open>"), folded, "nur die Stücke von 7 eingeklappt (beide): {tag}");
    }

    // Übernehmen ohne JavaScript: genau die Felder des Formulars, auf das der Knopf zeigt.
    let id = format!("pv-accept-{key}-6");
    let at = html.find(&format!("<form id=\"{id}\"")).expect("Formular des Vorschlags 6");
    let form = &html[at..at + html[at..].find("</form>").unwrap()];
    assert!(form.contains("method=\"post\" action=\"/act\""), "an /act: {form}");
    assert!(flow.contains(&format!("form=\"{id}\">Übernehmen</button>")), "der Knopf im Stück zeigt darauf");
    let fields: Vec<String> = form
        .split("<input type=\"hidden\" name=\"")
        .skip(1)
        .map(|input| {
            let (name, rest) = input.split_once("\" value=\"").unwrap();
            format!("{name}={}", rest.split('"').next().unwrap())
        })
        .collect();
    let dir = setup.server.mirror_dir();
    let before = common::git(&dir, &["rev-list", "--count", "HEAD"]);
    let (status, location) = setup.server.post_form("/act", &fields.join("&")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernehmen mit den Feldern {fields:?}: {location:?}");
    let node: Value = setup.server.rpc("node.get", json!({ "node": 2 })).await["result"].clone();
    assert_eq!(common::currents(&node), 1, "genau eine aktuelle Fassung");
    let current = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap();
    assert_eq!(current["number"], 6, "Nathans Vorschlag ist aktuell");
    if let (Some(b), Some(a)) = (before, common::git(&dir, &["rev-list", "--count", "HEAD"])) {
        assert_eq!(a.trim().parse::<u32>().unwrap(), b.trim().parse::<u32>().unwrap() + 1, "genau ein Commit");
    }
}

#[tokio::test]
async fn in_the_preview_proposals_stand_at_their_place_inside_the_section() {
    let setup = Setup::new().await;
    let key = setup.key;
    let html = setup.page("/?work=1&node=2&view=preview", None).await;
    let sheet = &html[html.find("<div class=\"sheet\">").expect("Vorschau-Blatt")..];
    let second = sheet.find("<p>Anna gießt die Beete am Abend.</p>").expect("mittlerer Absatz");
    let third = sheet.find("<p>Der Zaun braucht neue Farbe.</p>").expect("letzter Absatz");
    let lektor = places(sheet, key, 5);
    assert!(
        second < lektor[0] && lektor[0] < third,
        "der Vorschlag zum mittleren Absatz steht zwischen ihm und dem nächsten, nicht hinter dem Abschnitt"
    );
    let first = sheet.find("<p>Im Garten blüht der Flieder.</p>").unwrap();
    assert!(places(sheet, key, 7)[0] < first, "das eingefügte Vorwort steht vor dem ersten Absatz");
    assert!(
        sheet.contains(&format!("<div class=\"pv-section\" data-widget=\"proposals\" data-key=\"{key}\"")),
        "der Abschnitt der Vorschau trägt das Widget proposals mit seinem Schlüssel (Klappen, „Alle einklappen“): {sheet}"
    );
}

/// Ursache „Quelltext aktiv nach dem Laden“ (Backlog, Runde 8): Der Server lieferte den Umschalter
/// immer mit „Satzbild“ gedrückt und ein leeres Satzbild; das Widget schaltete beim Einhängen erst auf
/// Quelltext und öffnete das Satzbild erst nach zwei RPC-Antworten. Jetzt zeichnet der Server den
/// Zustand aus dem Cookie `baruch_editor` und liefert das Satzbild fertig mit (`data-ws-ready`).
/// Vor der Behebung scheitert dieser Test an `data-ws-ready` und am Cookie `quelltext`.
#[tokio::test]
async fn the_server_already_renders_the_satzbild_as_default_and_the_source_only_on_request() {
    let setup = Setup::new().await;
    let html = setup.page("/?work=1&node=2&rev=4", None).await;
    let area = &html[html.find("data-widget=\"wysiwyg\"").unwrap()..];
    let area = &area[..area.find("<textarea").unwrap()];
    assert!(area.contains("data-ws-start=\"satz\""), "Vorgabe Satzbild am Bereich: {area}");
    assert!(
        area.contains("data-ws-mode=\"satz\" aria-pressed=\"true\"")
            && area.contains("data-ws-mode=\"quelltext\" aria-pressed=\"false\""),
        "„Satzbild“ gedrückt schon im HTML: {area}"
    );
    let parsed = setup.server.rpc("wysiwyg.parse", json!({ "body": SHOWN })).await;
    let expected = parsed["result"]["html"].as_str().unwrap();
    assert!(
        area.contains(&format!("spellcheck=\"true\" lang=\"de\" data-ws-ready>{expected}</div>")),
        "das Satzbild der gezeigten Fassung kommt fertig vom Server, gleich wysiwyg.parse: {area}"
    );
    assert!(area.contains("<span class=\"ws-tools\" data-ws-tools>"), "Werkzeuge sichtbar (im Satzbild)");
    assert!(!area.contains("details"), "die Vorschläge setzt erst das Widget ins Satzbild");

    let source = setup.page("/?work=1&node=2&rev=4", Some("baruch_editor=quelltext")).await;
    let area = &source[source.find("data-widget=\"wysiwyg\"").unwrap()..];
    let area = &area[..area.find("<textarea").unwrap()];
    assert!(
        area.contains("data-ws-mode=\"satz\" aria-pressed=\"false\"")
            && area.contains("data-ws-mode=\"quelltext\" aria-pressed=\"true\""),
        "mit Cookie „quelltext“ ist der Quelltext gedrückt: {area}"
    );
    assert!(!area.contains("data-ws-ready"), "kein fertiges Satzbild, wenn der Quelltext gewünscht ist: {area}");
    assert!(area.contains("data-ws-tools hidden") && area.contains("data-ws-wrap hidden"), "Satzbild-Teile verborgen");
    assert!(area.contains("data-ws-start=\"quelltext\""), "{area}");

    // ui.render (Klick in der Gliederung) liest dasselbe Cookie.
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.render", "params": { "node": 2 } });
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, "baruch_editor=quelltext")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (_, _, text) = setup.server.send(request).await;
    let answer: Value = serde_json::from_str(&text).unwrap();
    let editor = answer["result"]["editor"].as_str().unwrap();
    assert!(editor.contains("data-ws-mode=\"quelltext\" aria-pressed=\"true\""), "nach ui.render ebenso: {editor}");
    let rendered = setup.server.rpc("ui.render", json!({ "node": 2 })).await;
    let editor = rendered["result"]["editor"].as_str().unwrap();
    assert!(editor.contains(" data-ws-ready>"), "ohne Cookie auch nach ui.render das fertige Satzbild");

    // Das Widget übernimmt das fertige Satzbild beim Einhängen und setzt die Vorschläge an ihre Stelle.
    let js = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "wysiwyg.js").unwrap().1;
    for part in ["function adoptSheet", "data-ws-ready", "function placeProposals", "function returnProposals"] {
        assert!(js.contains(part), "wysiwyg.js enthält {part}");
    }
    assert!(
        js.contains("data-pv-gap") && js.contains("contentEditable = \"false\""),
        "Stücke nicht bearbeitbar ins Satzbild"
    );
    assert!(js.matches("if (isProposal(child))").count() >= 2, "editsOf und blocksIn übergehen .pv");
    assert!(js.contains("block.closest(\".pv\")"), "markSelection übergeht Blöcke in Vorschlägen");
    let start = js.find("register(\"wysiwyg\"").unwrap();
    let mount = &js[start..];
    let adopt = mount.find("adoptSheet(state, root)").expect("Einhängen übernimmt das Satzbild");
    assert!(
        !mount[..adopt].contains("show(state, root)"),
        "vor dem Übernehmen schaltet das Widget nicht mehr auf Quelltext um (die Ursache)"
    );
}
