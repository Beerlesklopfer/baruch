// SPDX-License-Identifier: AGPL-3.0-or-later
//! Übersicht der Befunde (Runde 28, befund-uebersicht): RPC `finding.matrix`, Seite `GET /werke/{id}/befunde` und
//! Formulare `POST /werke/befunde/act`.
//!
//! - **Matrix** ([`matrix`]): Zeilen sind Kapitel (ein Kapitel samt seinen Abschnitten; Abschnitte ohne Kapitel
//!   stehen für sich, ein Teil mit eigenen Befunden bekommt eine Zeile „Teil selbst“), gruppiert nach Teilen
//!   (Summen je Teil), Spalten die Personas mit Befunden (Reihenfolge der Personas des Werks, dann alphabetisch).
//!   Je Zelle die Zahl je Status; Lob zählt nur in `praise` (und nur mit `include_praise`), angehängte Befunde
//!   (`similar_to`) nie. Ohne Filter ist `open` je Zeile genau die Summe der Abzeichen (Zählregel Vertrag §1.5,
//!   `open_badge`). Befunde ohne Abschnitt (Fäden, gelöschte Abschnitte) stehen in der Gruppe „Ohne Abschnitt“.
//! - **Seite**: Matrix mit Filtern (GET-Parameter wie bei `finding.matrix`), Zelle per `?key=&persona=` (Befunde
//!   des Kapitels samt Unterpunkten, angehängte unter dem Original), einzelner Befund per `?befund=` (Verlauf,
//!   angehängte, Ursprünge eines Sammelvorschlags), sonst die Liste „offen, älter als n Tagen“ (Vorgabe 14), die
//!   Sammelvorschläge und der Verlauf je Kapitel (eingeflossen in welche Fassungen, verworfen, verschoben,
//!   erledigt). Alles ohne JavaScript; das Widget `findings` (web/src/findings.ts) filtert ohne Neuladen.
//! - **Formulare**: `finding.set_status` und `finding.decide` gehen an [`super::rpc`] (dort Sperre und Commit);
//!   danach 303 auf `back` (nur eigene Adressen) bzw. den Befund; Fehler 200 mit Hinweis und dem Entwurf im
//!   Formular (nichts geht verloren).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::Work;
use baruch_store::{FindingFilter, StoreError, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use super::view::{card, entry_json, kind_sign, node_label, status_sign};
use super::{Finding, FindingKind, FindingStatus};
use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::view::{Settings, levels, query_escape};

pub const METHODS: &[&str] = &["finding.matrix"];

/// Vorgabe der Liste „offen, älter als n Tagen“ auf der Seite.
pub const DEFAULT_OLDER_DAYS: u32 = 14;
/// Größte Zahl der Tage für `older_than_days` (zehn Jahre).
pub const MAX_OLDER_DAYS: u32 = 3650;
/// Ab so vielen offenen Befunden gilt ein Kapitel als „viele offen“ (Hervorhebung in Matrix und Gedankenkarte).
pub const MANY_OPEN: usize = 3;

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

// ---- Filter --------------------------------------------------------------------------------------

/// Filter der Matrix und der Seite (GET-Parameter und RPC gleich benannt).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MatrixFilter {
    /// Leer = alle.
    pub status: Vec<FindingStatus>,
    /// Leer = alle (Lob nur mit `include_praise` oder ausdrücklich).
    pub kind: Vec<FindingKind>,
    pub persona: Option<String>,
    /// `created >= since` (ISO, z. B. `2026-11-01`).
    pub since: Option<String>,
    /// `created < until` (ISO).
    pub until: Option<String>,
    pub include_praise: bool,
    /// Nur Befunde, die älter als so viele Tage sind.
    pub older_than_days: Option<u32>,
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Statuswörter „open,adopted“ bzw. Liste; unbekannte → Fehlertext.
fn parse_statuses(words: &[String]) -> Result<Vec<FindingStatus>, String> {
    let mut out = Vec::new();
    for word in words.iter().flat_map(|w| w.split(',')).map(str::trim).filter(|w| !w.is_empty()) {
        let status = FindingStatus::from_db(word).ok_or_else(|| {
            format!("status: „open“, „adopted“, „rejected“, „deferred“ oder „done“ erwartet, nicht „{word}“")
        })?;
        if !out.contains(&status) {
            out.push(status);
        }
    }
    Ok(out)
}

fn parse_kinds(words: &[String]) -> Result<Vec<FindingKind>, String> {
    let mut out = Vec::new();
    for word in words.iter().flat_map(|w| w.split(',')).map(str::trim).filter(|w| !w.is_empty()) {
        let kind = FindingKind::from_db(word)
            .ok_or_else(|| format!("kind: „praise“, „hint“ oder „critical“ erwartet, nicht „{word}“"))?;
        if !out.contains(&kind) {
            out.push(kind);
        }
    }
    Ok(out)
}

/// Zeitangabe für `since`/`until`: nur Ziffern, `-`, `:`, `.`, `T`, `Z`, höchstens 30 Zeichen.
fn clean_time(value: &str) -> Option<String> {
    let value = value.trim();
    let ok = !value.is_empty()
        && value.len() <= 30
        && value.starts_with(|c: char| c.is_ascii_digit())
        && value.chars().all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | '.' | 'T' | 'Z'));
    ok.then(|| value.to_owned())
}

