// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23 „Härtung“ (Besitz Integrator): Abnahme Ende zu Ende über die Grenzen der drei Aufgaben hinweg.
//!
//! Werke frei erfunden; kein Eliezer, kein Netz (Modelle nur über Nachbauten auf 127.0.0.1:0). Jede Zusicherung mit
//! Begründung. Weitere Dateien als Untermodule unter `tests/round23_abnahme/*.rs`, eingebunden mit
//! `#[path = "round23_abnahme/x.rs"] mod x;` – `tests/main.rs` bleibt unverändert.

use crate::common;
use crate::round23_geheimnisse::{redirecting, with_models};
use crate::round23_mcp_namen::{
    Caller, create_token, error_code, finding, local, mcp, mill, ok as tool, refused, remote, result,
};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::mock::Step;
use baruch_server::assist::prompts;
use baruch_server::websearch::mock::{MockSite, MockWebMcp};
use serde_json::{Value, json};

/// Erfundenes Geheimnis mit seinen kodierten Formen (`printf 'Bernstein-2311' | base64` usw.).
const SECRET: &str = "Bernstein-2311";

fn urlencode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn form(uri: &str, body: String) -> Request<Body> {
    Request::post(uri).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded").body(Body::from(body)).unwrap()
}

fn get(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.body(Body::empty()).unwrap()
}

/// Aktuelle Fassung (Nummer, Text) des Punkts an Stelle `node`.
async fn current(server: &common::Server, work: i64, node: usize) -> (u32, String) {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let n = &loaded.nodes[node];
    let rev = &n.revisions[n.current_index().expect("aktuelle Fassung")];
    (rev.number, rev.body.clone())
}

