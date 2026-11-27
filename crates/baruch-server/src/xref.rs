// SPDX-License-Identifier: AGPL-3.0-or-later
//! Querverweise und Sprungmarken (Runde 30a).
//!
//! - Sprung-Adresse [`jump_url`]: `/?work=W&key=K[&rev=R]&find=<Text>[&at=<Byte>]`; der Server ignoriert `find` und
//!   `at`, `web/src/jump.ts` markiert damit die Stelle. Gebraucht von Suche, Querverweisen und Persona-Sprungmarken.
//! - Sprungmarken in Persona-Beiträgen (`[[key:N]]`, `[[key:N|„Zitat“]]`) prüft [`check_marker`]: nur geprüfte werden
//!   Links (gebaut von Baruch, nie vom Modell).
//! - [`outline_mentions`]: eindeutige Nennungen von Gliederungspunkten im Text („die Einleitung“, „Vorwort“, „im
//!   dritten Kapitel“, „Kapitel III“, „Abschnitt 2.3“, Titel in Anführungszeichen) – Grundlage der Liste „mögliche
//!   Querverweise“ ([`candidates`]) und der Gliederungs-Links im Chat.
//! - RPC `xref.targets`, `xref.resolve`, `xref.candidates`, `xref.convert` (nur der Mensch, `confirm: true`, eine
//!   neue Fassung, ein Commit), `xref.broken`; Seiten `GET /verweise` und `POST /verweise/act` (ohne JavaScript).
//! - Kaputte Verweise ([`broken_in`]) stehen auch in der Druckprüfung der Satz-Seite; im Satz werden sie zum bloßen
//!   Linktext, damit der Rest des Werks gesetzt wird (`typeset.rs`).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::{NodeKind, Work};
use baruch_store::WorkId;
use baruch_typst::xref::{find_xrefs, key_label, parse_key_label, xref_snippet};
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use regex::Regex;
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::search::TextKind;
use crate::view::Settings;

/// Höchstens so viele Zeichen von `find` kommen in die Sprung-Adresse.
const MAX_FIND_CHARS: usize = 200;

/// Ziel eines Sprungs: Werk, Abschnitt, wahlweise Fassung, zu markierender Text und Byte-Stelle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JumpTarget {
    pub work: WorkId,
    pub key: i64,
    pub rev: Option<u32>,
    pub find: Option<String>,
    pub at: Option<usize>,
}

/// `view::node_url(work, key, rev, false, "", "")` + `&find=<prozentkodiert, höchstens 200 Zeichen>` + `&at=<Byte>`.
pub fn jump_url(target: &JumpTarget) -> String {
    let mut url = crate::view::node_url(target.work, target.key, target.rev, false, "", "");
    if let Some(find) = target.find.as_deref().filter(|f| !f.is_empty()) {
        let cut: String = find.chars().take(MAX_FIND_CHARS).collect();
        url.push_str(&format!("&find={}", crate::view::query_escape(&cut)));
    }
    if let Some(at) = target.at {
        url.push_str(&format!("&at={at}"));
    }
    url
}

/// Ergebnis der Prüfung einer Sprungmarke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerCheck {
    /// Geprüft: Ziel und Beschriftung („<Nummer> <Titel>“ bzw. „Zitat“).
    Linked { target: JumpTarget, label: String },
    /// Den Schlüssel gibt es im Werk nicht.
    UnknownKey { key: i64 },
    /// Das Zitat steht nicht in der Fassung.
    QuoteMissing { key: i64, quote: String },
}

