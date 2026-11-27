// SPDX-License-Identifier: AGPL-3.0-or-later
//! Initial filling: a really new database gets `baruch_core::sample::work()`, with the title key (`title_key`) and the
//! empty work template as revision 1 (contract Round 4).
//!
//! Whether seeding has already happened is recorded in `store_meta` (key `seeded`, migration 0007), not in the number
//! of works: if all works are deleted, a restart does not seed again either (contract Round 5). A database that already
//! had works or personas when 0007 ran counts as seeded.

use std::collections::HashMap;

use baruch_core::RevisionStatus;
use sqlx::{SqliteConnection, SqlitePool};

use crate::works::{insert_template_revision, title_key};
use crate::{StoreError, WorkId, author_columns, import, slug, words};

/// Key in `store_meta`: sample work seeded.
pub(crate) const SEEDED: &str = "seeded";

/// Fills in one transaction (`BEGIN IMMEDIATE`, so that two simultaneous starts do not fill twice), provided
/// `store_meta.seeded` is missing, and sets the marker in the same transaction. Returns `true` if it filled.
pub(crate) async fn seed_if_empty(pool: &SqlitePool) -> Result<bool, StoreError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let seeded: Option<String> =
        sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?").bind(SEEDED).fetch_optional(&mut *tx).await?;
    if seeded.is_some() {
        return Ok(false);
    }
    insert_work(&mut tx, &baruch_core::sample::work()).await?;
    sqlx::query("INSERT INTO store_meta (key, value) VALUES (?, '1')").bind(SEEDED).execute(&mut *tx).await?;
    // A freshly seeded database does not date from before Round 14: the conversion of old part texts (`legacy_parts`,
    // user question 8) counts as done; the sample work keeps its empty parts as before. Only older databases (without
    // this marker) convert at the first start.
    sqlx::query("INSERT OR IGNORE INTO store_meta (key, value) VALUES (?, '1')")
        .bind(crate::legacy_parts::CONVERTED_KEY)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// Writes a complete work. Personas are global and are only created if there are none yet.
pub(crate) async fn insert_work(conn: &mut SqliteConnection, work: &baruch_core::Work) -> Result<WorkId, StoreError> {
    // Free folder name as in the import; slugs of deleted works (`work_retired`) stay avoided.
    let slug = import::free_slug(conn, &slug(&work.title)).await?;
    let work_id: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, title_key) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&slug)
    .bind(words::work_kind(work.kind))
    .bind(&work.title)
    .bind(&work.author)
    .bind(title_key(&work.title))
    .fetch_one(&mut *conn)
    .await?;
    // As migration 0005 does for existing works: the (empty) work template as revision 1.
    insert_template_revision(conn, work_id, 1, "", "Autor", "Beispielwerk gesät").await?;

    // Order below the same parent = order in `work.nodes`.
    let mut next_position: HashMap<Option<usize>, i64> = HashMap::new();
    for (index, node) in work.nodes.iter().enumerate() {
        if node.id != index {
            return Err(crate::broken(format!("Knoten {index} trägt die id {}", node.id)));
        }
        let position = next_position.entry(node.parent).or_insert(0);
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind, findings) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work_id)
        .bind(node.id as i64)
        .bind(node.parent.map(|p| p as i64))
        .bind(*position)
        .bind(&node.number)
        .bind(&node.title)
        .bind(words::node_kind(node.kind))
        .bind(node.findings as i64)
        .fetch_one(&mut *conn)
        .await?;
        *position += 1;

        // Parent as in rule R from migration 0004: the highest earlier revision that was or is current (current,
        // superseded); without one, a root.
        let mut last_shown: Option<u32> = None;
        for revision in &node.revisions {
            let (author_kind, author_name, model) = author_columns(&revision.author);
            let based_on = last_shown.filter(|&b| b < revision.number);
            if matches!(revision.status, RevisionStatus::Current | RevisionStatus::Superseded) {
                last_shown = Some(revision.number);
            }
            sqlx::query(
                "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(node_id)
            .bind(revision.number as i64)
            .bind(&revision.body)
            .bind(author_kind)
            .bind(author_name)
            .bind(model)
            .bind(words::status(revision.status))
            .bind(&revision.note)
            .bind(based_on.map(i64::from))
            .execute(&mut *conn)
            .await?;
        }

        for (position, post) in node.posts.iter().enumerate() {
            insert_post(conn, node_id, position as i64, post, &post.body).await?;
        }
    }

    let personas: i64 = sqlx::query_scalar("SELECT count(*) FROM persona").fetch_one(&mut *conn).await?;
    if personas == 0 {
        for (position, persona) in work.personas.iter().enumerate() {
            sqlx::query(
                "INSERT INTO persona (handle, name, role, model, context_length, active, position) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&persona.handle)
            .bind(&persona.name)
            .bind(&persona.role)
            .bind(&persona.model)
            .bind(persona.context_length as i64)
            .bind(persona.active as i64)
            .bind(position as i64)
            .execute(&mut *conn)
            .await?;
        }
    }
    Ok(work_id)
}

/// A post together with its sources (JSON) and references; returns the new id (`post.id`; `Post::id` and
/// `Post::created` of the argument are not read, the database assigns them).
pub(crate) async fn insert_post(
    conn: &mut SqliteConnection,
    node_id: i64,
    position: i64,
    post: &baruch_core::Post,
    body: &str,
) -> Result<i64, StoreError> {
    let sources = serde_json::to_string(&post.sources).expect("Liste von Strings ist immer JSON");
    let post_id: i64 = sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(node_id)
    .bind(position)
    .bind(&post.author)
    .bind(body)
    .bind(sources)
    .fetch_one(&mut *conn)
    .await?;
    for (position, r) in post.refs.iter().enumerate() {
        sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, ?, ?, ?, ?)")
            .bind(post_id)
            .bind(position as i64)
            .bind(r.node as i64)
            .bind(r.rev as i64)
            .bind(&r.quote)
            .execute(&mut *conn)
            .await?;
    }
    Ok(post_id)
}
