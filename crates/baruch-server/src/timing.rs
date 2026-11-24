// SPDX-License-Identifier: AGPL-3.0-or-later
//! Zeitwächter (Runde 16b, Schnittstelle R16-S1): Sprechdauer je Gliederungspunkt und gesamt.
//!
//! Je Punkt in `seq`-Reihenfolge zählt die **aktuelle** Fassung ([`baruch_typst::timing::spoken_words`]);
//! `words`/`seconds` sind nur der eigene Text, `start_seconds` ist die Summe der `seconds` aller
//! vorherigen Punkte (Zeitmarke), `subtree_seconds` eigener Text und alle Nachfahren, `depth` die Zahl der
//! Vorfahren. Gesamt ist die Summe der Punkte; `status` ist `over` genau dann, wenn die Gesamtzeit die
//! Zielzeit übersteigt (`over_seconds` = Differenz), `none` ohne Zielzeit.
//!
//! Nur lesend: keine Schreibsperre, kein Spiegel-Commit. Die Einstellungen (Wörter je Minute, Zielzeit)
//! stehen nur in SQLite (`work_timing`), nicht im Spiegel.

use baruch_core::WorkKind;
use baruch_store::WorkId;
use baruch_typst::timing::{clock, speaking_seconds, spoken_words};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::RpcError;

/// Stand gegenüber der Zielzeit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingStatus {
    /// Keine Zielzeit gesetzt.
    None,
    Ok,
    Over,
}

/// Sprechdauer eines Gliederungspunkts.
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

/// Sprechdauer eines Werks.
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
    /// `true`, wenn Predigt oder Zielzeit gesetzt (dann zeigen Gliederung, Baum und Editorkopf die Zeiten).
    pub fn shown(&self) -> bool {
        self.kind == WorkKind::Sermon || self.target_seconds.is_some()
    }
}

/// Zeiten eines geladenen Werks mit seinen Einstellungen (rein, ohne Datenbank).
pub fn timing_of(work_id: WorkId, work: &baruch_core::Work, settings: baruch_store::TimingSettings) -> WorkTiming {
    let wpm = settings.words_per_minute;
    let count = work.nodes.len();
    let words: Vec<usize> =
        work.nodes.iter().map(|n| n.current_index().map_or(0, |i| spoken_words(&n.revisions[i].body))).collect();
    let seconds: Vec<u32> = words.iter().map(|&w| speaking_seconds(w, wpm)).collect();
    // Nachfahren stehen in Baumreihenfolge hinter ihrem Elternteil: rückwärts aufsummieren.
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

/// Zeiten eines Werks aus der Datenbank (nur lesend).
pub async fn work_timing(state: &AppState, work: WorkId) -> Result<WorkTiming, RpcError> {
    let settings = state.store.timing_settings(work).await?;
    let loaded = state.store.load_work(work).await?;
    Ok(timing_of(work, &loaded, settings))
}

/// JSON-Antwort von `work.timing` (und MCP `work_timing`): alle Felder, dazu `shown`, `clock`,
/// `target_clock`, `over_clock` und je Abschnitt `clock`, `start_clock`, `subtree_clock`.
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

/// Längster Text für `timing.count` (Zeichen).
pub const MAX_COUNT_CHARS: usize = 1_000_000;

/// Bedient dieses Modul die RPC-Methode? `work.timing`, `work.timing_set`, `timing.count`.
pub fn handles(method: &str) -> bool {
    matches!(method, "work.timing" | "work.timing_set" | "timing.count")
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(crate::rpc::INVALID_PARAMS, message)
}

/// Ganze Zahl aus `params[name]` (Zahl oder Ziffern als Text, Formulare): fehlt → `None`, `null` oder
/// leerer Text → `Some(None)`.
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

/// `work.timing_set`: neue Einstellungen aus den Parametern über die alten gelegt (fehlende Felder bleiben).
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

/// Einstellungen eines Werks setzen (nur der Mensch: RPC und Formular, nicht MCP, nicht `/act`).
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
/// `timing.count {work?, text}` (`docs/RPC.md`, „Zeitwächter“). `work` ist schon aufgelöst
/// (fehlt → Vorgabewerk); `work.timing_set` verlangt es ausdrücklich.
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
        _ => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}
