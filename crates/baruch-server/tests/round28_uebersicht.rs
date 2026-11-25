// SPDX-License-Identifier: AGPL-3.0-or-later
//! Übersicht der Befunde: Matrix, Seite /werke/<id>/befunde, Abzeichen, Akkordeon, Gedankenkarte, Fassungsbaum, MCP
//! findings/finding_propose_status (Runde 28, Aufgabe befund-uebersicht).
//!
//! Das „Wanderbuch“ (3 Teile, 12 Kapitel, zwei Abschnitte) und alle Befunde sind frei erfunden. Befunde entstehen hier
//! über `Store::finding_insert` (S1 voll), Status und Vorschläge über rohes SQL auf `store.pool()`: Die Statuswege
//! (`finding.set_status`, `finding.decide`, `finding_propose_status`) setzt die Aufgabe befunde um. Wo ein Test durch
//! diese Wege geht (Formular, MCP-Vorschlag), nimmt er bis zum Zusammenführen die ehrliche Ablehnung „noch nicht
//! umgesetzt (R28a)“ an und prüft danach den Erfolg; den ganzen Weg bis zum Commit prüft der Integrator in
//! `round28_abnahme.rs`. Kein Modell, kein Netz.

use crate::common;

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_store::{FindingKind, NewFinding};
use serde_json::{Value, json};

const PARTS: [(&str, [&str; 4]); 3] = [
    ("Aufbruch", ["Morgen", "Brücke", "Furt", "Pass"]),
    ("Wanderung", ["Hütte", "Grat", "Sturm", "Lichtung"]),
    ("Ankunft", ["Tal", "Dorf", "Herd", "Abend"]),
];

