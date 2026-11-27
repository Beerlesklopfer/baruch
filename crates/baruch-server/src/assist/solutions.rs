// SPDX-License-Identifier: AGPL-3.0-or-later
//! Solution proposals in persona answers (Round 30b, chat-loesungen; user 2026-11-09: „Was aus dem Chat nicht
//! hervorgeht, sind Lösungsvorschläge. Das sollte das Ziel sein.“).
//!
//! - **Form** below a finding line (`Kritisch:` or list item), optionally indented with „- “:
//!   `statt: „<wörtliches Zitat>“` and `besser: „<neue Formulierung>“` or `Schritt: <klarer nächster Schritt>`;
//!   case-insensitive; the short form `statt „…“ besser „…“` in one line counts too. Parsed by [`parse_solutions`]
//!   (one line is recognised by [`is_solution_line`]); the findings ([`crate::findings::parse`]) attach the solution
//!   lines to the finding above, `ingest` stores them per finding (`Store::finding_set_solutions`).
//! - **Adoptable** is a solution with quotation and new wording whose quotation stands in the revision ([`locate`]:
//!   literal, whitespace may differ). [`post_solutions`] returns the solutions of a post against the shown
//!   revision (hook in `view.rs`).
//! - **RFC** ([`rfc_proposal`], hook in `chat.rs`): the lector closes with „Mein Vorschlag:“ and two or three
//!   solutions; Baruch inserts the adoptable ones into the review basis and creates **one** collective proposal (note
//!   „Mein Vorschlag von @lektor (Beitrag n)“).
//! - **RPC** `solution.list`, `solution.adopt`, `solution.variants`, `solution.reject_siblings`, `solution.compare`;
//!   page `GET /varianten` (comparison side by side) and form `POST /loesungen/act` (without JavaScript, 303).
//!
//! Adopting a revision stays with the human: `solution.adopt` only creates a proposal (origin the persona
//! of the post) and proposes the findings as „übernommen in Fassung n“; the human confirms both.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::{Node, Work};
use baruch_store::{IdeaOrigin, Solution, StatusChange, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value as Tpl, context};
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// A solution as the UI shows it (position in the post, adoptable, finding).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SolutionView {
    pub index: usize,
    pub quote: String,
    pub replacement: String,
    pub step: String,
    /// The quotation stands literally in the shown revision.
    pub adoptable: bool,
    pub finding: Option<i64>,
}

/// Maximum lengths like `Store::finding_set_solutions` (quotation like `finding.quote`, wording and step like text).
const MAX_QUOTE: usize = baruch_store::MAX_QUOTE_CHARS;
const MAX_TEXT: usize = baruch_store::MAX_FINDING_CHARS;
/// At most this many solutions [`rfc_proposal`] inserts (the lector names two or three).
pub const RFC_MAX_SOLUTIONS: usize = 3;

/// Quotation marks: opening → possible closing ones („…“ also „…”, as models often write it).
const QUOTES: [(char, &[char]); 5] =
    [('„', &['“', '”']), ('"', &['"']), ('»', &['«']), ('“', &['”', '“']), ('‚', &['‘', '’'])];

/// Skip list and Markdown characters, a number („1.“, „2)“) and whitespace at the start of the line.
fn strip_marks(line: &str) -> &str {
    let marks = |c: char| c.is_whitespace() || matches!(c, '-' | '*' | '•' | '–' | '+' | '>' | '_');
    let probe = line.trim_start_matches(marks);
    let digits = probe.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits <= 3 && probe[digits..].starts_with(['.', ')']) {
        return probe[digits + 1..].trim_start_matches(marks);
    }
    probe
}

/// Does `s` start (ASCII, case-insensitive) with `word` as a whole word? Then the rest after it (without `*`/`_`).
fn keyword<'a>(s: &'a str, word: &str) -> Option<&'a str> {
    let head = s.get(..word.len())?;
    if !head.eq_ignore_ascii_case(word) {
        return None;
    }
    let rest = &s[word.len()..];
    if rest.chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    Some(rest.trim_start_matches(['*', '_']))
}

/// After a keyword: optional colon (also bold), then whitespace. Returns the rest and whether a
/// colon stood there.
fn after_colon(rest: &str) -> (&str, bool) {
    let trimmed = rest.trim_start();
    match trimmed.strip_prefix(':') {
        Some(after) => (after.trim_start_matches(['*', '_']).trim_start(), true),
        None => (trimmed, false),
    }
}

/// Quotation at the start of `s` (after whitespace): content (trimmed) and the rest behind the closing mark.
fn take_quote(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let open = s.chars().next()?;
    let (_, closers) = QUOTES.iter().find(|(o, _)| *o == open)?;
    let body = &s[open.len_utf8()..];
    let end = body.char_indices().find(|(_, c)| closers.contains(c)).map(|(i, _)| i)?;
    let close_len = body[end..].chars().next().map_or(0, char::len_utf8);
    let quote = body[..end].trim().to_owned();
    (!quote.is_empty()).then(|| (quote, &body[end + close_len..]))
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Text behind `besser:` without quotation marks: the quotation, otherwise the rest of the line (trimmed, without
/// sentence-ending marks outside).
fn better_text(rest: &str) -> Option<String> {
    if let Some((quote, _)) = take_quote(rest) {
        return Some(quote);
    }
    let plain = rest.trim().trim_matches(|c: char| matches!(c, '*' | '_')).trim();
    (!plain.is_empty()).then(|| plain.to_owned())
}

/// One line of the solution form.
enum Piece {
    /// `statt: „…“`, optionally with `besser „…“` in the same line.
    Instead(String, Option<String>),
    /// `besser: …`
    Better(String),
    /// `Schritt: …`
    Step(String),
}

/// Read a line by the solution form (at the start of the line after list and Markdown characters).
fn piece(line: &str) -> Option<Piece> {
    let probe = strip_marks(line);
    if let Some(rest) = keyword(probe, "statt") {
        let (rest, _) = after_colon(rest);
        let (quote, rest) = take_quote(rest)?;
        let better = short_better(rest);
        return Some(Piece::Instead(quote, better));
    }
    if let Some(rest) = keyword(probe, "besser") {
        let (rest, colon) = after_colon(rest);
        // „besser“ without colon only with a quotation („besser „…““), otherwise it is a sentence („Besser wäre …“).
        if !colon && take_quote(rest).is_none() {
            return None;
        }
        return better_text(rest).map(Piece::Better);
    }
    if let Some(rest) = keyword(probe, "schritt") {
        let (rest, colon) = after_colon(rest);
        // Only with colon: „Schritt für Schritt …“ is not a solution step.
        let step = rest.trim().trim_matches(|c: char| matches!(c, '*' | '_')).trim();
        return (colon && !step.is_empty()).then(|| Piece::Step(step.to_owned()));
    }
    None
}

/// Behind a quotation: `besser „…“` or `, besser: „…“` or `→ „…“`?
fn short_better(rest: &str) -> Option<String> {
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '–' | '-' | '*' | '_'));
    if let Some(after) = rest.strip_prefix('→') {
        return take_quote(after).map(|(q, _)| q);
    }
    let after = keyword(rest, "besser")?;
    let (after, _) = after_colon(after);
    take_quote(after).map(|(q, _)| q)
}