/// Leerraum vereinheitlichen (Folgen → ein Leerzeichen, Ränder weg).
fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Sprungmarke prüfen: Gibt es den Schlüssel im Werk? Steht das Zitat (Leerraum vereinheitlicht, Anführungszeichen
/// „…“ bzw. "…" am Rand entfernt) in Fassung `rev` bzw. der aktuellen? Beschriftung „<Nummer> <Titel>“ bzw. „Zitat“.
/// Beim Zitat trägt das Ziel die Byte-Stelle des Zitats (`at`), wenn es wörtlich (ohne Vereinheitlichung) vorkommt.
pub fn check_marker(
    work_id: WorkId,
    work: &baruch_core::Work,
    key: i64,
    quote: Option<&str>,
    rev: Option<u32>,
) -> MarkerCheck {
    let Some(node) = work.nodes.iter().find(|n| n.key == key) else {
        return MarkerCheck::UnknownKey { key };
    };
    let title = format!("{} {}", node.number, node.title).trim().to_owned();
    let quote = quote.map(|q| q.trim().trim_matches(['„', '“', '”', '"']).trim()).filter(|q| !q.is_empty());
    let Some(quote) = quote else {
        let target = JumpTarget { work: work_id, key, rev, find: None, at: None };
        return MarkerCheck::Linked { target, label: title };
    };
    let number = rev.or_else(|| node.current_number());
    let body = number.and_then(|n| node.revision(n)).map(|r| r.body.as_str()).unwrap_or("");
    let wanted = squash(quote);
    if wanted.is_empty() || !squash(body).contains(&wanted) {
        return MarkerCheck::QuoteMissing { key, quote: quote.to_owned() };
    }
    let target = JumpTarget { work: work_id, key, rev, find: Some(quote.to_owned()), at: body.find(quote) };
    MarkerCheck::Linked { target, label: "Zitat".to_owned() }
}

/// Eine Nennung eines Gliederungspunkts im Text (Bytes `start..end`, Schlüssel, Text).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutlineMention {
    pub start: usize,
    pub end: usize,
    pub key: i64,
    pub text: String,
}

/// Ordnungszahlen eins bis zwölf (Stamm, gebeugt mit -e, -en, -er, -es, -em).
const ORDINALS: [&str; 12] =
    ["erst", "zweit", "dritt", "viert", "fünft", "sechst", "siebt", "acht", "neunt", "zehnt", "elft", "zwölft"];

/// Größte Ordnungszahl in Worten, die erkannt wird („hundertste“).
pub const MAX_ORDINAL: usize = 100;

/// Stamm der Ordnungszahl `n` (1 bis [`MAX_ORDINAL`], Runde 36 über „zwölfte“ hinaus): „dreizehnt“, „zwanzigst“,
/// „einundvierzigst“, „hundertst“.
fn ordinal_stem(n: usize) -> Option<String> {
    const TEENS: [&str; 7] = ["drei", "vier", "fünf", "sech", "sieb", "acht", "neun"];
    const UNITS: [&str; 9] = ["ein", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun"];
    const TENS: [&str; 8] = ["zwanzig", "dreißig", "vierzig", "fünfzig", "sechzig", "siebzig", "achtzig", "neunzig"];
    match n {
        1..=12 => Some(ORDINALS[n - 1].to_owned()),
        13..=19 => Some(format!("{}zehnt", TEENS[n - 13])),
        20..=99 => {
            let tens = TENS[n / 10 - 2];
            Some(match n % 10 {
                0 => format!("{tens}st"),
                unit => format!("{}und{tens}st", UNITS[unit - 1]),
            })
        }
        100 => Some("hundertst".to_owned()),
        _ => None,
    }
}

/// Endungen einer gebeugten Ordnungszahl.
const ORDINAL_ENDINGS: [&str; 5] = ["en", "er", "es", "em", "e"];

/// Ordnungszahl in Worten als Zahl: „dritten“ → 3, „Einundvierzigstes“ → 41; alles andere → `None`. Verglichen wird
/// der ganze Stamm (nicht sein Anfang): „achtzehnten“ ist 18, nicht 8.
fn ordinal(word: &str) -> Option<usize> {
    let lower = word.to_lowercase();
    let stem = ORDINAL_ENDINGS.iter().find_map(|ending| lower.strip_suffix(ending))?;
    (1..=MAX_ORDINAL).find(|n| ordinal_stem(*n).as_deref() == Some(stem))
}

/// Römische Zahl (I bis MMMCMXCIX, Runde 36 über XXXIX hinaus) als Zahl; nur die übliche Schreibweise gilt
/// („IIII“, „VX“ oder „IC“ sind keine Zahl).
fn roman(text: &str) -> Option<usize> {
    const THOUSANDS: [&str; 4] = ["", "M", "MM", "MMM"];
    const HUNDREDS: [&str; 10] = ["", "C", "CC", "CCC", "CD", "D", "DC", "DCC", "DCCC", "CM"];
    const TENS: [&str; 10] = ["", "X", "XX", "XXX", "XL", "L", "LX", "LXX", "LXXX", "XC"];
    const ONES: [&str; 10] = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX"];
    if text.is_empty() || text.len() > 15 {
        return None;
    }
    (1..4000).find(|n| {
        format!("{}{}{}{}", THOUSANDS[n / 1000], HUNDREDS[n / 100 % 10], TENS[n / 10 % 10], ONES[n % 10]) == text
    })
}

/// Muster der Zählnennungen: „im dritten Kapitel“, „drittes Kapitel“, „Kapitel III“, „Kapitel 3“, „Teil II“,
/// „zweiten Teil“, „Abschnitt 2.3“.
fn counted_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Alle Stämme von 1 bis MAX_ORDINAL, die längsten zuerst (sonst träfe „acht“ vor „achtzehnt“).
        let mut stems: Vec<String> = (1..=MAX_ORDINAL).filter_map(ordinal_stem).collect();
        stems.sort_by_key(|stem| std::cmp::Reverse(stem.len()));
        let ordinals = stems.join("|");
        Regex::new(&format!(
            r"\b(?:(?P<ord>(?i:{ordinals})(?:e|en|er|es|em))\s+(?P<kind1>Kapitel|Teil|Teils|Abschnitt|Abschnitts)\b|(?P<kind2>Kapitel|Teil|Abschnitt)\s+(?P<num>[IVXLCDM]+\b|\d+(?:\.\d+)*))"
        ))
        .expect("Muster der Gliederungsnennungen")
    })
}

