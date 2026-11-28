// SPDX-License-Identifier: AGPL-3.0-or-later
//! View models for the templates and the drawing of the page and of its three areas.
//!
//! Revisions are addressed everywhere through their **number** (address `rev`, paging, forms),
//! not through their position: after a revision is deleted the sequence of numbers has gaps. "‹ n/m ›"
//! shows the position n among m revisions, the current revision is highlighted (`is-current`).
//!
//! Round 35 (accounts and roles, contract §3.3): [`page`] and [`panes`] ask [`crate::auth::current`]. If a
//! Lektor is logged in, the three areas come from templates of their own (`fragments/lektor_tree.html`,
//! `lektor_editor.html`, `lektor_assistants.html`): reading, „Vorschlagen“ (`revision.propose`), post (`post.add`),
//! changing or removing own posts and finding (`finding.add`) – everything through `POST /lektorat/act` or `/rpc`.
//! The buttons of the author (save, accept, reject, switch, delete, hide, personas, literature)
//! are then not in the HTML at all. For everyone else the page stays byte for byte as it was. Both functions
//! run in the task of the request (never spawned off), otherwise the answer would be "without accounts".

use std::collections::HashMap;

use baruch_core::{Author, DiffKind, Node, NodeKind, RevisionStatus, Work, WorkKind, line_diff, revision_fold};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::{Deserialize, Serialize};

use baruch_store::{FALLBACK_COLOR, NodeRevisions, PersonaColor, RevisionInfo, WorkId, WorkSummary};

use crate::i18n::tc;
use crate::rpc::LibraryView;

/// What is shown right now; it also stands in the address (`/?work=1&key=12&rev=3&view=preview`, Round 8).
/// Canonical is the fixed key `key` (`Node::key`); `node` (position) only counts as a fallback
/// when reading older addresses, generated links no longer carry it.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Location {
    /// Work; if it is missing, the default work.
    pub work: Option<WorkId>,
    /// Fixed key of the section; takes precedence over `node` and stays the same when the outline is rearranged.
    pub key: Option<i64>,
    /// Position (seq) of the section: only a fallback if `key` is missing or unknown.
    pub node: Option<usize>,
    /// Revision number, from 1; if it is missing (or does not exist), the current one.
    pub rev: Option<usize>,
    pub view: Option<String>,
    /// Revisions set on trial for the preview, `"<node>:<rev>[,<node>:<rev>…]"`. Invalid
    /// entries are skipped; the mark "current" does not change through this.
    pub pick: Option<String>,
    /// Search of the literature (popup of the header or page `/literatur`; work and library), `?lq=…`.
    pub lq: Option<String>,
    /// Round 22: feedback after saving without JavaScript (`/gliederung/act`): `amended-<k>` (amended,
    /// state k) or `new-<n>-<grund>` (new revision n, reason according to `NewReason::id`) or `new-<n>`. Does not
    /// stand in generated addresses; invalid values are skipped.
    pub saved: Option<String>,
    /// 0.9.0 (user 2026-11-24): `alle` opens the switch „ältere Fassungen“ – paging ‹ › then also shows the
    /// revisions hidden under the mark ([`baruch_core::revision_fold`]); otherwise it skips them.
    pub fassungen: Option<String>,
}

/// Settings of the user interface from cookies (`baruch_*`, set by the widgets). With them the server
/// draws correctly right away: theme, column widths, open section on the right, collapsed chapters,
/// view of the editor.
#[derive(Debug, Clone, Serialize)]
pub struct Settings {
    /// `light` or `dark`; if it is missing, the page follows the system.
    pub theme: Option<&'static str>,
    /// Width of the left and the right column in percent (12–45).
    pub left: Option<f32>,
    pub right: Option<f32>,
    /// Open section of the accordion on the right: `personas`, `revisions` (Fassungen) or `chat` (default).
    pub accordion: &'static str,
    /// Collapsed parts and chapters.
    pub closed: Vec<usize>,
    /// Work opened last (cookie `baruch_work`); `/` without `?work=` opens it if it is still there.
    pub work: Option<WorkId>,
    /// Collapsed proposals in the reading flow as `(key, rev)` (cookie `baruch_folded` =
    /// '.'-separated `<key>-<rev>`, at most [`MAX_FOLDED`] entries, only `[0-9.-]`).
    pub folded: Vec<(i64, u32)>,
    /// View of the editor (cookie `baruch_editor`, set only by the switch): `satz` (default) or
    /// `quelltext`. With it the server draws the switch (`aria-pressed`) and the typeset view correctly right
    /// away, instead of the widget switching only after loading (Round 8, 2d).
    pub editor: &'static str,
    /// Proposals in the reading flow initially (cookie `baruch_pv`, settings „Darstellung“, Round 9):
    /// `open` (default) or `closed` – then the server draws all collapsed except the shown one.
    /// With `closed` [`Settings::folded`] additionally carries the marker [`FOLD_ALL`].
    pub pv: &'static str,
}

/// Marker in [`Settings::folded`] for `baruch_pv=closed`: "all proposals collapsed". Key 0
/// does not exist (`parse_folded` takes only keys and numbers from 1), so the marker never hits
/// a real proposal; this way the setting works through the same path as the cookie `baruch_folded`.
pub const FOLD_ALL: (i64, u32) = (0, 0);

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: None,
            left: None,
            right: None,
            accordion: "chat",
            closed: Vec::new(),
            work: None,
            folded: Vec::new(),
            editor: "satz",
            pv: "open",
        }
    }
}

/// Maximum number of entries in the cookie `baruch_folded`; further ones are skipped.
pub const MAX_FOLDED: usize = 200;

/// Value of `baruch_folded` → `(key, rev)`; if it contains anything other than `[0-9.-]`, it does not count.
fn parse_folded(value: &str) -> Vec<(i64, u32)> {
    if !value.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b'-') {
        return Vec::new();
    }
    let mut out: Vec<(i64, u32)> = Vec::new();
    for entry in value.split('.').filter(|e| !e.is_empty()).take(MAX_FOLDED) {
        let Some((key, rev)) = entry.split_once('-') else { continue };
        let (Ok(key), Ok(rev)) = (key.parse::<i64>(), rev.parse::<u32>()) else { continue };
        if key > 0 && rev > 0 && !out.contains(&(key, rev)) {
            out.push((key, rev));
        }
    }
    out
}

impl Settings {
    /// Read from the `Cookie` header; unknown and invalid values are skipped.
    pub fn from_cookie_header(header: Option<&str>) -> Self {
        let mut settings = Settings::default();
        let width = |v: &str| v.trim_end_matches('%').parse::<f32>().ok().filter(|w| (12.0..=45.0).contains(w));
        for pair in header.unwrap_or("").split(';') {
            let Some((key, value)) = pair.trim().split_once('=') else { continue };
            match key {
                "baruch_theme" => {
                    settings.theme = match value {
                        "light" => Some("light"),
                        "dark" => Some("dark"),
                        _ => None,
                    }
                }
                "baruch_left" => settings.left = width(value),
                "baruch_right" => settings.right = width(value),
                "baruch_accordion" if value == "personas" => settings.accordion = "personas",
                "baruch_accordion" if value == "revisions" => settings.accordion = "revisions",
                // Round 28: accordion „Befunde“ (fragments/findings_panel.html).
                "baruch_accordion" if value == "befunde" => settings.accordion = "befunde",
                "baruch_closed" => settings.closed = value.split('.').filter_map(|id| id.parse().ok()).collect(),
                "baruch_work" => settings.work = value.parse().ok().filter(|&id: &WorkId| id > 0),
                "baruch_folded" => settings.folded = parse_folded(value),
                "baruch_editor" if value == "quelltext" => settings.editor = "quelltext",
                "baruch_pv" if value == "closed" => settings.pv = "closed",
                _ => {}
            }
        }
        if settings.pv == "closed" {
            settings.folded.push(FOLD_ALL);
        }
        settings
    }

    /// Inline style for `<html>` with the column widths.
    fn style(&self) -> String {
        let mut style = String::new();
        if let Some(left) = self.left {
            style.push_str(&format!("--left:{left:.1}%;"));
        }
        if let Some(right) = self.right {
            style.push_str(&format!("--right:{right:.1}%;"));
        }
        style
    }
}

/// Resolved place: valid section, position (`index`, from 0) and number (`rev`) of the shown
/// revision, view and the valid revisions set on trial (`pick`).
#[derive(Debug, Clone)]
pub struct Resolved {
    pub node: usize,
    /// Fixed key of the shown section (`Node::key`); stands in every generated address.
    pub key: i64,
    pub index: usize,
    pub rev: u32,
    pub preview: bool,
    pub pick: Vec<(usize, u32)>,
    /// The address named a key that does not exist in the work (any more); shown is then the
    /// fallback (`node` or the default section), the editor header names the key.
    pub missing_key: Option<i64>,
    /// Round 22: feedback after saving ([`Location::saved`], checked in [`saved_notice`]).
    pub saved: Option<String>,
    /// 0.9.0: switch „ältere Fassungen“ open (`?fassungen=alle`).
    pub show_all: bool,
}

impl Location {
    /// Determine the section (contract Round 8, §1): `key` found → this section (takes precedence over `node`);
    /// `key` unknown → `node`, otherwise the default section, with `missing_key`; without `key` as before
    /// `node` or the default.
    pub fn resolve(&self, work: &Work) -> Resolved {
        let by_key = self.key.and_then(|key| work.nodes.iter().find(|n| n.key == key)).map(|n| n.id);
        let missing_key = self.key.filter(|_| by_key.is_none());
        let fallback = || self.node.filter(|&id| id < work.nodes.len()).unwrap_or(default_node(work));
        let node = by_key.unwrap_or_else(fallback);
        let n = &work.nodes[node];
        let index = self
            .rev
            .and_then(|rev| u32::try_from(rev).ok())
            .and_then(|rev| n.position_of(rev))
            .or_else(|| n.current_index())
            .unwrap_or(0);
        let rev = n.revisions.get(index).map_or(0, |r| r.number);
        let pick = parse_pick(work, self.pick.as_deref());
        let preview = self.view.as_deref() == Some("preview");
        let show_all = self.fassungen.as_deref() == Some("alle");
        Resolved { node, key: n.key, index, rev, preview, pick, missing_key, saved: self.saved.clone(), show_all }
    }
}

/// Canonical address of a section (contract Round 8, §1):
/// `/?work=W&key=K[&rev=R][&view=preview][&pick=…][&lq=…]`, in this order. Only digits, fixed
/// names and percent-encoded values; `pick` and `lq` come finished as `&pick=…` and `&lq=…` (or empty).
pub fn node_url(work: WorkId, key: i64, rev: Option<u32>, preview: bool, pick_q: &str, lq_q: &str) -> String {
    let mut url = format!("/?work={work}&key={key}");
    if let Some(rev) = rev {
        url.push_str(&format!("&rev={rev}"));
    }
    if preview {
        url.push_str("&view=preview");
    }
    url.push_str(pick_q);
    url.push_str(lq_q);
    url
}

