// SPDX-License-Identifier: AGPL-3.0-or-later
//! „Verbindung prüfen“ für LanceDB (Runde 12, ersetzt den STUB in `connections::run_check`): Ordner
//! (vorhanden, beschreibbar), Index (Tabellen, Einträge, Modell, Dimension), eingestelltes Modell und eine
//! Probe-Einbettung eines festen Satzes. `ok` = Ordner nutzbar ∧ Probe gelingt ∧ Modell und Dimension
//! passen zum Index (falls schon einer da ist). Das Ergebnis speichert `Store::connection_record_check`;
//! die JSON-Form ist dieselbe wie bei den anderen Verbindungen (`{kind, ok, message, details, at}`).

use baruch_rag::RagError;
use baruch_store::ConnectionKind;
use serde_json::{Value, json};

use crate::AppState;
use crate::connections::Fault;

/// Fester Text der Probe-Einbettung (kein Buchtext).
pub const PROBE_TEXT: &str = "Baruch prüft die Einbettung.";

/// Ist der Ordner beschreibbar? (Probedatei anlegen und löschen.)
fn writable(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let probe = dir.join(".baruch-schreibprobe");
    std::fs::write(&probe, b"ok").map_err(|e| format!("nicht beschreibbar: {e}"))?;
    std::fs::remove_file(&probe).map_err(|e| format!("Probedatei nicht löschbar: {e}"))
}

/// Prüfung durchführen und speichern.
pub async fn check_lancedb(state: &AppState) -> Result<Value, Fault> {
    let rt = super::runtime(state);
    let mut details = json!({
        "dir": Value::Null, "exists": false, "writable": false, "tables": [], "entries": 0,
        "model": Value::Null, "dimension": Value::Null, "configured_model": Value::Null, "embedder": Value::Null,
    });
    let (ok, message) = match super::index::config(&state.store, &state.data).await {
        Err(e) => (false, e.message().to_owned()),
        Ok(cfg) => {
            details["dir"] = json!(cfg.dir.display().to_string());
            details["configured_model"] = json!(cfg.configured);
            details["exists"] = json!(cfg.dir.is_dir());
            let usable = writable(&cfg.dir);
            details["writable"] = json!(usable.is_ok());
            let mut problems: Vec<String> = Vec::new();
            if let Err(why) = &usable {
                problems.push(format!("Ordner {}: {why}.", cfg.dir.display()));
            }
            let mut meta = None;
            if usable.is_ok() {
                match rt.index(&cfg.dir).await {
                    Err(e) => problems.push(e.message().to_owned()),
                    Ok(index) => {
                        details["tables"] = json!(index.tables().await.unwrap_or_default());
                        details["entries"] = json!(index.count(None).await.unwrap_or(0));
                        match index.meta() {
                            Ok(m) => {
                                details["model"] = json!(m.requested);
                                details["dimension"] = json!(m.dimension);
                                meta = Some(m);
                            }
                            Err(e) => problems.push(e.message().to_owned()),
                        }
                    }
                }
            }
            let embedder = rt.factory().for_model(&cfg.model);
            let probe = embedder.embed(&[PROBE_TEXT.to_owned()]).await;
            match &probe {
                Ok(answer) => {
                    details["embedder"] = json!("ok");
                    details["probe"] = json!({ "model": answer.model, "dimension": answer.dimension });
                }
                Err(e) => {
                    details["embedder"] = json!(e.message());
                    problems.push(e.message().to_owned());
                }
            }
            if let Some(meta) = &meta {
                if let Some(e) = super::index::mismatch(meta, &cfg) {
                    problems.push(e.message().to_owned());
                } else if let (Some(d), Ok(answer)) = (meta.dimension, &probe)
                    && d != answer.dimension
                {
                    problems.push(
                        RagError::ModelMismatch(format!(
                            "Das Modell liefert {} Dimensionen, der Index hat {d}; „Index neu aufbauen“ hilft.",
                            answer.dimension
                        ))
                        .message()
                        .to_owned(),
                    );
                }
            }
            if problems.is_empty() {
                let entries = details["entries"].as_u64().unwrap_or(0);
                let dimension = probe.as_ref().map(|a| a.dimension).unwrap_or_default();
                (
                    true,
                    format!(
                        "LanceDB bereit: Ordner {}, {entries} Einträge, Einbettung mit {} ({dimension} Dimensionen) geht.",
                        cfg.dir.display(),
                        cfg.configured
                    ),
                )
            } else {
                (false, problems.join(" "))
            }
        }
    };
    let saved = state.store.connection_record_check(ConnectionKind::Lancedb, ok, &message, &details).await?;
    let at = saved.check.map(|c| c.at);
    Ok(json!({ "kind": ConnectionKind::Lancedb, "ok": ok, "message": message, "details": details, "at": at }))
}
