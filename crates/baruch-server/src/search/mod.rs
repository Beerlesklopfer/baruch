// SPDX-License-Identifier: AGPL-3.0-or-later
//! Im Werk suchen und ersetzen (Runde 30a).
//!
//! - RPC `search.find`, `search.preview`, `search.replace`, `search.undo` (alle mit `work?`, die Felder von
//!   [`SearchQuery`] flach); `replace`/`undo` nur der Mensch (`confirm: true`), nicht über MCP und nicht über `/act`.
//! - Seiten ohne JavaScript ([`page`]): `GET /suche` und `POST /suche/act` (303 zurück auf `/suche` mit dem Bereich).
//! - Vorgaben (Vertrag §3.1): Textarten beim Suchen `text, heading, footnote, quote`, beim Ersetzen `text, heading,
//!   footnote` (Bibelzitate und Zitate bleiben, Code nie); `q` höchstens 500, `replacement` höchstens 2000 Zeichen;
//!   Regex nur mit dem Crate `regex` (Größengrenzen in [`scan::pattern`]); 2000 ms je Aufruf (danach
//!   `timed_out: true`); höchstens 2000 Treffer (Vorgabe 500), höchstens 500 Abschnitte je Ersetzen. `range=all` nur
//!   bei `search.find`, `rev=shown` nur bei `range=section|selection`.
//! - Gesucht wird im Typst-Quelltext der Fassung ([`scan`]); Offsets in Bytes und UTF-16-Einheiten. Im
//!   Wortlaut-Modus findet ein Typst-Sonderzeichen auch seine maskierte Form, und der Ersatz wird maskiert; im
//!   Regex-Modus steht der Ersatz (mit `$1`/`${name}`) wörtlich im Quelltext.
//! - Ersetzen ([`replace`]): je Abschnitt eine neue Fassung vom Menschen über `Store::replace_apply`, genau ein
//!   Spiegel-Commit „Ersetzt „x“ → „y“ in n Abschnitten“; Rückgängig über `Store::replace_undo` mit einem Commit.

pub mod page;
pub mod replace;
pub mod scan;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use baruch_core::{NodeKind, Work};
use serde_json::Value;

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::xref::{JumpTarget, jump_url};
use scan::{Found, PatternOptions};

/// Art des Texts, in dem ein Treffer liegt.
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
    /// Name in Adressen und JSON.
    pub fn word(self) -> &'static str {
        match self {
            TextKind::Text => "text",
            TextKind::Heading => "heading",
            TextKind::Footnote => "footnote",
            TextKind::Quote => "quote",
            TextKind::Code => "code",
        }
    }

    /// Deutsche Bezeichnung.
    pub fn label(self) -> &'static str {
        match self {
            TextKind::Text => "Text",
            TextKind::Heading => "Überschriften",
            TextKind::Footnote => "Fußnoten",
            TextKind::Quote => "Zitate (auch Bibelzitate)",
            TextKind::Code => "Typst-Code",
        }
    }

    pub fn parse(word: &str) -> Option<TextKind> {
        ALL_KINDS.iter().copied().find(|k| k.word() == word)
    }
}

/// Alle Textarten in Anzeigereihenfolge.
pub const ALL_KINDS: [TextKind; 5] =
    [TextKind::Text, TextKind::Heading, TextKind::Footnote, TextKind::Quote, TextKind::Code];
/// Vorgabe beim Suchen.
pub const SEARCH_KINDS: [TextKind; 4] = [TextKind::Text, TextKind::Heading, TextKind::Footnote, TextKind::Quote];
/// Vorgabe beim Ersetzen.
pub const REPLACE_KINDS: [TextKind; 3] = [TextKind::Text, TextKind::Heading, TextKind::Footnote];

/// Bereich einer Suche.
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

    /// Braucht der Bereich einen Abschnitt (`key`)?
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

/// Welche Fassung durchsucht wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevChoice {
    #[default]
    Current,
    Shown,
}

/// Eine Suche (RPC-Parameter flach, Adress-Parameter von `/suche` mit denselben Namen).
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
    /// Nur `range=selection`, UTF-16-Einheiten.
    pub start: Option<usize>,
    pub end: Option<usize>,
    #[serde(default)]
    pub rev: RevChoice,
    pub shown_rev: Option<u32>,
    /// `None` = Vorgabe (§3.1).
    pub kinds: Option<Vec<TextKind>>,
    /// Ungespeicherter Editortext von `key`.
    pub text: Option<String>,
    pub limit: Option<usize>,
}

