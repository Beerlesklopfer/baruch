// SPDX-License-Identifier: AGPL-3.0-or-later
//! Passage check (Round 10, task "scripture"; wording since 2026-11-06): RPC `scripture.check`, MCP
//! `scripture_check` ([`crate::mcp::scripture`]) and the hooks for the personas @schrift and @belege.
//!
//! Book, chapter and verse are checked against the versification of the Schlachter 1951
//! ([`baruch_scripture::SOURCE`]). The **wording** is checked by Baruch only with a chosen SWORD Bible of the
//! work ([`crate::bibles`]): per passage the text of the Bible and the comparison with the quotation at the
//! reference. Without a Bible every quotation is „ungeprüft“, and that is what every answer says. A model never
//! confirms a wording itself.
//!
//! - [`refs`]: if the whole text is a reference („Joh 3,16; Hesek 3,1“), it is read strictly
//!   (unknown books and errors are reported as well); otherwise the text is searched as running text for passages
//!   of known books.
//! - [`facts_for`] / [`facts_for_work`]: facts block for the system prompt of @schrift and @belege.
//! - [`mark_answer`] / [`mark_answer_for_work`]: note on the wording at answers with passages; in addition,
//!   passages whose digits the model has torn apart („Mk 1 0,42–45“) are put together again.
//!
//! Everything is read only: no writing, no lock, no commit.
//!
//! **Work language (Round 39b):** the words Baruch writes for read-back – the closing lines ([`mark_scripture`],
//! [`mark_others`], [`mark_bible`]), the wording labels in them and in the `wording` field of `scripture.check` /
//! `scripture_check` ([`wording_label_in`]) and the facts block – follow the language of the work through the book
//! catalog ([`baruch_core::book_text`]). Every recognition (closing lines in the chat, [`crate::chat_render`])
//! accepts both languages. German works stay exactly as before; the constants below are the German values.

use baruch_core::WorkLanguage;
use baruch_core::book_text::book_t;
use baruch_scripture::{Found, RefStatus};
use baruch_store::WorkId;
use serde_json::{Value, json};

use crate::AppState;
use crate::bibles::{Choice, WordingResult, check_wording};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Maximum length of the checked text (characters).
pub const MAX_TEXT_CHARS: usize = 200_000;

/// Maximum number of reported passages per check.
pub const MAX_REFS: usize = 200;

/// Maximum number of passages in the facts block.
const MAX_FACTS: usize = 40;

/// Maximum length of the Bible text of a deviating passage in the facts block (characters).
const MAX_FACT_TEXT: usize = 400;

/// Wording of a passage without a Bible.
pub const UNCHECKED: &str = "ungeprüft";

/// Closing sentence of the facts block without a Bible.
pub const FACTS_END: &str =
    "Wortlaut: Baruch hat keine lokale Bibel; jedes Zitat bleibt ungeprüft. Bestätige keinen Wortlaut.";

/// Closing sentence of the facts block with a Bible.
pub const FACTS_END_BIBLE: &str = "Wortlaut: Die Ergebnisse oben hat Baruch mit dem genannten SWORD-Modul geprüft, \
nicht du. Gib sie nur wieder; bestätige nie selbst einen Wortlaut, und schreibe keinen Bibeltext ab außer dem \
geprüften Vers einer Abweichung.";

/// Line that [`mark_answer`] appends to answers of @schrift and @belege (without a Bible).
pub const MARK_SCRIPTURE: &str = "Wortlaut: ungeprüft – keine lokale Bibel in Baruch (geprüft nur Buch/Kapitel/Vers).";

/// Line that [`mark_answer`] appends to answers of other personas with passages (work without a readable Bible).
pub const MARK_OTHERS: &str = "Hinweis: Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft.";

/// Round 19c: end of the line for other personas if the work has a readable Bible; before it stands
/// „[`MARK_BIBLE`] mit dem Modul X“ (begins with [`MARK_BIBLE`], so it also counts as a closing note of Baruch).
pub const MARK_OTHERS_BIBLE_END: &str =
    "die Persona hat ihn nicht selbst bestätigt (Ergebnis je Stelle: Zitate prüfen oder @belege).";

