// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einstellungsseiten „Verbindungen“ und „Quellen“: Inhalt für den Rahmen der Einstellungen
//! ([`render_connections`], [`render_sources`], Vertrag Runde 9 §3) und ihre Formulare.
//!
//! Die Formulare antworten immer mit 303 auf `/einstellungen/<seite>?ok=<code>` bzw.
//! `?fehler=<code>[&kind=…]` (mit `&work=W`, Anker `#<kind>` bzw. `#quelle-<id>`). In der Adresse
//! stehen nur feste Codes, nie Eingaben oder Geheimnisse; die Seite übersetzt sie in deutschen Text
//! ([`notice_text`], [`error_text`]). So zeichnet der Rahmen der Einstellungen die Seite, mit und
//! ohne JavaScript (im Dialog übernimmt `settings.ts` den Rahmen aus der Antwort).
//!
//! Runde 12 (Aufgabe „pgvector“): Abschnitt `#rag` mit den Wegen des Werks, den Sammlungen je Remote-Weg
//! (Kästchen aus der letzten Prüfung, wiederholtes Feld `collection`, dazu `collection_new`), dem
//! „Status der Wege“ (`search_hooks::rag_status`) und „Index neu aufbauen“ (`rag.reindex` mit
//! `confirm=1`, `search_hooks::rag_reindex`). Das Formular kommt als Liste von Paaren an
//! (`Form<Vec<(String, String)>>`), weil `collection` mehrfach vorkommt.

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

use super::{Fault, account, collections, config, describe, open_part, parse_kind, sources, usage_is_stub, usage_stub};
use crate::AppState;

/// Eigene Template-Umgebung (Autoescape für `.html`).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
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
    templates()
        .get_template(name)
        .and_then(|t| t.render(Jinja::from(minijinja::value::Serde(&ctx))))
        .map_err(|e| format!("Vorlage {name}: {e}"))
}

/// Erfolgsmeldung zu `?ok=`; unbekannte Codes zeigen nichts.
pub fn notice_text(code: &str) -> Option<&'static str> {
    Some(match code {
        "gespeichert" => "Die Einstellungen sind gespeichert.",
        "geheimnis" => "Das Geheimnis ist im Schlüsselbund hinterlegt.",
        "gespeichert_geheimnis" => "Die Einstellungen sind gespeichert, das Geheimnis ist im Schlüsselbund hinterlegt.",
        "geheimnis_entfernt" => "Das Geheimnis ist aus dem Schlüsselbund entfernt.",
        "geprueft" => "Die Verbindung ist geprüft; das Ergebnis steht bei der Verbindung.",
        "kurztest" => "Der Kurztest ist gelaufen; das Ergebnis steht bei Eliezer.",
        "rag" => "Die RAG-Wege des Werks sind gespeichert.",
        "sammlungen" => "Die Sammlungen des Werks sind gespeichert.",
        "reindex" => "Der Index des Werks wird neu aufgebaut; den Stand zeigt „Status der Wege“.",
        "angelegt" => "Die Quelle ist angelegt.",
        "geaendert" => "Die Quelle ist geändert.",
        "verschoben" => "Die Quelle ist verschoben.",
        "geloescht" => "Die Quelle ist gelöscht.",
        "bibel_ordner" => "Die Modulordner der Bibeln sind gespeichert.",
        "bibel_vorgabe" => "Die Modulordner der Bibeln stehen wieder auf der Vorgabe.",
        "bibel_gewaehlt" => "Die Bibel des Werks ist gewählt; die Stellenprüfung prüft jetzt den Wortlaut.",
        "bibel_keine" => "Das Werk hat keine Bibel mehr; der Wortlaut bleibt ungeprüft.",
        "bibel_schluessel" => "Der Schlüssel passt und ist im Schlüsselbund hinterlegt; das Modul ist lesbar.",
        "bibel_schluessel_entfernt" => "Der Schlüssel ist aus dem Schlüsselbund entfernt.",
        _ => return None,
    })
}