/// Zahl ohne Schlusspunkt („3.“ → „3“).
fn bare_number(number: &str) -> &str {
    number.trim().trim_end_matches('.')
}

/// Ziel einer Zählnennung: Art und Ordnungszahl bzw. Nummer → eindeutiger Punkt.
fn counted_target(work: &Work, kind: NodeKind, index: Option<usize>, number: Option<&str>) -> Option<i64> {
    let of_kind: Vec<&baruch_core::Node> = work.nodes.iter().filter(|n| n.kind == kind).collect();
    // Gezählt („das dritte Kapitel“) werden nur Punkte mit Nummer: ein Vorwort ohne Nummer ist kein erstes Kapitel.
    let counted: Vec<&baruch_core::Node> = of_kind.iter().copied().filter(|n| !n.number.trim().is_empty()).collect();
    if let Some(number) = number {
        let by_number: Vec<&&baruch_core::Node> = of_kind.iter().filter(|n| bare_number(&n.number) == number).collect();
        if let [one] = by_number.as_slice() {
            return Some(one.key);
        }
        if !by_number.is_empty() {
            return None;
        }
        // „Abschnitt 2.3“ ohne passende Nummer: auch Unterabschnitte mit dieser Nummer.
        let any: Vec<&baruch_core::Node> = work.nodes.iter().filter(|n| bare_number(&n.number) == number).collect();
        if let [one] = any.as_slice()
            && number.contains('.')
        {
            return Some(one.key);
        }
        let n = number.parse::<usize>().ok().or_else(|| roman(number))?;
        return counted.get(n.checked_sub(1)?).map(|n| n.key);
    }
    counted.get(index?.checked_sub(1)?).map(|n| n.key)
}

