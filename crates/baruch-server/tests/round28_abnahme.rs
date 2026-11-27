// SPDX-License-Identifier: AGPL-3.0-or-later
//! Acceptance of Round 28 (integrator): invented work with 3 parts, 12 chapters, 6 personas; RFC, status, matrix,
//! badges, round trip u4, chat (contract R28 §6).
//!
//! Here is what only works after the three branches have been merged: the splitting (befunde) depends on the RFC and on
//! the decisions in the context (chat-auftrag), the numbers are shown by matrix, badges, map and revision tree
//! (befund-uebersicht). No network, no Eliezer: models only through `ScriptedModel`, which records every call sent; the
//! order of the answers is set by the test from `assist::chat::rfc_personas`, so that no script depends on the order of
//! the personas in the database. The „Brückenbuch“ is freely invented.

use crate::common;

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::port::{ModelCall, Role};
use baruch_server::assist::{Mode, Order, OrderFrame, Scope, context, prompts};
use baruch_store::{FindingFilter, FindingKind, FindingStatus};
use serde_json::{Value, json};

const PARTS: [(&str, [&str; 4]); 3] = [
    ("Ufer", ["Steg", "Brücke", "Fähre", "Pass"]),
    ("Strom", ["Schleuse", "Wehr", "Insel", "Mündung"]),
    ("Meer", ["Hafen", "Leuchtfeuer", "Sandbank", "Heimkehr"]),
];

/// Wording of the chapter „Brücke“: Nathan's critical finding quotes from it (duty to give evidence, Round 17).
const BRIDGE_TEXT: &str = "Die Brücke ächzt unter dem Karren. Unten zieht der Fluss grau vorbei.";

fn book() -> String {
    let mut text = String::from("#set document(title: \"Brückenbuch\", author: \"Ilse Fluss\")\n\n");
    for (part, chapters) in PARTS {
        text.push_str(&baruch_typst::heading_line(0, part, None));
        text.push_str("\n\nVorrede zum Teil.\n\n");
        for chapter in chapters {
            let body = if chapter == "Brücke" {
                BRIDGE_TEXT.to_owned()
            } else {
                format!("Der Weg führt weiter zum Kapitel {chapter}.")
            };
            text.push_str(&format!("= {chapter}\n\n{body}\n\n"));
        }
    }
    text
}

struct Bridge {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    slug: String,
    keys: HashMap<String, i64>,
}

impl Bridge {
    fn key(&self, title: &str) -> i64 {
        *self.keys.get(title).unwrap_or_else(|| panic!("kein Punkt {title}: {:?}", self.keys))
    }

    async fn commits(&self) -> usize {
        common::git(&self.server.mirror_dir_of(&self.slug), &["rev-list", "--count", "HEAD"])
            .map_or(0, |c| c.trim().parse().unwrap())
    }

    async fn last_commit(&self) -> String {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "-1", "--format=%s%n%b"]).unwrap_or_default()
    }

    /// Form without JavaScript; returns status, Location and body.
    async fn form(&self, uri: &str, form: String) -> (StatusCode, Option<String>, String) {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form))
            .unwrap();
        let (status, headers, body) = self.server.send(request).await;
        let location = headers.get(header::LOCATION).map(|l| l.to_str().unwrap().to_owned());
        (status, location, body)
    }

    async fn page(&self, uri: &str) -> String {
        let (status, html) = self.server.get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
        unescape(&html)
    }
}

async fn bridge() -> Bridge {
    let model = ScriptedModel::new();
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model.clone()),
    )
    .await
    .expect("Datenordner lässt sich nicht öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let report = server.import(&book(), "brueckenbuch.typ").await;
    let keys = server
        .state
        .store
        .load_work(report.work)
        .await
        .unwrap()
        .nodes
        .iter()
        .map(|n| (n.title.clone(), n.key))
        .collect();
    Bridge { server, model, work: report.work, slug: report.slug, keys }
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none_or(Value::is_null), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

fn user(call: &ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::User).map(|m| m.content.as_str()).unwrap()
}

fn system(call: &ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::System).map(|m| m.content.as_str()).unwrap()
}

