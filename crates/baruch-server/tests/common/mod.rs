// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gemeinsame Hilfen der Server-Tests: echter Datenordner im Temp-Verzeichnis (SQLite-Datei,
//! Bibliothek, Git-Spiegel), Router über `tower::ServiceExt::oneshot`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Eigene kleine Typst-Datei: Präambel mit `#set document`, zwei Kapitel, Abschnitt, Unterabschnitt.
pub const GARDEN: &str = include_str!("../data/gartenbuch.typ");

/// Die Markierung „PDF entsteht“ (`baruch_server::pdf::claim`) gilt je Werk-id im ganzen Prozess. Seit alle
/// Server-Tests in einer Binärdatei laufen (`tests/main.rs`), teilen sich die Datenordner **aller** Module
/// dieselben Werk-ids: Wer ein PDF erzeugt oder die Markierung setzt, hält vorher diese Sperre.
pub static PDF_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Die Warteschlange der Modelle (`baruch_server::models`, eine Anfrage zur Zeit, höchstens acht Wartende) gilt
/// im ganzen Prozess. Früher lief jede Testdatei in einem eigenen Prozess; im Bündel (`tests/main.rs`) würden
/// sich die Dateien, die sie füllen bzw. ihren Zustand prüfen (`round10.rs`, `round10_models.rs`), gegenseitig
/// stören („Warteschlange voll“, fremder `running`). [`model_queue`] lässt darum nur Tests **eines** Moduls
/// zugleich hinein; Tests desselben Moduls laufen weiter gleichzeitig (wie vorher in ihrer Datei).
pub async fn model_queue(module: &'static str) -> ModuleTurn {
    loop {
        let changed = TURN_CHANGED.notified();
        {
            let mut turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if turn.0.is_none_or(|owner| owner == module) {
                turn.0 = Some(module);
                turn.1 += 1;
                return ModuleTurn;
            }
        }
        // `notified()` ist schon vor dem Freigeben angelegt: kein Wecken geht verloren.
        changed.await;
    }
}

static TURN: std::sync::Mutex<(Option<&'static str>, usize)> = std::sync::Mutex::new((None, 0));
static TURN_CHANGED: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Anteil eines Tests an der Reihe seines Moduls ([`model_queue`]); gibt sie beim Verwerfen frei.
pub struct ModuleTurn;

impl Drop for ModuleTurn {
    fn drop(&mut self) {
        let mut turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        turn.1 -= 1;
        if turn.1 == 0 {
            turn.0 = None;
            TURN_CHANGED.notify_waiters();
        }
    }
}

pub struct Server {
    pub data: tempfile::TempDir,
    pub state: Arc<baruch_server::AppState>,
    pub app: Router,
}

/// Neuer Datenordner mit Beispielwerk und Beispielbibliothek.
pub async fn open() -> Server {
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open(data.path()).await.expect("Datenordner lässt sich nicht öffnen");
    let state = Arc::new(state);
    Server { app: baruch_server::router(state.clone()), state, data }
}

impl Server {
    pub fn path(&self) -> &Path {
        self.data.path()
    }

    /// Der einzige Spiegelordner unter `<data>/works/`.
    pub fn mirror_dir(&self) -> PathBuf {
        let mut dirs: Vec<PathBuf> =
            std::fs::read_dir(self.path().join("works")).unwrap().map(|e| e.unwrap().path()).collect();
        assert_eq!(dirs.len(), 1, "genau ein Werk erwartet: {dirs:?}");
        dirs.pop().unwrap()
    }

    /// Spiegelordner eines bestimmten Werks: `<data>/works/<slug>`.
    pub fn mirror_dir_of(&self, slug: &str) -> PathBuf {
        self.path().join("works").join(slug)
    }

    /// Typst-Quelltext über `baruch_server::import::import_typst` als neues Werk anlegen.
    pub async fn import(&self, source: &str, file_name: &str) -> baruch_server::import::ImportReport {
        baruch_server::import::import_typst(&self.state, source, file_name).await.expect("Import scheitert")
    }

    /// Die eigene Testdatei `tests/data/gartenbuch.typ` importieren.
    pub async fn import_garden(&self) -> baruch_server::import::ImportReport {
        self.import(GARDEN, "gartenbuch.typ").await
    }

    pub async fn rpc(&self, method: &str, params: Value) -> Value {
        rpc(self.app.clone(), method, params).await
    }

    /// Beliebige Anfrage; Antwort als Status, Kopfzeilen und Text.
    pub async fn send(&self, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, String) {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
    }

    pub async fn get(&self, uri: &str) -> (StatusCode, String) {
        let response = self.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    pub async fn post_form(&self, uri: &str, form: &str) -> (StatusCode, Option<String>) {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_owned()))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let location = response.headers().get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned());
        (response.status(), location)
    }
}

/// JSON-RPC-Aufruf an einen Router (auch aus `tokio::spawn`, daher ohne `&Server`).
pub async fn rpc(app: Router, method: &str, params: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Anzahl der Revisionen mit Status `current` in einer `node.get`-Antwort.
pub fn currents(node: &Value) -> usize {
    node["revisions"].as_array().unwrap().iter().filter(|r| r["status"] == "current").count()
}

/// `git` im Spiegelordner; `None`, wenn kein git-Programm da ist.
pub fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    Some(String::from_utf8(output.stdout).unwrap())
}