/// Der durchgehende Ablauf an einem erfundenen Werk („Das Mühlenbuch“), alles ohne JavaScript bzw. über die Wege,
/// die auch ohne JavaScript gehen (Formulare, RPC, MCP): Zitat einfügen und speichern, fortschreiben und über den Baum
/// ändern, Rückfrage-Felder von `ask_persona`, Tokens mit Umlaut und Kollision, Client ohne Token, `own_reads`,
/// feste Meldecodes.
#[tokio::test]
async fn the_round_23_walk_through_holds_from_the_quote_to_the_tokens() {
    let _turn = common::model_queue(module_path!()).await;
    let m = mill([]).await;
    let (work, key) = (m.work, m.keys[0]);

    // ---- 1. Zitat über den Knopf, Stellenangabe in der Zeile danach, speichern ohne JavaScript ------------------
    let (rev, body) = current(&m.server, work, 0).await;
    let snippet = baruch_server::ribbon::QUOTE_SNIPPET;
    // Runde 36: Der Knopf fügt nur noch den leeren Block ein (keine Folgezeile „– “, die ungefüllt als einzelner
    // Gedankenstrich im Buch stand); die Stellenangabe tippt der Mensch in die Zeile direkt darunter.
    assert_eq!(
        snippet, "#quote(block: true)[]\n",
        "Testvoraussetzung: der Knopf fügt den leeren Block ein, ohne Zeile mit Gedankenstrich"
    );
    // Was der Mensch nach dem Knopf tippt: das Zitat in den Rumpf, die Stelle in die Zeile darunter.
    let quote =
        format!("{}– Mühlenordnung 7\n", snippet.replacen("[]", "[Wer mahlt, hört den Bach auch im Schlaf.]", 1));
    let with_quote = format!("{}\n\n{quote}\nDann schloss der Müller das Wehr.\n", body.trim_end());
    let save = format!(
        "method=outline.save&work={work}&key={key}&based_on={rev}&based_on_amend=0&mode=auto&body={}",
        urlencode(&with_quote)
    );
    let (status, _, page) = m.server.send(form("/gliederung/act", save)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "das Formular des Editors speichert ohne JavaScript: {page}");
    let (rev, stored) = current(&m.server, work, 0).await;
    assert_eq!(stored, with_quote, "gespeichert ist genau der eingegebene Text (byte-gleich)");

    let parsed = m.server.rpc("wysiwyg.parse", json!({ "work": work, "body": stored })).await;
    let blocks = result(&parsed)["blocks"].as_array().expect("blocks").clone();
    let at = blocks.iter().position(|b| b["kind"] == "quote").unwrap_or_else(|| panic!("kein Zitatblock: {parsed}"));
    assert_eq!(blocks[at]["block"], true, "das Satzbild liest einen Zitat-Block (block: true): {}", blocks[at]);
    assert_eq!(
        (&blocks[at + 1]["kind"], blocks[at + 1]["raw"].as_str().map(str::trim_end)),
        (&json!("paragraph"), Some("– Mühlenordnung 7")),
        "die Stellenangabe direkt unter dem Zitat ist ein eigener Absatz: {}",
        blocks[at + 1]
    );
    assert!(!blocks.iter().any(|b| b["kind"] == "island"), "kein Stück ist eine Code-Insel: {parsed}");
    let raws: String = blocks.iter().map(|b| b["raw"].as_str().unwrap()).collect();
    assert_eq!(raws, stored, "die Blöcke ergeben verkettet wieder den Text (byte-gleich)");
    let html = result(&parsed)["html"].as_str().unwrap();
    assert!(html.contains("data-kind=\"quote\"") && !html.contains("ws-raw"), "Satzbild ohne rohen Typst-Code: {html}");
    let keep: Vec<Value> = (0..blocks.len()).map(|i| json!({ "keep": i })).collect();
    let back = m.server.rpc("wysiwyg.serialize", json!({ "work": work, "body": stored, "edits": keep })).await;
    assert_eq!(result(&back)["body"], json!(stored), "unberührt gibt das Satzbild den Text byte-gleich zurück");

    let client = local(&m, "Prüfstand").await;
    {
        let _pdf = common::PDF_SERIAL.lock().await;
        let checked = tool(&m, &client, "typst_check", json!({ "work": work })).await;
        assert_eq!(checked["ok"], true, "das Werk mit Zitat und Stellenangabe setzt Typst ohne Fehler: {checked}");
    }

    // ---- 2. Fortschreiben, dann Ändern über den Baum (Formular an /act) → Schritte im Verlauf -------------------
    // Stand der Fassung nach dem Speichern des Zitats (0 bei neuer Fassung, 1 falls schon fortgeschrieben).
    let before = m.server.rpc("revision.amends", json!({ "work": work, "key": key, "rev": rev })).await;
    let base = u32::try_from(result(&before)["amend_seq"].as_u64().expect("amend_seq")).unwrap();
    let amended = stored.replacen("im Schlaf", "noch im Schlaf", 1);
    let saved = m
        .server
        .rpc(
            "revision.save",
            json!({ "work": work, "key": key, "body": amended, "based_on": rev, "based_on_amend": base, "mode": "auto" }),
        )
        .await;
    assert_eq!(result(&saved)["saved"], "amended", "eine kleine Änderung schreibt die eigene Fassung fort: {saved}");
    let over_tree = amended.replacen("das Wehr.\n", "das alte Wehr.\n", 1);
    assert_ne!(over_tree, amended, "Testvoraussetzung: der Text für den Baum ist ein anderer");
    let update = |stand: u32, text: &str| {
        format!(
            "method=revision.update&work={work}&key={key}&rev={rev}&based_on_amend={stand}&body={}",
            urlencode(text)
        )
    };
    let (status, _, stale) = m.server.send(form("/act", update(base, &over_tree))).await;
    assert!(
        status.is_client_error() && stale.contains("Veraltet"),
        "ein Formular mit veraltetem Stand wird auch ohne JavaScript abgelehnt (Status {status}): {stale}"
    );
    assert_eq!(current(&m.server, work, 0).await.1, amended, "nichts überschrieben");
    let (status, _, page) = m.server.send(form("/act", update(base + 1, &over_tree))).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "„Fassung ändern“ über das Formular des Baums: {page}");
    let history = m.server.rpc("revision.amends", json!({ "work": work, "key": key, "rev": rev })).await;
    let steps = result(&history)["steps"].as_array().unwrap().clone();
    assert_eq!(
        steps.len(),
        base as usize + 2,
        "Fortschreibung und Ändern über den Baum stehen beide im Verlauf: {history}"
    );
    let last = steps.last().unwrap();
    assert_eq!(last["body_before"], json!(amended), "der Schritt aus dem Baum bewahrt den Text davor: {history}");
    assert_eq!(last["body_after"], json!(over_tree), "und nennt den Text danach: {history}");
    assert_eq!(result(&history)["amend_seq"], base + 2, "die Fassung steht zwei Stände weiter");

    // ---- 3. ask_persona liefert die Rückfrage-Felder -----------------------------------------------------------
    let own = finding(&m, 0, None, 0, "@nathan", "„alten Wehr“ bleibt ohne Bild.").await;
    m.model
        .push(Step::words(&format!("Erledigt: Befund {own} (Stelle „alten Wehr“)\n\n{}", prompts::ALL_DONE_SENTENCE)));
    let asked =
        tool(&m, &client, "ask_persona", json!({ "work": work, "key": key, "persona": "@nathan", "mode": "review" }))
            .await;
    assert_eq!(asked["done_proposals"], json!([own]), "der als erledigt gemeldete eigene Befund: {asked}");
    assert_eq!(asked["all_done"], true, "die Persona meldet, dass alle ihre Befunde erledigt scheinen: {asked}");

    // ---- 4. Tokens: Umlaut umschrieben, eindeutige Handles, Kollision abgelehnt ---------------------------------
    let kueche = result(&create_token(&m, "Laptop Küche").await)["secret"].as_str().unwrap().to_owned();
    let kuche = result(&create_token(&m, "laptop-kuche").await)["secret"].as_str().unwrap().to_owned();
    for name in ["Laptop Kueche", "laptop-k-che", "LAPTOP KUCHE"] {
        let answer = create_token(&m, name).await;
        assert_eq!(error_code(&answer), -32000, "{name:?} ergäbe einen vergebenen (Alt-)Handle: {answer}");
        assert!(!answer.to_string().contains("baruch_"), "eine Ablehnung trägt keinen Klartext eines Tokens");
    }
    let mut authors = Vec::new();
    for secret in [&kueche, &kuche] {
        let remote = remote(&m, secret).await;
        let post = tool(&m, &remote, "add_post", json!({ "work": work, "key": key, "body": "Gruß vom Laptop." })).await;
        authors.push(post["author"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        authors,
        ["@mcp:laptop-kueche", "@mcp:laptop-kuche"],
        "„Laptop Küche“ und „laptop-kuche“ schreiben unter verschiedenen, eindeutigen Handles (ü → ue)"
    );
    let (_, page) = m.server.get("/einstellungen/mcp").await;
    assert!(
        page.contains("<code>@mcp:laptop-kueche</code>") && page.contains("<code>@mcp:laptop-kuche</code>"),
        "die Token-Liste zeigt beide Autoren (ohne JavaScript): {page}"
    );
    assert!(!page.contains(&kueche) && !page.contains(&kuche), "nie ein Klartext eines Tokens auf der Seite");

    // ---- 5. Ohne Token: nie der Name eines Tokens, nie der einer Persona als Autor ------------------------------
    let twin = local(&m, "Laptop Küche").await;
    let post = tool(&m, &twin, "add_post", json!({ "work": work, "key": key, "body": "Vom Zwilling." })).await;
    assert_eq!(post["author"], "@mcp:lokal-laptop-kueche", "Client ohne Token mit dem Namen eines Tokens: {post}");
    let namesake = local(&m, "lektor").await;
    let post = tool(&m, &namesake, "add_post", json!({ "work": work, "key": key, "body": "Kein Lektor." })).await;
    assert_eq!(
        post["author"], "@mcp:lektor",
        "ein Client, der sich wie eine Persona nennt, schreibt als @mcp:…, nie als die Persona @lektor: {post}"
    );
    let with_token = remote(&m, &kueche).await;
    let text = refused(
        &m,
        &with_token,
        "add_post",
        json!({ "work": work, "key": key, "body": "Als Lektor ausgegeben.", "persona": "@lektor" }),
    )
    .await;
    assert!(text.contains("nur im eigenen Namen"), "mit Token gibt es keinen Beitrag unter Persona-Namen: {text}");
    let stranger = Caller { foreign: true, token: None, session: None };
    let message = json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call",
                          "params": { "name": "add_post", "arguments": { "work": work, "key": key, "body": "Fremd." } } });
    let (status, _, answer) = mcp(&m, &stranger, message).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "von einem anderen Rechner ohne Token geht gar nichts: {answer}");

    // ---- 6. own_reads: fremde Seite als Unterressource, eigene Navigation -----------------------------------------
    let uri = format!("/gliederung?work={work}");
    let image = [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "no-cors"), ("sec-fetch-dest", "image")];
    let (status, _, text) = m.server.send(get(&uri, &image)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "eine fremde Seite liest nichts als Unterressource: {text}");
    assert_eq!(text, "Abgelehnt: Anfrage von einer fremden Seite (lesend)", "Meldung der Schicht own_reads");
    let link = [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "document")];
    let (status, _, page) = m.server.send(get(&uri, &link)).await;
    assert_eq!(status, StatusCode::OK, "ein Link von außen (Navigation) geht weiter durch");

    // ---- 7. Ohne JavaScript: Antrag zur Gliederung im Menü, Meldung nur über feste Codes -------------------------
    assert!(
        page.contains("data-badge-item=\"outline_requests\" hidden>"),
        "ohne offenen Antrag ist der Eintrag „Anträge zur Gliederung“ verborgen"
    );
    tool(&m, &client, "outline_rename", json!({ "work": work, "key": m.keys[1], "title": "Der neue Mahlgang" })).await;
    let (_, page) = m.server.get(&format!("/einstellungen/daten?work={work}")).await;
    assert!(
        page.contains("<span class=\"badge\" data-badge=\"outline_requests\">1</span>")
            && page.contains(">Buch <span class=\"badge\" data-badge=\"outline_requests\" title=\"offene Anträge zur Gliederung\">1</span></a>")
            && page.contains("data-badge-item=\"outline_requests\">Anträge zur Gliederung"),
        // Runde 33 (Menüband): Eintrag und Zahl stehen im Reiter Buch (früher Menü Werk ▾), dazu das Abzeichen am Reiter.
        "der Antrag des Clients steht ohne JavaScript im Band jeder Seite (hier: Einstellungen): {page}"
    );
    let (status, page) =
        m.server.get(&format!("/einstellungen/daten?work={work}&ok=sicherung&ziel=ordner-aus-der-adresse")).await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    // Runde 36: Der feste Code allein ergibt keine Meldung mehr – nur nach einer eben angelegten Sicherung.
    assert!(!page.contains("Sicherung angelegt"), "ein Link mit ok=sicherung zeigt keine Erfolgsmeldung: {page}");
    // Runde 33 (Menüband): Die Reiter des Kopfs wiederholen die Abfrage der Seite in ihren Adressen (href, maskiert);
    // angezeigt wird sie nicht. Geprüft wird darum alles außer den href-Werten.
    let shown: String = page
        .split("href=\"")
        .enumerate()
        .map(|(i, part)| if i == 0 { part } else { &part[part.find('"').unwrap_or(0)..] })
        .collect();
    assert!(!shown.contains("ordner-aus-der-adresse"), "Text aus der Adresse (ziel) wird nie angezeigt: {page}");
}

