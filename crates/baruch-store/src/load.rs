// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read a work from the database and check the rules while doing so (seq without gaps, revision numbers from 1 unique
//! and ascending – with gaps after a deletion –, exactly one `current` per section with revisions).
//!
//! The numbers (`Node.number`) are computed by the loading from the outline (contract Round 4, section 2, and Round 6,
//! section 2, [`numbers`]): scheme `work.numbering_scheme` (migration 0008) with the custom pattern `work.numbering` or
//! `#set heading(numbering: …)` of the work template, otherwise "1.1" ([`work_numbering`]; since Round 14
//! `Numbering::from_template` reads the template the way Typst sets it – the same source as export and typesetting).
//! Parts count on their own (Roman, „Teil I“, „Erster Teil“ or not at all), chapters continuously across parts,
//! sub-counters per parent; nodes with `numbered = 0` do not count and have no number.

use std::collections::HashMap;

use baruch_core::{Node, NodeKind, Persona, Post, Revision, RevisionStatus, TextRef, Work};
use sqlx::SqliteConnection;

use baruch_typst::scheme::{Numbering, Scheme};

use crate::{StoreError, WorkId, broken, parse_author, to_u32, to_usize, words};

type NodeRow = (i64, i64, Option<i64>, String, String, String, i64, i64);
type RevisionRow = (i64, i64, String, String, String, Option<String>, String, String);
type PostRow = (i64, i64, String, String, String, String);
type RefRow = (i64, i64, i64, String);
type PersonaRow = (String, String, String, String, i64, i64);
type WorkRow = (String, String, String, String, Option<String>, String);

/// Numbering of a work from the scheme (`work.numbering_scheme`), the custom pattern (`work.numbering`) and the work
/// template. Never fails: an unknown scheme counts as "from the work template", an unreadable custom pattern as before
/// as a Typst pattern for all levels.
pub(crate) fn work_numbering(scheme: &str, numbering: Option<&str>, template: &str) -> Numbering {
    let scheme = Scheme::from_id(scheme).unwrap_or(Scheme::Template);
    match scheme {
        Scheme::Custom => Numbering::lenient(numbering.unwrap_or(baruch_typst::scheme::DEFAULT_PATTERN)),
        _ => Numbering::of_scheme(scheme, None, template).unwrap_or_else(|_| Numbering::off()),
    }
}

/// Level of a node: part 0, chapter 1, section 2, section below a section 3 (at most).
pub(crate) fn level_of(kind: NodeKind, parent_level: Option<u8>) -> u8 {
    match kind {
        NodeKind::Part => 0,
        NodeKind::Chapter => 1,
        NodeKind::Section => parent_level.map_or(2, |p| (p + 1).clamp(2, 3)),
    }
}

/// A node for the computation of the numbers, in tree order: kind, parent (position in the same list) and whether it
/// counts.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Numbered {
    pub kind: NodeKind,
    pub parent: Option<usize>,
    pub numbered: bool,
}

/// Numbers of all nodes (same order as `nodes`). Parts: their own counter ([`Numbering::part`]), chapters continuously
/// across parts, sections per parent; the numbering forms chapters and sections from the counter values `[chapter,
/// section, …]` ([`Numbering::heading`], the number of all numbered chapters for „Schluss“ of the sermon).
pub(crate) fn numbers(numbering: &Numbering, nodes: &[Numbered]) -> Vec<String> {
    let mut parts = 0usize;
    let mut chapters = 0usize;
    let mut children: HashMap<usize, usize> = HashMap::new();
    let mut paths: Vec<Vec<usize>> = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        // Parent only if it comes earlier (tree order); otherwise as without a parent.
        let parent = node.parent.filter(|&p| p < index);
        let path = match node.kind {
            NodeKind::Part => {
                if node.numbered {
                    parts += 1;
                }
                vec![parts]
            }
            NodeKind::Chapter | NodeKind::Section => {
                let under_heading =
                    node.kind == NodeKind::Section && parent.is_some_and(|p| nodes[p].kind != NodeKind::Part);
                let (mut path, counter) = match parent {
                    Some(p) if under_heading => (paths[p].clone(), children.entry(p).or_insert(0)),
                    _ => (Vec::new(), &mut chapters),
                };
                if node.numbered {
                    *counter += 1;
                }
                path.push(*counter);
                path
            }
        };
        paths.push(path);
    }
    let total =
        nodes.iter().zip(&paths).filter(|(n, path)| n.numbered && n.kind != NodeKind::Part && path.len() == 1).count();
    nodes
        .iter()
        .zip(&paths)
        .map(|(node, path)| match (node.numbered, node.kind) {
            (false, _) => String::new(),
            (true, NodeKind::Part) => numbering.part(path[0]),
            (true, _) => numbering.heading(path, total),
        })
        .collect()
}

