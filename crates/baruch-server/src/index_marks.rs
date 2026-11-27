// SPDX-License-Identifier: AGPL-3.0-or-later
//! Registermarken im Text: RPC `index.entries`, `index.mark`, `index.unmark`, `index.overview` (Runde 31, Vertrag
//! §3.2; Signaturen seit R31-S1 eingefroren, Körper Aufgabe register-satz).
//!
//! Eine Registermarke ist Buchtext: Setzen und Entfernen ergeben eine neue Fassung des Menschen auf der genannten Fassung
//! (wie `save_human_on`, Schreibsperre vor dem Schreiben), protokolliert über `mirror::record` (ein Commit). Die Marke
//! selbst setzt bzw. entfernt [`baruch_typst::index::mark_word`] bzw. [`baruch_typst::index::remove_mark`]: kanonische
//! Form direkt hinter dem Wort, nie in Überschriftszeilen, Code, Kommentaren, Raw oder anderen Marken.
//!
//! - `index.entries {work?, key?}` → `{work, entries:[{key, register, term, sub, see, also, auto, start, end}]}`: die
//!   Marken der aktuellen Fassungen (Bytebereich im gespeicherten Text) und – bei eingeschaltetem Bibelstellenregister –
//!   die automatischen Bibelstellen der Projektion (`auto: true`, `start`/`end` = null).
//! - `index.mark {work?, key, rev, word, occurrence?, register?, term?, sub?, see?, also?}` → `{work, key, node, rev,
//!   marks, commit?|mirror?}`; Wort fehlt, ungültige Marke oder unbekanntes Register → `INVALID_PARAMS`.
//! - `index.unmark {work?, key, rev, start, end}` → `{work, key, node, rev, commit?|mirror?}`.
//! - `index.overview {work?}` → `{work, registers:[{slug, title, kind, terms:[IndexTerm]}]}`.

use baruch_store::WorkId;
use baruch_typst::index::{IndexMark, Occurrence, Projector, RegisterView, find_marks};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// RPC-Methoden dieses Moduls.
pub const METHODS: &[&str] = &["index.entries", "index.mark", "index.unmark", "index.overview"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Ganze Zahl aus Zahl oder Text; fehlt/leer → `None`.
fn int_param(params: &Value, name: &str) -> Result<Option<i64>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(|| invalid(format!("{name}: ganze Zahl erwartet"))),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => {
            s.trim().parse().map(Some).map_err(|_| invalid(format!("{name}: ganze Zahl erwartet")))
        }
        Some(_) => Err(invalid(format!("{name}: ganze Zahl erwartet"))),
    }
}

fn required(params: &Value, name: &str, what: &str) -> Result<i64, RpcError> {
    int_param(params, name)?.ok_or_else(|| invalid(format!("{name}: {what} fehlt")))
}

fn rev_param(params: &Value) -> Result<u32, RpcError> {
    u32::try_from(required(params, "rev", "Fassung")?).map_err(|_| invalid("rev: Nummer einer Fassung erwartet"))
}

fn text_param(params: &Value, name: &str) -> Result<String, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.trim().to_owned()),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

fn occurrence_param(params: &Value) -> Result<Occurrence, RpcError> {
    let wrong = || invalid("occurrence: erwartet „first“, „all“ oder eine Zahl ab 1");
    match params.get("occurrence") {
        None | Some(Value::Null) => Ok(Occurrence::First),
        Some(Value::Number(n)) => {
            n.as_u64().filter(|n| *n >= 1).and_then(|n| u32::try_from(n).ok()).map(Occurrence::Nth).ok_or_else(wrong)
        }
        Some(Value::String(s)) if s.trim().is_empty() => Ok(Occurrence::First),
        Some(Value::String(s)) => Occurrence::from_param(s).ok_or_else(wrong),
        Some(_) => Err(wrong()),
    }
}

