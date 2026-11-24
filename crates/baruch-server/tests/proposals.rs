// SPDX-License-Identifier: AGPL-3.0-or-later
//! Vorschläge im Lesefluss (Vertrag Runde 7, „proposals“, 5e; an ihrer Stelle seit Runde 8, „inline“):
//! je Stück eines offenen Vorschlags ein `<details class="pv">` in der Farbe der Persona, mit einem
//! Kopf, Satzbild- und Zeilen-Diff des Stücks, Übernehmen/Verwerfen über `/act` (ohne JavaScript),
//! an derselben Stelle nach Persona-Reihenfolge, gefaltet über das Cookie `baruch_folded`; im Editor
//! und in der Vorschau. Die Vorschläge an ihrer Stelle im Einzelnen prüft `round8_inline.rs`.
//!
//! Beispielwerk: Einleitung (Stelle 2) hat Fassung 1 (ersetzt), 2 (aktuell), 3 (Vorschlag von @lektor
//! auf 2). Die Tests hängen frei erfundene Gartentexte als weitere Vorschläge an.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::personas::style;
use baruch_store::{FALLBACK_COLOR, PERSONA_PALETTE};
use serde_json::{Value, json};

const GARDEN_NATHAN: &str = "= Einleitung\n\nIn Annas Laube wächst der Efeu über die Bank.\n";
const GARDEN_LEKTOR: &str = "= Einleitung\n\nDie Rosen am Zaun brauchen Wasser.\n";

/// Key der Einleitung.
async fn intro_key(server: &common::Server) -> i64 {
    let node = server.rpc("node.get", json!({ "node": 2 })).await;
    node["result"]["key"].as_i64().expect("key der Einleitung")
}

/// Zwei weitere Vorschläge: 4 von @nathan, 5 von @lektor (beide auf der aktuellen Fassung 2).
async fn two_more_proposals(server: &common::Server) {
    let store = &server.state.store;
    let four = store.add_proposal(1, 2, GARDEN_NATHAN, "@nathan", "gpt-4.1-mini", "Efeu ergänzt").await.unwrap();
    let five = store.add_proposal(1, 2, GARDEN_LEKTOR, "@lektor", "google/gemma-4-26b-a4b", "Rosen").await.unwrap();
    assert_eq!((four, five), (4, 5), "neue Nummern 4 und 5");
}

async fn get_with_cookie(server: &common::Server, uri: &str, cookie: &str) -> String {
    let request = Request::get(uri).header(header::COOKIE, cookie).body(Body::empty()).unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
    html
}

/// Öffnendes `<details …>` des (ersten) Stücks von Vorschlag `<key>-<rev>`.
fn pv_tag(html: &str, key: i64, rev: u32) -> &str {
    let at = html.find(&format!("data-pv=\"{key}-{rev}\"")).unwrap_or_else(|| panic!("Vorschlag {rev} fehlt: {html}"));
    let start = html[..at].rfind("<details").expect("data-pv steht an einem <details>");
    &html[start..at + html[at..].find('>').unwrap() + 1]
}

/// Inhalt des (ersten) Stücks eines Vorschlags bis zu seinem `</details>`.
fn pv_body(html: &str, key: i64, rev: u32) -> &str {
    let at = html.find(&format!("data-pv=\"{key}-{rev}\"")).unwrap();
    &html[at..at + html[at..].find("</details>").unwrap()]
}

/// Formular `id` (Übernehmen/Verwerfen eines Vorschlags) bis `</form>`.
fn form_by_id<'a>(html: &'a str, id: &str) -> &'a str {
    let at = html.find(&format!("<form id=\"{id}\"")).unwrap_or_else(|| panic!("Formular {id} fehlt: {html}"));
    &html[at..at + html[at..].find("</form>").unwrap()]
}

fn current(node: &Value) -> u64 {
    node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap()["number"].as_u64().unwrap()
}

