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

pub const MODULES_DESCRIPTION: &str = "List SWORD Bible modules (read-only): module directories, per module name, \
description, versification (every scheme sword-modules knows), licence note of the conf (license, notice; \
DistributionLicense, Copyright), cipher (none|locked|unlocked: encrypted, key missing or stored – only the human \
stores the key), readable yes/no with reason, and the chosen Bible of the work (work_bible). Only the human can choose \
(Einstellungen → Quellen). Since Round 15 commentaries and lexicons are in the list too, per module with kind \
(bible, commentary, lexicon), plus work_sources (the commentaries and lexicons chosen for the work).";

pub const LOOKUP_DESCRIPTION: &str = "Look up a Bible reference in a SWORD module (read-only): ref like \
\"Joh 3,16\", \"Ps 23\" or \"1. Kor 13,4–7; 14,1\" (German notation, checked against the versification of the \
Schlachter 1951), module (default: the Bible of the work). Reading goes through the mapping of the German \
versification onto that of the module. Answer: bible (module, versification, licence note) and per reference status, \
message, verses (chapter, verse, text in the versification of the module; text null = verse missing in the module; \
german_ref = the German reference of the verse or german_problem if it cannot be mapped back, Round 19c), module_ref \
(reference read, e.g. \"Joel 2,28–32 (KJV)\", only if it differs), rules (rules applied), truncated (more than 400 \
verses); what cannot be mapped reports status versification_mismatch with reason. Plain text without footnotes, \
cross-references and headings. Do not copy Bible text into posts except the checked verse.";

pub const SEARCH_DESCRIPTION: &str = "Search a word or phrase in a SWORD module (read-only, linear search over all \
verses; case, punctuation and hyphens do not count): query, module (default: the Bible of the work), limit (default \
50, at most 200). Time limit 5 s; truncated reports a stop at the hit or time limit. Answer with bible (module, \
versification, licence note) and hits (ref, text).";

pub fn modules_schema() -> Value {
    json!({ "type": "object", "properties": { "work": work_schema() } })
}

pub fn lookup_schema() -> Value {
    json!({ "type": "object", "required": ["ref"], "properties": {
        "ref": { "type": "string", "description": "reference, e.g. \"Joh 3,16\" (at most 500 characters)" },
        "module": { "type": "string", "description": "module name (bible_modules); default the Bible of the work" },
        "work": work_schema() } })
}

pub fn search_schema() -> Value {
    json!({ "type": "object", "required": ["query"], "properties": {
        "query": { "type": "string", "description": "word or phrase (1–200 characters)" },
        "module": { "type": "string", "description": "module name; default the Bible of the work" },
        "limit": { "type": "integer", "minimum": 1, "maximum": 200 },
        "work": work_schema() } })
}

pub const COMMENTARY_LOOKUP_DESCRIPTION: &str = "Look up commentaries on a Bible reference (read-only, local SWORD \
modules): ref like \"Ps 139,23–24\" (German notation), module (default: the commentaries the human chose for the \
work, Einstellungen → Werk). Reading uses the versification of the commentary. Answer: per commentary module, \
description, versification, notice (licence note of the conf, name it with every quote) and entries (ref, text, \
truncated; plain text without markup, cut to 4000 characters) or problem.";

pub const LEXICON_LOOKUP_DESCRIPTION: &str = "Look up a headword in a SWORD lexicon (read-only, local): key \
(headword; case by the conf, Strong numbers like G25 or H430 work too), module (default: the first lexicon chosen for \
the work). If the headword is missing, Baruch (Round 19c) tries Strong notations (G26 ↔ G0026), simple endings, \
umlaut reduction (an umlaut back to its base vowel) and finally the last component of a compound \
(\"Feigenbaum\" → \"Baum\"). Answer: module, key (as in the module), found, text (plain text, cut to 4000 \
characters), notice (licence note of the conf, name it with every quote), truncated, probe (the form tried if not the \
headword itself matched, otherwise null), asked (the headword asked for).";

pub const LEXICON_SEARCH_DESCRIPTION: &str = "Search a word or phrase in headwords and articles of a SWORD lexicon \
(read-only, local, linear; case and punctuation do not count): query, module (default: the first lexicon chosen for \
the work), limit (default 50, at most 200). Headword hits first; time limit 5 s. Answer: module, hits (key, excerpt, \
in_key), truncated, notice (licence note of the conf).";

pub fn commentary_lookup_schema() -> Value {
    json!({ "type": "object", "required": ["work", "ref"], "properties": {
        "work": work_schema(),
        "ref": { "type": "string", "description": "reference, e.g. \"Ps 139,23–24\" (at most 500 characters)" },
        "module": { "type": "string", "description": "commentary (bible_modules, kind commentary); default the selection of the work" } } })
}

pub fn lexicon_lookup_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key"], "properties": {
        "work": work_schema(),
        "key": { "type": "string", "description": "Stichwort (1–200 Zeichen)" },
        "module": { "type": "string", "description": "lexicon (bible_modules, kind lexicon); default the first of the work" } } })
}

pub fn lexicon_search_schema() -> Value {
    json!({ "type": "object", "required": ["work", "query"], "properties": {
        "work": work_schema(),
        "query": { "type": "string", "description": "word or phrase (1–200 characters)" },
        "module": { "type": "string", "description": "lexicon; default the first of the work" },
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
            let reference = a.reference.ok_or_else(|| Failure::Args("ref is missing".into()))?;
            if reference.chars().count() > 500 {
                return Err(Failure::Tool("ref: at most 500 characters".into()));
            }
            Ok(bibles::lookup(state, &choice(state, work, &a.module).await?, &reference).await?)
        }
        "bible_search" => {
            let query = a.query.ok_or_else(|| Failure::Args("query is missing".into()))?;
            let limit = a.limit.map_or(bibles::SEARCH_LIMIT, |l| (l as usize).clamp(1, bibles::SEARCH_MAX));
            Ok(bibles::search(&choice(state, work, &a.module).await?, &query, limit).await?)
        }
        "commentary_lookup" => {
            let reference = a.reference.ok_or_else(|| Failure::Args("ref is missing".into()))?;
            if reference.trim().is_empty() || reference.chars().count() > 500 {
                return Err(Failure::Tool("ref: 1 to 500 characters".into()));
            }
            Ok(bibles::commentary_lookup(state, work, &reference, a.module.as_deref()).await?)
        }
        "lexicon_lookup" => {
            let key = a.key.ok_or_else(|| Failure::Args("key is missing".into()))?;
            Ok(bibles::lexicon_lookup(state, work, &key, a.module.as_deref()).await?)
        }
        "lexicon_search" => {
            let query = a.query.ok_or_else(|| Failure::Args("query is missing".into()))?;
            let limit = a.limit.map_or(bibles::SEARCH_LIMIT, |l| (l as usize).clamp(1, bibles::SEARCH_MAX));
            Ok(bibles::lexicon_search(state, work, &query, a.module.as_deref(), limit).await?)
        }
        _ => Err(Failure::Tool(format!("unknown tool {name}"))),
    }
}
