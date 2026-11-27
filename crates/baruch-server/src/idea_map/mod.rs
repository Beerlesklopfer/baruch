// SPDX-License-Identifier: AGPL-3.0-or-later
//! Idea map (contract Round 25): the ideas of a work as a map around the work, its parts and chapters,
//! with links between ideas, stored positions and (display only) nearness.
//!
//! - Data and RPC (karte-daten, this module): [`build`] assembles the map; RPC `idea.map`, `idea.links`,
//!   `idea.link`, `idea.unlink`, `idea.link_restore`, `idea.map_pos_set`, `idea.map_pos_reset` ([`call`],
//!   `docs/RPC.md` „Gedankenkarte“). Origin through RPC is always the human.
//! - Page (karte-bild, [`bild`]): `/werke/{id}/karte` (HTML, usable without JavaScript); exports `.svg`, `.pdf`,
//!   `.md`, `.mm` and form `POST /werke/{id}/karte/act` since Round 34 (R34-S1) in [`files`].
//! - Round 34: [`tree`] – the mind map as a tree ([`tree::MindMap`], [`tree::tree`]), the cloud next to it
//!   ([`tree::Cloud`], [`tree::cloud`]) and the tree import ([`tree::import`]); RPC `idea.tree` → `{map, cloud}`.
//!   `idea.map` (flat view) stays and additionally names `title`, `parent`, `position`, `main_topic`,
//!   `cloud` per idea. [`bild::mindmap`] draws the mind map.
//!
//! Structure (contract §2): the work in the middle (`w`), around it parts and chapters (`n:<key>`); sections under a
//! chapter are summarised by the chapter (anchor = nearest ancestor of kind chapter, otherwise the item itself);
//! ideas without a relation hang on the cloud „Ohne Kapitel“ (`none`). Ideas without a work do not appear.
//!
//! Sub-modules (karte-daten): [`links`] writes links with a mirror commit (also for MCP), [`near`]
//! computes the nearness from stored vectors ([`crate::rag::neighbors`]) and equal tags. Positions
//! are display only (no commit).
//!
//! Round 19b: [`groups`] – groups of the nearness (`idea.map_groups`), names by an assistant as a suggestion only in
//! memory (`idea.map_groups_name`), adopting as a tag by the human (`idea.map_group_accept`, only RPC
//! and form, no MCP tool) and dismissing (`idea.map_group_dismiss`).

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

/// Methods of this module.
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
    // Round 34: the mind map as a tree with cloud (tree.rs).
    "idea.tree",
];

/// Maximum length of the label of an idea (tags) in characters.
const LABEL_CHARS: usize = 60;

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Filter of the map (GET and RPC alike, contract §2).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MapFilter {
    /// `None` = all.
    pub status: Option<IdeaStatus>,
    pub word: Option<String>,
    /// "human" | "persona" | "mcp"
    pub origin: Option<String>,
    pub q: Option<String>,
    pub near: bool,
    /// 1..=5, default 3
    pub near_limit: Option<u32>,
    /// 0..=1, default 0.75
    pub near_threshold: Option<f32>,
    /// Expanded chapters (keys).
    pub open: Vec<i64>,
    /// Round 28: layer „Befunde“ – count open findings per node (page: `befunde=1`, RPC: `findings: true`).
    #[serde(default)]
    pub findings: bool,
}

/// Kind of a node of the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MapNodeKind {
    Work,
    Part,
    Chapter,
    Section,
    NoChapter,
}

/// A section summarised into its chapter.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapSection {
    pub key: i64,
    pub number: String,
    pub title: String,
    pub ideas: u32,
    /// Round 28: open findings of exactly this section (only with the layer „Befunde“, otherwise 0).
    pub open_findings: u32,
}

/// Node of the outline on the map.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapNode {
    /// "w" | "n:<key>" | "none"
    pub id: String,
    pub kind: MapNodeKind,
    pub key: Option<i64>,
    /// id of the parent node ("w" for top-level ones); `None` only for "w".
    pub parent: Option<String>,
    /// Displayed number, "" without.
    pub number: String,
    pub title: String,
    /// Summarised sections (collapsed).
    pub sections: Vec<MapSection>,
    pub open_ideas: u32,
    pub total_ideas: u32,
    /// Round 28: open findings (counting rule contract R28 §1.5) on the node and everything below (a chapter with its
    /// sections, a part with chapters, the work everything); only with the layer „Befunde“, otherwise 0.
    pub open_findings: u32,
}

