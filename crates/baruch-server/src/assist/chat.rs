// SPDX-License-Identifier: AGPL-3.0-or-later
//! Section chat with the personas (contract Round 10; since Round 15, interface S2, its own file):
//! mentioned personas ([`mentions`]), running a chat message ([`chat`]) and the same from RPC parameters
//! ([`run_chat`], [`chat_json`]). The sender's post is stored immediately and goes to the caller as
//! [`Progress::Posted`].
//!
//! Round 28 (chat-auftrag), orders in the chat:
//! - **RFC** ([`is_rfc`]): „RFC“ at the start of the message or „@alle“ calls every active persona exactly once, the
//!   lector (by setting) last with the answers of the others framed as data
//!   ([`super::OrderFrame::rfc_answers`]) and the order to collect and weigh; its findings become
//!   collective findings (`rfc_summary`). [`MAX_CHAT_PERSONAS`] does not apply here, the model queue does
//!   (the personas answer one after another).
//! - **Write order** ([`is_write_order`] or `as_order`): the mentioned persona (with `as_order` and no mention the
//!   lector) writes a **proposal** for the section ([`Mode::Propose`], never adopted) and a short
//!   post appears in the chat ([`order_note`]). Deleting and new sections stay with the human (outline). If the
//!   message names findings with „[B<id>]“, Baruch proposes „übernommen in Fassung n“ for each
//!   ([`crate::findings::rpc::propose`], origin the persona); the human confirms. Since Round 23 only
//!   references from the message count, no longer those from the model text of the proposal.
//! - Without either it stays a post per mentioned persona (at most [`MAX_CHAT_PERSONAS`]).
//!
//! Round 30 (hooks of interface R30-S1): all personas of a call review the same revision
//! ([`super::basis::choose`], [`ChatOrder::basis`]) and answer the sender's post (`links.reply_to`);
//! a variants order ([`super::variants::is_variants_order`]) goes to [`super::variants::run`]; after the lector
//! in an RFC [`super::solutions::rfc_proposal`] optionally creates a collective proposal.
//!
//! Round 30c (chat-bedienung):
//! - **Address** ([`addressed`]): besides `@handle` (anywhere, as before) the name or the handle without „@“ counts
//!   **at the start of a sentence or post**, followed by „,“ or „:“ („Lektor, …“, „Exeget:“), case-insensitive; not in
//!   running text („der Lektor meint“ calls nobody).
//! - **Relay** ([`line_addresses`]): if a line of a persona answer starts with `@handle:` or an address, the
//!   addressed persona answers once – one level (relay answers call nobody), at most
//!   [`MAX_RELAY_ANSWERS`] relay answers per original message, every persona at most once per message (whoever already
//!   answered or failed is not called again), not in an RFC and not in a write order. The
//!   relay post carries `relay_from` (the addressing persona) and `reply_to` (its post), the order carries the answer
//!   framed ([`OrderFrame::relay`]); visible as „antwortet auf Beitrag n von @…“.
//! - **Errors** of every persona are noted by [`crate::chat_live::record_failure`] for the live view (`post.since`).
//! - **Review basis** from RPC and form: `basis` = `shown` | `current` | `<n>` ([`run_chat`]).
//!
//! Round 23 (mcp-namen):
//! - **Quote lines** („> …“, [`unquoted`]) call nobody and make neither an RFC nor a write order.
//! - **One relay path** ([`relay_targets`]): address at the start of a line for all, with [`ChatOrder::relay`] (MCP
//!   `chat`) also every mention; `relay_section` in `mcp/tools.rs` no longer exists.
//! - `persona.chat` returns `done_proposals`, `done_notes`, `all_done` ([`chat_json`]) if a persona reported own
//!   findings as resolved.

use baruch_store::{IdeaOrigin, PersonaRow, WorkId};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    FramedAnswer, MAX_CHAT_PERSONAS, Mode, Order, OrderFrame, Outcome, PostLinks, Progress, SavedPost, Scope,
    check_text, context, post, run, send,
};
use crate::AppState;
use crate::rpc::{INVALID_PARAMS, RpcError};

/// Mention that calls all active personas in an RFC.
pub const ALL_MENTION: &str = "@alle";

/// At most this many relay answers per original message (Round 30c).
pub const MAX_RELAY_ANSWERS: usize = 3;

/// Is `c` a character of a handle?
fn handle_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Does `text` start (case-insensitively) with `word` as a whole word? Then the bytes after it.
fn starts_with_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let n = word.chars().count();
    if n == 0 {
        return None;
    }
    let head: String = text.chars().take(n).collect();
    if head.chars().count() < n || head.to_lowercase() != word.to_lowercase() {
        return None;
    }
    let rest = &text[head.len()..];
    rest.chars().next().is_none_or(|c| !handle_char(c)).then_some(rest)
}

/// Does a „,“ or „:“ follow (after whitespace and `**`)?
fn followed_by_address_mark(rest: &str) -> bool {
    let rest = rest.trim_start().trim_start_matches("**").trim_start();
    rest.starts_with(',') || rest.starts_with(':')
}

/// Address at the start of `text`: name or handle without „@“ of an active persona, followed by „,“ or „:“.
fn address_at(text: &str, personas: &[PersonaRow]) -> Option<String> {
    let text = text.trim_start_matches("**");
    personas.iter().filter(|p| p.active).find_map(|p| {
        let bare = p.handle.trim_start_matches('@');
        [p.name.trim(), bare]
            .into_iter()
            .filter(|w| !w.is_empty())
            .any(|w| starts_with_word(text, w).is_some_and(followed_by_address_mark))
            .then(|| p.handle.clone())
    })
}

