// SPDX-License-Identifier: AGPL-3.0-or-later
//! Search and replace in the work (Round 30a).
//!
//! - RPC `search.find`, `search.preview`, `search.replace`, `search.undo` (all with `work?`, the fields of
//!   [`SearchQuery`] flat); `replace`/`undo` only the human (`confirm: true`), not through MCP and not through `/act`.
//! - Pages without JavaScript ([`page`]): `GET /suche` and `POST /suche/act` (303 back to `/suche` with the range).
//! - Defaults (contract §3.1): text kinds for searching `text, heading, footnote, quote`, for replacing `text,
//!   heading, footnote` (Bible quotations and quotations stay, code never); `q` at most 500, `replacement` at most
//!   2000 characters; regex only with the crate `regex` (size limits in [`scan::pattern`]); 2000 ms per call
//!   (afterwards `timed_out: true`); at most 2000 hits (default 500), at most 500 sections per replace. `range=all`
//!   only for `search.find`, `rev=shown` only for `range=section|selection`.
//! - The search runs in the Typst source text of the revision ([`scan`]); offsets in bytes and UTF-16 units. In
//!   the literal mode a Typst special character also finds its escaped form, and the replacement is escaped; in
//!   the regex mode the replacement (with `$1`/`${name}`) stands literally in the source text.
//! - Replacing ([`replace`]): per section a new revision by the human through `Store::replace_apply`, exactly one
//!   mirror commit „Ersetzt „x“ → „y“ in n Abschnitten“; undo through `Store::replace_undo` with one commit.

pub mod page;
pub mod replace;
pub mod scan;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use baruch_core::{NodeKind, Work};
use serde_json::Value;

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::xref::{JumpTarget, jump_url};
use scan::{Found, PatternOptions};

/// Kind of the text in which a hit lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    Text,
    Heading,
    Footnote,
    Quote,
    Code,
}

impl TextKind {
    /// Name in addresses and JSON.
    pub fn word(self) -> &'static str {
        match self {
            TextKind::Text => "text",
            TextKind::Heading => "heading",
            TextKind::Footnote => "footnote",
            TextKind::Quote => "quote",
            TextKind::Code => "code",
        }
    }

    /// Label in the language of the request.
    pub fn label(self) -> String {
        match self {
            TextKind::Text => tc("reference_msg.search.kind_text", &[]),
            TextKind::Heading => tc("reference_msg.search.kind_heading", &[]),
            TextKind::Footnote => tc("reference_msg.search.kind_footnote", &[]),
            TextKind::Quote => tc("reference_msg.search.kind_quote", &[]),
            TextKind::Code => tc("reference_msg.search.kind_code", &[]),
        }
    }

    pub fn parse(word: &str) -> Option<TextKind> {
        ALL_KINDS.iter().copied().find(|k| k.word() == word)
    }
}

/// All text kinds in display order.
pub const ALL_KINDS: [TextKind; 5] =
    [TextKind::Text, TextKind::Heading, TextKind::Footnote, TextKind::Quote, TextKind::Code];
/// Default for searching.
pub const SEARCH_KINDS: [TextKind; 4] = [TextKind::Text, TextKind::Heading, TextKind::Footnote, TextKind::Quote];
/// Default for replacing.
pub const REPLACE_KINDS: [TextKind; 3] = [TextKind::Text, TextKind::Heading, TextKind::Footnote];

/// Range of a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchRange {
    Selection,
    #[default]
    Section,
    Chapter,
    Part,
    From,
    Picked,
    Work,
    All,
}

impl SearchRange {
    pub fn word(self) -> &'static str {
        match self {
            SearchRange::Selection => "selection",
            SearchRange::Section => "section",
            SearchRange::Chapter => "chapter",
            SearchRange::Part => "part",
            SearchRange::From => "from",
            SearchRange::Picked => "picked",
            SearchRange::Work => "work",
            SearchRange::All => "all",
        }
    }

    pub fn parse(word: &str) -> Option<SearchRange> {
        [
            SearchRange::Selection,
            SearchRange::Section,
            SearchRange::Chapter,
            SearchRange::Part,
            SearchRange::From,
            SearchRange::Picked,
            SearchRange::Work,
            SearchRange::All,
        ]
        .into_iter()
        .find(|r| r.word() == word)
    }

    /// Does the range need a section (`key`)?
    fn needs_key(self) -> bool {
        matches!(
            self,
            SearchRange::Selection
                | SearchRange::Section
                | SearchRange::Chapter
                | SearchRange::Part
                | SearchRange::From
        )
    }
}

