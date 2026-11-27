// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 35 „Konten und Rollen“, Besitz rollen: Rollen je Werk, Werk-Bindung, Herkunft `@lektorat:<slug>`.
//!
//! Wer die Anfrage stellt, legen die Tests über `common::with_who` fest (keine Sitzung, kein Passwort); Konten
//! entstehen per SQL mit dem festen Hash-Text `$argon2id$test`. Sobald es Konten gibt, trägt JEDE Anfrage ein
//! ausdrückliches `Who` – ohne wäre sie nach dem Zusammenführen mit „konten“ unangemeldet. Zwei frei erfundene
//! Werke: das Beispielwerk der neuen Instanz (kleinste id, also das Vorgabewerk – für den Lektor fremd) und ein
//! importiertes (freigegeben). Kein Modell, kein Netz.

use crate::common;

use std::collections::BTreeMap;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::auth::{AccountKind, Identity, Who, lektor_handle};
use baruch_server::roles::{self, LEKTOR_ROUTES, LEKTOR_RPC, Role, WorkFrom};
use serde_json::{Value, json};

pub(crate) const FORBIDDEN: i64 = -32003;
const REFUSED: i64 = -32000;
const INVALID_PARAMS: i64 = -32602;

const BOOK: &str = "#set document(title: \"Der Leuchtturm am Sund\", author: \"Marit Holm\")\n\n\
= Nebelhorn\n\nDas Horn rief dreimal über den Sund, und die Möwen stoben vom Geländer.\n\n\
= Lampenöl\n\nIm Keller roch es nach Öl und kaltem Stein.\n";

/// Bühne: Server, das freigegebene und das fremde Werk, je der feste Schlüssel des ersten Abschnitts, die Konten.
pub(crate) struct Stage {
    pub server: common::Server,
    pub granted: i64,
    pub granted_slug: String,
    pub granted_key: i64,
    pub foreign: i64,
    pub foreign_key: i64,
    pub foreign_title: String,
    /// Autor-Konto „Tilda“.
    pub author: Who,
    /// Lektor-Konto „Erika“ mit Freigabe für `granted`.
    pub erika: Who,
    pub erika_id: i64,
    /// Lektor-Konto „Gustav“ ohne Freigabe.
    pub gustav: Who,
    pub gustav_id: i64,
    pub author_id: i64,
}

pub(crate) fn account(id: i64, name: &str, kind: AccountKind) -> Who {
    Who::Account(Identity {
        id,
        name: name.to_owned(),
        display: format!("{name} Beispiel"),
        kind,
        handle: lektor_handle(name),
    })
}

/// Konto per SQL (Fremdschlüssel für `work_member`); der Hash ist ein fester Text, angemeldet wird nie.
pub(crate) async fn insert_account(server: &common::Server, name: &str, kind: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO account (name, name_key, slug, display, kind, password) \
         VALUES (?, lower(?), lower(?), ?, ?, '$argon2id$test') RETURNING id",
    )
    .bind(name)
    .bind(name)
    .bind(name)
    .bind(format!("{name} Beispiel"))
    .bind(kind)
    .fetch_one(server.state.store.pool())
    .await
    .unwrap()
}

pub(crate) async fn stage() -> Stage {
    let server = common::open().await;
    // Noch gibt es keine Konten: Die Vorbereitung läuft offen wie vor Runde 35.
    let works = server.rpc("work.list", json!({})).await;
    let sample = &works["result"]["works"][0];
    let foreign = sample["id"].as_i64().expect("das Beispielwerk der neuen Instanz");
    let foreign_title = sample["title"].as_str().unwrap().to_owned();
    let report = server.import(BOOK, "leuchtturm.typ").await;
    let granted = report.work;
    assert!(foreign < granted, "Vorbedingung: Das fremde Werk hat die kleinste id und ist damit das Vorgabewerk");
    let key_of = |answer: Value| answer["result"]["key"].as_i64().expect("fester Schlüssel des ersten Abschnitts");
    let granted_key = key_of(server.rpc("node.get", json!({ "work": granted, "node": 0 })).await);
    let foreign_key = key_of(server.rpc("node.get", json!({ "work": foreign, "node": 0 })).await);
    let author_id = insert_account(&server, "Tilda", "author").await;
    let erika_id = insert_account(&server, "Erika", "lektor").await;
    let gustav_id = insert_account(&server, "Gustav", "lektor").await;
    server.state.store.member_grant(granted, erika_id, "lektor").await.expect("Freigabe für Erika");
    Stage {
        server,
        granted,
        granted_slug: report.slug,
        granted_key,
        foreign,
        foreign_key,
        foreign_title,
        author: account(author_id, "Tilda", AccountKind::Author),
        erika: account(erika_id, "Erika", AccountKind::Lektor),
        erika_id,
        gustav: account(gustav_id, "Gustav", AccountKind::Lektor),
        gustav_id,
        author_id,
    }
}

pub(crate) async fn send_as(
    server: &common::Server,
    who: &Who,
    request: Request<Body>,
) -> (StatusCode, HeaderMap, String) {
    server.send(common::with_who(request, who.clone())).await
}

pub(crate) async fn get_as(server: &common::Server, who: &Who, uri: &str) -> (StatusCode, HeaderMap, String) {
    send_as(server, who, Request::get(uri).body(Body::empty()).unwrap()).await
}

pub(crate) async fn form_as(
    server: &common::Server,
    who: &Who,
    uri: &str,
    form: &str,
) -> (StatusCode, HeaderMap, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap();
    send_as(server, who, request).await
}

/// Rohe JSON-RPC-Anfrage (einzeln oder Batch) als `who`.
pub(crate) async fn rpc_raw(server: &common::Server, who: &Who, body: Value) -> Value {
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, _, text) = send_as(server, who, request).await;
    assert_eq!(status, StatusCode::OK, "/rpc antwortet auch bei Fehlern mit 200 und JSON: {text}");
    serde_json::from_str(&text).unwrap()
}

pub(crate) async fn rpc_as(server: &common::Server, who: &Who, method: &str, params: Value) -> Value {
    rpc_raw(server, who, json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })).await
}

pub(crate) fn code(answer: &Value) -> Option<i64> {
    answer["error"]["code"].as_i64()
}

fn message(answer: &Value) -> &str {
    answer["error"]["message"].as_str().unwrap_or("")
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

pub(crate) fn location(headers: &HeaderMap) -> &str {
    headers.get(header::LOCATION).map_or("", |v| v.to_str().unwrap())
}

pub(crate) fn denied_by_role(headers: &HeaderMap) -> bool {
    headers.get("x-baruch-abgelehnt").is_some_and(|v| v == "rolle")
}

/// Zeilenzahl jeder Tabelle (auch der Volltextindizes): der ganze Stand der Datenbank in Zahlen.
pub(crate) async fn row_counts(server: &common::Server) -> BTreeMap<String, i64> {
    let pool = server.state.store.pool();
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .fetch_all(pool)
        .await
        .unwrap();
    let mut counts = BTreeMap::new();
    for table in tables {
        let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM \"{table}\"")))
            .fetch_one(pool)
            .await
            .unwrap();
        counts.insert(table, n);
    }
    counts
}