fn book() -> String {
    let mut text = String::from("#set document(title: \"Wanderbuch\")\n\n");
    for (part, chapters) in PARTS {
        text.push_str(&baruch_typst::heading_line(0, part, None));
        text.push_str("\n\nVorrede zum Teil.\n\n");
        for chapter in chapters {
            text.push_str(&format!("= {chapter}\n\nDer Weg geht weiter durch {chapter}.\n\n"));
            if chapter == "Morgen" {
                text.push_str("== Nebel\n\nGrau liegt das Feld.\n\n== Tau\n\nNass sind die Schuhe.\n\n");
            }
        }
    }
    text
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Ids der erfundenen Befunde (für Zusicherungen).
#[derive(Default)]
struct Ids {
    morgen_lektor: Vec<i64>,
    morgen_rejected: i64,
    morgen_adopted: i64,
    morgen_praise: i64,
    morgen_attached: i64,
    nebel_nathan: i64,
    sturm_nathan: Vec<i64>,
    sturm_deferred: i64,
    abend_done: i64,
    abend_summary: i64,
    unplaced: i64,
}

struct Book {
    server: common::Server,
    work: i64,
    keys: HashMap<String, i64>,
    ids: Ids,
}

impl Book {
    fn key(&self, title: &str) -> i64 {
        *self.keys.get(title).unwrap_or_else(|| panic!("kein Punkt {title}: {:?}", self.keys))
    }

    fn pool(&self) -> &sqlx::SqlitePool {
        self.server.state.store.pool()
    }

    async fn page(&self, query: &str) -> (StatusCode, String) {
        let uri = if query.is_empty() {
            format!("/werke/{}/befunde", self.work)
        } else {
            format!("/werke/{}/befunde?{query}", self.work)
        };
        let (status, html) = self.server.get(&uri).await;
        (status, unescape(&html))
    }

    /// Arbeitsseite eines Abschnitts (Adressen entmaskiert wie bei [`Book::page`]).
    async fn workspace(&self, key: i64) -> (StatusCode, String) {
        let (status, html) = self.server.get(&format!("/?work={}&key={key}", self.work)).await;
        (status, unescape(&html))
    }

    async fn matrix(&self, params: Value) -> Value {
        let mut params = params;
        params["work"] = json!(self.work);
        result(&self.server.rpc("finding.matrix", params).await).clone()
    }
}

/// Befund anlegen (S1 voll: `Store::finding_insert`).
async fn insert(
    b: &Book,
    key: Option<i64>,
    persona: &str,
    kind: FindingKind,
    body: &str,
    similar_to: Option<i64>,
    summary_of: &[String],
) -> i64 {
    let new = NewFinding {
        work: b.work,
        key,
        post: None,
        thread_post: None,
        position: 0,
        persona,
        kind,
        body,
        quote: "",
        quote_check: "",
        similar_to,
        summary_of,
    };
    b.server.state.store.finding_insert(&new).await.expect("Befund lässt sich anlegen").id
}

/// Uuid einer Fassung (Abschnitt `key`, Nummer `rev`).
async fn revision_uuid(b: &Book, key: i64, rev: i64) -> String {
    sqlx::query_scalar("SELECT uuid FROM revision WHERE node_id = ? AND number = ?")
        .bind(key)
        .bind(rev)
        .fetch_one(b.pool())
        .await
        .expect("Fassung hat eine uuid")
}

async fn uuid_of(b: &Book, id: i64) -> String {
    sqlx::query_scalar("SELECT uuid FROM finding WHERE id = ?").bind(id).fetch_one(b.pool()).await.unwrap()
}

/// Erfundenes Werk mit Befunden in drei Teilen (siehe Kommentare je Gruppe).
async fn open_book() -> Book {
    let server = common::open().await;
    let report = server.import(&book(), "wanderbuch.typ").await;
    let answer = server.rpc("work.get", json!({ "work": report.work })).await;
    let keys = result(&answer)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["title"].as_str().unwrap().to_owned(), n["key"].as_i64().unwrap()))
        .collect();
    let mut b = Book { server, work: report.work, keys, ids: Ids::default() };
    let (morgen, nebel, sturm, abend, tal) =
        (b.key("Morgen"), b.key("Nebel"), b.key("Sturm"), b.key("Abend"), b.key("Tal"));
    let hint = FindingKind::Hint;

    // Morgen: @lektor zwei offen, einer verworfen; @exeget einer übernommen (Fassung 1); @hoerer Lob; ein angehängter.
    let l1 = insert(&b, Some(morgen), "@lektor", hint, "Der erste Satz trägt den Leser.", None, &[]).await;
    let l2 = insert(&b, Some(morgen), "@lektor", hint, "Der Übergang zum Nebel fehlt.", None, &[]).await;
    let rejected = insert(&b, Some(morgen), "@lektor", hint, "Das Wort Weg kommt zu oft vor.", None, &[]).await;
    let adopted = insert(&b, Some(morgen), "@exeget", hint, "Die Uhrzeit nennen.", None, &[]).await;
    let praise =
        insert(&b, Some(morgen), "@hoerer", FindingKind::Praise, "Schöner Anfang am Feldrand.", None, &[]).await;
    let attached = insert(&b, Some(morgen), "@lektor", hint, "Der erste Satz trägt.", Some(l1), &[]).await;
    sqlx::query("UPDATE finding SET status = 'rejected', reason = 'Gewollte Wiederholung' WHERE id = ?")
        .bind(rejected)
        .execute(b.pool())
        .await
        .unwrap();
    let rev1 = revision_uuid(&b, morgen, 1).await;
    sqlx::query("UPDATE finding SET status = 'adopted', adopted_revision = ? WHERE id = ?")
        .bind(&rev1)
        .bind(adopted)
        .execute(b.pool())
        .await
        .unwrap();
    // Offener Vorschlag von @lektor: l1 sei in Fassung 1 übernommen.
    sqlx::query(
        "INSERT INTO finding_status (finding_id, status, adopted_revision, actor_kind, actor, proposed, note) \
         VALUES (?, 'adopted', ?, 'persona', '@lektor', 1, 'steht schon so da')",
    )
    .bind(l1)
    .bind(&rev1)
    .execute(b.pool())
    .await
    .unwrap();
    // Nebel (Abschnitt unter Morgen): @nathan einer offen.
    let nebel_nathan = insert(&b, Some(nebel), "@nathan", hint, "Grau ist kein Bild.", None, &[]).await;

    // Sturm (Teil Wanderung): @nathan drei offen und kritisch (einer alt), @schrift einer verschoben nach Tal.
    let mut sturm_nathan = Vec::new();
    for body in ["Der Sturm kommt ohne Ankündigung.", "Wer spricht hier?", "Zeitform wechselt."] {
        sturm_nathan.push(insert(&b, Some(sturm), "@nathan", FindingKind::Critical, body, None, &[]).await);
    }
    sqlx::query("UPDATE finding SET created = '2026-01-01T08:00:00.000Z' WHERE id = ?")
        .bind(sturm_nathan[0])
        .execute(b.pool())
        .await
        .unwrap();
    let deferred = insert(&b, Some(sturm), "@schrift", hint, "Psalmbezug prüfen.", None, &[]).await;
    let tal_uuid: String =
        sqlx::query_scalar("SELECT uuid FROM node WHERE id = ?").bind(tal).fetch_one(b.pool()).await.unwrap();
    sqlx::query("UPDATE finding SET status = 'deferred', deferred_node = ? WHERE id = ?")
        .bind(&tal_uuid)
        .bind(deferred)
        .execute(b.pool())
        .await
        .unwrap();

    // Abend (Teil Ankunft): @belege einer erledigt; Sammelvorschlag von @lektor über zwei fremde Befunde.
    let done = insert(&b, Some(abend), "@belege", hint, "Quelle steht schon im Anhang.", None, &[]).await;
    sqlx::query("UPDATE finding SET status = 'done' WHERE id = ?").bind(done).execute(b.pool()).await.unwrap();
    let origins = vec![uuid_of(&b, sturm_nathan[1]).await, uuid_of(&b, l2).await];
    let summary =
        insert(&b, Some(abend), "@lektor", hint, "Jetzt wichtig: Sprecher und Übergang klären.", None, &origins).await;

    // Ohne Abschnitt (Faden): zählt in der Matrix unter „Ohne Abschnitt“, nicht in den Abzeichen.
    let unplaced = insert(&b, None, "@lektor", hint, "Allgemein: mehr Pausen.", None, &[]).await;

    b.ids = Ids {
        morgen_lektor: vec![l1, l2],
        morgen_rejected: rejected,
        morgen_adopted: adopted,
        morgen_praise: praise,
        morgen_attached: attached,
        nebel_nathan,
        sturm_nathan,
        sturm_deferred: deferred,
        abend_done: done,
        abend_summary: summary,
        unplaced,
    };
    b
}

/// HTML-Maskierung von minijinja zurücknehmen, soweit Adressen sie tragen (`/` als `&#x2f;`, `&` als `&amp;`):
/// Zusicherungen vergleichen dann mit den Adressen, die der Browser liest.
fn unescape(html: &str) -> String {
    html.replace("&#x2f;", "/").replace("&amp;", "&")
}