/// Which revision is searched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevChoice {
    #[default]
    Current,
    Shown,
}

/// A search (RPC parameters flat, address parameters of `/suche` with the same names).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default)]
    pub case: bool,
    #[serde(default)]
    pub word: bool,
    #[serde(default, rename = "re")]
    pub regex: bool,
    #[serde(default)]
    pub range: SearchRange,
    pub key: Option<i64>,
    #[serde(default)]
    pub sel: Vec<i64>,
    /// Only `range=selection`, UTF-16 units.
    pub start: Option<usize>,
    pub end: Option<usize>,
    #[serde(default)]
    pub rev: RevChoice,
    pub shown_rev: Option<u32>,
    /// `None` = default (§3.1).
    pub kinds: Option<Vec<TextKind>>,
    /// Unsaved editor text of `key`.
    pub text: Option<String>,
    pub limit: Option<usize>,
}

impl SearchQuery {
    fn options(&self) -> PatternOptions {
        PatternOptions { case: self.case, word: self.word, regex: self.regex }
    }
}

/// A hit: section, revision, kind, byte and UTF-16 positions, line, context, jump address.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SearchHit {
    pub work: baruch_store::WorkId,
    pub key: i64,
    pub number: String,
    pub title: String,
    pub rev: u32,
    pub kind: TextKind,
    pub start: usize,
    pub end: usize,
    pub u16_start: usize,
    pub u16_end: usize,
    pub line: usize,
    pub before: String,
    pub matched: String,
    pub after: String,
    pub url: String,
}

/// Result of [`find`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub total: usize,
    pub truncated: bool,
    pub timed_out: bool,
    pub range_label: String,
}

/// Maximum length of the search text (characters).
pub const MAX_QUERY_CHARS: usize = 500;
/// Maximum length of the replacement (characters).
pub const MAX_REPLACEMENT_CHARS: usize = 2000;
/// Maximum number of hits per call and the default.
pub const MAX_HITS: usize = 2000;
pub const DEFAULT_HITS: usize = 500;
/// Time limit per call.
pub const TIME_LIMIT: Duration = Duration::from_millis(2000);
/// At most this many bytes of inserted replacement texts per preview or replace (all hits together); above that
/// Baruch refuses instead of filling the memory with hits × replacement.
pub const MAX_EXPANDED_BYTES: usize = 8 * 1024 * 1024;
/// Characters of context before and after a hit.
const CONTEXT_CHARS: usize = 40;

/// A section in the range with its searched text.
#[derive(Debug, Clone)]
pub(crate) struct Section {
    pub work: baruch_store::WorkId,
    pub key: i64,
    pub number: String,
    pub title: String,
    pub rev: u32,
    pub current: bool,
    pub text: String,
    /// Searched range (bytes) – for `selection` only the selection.
    pub window: std::ops::Range<usize>,
}

/// A section with its hits.
#[derive(Debug, Clone)]
pub(crate) struct SectionHits {
    pub section: Section,
    pub found: Vec<Found>,
}