/// HEAD des Spiegels je Werk.
pub(crate) async fn heads(server: &common::Server, works: &[i64]) -> Vec<Option<String>> {
    let mut out = Vec::new();
    for work in works {
        out.push(server.state.mirror(*work).await.unwrap().head().unwrap());
    }
    out
}

fn enc(text: &str) -> String {
    text.bytes()
        .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

async fn current_body(server: &common::Server, work: i64, node: usize) -> String {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let n = &loaded.nodes[node];
    n.revision(n.current_number().unwrap()).unwrap().body.clone()
}

// ---- Rollen und Sichtbarkeit ----------------------------------------------------------------------------------

#[tokio::test]
async fn the_role_follows_the_account_kind_and_the_grant() {
    let s = stage().await;
    let state = &s.server.state;
    for who in [Who::Open, Who::LocalAuthor, s.author.clone()] {
        for work in [s.granted, s.foreign] {
            assert_eq!(
                roles::role_in(state, &who, work).await.unwrap(),
                Some(Role::Author),
                "{who:?} ist Autor in jedem Werk (ohne Zeile in work_member)"
            );
        }
    }
    assert_eq!(roles::role_in(state, &s.erika, s.granted).await.unwrap(), Some(Role::Lektor), "freigegeben: Lektor");
    assert_eq!(roles::role_in(state, &s.erika, s.foreign).await.unwrap(), None, "nicht freigegeben: keine Rolle");
    assert_eq!(roles::role_in(state, &s.gustav, s.granted).await.unwrap(), None, "Lektor-Konto ohne Freigabe");
    assert_eq!(roles::role_in(state, &Who::Anonymous, s.granted).await.unwrap(), None, "ohne Anmeldung keine Rolle");
    let all: Vec<i64> = roles::visible_works(state, &s.author).await.unwrap().iter().map(|w| w.id).collect();
    assert_eq!(all, [s.foreign, s.granted], "der Autor sieht alle Werke");
    assert!(roles::visible_works(state, &Who::Anonymous).await.unwrap().is_empty(), "ohne Anmeldung kein Werk");
}

#[tokio::test]
async fn a_lektor_sees_only_granted_works_and_revoke_and_lock_act_on_the_next_request() {
    let s = stage().await;
    let server = &s.server;
    let titles = |answer: &Value| -> Vec<String> {
        result(answer)["works"].as_array().unwrap().iter().map(|w| w["title"].as_str().unwrap().to_owned()).collect()
    };
    let listed = rpc_as(server, &s.erika, "work.list", json!({})).await;
    assert_eq!(titles(&listed), ["Der Leuchtturm am Sund"], "work.list zeigt Erika nur das freigegebene Werk");
    let (status, _, page) = get_as(server, &s.erika, "/werke").await;
    assert_eq!(status, StatusCode::OK, "die Werkliste ist für Lektoren erlaubt");
    assert!(page.contains("Der Leuchtturm am Sund"), "die Werkliste nennt das freigegebene Werk");
    assert!(!page.contains(&s.foreign_title), "die Werkliste nennt kein fremdes Werk");
    let (status, _, work_page) = get_as(server, &s.erika, &format!("/?work={}", s.granted)).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite des freigegebenen Werks");
    assert!(
        !work_page.contains(&s.foreign_title),
        "auch die Werkwahl im Kopf der Arbeitsseite nennt kein fremdes Werk"
    );
    for page in ["/ueber".to_owned(), format!("/einstellungen/darstellung?work={}", s.granted)] {
        let (status, headers, body) = get_as(server, &s.erika, &page).await;
        assert_eq!(status, StatusCode::OK, "{page} ist für Lektoren erlaubt");
        assert!(!denied_by_role(&headers), "{page}: keine Ablehnung der Rolle");
        assert!(!body.contains(&s.foreign_title), "{page} nennt kein fremdes Werk (Werkwahl im Menüband)");
    }
    // Runde 36: Die Einstellungsseite braucht kein work mehr (Werk-Bindung gelockert) – sie zeigt ohne work das
    // freigegebene Werk, nie das Vorgabewerk der Instanz (vorher: Umleitung auf `?work=`; siehe `round36_lektor`).
    let (status, _, page) = get_as(server, &s.erika, "/einstellungen/darstellung").await;
    assert_eq!(status, StatusCode::OK, "Darstellung ohne work ist erreichbar");
    assert!(
        !page.contains(&s.foreign_title) && page.contains(&format!("work={}", s.granted)),
        "Darstellung ohne work: das freigegebene Werk, nie das Vorgabewerk"
    );
    let (_, _, author_page) = get_as(server, &s.author, "/werke").await;
    assert!(
        author_page.contains(&s.foreign_title) && author_page.contains("Der Leuchtturm am Sund"),
        "Gegenprobe: der Autor sieht beide Werke"
    );

    // Ohne Freigabe: leere Liste, und die Startseite führt auf die (leere) Werkliste.
    let none = rpc_as(server, &s.gustav, "work.list", json!({})).await;
    assert!(titles(&none).is_empty(), "ohne Freigabe ist work.list leer");
    let (status, headers, _) = get_as(server, &s.gustav, "/").await;
    assert_eq!((status, location(&headers)), (StatusCode::SEE_OTHER, "/werke"), "ohne Freigabe geht es zur Werkliste");
    let (status, _, empty) = get_as(server, &s.gustav, "/werke").await;
    assert_eq!(status, StatusCode::OK, "die leere Werkliste ist eine Seite, kein Fehler");
    assert!(
        !empty.contains("Der Leuchtturm am Sund") && !empty.contains(&s.foreign_title),
        "ohne Freigabe nennt die Werkliste kein Werk"
    );
    let no_work = rpc_as(server, &s.gustav, "work.get", json!({})).await;
    assert_eq!(
        code(&no_work),
        Some(FORBIDDEN),
        "ohne Freigabe gibt es kein Werk zu lesen – auch nicht das Vorgabewerk"
    );

    // Entzug wirkt bei der nächsten Anfrage.
    assert_eq!(code(&rpc_as(server, &s.erika, "work.get", json!({ "work": s.granted })).await), None, "vorher lesbar");
    assert!(server.state.store.member_revoke(s.granted, s.erika_id).await.unwrap(), "Freigabe entzogen");
    let after = rpc_as(server, &s.erika, "work.get", json!({ "work": s.granted })).await;
    assert_eq!(code(&after), Some(FORBIDDEN), "nach dem Entzug ist das Werk bei der nächsten Anfrage gesperrt");
    let (status, headers, _) = get_as(server, &s.erika, &format!("/?work={}", s.granted)).await;
    assert!(status == StatusCode::FORBIDDEN && denied_by_role(&headers), "auch die Seite ist nach dem Entzug gesperrt");
    assert!(titles(&rpc_as(server, &s.erika, "work.list", json!({})).await).is_empty(), "und work.list ist leer");

    // Sperre des Kontos wirkt bei der nächsten Anfrage, auch wenn noch jemand mit der alten Identität käme.
    server.state.store.member_grant(s.granted, s.erika_id, "lektor").await.unwrap();
    assert_eq!(code(&rpc_as(server, &s.erika, "work.get", json!({ "work": s.granted })).await), None, "wieder lesbar");
    sqlx::query("UPDATE account SET active = 0 WHERE id = ?")
        .bind(s.erika_id)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let locked = rpc_as(server, &s.erika, "work.get", json!({ "work": s.granted })).await;
    assert_eq!(code(&locked), Some(FORBIDDEN), "ein gesperrtes Konto hat keine wirksame Freigabe mehr");
    let (status, headers, _) = get_as(server, &s.erika, &format!("/werke/{}/befunde", s.granted)).await;
    assert!(status == StatusCode::FORBIDDEN && denied_by_role(&headers), "auch Seiten sind nach der Sperre zu");
}

// ---- Werk-Bindung ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn every_allowed_route_refuses_a_work_that_is_not_granted() {
    let s = stage().await;
    let server = &s.server;
    let fill = |pattern: &str, work: i64| pattern.replace("{id}", &work.to_string()).replace("{name}", "bild");
    for (verb, pattern, from) in LEKTOR_ROUTES.iter().filter(|(verb, _, _)| *verb == "GET") {
        match from {
            WorkFrom::Path => {
                let (status, headers, body) = get_as(server, &s.erika, &fill(pattern, s.foreign)).await;
                assert!(
                    status == StatusCode::FORBIDDEN && denied_by_role(&headers),
                    "{verb} {pattern}: ein nicht freigegebenes Werk im Pfad ist gesperrt (Status {status})"
                );
                assert_eq!(
                    body,
                    format!("Abgelehnt: unbekanntes Werk {}", s.foreign),
                    "{pattern}: dieselbe Meldung wie für ein Werk, das es nicht gibt"
                );
                let (status, _, unknown) = get_as(server, &s.erika, &fill(pattern, 9999)).await;
                assert_eq!(
                    (status, unknown.as_str()),
                    (StatusCode::FORBIDDEN, "Abgelehnt: unbekanntes Werk 9999"),
                    "{pattern}: Gegenprobe mit einem Werk, das es nicht gibt – nicht zu unterscheiden"
                );
                let (status, headers, _) = get_as(server, &s.erika, &fill(pattern, s.granted)).await;
                assert!(
                    !denied_by_role(&headers),
                    "{pattern}: das freigegebene Werk sperrt die Rolle nicht ({status})"
                );
            }
            WorkFrom::Query => {
                let (status, headers, body) = get_as(server, &s.erika, &format!("{pattern}?work={}", s.foreign)).await;
                assert!(
                    status == StatusCode::FORBIDDEN && denied_by_role(&headers),
                    "{verb} {pattern}: ein nicht freigegebenes Werk in der Adresse ist gesperrt (Status {status})"
                );
                assert_eq!(body, format!("Abgelehnt: unbekanntes Werk {}", s.foreign), "{pattern}: Meldung");
                // Ein zweites, kodiert benanntes work darf die Prüfung nicht unterlaufen.
                let smuggled = format!("{pattern}?work={}&wor%6B={}", s.granted, s.foreign);
                let (status, headers, _) = get_as(server, &s.erika, &smuggled).await;
                assert!(
                    status == StatusCode::FORBIDDEN && denied_by_role(&headers),
                    "{pattern}: auch ein zusätzliches (kodiertes) fremdes work wird abgelehnt"
                );
                // Ohne work: nie das Vorgabewerk, sondern Umleitung auf das freigegebene.
                let (status, headers, _) = get_as(server, &s.erika, &format!("{pattern}?key=5")).await;
                let path = if *pattern == "/" { "/".to_owned() } else { (*pattern).to_owned() };
                assert_eq!(
                    (status, location(&headers)),
                    (StatusCode::SEE_OTHER, format!("{path}?key=5&work={}", s.granted).as_str()),
                    "{pattern}: fehlt work, führt 303 auf dieselbe Adresse mit dem freigegebenen Werk"
                );
                let (status, headers, _) = get_as(server, &s.erika, pattern).await;
                assert_eq!(
                    (status, location(&headers)),
                    (StatusCode::SEE_OTHER, format!("{path}?work={}", s.granted).as_str()),
                    "{pattern}: auch ohne jede Angabe"
                );
                let (status, headers, _) = get_as(server, &s.erika, &format!("{pattern}?work={}", s.granted)).await;
                assert!(
                    !denied_by_role(&headers),
                    "{pattern}: das freigegebene Werk sperrt die Rolle nicht ({status})"
                );
            }
            WorkFrom::None | WorkFrom::Param => {
                // Auch Seiten ohne eigenes Werk nehmen kein fremdes work in der Adresse an (Menüband, Werkwahl).
                let (status, headers, _) = get_as(server, &s.erika, &format!("{pattern}?work={}", s.foreign)).await;
                let filled = fill(pattern, s.granted);
                assert!(
                    status == StatusCode::FORBIDDEN && denied_by_role(&headers),
                    "{verb} {filled}: ein fremdes work in der Adresse wird auch hier abgelehnt (Status {status})"
                );
            }
        }
    }
}

