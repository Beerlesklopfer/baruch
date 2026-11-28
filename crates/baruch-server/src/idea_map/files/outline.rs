// SPDX-License-Identifier: AGPL-3.0-or-later
//! Outline view of the mind map (contract Round 34 §5, owner mindmap_austausch): `/werke/{id}/karte/gliederung`
//! operates the mind map entirely without JavaScript.
//!
//! The page shows the tree as a nested list (`<details>` per node with sub-nodes; `zu=` collapses in advance),
//! next to it the **cloud** – ideas without a place ([`super::export::cloud`]) – with counter „n in der Wolke“ and,
//! where the nearness knows a partner in the tree, the suggestion „passt zu Ast …“. `wahl=<id>` chooses a node; for
//! it the forms stand at the top: rename, sub-node, sibling, „In die Mindmap unter …“/re-hang, „In die
//! Wolke“, order, relation, link/unlink, delete – all to `POST /werke/{id}/karte/act` with
//! `seite=gliederung`. Parts and chapters belong to the outline of the work: for them there is only “sub-node”.
//! Below the tree: relations, contradictions and deleted ideas (restore).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use baruch_store::{Idea, IdeaFilter, IdeaId, IdeaScope, IdeaStatus, LinkKind, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};

use super::export::{self, cloud, placed};
use super::{closed_of, is_node_id, load_tree};
use crate::AppState;
use crate::i18n::tc;
use crate::idea_map::MapFilter;
use crate::idea_map::bild::{filter_query, parse_filter, plain, with_query};
use crate::idea_map::tree::{Scene, TreeKind, TreeNode};
use crate::view::Settings;

/// What the address of the outline view carries besides the filter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    /// `wahl=<id>`: chosen node (its forms stand at the top).
    pub chosen: Option<String>,
    /// `ok=<wort>` after a successful action.
    pub ok: Option<String>,
    /// `undo=<id>`: link just removed (form „Wiederherstellen“).
    pub undo: Option<i64>,
    /// `wieder=<id>`: idea just deleted (form „Wiederherstellen“).
    pub restore: Option<IdeaId>,
    /// `zu=<ids>`: nodes collapsed in advance.
    pub closed: Vec<String>,
}

/// Templates of the own pages (base.html, outline view, import).
pub(crate) fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../../templates/base.html")),
            ("karte_gliederung.html", include_str!("../../../templates/karte_gliederung.html")),
            ("karte_einlesen.html", include_str!("../../../templates/karte_einlesen.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Mindmap (Gliederungsansicht/Einlesen) fehlerhaft");
        }
        env
    })
}

/// Self-built address as a safe value for `href`: `&` escaped; should it contain an unsafe character, `#`
/// stands there.
pub(crate) fn href(url: String) -> Value {
    if url.chars().any(|c| matches!(c, '"' | '<' | '>' | '\'' | ' ') || c.is_control()) {
        return Value::from_safe_string("#".to_owned());
    }
    Value::from_safe_string(url.replace('&', "&amp;"))
}

/// Feedback for `ok=<wort>`.
fn notice_of(ok: &str) -> String {
    match ok {
        "angelegt" => tc("ideas_msg.map_notice.created", &[]),
        "umbenannt" => tc("ideas_msg.map_notice.renamed", &[]),
        "bezug" => tc("ideas_msg.map_notice.reference", &[]),
        "umgehaengt" => tc("ideas_msg.outline_notice.moved", &[]),
        "in-wolke" => tc("ideas_msg.map_notice.to_cloud", &[]),
        "ort-unveraendert" => tc("ideas_msg.map_notice.place_unchanged", &[]),
        "geloescht" => tc("ideas_msg.outline_notice.deleted", &[]),
        "einfall-wieder" => tc("ideas_msg.map_notice.restored", &[]),
        "verbunden" => tc("ideas_msg.map_notice.linked", &[]),
        "unveraendert" => tc("ideas_msg.map_notice.link_exists", &[]),
        "geloest" => tc("ideas_msg.map_notice.unlinked", &[]),
        "wiederhergestellt" => tc("ideas_msg.map_notice.link_restored", &[]),
        "eingelesen" => tc("ideas_msg.outline_notice.imported", &[]),
        _ => tc("ideas_msg.notice.saved", &[]),
    }
}

fn cut(text: &str, max: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        return one;
    }
    let mut short: String = one.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}

fn anchor_of(id: &str) -> String {
    format!("k-{}", id.replace(':', "-"))
}