/// Sentence starts in `message` (bytes): start of the post, after a line break and after „.“, „!“, „?“ with whitespace.
fn sentence_starts(message: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut expect = true;
    let mut after_mark = false;
    for (i, c) in message.char_indices() {
        if c == '\n' {
            expect = true;
            after_mark = false;
            continue;
        }
        if c.is_whitespace() {
            if after_mark {
                expect = true;
                after_mark = false;
            }
            continue;
        }
        if expect {
            out.push(i);
            expect = false;
        }
        after_mark = matches!(c, '.' | '!' | '?');
    }
    out
}

/// The message without quote lines (Round 23): lines that – after whitespace – start with „>“ are foreign text
/// („Zitieren“ puts someone else's post as „> …“ at the start of one's own message). They call nobody,
/// make no RFC and no write order: [`addressed`], [`mentions`], [`is_rfc`] and [`is_write_order`] read
/// only the rest. Without a quote line the text stays unchanged (borrowed).
pub fn unquoted(message: &str) -> std::borrow::Cow<'_, str> {
    if !message.lines().any(|line| line.trim_start().starts_with('>')) {
        return std::borrow::Cow::Borrowed(message);
    }
    let kept: Vec<&str> = message.lines().filter(|line| !line.trim_start().starts_with('>')).collect();
    std::borrow::Cow::Owned(kept.join("\n"))
}

/// Addressed personas (Round 30c): `@handle` anywhere (like [`mentions`]) plus name or handle without „@“ at the
/// start of a sentence or post with „,“ or „:“ („Lektor, …“, „Exeget:“). Active ones only, no duplicates, in message
/// order, at most [`MAX_CHAT_PERSONAS`]. „der Lektor meint“ in running text does not count. Quote lines („> …“)
/// do not count ([`unquoted`], Round 23).
pub fn addressed(message: &str, personas: &[PersonaRow]) -> Vec<String> {
    let message = unquoted(message);
    let message = message.as_ref();
    let mut hits: Vec<(usize, String)> = Vec::new();
    let mut offset = 0;
    while let Some(at) = message[offset..].find('@') {
        let start = offset + at;
        let after = &message[start + 1..];
        let end = after.find(|c: char| !handle_char(c)).unwrap_or(after.len());
        let word = after[..end].to_lowercase();
        offset = start + 1 + end;
        if let Some(p) = personas.iter().find(|p| p.active && p.handle.trim_start_matches('@').to_lowercase() == word)
            && !word.is_empty()
        {
            hits.push((start, p.handle.clone()));
        }
    }
    for start in sentence_starts(message) {
        if let Some(handle) = address_at(&message[start..], personas) {
            hits.push((start, handle));
        }
    }
    hits.sort_by_key(|(at, _)| *at);
    let mut found: Vec<String> = Vec::new();
    for (_, handle) in hits {
        if !found.contains(&handle) {
            found.push(handle);
            if found.len() == MAX_CHAT_PERSONAS {
                break;
            }
        }
    }
    found
}

/// Personas that an answer addresses at the **start of a line** (relay, Round 30c): `@handle:` or `@handle,` or an
/// address (name or handle without „@“ with „,“/„:“), also behind list markers and `**`. Quote lines („> …“)
/// do not count. Active ones only, no duplicates, in the order of the answer.
pub fn line_addresses(answer: &str, personas: &[PersonaRow]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in answer.lines() {
        let mut rest = line.trim_start();
        if rest.starts_with('>') {
            continue;
        }
        loop {
            let before = rest;
            for prefix in ["- ", "* ", "• ", "– ", "+ ", "**"] {
                rest = rest.strip_prefix(prefix).unwrap_or(rest).trim_start();
            }
            if rest == before {
                break;
            }
        }
        let handle = if let Some(after) = rest.strip_prefix('@') {
            let end = after.find(|c: char| !handle_char(c)).unwrap_or(after.len());
            let word = after[..end].to_lowercase();
            personas
                .iter()
                .find(|p| p.active && !word.is_empty() && p.handle.trim_start_matches('@').to_lowercase() == word)
                .filter(|_| followed_by_address_mark(&after[end..]))
                .map(|p| p.handle.clone())
        } else {
            address_at(rest, personas)
        };
        if let Some(handle) = handle
            && !found.contains(&handle)
        {
            found.push(handle);
        }
    }
    found
}

/// Order for the addressed persona in a relay (built by Baruch, not by the model; the answer itself stands
/// framed in [`OrderFrame::relay`]).
pub fn relay_text(from: &str, post: i64, message: &str) -> String {
    let text = format!(
        "{from} addressed you directly in post {post}. Respond to this address (the answer of {from} stands as data \
         in the chat). The message of the author was: {message}"
    );
    text.chars().take(super::MAX_QUESTION_CHARS).collect()
}

/// Personas mentioned in a message (`@handle`, case-insensitive): active ones only, no
/// duplicates, in message order, at most [`MAX_CHAT_PERSONAS`]. Quote lines („> …“) do not count
/// ([`unquoted`], Round 23).
pub fn mentions(message: &str, personas: &[PersonaRow]) -> Vec<String> {
    let message = unquoted(message);
    let mut found: Vec<String> = Vec::new();
    let mut rest = message.as_ref();
    while let Some(at) = rest.find('@') {
        let after = &rest[at + 1..];
        let end = after.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')).unwrap_or(after.len());
        let word = after[..end].to_lowercase();
        rest = &after[end..];
        if word.is_empty() {
            continue;
        }
        if let Some(p) = personas.iter().find(|p| p.active && p.handle.trim_start_matches('@').to_lowercase() == word)
            && !found.contains(&p.handle)
        {
            found.push(p.handle.clone());
            if found.len() == MAX_CHAT_PERSONAS {
                break;
            }
        }
    }
    found
}