/// Eindeutige Nennungen von Gliederungspunkten in `text`, nach Stelle, ohne Überlappung: Zählnennungen („im dritten
/// Kapitel“ → „dritten Kapitel“, „Kapitel III“, „Abschnitt 2.3“; seit Runde 36 Ordnungszahlen bis „hundertste“
/// und römische Zahlen in üblicher Schreibweise über XXXIX hinaus, etwa „Kapitel XLII“) und Titel (mindestens vier Zeichen, als ganze Wörter;
/// mit vorangestelltem Artikel „die Einleitung“). Ein Titel, den mehrere Punkte tragen, und eine Zahl ohne genau ein
/// Ziel werden nicht genannt (eindeutig oder gar nicht). Kürzere Titel zählen nur in Anführungszeichen.
pub fn outline_mentions(work: &baruch_core::Work, text: &str) -> Vec<OutlineMention> {
    let mut found: Vec<OutlineMention> = Vec::new();
    for caps in counted_pattern().captures_iter(text) {
        let whole = caps.get(0).expect("Gruppe 0");
        let (kind_word, index, number) = if let Some(ord) = caps.name("ord") {
            (caps.name("kind1").map_or("", |m| m.as_str()), ordinal(ord.as_str()), None)
        } else {
            (caps.name("kind2").map_or("", |m| m.as_str()), None, caps.name("num").map(|m| m.as_str()))
        };
        let kind = match kind_word {
            "Kapitel" => NodeKind::Chapter,
            "Teil" | "Teils" => NodeKind::Part,
            _ => NodeKind::Section,
        };
        if let Some(key) = counted_target(work, kind, index, number) {
            found.push(OutlineMention { start: whole.start(), end: whole.end(), key, text: whole.as_str().to_owned() });
        }
    }
    // Titel: je Titel (ohne Groß/klein) genau ein Punkt.
    let mut titles: HashMap<String, Vec<i64>> = HashMap::new();
    for node in &work.nodes {
        let title = node.title.trim();
        if !title.is_empty() {
            titles.entry(title.to_lowercase()).or_default().push(node.key);
        }
    }
    for node in &work.nodes {
        let title = node.title.trim();
        if title.is_empty() || titles.get(&title.to_lowercase()).map_or(0, Vec::len) != 1 {
            continue;
        }
        let quoted_only = title.chars().count() < 4;
        let pattern = format!(r"(?i:\b(?P<art>(?:die|der|das|den|dem|des)\s+)?){}", regex::escape(title));
        let Ok(re) = Regex::new(&pattern) else { continue };
        for caps in re.captures_iter(text) {
            let whole = caps.get(0).expect("Gruppe 0");
            let title_start = whole.end() - title.len();
            // Ganze Wörter: kein Wortzeichen direkt davor bzw. dahinter.
            let before = text[..title_start].chars().next_back();
            let after = text[whole.end()..].chars().next();
            let wordy = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            if wordy(before) && caps.name("art").is_none() || wordy(after) {
                continue;
            }
            let quoted = matches!(before, Some('„' | '"' | '“' | '»')) && matches!(after, Some('“' | '"' | '”' | '«'));
            if quoted_only && !quoted {
                continue;
            }
            // Groß/klein des Titels selbst muss stimmen (der Artikel darf klein oder groß sein).
            if &text[title_start..whole.end()] != title {
                continue;
            }
            found.push(OutlineMention {
                start: whole.start(),
                end: whole.end(),
                key: node.key,
                text: whole.as_str().to_owned(),
            });
        }
    }
    found.sort_by_key(|m| (m.start, std::cmp::Reverse(m.end)));
    let mut out: Vec<OutlineMention> = Vec::new();
    for m in found {
        if out.last().is_some_and(|last| m.start < last.end) {
            continue;
        }
        out.push(m);
    }
    out
}

/// Ein möglicher Querverweis: Abschnitt, Fassung, Stelle, Text, Ziel und der Ersatz (`#link(<kN>)[Text]`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Candidate {
    pub key: i64,
    pub rev: u32,
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub target_key: i64,
    pub replacement: String,
}

/// Mögliche Querverweise in einem Text: Nennungen im Fließtext (nicht in Überschrift, Zitat, Code, Fußnote), nicht in
/// einem vorhandenen Verweis, nicht auf den eigenen Abschnitt.
pub fn candidates_in(work: &Work, key: i64, rev: u32, text: &str) -> Vec<Candidate> {
    let kinds = crate::search::scan::classify(text);
    let existing = find_xrefs(text);
    outline_mentions(work, text)
        .into_iter()
        .filter(|m| m.key != key)
        .filter(|m| crate::search::scan::kind_of(&kinds, &(m.start..m.end)) == TextKind::Text)
        .filter(|m| !existing.iter().any(|x| m.start < x.end && x.start < m.end))
        .map(|m| Candidate {
            key,
            rev,
            start: m.start,
            end: m.end,
            replacement: xref_snippet(m.key, &m.text, false),
            text: m.text,
            target_key: m.key,
        })
        .collect()
}