#[tokio::test]
async fn every_allowed_method_refuses_a_work_that_is_not_granted_and_never_falls_back_to_the_default_work() {
    let s = stage().await;
    let server = &s.server;
    let before = (row_counts(server).await, heads(server, &[s.granted, s.foreign]).await);
    for (method, from) in LEKTOR_RPC {
        // Ein genanntes fremdes Werk wird bei JEDER erlaubten Methode abgelehnt, auch bei denen ohne Werk.
        for (work, what) in
            [(json!(s.foreign), "fremdes Werk"), (json!(s.foreign.to_string()), "fremdes Werk als Text")]
        {
            let answer = rpc_as(server, &s.erika, method, json!({ "work": work, "key": s.foreign_key })).await;
            assert_eq!(code(&answer), Some(FORBIDDEN), "{method}: {what} → -32003, nicht {answer}");
            assert_eq!(
                message(&answer),
                format!("unbekanntes Werk {}", s.foreign),
                "{method}: dieselbe Meldung wie für ein Werk, das es nicht gibt"
            );
        }
        let unknown = rpc_as(server, &s.erika, method, json!({ "work": 9999 })).await;
        assert_eq!(
            (code(&unknown), message(&unknown)),
            (Some(FORBIDDEN), "unbekanntes Werk 9999"),
            "{method}: Gegenprobe mit einem Werk, das es nicht gibt"
        );
        if *from == WorkFrom::Param {
            let none = rpc_as(server, &s.gustav, method, json!({})).await;
            assert_eq!(code(&none), Some(FORBIDDEN), "{method}: ohne freigegebenes Werk gibt es kein Vorgabewerk");
        }
        let odd = rpc_as(server, &s.erika, method, json!([s.foreign])).await;
        assert!(code(&odd).is_some(), "{method}: Parameter als Liste tragen kein prüfbares Werk und werden abgelehnt");
    }
    assert_eq!(
        (row_counts(server).await, heads(server, &[s.granted, s.foreign]).await),
        before,
        "kein abgelehnter Aufruf hat eine Zeile oder einen Commit hinterlassen"
    );

    // Fehlt work, gilt das freigegebene Werk – das Vorgabewerk der Instanz ist das fremde.
    let got = rpc_as(server, &s.erika, "work.get", json!({})).await;
    assert_eq!(result(&got)["work"]["title"], "Der Leuchtturm am Sund", "work.get ohne work liest das freigegebene");
    let badges = rpc_as(server, &s.erika, "menu.badges", json!({})).await;
    assert_eq!(result(&badges)["work"], json!(s.granted), "menu.badges ohne work zählt im freigegebenen Werk");
    let badges = rpc_as(server, &s.erika, "menu.badges", Value::Null).await;
    assert_eq!(result(&badges)["work"], json!(s.granted), "auch ganz ohne Parameter");
    let rendered = rpc_as(server, &s.erika, "ui.render", json!({})).await;
    assert!(
        result(&rendered)["url"].as_str().unwrap().contains(&format!("work={}", s.granted)),
        "ui.render ohne work zeichnet das freigegebene Werk: {}",
        result(&rendered)["url"]
    );
    let author_default = rpc_as(server, &s.author, "work.get", json!({})).await;
    assert_eq!(
        result(&author_default)["work"]["title"],
        json!(s.foreign_title),
        "Gegenprobe: für den Autor gilt ohne work weiter das Vorgabewerk"
    );
}

