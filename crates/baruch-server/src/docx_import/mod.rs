// SPDX-License-Identifier: AGPL-3.0-or-later
//! Word-Import mit Vorschau (Runde 32; Schnittstelle R32-S1, Besitz docx_import).
//!
//! Eine .docx-Datei wird gelesen ([`baruch_docx::read`]), ihre Gliederung samt Bearbeitern und Warnungen gezeigt und
//! erst nach ausdrücklicher Bestätigung des Menschen als Werk übernommen ([`crate::import::import_generated`]):
//! Fassung 1 ist der Stand **vor** allen verfolgten Änderungen; die Änderungen je Bearbeiter werden offene
//! Vorschläge, Kommentare werden Beiträge – beides unter `@word:<name>` ([`word_author`]) mit Modell
//! [`WORD_MODEL`], nie als aktuelle Fassung. Kein Import über MCP.
//!
//! - RPC (global, ohne Werk): [`METHODS`] – siehe docs/RPC.md „Runde 32 – Word“.
//! - Seiten: `GET /werke/import/word`, Formulare `POST /werke/import/word/vorschau|uebernehmen|verwerfen`
//!   ([`pages`], [`writes`], hinter `guard::same_origin`).
//! - CLI: `baruch-server import DATEI.docx [--uebernehmen] [--title …]` ([`cli_import`]); ohne `--uebernehmen` nur
//!   Vorschau (nichts geschrieben, auch kein Entwurf).

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::Value;

use crate::AppState;
use crate::rpc::{REFUSED, RpcError};

/// Methoden dieses Moduls.
pub const METHODS: &[&str] = &["docx_import.preview", "docx_import.get", "docx_import.apply", "docx_import.discard"];

/// Größte Word-Datei, die der Import annimmt (wie `baruch_docx::Limits::DEFAULT.max_file_bytes`).
pub const MAX_DOCX_BYTES: u64 = 64 * 1024 * 1024;

/// Grenze der Anfrage an `POST /werke/import/word/vorschau` (Datei samt Formularfeldern).
pub const UPLOAD_LIMIT: usize = MAX_DOCX_BYTES as usize + 64 * 1024;

/// Modell-Feld der Vorschläge aus Word (`Store::add_proposal` verlangt ein Modell).
pub const WORD_MODEL: &str = "word";

/// Ordner der Entwürfe unter dem Datenordner (`<data>/docx-drafts/<id>/`).
pub const DRAFT_DIR: &str = "docx-drafts";

/// Meldung aller Wege dieses Moduls, solange sie Stub sind.
const NOT_YET: &str = "Der Word-Import ist noch nicht umgesetzt (Runde 32).";

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Endet der Dateiname (ohne Rücksicht auf Groß-/Kleinschreibung) auf `.docx` und hat davor einen Namen?
pub fn is_docx_name(file_name: &str) -> bool {
    const SUFFIX: &str = ".docx";
    let lower = file_name.to_lowercase();
    lower.len() > SUFFIX.len() && lower.ends_with(SUFFIX)
}

/// Größte Länge des Namensteils eines Handles `@word:<name>`.
const MAX_WORD_NAME_CHARS: usize = 40;

/// Persona-Handle eines Bearbeiters aus Word (Vertrag Runde 32 §0.5): `@word:<name>` mit dem Namen klein, ä→ae,
/// ö→oe, ü→ue, ß→ss, jede Folge anderer Zeichen außer a–z und 0–9 als ein `-`, ohne `-` am Rand, höchstens 40
/// Zeichen; bleibt nichts übrig, `@word:unbekannt`. Der Originalname steht in der Notiz der Fassung bzw. am Anfang
/// des Beitrags.
pub fn word_author(name: &str) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        match c {
            'a'..='z' | '0'..='9' => slug.push(c),
            'ä' => slug.push_str("ae"),
            'ö' => slug.push_str("oe"),
            'ü' => slug.push_str("ue"),
            'ß' => slug.push_str("ss"),
            _ if slug.ends_with('-') || slug.is_empty() => {}
            _ => slug.push('-'),
        }
    }
    // Nur ASCII: Zeichen und Bytes sind dasselbe.
    slug.truncate(MAX_WORD_NAME_CHARS);
    let slug = slug.trim_matches('-');
    if slug.is_empty() { "@word:unbekannt".to_owned() } else { format!("@word:{slug}") }
}