/// Mögliche Querverweise eines Abschnitts (aktuelle Fassung) bzw. des ganzen Werks.
pub fn candidates(work: &Work, key: Option<i64>) -> Vec<Candidate> {
    work.nodes
        .iter()
        .filter(|n| key.is_none_or(|k| k == n.key))
        .filter_map(|n| n.current_number().and_then(|r| n.revision(r)).map(|r| (n, r)))
        .flat_map(|(n, r)| candidates_in(work, n.key, r.number, &r.body))
        .collect()
}

/// Ein kaputter Verweis: Abschnitt, Fassung, Sprungmarke, Stelle und Linktext.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Broken {
    pub key: i64,
    pub rev: u32,
    pub label: String,
    pub start: usize,
    pub text: String,
    pub target: i64,
}

/// Kaputte Verweise des Werks (aktuelle Fassungen): Ziel gibt es nicht oder es hat keine aktuelle Fassung (dann
/// steht seine Sprungmarke nicht im Export).
pub fn broken_in(work: &Work) -> Vec<Broken> {
    let present: std::collections::HashSet<i64> =
        work.nodes.iter().filter(|n| n.current_number().is_some()).map(|n| n.key).collect();
    let mut out = Vec::new();
    for node in &work.nodes {
        let Some(rev) = node.current_number().and_then(|r| node.revision(r)) else { continue };
        for x in find_xrefs(&rev.body) {
            if !present.contains(&x.key) {
                out.push(Broken {
                    key: node.key,
                    rev: rev.number,
                    label: key_label(x.key),
                    start: x.start,
                    text: x.text,
                    target: x.key,
                });
            }
        }
    }
    out
}

/// Ziele für den Dialog „Verweis“: Schlüssel, Sprungmarke, Nummer, Titel, Ebene, fertiger Verweis.
fn targets(work: &Work, q: &str, limit: usize) -> Vec<Value> {
    let needle = q.trim().to_lowercase();
    work.nodes
        .iter()
        .filter(|n| {
            needle.is_empty()
                || n.title.to_lowercase().contains(&needle)
                || n.number.to_lowercase().starts_with(&needle)
                || key_label(n.key) == needle
        })
        .take(limit)
        .map(|n| {
            let level = level_of(work, n);
            json!({
                "key": n.key, "label": key_label(n.key), "number": n.number, "title": n.title, "level": level,
                "snippet": xref_snippet(n.key, &baruch_typst::escape_title(&n.title), false),
            })
        })
        .collect()
}

/// Ebene eines Punkts: 0 Teil, 1 Kapitel, 2 Abschnitt, 3 Unterabschnitt.
fn level_of(work: &Work, node: &baruch_core::Node) -> u8 {
    match node.kind {
        NodeKind::Part => 0,
        NodeKind::Chapter => 1,
        NodeKind::Section => {
            let parent = node.parent.and_then(|p| work.nodes.get(p));
            if parent.is_some_and(|p| p.kind == NodeKind::Section) { 3 } else { 2 }
        }
    }
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Querverweise umwandeln (nur der Mensch): die gewählten möglichen Verweise der Fassung `rev` einsetzen und als neue
/// Fassung auf `rev` speichern (ein Commit). Antwort `{node, key, rev, converted}`.
pub async fn convert(state: &AppState, work: WorkId, key: i64, rev: u32, picks: &[usize]) -> Result<Value, RpcError> {
    if picks.is_empty() {
        return Err(invalid("Bitte mindestens einen möglichen Verweis wählen (picks)."));
    }
    let held = state.write_lock.lock().await;
    let loaded = state.store.load_work(work).await?;
    let node = loaded
        .nodes
        .iter()
        .find(|n| n.key == key)
        .ok_or_else(|| invalid(format!("Den Abschnitt {key} gibt es in diesem Werk nicht.")))?;
    let revision =
        node.revision(rev).ok_or_else(|| invalid(format!("Fassung {rev} gibt es in diesem Abschnitt nicht.")))?;
    let list = candidates_in(&loaded, key, rev, &revision.body);
    let mut chosen: Vec<&Candidate> = Vec::new();
    for &i in picks {
        let c = list.get(i).ok_or_else(|| invalid(format!("Möglichen Verweis {i} gibt es nicht (mehr).")))?;
        if !chosen.contains(&c) {
            chosen.push(c);
        }
    }
    chosen.sort_by_key(|c| c.start);
    let mut body = String::with_capacity(revision.body.len() + 32 * chosen.len());
    let mut at = 0;
    for c in &chosen {
        body.push_str(&revision.body[at..c.start]);
        body.push_str(&c.replacement);
        at = c.end;
    }
    body.push_str(&revision.body[at..]);
    let seq = node.id;
    let change = state.store.save_human_on(work, seq, &body, crate::mirror::HUMAN_NAME, rev).await?;
    let Some(change) = change else {
        return Ok(json!({ "node": seq, "key": key, "rev": rev, "converted": 0 }));
    };
    let mut answer = json!({ "node": change.node, "key": key, "rev": change.rev, "converted": chosen.len() });
    match crate::mirror::record(state, &held, &change).await {
        Ok(Some(c)) => answer["commit"] = json!(c),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
    Ok(answer)
}

/// RPC-Methoden dieses Moduls.
pub const METHODS: &[&str] = &["xref.targets", "xref.resolve", "xref.candidates", "xref.convert", "xref.broken"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn key_param(params: &Value, name: &str) -> Result<Option<i64>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(|| invalid(format!("{name}: Zahl erwartet"))),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| invalid(format!("{name}: Zahl erwartet"))),
        Some(_) => Err(invalid(format!("{name}: Zahl erwartet"))),
    }
}

