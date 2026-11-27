// SPDX-License-Identifier: AGPL-3.0-or-later
//! Live view in the chat (Round 30c, chat-bedienung): „@lektor schreibt …“ while a model order runs, and
//! errors of failed model calls – both for `post.since` and `thread.since` (polled from `web/src/chat.ts`).
//!
//! - [`typing`] (section chat) and [`typing_thread`] (thread) register a persona and return a guard
//!   ([`TypingGuard`]); when the guard drops (end, error or cancellation of the order), the persona is
//!   deregistered again. If the same persona runs twice in the same place, every guard counts for itself (the
//!   display stays until the last one drops).
//! - [`record_failure`] and [`record_thread_failure`] note a failed call (persona, message, time);
//!   [`failures_now`] and [`failures_thread`] return those of the last [`FAILURE_WINDOW_SECS`] seconds, at most
//!   [`MAX_FAILURES`] per place (the newest). The browser shows each only once (it remembers `at`).
//!
//! Both live only in the memory of the process (no migration, no mirror): after a restart no order
//! runs any more either. The key is the work and the fixed key of the section or the id of the thread.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use baruch_store::WorkId;
use serde::Serialize;

/// Show errors this long (seconds).
pub const FAILURE_WINDOW_SECS: u64 = 600;
/// At most this many errors per place.
pub const MAX_FAILURES: usize = 5;

/// Place of a display: section (fixed key) or thread (id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Place {
    Section(i64),
    Thread(i64),
}

/// A running guard.
#[derive(Debug, Clone)]
struct Entry {
    id: u64,
    persona: String,
    since: String,
}

/// A failed call.
#[derive(Debug, Clone)]
struct Fail {
    persona: String,
    message: String,
    at: String,
    when: Instant,
}

#[derive(Default)]
struct Live {
    next: u64,
    typing: HashMap<(WorkId, Place), Vec<Entry>>,
    failures: HashMap<(WorkId, Place), Vec<Fail>>,
}

fn live() -> std::sync::MutexGuard<'static, Live> {
    static LIVE: std::sync::OnceLock<Mutex<Live>> = std::sync::OnceLock::new();
    LIVE.get_or_init(|| Mutex::new(Live::default())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Now as ISO time (UTC, milliseconds) – like the times of the database, comparable as text.
fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Handle with „@“ (orders name the persona sometimes with, sometimes without).
fn handle(persona: &str) -> String {
    let p = persona.trim();
    if p.starts_with('@') { p.to_owned() } else { format!("@{p}") }
}

/// Guard of a running order; on drop the persona is deregistered.
pub struct TypingGuard {
    key: Option<(WorkId, Place)>,
    id: u64,
}

impl Drop for TypingGuard {
    fn drop(&mut self) {
        let Some(key) = self.key else { return };
        let mut live = live();
        if let Some(list) = live.typing.get_mut(&key) {
            list.retain(|e| e.id != self.id);
            if list.is_empty() {
                live.typing.remove(&key);
            }
        }
    }
}

fn enter(work: WorkId, place: Place, persona: &str) -> TypingGuard {
    if persona.trim().is_empty() {
        return TypingGuard { key: None, id: 0 };
    }
    let mut live = live();
    live.next += 1;
    let id = live.next;
    live.typing.entry((work, place)).or_default().push(Entry { id, persona: handle(persona), since: now_iso() });
    TypingGuard { key: Some((work, place)), id }
}

/// Persona `persona` is writing in the section `key` of the work `work` (until the guard drops).
pub fn typing(work: WorkId, key: i64, persona: &str) -> TypingGuard {
    enter(work, Place::Section(key), persona)
}

/// Persona `persona` is writing in the thread `thread` of the work `work` (until the guard drops).
pub fn typing_thread(work: WorkId, thread: i64, persona: &str) -> TypingGuard {
    enter(work, Place::Thread(thread), persona)
}

/// A persona that is writing right now, and since when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Typing {
    pub persona: String,
    pub since: String,
}

fn typing_at(work: WorkId, place: Place) -> Vec<Typing> {
    let live = live();
    let mut out: Vec<Typing> = Vec::new();
    for e in live.typing.get(&(work, place)).map(Vec::as_slice).unwrap_or_default() {
        // Show the same persona twice (two orders) only once, with the earlier start.
        if !out.iter().any(|t| t.persona == e.persona) {
            out.push(Typing { persona: e.persona.clone(), since: e.since.clone() });
        }
    }
    out
}

/// Who is writing in the section `key` right now?
pub fn typing_now(work: WorkId, key: i64) -> Vec<Typing> {
    typing_at(work, Place::Section(key))
}

/// Who is writing in the thread `thread` right now?
pub fn typing_in_thread(work: WorkId, thread: i64) -> Vec<Typing> {
    typing_at(work, Place::Thread(thread))
}