/// Line without the new wordings of the short form („besser „…““, „→ „…““): what stands after „besser“ is not a
/// quotation from the text and must not trigger the evidence check of a „Kritisch:“ finding.
pub fn strip_replacements(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let lower = rest.to_ascii_lowercase();
        let next = [lower.find("besser"), rest.find('→')].into_iter().flatten().min();
        let Some(at) = next else { break };
        let word = if rest[at..].starts_with('→') { '→'.len_utf8() } else { "besser".len() };
        let (after, _) = after_colon(&rest[at + word..]);
        match take_quote(after) {
            Some((_, tail)) => {
                out.push_str(&rest[..at]);
                rest = tail;
            }
            None => {
                out.push_str(&rest[..at + word]);
                rest = &rest[at + word..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Is the line a solution line (`statt:` with quotation, `besser:`, `Schritt:`)?
pub fn is_solution_line(line: &str) -> bool {
    piece(line).is_some()
}

/// Short form in the middle of a line: „… statt „x“ besser „y“ …“ (every occurrence).
fn inline_pairs(line: &str) -> Vec<Solution> {
    let mut out = Vec::new();
    let lower = line.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find("statt") {
        let at = from + found;
        from = at + "statt".len();
        let boundary = line[..at].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        if !boundary {
            continue;
        }
        let (rest, _) = after_colon(&line[at + "statt".len()..]);
        let Some((quote, rest)) = take_quote(rest) else { continue };
        if let Some(better) = short_better(rest) {
            out.push(solution(&quote, &better, ""));
        }
    }
    out
}

fn solution(quote: &str, replacement: &str, step: &str) -> Solution {
    Solution { quote: clip(quote, MAX_QUOTE), replacement: clip(replacement, MAX_TEXT), step: clip(step, MAX_TEXT) }
}

/// Solutions from the text of a persona answer (or of a finding), in text order. `statt` without `besser` counts
/// only together with a following `Schritt:` (then quotation and step); a `besser` without `statt` before is dropped.
pub fn parse_solutions(text: &str) -> Vec<Solution> {
    let mut out: Vec<Solution> = Vec::new();
    let mut pending: Option<String> = None;
    for line in text.lines() {
        match piece(line) {
            Some(Piece::Instead(quote, Some(better))) => {
                pending = None;
                out.push(solution(&quote, &better, ""));
            }
            Some(Piece::Instead(quote, None)) => pending = Some(quote),
            Some(Piece::Better(better)) => {
                if let Some(quote) = pending.take() {
                    out.push(solution(&quote, &better, ""));
                }
            }
            Some(Piece::Step(step)) => {
                let quote = pending.take().unwrap_or_default();
                out.push(solution(&quote, "", &step));
            }
            None => {
                pending = None;
                out.extend(inline_pairs(line));
            }
        }
    }
    out
}

/// Position of a quotation in `text`: literal, otherwise with arbitrary whitespace between the words (line breaks in
/// the Typst source); the first occurrence. Empty quotation → `None`.
pub fn locate(text: &str, quote: &str) -> Option<std::ops::Range<usize>> {
    let quote = quote.trim();
    if quote.is_empty() {
        return None;
    }
    if let Some(at) = text.find(quote) {
        return Some(at..at + quote.len());
    }
    let pattern = quote.split_whitespace().map(regex::escape).collect::<Vec<_>>().join(r"\s+");
    let re = regex::RegexBuilder::new(&pattern).size_limit(1 << 20).build().ok()?;
    re.find(text).map(|m| m.range())
}

/// Can the solution be inserted into `text` (quotation and wording, quotation stands in the text)?
pub fn adoptable(solution: &Solution, text: &str) -> bool {
    !solution.replacement.trim().is_empty() && locate(text, &solution.quote).is_some()
}

/// Insert solutions in order into `text` (each the first occurrence of the quotation in the already changed text).
/// If a quotation is missing (also because an earlier solution replaced it) → error with the position in `labels`.
pub fn apply_solutions(text: &str, solutions: &[(&str, &Solution)]) -> Result<String, String> {
    let mut out = text.to_owned();
    for (label, s) in solutions {
        if s.replacement.trim().is_empty() {
            return Err(format!(
                "{label} hat keine neue Formulierung (nur einen Schritt); sie lässt sich nicht einsetzen."
            ));
        }
        let Some(range) = locate(&out, &s.quote) else {
            return Err(format!("{label}: Das Zitat „{}“ steht nicht (mehr) im Text.", clip(&s.quote, 80)));
        };
        out.replace_range(range, s.replacement.trim());
    }
    Ok(out)
}

/// Solutions of a post against the shown revision `shown` (position from 0 in the order of
/// [`parse_solutions`]).
pub fn post_solutions(body: &str, shown: &str) -> Vec<SolutionView> {
    // Integrator Round 30: the same solution (e.g. below the finding and again below „Mein Vorschlag:“) appears only
    // once; `index` stays the position in `parse_solutions` so that `picks` of `solution.adopt` mean the same solution.
    let mut seen = std::collections::HashSet::new();
    parse_solutions(body)
        .into_iter()
        .enumerate()
        .filter(|(_, s)| seen.insert((s.quote.clone(), s.replacement.clone(), s.step.clone())))
        .map(|(index, s)| SolutionView {
            index,
            adoptable: adoptable(&s, shown),
            quote: s.quote,
            replacement: s.replacement,
            step: s.step,
            finding: None,
        })
        .collect()
}

/// Solutions in the closing block „Mein Vorschlag:“ (lines after the heading up to the end or up to a marker).
pub fn proposal_block(text: &str) -> Vec<Solution> {
    let mut block: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if crate::findings::parse::proposal_heading(line) {
            inside = true;
            // „Mein Vorschlag: statt „x“ besser „y““ – the short form may stand in the same line.
            block.push(line);
            continue;
        }
        if inside {
            block.push(line);
        }
    }
    parse_solutions(&block.join("\n"))
}

// ---- Revisions ---------------------------------------------------------------------------------

/// Section `key` with position (seq).
fn node_of(work: &Work, key: i64) -> Option<&Node> {
    work.nodes.iter().find(|n| n.key == key)
}

/// Text of the revision `rev` (without a value the current one) with its number.
fn revision_text(node: &Node, rev: Option<u32>) -> Result<(u32, String), RpcError> {
    let number = rev.or_else(|| node.current_number()).ok_or_else(|| refused("Der Abschnitt hat keine Fassung."))?;
    let revision = node
        .revision(number)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("Fassung {number} gibt es in diesem Abschnitt nicht.")))?;
    Ok((number, revision.body.clone()))
}

fn refused(message: impl Into<String>) -> RpcError {
    RpcError::new(REFUSED, message.into())
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message.into())
}

