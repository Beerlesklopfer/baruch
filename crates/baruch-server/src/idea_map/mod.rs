// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gedankenkarte (Vertrag Runde 25): die Einfälle eines Werks als Karte um das Werk, seine Teile und Kapitel,
//! mit Verbindungen zwischen Einfällen, gespeicherten Lagen und (nur angezeigt) Nähe.
//!
//! - Daten und RPC (karte-daten, dieses Modul): [`build`] setzt die Karte zusammen; RPC `idea.map`, `idea.links`,
//!   `idea.link`, `idea.unlink`, `idea.link_restore`, `idea.map_pos_set`, `idea.map_pos_reset` ([`call`],
//!   `docs/RPC.md` „Gedankenkarte“). Herkunft über RPC immer der Mensch.
//! - Seite (karte-bild, [`bild`]): `/werke/{id}/karte` (HTML, ohne JavaScript bedienbar); Ausgaben `.svg`, `.pdf`,
//!   `.md`, `.mm` und Formular `POST /werke/{id}/karte/act` seit Runde 34 (R34-S1) in [`files`].
//! - Runde 34: [`tree`] – die Mindmap als Baum ([`tree::MindMap`], [`tree::tree`]), die Wolke daneben
//!   ([`tree::Cloud`], [`tree::cloud`]) und der Baum-Import ([`tree::import`]); RPC `idea.tree` → `{map, cloud}`.
//!   `idea.map` (flache Sicht) bleibt und nennt je Einfall zusätzlich `title`, `parent`, `position`, `main_topic`,
//!   `cloud`. [`bild::mindmap`] zeichnet die Mindmap.
//!
//! Aufbau (Vertrag §2): Mitte das Werk (`w`), darum Teile und Kapitel (`n:<key>`); Abschnitte unter einem Kapitel
//! fasst das Kapitel zusammen (Anker = nächster Vorfahre der Art Kapitel, sonst der Punkt selbst); Einfälle ohne
//! Bezug hängen an der Wolke „Ohne Kapitel“ (`none`). Einfälle ohne Werk erscheinen nicht.
//!
//! Untermodule (karte-daten): [`links`] schreibt Verbindungen mit Spiegel-Commit (auch für MCP), [`near`]
//! berechnet die Nähe aus gespeicherten Vektoren ([`crate::rag::neighbors`]) und gleichen Schlagwörtern. Lagen
//! sind reine Anzeige (kein Commit).
//!
//! Runde 19b: [`groups`] – Gruppen der Nähe (`idea.map_groups`), Namen durch einen Assistenten als Vorschlag nur im
//! Speicher (`idea.map_groups_name`), Übernehmen als Schlagwort durch den Menschen (`idea.map_group_accept`, nur RPC
//! und Formular, kein MCP-Werkzeug) und Verwerfen (`idea.map_group_dismiss`).

pub mod bild;
pub mod files;
pub mod groups;
pub mod links;
pub mod near;
pub mod tree;

use std::collections::{BTreeSet, HashMap, HashSet};

use baruch_core::NodeKind;
use baruch_store::{
    Idea, IdeaFilter, IdeaId, IdeaLink, IdeaOrigin, IdeaScope, IdeaStatus, LinkKind, MapPos, NewIdeaLink, WorkId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::{AppState, mirror};

/// Methoden dieses Moduls.
pub const METHODS: &[&str] = &[
    "idea.map",
    "idea.links",
    "idea.link",
    "idea.unlink",
    "idea.link_restore",
    "idea.map_pos_set",
    "idea.map_pos_reset",
    "idea.map_groups",
    "idea.map_groups_name",
    "idea.map_group_accept",
    "idea.map_group_dismiss",
    // Runde 34: die Mindmap als Baum samt Wolke (tree.rs).
    "idea.tree",
];

/// Höchstlänge der Beschriftung eines Einfalls (Schlagwörter) in Zeichen.
const LABEL_CHARS: usize = 60;

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Filter der Karte (GET und RPC gleich, Vertrag §2).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MapFilter {
    /// `None` = alle.
    pub status: Option<IdeaStatus>,
    pub word: Option<String>,
    /// "human" | "persona" | "mcp"
    pub origin: Option<String>,
    pub q: Option<String>,
    pub near: bool,
    /// 1..=5, Vorgabe 3
    pub near_limit: Option<u32>,
    /// 0..=1, Vorgabe 0.75
    pub near_threshold: Option<f32>,
    /// Aufgeklappte Kapitel (keys).
    pub open: Vec<i64>,
    /// Runde 28: Ebene „Befunde“ – offene Befunde je Knoten zählen (Seite: `befunde=1`, RPC: `findings: true`).
    #[serde(default)]
    pub findings: bool,
}

/// Art eines Knotens der Karte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MapNodeKind {
    Work,
    Part,
    Chapter,
    Section,
    NoChapter,
}