/// Geheimnisse Ende zu Ende: Ein Schlüssel, den die Gegenstelle kodiert zurückspiegelt, erscheint in keiner Meldung;
/// die Prüfung einer Verbindung folgt keiner Weiterleitung; Treffer der Websuche im eigenen Netz werden nicht abgerufen.
#[tokio::test]
async fn secrets_stay_masked_checks_follow_no_redirect_and_private_hits_are_not_fetched() {
    // Kodiert zurückgespiegelt: Base64 (auch als Teil von `Basic base64(nutzer:schlüssel)`) und Prozentform.
    let base64 = baruch_store::encode_base64(SECRET.as_bytes());
    let basic = baruch_store::encode_base64(format!("nutzer:{SECRET}").as_bytes());
    let percent = SECRET.replace('-', "%2D");
    let (server, eliezer) = with_models().await;
    let names = [format!("echo-{base64}"), format!("Basic {basic}"), format!("echo-{percent}")];
    eliezer.set_models(&names.iter().map(String::as_str).collect::<Vec<_>>());
    let set = server.rpc("connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })).await;
    assert!(set["error"].is_null(), "Schlüssel gesetzt: {set}");
    let checked = server.rpc("connection.check", json!({ "kind": "eliezer" })).await.to_string();
    let (status, page) = server.get("/einstellungen/verbindungen").await;
    assert_eq!(status, StatusCode::OK, "die Seite der Verbindungen lädt");
    for (what, text) in [("Prüfergebnis", &checked), ("Seite der Verbindungen", &page)] {
        for (form, needle) in [
            ("wörtlich", SECRET),
            ("Base64", base64.trim_end_matches('=')),
            ("Basic-Kopf", basic.trim_end_matches('=')),
            ("Prozentform", percent.as_str()),
        ] {
            assert!(!text.contains(needle), "{what}: der Schlüssel steht nicht darin ({form})");
        }
    }
    assert!(checked.contains("***"), "die gespiegelten Namen sind geschwärzt, nicht weggelassen: {checked}");

    // Prüfung folgt keiner Weiterleitung: Der zweite Nachbau sieht keine Anfrage und keinen Schlüssel.
    let (first, target) = redirecting("302 Found").await;
    let set = server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": first.url } })).await;
    assert!(set["error"].is_null(), "Adresse gesetzt: {set}");
    let set = server.rpc("connection.set_secret", json!({ "kind": "eliezer", "secret": SECRET })).await;
    assert!(set["error"].is_null(), "Schlüssel an die neue Adresse gebunden: {set}");
    let checked = server.rpc("connection.check", json!({ "kind": "eliezer" })).await;
    assert_eq!(checked["result"]["ok"], false, "eine Weiterleitung ist kein Erfolg: {checked}");
    assert!(
        first.requests().iter().any(|r| r.contains(&format!("Bearer {SECRET}"))),
        "die eingetragene Adresse bekam die Anfrage mit dem Schlüssel (sonst prüfte der Test nichts)"
    );
    assert!(target.requests().is_empty(), "das Ziel der Weiterleitung sieht keine Anfrage: {:?}", target.requests());

    // Websuche: Adressen des eigenen Netzes gehen nie an fetch_url.
    let site = MockSite::with_fixtures().await;
    let mcp = MockWebMcp::start(Some(&site)).await;
    let set = server.rpc("connection.set", json!({ "kind": "multigpt", "config": { "url": mcp.url() } })).await;
    assert!(set["error"].is_null(), "MultiGPT-Nachbau eingetragen: {set}");
    let hit =
        |title: &str, url: &str| json!({ "title": title, "url": url, "content": format!("Ausschnitt zu {title}") });
    let private = ["http://127.0.0.1:8765/einstellungen", "http://192.168.1.1/verwaltung", "http://[::1]:8765/mcp"];
    let mut results: Vec<Value> = private.iter().map(|url| hit("Intern", url)).collect();
    results.push(hit("Licht", "https://lichtkunde.example/feuer"));
    mcp.set_results(results);
    let research = baruch_server::websearch::research(&server.state, "Feuer am Leuchtturm").await;
    let fetched: Vec<String> =
        mcp.tool_calls("fetch_url").iter().map(|a| a["url"].as_str().unwrap_or("").to_owned()).collect();
    assert_eq!(fetched, ["https://lichtkunde.example/feuer"], "nur die öffentliche Adresse wurde abgerufen");
    for url in private {
        assert!(
            research.notes.iter().any(|n| n.contains("nicht abgerufen") && n.contains(url)),
            "zu {url} steht ein ehrlicher Hinweis: {:?}",
            research.notes
        );
    }
}

/// Codereview ki-2 (Rest, Integrator): `/mcp` hat eine eigene, kleinere Größengrenze als `/rpc`.
#[tokio::test]
async fn an_mcp_request_over_its_own_body_limit_is_refused_with_413() {
    let server = common::open().await;
    let limit = baruch_server::MCP_BODY_LIMIT;
    assert!(
        limit < baruch_server::works::RPC_BODY_LIMIT && limit > baruch_store::MAX_ASSET_BYTES / 3 * 4 + 64 * 1024,
        "die Grenze von /mcp liegt unter der von /rpc und über einem Bild mit 8 MiB als Base64 samt Rahmen: {limit}"
    );
    let filler = "x".repeat(limit + 1);
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                          "params": { "name": "add_post", "arguments": { "key": 1, "body": filler } } });
    let request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "eine Anfrage über der Grenze wird nicht erst gelesen (413)");
}
