// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hook points for the reference check (contract Round 10, §5.10), wired to
//! [`crate::scripture::facts_for_work`] and [`crate::scripture::mark_answer_for_work`] (with the Bible of the
//! work, since 2026-11-06), and – Round 12, §2.3 – for the semantic search of the personas and the
//! MCP tools `rag_search`/`rag_status` ([`rag_search`], [`rag_status`]), and – Round 13 – for the web search
//! of `@belege` ([`web_research`], [`save_web_evidence`]), and – Round 15 – for the evidence from SWORD
//! ([`bible_research`], data from [`crate::bible_research::research`]).

use baruch_rag::{Answer, RagError, SearchRequest, Status};
use baruch_store::WorkId;

use crate::AppState;

/// Fact block for the system prompt (only `@schrift`/`@belege` get one); `None` = none.
pub async fn scripture_facts(state: &AppState, work: WorkId, handle: &str, section: &str) -> Option<String> {
    crate::scripture::facts_for_work(state, work, handle, section).await
}

/// Post-process a persona's answer (note on the wording, separated digits of a reference).
pub async fn mark_answer(state: &AppState, work: WorkId, handle: &str, section: &str, body: String) -> String {
    crate::scripture::mark_answer_for_work(state, work, handle, section, body).await
}

/// Semantic search over the ways of the work (personas with RAG evidence, MCP `rag_search`).
pub async fn rag_search(state: &AppState, request: &SearchRequest) -> Result<Answer, RagError> {
    crate::rag::search(state, request).await
}

/// State of the ways of a work without network (MCP `rag_status`).
pub async fn rag_status(state: &AppState, work: WorkId) -> Result<Status, RagError> {
    crate::rag::status(state, work).await
}

/// Evidence from SWORD (Bible, commentaries, lexicons) on the references in `text` for the persona `handle` (Round 15).
/// `None` = the persona gets no block; switch, permission and limits are decided solely by
/// [`crate::bible_research::research`].
pub async fn bible_research(
    state: &AppState,
    work: WorkId,
    handle: &str,
    text: &str,
    max_chars: usize,
) -> Option<crate::bible_research::Research> {
    crate::bible_research::research(state, work, handle, text, max_chars).await
}

/// Web search for `@belege` (Round 13): preferred sources, then the web; fetched pages for block and check.
pub async fn web_research(state: &AppState, query: &str) -> crate::websearch::Research {
    crate::websearch::research(state, query).await
}

/// Write the evidence of a stored post into `web_evidence`. The post already exists; if writing
/// fails, stderr reports it (the marking in the post stays valid).
pub async fn save_web_evidence(
    state: &AppState,
    post: i64,
    work: WorkId,
    key: i64,
    persona: &str,
    research: &crate::websearch::Research,
    drafts: &[crate::websearch::verify::Draft],
) {
    let pool = state.store.pool();
    if let Err(e) = crate::websearch::evidence::record(pool, post, work, key, persona, research, drafts).await {
        eprintln!("Websuche: Belege zu Beitrag {post} nicht gespeichert: {e}");
    }
}
