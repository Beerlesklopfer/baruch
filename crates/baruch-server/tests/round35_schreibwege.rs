// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 35 „Konten und Rollen“, owner rollen: for Lektors everything is locked that is not explicitly allowed.
//!
//! The lists are built from the source (`common::registered_routes`, `common::rpc_method_candidates`), not from a
//! maintained table: a route or method of future rounds is in them by itself and has to be locked for Lektors until it
//! is allowed in `roles::LEKTOR_ROUTES` or `roles::LEKTOR_RPC`. Stage and helpers from `round35_rollen` (Lektor Erika
//! with one granted work; the default work of the instance is foreign).

use crate::common;
use crate::round35_rollen::{
    FORBIDDEN, Stage, code, denied_by_role, form_as, get_as, heads, row_counts, rpc_as, rpc_raw, send_as, stage,
};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::roles::{LEKTOR_ROUTES, LEKTOR_RPC};
use serde_json::{Value, json};

/// Does the path fit the pattern (`{name}` = exactly one non-empty segment)? An own, plain reconstruction of the rule
/// of the router – the test is not meant to check with the function that it checks.
fn matches(pattern: &str, path: &str) -> bool {
    let wanted: Vec<&str> = pattern.split('/').collect();
    let given: Vec<&str> = path.split('/').collect();
    wanted.len() == given.len()
        && wanted.iter().zip(&given).all(|(w, g)| if w.starts_with('{') { !g.is_empty() } else { w == g })
}

/// Does the table of the Lektors allow this call?
fn allowed(verb: &str, path: &str) -> bool {
    LEKTOR_ROUTES.iter().any(|(v, pattern, _)| *v == verb && matches(pattern, path))
}

/// Fill a pattern with values: `{id}` with the work, every other placeholder with `1` (also `{*rest}`).
fn fill(pattern: &str, work: i64) -> String {
    pattern
        .split('/')
        .map(|segment| match segment {
            "{id}" => work.to_string(),
            s if s.starts_with('{') => "1".to_owned(),
            s => s.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The whole state that must not change: row count of every table and HEAD of both mirrors.
async fn snapshot(s: &Stage) -> (std::collections::BTreeMap<String, i64>, Vec<Option<String>>) {
    (row_counts(&s.server).await, heads(&s.server, &[s.granted, s.foreign]).await)
}

#[tokio::test]
async fn every_registered_route_outside_the_table_is_closed_for_a_lektor() {
    let s = stage().await;
    let server = &s.server;
    let routes = common::registered_routes();
    assert!(routes.len() >= 130, "Vorbedingung: die Liste aus dem Quelltext ist die ganze ({} Routen)", routes.len());
    let before = snapshot(&s).await;
    let mut closed = 0;
    for (verb, pattern) in &routes {
        if pattern == "/mcp" {
            // `/mcp` stays with tokens: the sign-in layer leaves it untouched (contract §0.3), a session – and with it
            // a role – does not exist there.
            continue;
        }
        for work in [s.granted, s.foreign] {
            let path = fill(pattern, work);
            if allowed(verb, &path) {
                continue;
            }
            // With the granted work in the address: even the most favourable case for the Lektor stays locked.
            let uri = format!("{path}?work={}", s.granted);
            let (status, headers, body) = match verb.as_str() {
                "GET" => get_as(server, &s.erika, &uri).await,
                _ => {
                    let form =
                        format!("work={}&method=revision.accept&key={}&rev=1&confirm=on", s.granted, s.granted_key);
                    form_as(server, &s.erika, &uri, &form).await
                }
            };
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{verb} {path} steht nicht in LEKTOR_ROUTES und muss für Lektoren 403 sein: {body}"
            );
            assert!(denied_by_role(&headers), "{verb} {path}: die Ablehnung trägt x-baruch-abgelehnt: rolle");
            assert!(body.starts_with("Abgelehnt: "), "{verb} {path}: Text der Ablehnung, nicht {body:?}");
            closed += 1;
        }
    }
    assert!(closed >= 200, "Vorbedingung: der größte Teil der Routen ist für Lektoren gesperrt (geprüft: {closed})");
    // JSON as the body makes no difference: the lock takes hold before every handler.
    for (verb, pattern) in
        routes.iter().filter(|(verb, pattern)| verb == "POST" && !allowed(verb, &fill(pattern, s.granted)))
    {
        if pattern == "/mcp" {
            continue;
        }
        let request = Request::post(fill(pattern, s.granted))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "work": s.granted, "confirm": true }).to_string()))
            .unwrap();
        let (status, headers, _) = send_as(server, &s.erika, request).await;
        assert!(
            status == StatusCode::FORBIDDEN && denied_by_role(&headers),
            "{verb} {pattern} mit JSON: 403, nicht {status}"
        );
    }
    // Verbs other than GET, HEAD and POST do not exist for Lektors – not on allowed paths either.
    for method in ["PUT", "DELETE", "PATCH"] {
        let request =
            Request::builder().method(method).uri(format!("/?work={}", s.granted)).body(Body::empty()).unwrap();
        let (status, headers, _) = send_as(server, &s.erika, request).await;
        assert!(status == StatusCode::FORBIDDEN && denied_by_role(&headers), "{method} / ist für Lektoren gesperrt");
    }
    // Variations of allowed paths are not allowed paths.
    for uri in [
        format!("/werke/{}/befunde/", s.granted),
        format!("/werke/{}/befunde/act", s.granted),
        format!("/werke/%3{}/befunde", s.granted % 10),
        "/werke/".to_owned(),
        "/%77erke".to_owned(),
        "/static/js/../../act".to_owned(),
        "/einstellungen/daten".to_owned(),
    ] {
        let (status, headers, _) = get_as(server, &s.erika, &uri).await;
        assert!(status == StatusCode::FORBIDDEN && denied_by_role(&headers), "GET {uri}: 403, nicht {status}");
    }
    for uri in ["/lektorat/act/", "/rpc/", "/static/js/../../act", "/", "/werke"] {
        let (status, headers, _) = form_as(server, &s.erika, uri, "method=post.add").await;
        assert!(status == StatusCode::FORBIDDEN && denied_by_role(&headers), "POST {uri}: 403, nicht {status}");
    }
    assert_eq!(
        snapshot(&s).await,
        before,
        "nach allen Versuchen sind Datenbank (Zeilenzahlen) und Spiegel (HEAD) unverändert"
    );
}

