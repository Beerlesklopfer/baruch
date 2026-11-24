// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeuge der SWORD-Bibeln (2026-11-06): `bible_modules`, `bible_lookup`, `bible_search`. Nur lesend;
//! Modulordner und Bibel je Werk wählt nur der Mensch (Einstellungen → Quellen, RPC `bible.set_*`). Jede
//! Antwort nennt Modul, Versifikation und den Lizenzvermerk der conf. Dieselben Wege wie RPC `bible.*`
//! ([`crate::bibles`]).
//!
//! Runde 15 (Punkt 5a): `commentary_lookup`, `lexicon_lookup`, `lexicon_search` – Kommentare und Lexika (nur
//! lesend, lokale SWORD-Module; Auswahl je Werk trifft der Mensch unter Einstellungen → Werk). Jede Antwort
//! nennt den Lizenzvermerk der conf (`notice`).

use serde::Deserialize;
use serde_json::{Value, json};

use super::tools::{Failure, WorkRef, args, resolve, work_schema};
use crate::AppState;
use crate::bibles::{self, Choice};

pub const NAMES: &[&str] =
    &["bible_modules", "bible_lookup", "bible_search", "commentary_lookup", "lexicon_lookup", "lexicon_search"];

pub const MODULES_DESCRIPTION: &str = "SWORD-Bibelmodule auflisten (nur lesend): Modulordner, je Modul Name, \
Beschreibung, Versifikation (jede Zählung, die sword-modules kennt), Lizenzvermerk der conf (license, notice; \
DistributionLicense, Copyright), cipher (none|locked|unlocked: verschlüsselt, Schlüssel fehlt bzw. hinterlegt – den \
Schlüssel hinterlegt nur der Mensch), lesbar ja/nein mit Grund, und die gewählte Bibel des Werks (work_bible). Wählen kann nur der Mensch \
(Einstellungen → Quellen). Seit Runde 15 stehen auch Kommentare und Lexika in der Liste, je Modul mit kind \
(bible, commentary, lexicon), dazu work_sources (die für das Werk gewählten Kommentare und Lexika).";

pub const LOOKUP_DESCRIPTION: &str = "Bibelstelle in einem SWORD-Modul nachschlagen (nur lesend): ref wie \
„Joh 3,16“, „Ps 23“ oder „1. Kor 13,4–7; 14,1“ (deutsche Schreibweise, Prüfung gegen die Verszählung der \
Schlachter 1951), module (Vorgabe: die Bibel des Werks). Gelesen wird über die Abbildung der deutschen Zählung auf \
die des Moduls. Antwort: bible (Modul, Versifikation, Lizenzvermerk) und je Angabe status, message, verses (chapter, \
verse, text in der Zählung des Moduls; text null = Vers fehlt im Modul; german_ref = die deutsche Stelle des \
Verses bzw. german_problem, wenn er sich nicht zurückbilden lässt, Runde 19c), module_ref (gelesene Stelle, z. B. \
„Joel 2,28–32 (KJV)“, nur wenn sie abweicht), rules (angewandte Regeln), truncated (mehr als 400 Verse); nicht \
Abbildbares meldet status versification_mismatch mit Grund. Klartext ohne \
Fußnoten, Querverweise und Überschriften. Bibeltext nicht in Beiträge kopieren außer dem geprüften Vers.";

pub const SEARCH_DESCRIPTION: &str = "Wort oder Wortfolge in einem SWORD-Modul suchen (nur lesend, lineare \
Suche über alle Verse; Groß/Klein, Satzzeichen und Bindestriche zählen nicht): query, module (Vorgabe: die Bibel \
des Werks), limit (Vorgabe 50, höchstens 200). Zeitgrenze 5 s; truncated meldet einen Abbruch an Treffer- oder \
Zeitgrenze. Antwort mit bible (Modul, Versifikation, Lizenzvermerk) und hits (ref, text).";

pub fn modules_schema() -> Value {
    json!({ "type": "object", "properties": { "work": work_schema() } })
}

pub fn lookup_schema() -> Value {
    json!({ "type": "object", "required": ["ref"], "properties": {
        "ref": { "type": "string", "description": "Stellenangabe, z. B. „Joh 3,16“ (höchstens 500 Zeichen)" },
        "module": { "type": "string", "description": "Modulname (bible_modules); Vorgabe die Bibel des Werks" },
        "work": work_schema() } })
}

pub fn search_schema() -> Value {
    json!({ "type": "object", "required": ["query"], "properties": {
        "query": { "type": "string", "description": "Wort oder Wortfolge (1–200 Zeichen)" },
        "module": { "type": "string", "description": "Modulname; Vorgabe die Bibel des Werks" },
        "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
        "work": work_schema() } })
}

pub const COMMENTARY_LOOKUP_DESCRIPTION: &str = "Kommentare zu einer Bibelstelle nachschlagen (nur lesend, lokale \
SWORD-Module): ref wie „Ps 139,23–24“ (deutsche Schreibweise), module (Vorgabe: die Kommentare, die der Mensch für \
das Werk gewählt hat, Einstellungen → Werk). Gelesen wird in der Verszählung des Kommentars. Antwort: je Kommentar \
module, description, versification, notice (Lizenzvermerk der conf, bei jedem Zitat nennen) und entries (ref, text, \
truncated; Klartext ohne Auszeichnung, gekürzt auf 4000 Zeichen) bzw. problem.";