/// RPC `xref.*` beantworten.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "xref.targets" => {
            let loaded = state.store.load_work(work).await?;
            let q = params.get("q").and_then(Value::as_str).unwrap_or("");
            let limit = key_param(&params, "limit")?.unwrap_or(50).clamp(1, 500) as usize;
            Ok(json!({ "targets": targets(&loaded, q, limit) }))
        }
        "xref.resolve" => {
            let label = params.get("label").and_then(Value::as_str).unwrap_or("").trim();
            let label = label.trim_start_matches('<').trim_end_matches('>');
            let key =
                parse_key_label(label).ok_or_else(|| invalid(format!("„{label}“ ist keine Sprungmarke (kN).")))?;
            let loaded = state.store.load_work(work).await?;
            let node = loaded
                .nodes
                .iter()
                .find(|n| n.key == key)
                .ok_or_else(|| invalid(format!("Die Sprungmarke {label} gehört zu keinem Gliederungspunkt.")))?;
            let url = jump_url(&JumpTarget { work, key, rev: None, find: None, at: None });
            Ok(json!({ "key": key, "url": url, "number": node.number, "title": node.title }))
        }
        "xref.candidates" => {
            let loaded = state.store.load_work(work).await?;
            Ok(json!({ "candidates": candidates(&loaded, key_param(&params, "key")?) }))
        }
        "xref.convert" => {
            if params.get("confirm") != Some(&Value::Bool(true)) {
                return Err(invalid("Umwandeln bitte bestätigen (confirm: true)."));
            }
            let key = key_param(&params, "key")?.ok_or_else(|| invalid("key: Abschnitt fehlt"))?;
            let rev = key_param(&params, "rev")?.ok_or_else(|| invalid("rev: Fassung fehlt"))?;
            let picks: Vec<usize> = params
                .get("picks")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).map(|p| p as usize).collect())
                .unwrap_or_default();
            convert(state, work, key, u32::try_from(rev).map_err(|_| invalid("rev: Nummer erwartet"))?, &picks).await
        }
        "xref.broken" => {
            let loaded = state.store.load_work(work).await?;
            Ok(json!({ "broken": broken_in(&loaded) }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("xrefs.html", include_str!("../templates/xrefs.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Querverweise fehlerhaft");
        }
        env
    })
}

fn plain(status: StatusCode, message: &str) -> Response {
    (
        status,
        Html(format!(
            "<!doctype html><meta charset=\"utf-8\"><title>Querverweise</title><p>{}</p>",
            crate::search::replace::escape(message)
        )),
    )
        .into_response()
}

fn first<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim()).filter(|v| !v.is_empty())
}

/// Beschriftung „2.3 Titel“ eines Schlüssels.
fn label_for(work: &Work, key: i64) -> String {
    work.nodes
        .iter()
        .find(|n| n.key == key)
        .map_or_else(|| key_label(key), |n| format!("{} {}", n.number, n.title).trim().to_owned())
}

