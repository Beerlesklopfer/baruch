// SPDX-License-Identifier: AGPL-3.0-or-later
//! Erstbefüllung: Eine wirklich neue Datenbank bekommt `baruch_core::sample::work()`, mit
//! Titelschlüssel (`title_key`) und der leeren Werkvorlage als Fassung 1 (Vertrag Runde 4).
//!
//! Ob schon gesät wurde, steht in `store_meta` (Schlüssel `seeded`, Migration 0007), nicht in der
//! Zahl der Werke: Sind alle Werke gelöscht, sät auch ein Neustart nicht neu (Vertrag Runde 5). Eine
//! Datenbank, die beim Lauf von 0007 schon Werke oder Personas hatte, gilt als gesät.

use std::collections::HashMap;

use baruch_core::RevisionStatus;
use sqlx::{SqliteConnection, SqlitePool};

use crate::works::{insert_template_revision, title_key};
use crate::{StoreError, WorkId, author_columns, import, slug, words};

/// Schlüssel in `store_meta`: Beispielwerk gesät.
pub(crate) const SEEDED: &str = "seeded";

/// Befüllt in einer Transaktion (`BEGIN IMMEDIATE`, damit zwei gleichzeitige Starts nicht
/// doppelt befüllen), sofern `store_meta.seeded` fehlt, und setzt die Markierung in derselben
/// Transaktion. Gibt `true` zurück, wenn befüllt wurde.
pub(crate) async fn seed_if_empty(pool: &SqlitePool) -> Result<bool, StoreError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let seeded: Option<String> =
        sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?").bind(SEEDED).fetch_optional(&mut *tx).await?;
    if seeded.is_some() {
        return Ok(false);
    }
    insert_work(&mut tx, &baruch_core::sample::work()).await?;
    sqlx::query("INSERT INTO store_meta (key, value) VALUES (?, '1')").bind(SEEDED).execute(&mut *tx).await?;
    // Eine frisch gesäte Datenbank stammt nicht von vor Runde 14: Die Umwandlung alter Teiltexte
    // (`legacy_parts`, Nutzerfrage 8) gilt als erledigt; das Beispielwerk behält seine leeren Teile wie
    // bisher. Nur ältere Datenbanken (ohne diesen Merker) wandeln beim ersten Start um.
    sqlx::query("INSERT OR IGNORE INTO store_meta (key, value) VALUES (?, '1')")
        .bind(crate::legacy_parts::CONVERTED_KEY)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// Schreibt ein vollständiges Werk. Personas sind global und werden nur angelegt, wenn es noch
/// keine gibt.
pub(crate) async fn insert_work(conn: &mut SqliteConnection, work: &baruch_core::Work) -> Result<WorkId, StoreError> {
    // Freier Ordnername wie beim Import; slugs gelöschter Werke (`work_retired`) bleiben gemieden.
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
    // Wie Migration 0005 für vorhandene Werke: die (leere) Werkvorlage als Fassung 1.
    insert_template_revision(conn, work_id, 1, "", "Autor", "Beispielwerk gesät").await?;

    // Reihenfolge unter demselben Elternteil = Reihenfolge in `work.nodes`.
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

        // Elternteil wie Regel R aus Migration 0004: die höchste frühere Fassung, die aktuell war
        // oder ist (current, superseded); ohne eine solche eine Wurzel.
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

/// Ein Beitrag samt Quellen (JSON) und Verweisen; gibt die neue id zurück (`post.id`; `Post::id` und
/// `Post::created` des Arguments werden nicht gelesen, die vergibt die Datenbank).
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