/// Ein zum Kapitel zusammengefasster Abschnitt.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapSection {
    pub key: i64,
    pub number: String,
    pub title: String,
    pub ideas: u32,
    /// Runde 28: offene Befunde genau dieses Abschnitts (nur mit der Ebene „Befunde“, sonst 0).
    pub open_findings: u32,
}

/// Knoten der Gliederung auf der Karte.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapNode {
    /// "w" | "n:<key>" | "none"
    pub id: String,
    pub kind: MapNodeKind,
    pub key: Option<i64>,
    /// id des Elternknotens ("w" für oberste); `None` nur bei "w".
    pub parent: Option<String>,
    /// Angezeigte Nummer, "" ohne.
    pub number: String,
    pub title: String,
    /// Zusammengefasste Abschnitte (zugeklappt).
    pub sections: Vec<MapSection>,
    pub open_ideas: u32,
    pub total_ideas: u32,
    /// Runde 28: offene Befunde (Zählregel Vertrag R28 §1.5) am Knoten und allem darunter (ein Kapitel samt seinen
    /// Abschnitten, ein Teil samt Kapiteln, das Werk alles); nur mit der Ebene „Befunde“, sonst 0.
    pub open_findings: u32,
}

/// Ein Einfall auf der Karte.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapIdea {
    pub id: IdeaId,
    /// "i:<id>"
    pub node: String,
    /// [`MapNode::id`], an dem er hängt ("none" ohne Bezug).
    pub anchor: String,
    /// Genauer Bezug (auch ein Abschnitt).
    pub key: Option<i64>,
    /// Schlagwörter „a, b“, höchstens 60 Zeichen.
    pub label: String,
    pub words: Vec<String>,
    pub note: String,
    pub status: IdeaStatus,
    pub origin: IdeaOrigin,
    /// Ohne `q` immer `true`.
    pub hit: bool,
}

/// Zustand der Nähe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NearState {
    Ok,
    Off,
    NoEmbedder,
    Unavailable,
}

/// Woraus eine Nähe folgt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NearVia {
    Vector,
    Words,
    Both,
}

/// Zwei nahe Einfälle (`a < b`), nur Anzeige.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NearPair {
    pub a: IdeaId,
    pub b: IdeaId,
    pub score: f32,
    pub via: NearVia,
    pub shared_words: Vec<String>,
}

/// Nähe der Einfälle (nie gespeichert).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Near {
    pub state: NearState,
    pub message: String,
    pub pairs: Vec<NearPair>,
}

/// Die ganze Karte eines Werks.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IdeaMap {
    pub work: WorkId,
    pub title: String,
    pub nodes: Vec<MapNode>,
    pub ideas: Vec<MapIdea>,
    /// Aktive Verbindungen zwischen gezeigten Einfällen.
    pub links: Vec<IdeaLink>,
    pub near: Near,
    /// Gespeicherte Lagen gezeigter Einfälle.
    pub positions: Vec<MapPos>,
    /// Angewandter Filter.
    pub filter: MapFilter,
}