/// `GET /verweise?work=W[&key=K]`.
async fn page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let work = match first(&pairs, "work").and_then(|w| w.parse::<WorkId>().ok()) {
        Some(w) => w,
        None => match state.default_work().await {
            Ok(w) => w,
            Err(e) => return plain(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        },
    };
    let Ok(loaded) = state.store.load_work(work).await else {
        return plain(StatusCode::NOT_FOUND, "Unbekanntes Werk.");
    };
    let key = first(&pairs, "key").and_then(|k| k.parse::<i64>().ok());
    // Mögliche Verweise je Abschnitt (Formular je Abschnitt).
    let mut groups: Vec<Value> = Vec::new();
    for c in candidates(&loaded, key) {
        let node = loaded.nodes.iter().find(|n| n.key == c.key);
        let body = node.and_then(|n| n.revision(c.rev)).map(|r| r.body.as_str()).unwrap_or("");
        let line = baruch_typst::line_at(body, c.start);
        let index =
            groups.last().filter(|g| g["key"] == json!(c.key)).map(|g| g["items"].as_array().map_or(0, Vec::len));
        let target_url = jump_url(&JumpTarget { work, key: c.target_key, rev: None, find: None, at: None });
        let at_url =
            jump_url(&JumpTarget { work, key: c.key, rev: None, find: Some(c.text.clone()), at: Some(c.start) });
        let entry = json!({
            "index": index.unwrap_or(0), "text": c.text, "line": line, "target": label_for(&loaded, c.target_key),
            "replacement": c.replacement, "target_url": target_url.replace('&', "&amp;"),
            "at_url": at_url.replace('&', "&amp;"),
        });
        match groups.last_mut() {
            Some(g) if g["key"] == json!(c.key) => g["items"].as_array_mut().expect("Liste").push(entry),
            _ => {
                groups.push(json!({ "key": c.key, "rev": c.rev, "label": label_for(&loaded, c.key), "items": [entry] }))
            }
        }
    }
    let broken: Vec<Value> = broken_in(&loaded)
        .into_iter()
        .filter(|b| key.is_none_or(|k| k == b.key))
        .map(|b| {
            let url = jump_url(&JumpTarget { work, key: b.key, rev: None, find: Some(b.text.clone()), at: Some(b.start) });
            json!({ "section": label_for(&loaded, b.key), "label": b.label, "text": b.text, "url": url.replace('&', "&amp;") })
        })
        .collect();
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings)),
        work => minijinja::context! { id => work, title => loaded.title.clone() },
        key => key,
        section => key.map(|k| label_for(&loaded, k)),
        groups => JinjaValue::from(Serde(groups)),
        broken => JinjaValue::from(Serde(broken)),
        done => first(&pairs, "ok").map(str::to_owned),
        error => first(&pairs, "fehler").map(str::to_owned),
    };
    match templates().get_template("xrefs.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => plain(StatusCode::INTERNAL_SERVER_ERROR, &format!("Vorlage xrefs.html: {e}")),
    }
}

/// `POST /verweise/act`: `method=xref.convert`, `work`, `key`, `rev`, `picks` (mehrfach), `confirm` → 303 zurück.
async fn act(State(state): State<Arc<AppState>>, Form(pairs): Form<Vec<(String, String)>>) -> Response {
    let work = match first(&pairs, "work").and_then(|w| w.parse::<WorkId>().ok()) {
        Some(w) => w,
        None => match state.default_work().await {
            Ok(w) => w,
            Err(e) => return plain(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        },
    };
    let scope = first(&pairs, "scope").and_then(|k| k.parse::<i64>().ok());
    let back = match scope {
        Some(k) => format!("/verweise?work={work}&key={k}"),
        None => format!("/verweise?work={work}"),
    };
    if first(&pairs, "method") != Some("xref.convert") {
        return plain(StatusCode::BAD_REQUEST, "Unbekannte Aktion (method).");
    }
    if !matches!(first(&pairs, "confirm"), Some("1" | "true" | "on")) {
        return Redirect::to(&format!(
            "{back}&fehler={}",
            crate::view::query_escape("Bitte das Häkchen „Ja, umwandeln“ setzen.")
        ))
        .into_response();
    }
    let key = first(&pairs, "key").and_then(|k| k.parse::<i64>().ok());
    let rev = first(&pairs, "rev").and_then(|r| r.parse::<u32>().ok());
    let picks: Vec<usize> =
        pairs.iter().filter(|(k, _)| k == "picks").filter_map(|(_, v)| v.trim().parse().ok()).collect();
    let (Some(key), Some(rev)) = (key, rev) else {
        return plain(StatusCode::BAD_REQUEST, "key und rev fehlen.");
    };
    match convert(&state, work, key, rev, &picks).await {
        Ok(answer) => Redirect::to(&format!("{back}&ok={}", answer["converted"])).into_response(),
        Err(e) => Redirect::to(&format!("{back}&fehler={}", crate::view::query_escape(&e.message))).into_response(),
    }
}

/// Seiten (`GET /verweise`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/verweise", get(page))
}

