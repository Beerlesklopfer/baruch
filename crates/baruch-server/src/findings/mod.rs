// SPDX-License-Identifier: AGPL-3.0-or-later
//! Befunde (Runde 28): Antworten von Personas werden in Befunde zerlegt ([`ingest`]), die der Mensch übernimmt,
//! verwirft, verschiebt oder erledigt. `mod.rs`, [`parse`] und [`rpc`] gehören der Aufgabe befunde, [`page`] und
//! [`view`] befund-uebersicht.
//!
//! - Zerlegen ändert keinen Beitrag; zerlegt werden Antworten von Personas aus ask, review, chat (Abschnitt), Faden
//!   und MCP `ask_persona`/`chat`, nie Vorschläge. Vermerke von Baruch sind keine Befunde (Vertrag R28 §1.3).
//! - Status setzt nur der Mensch; Assistenten stellen Vorschläge ([`rpc::propose`]). Jede Statusänderung, jeder
//!   Vorschlag und jede Entscheidung ist genau ein Commit im Spiegel; das Anlegen beim Zerlegen nicht.
//! - Verweis auf einen Befund im Text: „[B<id>]“.
//! - Vorhandene Beiträge zerlegt [`backfill`] einmal je Datenbank (beim Start von `serve`).

pub mod page;
pub mod parse;
pub mod rpc;
pub mod view;

use std::collections::HashSet;

pub use baruch_store::{
    DeferTarget, Finding, FindingFilter, FindingKind, FindingStatus, FindingStatusEntry, StatusChange,
};
use baruch_store::{NewFinding, WorkId};
use serde_json::json;

use crate::AppState;

/// Woher die zerlegte Antwort stammt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOrigin {
    /// Beitrag im Abschnitts-Chat (`post.id`) des Abschnitts `key`.
    Post { id: i64, key: i64 },
    /// Fadenbeitrag (`chat_thread_post.id`) im Faden `thread`.
    ThreadPost { id: i64, thread: i64 },
}

/// Eine Antwort, die in Befunde zerlegt werden soll.
#[derive(Debug, Clone)]
pub struct IngestSource<'a> {
    pub work: WorkId,
    pub origin: IngestOrigin,
    /// Autor der Antwort (`@handle`; beim manuellen Umwandeln der Autor des Beitrags).
    pub persona: &'a str,
    /// Antworttext ohne Baruch-Vermerke.
    pub text: &'a str,
    /// Sammlung des Lektors beim RFC: „[B<id>]“ im Text werden `summary_of`.
    pub summary: bool,
    /// Der Mensch wandelt um: ohne Marken ein Hinweis mit dem ganzen Text.
    pub manual: bool,
}

/// Ergebnis von [`ingest`]: neu angelegte und an ähnliche angehängte Befunde.
#[derive(Debug, Clone, Default)]
pub struct IngestReport {
    pub created: Vec<Finding>,
    pub attached: Vec<Finding>,
}

/// Zerlegt und speichert; nimmt NIE `state.write_lock` (der Aufrufer kann sie halten), nur Store-Transaktionen;
/// wiederholbar (eindeutig je Beitrag und Stelle: schon vorhandene Stellen werden übergangen); kein Commit
/// (`befunde.md` steht im nächsten Schnappschuss).
///
/// - Zerlegt wird mit [`parse::split_answer`]; ergibt die Antwort keinen Befund und wandelt der Mensch um
///   (`manual`), wird der ganze Text ein Hinweis ([`parse::whole_answer`]).
/// - Ähnlichkeit (Vertrag R28 §1.4, nur im Abschnitt): Gleicht ein neuer Befund ([`parse::similar`]) einem
///   entschiedenen, nicht angehängten Befund desselben Abschnitts (übernommen, verworfen, verschoben, erledigt) oder
///   einem offenen derselben Persona dort (aus einem anderen Beitrag), wird er angehängt (`similar_to`, zählt nicht
///   als offen); entschiedene gehen vor offenen. Fadenbeiträge ohne Abschnitt werden nicht verglichen.
/// - `summary`: „[B<id>]“ im Text werden `summary_of` (UUIDs der genannten Befunde dieses Werks; unbekannte fallen
///   weg).
/// - Runde 30b: Lösungen unter einem Befund (`statt:`/`besser:` bzw. `Schritt:`, [`crate::assist::solutions`])
///   speichert `Store::finding_set_solutions` je Befund; der Schlussblock „Mein Vorschlag:“ des Lektors ergibt keine
///   Befunde.
pub async fn ingest(state: &AppState, source: &IngestSource<'_>) -> Result<IngestReport, String> {
    let store = &state.store;
    let (post, thread_post, key, sql, origin_id) = match source.origin {
        IngestOrigin::Post { id, key } => {
            (Some(id), None, Some(key), "SELECT position FROM finding WHERE post_id = ?", id)
        }
        IngestOrigin::ThreadPost { id, .. } => {
            (None, Some(id), None, "SELECT position FROM finding WHERE thread_post_id = ?", id)
        }
    };
    let mut parsed = parse::split_answer(source.text);
    if parsed.is_empty() && source.manual {
        parsed.extend(parse::whole_answer(source.text));
    }
    if parsed.is_empty() {
        return Ok(IngestReport::default());
    }
    let present: Vec<i64> =
        sqlx::query_scalar(sql).bind(origin_id).fetch_all(store.pool()).await.map_err(|e| e.to_string())?;
    let candidates: Vec<Finding> = match key {
        Some(key) => {
            let filter =
                FindingFilter { keys: vec![key], include_praise: true, limit: Some(2000), ..Default::default() };
            store
                .finding_list(source.work, &filter)
                .await
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|f| f.post.is_none() || f.post != post)
                .collect()
        }
        None => Vec::new(),
    };
    let mut report = IngestReport::default();
    for found in parsed {
        if present.contains(&i64::from(found.position)) {
            continue;
        }
        let matching = |decided: bool| {
            candidates.iter().find(|c| {
                let fits = if decided {
                    c.status != FindingStatus::Open
                } else {
                    c.status == FindingStatus::Open && c.persona == source.persona
                };
                fits && parse::similar(&c.body, &found.body)
            })
        };
        let similar_to = matching(true).or_else(|| matching(false)).map(|c| c.id);
        let mut summary_of: Vec<String> = Vec::new();
        if source.summary {
            for id in &found.refs {
                if let Ok(referred) = store.finding_get(source.work, *id).await
                    && !summary_of.contains(&referred.uuid)
                {
                    summary_of.push(referred.uuid);
                }
            }
        }
        let new = NewFinding {
            work: source.work,
            key,
            post,
            thread_post,
            position: found.position,
            persona: source.persona,
            kind: found.kind,
            body: &found.body,
            quote: &found.quote,
            quote_check: &found.quote_check,
            similar_to,
            summary_of: &summary_of,
        };
        let mut finding = store.finding_insert(&new).await.map_err(|e| e.to_string())?;
        // Runde 30b: Lösungen je Befund (`statt`/`besser` bzw. `Schritt`); scheitert das, bleibt der Befund.
        if !found.solutions.is_empty() {
            match store.finding_set_solutions(source.work, finding.id, &found.solutions).await {
                Ok(()) => finding.solutions = found.solutions.clone(),
                Err(e) => eprintln!("Lösungen zu Befund {}: {e}", finding.id),
            }
        }
        if finding.similar_to.is_some() {
            report.attached.push(finding);
        } else {
            report.created.push(finding);
        }
    }
    Ok(report)
}