/// Result of [`collect`].
pub(crate) struct Collected {
    pub sections: Vec<SectionHits>,
    pub total: usize,
    pub truncated: bool,
    pub timed_out: bool,
    pub range_label: String,
    /// The replacement texts exceeded [`MAX_EXPANDED_BYTES`]: collecting aborted, the result is incomplete.
    pub over_budget: bool,
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Check the limits of the search.
fn check_query(query: &SearchQuery) -> Result<(), RpcError> {
    let chars = query.q.chars().count();
    if chars == 0 {
        return Err(invalid(tc("reference_msg.search.query_missing", &[])));
    }
    if chars > MAX_QUERY_CHARS {
        return Err(invalid(tc(
            "reference_msg.search.query_too_long",
            &[("max", MAX_QUERY_CHARS.into()), ("chars", chars.into())],
        )));
    }
    if query.rev == RevChoice::Shown && !matches!(query.range, SearchRange::Section | SearchRange::Selection) {
        return Err(invalid(tc("reference_msg.search.shown_rev", &[])));
    }
    if query.range.needs_key() && query.key.is_none() {
        return Err(invalid(tc("reference_msg.search.key_missing", &[("range", query.range.word().into())])));
    }
    if query.range == SearchRange::Picked && query.sel.is_empty() {
        return Err(invalid(tc("reference_msg.search.sel_missing", &[])));
    }
    Ok(())
}

/// „2.3 Titel“.
fn label_of(number: &str, title: &str) -> String {
    format!("{number} {title}").trim().to_owned()
}

/// Positions (seq) of the section `root` and of its sub-items, in tree order.
fn subtree(work: &Work, root: usize) -> Vec<usize> {
    let mut out = vec![root];
    for node in work.nodes.iter().skip(root + 1) {
        let mut parent = node.parent;
        let mut inside = false;
        while let Some(p) = parent {
            if p == root {
                inside = true;
                break;
            }
            parent = work.nodes.get(p).and_then(|n| n.parent);
        }
        if !inside {
            break;
        }
        out.push(node.id);
    }
    out
}

/// Nearest ancestor (or the item itself) of the kind `kind`.
fn ancestor_of(work: &Work, seq: usize, kind: NodeKind) -> Option<usize> {
    let mut at = Some(seq);
    while let Some(s) = at {
        let node = work.nodes.get(s)?;
        if node.kind == kind {
            return Some(s);
        }
        at = node.parent;
    }
    None
}

/// Position of the fixed key in the work.
fn seq_of(work: &Work, key: i64) -> Result<usize, RpcError> {
    work.nodes
        .iter()
        .position(|n| n.key == key)
        .ok_or_else(|| invalid(tc("reference_msg.marks.no_section", &[("key", key.into())])))
}

/// Sections (positions) of a work in the range and the label of the range.
fn sections_in(work: &Work, query: &SearchQuery) -> Result<(Vec<usize>, String), RpcError> {
    let named = |seq: usize| {
        let n = &work.nodes[seq];
        label_of(&n.number, &n.title)
    };
    Ok(match query.range {
        SearchRange::Selection => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            (vec![seq], tc("reference_msg.search.range_selection", &[("name", named(seq).into())]))
        }
        SearchRange::Section => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            (vec![seq], tc("reference_msg.search.range_section", &[("name", named(seq).into())]))
        }
        SearchRange::Chapter => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            let root = ancestor_of(work, seq, NodeKind::Chapter).unwrap_or(seq);
            (subtree(work, root), tc("reference_msg.search.range_chapter", &[("name", named(root).into())]))
        }
        SearchRange::Part => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            match ancestor_of(work, seq, NodeKind::Part) {
                Some(root) => {
                    (subtree(work, root), tc("reference_msg.search.range_part", &[("name", named(root).into())]))
                }
                None => ((0..work.nodes.len()).collect(), tc("reference_msg.search.range_work_no_parts", &[])),
            }
        }
        SearchRange::From => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            ((seq..work.nodes.len()).collect(), tc("reference_msg.search.range_from", &[("name", named(seq).into())]))
        }
        SearchRange::Picked => {
            let mut seqs = Vec::new();
            for key in &query.sel {
                let seq = seq_of(work, *key)?;
                if !seqs.contains(&seq) {
                    seqs.push(seq);
                }
            }
            seqs.sort_unstable();
            let n = seqs.len();
            let label = if n == 1 {
                tc("reference_msg.search.range_picked_one", &[("count", n.into())])
            } else {
                tc("reference_msg.search.range_picked", &[("count", n.into())])
            };
            (seqs, label)
        }
        SearchRange::Work | SearchRange::All => {
            ((0..work.nodes.len()).collect(), tc("reference_msg.search.range_work", &[]))
        }
    })
}