/// Does the message call all active personas (RFC)? „RFC“ as the first word (case-insensitive, also „RFC:“)
/// or „@alle“ as a mention anywhere (whole word: „@allerlei“ does not count). Quote lines („> …“) do not count
/// ([`unquoted`], Round 23): „> @alle …“ from a quoted post calls nobody.
pub fn is_rfc(message: &str) -> bool {
    let message = unquoted(message);
    let message = message.as_ref();
    let first: String = message.trim_start().chars().take_while(|c| c.is_alphanumeric()).collect();
    if first.eq_ignore_ascii_case("rfc") {
        return true;
    }
    let lower = message.to_lowercase();
    lower.match_indices(ALL_MENTION).any(|(at, m)| {
        lower[at + m.len()..].chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-'))
    })
}

/// Imperatives of a write order (lower case, whole first word after mentions and „bitte“).
const WRITE_VERBS: &[&str] = &[
    "schreib",
    "schreibe",
    "formulier",
    "formuliere",
    "erstell",
    "erstelle",
    "verfass",
    "verfasse",
    "überarbeite",
    "überarbeit",
    "kürze",
    "kürz",
    "ergänze",
    "ergänz",
    "entwirf",
    "entwerfe",
    "glätte",
    "straffe",
    "redigiere",
    "übersetze",
];

/// Is the message a write order („@lektor Schreibe …“, „Bitte formuliere …“)? Recognised by the first word after
/// mentions (`@…`), punctuation and „bitte“ (imperative of a writing verb, [`WRITE_VERBS`]). Questions („Kannst du
/// schreiben, …?“) or messages where the verb only comes later are not orders – for those there is
/// „als Auftrag“ (`as_order`). Quote lines („> …“) do not count ([`unquoted`], Round 23).
pub fn is_write_order(message: &str) -> bool {
    let message = unquoted(message);
    let words = message
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ':' | ';' | '!' | '.'))
        .filter(|w| !w.is_empty());
    for word in words {
        let lower = word.to_lowercase();
        if lower.starts_with('@') || lower == "bitte" {
            continue;
        }
        return WRITE_VERBS.contains(&lower.as_str());
    }
    false
}

/// Word stems of a check order (lower case): „prüf den Vorschlag“, „bitte überprüfen“, „check das“, „Review bitte“.
const CHECK_STEMS: &[&str] =
    &["prüf", "überprüf", "nachprüf", "gegenprüf", "kontrollier", "check", "review", "verif", "examin", "proofread"];

/// Is the message to a directly addressed persona a check order (Round 36)? A word starts with one of the
/// [`CHECK_STEMS`] (anywhere in the message, also as a question „Kannst du das prüfen?“), or the message names
/// a finding („[B12]“). Only then does the persona see its own open findings ([`context::is_recheck`]); with
/// a plain question („@nathan wie alt war Mose?“) it does not – otherwise every question would pull the old findings
/// into the answer. Mentions (`@…`) and quote lines („> …“, [`unquoted`]) do not count.
pub fn is_check_order(message: &str) -> bool {
    let message = unquoted(message);
    if !finding_refs(&message).is_empty() {
        return true;
    }
    message
        .split(|c: char| !(c.is_alphanumeric() || c == '@' || c == '_' || c == '-'))
        .filter(|word| !word.is_empty() && !word.starts_with('@'))
        .any(|word| {
            let lower = word.to_lowercase();
            CHECK_STEMS.iter().any(|stem| lower.starts_with(stem))
        })
}

/// „[B<id>]“ in a text (no duplicates, in text order).
pub fn finding_refs(text: &str) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("[B") {
        let after = &rest[at + 2..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        rest = &after[digits.len()..];
        if !digits.is_empty()
            && rest.starts_with(']')
            && let Ok(id) = digits.parse::<i64>()
            && !out.contains(&id)
        {
            out.push(id);
        }
    }
    out
}

/// Short post of the persona about its proposal (Round 28). Baruch writes it (no second model call); it
/// starts with no finding marker so that parsing yields no finding from it.
pub fn order_note(rev: u32) -> String {
    format!(
        "Fassung {rev} dieses Abschnitts liegt als Vorschlag bereit (noch nicht übernommen). Übernehmen oder verwerfen \
         kann nur der Mensch; neue Abschnitte anlegen und löschen bleiben bei ihm (Gliederung)."
    )
}

/// A proposal created by a write order in the chat.
#[derive(Debug, Clone, Serialize)]
pub struct ChatProposal {
    pub persona: String,
    pub rev: u32,
    pub url: String,
}

/// Result of a chat message.
#[derive(Debug, Clone, Serialize)]
pub struct ChatOutcome {
    /// The sender's post, then per persona its answer.
    pub posts: Vec<SavedPost>,
    pub answers: Vec<Outcome>,
    /// Personas whose answer failed, with message (the other posts stay).
    pub errors: Vec<ChatError>,
    pub url: String,
    /// Round 28: RFC or „@alle“ (all active personas, the lector last).
    pub rfc: bool,
    /// Round 28: proposals from write orders.
    pub proposals: Vec<ChatProposal>,
    /// Round 28: findings from the answers (ids, `[B<id>]`).
    pub findings: Vec<i64>,
    /// Round 28: notes that concern no persona (e.g. a status proposal for a finding failed).
    pub notes: Vec<String>,
    /// Personas called by the relay level (also failed ones). Their answers call nobody any more – the
    /// MCP tool `chat` adds mentions only from the other answers (one level).
    #[serde(skip)]
    pub relayed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatError {
    pub persona: String,
    pub message: String,
}

/// A chat message: work, section (`key`), text, shown revision, sender and optionally a
/// persona that should answer even without a mention (MCP `chat {persona}`).
#[derive(Debug, Clone)]
pub struct ChatOrder<'a> {
    pub work: WorkId,
    pub key: i64,
    pub message: &'a str,
    pub rev: Option<u32>,
    pub author: &'a str,
    pub extra: Option<&'a str>,
    /// Like [`Order::web`]: allow web search (UI yes, MCP never).
    pub web: bool,
    /// Round 28: the message is an order (write orders become proposals); MCP and default false.
    pub as_order: bool,
    /// Round 30b: review basis of all personas of this call (default: the shown revision).
    pub basis: super::basis::BasisChoice,
    /// Round 23: relay also on a mere mention (`@handle` anywhere in an answer) – this is how MCP `chat` has called
    /// since Round 15. Without it (UI) only the address at the start of a line calls ([`line_addresses`]). Both run
    /// through the same relay level ([`relay_targets`]).
    pub relay: bool,
}