/// `"<node>:<rev>,…"` → valid pairs (section and revision exist); per section the last
/// entry counts, invalid ones are skipped. Result sorted by section.
pub fn parse_pick(work: &Work, pick: Option<&str>) -> Vec<(usize, u32)> {
    let mut chosen: Vec<(usize, u32)> = Vec::new();
    for entry in pick.unwrap_or("").split(',') {
        let Some((node, rev)) = entry.trim().split_once(':') else { continue };
        let (Ok(node), Ok(rev)) = (node.trim().parse::<usize>(), rev.trim().parse::<u32>()) else { continue };
        if work.nodes.get(node).and_then(|n| n.revision(rev)).is_none() {
            continue;
        }
        chosen.retain(|&(n, _)| n != node);
        chosen.push((node, rev));
    }
    chosen.sort_unstable();
    chosen
}

/// `&pick=2:3,4:1` for a non-empty selection, otherwise empty (only digits, `:` and `,`).
pub fn pick_param(pick: &[(usize, u32)]) -> String {
    if pick.is_empty() {
        return String::new();
    }
    let list: Vec<String> = pick.iter().map(|(node, rev)| format!("{node}:{rev}")).collect();
    format!("&pick={}", list.join(","))
}

/// First section with an open proposal, otherwise the very first.
fn default_node(work: &Work) -> usize {
    work.nodes.iter().find(|n| n.open_proposals() > 0).map_or(0, |n| n.id)
}

impl Resolved {
    /// Address of the place: `/?work=<id>&key=…&rev=…[&view=preview][&pick=…][&lq=…]` ([`node_url`]).
    pub fn url(&self, work: WorkId, lq: Option<&str>) -> String {
        let url = node_url(work, self.key, Some(self.rev), self.preview, &pick_param(&self.pick), &lq_param(lq));
        if self.show_all { format!("{url}{FOLD_OPEN_Q}") } else { url }
    }
}

/// 0.9.0: suffix of the address for the open switch „ältere Fassungen“ ([`Location::fassungen`]).
pub const FOLD_OPEN_Q: &str = "&fassungen=alle";

/// `&lq=<encoded>` for a non-empty search, otherwise empty.
pub fn lq_param(lq: Option<&str>) -> String {
    match lq.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => format!("&lq={}", query_escape(q)),
        None => String::new(),
    }
}

/// Percent-encoding for a value in the address: everything except `A–Z a–z 0–9 - . _ ~` as `%XX`.
pub fn query_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The shown work (id, folder name) and all works for the work choice („Titel · Autor · n Abschnitte“).
#[derive(Debug, Clone)]
pub struct Shelf {
    pub id: WorkId,
    pub slug: String,
    pub works: Vec<WorkSummary>,
}

#[derive(Serialize)]
struct TreeItem {
    id: usize,
    /// Fixed key (`Node::key`): the forms send it along, the position `id` can become outdated.
    key: i64,
    number: String,
    title: String,
    part: bool,
    section: bool,
    proposals: usize,
    /// Round 28: open findings including sub-items (counting rule §1.5, [`crate::findings::view::subtree_open`]);
    /// set by [`attach_findings`], 0 before.
    findings: usize,
    /// Of them at the item itself.
    findings_own: usize,
    selected: bool,
    /// Round 22 (R22a): further non-current revisions other than open proposals (replaced, rejected).
    alternatives: usize,
    /// Expanded: not collapsed, or the chosen section lies in it.
    open: bool,
    /// Level in the outline (part 0, chapter 1, section 2, subsection 3) and its name.
    level: u8,
    level_name: String,
    /// Neighbours on the same level under the same parent (targets for ↑ and ↓).
    up: Option<usize>,
    down: Option<usize>,
    up_key: Option<i64>,
    down_key: Option<i64>,
    /// ← (promote) or → (demote) possible; parts never.
    promote: bool,
    demote: bool,
    children: Vec<TreeItem>,
    /// Round 16b: speaking time (for items with sub-items including descendants) as "m:ss" and time mark;
    /// `None` if the work shows no times ([`crate::timing::WorkTiming::shown`]).
    duration: Option<String>,
    start: Option<String>,
}

/// Level per node from kind and parent: part 0, chapter 1, section 2, section under a section 3.
/// Nodes stand in tree order, so the parent comes before.
pub fn levels(work: &Work) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(work.nodes.len());
    for node in &work.nodes {
        let parent = node.parent.filter(|&p| p < out.len()).map(|p| out[p]);
        out.push(match node.kind {
            NodeKind::Part => 0,
            NodeKind::Chapter => 1,
            NodeKind::Section => parent.map_or(2, |p| (p + 1).clamp(2, 3)),
        });
    }
    out
}

/// Name of a level; for sermons level 1 is called „Predigtteil“.
pub fn level_name(kind: WorkKind, level: u8) -> String {
    match (level, kind) {
        (0, _) => tc("view_msg.level.part", &[]),
        (1, WorkKind::Sermon) => tc("view_msg.level.sermon_part", &[]),
        (1, WorkKind::Book) => tc("view_msg.level.chapter", &[]),
        (2, _) => tc("view_msg.level.section", &[]),
        _ => tc("view_msg.level.subsection", &[]),
    }
}

/// Previous and next node of the same level under the same parent.
pub fn neighbours(work: &Work, levels: &[u8], node: usize) -> (Option<usize>, Option<usize>) {
    let n = &work.nodes[node];
    let same: Vec<usize> = work.children(n.parent).filter(|s| levels[s.id] == levels[node]).map(|s| s.id).collect();
    let at = same.iter().position(|&s| s == node);
    let up = at.and_then(|i| i.checked_sub(1)).map(|i| same[i]);
    let down = at.and_then(|i| same.get(i + 1).copied());
    (up, down)
}

fn tree(work: &Work, parent: Option<usize>, selected: usize, closed: &[usize]) -> Vec<TreeItem> {
    tree_with(work, &levels(work), parent, selected, closed)
}

fn tree_with(work: &Work, levels: &[u8], parent: Option<usize>, selected: usize, closed: &[usize]) -> Vec<TreeItem> {
    work.children(parent)
        .map(|n| {
            let children = tree_with(work, levels, Some(n.id), selected, closed);
            let holds_selected = children.iter().any(|c| c.selected || c.holds_selected());
            let level = levels[n.id];
            let (up, down) = neighbours(work, levels, n.id);
            TreeItem {
                id: n.id,
                key: n.key,
                up_key: up.map(|u| work.nodes[u].key),
                down_key: down.map(|d| work.nodes[d].key),
                number: n.number.clone(),
                title: n.title.clone(),
                part: n.kind == NodeKind::Part,
                section: n.kind == NodeKind::Section,
                proposals: n.open_proposals(),
                findings: 0,
                findings_own: 0,
                selected: n.id == selected,
                alternatives: alternatives_of(n).len(),
                open: !closed.contains(&n.id) || holds_selected,
                level,
                level_name: level_name(work.kind, level),
                up,
                down,
                promote: level >= 2,
                demote: (1..=2).contains(&level) && n.id > 0,
                children,
                duration: None,
                start: None,
            }
        })
        .collect()
}

impl TreeItem {
    fn holds_selected(&self) -> bool {
        self.children.iter().any(|c| c.selected || c.holds_selected())
    }
}

// ---- Round 16b: time guard and releases of the personas ------------------------------------------------

/// Release of a persona from `persona.list` (Round 15b): connection local/external and whether a job may go out.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PersonaAccess {
    pub handle: String,
    /// „lokal“ or „extern“.
    pub locality: String,
    pub external_allowed: bool,
    /// Name of the connection (empty if none).
    pub connection: String,
    /// Effective context length in tokens without a network query (`persona.list` → `context.tokens`, integrator
    /// Round 17); `None` = not known, then the page shows the setting of the persona.
    pub context_tokens: Option<u32>,
}

impl PersonaAccess {
    /// External without a release: jobs would not go out, the persona is locked on the workspace page.
    pub fn blocked(&self) -> bool {
        self.locality == "extern" && !self.external_allowed
    }
}

/// What the workspace page needs beyond [`LibraryView`] (Round 16b): times of the work and releases of the
/// personas. Both read only.
#[derive(Debug, Clone, Default)]
pub struct ViewExtras {
    pub timing: Option<crate::timing::WorkTiming>,
    pub personas: Vec<PersonaAccess>,
    /// Lector according to the setting with the effective context length without a network query (`persona.list` →
    /// `lector`, integrator Round 17); `None` = no list loaded, then `Work::lector` applies.
    pub lector: Option<String>,
    /// Buttons of the ribbon according to the work template (Round 18a, [`crate::ribbon::ribbon`]).
    pub ribbon: crate::ribbon::Ribbon,
    /// CSS of the typeset view according to template and settings of the work (Round 18c,
    /// [`crate::sheet::sheet_css_for`]); `None` if it could not be computed (then the widget fetches it by RPC).
    pub sheet_css: Option<String>,
    /// Round 28: open findings per section and findings for the accordion ([`crate::findings::view::load`]).
    pub findings: crate::findings::view::FindingsView,
    /// Round 28: edited posts of the work, `post.id` → time of the last change ([`crate::posts::edited_map`]).
    pub post_edits: HashMap<i64, String>,
    /// Round 30: metadata of the posts of the work (only non-empty ones, [`baruch_store::Store::work_post_meta`]).
    pub post_meta: HashMap<i64, baruch_store::PostMeta>,
    /// Round 19c: box „Bibeln“ of the literature for the workspace page without JavaScript
    /// ([`crate::literature::bibles::workspace_shelf`]); `None` = not loaded (then no box).
    pub bibles: Option<crate::literature::bibles::Shelf>,
    /// 0.9.0: marks „ältere Fassungen ausblenden“ of the work, `Node::key` → number
    /// ([`baruch_store::Store::work_revision_folds`]).
    pub revision_folds: HashMap<i64, u32>,
}

/// Load times ([`crate::timing::work_timing`]) and releases of the personas (only through `persona.list`).
pub async fn extras(state: &crate::AppState, work: WorkId) -> Result<ViewExtras, crate::rpc::RpcError> {
    let timing = Some(crate::timing::work_timing(state, work).await?);
    let list = crate::personas::call(state, "persona.list", serde_json::json!({})).await?;
    let personas = list["personas"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|p| PersonaAccess {
                    handle: p["handle"].as_str().unwrap_or_default().to_owned(),
                    locality: p["locality"].as_str().unwrap_or("lokal").to_owned(),
                    external_allowed: p["external_allowed"].as_bool().unwrap_or(false),
                    connection: p["connection"]["name"].as_str().unwrap_or_default().to_owned(),
                    context_tokens: p["context"]["tokens"].as_u64().and_then(|t| u32::try_from(t).ok()),
                })
                .collect()
        })
        .unwrap_or_default();
    let lector = list["lector"].as_str().map(str::to_owned);
    let ribbon = crate::ribbon::ribbon(state, work).await?;
    let sheet_css = crate::sheet::sheet_css_for(state, work).await.ok();
    let findings = crate::findings::view::load(state, work).await.unwrap_or_default();
    let post_edits = crate::posts::edited_map(state, work).await.unwrap_or_default();
    let post_meta = state.store.work_post_meta(work).await.unwrap_or_default();
    let bibles = crate::literature::bibles::workspace_shelf(state, work).await;
    let revision_folds = state.store.work_revision_folds(work).await.unwrap_or_default();
    Ok(ViewExtras {
        timing,
        personas,
        lector,
        ribbon,
        sheet_css,
        findings,
        post_edits,
        post_meta,
        bibles,
        revision_folds,
    })
}