/// An idea on the map.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapIdea {
    pub id: IdeaId,
    /// "i:<id>"
    pub node: String,
    /// [`MapNode::id`] it hangs on ("none" without a relation).
    pub anchor: String,
    /// Exact relation (also a section).
    pub key: Option<i64>,
    /// Tags „a, b“, at most 60 characters.
    pub label: String,
    pub words: Vec<String>,
    pub note: String,
    pub status: IdeaStatus,
    pub origin: IdeaOrigin,
    /// Without `q` always `true`.
    pub hit: bool,
}

/// State of the nearness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NearState {
    Ok,
    Off,
    NoEmbedder,
    Unavailable,
}

/// What a nearness follows from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NearVia {
    Vector,
    Words,
    Both,
}

/// Two near ideas (`a < b`), display only.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NearPair {
    pub a: IdeaId,
    pub b: IdeaId,
    pub score: f32,
    pub via: NearVia,
    pub shared_words: Vec<String>,
}

/// Nearness of the ideas (never stored).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Near {
    pub state: NearState,
    pub message: String,
    pub pairs: Vec<NearPair>,
}

/// The whole map of a work.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IdeaMap {
    pub work: WorkId,
    pub title: String,
    pub nodes: Vec<MapNode>,
    pub ideas: Vec<MapIdea>,
    /// Active links between ideas shown.
    pub links: Vec<IdeaLink>,
    pub near: Near,
    /// Stored positions of ideas shown.
    pub positions: Vec<MapPos>,
    /// Applied filter.
    pub filter: MapFilter,
}

// ---- Parameters --------------------------------------------------------------------------------

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Integer from a JSON value: number or digits as text (forms); missing, `null` or `""` → `None`.
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

/// Mandatory integer.
fn int(params: &Json, name: &str, what: &str) -> Result<i64, RpcError> {
    opt_int(params, name)?.ok_or_else(|| invalid(format!("{name} fehlt ({what})")))
}

/// Decimal number (number or text); missing → -32602.
fn number(params: &Json, name: &str) -> Result<f64, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match params.get(name) {
        None | Some(Json::Null) => Err(invalid(format!("{name} fehlt"))),
        Some(Json::Number(n)) => n.as_f64().ok_or_else(wrong),
        Some(Json::String(s)) => s.trim().parse().map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Text; missing, `null` or only whitespace → `None`.
fn opt_text(params: &Json, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

/// Boolean: `true`/`false`, `1`/`0`, `"1"`/`"0"`/`"true"`/`"false"`/`"on"` (forms); missing → `None`.
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

/// Filter of the map from the parameters of `idea.map` (the same names as the query of the page
/// `/werke/{id}/karte`): `status` open|done|all, `word`, `origin` human|persona|mcp, `q`, `near` (default
/// `true`), `near_limit` 1..=5, `near_threshold` 0..=1, `open` (list of keys or one key), `findings` (layer
/// „Befunde“, default `false`; Round 28). Wrong values → -32602; empty texts count as missing.
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

/// `kind` of a link (exactly the words from [`LinkKind::as_str`]).
fn kind_param(params: &Json) -> Result<LinkKind, RpcError> {
    let word = opt_text(params, "kind")?.ok_or_else(|| invalid("kind fehlt (Art der Verbindung)"))?;
    LinkKind::parse(word.trim()).ok_or_else(|| {
        invalid(format!(
            "kind: „leads_to“, „belongs_to“, „example_of“ oder „contradicts“ erwartet, nicht „{}“",
            word.trim()
        ))
    })
}

/// Check the work (unknown → error as for the other methods).
async fn check_work(state: &AppState, work: WorkId) -> Result<(), RpcError> {
    state.store.work_slug(work).await?;
    Ok(())
}

/// RPC of the idea map (contract §4.1). Origin of the writing methods is always the human
/// ([`mirror::HUMAN_NAME`]); they run under the write lock ([`links`]).
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
            // Round 34: per idea `title`, `parent`, `position` (and `main_topic`, `cloud`) – added in the
            // answer, not on the type `MapIdea`, which the frozen layout `bild/layout.rs` builds by hand.
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
        // Round 19b: groups and their names (groups.rs).
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
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {other}"))),
    }
}