/// Who answers in an RFC: all active personas in their order, the lector (by setting, if active)
/// taken out and returned as the second result.
pub async fn rfc_personas(state: &AppState) -> Result<(Vec<String>, Option<String>), RpcError> {
    let (rows, _, lector) = crate::personas::rows_and_lector(state).await?;
    let lector = lector.filter(|l| rows.iter().any(|p| p.active && &p.handle == l));
    let others =
        rows.iter().filter(|p| p.active && Some(&p.handle) != lector.as_ref()).map(|p| p.handle.clone()).collect();
    Ok((others, lector))
}

/// Chat message: first the post of `author` (stored immediately), then one after another per mentioned
/// active persona (plus `extra`, if given) its answer as a post – in an RFC all active ones, in a
/// write order a proposal with a short post (see module doc). Answers of the personas call no further
/// personas.
pub async fn chat(
    state: &AppState,
    order: &ChatOrder<'_>,
    progress: Option<&UnboundedSender<Progress>>,
) -> Result<ChatOutcome, RpcError> {
    let ChatOrder { work, key, message, rev, author, extra, web, as_order, basis, relay } = order.clone();
    check_text(message, "Die Nachricht")?;
    let message = message.trim();
    if message.is_empty() {
        return Err(RpcError::new(INVALID_PARAMS, "Die Nachricht ist leer."));
    }
    let rfc = is_rfc(message);
    let write = !rfc && (as_order || is_write_order(message));
    let rows = state.store.persona_list().await?;
    let mut addressed = Vec::new();
    let mut lector_last: Option<String> = None;
    if rfc {
        let (others, lector) = rfc_personas(state).await?;
        addressed = others;
        lector_last = lector;
    } else {
        if let Some(extra) = extra.filter(|e| !e.trim().is_empty()) {
            addressed.push(context::active_persona(state, extra).await?.handle);
        }
        // Round 30c: `@handle` anywhere and address by name at the start of a sentence.
        for handle in self::addressed(message, &rows) {
            if !addressed.contains(&handle) && addressed.len() < MAX_CHAT_PERSONAS {
                addressed.push(handle);
            }
        }
        // „als Auftrag“ without a mention: the lector writes.
        if write && as_order && addressed.is_empty() {
            addressed.extend(rfc_personas(state).await?.1);
        }
    }
    // Round 30b: one review basis for all personas of this call.
    let basis = super::basis::choose(state, work, key, rev, basis).await?;
    let first = {
        let _held = state.write_lock.lock().await;
        let seq = state.store.node_seq(work, key).await?;
        let id = state.store.add_post(work, seq, &post(author, message, vec![])).await?;
        SavedPost { id, author: author.to_owned(), body: message.to_owned() }
    };
    send(progress, Progress::Posted(first.clone()));
    let mut result = ChatOutcome {
        posts: vec![first],
        answers: Vec::new(),
        errors: Vec::new(),
        url: crate::view::node_url(work, key, rev, false, "", ""),
        rfc,
        proposals: Vec::new(),
        findings: Vec::new(),
        notes: Vec::new(),
        relayed: Vec::new(),
    };
    let first_id = result.posts[0].id;
    let base = |handle: &str, mode: Mode, frame: OrderFrame| Order {
        work,
        key,
        persona: handle.to_owned(),
        mode,
        text: message.to_owned(),
        rev: Some(basis.rev),
        asker: Some(author.to_owned()),
        post_question: false,
        sources: vec![],
        web,
        scope: Scope::Section,
        frame,
        links: PostLinks { reply_to: Some(first_id), ..Default::default() },
        basis: Some(basis.clone()),
    };
    // Round 30b: variants order („Varianten …“) instead of the pass through the personas.
    if super::variants::is_variants_order(message) {
        let variant_order = super::variants::VariantOrder {
            work,
            key,
            rev: Some(basis.rev),
            findings: finding_refs(message),
            count: None,
            asker: author.to_owned(),
            from_post: Some(first_id),
        };
        match super::variants::run(state, &variant_order, progress).await {
            Ok(outcome) => {
                for variant in &outcome.variants {
                    result.proposals.push(ChatProposal {
                        persona: outcome.persona.clone(),
                        rev: variant.rev,
                        url: variant.url.clone(),
                    });
                }
                result.notes.extend(outcome.notes);
            }
            Err(e) => {
                let persona = rfc_personas(state).await.ok().and_then(|(_, lector)| lector).unwrap_or_default();
                fail(&mut result, work, key, persona, e.message);
            }
        }
        return Ok(result);
    }
    let mut framed: Vec<FramedAnswer> = Vec::new();
    // Round 30c: answers to the message (for the relay) and who already had a turn.
    let mut answered: Vec<FramedAnswer> = Vec::new();
    let mut called: Vec<String> = addressed.clone();
    for handle in addressed {
        if write {
            let order = base(&handle, Mode::Propose, OrderFrame { as_order: true, ..Default::default() });
            match write_order(state, &order, message, progress, &mut result).await {
                Ok(()) => {}
                Err(e) => fail(&mut result, work, key, handle, e.message),
            }
            continue;
        }
        // Round 22: in an RFC every persona reviews again and sees its own open findings of the section. Bug
        // 0.9.0 (critique cycle): also a directly addressed persona („@nathan prüf den Vorschlag“) – otherwise the
        // critic reported its old findings on the proposal again instead of rechecking them.
        // Round 36 (note 0.9.0): only in an RFC and for a check order ([`is_check_order`]). A plain question
        // to the persona no longer pulls its open findings into the context.
        let recheck = rfc || is_check_order(message);
        let order = base(&handle, Mode::Chat, OrderFrame { recheck, ..OrderFrame::default() });
        match run(state, &order, progress).await {
            Ok(outcome) => {
                let answer = FramedAnswer {
                    persona: outcome.persona.clone(),
                    post: outcome.post.as_ref().map(|p| p.id),
                    text: outcome.text.clone(),
                    findings: outcome.findings.clone(),
                };
                if rfc {
                    framed.push(answer);
                } else {
                    answered.push(answer);
                }
                keep(&mut result, outcome);
            }
            Err(e) => fail(&mut result, work, key, handle, e.message),
        }
    }
    // Round 30c: one relay level (not in an RFC, not in a write order). Round 23: the only relay path of the
    // section chat – address at the start of a line for all, with `relay` (MCP) also every mention.
    if !rfc && !write {
        for (target, from) in relay_targets(&answered, &rows, &mut called, relay) {
            result.relayed.push(target.clone());
            let post_id = from.post.unwrap_or_default();
            let frame = OrderFrame { relay: Some(from.clone()), ..Default::default() };
            let mut order = base(&target, Mode::Chat, frame);
            order.text = relay_text(&from.persona, post_id, message);
            order.asker = Some(from.persona.clone());
            order.links =
                PostLinks { reply_to: Some(post_id), repeat_of: None, relay_from: Some(from.persona.clone()) };
            match run(state, &order, progress).await {
                Ok(outcome) => keep(&mut result, outcome),
                Err(e) => fail(&mut result, work, key, target, e.message),
            }
        }
    }
    if let Some(lector) = lector_last {
        let frame = OrderFrame { as_order: false, rfc_answers: framed, rfc_summary: true, relay: None, recheck: true };
        let lector_order = base(&lector, Mode::Chat, frame);
        match run(state, &lector_order, progress).await {
            Ok(outcome) => {
                // Round 30b: „Mein Vorschlag:“ of the lector as one collective proposal.
                match super::solutions::rfc_proposal(state, &lector_order, &outcome, progress).await {
                    Ok(Some(p)) => result.proposals.push(p),
                    Ok(None) => {}
                    Err(e) => result.notes.push(e.message),
                }
                keep(&mut result, outcome);
            }
            Err(e) => fail(&mut result, work, key, lector, e.message),
        }
    }
    Ok(result)
}

