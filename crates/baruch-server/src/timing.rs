// SPDX-License-Identifier: AGPL-3.0-or-later
//! Time guard (Round 16b, interface R16-S1): speaking time per outline item and in total.
//!
//! Per item in `seq` order the **current** revision counts ([`baruch_typst::timing::spoken_words`]);
//! `words`/`seconds` are only the own text, `start_seconds` is the sum of the `seconds` of all
//! previous items (time mark), `subtree_seconds` own text and all descendants, `depth` the number of
//! ancestors. The total is the sum of the items; `status` is `over` exactly when the total time exceeds the
//! target time (`over_seconds` = difference), `none` without a target time.
//!
//! Read only: no write lock, no mirror commit. The settings (words per minute, target time)
//! are only in SQLite (`work_timing`), not in the mirror.

use baruch_core::WorkKind;
use baruch_store::WorkId;
use baruch_typst::timing::{clock, speaking_seconds, spoken_words};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::RpcError;

/// State compared with the target time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingStatus {
    /// No target time set.
    None,
    Ok,
    Over,
}

/// Speaking time of an outline item.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SectionTiming {
    pub key: i64,
    pub seq: usize,
    pub depth: usize,
    pub number: String,
    pub title: String,
    pub words: usize,
    pub seconds: u32,
    pub start_seconds: u32,
    pub subtree_seconds: u32,
}

/// Speaking time of a work.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WorkTiming {
    pub work: WorkId,
    pub kind: WorkKind,
    pub words_per_minute: u32,
    pub target_seconds: Option<u32>,
    pub words: usize,
    pub seconds: u32,
    pub status: TimingStatus,
    pub over_seconds: Option<u32>,
    pub sections: Vec<SectionTiming>,
}

impl WorkTiming {
    /// `true` if sermon or target time set (then outline, tree and editor header show the times).
    pub fn shown(&self) -> bool {
        self.kind == WorkKind::Sermon || self.target_seconds.is_some()
    }
}

/// Times of a loaded work with its settings (pure, without database).
pub fn timing_of(work_id: WorkId, work: &baruch_core::Work, settings: baruch_store::TimingSettings) -> WorkTiming {
    let wpm = settings.words_per_minute;
    let count = work.nodes.len();
    let words: Vec<usize> =
        work.nodes.iter().map(|n| n.current_index().map_or(0, |i| spoken_words(&n.revisions[i].body))).collect();
    let seconds: Vec<u32> = words.iter().map(|&w| speaking_seconds(w, wpm)).collect();
    // Descendants stand in tree order behind their parent: sum up backwards.
    let mut subtree = seconds.clone();
    for index in (0..count).rev() {
        if let Some(parent) = work.nodes[index].parent.filter(|&p| p < index) {
            subtree[parent] = subtree[parent].saturating_add(subtree[index]);
        }
    }
    let mut sections = Vec::with_capacity(count);
    let mut start = 0u32;
    for (index, node) in work.nodes.iter().enumerate() {
        let mut depth = 0;
        let mut up = node.parent;
        while let Some(p) = up {
            if depth > count {
                break;
            }
            depth += 1;
            up = work.nodes.get(p).and_then(|n| n.parent);
        }
        sections.push(SectionTiming {
            key: node.key,
            seq: index,
            depth,
            number: node.number.clone(),
            title: node.title.clone(),
            words: words[index],
            seconds: seconds[index],
            start_seconds: start,
            subtree_seconds: subtree[index],
        });
        start = start.saturating_add(seconds[index]);
    }
    let total: u32 = start;
    let (status, over_seconds) = match settings.target_seconds {
        None => (TimingStatus::None, None),
        Some(target) if total > target => (TimingStatus::Over, Some(total - target)),
        Some(_) => (TimingStatus::Ok, None),
    };
    WorkTiming {
        work: work_id,
        kind: work.kind,
        words_per_minute: wpm,
        target_seconds: settings.target_seconds,
        words: words.iter().sum(),
        seconds: total,
        status,
        over_seconds,
        sections,
    }
}

/// Times of a work from the database (read only).
pub async fn work_timing(state: &AppState, work: WorkId) -> Result<WorkTiming, RpcError> {
    let settings = state.store.timing_settings(work).await?;
    let loaded = state.store.load_work(work).await?;
    Ok(timing_of(work, &loaded, settings))
}

/// JSON answer of `work.timing` (and MCP `work_timing`): all fields, plus `shown`, `clock`,
/// `target_clock`, `over_clock` and per section `clock`, `start_clock`, `subtree_clock`.
pub fn timing_json(timing: &WorkTiming) -> Value {
    let mut value = json!(timing);
    value["shown"] = json!(timing.shown());
    value["clock"] = json!(clock(timing.seconds));
    value["target_clock"] = json!(timing.target_seconds.map(clock));
    value["over_clock"] = json!(timing.over_seconds.map(clock));
    if let Some(sections) = value["sections"].as_array_mut() {
        for (entry, section) in sections.iter_mut().zip(&timing.sections) {
            entry["clock"] = json!(clock(section.seconds));
            entry["start_clock"] = json!(clock(section.start_seconds));
            entry["subtree_clock"] = json!(clock(section.subtree_seconds));
        }
    }
    value
}

