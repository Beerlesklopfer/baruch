// SPDX-License-Identifier: AGPL-3.0-or-later
//! Settings pages „Verbindungen“ and „Quellen“: content for the frame of the settings
//! ([`render_connections`], [`render_sources`], contract Round 9 §3) and their forms.
//!
//! The forms always answer with 303 to `/einstellungen/<seite>?ok=<code>` or
//! `?fehler=<code>[&kind=…]` (with `&work=W`, anchor `#<kind>` or `#quelle-<id>`). The address
//! carries only fixed codes, never inputs or secrets; the page translates them into German text
//! ([`notice_text`], [`error_text`]). This way the frame of the settings draws the page, with and
//! without JavaScript (in the dialog `settings.ts` takes the frame from the response).
//!
//! Round 12 (task “pgvector”): section `#rag` with the ways of the work, the collections per remote way
//! (checkboxes from the last check, repeated field `collection`, plus `collection_new`), the
//! „Status der Wege“ (`search_hooks::rag_status`) and „Index neu aufbauen“ (`rag.reindex` with
//! `confirm=1`, `search_hooks::rag_reindex`). The form arrives as a list of pairs
//! (`Form<Vec<(String, String)>>`) because `collection` occurs several times.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use axum::extract::{Form, State};
use axum::response::{IntoResponse, Redirect, Response};
use baruch_rag::Way;
use baruch_store::{ConnectionKind, RagWay, WorkId};
use minijinja::{Environment, Value as Jinja};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Fault, account, collections, config, describe, open_part, parse_kind, sources, usage_is_stub};
use crate::AppState;

/// Own template environment (autoescape for `.html`).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("settings/verbindungen.html", include_str!("../../templates/settings/verbindungen.html")),
            ("settings/quellen.html", include_str!("../../templates/settings/quellen.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Verbindungen fehlerhaft");
        }
        env
    })
}

fn render(name: &str, ctx: Value) -> Result<String, String> {
    templates().get_template(name).and_then(|t| t.render(Jinja::from(minijinja::value::Serde(&ctx)))).map_err(|e| {
        crate::i18n::tc("connections_msg.page.template", &[("name", name.into()), ("detail", e.to_string().into())])
    })
}

/// Success message for `?ok=`; unknown codes show nothing.
pub fn notice_text(code: &str) -> Option<String> {
    Some(crate::i18n::tc(
        match code {
            "gespeichert" => "connections_msg.ok.gespeichert",
            "geheimnis" => "connections_msg.ok.geheimnis",
            "gespeichert_geheimnis" => "connections_msg.ok.gespeichert_geheimnis",
            "geheimnis_entfernt" => "connections_msg.ok.geheimnis_entfernt",
            "geprueft" => "connections_msg.ok.geprueft",
            "kurztest" => "connections_msg.ok.kurztest",
            "rag" => "connections_msg.ok.rag",
            "sammlungen" => "connections_msg.ok.sammlungen",
            "reindex" => "connections_msg.ok.reindex",
            "angelegt" => "connections_msg.ok.angelegt",
            "geaendert" => "connections_msg.ok.geaendert",
            "verschoben" => "connections_msg.ok.verschoben",
            "geloescht" => "connections_msg.ok.geloescht",
            "bibel_ordner" => "connections_msg.ok.bibel_ordner",
            "bibel_vorgabe" => "connections_msg.ok.bibel_vorgabe",
            "bibel_gewaehlt" => "connections_msg.ok.bibel_gewaehlt",
            "bibel_keine" => "connections_msg.ok.bibel_keine",
            "bibel_schluessel" => "connections_msg.ok.bibel_schluessel",
            "bibel_schluessel_entfernt" => "connections_msg.ok.bibel_schluessel_entfernt",
            _ => return None,
        },
        &[],
    ))
}

/// Error message for `?fehler=`; an unknown code yields a general message.
pub fn error_text(code: &str) -> String {
    crate::i18n::tc(
        match code {
            "kind" => "connections_msg.error.kind",
            "url" => "connections_msg.error.url",
            "url_schema" => "connections_msg.error.url_schema",
            "url_benutzer" => "connections_msg.error.url_benutzer",
            "url_geheimnis" => "connections_msg.error.url_geheimnis",
            "geheimnis_feld" => "connections_msg.error.geheimnis_feld",
            "host" => "connections_msg.error.host",
            "port" => "connections_msg.error.port",
            "tls" => "connections_msg.error.tls",
            "feld" => "connections_msg.error.feld",
            "ordner" => "connections_msg.error.ordner",
            "modell" => "connections_msg.error.modell",
            "praefix" => "connections_msg.error.praefix",
            "geheimnis_leer" => "connections_msg.error.geheimnis_leer",
            "geheimnis_ungueltig" => "connections_msg.error.geheimnis_ungueltig",
            "kein_geheimnis" => "connections_msg.error.kein_geheimnis",
            "bestaetigen" => "connections_msg.error.bestaetigen",
            "schluesselbund" => "connections_msg.error.schluesselbund",
            "gespeichert_ohne_geheimnis" => "connections_msg.error.gespeichert_ohne_geheimnis",
            "werk" => "connections_msg.error.werk",
            "weg" => "connections_msg.error.weg",
            "sammlung" => "connections_msg.error.sammlung",
            "reindex" => "connections_msg.error.reindex",
            "tabelle" => "connections_msg.error.tabelle",
            "spalte" => "connections_msg.error.spalte",
            "titel" => "connections_msg.error.titel",
            "adresse" => "connections_msg.error.adresse",
            "notiz" => "connections_msg.error.notiz",
            "quelle" => "connections_msg.error.quelle",
            "position" => "connections_msg.error.position",
            "methode" => "connections_msg.error.methode",
            "kurztest_modell" => "connections_msg.error.kurztest_modell",
            "intern" => "connections_msg.error.intern",
            "bibel_ordner" => "connections_msg.error.bibel_ordner",
            "bibel_modul" => "connections_msg.error.bibel_modul",
            "bibel_schluessel_falsch" => "connections_msg.error.bibel_schluessel_falsch",
            "bibel_schluessel" => "connections_msg.error.bibel_schluessel",
            _ => "connections_msg.error.other",
        },
        &[],
    )
}

/// Messages from the query for the template.
fn messages(query: &BTreeMap<String, String>) -> (Option<String>, Option<String>) {
    let notice = query.get("ok").and_then(|code| notice_text(code).or_else(|| model_notice_text(code)));
    let error = query.get("fehler").map(|code| error_text(code));
    (notice, error)
}

/// Time `2026-11-06T09:30:12.345Z` (UTC) → `2026-11-06 10:30:12` in local time.
fn shown_time(at: &str) -> String {
    crate::localtime::seconds(at)
}

