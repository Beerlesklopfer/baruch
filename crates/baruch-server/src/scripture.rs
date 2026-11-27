// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stellenprüfung (Runde 10, Aufgabe „scripture“; Wortlaut seit 2026-11-06): RPC `scripture.check`, MCP
//! `scripture_check` ([`crate::mcp::scripture`]) und die Haken für die Personas @schrift und @belege.
//!
//! Geprüft werden Buch, Kapitel und Vers gegen die Verszählung der Schlachter 1951
//! ([`baruch_scripture::SOURCE`]). Den **Wortlaut** prüft Baruch nur mit einer gewählten SWORD-Bibel des
//! Werks ([`crate::bibles`]): je Stelle Text der Bibel und Vergleich mit dem Zitat beim Verweis. Ohne Bibel ist
//! jedes Zitat „ungeprüft“, und so steht es in jeder Antwort. Ein Modell bestätigt nie selbst einen Wortlaut.
//!
//! - [`refs`]: Ist der ganze Text eine Stellenangabe („Joh 3,16; Hesek 3,1“), wird sie streng gelesen
//!   (auch unbekannte Bücher und Fehler werden gemeldet); sonst wird der Text als Fließtext nach Stellen
//!   bekannter Bücher durchsucht.
//! - [`facts_for`] / [`facts_for_work`]: Faktenblock für den System-Prompt von @schrift und @belege.
//! - [`mark_answer`] / [`mark_answer_for_work`]: Hinweis zum Wortlaut an Antworten mit Stellen; dazu werden
//!   Stellen, deren Ziffern das Modell auseinandergerissen hat („Mk 1 0,42–45“), wieder zusammengesetzt.
//!
//! Alles ist nur lesend: kein Schreiben, keine Sperre, kein Commit.

use baruch_scripture::{Found, RefStatus};
use baruch_store::WorkId;
use serde_json::{Value, json};

use crate::AppState;
use crate::bibles::{Choice, WordingResult, check_wording};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Höchstlänge des geprüften Texts (Zeichen).
pub const MAX_TEXT_CHARS: usize = 200_000;

/// Höchstzahl gemeldeter Stellen je Prüfung.
pub const MAX_REFS: usize = 200;

/// Höchstzahl der Stellen im Faktenblock.
const MAX_FACTS: usize = 40;

/// Höchstlänge des Bibeltexts einer abweichenden Stelle im Faktenblock (Zeichen).
const MAX_FACT_TEXT: usize = 400;

/// Wortlaut einer Stelle ohne Bibel.
pub const UNCHECKED: &str = "ungeprüft";

/// Schlusssatz des Faktenblocks ohne Bibel.
pub const FACTS_END: &str =
    "Wortlaut: Baruch hat keine lokale Bibel; jedes Zitat bleibt ungeprüft. Bestätige keinen Wortlaut.";

/// Schlusssatz des Faktenblocks mit Bibel.
pub const FACTS_END_BIBLE: &str = "Wortlaut: Die Ergebnisse oben hat Baruch mit dem genannten SWORD-Modul geprüft, \
nicht du. Gib sie nur wieder; bestätige nie selbst einen Wortlaut, und schreibe keinen Bibeltext ab außer dem \
geprüften Vers einer Abweichung.";

/// Zeile, die [`mark_answer`] an Antworten von @schrift und @belege hängt (ohne Bibel).
pub const MARK_SCRIPTURE: &str = "Wortlaut: ungeprüft – keine lokale Bibel in Baruch (geprüft nur Buch/Kapitel/Vers).";

/// Zeile, die [`mark_answer`] an Antworten anderer Personas mit Stellen hängt (Werk ohne lesbare Bibel).
pub const MARK_OTHERS: &str = "Hinweis: Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft.";