/// Hang open findings (including sub-items, or at the item itself) on the entries of the outline (Round 28).
fn attach_findings(items: &mut [TreeItem], sums: &[usize], own: &HashMap<i64, usize>) {
    for item in items {
        item.findings = sums.get(item.id).copied().unwrap_or(0);
        item.findings_own = own.get(&item.key).copied().unwrap_or(0);
        attach_findings(&mut item.children, sums, own);
    }
}

/// Hang duration and time mark on the entries of the outline (only if the work shows times). Items with
/// sub-items show the sum including descendants, leaves their own text.
fn attach_timing(items: &mut [TreeItem], timing: &crate::timing::WorkTiming) {
    use baruch_typst::timing::clock;
    for item in items {
        if let Some(section) = timing.sections.get(item.id) {
            let seconds = if item.children.is_empty() { section.seconds } else { section.subtree_seconds };
            item.duration = Some(clock(seconds));
            item.start = Some(clock(section.start_seconds));
        }
        attach_timing(&mut item.children, timing);
    }
}

/// Context `timing` for tree and editor header: `None` if the work shows no times (book without a target time).
fn timing_context(timing: Option<&crate::timing::WorkTiming>, node: &Node, shown_body: &str) -> Option<Value> {
    use baruch_typst::timing::{clock, speaking_seconds, spoken_words};
    let timing = timing.filter(|t| t.shown())?;
    let current = timing.sections.get(node.id).map_or(0, |s| s.seconds);
    let words = spoken_words(shown_body);
    let section = speaking_seconds(words, timing.words_per_minute);
    Some(context! {
        total => timing.seconds, total_clock => clock(timing.seconds),
        target => timing.target_seconds, target_clock => timing.target_seconds.map(clock),
        over => timing.over_seconds.is_some(), over_clock => timing.over_seconds.map(clock),
        words_per_minute => timing.words_per_minute, sermon => timing.kind == WorkKind::Sermon,
        // Shown revision of the section (in the editor what stands in the text field) and the sum without the
        // current revision of this section (for the live computation of the widget).
        section => section, section_clock => clock(section), section_words => words,
        rest => timing.seconds.saturating_sub(current),
        start_clock => timing.sections.get(node.id).map(|s| clock(s.start_seconds)),
    })
}

#[derive(Serialize)]
struct DiffLine {
    kind: &'static str,
    text: String,
}

#[derive(Serialize)]
struct PreviewLine {
    level: u8,
    text: String,
}

fn status_label(status: RevisionStatus) -> String {
    match status {
        RevisionStatus::Current => tc("view_msg.status.current", &[]),
        RevisionStatus::Proposed => tc("view_msg.status.proposed", &[]),
        RevisionStatus::Rejected => tc("view_msg.status.rejected", &[]),
        RevisionStatus::Superseded => tc("view_msg.status.superseded", &[]),
    }
}

fn author_label(author: &Author) -> String {
    match author {
        Author::Human { name } => name.clone(),
        // Round 35: a Lektor writes without a model – only the origin is shown, no empty "()".
        Author::Assistant { persona, model } if model.is_empty() && is_lektorat(persona) => persona.clone(),
        Author::Assistant { persona, model } => format!("{persona} ({model})"),
    }
}

/// Origin of a Lektor account (`@lektorat:<slug>`, [`crate::auth::lektor_handle`]) – a human, not a persona.
fn is_lektorat(handle: &str) -> bool {
    handle.starts_with("@lektorat:")
}

/// Chapter (or part) of a node: the node itself if it is not a section, otherwise the
/// nearest ancestor that is not a section (sections without such an ancestor stand for themselves).
fn chapter_of(work: &Work, node: usize) -> usize {
    let mut at = node;
    let mut steps = 0;
    while let Some(n) = work.nodes.get(at) {
        match (n.kind, n.parent) {
            (NodeKind::Section, Some(parent)) if steps < work.nodes.len() => {
                at = parent;
                steps += 1;
            }
            _ => break,
        }
    }
    at
}

/// The chapter and all nodes below it in tree order.
fn chapter_nodes(work: &Work, node: usize) -> Vec<&Node> {
    fn walk<'a>(work: &'a Work, id: usize, out: &mut Vec<&'a Node>, depth: usize) {
        let Some(n) = work.nodes.get(id) else { return };
        if depth > work.nodes.len() || out.iter().any(|o| o.id == id) {
            return; // loop in parent
        }
        out.push(n);
        for child in work.children(Some(id)) {
            walk(work, child.id, out, depth + 1);
        }
    }
    let mut out = Vec::new();
    walk(work, chapter_of(work, node), &mut out, 0);
    out
}

/// A section of the preview: its blocks (lines) with the open proposals at their place.
#[derive(Serialize)]
struct PreviewSection {
    node: usize,
    flow: InlineView,
}

/// Placeholder for the Typst preview: texts of the chapter, headings by `=`. Per section the
/// current revision, or the one chosen on trial in `pick` (the mark stays untouched); the
/// open proposals of every section stand at their place between its blocks (Round 8, only
/// line diff), the base is the shown revision of the section.
fn preview(work: &Work, node: &Node, pick: &[(usize, u32)], ctx: &ProposalContext<'_>) -> Vec<PreviewSection> {
    let mut sections = Vec::new();
    for n in chapter_nodes(work, node.id) {
        let picked = pick.iter().find(|(id, _)| *id == n.id).and_then(|&(_, rev)| n.revision(rev));
        let Some(revision) = picked.or_else(|| n.current_index().map(|i| &n.revisions[i])) else { continue };
        let proposals = proposals_of(n, revision, ctx, false);
        sections.push(PreviewSection { node: n.id, flow: inline_view(n, revision, proposals, true, None) });
    }
    sections
}

/// Lines of a text for the preview: headings `=`, `==`, `===`, otherwise text; empty ones are dropped.
fn preview_lines(body: &str) -> Vec<PreviewLine> {
    let mut lines = Vec::new();
    for line in body.lines() {
        // Headings `=`, `==`, `===`, otherwise text (placeholder until the Typst typesetting).
        let equals = line.bytes().take_while(|&b| b == b'=').count();
        let (level, text) = match line.get(equals..) {
            Some(rest) if (1..=3).contains(&equals) && rest.starts_with(' ') => (equals as u8, rest.trim().to_owned()),
            _ => (0, line.to_owned()),
        };
        if !text.trim().is_empty() {
            lines.push(PreviewLine { level, text });
        }
    }
    lines
}

/// What the proposals in the reading flow need: work (for image addresses), revision tree (base),
/// persona colours (order, name), alternative texts of the images and collapsed proposals. If the
/// shown revision is a proposal, it stands open and highlighted ([`proposals_of`]).
struct ProposalContext<'a> {
    work: WorkId,
    revisions: &'a [NodeRevisions],
    colors: &'a [PersonaColor],
    image_alts: &'a HashMap<String, String>,
    folded: &'a [(i64, u32)],
    /// Work language: page abbreviation of cross references in the typeset view (Round 39b).
    lang: baruch_core::WorkLanguage,
}

impl ProposalContext<'_> {
    /// Image for a path as in the typeset view (`wysiwyg.parse`): address of the image page, alternative text
    /// by normal form.
    fn image(&self, path: &str) -> Option<baruch_wysiwyg::ImageRef> {
        Some(crate::wysiwyg::image_ref_in(self.work, path, self.image_alts))
    }
}

/// State of `counter(heading).get().first()` in the chapter of a node: the number of nodes up to
/// this chapter (tree order) whose heading Typst counts (`counted`, from
/// [`baruch_store::Store::counted_headings`]). Typst counts only numbered headings: a chapter with
/// „Nummer aus“ (e.g. a preface) does not count; in it the state before applies (before the first: 0).
/// `Some(None)` = state not known for certain; `None` for parts and sections without a chapter.
fn chapter_number(work: &Work, node: usize, counted: Option<&[i64]>) -> Option<Option<usize>> {
    let chapter = chapter_of(work, node);
    if work.nodes.get(chapter)?.kind != NodeKind::Chapter {
        return None;
    }
    Some(counted.map(|keys| work.nodes.iter().filter(|n| n.id <= chapter && keys.contains(&n.key)).count()))
}

/// Chapter image for the editor header (Round 11): determined for certain from the template (image, caption,
/// alternative text; `missing` if the path belongs to no image of the work), otherwise only the note
/// „Bild aus der Vorlage“; without computed images in the template nothing.
fn chapter_image(work: &Work, node: usize, work_id: WorkId, images: &crate::wysiwyg::ImageView) -> Option<Value> {
    use crate::wysiwyg::ChapterImage;
    let number = chapter_number(work, node, images.counted.as_deref())?;
    Some(match images.chapter_image(number)? {
        ChapterImage::Known { path, asset } => {
            // Image names consist only of `[a-z0-9._-]` (valid_asset_name): the addresses are safe.
            let url = |tail: &str| {
                asset.as_ref().map(|a| Value::from_safe_string(format!("/werke/{work_id}/bilder/{}{tail}", a.name)))
            };
            context! {
                known => true, chapter => number, path => crate::assets::path_text(&path),
                missing => asset.is_none(), name => asset.as_ref().map(|a| a.name.clone()),
                caption => asset.as_ref().map(|a| a.caption.clone()), alt => asset.as_ref().map(|a| a.alt.clone()),
                thumb => url("/vorschau?b=320"), detail => url(""),
            }
        }
        ChapterImage::FromTemplate { expr } => {
            context! { known => false, chapter => number, expr => crate::assets::path_text(&expr) }
        }
    })
}