/// Persona-Handle: höchstens 200 Zeichen, ohne Steuerzeichen.
fn clean_persona(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.chars().count() <= 200 && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

impl MatrixFilter {
    /// Aus den RPC-Parametern; falsche Werte → `-32602`.
    pub fn from_params(params: &Json) -> Result<MatrixFilter, RpcError> {
        let words = |name: &str| -> Result<Vec<String>, RpcError> {
            match params.get(name) {
                None | Some(Json::Null) => Ok(Vec::new()),
                Some(Json::String(s)) => Ok(vec![s.clone()]),
                Some(Json::Array(list)) => list
                    .iter()
                    .map(|v| v.as_str().map(str::to_owned).ok_or_else(|| invalid(format!("{name}: Texte erwartet"))))
                    .collect(),
                Some(_) => Err(invalid(format!("{name}: Text oder Liste erwartet"))),
            }
        };
        let text = |name: &str| -> Result<Option<String>, RpcError> {
            match params.get(name) {
                None | Some(Json::Null) => Ok(None),
                Some(Json::String(s)) if s.trim().is_empty() => Ok(None),
                Some(Json::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
            }
        };
        let time = |name: &str| -> Result<Option<String>, RpcError> {
            text(name)?
                .map(|t| clean_time(&t).ok_or_else(|| invalid(format!("{name}: Zeit wie 2026-11-01 erwartet"))))
                .transpose()
        };
        let include_praise = match params.get("include_praise") {
            None | Some(Json::Null) => false,
            Some(Json::Bool(b)) => *b,
            Some(_) => return Err(invalid("include_praise: true oder false erwartet")),
        };
        let older_than_days = match params.get("older_than_days") {
            None | Some(Json::Null) => None,
            Some(Json::Number(n)) => match n.as_u64() {
                Some(days) if days <= u64::from(MAX_OLDER_DAYS) => Some(days as u32),
                _ => return Err(invalid(format!("older_than_days: 0 bis {MAX_OLDER_DAYS} erwartet"))),
            },
            Some(_) => return Err(invalid("older_than_days: Zahl erwartet")),
        };
        Ok(MatrixFilter {
            status: parse_statuses(&words("status")?).map_err(invalid)?,
            kind: parse_kinds(&words("kind")?).map_err(invalid)?,
            persona: text("persona")?
                .map(|p| clean_persona(&p).ok_or_else(|| invalid("persona: ungültig")))
                .transpose()?,
            since: time("since")?,
            until: time("until")?,
            include_praise,
            older_than_days,
        })
    }

    /// Aus den Paaren der Adresse; Ungültiges fällt weg (wie bei den übrigen Seiten).
    pub fn from_pairs(pairs: &[(String, String)]) -> MatrixFilter {
        let mut filter = MatrixFilter::default();
        let mut statuses = Vec::new();
        let mut kinds = Vec::new();
        for (name, value) in pairs {
            match name.as_str() {
                "status" => statuses.push(value.clone()),
                "kind" => kinds.push(value.clone()),
                "persona" => filter.persona = clean_persona(value),
                "since" => filter.since = clean_time(value),
                "until" => filter.until = clean_time(value),
                "include_praise" => filter.include_praise = matches!(value.trim(), "1" | "true" | "on"),
                "older_than_days" => {
                    filter.older_than_days = value.trim().parse().ok().filter(|d| *d <= MAX_OLDER_DAYS)
                }
                _ => {}
            }
        }
        // Einzeln gültige Wörter übernehmen, ungültige übergehen.
        filter.status =
            statuses.iter().filter_map(|s| parse_statuses(std::slice::from_ref(s)).ok()).flatten().collect();
        filter.status.dedup();
        filter.kind = kinds.iter().filter_map(|k| parse_kinds(std::slice::from_ref(k)).ok()).flatten().collect();
        filter.kind.dedup();
        filter
    }

    /// Kanonischer Abfrageteil (ohne `?`, prozentkodiert, leer ohne Filter).
    pub fn query(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for status in &self.status {
            parts.push(format!("status={}", status.as_db()));
        }
        for kind in &self.kind {
            parts.push(format!("kind={}", kind.as_db()));
        }
        if let Some(persona) = &self.persona {
            parts.push(format!("persona={}", query_escape(persona)));
        }
        if let Some(since) = &self.since {
            parts.push(format!("since={}", query_escape(since)));
        }
        if let Some(until) = &self.until {
            parts.push(format!("until={}", query_escape(until)));
        }
        if self.include_praise {
            parts.push("include_praise=1".to_owned());
        }
        if let Some(days) = self.older_than_days {
            parts.push(format!("older_than_days={days}"));
        }
        parts.join("&")
    }

    /// Gibt es überhaupt einen Filter?
    pub fn is_empty(&self) -> bool {
        self.query().is_empty()
    }

    /// Auswahl für den Store: `keys` (leer = alle), angehängte nie.
    fn store_filter(&self, keys: Vec<i64>, persona: Option<&str>) -> FindingFilter {
        let cutoff = self.older_than_days.map(cutoff_days);
        let until = match (&self.until, cutoff) {
            (Some(u), Some(c)) => Some(if *u < c { u.clone() } else { c }),
            (u, c) => u.clone().or(c),
        };
        FindingFilter {
            keys,
            statuses: self.status.clone(),
            kinds: self.kind.clone(),
            persona: persona.map(str::to_owned).or_else(|| self.persona.clone()),
            since: self.since.clone(),
            until,
            include_praise: self.include_praise,
            include_attached: false,
            limit: Some(super::view::VIEW_LIMIT),
            ..FindingFilter::default()
        }
    }
}

/// Zeitpunkt vor `days` Tagen (UTC, Form wie `created`: `2026-11-09T10:11:12.123Z`).
pub fn cutoff_days(days: u32) -> String {
    let at = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
    at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

// ---- Matrix --------------------------------------------------------------------------------------

/// Zahlen einer Zelle bzw. Summe: je Status (ohne Lob), Lob und alle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub open: usize,
    pub adopted: usize,
    pub rejected: usize,
    pub deferred: usize,
    pub done: usize,
    /// Lob (jeder Status; nur mit `include_praise` gezählt).
    pub praise: usize,
    pub total: usize,
    /// Runde 30b: Befunde ohne Lob mit mindestens einer Lösung bzw. ohne Lösung.
    pub solved: usize,
    pub unsolved: usize,
}

impl Counts {
    fn add(&mut self, finding: &Finding) {
        self.total += 1;
        if finding.kind == FindingKind::Praise {
            self.praise += 1;
            return;
        }
        if finding.solutions.is_empty() {
            self.unsolved += 1;
        } else {
            self.solved += 1;
        }
        match finding.status {
            // Runde 23a (PM-Entscheidung): Ein offener Sammelbefund zählt nicht als „offen“ – seine Ursprünge zählen
            // schon. So bleibt „offen“ je Zeile die Summe der Abzeichen (`finding_open_counts`); in `total` und in der
            // Liste „Sammelvorschläge“ steht er weiter.
            FindingStatus::Open if !finding.summary_of.is_empty() => {}
            FindingStatus::Open => self.open += 1,
            FindingStatus::Adopted => self.adopted += 1,
            FindingStatus::Rejected => self.rejected += 1,
            FindingStatus::Deferred => self.deferred += 1,
            FindingStatus::Done => self.done += 1,
        }
    }

    fn plus(&mut self, other: &Counts) {
        self.open += other.open;
        self.adopted += other.adopted;
        self.rejected += other.rejected;
        self.deferred += other.deferred;
        self.done += other.done;
        self.praise += other.praise;
        self.total += other.total;
        self.solved += other.solved;
        self.unsolved += other.unsolved;
    }

    /// Runde 30b: „mit Lösung / ohne Lösung“ in Worten, z. B. „2 mit Lösung, 1 ohne Lösung“; ohne Befunde (ohne Lob)
    /// leer.
    pub fn solution_words(&self) -> String {
        if self.solved + self.unsolved == 0 {
            return String::new();
        }
        format!("{} mit Lösung, {} ohne Lösung", self.solved, self.unsolved)
    }

    /// Zahl zu einem Status.
    pub fn of(&self, status: FindingStatus) -> usize {
        match status {
            FindingStatus::Open => self.open,
            FindingStatus::Adopted => self.adopted,
            FindingStatus::Rejected => self.rejected,
            FindingStatus::Deferred => self.deferred,
            FindingStatus::Done => self.done,
        }
    }

    /// Zeichen und Zahl je vorhandenem Status, z. B. „○2 ✓1“ (Lob „+1“); ohne Befunde „–“.
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = FindingStatus::ALL
            .into_iter()
            .filter(|s| self.of(*s) > 0)
            .map(|s| format!("{}{}", status_sign(s), self.of(s)))
            .collect();
        if self.praise > 0 {
            parts.push(format!("{}{}", kind_sign(FindingKind::Praise), self.praise));
        }
        if parts.is_empty() { "–".to_owned() } else { parts.join(" ") }
    }

    /// In Worten, z. B. „2 offen, 1 übernommen“; ohne Befunde „keine Befunde“.
    pub fn words(&self) -> String {
        let mut parts: Vec<String> = FindingStatus::ALL
            .into_iter()
            .filter(|s| self.of(*s) > 0)
            .map(|s| format!("{} {}", self.of(s), s.label()))
            .collect();
        if self.praise > 0 {
            parts.push(format!("{} Lob", self.praise));
        }
        if parts.is_empty() { "keine Befunde".to_owned() } else { parts.join(", ") }
    }
}

/// Eine Zeile der Matrix: ein Kapitel samt Unterpunkten (bzw. ein Teil selbst, ein Abschnitt ohne Kapitel, „ohne
/// Abschnitt“).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatrixRow {
    /// Fester Schlüssel des Kapitels; `None` = ohne Abschnitt.
    pub key: Option<i64>,
    pub number: String,
    pub title: String,
    /// Ebene (Teil 0, Kapitel 1, …).
    pub level: u8,
    /// Zeile für den Teil selbst (Befunde am Teil oder an Abschnitten direkt darunter).
    pub part: bool,
    /// Schlüssel des Kapitels und aller Unterpunkte.
    pub keys: Vec<i64>,
    /// Je Persona (Reihenfolge von [`Matrix::personas`]).
    pub cells: Vec<Counts>,
    pub total: Counts,
    /// Offene Befunde nach den Abzeichen (Zählregel §1.5, ohne Filter), Summe über `keys`.
    pub open_badge: usize,
}