/// Line for other personas with a Bible: „Wortlaut geprüft von Baruch (nicht vom Modell) mit dem Modul X; die Persona
/// hat ihn nicht selbst bestätigt (…).“
pub fn mark_others_bible(module: &str) -> String {
    format!("{MARK_BIBLE} mit dem Modul {module}; {MARK_OTHERS_BIBLE_END}")
}

/// Beginning of the line with a Bible (then module, versification, licence and the results per passage).
pub const MARK_BIBLE: &str = "Wortlaut geprüft von Baruch (nicht vom Modell)";

/// Wording of a passage without a Bible in the work language ([`UNCHECKED`] in German).
pub fn unchecked_in(lang: WorkLanguage) -> String {
    book_t(lang, "book.scripture.unchecked", &[])
}

/// [`MARK_SCRIPTURE`] in the work language.
pub fn mark_scripture(lang: WorkLanguage) -> String {
    book_t(lang, "book.scripture.mark_scripture", &[])
}

/// [`MARK_OTHERS`] in the work language.
pub fn mark_others(lang: WorkLanguage) -> String {
    book_t(lang, "book.scripture.mark_others", &[])
}

/// [`MARK_BIBLE`] in the work language.
pub fn mark_bible(lang: WorkLanguage) -> String {
    book_t(lang, "book.scripture.mark_bible", &[])
}

/// [`mark_others_bible`] in the work language.
pub fn mark_others_bible_in(module: &str, lang: WorkLanguage) -> String {
    book_t(lang, "book.scripture.mark_others_bible", &[("mark", &mark_bible(lang)), ("module", module)])
}

/// Starts of the closing lines of the passage check in every work language (for recognitions such as the chat).
pub fn closing_marks() -> Vec<String> {
    WorkLanguage::ALL.into_iter().flat_map(|lang| [mark_others(lang), mark_scripture(lang), mark_bible(lang)]).collect()
}

/// Catalog keys of the wording labels of [`crate::bibles`], longest German text first (one contains the other).
const WORDING_KEYS: [&str; 10] = [
    "book.scripture.invalid_ref",
    "book.scripture.versification_mismatch",
    "book.scripture.missing_verse",
    "book.scripture.module_spacing",
    "book.scripture.wording_unchecked",
    "book.scripture.wording_matches",
    "book.scripture.wording_differs",
    "book.scripture.no_quote",
    "book.scripture.partial",
    "book.scripture.unchecked",
];

/// A wording label of [`crate::bibles`] (German, such as „Wortlaut stimmt (Teilzitat)“ or „Vers fehlt im Modul X:
/// …“) in the work language: the fixed phrases from the book catalog, module names, references and reasons stay.
/// German: unchanged.
pub fn wording_label_in(label: &str, lang: WorkLanguage) -> String {
    if lang == WorkLanguage::De {
        return label.to_owned();
    }
    let mut out = label.to_owned();
    for key in WORDING_KEYS {
        out = out.replace(&book_t(WorkLanguage::De, key, &[]), &book_t(lang, key, &[]));
    }
    let module = |l| format!("({} ", book_t(l, "book.scripture.module", &[]));
    out.replace(&module(WorkLanguage::De), &module(lang))
}

const METHODS: &[&str] = &["scripture.check"];

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// RPC `scripture.check {text, work?, module?}` → `{refs, truncated, source, bible}` (`docs/RPC.md`,
/// „Bibelstellen“). Read only.
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
            let lang = crate::work_language::of(state, work).await;
            check_with_in(&choice, text, lang).map_err(|message| RpcError::new(INVALID_PARAMS, message))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Check of a text as JSON without a Bible (wording „ungeprüft“); `Err` if the text is too long.
pub fn check(text: &str) -> Result<Value, String> {
    check_with(&Choice::None, text)
}

/// Check with the Bible of the work `work` (without a work: no Bible); the wording labels in the language of the work
/// (Round 39b; without a work German).
pub async fn check_for_work(state: &AppState, work: Option<WorkId>, text: &str) -> Result<Value, RpcError> {
    let lang = match work {
        Some(w) => crate::work_language::of(state, w).await,
        None => WorkLanguage::De,
    };
    check_for_work_in(state, work, text, lang).await
}