// ---- Parameter ---------------------------------------------------------------------------------

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Ganzzahl aus einem JSON-Wert: Zahl oder Ziffern als Text (Formulare); fehlt, `null` oder `""` → `None`.
fn opt_int(params: &Json, name: &str) -> Result<Option<i64>, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match params.get(name) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Json::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(Json::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(_) => Err(wrong()),
    }
}

/// Pflicht-Ganzzahl.
fn int(params: &Json, name: &str, what: &str) -> Result<i64, RpcError> {
    opt_int(params, name)?.ok_or_else(|| invalid(format!("{name} fehlt ({what})")))
}

/// Kommazahl (Zahl oder Text); fehlt → -32602.
fn number(params: &Json, name: &str) -> Result<f64, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match params.get(name) {
        None | Some(Json::Null) => Err(invalid(format!("{name} fehlt"))),
        Some(Json::Number(n)) => n.as_f64().ok_or_else(wrong),
        Some(Json::String(s)) => s.trim().parse().map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Text; fehlt, `null` oder nur Leerraum → `None`.
fn opt_text(params: &Json, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

/// Wahrheitswert: `true`/`false`, `1`/`0`, `"1"`/`"0"`/`"true"`/`"false"`/`"on"` (Formulare); fehlt → `None`.
fn opt_bool(params: &Json, name: &str) -> Result<Option<bool>, RpcError> {
    let wrong = || invalid(format!("{name}: true oder false erwartet"));
    match params.get(name) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Bool(b)) => Ok(Some(*b)),
        Some(Json::Number(n)) => match n.as_i64() {
            Some(0) => Ok(Some(false)),
            Some(1) => Ok(Some(true)),
            _ => Err(wrong()),
        },
        Some(Json::String(s)) => match s.trim() {
            "1" | "true" | "on" => Ok(Some(true)),
            "0" | "false" | "off" => Ok(Some(false)),
            _ => Err(wrong()),
        },
        Some(_) => Err(wrong()),
    }
}

/// Filter der Karte aus den Parametern von `idea.map` (dieselben Namen wie die Abfrage der Seite
/// `/werke/{id}/karte`): `status` open|done|all, `word`, `origin` human|persona|mcp, `q`, `near` (Vorgabe
/// `true`), `near_limit` 1..=5, `near_threshold` 0..=1, `open` (Liste von keys oder ein key), `findings` (Ebene
/// „Befunde“, Vorgabe `false`; Runde 28). Falsche Werte → -32602; leere Texte zählen als fehlend.
pub fn filter_from_params(params: &Json) -> Result<MapFilter, RpcError> {
    let status = match opt_text(params, "status")?.as_deref().map(str::trim) {
        None | Some("all") => None,
        Some(word) => Some(
            IdeaStatus::parse(word)
                .ok_or_else(|| invalid(format!("status: „open“, „done“ oder „all“ erwartet, nicht „{word}“")))?,
        ),
    };
    let origin = match opt_text(params, "origin")?.as_deref().map(str::trim) {
        None => None,
        Some(kind @ ("human" | "persona" | "mcp")) => Some(kind.to_owned()),
        Some(other) => {
            return Err(invalid(format!("origin: „human“, „persona“ oder „mcp“ erwartet, nicht „{other}“")));
        }
    };
    let near_limit = match opt_int(params, "near_limit")? {
        None => None,
        Some(n @ 1..=5) => Some(n as u32),
        Some(n) => return Err(invalid(format!("near_limit: 1 bis 5, nicht {n}"))),
    };
    let near_threshold = match params.get("near_threshold") {
        None | Some(Json::Null) => None,
        Some(Json::String(s)) if s.trim().is_empty() => None,
        Some(_) => {
            let t = number(params, "near_threshold")?;
            if !(0.0..=1.0).contains(&t) {
                return Err(invalid(format!("near_threshold: 0 bis 1, nicht {t}")));
            }
            Some(t as f32)
        }
    };
    let open = match params.get("open") {
        None | Some(Json::Null) => Vec::new(),
        Some(Json::Array(list)) => {
            let mut keys = Vec::new();
            for value in list {
                let one = json!({ "open": value });
                if let Some(key) = opt_int(&one, "open")? {
                    keys.push(key);
                }
            }
            keys
        }
        Some(_) => opt_int(params, "open")?.into_iter().collect(),
    };
    Ok(MapFilter {
        status,
        word: opt_text(params, "word")?.map(|w| w.trim().to_owned()),
        origin,
        q: opt_text(params, "q")?.map(|q| q.trim().to_owned()),
        near: opt_bool(params, "near")?.unwrap_or(true),
        near_limit,
        near_threshold,
        open,
        findings: opt_bool(params, "findings")?.unwrap_or(false),
    })
}