/// Status proposals „übernommen in Fassung `rev`“ for findings of this section (origin the persona); findings from
/// other sections get none (the number would not be valid there). Returns the proposed ids and notes.
async fn propose_adopted(
    state: &AppState,
    work: WorkId,
    key: i64,
    ids: &[i64],
    rev: u32,
    persona: &str,
    note: &str,
) -> (Vec<i64>, Vec<String>) {
    let by = IdeaOrigin::Persona { handle: persona.to_owned() };
    let mut done = Vec::new();
    let mut notes = Vec::new();
    for &id in ids {
        match state.store.finding_get(work, id).await {
            Ok(finding) if finding.key == Some(key) => {}
            Ok(_) => {
                notes.push(format!("[B{id}]: Der Befund gehört nicht zu diesem Abschnitt; kein Statusvorschlag."));
                continue;
            }
            Err(e) => {
                notes.push(format!("[B{id}]: {}", RpcError::from(e).message));
                continue;
            }
        }
        let change = StatusChange::Adopted { rev };
        match crate::findings::rpc::propose(state, work, id, &change, &by, note).await {
            Ok(_) => done.push(id),
            Err(e) => notes.push(format!("[B{id}]: {}", e.message)),
        }
    }
    (done, notes)
}

/// „Mein Vorschlag:“ of the lector in an RFC as **one** collective proposal: the adoptable solutions of the closing
/// block (at most [`RFC_MAX_SOLUTIONS`]) inserted into the review basis (`base.basis` or `base.rev`), origin the
/// lector, note „Mein Vorschlag von @lektor (Beitrag n)“. Findings the block names with `[B<id>]` are proposed as
/// „übernommen in Fassung n“. `None` = no block, no adoptable solution, or the text would stay the same.
pub async fn rfc_proposal(
    state: &AppState,
    base: &super::Order,
    lector: &super::Outcome,
    progress: Option<&UnboundedSender<super::Progress>>,
) -> Result<Option<super::chat::ChatProposal>, RpcError> {
    let _ = progress;
    let solutions = proposal_block(&lector.text);
    if solutions.is_empty() {
        return Ok(None);
    }
    let work = state.store.load_work(base.work).await?;
    let node = node_of(&work, base.key).ok_or_else(|| refused("Der Abschnitt ist nicht mehr da."))?;
    let wanted = base.basis.as_ref().map(|b| b.rev).or(base.rev);
    let (basis_rev, text) = revision_text(node, wanted)?;
    let mut changed = text.clone();
    let mut used = 0;
    for s in solutions.iter().filter(|s| adoptable(s, &text)) {
        if used == RFC_MAX_SOLUTIONS {
            break;
        }
        // A solution whose quotation an earlier one already replaced is dropped (no error for the collective proposal).
        if let Ok(next) = apply_solutions(&changed, &[("Lösung", s)]) {
            changed = next;
            used += 1;
        }
    }
    if used == 0 || changed == text {
        return Ok(None);
    }
    let post = lector.post.as_ref().map(|p| p.id);
    let note = match post {
        Some(id) => format!("Mein Vorschlag von {} (Beitrag {id})", lector.persona),
        None => format!("Mein Vorschlag von {}", lector.persona),
    };
    let rev = {
        let _held = state.write_lock.lock().await;
        let seq = state.store.node_seq(base.work, base.key).await?;
        let rev = state
            .store
            .add_proposal_on(base.work, seq, &changed, &lector.persona, &lector.model, &note, basis_rev, None)
            .await?;
        crate::mirror::side_refs_after_proposal(state, &_held, base.work).await;
        rev
    };
    let block: String =
        lector.text.lines().skip_while(|l| !crate::findings::parse::proposal_heading(l)).collect::<Vec<_>>().join("\n");
    let refs = super::chat::finding_refs(&block);
    if !refs.is_empty() {
        let _ = propose_adopted(state, base.work, base.key, &refs, rev, &lector.persona, &note).await;
    }
    Ok(Some(super::chat::ChatProposal {
        persona: lector.persona.clone(),
        rev,
        url: crate::view::node_url(base.work, base.key, Some(rev), false, "", ""),
    }))
}