/// Eine Gruppe (ein Teil, „ohne Teil“ bzw. „ohne Abschnitt“) mit Summen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatrixGroup {
    /// „part“, „none“ (Kapitel vor bzw. ohne Teil) oder „unplaced“ (ohne Abschnitt).
    pub kind: &'static str,
    pub key: Option<i64>,
    pub number: String,
    pub title: String,
    pub rows: Vec<MatrixRow>,
    pub totals: Vec<Counts>,
    pub total: Counts,
    pub open_badge: usize,
}

/// Verlauf eines Kapitels: entschiedene Befunde und die Fassungen, in die sie eingeflossen sind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryRow {
    pub key: i64,
    pub label: String,
    pub total: Counts,
    /// Je Abschnitt und Fassung die übernommenen Befunde (ids aufsteigend).
    pub revisions: Vec<AdoptedIn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdoptedIn {
    pub key: i64,
    pub label: String,
    pub rev: u32,
    pub findings: Vec<i64>,
    /// Runde 22: je Befund (gleiche Reihenfolge wie `findings`) der Stand der Fortschreibung beim Übernehmen; 0 = keiner.
    pub stands: Vec<u32>,
}

/// Die ganze Matrix eines Werks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Matrix {
    pub work: WorkId,
    pub title: String,
    pub personas: Vec<String>,
    pub groups: Vec<MatrixGroup>,
    pub totals: Vec<Counts>,
    pub total: Counts,
    /// Abzeichen: `node.id` → offene Befunde (nur der Abschnitt selbst, ohne Filter).
    pub open_by_key: BTreeMap<i64, usize>,
    /// Summe aller Abzeichen (wie `menu.badges` → `open_findings`).
    pub open_findings: usize,
    pub history: Vec<HistoryRow>,
    /// Angewandter Filter.
    pub filter: MatrixFilter,
    /// Gekappt: Zahl aller Befunde im Filter, wenn die Zellen nur die neuesten [`super::view::VIEW_LIMIT`] zählen
    /// (`None` = vollständig). Die Abzeichen zählen immer alle.
    pub truncated: Option<usize>,
}

/// Kapitel-Einheit einer Stelle: der Punkt selbst, wenn er Kapitel oder Teil ist, sonst der nächste Vorfahre der
/// Ebene ≤ 1; ohne solchen der oberste Vorfahre.
fn unit_of(work: &Work, levels: &[u8], seq: usize) -> usize {
    let mut at = seq;
    let mut steps = 0;
    while levels[at] > 1 && steps <= work.nodes.len() {
        match work.nodes[at].parent {
            Some(parent) if parent < at => at = parent,
            _ => break,
        }
        steps += 1;
    }
    at
}

/// Teil einer Stelle (nächster Vorfahre bzw. sie selbst auf Ebene 0).
fn part_of(work: &Work, levels: &[u8], seq: usize) -> Option<usize> {
    let mut at = Some(seq);
    let mut steps = 0;
    while let Some(i) = at {
        if levels[i] == 0 {
            return Some(i);
        }
        at = work.nodes[i].parent.filter(|&p| p < i);
        steps += 1;
        if steps > work.nodes.len() {
            break;
        }
    }
    None
}