/// Whom the relay level calls (Round 23: one path for the UI and MCP, before that additionally `relay_section` in
/// `mcp/tools.rs`): per answer to the message (`answered`, stored ones only) first the addresses at the start of a
/// line ([`line_addresses`], together at most [`MAX_RELAY_ANSWERS`]), with `mentions_too` (MCP `chat`) then every
/// further mention ([`mentions`]) as long as at most [`MAX_CHAT_PERSONAS`] personas are called in total. Nobody is
/// called twice (`called` = who already had a turn; the chosen ones are added), no persona calls itself.
/// Relay answers call nobody any more – they are not in `answered`.
pub fn relay_targets(
    answered: &[FramedAnswer],
    personas: &[PersonaRow],
    called: &mut Vec<String>,
    mentions_too: bool,
) -> Vec<(String, FramedAnswer)> {
    let mut queue: Vec<(String, FramedAnswer)> = Vec::new();
    let mut direct_count = 0usize;
    for answer in answered {
        if answer.post.is_none() {
            continue;
        }
        let direct = line_addresses(&answer.text, personas);
        let mut candidates: Vec<(String, bool)> = direct.iter().map(|h| (h.clone(), true)).collect();
        if mentions_too {
            for m in mentions(&answer.text, personas) {
                if !direct.contains(&m) {
                    candidates.push((m, false));
                }
            }
        }
        for (target, is_direct) in candidates {
            if target == answer.persona || called.contains(&target) {
                continue;
            }
            let allowed = if is_direct { direct_count < MAX_RELAY_ANSWERS } else { called.len() < MAX_CHAT_PERSONAS };
            if !allowed {
                continue;
            }
            if is_direct {
                direct_count += 1;
            }
            called.push(target.clone());
            queue.push((target, answer.clone()));
        }
    }
    queue
}

/// Error of a persona into the result and the live view ([`crate::chat_live::record_failure`]).
fn fail(result: &mut ChatOutcome, work: WorkId, key: i64, persona: String, message: String) {
    crate::chat_live::record_failure(work, key, &persona, &message);
    result.errors.push(ChatError { persona, message });
}

/// Take a persona's answer into the result.
fn keep(result: &mut ChatOutcome, outcome: Outcome) {
    if let Some(post) = &outcome.post {
        result.posts.push(post.clone());
    }
    result.findings.extend(&outcome.findings);
    result.answers.push(outcome);
}