/// Fehlermeldung zu `?fehler=`; ein unbekannter Code ergibt eine allgemeine Meldung.
pub fn error_text(code: &str) -> &'static str {
    match code {
        "kind" => "Unbekannte Verbindung; nichts geändert.",
        "url" => "Die Adresse ist ungültig; nichts geändert.",
        "url_schema" => "Die Adresse muss mit http:// oder https:// beginnen; nichts geändert.",
        "url_benutzer" => {
            "Die Adresse darf keinen Benutzer und kein Passwort enthalten – Geheimnisse gehören in den Schlüsselbund; nichts geändert."
        }
        "url_geheimnis" => {
            "Die Adresse darf keinen Schlüssel und kein Token in der Abfrage tragen – dafür gibt es den Schlüsselbund; nichts geändert."
        }
        "geheimnis_feld" => {
            "Geheimnisse gehören nicht in die Einstellungen, sondern in den Schlüsselbund; nichts geändert."
        }
        "host" => "Der Rechner (Host) ist ungültig: Name oder IP-Adresse ohne Leerzeichen; nichts geändert.",
        "port" => "Der Port muss eine ganze Zahl von 1 bis 65535 sein; nichts geändert.",
        "tls" => "TLS: disable, prefer, require oder verify-full; nichts geändert.",
        "feld" => "Ein Feld ist ungültig (zu lang oder mehrzeilig); nichts geändert.",
        "ordner" => "Der Ordner darf nicht leer sein; nichts geändert.",
        "modell" => "Das Einbettungsmodell darf nicht leer sein (höchstens 200 Zeichen); nichts geändert.",
        "praefix" => {
            "Ein Aufgaben-Präfix darf höchstens 64 Zeichen lang sein und keine Steuerzeichen (auch keinen Zeilenumbruch) enthalten; nichts geändert."
        }
        "geheimnis_leer" => "Das Geheimnis ist leer; nichts geändert.",
        "geheimnis_ungueltig" => "Das Geheimnis ist zu lang oder mehrzeilig; nichts geändert.",
        "kein_geheimnis" => "LanceDB braucht kein Geheimnis.",
        "bestaetigen" => "Bitte bestätigen (Kästchen anhaken); nichts geändert.",
        "schluesselbund" => "Der Schlüsselbund ist nicht erreichbar oder lehnt ab; nichts geändert.",
        "gespeichert_ohne_geheimnis" => {
            "Die Einstellungen sind gespeichert, das Geheimnis aber nicht: Der Schlüsselbund ist nicht erreichbar oder lehnt ab. Das bisherige Geheimnis bleibt und wird nicht an eine geänderte Adresse geschickt."
        }
        "werk" => "Unbekanntes Werk; nichts geändert.",
        "weg" => "Unbekannter RAG-Weg; nichts geändert.",
        "sammlung" => "Sammlung ungültig (1 bis 200 Zeichen, eine Zeile, höchstens 100); nichts geändert.",
        "reindex" => "Der Index ließ sich nicht neu aufbauen; den Grund nennt „Status der Wege“.",
        "tabelle" => {
            "Die Tabelle muss ein Name aus Buchstaben, Ziffern und _ sein (wahlweise schema.tabelle); nichts geändert."
        }
        "spalte" => {
            "Ein Spaltenname ist ungültig (Buchstaben, Ziffern und _, nicht mit einer Ziffer beginnend; nur Stelle, Titel, Adresse und Modell dürfen leer sein); nichts geändert."
        }
        "titel" => "Der Titel muss 1 bis 200 Zeichen in einer Zeile haben; nichts geändert.",
        "adresse" => {
            "Die Adresse muss mit http:// oder https:// beginnen, einen Rechner nennen und darf keine Leerzeichen enthalten; nichts geändert."
        }
        "notiz" => "Die Notiz ist zu lang (höchstens 4000 Zeichen); nichts geändert.",
        "quelle" => "Diese Quelle gibt es nicht (mehr); bitte neu laden.",
        "position" => "Ungültige Stelle; nichts verschoben.",
        "methode" => "Unbekannte Aktion; nichts geändert.",
        "kurztest_modell" => {
            "Für den Kurztest bitte ein Modell wählen (eine Zeile, höchstens 200 Zeichen); nichts gesendet."
        }
        "intern" => "Interner Fehler; nichts geändert. Bitte erneut versuchen.",
        "bibel_ordner" => {
            "Modulordner: je Zeile ein absoluter Pfad (oder ~/…), höchstens 20 Ordner zu je 1000 Zeichen; nichts geändert."
        }
        "bibel_modul" => {
            "Dieses Bibelmodul gibt es in keinem Modulordner, oder Baruch kann es nicht lesen; nichts geändert."
        }
        "bibel_schluessel_falsch" => "Der Schlüssel passt nicht zum Modul; nichts gespeichert.",
        "bibel_schluessel" => {
            "Schlüssel: 1 bis 255 Zeichen in einer Zeile, nur für ein verschlüsseltes Modul aus den Modulordnern; nichts gespeichert."
        }
        _ => "Das hat nicht geklappt (unbekannter Fehlercode).",
    }
}