// ---- RPC ---------------------------------------------------------------------------------------

/// RPC methods of this module.
pub const METHODS: &[&str] =
    &["solution.list", "solution.adopt", "solution.variants", "solution.reject_siblings", "solution.compare"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn number(params: &Value, name: &str) -> Result<Option<i64>, RpcError> {
    super::rpc::number(params, name)
}

fn required(params: &Value, name: &str, what: &str) -> Result<i64, RpcError> {
    number(params, name)?.ok_or_else(|| invalid(format!("{name} fehlt ({what})")))
}

fn rev_param(params: &Value, name: &str) -> Result<Option<u32>, RpcError> {
    match number(params, name)? {
        None => Ok(None),
        Some(n) => u32::try_from(n)
            .ok()
            .filter(|n| *n >= 1)
            .map(Some)
            .ok_or_else(|| invalid(format!("{name}: Fassungsnummer ab 1 erwartet"))),
    }
}

/// List of numbers (`[1, 2]`, one number, digits as text or „1,2“ from forms).
fn numbers(params: &Value, name: &str) -> Result<Vec<i64>, RpcError> {
    let wrong = || invalid(format!("{name}: Liste von Zahlen erwartet"));
    let one = |v: &Value| -> Result<Vec<i64>, RpcError> {
        match v {
            Value::Number(n) => n.as_i64().map(|n| vec![n]).ok_or_else(wrong),
            Value::String(s) => s
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| p.parse::<i64>().map_err(|_| wrong()))
                .collect(),
            _ => Err(wrong()),
        }
    };
    match params.get(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                out.extend(one(item)?);
            }
            Ok(out)
        }
        Some(other) => one(other),
    }
}

fn confirmed(params: &Value) -> bool {
    match params.get("confirm") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.trim(), "1" | "true" | "on"),
        Some(Value::Number(n)) => n.as_i64() == Some(1),
        _ => false,
    }
}

/// Answer RPC `solution.*`.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    match method {
        "solution.list" => list(state, work, &params).await,
        "solution.adopt" => adopt(state, work, &params).await,
        "solution.variants" => variants(state, work, &params, crate::mirror::HUMAN_NAME).await,
        "solution.reject_siblings" => reject_siblings(state, work, &params).await,
        "solution.compare" => compare(state, work, &params).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

/// A post of the section (`post.id`), otherwise -32602.
fn post_in(node: &Node, id: i64) -> Result<&baruch_core::Post, RpcError> {
    node.posts
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| invalid(format!("Beitrag {id} gehört nicht zu diesem Abschnitt.")))
}

/// Shown revision for solutions of a post: `rev`, else the review basis of the post, else the current one.
async fn shown_for(state: &AppState, work: WorkId, post: Option<i64>, rev: Option<u32>) -> Option<u32> {
    if rev.is_some() {
        return rev;
    }
    let post = post?;
    state.store.post_meta(work, &[post]).await.ok()?.get(&post).and_then(|m| m.basis_rev)
}

/// `solution.list {key, post?, rev?}` → `{solutions: [SolutionView]}`: with `post` the solutions of the post (position
/// like `picks` of `solution.adopt`), else the solutions of all findings of the section (position per finding).
async fn list(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let key = required(params, "key", "fester Schlüssel des Abschnitts")?;
    let loaded = state.store.load_work(work).await?;
    let node =
        node_of(&loaded, key).ok_or_else(|| invalid(format!("Abschnitt {key} gibt es in diesem Werk nicht.")))?;
    let post = number(params, "post")?;
    let rev = shown_for(state, work, post, rev_param(params, "rev")?).await;
    let (shown_rev, shown) = revision_text(node, rev)?;
    let solutions: Vec<SolutionView> = match post {
        Some(id) => post_solutions(&post_in(node, id)?.body, &shown),
        None => {
            let filter = baruch_store::FindingFilter { keys: vec![key], limit: Some(2000), ..Default::default() };
            let mut out = Vec::new();
            for f in state.store.finding_list(work, &filter).await? {
                for (index, s) in f.solutions.iter().enumerate() {
                    out.push(SolutionView {
                        index,
                        quote: s.quote.clone(),
                        replacement: s.replacement.clone(),
                        step: s.step.clone(),
                        adoptable: adoptable(s, &shown),
                        finding: Some(f.id),
                    });
                }
            }
            out
        }
    };
    Ok(json!({ "solutions": solutions, "rev": shown_rev }))
}