/// Schlüssel einer Stelle und aller Unterpunkte.
pub fn subtree_keys(work: &Work, seq: usize) -> Vec<i64> {
    let mut keys = vec![work.nodes[seq].key];
    let mut inside: HashSet<usize> = HashSet::from([seq]);
    for node in work.nodes.iter().skip(seq + 1) {
        match node.parent {
            Some(p) if inside.contains(&p) => {
                inside.insert(node.id);
                keys.push(node.key);
            }
            _ => {}
        }
    }
    keys
}

/// Spalten: Personas mit Befunden, in der Reihenfolge der Personas des Werks, dann alphabetisch.
fn persona_columns(work: &Work, findings: &[Finding]) -> Vec<String> {
    let present: HashSet<&str> = findings.iter().map(|f| f.persona.as_str()).collect();
    let mut out: Vec<String> =
        work.personas.iter().map(|p| p.handle.clone()).filter(|h| present.contains(h.as_str())).collect();
    let mut rest: Vec<String> =
        present.into_iter().filter(|p| !out.iter().any(|o| o == p)).map(str::to_owned).collect();
    rest.sort();
    out.extend(rest);
    out
}

/// Setzt die Matrix zusammen (siehe Moduldoku).
pub async fn matrix(state: &AppState, work_id: WorkId, filter: &MatrixFilter) -> Result<Matrix, RpcError> {
    let work = state.store.load_work(work_id).await?;
    let open_by_key = state.store.finding_open_counts(work_id).await?;
    let store_filter = filter.store_filter(Vec::new(), None);
    let findings = state.store.finding_list(work_id, &store_filter).await?;
    let count = state.store.finding_count(work_id, &store_filter).await?;
    let mut matrix =
        build_matrix(work_id, &work, &findings, &open_by_key, filter, &adopted_map(state, work_id, &findings).await?);
    matrix.truncated = (count > findings.len()).then_some(count);
    Ok(matrix)
}

/// Je Abschnitt mit übernommenen Befunden die Fassungen ([`baruch_store::Store::findings_adopted_in`]).
async fn adopted_map(
    state: &AppState,
    work: WorkId,
    findings: &[Finding],
) -> Result<HashMap<i64, Vec<(u32, Vec<i64>)>>, RpcError> {
    let mut keys: Vec<i64> =
        findings.iter().filter(|f| f.status == FindingStatus::Adopted).filter_map(|f| f.key).collect();
    keys.sort_unstable();
    keys.dedup();
    let mut out = HashMap::new();
    for key in keys {
        out.insert(key, state.store.findings_adopted_in(work, key).await?);
    }
    Ok(out)
}

/// Höchstzahl der Befunde in der Liste „Offen, älter als n Tage“; gibt es mehr, zeigt die Seite die neuesten davon und
/// einen Hinweis auf die Kappung (Runde 23a).
pub const OLD_LIMIT: u32 = 200;

/// Reiner Aufbau der Matrix aus geladenen Daten (testbar ohne Server).
pub fn build_matrix(
    work_id: WorkId,
    work: &Work,
    findings: &[Finding],
    open_by_key: &HashMap<i64, usize>,
    filter: &MatrixFilter,
    adopted: &HashMap<i64, Vec<(u32, Vec<i64>)>>,
) -> Matrix {
    let levels = levels(work);
    let personas = persona_columns(work, findings);
    let column: HashMap<&str, usize> = personas.iter().enumerate().map(|(i, p)| (p.as_str(), i)).collect();
    let seq_of_key: HashMap<i64, usize> = work.nodes.iter().map(|n| (n.key, n.id)).collect();
    let empty = || vec![Counts::default(); personas.len()];

    // Zeilen je Einheit in Baumreihenfolge, Gruppen nach Teil.
    let mut groups: Vec<MatrixGroup> = Vec::new();
    let mut group_of_part: HashMap<Option<usize>, usize> = HashMap::new();
    let mut row_of_unit: HashMap<usize, (usize, usize)> = HashMap::new();
    for node in &work.nodes {
        if unit_of(work, &levels, node.id) != node.id {
            continue;
        }
        let part = part_of(work, &levels, node.id);
        let g = *group_of_part.entry(part).or_insert_with(|| {
            let (kind, key, number, title) = match part {
                Some(p) => ("part", Some(work.nodes[p].key), work.nodes[p].number.clone(), work.nodes[p].title.clone()),
                None => ("none", None, String::new(), "Ohne Teil".to_owned()),
            };
            groups.push(MatrixGroup {
                kind,
                key,
                number,
                title,
                rows: Vec::new(),
                totals: empty(),
                total: Counts::default(),
                open_badge: 0,
            });
            groups.len() - 1
        });
        let keys = if levels[node.id] == 0 {
            // Teil selbst: der Teil und die Punkte darunter, die keinem Kapitel gehören.
            subtree_keys(work, node.id)
                .into_iter()
                .filter(|k| seq_of_key.get(k).is_some_and(|&s| unit_of(work, &levels, s) == node.id))
                .collect()
        } else {
            subtree_keys(work, node.id)
        };
        let open_badge = keys.iter().map(|k| open_by_key.get(k).copied().unwrap_or(0)).sum();
        groups[g].rows.push(MatrixRow {
            key: Some(node.key),
            number: node.number.clone(),
            title: if levels[node.id] == 0 { format!("{} (Teil selbst)", node.title) } else { node.title.clone() },
            level: levels[node.id],
            part: levels[node.id] == 0,
            keys,
            cells: empty(),
            total: Counts::default(),
            open_badge,
        });
        row_of_unit.insert(node.id, (g, groups[g].rows.len() - 1));
    }
    let mut unplaced = MatrixRow {
        key: None,
        number: String::new(),
        title: "Ohne Abschnitt (Fäden, gelöschte Abschnitte)".to_owned(),
        level: 1,
        part: false,
        keys: Vec::new(),
        cells: empty(),
        total: Counts::default(),
        open_badge: 0,
    };
    for finding in findings {
        let Some(&col) = column.get(finding.persona.as_str()) else { continue };
        let place = finding
            .key
            .and_then(|k| seq_of_key.get(&k))
            .and_then(|&seq| row_of_unit.get(&unit_of(work, &levels, seq)).copied());
        let row = match place {
            Some((g, r)) => &mut groups[g].rows[r],
            None => &mut unplaced,
        };
        row.cells[col].add(finding);
        row.total.add(finding);
    }
    // Teil-Zeilen ohne Befunde fallen weg; Summen je Gruppe.
    for group in &mut groups {
        group.rows.retain(|r| !r.part || r.total.total > 0 || r.open_badge > 0);
        for row in &group.rows {
            for (sum, cell) in group.totals.iter_mut().zip(&row.cells) {
                sum.plus(cell);
            }
            group.total.plus(&row.total);
            group.open_badge += row.open_badge;
        }
    }
    groups.retain(|g| !g.rows.is_empty());
    if unplaced.total.total > 0 {
        let mut totals = empty();
        for (sum, cell) in totals.iter_mut().zip(&unplaced.cells) {
            sum.plus(cell);
        }
        let total = unplaced.total;
        groups.push(MatrixGroup {
            kind: "unplaced",
            key: None,
            number: String::new(),
            title: "Ohne Abschnitt".to_owned(),
            rows: vec![unplaced],
            totals,
            total,
            open_badge: 0,
        });
    }
    let mut totals = empty();
    let mut total = Counts::default();
    for group in &groups {
        for (sum, cell) in totals.iter_mut().zip(&group.totals) {
            sum.plus(cell);
        }
        total.plus(&group.total);
    }

    // Verlauf je Kapitel: Zeilen mit entschiedenen Befunden, dazu die Fassungen (nur gezeigte Befunde).
    let shown: HashSet<i64> = findings.iter().map(|f| f.id).collect();
    let stand_of: HashMap<i64, u32> = findings.iter().map(|f| (f.id, f.adopted_amend.unwrap_or(0))).collect();
    let mut history = Vec::new();
    for row in groups.iter().flat_map(|g| &g.rows) {
        let decided = row.total.adopted + row.total.rejected + row.total.deferred + row.total.done;
        let Some(key) = row.key else { continue };
        if decided == 0 {
            continue;
        }
        let mut revisions = Vec::new();
        for k in &row.keys {
            for (rev, ids) in adopted.get(k).map(Vec::as_slice).unwrap_or(&[]) {
                let ids: Vec<i64> = ids.iter().copied().filter(|id| shown.contains(id)).collect();
                if !ids.is_empty() {
                    let stands = ids.iter().map(|id| stand_of.get(id).copied().unwrap_or(0)).collect();
                    revisions.push(AdoptedIn {
                        key: *k,
                        label: node_label(work, *k).unwrap_or_default(),
                        rev: *rev,
                        findings: ids,
                        stands,
                    });
                }
            }
        }
        let label = if row.number.is_empty() { row.title.clone() } else { format!("{} {}", row.number, row.title) };
        history.push(HistoryRow { key, label, total: row.total, revisions });
    }
    Matrix {
        work: work_id,
        title: work.title.clone(),
        personas,
        groups,
        totals,
        total,
        open_findings: open_by_key.values().sum(),
        open_by_key: open_by_key.iter().map(|(k, v)| (*k, *v)).collect(),
        history,
        filter: filter.clone(),
        truncated: None,
    }
}