/// Carry out a write order: proposal for the section (never adopted), short post of the persona, status proposals
/// „übernommen in Fassung n“ for the findings named in the **message** (`[B<id>]`; since Round 23 no longer from
/// the text of the proposal).
async fn write_order(
    state: &AppState,
    order: &Order,
    message: &str,
    progress: Option<&UnboundedSender<Progress>>,
    result: &mut ChatOutcome,
) -> Result<(), RpcError> {
    let outcome = run(state, order, progress).await?;
    let Some(rev) = outcome.rev else {
        return Err(RpcError::new(crate::rpc::INTERNAL, "Der Vorschlag hat keine Fassungsnummer."));
    };
    let handle = outcome.persona.clone();
    let body = order_note(rev);
    let saved = {
        let _held = state.write_lock.lock().await;
        let seq = state.store.node_seq(order.work, order.key).await?;
        let id = state
            .store
            .add_post(order.work, seq, &post(&handle, &body, vec![format!("Vorschlag Fassung {rev}")]))
            .await?;
        SavedPost { id, author: handle.clone(), body }
    };
    // Round 23: references only from the message of the requester. `[B<id>]` in the model text of the proposal creates
    // no status proposal any more – the model could otherwise report findings as „übernommen“ that the order never
    // named (also through text smuggled into the section).
    let refs = finding_refs(message);
    let by = IdeaOrigin::Persona { handle: handle.clone() };
    for id in refs {
        // The revision number is valid only in the section of the order: a finding from another section would otherwise
        // get a foreign revision with the same number proposed as „übernommen“.
        match state.store.finding_get(order.work, id).await {
            Ok(finding) if finding.key == Some(order.key) => {}
            Ok(_) => {
                result.notes.push(format!(
                    "[B{id}]: Der Befund gehört nicht zu diesem Abschnitt; kein Statusvorschlag zu Fassung {rev}."
                ));
                continue;
            }
            Err(e) => {
                result.notes.push(format!("[B{id}]: {}", RpcError::from(e).message));
                continue;
            }
        }
        let change = baruch_store::StatusChange::Adopted { rev };
        let note = format!("Vorschlag Fassung {rev} aus dem Chat");
        if let Err(e) = crate::findings::rpc::propose(state, order.work, id, &change, &by, &note).await {
            result.notes.push(format!("[B{id}]: {}", e.message));
        }
    }
    result.proposals.push(ChatProposal { persona: handle, rev, url: outcome.url.clone() });
    result.posts.push(saved);
    result.answers.push(outcome);
    Ok(())
}

/// Answer of a chat message as JSON; `compressed` (Round 15) is the first compression of an answer
/// (`null` if no persona saw chunks). Round 28: `rfc`, `proposals` (`{persona, rev, url}`), `findings`
/// (ids) and `notes`.
pub fn chat_json(outcome: &ChatOutcome) -> Value {
    let compressed = outcome.answers.iter().find_map(|a| a.compressed.as_ref());
    let mut value = json!({ "posts": outcome.posts, "errors": outcome.errors, "url": outcome.url,
            "truncated": outcome.answers.iter().any(|a| a.truncated), "cut": outcome.answers.iter().any(|a| a.cut),
            "compressed": compressed, "rfc": outcome.rfc, "proposals": outcome.proposals,
            "findings": outcome.findings, "notes": outcome.notes });
    add_done(&mut value, &outcome.answers);
    value
}

/// Round 23: feedback „Erledigt: Befund n“ from the answers into the output (names as in `persona.ask`):
/// `done_proposals` (finding numbers with status proposal „erledigt“, no duplicates), `done_notes` (skipped
/// feedback with reason) and `all_done` (at least one persona says all its findings seem resolved). Each
/// field only if it is not empty or `true`.
pub fn add_done(value: &mut Value, answers: &[Outcome]) {
    let mut proposals: Vec<i64> = Vec::new();
    for id in answers.iter().flat_map(|a| a.done_proposals.iter().copied()) {
        if !proposals.contains(&id) {
            proposals.push(id);
        }
    }
    let notes: Vec<&String> = answers.iter().flat_map(|a| a.done_notes.iter()).collect();
    if !proposals.is_empty() {
        value["done_proposals"] = json!(proposals);
    }
    if !notes.is_empty() {
        value["done_notes"] = json!(notes);
    }
    if answers.iter().any(|a| a.all_done) {
        value["all_done"] = json!(true);
    }
}

/// Run a chat message of the human from RPC parameters.
pub async fn run_chat(
    state: &AppState,
    params: &Value,
    progress: Option<&UnboundedSender<Progress>>,
) -> Result<ChatOutcome, RpcError> {
    let (work, key, rev) = super::rpc::address(state, params).await?;
    let message = super::rpc::text(params, "message")?.unwrap_or_default();
    // Round 28: `as_order` (default false) – the message as an order; from forms also „1“ or „on“.
    let as_order = match params.get("as_order") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.trim(), "1" | "on" | "true"),
        _ => false,
    };
    let basis = basis_param(params)?;
    let order = ChatOrder {
        work,
        key,
        message: &message,
        rev,
        author: crate::mirror::HUMAN_NAME,
        extra: None,
        web: true,
        as_order,
        basis,
        relay: false,
    };
    chat(state, &order, progress).await
}

