// SPDX-License-Identifier: AGPL-3.0-or-later
//! Warteschlange: höchstens **eine** laufende Anfrage (`Semaphore(1)`), höchstens [`MAX_WAITING`]
//! Wartende; wer darüber hinaus kommt, bekommt sofort [`LlmError::Busy`]. Ein fallen gelassener
//! Auftrag (wartend oder laufend) gibt seinen Platz sofort frei.

use std::future::Future;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;
use tokio::sync::Semaphore;

use crate::LlmError;

/// Größte Zahl Wartender (ohne den laufenden Auftrag).
pub const MAX_WAITING: usize = 8;

/// Zustand der Warteschlange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueStatus {
    /// Bezeichnung des laufenden Auftrags (z. B. `"@lektor ask key 17"`, nie Text).
    pub running: Option<String>,
    /// Zahl der Wartenden.
    pub waiting: usize,
}

/// Siehe Modulkopf.
#[derive(Debug)]
pub struct Queue {
    permits: Semaphore,
    max_waiting: usize,
    waiting: AtomicUsize,
    running: Mutex<Option<String>>,
}

impl Default for Queue {
    fn default() -> Self {
        Queue::new(MAX_WAITING)
    }
}

/// Zählt einen Wartenden, solange er lebt.
struct Waiting<'a>(&'a AtomicUsize);

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Hält die Bezeichnung des laufenden Auftrags, solange er läuft.
struct Running<'a>(&'a Mutex<Option<String>>);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

impl Queue {
    pub fn new(max_waiting: usize) -> Queue {
        Queue { permits: Semaphore::new(1), max_waiting, waiting: AtomicUsize::new(0), running: Mutex::new(None) }
    }

    /// `work` ausführen, sobald nichts anderes läuft. `label` erscheint in [`Queue::status`].
    pub async fn run<T>(&self, label: &str, work: impl Future<Output = Result<T, LlmError>>) -> Result<T, LlmError> {
        let permit = match self.permits.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                let now = self.waiting.fetch_add(1, Ordering::SeqCst) + 1;
                let waiting = Waiting(&self.waiting);
                if now > self.max_waiting {
                    return Err(LlmError::Busy(format!(
                        "Warteschlange voll ({} Anfragen warten schon); bitte später erneut versuchen.",
                        self.max_waiting
                    )));
                }
                let permit =
                    self.permits.acquire().await.map_err(|_| LlmError::Failed("Warteschlange geschlossen.".into()))?;
                drop(waiting);
                permit
            }
        };
        *self.running.lock().unwrap_or_else(|e| e.into_inner()) = Some(label.to_owned());
        let running = Running(&self.running);
        let result = work.await;
        // Erst die Bezeichnung löschen, dann den Platz freigeben (sonst löschte sie den Nächsten).
        drop(running);
        drop(permit);
        result
    }

    /// Laufender Auftrag und Zahl der Wartenden.
    pub fn status(&self) -> QueueStatus {
        QueueStatus {
            running: self.running.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            waiting: self.waiting.load(Ordering::SeqCst),
        }
    }
}