/// RPC `finding.matrix {work, status?, kind?, persona?, since?, until?, include_praise?, older_than_days?}` →
/// [`Matrix`] als JSON; fehlt `work`, das Vorgabewerk.
pub async fn call(state: &AppState, method: &str, params: Json) -> Result<Json, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    if !params.is_object() {
        return Err(invalid("Parameter als Objekt erwartet"));
    }
    match method {
        "finding.matrix" => {
            let work = crate::rpc::work_param(state, &params).await?;
            let filter = MatrixFilter::from_params(&params)?;
            let matrix = matrix(state, work, &filter).await?;
            serde_json::to_value(matrix).map_err(|e| RpcError::new(INTERNAL, e.to_string()))
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {other}"))),
    }
}

// ---- Seite ---------------------------------------------------------------------------------------

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("befunde.html", include_str!("../../templates/befunde.html")),
            ("fragments/findings_panel.html", include_str!("../../templates/fragments/findings_panel.html")),
            // Runde 30 (Schnittstelle R30-S1): Befund-Block und Lösungen eines Beitrags.
            ("fragments/finding_block.html", include_str!("../../templates/fragments/finding_block.html")),
            ("fragments/post_solutions.html", include_str!("../../templates/fragments/post_solutions.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Befunde fehlerhaft");
        }
        env
    })
}

/// Selbst gebaute Adresse als sicherer Wert für `href` (`&` maskiert); mit unsicherem Zeichen `#`.
fn href(url: String) -> Value {
    if url.chars().any(|c| matches!(c, '"' | '<' | '>' | '\'' | ' ') || c.is_control()) {
        return Value::from_safe_string("#".to_owned());
    }
    Value::from_safe_string(url.replace('&', "&amp;"))
}

fn with_query(path: &str, query: &str) -> String {
    if query.is_empty() { path.to_owned() } else { format!("{path}?{query}") }
}

/// Rücksprung nur auf eigene Adressen: beginnt mit genau einem `/`, ohne Steuerzeichen, Leerraum und Anführung.
pub fn safe_back(back: Option<&str>) -> Option<String> {
    let back = back?.trim();
    let ok = back.starts_with('/')
        && !back.starts_with("//")
        && !back.starts_with("/\\")
        && back.len() <= 2000
        && !back.chars().any(|c| c.is_control() || matches!(c, ' ' | '"' | '<' | '>' | '\'' | '\\'));
    ok.then(|| back.to_owned())
}

/// Ziele für „verschoben nach Abschnitt“: alle Punkte des Werks, eingerückt nach Ebene.
pub fn defer_targets(work: &Work) -> Vec<Json> {
    let levels = levels(work);
    work.nodes
        .iter()
        .map(|n| {
            let indent = "\u{a0}\u{a0}".repeat(usize::from(levels[n.id]));
            json!({ "key": n.key, "label": format!("{indent}{}", node_label(work, n.key).unwrap_or_default()) })
        })
        .collect()
}

/// Entwurf eines gescheiterten Formulars (bleibt im Formular stehen).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Draft {
    pub id: Option<i64>,
    pub status: String,
    pub reason: String,
    pub rev: String,
    pub target_key: String,
    pub target_idea: String,
}

/// Was die Seite außer dem Filter trägt.
#[derive(Debug, Clone, Default)]
struct PageView {
    key: Option<i64>,
    persona: Option<String>,
    befund: Option<i64>,
    ok: Option<String>,
}