#[tokio::test]
async fn proposals_of_two_personas_stand_one_below_the_other_in_their_colors() {
    let server = common::open().await;
    two_more_proposals(&server).await;
    let key = intro_key(&server).await;
    let (status, html) = server.get("/?work=1&node=2&rev=2").await;
    assert_eq!(status, StatusCode::OK, "Seite lädt");
    let positions: Vec<usize> = [3u32, 5, 4]
        .iter()
        .map(|rev| html.find(&format!("data-pv=\"{key}-{rev}\"")).expect("jeder Vorschlag da"))
        .collect();
    assert!(
        positions[0] < positions[1] && positions[1] < positions[2],
        "Reihenfolge nach Persona (@lektor vor @nathan), darin nach Nummer (3, 5 | 4): {positions:?}"
    );
    let lektor = style(PERSONA_PALETTE[0]);
    let nathan = style(PERSONA_PALETTE[4]);
    assert!(pv_tag(&html, key, 3).contains(&format!("style=\"{lektor}\"")), "Vorschlag 3 in der Farbe des Lektors");
    assert!(pv_tag(&html, key, 5).contains(&format!("style=\"{lektor}\"")), "Vorschlag 5 in der Farbe des Lektors");
    assert!(pv_tag(&html, key, 4).contains(&format!("style=\"{nathan}\"")), "Vorschlag 4 in Nathans Farbe");
    assert!(pv_tag(&html, key, 4).ends_with(" open>"), "ohne Cookie offen: {}", pv_tag(&html, key, 4));
    assert!(
        pv_tag(&html, key, 4).contains("data-pv-at=\"1\" data-pv-gap=\"2\" data-pv-kind=\"changed\""),
        "alle drei ändern den Absatz (Block 1) und stehen hinter ihm: {}",
        pv_tag(&html, key, 4)
    );

    let nathan_body = pv_body(&html, key, 4);
    assert!(
        nathan_body.contains("<span class=\"pv-label\">Nathan (@nathan) · Fassung 4 · gpt-4.1-mini</span></summary>"),
        "der einzige Kopf steht in der Linie (Name, Handle, Nummer, Modell): {nathan_body}"
    );
    assert!(nathan_body.contains("<p class=\"pv-note small\">Efeu ergänzt</p>"), "Notiz im Stück: {nathan_body}");
    assert!(!nathan_body.contains("<header"), "kein zweiter Kopf mehr im Inhalt: {nathan_body}");
    for (method, action, button) in
        [("revision.accept", "accept", "Übernehmen"), ("revision.reject", "reject", "Verwerfen")]
    {
        let id = format!("pv-{action}-{key}-4");
        assert!(
            nathan_body.contains(&format!("type=\"submit\" form=\"{id}\">{button}</button>")),
            "Knopf {button} im Stück zeigt auf das Formular des Vorschlags: {nathan_body}"
        );
        let form = form_by_id(&html, &id);
        assert!(form.contains("method=\"post\" action=\"/act\" data-rpc"), "an /act ohne JavaScript: {form}");
        assert!(form.contains(&format!("name=\"method\" value=\"{method}\"")), "Methode {method}: {form}");
        assert!(form.contains("name=\"rev\" value=\"4\""), "{method} mit der Nummer DIESES Vorschlags: {form}");
        assert!(form.contains(&format!("name=\"key\" value=\"{key}\"")), "mit festem Schlüssel: {form}");
        let editor_form = html.find("<form class=\"editor-form\"").unwrap();
        let editor_end = editor_form + html[editor_form..].find("</form>").unwrap();
        let at = html.find(&format!("<form id=\"{id}\"")).unwrap();
        assert!(at > editor_end, "das Formular steht außerhalb des Editor-Formulars (nicht geschachtelt)");
    }
    assert!(!html.contains("card proposal"), "die alte Vorschlag-Karte ist fort");
    assert!(!html.contains("pv-list"), "die Liste unter dem Editor ist fort (Runde 8)");
    assert!(!html.contains("data-widget=\"wysiwyg-diff\""), "das alte Diff-Widget ist fort");
    assert!(html.contains("<section class=\"pv-flow\" data-widget=\"proposals\""), "Widget proposals am Lesefluss");
    assert!(
        html.contains("<button class=\"btn tiny\" type=\"button\" data-pv-all hidden>"),
        "„Alle“ ohne JavaScript verborgen"
    );
}