fn kind_word(kind: TreeKind) -> &'static str {
    match kind {
        TreeKind::Work => "work",
        TreeKind::Part => "part",
        TreeKind::Chapter => "chapter",
        TreeKind::Idea => "idea",
    }
}

struct Page<'a> {
    /// Is a search running (`q`)? Only then are hits highlighted (without a search every idea counts as a hit).
    searching: bool,
    work: WorkId,
    base: String,
    query: String,
    view: &'a View,
}

impl Page<'_> {
    /// Address of the outline view with filter and chosen node.
    fn select(&self, id: &str) -> Value {
        let mut query = self.query.clone();
        if !query.is_empty() {
            query.push('&');
        }
        query.push_str(&format!("wahl={id}"));
        href(format!("{}?{query}#wahl", self.base))
    }

    fn node(&self, node: &TreeNode) -> Value {
        let children: Vec<Value> = node.children.iter().map(|child| self.node(child)).collect();
        context! {
            id => node.id.as_str(),
            anchor => anchor_of(&node.id),
            kind => kind_word(node.kind),
            idea => node.idea,
            title => export::node_title(node),
            note => cut(&node.note, 300),
            has_note => node.has_note,
            // Without a title of their own the tags already are the displayed title.
            words => if node.kind == TreeKind::Idea && node.own_title.is_empty() { String::new() } else { node.words.join(", ") },
            status => node.status.map(export::status_label),
            done => node.status == Some(IdeaStatus::Done),
            origin => node.origin.as_ref().map(|o| o.label()),
            section => node.section_title.as_deref(),
            section_href => node.key.filter(|_| node.kind == TreeKind::Idea).map(|k| href(format!("/?work={}&key={k}", self.work))),
            node_href => node.key.filter(|_| node.kind != TreeKind::Idea).map(|k| href(format!("/?work={}&key={k}", self.work))),
            idea_href => node.idea.map(|id| href(format!("/einfaelle/{id}"))),
            hit => self.searching && node.hit,
            ghost => node.ghost,
            open_findings => node.open_findings,
            descendants => node.descendants,
            select => self.select(&node.id),
            chosen => self.view.chosen.as_deref() == Some(node.id.as_str()),
            closed => self.view.closed.iter().any(|c| c == &node.id),
            children => children,
        }
    }
}

/// Parent node and place among the siblings of a node.
fn locate<'a>(node: &'a TreeNode, id: &str) -> Option<(&'a TreeNode, usize)> {
    for (index, child) in node.children.iter().enumerate() {
        if child.id == id {
            return Some((node, index));
        }
        if let Some(found) = locate(child, id) {
            return Some(found);
        }
    }
    None
}

fn ids_below(node: &TreeNode, out: &mut HashSet<String>) {
    out.insert(node.id.clone());
    for child in &node.children {
        ids_below(child, out);
    }
}

/// Label in a selection list: indented by depth.
fn option_label(node: &TreeNode, depth: usize) -> String {
    format!("{}{}", "– ".repeat(depth.saturating_sub(1).min(12)), cut(&export::node_title(node), 80))
}