fn page_view(pairs: &[(String, String)]) -> PageView {
    let mut view = PageView::default();
    for (name, value) in pairs {
        match name.as_str() {
            "key" => view.key = value.trim().parse().ok(),
            "persona" => view.persona = clean_persona(value),
            "befund" => view.befund = value.trim().parse().ok(),
            "ok" => view.ok = Some(value.chars().filter(|c| c.is_ascii_alphanumeric()).take(20).collect()),
            _ => {}
        }
    }
    view
}

fn notice_of(ok: &str) -> &'static str {
    match ok {
        "status" => "Status gespeichert (ein Commit im Spiegel).",
        "entscheidung" => "Entscheidung gespeichert (ein Commit im Spiegel).",
        _ => "Gespeichert.",
    }
}

fn plain(status: StatusCode, text: impl Into<String>) -> Response {
    (status, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text.into()).into_response()
}

fn store_error(e: StoreError) -> Response {
    match e {
        StoreError::UnknownWork(_) => plain(StatusCode::NOT_FOUND, "unbekanntes Werk"),
        e => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")),
    }
}

/// Karten zu Befunden, angehängte nur unter ihrem Original (stehen sie ohne Original in der Liste, für sich).
fn cards(work_id: WorkId, work: &Work, list: &[Finding], all: &[Finding]) -> Vec<Json> {
    let ids: HashSet<i64> = list.iter().map(|f| f.id).collect();
    list.iter()
        .filter(|f| f.similar_to.is_none_or(|o| !ids.contains(&o)))
        .map(|f| card(work_id, work, f, all))
        .collect()
}

/// Matrix für die Vorlage: je Zelle Zeichen, Worte, Adresse und Stufe der Hervorhebung.
fn matrix_view(work_id: WorkId, matrix: &Matrix, filter: &MatrixFilter, view: &PageView) -> Json {
    let base = format!("/werke/{work_id}/befunde");
    let cell_url = |key: Option<i64>, persona: Option<&str>| -> String {
        let mut f = filter.clone();
        f.persona = persona.map(str::to_owned).or_else(|| filter.persona.clone());
        let mut query = f.query();
        if let Some(key) = key {
            query = if query.is_empty() { format!("key={key}") } else { format!("{query}&key={key}") };
        }
        format!("{}#zelle", with_query(&base, &query))
    };
    let cell = |c: &Counts, key: Option<i64>, persona: Option<&str>, selected: bool| -> Json {
        let heat = match c.open {
            0 => 0,
            n if n >= MANY_OPEN => 3,
            n => n.min(2),
        };
        json!({ "label": c.label(), "words": c.words(), "open": c.open, "total": c.total, "heat": heat,
                "solved": c.solved, "unsolved": c.unsolved, "solution_words": c.solution_words(),
                "url": (c.total > 0 && key.is_some()).then(|| cell_url(key, persona)), "selected": selected })
    };
    let groups: Vec<Json> = matrix
        .groups
        .iter()
        .map(|g| {
            let rows: Vec<Json> = g
                .rows
                .iter()
                .map(|r| {
                    let in_row = view.key.is_some_and(|k| r.keys.contains(&k));
                    let cells: Vec<Json> = r
                        .cells
                        .iter()
                        .zip(&matrix.personas)
                        .map(|(c, p)| cell(c, r.key, Some(p), in_row && view.persona.as_deref() == Some(p.as_str())))
                        .collect();
                    json!({ "key": r.key, "number": r.number, "title": r.title, "level": r.level, "part": r.part,
                            "cells": cells, "total": cell(&r.total, r.key, None, in_row && view.persona.is_none()),
                            "open_badge": r.open_badge, "many": r.total.open >= MANY_OPEN,
                            "node_url": r.key.map(|k| crate::view::node_url(work_id, k, None, false, "", "")) })
                })
                .collect();
            let totals: Vec<Json> = g.totals.iter().map(|c| cell(c, None, None, false)).collect();
            json!({ "kind": g.kind, "key": g.key, "number": g.number, "title": g.title, "rows": rows,
                    "totals": totals, "total": cell(&g.total, None, None, false), "open_badge": g.open_badge })
        })
        .collect();
    let totals: Vec<Json> = matrix.totals.iter().map(|c| cell(c, None, None, false)).collect();
    json!({ "personas": matrix.personas, "groups": groups, "totals": totals,
            "total": cell(&matrix.total, None, None, false), "open_findings": matrix.open_findings,
            "truncated": matrix.truncated })
}