#[tokio::test]
async fn the_diff_marks_changed_lines_and_blocks() {
    let server = common::open().await;
    two_more_proposals(&server).await;
    let key = intro_key(&server).await;
    let (_, html) = server.get("/?work=1&node=2").await;
    let body = pv_body(&html, key, 4);
    assert!(
        body.contains("<span class=\"added\">+ In Annas Laube wächst der Efeu über die Bank.</span>"),
        "Zeilen-Diff des Stücks: neu: {body}"
    );
    assert!(body.contains("<span class=\"removed\">− Dieses Buch handelt"), "Zeilen-Diff: entfernt: {body}");
    assert!(!body.contains("= Einleitung"), "die unveränderte Überschrift gehört nicht zum Stück: {body}");
    let satz = &body[body.find("<div class=\"pv-satz ws-sheet\">").expect("Satzbild des Stücks im Editor")..];
    assert!(
        satz.contains("class=\"ws-ins\"") && satz.contains("Annas Laube"),
        "Satzbild: neuer Block markiert: {satz}"
    );
    assert!(satz.contains("class=\"ws-del\""), "Satzbild: alter Block markiert");
    assert!(satz.contains("data-kind=\"changed\""), "Satzbild nennt die Art des Stücks");
    let (_, css) = server.get("/static/app.css").await;
    assert!(
        css.contains(".ws-sheet .pv .pv-quelltext { display: none; }")
            && css.contains(".ws-sheet .pv .pv-satz { display: block; }")
            && css.contains(".pv-satz { display: none;"),
        "im Satzbild das Satzbild des Stücks, sonst (und ohne JavaScript) der Zeilen-Diff – nur per CSS"
    );
}