#[tokio::test]
async fn ids_of_a_foreign_work_yield_nothing_under_the_granted_work() {
    let s = stage().await;
    let server = &s.server;
    // Fremde Daten als Autor anlegen: Beitrag, Befund, Faden und Bild im nicht freigegebenen Werk.
    let post = rpc_as(
        server,
        &s.author,
        "post.add",
        json!({ "work": s.foreign, "key": s.foreign_key, "body": "Geheime Randnotiz zum Moor." }),
    )
    .await;
    let post_id = result(&post)["id"].as_i64().unwrap();
    let finding = server
        .state
        .store
        .finding_insert(&baruch_store::NewFinding {
            work: s.foreign,
            key: Some(s.foreign_key),
            post: None,
            thread_post: None,
            position: 0,
            persona: "@kritiker",
            kind: baruch_store::FindingKind::Hint,
            body: "Geheimer Befund zum Moor.",
            quote: "",
            quote_check: "",
            similar_to: None,
            summary_of: &[],
        })
        .await
        .unwrap();
    let (thread, _) = server.state.store.thread_create(s.foreign, "Autor", "Geheimer Faden zum Moor.").await.unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><title>Geheimes Moor</title></svg>";
    let image = rpc_as(
        server,
        &s.author,
        "asset.add",
        json!({ "work": s.foreign, "file_name": "moor.svg", "content": svg, "encoding": "svg" }),
    )
    .await;
    let image_name = result(&image)["name"].as_str().unwrap().to_owned();
    let (status, _, served) =
        get_as(server, &s.author, &format!("/werke/{}/bilder/{image_name}/datei", s.foreign)).await;
    assert!(status == StatusCode::OK && served.contains("Geheimes Moor"), "Vorbedingung: der Autor bekommt das Bild");

    let g = s.granted;
    let calls = [
        ("post.versions", json!({ "work": g, "id": post_id })),
        ("post.edit", json!({ "work": g, "id": post_id, "body": "überschrieben" })),
        ("post.delete", json!({ "work": g, "id": post_id, "confirm": true })),
        ("post.page", json!({ "work": g, "key": s.foreign_key })),
        ("post.since", json!({ "work": g, "key": s.foreign_key, "after": 0 })),
        ("node.get", json!({ "work": g, "key": s.foreign_key })),
        ("revision.amends", json!({ "work": g, "key": s.foreign_key, "rev": 1 })),
        ("wysiwyg.diff", json!({ "work": g, "key": s.foreign_key, "rev": 1 })),
        ("finding.get", json!({ "work": g, "id": finding.id })),
        ("thread.get", json!({ "work": g, "thread": thread.id })),
        ("thread.since", json!({ "work": g, "thread": thread.id, "after": 0 })),
        ("thread.post_versions", json!({ "work": g, "thread": thread.id, "id": 1 })),
        ("revision.propose", json!({ "work": g, "key": s.foreign_key, "body": "Fremder Abschnitt, neu." })),
        ("post.add", json!({ "work": g, "key": s.foreign_key, "body": "Beitrag im fremden Abschnitt." })),
        (
            "finding.add",
            json!({ "work": g, "key": s.foreign_key, "kind": "hint", "body": "Befund am fremden Abschnitt." }),
        ),
    ];
    let before = (row_counts(server).await, heads(server, &[s.granted, s.foreign]).await);
    for (method, params) in calls {
        let answer = rpc_as(server, &s.erika, method, params).await;
        assert!(code(&answer).is_some(), "{method}: eine id des fremden Werks trifft unter dem freigegebenen nichts");
        assert!(!answer.to_string().contains("Geheim"), "{method}: die Antwort trägt keine fremden Daten: {answer}");
    }
    let listed = rpc_as(server, &s.erika, "finding.list", json!({ "work": g, "key": s.foreign_key })).await;
    assert!(!listed.to_string().contains("Geheim"), "finding.list filtert nach dem freigegebenen Werk: {listed}");
    let threads = rpc_as(server, &s.erika, "thread.list", json!({ "work": g })).await;
    assert!(!threads.to_string().contains("Geheim"), "thread.list zeigt keine Fäden des fremden Werks");
    assert_eq!(
        (row_counts(server).await, heads(server, &[s.granted, s.foreign]).await),
        before,
        "nichts davon hat geschrieben (der fremde Beitrag steht noch, kein Vorschlag, kein Befund)"
    );
    for uri in [
        format!("/werke/{g}/bilder/{image_name}/datei"),
        format!("/werke/{g}/bilder/{image_name}/vorschau"),
        format!("/beitrag?work={g}&id={post_id}"),
        format!("/faden?work={g}&thread={}", thread.id),
        format!("/beitraege?work={g}&key={}", s.foreign_key),
        format!("/?work={g}&key={}", s.foreign_key),
    ] {
        let (status, _, body) = get_as(server, &s.erika, &uri).await;
        assert!(!body.contains("Geheim"), "{uri}: keine fremden Daten unter dem freigegebenen Werk (Status {status})");
    }
}

