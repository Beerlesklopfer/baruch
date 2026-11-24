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

pub const DESCRIPTION: &str = "Bibelstellen prüfen, nur lesend: findet Stellenangaben wie „1. Kön 3,16–28“, \
„Ps 139,23-24“ oder „Joh 3,16; 4,1“ in text (ist text nur eine Angabe, wird sie streng gelesen, sonst im \
Fließtext gesucht) bzw. im Abschnitt key des Werks work (aktuelle Fassung) und prüft Buch, Kapitel und Vers \
gegen die Verszählung der Schlachter 1951. Antwort: refs (je input, span, normalized, book, status \
valid|unknown_book|no_such_chapter|no_such_verse|malformed, message mit dem Maximum, wording, wording_code) und \
bible. Den Wortlaut prüft Baruch nur mit der gewählten SWORD-Bibel des Werks (Einstellungen → Quellen): dann je \
Stelle wording_code match|differs|no_quote|missing_verse|versification_mismatch|invalid_ref|module_error mit \
bible_text, quote und Wort-Diff (diff, diff_line), quote_span ([Anfang, Ende) des Zitatinhalts in Bytes im \
geprüften Text oder null) und module_ref (gelesene Stelle in der Zählung des Moduls, z. B. „Mal 4,1–6 (KJV)“, nur \
wenn sie von der deutschen abweicht, sonst null), module_spacing (Runde 19c: der Wortlaut stimmt nur ohne \
Leerraum, etwa bei Silbentrennresten im Modul; wording_code bleibt match, wording „Wortlaut stimmt (Leerraum im \
Modul)“) und module_note (Hinweis, wenn das Modul auffällig viele Wortfragmente hat, sonst null; auch oben in der \
Antwort); bible nennt Modul, Versifikation, Lizenzvermerk und cipher \
(none|locked|unlocked). Ein Modul in anderer Verszählung (KJV, Luther, Vulgata, Synodal u. a.) liest Baruch über eine \
geprüfte Abbildung; was sich nicht abbilden lässt, bleibt versification_mismatch mit Grund. Ohne Bibel \
ist wording „ungeprüft“ (wording_code unchecked). Ein Modell bestätigt nie selbst einen Wortlaut.";

pub fn schema() -> Value {
    json!({ "type": "object", "properties": {
        "text": { "type": "string", "description": "Stellenangabe(n) oder Fließtext, z. B. ein Abschnitt (oder key)" },
        "work": work_schema(),
        "key": { "type": "integer", "description": "fester Schlüssel eines Abschnitts: dessen aktuelle Fassung prüfen \
                                                   (statt text)" } } })
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
        (None, None) => Err(Failure::Args("text oder key fehlt".into())),
    }
}