/// A failed model call for the display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Failure {
    pub persona: String,
    pub message: String,
    pub at: String,
}

fn record(work: WorkId, place: Place, persona: &str, message: &str) {
    let mut live = live();
    let list = live.failures.entry((work, place)).or_default();
    let window = Duration::from_secs(FAILURE_WINDOW_SECS);
    list.retain(|f| f.when.elapsed() < window);
    let persona = if persona.trim().is_empty() { String::new() } else { handle(persona) };
    list.push(Fail { persona, message: message.to_owned(), at: now_iso(), when: Instant::now() });
    let excess = list.len().saturating_sub(MAX_FAILURES);
    list.drain(..excess);
}

/// Note a failed call in the section `key`.
pub fn record_failure(work: WorkId, key: i64, persona: &str, message: &str) {
    record(work, Place::Section(key), persona, message);
}

/// Note a failed call in the thread `thread`.
pub fn record_thread_failure(work: WorkId, thread: i64, persona: &str, message: &str) {
    record(work, Place::Thread(thread), persona, message);
}

fn failures_at(work: WorkId, place: Place) -> Vec<Failure> {
    let live = live();
    let window = Duration::from_secs(FAILURE_WINDOW_SECS);
    live.failures
        .get(&(work, place))
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|f| f.when.elapsed() < window)
        .map(|f| Failure { persona: f.persona.clone(), message: f.message.clone(), at: f.at.clone() })
        .collect()
}

/// Failed calls of the last minutes in the section `key` (oldest first).
pub fn failures_now(work: WorkId, key: i64) -> Vec<Failure> {
    failures_at(work, Place::Section(key))
}

/// Failed calls of the last minutes in the thread `thread` (oldest first).
pub fn failures_thread(work: WorkId, thread: i64) -> Vec<Failure> {
    failures_at(work, Place::Thread(thread))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Own work ids that no integration test uses: the display is process-wide.
    const WORK: WorkId = 9_000_001;

    #[test]
    fn a_persona_is_typing_until_the_last_guard_falls() {
        assert!(typing_now(WORK, 7).is_empty(), "am Anfang schreibt niemand");
        let first = typing(WORK, 7, "lektor");
        let second = typing(WORK, 7, "@lektor");
        let other = typing(WORK, 8, "@exeget");
        let now = typing_now(WORK, 7);
        assert_eq!(now.len(), 1, "dieselbe Persona zweimal wird einmal gezeigt: {now:?}");
        assert_eq!(now[0].persona, "@lektor", "Handle immer mit „@“");
        assert!(now[0].since.ends_with('Z'), "Zeit als ISO-Text wie die Datenbank: {}", now[0].since);
        drop(first);
        assert_eq!(typing_now(WORK, 7).len(), 1, "eine Wache läuft noch: weiter „schreibt …“");
        drop(second);
        assert!(typing_now(WORK, 7).is_empty(), "letzte Wache gefallen: ausgetragen");
        assert_eq!(typing_now(WORK, 8)[0].persona, "@exeget", "anderer Abschnitt bleibt unberührt");
        drop(other);
        assert!(typing_now(WORK, 8).is_empty(), "auch dort ausgetragen");
    }

    #[test]
    fn sections_and_threads_with_the_same_number_are_separate_places() {
        let thread = typing_thread(WORK + 1, 3, "@schrift");
        assert!(typing_now(WORK + 1, 3).is_empty(), "Faden 3 ist nicht Abschnitt 3");
        assert_eq!(typing_in_thread(WORK + 1, 3).len(), 1, "im Faden steht die Persona");
        drop(thread);
        assert!(typing_in_thread(WORK + 1, 3).is_empty(), "nach dem Auftrag ausgetragen");
        let empty = typing(WORK + 1, 3, "  ");
        assert!(typing_now(WORK + 1, 3).is_empty(), "ohne Persona keine Anzeige");
        drop(empty);
    }

    #[test]
    fn failures_are_kept_per_place_and_only_the_newest_few() {
        for i in 0..(MAX_FAILURES + 2) {
            record_failure(WORK + 2, 1, "lektor", &format!("Fehler {i}"));
        }
        record_thread_failure(WORK + 2, 1, "@exeget", "Faden-Fehler");
        let section = failures_now(WORK + 2, 1);
        assert_eq!(section.len(), MAX_FAILURES, "höchstens {MAX_FAILURES} je Ort");
        assert_eq!(section.last().unwrap().message, format!("Fehler {}", MAX_FAILURES + 1), "die neuesten bleiben");
        assert_eq!(section[0].persona, "@lektor", "Handle mit „@“");
        let thread = failures_thread(WORK + 2, 1);
        assert_eq!(thread.len(), 1, "Faden getrennt vom Abschnitt: {thread:?}");
    }
}