/// Undo the HTML escaping of minijinja as far as addresses carry it.
fn unescape(html: &str) -> String {
    html.replace("&#x2f;", "/").replace("&amp;", "&")
}

/// Value of an attribute in the first tag that contains `marker`.
fn attr(html: &str, marker: &str, name: &str) -> String {
    let at = html.find(marker).unwrap_or_else(|| panic!("{marker} fehlt: {html}"));
    let start = if html[at..].starts_with('<') { at } else { html[..at].rfind('<').unwrap() };
    let end = at + html[at..].find('>').unwrap();
    let tag = &html[start..end];
    let key = format!(" {name}=\"");
    let from = tag.find(&key).unwrap_or_else(|| panic!("{name} fehlt in {tag}")) + key.len();
    tag[from..from + tag[from..].find('"').unwrap()].to_owned()
}

/// Number `data-findings-open` at the entry `data-key="<key>"` of the outline on the work page.
fn tree_badge(html: &str, key: i64) -> Option<usize> {
    let at = html.find(&format!(" data-key=\"{key}\" data-level="))?;
    let rest = &html[at..];
    let end = rest[1..].find("<li ").map_or(rest.len(), |e| e + 1);
    let entry = &rest[..end];
    let from = entry.find("data-findings-open=\"")? + "data-findings-open=\"".len();
    entry[from..from + entry[from..].find('"')?].parse().ok()
}

fn row_of(matrix: &Value, key: i64) -> Value {
    matrix["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["rows"].as_array().unwrap())
        .find(|r| r["key"] == key)
        .unwrap_or_else(|| panic!("keine Zeile für {key}: {matrix}"))
        .clone()
}

/// Answers of the five other personas in the RFC (exactly one finding with a mark each), by handle.
fn rfc_answer(handle: &str) -> &'static str {
    match handle {
        "@exeget" => "Hinweis: Der Übergang vom Steg zur Brücke kommt ohne Ankündigung.",
        "@schrift" => "Hinweis: Eine Anspielung auf Psalm 23 läge beim grauen Wasser nahe.",
        "@belege" => "Hinweis: Wie schwer der Karren ist, bleibt ohne jede Angabe.",
        "@nathan" => {
            "Kritisch: „Die Brücke ächzt“ wiederholt das Bild knarrender Bohlen aus dem Steg.\n- statt: „Die Brücke ächzt“\n- besser: „Die Brücke trägt“"
        }
        "@hoerer" => "Lob: Der graue Fluss unter den Füßen bleibt im Ohr.",
        other => panic!("unerwartete Persona im RFC: {other}"),
    }
}