/// Image paths of the preview for which there is no image in the work (Round 11: report clearly), without
/// duplicates in order; plus the path of the chapter image if it is missing.
fn missing_images(bodies: &[&str], images: &crate::wysiwyg::ImageView) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for body in bodies {
        for path in baruch_typst::images::image_paths(body) {
            if images.asset_by_path(&path).is_none() && !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// An open proposal (forms „Übernehmen“/„Verwerfen“ once per proposal) with its
/// pieces at their place.
#[derive(Serialize)]
struct ProposalView {
    /// `<key>-<rev>`: entry in the cookie `baruch_folded`, `data-pv` and part of the form ids.
    id: String,
    key: i64,
    node: usize,
    rev: u32,
    #[serde(skip)]
    hunks: Vec<HunkView>,
}

/// A piece of an open proposal at its place (`fragments/proposals.html`, Round 8): per
/// piece a `<details class="pv pc">` at the block `at` of the shown revision.
#[derive(Serialize)]
struct HunkView {
    #[serde(flatten)]
    meta: HunkMeta,
    /// Index of the first affected block of the shown revision (`data-b`), for inserted ones the
    /// block before which is inserted.
    at: usize,
    /// Gap in which the piece stands: before block `gap` (= behind block `gap - 1`; at the end the block count).
    gap: usize,
    kind: &'static str,
    /// Typeset view of the piece (`baruch_wysiwyg::hunk_html_with`, only in the editor).
    satz: Option<String>,
    lines: Vec<DiffLine>,
}

/// What all pieces of a proposal share.
#[derive(Serialize, Clone)]
struct HunkMeta {
    id: String,
    handle: String,
    /// Name of the persona; without a persona entry the handle.
    name: String,
    note: String,
    /// Is there a persona entry for the author? (otherwise `FALLBACK_COLOR`, at the end of the list)
    known: bool,
    /// CSS variables `--pc-light`, `--pc-dark`, `--pc-line`, `--pc-line-dark` (only checked `#rrggbb`).
    style: String,
    /// The only header: „Name (@handle) · Fassung n[ · Modell][ · auf Fassung m][ · Teil i/k]“.
    head: String,
    /// Only for the shown proposal: „Unterschied zu Fassung b[ (aktuell)] · gezeigt“ (its pieces
    /// then stand at their place in it itself, compared with its base); or the note
    /// that it equals the shown revision.
    base_note: Option<String>,
    open: bool,
    shown: bool,
}

/// Entry in the reading flow: a block of the shown revision (`b`, `raw`, in the preview `lines`) or
/// the pieces that stand at this place.
#[derive(Serialize)]
struct FlowEntry {
    b: Option<usize>,
    raw: String,
    lines: Vec<PreviewLine>,
    hunks: Vec<HunkView>,
}

/// Reading flow of a section with the open proposals at their place (editor: `proposals`,
/// preview: `flow` per section). `sheet`: in the editor the typeset view of the shown revision, finished by the
/// server (like `wysiwyg.parse`), so that the widget opens it without waiting for RPC (Round 8, 2d).
#[derive(Serialize)]
struct InlineView {
    key: i64,
    rev: u32,
    count: usize,
    /// Round 22 (R22a): number of the further non-current revisions other than proposals (note in the preview).
    alternatives: usize,
    proposals: Vec<ProposalView>,
    entries: Vec<FlowEntry>,
    sheet: Option<String>,
}

/// Persona entry for a handle: position in the order and colour.
fn color_of<'a>(colors: &'a [PersonaColor], handle: &str) -> Option<(usize, &'a PersonaColor)> {
    colors.iter().enumerate().find(|(_, c)| c.handle == handle)
}

/// Style of a persona (colour from the list, otherwise `FALLBACK_COLOR`).
fn persona_style(colors: &[PersonaColor], handle: &str) -> String {
    crate::personas::style(color_of(colors, handle).map_or(FALLBACK_COLOR, |(_, c)| c.color.as_str()))
}

/// Line diff for the template.
fn diff_lines(base: &str, proposal: &str) -> Vec<DiffLine> {
    line_diff(base, proposal)
        .into_iter()
        .map(|(kind, text)| DiffLine {
            kind: match kind {
                DiffKind::Same => "same",
                DiffKind::Added => "added",
                DiffKind::Removed => "removed",
            },
            text,
        })
        .collect()
}

/// Source text of the top-level blocks `range` (cut to the block count).
fn raw_of(blocks: &[baruch_wysiwyg::Block], range: &std::ops::Range<usize>) -> String {
    let (start, end) = (range.start.min(blocks.len()), range.end.min(blocks.len()));
    blocks[start..end].iter().map(|b| b.raw.as_str()).collect()
}

/// Open proposals of a section, by persona order (`position`), within it by number;
/// authors without a persona entry at the end. Each with its pieces against the shown revision
/// `shown` (Round 8: the base of the overlay is what „Übernehmen“ replaces). If `shown` itself is the
/// proposal, its pieces stand against its base (parent, otherwise the current one) at their place
/// in it. `satz`: also generate the typeset view of the pieces.
fn proposals_of(
    node: &Node,
    shown: &baruch_core::Revision,
    ctx: &ProposalContext<'_>,
    satz: bool,
) -> Vec<ProposalView> {
    let infos = infos_of(ctx.revisions, node.id);
    let current = node.current_number();
    let shown_blocks = baruch_wysiwyg::parse(&shown.body).blocks;
    let mut out: Vec<(usize, ProposalView)> = node
        .revisions
        .iter()
        .filter(|r| r.status == RevisionStatus::Proposed)
        .map(|revision| {
            let (handle, model) = match &revision.author {
                Author::Assistant { persona, model } => (persona.clone(), model.clone()),
                Author::Human { name } => (name.clone(), String::new()),
            };
            let entry = color_of(ctx.colors, &handle);
            let name = entry.map_or(handle.clone(), |(_, c)| c.name.clone());
            let based_on = infos
                .iter()
                .find(|i| i.number == revision.number)
                .and_then(|i| i.based_on)
                .filter(|&b| node.revision(b).is_some());
            let is_shown = shown.number == revision.number;
            // Shown is the proposal itself: its pieces against its base, positions in it.
            let base =
                if is_shown { based_on.or(current).filter(|&b| b != revision.number) } else { Some(shown.number) };
            let base_body = base.and_then(|b| node.revision(b)).map_or("", |r| r.body.as_str());
            let base_note = if is_shown {
                Some(match base {
                    Some(b) if base == current => tc("view_msg.proposal.diff_current", &[("rev", b.into())]),
                    Some(b) => tc("view_msg.proposal.diff", &[("rev", b.into())]),
                    None => tc("view_msg.proposal.shown", &[]),
                })
            } else {
                None
            };
            // A Lektor is an account, not a persona: there is no name next to the handle – do not name it twice.
            let mut head = if is_lektorat(&handle) && name == handle {
                tc(
                    "view_msg.proposal.head_lektor",
                    &[("handle", handle.as_str().into()), ("rev", revision.number.into())],
                )
            } else {
                tc(
                    "view_msg.proposal.head",
                    &[
                        ("name", name.as_str().into()),
                        ("handle", handle.as_str().into()),
                        ("rev", revision.number.into()),
                    ],
                )
            };
            if !model.is_empty() {
                head.push_str(&format!(" · {model}"));
            }
            // Round 22 (§2.5): state of the base if it was amended („auf Fassung n, Stand k“).
            let base_amend =
                infos.iter().find(|i| i.number == revision.number).and_then(|i| i.based_on_amend).filter(|&k| k > 0);
            match (based_on.filter(|&b| b != shown.number), base_amend) {
                (Some(b), Some(k)) => {
                    head.push_str(" · ");
                    head.push_str(&tc("view_msg.proposal.based_on_amend", &[("rev", b.into()), ("amend", k.into())]));
                }
                (Some(b), None) => {
                    head.push_str(" · ");
                    head.push_str(&tc("view_msg.proposal.on_rev", &[("rev", b.into())]));
                }
                (None, Some(k)) => {
                    head.push_str(" · ");
                    head.push_str(&tc("view_msg.proposal.based_on_state", &[("amend", k.into())]));
                }
                (None, None) => {}
            }
            // Review R22: base amended further since the proposal → note „veraltet“; accepting would take
            // the later amendments out of the current text.
            let raw_amend = infos.iter().find(|i| i.number == revision.number).and_then(|i| i.based_on_amend);
            if let (Some(b), Some(k)) = (based_on, raw_amend)
                && let Some(now) = infos.iter().find(|i| i.number == b).map(|i| i.amend_seq)
                && now > k
            {
                head.push_str(" · ");
                head.push_str(&tc("view_msg.proposal.stale", &[("rev", b.into()), ("now", now.into())]));
            }
            let hunks = baruch_wysiwyg::align(base_body, &revision.body);
            let (old, new) = (baruch_wysiwyg::parse(base_body).blocks, baruch_wysiwyg::parse(&revision.body).blocks);
            let id = format!("{}-{}", node.key, revision.number);
            let meta = HunkMeta {
                id: id.clone(),
                style: persona_style(ctx.colors, &handle),
                known: entry.is_some(),
                open: is_shown || !ctx.folded.iter().any(|&f| f == (node.key, revision.number) || f == FOLD_ALL),
                shown: is_shown,
                handle,
                name,
                note: revision.note.clone(),
                head: head.clone(),
                base_note,
            };
            let parts = hunks.len();
            let mut views: Vec<HunkView> = hunks
                .iter()
                .enumerate()
                .map(|(k, hunk)| {
                    // Position in the shown revision: for the shown proposal in it itself.
                    let place = if is_shown { &hunk.proposal } else { &hunk.base };
                    let mut meta = meta.clone();
                    if parts > 1 {
                        meta.head = format!(
                            "{head} · {}",
                            tc("view_msg.proposal.part", &[("k", (k + 1).into()), ("parts", parts.into())])
                        );
                    }
                    HunkView {
                        meta,
                        at: place.start,
                        gap: if place.is_empty() { place.start } else { place.end },
                        kind: match hunk.kind {
                            baruch_wysiwyg::HunkKind::Changed => "changed",
                            baruch_wysiwyg::HunkKind::Inserted => "inserted",
                            baruch_wysiwyg::HunkKind::Removed => "removed",
                        },
                        satz: satz.then(|| {
                            baruch_wysiwyg::hunk_html_in(base_body, &revision.body, hunk, &|p| ctx.image(p), ctx.lang)
                        }),
                        // Without whitespace at the end: the empty line behind a block belongs to its `raw` and
                        // otherwise stood as an empty "−" line at the end of the piece (backlog Round 8, Round 18c).
                        lines: diff_lines(raw_of(&old, &hunk.base).trim_end(), raw_of(&new, &hunk.proposal).trim_end()),
                    }
                })
                .collect();
            if views.is_empty() {
                // Equals the shown revision: one overlay at the end, so that accept/reject stay.
                let mut meta = meta;
                if meta.base_note.is_none() {
                    meta.base_note = Some(tc("view_msg.proposal.equals_shown", &[]));
                }
                let end = shown_blocks.len();
                views.push(HunkView { meta, at: end, gap: end, kind: "changed", satz: None, lines: Vec::new() });
            }
            let proposal = ProposalView { id, key: node.key, node: node.id, rev: revision.number, hunks: views };
            (entry.map_or(usize::MAX, |(position, _)| position), proposal)
        })
        .collect();
    out.sort_by_key(|(position, view)| (*position, view.rev));
    out.into_iter().map(|(_, view)| view).collect()
}

/// Reading flow of the shown revision `shown` of a section: its top-level blocks and between them the
/// pieces of all open proposals in their gap, at the same place one below another in
/// persona order, within it by number (order of [`proposals_of`]). `lines`: lines of the
/// preview per block; `sheet`: finished typeset view for the editor.
fn inline_view(
    node: &Node,
    shown: &baruch_core::Revision,
    mut proposals: Vec<ProposalView>,
    lines: bool,
    sheet: Option<String>,
) -> InlineView {
    let blocks = baruch_wysiwyg::parse(&shown.body).blocks;
    let mut gaps: Vec<Vec<HunkView>> = (0..=blocks.len()).map(|_| Vec::new()).collect();
    for proposal in &mut proposals {
        for hunk in std::mem::take(&mut proposal.hunks) {
            gaps[hunk.gap.min(blocks.len())].push(hunk);
        }
    }
    let mut entries = Vec::new();
    for (gap, hunks) in gaps.into_iter().enumerate() {
        if !hunks.is_empty() {
            entries.push(FlowEntry { b: None, raw: String::new(), lines: Vec::new(), hunks });
        }
        if let Some(block) = blocks.get(gap) {
            let shown_lines = if lines { preview_lines(&block.raw) } else { Vec::new() };
            entries.push(FlowEntry { b: Some(gap), raw: block.raw.clone(), lines: shown_lines, hunks: Vec::new() });
        }
    }
    let alternatives = alternatives_of(node).len();
    InlineView { key: node.key, rev: shown.number, count: proposals.len(), alternatives, proposals, entries, sheet }
}

/// Largest section text (bytes, 64 KiB) for which the server puts the typeset view into the HTML in advance
/// (Round 18c).
pub const READY_SHEET_MAX: usize = 64 * 1024;

/// Typeset view of the shown revision finished by the server (like `wysiwyg.parse`), so that the widget shows
/// it at once on loading (Round 8, 2d). Only if the text field delivers the same text the
/// indices `data-b` refer to: the browser strips a line break directly behind `<textarea>` and
/// turns `\r\n`/`\r` into a `\n`; then the widget fetches the typeset view through RPC as before.
///
/// Round 18c: sections over [`READY_SHEET_MAX`] bytes get no typeset view set in advance (it would double the
/// text in the HTML: text field, sheet, with proposals also the reading flow); the widget fetches it by RPC.
fn ready_sheet(body: &str, ctx: &ProposalContext<'_>) -> Option<String> {
    if body.len() > READY_SHEET_MAX || body.starts_with('\n') || body.contains('\r') {
        return None;
    }
    let doc = baruch_wysiwyg::parse(body);
    Some(baruch_wysiwyg::render_html_in(&doc, &|p| ctx.image(p), ctx.lang))
}

/// A revision in the small tree view (indented by `based_on`).
#[derive(Serialize)]
struct TreeRevision {
    number: u32,
    depth: usize,
    based_on: Option<u32>,
    status: String,
    status_key: &'static str,
    author: String,
    persona: bool,
    /// For persona revisions the CSS variables of their colour.
    style: Option<String>,
    created: String,
    edited: Option<String>,
    current: bool,
    shown: bool,
    show_url: String,
    try_url: String,
    /// Round 28: accepted findings this revision implements ([`baruch_store::Store::findings_adopted_in`]).
    findings: Vec<i64>,
    /// Findings with an open proposal „übernommen in dieser Fassung“.
    proposed_findings: Vec<i64>,
    /// `/werke/{work}/befunde`
    findings_base: String,
    /// Round 30b: marker of a variant ([`crate::assist::variants::revision_variant`]); `None` = none.
    variant: Option<serde_json::Value>,
    /// Round 22: state of the revision (0 = never amended) and link to its history (only for state > 0).
    amend_seq: u32,
    history_url: Option<String>,
    /// Round 22: state of the base of a proposal („beruht auf Fassung n, Stand k“, only for k > 0).
    based_on_amend: Option<u32>,
    /// 0.9.0: hidden under the mark (stands behind the switch „n ältere Fassungen“).
    hidden: bool,
    /// 0.9.0: „bis hierhin ausblenden“ is offered at this revision (only in the shown section).
    fold: bool,
}

/// Time stamp `2026-11-05T10:11:12.123Z` (UTC) → `2026-11-05 11:11` in local time.
fn short_time(time: &str) -> String {
    crate::localtime::minutes(time)
}

fn status_key(status: RevisionStatus) -> &'static str {
    match status {
        RevisionStatus::Current => "current",
        RevisionStatus::Proposed => "proposed",
        RevisionStatus::Rejected => "rejected",
        RevisionStatus::Superseded => "superseded",
    }
}