// ---- RPC ------------------------------------------------------------------------------------------

/// Longest text for `timing.count` (characters).
pub const MAX_COUNT_CHARS: usize = 1_000_000;

/// Does this module serve the RPC method? `work.timing`, `work.timing_set`, `timing.count`.
pub fn handles(method: &str) -> bool {
    matches!(method, "work.timing" | "work.timing_set" | "timing.count")
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(crate::rpc::INVALID_PARAMS, message)
}

/// Whole number from `params[name]` (number or digits as text, forms): missing → `None`, `null` or
/// empty text → `Some(None)`.
fn whole_number(params: &Value, name: &str) -> Result<Option<Option<i64>>, RpcError> {
    let wrong = || invalid(format!("{name}: ganze Zahl erwartet"));
    match params.get(name) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::Number(n)) => match n.as_i64() {
            Some(v) => Ok(Some(Some(v))),
            None => match n.as_f64() {
                Some(f) if f.fract() == 0.0 && f.abs() < 1e12 => Ok(Some(Some(f as i64))),
                _ => Err(wrong()),
            },
        },
        Some(Value::String(s)) if s.trim().is_empty() => Ok(Some(None)),
        Some(Value::String(s)) => s.trim().parse::<i64>().map(|v| Some(Some(v))).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// `work.timing_set`: new settings from the parameters laid over the old ones (missing fields stay).
pub fn settings_from_params(
    old: baruch_store::TimingSettings,
    params: &Value,
) -> Result<baruch_store::TimingSettings, RpcError> {
    use baruch_typst::timing::{MAX_TARGET_MINUTES, MAX_WORDS_PER_MINUTE, MIN_WORDS_PER_MINUTE};
    let mut new = old;
    match whole_number(params, "words_per_minute")? {
        None => {}
        Some(None) => return Err(invalid("words_per_minute: ganze Zahl von 60 bis 250 erwartet")),
        Some(Some(wpm)) => {
            if !(i64::from(MIN_WORDS_PER_MINUTE)..=i64::from(MAX_WORDS_PER_MINUTE)).contains(&wpm) {
                return Err(invalid("Wörter je Minute: erlaubt sind 60 bis 250."));
            }
            new.words_per_minute = wpm as u32;
        }
    }
    match whole_number(params, "target_minutes")? {
        None => {}
        Some(None) => new.target_seconds = None,
        Some(Some(minutes)) => {
            if !(1..=i64::from(MAX_TARGET_MINUTES)).contains(&minutes) {
                return Err(invalid("Zielzeit: erlaubt sind 1 bis 240 Minuten (leer bzw. null = keine Zielzeit)."));
            }
            new.target_seconds = Some(minutes as u32 * 60);
        }
    }
    Ok(new)
}

/// Set the settings of a work (only the human: RPC and form, not MCP, not `/act`).
pub async fn set_timing(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let old = state.store.timing_settings(work).await?;
    let new = settings_from_params(old, params)?;
    state.store.set_timing_settings(work, new).await.map_err(|e| match e {
        baruch_store::StoreError::Refused(m) => invalid(m),
        e => e.into(),
    })?;
    Ok(json!({ "work": work, "words_per_minute": new.words_per_minute, "target_seconds": new.target_seconds }))
}

/// RPC `work.timing {work?}`, `work.timing_set {work, words_per_minute?, target_minutes?}`,
/// `timing.count {work?, text}` (`docs/RPC.md`, „Zeitwächter“). `work` is already resolved
/// (missing → default work); `work.timing_set` requires it explicitly.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    match method {
        "work.timing" => Ok(timing_json(&work_timing(state, work).await?)),
        "work.timing_set" => {
            if matches!(params.get("work"), None | Some(Value::Null)) {
                return Err(invalid("work fehlt: Zeiten setzt man je Werk (work: Werk-id)."));
            }
            set_timing(state, work, &params).await
        }
        "timing.count" => {
            let text = match params.get("text") {
                Some(Value::String(s)) => s,
                Some(_) => return Err(invalid("text: Text erwartet")),
                None => return Err(invalid("text fehlt")),
            };
            if text.chars().count() > MAX_COUNT_CHARS {
                return Err(invalid("text: höchstens 1 000 000 Zeichen"));
            }
            let wpm = state.store.timing_settings(work).await?.words_per_minute;
            let words = spoken_words(text);
            let seconds = speaking_seconds(words, wpm);
            Ok(json!({ "words": words, "seconds": seconds, "clock": clock(seconds), "words_per_minute": wpm }))
        }
        _ => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}