// ---- Vorschlag ------------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_lektor_proposes_under_the_handle_and_only_the_author_accepts() {
    let s = stage().await;
    let server = &s.server;
    let (g, key) = (s.granted, s.granted_key);
    let original = current_body(server, g, 0).await;
    let head_before = heads(server, &[g]).await;
    let text = "# Nebelhorn\n\nDas Horn rief viermal über den Sund.";
    let proposed =
        rpc_as(server, &s.erika, "revision.propose", json!({ "work": g, "key": key, "body": text, "note": "Zählung" }))
            .await;
    let answer = result(&proposed);
    assert_eq!(answer["status"], "proposal", "ein Lektor legt nur einen Vorschlag an");
    assert_eq!(answer["author"], "@lektorat:erika", "Herkunft ist der Handle des Kontos");
    assert_eq!((answer["node"].as_u64(), answer["key"].as_i64()), (Some(0), Some(key)), "Abschnitt in der Antwort");
    let rev = answer["rev"].as_u64().unwrap();

    let node = rpc_as(server, &s.author, "node.get", json!({ "work": g, "key": key })).await;
    let revisions = result(&node)["revisions"].as_array().unwrap().clone();
    let new = revisions.iter().find(|r| r["number"].as_u64() == Some(rev)).expect("die neue Fassung");
    assert_eq!(new["status"], "proposed", "die Fassung ist ein offener Vorschlag");
    assert_eq!(
        new["author"],
        json!({ "kind": "assistant", "persona": "@lektorat:erika", "model": "" }),
        "Herkunft wie bei MCP-Vorschlägen, ohne Modell"
    );
    assert_eq!(new["note"], "Zählung", "die Notiz steht am Vorschlag");
    assert_eq!(
        new["body"].as_str().unwrap(),
        "= Nebelhorn\n\nDas Horn rief viermal über den Sund.",
        "Markdown-Überschriften werden wie bei revision.save zu Typst-Überschriften"
    );
    assert_eq!(common::currents(result(&node)), 1, "es bleibt bei genau einer aktuellen Fassung");
    assert_eq!(current_body(server, g, 0).await, original, "die aktuelle Fassung ist unverändert");

    // Spiegel: kein Commit auf dem Hauptzweig, aber der Seitenzweig der Fassung – mit Herkunft.
    assert_eq!(heads(server, &[g]).await, head_before, "ein Vorschlag ist kein Commit am Buch");
    let mirror = server.state.mirror(g).await.unwrap();
    let refs = mirror.side_refs("refs/revisions/").unwrap();
    let name = format!("refs/revisions/{key}/{rev}");
    assert!(refs.contains_key(&name), "der Vorschlag steht sofort als Seitenzweig im Spiegel: {refs:?}");
    if let Some(log) = common::git(&server.mirror_dir_of(&s.granted_slug), &["log", "-1", "--format=%an%n%B", &name]) {
        assert!(log.contains("@lektorat:erika"), "der Commit des Seitenzweigs nennt den Lektor als Herkunft: {log}");
    }

    // Formularweg ohne JavaScript: derselbe Vorschlag über /lektorat/act, danach zurück zur neuen Fassung.
    let form =
        format!("method=revision.propose&work={g}&key={key}&body={}", enc("= Nebelhorn\n\nDas Horn rief fünfmal."));
    let (status, headers, body) = form_as(server, &s.erika, "/lektorat/act", &form).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/?work={g}&key={key}&rev={}", rev + 1).as_str()),
        "das Formular legt den Vorschlag an und zeigt ihn: {body}"
    );
    assert_eq!(current_body(server, g, 0).await, original, "auch danach ist die aktuelle Fassung unverändert");

    // Was abgelehnt wird, ohne etwas zu schreiben.
    let counts = row_counts(server).await;
    let empty = rpc_as(server, &s.erika, "revision.propose", json!({ "work": g, "key": key, "body": "  \n" })).await;
    assert_eq!(code(&empty), Some(INVALID_PARAMS), "ein leerer Vorschlag ist keiner");
    let same = rpc_as(server, &s.erika, "revision.propose", json!({ "work": g, "key": key, "body": original })).await;
    assert_eq!(code(&same), Some(REFUSED), "der Text der aktuellen Fassung ist kein Vorschlag");
    let by_author =
        rpc_as(server, &s.author, "revision.propose", json!({ "work": g, "key": key, "body": "Neu." })).await;
    assert_eq!(code(&by_author), Some(REFUSED), "der Autor schlägt nicht vor, er speichert");
    assert!(message(&by_author).contains("revision.save"), "die Ablehnung nennt dem Autor seinen Weg: {by_author}");
    let (status, _, _) =
        form_as(server, &s.erika, "/lektorat/act", &format!("method=revision.propose&work={g}&key={key}&body=")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Fehler des Formulars sind 422 mit Text");
    assert_eq!(row_counts(server).await, counts, "keiner der abgelehnten Versuche hat eine Zeile angelegt");

    // Entscheiden darf nur der Autor.
    for method in [
        "revision.accept",
        "revision.set_current",
        "revision.reject",
        "revision.delete",
        "revision.update",
        "revision.save",
    ] {
        let params = json!({ "work": g, "key": key, "rev": rev, "body": "Übernommen?", "confirm": true });
        let answer = rpc_as(server, &s.erika, method, params).await;
        assert_eq!(code(&answer), Some(FORBIDDEN), "{method} bleibt dem Autor: {answer}");
        assert_eq!(message(&answer), format!("Only the author may do this: {method}"), "{method}: Meldung");
    }
    let (status, headers, _) =
        form_as(server, &s.erika, "/act", &format!("method=revision.accept&work={g}&key={key}&rev={rev}")).await;
    assert!(status == StatusCode::FORBIDDEN && denied_by_role(&headers), "auch das Formular /act ist für Lektoren zu");
    assert_eq!(current_body(server, g, 0).await, original, "nach allen Versuchen des Lektors gilt die alte Fassung");
    let accepted = rpc_as(server, &s.author, "revision.accept", json!({ "work": g, "key": key, "rev": rev })).await;
    assert!(result(&accepted)["commit"].is_string(), "der Autor übernimmt den Vorschlag (Commit im Spiegel)");
    assert_eq!(
        current_body(server, g, 0).await,
        "= Nebelhorn\n\nDas Horn rief viermal über den Sund.",
        "jetzt ist der Text des Lektors die aktuelle Fassung"
    );
}

// ---- Beiträge -------------------------------------------------------------------------------------------------

/// Die Meldung aus `posts::not_editable` (dort `pub(crate)`), wörtlich.
fn not_editable(id: i64, author: &str, actor: &str) -> String {
    format!(
        "Nur eigene Beiträge lassen sich bearbeiten: Beitrag {id} stammt von {author}, nicht von {actor}. Beiträge von \
         Personas bleiben unverändert (kopieren, zitieren, in Einfall oder Befund umwandeln geht)."
    )
}