impl SearchQuery {
    fn options(&self) -> PatternOptions {
        PatternOptions { case: self.case, word: self.word, regex: self.regex }
    }
}

/// Ein Treffer: Abschnitt, Fassung, Art, Byte- und UTF-16-Stellen, Zeile, Umfeld, Sprung-Adresse.
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

/// Ergebnis von [`find`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub total: usize,
    pub truncated: bool,
    pub timed_out: bool,
    pub range_label: String,
}

/// Höchstlänge des Suchtexts (Zeichen).
pub const MAX_QUERY_CHARS: usize = 500;
/// Höchstlänge des Ersatzes (Zeichen).
pub const MAX_REPLACEMENT_CHARS: usize = 2000;
/// Höchstzahl der Treffer je Aufruf und die Vorgabe.
pub const MAX_HITS: usize = 2000;
pub const DEFAULT_HITS: usize = 500;
/// Zeitgrenze je Aufruf.
pub const TIME_LIMIT: Duration = Duration::from_millis(2000);
/// Höchstens so viele Bytes eingesetzter Ersatztexte je Vorschau bzw. Ersetzen (alle Treffer zusammen); darüber
/// lehnt Baruch ab, statt den Speicher mit Treffern × Ersatz zu füllen.
pub const MAX_EXPANDED_BYTES: usize = 8 * 1024 * 1024;
/// Zeichen Umfeld vor und hinter einem Treffer.
const CONTEXT_CHARS: usize = 40;

/// Ein Abschnitt im Bereich mit seinem durchsuchten Text.
#[derive(Debug, Clone)]
pub(crate) struct Section {
    pub work: baruch_store::WorkId,
    pub key: i64,
    pub number: String,
    pub title: String,
    pub rev: u32,
    pub current: bool,
    pub text: String,
    /// Durchsuchter Bereich (Bytes) – bei `selection` nur die Auswahl.
    pub window: std::ops::Range<usize>,
}

/// Ein Abschnitt mit seinen Treffern.
#[derive(Debug, Clone)]
pub(crate) struct SectionHits {
    pub section: Section,
    pub found: Vec<Found>,
}

/// Ergebnis von [`collect`].
pub(crate) struct Collected {
    pub sections: Vec<SectionHits>,
    pub total: usize,
    pub truncated: bool,
    pub timed_out: bool,
    pub range_label: String,
    /// Die Ersatztexte überschritten [`MAX_EXPANDED_BYTES`]: Sammeln abgebrochen, das Ergebnis ist unvollständig.
    pub over_budget: bool,
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Grenzen der Suche prüfen.
fn check_query(query: &SearchQuery) -> Result<(), RpcError> {
    let chars = query.q.chars().count();
    if chars == 0 {
        return Err(invalid("Bitte einen Suchtext eingeben."));
    }
    if chars > MAX_QUERY_CHARS {
        return Err(invalid(format!("Der Suchtext darf höchstens {MAX_QUERY_CHARS} Zeichen haben (hier {chars}).")));
    }
    if query.rev == RevChoice::Shown && !matches!(query.range, SearchRange::Section | SearchRange::Selection) {
        return Err(invalid("Die gezeigte Fassung lässt sich nur im Abschnitt oder in der Auswahl durchsuchen."));
    }
    if query.range.needs_key() && query.key.is_none() {
        return Err(invalid(format!("Für den Bereich „{}“ fehlt der Abschnitt (key).", query.range.word())));
    }
    if query.range == SearchRange::Picked && query.sel.is_empty() {
        return Err(invalid("Bitte mindestens einen Gliederungspunkt auswählen (sel)."));
    }
    Ok(())
}

/// „2.3 Titel“.
fn label_of(number: &str, title: &str) -> String {
    format!("{number} {title}").trim().to_owned()
}

/// Stellen (seq) des Abschnitts `root` und seiner Unterpunkte, in Baumreihenfolge.
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

/// Nächster Vorfahr (oder der Punkt selbst) der Art `kind`.
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

/// Stelle des festen Schlüssels im Werk.
fn seq_of(work: &Work, key: i64) -> Result<usize, RpcError> {
    work.nodes
        .iter()
        .position(|n| n.key == key)
        .ok_or_else(|| invalid(format!("Den Abschnitt {key} gibt es in diesem Werk nicht.")))
}

/// Abschnitte (Stellen) eines Werks im Bereich und die Beschriftung des Bereichs.
fn sections_in(work: &Work, query: &SearchQuery) -> Result<(Vec<usize>, String), RpcError> {
    let named = |seq: usize| {
        let n = &work.nodes[seq];
        label_of(&n.number, &n.title)
    };
    Ok(match query.range {
        SearchRange::Selection => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            (vec![seq], format!("Auswahl in {}", named(seq)))
        }
        SearchRange::Section => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            (vec![seq], format!("Abschnitt {}", named(seq)))
        }
        SearchRange::Chapter => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            let root = ancestor_of(work, seq, NodeKind::Chapter).unwrap_or(seq);
            (subtree(work, root), format!("Kapitel {}", named(root)))
        }
        SearchRange::Part => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            match ancestor_of(work, seq, NodeKind::Part) {
                Some(root) => (subtree(work, root), format!("Teil {}", named(root))),
                None => ((0..work.nodes.len()).collect(), "ganzes Werk (keine Teile)".to_owned()),
            }
        }
        SearchRange::From => {
            let seq = seq_of(work, query.key.unwrap_or_default())?;
            ((seq..work.nodes.len()).collect(), format!("ab {} bis zum Ende", named(seq)))
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
            (seqs, if n == 1 { "1 ausgewählter Abschnitt".to_owned() } else { format!("{n} ausgewählte Abschnitte") })
        }
        SearchRange::Work | SearchRange::All => ((0..work.nodes.len()).collect(), "ganzes Werk".to_owned()),
    })
}