/// `solution.adopt {key, post | finding, rev?, picks}` → `{rev, url, proposed_findings, notes}` (see module doc):
/// `picks` are positions in the solutions of the post ([`parse_solutions`]) or – with `finding` – of the finding. Base
/// is `rev`, else the review basis of the post (`PostMeta::basis_rev`) or for a finding the current revision.
async fn adopt(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let key = required(params, "key", "fester Schlüssel des Abschnitts")?;
    let picks = numbers(params, "picks")?;
    if picks.is_empty() {
        return Err(invalid("picks fehlt: mindestens eine Lösung wählen"));
    }
    let loaded = state.store.load_work(work).await?;
    let node =
        node_of(&loaded, key).ok_or_else(|| invalid(format!("Abschnitt {key} gibt es in diesem Werk nicht.")))?;
    let finding = match number(params, "finding")? {
        Some(id) => Some(state.store.finding_get(work, id).await?),
        None => None,
    };
    if let Some(f) = &finding
        && f.key != Some(key)
    {
        return Err(invalid(format!("Befund {} gehört nicht zu diesem Abschnitt.", f.id)));
    }
    let post_id = match (&finding, number(params, "post")?) {
        (_, Some(id)) => id,
        (Some(f), None) => f.post.ok_or_else(|| invalid("Der Befund stammt aus keinem Beitrag dieses Abschnitts."))?,
        (None, None) => return Err(invalid("post fehlt (Beitrag mit den Lösungen) – oder finding (Befund)")),
    };
    let post = post_in(node, post_id)?;
    let solutions: Vec<Solution> = match &finding {
        Some(f) => f.solutions.clone(),
        None => parse_solutions(&post.body),
    };
    // Base: `rev`; else for a post its review basis, for a finding (findings list) the current revision.
    let rev = match &finding {
        Some(_) => rev_param(params, "rev")?,
        None => shown_for(state, work, Some(post_id), rev_param(params, "rev")?).await,
    };
    let (basis_rev, text) = revision_text(node, rev)?;
    let mut chosen: Vec<(String, &Solution)> = Vec::new();
    // Duplicate numbers count once: inserting the same solution twice would replace its own replacement again.
    let mut seen = std::collections::HashSet::new();
    for &pick in picks.iter().filter(|p| seen.insert(**p)) {
        let s = usize::try_from(pick)
            .ok()
            .and_then(|i| solutions.get(i))
            .ok_or_else(|| invalid(format!("Lösung {pick} gibt es nicht ({} Lösungen).", solutions.len())))?;
        if !adoptable(s, &text) {
            return Err(invalid(format!(
                "Lösung {pick} ist nicht übernehmbar: {}",
                if s.replacement.trim().is_empty() {
                    "Sie nennt nur einen Schritt, keine neue Formulierung.".to_owned()
                } else {
                    format!("Das Zitat „{}“ steht nicht in Fassung {basis_rev}.", clip(&s.quote, 80))
                }
            )));
        }
        chosen.push((format!("Lösung {pick}"), s));
    }
    let labelled: Vec<(&str, &Solution)> = chosen.iter().map(|(l, s)| (l.as_str(), *s)).collect();
    let changed = apply_solutions(&text, &labelled).map_err(invalid)?;
    if changed == text {
        return Err(refused("Die Lösungen ändern den Text nicht; kein Vorschlag angelegt."));
    }
    let persona = post.author.clone();
    let note = match &finding {
        Some(f) => format!("Lösung aus Beitrag {post_id} (Befund [B{}])", f.id),
        None => format!("Lösung aus Beitrag {post_id}"),
    };
    // Model as origin: the one configured for the persona (the post itself names none); otherwise empty.
    let model = state
        .store
        .persona_list()
        .await?
        .into_iter()
        .find(|p| p.handle == persona)
        .map(|p| p.model)
        .unwrap_or_default();
    let new_rev = {
        let _held = state.write_lock.lock().await;
        let seq = state.store.node_seq(work, key).await?;
        let rev = state.store.add_proposal_on(work, seq, &changed, &persona, &model, &note, basis_rev, None).await?;
        crate::mirror::side_refs_after_proposal(state, &_held, work).await;
        rev
    };
    // Findings that carry one of the chosen solutions: the named one or the findings of the post with this solution.
    let ids: Vec<i64> = match &finding {
        Some(f) => vec![f.id],
        None => {
            let filter = baruch_store::FindingFilter { keys: vec![key], limit: Some(2000), ..Default::default() };
            state
                .store
                .finding_list(work, &filter)
                .await?
                .into_iter()
                .filter(|f| f.post == Some(post_id))
                .filter(|f| f.solutions.iter().any(|s| chosen.iter().any(|(_, c)| *c == s)))
                .map(|f| f.id)
                .collect()
        }
    };
    let (proposed, notes) = propose_adopted(state, work, key, &ids, new_rev, &persona, &note).await;
    Ok(json!({
        "rev": new_rev, "base": basis_rev, "persona": persona, "note": note,
        "url": crate::view::node_url(work, key, Some(new_rev), false, "", ""),
        "proposed_findings": proposed, "notes": notes,
    }))
}

/// Number of variants from `count` (default [`super::variants::DEFAULT_VARIANTS`], only 2 or 3).
pub fn variant_count(params: &Value) -> Result<Option<u8>, RpcError> {
    match number(params, "count")? {
        None => Ok(None),
        Some(n @ 2..=3) => Ok(Some(n as u8)),
        Some(n) => {
            Err(invalid(format!("count: 2 oder 3 Varianten (höchstens {}), nicht {n}", super::variants::MAX_VARIANTS)))
        }
    }
}

/// `solution.variants {key, rev?, findings?, count?}` → `VariantOutcome` (lector by setting, proposals only).
pub async fn variants(state: &AppState, work: WorkId, params: &Value, asker: &str) -> Result<Value, RpcError> {
    let key = required(params, "key", "fester Schlüssel des Abschnitts")?;
    let post = number(params, "post")?;
    let mut findings = numbers(params, "findings")?;
    // From a post („Lösungen umsetzen“ below its solutions): without a value its findings (without praise).
    if findings.is_empty()
        && let Some(post) = post
    {
        let filter = baruch_store::FindingFilter { keys: vec![key], limit: Some(2000), ..Default::default() };
        findings = state
            .store
            .finding_list(work, &filter)
            .await?
            .into_iter()
            .filter(|f| f.post == Some(post) && f.kind != baruch_store::FindingKind::Praise)
            .map(|f| f.id)
            .collect();
    }
    let order = super::variants::VariantOrder {
        work,
        key,
        rev: rev_param(params, "rev")?,
        findings,
        count: variant_count(params)?,
        asker: asker.to_owned(),
        from_post: post,
    };
    let outcome = super::variants::run(state, &order, None).await?;
    serde_json::to_value(outcome).map_err(|e| RpcError::new(INTERNAL, e.to_string()))
}