#[tokio::test]
async fn accepting_by_form_switches_to_exactly_this_proposal_with_one_commit() {
    let server = common::open().await;
    two_more_proposals(&server).await;
    let key = intro_key(&server).await;
    let dir = server.mirror_dir();
    let before = common::git(&dir, &["rev-list", "--count", "HEAD"]);
    let (status, location) =
        server.post_form("/act", &format!("method=revision.accept&work=1&node=2&key={key}&rev=4&view=edit")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernehmen per Formular");
    let location = location.unwrap_or_default();
    assert_eq!(
        location,
        format!("/?work=1&key={key}&rev=4"),
        "zurück zur übernommenen Fassung, kanonisch über den festen Schlüssel (Runde 8) und ohne Vorschau"
    );
    let node = server.rpc("node.get", json!({ "node": 2 })).await["result"].clone();
    assert_eq!(current(&node), 4, "Vorschlag 4 ist jetzt aktuell: {node}");
    assert_eq!(common::currents(&node), 1, "genau eine aktuelle Fassung");
    if let (Some(before), Some(after)) = (before, common::git(&dir, &["rev-list", "--count", "HEAD"])) {
        let (b, a): (u32, u32) = (before.trim().parse().unwrap(), after.trim().parse().unwrap());
        assert_eq!(a, b + 1, "genau ein Commit im Spiegel");
    }
    let (_, html) = server.get("/?work=1&node=2&rev=4").await;
    assert!(!html.contains(&format!("data-pv=\"{key}-4\"")), "der übernommene ist kein offener Vorschlag mehr");
    assert!(html.contains(&format!("data-pv=\"{key}-3\"")), "die übrigen bleiben offen stehen");
    assert!(
        pv_body(&html, key, 5).contains(" · auf Fassung 2</span>"),
        "5 beruht weiter auf 2, gezeigt ist 4: der Kopf nennt die abweichende Basis: {}",
        pv_body(&html, key, 5)
    );
}

#[tokio::test]
async fn rejecting_by_form_removes_the_variant_from_the_reading_flow() {
    let server = common::open().await;
    two_more_proposals(&server).await;
    let key = intro_key(&server).await;
    let (status, location) =
        server.post_form("/act", &format!("method=revision.reject&work=1&node=2&key={key}&rev=5&view=edit")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Verwerfen per Formular");
    let location = location.unwrap_or_default();
    assert_eq!(
        location,
        format!("/?work=1&key={key}&rev=2"),
        "zurück zur aktuellen Fassung, kanonisch über den festen Schlüssel (Runde 8)"
    );
    let node = server.rpc("node.get", json!({ "node": 2 })).await["result"].clone();
    let five = node["revisions"].as_array().unwrap().iter().find(|r| r["number"] == 5).unwrap().clone();
    assert_eq!(five["status"], "rejected", "Vorschlag 5 verworfen: {five}");
    assert_eq!(current(&node), 2, "die aktuelle Fassung bleibt");
    let (_, html) = server.get("/?work=1&node=2").await;
    assert!(!html.contains(&format!("data-pv=\"{key}-5\"")), "der verworfene steht nicht mehr im Lesefluss");
    assert!(!html.contains(&format!("id=\"pv-accept-{key}-5\"")), "und hat kein Formular mehr");
    assert!(html.contains(&format!("data-pv=\"{key}-4\"")), "der andere bleibt");
}

#[tokio::test]
async fn the_cookie_folds_exactly_the_named_proposals_but_never_the_shown_one() {
    let server = common::open().await;
    two_more_proposals(&server).await;
    let key = intro_key(&server).await;
    let html = get_with_cookie(&server, "/?work=1&node=2&rev=2", &format!("baruch_folded={key}-4.999-1")).await;
    assert!(!pv_tag(&html, key, 4).contains(" open"), "4 eingeklappt: {}", pv_tag(&html, key, 4));
    for rev in [3, 5] {
        assert!(pv_tag(&html, key, rev).ends_with(" open>"), "{rev} bleibt offen");
    }
    let shown = get_with_cookie(&server, "/?work=1&node=2&rev=4", &format!("baruch_folded={key}-4")).await;
    let tag = pv_tag(&shown, key, 4);
    assert!(
        tag.ends_with(" open>") && tag.contains("pv-shown"),
        "der gezeigte Vorschlag steht offen und hervorgehoben: {tag}"
    );
    assert!(
        pv_body(&shown, key, 4).contains("Unterschied zu Fassung 2 (aktuell) · gezeigt"),
        "beim gezeigten Vorschlag stehen seine Stücke gegen seine Basis: {}",
        pv_body(&shown, key, 4)
    );

    let bad = get_with_cookie(&server, "/?work=1&node=2", &format!("baruch_folded={key}-4.x\"><b>")).await;
    assert!(pv_tag(&bad, key, 4).ends_with(" open>"), "ein Cookie mit fremden Zeichen gilt nicht");

    // ui.render liest dasselbe Cookie: der Zustand bleibt nach dem Neuzeichnen.
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.render", "params": { "node": 2, "rev": 2 } });
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("baruch_folded={key}-3"))
        .body(Body::from(body.to_string()))
        .unwrap();
    let (_, _, text) = server.send(request).await;
    let answer: Value = serde_json::from_str(&text).unwrap();
    let editor = answer["result"]["editor"].as_str().expect("editor im ui.render");
    assert!(!pv_tag(editor, key, 3).contains(" open"), "nach ui.render eingeklappt: {editor}");
    assert!(pv_tag(editor, key, 4).ends_with(" open>"), "die anderen offen");

    let script = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "proposals.js").map(|(_, body)| *body);
    let script = script.expect("proposals.js eingebettet");
    for part in ["register(\"proposals\"", "store.set(\"folded\"", "[data-pv-all]", "\"toggle\""] {
        assert!(script.contains(part), "proposals.js enthält {part}");
    }
    let main = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "main.js").unwrap().1;
    assert!(main.contains("import \"./proposals.js\";"), "main.js lädt das Widget");
}