/// Runde 19c: Schluss der Zeile für andere Personas, wenn das Werk eine lesbare Bibel hat; davor steht
/// „[`MARK_BIBLE`] mit dem Modul X“ (beginnt mit [`MARK_BIBLE`], gilt also auch als Schlusshinweis von Baruch).
pub const MARK_OTHERS_BIBLE_END: &str =
    "die Persona hat ihn nicht selbst bestätigt (Ergebnis je Stelle: Zitate prüfen oder @belege).";

/// Zeile für andere Personas mit Bibel: „Wortlaut geprüft von Baruch (nicht vom Modell) mit dem Modul X; die Persona
/// hat ihn nicht selbst bestätigt (…).“
pub fn mark_others_bible(module: &str) -> String {
    format!("{MARK_BIBLE} mit dem Modul {module}; {MARK_OTHERS_BIBLE_END}")
}

/// Anfang der Zeile mit Bibel (danach Modul, Versifikation, Lizenz und die Ergebnisse je Stelle).
pub const MARK_BIBLE: &str = "Wortlaut geprüft von Baruch (nicht vom Modell)";

const METHODS: &[&str] = &["scripture.check"];

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// RPC `scripture.check {text, work?, module?}` → `{refs, truncated, source, bible}` (`docs/RPC.md`,
/// „Bibelstellen“). Nur lesend.
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "scripture.check" => {
            let text = params
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::new(INVALID_PARAMS, "text fehlt (Stellenangabe oder Fließtext)"))?;
            let work = crate::rpc::work_param(state, &params).await?;
            let module = params.get("module").and_then(Value::as_str).map(str::trim).filter(|m| !m.is_empty());
            let choice = match module {
                Some(name) => crate::bibles::by_name(state, name).await?,
                None => crate::bibles::of_work(state, work).await?,
            };
            check_with(&choice, text).map_err(|message| RpcError::new(INVALID_PARAMS, message))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Prüfung eines Texts als JSON ohne Bibel (Wortlaut „ungeprüft“); `Err` bei zu langem Text.
pub fn check(text: &str) -> Result<Value, String> {
    check_with(&Choice::None, text)
}

/// Prüfung mit der Bibel des Werks `work` (ohne Werk: keine Bibel).
pub async fn check_for_work(state: &AppState, work: Option<WorkId>, text: &str) -> Result<Value, RpcError> {
    let choice = match work {
        Some(w) => crate::bibles::of_work(state, w).await?,
        None => Choice::None,
    };
    check_with(&choice, text).map_err(|message| RpcError::new(INVALID_PARAMS, message))
}

/// Prüfung eines Texts mit `choice` als JSON.
pub fn check_with(choice: &Choice, text: &str) -> Result<Value, String> {
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("text: höchstens {MAX_TEXT_CHARS} Zeichen"));
    }
    let found = refs(text);
    let truncated = found.len() > MAX_REFS;
    let found: Vec<Found> = found.into_iter().take(MAX_REFS).collect();
    let wording = check_wording(choice.bible(), text, &found);
    // Runde 19c: Hinweis auf auffällig viele Wortfragmente im Modul (je Antwort und je Stelle, sonst null).
    let note = crate::bibles::choice_note(choice);
    let list: Vec<Value> = found.iter().zip(&wording).map(|(f, w)| entry(f, w, note.as_deref())).collect();
    Ok(json!({ "refs": list, "truncated": truncated, "source": baruch_scripture::SOURCE, "bible": choice.json(),
               "module_note": note }))
}