/// `kind` einer Verbindung (genau die Wörter aus [`LinkKind::as_str`]).
fn kind_param(params: &Json) -> Result<LinkKind, RpcError> {
    let word = opt_text(params, "kind")?.ok_or_else(|| invalid("kind fehlt (Art der Verbindung)"))?;
    LinkKind::parse(word.trim()).ok_or_else(|| {
        invalid(format!(
            "kind: „leads_to“, „belongs_to“, „example_of“ oder „contradicts“ erwartet, nicht „{}“",
            word.trim()
        ))
    })
}

/// Werk prüfen (unbekannt → Fehler wie bei den übrigen Methoden).
async fn check_work(state: &AppState, work: WorkId) -> Result<(), RpcError> {
    state.store.work_slug(work).await?;
    Ok(())
}

/// RPC der Gedankenkarte (Vertrag §4.1). Herkunft der schreibenden Methoden ist immer der Mensch
/// ([`mirror::HUMAN_NAME`]); sie laufen unter der Schreibsperre ([`links`]).
pub async fn call(state: &AppState, method: &str, params: Json) -> Result<Json, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    if !params.is_object() {
        return Err(invalid("Parameter als Objekt erwartet"));
    }
    let human = IdeaOrigin::Human { name: mirror::HUMAN_NAME.to_owned() };
    match method {
        "idea.map" => {
            let work = int(&params, "work", "Werk")?;
            let filter = filter_from_params(&params)?;
            let map = build(state, work, &filter).await?;
            let mut answer = serde_json::to_value(map).map_err(|e| RpcError::new(INTERNAL, e.to_string()))?;
            // Runde 34: je Einfall `title`, `parent`, `position` (und `main_topic`, `cloud`) – in der Antwort
            // ergänzt, nicht am Typ `MapIdea`, den die eingefrorene Anordnung `bild/layout.rs` von Hand baut.
            let rows = state.store.idea_tree_rows(work).await?;
            let by_id: HashMap<IdeaId, &Idea> = rows.iter().map(|i| (i.id, i)).collect();
            if let Some(Json::Array(ideas)) = answer.get_mut("ideas") {
                for idea in ideas {
                    let Some(row) = idea["id"].as_i64().and_then(|id| by_id.get(&id)) else { continue };
                    idea["title"] = json!(row.title);
                    idea["parent"] = json!(row.parent);
                    idea["position"] = json!(row.position);
                    idea["main_topic"] = json!(row.main_topic);
                    idea["cloud"] = json!(row.cloud);
                }
            }
            Ok(answer)
        }
        "idea.tree" => {
            let work = int(&params, "work", "Werk")?;
            let filter = filter_from_params(&params)?;
            let (map, cloud) = tree::tree_and_cloud(state, work, &filter).await?;
            Ok(json!({ "map": map, "cloud": cloud }))
        }
        "idea.links" => {
            let deleted = opt_bool(&params, "deleted")?.unwrap_or(false);
            let links = match (opt_int(&params, "work")?, opt_int(&params, "idea")?) {
                (Some(work), None) => state.store.idea_links_of_work(work, deleted).await?,
                (None, Some(idea)) => state.store.idea_links_of_idea(idea, deleted).await?,
                _ => return Err(invalid("idea.links: genau eins von work und idea angeben")),
            };
            Ok(json!({ "links": links }))
        }
        "idea.link" => {
            let note = opt_text(&params, "note")?.unwrap_or_default();
            let new = NewIdeaLink {
                from: int(&params, "from", "Einfall, von dem die Verbindung ausgeht")?,
                to: int(&params, "to", "Einfall, zu dem sie führt")?,
                kind: kind_param(&params)?,
                note: &note,
                origin: human,
            };
            links::link(state, &new).await
        }
        "idea.unlink" => links::unlink(state, int(&params, "id", "Verbindung")?, &human).await,
        "idea.link_restore" => links::restore(state, int(&params, "id", "Verbindung")?, &human).await,
        "idea.map_pos_set" => {
            let work = int(&params, "work", "Werk")?;
            let idea = int(&params, "idea", "Einfall")?;
            let (x, y) = (number(&params, "x")?, number(&params, "y")?);
            check_work(state, work).await?;
            let pos = state.store.idea_map_pos_set(work, idea, x, y).await?;
            Ok(json!({ "pos": pos }))
        }
        "idea.map_pos_reset" => {
            let work = int(&params, "work", "Werk")?;
            let idea = opt_int(&params, "idea")?;
            check_work(state, work).await?;
            let removed = state.store.idea_map_pos_reset(work, idea).await?;
            Ok(json!({ "removed": removed }))
        }
        // Runde 19b: Gruppen und ihre Namen (groups.rs).
        "idea.map_groups" => {
            let work = int(&params, "work", "Werk")?;
            let filter = filter_from_params(&params)?;
            check_work(state, work).await?;
            Ok(json!({ "groups": groups::groups(state, work, &filter).await? }))
        }
        "idea.map_groups_name" => {
            let work = int(&params, "work", "Werk")?;
            let filter = filter_from_params(&params)?;
            let persona = opt_text(&params, "persona")?;
            check_work(state, work).await?;
            groups::name_groups(state, work, &filter, persona.as_deref()).await
        }
        "idea.map_group_accept" => {
            let work = int(&params, "work", "Werk")?;
            let group = groups::group_param(&params)?;
            let filter = filter_from_params(&params)?;
            let name = opt_text(&params, "name")?;
            check_work(state, work).await?;
            groups::accept(state, work, &group, name.as_deref(), &filter).await
        }
        "idea.map_group_dismiss" => {
            let work = int(&params, "work", "Werk")?;
            let group = groups::group_param(&params)?;
            check_work(state, work).await?;
            Ok(groups::dismiss(state, work, &group))
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {other}"))),
    }
}