/// Meldungen aus der Abfrage für die Vorlage.
fn messages(query: &BTreeMap<String, String>) -> (Option<&'static str>, Option<&'static str>) {
    let notice = query.get("ok").and_then(|code| notice_text(code).or_else(|| model_notice_text(code)));
    let error = query.get("fehler").map(|code| error_text(code));
    (notice, error)
}

/// Zeitpunkt `2026-11-06T09:30:12.345Z` (UTC) → `2026-11-06 10:30:12` in Ortszeit.
fn shown_time(at: &str) -> String {
    crate::localtime::seconds(at)
}

/// Angaben einer Prüfung als Zeilen (Modelle, Werkzeuge, Versionen, Sammlungen, Index), gekürzt. Formen
/// der Angaben nach Vertrag Runde 12 §5 (pgvector, MultiGPT, LanceDB).
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
            text.push_str(&format!(" … und {more} weitere"));
        }
        text
    };
    let number = |name: &str| details.get(name).and_then(Value::as_u64);
    let text = |name: &str| details.get(name).and_then(Value::as_str).filter(|t| !t.is_empty());
    // Gemeinsames Format der Sammlungen (pgvector, MultiGPT): [{name, count}].
    let collections = |unit: &str| -> Option<String> {
        let items: Vec<String> = collections::available_of(details)
            .iter()
            .map(|c| {
                let name = c["name"].as_str().unwrap_or("");
                match c["count"].as_i64() {
                    Some(count) => format!("{name} ({count} {unit})"),
                    None => name.to_owned(),
                }
            })
            .collect();
        (!items.is_empty()).then(|| format!("Sammlungen: {}", short(items)))
    };
    let mut lines = Vec::new();
    match kind {
        ConnectionKind::Eliezer => {
            let models = list("models");
            if !models.is_empty() {
                lines.push(format!("Modelle: {}", short(models)));
            }
        }
        ConnectionKind::Multigpt => {
            let name = details.pointer("/server/name").and_then(Value::as_str).unwrap_or("");
            let version = details.pointer("/server/version").and_then(Value::as_str).unwrap_or("");
            if !name.is_empty() {
                lines.push(format!("Server: {name} {version}").trim().to_owned());
            }
            let tools = list("tools");
            if !tools.is_empty() {
                lines.push(format!("Werkzeuge: {}", short(tools)));
            }
            if let Some(pages) = number("pages").filter(|&p| p > 1) {
                lines.push(format!("Werkzeugliste in {pages} Seiten"));
            }
            lines.extend(collections("Dokumente"));
        }
        ConnectionKind::Pgvector => {
            if let Some(version) = text("server_version") {
                lines.push(format!("Server: {version}"));
            }
            if details.get("server_version").is_some() {
                let vector = text("vector").unwrap_or("fehlt");
                lines.push(format!("Erweiterung vector: {vector}"));
            }
            if let Some(table) = text("table") {
                let columns: Vec<String> = details
                    .get("columns")
                    .and_then(Value::as_object)
                    .map(|map| map.iter().filter_map(|(role, c)| c.as_str().map(|c| format!("{role} = {c}"))).collect())
                    .unwrap_or_default();
                if columns.is_empty() {
                    lines.push(format!("Tabelle: {table}"));
                } else {
                    lines.push(format!("Tabelle: {table} ({})", columns.join(", ")));
                }
            }
            if let Some(chunks) = number("chunks") {
                lines.push(format!("Abschnitte: {chunks}"));
            }
            if let Some(dimension) = number("dimension") {
                lines.push(format!("Dimension: {dimension}"));
            }
            let models = list("models");
            if !models.is_empty() {
                lines.push(format!("Einbettungsmodelle in den Daten: {}", short(models)));
            }
            lines.extend(collections("Abschnitte"));
        }
        ConnectionKind::Lancedb => {
            if let Some(dir) = text("dir") {
                let exists = details.get("exists").and_then(Value::as_bool);
                let writable = details.get("writable").and_then(Value::as_bool);
                let state = match (exists, writable) {
                    (Some(false), _) => " (fehlt noch)",
                    (_, Some(false)) => " (nicht beschreibbar)",
                    (Some(true), Some(true)) => " (beschreibbar)",
                    _ => "",
                };
                lines.push(format!("Ordner: {dir}{state}"));
            }
            let tables = list("tables");
            if !tables.is_empty() {
                lines.push(format!("Tabellen: {}", short(tables)));
            }
            if let Some(entries) = number("entries") {
                lines.push(format!("Einträge: {entries}"));
            }
            match (text("model"), number("dimension")) {
                (Some(model), Some(dimension)) => lines.push(format!("Index: {model}, Dimension {dimension}")),
                (Some(model), None) => lines.push(format!("Index: {model}")),
                _ => {}
            }
            if let Some(configured) = text("configured_model") {
                lines.push(format!("Eingestelltes Modell: {configured}"));
            }
            match text("embedder") {
                Some("ok") => lines.push("Einbettung: geht".to_owned()),
                Some(why) => lines.push(format!("Einbettung: {why}")),
                None => {}
            }
        }
    }
    lines
}