/// Details of a check as lines (models, tools, versions, collections, index), shortened. Forms
/// of the details per contract Round 12 §5 (pgvector, MultiGPT, LanceDB).
pub fn detail_lines(kind: ConnectionKind, details: &Value) -> Vec<String> {
    let list = |name: &str| -> Vec<String> {
        details
            .get(name)
            .and_then(Value::as_array)
            .map_or_else(Vec::new, |items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
    };
    let short = |items: Vec<String>| -> String {
        const SHOWN: usize = 30;
        let more = items.len().saturating_sub(SHOWN);
        let mut text = items.into_iter().take(SHOWN).collect::<Vec<_>>().join(", ");
        if more > 0 {
            text = crate::i18n::tc("connections_msg.detail.more", &[("list", text.into()), ("more", more.into())]);
        }
        text
    };
    let number = |name: &str| details.get(name).and_then(Value::as_u64);
    let text = |name: &str| details.get(name).and_then(Value::as_str).filter(|t| !t.is_empty());
    // Shared format of the collections (pgvector, MultiGPT): [{name, count}].
    let collections = |unit: &str| -> Option<String> {
        let items: Vec<String> = collections::available_of(details)
            .iter()
            .map(|c| {
                let name = c["name"].as_str().unwrap_or("");
                match c["count"].as_i64() {
                    Some(count) => crate::i18n::tc(
                        "connections_msg.detail.count",
                        &[("name", name.into()), ("count", count.into()), ("unit", unit.into())],
                    ),
                    None => name.to_owned(),
                }
            })
            .collect();
        (!items.is_empty())
            .then(|| crate::i18n::tc("connections_msg.detail.collections", &[("list", short(items).into())]))
    };
    let mut lines = Vec::new();
    match kind {
        ConnectionKind::Eliezer => {
            let models = list("models");
            if !models.is_empty() {
                lines.push(crate::i18n::tc("connections_msg.detail.models", &[("list", short(models).into())]));
            }
        }
        ConnectionKind::Multigpt => {
            let name = details.pointer("/server/name").and_then(Value::as_str).unwrap_or("");
            let version = details.pointer("/server/version").and_then(Value::as_str).unwrap_or("");
            if !name.is_empty() {
                let server = format!("{name} {version}").trim().to_owned();
                lines.push(crate::i18n::tc("connections_msg.detail.server", &[("name", server.into())]));
            }
            let tools = list("tools");
            if !tools.is_empty() {
                lines.push(crate::i18n::tc("connections_msg.detail.tools", &[("list", short(tools).into())]));
            }
            if let Some(pages) = number("pages").filter(|&p| p > 1) {
                lines.push(crate::i18n::tc("connections_msg.detail.tool_pages", &[("pages", pages.into())]));
            }
            lines.extend(collections(&crate::i18n::tc("connections_msg.detail.unit_documents", &[])));
        }
        ConnectionKind::Pgvector => {
            if let Some(version) = text("server_version") {
                lines.push(crate::i18n::tc("connections_msg.detail.server", &[("name", version.into())]));
            }
            if details.get("server_version").is_some() {
                let missing = crate::i18n::tc("connections_msg.detail.vector_missing", &[]);
                let vector = text("vector").map_or(missing, str::to_owned);
                lines.push(crate::i18n::tc("connections_msg.detail.vector", &[("vector", vector.into())]));
            }
            if let Some(table) = text("table") {
                let columns: Vec<String> = details
                    .get("columns")
                    .and_then(Value::as_object)
                    .map(|map| map.iter().filter_map(|(role, c)| c.as_str().map(|c| format!("{role} = {c}"))).collect())
                    .unwrap_or_default();
                if columns.is_empty() {
                    lines.push(crate::i18n::tc("connections_msg.detail.table", &[("table", table.into())]));
                } else {
                    lines.push(crate::i18n::tc(
                        "connections_msg.detail.table_columns",
                        &[("table", table.into()), ("columns", columns.join(", ").into())],
                    ));
                }
            }
            if let Some(chunks) = number("chunks") {
                lines.push(crate::i18n::tc("connections_msg.detail.chunks", &[("count", chunks.into())]));
            }
            if let Some(dimension) = number("dimension") {
                lines.push(crate::i18n::tc("connections_msg.detail.dimension", &[("count", dimension.into())]));
            }
            let models = list("models");
            if !models.is_empty() {
                lines.push(crate::i18n::tc("connections_msg.detail.data_models", &[("list", short(models).into())]));
            }
            lines.extend(collections(&crate::i18n::tc("connections_msg.detail.unit_sections", &[])));
        }
        ConnectionKind::Lancedb => {
            if let Some(dir) = text("dir") {
                let exists = details.get("exists").and_then(Value::as_bool);
                let writable = details.get("writable").and_then(Value::as_bool);
                let key = match (exists, writable) {
                    (Some(false), _) => "connections_msg.detail.dir_missing",
                    (_, Some(false)) => "connections_msg.detail.dir_readonly",
                    (Some(true), Some(true)) => "connections_msg.detail.dir_writable",
                    _ => "connections_msg.detail.dir",
                };
                lines.push(crate::i18n::tc(key, &[("dir", dir.into())]));
            }
            let tables = list("tables");
            if !tables.is_empty() {
                lines.push(crate::i18n::tc("connections_msg.detail.tables", &[("list", short(tables).into())]));
            }
            if let Some(entries) = number("entries") {
                lines.push(crate::i18n::tc("connections_msg.detail.entries", &[("count", entries.into())]));
            }
            match (text("model"), number("dimension")) {
                (Some(model), Some(dimension)) => lines.push(crate::i18n::tc(
                    "connections_msg.detail.index_dimension",
                    &[("model", model.into()), ("dimension", dimension.into())],
                )),
                (Some(model), None) => {
                    lines.push(crate::i18n::tc("connections_msg.detail.index", &[("model", model.into())]));
                }
                _ => {}
            }
            if let Some(configured) = text("configured_model") {
                lines.push(crate::i18n::tc("connections_msg.detail.configured_model", &[("model", configured.into())]));
            }
            match text("embedder") {
                Some("ok") => lines.push(crate::i18n::tc("connections_msg.detail.embedder_ok", &[])),
                Some(why) => lines.push(crate::i18n::tc("connections_msg.detail.embedder", &[("detail", why.into())])),
                None => {}
            }
        }
    }
    lines
}

/// Input fields per kind for the template: `{name, label, value, type, options?, hint?}`.
fn form_fields(kind: ConnectionKind, config: &Value) -> Vec<Value> {
    // Fields not yet stored (mapping of pgvector, Round 12) show their default.
    let config = &config::effective(kind, config);
    let value = |name: &str| match config.get(name) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    config::fields(kind)
        .iter()
        .map(|name| {
            // Label and hint are catalog keys (an empty hint stays empty).
            let (label, kind_of, hint) = match *name {
                "url" => ("connections_msg.form.url", "url", "connections_msg.form.url_hint"),
                "host" => ("connections_msg.form.host", "text", "connections_msg.form.host_hint"),
                "port" => ("connections_msg.form.port", "number", ""),
                "database" => ("connections_msg.form.database", "text", "connections_msg.form.database_hint"),
                "user" => ("connections_msg.form.user", "text", ""),
                "tls" => ("connections_msg.form.tls", "select", ""),
                "dir" => ("connections_msg.form.dir", "text", "connections_msg.form.dir_hint"),
                "embedding_model" if kind == ConnectionKind::Pgvector => {
                    ("connections_msg.form.embedding_model", "text", "connections_msg.form.embedding_model_hint")
                }
                "embedding_model" => ("connections_msg.form.embedding_model", "text", ""),
                "table" => ("connections_msg.form.table", "text", "connections_msg.form.table_hint"),
                "text_column" => ("connections_msg.form.text_column", "text", ""),
                "vector_column" => {
                    ("connections_msg.form.vector_column", "text", "connections_msg.form.vector_column_hint")
                }
                "collection_column" => ("connections_msg.form.collection_column", "text", ""),
                "document_column" => ("connections_msg.form.document_column", "text", ""),
                "section_column" => {
                    ("connections_msg.form.section_column", "text", "connections_msg.form.optional_hint")
                }
                "title_column" => ("connections_msg.form.title_column", "text", "connections_msg.form.optional_hint"),
                "url_column" => ("connections_msg.form.url_column", "text", "connections_msg.form.optional_hint"),
                "model_column" => {
                    ("connections_msg.form.model_column", "text", "connections_msg.form.model_column_hint")
                }
                // Round 19b: task prefixes (suggestion for nomic-embed-text in the placeholder and in the hint).
                "query_prefix" if kind == ConnectionKind::Pgvector => {
                    ("connections_msg.form.query_prefix", "text", "connections_msg.form.query_prefix_pg_hint")
                }
                "query_prefix" => {
                    ("connections_msg.form.query_prefix", "text", "connections_msg.form.query_prefix_hint")
                }
                "document_prefix" => {
                    ("connections_msg.form.document_prefix", "text", "connections_msg.form.document_prefix_hint")
                }
                _ => ("connections_msg.form.other", "text", ""),
            };
            let label = crate::i18n::tc(label, &[]);
            let hint = if hint.is_empty() { String::new() } else { crate::i18n::tc(hint, &[]) };
            let required = match kind {
                ConnectionKind::Pgvector => {
                    *name == "embedding_model" || *name == "table" || config::PG_REQUIRED_COLUMNS.contains(name)
                }
                _ => matches!(*name, "url" | "dir" | "embedding_model"),
            };
            let mut field = json!({ "name": name, "label": label, "value": value(name), "type": kind_of,
                                    "hint": hint, "required": required });
            if *name == "tls" {
                field["options"] = json!(config::TLS_MODES);
            }
            match *name {
                "query_prefix" => {
                    field["placeholder"] = json!(config::NOMIC_PREFIXES.0);
                    field["maxlength"] = json!(baruch_rag::MAX_PREFIX_CHARS);
                }
                "document_prefix" => {
                    field["placeholder"] = json!(config::NOMIC_PREFIXES.1);
                    field["maxlength"] = json!(baruch_rag::MAX_PREFIX_CHARS);
                }
                _ => {}
            }
            field
        })
        .collect()
}

/// Model the quick test suggests if Eliezer knows it (trial 2026-11-06: all personas on Gemma).
const PROBE_DEFAULT: &str = "google/gemma-4-26b-a4b";

/// Quick test for Eliezer: selection from the model list of the last check and the models of the active
/// personas (without asking Eliezer – the page never asks itself), plus the last result.
async fn probe_context(state: &AppState, row: &baruch_store::ConnectionRow) -> Result<Value, String> {
    let mut options: Vec<String> = row
        .check
        .as_ref()
        .filter(|c| c.ok)
        .and_then(|c| c.details.get("models"))
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let listed = !options.is_empty();
    for persona in state.store.persona_list().await.map_err(|e| e.to_string())? {
        if persona.active && !options.contains(&persona.model) {
            options.push(persona.model);
        }
    }
    let selected = if options.iter().any(|m| m == PROBE_DEFAULT) {
        PROBE_DEFAULT.to_owned()
    } else {
        options.first().cloned().unwrap_or_default()
    };
    let last = crate::models::last_probe(state).await.map(|mut p| {
        if let Some(at) = p.get("at").and_then(Value::as_str) {
            p["at"] = json!(shown_time(at));
        }
        p
    });
    Ok(json!({ "options": options, "selected": selected, "listed": listed, "last": last }))
}

/// Page „Verbindungen“ as content for the frame of the settings: per connection settings,
/// state of the secret, last check and STUB note; plus the RAG ways of the work.
pub fn render_connections<'a>(
    state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(async move {
        let (notice, error) = messages(query);
        let error_kind = query.get("kind").and_then(|k| parse_kind(k).ok());
        let rows = state.store.connection_list().await.map_err(|e| e.to_string())?;
        let mut connections = Vec::new();
        for row in &rows {
            let described = describe(state, row).await;
            let check = row.check.as_ref().map(|c| {
                json!({
                    "ok": c.ok, "message": c.message, "at": shown_time(&c.at),
                    "lines": detail_lines(row.kind, &c.details),
                    "personas": c.details.get("personas").and_then(Value::as_array).cloned().unwrap_or_default(),
                })
            });
            let probe = if row.kind == ConnectionKind::Eliezer { Some(probe_context(state, row).await?) } else { None };
            // Round 19b: note for a stored password and TLS prefer/disable (pgvector only).
            let tls_warning = (row.kind == ConnectionKind::Pgvector)
                .then(|| super::pgvector::tls_warning(&row.config, described["secret"] == json!("hinterlegt")))
                .flatten();
            connections.push(json!({
                "tls_warning": tls_warning,
                "kind": row.kind, "title": described["title"], "account": account(row.kind),
                "secret": described["secret"], "secret_error": described.get("secret_error"),
                "secret_for": described.get("secret_for"),
                "fields": form_fields(row.kind, &row.config), "check": check,
                "checkable": true, "stub": usage_text(row.kind),
                "stub_open": usage_is_stub(row.kind), "open_part": open_part(row.kind), "probe": probe,
                "default": row.updated.is_none(),
                "error": if error_kind == Some(row.kind) { error.clone() } else { None },
            }));
        }
        let rag = match work {
            Some(work) => match state.store.work_rag(work).await {
                Ok((ways, default)) => {
                    let title = state.store.work_meta(work).await.map(|m| m.title).unwrap_or_default();
                    let chosen = ways.clone();
                    let ways: Vec<Value> = RagWay::ALL
                        .into_iter()
                        .map(|w| {
                            let label = match w {
                                RagWay::Lancedb => "LanceDB (lokal)",
                                RagWay::Pgvector => "pgvector",
                                RagWay::Multigpt => "MultiGPT",
                            };
                            json!({ "name": w.as_str(), "label": label, "checked": ways.contains(&w) })
                        })
                        .collect();
                    let mut sets = Vec::new();
                    for way in [RagWay::Pgvector, RagWay::Multigpt] {
                        let found = collections::get(state, work, way).await.map_err(|f| format!("{f:?}"))?;
                        sets.push(collection_context(way, chosen.contains(&way), &found));
                    }
                    let status = status_context(crate::search_hooks::rag_status(state, work).await);
                    Some(json!({ "work": work, "title": title, "ways": ways, "default": default,
                                 "collections": sets, "status": status }))
                }
                Err(baruch_store::StoreError::UnknownWork(_)) => None,
                Err(e) => return Err(e.to_string()),
            },
            None => None,
        };
        let models = models_context(state, query).await?;
        render(
            "settings/verbindungen.html",
            json!({ "notice": notice, "error": error, "connections": connections, "rag": rag, "work": work,
                    "models": models }),
        )
    })
}