/// Searched text of a section: unsaved editor text (only `key`, section/selection), shown revision
/// (`rev=shown`) or the current one. Without a revision → `None` (the section does not count).
fn section_text(
    work_id: baruch_store::WorkId,
    work: &Work,
    seq: usize,
    query: &SearchQuery,
) -> Result<Option<Section>, RpcError> {
    let node = &work.nodes[seq];
    let Some(current) = node.current_number() else { return Ok(None) };
    let own = query.key == Some(node.key) && matches!(query.range, SearchRange::Section | SearchRange::Selection);
    let rev = match (own, query.rev, query.shown_rev) {
        (true, RevChoice::Shown, Some(shown)) => shown,
        _ => current,
    };
    let text = match (&query.text, own) {
        (Some(text), true) => text.replace("\r\n", "\n"),
        _ => match node.revision(rev) {
            Some(r) => r.body.clone(),
            None => return Err(invalid(tc("reference_msg.marking.no_revision", &[("rev", rev.into())]))),
        },
    };
    let window = if own && query.range == SearchRange::Selection {
        let start = scan::byte_at_utf16(&text, query.start.unwrap_or(0));
        let end = scan::byte_at_utf16(&text, query.end.unwrap_or(usize::MAX)).max(start);
        start..end
    } else {
        0..text.len()
    };
    Ok(Some(Section {
        work: work_id,
        key: node.key,
        number: node.number.clone(),
        title: node.title.clone(),
        rev,
        current: rev == current,
        text,
        window,
    }))
}

/// Text kinds of the search or of the replace (code is never touched when replacing).
fn kinds_for(query: &SearchQuery, replace: bool) -> Vec<TextKind> {
    match (&query.kinds, replace) {
        (Some(kinds), false) if !kinds.is_empty() => kinds.clone(),
        (Some(kinds), true) if !kinds.is_empty() => kinds.iter().copied().filter(|k| *k != TextKind::Code).collect(),
        (_, false) => SEARCH_KINDS.to_vec(),
        (_, true) => REPLACE_KINDS.to_vec(),
    }
}

/// Collect hits in the range. `replacement`: also compute the replacement per hit (preview, replace; then the
/// text kinds of the replace apply). `hit_limit`: keep at most this many hits (counting goes on until the time).
pub(crate) async fn collect(
    state: &AppState,
    work_id: baruch_store::WorkId,
    query: &SearchQuery,
    replacement: Option<&str>,
    hit_limit: usize,
    limit: Duration,
) -> Result<Collected, RpcError> {
    check_query(query)?;
    let deadline = Instant::now() + limit;
    let re = scan::pattern(&query.q, query.options()).map_err(invalid)?;
    let kinds = kinds_for(query, replacement.is_some());
    let works: Vec<baruch_store::WorkId> = if query.range == SearchRange::All {
        state.store.works().await?.into_iter().map(|(id, _, _)| id).collect()
    } else {
        vec![work_id]
    };
    let mut out = Collected {
        sections: Vec::new(),
        total: 0,
        truncated: false,
        timed_out: false,
        range_label: String::new(),
        over_budget: false,
    };
    let mut kept = 0usize;
    let mut budget = MAX_EXPANDED_BYTES;
    for id in works {
        let work = state.store.load_work(id).await?;
        let (seqs, label) = sections_in(&work, query)?;
        out.range_label =
            if query.range == SearchRange::All { tc("reference_msg.search.range_all", &[]) } else { label };
        for seq in seqs {
            if Instant::now() >= deadline {
                out.timed_out = true;
                break;
            }
            let Some(section) = section_text(id, &work, seq, query)? else { continue };
            let (found, stop) = scan::find_in(
                &section.text,
                &re,
                &kinds,
                section.window.clone(),
                replacement.map(|r| (r, query.regex)),
                deadline,
                &mut budget,
            );
            if stop == scan::Stop::Budget {
                out.over_budget = true;
                out.truncated = true;
                break;
            }
            out.timed_out |= stop == scan::Stop::Late;
            if found.is_empty() {
                continue;
            }
            out.total += found.len();
            let room = hit_limit.saturating_sub(kept);
            if room == 0 {
                out.truncated = true;
                continue;
            }
            let mut found = found;
            if found.len() > room {
                found.truncate(room);
                out.truncated = true;
            }
            kept += found.len();
            out.sections.push(SectionHits { section, found });
        }
        if out.timed_out || out.over_budget {
            break;
        }
    }
    Ok(out)
}

