// SPDX-License-Identifier: AGPL-3.0-or-later
//! Findings (Round 28): answers of personas are parsed into findings ([`ingest`]) that the human adopts,
//! rejects, postpones or resolves. `mod.rs`, [`parse`] and [`rpc`] belong to the task befunde, [`page`] and
//! [`view`] to befund-uebersicht.
//!
//! - Parsing changes no post; parsed are answers of personas from ask, review, chat (section), thread
//!   and MCP `ask_persona`/`chat`, never proposals. Notes by Baruch are not findings (contract R28 §1.3).
//! - Only the human sets the status; assistants place proposals ([`rpc::propose`]). Every status change, every
//!   proposal and every decision is exactly one commit in the mirror; the creation when parsing is not.
//! - Reference to a finding in the text: „[B<id>]“.
//! - Existing posts are parsed by [`backfill`] once per database (at the start of `serve`).

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

/// Where the parsed answer comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOrigin {
    /// Post in the section chat (`post.id`) of the section `key`.
    Post { id: i64, key: i64 },
    /// Thread post (`chat_thread_post.id`) in the thread `thread`.
    ThreadPost { id: i64, thread: i64 },
}

/// An answer that is to be parsed into findings.
#[derive(Debug, Clone)]
pub struct IngestSource<'a> {
    pub work: WorkId,
    pub origin: IngestOrigin,
    /// Author of the answer (`@handle`; for manual conversion the author of the post).
    pub persona: &'a str,
    /// Answer text without Baruch notes.
    pub text: &'a str,
    /// Collection of the lector in an RFC: „[B<id>]“ in the text become `summary_of`.
    pub summary: bool,
    /// The human converts: without markers a note with the whole text.
    pub manual: bool,
}

/// Result of [`ingest`]: newly created findings and those attached to similar ones.
#[derive(Debug, Clone, Default)]
pub struct IngestReport {
    pub created: Vec<Finding>,
    pub attached: Vec<Finding>,
}

/// Parses and stores; NEVER takes `state.write_lock` (the caller may hold it), only store transactions;
/// repeatable (unique per post and position: already existing positions are skipped); no commit
/// (`befunde.md` is in the next snapshot).
///
/// - Parsing uses [`parse::split_answer`]; if the answer yields no finding and the human converts
///   (`manual`), the whole text becomes a note ([`parse::whole_answer`]).
/// - Similarity (contract R28 §1.4, only in the section): if a new finding resembles ([`parse::similar`]) a
///   decided, not attached finding of the same section (adopted, rejected, postponed, resolved) or
///   an open one of the same persona there (from another post), it is attached (`similar_to`, does not count
///   as open); decided ones take precedence over open ones. Thread posts without a section are not compared.
/// - `summary`: „[B<id>]“ in the text become `summary_of` (UUIDs of the named findings of this work; unknown ones
///   are dropped).
/// - Round 30b: solutions below a finding (`statt:`/`besser:` or `Schritt:`, [`crate::assist::solutions`])
///   are stored by `Store::finding_set_solutions` per finding; the closing block „Mein Vorschlag:“ of the lector
///   yields no findings.
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
        // Round 30b: solutions per finding (`statt`/`besser` or `Schritt`); if that fails, the finding stays.
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

/// Flag in `store_meta`: the backfill has run.
pub const BACKFILL_KEY: &str = "findings_backfill";

/// Once per database (`store_meta` [`BACKFILL_KEY`]): parse all posts and thread posts of personas (author is a
/// handle from `persona`; not the human, not `@mcp:…`) without changing them, oldest first. Returns the
/// number of created findings (also attached ones). Like [`ingest`] without write lock and without commit. If
/// a post fails, the backfill aborts and the flag stays unset: the next start tries again, positions already
/// created are skipped.
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

/// Address of a finding: `/werke/{work}/befunde?befund={id}`.
pub fn finding_url(work: WorkId, finding: &Finding) -> String {
    format!("/werke/{work}/befunde?befund={}", finding.id)
}

/// Finding as JSON: the fields of [`Finding`] plus `kind_label`, `status_label`, `url`.
pub fn finding_json(work: WorkId, finding: &Finding) -> serde_json::Value {
    let mut value = json!(finding);
    value["kind_label"] = json!(finding.kind.label());
    value["status_label"] = json!(finding.status.label());
    value["url"] = json!(finding_url(work, finding));
    value
}
