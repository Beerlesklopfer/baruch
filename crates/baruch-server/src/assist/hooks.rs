// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einhängepunkte für die Stellenprüfung (Vertrag Runde 10, §5.10), verdrahtet mit
//! [`crate::scripture::facts_for_work`] bzw. [`crate::scripture::mark_answer_for_work`] (mit der Bibel des
//! Werks, seit 2026-11-06), und – Runde 12, §2.3 – für die Bedeutungssuche der Personas und der
//! MCP-Werkzeuge `rag_search`/`rag_status` ([`rag_search`], [`rag_status`]), und – Runde 13 – für die Websuche
//! von `@belege` ([`web_research`], [`save_web_evidence`]), und – Runde 15 – für die Belege aus SWORD
//! ([`bible_research`], Daten aus [`crate::bible_research::research`]).

use baruch_rag::{Answer, RagError, SearchRequest, Status};
use baruch_store::WorkId;

use crate::AppState;

/// Faktenblock für den System-Prompt (nur `@schrift`/`@belege` bekommen einen); `None` = keiner.
pub async fn scripture_facts(state: &AppState, work: WorkId, handle: &str, section: &str) -> Option<String> {
    crate::scripture::facts_for_work(state, work, handle, section).await
}

/// Antwort einer Persona nachbearbeiten (Hinweis zum Wortlaut, getrennte Ziffern einer Stelle).
pub async fn mark_answer(state: &AppState, work: WorkId, handle: &str, section: &str, body: String) -> String {
    crate::scripture::mark_answer_for_work(state, work, handle, section, body).await
}

/// Bedeutungssuche über die Wege des Werks (Personas mit RAG-Belegen, MCP `rag_search`).
pub async fn rag_search(state: &AppState, request: &SearchRequest) -> Result<Answer, RagError> {
    crate::rag::search(state, request).await
}

/// Zustand der Wege eines Werks ohne Netz (MCP `rag_status`).
pub async fn rag_status(state: &AppState, work: WorkId) -> Result<Status, RagError> {
    crate::rag::status(state, work).await
}

/// Belege aus SWORD (Bibel, Kommentare, Lexika) zu den Stellen in `text` für die Persona `handle` (Runde 15).
/// `None` = die Persona bekommt keinen Block; Schalter, Erlaubnis und Grenzen entscheidet allein
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

/// Websuche für `@belege` (Runde 13): bevorzugte Quellen, dann das Web; abgerufene Seiten für Block und Prüfung.
pub async fn web_research(state: &AppState, query: &str) -> crate::websearch::Research {
    crate::websearch::research(state, query).await
}

/// Belege eines gespeicherten Beitrags in `web_evidence` schreiben. Der Beitrag steht schon; scheitert das
/// Schreiben, meldet stderr es (die Kennzeichnung im Beitrag bleibt gültig).
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