/// Revisions of a section in tree order: roots (without a parent or with an unknown one)
/// by number, below them the children by number each, with depth.
fn ordered_tree(infos: &[RevisionInfo]) -> Vec<(usize, &RevisionInfo)> {
    let known: Vec<u32> = infos.iter().map(|r| r.number).collect();
    let mut children: HashMap<Option<u32>, Vec<&RevisionInfo>> = HashMap::new();
    for info in infos {
        let parent = info.based_on.filter(|b| known.contains(b) && *b < info.number);
        children.entry(parent).or_default().push(info);
    }
    for list in children.values_mut() {
        list.sort_by_key(|r| r.number);
    }
    let mut out = Vec::with_capacity(infos.len());
    let mut stack: Vec<(usize, &RevisionInfo)> =
        children.get(&None).map(|roots| roots.iter().rev().map(|r| (0, *r)).collect()).unwrap_or_default();
    while let Some((depth, info)) = stack.pop() {
        out.push((depth, info));
        if let Some(list) = children.get(&Some(info.number)) {
            stack.extend(list.iter().rev().map(|r| (depth + 1, *r)));
        }
    }
    out
}

/// Context for the links of the tree view.
struct TreeLinks<'a> {
    work: WorkId,
    at: &'a Resolved,
    lq_q: &'a str,
    colors: &'a [PersonaColor],
    findings: &'a crate::findings::view::FindingsView,
    /// 0.9.0: sections of the work and their marks (hidden revisions).
    nodes: &'a [Node],
    folds: &'a HashMap<i64, u32>,
}

/// Revisions of a section (`node` = position, `key` = fixed key) for the tree view; the
/// links carry the key (`zeigen`) or that of the shown section (`probieren`), `pick` stays
/// `<seq>:<rev>`.
fn tree_revisions(node: usize, key: i64, infos: &[RevisionInfo], links: &TreeLinks<'_>) -> Vec<TreeRevision> {
    let mark = links.folds.get(&key).copied();
    ordered_tree(infos)
        .into_iter()
        .map(|(depth, info)| {
            let mut pick: Vec<(usize, u32)> = links.at.pick.iter().copied().filter(|&(n, _)| n != node).collect();
            pick.push((node, info.number));
            pick.sort_unstable();
            let (work, at, lq_q) = (links.work, links.at, links.lq_q);
            TreeRevision {
                number: info.number,
                depth,
                based_on: info.based_on,
                status: status_label(info.status),
                status_key: status_key(info.status),
                author: author_label(&info.author),
                persona: matches!(&info.author, Author::Assistant { persona, .. } if !is_lektorat(persona)),
                style: match &info.author {
                    Author::Assistant { persona, .. } => Some(persona_style(links.colors, persona)),
                    Author::Human { .. } => None,
                },
                created: short_time(&info.created),
                edited: info.edited.as_deref().map(short_time),
                current: info.status == RevisionStatus::Current,
                shown: node == at.node && info.number == at.rev,
                show_url: node_url(work, key, Some(info.number), false, "", lq_q),
                try_url: node_url(work, at.key, Some(at.rev), true, &pick_param(&pick), lq_q),
                findings: links.findings.adopted_in(key, info.number),
                proposed_findings: links.findings.proposed_for(key, info.number),
                findings_base: format!("/werke/{work}/befunde"),
                variant: crate::assist::variants::revision_variant(info),
                amend_seq: info.amend_seq,
                history_url: (info.amend_seq > 0).then(|| history_url(work, key, info.number)),
                based_on_amend: info.based_on_amend.filter(|&k| k > 0),
                hidden: links
                    .nodes
                    .get(node)
                    .is_some_and(|n| n.revision(info.number).is_some_and(|r| revision_fold::is_hidden(n, mark, r))),
                fold: node == at.node
                    && links.nodes.get(node).is_some_and(|n| revision_fold::offered(n, mark, info.number)),
            }
        })
        .collect()
}

/// 0.9.0 (user 2026-11-24): mark „ältere Fassungen ausblenden“ of the shown section for header and accordion.
#[derive(Debug, Serialize)]
struct FoldView {
    /// Stored mark (number) and effective limit (at most the current revision).
    mark: Option<u32>,
    cutoff: Option<u32>,
    /// Number of the hidden revisions.
    hidden: usize,
    /// Switch open: `?fassungen=alle` or the shown revision is itself hidden.
    open: bool,
    /// Counter and neighbours over the visible revisions (over all with the switch open).
    position: usize,
    total: usize,
    prev: Option<u32>,
    next: Option<u32>,
    /// Suffix of the paging links (`&fassungen=alle` with the switch open, otherwise empty).
    q: &'static str,
    /// „zeigen“ (switch open) and „verbergen“ (switch closed; if the shown revision stands under the mark, to the
    /// current revision).
    show_url: String,
    hide_url: String,
    /// „bis hierhin ausblenden“ at the shown revision, and all revisions with this offer (forms
    /// `rev-fold-<n>` in the accordion, buttons in the header and in the tree view point to them through `form`).
    offered: bool,
    numbers: Vec<u32>,
}

fn fold_view(
    node: &Node,
    revision: &baruch_core::Revision,
    at: &Resolved,
    folds: &HashMap<i64, u32>,
    work: WorkId,
    lq_q: &str,
    pick_q: &str,
) -> FoldView {
    let mark = folds.get(&node.key).copied();
    let hidden = revision_fold::hidden_count(node, mark);
    let open = hidden > 0 && (at.show_all || revision_fold::is_hidden(node, mark, revision));
    let visible: Vec<u32> = node
        .revisions
        .iter()
        .filter(|r| open || r.number == revision.number || !revision_fold::is_hidden(node, mark, r))
        .map(|r| r.number)
        .collect();
    let index = visible.iter().position(|&n| n == revision.number).unwrap_or(0);
    let url = |rev: u32| node_url(work, node.key, Some(rev), at.preview, pick_q, lq_q);
    let back = if revision_fold::is_hidden(node, mark, revision) {
        node.current_number().unwrap_or(revision.number)
    } else {
        revision.number
    };
    FoldView {
        mark,
        cutoff: revision_fold::cutoff(node, mark),
        hidden,
        open,
        position: index + 1,
        total: visible.len(),
        prev: index.checked_sub(1).and_then(|i| visible.get(i).copied()),
        next: visible.get(index + 1).copied(),
        q: if open { FOLD_OPEN_Q } else { "" },
        show_url: format!("{}{FOLD_OPEN_Q}", url(revision.number)),
        hide_url: url(back),
        offered: revision_fold::offered(node, mark, revision.number),
        numbers: node.revisions.iter().map(|r| r.number).filter(|&n| revision_fold::offered(node, mark, n)).collect(),
    }
}

/// Round 22: page „Verlauf“ of a revision (`GET /werke/{id}/fassungen/verlauf?key=K&rev=R`).
pub fn history_url(work: WorkId, key: i64, rev: u32) -> String {
    format!("/werke/{work}/fassungen/verlauf?key={key}&rev={rev}")
}

/// Round 22 (R22a): non-current revisions of a section other than open proposals (replaced, rejected), by
/// number – the note „weitere Fassung(en)“ in the reading flow and in the tree.
fn alternatives_of(node: &Node) -> Vec<&baruch_core::Revision> {
    node.revisions
        .iter()
        .filter(|r| matches!(r.status, RevisionStatus::Superseded | RevisionStatus::Rejected))
        .collect()
}

/// Round 22: feedback after saving without JavaScript from `saved` (`amended-<k>`, `new-<n>[-<grund>]`);
/// invalid → `None`.
fn saved_notice(saved: Option<&str>) -> Option<String> {
    let saved = saved?;
    let mut parts = saved.split('-');
    match (parts.next()?, parts.next()?.parse::<u32>().ok()?, parts.next(), parts.next()) {
        ("amended", k, None, None) => Some(tc("view_msg.saved.amended", &[("amend", k.into())])),
        ("new", n, None, None) => Some(tc("view_msg.saved.new", &[("rev", n.into())])),
        ("new", n, Some(reason), None) => {
            let label = REASONS.iter().find(|r| r.id() == reason)?.label();
            Some(tc("view_msg.saved.new_reason", &[("rev", n.into()), ("reason", label.into())]))
        }
        _ => None,
    }
}