/// Model connections for the template (Round 15b): per connection details from `models::describe`, last
/// check and last quick test in local time, models of the last check as the selection of the quick test.
async fn models_context(state: &AppState, query: &BTreeMap<String, String>) -> Result<Value, String> {
    let mut list = Vec::new();
    for connection in state.store.model_connection_list().await.map_err(|e| e.to_string())? {
        let mut m = crate::models::describe(state, &connection).await;
        let options: Vec<String> = connection
            .check
            .as_ref()
            .filter(|c| c.get("ok") == Some(&json!(true)))
            .and_then(|c| c.pointer("/details/models"))
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default();
        if let Some(check) = connection.check.as_ref() {
            let contexts: Vec<String> = check
                .pointer("/details/contexts")
                .and_then(Value::as_array)
                .map(|l| {
                    l.iter()
                        .filter_map(|m| {
                            let id = m.get("id")?.as_str()?;
                            let n = m.get("context_length")?.as_u64()?;
                            Some(crate::i18n::tc(
                                "connections_msg.detail.tokens",
                                &[("id", id.into()), ("count", n.into())],
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let lines = if contexts.is_empty() {
                Vec::new()
            } else {
                vec![crate::i18n::tc("connections_msg.detail.contexts", &[("list", contexts.join(", ").into())])]
            };
            m["check"] = json!({
                "ok": check.get("ok") == Some(&json!(true)),
                "message": check.get("message").and_then(Value::as_str).unwrap_or(""),
                "at": shown_time(check.get("at").and_then(Value::as_str).unwrap_or("")),
                "lines": lines,
            });
        }
        if let Some(probe) = connection.probe.as_ref() {
            let mut p = probe.clone();
            if let Some(at) = probe.get("at").and_then(Value::as_str) {
                p["at"] = json!(shown_time(at));
            }
            m["probe"] = p;
        }
        m["options"] = json!(options);
        list.push(m);
    }
    let kinds: Vec<Value> = baruch_llm::BackendKind::ALL
        .iter()
        .map(|k| json!({ "kind": k.as_str(), "title": crate::models::kind_title(k.as_str()) }))
        .collect();
    let error = query.get("modellfehler").map(|code| model_error_text(code));
    Ok(json!({ "connections": list, "kinds": kinds, "error": error }))
}

/// Success message of the model connections for `?ok=`.
pub fn model_notice_text(code: &str) -> Option<String> {
    Some(crate::i18n::tc(
        match code {
            "modell_gespeichert" => "connections_msg.model_ok.modell_gespeichert",
            "modell_angelegt" => "connections_msg.model_ok.modell_angelegt",
            "modell_geloescht" => "connections_msg.model_ok.modell_geloescht",
            "modell_schluessel" => "connections_msg.model_ok.modell_schluessel",
            "modell_schluessel_entfernt" => "connections_msg.model_ok.modell_schluessel_entfernt",
            "modell_geprueft" => "connections_msg.model_ok.modell_geprueft",
            "modell_kurztest" => "connections_msg.model_ok.modell_kurztest",
            _ => return None,
        },
        &[],
    ))
}

/// Error message of the model connections for `?modellfehler=`.
pub fn model_error_text(code: &str) -> String {
    crate::i18n::tc(
        match code {
            "name" => "connections_msg.model_error.name",
            "art" => "connections_msg.model_error.art",
            "adresse" => "connections_msg.model_error.adresse",
            "verbindung" => "connections_msg.model_error.verbindung",
            "eliezer" => "connections_msg.model_error.eliezer",
            "schluessel" => "connections_msg.model_error.schluessel",
            "schluesselbund" => "connections_msg.model_error.schluesselbund",
            "bestaetigen" => "connections_msg.model_error.bestaetigen",
            "modell" => "connections_msg.model_error.modell",
            "aktion" => "connections_msg.model_error.aktion",
            _ => "connections_msg.model_error.other",
        },
        &[],
    )
}

/// `POST /einstellungen/werk/bibeln` (Round 15, behind `guard::same_origin`): `method=bible.set_work_sources`,
/// `work`, repeated fields `commentary` and `lexicon` (checkboxes of the page Einstellungen → Werk). 303 to
/// `/einstellungen/werk?work=W&ok=kommentare#kommentare` or `…&kommentarfehler=1#kommentare`.
pub(crate) async fn work_bibles_act(
    State(state): State<Arc<AppState>>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let field = |name: &str| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim().to_owned());
    let values = |name: &str| -> Vec<String> {
        pairs.iter().filter(|(k, _)| k == name).map(|(_, v)| v.trim().to_owned()).filter(|v| !v.is_empty()).collect()
    };
    let work = match work_of(field("work").as_deref()) {
        Some(w) => w,
        None => match state.default_work().await {
            Ok(w) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
    };
    if field("method").as_deref().is_some_and(|m| m != "bible.set_work_sources") {
        return Redirect::to(&format!("/einstellungen/werk?work={work}&kommentarfehler=1#kommentare")).into_response();
    }
    let done = crate::bibles::set_work_sources(&state, work, &values("commentary"), &values("lexicon")).await;
    let to = match done {
        Ok(_) => format!("/einstellungen/werk?work={work}&ok=kommentare#kommentare"),
        Err(_) => format!("/einstellungen/werk?work={work}&kommentarfehler=1#kommentare"),
    };
    Redirect::to(&to).into_response()
}

/// Fields of `POST /einstellungen/modelle/act`.
#[derive(Deserialize, Default)]
pub(crate) struct ModelForm {
    action: Option<String>,
    id: Option<String>,
    name: Option<String>,
    kind: Option<String>,
    url: Option<String>,
    external: Option<String>,
    secret: Option<String>,
    confirm: Option<String>,
    model: Option<String>,
    work: Option<String>,
}

/// What a connection is used for ([`super::usage_stub`], German) in the language of the request.
fn usage_text(kind: ConnectionKind) -> String {
    let key = match kind {
        ConnectionKind::Eliezer => "connections_msg.usage.eliezer",
        ConnectionKind::Multigpt => "connections_msg.usage.multigpt",
        ConnectionKind::Pgvector => "connections_msg.usage.pgvector",
        ConnectionKind::Lancedb => "connections_msg.usage.lancedb",
    };
    crate::i18n::tc(key, &[])
}

/// How the web search uses the list of sources (page „Quellen“, `data-usage`), in the language of the request.
fn sources_usage_text() -> String {
    let site = match crate::i18n::current() {
        crate::i18n::Lang::De => "site:<Rechner>",
        crate::i18n::Lang::En => "site:<host>",
    };
    crate::i18n::tc("connections_msg.sources.usage", &[("site", site.into())])
}

/// Error code from an RPC response of the model connections (fixed codes, never inputs in the address). The message
/// is in the language of the request, so both the German and the English wording count.
fn model_fault(e: &crate::rpc::RpcError) -> &'static str {
    let m = e.message.as_str();
    if m.contains("Schlüsselbund") || m.contains("Keyring") {
        "schluesselbund"
    } else if m.contains("Eliezer") {
        "eliezer"
    } else if m.contains("Adresse") || m.contains("Address") || m.starts_with("url") {
        "adresse"
    } else if m.contains("Art") || m.contains("Kind") || m.starts_with("kind") {
        "art"
    } else if m.contains("Name") || m.contains(" name") {
        "name"
    } else if m.contains("secret") {
        "schluessel"
    } else if m.contains("gibt es nicht") || m.contains("does not exist") {
        "verbindung"
    } else {
        "unbekannt"
    }
}

/// `POST /einstellungen/modelle/act` (behind `guard::same_origin`): `action` ∈ `save` (without `id` create;
/// `name`, `kind`, `url`, `external=1|0|auto` – `auto` by the address), `secret` (`id`, `secret`), `delete_secret`
/// (`id`, `confirm=1`),
/// `delete` (`id`, `confirm=1`), `check` (`id`), `probe` (`id`, `model`). Always 303 to
/// `/einstellungen/verbindungen?ok=…#modell-<id>` or `?modellfehler=<code>#modelle`.
pub(crate) async fn models_act(State(state): State<Arc<AppState>>, Form(form): Form<ModelForm>) -> Response {
    let work = work_of(form.work.as_deref());
    let id = form.id.as_deref().map(str::trim).filter(|i| !i.is_empty()).and_then(|i| i.parse::<i64>().ok());
    let result: Result<(&str, String), &'static str> = async {
        let need = || id.ok_or("verbindung");
        match form.action.as_deref().unwrap_or("") {
            "save" => {
                let mut params = json!({
                    "name": form.name.as_deref().unwrap_or(""), "kind": form.kind.as_deref().unwrap_or(""),
                });
                // `auto` (creating, default): location by the address (`models::looks_local`); otherwise explicit
                // (checkbox when changing: unchecked = local).
                if form.external.as_deref().map(str::trim) != Some("auto") {
                    params["external"] = json!(checked(&form.external));
                }
                if let Some(id) = id {
                    params["id"] = json!(id);
                }
                if let Some(url) = form.url.as_deref() {
                    params["url"] = json!(url);
                }
                let row = crate::models::set_connection(&state, &params).await.map_err(|e| model_fault(&e))?;
                Ok((if id.is_some() { "modell_gespeichert" } else { "modell_angelegt" }, format!("modell-{}", row.id)))
            }
            "secret" => {
                let id = need()?;
                let secret = form.secret.as_deref().map(str::trim).unwrap_or("");
                if secret.is_empty() || secret.chars().count() > 4096 || secret.chars().any(char::is_control) {
                    return Err("schluessel");
                }
                crate::models::set_secret(&state, id, secret).await.map_err(|e| model_fault(&e))?;
                Ok(("modell_schluessel", format!("modell-{id}")))
            }
            "delete_secret" => {
                let id = need()?;
                if !checked(&form.confirm) {
                    return Err("bestaetigen");
                }
                crate::models::delete_secret(&state, id).await.map_err(|e| model_fault(&e))?;
                Ok(("modell_schluessel_entfernt", format!("modell-{id}")))
            }
            "delete" => {
                let id = need()?;
                if !checked(&form.confirm) {
                    return Err("bestaetigen");
                }
                crate::models::delete_connection(&state, id).await.map_err(|e| model_fault(&e))?;
                Ok(("modell_geloescht", "modelle".to_owned()))
            }
            "check" => {
                let id = need()?;
                crate::models::check_connection(&state, id).await.map_err(|e| model_fault(&e))?;
                Ok(("modell_geprueft", format!("modell-{id}")))
            }
            "probe" => {
                let id = need()?;
                let model = form.model.as_deref().map(str::trim).unwrap_or("");
                if model.is_empty() || model.chars().count() > 200 || model.chars().any(char::is_control) {
                    return Err("modell");
                }
                state.store.model_connection_get(id).await.map_err(|_| "verbindung")?;
                crate::models::probe_on(&state, id, model).await;
                Ok(("modell_kurztest", format!("modell-{id}")))
            }
            _ => Err("aktion"),
        }
    }
    .await;
    let work_q = work.map_or(String::new(), |w| format!("&work={w}"));
    let url = match result {
        Ok((ok, anchor)) => format!("/einstellungen/verbindungen?ok={ok}{work_q}#{anchor}"),
        Err(code) => format!("/einstellungen/verbindungen?modellfehler={code}{work_q}#modelle"),
    };
    Redirect::to(&url).into_response()
}

/// Collections of a remote way for the template (from `collections::get`): checkboxes from the last
/// check (`available`) and the chosen ones it does not report (any more) (`gone`, only if there was a
/// check), plus the time of the check.
pub fn collection_context(way: RagWay, way_on: bool, found: &Value) -> Value {
    let chosen: Vec<String> = found["collections"]
        .as_array()
        .map(|l| l.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let checked_at = found["available_at"].as_str().map(shown_time);
    let mut options: Vec<Value> = found["available"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|c| {
                    let name = c["name"].as_str().unwrap_or("");
                    json!({ "name": name, "count": c["count"], "checked": chosen.iter().any(|x| x == name),
                            "gone": false })
                })
                .collect()
        })
        .unwrap_or_default();
    for name in &chosen {
        if !options.iter().any(|o| o["name"].as_str() == Some(name.as_str())) {
            options.push(json!({ "name": name, "count": null, "checked": true, "gone": checked_at.is_some() }));
        }
    }
    let title = match way {
        RagWay::Multigpt => Way::Multigpt.title(),
        _ => Way::Pgvector.title(),
    };
    json!({
        "way": way.as_str(), "label": title, "on": way_on, "options": options,
        "checked_at": checked_at, "chosen": chosen.len(),
        "unit": if way == RagWay::Multigpt { "Dokumente" } else { "Abschnitte" },
    })
}

/// „Status der Wege“ for the template: per way state and message, the local index – or the reason
/// why there is no status.
pub fn status_context(status: Result<baruch_rag::Status, baruch_rag::RagError>) -> Value {
    match status {
        Err(e) => json!({ "ok": false, "message": e.message() }),
        Ok(status) => {
            let ways: Vec<Value> = status
                .ways
                .iter()
                .map(|w| {
                    json!({ "way": w.way.as_str(), "title": w.way.title(), "state": w.state, "label": w.label,
                            "message": w.message })
                })
                .collect();
            let i = &status.index;
            json!({
                "ok": true, "ways": ways, "fts": status.fts,
                "index": {
                    "dir": i.dir, "entries": i.entries, "model": i.model, "dimension": i.dimension,
                    "configured_model": i.configured_model, "built": i.built.as_deref().map(shown_time),
                    "pending": i.pending, "stale": i.stale, "problem": i.problem,
                },
            })
        }
    }
}

/// Page „Quellen“ as content for the frame: list with edit, move, delete and create.
pub fn render_sources<'a>(
    state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(async move {
        let (notice, error) = messages(query);
        let list = state.store.sources_list().await.map_err(|e| e.to_string())?;
        let count = list.len();
        let bibles = bible_context(state, work).await.map_err(|e| e.message)?;
        render(
            "settings/quellen.html",
            json!({ "notice": notice, "error": error, "sources": list, "count": count, "work": work,
                    "usage": sources_usage_text(), "bibles": bibles }),
        )
    })
}

/// Section „Bibeln (SWORD)“: module folders, Bibles found and the choice of the work (`work` or default work).
async fn bible_context(state: &AppState, work: Option<WorkId>) -> Result<Value, crate::rpc::RpcError> {
    let work = match work {
        Some(w) => w,
        None => state.default_work().await?,
    };
    let title = state.store.load_work(work).await.map(|w| w.title).unwrap_or_default();
    let mut value = crate::bibles::modules_value(state, work).await?;
    let dirs: Vec<String> =
        value["dirs"].as_array().into_iter().flatten().filter_map(|d| d.as_str().map(str::to_owned)).collect();
    value["dirs_text"] = json!(dirs.join("\n"));
    value["work_title"] = json!(title);
    let chosen = value["work_bible"].as_str().map(str::to_owned);
    let known = chosen.as_ref().is_some_and(|c| {
        value["modules"].as_array().into_iter().flatten().any(|m| m["module"].as_str() == Some(c.as_str()))
    });
    value["chosen_missing"] = json!(chosen.is_some() && !known);
    // R17c: are there encrypted modules? Then the section „Schlüssel verschlüsselter Module“ is shown.
    let sealed =
        value["modules"].as_array().into_iter().flatten().any(|m| m["cipher"].as_str().is_some_and(|c| c != "none"));
    value["sealed"] = json!(sealed);
    Ok(value)
}

// ---- Forms ---------------------------------------------------------------------------------------

/// Fields of `POST /einstellungen/verbindungen/act` (without `collection`, which occurs several times).
#[derive(Deserialize, Default)]
pub(crate) struct ConnectionForm {
    method: Option<String>,
    kind: Option<String>,
    work: Option<String>,
    url: Option<String>,
    host: Option<String>,
    port: Option<String>,
    database: Option<String>,
    user: Option<String>,
    tls: Option<String>,
    dir: Option<String>,
    embedding_model: Option<String>,
    table: Option<String>,
    text_column: Option<String>,
    vector_column: Option<String>,
    collection_column: Option<String>,
    document_column: Option<String>,
    section_column: Option<String>,
    title_column: Option<String>,
    url_column: Option<String>,
    model_column: Option<String>,
    /// Task prefixes (Round 19b; not trimmed).
    query_prefix: Option<String>,
    document_prefix: Option<String>,
    secret: Option<String>,
    confirm: Option<String>,
    /// Model for the quick test (`model.probe`, Eliezer only).
    model: Option<String>,
    way_lancedb: Option<String>,
    way_pgvector: Option<String>,
    way_multigpt: Option<String>,
    /// Way of the collections (`connection.collections_set`).
    way: Option<String>,
    /// New collection to add (one line).
    collection_new: Option<String>,
}

impl ConnectionForm {
    /// From the pairs of the form; for duplicate fields the last one counts (except `collection`, which
    /// [`collections_of`] collects).
    fn from_pairs(pairs: &[(String, String)]) -> ConnectionForm {
        let map: serde_json::Map<String, Value> = pairs
            .iter()
            .filter(|(name, _)| name != "collection")
            .map(|(name, value)| (name.clone(), Value::from(value.as_str())))
            .collect();
        serde_json::from_value(Value::Object(map)).unwrap_or_default()
    }
}

/// Chosen collections: every field `collection` in the order of the form, plus `collection_new`
/// if it is not empty (trimming and checking happen in the store).
fn collections_of(pairs: &[(String, String)], new: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> =
        pairs.iter().filter(|(name, _)| name == "collection").map(|(_, value)| value.clone()).collect();
    if let Some(new) = new.filter(|n| !n.trim().is_empty()) {
        out.push(new.to_owned());
    }
    out
}

fn checked(value: &Option<String>) -> bool {
    matches!(value.as_deref(), Some("1" | "on"))
}

fn work_of(form: Option<&str>) -> Option<WorkId> {
    form.and_then(|w| w.trim().parse::<WorkId>().ok()).filter(|&w| w > 0)
}

/// Way back: only fixed codes, work id as a number and a fixed anchor.
fn back(
    page: &str,
    result: Result<(&str, String), (&'static str, Option<ConnectionKind>)>,
    work: Option<WorkId>,
) -> Response {
    let work = work.map_or(String::new(), |w| format!("&work={w}"));
    let url = match result {
        Ok((ok, anchor)) => format!("/einstellungen/{page}?ok={ok}{work}#{anchor}"),
        Err((code, kind)) => {
            let kind = kind.map_or(String::new(), |k| format!("&kind={k}"));
            format!("/einstellungen/{page}?fehler={code}{kind}{work}")
        }
    };
    Redirect::to(&url).into_response()
}

/// `POST /einstellungen/verbindungen/act`: `method` ∈ `connection.set` (fields of the kind, optionally with
/// `secret`), `connection.set_secret`, `connection.delete_secret` (`confirm=1`), `connection.check`,
/// `connection.rag_set` (`work`, `way_*=1`), `model.probe` (only `kind=eliezer`, field `model`: quick test,
/// result on the page); since Round 12 `connection.collections_set` (`work`, `way`, repeated field
/// `collection`, plus `collection_new`) and `rag.reindex` (`work`, `confirm=1`). Always 303.
pub(crate) async fn connections_act(
    State(state): State<Arc<AppState>>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let form = ConnectionForm::from_pairs(&pairs);
    let work = work_of(form.work.as_deref());
    let method = form.method.as_deref().unwrap_or("");
    if method == "connection.collections_set" {
        let Some(target) = work else { return back("verbindungen", Err(("werk", None)), None) };
        let chosen = collections_of(&pairs, form.collection_new.as_deref());
        let result = match collections::remote_way(form.way.as_deref().unwrap_or("")) {
            Ok(way) => collections::set(&state, target, way, &chosen).await.map(|_| ()),
            Err(fault) => Err(fault),
        };
        return match result {
            Ok(()) => back("verbindungen", Ok(("sammlungen", "rag".into())), work),
            Err(fault) => back("verbindungen", Err((fault.code(), None)), work),
        };
    }
    if method == "rag.reindex" {
        let Some(target) = work else { return back("verbindungen", Err(("werk", None)), None) };
        if !checked(&form.confirm) {
            return back("verbindungen", Err(("bestaetigen", None)), work);
        }
        // Only the human through this form (no MCP tool, not through POST /act).
        return match crate::search_hooks::rag_reindex(&state, target).await {
            Ok(_) => back("verbindungen", Ok(("reindex", "rag".into())), work),
            Err(_) => back("verbindungen", Err(("reindex", None)), work),
        };
    }
    if method == "connection.rag_set" {
        let ways: Vec<RagWay> = [
            (RagWay::Lancedb, &form.way_lancedb),
            (RagWay::Pgvector, &form.way_pgvector),
            (RagWay::Multigpt, &form.way_multigpt),
        ]
        .into_iter()
        .filter(|(_, on)| checked(on))
        .map(|(way, _)| way)
        .collect();
        let Some(target) = work else { return back("verbindungen", Err(("werk", None)), None) };
        return match super::rag_set(&state, target, &ways).await {
            Ok(_) => back("verbindungen", Ok(("rag", "rag".into())), work),
            Err(fault) => back("verbindungen", Err((fault.code(), None)), work),
        };
    }
    let kind = match form.kind.as_deref().map(parse_kind) {
        Some(Ok(kind)) => kind,
        _ => return back("verbindungen", Err(("kind", None)), work),
    };
    let secret = form.secret.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let result: Result<&str, Fault> = match method {
        "connection.set" => {
            let given: serde_json::Map<String, Value> = [
                ("url", &form.url),
                ("host", &form.host),
                ("port", &form.port),
                ("database", &form.database),
                ("user", &form.user),
                ("tls", &form.tls),
                ("dir", &form.dir),
                ("embedding_model", &form.embedding_model),
                ("table", &form.table),
                ("text_column", &form.text_column),
                ("vector_column", &form.vector_column),
                ("collection_column", &form.collection_column),
                ("document_column", &form.document_column),
                ("section_column", &form.section_column),
                ("title_column", &form.title_column),
                ("url_column", &form.url_column),
                ("model_column", &form.model_column),
                ("query_prefix", &form.query_prefix),
                ("document_prefix", &form.document_prefix),
            ]
            .into_iter()
            .filter(|(name, _)| config::fields(kind).contains(name))
            .filter_map(|(name, value)| value.as_ref().map(|v| (name.to_owned(), Value::from(v.as_str()))))
            .collect();
            // First check everything (secret like settings), then write: an invalid secret
            // must not show up only when the settings are already stored (“nothing changed”
            // must be true). The secret comes after the settings so that it is valid for the new target.
            match secret.map(|s| super::valid_secret(kind, s)).transpose() {
                Err(fault) => Err(fault),
                Ok(secret) => match super::set_config(&state, kind, &Value::Object(given)).await {
                    Err(fault) => Err(fault),
                    Ok(_) => match secret {
                        Some(secret) => match super::set_secret(&state, kind, secret).await {
                            Ok(()) => Ok("gespeichert_geheimnis"),
                            // The settings are already stored; the message says so honestly.
                            Err(Fault::Keyring(_)) => Err(Fault::Invalid("gespeichert_ohne_geheimnis", String::new())),
                            Err(fault) => Err(fault),
                        },
                        None => Ok("gespeichert"),
                    },
                },
            }
        }
        "connection.set_secret" => {
            super::set_secret(&state, kind, form.secret.as_deref().unwrap_or("")).await.map(|()| "geheimnis")
        }
        "connection.delete_secret" if !checked(&form.confirm) => Err(Fault::Invalid("bestaetigen", String::new())),
        "connection.delete_secret" => super::delete_secret(&state, kind).await.map(|()| "geheimnis_entfernt"),
        "connection.check" => super::run_check(&state, kind).await.map(|_| "geprueft"),
        "model.probe" if kind != ConnectionKind::Eliezer => Err(Fault::Invalid("methode", String::new())),
        "model.probe" => {
            let model = form.model.as_deref().map(str::trim).unwrap_or("");
            if model.is_empty() || model.chars().count() > 200 || model.chars().any(char::is_control) {
                Err(Fault::Invalid("kurztest_modell", String::new()))
            } else {
                // A failure (not reachable, unknown model …) is a result on the page.
                crate::models::probe(&state, model).await;
                Ok("kurztest")
            }
        }
        _ => Err(Fault::Invalid("methode", String::new())),
    };
    match result {
        Ok(ok) => back("verbindungen", Ok((ok, kind.as_str().to_owned())), work),
        Err(fault) => back("verbindungen", Err((fault.code(), Some(kind))), work),
    }
}

/// Fields of `POST /einstellungen/quellen/act`.
#[derive(Deserialize, Default)]
pub(crate) struct SourceForm {
    method: Option<String>,
    id: Option<String>,
    title: Option<String>,
    url: Option<String>,
    note: Option<String>,
    position: Option<String>,
    direction: Option<String>,
    confirm: Option<String>,
    work: Option<String>,
    /// Module folders of the Bibles, one per line (`bible.set_dirs`).
    dirs: Option<String>,
    /// `reset=1`: module folders back to the default.
    reset: Option<String>,
    /// Chosen Bible (`bible.set_work`; empty = none), or module for `bible.cipher_key_*`.
    module: Option<String>,
    /// Key of an encrypted module (`bible.cipher_key_set`, R17c); goes only into the keyring.
    key: Option<String>,
}

/// `POST /einstellungen/quellen/act`: `method` ∈ `sources.create` (title, url, note),
/// `sources.update` (id, title, url, note), `sources.move` (id, `direction=up|down` or `position`),
/// `sources.delete` (id, `confirm=1`), `bible.set_dirs` (`dirs` one folder per line, `reset=1` = default),
/// `bible.set_work` (`work`, `module`, empty = no Bible), `bible.cipher_key_set` (`module`, `key`; R17c) and
/// `bible.cipher_key_delete` (`module`, `confirm=1`). Always 303.
pub(crate) async fn sources_act(State(state): State<Arc<AppState>>, Form(form): Form<SourceForm>) -> Response {
    let work = work_of(form.work.as_deref());
    let id = form.id.as_deref().and_then(|id| id.trim().parse::<i64>().ok());
    let need_id = || id.ok_or(Fault::Invalid("quelle", String::new()));
    let result: Result<(&str, String), Fault> = async {
        match form.method.as_deref().unwrap_or("") {
            "sources.create" => {
                let created = sources::create(
                    &state,
                    form.title.as_deref().unwrap_or(""),
                    form.url.as_deref().unwrap_or(""),
                    form.note.as_deref().unwrap_or(""),
                )
                .await?;
                Ok(("angelegt", format!("quelle-{}", created.id)))
            }
            "sources.update" => {
                let id = need_id()?;
                sources::update(&state, id, form.title.as_deref(), form.url.as_deref(), form.note.as_deref()).await?;
                Ok(("geaendert", format!("quelle-{id}")))
            }
            "sources.move" => {
                let id = need_id()?;
                let list = state.store.sources_list().await?;
                let now = list
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.position)
                    .ok_or(Fault::Invalid("quelle", String::new()))?;
                let position = match (form.direction.as_deref(), form.position.as_deref()) {
                    (Some("up"), _) => now - 1,
                    (Some("down"), _) => now + 1,
                    (None, Some(position)) => {
                        position.trim().parse::<i64>().map_err(|_| Fault::Invalid("position", String::new()))?
                    }
                    _ => return Err(Fault::Invalid("position", String::new())),
                };
                sources::move_to(&state, id, position).await?;
                Ok(("verschoben", format!("quelle-{id}")))
            }
            "bible.set_dirs" => {
                if checked(&form.reset) {
                    state.store.set_bible_dirs(None).await?;
                    return Ok(("bibel_vorgabe", "bibeln".to_owned()));
                }
                let lines: Vec<String> = form.dirs.as_deref().unwrap_or("").lines().map(str::to_owned).collect();
                let dirs = crate::bibles::parse_dirs(&lines).map_err(|e| Fault::Invalid("bibel_ordner", e.message))?;
                state.store.set_bible_dirs(Some(&dirs)).await?;
                Ok(("bibel_ordner", "bibeln".to_owned()))
            }
            "bible.set_work" => {
                let target = match work {
                    Some(w) => w,
                    None => state.default_work().await?,
                };
                let module = form.module.as_deref().map(str::trim).filter(|m| !m.is_empty());
                let params = json!({ "work": target, "module": module });
                crate::bibles::call(&state, "bible.set_work", params)
                    .await
                    .map_err(|e| Fault::Invalid("bibel_modul", e.message))?;
                Ok((if module.is_some() { "bibel_gewaehlt" } else { "bibel_keine" }, "bibeln".to_owned()))
            }
            "bible.cipher_key_set" | "bible.cipher_key_delete" => {
                let module = form.module.as_deref().map(str::trim).unwrap_or("");
                if form.method.as_deref() == Some("bible.cipher_key_delete") {
                    if !checked(&form.confirm) {
                        return Err(Fault::Invalid("bestaetigen", String::new()));
                    }
                    crate::bibles::cipher_key_delete(&state, module, true)
                        .await
                        .map_err(|e| Fault::Invalid("bibel_schluessel", e.message))?;
                    return Ok(("bibel_schluessel_entfernt", "bibeln".to_owned()));
                }
                match crate::bibles::cipher_key_set(&state, module, form.key.as_deref().unwrap_or("")).await {
                    Ok(_) => Ok(("bibel_schluessel", "bibeln".to_owned())),
                    Err(e) if e.code == crate::rpc::REFUSED && e.message == crate::bibles::CIPHER_KEY_WRONG => {
                        Err(Fault::Invalid("bibel_schluessel_falsch", String::new()))
                    }
                    Err(e) if e.code == crate::rpc::REFUSED => Err(Fault::Keyring(String::new())),
                    Err(_) => Err(Fault::Invalid("bibel_schluessel", String::new())),
                }
            }
            "sources.delete" => {
                let id = need_id()?;
                if !checked(&form.confirm) {
                    return Err(Fault::Invalid("bestaetigen", String::new()));
                }
                sources::delete(&state, id).await?;
                Ok(("geloescht", "quellen".to_owned()))
            }
            _ => Err(Fault::Invalid("methode", String::new())),
        }
    }
    .await;
    back("quellen", result.map_err(|fault| (fault.code(), None)), work)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_the_pages_send_has_a_german_text() {
        for code in [
            "kind",
            "url",
            "url_schema",
            "url_benutzer",
            "url_geheimnis",
            "geheimnis_feld",
            "host",
            "port",
            "tls",
            "feld",
            "ordner",
            "modell",
            "geheimnis_leer",
            "geheimnis_ungueltig",
            "kein_geheimnis",
            "bestaetigen",
            "schluesselbund",
            "werk",
            "weg",
            "sammlung",
            "reindex",
            "tabelle",
            "spalte",
            "titel",
            "adresse",
            "notiz",
            "quelle",
            "position",
            "methode",
            "kurztest_modell",
            "intern",
            "bibel_ordner",
            "bibel_modul",
            "bibel_schluessel",
            "bibel_schluessel_falsch",
            "praefix",
        ] {
            assert_ne!(error_text(code), error_text("gibt-es-nicht"), "{code} hat einen eigenen Text");
        }
        for code in ["sammlungen", "reindex", "rag", "gespeichert"] {
            assert!(notice_text(code).is_some(), "Erfolgscode {code} hat einen deutschen Text");
        }
        assert_eq!(notice_text("<script>"), None, "unbekannte Erfolgscodes zeigen nichts");
        assert_eq!(
            shown_time("2026-11-06T09:30:12.345Z"),
            crate::localtime::seconds("2026-11-06T09:30:12.345Z"),
            "Prüfzeit lesbar in Ortszeit, ohne „UTC“ (Umrechnung: localtime::tests)"
        );
    }

    /// Page „Verbindungen“ with only the section of the RAG ways and a status with `stale` leftover
    /// jobs (Round 19b: the index worker fills in the number, it is displayed here).
    fn page_with_stale(stale: usize) -> String {
        let status = baruch_rag::Status {
            work: 1,
            ways: Vec::new(),
            index: baruch_rag::IndexInfo {
                dir: "/tmp/index".into(),
                entries: 3,
                model: None,
                dimension: None,
                configured_model: "nomic-embed-text".into(),
                built: None,
                pending: 0,
                stale,
                problem: None,
            },
            fts: true,
        };
        let rag = json!({ "work": 1, "title": "Probe", "ways": [], "default": true, "collections": [],
                          "status": status_context(Ok(status)) });
        render(
            "settings/verbindungen.html",
            json!({ "notice": null, "error": null, "connections": [], "rag": rag, "work": 1,
                    "models": { "connections": [], "kinds": [], "error": null } }),
        )
        .expect("Vorlage rendert")
    }

    #[test]
    fn stale_jobs_show_only_when_there_are_some() {
        let none = page_with_stale(0);
        assert!(!none.contains("liegengeblieben"), "ohne liegengebliebene Aufträge kein Hinweis: {none}");
        let two = page_with_stale(2);
        assert!(
            two.contains("2 Aufträge liegengeblieben – erneuter Versuch nach einer Änderung oder „Index neu aufbauen“"),
            "Zahl und Weg zum erneuten Versuch genannt: {two}"
        );
        assert!(two.contains("data-index-stale=\"2\""), "maschinenlesbar für Tests und Widgets: {two}");
        let one = page_with_stale(1);
        assert!(one.contains("1 Auftrag liegengeblieben"), "Einzahl bei einem Auftrag: {one}");
    }
}