/// Schlagwörter „a, b“, höchstens [`LABEL_CHARS`] Zeichen (gekürzt mit „…“).
fn label_of(words: &[String]) -> String {
    let joined = words.join(", ");
    if joined.chars().count() <= LABEL_CHARS {
        joined
    } else {
        let mut cut: String = joined.chars().take(LABEL_CHARS - 1).collect();
        cut.push('…');
        cut
    }
}

/// Alle nicht gelöschten Einfälle des Werks, nach id (seitenweise, die Liste hat eine Obergrenze je Abruf).
async fn ideas_of(state: &AppState, work: WorkId) -> Result<Vec<Idea>, RpcError> {
    let mut all = Vec::new();
    let page = 500u32;
    loop {
        let filter =
            IdeaFilter { scope: IdeaScope::Work(work), limit: page, offset: all.len() as u32, ..Default::default() };
        let batch = state.store.idea_list(&filter).await?;
        let done = (batch.len() as u32) < page;
        all.extend(batch);
        if done {
            break;
        }
    }
    all.sort_by_key(|idea| idea.id);
    Ok(all)
}

/// Treffer der Volltextsuche `q` im Werk (FTS wie `idea.search`); `None`, wenn `q` kein Suchwort enthält.
async fn hits_of(state: &AppState, work: WorkId, q: &str) -> Result<Option<HashSet<IdeaId>>, RpcError> {
    let Some(fts) = baruch_library::fts_query(q) else { return Ok(None) };
    let mut hits = HashSet::new();
    let page = 500u32;
    let mut offset = 0u32;
    loop {
        let filter = IdeaFilter { scope: IdeaScope::Work(work), limit: page, offset, ..Default::default() };
        let found = state.store.idea_search(&fts, &filter).await?;
        let count = found.len() as u32;
        hits.extend(found.into_iter().map(|(idea, _)| idea.id));
        if count < page {
            break;
        }
        offset += page;
    }
    Ok(Some(hits))
}