/// Like [`check_for_work`] with the language of the labels given (the passage page of the interface stays German).
pub async fn check_for_work_in(
    state: &AppState,
    work: Option<WorkId>,
    text: &str,
    lang: WorkLanguage,
) -> Result<Value, RpcError> {
    let choice = match work {
        Some(w) => crate::bibles::of_work(state, w).await?,
        None => Choice::None,
    };
    check_with_in(&choice, text, lang).map_err(|message| RpcError::new(INVALID_PARAMS, message))
}

/// Check of a text with `choice` as JSON (German labels).
pub fn check_with(choice: &Choice, text: &str) -> Result<Value, String> {
    check_with_in(choice, text, WorkLanguage::De)
}

/// Like [`check_with`] with the wording labels (`wording`) in the work language `lang` (Round 39b); the codes
/// (`wording_code`, `status`) stay the same in every language.
pub fn check_with_in(choice: &Choice, text: &str, lang: WorkLanguage) -> Result<Value, String> {
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("text: höchstens {MAX_TEXT_CHARS} Zeichen"));
    }
    let found = refs(text);
    let truncated = found.len() > MAX_REFS;
    let found: Vec<Found> = found.into_iter().take(MAX_REFS).collect();
    let wording = check_wording(choice.bible(), text, &found);
    // Round 19c: note on strikingly many word fragments in the module (per answer and per passage, else null).
    let note = crate::bibles::choice_note(choice);
    let list: Vec<Value> = found.iter().zip(&wording).map(|(f, w)| entry(f, w, note.as_deref(), lang)).collect();
    Ok(json!({ "refs": list, "truncated": truncated, "source": baruch_scripture::SOURCE, "bible": choice.json(),
               "module_note": note }))
}

fn entry(found: &Found, wording: &WordingResult, module_note: Option<&str>, lang: WorkLanguage) -> Value {
    let book = found.passage.as_ref().map(|p| p.book);
    let short = match wording.code {
        "unchecked" => unchecked_in(lang),
        _ => wording_label_in(&wording.label, lang),
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
        // R17c (additive): byte range of the quotation content in the checked text and the passage read in the
        // versification of the module if it differs from the German one.
        "quote_span": wording.quote_span.map(|(s, e)| [s, e]),
        "module_ref": wording.module_ref,
        // Round 19c: the wording matches only without whitespace (code stays "match"), note on the module.
        "module_spacing": wording.module_spacing,
        "module_note": module_note,
    })
}

/// Passages in `text`: read strictly if the whole text is a reference, otherwise searched in the running text.
pub fn refs(text: &str) -> Vec<Found> {
    let strict = baruch_scripture::parse(text);
    let is_reference = !strict.is_empty()
        && !text.contains('\n')
        && strict.iter().all(|f| !matches!(f.status, RefStatus::Malformed(_)));
    if is_reference { strict } else { baruch_scripture::scan(text) }
}

/// @schrift or @belege (with or without @, case does not matter)?
fn checks_scripture(handle: &str) -> bool {
    let name = handle.trim().trim_start_matches('@');
    name.eq_ignore_ascii_case("schrift") || name.eq_ignore_ascii_case("belege")
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max { text.to_owned() } else { text.chars().take(max).collect::<String>() + " …" }
}

/// Facts block of the passage check for the system prompt of @schrift and @belege (without a Bible); else `None`.
pub fn facts_for(handle: &str, section: &str) -> Option<String> {
    facts_with(&Choice::None, handle, section)
}

/// Facts block with the Bible of the work (wording checked per passage if one is chosen).
pub async fn facts_for_work(state: &AppState, work: WorkId, handle: &str, section: &str) -> Option<String> {
    if !checks_scripture(handle) {
        return None;
    }
    let choice = crate::bibles::of_work(state, work).await.unwrap_or(Choice::None);
    let lang = crate::work_language::of(state, work).await;
    facts_with_in(&choice, handle, section, lang)
}

/// Facts block with `choice` (public for tests with a test Bible).
pub fn facts_with(choice: &Choice, handle: &str, section: &str) -> Option<String> {
    facts_with_in(choice, handle, section, WorkLanguage::De)
}