/// Context: at most [`CONTEXT_CHARS`] characters on the same line.
fn context(text: &str, range: &std::ops::Range<usize>) -> (String, String) {
    let before_line = &text[..range.start];
    let before_line = before_line.rsplit('\n').next().unwrap_or("");
    let count = before_line.chars().count();
    let before: String = before_line.chars().skip(count.saturating_sub(CONTEXT_CHARS)).collect();
    let after_line = text[range.end..].split('\n').next().unwrap_or("");
    let after: String = after_line.chars().take(CONTEXT_CHARS).collect();
    (before, after)
}

/// Hit for the answer.
pub(crate) fn hit(section: &Section, found: &Found) -> SearchHit {
    let text = &section.text;
    let range = found.range.clone();
    let (before, after) = context(text, &range);
    let matched = text[range.clone()].to_owned();
    let target = JumpTarget {
        work: section.work,
        key: section.key,
        rev: (!section.current).then_some(section.rev),
        find: Some(matched.clone()),
        at: Some(range.start),
    };
    SearchHit {
        work: section.work,
        key: section.key,
        number: section.number.clone(),
        title: section.title.clone(),
        rev: section.rev,
        kind: found.kind,
        start: range.start,
        end: range.end,
        u16_start: scan::utf16_at(text, range.start),
        u16_end: scan::utf16_at(text, range.end),
        line: baruch_typst::line_at(text, range.start),
        before,
        matched,
        after,
        url: jump_url(&target),
    }
}

/// Limit of the hits from the request (default 500, at most 2000).
fn hit_limit(query: &SearchQuery) -> usize {
    query.limit.unwrap_or(DEFAULT_HITS).clamp(1, MAX_HITS)
}

/// Search in the work.
pub async fn find(state: &AppState, work: baruch_store::WorkId, query: &SearchQuery) -> Result<SearchResult, RpcError> {
    find_within(state, work, query, TIME_LIMIT).await
}

/// Like [`find`] with its own time limit (tests of the time limit).
#[doc(hidden)]
pub async fn find_within(
    state: &AppState,
    work: baruch_store::WorkId,
    query: &SearchQuery,
    limit: Duration,
) -> Result<SearchResult, RpcError> {
    let collected = collect(state, work, query, None, hit_limit(query), limit).await?;
    let hits = collected.sections.iter().flat_map(|s| s.found.iter().map(|f| hit(&s.section, f))).collect();
    Ok(SearchResult {
        hits,
        total: collected.total,
        truncated: collected.truncated,
        timed_out: collected.timed_out,
        range_label: collected.range_label,
    })
}

/// RPC methods of this module.
pub const METHODS: &[&str] = &["search.find", "search.preview", "search.replace", "search.undo"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Search from RPC parameters (flat).
pub(crate) fn query_from(params: &Value) -> Result<SearchQuery, RpcError> {
    serde_json::from_value(params.clone())
        .map_err(|e| invalid(tc("reference_msg.search.params", &[("error", e.to_string().into())])))
}

/// Answer RPC `search.*`.
pub async fn call(
    state: &AppState,
    work: baruch_store::WorkId,
    method: &str,
    params: Value,
) -> Result<Value, RpcError> {
    match method {
        "search.find" => {
            let query = query_from(&params)?;
            Ok(serde_json::to_value(find(state, work, &query).await?).expect("Suchergebnis ist JSON"))
        }
        "search.preview" => replace::preview_rpc(state, work, &params).await,
        "search.replace" => replace::replace_rpc(state, work, &params).await,
        "search.undo" => replace::undo_rpc(state, work, &params).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Pages (`GET /suche`).
pub fn pages() -> Router<Arc<AppState>> {
    page::pages()
}

/// Writing routes (`POST /suche/act`, behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    page::writes()
}