pub(crate) async fn work(conn: &mut SqliteConnection, work: WorkId) -> Result<Work, StoreError> {
    let (kind, title, author, template, numbering, scheme): WorkRow =
        sqlx::query_as("SELECT kind, title, author, template, numbering, numbering_scheme FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(StoreError::UnknownWork(work))?;

    let rows: Vec<NodeRow> = sqlx::query_as(
        "SELECT id, seq, parent_seq, number, title, kind, findings, numbered FROM node WHERE work_id = ? ORDER BY seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut nodes = Vec::with_capacity(rows.len());
    let mut counted = Vec::with_capacity(rows.len());
    for (index, (key, seq, parent, number, title, kind, findings, numbered)) in rows.into_iter().enumerate() {
        let id = to_usize(seq, "seq")?;
        if id != index {
            return Err(broken(format!("Werk {work}: seq {id} statt {index} (Lücke in der Gliederung)")));
        }
        let parent = parent.map(|p| to_usize(p, "parent_seq")).transpose()?;
        let kind = words::parse_node_kind(&kind)?;
        counted.push(Numbered { kind, parent, numbered: numbered != 0 });
        nodes.push(Node {
            id,
            key,
            parent,
            number,
            title,
            kind,
            revisions: Vec::new(),
            posts: Vec::new(),
            findings: to_u32(findings, "findings")?,
        });
    }
    let numbering = work_numbering(&scheme, numbering.as_deref(), &template);
    for (node, number) in nodes.iter_mut().zip(numbers(&numbering, &counted)) {
        node.number = number;
    }

    let revisions: Vec<RevisionRow> = sqlx::query_as(
        "SELECT n.seq, r.number, r.body, r.author_kind, r.author_name, r.model, r.status, r.note \
         FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? ORDER BY n.seq, r.number",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    for (seq, number, body, author_kind, author_name, model, status, note) in revisions {
        let node = &mut nodes[to_usize(seq, "seq")?];
        let number = to_u32(number, "Revisionsnummer")?;
        // Unique and ascending (gaps after a deletion are allowed).
        if number == 0 || node.revisions.last().is_some_and(|last| last.number >= number) {
            return Err(broken(format!("Abschnitt {}: Revision {number} außer der Reihe", node.id)));
        }
        node.revisions.push(Revision {
            number,
            body,
            author: parse_author(&author_kind, author_name, model)?,
            status: words::parse_status(&status)?,
            note,
        });
    }
    for node in &nodes {
        check_one_current(node)?;
    }

    let refs: Vec<RefRow> = sqlx::query_as(
        "SELECT pr.post_id, pr.node_seq, pr.rev, pr.quote \
         FROM post_ref pr JOIN post p ON p.id = pr.post_id JOIN node n ON n.id = p.node_id \
         WHERE n.work_id = ? ORDER BY pr.post_id, pr.position",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut refs_by_post: HashMap<i64, Vec<TextRef>> = HashMap::new();
    for (post_id, node_seq, rev, quote) in refs {
        refs_by_post.entry(post_id).or_default().push(TextRef {
            node: to_usize(node_seq, "node_seq")?,
            rev: to_u32(rev, "rev")?,
            quote,
        });
    }

    let posts: Vec<PostRow> = sqlx::query_as(
        "SELECT p.id, n.seq, p.author, p.body, p.sources, p.created \
         FROM post p JOIN node n ON n.id = p.node_id \
         WHERE n.work_id = ? ORDER BY n.seq, p.position",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    for (post_id, seq, author, body, sources, created) in posts {
        let sources: Vec<String> = serde_json::from_str(&sources)
            .map_err(|e| broken(format!("Beitrag {post_id}: Quellen sind kein JSON ({e})")))?;
        nodes[to_usize(seq, "seq")?].posts.push(Post {
            id: post_id,
            created,
            author,
            body,
            sources,
            refs: refs_by_post.remove(&post_id).unwrap_or_default(),
        });
    }

    let personas: Vec<PersonaRow> =
        sqlx::query_as("SELECT handle, name, role, model, context_length, active FROM persona ORDER BY position, id")
            .fetch_all(&mut *conn)
            .await?;
    let personas = personas
        .into_iter()
        .map(|(handle, name, role, model, context_length, active)| {
            Ok(Persona {
                handle,
                name,
                role,
                model,
                context_length: to_u32(context_length, "context_length")?,
                active: active != 0,
            })
        })
        .collect::<Result<Vec<_>, StoreError>>()?;

    let lector_choice = crate::personas::read_lector_choice(&mut *conn).await?;
    Ok(Work { kind: words::parse_work_kind(&kind)?, title, author, nodes, personas, lector_choice })
}

/// A section with revisions has exactly one `current`.
fn check_one_current(node: &Node) -> Result<(), StoreError> {
    let currents = node.revisions.iter().filter(|r| r.status == RevisionStatus::Current).count();
    if node.revisions.is_empty() || currents == 1 {
        Ok(())
    } else {
        Err(broken(format!("Abschnitt {}: {currents} aktuelle Revisionen statt genau einer", node.id)))
    }
}