/// Tags „a, b“, at most [`LABEL_CHARS`] characters (shortened with „…“).
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

/// All non-deleted ideas of the work, by id (page by page, the list has an upper limit per call).
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

/// Hits of the full-text search `q` in the work (FTS like `idea.search`); `None` if `q` contains no search word.
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

/// Assembles the map of the work (contract §2, §3.2).
///
/// - Nodes: work (`w`), parts and chapters from the outline (`n:<key>`, parent node = nearest ancestor that
///   is itself a node, otherwise `w`); sections under a chapter are summarised by the chapter in `sections`
///   (anchor = nearest ancestor of kind chapter), unless the chapter is in `open` – then its
///   sections are nodes of their own (kind `section`); sections without a chapter above are always nodes of their own.
///   The cloud „Ohne Kapitel“ (`none`) exists only if an idea shown has no (valid) relation.
/// - Ideas: the non-deleted ones of the work, without those excluded by `status`, `word` (same `word_key`) or
///   `origin`; `q` (FTS like `idea.search`) marks hits (`hit`), the rest stays with `hit =
///   false` (without `q` all `true`).
/// - Counters: `open_ideas`/`total_ideas` per node = ideas shown that hang on it or on a node below
///   (a part with its chapters, a chapter with its sections, collapsed as well as expanded), for the
///   work all shown; `MapSection::ideas` = ideas shown with exactly this relation.
/// - Links: active ones between ideas shown; positions: stored ones of ideas shown.
/// - Nearness: [`near::compute`] from stored vectors and equal tags (never an embedding request).
/// - `filter` of the answer is the applied one: defaults for `near_limit`/`near_threshold` filled in, `open` only
///   with chapters of the work (sorted, no duplicates), `q` without a search word is dropped.
pub async fn build(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<IdeaMap, RpcError> {
    let loaded = state.store.load_work(work).await?;
    let by_seq: HashMap<usize, &baruch_core::Node> = loaded.nodes.iter().map(|n| (n.id, n)).collect();

    // Expanded chapters: only keys of chapters of this work.
    let chapter_keys: HashSet<i64> =
        loaded.nodes.iter().filter(|n| n.kind == NodeKind::Chapter).map(|n| n.key).collect();
    let open: BTreeSet<i64> = filter.open.iter().copied().filter(|k| chapter_keys.contains(k)).collect();

    // Per item (position) the node at which it appears on the map.
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
    // Position → place in `nodes`.
    let mut slot_of: HashMap<usize, usize> = HashMap::new();
    for node in &loaded.nodes {
        // Nearest ancestor of kind chapter (without the item itself).
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
        // Parent node: nearest ancestor that is itself a node of the map.
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
    // Per place in `nodes` the place of the parent node (without the work, which counts all anyway).
    let slot_by_id: HashMap<String, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
    let parent_slot: Vec<Option<usize>> = nodes
        .iter()
        .map(|n| n.parent.as_deref().and_then(|p| slot_by_id.get(p).copied()).filter(|&p| p != 0))
        .collect();

    // Round 28: layer „Befunde“ – open findings per item on its node and all ancestors (like the ideas).
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

    // Ideas by filter.
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
                // The anchor and all its ancestors up to below the work: a part counts the ideas of its
                // chapters, an expanded chapter those of its sections (usage: number in the circle).
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
            // Round 34: the title of the idea, otherwise as before the tags.
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
        // Contract §3.2: label „a, b“, at most 60 characters – otherwise it would burst the node of the map.
        assert_eq!(label_of(&["Laube".into(), "Rose".into()]), "Laube, Rose", "Wörter mit Komma verbunden");
        let long: Vec<String> = (0..20).map(|i| format!("Wort{i}")).collect();
        let label = label_of(&long);
        assert_eq!(label.chars().count(), LABEL_CHARS, "gekürzt auf genau 60 Zeichen: {label}");
        assert!(label.ends_with('…'), "die Kürzung ist sichtbar: {label}");
    }
}