/// Eingabefelder je Art für die Vorlage: `{name, label, value, type, options?, hint?}`.
fn form_fields(kind: ConnectionKind, config: &Value) -> Vec<Value> {
    // Noch nicht gespeicherte Felder (Zuordnung von pgvector, Runde 12) zeigen ihre Vorgabe.
    let config = &config::effective(kind, config);
    let value = |name: &str| match config.get(name) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    config::fields(kind)
        .iter()
        .map(|name| {
            let (label, kind_of, hint) = match *name {
                "url" => ("Adresse", "url", "http:// oder https://, ohne Benutzer und Passwort"),
                "host" => ("Rechner (Host)", "text", "leer = nicht eingerichtet"),
                "port" => ("Port", "number", ""),
                "database" => ("Datenbank", "text", "leer = wie der Benutzer"),
                "user" => ("Benutzer", "text", ""),
                "tls" => ("TLS", "select", ""),
                "dir" => ("Ordner", "text", "relativ zum Datenordner oder absolut"),
                "embedding_model" if kind == ConnectionKind::Pgvector => {
                    ("Einbettungsmodell", "text", "dasselbe Modell, mit dem die Daten eingebettet sind")
                }
                "embedding_model" => ("Einbettungsmodell", "text", ""),
                "table" => ("Tabelle", "text", "tabelle oder schema.tabelle, genau wie in der Datenbank"),
                "text_column" => ("Spalte Text", "text", ""),
                "vector_column" => ("Spalte Vektor", "text", "Typ vector (pgvector)"),
                "collection_column" => ("Spalte Sammlung", "text", ""),
                "document_column" => ("Spalte Dokument", "text", ""),
                "section_column" => ("Spalte Stelle", "text", "leer = gibt es nicht"),
                "title_column" => ("Spalte Titel", "text", "leer = gibt es nicht"),
                "url_column" => ("Spalte Adresse", "text", "leer = gibt es nicht"),
                "model_column" => {
                    ("Spalte Modell", "text", "leer = gibt es nicht; sonst prüft Baruch das Modell je Sammlung")
                }
                // Runde 19b: Aufgaben-Präfixe (Vorschlag für nomic-embed-text im Platzhalter und im Hinweis).
                "query_prefix" if kind == ConnectionKind::Pgvector => (
                    "Präfix der Anfrage",
                    "text",
                    "wie die Daten eingebettet sind; für nomic-embed-text „search_query: “ (mit Leerzeichen am Ende); leer = kein Präfix",
                ),
                "query_prefix" => (
                    "Präfix der Anfrage",
                    "text",
                    "für nomic-embed-text „search_query: “ (mit Leerzeichen am Ende); leer = kein Präfix",
                ),
                "document_prefix" => (
                    "Präfix der Dokumente",
                    "text",
                    "für nomic-embed-text „search_document: “; leer = kein Präfix. Eine Änderung verlangt „Index neu aufbauen“",
                ),
                _ => ("Feld", "text", ""),
            };
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

/// Modell, das der Kurztest vorschlägt, wenn Eliezer es kennt (Versuch 2026-11-06: alle Personas auf Gemma).
const PROBE_DEFAULT: &str = "google/gemma-4-26b-a4b";

/// Kurztest für Eliezer: Auswahl aus der Modellliste der letzten Prüfung und den Modellen der aktiven
/// Personas (ohne Abfrage bei Eliezer – die Seite fragt nie selbst), dazu das letzte Ergebnis.
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

/// Seite „Verbindungen“ als Inhalt für den Rahmen der Einstellungen: je Verbindung Einstellungen,
/// Zustand des Geheimnisses, letzte Prüfung und STUB-Hinweis; dazu die RAG-Wege des Werks.
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
            // Runde 19b: Hinweis bei hinterlegtem Passwort und TLS prefer/disable (nur pgvector).
            let tls_warning = (row.kind == ConnectionKind::Pgvector)
                .then(|| super::pgvector::tls_warning(&row.config, described["secret"] == json!("hinterlegt")))
                .flatten();
            connections.push(json!({
                "tls_warning": tls_warning,
                "kind": row.kind, "title": described["title"], "account": account(row.kind),
                "secret": described["secret"], "secret_error": described.get("secret_error"),
                "secret_for": described.get("secret_for"),
                "fields": form_fields(row.kind, &row.config), "check": check,
                "checkable": true, "stub": usage_stub(row.kind),
                "stub_open": usage_is_stub(row.kind), "open_part": open_part(row.kind), "probe": probe,
                "default": row.updated.is_none(),
                "error": if error_kind == Some(row.kind) { error } else { None },
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

/// Modell-Verbindungen für die Vorlage (Runde 15b): je Verbindung Angaben aus `models::describe`, letzte
/// Prüfung und letzter Kurztest in Ortszeit, Modelle der letzten Prüfung als Auswahl des Kurztests.
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
                            Some(format!("{id}: {n} Tokens"))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let lines = if contexts.is_empty() {
                Vec::new()
            } else {
                vec![format!("Gemeldete Kontextlängen: {}", contexts.join(", "))]
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

/// Erfolgsmeldung der Modell-Verbindungen zu `?ok=`.
pub fn model_notice_text(code: &str) -> Option<&'static str> {
    Some(match code {
        "modell_gespeichert" => "Die Modell-Verbindung ist gespeichert.",
        "modell_angelegt" => "Die Modell-Verbindung ist angelegt.",
        "modell_geloescht" => "Die Modell-Verbindung ist gelöscht; Personas, die sie nutzten, nutzen wieder Eliezer.",
        "modell_schluessel" => "Der Schlüssel ist im Schlüsselbund hinterlegt und gilt für die jetzige Adresse.",
        "modell_schluessel_entfernt" => "Der Schlüssel ist aus dem Schlüsselbund entfernt.",
        "modell_geprueft" => "Die Modell-Verbindung ist geprüft; das Ergebnis steht bei ihr.",
        "modell_kurztest" => "Der Kurztest ist gelaufen; das Ergebnis steht bei der Verbindung.",
        _ => return None,
    })
}

/// Fehlermeldung der Modell-Verbindungen zu `?modellfehler=`.
pub fn model_error_text(code: &str) -> &'static str {
    match code {
        "name" => "Der Name fehlt, ist zu lang oder gibt es schon; nichts geändert.",
        "art" => "Unbekannte Art (Eliezer, OpenAI-kompatibel oder Ollama); nichts geändert.",
        "adresse" => "Die Adresse ist ungültig (http:// oder https://, ohne Benutzer und Schlüssel); nichts geändert.",
        "verbindung" => "Diese Modell-Verbindung gibt es nicht (mehr); bitte neu laden.",
        "eliezer" => "Die Verbindung Eliezer lässt sich nicht löschen und bleibt von der Art Eliezer.",
        "schluessel" => "Der Schlüssel ist leer, zu lang oder mehrzeilig; nichts geändert.",
        "schluesselbund" => "Der Schlüsselbund ist nicht erreichbar oder lehnt ab; nichts geändert.",
        "bestaetigen" => "Bitte bestätigen (Kästchen anhaken); nichts geändert.",
        "modell" => "Für den Kurztest bitte ein Modell angeben (eine Zeile, höchstens 200 Zeichen); nichts gesendet.",
        "aktion" => "Unbekannte Aktion; nichts geändert.",
        _ => "Das hat nicht geklappt; nichts geändert.",
    }
}

/// `POST /einstellungen/werk/bibeln` (Runde 15, hinter `guard::same_origin`): `method=bible.set_work_sources`,
/// `work`, wiederholte Felder `commentary` und `lexicon` (Kästchen der Seite Einstellungen → Werk). 303 auf
/// `/einstellungen/werk?work=W&ok=kommentare#kommentare` bzw. `…&kommentarfehler=1#kommentare`.
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

/// Felder von `POST /einstellungen/modelle/act`.
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

/// Fehlercode aus einer RPC-Antwort der Modell-Verbindungen (feste Codes, nie Eingaben in der Adresse).
fn model_fault(e: &crate::rpc::RpcError) -> &'static str {
    let m = e.message.as_str();
    if m.contains("Schlüsselbund") {
        "schluesselbund"
    } else if m.contains("Eliezer") {
        "eliezer"
    } else if m.contains("Adresse") || m.starts_with("url") {
        "adresse"
    } else if m.contains("Art") || m.starts_with("kind") {
        "art"
    } else if m.contains("Name") {
        "name"
    } else if m.contains("secret") {
        "schluessel"
    } else if m.contains("gibt es nicht") {
        "verbindung"
    } else {
        "unbekannt"
    }
}

/// `POST /einstellungen/modelle/act` (hinter `guard::same_origin`): `action` ∈ `save` (ohne `id` anlegen;
/// `name`, `kind`, `url`, `external=1|0|auto` – `auto` nach der Adresse), `secret` (`id`, `secret`), `delete_secret` (`id`, `confirm=1`),
/// `delete` (`id`, `confirm=1`), `check` (`id`), `probe` (`id`, `model`). Immer 303 auf
/// `/einstellungen/verbindungen?ok=…#modell-<id>` bzw. `?modellfehler=<code>#modelle`.
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
                // `auto` (Anlegen, Vorgabe): Lage nach der Adresse (`models::looks_local`); sonst ausdrücklich
                // (Kästchen beim Ändern: ohne Haken lokal).
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

/// Sammlungen eines Remote-Wegs für die Vorlage (aus `collections::get`): Kästchen aus der letzten
/// Prüfung (`available`) und die gewählten, die sie nicht (mehr) meldet (`gone`, nur wenn es eine Prüfung
/// gab), dazu der Zeitpunkt der Prüfung.
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

/// „Status der Wege“ für die Vorlage: je Weg Zustand und Meldung, der lokale Index – oder der Grund,
/// warum es keinen Status gibt.
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

/// Seite „Quellen“ als Inhalt für den Rahmen: Liste mit Bearbeiten, Verschieben, Löschen und Anlegen.
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
                    "usage": sources::USAGE, "bibles": bibles }),
        )
    })
}