/// Facts block in the work language (Round 39b): an English work gets the block in English, so that the model
/// renders the results with the English words; German exactly as before.
pub fn facts_with_in(choice: &Choice, handle: &str, section: &str, lang: WorkLanguage) -> Option<String> {
    if lang == WorkLanguage::En {
        return facts_with_en(choice, handle, section);
    }
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
    // Round 19c: module with strikingly many word fragments (hyphenation remnants) – the note stands before
    // the passages.
    if let Some(note) = choice.bible().and_then(crate::bibles::module_note) {
        lines.push(format!("Hinweis: {note}"));
    }
    if found.is_empty() {
        lines.push("- keine Stellenangabe gefunden".to_owned());
    }
    // The same passage with the same result only once; with another quotation (another result) once more.
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

/// Closing sentence of the English facts block without a Bible.
pub const FACTS_END_EN: &str =
    "Wording: Baruch has no local Bible; every quotation stays unchecked. Do not confirm any wording.";

/// Closing sentence of the English facts block with a Bible.
pub const FACTS_END_BIBLE_EN: &str = "Wording: Baruch checked the results above with the named SWORD module, not \
you. Only render them; never confirm a wording yourself, and do not copy Bible text except the checked verse of a \
difference.";

/// The facts block of [`facts_with_in`] for an English work: the same facts, English sentences and labels.
fn facts_with_en(choice: &Choice, handle: &str, section: &str) -> Option<String> {
    if !checks_scripture(handle) {
        return None;
    }
    let lang = WorkLanguage::En;
    let bible = choice.bible();
    let head = match (bible, choice) {
        (Some(b), _) => format!(
            "Reference check (Baruch, versification Schlachter 1951; wording from the SWORD module {} \"{}\", \
             versification {}, licence according to conf: {}):",
            b.info.name, b.info.description, b.v11n.name, b.info.notice
        ),
        (None, Choice::None) => "Reference check (Baruch, versification Schlachter 1951 – no wording):".to_owned(),
        (None, other) => format!(
            "Reference check (Baruch, versification Schlachter 1951 – no wording; the chosen Bible cannot be read: \
             {}):",
            other.json()["problem"].as_str().unwrap_or("unknown")
        ),
    };
    let found = baruch_scripture::scan(section);
    let wording = check_wording(bible, section, &found);
    let mut lines = vec![head];
    if let Some(note) = choice.bible().and_then(crate::bibles::module_note) {
        lines.push(format!("Note: {note}"));
    }
    if found.is_empty() {
        lines.push("- no reference found".to_owned());
    }
    let mut seen: Vec<(String, String, Option<String>)> = Vec::new();
    for (f, w) in found.iter().zip(&wording) {
        let label = f.normalized.clone().unwrap_or_else(|| f.input.clone());
        let id = (label.clone(), w.code.to_owned(), w.quote.clone());
        if seen.contains(&id) {
            continue;
        }
        if seen.len() == MAX_FACTS {
            lines.push(format!("- … further references not listed (at most {MAX_FACTS})"));
            break;
        }
        let result = if f.status == RefStatus::Valid { "exists".to_owned() } else { f.message() };
        let said = match w.code {
            "unchecked" => book_t(lang, "book.scripture.wording_unchecked", &[]),
            "differs" => format!(
                "{} – quotation \"{}\", Bible \"{}\", differences: {}",
                wording_label_in(&w.label, lang),
                shorten(w.quote.as_deref().unwrap_or_default(), MAX_FACT_TEXT),
                shorten(w.bible_text.as_deref().unwrap_or_default(), MAX_FACT_TEXT),
                baruch_scripture::wording::diff_line(&w.diff)
            ),
            _ => wording_label_in(&w.label, lang),
        };
        lines.push(format!("- \"{}\" → {label}: {result}; {said}", f.input));
        seen.push(id);
    }
    lines.push(if bible.is_some() { FACTS_END_BIBLE_EN } else { FACTS_END_EN }.to_owned());
    Some(lines.join("\n"))
}

/// Appends the note on the wording to an answer (without a Bible): for @schrift/@belege, if section or
/// answer name a passage, [`MARK_SCRIPTURE`]; for other personas, if the answer names a passage,
/// [`MARK_OTHERS`] (with a Bible [`mark_others_bible`], see [`mark_with`]); otherwise the answer stays unchanged.
/// If the line is already there, it is not added once more. Torn-apart digits of a passage of the section are
/// put together.
pub fn mark_answer(handle: &str, section: &str, body: String) -> String {
    mark_with(&Choice::None, handle, section, body)
}

/// Like [`mark_answer`], with the Bible of the work: for @schrift/@belege the line [`MARK_BIBLE`] with
/// module, versification, licence and the result per passage (by Baruch, not by the model) then stands instead of
/// „ungeprüft“. Round 19c: for other personas (such as the lector) the line [`mark_others_bible`] stands with a
/// Bible instead of [`MARK_OTHERS`]; without a Bible it stays with [`MARK_OTHERS`].
pub async fn mark_answer_for_work(state: &AppState, work: WorkId, handle: &str, section: &str, body: String) -> String {
    // Open the Bible only if a line comes into question at all (passage in section or answer).
    let wanted = if checks_scripture(handle) {
        !baruch_scripture::scan(section).is_empty() || !baruch_scripture::scan(&body).is_empty()
    } else {
        !baruch_scripture::scan(&body).is_empty()
    };
    let choice = if wanted { crate::bibles::of_work(state, work).await.unwrap_or(Choice::None) } else { Choice::None };
    let lang = crate::work_language::of(state, work).await;
    mark_with_in(&choice, handle, section, body, lang)
}

/// Like [`mark_answer`] with `choice` (public for tests with a test Bible).
pub fn mark_with(choice: &Choice, handle: &str, section: &str, body: String) -> String {
    mark_with_in(choice, handle, section, body, WorkLanguage::De)
}

/// Like [`mark_with`] with the closing line in the work language `lang` (Round 39b). If the line is already there,
/// it is not added once more.
pub fn mark_with_in(choice: &Choice, handle: &str, section: &str, body: String, lang: WorkLanguage) -> String {
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
            Some(bible) if any => Some(bible_line(bible, section, &in_section, lang)),
            _ => any.then(|| mark_scripture(lang)),
        }
    } else if baruch_scripture::scan(&body).is_empty() {
        None
    } else {
        Some(match choice.bible() {
            Some(bible) => mark_others_bible_in(&bible.info.name, lang),
            None => mark_others(lang),
        })
    };
    match line {
        Some(line) if !body.contains(&line) => format!("{}\n\n{line}", body.trim_end()),
        _ => body,
    }
}