/// Die Seite (auch nach einem gescheiterten Formular mit `error` und `draft`).
async fn page_response(
    state: &AppState,
    headers: &HeaderMap,
    work_id: WorkId,
    pairs: &[(String, String)],
    error: Option<String>,
    draft: Option<Draft>,
) -> Response {
    let work = match state.store.load_work(work_id).await {
        Ok(work) => work,
        Err(e) => return store_error(e),
    };
    let filter = MatrixFilter::from_pairs(pairs);
    let mut view = page_view(pairs);
    if let Some(Draft { id: Some(id), .. }) = &draft {
        view.befund = Some(*id);
    }
    let matrix = match matrix(state, work_id, &filter).await {
        Ok(m) => m,
        Err(e) => return plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)),
    };
    // Alle Befunde (samt Lob und angehängten) für Ursprünge, angehängte und Sammelvorschläge.
    let all_filter = FindingFilter {
        include_praise: true,
        include_attached: true,
        limit: Some(super::view::VIEW_LIMIT),
        ..FindingFilter::default()
    };
    let all = match state.store.finding_list(work_id, &all_filter).await {
        Ok(all) => all,
        Err(e) => return store_error(e),
    };
    let base = format!("/werke/{work_id}/befunde");
    let query = filter.query();

    // Einzelner Befund.
    let mut detail = Json::Null;
    if let Some(id) = view.befund {
        match state.store.finding_get(work_id, id).await {
            Ok(finding) => {
                let history: Vec<Json> = state
                    .store
                    .finding_history(work_id, id)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .map(|e| entry_json(&work, e))
                    .collect();
                let similar = finding
                    .similar_to
                    .and_then(|o| all.iter().find(|f| f.id == o))
                    .map(|o| card(work_id, &work, o, &all));
                detail =
                    json!({ "card": card(work_id, &work, &finding, &all), "history": history, "similar": similar });
            }
            Err(StoreError::UnknownFinding { .. }) => {
                detail = json!({ "missing": id });
            }
            Err(e) => return store_error(e),
        }
    }

    // Zelle: Befunde des Kapitels samt Unterpunkten (und der Persona), angehängte unter dem Original.
    let mut cell = Json::Null;
    if let Some(key) = view.key {
        if let Some(node) = work.nodes.iter().find(|n| n.key == key) {
            let keys = subtree_keys(&work, node.id);
            let mut store_filter = filter.store_filter(keys, view.persona.as_deref());
            store_filter.include_attached = true;
            match state.store.finding_list(work_id, &store_filter).await {
                Ok(list) => {
                    cell = json!({
                        "key": key, "label": node_label(&work, key), "persona": view.persona,
                        "node_url": crate::view::node_url(work_id, key, None, false, "", ""),
                        "cards": cards(work_id, &work, &list, &all), "count": list.iter().filter(|f| f.similar_to.is_none()).count(),
                    });
                }
                Err(e) => return store_error(e),
            }
        } else {
            cell = json!({ "missing": key });
        }
    }

    // Übersicht: „offen, älter als n Tagen“ und Sammelvorschläge.
    let older_days = filter.older_than_days.unwrap_or(DEFAULT_OLDER_DAYS);
    let old_filter = FindingFilter {
        statuses: vec![FindingStatus::Open],
        until: Some(cutoff_days(older_days)),
        persona: filter.persona.clone(),
        limit: Some(OLD_LIMIT),
        ..FindingFilter::default()
    };
    // Runde 23a: Zahl ohne Grenze, damit die Seite eine Kappung der Liste nennt.
    let old_total = match state.store.finding_count(work_id, &old_filter).await {
        Ok(count) => count,
        Err(e) => return store_error(e),
    };
    let old: Vec<Json> = match state.store.finding_list(work_id, &old_filter).await {
        Ok(list) => list.iter().filter(|f| f.key.is_some()).map(|f| card(work_id, &work, f, &all)).collect(),
        Err(e) => return store_error(e),
    };
    let summaries: Vec<Json> =
        all.iter().filter(|f| !f.summary_of.is_empty()).map(|f| card(work_id, &work, f, &all)).collect();

    // Personas zur Auswahl: die des Werks und die mit Befunden.
    let mut personas: Vec<String> = work.personas.iter().map(|p| p.handle.clone()).collect();
    for f in &all {
        if !personas.contains(&f.persona) {
            personas.push(f.persona.clone());
        }
    }
    let here = {
        let mut q = query.clone();
        if let Some(key) = view.key {
            q = if q.is_empty() { format!("key={key}") } else { format!("{q}&key={key}") };
            if let Some(p) = &view.persona
                && filter.persona.as_deref() != Some(p.as_str())
            {
                q.push_str(&format!("&persona={}", query_escape(p)));
            }
        }
        if let Some(id) = view.befund {
            q = if q.is_empty() { format!("befund={id}") } else { format!("{q}&befund={id}") };
        }
        with_query(&base, &q)
    };
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let statuses: Vec<Json> = FindingStatus::ALL
        .into_iter()
        .map(|s| json!({ "value": s.as_db(), "label": s.label(), "sign": status_sign(s), "checked": filter.status.contains(&s) }))
        .collect();
    let kinds: Vec<Json> = FindingKind::ALL
        .into_iter()
        .map(|k| json!({ "value": k.as_db(), "label": k.label(), "sign": kind_sign(k), "checked": filter.kind.contains(&k) }))
        .collect();
    let ctx = context! {
        settings => Value::from(Serde(settings)),
        work => context! { id => work_id, title => work.title.as_str() },
        title => work.title.as_str(),
        base => href(base.clone()),
        here => here.as_str(),
        reset => href(base.clone()),
        workspace => href(format!("/?work={work_id}")),
        filter => Value::from(Serde(json!({
            "persona": filter.persona, "since": filter.since, "until": filter.until,
            "include_praise": filter.include_praise, "older_than_days": filter.older_than_days,
            "active": !filter.is_empty(), "query": query,
        }))),
        praise_toggle => href(with_query(&base, &{
            let mut f = filter.clone();
            f.include_praise = !filter.include_praise;
            f.query()
        })),
        statuses => Value::from(Serde(statuses)),
        kinds => Value::from(Serde(kinds)),
        personas => personas,
        matrix => Value::from(Serde(matrix_view(work_id, &matrix, &filter, &view))),
        history => Value::from(Serde(&matrix.history)),
        detail => Value::from(Serde(detail)),
        cell => Value::from(Serde(cell)),
        old => Value::from(Serde(old)),
        old_total => old_total,
        old_limit => OLD_LIMIT,
        older_days => older_days,
        summaries => Value::from(Serde(summaries)),
        targets => Value::from(Serde(defer_targets(&work))),
        many => MANY_OPEN,
        view_limit => super::view::VIEW_LIMIT,
        notice => view.ok.as_deref().map(notice_of),
        error => error,
        draft => Value::from(Serde(draft.unwrap_or_default())),
    };
    match templates().get_template("befunde.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (StatusCode::OK, Html(html)).into_response(),
        Err(e) => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage befunde.html: {e}")),
    }
}

/// `GET /werke/{id}/befunde`.
async fn findings_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    page_response(&state, &headers, work, &pairs, None, None).await
}

/// Formular an `POST /werke/befunde/act`: dieselben Felder wie die RPC-Methoden, alles als Text.
#[derive(Debug, Default, Deserialize)]
struct ActForm {
    method: String,
    work: Option<String>,
    id: Option<String>,
    entry: Option<String>,
    status: Option<String>,
    reason: Option<String>,
    rev: Option<String>,
    target_key: Option<String>,
    target_idea: Option<String>,
    confirm: Option<String>,
    back: Option<String>,
}

fn number(value: &Option<String>) -> Option<i64> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty()).and_then(|v| v.parse().ok())
}

/// Parameter für [`super::rpc::parse_change`]: leere Felder fallen weg, Zahlen als Zahlen.
pub fn change_params(
    work: WorkId,
    id: i64,
    form_status: &str,
    reason: &str,
    rev: Option<i64>,
    target_key: Option<i64>,
    target_idea: Option<i64>,
) -> Json {
    let mut params = json!({ "work": work, "id": id, "status": form_status.trim() });
    if !reason.trim().is_empty() {
        params["reason"] = json!(reason);
    }
    if let Some(rev) = rev {
        params["rev"] = json!(rev);
    }
    if let Some(key) = target_key {
        params["target_key"] = json!(key);
    }
    if let Some(idea) = target_idea {
        params["target_idea"] = json!(idea);
    }
    params
}