/// Round 30c: review basis from RPC or form (`basis` = `shown` | `current` | `<n>`, number or text); missing or
/// empty → the shown one.
pub fn basis_param(params: &Value) -> Result<super::basis::BasisChoice, RpcError> {
    let text = match params.get("basis") {
        None | Some(Value::Null) => return Ok(super::basis::BasisChoice::Shown),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) if s.trim().is_empty() => return Ok(super::basis::BasisChoice::Shown),
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "basis: shown, current oder eine Fassungsnummer")),
    };
    super::basis::BasisChoice::parse(&text)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "basis: shown, current oder eine Fassungsnummer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(handle: &str, active: bool) -> PersonaRow {
        PersonaRow {
            handle: handle.into(),
            name: handle.trim_start_matches('@').into(),
            role: String::new(),
            model: "m".into(),
            context_length: 1,
            active,
            color: "#000000".into(),
            chosen: false,
            prompt: None,
        }
    }

    #[test]
    fn mentions_take_active_personas_once_and_at_most_three() {
        let rows = [
            row("@lektor", true),
            row("@exeget", true),
            row("@nathan", false),
            row("@hoerer", true),
            row("@schrift", true),
        ];
        assert_eq!(mentions("@Lektor und @lektor, bitte", &rows), vec!["@lektor"], "ohne Doppelte, Groß/klein egal");
        assert_eq!(
            mentions("@nathan? @unbekannt", &rows),
            Vec::<String>::new(),
            "inaktive und unbekannte zählen nicht"
        );
        assert_eq!(
            mentions("@schrift @hoerer @exeget @lektor", &rows),
            vec!["@schrift", "@hoerer", "@exeget"],
            "höchstens drei, in der Reihenfolge der Nachricht"
        );
        assert_eq!(mentions("mail@lektor.de", &rows), vec!["@lektor"], "auch mitten im Wort – wie im Chat üblich");
    }

    #[test]
    fn rfc_means_rfc_as_first_word_or_the_mention_alle() {
        for yes in ["RFC für den Abschnitt", "rfc: bitte alle", "  RFC", "Was meint ihr, @alle?", "@Alle bitte"] {
            assert!(is_rfc(yes), "{yes:?}: RFC am Anfang bzw. @alle ruft alle (Fund am echten Buch, Beitrag 41)");
        }
        for no in ["Ist das ein RFC?", "RFCs sind gut", "@allerlei Fragen", "@lektor alle Absätze", "alle zusammen"] {
            assert!(!is_rfc(no), "{no:?}: RFC nur als erstes Wort, @alle nur als ganze Erwähnung");
        }
    }

    #[test]
    fn a_write_order_is_an_imperative_writing_verb_after_mentions_and_bitte() {
        for yes in [
            "@lektor Schreibe ein Vorwort",
            "@lektor, bitte formuliere den Schluss neu",
            "Bitte überarbeite den zweiten Absatz",
            "@exeget kürze den Abschnitt auf die Hälfte",
            "Erstelle eine Fassung ohne Fremdwörter",
        ] {
            assert!(is_write_order(yes), "{yes:?}: Imperativ eines Schreibverbs nach Erwähnung/„bitte“");
        }
        for no in [
            "@lektor Wie wirkt der Anfang?",
            "Kannst du das schreiben?",
            "So falle ich mit der Tür ins Haus",
            "@lektor",
            "Schriftlich ist das schwierig",
            "Ergänzend: der Schluss trägt",
        ] {
            assert!(!is_write_order(no), "{no:?}: Frage, Bemerkung oder anderes Wort – kein Auftrag");
        }
    }

    #[test]
    fn finding_references_are_read_once_in_order() {
        assert_eq!(finding_refs("Setzt [B12] und [B3] um, dazu nochmals [B12]."), vec![12, 3], "ohne Doppelte");
        assert_eq!(finding_refs("[B] [Bx] [B4 ] B5] [B6]"), vec![6], "nur [B<Ziffern>] zählt");
    }

    fn named(handle: &str, name: &str) -> PersonaRow {
        PersonaRow { name: name.into(), ..row(handle, true) }
    }

    #[test]
    fn an_address_with_name_at_the_start_of_a_sentence_counts_like_a_mention() {
        let rows = [named("@lektor", "Lektor"), named("@exeget", "Exeget"), named("@schrift", "Schriftkundiger")];
        assert_eq!(addressed("Lektor, was meinst du?", &rows), vec!["@lektor"], "Name mit Komma am Beitragsanfang");
        assert_eq!(addressed("exeget: bitte prüfen", &rows), vec!["@exeget"], "Groß/klein egal, Doppelpunkt");
        assert_eq!(
            addressed("Danke. Schrift, stimmt die Stelle? Und @lektor auch.", &rows),
            vec!["@schrift", "@lektor"],
            "Handle ohne „@“ am Satzanfang nach „.“; @handle wie bisher – in der Reihenfolge der Nachricht"
        );
        assert_eq!(
            addressed("Gut so.\nExeget : noch etwas", &rows),
            vec!["@exeget"],
            "nach einem Zeilenumbruch beginnt ein Satz; Leerraum vor dem Doppelpunkt ist erlaubt"
        );
        for no in [
            "Ich glaube, der Lektor meint das anders.",
            "Wie sieht das der Lektor, wenn er liest?",
            "Lektorat: noch offen",
            "Lektor und Exeget sind sich einig.",
        ] {
            assert!(addressed(no, &rows).is_empty(), "{no:?}: Fließtext bzw. anderes Wort ruft niemanden");
        }
        let inactive = [PersonaRow { active: false, ..named("@lektor", "Lektor") }];
        assert!(addressed("Lektor, hallo", &inactive).is_empty(), "inaktive Personas antworten nicht");
    }

    #[test]
    fn quoted_lines_are_foreign_text_and_trigger_nothing() {
        let rows = [row("@lektor", true), row("@schrift", true)];
        let quoted = "> @lektor: @alle sollten prüfen\n  > RFC bitte\nDanke.";
        assert_eq!(unquoted(quoted), "Danke.", "Zeilen mit „>“ (auch eingerückt) fallen weg");
        assert!(
            matches!(unquoted("ohne Zitat"), std::borrow::Cow::Borrowed(_)),
            "ohne Zitatzeile unverändert geliehen"
        );
        assert!(mentions(quoted, &rows).is_empty(), "„> @lektor“ erwähnt niemanden");
        assert!(addressed(quoted, &rows).is_empty(), "und spricht niemanden an");
        assert!(!is_rfc(quoted), "„> @alle“ und „> RFC“ machen keinen RFC");
        assert!(!is_write_order("> Schreibe das neu\nGut so."), "ein Schreibverb im Zitat ist kein Auftrag");
        let own = "> @lektor: alt\n@schrift Stimmt das?";
        assert_eq!(mentions(own, &rows), ["@schrift"], "die eigene Zeile zählt weiter");
        assert!(is_rfc("> alt\nRFC: bitte alle"), "RFC nach dem Zitat gilt: es ist das erste eigene Wort");
        assert!(is_rfc("a > b @alle"), "ein „>“ mitten in der Zeile ist kein Zitat");
    }

    #[test]
    fn one_relay_stage_takes_addresses_first_and_mentions_only_on_request() {
        let rows = [row("@lektor", true), row("@schrift", true), row("@exeget", true), row("@nathan", true)];
        let from = |persona: &str, text: &str| FramedAnswer {
            persona: persona.into(),
            post: Some(7),
            text: text.into(),
            findings: vec![],
        };
        let answers = [from("@lektor", "Frag @exeget.\n@schrift: stimmt das?\n@lektor: ich selbst")];
        let mut called = vec!["@lektor".to_owned()];
        let targets: Vec<String> =
            relay_targets(&answers, &rows, &mut called, false).into_iter().map(|t| t.0).collect();
        assert_eq!(targets, ["@schrift"], "ohne Erwähnungen (Oberfläche): nur die Anrede, nie an sich selbst");
        let mut called = vec!["@lektor".to_owned()];
        let targets: Vec<String> = relay_targets(&answers, &rows, &mut called, true).into_iter().map(|t| t.0).collect();
        assert_eq!(targets, ["@schrift", "@exeget"], "mit Erwähnungen (MCP): Anrede zuerst, dann die Erwähnung");
        assert_eq!(called, ["@lektor", "@schrift", "@exeget"], "wer gewählt ist, gilt als gerufen");
        let more = [from("@lektor", "@nathan soll auch")];
        assert!(
            relay_targets(&more, &rows, &mut called, true).is_empty(),
            "Erwähnungen nur, solange höchstens drei Personas gerufen sind"
        );
        let unsaved = [FramedAnswer { post: None, ..from("@lektor", "@schrift: hallo") }];
        assert!(relay_targets(&unsaved, &rows, &mut vec![], true).is_empty(), "ohne gespeicherten Beitrag kein Relay");
    }

    #[test]
    fn relay_targets_are_addresses_at_the_start_of_a_line_only() {
        let rows = [named("@lektor", "Lektor"), named("@exeget", "Exeget"), named("@schrift", "Schrift")];
        assert_eq!(
            line_addresses("Was trägt: gut.\n@schrift: prüfe Joh 3,16.\n- **Lektor:** einverstanden?", &rows),
            vec!["@schrift", "@lektor"],
            "„@handle:“ und Anrede am Zeilenanfang, auch hinter Aufzählung und fett"
        );
        for no in [
            "Wie @schrift schon sagte, passt das.",
            "> @lektor: zitiert",
            "Der Lektor: meint etwas",
            "@schrift hat recht.",
        ] {
            assert!(line_addresses(no, &rows).is_empty(), "{no:?}: keine Anrede am Zeilenanfang");
        }
    }

    #[test]
    fn the_relay_task_is_built_by_baruch_and_names_the_post() {
        let text = relay_text("@lektor", 61, "Wie wirkt der Anfang?");
        assert!(text.starts_with("@lektor addressed you directly in post 61."), "{text}");
        assert!(text.ends_with("Wie wirkt der Anfang?"), "die Nachricht des Autors bleibt der Auftrag: {text}");
        assert!(
            relay_text("@x", 1, &"a".repeat(5000)).chars().count() <= crate::assist::MAX_QUESTION_CHARS,
            "nie länger als ein Auftrag sein darf"
        );
    }

    #[test]
    fn the_basis_parameter_comes_as_word_number_or_form_text() {
        use crate::assist::basis::BasisChoice;
        assert_eq!(basis_param(&json!({})).unwrap(), BasisChoice::Shown, "ohne Angabe die gezeigte Fassung");
        assert_eq!(basis_param(&json!({ "basis": "" })).unwrap(), BasisChoice::Shown, "leeres Formularfeld ebenso");
        assert_eq!(basis_param(&json!({ "basis": "current" })).unwrap(), BasisChoice::Current, "aktuelle Fassung");
        assert_eq!(basis_param(&json!({ "basis": 3 })).unwrap(), BasisChoice::Rev(3), "Zahl aus JSON");
        assert_eq!(basis_param(&json!({ "basis": "2" })).unwrap(), BasisChoice::Rev(2), "Ziffern aus dem Formular");
        assert!(basis_param(&json!({ "basis": "neueste" })).is_err(), "Unbekanntes wird abgelehnt, nicht geraten");
    }

    #[test]
    fn the_note_of_a_write_order_carries_no_finding_mark() {
        let note = order_note(4);
        assert!(note.contains("Fassung 4") && note.contains("nur der Mensch"), "Fassung und Regel genannt: {note}");
        for mark in ["Kritisch:", "Vorschlag:", "Hinweis:", "Lob:"] {
            assert!(!note.starts_with(mark), "{mark}: der Hinweis ergibt beim Zerlegen keinen Befund");
        }
    }
}