/// `solution.reject_siblings {key, rev, confirm: true}` → `{rejected: [rev], group}`: rejects the other open
/// proposals of the variant group of `rev` (the human only; `rev` itself stays).
async fn reject_siblings(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let key = required(params, "key", "fester Schlüssel des Abschnitts")?;
    let rev = rev_param(params, "rev")?.ok_or_else(|| invalid("rev fehlt (die Variante, die bleibt)"))?;
    if !confirmed(params) {
        return Err(invalid("confirm: true fehlt – die übrigen Varianten werden verworfen"));
    }
    let _held = state.write_lock.lock().await;
    let seq = state.store.node_seq(work, key).await?;
    let tree = state.store.revision_tree(work).await?;
    let infos = tree.iter().find(|n| n.node == seq).map(|n| n.revisions.as_slice()).unwrap_or_default();
    let info =
        infos.iter().find(|i| i.number == rev).ok_or_else(|| invalid(format!("Fassung {rev} gibt es nicht.")))?;
    let group = info
        .variant
        .as_ref()
        .map(|v| v.group.clone())
        .ok_or_else(|| invalid(format!("Fassung {rev} ist keine Variante.")))?;
    let mut rejected = Vec::new();
    for sibling in infos {
        let same = sibling.variant.as_ref().is_some_and(|v| v.group == group);
        if same && sibling.number != rev && sibling.status == baruch_core::RevisionStatus::Proposed {
            state.store.reject(work, seq, sibling.number).await?;
            rejected.push(sibling.number);
        }
    }
    Ok(json!({ "rejected": rejected, "group": group, "kept": rev }))
}

/// Line diff as safe HTML (everything escaped): equal lines plain, new ones in `<ins>`, removed ones in `<del>`.
pub fn diff_html(old: &str, new: &str) -> String {
    let mut out = String::from("<div class=\"sol-diff\">");
    for (kind, line) in baruch_core::line_diff(old.trim_end(), new.trim_end()) {
        let text = crate::bibles::html_escape(&line);
        let text = if text.is_empty() { "&nbsp;".to_owned() } else { text };
        match kind {
            baruch_core::DiffKind::Same => out.push_str(&format!("<div class=\"d-same\">{text}</div>")),
            baruch_core::DiffKind::Added => out.push_str(&format!("<ins class=\"d-add\">{text}</ins>")),
            baruch_core::DiffKind::Removed => out.push_str(&format!("<del class=\"d-del\">{text}</del>")),
        }
    }
    out.push_str("</div>");
    out
}

/// Comparison of a group: base, per variant the difference to the base and among each other.
async fn comparison(state: &AppState, work: WorkId, key: i64, group: &str) -> Result<Value, RpcError> {
    let seq = state.store.node_seq(work, key).await?;
    let members = state.store.variant_group(work, seq, group).await?;
    if members.is_empty() {
        return Err(invalid(format!("Variantengruppe {group} gibt es in diesem Abschnitt nicht.")));
    }
    let loaded = state.store.load_work(work).await?;
    let node = node_of(&loaded, key).ok_or_else(|| invalid("Der Abschnitt ist nicht mehr da."))?;
    let tree = state.store.revision_tree(work).await?;
    let infos = tree.iter().find(|n| n.node == seq).map(|n| n.revisions.as_slice()).unwrap_or_default();
    let base = infos.iter().find(|i| i.number == members[0].0).and_then(|i| i.based_on);
    let base_text = base.and_then(|b| node.revision(b)).map(|r| r.body.clone()).unwrap_or_default();
    let texts: Vec<(u32, String)> = members
        .iter()
        .map(|(rev, _)| (*rev, node.revision(*rev).map(|r| r.body.clone()).unwrap_or_default()))
        .collect();
    let variants: Vec<Value> = members
        .iter()
        .zip(&texts)
        .map(|((rev, info), (_, text))| {
            let status = node.revision(*rev).map(|r| r.status);
            let others: Vec<Value> = members
                .iter()
                .zip(&texts)
                .filter(|((other, _), _)| other != rev)
                .map(|((other, other_info), (_, other_text))| {
                    json!({ "rev": other, "label": other_info.label, "diff_html": diff_html(other_text, text) })
                })
                .collect();
            json!({
                "rev": rev, "label": info.label, "summary": info.summary, "findings": info.findings,
                "status": status.map(|s| match s {
                    baruch_core::RevisionStatus::Current => "current",
                    baruch_core::RevisionStatus::Proposed => "proposed",
                    baruch_core::RevisionStatus::Rejected => "rejected",
                    baruch_core::RevisionStatus::Superseded => "superseded",
                }),
                "diff_html": diff_html(&base_text, text),
                "against": others,
                "url": crate::view::node_url(work, key, Some(*rev), false, "", ""),
            })
        })
        .collect();
    Ok(json!({ "base": base, "group": group, "key": key, "work": work, "variants": variants,
               "title": crate::findings::view::node_label(&loaded, key).unwrap_or_default() }))
}

/// `solution.compare {key, group}` → `{base, variants: [{rev, label, summary, findings, diff_html, against}]}`.
async fn compare(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let key = required(params, "key", "fester Schlüssel des Abschnitts")?;
    let group = super::rpc::text(params, "group")?.ok_or_else(|| invalid("group fehlt (Variantengruppe)"))?;
    comparison(state, work, key, group.trim()).await
}

// ---- Pages -------------------------------------------------------------------------------------

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("variants.html", include_str!("../../templates/variants.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Varianten fehlerhaft");
        }
        env
    })
}

fn plain(status: StatusCode, text: impl Into<String>) -> Response {
    (status, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text.into()).into_response()
}

/// Page `variants.html` with comparison or error.
fn page(
    headers: &HeaderMap,
    work: WorkId,
    title: &str,
    comparison: Option<Value>,
    error: Option<String>,
    back: String,
) -> Response {
    let settings = crate::view::Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let ctx = context! {
        settings => Tpl::from(Serde(settings)),
        work => context! { id => work, title => title },
        title => title,
        cmp => Tpl::from(Serde(comparison)),
        error => error,
        back => back,
    };
    match templates().get_template("variants.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (StatusCode::OK, Html(html)).into_response(),
        Err(e) => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage variants.html: {e}")),
    }
}