#[tokio::test]
async fn posts_of_a_lektor_carry_the_handle_and_only_own_posts_are_editable() {
    let s = stage().await;
    let server = &s.server;
    let (g, key) = (s.granted, s.granted_key);
    server.state.store.member_grant(g, s.gustav_id, "lektor").await.unwrap();
    let add = |who: Who, body: &'static str| async move {
        let answer = rpc_as(server, &who, "post.add", json!({ "work": g, "key": key, "body": body })).await;
        result(&answer)["id"].as_i64().unwrap()
    };
    let by_author = add(s.author.clone(), "Bitte auf die Zählung achten.").await;
    let by_erika = add(s.erika.clone(), "Viermal, nicht dreimal?").await;
    let by_gustav = add(s.gustav.clone(), "Mir gefällt das Horn.").await;
    let (status, headers, _) = form_as(
        server,
        &s.erika,
        "/lektorat/act",
        &format!("method=post.add&work={g}&key={key}&body={}", enc("Noch ein Gedanke.")),
    )
    .await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/?work={g}&key={key}").as_str()),
        "Beitrag über das Formular, danach zurück zum Abschnitt"
    );

    let page = rpc_as(server, &s.erika, "post.page", json!({ "work": g, "key": key })).await;
    let posts = result(&page)["posts"].as_array().unwrap().clone();
    let seen: Vec<(&str, bool)> =
        posts.iter().map(|p| (p["author"].as_str().unwrap(), p["own"].as_bool().unwrap())).collect();
    assert_eq!(
        seen,
        [("Autor", false), ("@lektorat:erika", true), ("@lektorat:gustav", false), ("@lektorat:erika", true)],
        "Beiträge tragen den Handle ihres Lektors bzw. „Autor“; eigen sind für Erika nur ihre"
    );
    let author_view = rpc_as(server, &s.author, "post.page", json!({ "work": g, "key": key })).await;
    let own: Vec<bool> =
        result(&author_view)["posts"].as_array().unwrap().iter().map(|p| p["own"].as_bool().unwrap()).collect();
    assert_eq!(own, [true, false, false, false], "für den Autor sind wie bisher nur die Beiträge unter „Autor“ eigene");

    // Bearbeiten und Löschen: nur eigene.
    let edited = rpc_as(
        server,
        &s.erika,
        "post.edit",
        json!({ "work": g, "id": by_erika, "body": "Viermal – so steht es im Logbuch." }),
    )
    .await;
    assert_eq!(result(&edited)["body"], "Viermal – so steht es im Logbuch.", "den eigenen Beitrag bearbeitet Erika");
    assert!(result(&edited)["commit"].is_string(), "das Bearbeiten ist ein Commit im Spiegel");
    if let Some(log) = common::git(&server.mirror_dir_of(&s.granted_slug), &["log", "-1", "--format=%an%n%B"]) {
        assert!(log.starts_with("@lektorat:erika\n"), "Autor des Commits ist der Lektor: {log}");
        assert!(log.contains(&format!("Beitrag {by_erika}: bearbeitet")), "Betreff wie bei jedem bearbeiteten Beitrag");
    }
    for (id, owner) in [(by_author, "Autor"), (by_gustav, "@lektorat:gustav")] {
        let expected = not_editable(id, owner, "@lektorat:erika");
        let edit =
            rpc_as(server, &s.erika, "post.edit", json!({ "work": g, "id": id, "body": "fremd überschrieben" })).await;
        assert_eq!((code(&edit), message(&edit)), (Some(REFUSED), expected.as_str()), "fremden Beitrag bearbeiten");
        let delete = rpc_as(server, &s.erika, "post.delete", json!({ "work": g, "id": id, "confirm": true })).await;
        assert_eq!((code(&delete), message(&delete)), (Some(REFUSED), expected.as_str()), "fremden Beitrag löschen");
        let (status, _, body) =
            form_as(server, &s.erika, "/lektorat/act", &format!("method=post.delete&work={g}&id={id}&confirm=on"))
                .await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "auch über das Formular bleibt der fremde Beitrag: {body}"
        );
    }
    let unconfirmed = rpc_as(server, &s.erika, "post.delete", json!({ "work": g, "id": by_erika })).await;
    assert_eq!(code(&unconfirmed), Some(INVALID_PARAMS), "Löschen braucht auch beim Lektor die Bestätigung");
    for method in ["post.move", "post.to_idea", "post.to_findings", "post.repeat"] {
        let answer =
            rpc_as(server, &s.erika, method, json!({ "work": g, "id": by_erika, "target_key": key, "words": "Horn" }))
                .await;
        assert_eq!(code(&answer), Some(FORBIDDEN), "{method} gibt es für Lektoren nicht");
    }
    let (status, headers, _) = form_as(
        server,
        &s.erika,
        "/lektorat/act",
        &format!("method=post.delete&work={g}&id={by_erika}&confirm=on&back=post"),
    )
    .await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/?work={g}&key={key}").as_str()),
        "den eigenen Beitrag löscht Erika über das Formular"
    );
    let left = rpc_as(server, &s.author, "post.page", json!({ "work": g, "key": key })).await;
    let ids: Vec<i64> = result(&left)["posts"].as_array().unwrap().iter().map(|p| p["id"].as_i64().unwrap()).collect();
    assert!(ids.contains(&by_author) && ids.contains(&by_gustav) && !ids.contains(&by_erika), "weg ist nur der eigene");

    // Der Autor handelt weiter als „Autor“ und verwaltet jeden Beitrag.
    let own_edit = rpc_as(
        server,
        &s.author,
        "post.edit",
        json!({ "work": g, "id": by_author, "body": "Bitte die Zählung prüfen." }),
    )
    .await;
    assert_eq!(code(&own_edit), None, "der Autor bearbeitet seinen Beitrag wie vor Runde 35");
    let removed =
        rpc_as(server, &s.author, "post.delete", json!({ "work": g, "id": by_gustav, "confirm": true })).await;
    assert_eq!(
        result(&removed)["deleted"]["author"],
        "@lektorat:gustav",
        "der Autor darf auch Beiträge der Lektoren löschen"
    );
}