/// Wert eines Attributs im ersten Tag, das `marker` enthält.
fn attr(html: &str, marker: &str, name: &str) -> String {
    let at = html.find(marker).unwrap_or_else(|| panic!("{marker} fehlt: {html}"));
    let start = if html[at..].starts_with('<') { at } else { html[..at].rfind('<').unwrap() };
    let end = at + html[at..].find('>').unwrap();
    let tag = &html[start..end];
    let key = format!(" {name}=\"");
    let from = tag.find(&key).unwrap_or_else(|| panic!("{name} fehlt in {tag}")) + key.len();
    tag[from..from + tag[from..].find('"').unwrap()].to_owned()
}

/// Erste Zahl `data-findings-open` nach dem Eintrag mit `data-key="<key>"` in der Gliederung der Arbeitsseite.
fn tree_badge(html: &str, key: i64) -> Option<usize> {
    let at = html.find(&format!(" data-key=\"{key}\" data-level="))?;
    let rest = &html[at..];
    // Nur innerhalb des eigenen Eintrags (bis zum ersten Unterpunkt bzw. nächsten Eintrag).
    let end = rest[1..].find("<li ").map_or(rest.len(), |e| e + 1);
    let entry = &rest[..end];
    let from = entry.find("data-findings-open=\"")? + "data-findings-open=\"".len();
    entry[from..from + entry[from..].find('"')?].parse().ok()
}