/// Abschnitt „Bibeln (SWORD)“: Modulordner, gefundene Bibeln und die Wahl des Werks (`work` oder Vorgabewerk).
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
    // R17c: Gibt es verschlüsselte Module? Dann steht der Abschnitt „Schlüssel verschlüsselter Module“ da.
    let sealed =
        value["modules"].as_array().into_iter().flatten().any(|m| m["cipher"].as_str().is_some_and(|c| c != "none"));
    value["sealed"] = json!(sealed);
    Ok(value)
}

// ---- Formulare -----------------------------------------------------------------------------------

/// Felder von `POST /einstellungen/verbindungen/act` (ohne `collection`, das mehrfach vorkommt).
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
    /// Aufgaben-Präfixe (Runde 19b; nicht beschnitten).
    query_prefix: Option<String>,
    document_prefix: Option<String>,
    secret: Option<String>,
    confirm: Option<String>,
    /// Modell für den Kurztest (`model.probe`, nur Eliezer).
    model: Option<String>,
    way_lancedb: Option<String>,
    way_pgvector: Option<String>,
    way_multigpt: Option<String>,
    /// Weg der Sammlungen (`connection.collections_set`).
    way: Option<String>,
    /// Neue Sammlung zum Hinzufügen (eine Zeile).
    collection_new: Option<String>,
}

