// SPDX-License-Identifier: AGPL-3.0-or-later
//! Queue: at most **one** running request (`Semaphore(1)`), at most [`MAX_WAITING`] waiting; whoever comes beyond
//! that gets [`LlmError::Busy`] at once. A dropped job (waiting or running) frees its place at once.

use std::future::Future;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;
use tokio::sync::Semaphore;

use crate::LlmError;

/// Largest number of waiting jobs (without the running job).
pub const MAX_WAITING: usize = 8;

/// State of the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueueStatus {
    /// Label of the running job (e.g. `"@lektor ask key 17"`, never text).
    pub running: Option<String>,
    /// Number of waiting jobs.
    pub waiting: usize,
}

/// See module header.
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

/// Counts a waiting job as long as it lives.
struct Waiting<'a>(&'a AtomicUsize);

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Holds the label of the running job as long as it runs.
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

    /// Run `work` as soon as nothing else is running. `label` appears in [`Queue::status`].
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
        // First clear the label, then free the place (otherwise it would clear the next one).
        drop(running);
        drop(permit);
        result
    }

    /// Running job and number of waiting jobs.
    pub fn status(&self) -> QueueStatus {
        QueueStatus {
            running: self.running.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            waiting: self.waiting.load(Ordering::SeqCst),
        }
    }
}