/// `POST /werke/befunde/act`: `method` = finding.set_status | finding.decide → [`super::rpc`]; danach 303 auf
/// `back` bzw. den Befund, Fehler 200 mit Hinweis und Entwurf.
async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    if !matches!(form.method.as_str(), "finding.set_status" | "finding.decide") {
        return plain(StatusCode::BAD_REQUEST, "unbekannte Aktion");
    }
    let Some(work) = number(&form.work) else {
        return plain(StatusCode::BAD_REQUEST, "work fehlt (Werk-id)");
    };
    if let Err(e) = state.store.work_slug(work).await {
        return store_error(e);
    }
    let id = number(&form.id);
    let draft = Draft {
        id,
        status: form.status.clone().unwrap_or_default(),
        reason: form.reason.clone().unwrap_or_default(),
        rev: form.rev.clone().unwrap_or_default(),
        target_key: form.target_key.clone().unwrap_or_default(),
        target_idea: form.target_idea.clone().unwrap_or_default(),
    };
    let result = match form.method.as_str() {
        "finding.set_status" => match id {
            None => Err(invalid("id fehlt (Befund)")),
            Some(id) => {
                let params = change_params(
                    work,
                    id,
                    form.status.as_deref().unwrap_or(""),
                    form.reason.as_deref().unwrap_or(""),
                    number(&form.rev),
                    number(&form.target_key),
                    number(&form.target_idea),
                );
                match super::rpc::parse_change(&params) {
                    Ok(change) => super::rpc::set_status(&state, work, id, &change).await,
                    Err(e) => Err(e),
                }
            }
        },
        _ => match number(&form.entry) {
            None => Err(invalid("entry fehlt (Vorschlag)")),
            Some(entry) => {
                let confirm = matches!(form.confirm.as_deref().map(str::trim), Some("1" | "true" | "on"));
                super::rpc::decide(&state, work, entry, confirm).await
            }
        },
    };
    match result {
        Ok(answer) => {
            let target = answer["finding"]["id"].as_i64().or(id);
            let ok = if form.method == "finding.decide" { "entscheidung" } else { "status" };
            let url = match (safe_back(form.back.as_deref()), target) {
                (Some(back), _) => back,
                (None, Some(t)) => format!("/werke/{work}/befunde?befund={t}&ok={ok}"),
                (None, None) => format!("/werke/{work}/befunde?ok={ok}"),
            };
            Redirect::to(&url).into_response()
        }
        Err(e) if e.code == INTERNAL => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)),
        Err(e) => {
            // Fehler: dieselbe Seite mit Hinweis, der Entwurf bleibt im Formular (nichts geht verloren).
            let pairs: Vec<(String, String)> = Vec::new();
            page_response(&state, &headers, work, &pairs, Some(e.message), Some(draft)).await
        }
    }
}

/// Seiten (lesend).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/befunde", get(findings_page))
}

/// Formulare (im Router der schreibenden Routen, hinter `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/befunde/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_filters_read_from_the_address_drop_what_is_invalid() {
        let pairs: Vec<(String, String)> = [
            ("status", "open"),
            ("status", "rejected,quatsch"),
            ("kind", "critical"),
            ("persona", "@lektor"),
            ("since", "2026-11-01"),
            ("until", "<script>"),
            ("include_praise", "1"),
            ("older_than_days", "99999"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .collect();
        let filter = MatrixFilter::from_pairs(&pairs);
        assert_eq!(
            filter.status,
            vec![FindingStatus::Open],
            "„rejected,quatsch“ ist als Ganzes ungültig und fällt weg"
        );
        assert_eq!(filter.kind, vec![FindingKind::Critical], "Art übernommen");
        assert_eq!(filter.since.as_deref(), Some("2026-11-01"), "Datum übernommen");
        assert_eq!(filter.until, None, "fremde Zeichen in until fallen weg");
        assert!(filter.include_praise, "Schalter Lob");
        assert_eq!(filter.older_than_days, None, "mehr als zehn Jahre fällt weg");
        assert_eq!(
            filter.query(),
            "status=open&kind=critical&persona=%40lektor&since=2026-11-01&include_praise=1",
            "kanonischer Abfrageteil, prozentkodiert"
        );
        assert_eq!(MatrixFilter::from_pairs(&filter_pairs(&filter.query())), filter, "Rundreise über die Adresse");
    }

    fn filter_pairs(query: &str) -> Vec<(String, String)> {
        query.split('&').filter_map(|p| p.split_once('=')).map(|(a, b)| (a.to_owned(), b.replace("%40", "@"))).collect()
    }

    #[test]
    fn rpc_filters_refuse_unknown_words() {
        let bad = MatrixFilter::from_params(&json!({ "status": "offen" })).unwrap_err();
        assert_eq!(bad.code, INVALID_PARAMS, "unbekanntes Statuswort → -32602");
        let list = MatrixFilter::from_params(&json!({ "status": ["open", "done"], "kind": "hint,critical" })).unwrap();
        assert_eq!(list.status, vec![FindingStatus::Open, FindingStatus::Done], "Liste wie Komma-Text");
        assert_eq!(list.kind, vec![FindingKind::Hint, FindingKind::Critical], "Komma-Text");
        assert!(MatrixFilter::from_params(&json!({ "older_than_days": -1 })).is_err(), "negative Tage abgelehnt");
    }

    #[test]
    fn counts_show_sign_and_number_not_only_color() {
        let c = Counts { open: 2, adopted: 1, praise: 1, total: 4, ..Counts::default() };
        assert_eq!(c.label(), "○2 ✓1 +1", "Zeichen und Zahl je Status");
        assert_eq!(c.words(), "2 offen, 1 übernommen, 1 Lob", "in Worten für title und Bildschirmleser");
        assert_eq!(Counts::default().label(), "–", "leer");
        assert_eq!(safe_back(Some("//fremd.example/")), None, "fremde Ziele abgelehnt");
        assert_eq!(safe_back(Some("/?work=1&key=2")).as_deref(), Some("/?work=1&key=2"), "eigene Adresse bleibt");
    }
}