/// The forms for the chosen node (`None` if it is not in the tree shown or is the root).
fn panel(scene: &Scene, chosen: &str) -> Option<Value> {
    let map = &scene.map;
    let node = map.find(chosen)?;
    let (parent, _) = locate(&map.root, chosen)?;
    let in_cloud = scene.in_cloud(&node.id);
    // Free main topic: idea directly on the work, own main branch.
    let main_topic = parent.kind == TreeKind::Work && node.kind == TreeKind::Idea && !in_cloud;
    if node.kind != TreeKind::Idea {
        return Some(context! {
            id => node.id.as_str(), kind => kind_word(node.kind), title => export::node_title(node), key => node.key,
            editable => false,
        });
    }
    let mut below = HashSet::new();
    ids_below(node, &mut below);
    let walk = map.walk();
    // Targets for re-hanging: chapters/parts and ideas, not the node itself or its descendants (cycle).
    let targets: Vec<Value> = walk
        .iter()
        .filter(|(n, _)| n.kind != TreeKind::Work && !below.contains(&n.id))
        .filter(|(n, _)| n.kind == TreeKind::Idea || n.key.is_some())
        .map(|(n, depth)| {
            let value = match n.kind {
                TreeKind::Idea => n.id.clone(),
                _ => format!("n:{}", n.key.unwrap_or_default()),
            };
            context! { value => value, label => option_label(n, *depth), here => n.id == parent.id }
        })
        .collect();
    let chapters: Vec<Value> = walk
        .iter()
        .filter(|(n, _)| matches!(n.kind, TreeKind::Part | TreeKind::Chapter))
        .filter_map(|(n, depth)| {
            Some(context! { key => n.key?, label => option_label(n, *depth), here => node.key == n.key })
        })
        .collect();
    let others: Vec<Value> = walk
        .iter()
        .filter(|(n, _)| n.kind == TreeKind::Idea && n.id != node.id)
        .filter_map(|(n, _)| Some(context! { id => n.idea?, label => format!("{} – {}", n.idea?, cut(&n.title, 70)) }))
        .collect();
    // Siblings: ideas in the same place – in the cloud only those of the cloud, on the work only the main topics.
    let siblings: Vec<&TreeNode> =
        parent.children.iter().filter(|c| c.kind == TreeKind::Idea && scene.in_cloud(&c.id) == in_cloud).collect();
    let place = siblings.iter().position(|s| s.id == node.id).unwrap_or(0);
    let id_of = |n: Option<&&TreeNode>| n.and_then(|n| n.idea);
    let up = place.checked_sub(1).and_then(|p| id_of(siblings.get(p)));
    // “Down” = before the sibling after next; if there is none, to the end (`before` empty).
    let down =
        siblings.get(place + 1).map(|_| id_of(siblings.get(place + 2)).map(|id| id.to_string()).unwrap_or_default());
    let titles: HashMap<IdeaId, &str> =
        walk.iter().filter_map(|(n, _)| n.idea.map(|id| (id, n.title.as_str()))).collect();
    let own = node.idea.unwrap_or_default();
    let links: Vec<Value> = export::links_of(own, &map.links)
        .into_iter()
        .map(|(link, arrow, other)| {
            context! {
                id => link.id, arrow => arrow, other => other,
                other_label => titles.get(&other).map(|t| cut(t, 70)).unwrap_or_default(),
                kind_label => link.kind.label(), note => link.note.as_str(),
            }
        })
        .collect();
    Some(context! {
        id => node.id.as_str(), kind => "idea", idea => own, title => node.title.as_str(),
        own_title => node.own_title.as_str(), editable => node.editable, in_cloud => in_cloud,
        main_topic => main_topic,
        children => node.children.len(), descendants => node.descendants,
        // A sibling is created on the same parent node, directly behind this node.
        sibling_parent => parent.idea.filter(|_| parent.kind == TreeKind::Idea),
        sibling_key => parent.key.filter(|_| matches!(parent.kind, TreeKind::Part | TreeKind::Chapter)),
        sibling_before => id_of(siblings.get(place + 1)),
        parent_title => if in_cloud {
            tc("ideas_msg.outline_notice.cloud", &[])
        } else if main_topic {
            tc("ideas_msg.outline_notice.main_topic", &[("title", export::node_title(parent).into())])
        } else {
            export::node_title(parent)
        },
        targets => targets, chapters => chapters, key => node.key, others => others, links => links,
        up => up, down => down,
        kinds => LinkKind::ALL.iter().map(|k| context! { value => k.as_str(), label => k.label() }).collect::<Vec<_>>(),
    })
}

/// Suggestion of the nearness for an idea of the cloud ([`Scene::hints`]): the nearest idea that already hangs in
/// the mind map, and its main branch.
fn hint(scene: &Scene, idea: IdeaId) -> Option<Value> {
    let hint = scene.hints.iter().find(|h| h.idea == idea)?;
    let partner: IdeaId = hint.node.strip_prefix("i:")?.parse().ok()?;
    // Title of the main branch as in the list (chapter with number).
    let branch = scene.map.find(&hint.branch).map(export::node_title).unwrap_or_else(|| hint.branch_title.clone());
    Some(context! { branch => cut(&branch, 80), partner => partner, partner_title => cut(&hint.node_title, 80) })
}

fn deleted_label(idea: &Idea) -> String {
    if !idea.title.trim().is_empty() {
        cut(&idea.title, 80)
    } else if !idea.words.is_empty() {
        cut(&idea.words.join(", "), 80)
    } else if !idea.note.trim().is_empty() {
        cut(&idea.note, 80)
    } else {
        format!("Einfall {}", idea.id)
    }
}