/// Durchsuchter Text eines Abschnitts: ungespeicherter Editortext (nur `key`, Abschnitt/Auswahl), gezeigte Fassung
/// (`rev=shown`) oder die aktuelle. Ohne Fassung → `None` (der Abschnitt zählt nicht).
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
            None => return Err(invalid(format!("Fassung {rev} gibt es in diesem Abschnitt nicht."))),
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

/// Textarten der Suche bzw. des Ersetzens (Code wird beim Ersetzen nie angefasst).
fn kinds_for(query: &SearchQuery, replace: bool) -> Vec<TextKind> {
    match (&query.kinds, replace) {
        (Some(kinds), false) if !kinds.is_empty() => kinds.clone(),
        (Some(kinds), true) if !kinds.is_empty() => kinds.iter().copied().filter(|k| *k != TextKind::Code).collect(),
        (_, false) => SEARCH_KINDS.to_vec(),
        (_, true) => REPLACE_KINDS.to_vec(),
    }
}

/// Treffer im Bereich sammeln. `replacement`: Ersatz je Treffer mitrechnen (Vorschau, Ersetzen; dann gelten die
/// Textarten des Ersetzens). `hit_limit`: höchstens so viele Treffer behalten (gezählt wird weiter bis zur Zeit).
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
        out.range_label = if query.range == SearchRange::All { "alle Werke".to_owned() } else { label };
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

/// Umfeld: höchstens [`CONTEXT_CHARS`] Zeichen auf derselben Zeile.
fn context(text: &str, range: &std::ops::Range<usize>) -> (String, String) {
    let before_line = &text[..range.start];
    let before_line = before_line.rsplit('\n').next().unwrap_or("");
    let count = before_line.chars().count();
    let before: String = before_line.chars().skip(count.saturating_sub(CONTEXT_CHARS)).collect();
    let after_line = text[range.end..].split('\n').next().unwrap_or("");
    let after: String = after_line.chars().take(CONTEXT_CHARS).collect();
    (before, after)
}

/// Treffer für die Antwort.
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

/// Grenze der Treffer aus der Anfrage (Vorgabe 500, höchstens 2000).
fn hit_limit(query: &SearchQuery) -> usize {
    query.limit.unwrap_or(DEFAULT_HITS).clamp(1, MAX_HITS)
}

/// Im Werk suchen.
pub async fn find(state: &AppState, work: baruch_store::WorkId, query: &SearchQuery) -> Result<SearchResult, RpcError> {
    find_within(state, work, query, TIME_LIMIT).await
}

/// Wie [`find`] mit eigener Zeitgrenze (Tests der Zeitgrenze).
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

/// RPC-Methoden dieses Moduls.
pub const METHODS: &[&str] = &["search.find", "search.preview", "search.replace", "search.undo"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Suche aus RPC-Parametern (flach).
pub(crate) fn query_from(params: &Value) -> Result<SearchQuery, RpcError> {
    serde_json::from_value(params.clone()).map_err(|e| invalid(format!("Suche: {e}")))
}

/// RPC `search.*` beantworten.
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

/// Seiten (`GET /suche`).
pub fn pages() -> Router<Arc<AppState>> {
    page::pages()
}

/// Schreibende Routen (`POST /suche/act`, hinter `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    page::writes()
}