/// All reasons for a new revision (for [`saved_notice`]).
const REASONS: [baruch_store::NewReason; 8] = [
    baruch_store::NewReason::Requested,
    baruch_store::NewReason::NoCurrent,
    baruch_store::NewReason::PolicyOff,
    baruch_store::NewReason::NotOwn,
    baruch_store::NewReason::NotOnCurrent,
    baruch_store::NewReason::HeadingChanged,
    baruch_store::NewReason::LargeChange,
    baruch_store::NewReason::TimeElapsed,
];

/// Revisions of a section from the revision tree of the work (empty if unknown).
fn infos_of(tree: &[NodeRevisions], node: usize) -> &[RevisionInfo] {
    tree.iter().find(|n| n.node == node).map_or(&[], |n| n.revisions.as_slice())
}

/// Posts with resolved references (number, title, address of the place in key form), id and
/// instant (`data-post-id`, `data-post-created` for the widget "chat": new since the last visit).
/// Round 28: only the posts in [`crate::posts::visible_range`], per post additionally `edited` (time of the last
/// change or none), `edited_time` (short) and `own` (post of the human, that is editable).
/// Round 30 (hook): `extra` ([`crate::chat_render::post_extra`]) and `solutions`
/// ([`crate::assist::solutions::post_solutions`] against the shown revision `shown_rev`).
#[allow(clippy::too_many_arguments)]
fn posts(
    work: &Work,
    node: &Node,
    work_id: WorkId,
    lq: &str,
    colors: &[PersonaColor],
    edits: &HashMap<i64, String>,
    shown_rev: u32,
    extras: &ViewExtras,
    lektor: Option<&crate::auth::Identity>,
) -> Value {
    let shown_body = node.revisions.iter().find(|r| r.number == shown_rev).map_or("", |r| r.body.as_str());
    let ctx = crate::chat_render::PostCtx { work_id, work, key: node.key, shown_rev, extras };
    let range = crate::posts::visible_range(node.posts.len());
    let posts: Vec<Value> = node.posts[range]
        .iter()
        .map(|post| {
            let refs: Vec<Value> = post
                .refs
                .iter()
                .filter_map(|r| work.nodes.get(r.node).map(|target| (r, target)))
                .map(|(r, target)| {
                    context! {
                        number => target.number.as_str(), title => target.title.as_str(),
                        rev => r.rev, quote => r.quote.as_str(),
                        url => node_url(work_id, target.key, Some(r.rev), false, "", lq),
                    }
                })
                .collect();
            // Colour only for posts of personas (handle with an entry), not for the human.
            let style = color_of(colors, &post.author).map(|(_, c)| crate::personas::style(&c.color));
            let edited = edits.get(&post.id);
            context! {
                id => post.id, created => post.created.as_str(), time => short_time(&post.created),
                author => post.author.as_str(), body => post.body.as_str(), style => style,
                sources => Value::from(Serde(&post.sources)), refs => refs,
                edited => edited.map(String::as_str), edited_time => edited.map(|t| short_time(t)),
                own => post.author == crate::mirror::HUMAN_NAME,
                // Round 35: own post of a Lektor (author of the post = his origin) – only that one may he
                // change or remove.
                mine => lektor.is_some_and(|l| post.author == l.handle),
                extra => post_extra_for(&ctx, post, lektor.is_some()),
                solutions => Value::from(Serde(crate::assist::solutions::post_solutions(&post.body, shown_body))),
            }
        })
        .collect();
    Value::from(posts)
}

/// Extra of a post for the template ([`crate::chat_render::post_extra`]). For Lektors with an empty list of the
/// cards of the finding blocks: the cards carry the forms for the status (only the author); without a card the
/// template shows the finding as ordinary text of the post. The list stays as an empty map – the template looks
/// things up in it, and a lookup in a missing value would be an error of the template.
fn post_extra_for(ctx: &crate::chat_render::PostCtx<'_>, post: &baruch_core::Post, lektor: bool) -> Value {
    let extra = crate::chat_render::post_extra(ctx, post);
    if !lektor {
        return Value::from(Serde(extra));
    }
    match serde_json::to_value(&extra) {
        Ok(serde_json::Value::Object(mut map)) => {
            map.insert("finding_cards".to_owned(), serde_json::json!({}));
            Value::from(Serde(serde_json::Value::Object(map)))
        }
        Ok(other) => Value::from(Serde(other)),
        Err(_) => Value::from(()),
    }
}

/// Targets for „Verschieben…“ in the section chat: all sections of the work except the shown one, each
/// `{key, label}` with "number title" (indented by level).
fn post_targets(work: &Work, shown: usize) -> Vec<Value> {
    let levels = levels(work);
    work.nodes
        .iter()
        .filter(|n| n.id != shown)
        .map(|n| {
            // Non-breaking spaces: ordinary ones would collapse in <option>.
            let indent = "\u{a0}\u{a0}".repeat(usize::from(levels[n.id].saturating_sub(1)));
            let untitled = tc("view_msg.xref.untitled", &[]);
            let title = if n.title.is_empty() { untitled.as_str() } else { n.title.as_str() };
            let label =
                if n.number.is_empty() { format!("{indent}{title}") } else { format!("{indent}{} {title}", n.number) };
            context! { key => n.key, label => label }
        })
        .collect()
}

/// Accordion „Befunde“ at the shown section (Round 28): open findings (without praise, attached ones below the
/// original) as cards with status forms, decided ones and praise collapsed, open proposals, targets for
/// „verschoben“ and the return to the workspace page.
fn findings_panel(
    work: &Work,
    node: &Node,
    work_id: WorkId,
    at: &Resolved,
    findings: &crate::findings::view::FindingsView,
    lq: &Option<String>,
) -> serde_json::Value {
    use crate::findings::{FindingKind, FindingStatus};
    let all = &findings.findings;
    let own: Vec<&crate::findings::Finding> = findings.of_key(node.key).collect();
    // Attached findings stand below their original (in the card), not on their own.
    let top: Vec<&crate::findings::Finding> =
        own.iter().copied().filter(|f| f.similar_to.is_none_or(|o| !own.iter().any(|x| x.id == o))).collect();
    let cards = |keep: &dyn Fn(&crate::findings::Finding) -> bool| -> Vec<serde_json::Value> {
        top.iter().filter(|f| keep(f)).map(|f| crate::findings::view::card(work_id, work, f, all)).collect()
    };
    let open = cards(&|f| f.kind != FindingKind::Praise && (f.status == FindingStatus::Open || f.pending.is_some()));
    let decided = cards(&|f| f.kind != FindingKind::Praise && f.status != FindingStatus::Open && f.pending.is_none());
    let praise = cards(&|f| f.kind == FindingKind::Praise);
    let back = node_url(work_id, node.key, Some(at.rev), at.preview, "", &lq_param(lq.as_deref()));
    serde_json::json!({
        "open": findings.open_by_key.get(&node.key).copied().unwrap_or(0),
        "pending": own.iter().filter(|f| f.pending.is_some()).count(),
        "total": own.iter().filter(|f| f.similar_to.is_none()).count(),
        "cards": open,
        "decided": decided,
        "praise": praise,
        "targets": crate::findings::page::defer_targets(work),
        "back": back,
        "page_url": format!("/werke/{work_id}/befunde?key={}#zelle", node.key),
    })
}

/// Patterns for selection in the header of the outline (plus „aus der Vorlage“, „keine Nummern“ and an
/// own pattern in the pattern language, Round 6: "I|1" only chapters, "…;…" per level).
const NUMBERING_PRESETS: &[&str] = &["1.1", "I.1", "1.a)", "A.1", "I|1", "1|1", "I.;A.;1.;a);aa)"];

pub struct Panes {
    pub tree: String,
    pub editor: String,
    pub assistants: String,
}