fn entry(found: &Found, wording: &WordingResult, module_note: Option<&str>) -> Value {
    let book = found.passage.as_ref().map(|p| p.book);
    let short = match wording.code {
        "unchecked" => UNCHECKED.to_owned(),
        _ => wording.label.clone(),
    };
    json!({
        "input": found.input,
        "span": [found.span.0, found.span.1],
        "normalized": found.normalized,
        "book": book.map(|b| b.name),
        "book_id": book.map(|b| b.id),
        "status": found.status.code(),
        "message": found.message(),
        "wording": short,
        "wording_code": wording.code,
        "quote": wording.quote,
        "bible_text": wording.bible_text,
        "partial": wording.partial,
        "diff": wording.diff,
        "diff_line": (!wording.diff.is_empty()).then(|| baruch_scripture::wording::diff_line(&wording.diff)),
        // R17c (additiv): Byte-Bereich des Zitatinhalts im geprüften Text und die gelesene Stelle in der Zählung
        // des Moduls, wenn sie von der deutschen abweicht.
        "quote_span": wording.quote_span.map(|(s, e)| [s, e]),
        "module_ref": wording.module_ref,
        // Runde 19c: Wortlaut stimmt nur ohne Leerraum (Code bleibt "match"), Hinweis zum Modul.
        "module_spacing": wording.module_spacing,
        "module_note": module_note,
    })
}

/// Stellen in `text`: streng gelesen, wenn der ganze Text eine Stellenangabe ist, sonst im Fließtext gesucht.
pub fn refs(text: &str) -> Vec<Found> {
    let strict = baruch_scripture::parse(text);
    let is_reference = !strict.is_empty()
        && !text.contains('\n')
        && strict.iter().all(|f| !matches!(f.status, RefStatus::Malformed(_)));
    if is_reference { strict } else { baruch_scripture::scan(text) }
}

/// @schrift oder @belege (mit oder ohne @, Groß-/Kleinschreibung egal)?
fn checks_scripture(handle: &str) -> bool {
    let name = handle.trim().trim_start_matches('@');
    name.eq_ignore_ascii_case("schrift") || name.eq_ignore_ascii_case("belege")
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max { text.to_owned() } else { text.chars().take(max).collect::<String>() + " …" }
}

/// Faktenblock der Stellenprüfung für den System-Prompt von @schrift und @belege (ohne Bibel); sonst `None`.
pub fn facts_for(handle: &str, section: &str) -> Option<String> {
    facts_with(&Choice::None, handle, section)
}

/// Faktenblock mit der Bibel des Werks (Wortlaut je Stelle geprüft, wenn eine gewählt ist).
pub async fn facts_for_work(state: &AppState, work: WorkId, handle: &str, section: &str) -> Option<String> {
    if !checks_scripture(handle) {
        return None;
    }
    let choice = crate::bibles::of_work(state, work).await.unwrap_or(Choice::None);
    facts_with(&choice, handle, section)
}

/// Faktenblock mit `choice` (öffentlich für Tests mit einer Test-Bibel).
pub fn facts_with(choice: &Choice, handle: &str, section: &str) -> Option<String> {
    if !checks_scripture(handle) {
        return None;
    }
    let bible = choice.bible();
    let head = match (bible, choice) {
        (Some(b), _) => format!(
            "Stellenprüfung (Baruch, Verszählung Schlachter 1951; Wortlaut aus dem SWORD-Modul {} „{}“, \
             Versifikation {}, Lizenz laut conf: {}):",
            b.info.name, b.info.description, b.v11n.name, b.info.notice
        ),
        (None, Choice::None) => "Stellenprüfung (Baruch, Verszählung Schlachter 1951 – kein Wortlaut):".to_owned(),
        (None, other) => format!(
            "Stellenprüfung (Baruch, Verszählung Schlachter 1951 – kein Wortlaut; die gewählte Bibel ist nicht \
             lesbar: {}):",
            other.json()["problem"].as_str().unwrap_or("unbekannt")
        ),
    };
    let found = baruch_scripture::scan(section);
    let wording = check_wording(bible, section, &found);
    let mut lines = vec![head];
    // Runde 19c: Modul mit auffällig vielen Wortfragmenten (Trennreste) – der Hinweis steht vor den Stellen.
    if let Some(note) = choice.bible().and_then(crate::bibles::module_note) {
        lines.push(format!("Hinweis: {note}"));
    }
    if found.is_empty() {
        lines.push("- keine Stellenangabe gefunden".to_owned());
    }
    // Dieselbe Stelle mit demselben Ergebnis nur einmal; mit anderem Zitat (anderes Ergebnis) noch einmal.
    let mut seen: Vec<(String, String, Option<String>)> = Vec::new();
    for (f, w) in found.iter().zip(&wording) {
        let label = f.normalized.clone().unwrap_or_else(|| f.input.clone());
        let id = (label.clone(), w.code.to_owned(), w.quote.clone());
        if seen.contains(&id) {
            continue;
        }
        if seen.len() == MAX_FACTS {
            lines.push(format!("- … weitere Stellen nicht aufgeführt (höchstens {MAX_FACTS})"));
            break;
        }
        let result = if f.status == RefStatus::Valid { "vorhanden".to_owned() } else { f.message() };
        let said = match w.code {
            "unchecked" => format!("Wortlaut {UNCHECKED}"),
            "differs" => format!(
                "{} – Zitat „{}“, Bibel „{}“, Unterschiede: {}",
                w.label,
                shorten(w.quote.as_deref().unwrap_or_default(), MAX_FACT_TEXT),
                shorten(w.bible_text.as_deref().unwrap_or_default(), MAX_FACT_TEXT),
                baruch_scripture::wording::diff_line(&w.diff)
            ),
            _ => w.label.clone(),
        };
        lines.push(format!("- „{}“ → {label}: {result}; {said}", f.input));
        seen.push(id);
    }
    lines.push(if bible.is_some() { FACTS_END_BIBLE } else { FACTS_END }.to_owned());
    Some(lines.join("\n"))
}