fn row_of(matrix: &Value, key: i64) -> &Value {
    matrix["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["rows"].as_array().unwrap())
        .find(|r| r["key"] == key)
        .unwrap_or_else(|| panic!("keine Zeile für {key}: {matrix}"))
}

#[tokio::test]
async fn the_matrix_groups_twelve_chapters_into_three_parts_with_counts_per_status() {
    let b = open_book().await;
    let m = b.matrix(json!({})).await;
    let groups = m["groups"].as_array().unwrap();
    let parts: Vec<&str> =
        groups.iter().filter(|g| g["kind"] == "part").map(|g| g["title"].as_str().unwrap()).collect();
    assert_eq!(parts, ["Aufbruch", "Wanderung", "Ankunft"], "Teile als Gruppen in Reihenfolge der Gliederung: {m}");
    let chapters: usize = groups
        .iter()
        .filter(|g| g["kind"] == "part")
        .flat_map(|g| g["rows"].as_array().unwrap())
        .filter(|r| r["part"] == false)
        .count();
    assert_eq!(
        chapters, 12,
        "jedes der zwölf Kapitel hat eine Zeile, auch ohne Befunde (das Werk soll sich nicht verlieren)"
    );
    assert_eq!(
        groups.last().unwrap()["kind"],
        "unplaced",
        "der Befund ohne Abschnitt steht in einer eigenen Gruppe am Ende"
    );

    let personas: Vec<&str> = m["personas"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
    assert!(!personas.contains(&"@hoerer"), "ohne include_praise keine Spalte nur für Lob: {personas:?}");
    let col = |p: &str| personas.iter().position(|x| *x == p).unwrap_or_else(|| panic!("Spalte {p}: {personas:?}"));

    let morgen = row_of(&m, b.key("Morgen"));
    let lektor = &morgen["cells"][col("@lektor")];
    assert_eq!(
        (lektor["open"].as_u64(), lektor["rejected"].as_u64()),
        (Some(2), Some(1)),
        "Morgen/@lektor: zwei offen, einer verworfen; der angehängte zählt nicht: {lektor}"
    );
    assert_eq!(morgen["cells"][col("@nathan")]["open"], 1, "der Abschnitt Nebel zählt beim Kapitel Morgen mit");
    assert_eq!(morgen["cells"][col("@exeget")]["adopted"], 1, "übernommen zählt in seiner Spalte");
    assert_eq!(morgen["total"]["open"], 3, "Kapitel samt Abschnitt: 2 + 1 offen");

    let wanderung = groups.iter().find(|g| g["title"] == "Wanderung").unwrap();
    assert_eq!(wanderung["total"]["open"], 3, "Summe je Teil: Sturm mit drei offenen");
    assert_eq!(wanderung["total"]["deferred"], 1, "Summe je Teil: einer verschoben");
    assert_eq!(
        m["total"]["open"], 7,
        "gesamt: 3 Morgen + 3 Sturm + 1 ohne Abschnitt; der Sammelvorschlag am Abend zählt seit Runde 23a nicht"
    );
    assert_eq!(
        m["open_findings"], 6,
        "Abzeichen ohne den Befund ohne Abschnitt (Zählregel §1.5) und ohne den Sammelvorschlag (Runde 23a)"
    );

    // Ohne Filter ist „offen“ je Zeile genau die Summe der Abzeichen ihrer Punkte.
    for row in groups.iter().filter(|g| g["kind"] != "unplaced").flat_map(|g| g["rows"].as_array().unwrap()) {
        assert_eq!(row["total"]["open"], row["open_badge"], "Matrix und Abzeichen gleich für {}", row["title"]);
    }
    // Verlauf je Kapitel: Morgen hat eine Übernahme in Fassung 1.
    let history = m["history"].as_array().unwrap();
    let morgen_history = history.iter().find(|h| h["key"] == b.key("Morgen")).expect("Morgen im Verlauf");
    assert_eq!(morgen_history["revisions"][0]["rev"], 1, "eingeflossen in Fassung 1: {morgen_history}");
    assert_eq!(morgen_history["revisions"][0]["findings"], json!([b.ids.morgen_adopted]), "welcher Befund");
    assert_eq!(morgen_history["total"]["rejected"], 1, "verworfen im Verlauf gezählt");
}

#[tokio::test]
async fn the_page_shows_the_matrix_with_signs_and_numbers_without_javascript() {
    let b = open_book().await;
    let (status, html) = b.page("").await;
    assert_eq!(status, StatusCode::OK, "Seite antwortet: {html}");
    assert!(!html.contains("noch nicht umgesetzt"), "kein Stub mehr auf der Seite");
    assert_eq!(html.matches("scope=\"rowgroup\"").count(), 4, "drei Teile und „Ohne Abschnitt“ als Gruppenköpfe");
    for part in ["Aufbruch", "Wanderung", "Ankunft"] {
        assert!(html.contains(&format!("Summe {part}")), "Summenzeile des Teils {part}");
    }
    assert_eq!(html.matches("<tr class=\"fd-row").count(), 13, "12 Kapitel und die Zeile „ohne Abschnitt“");
    assert!(html.contains(">○2 ✗1</a>"), "Zelle Morgen/@lektor zeigt Zeichen und Zahl, nicht nur Farbe: {html}");
    assert!(html.contains("title=\"2 offen, 1 verworfen\""), "Worte im title für Bildschirmleser");
    assert!(
        html.contains("fd-many") && html.contains("viele offen"),
        "Sturm mit drei offenen ist hervorgehoben, auch als Text"
    );
    assert!(html.contains("method=\"get\"") && html.contains("name=\"older_than_days\""), "Filter als GET-Formular");
    // Übersicht: offen und alt, Sammelvorschlag mit Ursprüngen, Verlauf.
    assert!(
        html.contains(&format!("id=\"befund-{}\"", b.ids.sturm_nathan[0])),
        "der alte offene Befund in „älter als“"
    );
    assert!(
        !html.contains(&format!("id=\"befund-{}\"", b.ids.sturm_nathan[2])),
        "ein frischer offener Befund steht nicht in „älter als 14 Tage“"
    );
    assert!(html.contains("Sammelvorschlag von @lektor"), "Sammelvorschlag benannt");
    for origin in [b.ids.sturm_nathan[1], b.ids.morgen_lektor[1]] {
        assert!(
            html.contains(&format!("befunde?befund={origin}\"")),
            "der Sammelvorschlag verlinkt den Ursprung {origin}"
        );
    }
    assert!(html.contains("Verlauf je Kapitel") && html.contains("Fassung 1: "), "Verlauf nennt die Fassung");
    assert!(html.contains(&format!("[B{}]", b.ids.morgen_adopted)), "Verlauf nennt den übernommenen Befund");
    let (status, _) = b.server.get("/werke/999999/befunde").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn a_cell_link_shows_exactly_the_findings_of_that_cell_with_attached_ones_below() {
    let b = open_book().await;
    let (_, overview) = b.page("").await;
    let morgen = b.key("Morgen");
    let link = format!("/werke/{}/befunde?persona=%40lektor&key={morgen}#zelle", b.work);
    assert!(overview.contains(&format!("href=\"{link}\"")), "Zelle verlinkt ?key=&persona=: {overview}");

    let (status, html) = b.page(&format!("persona=%40lektor&key={morgen}")).await;
    assert_eq!(status, StatusCode::OK, "die Zelle ist eine eigene Seite (GET, ohne JavaScript)");
    let mut want = b.ids.morgen_lektor.clone();
    want.push(b.ids.morgen_rejected);
    for id in &want {
        assert!(html.contains(&format!("id=\"befund-{id}\"")), "Befund {id} der Zelle fehlt");
    }
    for other in [b.ids.morgen_adopted, b.ids.nebel_nathan, b.ids.sturm_nathan[1], b.ids.abend_summary] {
        assert!(!html.contains(&format!("id=\"befund-{other}\"")), "Befund {other} gehört nicht in die Zelle");
    }
    let original = html.find(&format!("id=\"befund-{}\"", b.ids.morgen_lektor[0])).unwrap();
    let attached = html.find(&format!("id=\"befund-{}\"", b.ids.morgen_attached)).expect("angehängter Befund sichtbar");
    assert!(attached > original, "der angehängte steht unter seinem Original");
    assert!(html.contains("1 angehängt (gleichen diesem Befund, zählen nicht als offen)"), "als angehängt benannt");
    assert!(html.contains("Befunde in <a"), "Überschrift der Zelle");

    // Zelle samt Abschnitt: Summe Morgen (alle Personas) enthält den Befund im Abschnitt Nebel.
    let (_, all) = b.page(&format!("key={morgen}")).await;
    assert!(all.contains(&format!("id=\"befund-{}\"", b.ids.nebel_nathan)), "Kapitel samt Unterpunkten");
}

#[tokio::test]
async fn filters_act_through_get_parameters_and_praise_waits_for_its_switch() {
    let b = open_book().await;
    let rejected = b.matrix(json!({ "status": "rejected" })).await;
    assert_eq!((rejected["total"]["rejected"].as_u64(), rejected["total"]["open"].as_u64()), (Some(1), Some(0)));
    let nathan = b.matrix(json!({ "persona": "@nathan" })).await;
    assert_eq!(nathan["personas"], json!(["@nathan"]), "Persona-Filter: eine Spalte");
    assert_eq!(nathan["total"]["open"], 4, "Nebel 1 + Sturm 3");
    let critical = b.matrix(json!({ "kind": ["critical"] })).await;
    assert_eq!(critical["total"]["total"], 3, "nur die kritischen von @nathan");
    let future = b.matrix(json!({ "since": "2999-01-01" })).await;
    assert_eq!(future["total"]["total"], 0, "Zeitraum ab 2999: nichts");
    let old = b.matrix(json!({ "older_than_days": 30 })).await;
    assert_eq!(old["total"]["total"], 1, "älter als 30 Tage: nur der zurückdatierte Befund");
    let bad = b.server.rpc("finding.matrix", json!({ "work": b.work, "status": "offen" })).await;
    assert_eq!(bad["error"]["code"], -32602, "deutsches Wort statt Datenbankwort → ungültig: {bad}");

    // Dieselben Filter als GET-Parameter der Seite.
    let (_, html) = b.page("status=rejected").await;
    assert!(html.contains(">✗1</a>"), "nur verworfen gezählt");
    assert!(!html.contains(">○2 ✗1</a>"), "offene fallen mit dem Statusfilter weg");
    assert!(html.contains("value=\"rejected\" checked"), "Filter bleibt im Formular angehakt");
    let (_, empty) = b.page("since=2999-01-01").await;
    assert!(empty.contains("Keine Befunde für diesen Filter"), "ehrlich leer");

    // Lob erst mit Schalter.
    let (_, plain) = b.page("").await;
    assert!(!plain.contains("<th scope=\"col\">@hoerer</th>"), "Lob standardmäßig ausgeblendet");
    assert!(plain.contains("Lob einblenden"), "Schalter als Link (ohne JavaScript)");
    let (_, praise) = b.page("include_praise=1").await;
    assert!(praise.contains("<th scope=\"col\">@hoerer</th>"), "mit Schalter eine Spalte für das Lob");
    assert!(praise.contains(">+1</a>"), "Lob mit eigenem Zeichen gezählt");
    let with_praise = b.matrix(json!({ "include_praise": true })).await;
    assert_eq!(
        with_praise["total"]["open"], 7,
        "Lob zählt nie als offen (dieselben 7 wie ohne Schalter; der Sammelvorschlag zählt seit Runde 23a nicht)"
    );
    let (_, cell) = b.page(&format!("include_praise=1&key={}", b.key("Morgen"))).await;
    assert!(cell.contains(&format!("id=\"befund-{}\"", b.ids.morgen_praise)), "Lob in der Zelle mit Schalter");
}

#[tokio::test]
async fn a_single_finding_shows_its_target_history_and_origin_links() {
    let b = open_book().await;
    let (_, deferred) = b.page(&format!("befund={}", b.ids.sturm_deferred)).await;
    assert!(deferred.contains("verschoben nach 9 Tal"), "Ziel des verschobenen Befunds in Worten: {deferred}");
    assert!(deferred.contains("Noch keine Änderung"), "ohne Verlaufseinträge ehrlich benannt");
    let (_, unplaced) = b.page(&format!("befund={}", b.ids.unplaced)).await;
    assert!(unplaced.contains("ohne Abschnitt"), "Befund aus einem Faden ohne Abschnitt");
    let (_, done) = b.page(&format!("befund={}", b.ids.abend_done)).await;
    assert!(done.contains("erledigt ohne Änderung"), "Status in Worten");
    let (_, proposed) = b.page(&format!("befund={}", b.ids.morgen_lektor[0])).await;
    assert!(proposed.contains("Vorschlag, offen"), "der offene Vorschlag steht im Verlauf");
    assert!(proposed.contains("value=\"finding.decide\""), "und lässt sich auf der Seite entscheiden");
    let (_, attached) = b.page(&format!("befund={}", b.ids.morgen_attached)).await;
    assert!(
        attached.contains(&format!(
            "Angehängt an <a href=\"/werke/{}/befunde?befund={}\">",
            b.work, b.ids.morgen_lektor[0]
        )),
        "angehängter Befund verweist auf sein Original"
    );
    let (status, missing) = b.page("befund=999999").await;
    assert_eq!(status, StatusCode::OK, "unbekannter Befund: Seite mit Hinweis");
    assert!(missing.contains("Befund 999999 gibt es in diesem Werk nicht."), "ehrlicher Hinweis");
}

#[tokio::test]
async fn badges_in_outline_editor_and_menu_agree_with_the_matrix() {
    let b = open_book().await;
    let counts = b.server.state.store.finding_open_counts(b.work).await.unwrap();
    let total: usize = counts.values().sum();
    let m = b.matrix(json!({})).await;
    assert_eq!(m["open_findings"], total, "Matrix und finding_open_counts zählen gleich");

    let morgen = b.key("Morgen");
    let (status, html) = b.workspace(morgen).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite des Abschnitts lädt");
    assert_eq!(tree_badge(&html, morgen), Some(3), "Kapitel Morgen samt Nebel: 2 + 1");
    assert_eq!(tree_badge(&html, b.key("Nebel")), Some(1), "Abschnitt Nebel für sich");
    assert_eq!(tree_badge(&html, b.key("Aufbruch")), Some(3), "Teil Aufbruch summiert seine Kapitel");
    assert_eq!(tree_badge(&html, b.key("Wanderung")), Some(3), "Teil Wanderung");
    assert_eq!(
        tree_badge(&html, b.key("Ankunft")),
        None,
        "Teil Ankunft: offen ist nur der Sammelvorschlag, und der zählt seit Runde 23a nicht in „k offen“"
    );
    assert_eq!(tree_badge(&html, b.key("Brücke")), None, "ohne offene Befunde kein Abzeichen");
    assert_eq!(row_of(&m, morgen)["open_badge"], 3, "dieselbe Zahl in finding.matrix");
    assert_eq!(
        attr(&html, "editor-findings", "data-findings-open"),
        "3",
        "Editorkopf zeigt die Summe samt Abschnitten"
    );
    assert!(html.contains(">3 offen</a>"), "Editorkopf als Text „3 offen“");
    // Runde 33 (Menüband): Das Abzeichen stand am Menüknopf „Werk ▾“ und steht jetzt am Reiter Prüfen (dort liegen
    // die Befunde) – dieselbe Zahl, derselbe Haken.
    assert!(
        html.contains(&format!(">Prüfen <span class=\"badge warning\" data-badge=\"open_findings\" title=\"offene Befunde im Werk\">{total} offen</span></a>")),
        "Abzeichen am Reiter Prüfen mit der Summe, vom Server gezeichnet"
    );
    assert!(
        html.contains(&format!(
            "Befunde <span class=\"badge warning\" data-badge=\"open_findings\">{total}</span></a>"
        )),
        "und die Zahl am Eintrag „Befunde“"
    );

    let badges = result(&b.server.rpc("menu.badges", json!({ "work": b.work })).await).clone();
    assert_eq!(badges["open_findings"], total, "menu.badges liefert open_findings: {badges}");

    let (status, outline) = b.server.get(&format!("/gliederung?work={}", b.work)).await;
    let outline = unescape(&outline);
    assert_eq!(status, StatusCode::OK, "/gliederung lädt");
    let marker = format!("befunde?key={morgen}#zelle\" data-findings-open=\"");
    assert_eq!(attr(&outline, &marker, "data-findings-open"), "3", "auch /gliederung zeigt „3 offen“ an Morgen");
    let sturm = format!("befunde?key={}#zelle\" data-findings-open=\"", b.key("Sturm"));
    assert_eq!(attr(&outline, &sturm, "data-findings-open"), "3", "/gliederung an Sturm");
}

#[tokio::test]
async fn the_findings_accordion_offers_status_forms_and_open_proposals() {
    let b = open_book().await;
    let morgen = b.key("Morgen");
    let (_, html) = b.workspace(morgen).await;
    let start = html.find("data-section=\"befunde\"").expect("Akkordeon „Befunde“ am Abschnitt");
    let panel = &html[start..start + html[start..].find("</details>\n{#").unwrap_or(html.len() - start)];
    assert!(panel.contains(">2 offen</span>"), "Kopf: offene Befunde des Abschnitts selbst");
    assert!(panel.contains("action=\"/werke/befunde/act\""), "Formulare ohne JavaScript");
    assert!(
        panel.contains("<textarea name=\"reason\" rows=\"2\" maxlength=\"2000\" required>"),
        "Begründung Pflicht bei verworfen"
    );
    assert!(panel.contains("<select name=\"rev\" required>"), "Fassungsauswahl bei übernommen");
    assert!(panel.contains("Fassung 1 (aktuell)"), "die Fassungen des Abschnitts zur Auswahl");
    assert!(panel.contains("name=\"target_key\"") && panel.contains("name=\"target_idea\""), "Ziel bei verschoben");
    assert!(panel.contains("value=\"finding.decide\""), "offene Vorschläge mit Entscheidung");
    assert!(panel.contains("@lektor schlägt vor") && panel.contains("übernommen in Fassung 1"), "Vorschlag in Worten");
    assert!(
        panel.contains("name=\"confirm\" value=\"1\"") && panel.contains("name=\"confirm\" value=\"0\""),
        "Bestätigen und Verwerfen"
    );
    assert!(
        panel.contains(&format!("name=\"back\" value=\"/?work={}&key={morgen}", b.work)),
        "zurück auf die Arbeitsseite"
    );
    assert!(panel.contains("1 Lob (ausgeblendet)"), "Lob eingeklappt");
    assert!(panel.contains("entschieden"), "entschiedene eingeklappt");
}

#[tokio::test]
async fn posting_a_status_form_goes_through_findings_rpc_and_keeps_the_draft_on_error() {
    let b = open_book().await;
    let id = b.ids.morgen_lektor[1];
    let back = format!("/?work={}&key={}", b.work, b.key("Morgen"));
    let form = format!(
        "method=finding.set_status&work={}&id={id}&status=rejected&reason=Bleibt+so%2C+mit+Absicht&back={}",
        b.work,
        back.replace('/', "%2F").replace('?', "%3F").replace('=', "%3D").replace('&', "%26")
    );
    let request = Request::post("/werke/befunde/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.clone()))
        .unwrap();
    let (status, headers, body) = b.server.send(request).await;
    // Seit dem Zusammenführen mit befunde (Integrator R28): Status gesetzt, 303 zurück auf die Arbeitsseite.
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular erfolgreich: {body}");
    assert_eq!(headers.get(header::LOCATION).unwrap(), back.as_str(), "303 zurück auf back");
    let finding = b.server.state.store.finding_get(b.work, id).await.unwrap();
    assert_eq!(finding.status.as_db(), "rejected", "Status gesetzt");
    assert_eq!(finding.reason, "Bleibt so, mit Absicht", "Begründung gespeichert");

    // Fehlerweg: verschoben mit zwei Zielen → 200 mit Hinweis statt Umleitung (Vertrag 4.5).
    let other = b.ids.morgen_lektor[0];
    let bad = format!(
        "method=finding.set_status&work={}&id={other}&status=deferred&target_key={}&target_idea=1",
        b.work,
        b.key("Morgen")
    );
    let request = Request::post("/werke/befunde/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(bad))
        .unwrap();
    let (status, _, body) = b.server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Fehler als Seite mit Hinweis: {body}");
    assert!(body.contains("role=\"alert\""), "Hinweis als Alarm: {body}");

    let (status, _) = b.server.post_form("/werke/befunde/act", &format!("method=finding.delete&work={}", b.work)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannte Aktion");
    let (status, _) = b.server.post_form("/werke/befunde/act", "method=finding.decide&work=999999&entry=1").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk");
    let foreign = Request::post("/werke/befunde/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("sec-fetch-site", "cross-site")
        .body(Body::from(form))
        .unwrap();
    let (status, _, _) = b.server.send(foreign).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "Formular von einer fremden Seite abgelehnt (guard::same_origin)");
    let still = b.server.state.store.finding_get(b.work, b.ids.morgen_lektor[0]).await.unwrap();
    assert_eq!(still.status.as_db(), "open", "nichts anderes verändert");
}

#[tokio::test]
async fn the_revision_tree_names_adopted_findings_and_marks_open_proposals() {
    let b = open_book().await;
    let morgen = b.key("Morgen");
    let adopted = b.server.state.store.findings_adopted_in(b.work, morgen).await.unwrap();
    assert_eq!(adopted, vec![(1, vec![b.ids.morgen_adopted])], "Store: Fassung 1 setzt den Befund um");
    let (_, html) = b.workspace(morgen).await;
    assert_eq!(
        attr(&html, "data-findings-adopted=", "data-findings-adopted"),
        b.ids.morgen_adopted.to_string(),
        "Fassungsbaum: dieselbe Zuordnung"
    );
    assert!(
        html.contains(&format!(
            "✓ setzt Befund <a href=\"/werke/{}/befunde?befund={}\">[B{}]</a> um",
            b.work, b.ids.morgen_adopted, b.ids.morgen_adopted
        )),
        "Text mit Link"
    );
    assert_eq!(
        attr(&html, "data-findings-proposed=", "data-findings-proposed"),
        b.ids.morgen_lektor[0].to_string(),
        "offener Vorschlag „übernommen in Fassung 1“ gekennzeichnet"
    );
    assert!(html.contains("(offen, der Mensch bestätigt)"), "als offen benannt");
}

#[tokio::test]
async fn the_idea_map_findings_layer_highlights_chapters_with_many_open_findings() {
    let b = open_book().await;
    let sturm = b.key("Sturm");
    let (_, plain) = b.server.get(&format!("/werke/{}/karte", b.work)).await;
    let plain = unescape(&plain);
    assert!(!plain.contains("mm-many-findings"), "ohne Ebene keine Hervorhebung");
    assert!(plain.contains("Ebene Befunde einblenden"), "Schalter als Link (ohne JavaScript)");
    let (status, html) = b.server.get(&format!("/werke/{}/karte?befunde=1", b.work)).await;
    let html = unescape(&html);
    assert_eq!(status, StatusCode::OK, "die Gedankenkarte mit Ebene Befunde lädt");
    let marker = format!("data-node=\"n:{sturm}\"");
    assert_eq!(attr(&html, &marker, "data-open-findings"), "3", "Sturm trägt seine offenen Befunde im Bild");
    // Runde 34: Die Gedankenkarte ist die Mindmap (Klassen `mm-…`, Zahl am Ast des Kapitels).
    assert!(attr(&html, &marker, "class").contains("mm-many-findings"), "Sturm hervorgehoben (≥ 3 offen)");
    assert!(html.contains(">3 offen</text>"), "Zahl als Text im Bild, nicht nur Farbe");
    assert!(html.contains("Ebene Befunde ausblenden"), "Schalter zurück");
    let (_, outline) = b.server.get(&format!("/werke/{}/karte/gliederung?befunde=1", b.work)).await;
    assert!(unescape(&outline).contains("3 offene Befunde"), "die Gliederungsansicht nennt die Zahl als Text");

    let map =
        result(&b.server.rpc("idea.map", json!({ "work": b.work, "near": false, "findings": true })).await).clone();
    let node = |key: i64| map["nodes"].as_array().unwrap().iter().find(|n| n["key"] == key).unwrap().clone();
    assert_eq!(node(sturm)["open_findings"], 3, "RPC: offene Befunde je Knoten");
    assert_eq!(node(b.key("Wanderung"))["open_findings"], 3, "Teil zählt seine Kapitel");
    assert_eq!(node(b.key("Morgen"))["open_findings"], 3, "Kapitel zählt seine zugeklappten Abschnitte");
    assert_eq!(map["nodes"][0]["open_findings"], 6, "das Werk zählt alles mit Abschnitt, ohne den Sammelvorschlag");
    let off = result(&b.server.rpc("idea.map", json!({ "work": b.work, "near": false })).await).clone();
    assert_eq!(off["nodes"][0]["open_findings"], 0, "ohne Ebene keine Zahl (kein Aufwand)");
}

// ---- MCP ---------------------------------------------------------------------------------------

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

async fn initialize(server: &common::Server, name: &str) -> String {
    let (headers, answer) = mcp(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": name, "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    mcp(
        server,
        Some(session),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": name, "arguments": arguments } }),
    )
    .await
    .1
}

#[tokio::test]
async fn mcp_reads_findings_and_only_proposes_status_changes() {
    let b = open_book().await;
    let s = initialize(&b.server, "befund-probe").await;
    let (_, listed) = mcp(&b.server, Some(&s), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    for name in ["findings", "finding_propose_status"] {
        let t = listed["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap();
        let text = t["description"].as_str().unwrap();
        assert!(!text.contains("noch nicht umgesetzt") && !text.contains("Stub"), "{name} ist kein Stub: {text}");
    }

    let morgen = b.key("Morgen");
    let answer = tool(&b.server, &s, "findings", json!({ "work": b.work, "key": morgen })).await;
    assert_eq!(answer["result"]["isError"], false, "findings gelingt: {answer}");
    let list = &answer["result"]["structuredContent"];
    let mut ids: Vec<i64> = list["findings"].as_array().unwrap().iter().map(|f| f["id"].as_i64().unwrap()).collect();
    ids.sort_unstable();
    let mut want = vec![b.ids.morgen_lektor[0], b.ids.morgen_lektor[1], b.ids.morgen_rejected, b.ids.morgen_adopted];
    want.sort_unstable();
    assert_eq!(ids, want, "nur der Abschnitt Morgen, ohne Lob und ohne angehängte");
    let first = &list["findings"][0];
    for field in ["kind_label", "status_label", "url", "persona", "body"] {
        assert!(!first[field].is_null(), "finding_json mit {field}: {first}");
    }
    let rejected = tool(&b.server, &s, "findings", json!({ "work": b.work, "status": "rejected" })).await;
    assert_eq!(rejected["result"]["structuredContent"]["total"], 1, "Statusfilter");
    let praise =
        tool(&b.server, &s, "findings", json!({ "work": b.work, "key": morgen, "include_praise": true })).await;
    assert_eq!(praise["result"]["structuredContent"]["total"], 5, "mit Lob ein Befund mehr");
    let bad = tool(&b.server, &s, "findings", json!({ "work": b.work, "kind": "lob" })).await;
    assert_eq!(bad["result"]["isError"], true, "unbekannte Art → isError: {bad}");

    let id = b.ids.morgen_lektor[1];
    let human_only =
        tool(&b.server, &s, "finding_propose_status", json!({ "work": b.work, "id": id, "status": "rejected" })).await;
    assert_eq!(human_only["result"]["isError"], true, "verwerfen nur der Mensch: {human_only}");
    assert!(human_only.to_string().contains("Nur der Mensch"), "ehrliche Meldung: {human_only}");
    let no_rev =
        tool(&b.server, &s, "finding_propose_status", json!({ "work": b.work, "id": id, "status": "adopted" })).await;
    assert_eq!(no_rev["result"]["isError"], true, "übernommen braucht rev: {no_rev}");
    let two = tool(
        &b.server,
        &s,
        "finding_propose_status",
        json!({ "work": b.work, "id": id, "status": "deferred", "target_key": morgen, "target_idea": 1 }),
    )
    .await;
    assert_eq!(two["result"]["isError"], true, "verschoben braucht genau ein Ziel: {two}");

    for (args, origin) in [
        (
            json!({ "work": b.work, "id": id, "status": "adopted", "rev": 1, "note": "steht so da" }),
            "@mcp:befund-probe",
        ),
        (json!({ "work": b.work, "id": id, "status": "done", "persona": "@nathan" }), "@nathan"),
    ] {
        let answer = tool(&b.server, &s, "finding_propose_status", args).await;
        let text = answer.to_string();
        // Seit dem Zusammenführen mit befunde: Vorschlag angenommen, Status unverändert.
        assert_eq!(answer["result"]["isError"], false, "Vorschlag angenommen: {text}");
        assert!(text.contains(origin), "Herkunft {origin} im Vorschlag: {text}");
        let finding = b.server.state.store.finding_get(b.work, id).await.unwrap();
        assert_eq!(finding.status.as_db(), "open", "ein Vorschlag ändert den Status nicht");
    }
}

#[tokio::test]
async fn the_widget_script_is_served_with_filter_and_keyboard_handling() {
    let b = open_book().await;
    let (status, js) = b.server.get("/static/js/findings.js").await;
    assert_eq!(status, StatusCode::OK, "Widget ausgeliefert");
    assert!(js.contains("register(\"findings\""), "Widget „findings“ registriert");
    for part in ["data-findings-main", "ArrowDown", "pushState", "baruch:navigate"] {
        assert!(js.contains(part), "Widget enthält {part} (Tausch ohne Neuladen, Tastatur, Verlauf, Neuzeichnen)");
    }
    assert!(js.contains("data-finding-form"), "Statusformulare über RPC");
    let (_, html) = b.page("").await;
    assert!(html.contains("data-widget=\"findings\""), "Seite hängt das Widget ein");
    assert!(
        html.contains("data-findings-filter") && html.contains("data-findings-matrix"),
        "Haken für Filter und Tastatur"
    );
}