/// RPC `index.entries/mark/unmark/overview` beantworten.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "index.entries" => entries(state, work, int_param(&params, "key")?).await,
        "index.mark" => {
            let key = required(&params, "key", "Abschnitt")?;
            let rev = rev_param(&params)?;
            let word = text_param(&params, "word")?;
            if word.is_empty() {
                return Err(invalid("word: Wort fehlt"));
            }
            let occurrence = occurrence_param(&params)?;
            let register = text_param(&params, "register")?;
            let term = text_param(&params, "term")?;
            let index_mark = IndexMark {
                register: if register.is_empty() { baruch_typst::index::KEYWORD_REGISTER.to_owned() } else { register },
                term: if term.is_empty() { word.clone() } else { term },
                sub: text_param(&params, "sub")?,
                see: text_param(&params, "see")?,
                also: text_param(&params, "also")?,
                sort: String::new(),
            };
            mark(state, work, MarkRequest { key, rev, word, occurrence, mark: index_mark }).await
        }
        "index.unmark" => {
            let key = required(&params, "key", "Abschnitt")?;
            let rev = rev_param(&params)?;
            let start = required(&params, "start", "Anfang der Marke")?;
            let end = required(&params, "end", "Ende der Marke")?;
            let span = (
                usize::try_from(start).map_err(|_| invalid("start: Byte-Stelle ab 0 erwartet"))?,
                usize::try_from(end).map_err(|_| invalid("end: Byte-Stelle ab 0 erwartet"))?,
            );
            if span.0 >= span.1 {
                return Err(invalid("start/end: leerer Bereich"));
            }
            unmark(state, work, key, rev, span).await
        }
        "index.overview" => {
            let views = overview(state, work).await?;
            let registers: Vec<Value> = views
                .iter()
                .map(|v| {
                    json!({
                        "slug": v.register.slug,
                        "title": v.register.title,
                        "kind": v.register.kind,
                        "terms": v.terms,
                    })
                })
                .collect();
            Ok(json!({ "work": work, "registers": registers }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Marke an einem Wort setzen: Abschnitt `key`, Grundlage Fassung `rev`.
#[derive(Debug, Clone)]
pub struct MarkRequest {
    pub key: i64,
    pub rev: u32,
    pub word: String,
    pub occurrence: baruch_typst::index::Occurrence,
    pub mark: baruch_typst::index::IndexMark,
}

/// Neue Fassung des Menschen aus der Fassung `rev` von Abschnitt `key`, deren Text `change` umformt (Schreibsperre,
/// `save_human_on`, ein Spiegel-Commit). Antwort `{work, key, node, rev, commit?|mirror?}` plus `extra`.
async fn rewrite(
    state: &AppState,
    work: WorkId,
    key: i64,
    rev: u32,
    change: impl FnOnce(&str) -> Result<(String, Value), RpcError>,
) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let loaded = state.store.load_work(work).await?;
    let node = loaded
        .nodes
        .iter()
        .find(|n| n.key == key)
        .ok_or_else(|| invalid(format!("Den Abschnitt {key} gibt es in diesem Werk nicht.")))?;
    let revision =
        node.revision(rev).ok_or_else(|| invalid(format!("Fassung {rev} gibt es in diesem Abschnitt nicht.")))?;
    let (body, extra) = change(&revision.body)?;
    let seq = node.id;
    let saved = state.store.save_human_on(work, seq, &body, crate::mirror::HUMAN_NAME, rev).await?;
    let mut answer = json!({ "work": work, "key": key, "node": seq, "rev": rev });
    if let Some(saved) = saved {
        answer["node"] = json!(saved.node);
        answer["rev"] = json!(saved.rev);
        match crate::mirror::record(state, &held, &saved).await {
            Ok(Some(c)) => answer["commit"] = json!(c),
            Ok(None) => {}
            Err(e) => answer["mirror"] = json!(e),
        }
    }
    if let (Some(target), Value::Object(more)) = (answer.as_object_mut(), extra) {
        target.extend(more);
    }
    Ok(answer)
}

/// Marke setzen → neue Fassung des Menschen.
pub async fn mark(state: &AppState, work: WorkId, request: MarkRequest) -> Result<Value, RpcError> {
    let registers = crate::glossary::registers(state, work).await?;
    if !registers.iter().any(|r| r.slug == request.mark.register) {
        return Err(invalid(format!(
            "Register „{}“ gibt es in diesem Werk nicht (vorhanden: {}).",
            request.mark.register,
            registers.iter().map(|r| r.slug.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }
    request.mark.validate().map_err(|e| invalid(e.to_string()))?;
    let MarkRequest { key, rev, word, occurrence, mark } = request;
    rewrite(state, work, key, rev, |body| {
        let (text, n) =
            baruch_typst::index::mark_word(body, &word, occurrence, &mark).map_err(|e| invalid(e.to_string()))?;
        Ok((text, json!({ "marks": n })))
    })
    .await
}

/// Marke im Bereich `span` entfernen → neue Fassung des Menschen.
pub async fn unmark(
    state: &AppState,
    work: WorkId,
    key: i64,
    rev: u32,
    span: (usize, usize),
) -> Result<Value, RpcError> {
    rewrite(state, work, key, rev, |body| {
        let text = baruch_typst::index::remove_mark(body, span).map_err(|e| invalid(e.to_string()))?;
        Ok((text, json!({})))
    })
    .await
}

/// Aktuelle Texte der Abschnitte (Baumreihenfolge), auf `key` beschränkt (unbekannt → `INVALID_PARAMS`).
async fn current_bodies(state: &AppState, work: WorkId, key: Option<i64>) -> Result<Vec<(i64, String)>, RpcError> {
    let loaded = state.store.load_work(work).await?;
    if let Some(key) = key
        && !loaded.nodes.iter().any(|n| n.key == key)
    {
        return Err(invalid(format!("Den Abschnitt {key} gibt es in diesem Werk nicht.")));
    }
    Ok(loaded
        .nodes
        .iter()
        .filter(|n| key.is_none_or(|k| k == n.key))
        .filter_map(|n| n.current_number().and_then(|r| n.revision(r)).map(|r| (n.key, r.body.clone())))
        .collect())
}

/// Marken der aktuellen Fassungen (eines Abschnitts oder des ganzen Werks) samt automatischen Bibelstellen.
pub async fn entries(state: &AppState, work: WorkId, key: Option<i64>) -> Result<Value, RpcError> {
    let bodies = current_bodies(state, work, key).await?;
    let back = state.store.back_matter(work).await?;
    let scripture = back
        .settings
        .index_pages
        .as_ref()
        .is_some_and(|p| p.registers.iter().any(|r| r == baruch_typst::index::SCRIPTURE_REGISTER));
    let auto = |m: &IndexMark| m.register == baruch_typst::index::SCRIPTURE_REGISTER && !m.sort.is_empty();
    let mut list = Vec::new();
    for (key, body) in &bodies {
        let own = find_marks(body);
        for found in &own {
            let m = &found.mark;
            list.push(json!({
                "key": key, "register": m.register, "term": m.term, "sub": m.sub, "see": m.see, "also": m.also,
                "auto": false, "start": found.span.0, "end": found.span.1,
            }));
        }
        if scripture {
            // Nur die Bibelstellen-Marken der Projektion (je Abschnitt eine eigene, damit Kapitelzustand keine Rolle
            // spielt); vorhandene automatische Marken im gespeicherten Text zählen nicht doppelt.
            let mut existing: Vec<&IndexMark> = own.iter().map(|f| &f.mark).filter(|m| auto(m)).collect();
            let projected = Projector::new(&back).project(body).into_owned();
            for found in find_marks(&projected).into_iter().filter(|f| auto(&f.mark)) {
                if let Some(i) = existing.iter().position(|m| **m == found.mark) {
                    existing.remove(i);
                    continue;
                }
                let m = &found.mark;
                list.push(json!({
                    "key": key, "register": m.register, "term": m.term, "sub": m.sub, "see": m.see, "also": m.also,
                    "auto": true, "start": null, "end": null,
                }));
            }
        }
    }
    Ok(json!({ "work": work, "entries": list }))
}

/// Übersicht je Register.
pub async fn overview(state: &AppState, work: WorkId) -> Result<Vec<baruch_typst::index::RegisterView>, RpcError> {
    let bodies = current_bodies(state, work, None).await?;
    let back = state.store.back_matter(work).await?;
    let pairs: Vec<(i64, &str)> = bodies.iter().map(|(k, b)| (*k, b.as_str())).collect();
    let views: Vec<RegisterView> = baruch_typst::index::overview(&back, &pairs);
    Ok(views)
}