// ---- Befunde --------------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_lektor_adds_findings_under_the_handle_and_the_author_decides() {
    let s = stage().await;
    let server = &s.server;
    let (g, key) = (s.granted, s.granted_key);
    let head_before = heads(server, &[g]).await;
    let added = rpc_as(server, &s.erika, "finding.add", json!({ "work": g, "key": key, "kind": "critical", "body": "Dreimal oder viermal? Kapitel 2 sagt viermal.", "quote": "rief dreimal" })).await;
    let finding = &result(&added)["finding"];
    let id = finding["id"].as_i64().unwrap();
    assert_eq!(finding["persona"], "@lektorat:erika", "der Befund steht unter dem Handle des Lektors");
    assert_eq!(
        (finding["kind"].as_str(), finding["status"].as_str()),
        (Some("critical"), Some("open")),
        "Art und Status"
    );
    assert_eq!((finding["key"].as_i64(), finding["post"].as_i64()), (Some(key), None), "am Abschnitt, ohne Beitrag");
    assert_eq!(finding["quote"], "rief dreimal", "das Zitat steht am Befund");
    let commit = result(&added)["commit"].as_str().expect("das Anlegen ist ein Commit im Spiegel").to_owned();
    let head = heads(server, &[g]).await;
    assert!(head != head_before && head[0].as_deref() == Some(commit.as_str()), "HEAD des Spiegels ist dieser Commit");
    if let Some(log) = common::git(&server.mirror_dir_of(&s.granted_slug), &["log", "-1", "--format=%an <%ae>%n%B"]) {
        assert!(
            log.starts_with("Erika Beispiel (@lektorat:erika) <lektorat-erika@lektorat.baruch.invalid>"),
            "Autor des Commits ist der Lektor: {log}"
        );
        assert!(
            log.contains(&format!("Befund {id}: angelegt von @lektorat:erika")),
            "Betreff nennt Befund und Herkunft"
        );
        assert!(
            log.contains("Baruch-Action: finding-add") && log.contains("Baruch-Lektor: @lektorat:erika"),
            "Trailer der Herkunft: {log}"
        );
        assert!(log.contains(&format!("Baruch-Node: {key}")), "der Abschnitt steht als fester Schlüssel im Trailer");
        assert!(!log.contains("MCP-Client"), "ein Lektor ist kein MCP-Client");
    }

    // Ohne key: Anmerkung zum Buch, über das Formular, danach auf die Befundseite.
    let form = format!(
        "method=finding.add&work={g}&kind=praise&body={}&back=befunde",
        enc("Der Ton trägt durch das ganze Buch.")
    );
    let (status, headers, _) = form_as(server, &s.erika, "/lektorat/act", &form).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, format!("/werke/{g}/befunde").as_str()),
        "Anmerkung über das Formular"
    );
    let listed = rpc_as(server, &s.erika, "finding.list", json!({ "work": g, "include_praise": true })).await;
    let all = result(&listed)["findings"].as_array().unwrap().clone();
    let note = all.iter().find(|f| f["kind"] == "praise").expect("die Anmerkung zum Buch");
    assert!(note["key"].is_null() && note["node"].is_null(), "ohne key hängt der Befund an keinem Abschnitt");
    assert_eq!(note["persona"], "@lektorat:erika", "auch die Anmerkung trägt den Handle");
    let seen = rpc_as(server, &s.author, "finding.list", json!({ "work": g, "include_praise": true })).await;
    let personas: Vec<&str> =
        result(&seen)["findings"].as_array().unwrap().iter().map(|f| f["persona"].as_str().unwrap()).collect();
    assert_eq!(personas, ["@lektorat:erika", "@lektorat:erika"], "der Autor sieht beide Befunde mit ihrer Herkunft");
    for who in [&s.author, &s.erika] {
        let (status, headers, _) = get_as(server, who, &format!("/werke/{g}/befunde")).await;
        assert!(
            status == StatusCode::OK && !denied_by_role(&headers),
            "die Befundseite des Werks öffnet sich für {who:?}"
        );
    }

    // Abgelehnt, ohne zu schreiben.
    let before = (row_counts(server).await, heads(server, &[g]).await);
    for (params, why) in [
        (json!({ "work": g, "kind": "hint", "body": "   " }), "ein Befund braucht einen Text"),
        (json!({ "work": g, "kind": "wichtig", "body": "Text" }), "unbekannte Art"),
        (json!({ "work": g, "body": "Text" }), "die Art fehlt"),
        (
            json!({ "work": g, "kind": "hint", "body": "Text", "key": 999_999 }),
            "ein Abschnitt, den es im Werk nicht gibt",
        ),
    ] {
        let answer = rpc_as(server, &s.erika, "finding.add", params).await;
        assert_eq!(code(&answer), Some(INVALID_PARAMS), "{why}: {answer}");
    }
    let by_author =
        rpc_as(server, &s.author, "finding.add", json!({ "work": g, "kind": "hint", "body": "Text" })).await;
    assert_eq!(code(&by_author), Some(REFUSED), "finding.add ist der Weg der Lektoren, nicht des Autors");
    assert_eq!((row_counts(server).await, heads(server, &[g]).await), before, "nichts davon hat geschrieben");

    // Der Status bleibt dem Autor.
    for (method, params) in [
        ("finding.set_status", json!({ "work": g, "id": id, "status": "done" })),
        ("finding.decide", json!({ "work": g, "entry": 1, "confirm": true })),
    ] {
        let answer = rpc_as(server, &s.erika, method, params).await;
        assert_eq!(code(&answer), Some(FORBIDDEN), "{method} bleibt dem Autor");
    }
    let (status, headers, _) =
        form_as(server, &s.erika, "/werke/befunde/act", &format!("work={g}&id={id}&status=done")).await;
    assert!(
        status == StatusCode::FORBIDDEN && denied_by_role(&headers),
        "auch das Formular der Befundseite ist für Lektoren zu"
    );
    let done = rpc_as(server, &s.author, "finding.set_status", json!({ "work": g, "id": id, "status": "done" })).await;
    assert_eq!(result(&done)["finding"]["status"], "done", "der Autor setzt den Status des Lektor-Befunds");
}

// ---- Freigaben ------------------------------------------------------------------------------------------------