fn values(work: &Work, shelf: &Shelf, at: &Resolved, settings: &Settings, library: &LibraryView) -> Value {
    // Round 35: who asks? A Lektor gets templates of his own (see the module header).
    let who = crate::auth::current();
    let lektor = who.lektor();
    let node = &work.nodes[at.node];
    let revision = &node.revisions[at.index];
    let total = node.revisions.len();
    let proposal = revision.status == RevisionStatus::Proposed;
    let infos = infos_of(&library.revisions, node.id);
    let info = infos.iter().find(|r| r.number == revision.number);
    let current = node.current_number();
    let colors = library.persona_colors.as_slice();
    // Open proposals at their place in the reading flow: in the editor those of the section with typeset view and
    // line diff, in the preview per section of the chapter.
    let proposal_ctx = ProposalContext {
        work: shelf.id,
        revisions: &library.revisions,
        colors,
        image_alts: &library.image_alts,
        folded: &settings.folded,
        lang: shelf.works.iter().find(|w| w.id == shelf.id).map_or_else(Default::default, |w| w.language),
    };
    // Round 8: in the editor the pieces at their place in the shown revision, plus their finished
    // typeset view (only if the typeset view is the default, otherwise it would stay unused).
    let proposals = (!at.preview).then(|| {
        let sheet = (settings.editor == "satz").then(|| ready_sheet(&revision.body, &proposal_ctx)).flatten();
        inline_view(node, revision, proposals_of(node, revision, &proposal_ctx, true), false, sheet)
    });
    // Round 11: images of the work and of the template (chapter image in the header, inserting, missing images).
    let images = &library.images;
    let chapter_picture = chapter_image(work, node.id, shelf.id, images);
    let missing = if at.preview {
        let bodies: Vec<&str> = chapter_nodes(work, node.id)
            .into_iter()
            .filter_map(|n| {
                let picked = at.pick.iter().find(|(id, _)| *id == n.id).and_then(|&(_, rev)| n.revision(rev));
                picked.or_else(|| n.current_index().map(|i| &n.revisions[i])).map(|r| r.body.as_str())
            })
            .collect();
        missing_images(&bodies, images)
    } else {
        missing_images(&[revision.body.as_str()], images)
    };
    let image_choices: Vec<Value> = images
        .assets
        .iter()
        .map(|a| context! { name => a.name.as_str(), caption => a.caption.as_str(), alt => a.alt.as_str() })
        .collect();
    // Round 18c: places for „Bild einfügen“ without JavaScript (end, beginning, before every top-level block of the
    // shown revision); with JavaScript "at the cursor position" is added (web/src/sheet.ts).
    let image_places: Vec<Value> = if at.preview || image_choices.is_empty() {
        Vec::new()
    } else {
        crate::assets::image_places(&revision.body)
            .into_iter()
            .map(|(value, label)| context! { value => value, label => label })
            .collect()
    };
    // Integrator Round 17 (L4): lector and context as in Einstellungen → Assistenten (effective, without a network
    // query); without a loaded list as before according to `Work::lector` and the setting.
    let lector = library.extras.lector.clone().or_else(|| work.lector().map(|p| p.handle.clone()));
    let personas: Vec<_> = work
        .personas
        .iter()
        .map(|p| {
            // Round 16b: local/external from persona.list; external without a release is locked (not selectable).
            let access = library.extras.personas.iter().find(|a| a.handle == p.handle);
            context! {
                handle => p.handle.as_str(), name => p.name.as_str(), role => p.role.as_str(), model => p.model.as_str(),
                context_k => access.and_then(|a| a.context_tokens).unwrap_or(p.context_length) / 1024,
                active => p.active,
                lector => lector.as_deref() == Some(p.handle.as_str()),
                style => persona_style(colors, &p.handle),
                locality => access.map_or("lokal", |a| a.locality.as_str()),
                connection => access.map_or("", |a| a.connection.as_str()),
                blocked => access.is_some_and(PersonaAccess::blocked),
            }
        })
        .collect();
    let mut tree_items = tree(work, None, at.node, &settings.closed);
    // Round 28: badge „k offen“ per item, summed up for chapters and parts (counting rule §1.5).
    let findings_view = &library.extras.findings;
    let open_sums = crate::findings::view::subtree_open(work, &findings_view.open_by_key);
    attach_findings(&mut tree_items, &open_sums, &findings_view.open_by_key);
    let shown_timing = library.extras.timing.as_ref().filter(|t| t.shown());
    if let Some(timing) = shown_timing {
        attach_timing(&mut tree_items, timing);
    }
    let timing = timing_context(shown_timing, node, &revision.body);
    let view = if at.preview { "preview" } else { "edit" };
    let lq_q = lq_param(library.query.as_deref());
    let shown_work = shelf.id;
    let pick_q = pick_param(&at.pick);
    let links = TreeLinks {
        work: shelf.id,
        at,
        lq_q: &lq_q,
        colors,
        findings: findings_view,
        nodes: &work.nodes,
        folds: &library.extras.revision_folds,
    };
    let fold = fold_view(node, revision, at, &library.extras.revision_folds, shelf.id, &lq_q, &pick_q);
    let findings_panel = findings_panel(work, node, shelf.id, at, findings_view, &library.query);
    let section_tree = tree_revisions(node.id, node.key, infos, &links);
    let chapter_tree: Vec<Value> = chapter_nodes(work, node.id)
        .into_iter()
        .map(|n| {
            let revisions = tree_revisions(n.id, n.key, infos_of(&library.revisions, n.id), &links);
            context! {
                node => n.id, key => n.key, number => n.number.as_str(), title => n.title.as_str(),
                current => n.current_number(), selected => n.id == node.id,
                revisions => Value::from(Serde(revisions)),
            }
        })
        .collect();
    let picked: Vec<Value> = at
        .pick
        .iter()
        .filter_map(|&(id, rev)| work.nodes.get(id).map(|n| (n, rev)))
        .map(|(n, rev)| context! { node => n.id, key => n.key, number => n.number.as_str(), title => n.title.as_str(), rev => rev, current => n.current_number() == Some(rev) })
        .collect();
    context! {
        work => context! {
            id => shelf.id, slug => shelf.slug.as_str(), title => work.title.as_str(), author => work.author.as_str(),
        },
        // Header in base.html (Round 9): work choice (`topbar.works`), menus, badges (requests,
        // open proposals).
        topbar => crate::menu::workspace_topbar(work, shelf, at, library),
        lq_q => lq_q.as_str(),
        pick_q => pick_q.as_str(),
        tree => Value::from(Serde(tree_items)),
        // Round 16b: time guard (only for a sermon or a target time, otherwise none).
        timing => timing,
        // `key` (fixed key) for editor, chat and literature forms: the position `id` can
        // become outdated if another tab rearranges the outline.
        node => context! {
            id => node.id, key => node.key, number => node.number.as_str(), title => node.title.as_str(),
            posts => posts(work, node, shelf.id, &lq_q, colors, &library.extras.post_edits, at.rev, &library.extras, lektor),
            // Round 28: position of the first shown post (0 = all shown; older ones through /beitraege).
            post_older => crate::posts::visible_range(node.posts.len()).start,
            post_count => node.posts.len(),
            post_targets => post_targets(work, node.id),
            // Round 28: open findings including sub-items (editor header) and at the section itself.
            open_findings => open_sums.get(node.id).copied().unwrap_or(0),
            open_findings_own => findings_view.open_by_key.get(&node.key).copied().unwrap_or(0),
            findings_url => format!("/werke/{}/befunde?key={}#zelle", shelf.id, node.key),
            // The address named a key that does not exist (any more) (note in the editor header).
            missing_key => at.missing_key,
        },
        numbering => library.numbering.as_ref().map(|n| Value::from(Serde(n))),
        numbering_presets => Value::from(Serde(NUMBERING_PRESETS)),
        outline_requests => library.outline_requests,
        cover => library.cover.as_ref().map(|c| Value::from(Serde(c))),
        rev => context! {
            // 0.9.0: counter and paging over the visible revisions (hidden ones only with the switch open).
            number => revision.number, position => fold.position, total => fold.total, all_total => total,
            status => status_label(revision.status), status_key => status_key(revision.status),
            author => author_label(&revision.author),
            author_style => match &revision.author {
                Author::Assistant { persona, .. } => Some(persona_style(colors, persona)),
                Author::Human { .. } => None,
            },
            body => revision.body.as_str(), note => revision.note.as_str(), proposal => proposal,
            is_current => revision.status == RevisionStatus::Current, current => current,
            based_on => info.and_then(|i| i.based_on), edited => info.and_then(|i| i.edited.as_deref()).map(short_time),
            prev => fold.prev, next => fold.next,
            // Badge in the header of the accordion „Fassungen“: open proposals of the section.
            open_proposals => node.open_proposals(),
            // Bug 0.9.0 (Kritik-Zyklus): open proposals for the check basis („Vorschlag n von @x“).
            open_list => node.revisions.iter().filter(|r| r.status == RevisionStatus::Proposed).map(|r| context! {
                number => r.number,
                label => match &r.author {
                    Author::Assistant { persona, .. } => {
                        tc("view_msg.basis.proposal_by", &[("rev", r.number.into()), ("persona", persona.as_str().into())])
                    }
                    Author::Human { .. } => tc("view_msg.basis.proposal", &[("rev", r.number.into())]),
                },
            }).collect::<Vec<_>>(),
            // Round 22: state of the shown revision, history, state of the base of a proposal.
            amend_seq => info.map_or(0, |i| i.amend_seq),
            history_url => info.filter(|i| i.amend_seq > 0).map(|i| history_url(shown_work, node.key, i.number)),
            based_on_amend => info.and_then(|i| i.based_on_amend).filter(|&k| k > 0),
            own_current => revision.status == RevisionStatus::Current && matches!(revision.author, Author::Human { .. }),
            // Round 22 (R22a): further revisions other than proposals with a link (?rev=) and „aktuell machen“.
            alternatives => alternatives_of(node).iter().map(|r| context! {
                number => r.number, status => status_label(r.status), author => author_label(&r.author),
                shown => r.number == revision.number,
                url => node_url(shown_work, node.key, Some(r.number), at.preview, "", &lq_q),
                hidden => revision_fold::is_hidden(node, fold.mark, r),
                fold => revision_fold::offered(node, fold.mark, r.number),
            }).collect::<Vec<_>>(),
            // 0.9.0: mark „bis hierhin ausblenden“ (fragments/revisions.html, editor header).
            fold => Value::from(Serde(&fold)),
            saved_notice => saved_notice(at.saved.as_deref()),
        },
        proposals => Value::from(Serde(&proposals)),
        preview => Value::from(Serde(if at.preview { preview(work, node, &at.pick, &proposal_ctx) } else { Vec::new() })),
        picked => picked,
        chapter_image => chapter_picture,
        missing_images => missing.iter().map(|p| crate::assets::path_text(p)).collect::<Vec<_>>(),
        image_choices => image_choices,
        image_places => image_places,
        revtree => context! { section => Value::from(Serde(section_tree)), chapter => chapter_tree },
        // Round 28: accordion „Befunde“ (fragments/findings_panel.html).
        findings_panel => Value::from(Serde(findings_panel)),
        personas => personas,
        // Round 10: title of the lector badge („fest gewählt“ or „größter Kontext“).
        lector_fixed => work.lector_choice.handle().is_some_and(|h| {
            lector.as_deref().is_some_and(|l| l.trim_start_matches('@') == h.trim().trim_start_matches('@'))
        }),
        // Round 39b (contract §2.4): work language of the Typst snippets the editor widgets insert.
        work_lang => shelf.works.iter().find(|w| w.id == shelf.id).map_or("de", |w| w.language.as_str()),
        view => view,
        other_view => if at.preview { "edit" } else { "preview" },
        url => at.url(shelf.id, library.query.as_deref()),
        settings => Value::from(Serde(settings)),
        html_style => settings.style(),
        library => Value::from(Serde(&library.titles)),
        library_query => library.query.as_deref(),
        library_hits => Value::from(Serde(&library.hits)),
        // Round 23a: box „Bilder des Werks“ for the search, also without JavaScript in the delivered HTML.
        library_images_html => Value::from_safe_string(library.images_html.clone()),
        // Round 19c: Bible context of the literature on the workspace page (without JavaScript; with JavaScript the
        // popup loads the same box through `literature.render`).
        bibles => library.extras.bibles.as_ref().map(|b| b.context_at(Some(at.key), Some(at.rev))),
        // Round 18a/18c (interface R18-S1): buttons of the ribbon and CSS of the typeset view of the work.
        ribbon => Value::from(Serde(&library.extras.ribbon)),
        sheet_css => library.extras.sheet_css.as_deref(),
        // Round 35: none except for a logged-in Lektor (then origin and name for the Lektor templates).
        lektor => lektor.map(|l| context! {
            handle => l.handle.as_str(),
            name => if l.display.trim().is_empty() { l.name.as_str() } else { l.display.as_str() },
        }),
    }
}

/// Templates of the three areas: for Lektors their own (`fragments/lektor_*.html`), otherwise those of the author.
fn pane_templates() -> [&'static str; 3] {
    if crate::auth::current().lektor().is_some() {
        ["fragments/lektor_tree.html", "fragments/lektor_editor.html", "fragments/lektor_assistants.html"]
    } else {
        ["fragments/tree.html", "fragments/editor.html", "fragments/assistants.html"]
    }
}

pub fn panes(
    env: &Environment<'_>,
    work: &Work,
    shelf: &Shelf,
    at: Resolved,
    settings: &Settings,
    library: &LibraryView,
) -> Result<Panes, minijinja::Error> {
    let ctx = values(work, shelf, &at, settings, library);
    let [tree, editor, assistants] = pane_templates();
    Ok(Panes {
        tree: env.get_template(tree)?.render(&ctx)?,
        editor: env.get_template(editor)?.render(&ctx)?,
        assistants: env.get_template(assistants)?.render(&ctx)?,
    })
}

pub fn page(
    env: &Environment<'_>,
    work: &Work,
    shelf: &Shelf,
    at: Resolved,
    settings: &Settings,
    library: &LibraryView,
) -> Result<String, minijinja::Error> {
    env.get_template("workspace.html")?.render(values(work, shelf, &at, settings, library))
}