/// Schreibende Routen (`POST /verweise/act`, hinter `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/verweise/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_in_words_reach_a_hundred_and_compare_the_whole_stem() {
        for (word, n) in [
            ("erste", 1),
            ("Dritten", 3),
            ("achtes", 8),
            ("zwölfter", 12),
            ("dreizehnten", 13),
            ("sechzehnte", 16),
            ("siebzehntem", 17),
            ("achtzehnten", 18),
            ("zwanzigstes", 20),
            ("einundzwanzigsten", 21),
            ("dreißigste", 30),
            ("Einundvierzigstes", 41),
            ("neunundneunzigsten", 99),
            ("hundertste", 100),
        ] {
            assert_eq!(ordinal(word), Some(n), "„{word}“ ist die Ordnungszahl {n}");
        }
        for word in ["achte Kapitel", "letzten", "nächsten", "achtzehn", "zwanzig", "dritt", "hundertersten", ""] {
            assert_eq!(ordinal(word), None, "„{word}“ ist keine erkannte Ordnungszahl");
        }
        let stems: Vec<String> = (1..=MAX_ORDINAL).filter_map(ordinal_stem).collect();
        assert_eq!(stems.len(), MAX_ORDINAL, "jede Zahl bis hundert hat einen Stamm");
        let mut unique = stems.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), stems.len(), "kein Stamm steht für zwei Zahlen (sonst wäre die Nennung mehrdeutig)");
    }

    #[test]
    fn roman_numbers_reach_beyond_thirty_nine_in_the_usual_spelling_only() {
        for (text, n) in [("I", 1), ("IV", 4), ("XXXIX", 39), ("XL", 40), ("XLII", 42), ("XC", 90), ("C", 100)] {
            assert_eq!(roman(text), Some(n), "{text} = {n}");
        }
        assert_eq!(roman("MCMXCIV"), Some(1994), "auch große Zahlen in üblicher Schreibweise");
        for text in ["", "IIII", "VX", "IC", "XXXX", "iv", "A", "MMMM"] {
            assert_eq!(roman(text), None, "„{text}“ ist keine römische Zahl in üblicher Schreibweise");
        }
    }

    #[test]
    fn a_jump_url_carries_find_and_at_after_the_canonical_address() {
        let target = JumpTarget { work: 2, key: 105, rev: Some(3), find: Some("Licht & Wasser".into()), at: Some(17) };
        assert_eq!(
            jump_url(&target),
            "/?work=2&key=105&rev=3&find=Licht%20%26%20Wasser&at=17",
            "kanonische Adresse, dann find prozentkodiert und at (Vertrag §3.2)"
        );
        let long = JumpTarget { find: Some("a".repeat(300)), at: None, rev: None, ..target };
        assert_eq!(
            jump_url(&long),
            format!("/?work=2&key=105&find={}", "a".repeat(200)),
            "find auf 200 Zeichen gekürzt"
        );
    }

    #[test]
    fn roman_numerals_read_up_to_thirty_nine() {
        for (text, n) in [("I", 1), ("IV", 4), ("IX", 9), ("XII", 12), ("XIV", 14), ("XXXIX", 39)] {
            assert_eq!(roman(text), Some(n), "{text}");
        }
        assert_eq!(roman("IIII"), None, "keine gültige römische Zahl");
        assert_eq!(roman(""), None, "leer");
    }
}