/// Setzt die Karte des Werks zusammen (Vertrag §2, §3.2).
///
/// - Knoten: Werk (`w`), Teile und Kapitel aus der Gliederung (`n:<key>`, Elternknoten = nächster Vorfahre, der
///   selbst Knoten ist, sonst `w`); Abschnitte unter einem Kapitel fasst das Kapitel in `sections` zusammen
///   (Anker = nächster Vorfahre der Art Kapitel), es sei denn, das Kapitel steht in `open` – dann sind seine
///   Abschnitte eigene Knoten (Art `section`); Abschnitte ohne Kapitel darüber sind immer eigene Knoten. Die
///   Wolke „Ohne Kapitel“ (`none`) gibt es nur, wenn ein gezeigter Einfall keinen (gültigen) Bezug hat.
/// - Einfälle: die nicht gelöschten des Werks, ohne die, die `status`, `word` (gleicher `word_key`) oder
///   `origin` ausschließen; `q` (FTS wie `idea.search`) markiert Treffer (`hit`), der Rest bleibt mit `hit =
///   false` (ohne `q` alle `true`).
/// - Zähler: `open_ideas`/`total_ideas` je Knoten = gezeigte Einfälle, die an ihm oder an einem Knoten darunter
///   hängen (ein Teil samt seinen Kapiteln, ein Kapitel samt seinen Abschnitten, zugeklappt wie aufgeklappt), beim
///   Werk alle gezeigten; `MapSection::ideas` = gezeigte Einfälle mit genau diesem Bezug.
/// - Verbindungen: aktive zwischen gezeigten Einfällen; Lagen: gespeicherte gezeigter Einfälle.
/// - Nähe: [`near::compute`] aus gespeicherten Vektoren und gleichen Schlagwörtern (nie eine Einbettungsanfrage).
/// - `filter` der Antwort ist der angewandte: Vorgaben für `near_limit`/`near_threshold` eingesetzt, `open` nur mit
///   Kapiteln des Werks (sortiert, ohne Doppel), `q` ohne Suchwort entfällt.
pub async fn build(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<IdeaMap, RpcError> {
    let loaded = state.store.load_work(work).await?;
    let by_seq: HashMap<usize, &baruch_core::Node> = loaded.nodes.iter().map(|n| (n.id, n)).collect();

    // Aufgeklappte Kapitel: nur keys von Kapiteln dieses Werks.
    let chapter_keys: HashSet<i64> =
        loaded.nodes.iter().filter(|n| n.kind == NodeKind::Chapter).map(|n| n.key).collect();
    let open: BTreeSet<i64> = filter.open.iter().copied().filter(|k| chapter_keys.contains(k)).collect();

    // Je Punkt (Stelle) der Knoten, an dem er auf der Karte erscheint.
    let mut anchor_of: HashMap<usize, usize> = HashMap::new();
    let mut nodes = vec![MapNode {
        id: "w".into(),
        kind: MapNodeKind::Work,
        key: None,
        parent: None,
        number: String::new(),
        title: loaded.title.clone(),
        sections: Vec::new(),
        open_ideas: 0,
        total_ideas: 0,
        open_findings: 0,
    }];
    // Stelle → Platz in `nodes`.
    let mut slot_of: HashMap<usize, usize> = HashMap::new();
    for node in &loaded.nodes {
        // Nächster Vorfahre der Art Kapitel (ohne den Punkt selbst).
        let mut chapter = None;
        let mut up = node.parent;
        while let Some(seq) = up {
            let Some(parent) = by_seq.get(&seq) else { break };
            if parent.kind == NodeKind::Chapter {
                chapter = Some(parent);
                break;
            }
            up = parent.parent;
        }
        if let (NodeKind::Section, Some(chapter)) = (node.kind, chapter)
            && !open.contains(&chapter.key)
        {
            anchor_of.insert(node.id, chapter.id);
            if let Some(&slot) = slot_of.get(&chapter.id) {
                nodes[slot].sections.push(MapSection {
                    key: node.key,
                    number: node.number.clone(),
                    title: node.title.clone(),
                    ideas: 0,
                    open_findings: 0,
                });
            }
            continue;
        }
        // Elternknoten: nächster Vorfahre, der selbst ein Knoten der Karte ist.
        let mut parent = "w".to_owned();
        let mut up = node.parent;
        while let Some(seq) = up {
            if let Some(&slot) = slot_of.get(&seq) {
                parent = nodes[slot].id.clone();
                break;
            }
            up = by_seq.get(&seq).and_then(|n| n.parent);
        }
        let kind = match node.kind {
            NodeKind::Part => MapNodeKind::Part,
            NodeKind::Chapter => MapNodeKind::Chapter,
            NodeKind::Section => MapNodeKind::Section,
        };
        anchor_of.insert(node.id, node.id);
        slot_of.insert(node.id, nodes.len());
        nodes.push(MapNode {
            id: format!("n:{}", node.key),
            kind,
            key: Some(node.key),
            parent: Some(parent),
            number: node.number.clone(),
            title: node.title.clone(),
            sections: Vec::new(),
            open_ideas: 0,
            total_ideas: 0,
            open_findings: 0,
        });
    }
    let seq_of_key: HashMap<i64, usize> = loaded.nodes.iter().map(|n| (n.key, n.id)).collect();
    // Je Platz in `nodes` der Platz des Elternknotens (ohne das Werk, das ohnehin alle zählt).
    let slot_by_id: HashMap<String, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
    let parent_slot: Vec<Option<usize>> = nodes
        .iter()
        .map(|n| n.parent.as_deref().and_then(|p| slot_by_id.get(p).copied()).filter(|&p| p != 0))
        .collect();

    // Runde 28: Ebene „Befunde“ – offene Befunde je Punkt an seinem Knoten und allen Vorfahren (wie die Einfälle).
    if filter.findings {
        let counts = state.store.finding_open_counts(work).await?;
        for node in &loaded.nodes {
            let Some(&count) = counts.get(&node.key) else { continue };
            let count = u32::try_from(count).unwrap_or(u32::MAX);
            let Some(&slot) = anchor_of.get(&node.id).and_then(|a| slot_of.get(a)) else { continue };
            let mut up = Some(slot);
            while let Some(at) = up {
                nodes[at].open_findings += count;
                up = parent_slot[at];
            }
            if let Some(section) = nodes[slot].sections.iter_mut().find(|s| s.key == node.key) {
                section.open_findings += count;
            }
            nodes[0].open_findings += count;
        }
    }

    // Einfälle nach Filter.
    let word = filter.word.as_deref().map(near::word_key).filter(|w| !w.is_empty());
    let origin = filter.origin.as_deref().filter(|o| !o.trim().is_empty());
    let q = filter.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    let hits = match q {
        Some(q) => hits_of(state, work, q).await?,
        None => None,
    };
    let shown: Vec<Idea> = ideas_of(state, work)
        .await?
        .into_iter()
        .filter(|idea| filter.status.is_none_or(|s| idea.status == s))
        .filter(|idea| origin.is_none_or(|o| idea.origin.kind() == o))
        .filter(|idea| word.as_ref().is_none_or(|w| idea.words.iter().any(|x| near::word_key(x) == *w)))
        .collect();

    let mut ideas = Vec::with_capacity(shown.len());
    let mut none_open = 0u32;
    let mut none_total = 0u32;
    for idea in shown {
        let open_count = u32::from(idea.status == IdeaStatus::Open);
        let seq = idea.key.and_then(|key| seq_of_key.get(&key).copied());
        let anchor = match seq.and_then(|seq| anchor_of.get(&seq).and_then(|a| slot_of.get(a))) {
            Some(&slot) => {
                // Der Anker und alle seine Vorfahren bis unter das Werk: Ein Teil zählt die Einfälle seiner
                // Kapitel, ein aufgeklapptes Kapitel die seiner Abschnitte (Bedienung: Zahl im Kreis).
                let mut up = Some(slot);
                while let Some(at) = up {
                    nodes[at].open_ideas += open_count;
                    nodes[at].total_ideas += 1;
                    up = parent_slot[at];
                }
                if let Some(section) = nodes[slot].sections.iter_mut().find(|s| Some(s.key) == idea.key) {
                    section.ideas += 1;
                }
                nodes[slot].id.clone()
            }
            None => {
                none_open += open_count;
                none_total += 1;
                "none".to_owned()
            }
        };
        nodes[0].open_ideas += open_count;
        nodes[0].total_ideas += 1;
        ideas.push(MapIdea {
            id: idea.id,
            node: format!("i:{}", idea.id),
            anchor,
            key: idea.key,
            // Runde 34: der Titel des Einfalls, sonst wie bisher die Schlagwörter.
            label: if idea.title.is_empty() { label_of(&idea.words) } else { idea.title.clone() },
            hit: hits.as_ref().is_none_or(|h| h.contains(&idea.id)),
            words: idea.words,
            note: idea.note,
            status: idea.status,
            origin: idea.origin,
        });
    }
    if none_total > 0 {
        nodes.push(MapNode {
            id: "none".into(),
            kind: MapNodeKind::NoChapter,
            key: None,
            parent: Some("w".into()),
            number: String::new(),
            title: "Ohne Kapitel".into(),
            sections: Vec::new(),
            open_ideas: none_open,
            total_ideas: none_total,
            open_findings: 0,
        });
    }

    let ids: HashSet<IdeaId> = ideas.iter().map(|i| i.id).collect();
    let links: Vec<IdeaLink> = state
        .store
        .idea_links_of_work(work, false)
        .await?
        .into_iter()
        .filter(|l| ids.contains(&l.from) && ids.contains(&l.to))
        .collect();
    let positions: Vec<MapPos> =
        state.store.idea_map_positions(work).await?.into_iter().filter(|p| ids.contains(&p.idea)).collect();
    let near_limit = filter.near_limit.unwrap_or(near::DEFAULT_LIMIT);
    let near_threshold = filter.near_threshold.unwrap_or(near::DEFAULT_THRESHOLD);
    let near = near::compute(state, work, &ideas, filter.near, near_limit, near_threshold).await;

    let applied = MapFilter {
        status: filter.status,
        word: filter.word.as_deref().map(str::trim).filter(|w| !w.is_empty()).map(str::to_owned),
        origin: origin.map(str::to_owned),
        q: hits.as_ref().and(q).map(str::to_owned),
        near: filter.near,
        near_limit: Some(near_limit),
        near_threshold: Some(near_threshold),
        open: open.into_iter().collect(),
        findings: filter.findings,
    };
    Ok(IdeaMap { work, title: loaded.title, nodes, ideas, links, near, positions, filter: applied })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_join_the_words_and_stay_within_sixty_characters() {
        // Vertrag §3.2: Beschriftung „a, b“, höchstens 60 Zeichen – sonst sprengte sie den Knoten der Karte.
        assert_eq!(label_of(&["Laube".into(), "Rose".into()]), "Laube, Rose", "Wörter mit Komma verbunden");
        let long: Vec<String> = (0..20).map(|i| format!("Wort{i}")).collect();
        let label = label_of(&long);
        assert_eq!(label.chars().count(), LABEL_CHARS, "gekürzt auf genau 60 Zeichen: {label}");
        assert!(label.ends_with('…'), "die Kürzung ist sichtbar: {label}");
    }
}
