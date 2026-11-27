// SPDX-License-Identifier: AGPL-3.0-or-later
//! Signals of the system on which Baruch shuts down cleanly (Round 32, task beenden).
//!
//! - everywhere: Ctrl+C;
//! - Unix: `SIGTERM` (`systemctl stop`, `kill`, logging out). **Not** `SIGHUP`: `nohup baruch-server …` sets
//!   `SIGHUP` to "ignore", and a receiver of our own would lift that again – Baruch would then end with the
//!   terminal after all. The launcher `packaging/linux/baruch` translates `SIGHUP` itself into `SIGTERM` to the
//!   server;
//! - Windows: Ctrl+Break (`ctrl_break`), closing the console window (`ctrl_close`), shutting down the system
//!   (`ctrl_shutdown`). After the last two Windows ends the process by itself after a few seconds;
//!   therefore the shorter grace period [`WINDOWS_CLOSE_GRACE`] applies there. The Windows branch is only checked
//!   by compiling (`cargo check --target x86_64-pc-windows-gnu` on exactly this file), never run on Windows.
//!
//! The file depends only on `tokio` (no `crate::…`), so that it can be compiled on its own for a foreign
//! target.

use std::time::Duration;

/// Grace period after `ctrl_close` and `ctrl_shutdown`: Windows waits for the receiver about five seconds and
/// then ends the process hard; the database is to be closed before that.
pub const WINDOWS_CLOSE_GRACE: Duration = Duration::from_secs(3);

/// A signal of the system: its name for the message and – if the system does not wait long – an upper limit
/// for the grace period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemSignal {
    pub name: &'static str,
    pub grace_limit: Option<Duration>,
}

impl SystemSignal {
    const fn patient(name: &'static str) -> Self {
        Self { name, grace_limit: None }
    }
}

/// Ctrl+C; if the receiver cannot be set up, this signal never comes (instead of quitting at once).
#[cfg(not(unix))]
async fn ctrl_c() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        eprintln!("Signale: Strg+C lässt sich nicht abfangen: {e}");
        std::future::pending::<()>().await;
    }
}

/// Waits for the first signal that is to end Baruch.
///
/// The receivers are set up **with the call**, not only on the first waiting (integration Round 32): `main`
/// calls this before „Baruch läuft“ stands on the standard output. Otherwise a `SIGTERM` immediately after
/// this line (a launcher or `systemctl stop` right after the start) would still hit the default of the system and
/// end the process hard, without closing the database. Needs a running Tokio runtime.
#[cfg(unix)]
pub fn system_signal() -> impl Future<Output = SystemSignal> + Send + 'static {
    use tokio::signal::unix::{SignalKind, signal};
    let terminate = signal(SignalKind::terminate());
    let interrupt = signal(SignalKind::interrupt());
    async move {
        // If a receiver cannot be set up, its signal never comes; the other one still applies.
        async fn received(receiver: std::io::Result<tokio::signal::unix::Signal>, name: &str) {
            match receiver {
                Ok(mut receiver) => {
                    receiver.recv().await;
                }
                Err(e) => {
                    eprintln!("Signale: {name} lässt sich nicht abfangen: {e}");
                    std::future::pending::<()>().await;
                }
            }
        }
        tokio::select! {
            () = received(interrupt, "Strg+C") => SystemSignal::patient("Strg+C"),
            () = received(terminate, "SIGTERM") => SystemSignal::patient("SIGTERM"),
        }
    }
}

/// Waits for the first signal that is to end Baruch.
#[cfg(windows)]
pub async fn system_signal() -> SystemSignal {
    use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_shutdown};
    // If a receiver cannot be set up, its signal never comes; the others still apply.
    async fn or_never<T>(receiver: std::io::Result<T>, name: &str) -> T {
        match receiver {
            Ok(receiver) => receiver,
            Err(e) => {
                eprintln!("Signale: {name} lässt sich nicht abfangen: {e}");
                std::future::pending().await
            }
        }
    }
    let pause = async { or_never(ctrl_break(), "Strg+Pause").await.recv().await };
    let close = async { or_never(ctrl_close(), "Schließen des Fensters").await.recv().await };
    let shutdown = async { or_never(ctrl_shutdown(), "Herunterfahren").await.recv().await };
    let hurried = |name: &'static str| SystemSignal { name, grace_limit: Some(WINDOWS_CLOSE_GRACE) };
    tokio::select! {
        () = ctrl_c() => SystemSignal::patient("Strg+C"),
        _ = pause => SystemSignal::patient("Strg+Pause"),
        _ = close => hurried("Schließen des Fensters"),
        _ = shutdown => hurried("Herunterfahren des Systems"),
    }
}

/// Waits for the first signal that is to end Baruch.
#[cfg(not(any(unix, windows)))]
pub async fn system_signal() -> SystemSignal {
    ctrl_c().await;
    SystemSignal::patient("Strg+C")
}