#[tokio::test]
async fn the_preview_shows_open_proposals_at_their_place_in_the_section() {
    let server = common::open().await;
    two_more_proposals(&server).await;
    let key = intro_key(&server).await;
    let (_, html) = server.get("/?work=1&node=2&view=preview").await;
    let sheet = &html[html.find("<div class=\"sheet\">").expect("Vorschau-Blatt")..];
    let heading = sheet.find("<h1>Einleitung</h1>").expect("Überschrift der Einleitung in der Vorschau");
    let paragraph =
        sheet.find("<p>Es richtet sich an Betroffene, an Seelsorger und an Gemeinden.</p>").expect("Absatz");
    let variant = sheet.find(&format!("data-pv=\"{key}-3\"")).expect("Vorschlag in der Vorschau");
    assert!(heading < paragraph && paragraph < variant, "der Vorschlag steht hinter dem Absatz, den er ändert");
    let body = pv_body(sheet, key, 4);
    assert!(body.contains("<pre class=\"pv-quelltext diff\">"), "Zeilen-Diff in der Vorschau");
    assert!(!body.contains("pv-satz"), "in der Vorschau kein Satzbild-Diff");
    let form = form_by_id(sheet, &format!("pv-accept-{key}-4"));
    assert!(form.contains("name=\"view\" value=\"preview\""), "Formulare bleiben in der Vorschau: {form}");
    assert!(!html.contains("data-widget=\"wysiwyg\""), "die Vorschau hat keinen Editor");
}

#[tokio::test]
async fn unknown_authors_come_last_in_the_fallback_color() {
    let server = common::open().await;
    two_more_proposals(&server).await;
    let six = server.state.store.add_proposal(1, 2, GARDEN_NATHAN, "@mcp:gartenhelfer", "probe", "").await.unwrap();
    let key = intro_key(&server).await;
    let (_, html) = server.get("/?work=1&node=2").await;
    let tag = pv_tag(&html, key, six);
    assert!(tag.contains(&format!("style=\"{}\"", style(FALLBACK_COLOR))), "Grau ohne Persona-Eintrag: {tag}");
    assert!(tag.contains("pv-unknown"), "als unbekannt markiert");
    let last = html.rfind("data-pv=\"").unwrap();
    assert_eq!(last, html.find(&format!("data-pv=\"{key}-{six}\"")).unwrap(), "an derselben Stelle ganz am Ende");
}

#[tokio::test]
async fn notes_models_handles_and_names_are_escaped() {
    let server = common::open().await;
    sqlx::query(
        "INSERT INTO persona (handle, name, role, model, context_length, position) VALUES ('@x\"<>&''', 'Rosa \"<>&''', 'Probe', 'm', 1, 50)",
    )
    .execute(server.state.store.pool())
    .await
    .unwrap();
    let rev =
        server.state.store.add_proposal(1, 2, GARDEN_LEKTOR, "@x\"<>&'", "m\"<>&'", "Notiz \"<>&'").await.unwrap();
    let key = intro_key(&server).await;
    let (_, html) = server.get("/?work=1&node=2").await;
    let body = pv_body(&html, key, rev);
    for escaped in [
        "Rosa &quot;&lt;&gt;&amp;&#x27;",
        "@x&quot;&lt;&gt;&amp;&#x27;",
        "m&quot;&lt;&gt;&amp;&#x27;",
        "Notiz &quot;&lt;&gt;&amp;&#x27;",
    ] {
        assert!(body.contains(escaped), "maskiert: {escaped} in {body}");
    }
    for raw in ["Rosa \"<>&'", "@x\"<>&'", "m\"<>&'", "Notiz \"<>&'"] {
        assert!(!html.contains(raw), "kein roher Text {raw}");
    }
    assert!(
        body.contains(
            "<span class=\"pv-label\">Rosa &quot;&lt;&gt;&amp;&#x27; (@x&quot;&lt;&gt;&amp;&#x27;) · Fassung"
        ),
        "auch der Kopf in der Linie ist maskiert: {body}"
    );
}