fn pair<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()).filter(|v| !v.trim().is_empty())
}

fn pair_number(pairs: &[(String, String)], name: &str) -> Option<i64> {
    pair(pairs, name).and_then(|v| v.trim().parse().ok())
}

/// `GET /varianten?work=W&key=K&group=G`: variants side by side (difference to the base and among each other), each
/// with „übrige verwerfen“ (form to `/loesungen/act`) and link into the revision tree; complete without JavaScript.
async fn variants_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let work = match pair_number(&pairs, "work") {
        Some(w) => w,
        None => match state.default_work().await {
            Ok(w) => w,
            Err(e) => return plain(StatusCode::BAD_REQUEST, RpcError::from(e).message),
        },
    };
    let title = match state.store.load_work(work).await {
        Ok(w) => w.title,
        Err(e) => return plain(StatusCode::NOT_FOUND, RpcError::from(e).message),
    };
    let Some(group) = pair(&pairs, "group") else {
        return plain(StatusCode::BAD_REQUEST, "group fehlt (Variantengruppe)");
    };
    // Without `key` (link from the revision tree of the chapter) the group determines the section.
    let key = match pair_number(&pairs, "key") {
        Some(key) => key,
        None => match state.store.variant_group_key(work, group.trim()).await {
            Ok(Some(key)) => key,
            Ok(None) => {
                return plain(StatusCode::NOT_FOUND, format!("Variantengruppe {group} gibt es in diesem Werk nicht."));
            }
            Err(e) => return plain(StatusCode::INTERNAL_SERVER_ERROR, RpcError::from(e).message),
        },
    };
    let back = crate::view::node_url(work, key, None, false, "", "");
    match comparison(&state, work, key, group.trim()).await {
        Ok(cmp) => page(&headers, work, &title, Some(cmp), None, back),
        Err(e) if e.code == INTERNAL => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)),
        Err(e) => page(&headers, work, &title, None, Some(e.message), back),
    }
}

/// `POST /loesungen/act`: fields `method` (solution.adopt | solution.variants | solution.reject_siblings), `work`,
/// `key`, `rev`, `post`, `finding`, `picks` (multiple), `findings` (multiple), `count`, `confirm`, `back`; then 303
/// to `back` (own address) or the new revision; error → 200 with notice (`role="alert"`) and way back.
async fn act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let method = pair(&pairs, "method").unwrap_or("").trim().to_owned();
    if !matches!(method.as_str(), "solution.adopt" | "solution.variants" | "solution.reject_siblings") {
        return plain(StatusCode::BAD_REQUEST, "unbekannte Aktion");
    }
    let Some(work) = pair_number(&pairs, "work") else {
        return plain(StatusCode::BAD_REQUEST, "work fehlt (Werk-id)");
    };
    let title = match state.store.load_work(work).await {
        Ok(w) => w.title,
        Err(e) => return plain(StatusCode::NOT_FOUND, RpcError::from(e).message),
    };
    let mut params = json!({});
    for name in ["key", "rev", "post", "finding", "count", "confirm", "group"] {
        if let Some(v) = pair(&pairs, name) {
            params[name] = json!(v.trim());
        }
    }
    for name in ["picks", "findings"] {
        let all: Vec<Value> =
            pairs.iter().filter(|(k, v)| k == name && !v.trim().is_empty()).map(|(_, v)| json!(v.trim())).collect();
        if !all.is_empty() {
            params[name] = Value::Array(all);
        }
    }
    let key = pair_number(&pairs, "key");
    let back = crate::findings::page::safe_back(pair(&pairs, "back"))
        .or_else(|| key.map(|k| crate::view::node_url(work, k, None, false, "", "")))
        .unwrap_or_else(|| format!("/?work={work}"));
    match call(&state, work, &method, params).await {
        Ok(answer) => {
            let target = match method.as_str() {
                "solution.adopt" => answer["url"].as_str().map(str::to_owned),
                "solution.variants" => match (key, answer["group"].as_str()) {
                    (Some(k), Some(g)) => Some(format!("/varianten?work={work}&key={k}&group={g}")),
                    _ => None,
                },
                _ => None,
            };
            let url = if method == "solution.reject_siblings" { back } else { target.unwrap_or(back) };
            Redirect::to(&url).into_response()
        }
        Err(e) if e.code == INTERNAL => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)),
        Err(e) => page(&headers, work, &title, None, Some(e.message), back),
    }
}

/// Pages (`GET /varianten`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/varianten", get(variants_page))
}

