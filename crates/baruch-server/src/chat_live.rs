// SPDX-License-Identifier: AGPL-3.0-or-later
//! Live-Anzeige im Chat (Runde 30c, chat-bedienung): „@lektor schreibt …“, solange ein Modellauftrag läuft, und
//! Fehler gescheiterter Modellaufrufe – beides für `post.since` bzw. `thread.since` (Abfrage aus `web/src/chat.ts`).
//!
//! - [`typing`] (Abschnitts-Chat) bzw. [`typing_thread`] (Faden) tragen eine Persona ein und geben eine Wache
//!   ([`TypingGuard`]) zurück; fällt die Wache (Ende, Fehler oder Abbruch des Auftrags), ist die Persona wieder
//!   ausgetragen. Läuft dieselbe Persona zweimal am selben Ort, zählt jede Wache für sich (die Anzeige bleibt, bis
//!   die letzte fällt).
//! - [`record_failure`] bzw. [`record_thread_failure`] merken einen gescheiterten Aufruf (Persona, Meldung, Zeit);
//!   [`failures_now`] bzw. [`failures_thread`] liefern die der letzten [`FAILURE_WINDOW_SECS`] Sekunden, höchstens
//!   [`MAX_FAILURES`] je Ort (die neuesten). Der Browser zeigt jede nur einmal (er merkt sich `at`).
//!
//! Beides lebt nur im Arbeitsspeicher des Prozesses (keine Migration, kein Spiegel): Nach einem Neustart läuft auch
//! kein Auftrag mehr. Schlüssel ist das Werk und der feste Schlüssel des Abschnitts bzw. die id des Fadens.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use baruch_store::WorkId;
use serde::Serialize;

/// Fehler so lange zeigen (Sekunden).
pub const FAILURE_WINDOW_SECS: u64 = 600;
/// Höchstens so viele Fehler je Ort.
pub const MAX_FAILURES: usize = 5;

/// Ort einer Anzeige: Abschnitt (fester Schlüssel) oder Faden (id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Place {
    Section(i64),
    Thread(i64),
}

/// Eine laufende Wache.
#[derive(Debug, Clone)]
struct Entry {
    id: u64,
    persona: String,
    since: String,
}

/// Ein gescheiterter Aufruf.
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

/// Jetzt als ISO-Zeit (UTC, Millisekunden) – wie die Zeiten der Datenbank, als Text vergleichbar.
fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Handle mit „@“ (Aufträge nennen die Persona mal mit, mal ohne).
fn handle(persona: &str) -> String {
    let p = persona.trim();
    if p.starts_with('@') { p.to_owned() } else { format!("@{p}") }
}

/// Wache eines laufenden Auftrags; beim Fallenlassen wird die Persona ausgetragen.
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

/// Persona `persona` schreibt gerade im Abschnitt `key` des Werks `work` (bis die Wache fällt).
pub fn typing(work: WorkId, key: i64, persona: &str) -> TypingGuard {
    enter(work, Place::Section(key), persona)
}

/// Persona `persona` schreibt gerade im Faden `thread` des Werks `work` (bis die Wache fällt).
pub fn typing_thread(work: WorkId, thread: i64, persona: &str) -> TypingGuard {
    enter(work, Place::Thread(thread), persona)
}

/// Eine Persona, die gerade schreibt, und seit wann.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Typing {
    pub persona: String,
    pub since: String,
}

fn typing_at(work: WorkId, place: Place) -> Vec<Typing> {
    let live = live();
    let mut out: Vec<Typing> = Vec::new();
    for e in live.typing.get(&(work, place)).map(Vec::as_slice).unwrap_or_default() {
        // Dieselbe Persona zweimal (zwei Aufträge) nur einmal zeigen, mit dem früheren Beginn.
        if !out.iter().any(|t| t.persona == e.persona) {
            out.push(Typing { persona: e.persona.clone(), since: e.since.clone() });
        }
    }
    out
}

/// Wer schreibt gerade im Abschnitt `key`?
pub fn typing_now(work: WorkId, key: i64) -> Vec<Typing> {
    typing_at(work, Place::Section(key))
}

/// Wer schreibt gerade im Faden `thread`?
pub fn typing_in_thread(work: WorkId, thread: i64) -> Vec<Typing> {
    typing_at(work, Place::Thread(thread))
}

/// Ein gescheiterter Modellaufruf für die Anzeige.
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

/// Gescheiterten Aufruf im Abschnitt `key` merken.
pub fn record_failure(work: WorkId, key: i64, persona: &str, message: &str) {
    record(work, Place::Section(key), persona, message);
}

/// Gescheiterten Aufruf im Faden `thread` merken.
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

/// Gescheiterte Aufrufe der letzten Minuten im Abschnitt `key` (älteste zuerst).
pub fn failures_now(work: WorkId, key: i64) -> Vec<Failure> {
    failures_at(work, Place::Section(key))
}

/// Gescheiterte Aufrufe der letzten Minuten im Faden `thread` (älteste zuerst).
pub fn failures_thread(work: WorkId, thread: i64) -> Vec<Failure> {
    failures_at(work, Place::Thread(thread))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Eigene Werk-ids, die kein Integrationstest nutzt: die Anzeige ist prozessweit.
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