#[test]
fn every_route_of_the_table_exists_in_the_router() {
    let routes = common::registered_routes();
    for (verb, pattern, _) in LEKTOR_ROUTES {
        // The route exists if a pattern of the router accepts its path (`/einstellungen/{seite}` accepts
        // `/einstellungen/darstellung`).
        let path = fill(pattern, 1);
        let registered = routes.iter().any(|(v, registered)| v == verb && matches(registered, &path));
        assert!(
            registered,
            "{verb} {pattern} steht in LEKTOR_ROUTES, aber nicht mehr im Router – die Tabelle erlaubt etwas, das es \
             nicht gibt (oder eine Route wurde umbenannt, ohne die Tabelle nachzuführen)"
        );
    }
}

#[tokio::test]
async fn every_method_outside_the_table_is_forbidden_for_a_lektor() {
    let s = stage().await;
    let server = &s.server;
    let candidates = common::rpc_method_candidates();
    let outside: Vec<&String> =
        candidates.iter().filter(|c| !LEKTOR_RPC.iter().any(|(m, _)| *m == c.as_str())).collect();
    assert!(
        outside.len() >= 250,
        "Vorbedingung: die Kandidaten aus dem Quelltext sind die ganze Liste ({})",
        outside.len()
    );
    // What the contract names explicitly is in the list (and is thereby checked below).
    for named in [
        "revision.save",
        "revision.accept",
        "revision.set_current",
        "revision.update",
        "revision.delete",
        "revision.reject",
        "revision.fold_set",
        "revision.fold_clear",
        "revision.amend_undo",
        "revision.policy_set",
        "outline.save",
        "outline.add",
        "outline.rename",
        "outline.move",
        "outline.delete",
        "outline.set_level",
        "asset.add",
        "asset.delete",
        "work.create",
        "work.update",
        "work.delete",
        "work.import",
        "work.export",
        "work.exchange_export",
        "work.exchange_import",
        "work.pdf",
        "work.epub",
        "persona.chat",
        "persona.update",
        "idea.create",
        "idea.update",
        "idea.delete",
        "idea.list",
        "thread.post",
        "thread.post_edit",
        "thread.post_delete",
        "thread.repeat",
        "post.repeat",
        "post.move",
        "post.to_idea",
        "post.to_findings",
        "finding.set_status",
        "finding.decide",
        "search.replace",
        "search.undo",
        "glossary.add",
        "index.mark",
        "member.list",
        "member.grant",
        "member.revoke",
        "account.list",
        "account.create",
        "account.set_active",
        "account.set_local_open",
        "mcp_token.create",
        "model.list",
        "library.list",
        "library.search",
        "instance.set_name",
        "bible.set_dirs",
    ] {
        assert!(outside.iter().any(|c| c.as_str() == named), "{named} steht unter den geprüften, gesperrten Methoden");
    }
    let before = snapshot(&s).await;
    let params = json!({ "work": s.granted, "key": s.granted_key, "node": 0, "rev": 1, "id": 1, "confirm": true,
        "body": "Text des Lektors.", "title": "Neu", "message": "Hallo", "name": "x", "account": s.erika_id });
    for method in &outside {
        let answer = rpc_as(server, &s.erika, method, params.clone()).await;
        assert_eq!(code(&answer), Some(FORBIDDEN), "{method} steht nicht in LEKTOR_RPC: -32003, nicht {answer}");
        assert_eq!(
            answer["error"]["message"],
            json!(format!("Only the author may do this: {method}")),
            "{method}: die Meldung nennt die Methode"
        );
        let form = format!("method={method}&work={}&key={}&rev=1&id=1&confirm=on&body=Text", s.granted, s.granted_key);
        let (status, headers, body) = form_as(server, &s.erika, "/lektorat/act", &form).await;
        assert!(
            status == StatusCode::FORBIDDEN && denied_by_role(&headers),
            "{method} über /lektorat/act: 403 mit x-baruch-abgelehnt, nicht {status} {body}"
        );
    }
    // As a batch: every answer individually -32003, in the order of the requests.
    let batch: Vec<Value> = outside
        .iter()
        .enumerate()
        .map(|(i, method)| json!({ "jsonrpc": "2.0", "id": i, "method": method, "params": params }))
        .collect();
    let answers = rpc_raw(server, &s.erika, Value::Array(batch)).await;
    let answers = answers.as_array().expect("ein Batch wird als Liste beantwortet");
    assert_eq!(answers.len(), outside.len(), "eine Antwort je Anfrage");
    for (i, (answer, method)) in answers.iter().zip(&outside).enumerate() {
        assert_eq!(answer["id"], json!(i), "{method}: Antworten in der Reihenfolge der Anfragen");
        assert_eq!(code(answer), Some(FORBIDDEN), "{method} im Batch: -32003, nicht {answer}");
    }
    // Names that do not exist at all are forbidden too – not „unbekannt“ (no probing of the methods).
    for method in ["gibt.es_nicht", "", "revision.Accept", " revision.accept", "revision.accept ", "work"] {
        let answer = rpc_as(server, &s.erika, method, json!({ "work": s.granted })).await;
        assert_eq!(code(&answer), Some(FORBIDDEN), "„{method}“: vor jedem Dispatch verboten, nicht {answer}");
    }
    let (status, headers, _) = form_as(server, &s.erika, "/lektorat/act", "work=1").await;
    assert!(
        status == StatusCode::FORBIDDEN && denied_by_role(&headers),
        "/lektorat/act ohne method ist keine erlaubte Aktion"
    );
    assert_eq!(snapshot(&s).await, before, "kein gesperrter Aufruf hat eine Zeile oder einen Commit hinterlassen");

    // The form is no second way to methods outside the table for the author either.
    let form = format!("method=work.delete&work={}&confirm=on", s.granted);
    let (status, _, _) = form_as(server, &s.author, "/lektorat/act", &form).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "/lektorat/act nimmt nur Methoden aus LEKTOR_RPC, von wem auch immer");
    assert_eq!(snapshot(&s).await, before, "und das Werk steht noch");
}

#[tokio::test]
async fn every_method_of_the_table_exists() {
    let s = stage().await;
    for (method, _) in LEKTOR_RPC {
        // Without further parameters: writing methods fail on missing values, reading ones answer.
        let answer = rpc_as(&s.server, &s.author, method, json!({ "work": s.granted })).await;
        assert_ne!(
            code(&answer),
            Some(-32601),
            "{method} steht in LEKTOR_RPC, aber der Server kennt die Methode nicht (umbenannt oder entfernt?)"
        );
        // For the Lektor it is not „verboten“ (what is missing otherwise are only the parameters).
        let answer = rpc_as(&s.server, &s.erika, method, json!({ "work": s.granted })).await;
        assert_ne!(code(&answer), Some(FORBIDDEN), "{method} ist dem Lektor erlaubt: {answer}");
    }
}