/// Writing routes (`POST /loesungen/act`, behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/loesungen/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(quote: &str, replacement: &str, step: &str) -> Solution {
        Solution { quote: quote.into(), replacement: replacement.into(), step: step.into() }
    }

    #[test]
    fn the_fixed_form_is_read_with_and_without_list_signs_and_in_any_case() {
        let text = "Kritisch: „immer“ verallgemeinert.\n  - statt: „immer leuchtet der Turm“\n  - besser: „meist leuchtet der Turm“\n\
                    Kritisch: Der Schluss hängt.\nSCHRITT: letzten Absatz streichen\n\
                    - Kritisch: Bild schief.\n  * Statt „Wachturm“ besser „Leuchtturm“\n\
                    Kritisch: Satz zu lang – statt: „und dann und dann“, besser: „dann“.";
        assert_eq!(
            parse_solutions(text),
            vec![
                s("immer leuchtet der Turm", "meist leuchtet der Turm", ""),
                s("", "", "letzten Absatz streichen"),
                s("Wachturm", "Leuchtturm", ""),
                s("und dann und dann", "dann", ""),
            ],
            "statt/besser untereinander, Schritt (groß geschrieben), Kurzform als Punkt und mitten im Satz"
        );
    }

    #[test]
    fn sentences_that_only_resemble_the_form_are_no_solutions() {
        for prose in [
            "Schritt für Schritt baut er die Mole.",
            "Besser wäre ein anderer Anfang.",
            "Stattdessen bleibt er am Kai.",
            "statt des Turms ein Haus",
            "besser „ohne Statt“",
        ] {
            assert!(parse_solutions(prose).is_empty(), "{prose:?}: kein Zitat bzw. kein Doppelpunkt – keine Lösung");
        }
        assert!(!is_solution_line("Kritisch: „immer“"), "eine Befundzeile ist keine Lösungszeile");
        assert!(is_solution_line("   - **Schritt:** streichen"), "fett gesetzt und eingerückt");
    }

    #[test]
    fn a_quote_is_found_verbatim_or_across_line_breaks_and_adopted_in_order() {
        let text = "== Leuchtturm\n\nDer Turm leuchtet\nüber der Mole. Der Turm leuchtet.\n";
        let found = locate(text, "Turm leuchtet über der Mole").expect("über den Zeilenumbruch hinweg");
        assert_eq!(&text[found], "Turm leuchtet\nüber der Mole", "Zeilenumbruch statt Leerzeichen");
        assert!(locate(text, "Wachturm").is_none(), "nicht im Text");
        assert!(adoptable(&s("Turm leuchtet", "Turm glimmt", ""), text), "Zitat und Formulierung");
        assert!(!adoptable(&s("Turm leuchtet", "", "kürzen"), text), "nur ein Schritt ist nicht einsetzbar");
        let a = s("Der Turm leuchtet\nüber", "Das Feuer brennt über", "");
        let b = s("Der Turm leuchtet.", "Das Licht bleibt.", "");
        let out = apply_solutions(text, &[("Lösung 0", &a), ("Lösung 1", &b)]).unwrap();
        assert_eq!(out, "== Leuchtturm\n\nDas Feuer brennt über der Mole. Das Licht bleibt.\n", "beide nacheinander");
        let err = apply_solutions(text, &[("Lösung 0", &b), ("Lösung 1", &b)]).unwrap_err();
        assert!(err.starts_with("Lösung 1:"), "ein schon ersetztes Zitat steht nicht mehr da: {err}");
    }

    #[test]
    fn post_solutions_mark_only_quotes_of_the_shown_version_as_adoptable() {
        let body = "Kritisch: „Kai“ unklar.\n- statt: „am Kai“\n- besser: „an der Mole“\nKritisch: x\n- statt: „Möwe“\n- besser: „Seeschwalbe“";
        let views = post_solutions(body, "== Hafen\n\nDas Boot liegt am Kai.\n");
        assert_eq!(views.len(), 2, "{views:?}");
        assert!(views[0].adoptable && !views[1].adoptable, "nur das Zitat, das in der Fassung steht: {views:?}");
        assert_eq!((views[0].index, views[1].index), (0, 1), "Stellen wie picks");
    }

    #[test]
    fn a_solution_repeated_under_mein_vorschlag_is_listed_once_with_its_first_index() {
        let body = "Kritisch: „Kai“ unklar.\n- statt: „am Kai“\n- besser: „an der Mole“\n\nMein Vorschlag:\n\
                    1. statt „am Kai“ besser „an der Mole“\n2. statt „Boot“ besser „Kutter“";
        let views = post_solutions(body, "== Hafen\n\nDas Boot liegt am Kai.\n");
        let shown: Vec<(usize, &str)> = views.iter().map(|v| (v.index, v.replacement.as_str())).collect();
        assert_eq!(
            shown,
            [(0, "an der Mole"), (2, "Kutter")],
            "die Wiederholung im Schlussblock fällt weg; die Stellen bleiben die von parse_solutions (picks)"
        );
    }

    #[test]
    fn the_closing_block_mein_vorschlag_is_read_for_the_collective_proposal() {
        let text = "Was trägt: „Licht“ trägt.\nJetzt wichtig: [B3]\n- statt: „x“\n- besser: „y“\n\n**Mein Vorschlag:**\n\
                    1. statt „am Kai“ besser „an der Mole“ [B3]\n2. statt: „Möwe“\n   besser: „Seeschwalbe“";
        assert_eq!(
            proposal_block(text),
            vec![s("am Kai", "an der Mole", ""), s("Möwe", "Seeschwalbe", "")],
            "nur der Schlussblock zählt für den Sammel-Vorschlag"
        );
        assert!(
            proposal_block("Kritisch: x\n- statt: „a“\n- besser: „b“").is_empty(),
            "ohne Block kein Sammel-Vorschlag"
        );
    }

    #[test]
    fn a_short_form_replacement_on_a_critical_line_is_no_quote_to_check() {
        let line = "Kritisch: „am Kai“ ist blass – statt „am Kai“ besser „an der Klippe“.";
        assert_eq!(
            strip_replacements(line),
            "Kritisch: „am Kai“ ist blass – statt „am Kai“ .",
            "Formulierung entfernt"
        );
        let corpus = "Das Boot liegt am Kai.";
        assert_eq!(
            crate::assist::prompts::mark_unbacked_criticals(line, corpus),
            line,
            "„an der Klippe“ steht nicht im Text, ist aber die neue Formulierung – kein Belegvermerk"
        );
        assert_eq!(strip_replacements("Besser wäre es."), "Besser wäre es.", "ohne Zitat bleibt der Satz");
    }

    #[test]
    fn the_diff_is_escaped_html() {
        let html = diff_html("<b>alt</b>\nbleibt", "neu & gut\nbleibt");
        assert!(html.contains("<del class=\"d-del\">&lt;b&gt;alt&lt;/b&gt;</del>"), "entfernt und maskiert: {html}");
        assert!(html.contains("<ins class=\"d-add\">neu &amp; gut</ins>"), "neu und maskiert: {html}");
        assert!(!html.contains("<b>"), "kein rohes HTML: {html}");
    }
}