pub fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    crate::i18n::install(&mut env);
    for (name, source) in [
        ("base.html", include_str!("../templates/base.html")),
        ("workspace.html", include_str!("../templates/workspace.html")),
        ("fragments/tree.html", include_str!("../templates/fragments/tree.html")),
        ("fragments/editor.html", include_str!("../templates/fragments/editor.html")),
        ("fragments/edit_toolbar.html", include_str!("../templates/fragments/edit_toolbar.html")),
        ("fragments/revtree.html", include_str!("../templates/fragments/revtree.html")),
        ("fragments/revisions.html", include_str!("../templates/fragments/revisions.html")),
        ("fragments/findings_panel.html", include_str!("../templates/fragments/findings_panel.html")),
        ("fragments/assistants.html", include_str!("../templates/fragments/assistants.html")),
        // Round 35: the three areas for Lektors.
        ("fragments/lektor_tree.html", include_str!("../templates/fragments/lektor_tree.html")),
        ("fragments/lektor_editor.html", include_str!("../templates/fragments/lektor_editor.html")),
        ("fragments/lektor_assistants.html", include_str!("../templates/fragments/lektor_assistants.html")),
        ("fragments/proposals.html", include_str!("../templates/fragments/proposals.html")),
        ("fragments/library.html", include_str!("../templates/fragments/library.html")),
        ("fragments/literature_bibles.html", include_str!("../templates/fragments/literature_bibles.html")),
        ("fragments/work_new.html", include_str!("../templates/fragments/work_new.html")),
        ("fragments/idea_capture.html", include_str!("../templates/fragments/idea_capture.html")),
        ("fragments/toast.html", include_str!("../templates/fragments/toast.html")),
        ("fragments/assist_scope.html", include_str!("../templates/fragments/assist_scope.html")),
        // Round 30 (interface R30-S1): scaffolds, included by no template.
        ("fragments/search_bar.html", include_str!("../templates/fragments/search_bar.html")),
        ("fragments/post_item.html", include_str!("../templates/fragments/post_item.html")),
        ("fragments/finding_block.html", include_str!("../templates/fragments/finding_block.html")),
        ("fragments/post_solutions.html", include_str!("../templates/fragments/post_solutions.html")),
        ("fragments/basis_select.html", include_str!("../templates/fragments/basis_select.html")),
        // Round 24 (interface R24-S1): about Baruch (src/about.rs).
        ("about.html", include_str!("../templates/about.html")),
    ] {
        env.add_template(name, source).expect("Vorlage fehlerhaft");
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_from_cookies() {
        let s = Settings::from_cookie_header(Some(
            "other=1; baruch_theme=dark; baruch_left=22.5%; baruch_right=99%; baruch_accordion=personas; baruch_closed=1.6.x",
        ));
        assert_eq!(s.theme, Some("dark"));
        assert_eq!(s.left, Some(22.5));
        assert_eq!(s.right, None, "außerhalb 12–45 % wird übergangen");
        assert_eq!(s.accordion, "personas");
        let revisions = Settings::from_cookie_header(Some("baruch_accordion=revisions"));
        assert_eq!(revisions.accordion, "revisions", "das Akkordeon „Fassungen“ bleibt nach dem Neuladen offen");
        assert_eq!(s.closed, vec![1, 6]);
        assert_eq!(s.style(), "--left:22.5%;");

        let bad = Settings::from_cookie_header(Some("baruch_theme=\"><script>; baruch_accordion=evil"));
        assert_eq!(bad.theme, None);
        assert_eq!(bad.accordion, "chat");
        assert_eq!(bad.editor, "satz", "ohne Cookie baruch_editor ist das Satzbild die Vorgabe");

        let source = Settings::from_cookie_header(Some("baruch_editor=quelltext"));
        assert_eq!(source.editor, "quelltext", "nur die ausdrückliche Wahl „quelltext“ öffnet den Quelltext");
        for other in ["satz", "x\"><b>", ""] {
            let s = Settings::from_cookie_header(Some(&format!("baruch_editor={other}")));
            assert_eq!(s.editor, "satz", "{other:?} gilt nicht als Quelltext");
        }
        assert_eq!(Settings::default().editor, "satz", "auch die Vorgabe ohne Kopf ist das Satzbild");
    }

    #[test]
    fn proposals_start_open_unless_the_pv_cookie_says_closed() {
        let open = Settings::from_cookie_header(Some("baruch_folded=12-3"));
        assert_eq!(open.pv, "open", "ohne baruch_pv stehen Vorschläge offen (Vorgabe wie bisher)");
        assert_eq!(open.folded, vec![(12, 3)], "ohne baruch_pv kein Merker FOLD_ALL");
        assert_eq!(Settings::default().pv, "open", "Vorgabe ohne Kopf: offen");
        let closed = Settings::from_cookie_header(Some("baruch_pv=closed; baruch_folded=12-3"));
        assert_eq!(closed.pv, "closed", "baruch_pv=closed wird gelesen");
        assert_eq!(closed.folded, vec![(12, 3), FOLD_ALL], "der Merker steht hinter den echten Einträgen");
        for other in ["open", "zu", "\"><b>", ""] {
            let s = Settings::from_cookie_header(Some(&format!("baruch_pv={other}")));
            assert_eq!((s.pv, s.folded.len()), ("open", 0), "{other:?} gilt nicht als eingeklappt");
        }
    }

    #[test]
    fn folded_proposals_come_from_a_clean_cookie_only() {
        let s = Settings::from_cookie_header(Some("baruch_folded=12-3.12-3.7-1.-2.0-4.5-0.9.3--4; baruch_theme=dark"));
        assert_eq!(s.folded, vec![(12, 3), (7, 1)], "doppelte, leere Nummern und kaputte Einträge übergangen");
        let bad = Settings::from_cookie_header(Some("baruch_folded=12-3.\"><b>"));
        assert!(bad.folded.is_empty(), "ein Wert mit fremden Zeichen gilt nicht: {:?}", bad.folded);
        let many: Vec<String> = (1..=250).map(|k| format!("{k}-1")).collect();
        let long = Settings::from_cookie_header(Some(&format!("baruch_folded={}", many.join("."))));
        assert_eq!(long.folded.len(), MAX_FOLDED, "höchstens {MAX_FOLDED} Einträge");
        assert_eq!(long.folded[0], (1, 1), "die ersten zählen");
    }

    #[test]
    fn urls_carry_work_key_and_the_encoded_search() {
        let at = Resolved {
            node: 2,
            key: 31,
            index: 0,
            rev: 1,
            preview: true,
            pick: vec![],
            missing_key: None,
            saved: None,
            show_all: false,
        };
        assert_eq!(
            at.url(7, Some(" langmütig & mehr ")),
            "/?work=7&key=31&rev=1&view=preview&lq=langm%C3%BCtig%20%26%20mehr",
            "Reihenfolge work, key, rev, view, lq; kein node= mehr (Runde 8)"
        );
        assert_eq!(at.url(1, Some("  ")), "/?work=1&key=31&rev=1&view=preview", "leere Suche fällt weg");
        let picked = Resolved { pick: vec![(2, 3), (4, 1)], ..at };
        assert_eq!(picked.url(1, None), "/?work=1&key=31&rev=1&view=preview&pick=2:3,4:1", "pick nach view");
        assert_eq!(node_url(3, 9, None, false, "", ""), "/?work=3&key=9", "ohne rev nur Werk und Schlüssel");
    }

    /// Sample work with fixed keys (in memory they are 0): key = 100 + position.
    fn keyed_sample() -> Work {
        let mut work = baruch_core::sample::work();
        for node in &mut work.nodes {
            node.key = 100 + node.id as i64;
        }
        work
    }

    #[test]
    fn the_address_names_the_revision_number_not_its_position() {
        let mut work = keyed_sample();
        work.nodes[2].revisions.remove(0); // gap: revisions 2 and 3
        let at = Location { node: Some(2), rev: Some(3), ..Location::default() }.resolve(&work);
        assert_eq!((at.index, at.rev), (1, 3), "Nummer 3 steht an Stelle 1");
        assert_eq!(at.url(1, None), "/?work=1&key=102&rev=3", "Adresse trägt Schlüssel und Nummer");
        let gone = Location { node: Some(2), rev: Some(1), ..Location::default() }.resolve(&work);
        assert_eq!(gone.rev, 2, "eine gelöschte Nummer führt auf die aktuelle Fassung");
    }

    #[test]
    fn the_key_goes_before_the_position_and_an_unknown_key_falls_back() {
        let work = keyed_sample();
        let at = Location { key: Some(104), node: Some(1), ..Location::default() }.resolve(&work);
        assert_eq!((at.node, at.key, at.missing_key), (4, 104, None), "der Schlüssel gewinnt gegen node=1");
        let gone = Location { key: Some(999), node: Some(5), ..Location::default() }.resolve(&work);
        assert_eq!((gone.node, gone.missing_key), (5, Some(999)), "unbekannter Schlüssel → node, mit Hinweis");
        let fallback = Location { key: Some(999), ..Location::default() }.resolve(&work);
        assert_eq!(fallback.node, default_node(&work), "ohne node der Vorgabe-Abschnitt");
        assert_eq!(fallback.missing_key, Some(999), "auch dann mit Hinweis");
        let old = Location { node: Some(3), ..Location::default() }.resolve(&work);
        assert_eq!((old.node, old.key, old.missing_key), (3, 103, None), "alte Adressen mit node= gehen weiter");
    }

    #[test]
    fn move_targets_list_every_other_section_with_number_and_title() {
        let work = keyed_sample();
        let targets = post_targets(&work, 2);
        assert_eq!(targets.len(), work.nodes.len() - 1, "alle außer dem gezeigten");
        let keys: Vec<i64> = targets.iter().map(|t| t.get_attr("key").unwrap().as_i64().unwrap()).collect();
        assert!(!keys.contains(&102), "der gezeigte Abschnitt fehlt: {keys:?}");
        let label = targets[3].get_attr("label").unwrap().to_string();
        assert!(label.contains(&work.nodes[4].title), "Ziel nennt den Titel: {label}");
    }

    #[test]
    fn pick_keeps_only_valid_entries_one_per_section() {
        let work = baruch_core::sample::work();
        let pick = parse_pick(&work, Some("2:1, x:1,4:9,99:1,4:1,2:3,;:"));
        assert_eq!(pick, vec![(2, 3), (4, 1)], "ungültige Einträge übergangen, je Abschnitt der letzte");
        assert_eq!(pick_param(&pick), "&pick=2:3,4:1");
        assert_eq!(pick_param(&[]), "", "ohne Auswahl kein Parameter");
    }

    #[test]
    fn the_tree_indents_children_under_their_parent() {
        let info = |number, based_on| RevisionInfo {
            number,
            based_on,
            status: RevisionStatus::Superseded,
            author: Author::Human { name: "Autor".into() },
            note: String::new(),
            created: "2026-11-05T10:11:12.123Z".into(),
            edited: None,
            variant: None,
            amend_seq: 0,
            based_on_amend: None,
        };
        let infos = [info(1, None), info(2, Some(1)), info(4, Some(1)), info(5, Some(2)), info(6, None)];
        let order: Vec<(usize, u32)> = ordered_tree(&infos).into_iter().map(|(d, r)| (d, r.number)).collect();
        assert_eq!(order, vec![(0, 1), (1, 2), (2, 5), (1, 4), (0, 6)], "Tiefensuche, Kinder nach Nummer");
        assert_eq!(
            short_time("2026-11-05T10:11:12.123Z"),
            crate::localtime::minutes("2026-11-05T10:11:12.123Z"),
            "Fassungen zeigen ihre Zeit in Ortszeit (Umrechnung selbst: localtime::tests)"
        );
    }

    #[test]
    fn closed_chapter_stays_open_when_it_holds_the_selection() {
        let work = baruch_core::sample::work();
        // Part I (1) with chapter 2 (3) and section 2.1 (4); chosen is 2.1.
        let items = tree(&work, None, 4, &[1, 3, 6]);
        let part1 = items.iter().find(|i| i.id == 1).unwrap();
        assert!(part1.open);
        assert!(part1.children.iter().find(|i| i.id == 3).unwrap().open);
        assert!(!items.iter().find(|i| i.id == 6).unwrap().open);
    }
}
