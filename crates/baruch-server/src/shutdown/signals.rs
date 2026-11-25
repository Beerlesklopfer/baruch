// SPDX-License-Identifier: AGPL-3.0-or-later
//! Signale des Systems, auf die Baruch sauber herunterfährt (Runde 32, Aufgabe beenden).
//!
//! - überall: Strg+C;
//! - Unix: `SIGTERM` (`systemctl stop`, `kill`, Abmelden). **Nicht** `SIGHUP`: `nohup baruch-server …` stellt
//!   `SIGHUP` auf „ignorieren“, und ein eigener Empfänger höbe das wieder auf – Baruch endete dann doch mit dem
//!   Terminal. Der Starter `packaging/linux/baruch` übersetzt `SIGHUP` selbst in `SIGTERM` an den Server;
//! - Windows: Strg+Pause (`ctrl_break`), Schließen des Konsolenfensters (`ctrl_close`), Herunterfahren des Systems
//!   (`ctrl_shutdown`). Nach den letzten beiden beendet Windows den Prozess nach wenigen Sekunden von sich aus;
//!   darum gilt dort die kürzere Frist [`WINDOWS_CLOSE_GRACE`]. Der Windows-Zweig ist nur übersetzt geprüft
//!   (`cargo check --target x86_64-pc-windows-gnu` an genau dieser Datei), nie auf Windows gelaufen.
//!
//! Die Datei hängt nur von `tokio` ab (kein `crate::…`), damit sie sich für ein fremdes Ziel einzeln übersetzen
//! lässt.

use std::time::Duration;

/// Frist nach `ctrl_close` und `ctrl_shutdown`: Windows wartet auf den Empfänger rund fünf Sekunden und beendet
/// den Prozess dann hart; die Datenbank soll vorher geschlossen sein.
pub const WINDOWS_CLOSE_GRACE: Duration = Duration::from_secs(3);

/// Ein Signal des Systems: sein Name für die Meldung und – wenn das System nicht lange wartet – eine Obergrenze
/// für die Frist.
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

/// Strg+C; lässt sich der Empfänger nicht einrichten, kommt dieses Signal nie (statt sofort zu beenden).
#[cfg(not(unix))]
async fn ctrl_c() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        eprintln!("Signale: Strg+C lässt sich nicht abfangen: {e}");
        std::future::pending::<()>().await;
    }
}

/// Wartet auf das erste Signal, das Baruch beenden soll.
///
/// Die Empfänger sind **mit dem Aufruf** eingerichtet, nicht erst beim ersten Warten (Integration Runde 32): `main`
/// ruft dies auf, bevor „Baruch läuft“ auf der Standardausgabe steht. Sonst träfe ein `SIGTERM` unmittelbar nach
/// dieser Zeile (ein Starter oder `systemctl stop` gleich nach dem Start) noch die Vorgabe des Systems und
/// beendete den Prozess hart, ohne die Datenbank zu schließen. Braucht eine laufende Tokio-Laufzeit.
#[cfg(unix)]
pub fn system_signal() -> impl Future<Output = SystemSignal> + Send + 'static {
    use tokio::signal::unix::{SignalKind, signal};
    let terminate = signal(SignalKind::terminate());
    let interrupt = signal(SignalKind::interrupt());
    async move {
        // Lässt sich ein Empfänger nicht einrichten, kommt sein Signal nie; der andere gilt weiter.
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

/// Wartet auf das erste Signal, das Baruch beenden soll.
#[cfg(windows)]
pub async fn system_signal() -> SystemSignal {
    use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_shutdown};
    // Lässt sich ein Empfänger nicht einrichten, kommt sein Signal nie; die übrigen gelten weiter.
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

/// Wartet auf das erste Signal, das Baruch beenden soll.
#[cfg(not(any(unix, windows)))]
pub async fn system_signal() -> SystemSignal {
    ctrl_c().await;
    SystemSignal::patient("Strg+C")
}