/// Hängt an eine Antwort den Hinweis zum Wortlaut (ohne Bibel): bei @schrift/@belege, wenn Abschnitt oder
/// Antwort eine Stelle nennen, [`MARK_SCRIPTURE`]; bei anderen Personas, wenn die Antwort eine Stelle nennt,
/// [`MARK_OTHERS`] (mit Bibel [`mark_others_bible`], siehe [`mark_with`]); sonst bleibt die Antwort unverändert. Steht die Zeile schon da, kommt sie nicht noch
/// einmal dazu. Auseinandergerissene Ziffern einer Stelle des Abschnitts werden zusammengesetzt.
pub fn mark_answer(handle: &str, section: &str, body: String) -> String {
    mark_with(&Choice::None, handle, section, body)
}

/// Wie [`mark_answer`], mit der Bibel des Werks: Bei @schrift/@belege steht dann statt „ungeprüft“ die
/// Zeile [`MARK_BIBLE`] mit Modul, Versifikation, Lizenz und dem Ergebnis je Stelle (von Baruch, nicht vom
/// Modell). Runde 19c: Bei anderen Personas (etwa dem Lektor) steht mit Bibel statt [`MARK_OTHERS`] die Zeile
/// [`mark_others_bible`]; ohne Bibel bleibt es bei [`MARK_OTHERS`].
pub async fn mark_answer_for_work(state: &AppState, work: WorkId, handle: &str, section: &str, body: String) -> String {
    // Die Bibel nur öffnen, wenn eine Zeile überhaupt in Frage kommt (Stelle in Abschnitt oder Antwort).
    let wanted = if checks_scripture(handle) {
        !baruch_scripture::scan(section).is_empty() || !baruch_scripture::scan(&body).is_empty()
    } else {
        !baruch_scripture::scan(&body).is_empty()
    };
    let choice = if wanted { crate::bibles::of_work(state, work).await.unwrap_or(Choice::None) } else { Choice::None };
    mark_with(&choice, handle, section, body)
}

