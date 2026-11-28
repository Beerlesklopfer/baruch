// SPDX-License-Identifier: AGPL-3.0-or-later
//! „Verbindung prüfen“ for LanceDB (Round 12, replaces the STUB in `connections::run_check`): folder
//! (present, writable), index (tables, entries, model, dimension), configured model and a
//! probe embedding of a fixed sentence. `ok` = folder usable ∧ probe succeeds ∧ model and dimension
//! match the index (if there already is one). The result is stored by `Store::connection_record_check`;
//! the JSON form is the same as for the other connections (`{kind, ok, message, details, at}`).

use baruch_rag::RagError;
use baruch_store::ConnectionKind;
use serde_json::{Value, json};

use crate::AppState;
use crate::connections::Fault;
use crate::i18n::tc;

/// Fixed text of the probe embedding (no book text). Stays German on purpose: it is only sent to the embedding model,
/// never shown, and a fixed text keeps the probe comparable (Round 39c).
pub const PROBE_TEXT: &str = "Baruch prüft die Einbettung.";

/// Is the folder writable? (Create and delete a probe file.)
fn writable(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| tc("rag_msg.check.dir_create", &[("detail", e.to_string().into())]))?;
    let probe = dir.join(".baruch-schreibprobe");
    std::fs::write(&probe, b"ok").map_err(|e| tc("rag_msg.check.not_writable", &[("detail", e.to_string().into())]))?;
    std::fs::remove_file(&probe).map_err(|e| tc("rag_msg.check.probe_delete", &[("detail", e.to_string().into())]))
}

/// Run the check and store it.
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
                problems.push(tc(
                    "rag_msg.check.folder",
                    &[("dir", cfg.dir.display().to_string().into()), ("why", why.into())],
                ));
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
                        RagError::ModelMismatch(tc(
                            "rag_msg.check.dimension",
                            &[("got", answer.dimension.to_string().into()), ("index", d.to_string().into())],
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
                    tc(
                        "rag_msg.check.ready",
                        &[
                            ("dir", cfg.dir.display().to_string().into()),
                            ("entries", entries.into()),
                            ("model", cfg.configured.as_str().into()),
                            ("dimension", dimension.to_string().into()),
                        ],
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