/// Merker in `store_meta`: Der Nachtrag ist gelaufen.
pub const BACKFILL_KEY: &str = "findings_backfill";

/// Einmal je Datenbank (`store_meta` [`BACKFILL_KEY`]): alle Beiträge und Fadenbeiträge von Personas (Autor ist ein
/// Handle aus `persona`; nicht der Mensch, nicht `@mcp:…`) zerlegen, ohne sie zu ändern, älteste zuerst. Gibt die
/// Zahl der angelegten Befunde zurück (auch angehängte). Wie [`ingest`] ohne Schreibsperre und ohne Commit. Scheitert
/// ein Beitrag, bricht der Nachtrag ab und der Merker bleibt ungesetzt: Der nächste Start versucht es wieder, schon
/// angelegte Stellen werden übergangen.
pub async fn backfill(state: &AppState) -> Result<usize, String> {
    let pool = state.store.pool();
    let done: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
        .bind(BACKFILL_KEY)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    if done.is_some() {
        return Ok(0);
    }
    let personas: HashSet<String> = sqlx::query_scalar::<_, String>("SELECT handle FROM persona")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();
    let posts: Vec<(i64, i64, WorkId, String, String)> = sqlx::query_as(
        "SELECT p.id, p.node_id, n.work_id, p.author, p.body FROM post p JOIN node n ON n.id = p.node_id \
         ORDER BY p.created, p.id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let mut count = 0;
    for (id, key, work, author, body) in posts {
        if !personas.contains(&author) {
            continue;
        }
        let source = IngestSource {
            work,
            origin: IngestOrigin::Post { id, key },
            persona: &author,
            text: &body,
            summary: false,
            manual: false,
        };
        let report = ingest(state, &source).await.map_err(|e| format!("Beitrag {id}: {e}"))?;
        count += report.created.len() + report.attached.len();
    }
    let thread_posts: Vec<(i64, i64, WorkId, String, String)> = sqlx::query_as(
        "SELECT tp.id, tp.thread_id, t.work_id, tp.author, tp.body FROM chat_thread_post tp \
         JOIN chat_thread t ON t.id = tp.thread_id ORDER BY tp.created, tp.id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    for (id, thread, work, author, body) in thread_posts {
        if !personas.contains(&author) {
            continue;
        }
        let source = IngestSource {
            work,
            origin: IngestOrigin::ThreadPost { id, thread },
            persona: &author,
            text: &body,
            summary: false,
            manual: false,
        };
        let report = ingest(state, &source).await.map_err(|e| format!("Fadenbeitrag {id}: {e}"))?;
        count += report.created.len() + report.attached.len();
    }
    sqlx::query(
        "INSERT INTO store_meta (key, value) VALUES (?, '1') ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(BACKFILL_KEY)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(count)
}

/// Adresse eines Befunds: `/werke/{work}/befunde?befund={id}`.
pub fn finding_url(work: WorkId, finding: &Finding) -> String {
    format!("/werke/{work}/befunde?befund={}", finding.id)
}

/// Befund als JSON: die Felder von [`Finding`] und dazu `kind_label`, `status_label`, `url`.
pub fn finding_json(work: WorkId, finding: &Finding) -> serde_json::Value {
    let mut value = json!(finding);
    value["kind_label"] = json!(finding.kind.label());
    value["status_label"] = json!(finding.status.label());
    value["url"] = json!(finding_url(work, finding));
    value
}