/// Uuid, kind, status, reason, revision, origins, attachment and history length per finding (without ids of the
/// instance).
async fn finding_rows(server: &common::Server, work: i64) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT f.uuid || '|' || f.persona || '|' || f.kind || '|' || f.status || '|' || f.reason || '|' \
         || coalesce(f.adopted_revision, '') || '|' || coalesce(f.deferred_node, '') || '|' || f.summary_of || '|' \
         || coalesce((SELECT o.uuid FROM finding o WHERE o.id = f.similar_to), '') || '|' \
         || (SELECT count(*) FROM finding_status s WHERE s.finding_id = f.id) \
         FROM finding f WHERE f.work_id = ? ORDER BY f.uuid",
    )
    .bind(work)
    .fetch_all(server.state.store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn rfc_becomes_findings_the_author_decides_and_every_view_shows_the_same_numbers() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge().await;
    let state = &b.server.state;
    let bridge_key = b.key("Brücke");
    let pass = b.key("Pass");

    // ---- RFC in the chapter Brücke: all six personas, the lector last with the summary --------------------------
    let (others, lector) = baruch_server::assist::chat::rfc_personas(state).await.unwrap();
    assert_eq!(lector.as_deref(), Some("@lektor"), "Vorgabe: fester Lektor @lektor");
    let mut sorted = others.clone();
    sorted.sort();
    assert_eq!(sorted, ["@belege", "@exeget", "@hoerer", "@nathan", "@schrift"], "sechs aktive Personas");
    let base: i64 =
        sqlx::query_scalar("SELECT coalesce(max(id), 0) FROM finding").fetch_one(state.store.pool()).await.unwrap();
    // Every answer yields exactly one finding; their ids therefore follow the order of the calls (checked below).
    let id_of = |handle: &str| base + 1 + others.iter().position(|h| h == handle).unwrap() as i64;
    for handle in &others {
        b.model.push(Step::words(rfc_answer(handle)));
    }
    let (exeget, belege, nathan) = (id_of("@exeget"), id_of("@belege"), id_of("@nathan"));
    let lector_answer = format!(
        "Jetzt wichtig:\n- Vorschlag: Steg und Brücke klanglich trennen, damit die Bohlen nicht doppelt knarren \
         [B{nathan}] [B{exeget}].\nKann warten:\n- Vorschlag: Gewicht des Karrens nennen [B{belege}]."
    );
    b.model.push(Step::words(&lector_answer));

    let posts_before: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, body FROM post ORDER BY id").fetch_all(state.store.pool()).await.unwrap();
    let answer = b
        .server
        .rpc("persona.chat", json!({ "work": b.work, "key": bridge_key, "message": "RFC zur Brücke, bitte" }))
        .await;
    let r = result(&answer).clone();
    assert_eq!(r["rfc"], true, "„RFC“ am Anfang erkannt: {r}");
    assert!(r["errors"].as_array().unwrap().is_empty(), "alle haben geantwortet: {r}");
    let calls = b.model.calls();
    let order: Vec<&str> = calls.iter().map(|c| c.persona.as_str()).collect();
    let mut expected: Vec<&str> = others.iter().map(String::as_str).collect();
    expected.push("@lektor");
    assert_eq!(order, expected, "jede aktive Persona genau einmal (mehr als drei), der Lektor zuletzt");
    let last = user(calls.last().unwrap());
    assert!(last.contains(context::RFC_TITLE) && last.contains(context::RFC_TASK), "Lektor sammelt: {last}");
    assert!(
        last.contains(&format!("[B{nathan}]")),
        "der Lektor sieht die Befunde der anderen mit ihren Verweisen [B<id>] (zum Zitieren): {last}"
    );

    // Split: one finding per persona (praise stored but hidden), the lector two summary proposals.
    let all = FindingFilter { include_praise: true, include_attached: true, ..FindingFilter::default() };
    let found = state.store.finding_list(b.work, &all).await.unwrap();
    assert_eq!(found.len(), 7, "fünf Befunde der anderen und zwei Sammelvorschläge des Lektors: {found:#?}");
    for handle in &others {
        let f = found.iter().find(|f| f.id == id_of(handle)).unwrap_or_else(|| panic!("Befund von {handle}"));
        assert_eq!(&f.persona, handle, "ids in der Reihenfolge der Aufrufe (Grundlage der [B<id>] im Skript)");
        assert_eq!(f.key, Some(bridge_key), "am Kapitel Brücke");
        assert!(f.post.is_some(), "Herkunft: der Beitrag der Persona");
    }
    assert_eq!(found.iter().find(|f| f.id == nathan).unwrap().kind, FindingKind::Critical, "„Kritisch:“");
    assert_eq!(found.iter().find(|f| f.id == id_of("@hoerer")).unwrap().kind, FindingKind::Praise, "„Lob:“");
    let summaries: Vec<_> = found.iter().filter(|f| f.persona == "@lektor").collect();
    assert_eq!(summaries.len(), 2, "zwei Sammelvorschläge");
    let uuid = |id: i64| found.iter().find(|f| f.id == id).unwrap().uuid.clone();
    assert_eq!(summaries[0].summary_of, vec![uuid(nathan), uuid(exeget)], "Ursprünge aus [B<id>] (Vertrag 1.6)");
    assert_eq!(summaries[1].summary_of, vec![uuid(belege)], "zweiter Sammelvorschlag");
    assert!(summaries.iter().all(|s| s.kind == FindingKind::Hint), "Sammelvorschläge sind Hinweise");
    let mut changed = 0;
    for (id, body) in &posts_before {
        let now: String = sqlx::query_scalar("SELECT body FROM post WHERE id = ?")
            .bind(id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
        changed += usize::from(&now != body);
    }
    assert_eq!(changed, 0, "Zerlegen ändert keinen Beitrag (Vertrag 1.3)");
    let lector_post: String = sqlx::query_scalar("SELECT body FROM post WHERE author = '@lektor' ORDER BY id DESC")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(lector_post, lector_answer, "der Beitrag des Lektors steht wörtlich im Chat");

    // ---- The author decides without JavaScript (forms), one commit per decision -------------------------------
    let back = format!("%2F%3Fwork%3D{}%26key%3D{bridge_key}", b.work);
    let before = b.commits().await;
    for (form, what) in [
        (
            format!("method=finding.set_status&work={}&id={exeget}&status=adopted&rev=1&back={back}", b.work),
            "übernommen",
        ),
        (
            format!(
                "method=finding.set_status&work={}&id={nathan}&status=rejected&reason=Das+Echo+ist+gewollt&back={back}",
                b.work
            ),
            "verworfen",
        ),
        (
            format!(
                "method=finding.set_status&work={}&id={belege}&status=deferred&target_key={pass}&back={back}",
                b.work
            ),
            "verschoben",
        ),
    ] {
        let (status, location, body) = b.form("/werke/befunde/act", form).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{what}: Formular gelingt: {body}");
        assert_eq!(location.as_deref(), Some(format!("/?work={}&key={bridge_key}", b.work).as_str()), "zurück");
    }
    assert_eq!(b.commits().await, before + 3, "jede Statusänderung genau ein Commit im Spiegel (Vertrag 1.2)");
    assert!(b.last_commit().await.starts_with(&format!("Befund {belege}:")), "Betreff nennt den Befund");
    let (status, _, body) = b
        .form(
            "/werke/befunde/act",
            format!("method=finding.set_status&work={}&id={}&status=rejected&back={back}", b.work, id_of("@schrift")),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "verwerfen ohne Begründung: Hinweis statt Umleitung");
    assert!(body.contains("Begründung"), "der Hinweis nennt die fehlende Begründung: {body}");
    assert_eq!(b.commits().await, before + 3, "abgelehnt: kein Commit");

    // ---- Next review: decisions framed, the repetition is attached --------------------------------------------
    b.model.push(Step::words(rfc_answer("@nathan")));
    let review = Order {
        work: b.work,
        key: bridge_key,
        persona: "@nathan".into(),
        mode: Mode::Review,
        text: String::new(),
        rev: None,
        asker: None,
        post_question: false,
        sources: vec![],
        web: false,
        scope: Scope::Section,
        frame: OrderFrame::default(),
        links: Default::default(),
        basis: None,
    };
    baruch_server::assist::run(state, &review, None).await.expect("Prüfung gelingt");
    let sent = user(b.model.calls().last().unwrap()).to_owned();
    assert!(sent.contains(context::DECISIONS_TITLE), "die Entscheidungen gehen gerahmt mit: {sent}");
    assert!(
        sent.contains(&format!("[B{nathan}] vom Autor verworfen")) && sent.contains("Das Echo ist gewollt"),
        "der verworfene Befund mit Verweis und Begründung: {sent}"
    );
    assert!(sent.contains(&format!("[B{exeget}] vom Autor übernommen in Fassung 1")), "der übernommene: {sent}");
    let found = state.store.finding_list(b.work, &all).await.unwrap();
    let repeat = found.iter().find(|f| f.id > base + 7).expect("die Wiederholung ist gespeichert");
    assert_eq!(repeat.similar_to, Some(nathan), "wiederholter verworfener Befund wird angehängt (Vertrag 1.4)");

    // ---- Numbers: matrix, finding_open_counts, badges in outline, editor header, /gliederung, map ---------
    // Open at the bridge counts the hint of @schrift; not: praise, accepted, discarded, postponed, the
    // attached repetition (counting rule 1.5) and – since Round 23a (PM decision) – the two summary proposals:
    // their origins already count, otherwise „k offen“ would rise twice after an RFC. Matrix and badges count
    // alike; the summary proposals are still in the list „Sammelvorschläge“ of the findings page.
    let counts = state.store.finding_open_counts(b.work).await.unwrap();
    assert_eq!(counts.get(&bridge_key), Some(&1), "finding_open_counts ohne Sammelvorschläge: {counts:?}");
    let m = result(&b.server.rpc("finding.matrix", json!({ "work": b.work })).await).clone();
    let row = row_of(&m, bridge_key);
    assert_eq!((row["open_badge"].as_u64(), row["total"]["open"].as_u64()), (Some(1), Some(1)), "Matrix: {row}");
    let workspace = b.page(&format!("/?work={}&key={bridge_key}", b.work)).await;
    assert_eq!(tree_badge(&workspace, bridge_key), Some(1), "Abzeichen in der Gliederung der Arbeitsseite");
    assert_eq!(tree_badge(&workspace, b.key("Ufer")), Some(1), "der Teil Ufer summiert");
    assert_eq!(attr(&workspace, "editor-findings", "data-findings-open"), "1", "Abzeichen im Editorkopf");
    let outline = b.page(&format!("/gliederung?work={}", b.work)).await;
    let marker = format!("befunde?key={bridge_key}#zelle\" data-findings-open=\"");
    assert_eq!(attr(&outline, &marker, "data-findings-open"), "1", "auch /gliederung");
    let map = b.page(&format!("/werke/{}/karte?befunde=1", b.work)).await;
    assert_eq!(attr(&map, &format!("data-node=\"n:{bridge_key}\""), "data-open-findings"), "1", "Gedankenkarte");
    assert!(map.contains(">1 offen</text>"), "Ebene Befunde als Text im Bild, nicht nur Farbe");

    // ---- Revision tree, findings page, accordion without JavaScript -------------------------------------------
    assert!(
        workspace.contains(&format!(
            "✓ setzt Befund <a href=\"/werke/{}/befunde?befund={exeget}\">[B{exeget}]</a> um",
            b.work
        )),
        "Fassungsbaum: Fassung 1 setzt den übernommenen Befund um"
    );
    assert!(workspace.contains("data-section=\"befunde\""), "Akkordeon „Befunde“ am Abschnitt");
    assert!(workspace.contains("action=\"/werke/befunde/act\""), "Statusformulare ohne JavaScript");
    let page = b.page(&format!("/werke/{}/befunde", b.work)).await;
    assert!(page.contains("Sammelvorschlag von @lektor"), "Sammelvorschläge mit Herkunft");
    assert!(page.contains(&format!("/werke/{}/befunde?befund={nathan}", b.work)), "Link auf einen Ursprung");
    let single = b.page(&format!("/werke/{}/befunde?befund={}", b.work, repeat.id)).await;
    assert!(
        single.contains(&format!("Angehängt an <a href=\"/werke/{}/befunde?befund={nathan}\">", b.work)),
        "die Wiederholung verweist auf das verworfene Original"
    );

    // ---- Lector as companion, Nathan as reviewer --------------------------------------------------------------
    let calls_before = b.model.calls().len();
    b.model.push(Step::words("Der Pass wirkt still und gut. Vorschlag: einen Laut der Höhe ergänzen."));
    let answer = b
        .server
        .rpc("persona.chat", json!({ "work": b.work, "key": pass, "message": "@lektor Wie wirkt der Pass?" }))
        .await;
    result(&answer);
    let calls = b.model.calls();
    assert_eq!(calls.len(), calls_before + 1, "Lektor ohne „Kritisch:“: keine Nachforderung");
    assert!(
        system(calls.last().unwrap()).contains("In the chat first respond to the author's message"),
        "der Lektor geht im Chat zuerst auf die Nachricht ein"
    );
    b.model.push(Step::words("Der Pass ist rund."));
    b.model.push(Step::words("Auch beim zweiten Mal rund."));
    let answer = b
        .server
        .rpc("persona.chat", json!({ "work": b.work, "key": pass, "message": "@nathan Wie wirkt der Pass?" }))
        .await;
    result(&answer);
    let calls = b.model.calls();
    assert_eq!(calls.len(), calls_before + 3, "Nathan ohne „Kritisch:“ wird einmal nachgefordert");
    assert!(system(calls.last().unwrap()).contains(prompts::CRITICAL_RETRY_NOTE), "mit Hinweis auf den Befund");

    // ---- Round trip u4 between two throwaway instances -------------------------------------------------------
    let xml = result(&b.server.rpc("work.exchange_export", json!({ "work": b.work, "format": "xml" })).await)["xml"]
        .as_str()
        .unwrap()
        .to_owned();
    // Round 34 (mindmap_daten): the writer now writes minor version 9 (tree of the ideas).
    assert!(xml.contains("version=\"10\"") && xml.contains("<befunde>"), "Unterversion 9 (seit Runde 34) mit Befunden");
    let other = common::open().await;
    let bytes = xml.as_bytes();
    let imported = other
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(bytes), "sha256": baruch_exchange::sha256_hex(bytes),
                    "mode": "new" }),
        )
        .await;
    let work_b = result(&imported)["work"].as_i64().unwrap();
    assert_eq!(
        finding_rows(&other, work_b).await,
        finding_rows(&b.server, b.work).await,
        "Befunde mit Status, Begründung, Fassung, Ziel, Ursprüngen, Anhang und Verlauf kommen gleich an"
    );
    let counts_b = other.state.store.finding_open_counts(work_b).await.unwrap();
    assert_eq!(counts_b.values().sum::<usize>(), counts.values().sum::<usize>(), "gleich viele offene Befunde");
}