/// Wie [`mark_answer`] mit `choice` (öffentlich für Tests mit einer Test-Bibel).
pub fn mark_with(choice: &Choice, handle: &str, section: &str, body: String) -> String {
    let in_section = baruch_scripture::scan(section);
    let mut labels: Vec<String> = Vec::new();
    for f in &in_section {
        for label in [f.normalized.clone(), Some(f.input.clone())].into_iter().flatten() {
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
    }
    let body = baruch_scripture::wording::rejoin_split_refs(&body, &labels);
    let line = if checks_scripture(handle) {
        let any = !in_section.is_empty() || !baruch_scripture::scan(&body).is_empty();
        match choice.bible() {
            Some(bible) if any => Some(bible_line(bible, section, &in_section)),
            _ => any.then(|| MARK_SCRIPTURE.to_owned()),
        }
    } else if baruch_scripture::scan(&body).is_empty() {
        None
    } else {
        Some(match choice.bible() {
            Some(bible) => mark_others_bible(&bible.info.name),
            None => MARK_OTHERS.to_owned(),
        })
    };
    match line {
        Some(line) if !body.contains(&line) => format!("{}\n\n{line}", body.trim_end()),
        _ => body,
    }
}

/// „Wortlaut geprüft von Baruch (nicht vom Modell), Modul GerSch, Versifikation German, Lizenz …: Joh 3,16
/// stimmt; Ps 23,1 kein Zitat gefunden.“
fn bible_line(bible: &baruch_sword::Bible, section: &str, found: &[Found]) -> String {
    let wording = check_wording(Some(bible), section, found);
    let mut parts: Vec<String> = Vec::new();
    for (f, w) in found.iter().zip(&wording) {
        let label = f.normalized.clone().unwrap_or_else(|| f.input.clone());
        let part = format!("{label} {}", w.label.trim_start_matches("Wortlaut "));
        if !parts.contains(&part) && parts.len() < MAX_FACTS {
            parts.push(part);
        }
    }
    let list = if parts.is_empty() { "keine Stelle im Abschnitt".to_owned() } else { parts.join("; ") };
    let line = format!(
        "{MARK_BIBLE}, Modul {}, Versifikation {}, Lizenz laut conf: {}: {list}.",
        bible.info.name, bible.v11n.name, bible.info.notice
    );
    // Runde 19c: Hinweis auf Wortfragmente im Modul in derselben Zeile (bleibt so ein Schlusshinweis).
    match crate::bibles::module_note(bible) {
        Some(note) => format!("{line} {note}"),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_schrift_and_belege_check_scripture() {
        for handle in ["@schrift", "schrift", "@Belege", " belege "] {
            assert!(checks_scripture(handle), "{handle} prüft Stellen");
        }
        for handle in ["@lektor", "@schriftführer", "", "@"] {
            assert!(!checks_scripture(handle), "{handle} prüft keine Stellen");
        }
    }

    #[test]
    fn a_pure_reference_is_read_strictly_and_prose_is_scanned() {
        let strict = refs("Hesek 3,1");
        assert_eq!(strict.len(), 1, "eine Angabe: {strict:?}");
        assert_eq!(strict[0].status, RefStatus::UnknownBook, "unbekanntes Buch wird gemeldet");
        let prose = refs("Wie der Psalm sagt (Ps 23,1), so auch Hesek 3,1 – ein Buch, das es so nicht gibt.");
        assert_eq!(prose.len(), 1, "im Fließtext nur bekannte Bücher: {prose:?}");
        assert_eq!(prose[0].normalized.as_deref(), Some("Ps 23,1"), "Psalm 23");
    }

    #[test]
    fn split_digits_of_a_reference_in_the_answer_are_rejoined() {
        // Befund des Nutzers (2026-11-06): In der Prüfliste von @schrift stand „Mk 1 0,42–45“. Der Parser
        // liest „Mk 10,42–45“ richtig; die Ziffern hatte das Modell getrennt.
        let section = "Wer groß sein will (Mk 10,42–45), dient.";
        let answer = "- Mk 1 0,42–45: Stelle gibt es; Wortlaut ungeprüft.".to_owned();
        let marked = mark_answer("@schrift", section, answer);
        assert!(marked.starts_with("- Mk 10,42–45: Stelle gibt es"), "Ziffern zusammengesetzt: {marked}");
        assert!(!marked.contains("Mk 1 0"), "keine getrennten Ziffern mehr: {marked}");
    }
}