/// RPC-Aufruf einer Methode aus [`METHODS`].
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    // STUB: docx_import.preview/get/apply/discard fehlen – Entwurf, Übernehmen mit Vorschlägen und Beiträgen (R32
    // docx_import)
    let _ = (state, method, params);
    Err(RpcError::new(REFUSED, NOT_YET))
}

/// Angaben der Kommandozeile.
#[derive(Debug, Clone, Default)]
pub struct CliDocxImport {
    pub title: Option<String>,
    /// `--uebernehmen`: Werk samt Vorschlägen und Beiträgen anlegen; sonst nur Vorschau.
    pub apply: bool,
}

/// `baruch-server import DATEI.docx [--uebernehmen] [--title T]`: `Ok(text)` → Standardausgabe, Exit 0;
/// `Err(text)` → Fehlerausgabe, Exit 1.
pub async fn cli_import(state: &AppState, file: &std::path::Path, options: &CliDocxImport) -> Result<String, String> {
    // STUB: Import über die Kommandozeile fehlt – nichts wird gelesen oder geschrieben (R32 docx_import)
    let _ = (state, options);
    Err(format!("{}: {NOT_YET}", file.display()))
}

async fn not_yet() -> Response {
    // STUB: Seite, Vorschau, Übernehmen und Verwerfen fehlen (R32 docx_import)
    (StatusCode::NOT_IMPLEMENTED, NOT_YET).into_response()
}

/// Seiten (GET), gemischt in `lib.rs` bei den Seiten: `GET /werke/import/word`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/import/word", get(not_yet))
}

/// Formulare (POST), gemischt in `lib.rs` im writes-Router hinter `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/import/word/vorschau", post(not_yet).layer(axum::extract::DefaultBodyLimit::max(UPLOAD_LIMIT)))
        .route("/werke/import/word/uebernehmen", post(not_yet))
        .route("/werke/import/word/verwerfen", post(not_yet))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_names_end_in_docx_with_a_name_before() {
        assert!(is_docx_name("buch.docx"), ".docx");
        assert!(is_docx_name("BUCH.DOCX"), "Groß/klein egal: Dateien anderer Systeme heißen auch einmal so");
        assert!(is_docx_name("ordner/mein buch.docx"), "ein Pfad davor stört nicht");
        assert!(!is_docx_name(".docx"), "ohne Namen davor ist es keine Datei, die sich benennen ließe");
        assert!(!is_docx_name("buch.docx.typ"), "entscheidend ist die letzte Endung");
        assert!(!is_docx_name("buch.doc"), "das alte .doc-Format liest Baruch nicht");
        assert!(!is_docx_name("buch.pdf"), "PDF gehört zu pdf_import");
    }

    #[test]
    fn word_authors_become_handles_by_the_rule_of_the_contract() {
        assert_eq!(word_author("Erika Beispiel"), "@word:erika-beispiel", "klein, Leerzeichen wird Bindestrich");
        assert_eq!(word_author("Jörg Müßig-Äpfel"), "@word:joerg-muessig-aepfel", "Umlaute und ß umschrieben");
        assert_eq!(
            word_author("  Dr.  A. — B!  "),
            "@word:dr-a-b",
            "jede Folge anderer Zeichen ist ein Bindestrich, keiner am Rand"
        );
        assert_eq!(
            word_author("ÉMILE"),
            "@word:mile",
            "andere Buchstaben als a–z entfallen (am Rand ohne Bindestrich)"
        );
        assert_eq!(word_author(""), "@word:unbekannt", "ohne Namen");
        assert_eq!(word_author("…?!"), "@word:unbekannt", "bleibt nichts übrig, heißt der Bearbeiter unbekannt");
        let long = word_author(&"ab ".repeat(30));
        assert_eq!(long, format!("@word:{}", "ab-".repeat(13) + "a"), "höchstens 40 Zeichen im Namen");
        let cut = word_author(&format!("{} z", "a".repeat(39)));
        assert_eq!(cut, format!("@word:{}", "a".repeat(39)), "nach dem Kürzen steht kein Bindestrich am Rand");
    }

    #[test]
    fn only_the_four_methods_belong_to_this_module() {
        for method in METHODS {
            assert!(handles(method), "{method} gehört dem Word-Import");
        }
        assert!(!handles("work.docx_export"), "der Export gehört docx_export");
        assert!(!handles("source_import.preview"), "PDF/EPUB gehört pdf_import");
    }
}