/// „Wortlaut geprüft von Baruch (nicht vom Modell), Modul GerSch, Versifikation German, Lizenz …: Joh 3,16
/// stimmt; Ps 23,1 kein Zitat gefunden.“
fn bible_line(bible: &baruch_sword::Bible, section: &str, found: &[Found], lang: WorkLanguage) -> String {
    let wording = check_wording(Some(bible), section, found);
    // „Wortlaut stimmt“ → „stimmt“ after the reference: the first word of the catalog label with its space.
    let matches = book_t(lang, "book.scripture.wording_matches", &[]);
    let word = matches.split_once(' ').map_or("", |(first, _)| first).to_owned() + " ";
    let mut parts: Vec<String> = Vec::new();
    for (f, w) in found.iter().zip(&wording) {
        let label = f.normalized.clone().unwrap_or_else(|| f.input.clone());
        let part = format!("{label} {}", wording_label_in(&w.label, lang).trim_start_matches(word.as_str()));
        if !parts.contains(&part) && parts.len() < MAX_FACTS {
            parts.push(part);
        }
    }
    let list = if parts.is_empty() { book_t(lang, "book.scripture.no_passage", &[]) } else { parts.join("; ") };
    let line = book_t(
        lang,
        "book.scripture.bible_line",
        &[
            ("mark", &mark_bible(lang)),
            ("module", &bible.info.name),
            ("versification", bible.v11n.name),
            ("notice", &bible.info.notice),
            ("list", &list),
        ],
    );
    // Round 19c: note on word fragments in the module in the same line (this way it stays a closing note).
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
        // Finding of the user (2026-11-06): the check list of @schrift said „Mk 1 0,42–45“. The parser
        // reads „Mk 10,42–45“ correctly; the model had separated the digits.
        let section = "Wer groß sein will (Mk 10,42–45), dient.";
        let answer = "- Mk 1 0,42–45: Stelle gibt es; Wortlaut ungeprüft.".to_owned();
        let marked = mark_answer("@schrift", section, answer);
        assert!(marked.starts_with("- Mk 10,42–45: Stelle gibt es"), "Ziffern zusammengesetzt: {marked}");
        assert!(!marked.contains("Mk 1 0"), "keine getrennten Ziffern mehr: {marked}");
    }
}