/// The outline view (also as the error page of a form: `status` and `error`).
pub(crate) async fn render(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    filter: &MapFilter,
    view: &View,
    status: StatusCode,
    error: Option<String>,
) -> Response {
    let (_, scene) = match load_tree(state, work, filter).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let map = &scene.map;
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let query = filter_query(&map.filter);
    let karte = format!("/werke/{work}/karte");
    let page = Page {
        searching: map.filter.q.is_some(),
        work,
        base: format!("{karte}/gliederung"),
        query: query.clone(),
        view,
    };
    let branches: Vec<Value> = placed(&scene).into_iter().map(|n| page.node(n)).collect();
    let loose: Vec<Value> = cloud(&scene)
        .into_iter()
        .map(|n| context! { node => page.node(n), hint => n.idea.and_then(|id| hint(&scene, id)) })
        .collect();
    let titles: HashMap<IdeaId, &str> =
        map.walk().into_iter().filter_map(|(n, _)| n.idea.map(|id| (id, n.title.as_str()))).collect();
    let name = |id: IdeaId| titles.get(&id).map(|t| cut(t, 70)).unwrap_or_else(|| format!("Einfall {id}"));
    let links: Vec<Value> = map
        .links
        .iter()
        .filter(|l| l.deleted.is_none() && titles.contains_key(&l.from) && titles.contains_key(&l.to))
        .map(|l| {
            context! {
                id => l.id, from => name(l.from), to => name(l.to),
                from_select => page.select(&format!("i:{}", l.from)), to_select => page.select(&format!("i:{}", l.to)),
                arrow => if l.kind.symmetric() { "↔" } else { "→" },
                kind_label => l.kind.label(), note => l.note.as_str(), contradiction => l.kind == LinkKind::Contradicts,
            }
        })
        .collect();
    let deleted: Vec<Value> = state
        .store
        .idea_list(&IdeaFilter { scope: IdeaScope::Work(work), deleted: true, limit: 100, ..IdeaFilter::default() })
        .await
        .unwrap_or_default()
        .iter()
        .map(|i| context! { id => i.id, label => deleted_label(i), at => i.deleted.as_deref().map(crate::localtime::seconds) })
        .collect();
    let chosen = view.chosen.as_deref().filter(|c| is_node_id(c) && *c != "w");
    let ctx = context! {
        settings => Value::from(Serde(settings)),
        work => context! { id => work, title => map.title.as_str() },
        title => map.title.as_str(),
        back => query.as_str(),
        act => href(format!("{karte}/act")),
        here => href(with_query(&page.base, &query)),
        map_href => href(with_query(&karte, &query)),
        import_href => href(format!("{karte}/einlesen")),
        outputs => context! {
            xmind => href(with_query(&format!("{karte}.xmind"), &query)),
            mm => href(with_query(&format!("{karte}.mm"), &query)),
            svg => href(with_query(&format!("{karte}.svg"), &query)),
            pdf => href(with_query(&format!("{karte}.pdf"), &query)),
            md => href(with_query(&format!("{karte}.md"), &query)),
        },
        ideas_href => href(format!("/einfaelle?work={work}")),
        workspace => href(format!("/?work={work}")),
        filter => context! {
            status => map.filter.status.map_or("all", IdeaStatus::as_str),
            word => map.filter.word.as_deref().unwrap_or(""),
            origin => map.filter.origin.as_deref().unwrap_or(""),
            q => map.filter.q.as_deref().unwrap_or(""),
            words => export::filter_words(&map.filter),
        },
        branches => branches,
        cloud => loose,
        cloud_count => scene.cloud_count,
        total => map.total_ideas,
        chosen => chosen,
        panel => chosen.and_then(|c| panel(&scene, c)),
        links => links,
        deleted => deleted,
        notice => view.ok.as_deref().map(notice_of),
        undo => view.undo.filter(|_| view.ok.as_deref() == Some("geloest")),
        restore => view.restore.filter(|_| view.ok.as_deref() == Some("geloescht")),
        error => error,
    };
    match templates().get_template("karte_gliederung.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            tc(
                "ideas_msg.page.template_error",
                &[("name", "karte_gliederung.html".into()), ("error", e.to_string().into())],
            ),
        ),
    }
}

/// `GET /werke/{id}/karte/gliederung`.
pub async fn page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, extra) = parse_filter(&pairs);
    let last = |name: &str| pairs.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.trim().to_owned());
    let view = View {
        chosen: last("wahl").filter(|w| is_node_id(w)),
        ok: extra.ok,
        undo: extra.undo,
        restore: last("wieder").and_then(|w| w.parse().ok()),
        closed: closed_of(&pairs),
    };
    render(&state, &headers, work, &filter, &view, StatusCode::OK, None).await
}
