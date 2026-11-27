// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `scripture_check` (Runde 10, Aufgabe „scripture“; Wortlaut seit 2026-11-06): Bibelstellen in
//! einem Text oder Abschnitt finden und Buch, Kapitel und Vers gegen die Verszählung der Schlachter 1951 prüfen,
//! mit gewählter SWORD-Bibel des Werks auch den Wortlaut. Nur lesend. Dieselbe Prüfung wie RPC
//! `scripture.check` bzw. `bible.check` ([`crate::scripture`], [`crate::bibles`]). Eintrag in der
//! Werkzeugliste und Aufruf setzt `tools.rs`.

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, resolve, work_schema};
use crate::AppState;

pub const NAME: &str = "scripture_check";

pub const DESCRIPTION: &str = "Check Bible references, read-only: finds references like \"1. Mose 3,16–19\", \
\"Ps 139,23-24\" or \"Joh 3,16; 4,1\" in text (if text is only a reference it is read strictly, otherwise searched \
in running text) or in the section key of the work work (current revision) and checks book, chapter and verse \
against the versification of the Schlachter 1951. Answer: refs (per input, span, normalized, book, status \
valid|unknown_book|no_such_chapter|no_such_verse|malformed, message with the maximum, wording, wording_code) and \
bible. Baruch checks the wording only with the chosen SWORD Bible of the work (Einstellungen → Quellen): then per \
reference wording_code match|differs|no_quote|missing_verse|versification_mismatch|invalid_ref|module_error with \
bible_text, quote and word diff (diff, diff_line), quote_span ([start, end) of the quote content in bytes in the \
checked text or null) and module_ref (reference read in the versification of the module, e.g. \"Mal 4,1–6 (KJV)\", \
only if it differs from the German one, otherwise null), module_spacing (Round 19c: the wording matches only without \
whitespace, e.g. with hyphenation remains in the module; wording_code stays match, wording \"Wortlaut stimmt \
(Leerraum im Modul)\") and module_note (hint if the module has strikingly many word fragments, otherwise null; also \
at the top of the answer); bible names module, versification, licence note and cipher (none|locked|unlocked). A \
module in another versification (KJV, Luther, Vulgate, Synodal and others) is read through a checked mapping; what \
cannot be mapped stays versification_mismatch with a reason. Without a Bible wording is the German word for \
\"unchecked\" (wording_code unchecked). A model never confirms a wording itself.";

pub fn schema() -> Value {
    json!({ "type": "object", "properties": {
        "text": { "type": "string", "description": "reference(s) or running text, e.g. a section (or key)" },
        "work": work_schema(),
        "key": { "type": "integer", "description": "stable key of a section: check its current revision \
                                                   (instead of text)" } } })
}

#[derive(Deserialize)]
struct CheckArgs {
    text: Option<String>,
    work: Option<WorkRef>,
    key: Option<i64>,
}

pub async fn call(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: CheckArgs = args(arguments)?;
    let work = match &a.work {
        Some(w) => resolve(&state.store, w).await?.0,
        None => state.default_work().await?,
    };
    match (a.key, a.text) {
        (Some(key), _) => Ok(crate::bibles::check_section(state, work, key).await?),
        (None, Some(text)) => Ok(crate::scripture::check_for_work(state, Some(work), &text).await?),
        (None, None) => Err(Failure::Args("text or key is missing".into())),
    }
}