impl ConnectionForm {
    /// Aus den Paaren des Formulars; bei doppelten Feldern gilt das letzte (außer `collection`, das
    /// [`collections_of`] sammelt).
    fn from_pairs(pairs: &[(String, String)]) -> ConnectionForm {
        let map: serde_json::Map<String, Value> = pairs
            .iter()
            .filter(|(name, _)| name != "collection")
            .map(|(name, value)| (name.clone(), Value::from(value.as_str())))
            .collect();
        serde_json::from_value(Value::Object(map)).unwrap_or_default()
    }
}

/// Gewählte Sammlungen: jedes Feld `collection` in der Reihenfolge des Formulars, dazu `collection_new`,
/// wenn es nicht leer ist (beschnitten und geprüft wird im Store).
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

/// Rücksprung: nur feste Codes, Werk-id als Zahl und ein fester Anker.
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

/// `POST /einstellungen/verbindungen/act`: `method` ∈ `connection.set` (Felder der Art, dazu wahlweise
/// `secret`), `connection.set_secret`, `connection.delete_secret` (`confirm=1`), `connection.check`,
/// `connection.rag_set` (`work`, `way_*=1`), `model.probe` (nur `kind=eliezer`, Feld `model`: Kurztest,
/// Ergebnis auf der Seite); seit Runde 12 `connection.collections_set` (`work`, `way`, wiederholtes Feld
/// `collection`, dazu `collection_new`) und `rag.reindex` (`work`, `confirm=1`). Immer 303.
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
        // Nur der Mensch über dieses Formular (kein MCP-Werkzeug, nicht über POST /act).
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
            // Erst alles prüfen (Geheimnis wie Einstellungen), dann schreiben: Ein ungültiges Geheimnis
            // darf nicht erst auffallen, wenn die Einstellungen schon gespeichert sind („nichts geändert“
            // muss stimmen). Das Geheimnis kommt nach den Einstellungen, damit es für das neue Ziel gilt.
            match secret.map(|s| super::valid_secret(kind, s)).transpose() {
                Err(fault) => Err(fault),
                Ok(secret) => match super::set_config(&state, kind, &Value::Object(given)).await {
                    Err(fault) => Err(fault),
                    Ok(_) => match secret {
                        Some(secret) => match super::set_secret(&state, kind, secret).await {
                            Ok(()) => Ok("gespeichert_geheimnis"),
                            // Die Einstellungen stehen schon; das sagt die Meldung ehrlich.
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
                // Ein Fehlschlag (nicht erreichbar, unbekanntes Modell …) ist ein Ergebnis auf der Seite.
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

/// Felder von `POST /einstellungen/quellen/act`.
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
    /// Modulordner der Bibeln, je Zeile einer (`bible.set_dirs`).
    dirs: Option<String>,
    /// `reset=1`: Modulordner zurück auf die Vorgabe.
    reset: Option<String>,
    /// Gewählte Bibel (`bible.set_work`; leer = keine), bzw. Modul für `bible.cipher_key_*`.
    module: Option<String>,
    /// Schlüssel eines verschlüsselten Moduls (`bible.cipher_key_set`, R17c); geht nur in den Schlüsselbund.
    key: Option<String>,
}

/// `POST /einstellungen/quellen/act`: `method` ∈ `sources.create` (title, url, note),
/// `sources.update` (id, title, url, note), `sources.move` (id, `direction=up|down` oder `position`),
/// `sources.delete` (id, `confirm=1`), `bible.set_dirs` (`dirs` je Zeile ein Ordner, `reset=1` = Vorgabe),
/// `bible.set_work` (`work`, `module`, leer = keine Bibel), `bible.cipher_key_set` (`module`, `key`; R17c) und
/// `bible.cipher_key_delete` (`module`, `confirm=1`). Immer 303.
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

    /// Seite „Verbindungen“ nur mit dem Abschnitt der RAG-Wege und einem Status mit `stale` liegengebliebenen
    /// Aufträgen (Runde 19b: die Zahl füllt der Index-Worker, angezeigt wird sie hier).
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