#[tokio::test]
async fn members_are_managed_with_full_rights_only() {
    let s = stage().await;
    let server = &s.server;
    let g = s.granted;
    for who in [&s.erika, &s.gustav] {
        for method in ["member.list", "member.grant", "member.revoke"] {
            let answer = rpc_as(server, who, method, json!({ "work": g, "account": s.gustav_id })).await;
            assert_eq!(code(&answer), Some(FORBIDDEN), "{method} ist nichts für Lektoren");
        }
    }
    let (status, headers, _) = form_as(
        server,
        &s.erika,
        &format!("/werke/{g}/mitwirkende/act"),
        &format!("action=grant&account={}", s.gustav_id),
    )
    .await;
    assert!(status == StatusCode::FORBIDDEN && denied_by_role(&headers), "ein Lektor gibt nichts frei");
    assert!(server.state.store.member_works(s.gustav_id).await.unwrap().is_empty(), "und Gustav hat weiter kein Werk");

    let listed = rpc_as(server, &s.author, "member.list", json!({ "work": g })).await;
    assert_eq!(
        result(&listed)["members"][0],
        json!({ "account": s.erika_id, "name": "Erika", "display": "Erika Beispiel", "handle": "@lektorat:erika",
            "role": "lektor", "granted": result(&listed)["members"][0]["granted"], "active": true }),
        "member.list zeigt die Freigabe mit Konto, Handle und Zeitpunkt"
    );
    assert_eq!(
        result(&listed)["candidates"],
        json!([{ "account": s.gustav_id, "name": "Gustav", "display": "Gustav Beispiel", "handle": "@lektorat:gustav" }]),
        "Kandidaten sind die aktiven Lektor-Konten ohne Freigabe – nicht der Autor"
    );
    assert!(!listed.to_string().contains("argon2"), "kein Hash in der Antwort");

    // Freigeben: idempotent, nur Lektor-Konten, nur vorhandene Werke.
    let first = rpc_as(server, &s.author, "member.grant", json!({ "work": g, "account": s.gustav_id })).await;
    assert_eq!(result(&first)["role"], "lektor", "Gustav bekommt die Rolle lektor");
    let again =
        rpc_as(server, &s.author, "member.grant", json!({ "work": g, "account": s.gustav_id, "role": "lektor" })).await;
    assert_eq!(
        result(&again)["granted"],
        result(&first)["granted"],
        "noch einmal freigeben ändert nichts (idempotent)"
    );
    assert_eq!(server.state.store.member_list(g).await.unwrap().len(), 2, "zwei Freigaben, keine doppelte Zeile");
    assert_eq!(
        roles::role_in(&server.state, &s.gustav, g).await.unwrap(),
        Some(Role::Lektor),
        "Gustav ist jetzt Lektor"
    );
    let cases = [
        (json!({ "work": g, "account": s.author_id }), REFUSED, "ein Autor-Konto bekommt keine Freigabe"),
        (json!({ "work": g, "account": 9999 }), INVALID_PARAMS, "unbekanntes Konto"),
        (json!({ "work": 9999, "account": s.gustav_id }), INVALID_PARAMS, "unbekanntes Werk"),
        (json!({ "account": s.gustav_id }), INVALID_PARAMS, "ohne work gilt nie das Vorgabewerk"),
        (json!({ "work": g, "account": s.gustav_id, "role": "author" }), INVALID_PARAMS, "andere Rolle als lektor"),
    ];
    for (params, expected, why) in cases {
        let answer = rpc_as(server, &s.author, "member.grant", params).await;
        assert_eq!(code(&answer), Some(expected), "{why}: {answer}");
    }
    assert_eq!(server.state.store.member_list(g).await.unwrap().len(), 2, "die abgelehnten Versuche gaben nichts frei");
    let revoked = rpc_as(server, &s.author, "member.revoke", json!({ "work": g, "account": s.gustav_id })).await;
    assert_eq!(result(&revoked)["revoked"], true, "entzogen");
    let nothing = rpc_as(server, &s.author, "member.revoke", json!({ "work": g, "account": s.gustav_id })).await;
    assert_eq!(result(&nothing)["revoked"], false, "ein zweites Mal gibt es nichts zu entziehen");

    // Formular des Autors mit den Codes aus Vertrag §3.4; auch ohne Konten (Who::Open) und am eigenen Rechner.
    let uri = format!("/werke/{g}/mitwirkende/act");
    let back = |code: &str| format!("/werke/{g}/mitwirkende?{code}");
    let cases = [
        (format!("action=grant&account={}", s.gustav_id), "ok=freigegeben"),
        (format!("action=grant&account={}", s.gustav_id), "ok=freigegeben"),
        (format!("action=revoke&account={}", s.gustav_id), "ok=entzogen"),
        ("action=grant&account=9999".to_owned(), "fehler=unbekanntes_konto"),
        ("action=grant&account=".to_owned(), "fehler=unbekanntes_konto"),
        (format!("action=grant&account={}", s.author_id), "fehler=kein_lektor"),
    ];
    for (form, expected) in cases {
        let (status, headers, body) = form_as(server, &s.author, &uri, &form).await;
        assert_eq!((status, location(&headers)), (StatusCode::SEE_OTHER, back(expected).as_str()), "{form}: {body}");
    }
    assert!(
        server.state.store.member_works(s.gustav_id).await.unwrap().is_empty(),
        "am Ende ist Gustav wieder ohne Werk"
    );
    let (status, headers, _) =
        form_as(server, &Who::LocalAuthor, &uri, &format!("action=grant&account={}", s.gustav_id)).await;
    assert_eq!(
        (status, location(&headers)),
        (StatusCode::SEE_OTHER, back("ok=freigegeben").as_str()),
        "auch der Autor am eigenen Rechner gibt frei"
    );
    let (status, _, _) = form_as(server, &s.author, &uri, "action=verschenken&account=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "eine unbekannte Aktion ist ein Fehler, keine Freigabe");
    let (status, _, _) =
        form_as(server, &s.author, "/werke/9999/mitwirkende/act", &format!("action=grant&account={}", s.gustav_id))
            .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ein unbekanntes Werk lässt sich nicht freigeben");
}

// ---- Literatur und volle Rechte -------------------------------------------------------------------------------

#[tokio::test]
async fn the_literature_list_is_empty_for_a_lektor() {
    let s = stage().await;
    let server = &s.server;
    let g = s.granted;
    let titles = rpc_as(server, &s.author, "library.list", json!({})).await;
    let title = result(&titles)["titles"][0]["title"].as_str().expect("die Beispielbibliothek hat Titel").to_owned();
    let (_, _, author_page) = get_as(server, &s.author, &format!("/?work={g}")).await;
    assert!(author_page.contains(&title), "Vorbedingung: die Arbeitsseite des Autors nennt den Titel „{title}“");
    // ui.render trägt die Literatursuche in der Adresse weiter (`lq`): für den Autor ja, für den Lektor nie.
    let author_render = rpc_as(server, &s.author, "ui.render", json!({ "work": g, "lq": "Horn" })).await;
    let author_url = result(&author_render)["url"].as_str().unwrap().to_owned();
    assert!(author_url.contains("lq=Horn"), "Vorbedingung: ui.render des Autors sucht in der Literatur: {author_url}");

    let (status, _, page) = get_as(server, &s.erika, &format!("/?work={g}&lq={}", enc(&title))).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite des Lektors");
    assert!(
        !page.contains(&title),
        "die Seite des Lektors nennt keinen Titel der Literatur – auch nicht über die Suche"
    );
    let (_, _, plain) = get_as(server, &s.erika, &format!("/?work={g}")).await;
    assert!(!plain.contains(&title), "auch ohne Suche steht kein Titel der Literatur auf der Seite des Lektors");
    let render = rpc_as(server, &s.erika, "ui.render", json!({ "work": g, "lq": "Horn" })).await;
    let url = result(&render)["url"].as_str().unwrap();
    assert!(!url.contains("lq="), "ui.render des Lektors sucht nicht in der Literatur: {url}");
    assert!(!result(&render).to_string().contains(&title), "und nennt keinen Titel der Literatur");
    for method in ["library.list", "library.search", "literature.render"] {
        let answer = rpc_as(server, &s.erika, method, json!({ "work": g, "query": "Horn" })).await;
        assert_eq!(code(&answer), Some(FORBIDDEN), "{method}: Literatur gehört dem Autor");
    }
}

#[tokio::test]
async fn with_full_rights_everything_answers_as_before() {
    let s = stage().await;
    let server = &s.server;
    let g = s.granted;
    // Lesende Antworten sind für alle drei Arten voller Rechte gleich (auch ohne Angabe des Werks: Vorgabewerk).
    for (method, params) in [
        ("work.get", json!({})),
        ("work.list", json!({})),
        ("node.get", json!({ "work": g, "node": 0 })),
        ("revision.tree", json!({ "work": g })),
        ("finding.list", json!({ "work": g })),
        ("ui.render", json!({ "work": g })),
        ("gibt.es_nicht", json!({})),
    ] {
        let open = rpc_as(server, &Who::Open, method, params.clone()).await;
        for who in [&Who::LocalAuthor, &s.author] {
            assert_eq!(
                rpc_as(server, who, method, params.clone()).await,
                open,
                "{method}: {who:?} bekommt dieselbe Antwort wie ohne Konten"
            );
        }
    }
    let unknown = rpc_as(server, &s.author, "gibt.es_nicht", json!({})).await;
    assert_eq!(code(&unknown), Some(-32601), "unbekannte Methoden bleiben für den Autor „unbekannt“, nicht „verboten“");
    // Autor-Aktionen stehen weiter unter „Autor“.
    let saved = rpc_as(
        server,
        &s.author,
        "revision.save",
        json!({ "work": g, "key": s.granted_key, "body": "= Nebelhorn\n\nDas Horn schwieg." }),
    )
    .await;
    assert!(result(&saved)["commit"].is_string(), "der Autor speichert wie bisher");
    let node = rpc_as(server, &s.author, "node.get", json!({ "work": g, "key": s.granted_key })).await;
    let current =
        result(&node)["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap().clone();
    assert_eq!(
        current["author"],
        json!({ "kind": "human", "name": "Autor" }),
        "die Fassung des Autor-Kontos steht unter „Autor“"
    );
    let post =
        rpc_as(server, &s.author, "post.add", json!({ "work": g, "key": s.granted_key, "body": "Notiz." })).await;
    let page = rpc_as(server, &s.author, "post.page", json!({ "work": g, "key": s.granted_key })).await;
    let last = result(&page)["posts"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(
        (last["id"].as_i64(), last["author"].as_str()),
        (result(&post)["id"].as_i64(), Some("Autor")),
        "Beiträge des Autors stehen unter „Autor“"
    );
    if let Some(log) = common::git(&server.mirror_dir_of(&s.granted_slug), &["log", "-1", "--format=%an"]) {
        assert_eq!(log.trim(), "Autor", "auch im Spiegel ist der Autor des Commits „Autor“");
    }
    // Ohne Anmeldung geht nichts: 401 statt Daten.
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 1, "method": "work.list" }).to_string()))
        .unwrap();
    let (status, _, body) = send_as(server, &Who::Anonymous, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "unangemeldet antwortet /rpc mit 401");
    assert!(body.contains("-32002"), "mit dem JSON-RPC-Fehler -32002: {body}");
}