#[tokio::test]
async fn chat_orders_and_post_edits_work_end_to_end_without_javascript() {
    let _turn = common::model_queue(module_path!()).await;
    let b = bridge().await;
    let state = &b.server.state;
    let bridge_key = b.key("Brücke");

    // A finding of the persona that a writing order is to implement: the status proposal lands at the finding.
    b.model.push(Step::words("Hinweis: Der Fluss bekommt keinen Namen."));
    let answer = b
        .server
        .rpc("persona.chat", json!({ "work": b.work, "key": bridge_key, "message": "@exeget Was fehlt?" }))
        .await;
    let target = result(&answer)["findings"][0].as_i64().unwrap_or_else(|| panic!("ein Befund entsteht: {answer}"));
    b.model.push(Step::words("= Brücke\n\nDie Brücke ächzt unter dem Karren. Unten zieht die Oder grau vorbei.\n"));
    let answer = b
        .server
        .rpc(
            "persona.chat",
            json!({ "work": b.work, "key": bridge_key,
                    "message": format!("@lektor Schreibe die Brücke neu und setze [B{target}] um") }),
        )
        .await;
    let r = result(&answer).clone();
    assert_eq!(r["proposals"].as_array().unwrap().len(), 1, "Bitte um Text im Chat ergibt einen Vorschlag: {r}");
    let rev = r["proposals"][0]["rev"].as_u64().unwrap() as u32;
    let finding = state.store.finding_get(b.work, target).await.unwrap();
    assert_eq!(finding.status, FindingStatus::Open, "den Status setzt nur der Mensch");
    let pending =
        finding.pending.expect("der Schreibauftrag stellt einen Statusvorschlag (über findings::rpc::propose)");
    assert_eq!(pending.adopted_rev, Some(rev), "„übernommen in Fassung {rev}“ als Vorschlag");
    assert!(pending.proposed, "nur Vorschlag");
    let before = b.commits().await;
    let (status, _, body) = b
        .form(
            "/werke/befunde/act",
            format!("method=finding.decide&work={}&entry={}&confirm=0&back=%2F", b.work, pending.id),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "der Mensch verwirft den Vorschlag ohne JavaScript: {body}");
    assert_eq!(b.commits().await, before + 1, "die Entscheidung ist ein Commit");
    assert_eq!(state.store.finding_get(b.work, target).await.unwrap().pending, None, "kein offener Vorschlag mehr");

    // Convert a persona post into findings: the post stays byte-identical.
    let persona_post: (i64, String) =
        sqlx::query_as("SELECT id, body FROM post WHERE author = '@lektor' ORDER BY id DESC")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    let converted = b.server.rpc("post.to_findings", json!({ "work": b.work, "id": persona_post.0 })).await;
    let converted = result(&converted).clone();
    assert!(
        !converted["findings"].as_array().unwrap().is_empty(),
        "manuelles Umwandeln ergibt mindestens einen Befund (manual = true): {converted}"
    );
    let now: String = sqlx::query_scalar("SELECT body FROM post WHERE id = ?")
        .bind(persona_post.0)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(now, persona_post.1, "der Beitrag bleibt unverändert");

    // Own post: edit with history and commit; a persona post cannot be edited.
    let (status, _) = b
        .server
        .post_form(
            "/assistenten/act",
            &format!("method=persona.chat&work={}&key={bridge_key}&message=Meine+erste+Notiz", b.work),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Notiz ohne Persona gespeichert");
    let own: i64 = sqlx::query_scalar("SELECT id FROM post WHERE author = 'Autor' ORDER BY id DESC")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let before = b.commits().await;
    let form = format!("method=post.edit&work={}&key={bridge_key}&id={own}&back=post&body=Meine+zweite+Notiz", b.work);
    let (status, location) = b.server.post_form("/beitraege/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "eigener Beitrag gespeichert");
    assert_eq!(location, Some(format!("/beitrag?work={}&id={own}", b.work)), "zurück auf die Beitragsseite");
    assert_eq!(b.commits().await, before + 1, "genau ein Commit");
    assert!(b.last_commit().await.starts_with(&format!("Beitrag {own}: bearbeitet")), "Betreff des Commits");
    let single = b.page(&format!("/beitrag?work={}&id={own}", b.work)).await;
    assert!(single.contains("Meine erste Notiz"), "die alte Fassung steht im Verlauf");
    assert!(single.contains("readonly") && single.contains(">Meine zweite Notiz</textarea>"), "Text markierbar");
    let persona_page = b.page(&format!("/beitrag?work={}&id={}", b.work, persona_post.0)).await;
    assert!(!persona_page.contains("value=\"post.edit\""), "Persona-Beitrag: kein Bearbeiten");
    assert!(persona_page.contains("value=\"persona.chat\""), "aber Zitieren als eigenes Chat-Formular");
    let form = format!("method=post.edit&work={}&key={bridge_key}&id={}&body=x", b.work, persona_post.0);
    let (status, _) = b.server.post_form("/beitraege/act", &form).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Persona-Beitrag über das Formular abgelehnt");

    // The chat scrolls within a limited height and stands at the newest post; whole history without JavaScript.
    let workspace = b.page(&format!("/?work={}&key={bridge_key}", b.work)).await;
    assert!(workspace.contains("id=\"chat-neueste\""), "Anker auf den neuesten Beitrag");
    assert!(
        workspace.contains(&format!("href=\"/beitraege?work={}&key={bridge_key}#chat-neueste\"", b.work)),
        "Link auf die ganze Historie, am neuesten Beitrag"
    );
    let (_, css) = b.server.get("/static/app.css").await;
    let chat_rule = &css[css.find(".chat {").unwrap()..];
    let chat_rule = &chat_rule[..chat_rule.find('}').unwrap()];
    assert!(chat_rule.contains("max-height") && chat_rule.contains("overflow-y: auto"), "Chat rollt: {chat_rule}");
    let history = b.page(&format!("/beitraege?work={}&key={bridge_key}", b.work)).await;
    assert!(history.contains(&format!("id=\"post-{own}\"")), "ganze Historie mit Ankern je Beitrag");

    // Thread: the RFC becomes findings with the origin thread post; anchor at the newest post.
    let (others, _) = baruch_server::assist::chat::rfc_personas(state).await.unwrap();
    for handle in &others {
        b.model.push(Step::words(rfc_answer(handle)));
    }
    b.model.push(Step::words("Jetzt wichtig: nichts. Kann warten: alles."));
    let started = b.server.rpc("thread.post", json!({ "work": b.work, "message": "RFC: Wie trägt das Buch?" })).await;
    let r = result(&started).clone();
    assert_eq!(r["rfc"], true, "RFC im Faden: {r}");
    let thread = r["thread"].as_i64().unwrap();
    let from_thread: i64 =
        sqlx::query_scalar("SELECT count(*) FROM finding WHERE thread_post_id IS NOT NULL AND work_id = ?")
            .bind(b.work)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(from_thread, 5, "je Persona ein Befund aus dem Faden (ohne Abschnitt, zählt nicht als offen)");
    let page = b.page(&format!("/faden?work={}&thread={thread}", b.work)).await;
    assert!(page.contains("id=\"chat-neueste\"") && page.contains("class=\"thread-scroll\""), "Faden rollt");
}