pub const LEXICON_LOOKUP_DESCRIPTION: &str = "Stichwort in einem SWORD-Lexikon nachschlagen (nur lesend, lokal): key \
(Stichwort; Groß-/Kleinschreibung nach der conf, Strong-Nummern wie G25 bzw. H430 gehen auch), module (Vorgabe: das \
erste für das Werk gewählte Lexikon). Fehlt das Stichwort, probiert Baruch (Runde 19c) Strong-Schreibweisen (G26 ↔ \
G0026), einfache Endungen, Umlaut-Rückführung („Bäume“ → „Baum“) und zuletzt den letzten Bestandteil eines \
Kompositums („Feigenbaum“ → „Baum“). Antwort: module, key (wie im Modul), found, text (Klartext, gekürzt auf 4000 \
Zeichen), notice (Lizenzvermerk der conf, bei jedem Zitat nennen), truncated, probe (die probierte Form, wenn nicht \
das Stichwort selbst passte, sonst null), asked (das gefragte Stichwort).";

pub const LEXICON_SEARCH_DESCRIPTION: &str = "Wort oder Wortfolge in Stichwörtern und Artikeln eines SWORD-Lexikons \
suchen (nur lesend, lokal, linear; Groß/Klein und Satzzeichen zählen nicht): query, module (Vorgabe: das erste für \
das Werk gewählte Lexikon), limit (Vorgabe 50, höchstens 200). Stichworttreffer zuerst; Zeitgrenze 5 s. Antwort: \
module, hits (key, excerpt, in_key), truncated, notice (Lizenzvermerk der conf).";

pub fn commentary_lookup_schema() -> Value {
    json!({ "type": "object", "required": ["work", "ref"], "properties": {
        "work": work_schema(),
        "ref": { "type": "string", "description": "Stellenangabe, z. B. „Ps 139,23–24“ (höchstens 500 Zeichen)" },
        "module": { "type": "string", "description": "Kommentar (bible_modules, kind commentary); Vorgabe die Auswahl des Werks" } } })
}

pub fn lexicon_lookup_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key"], "properties": {
        "work": work_schema(),
        "key": { "type": "string", "description": "Stichwort (1–200 Zeichen)" },
        "module": { "type": "string", "description": "Lexikon (bible_modules, kind lexicon); Vorgabe das erste des Werks" } } })
}

pub fn lexicon_search_schema() -> Value {
    json!({ "type": "object", "required": ["work", "query"], "properties": {
        "work": work_schema(),
        "query": { "type": "string", "description": "Wort oder Wortfolge (1–200 Zeichen)" },
        "module": { "type": "string", "description": "Lexikon; Vorgabe das erste des Werks" },
        "limit": { "type": "integer", "minimum": 1, "maximum": 200 } } })
}

#[derive(Deserialize)]
struct Args {
    work: Option<WorkRef>,
    module: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    query: Option<String>,
    limit: Option<u64>,
    key: Option<String>,
}

async fn work_of(state: &AppState, work: &Option<WorkRef>) -> Result<i64, Failure> {
    match work {
        Some(w) => Ok(resolve(&state.store, w).await?.0),
        None => Ok(state.default_work().await?),
    }
}

async fn choice(state: &AppState, work: i64, module: &Option<String>) -> Result<Choice, Failure> {
    Ok(match module.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        Some(name) => bibles::by_name(state, name).await?,
        None => bibles::of_work(state, work).await?,
    })
}

pub async fn call(state: &AppState, name: &str, arguments: Value) -> Result<Value, Failure> {
    let a: Args = args(arguments)?;
    let work = work_of(state, &a.work).await?;
    match name {
        "bible_modules" => Ok(bibles::modules_value(state, work).await?),
        "bible_lookup" => {
            let reference = a.reference.ok_or_else(|| Failure::Args("ref fehlt".into()))?;
            if reference.chars().count() > 500 {
                return Err(Failure::Tool("ref: höchstens 500 Zeichen".into()));
            }
            Ok(bibles::lookup(state, &choice(state, work, &a.module).await?, &reference).await?)
        }
        "bible_search" => {
            let query = a.query.ok_or_else(|| Failure::Args("query fehlt".into()))?;
            let limit = a.limit.map_or(bibles::SEARCH_LIMIT, |l| (l as usize).clamp(1, bibles::SEARCH_MAX));
            Ok(bibles::search(&choice(state, work, &a.module).await?, &query, limit).await?)
        }
        "commentary_lookup" => {
            let reference = a.reference.ok_or_else(|| Failure::Args("ref fehlt".into()))?;
            if reference.trim().is_empty() || reference.chars().count() > 500 {
                return Err(Failure::Tool("ref: 1 bis 500 Zeichen".into()));
            }
            Ok(bibles::commentary_lookup(state, work, &reference, a.module.as_deref()).await?)
        }
        "lexicon_lookup" => {
            let key = a.key.ok_or_else(|| Failure::Args("key fehlt".into()))?;
            Ok(bibles::lexicon_lookup(state, work, &key, a.module.as_deref()).await?)
        }
        "lexicon_search" => {
            let query = a.query.ok_or_else(|| Failure::Args("query fehlt".into()))?;
            let limit = a.limit.map_or(bibles::SEARCH_LIMIT, |l| (l as usize).clamp(1, bibles::SEARCH_MAX));
            Ok(bibles::lexicon_search(state, work, &query, a.module.as_deref(), limit).await?)
        }
        _ => Err(Failure::Tool(format!("unbekanntes Werkzeug {name}"))),
    }
}
