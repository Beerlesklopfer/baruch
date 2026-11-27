// SPDX-License-Identifier: AGPL-3.0-or-later
//! A work from the database as [`baruch_exchange::ExchangeWork`] (Round 26).
//!
//! Reads work, details, book details, template, outline with all revisions, posts, ideas (also deleted ones), threads,
//! images, the mind map (Round 25) and tombstones in one reading transaction; the format contains only UUIDs.
//!
//! Mapping (contract F3/F5):
//! - Numeric ids, `slug`, `node.number` (is computed), the source of the Typst import and everything outside the work
//!   (connections to services, personas together with models, requests, evidence, RAG assignments) do not occur.
//! - References run through UUIDs: `revision.based_on` (number) → UUID of the revision with this number in the same
//!   item; `post_ref.node_seq`/`rev` → UUID of the item or of its revision (`rev` without a matching revision → without
//!   `fassung`); `idea.node_id` → UUID of the item; cover and author photo → UUID of the image;
//!   `asset_revision.based_on` (number) → UUID of the image version.
//! - `position` of posts and thread posts is, in the format, the order from 1 (without gaps), not the internal position
//!   (from 0, with gaps after deletions).
//! - `done_ref` of an idea is free text; if it begins with "key N" and the item exists in the work, the UUID of the
//!   item stands there instead of "key N" (the rest stays verbatim).
//! - Images as raw bytes (Base64 from the database decoded, SVG text as UTF-8), `sha256` computed anew over the bytes.
//! - Map (Round 25, `<karte>`): all links (`idea_link`, also soft-deleted ones) whose two ideas belong to the work, by
//!   id; all positions (`idea_map_pos`) of the work whose idea still belongs to the work, by id. References through the
//!   UUIDs of the ideas; „widerspricht“ stands the way it is stored (from < to here).
//! - Tombstones: all entries in `uuid_tombstone` with the UUID of the work (also `idea_link`, `idea_map_pos`,
//!   `cover_block`).
//! - Minor version 3 (Round 27): addresses from `work_book`, the cover layout (`cover_block`, per block with uuid, in
//!   the order of the kinds) in `<buch>` and the typeset view settings (`work_sheet`). If a work has blocks but no row
//!   in `work_book`, the book details stand there with their defaults (otherwise there would be no `<buch>` for
//!   `<umschlag>`).
//! - Minor version 4 (Round 28): findings (`finding`, by id) with status history (`finding_status`, by id) in
//!   `<befunde>`; references through UUIDs (item, post, thread post, `similar_to` → UUID of the finding; revision and
//!   target are already in the database as UUIDs). Posts and thread posts carry `edited` and their old versions
//!   (`post_version`, by id). The tombstone kinds `finding`, `finding_status`, `post_version` are in the list.
//! - Minor version 5 (Round 30): solutions per finding (`finding.solutions`), variants on the revisions
//!   (`revision.variant_*`, findings as UUIDs; deleted ones are dropped) and metadata of the posts (basis as UUID of
//!   the revision, header line, reply to/repetition of as UUID of a post of the same place, relay). The job (`request`)
//!   and the log `replace_batch` stay local.
//! - Minor version 6 (Round 29): the typesetting settings (`work_typeset`, one row per work) with uuid, `updated`,
//!   `updated_by` and the settings; the tombstone kind `work_typeset` is in the list.
//! - Minor version 7 (Round 31): custom indexes (`index_register`, in their order) and all glossary terms
//!   (`glossary_term` with variants from `glossary_variant`, accepted and proposed ones, by `sort_key`); the tombstone
//!   kinds `glossary_term` and `index_register` are in the list. Index proposals (`index_proposal`) stay local
//!   (workflow of the instance, without uuid).
//! - Minor version 8 (Round 22): on every revision `amend_seq` and `based_on_amend` and the history `revision_amend`
//!   (by step, with uuid), on finding and history entry `adopted_amend`, the draft in the typesetting settings
//!   (`TypesetSettings.draft`); the tombstone kind `revision_amend` is in the list. The setting „Fassungen“
//!   (`work_revision_policy`) stays local (working style of the instance).

use std::collections::HashMap;

use baruch_exchange::{
    ExchangeAmend, ExchangeAmendKind, ExchangeGlossaryStatus, ExchangeGlossaryTerm, ExchangeRegister,
};
use baruch_exchange::{
    ExchangeAsset, ExchangeAssetRevision, ExchangeAssetStatus, ExchangeAuthor, ExchangeBackdrop, ExchangeBook,
    ExchangeCoverAlign, ExchangeCoverBlock, ExchangeCoverBlockKind, ExchangeFontStyle, ExchangeIdea,
    ExchangeIdeaStatus, ExchangeIdeaUse, ExchangeLink, ExchangeLinkKind, ExchangeMap, ExchangeNode, ExchangeNodeKind,
    ExchangeOriginKind, ExchangePosition, ExchangePost, ExchangePostRef, ExchangeRevision, ExchangeRevisionStatus,
    ExchangeSheet, ExchangeSourceKind, ExchangeSpineDirection, ExchangeSwordSource, ExchangeTemplate,
    ExchangeTemplateRevision, ExchangeThread, ExchangeThreadPost, ExchangeTiming, ExchangeTitleArea, ExchangeTombstone,
    ExchangeTombstoneKind, ExchangeWork, ExchangeWorkKind, sha256_hex,
};
use baruch_exchange::{
    ExchangeDecision, ExchangeFinding, ExchangeFindingEntry, ExchangeFindingKind, ExchangeFindingStatus,
    ExchangePostVersion,
};
use baruch_exchange::{ExchangePostMeta, ExchangeSolution, ExchangeTypeset, ExchangeVariant};
use sqlx::{Row, SqliteConnection};

use crate::{Store, StoreError, WorkId};

fn broken(what: impl std::fmt::Display) -> StoreError {
    StoreError::Exchange(format!("Datenbank passt nicht zum Austauschformat: {what}"))
}

/// Number from the database as `u32` for the format (numbers, positions, seconds).
fn number(value: i64, what: &str) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| broken(format!("{what} {value}")))
}

/// JSON list of strings (`post.sources`, `chat_thread_post.sources`).
fn sources(json: &str, what: &str) -> Result<Vec<String>, StoreError> {
    serde_json::from_str(json).map_err(|e| broken(format!("Quellen von {what}: {e}")))
}

type WorkRow =
    (String, String, String, String, String, String, String, String, Option<String>, String, Option<String>, String);

type NodeRow = (i64, i64, Option<i64>, i64, String, String, i64, String);

type RevisionRow = (
    i64,
    String,
    i64,
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
);

type IdeaRow = (
    i64,
    String,
    Option<i64>,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
);

type AssetRevisionRow =
    (String, i64, Option<i64>, String, String, String, String, String, String, String, String, String, Option<String>);

impl Store {
    /// The whole work in the model of the exchange format.
    pub async fn exchange_dump(&self, work: WorkId) -> Result<baruch_exchange::ExchangeWork, StoreError> {
        // One reading transaction: all queries see the same state (WAL snapshot from the first one on).
        let mut tx = self.pool.begin().await?;
        let dumped = dump(&mut tx, work).await?;
        tx.commit().await?;
        Ok(dumped)
    }
}

async fn dump(conn: &mut SqliteConnection, work: WorkId) -> Result<ExchangeWork, StoreError> {
    let row: Option<WorkRow> = sqlx::query_as(
        "SELECT uuid, kind, title, subtitle, author, audience, bible, created, updated, numbering_scheme, numbering, \
         template FROM work WHERE id = ?",
    )
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((uuid, kind, title, subtitle, author, audience, bible, created, updated, scheme, numbering, template)) =
        row
    else {
        return Err(StoreError::UnknownWork(work));
    };
    let kind = match kind.as_str() {
        "book" => ExchangeWorkKind::Book,
        "sermon" => ExchangeWorkKind::Sermon,
        other => return Err(broken(format!("Werkart {other}"))),
    };

    let bible_module: Option<String> = sqlx::query_scalar("SELECT module FROM work_bible WHERE work_id = ?")
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
    let sword_sources = sqlx::query_as::<_, (String, String)>(
        "SELECT kind, module FROM work_sword_source WHERE work_id = ? ORDER BY kind, position, module",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(kind, module)| {
        let kind = match kind.as_str() {
            "commentary" => ExchangeSourceKind::Commentary,
            "lexicon" => ExchangeSourceKind::Lexicon,
            other => return Err(broken(format!("Quellenart {other}"))),
        };
        Ok(ExchangeSwordSource { kind, module })
    })
    .collect::<Result<Vec<_>, StoreError>>()?;
    let timing = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT words_per_minute, target_seconds FROM work_timing WHERE work_id = ?",
    )
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?
    .map(|(words, target)| {
        Ok::<_, StoreError>(ExchangeTiming {
            words_per_minute: number(words, "Wörter je Minute")?,
            target_seconds: target.map(|s| number(s, "Zielzeit")).transpose()?,
        })
    })
    .transpose()?;
    let book = book(conn, work).await?;
    let sheet = sheet(conn, work).await?;
    let typeset = crate::typeset::row(conn, work).await?.map(|t| ExchangeTypeset {
        uuid: t.uuid,
        updated: t.updated,
        updated_by: t.updated_by,
        settings: t.settings,
    });

    let template_revisions = sqlx::query_as::<_, (String, i64, String, String, String, String)>(
        "SELECT uuid, number, body, author_name, note, created FROM work_template_revision WHERE work_id = ? \
         ORDER BY number",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(uuid, n, body, author, note, created)| {
        Ok(ExchangeTemplateRevision { uuid, number: number(n, "Vorlagenfassung")?, body, author, note, created })
    })
    .collect::<Result<Vec<_>, StoreError>>()?;

    let (outline, node_keys) = outline(conn, work).await?;
    let ideas = ideas(conn, work, &node_keys).await?;
    let threads = threads(conn, work).await?;
    let assets = assets(conn, work).await?;
    let map = map(conn, work).await?;
    let findings = findings(conn, work).await?;
    let registers = crate::glossary::register_rows(conn, work)
        .await?
        .into_iter()
        .map(|r| ExchangeRegister { uuid: r.uuid, slug: r.slug, title: r.title, created: r.created })
        .collect();
    let glossary = crate::glossary::all_terms_in(conn, work)
        .await?
        .into_iter()
        .map(|t| ExchangeGlossaryTerm {
            uuid: t.uuid,
            term: t.term,
            short: t.short,
            long: t.long,
            source: t.source,
            variants: t.variants,
            bible_refs: t.bible_refs,
            status: match t.status {
                crate::GlossaryStatus::Accepted => ExchangeGlossaryStatus::Accepted,
                crate::GlossaryStatus::Proposed => ExchangeGlossaryStatus::Proposed,
            },
            origin: t.origin,
            created: t.created,
            updated: t.updated,
            updated_by: t.updated_by,
        })
        .collect();
    let deleted = sqlx::query_as::<_, (String, String, String)>(
        "SELECT uuid, kind, deleted FROM uuid_tombstone WHERE work_uuid = ? ORDER BY deleted, uuid",
    )
    .bind(&uuid)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(uuid, kind, deleted)| {
        let kind = ExchangeTombstoneKind::from_db(&kind).ok_or_else(|| broken(format!("Grabstein-Art {kind}")))?;
        Ok(ExchangeTombstone { uuid, kind, deleted })
    })
    .collect::<Result<Vec<_>, StoreError>>()?;

    let language: String =
        sqlx::query_scalar("SELECT language FROM work WHERE id = ?").bind(work).fetch_one(&mut *conn).await?;
    let language = baruch_exchange::ExchangeLanguage::from_xml(&language).unwrap_or_default();
    Ok(ExchangeWork {
        uuid,
        kind,
        title,
        subtitle,
        author,
        audience,
        bible,
        language,
        created,
        updated,
        numbering_scheme: scheme,
        numbering,
        bible_module,
        sword_sources,
        timing,
        book,
        template: ExchangeTemplate { current: template, revisions: template_revisions },
        outline,
        ideas,
        threads,
        assets,
        deleted,
        map,
        sheet,
        findings,
        typeset,
        registers,
        glossary,
    })
}

/// Old versions per post (`thread = false`, key `post.id`) or thread post (`thread = true`, key `chat_thread_post.id`)
/// of the work, oldest first (Round 28).
async fn post_versions(
    conn: &mut SqliteConnection,
    work: WorkId,
    thread: bool,
) -> Result<HashMap<i64, Vec<ExchangePostVersion>>, StoreError> {
    let sql = if thread {
        "SELECT v.thread_post_id, v.uuid, v.body, v.valid_from, v.replaced, v.edited_by FROM post_version v \
         JOIN chat_thread_post tp ON tp.id = v.thread_post_id JOIN chat_thread t ON t.id = tp.thread_id \
         WHERE t.work_id = ? ORDER BY v.id"
    } else {
        "SELECT v.post_id, v.uuid, v.body, v.valid_from, v.replaced, v.edited_by FROM post_version v \
         JOIN post p ON p.id = v.post_id JOIN node n ON n.id = p.node_id WHERE n.work_id = ? ORDER BY v.id"
    };
    let rows: Vec<(i64, String, String, String, String, String)> =
        sqlx::query_as(sql).bind(work).fetch_all(&mut *conn).await?;
    let mut out: HashMap<i64, Vec<ExchangePostVersion>> = HashMap::new();
    for (owner, uuid, body, valid_from, replaced, edited_by) in rows {
        out.entry(owner).or_default().push(ExchangePostVersion { uuid, body, valid_from, replaced, edited_by });
    }
    Ok(out)
}

/// Findings of the work with status history (Round 28), by id.
async fn findings(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<ExchangeFinding>, StoreError> {
    let entry_rows = sqlx::query(
        "SELECT s.finding_id, s.uuid, s.status, s.reason, s.adopted_revision, s.deferred_node, s.deferred_idea, \
         s.actor_kind, s.actor, s.proposed, s.note, s.decision, s.decided, s.decided_by, s.created, s.adopted_amend \
         FROM finding_status s JOIN finding f ON f.id = s.finding_id WHERE f.work_id = ? ORDER BY s.id",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut history: HashMap<i64, Vec<ExchangeFindingEntry>> = HashMap::new();
    for row in &entry_rows {
        let status: String = row.try_get(2)?;
        let kind: String = row.try_get(7)?;
        let proposed: i64 = row.try_get(9)?;
        let decision: Option<String> = row.try_get(11)?;
        let decision = decision
            .map(|d| ExchangeDecision::from_db(&d).ok_or_else(|| broken(format!("Entscheidung {d}"))))
            .transpose()?;
        history.entry(row.try_get(0)?).or_default().push(ExchangeFindingEntry {
            uuid: row.try_get(1)?,
            status: ExchangeFindingStatus::from_db(&status).ok_or_else(|| broken(format!("Befundstatus {status}")))?,
            reason: row.try_get(3)?,
            adopted_revision: row.try_get(4)?,
            deferred_node: row.try_get(5)?,
            deferred_idea: row.try_get(6)?,
            actor_kind: origin_of(&kind)?,
            actor: row.try_get(8)?,
            proposed: proposed == 1,
            note: row.try_get(10)?,
            decision,
            decided: row.try_get(12)?,
            decided_by: row.try_get(13)?,
            created: row.try_get(14)?,
            adopted_amend: amend_of(row.try_get(15)?)?,
        });
    }
    let rows = sqlx::query(
        "SELECT f.id, f.uuid, n.uuid, p.uuid, tp.uuid, f.position, f.persona, f.kind, f.body, f.quote, f.quote_check, \
         f.status, f.reason, f.adopted_revision, f.deferred_node, f.deferred_idea, s.uuid, f.summary_of, f.created, \
         f.updated, f.solutions, f.adopted_amend FROM finding f LEFT JOIN node n ON n.id = f.node_id LEFT JOIN post p ON p.id = f.post_id \
         LEFT JOIN chat_thread_post tp ON tp.id = f.thread_post_id LEFT JOIN finding s ON s.id = f.similar_to \
         WHERE f.work_id = ? ORDER BY f.id",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: i64 = row.try_get(0)?;
        let kind: String = row.try_get(7)?;
        let status: String = row.try_get(11)?;
        let summary: String = row.try_get(17)?;
        out.push(ExchangeFinding {
            uuid: row.try_get(1)?,
            node: row.try_get(2)?,
            post: row.try_get(3)?,
            thread_post: row.try_get(4)?,
            position: number(row.try_get(5)?, "Stelle eines Befunds")?,
            persona: row.try_get(6)?,
            kind: ExchangeFindingKind::from_db(&kind).ok_or_else(|| broken(format!("Befundart {kind}")))?,
            body: row.try_get(8)?,
            quote: row.try_get(9)?,
            quote_check: row.try_get(10)?,
            status: ExchangeFindingStatus::from_db(&status).ok_or_else(|| broken(format!("Befundstatus {status}")))?,
            reason: row.try_get(12)?,
            adopted_revision: row.try_get(13)?,
            deferred_node: row.try_get(14)?,
            deferred_idea: row.try_get(15)?,
            similar_to: row.try_get(16)?,
            summary_of: serde_json::from_str(&summary)
                .map_err(|e| broken(format!("Befund {id}: summary_of ist keine Liste ({e})")))?,
            created: row.try_get(18)?,
            updated: row.try_get(19)?,
            history: history.remove(&id).unwrap_or_default(),
            solutions: solutions_of(id, &row.try_get::<String, _>(20)?)?,
            adopted_amend: amend_of(row.try_get(21)?)?,
        });
    }
    Ok(out)
}

/// Round 22: state (`adopted_amend`, `amend_seq`, `based_on_amend`) from the database; negative → damaged.
fn amend_of(value: Option<i64>) -> Result<Option<u32>, StoreError> {
    value.map(|v| u32::try_from(v).map_err(|_| broken(format!("Stand {v}")))).transpose()
}

/// Solutions of a finding (Round 30b, `finding.solutions`) → `<loesung>`.
fn solutions_of(id: i64, json: &str) -> Result<Vec<ExchangeSolution>, StoreError> {
    let list: Vec<crate::Solution> =
        serde_json::from_str(json).map_err(|e| broken(format!("Befund {id}: solutions ist keine Liste ({e})")))?;
    Ok(list
        .into_iter()
        .map(|s| ExchangeSolution { quote: s.quote, replacement: s.replacement, step: s.step })
        .collect())
}

/// Metadata of a post (Round 30, migration 0047) → [`ExchangePostMeta`]: basis as UUID of the revision (only posts of a
/// section), reply/repetition as UUID of a post of **the same place** (a target at another place, for instance after
/// moving, cannot be expressed in the format and is dropped).
fn post_meta_of(
    basis: Option<String>,
    label: String,
    reply_to: Option<i64>,
    repeat_of: Option<i64>,
    relay_from: Option<String>,
    same_place: &HashMap<i64, String>,
) -> ExchangePostMeta {
    ExchangePostMeta {
        basis_revision: basis,
        basis_label: label,
        reply_to: reply_to.and_then(|id| same_place.get(&id).cloned()),
        repeat_of: repeat_of.and_then(|id| same_place.get(&id).cloned()),
        relay_from,
    }
}

type SheetRow =
    (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>);

/// Typeset view settings (`work_sheet`, Round 18c); without a row or completely empty `None`.
async fn sheet(conn: &mut SqliteConnection, work: WorkId) -> Result<Option<ExchangeSheet>, StoreError> {
    let row: Option<SheetRow> = sqlx::query_as(
        "SELECT font, heading_font, size, fill, heading_fill, background, width FROM work_sheet WHERE work_id = ?",
    )
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row
        .map(|(font, heading_font, size, fill, heading_fill, background, width)| ExchangeSheet {
            font,
            heading_font,
            size,
            fill,
            heading_fill,
            background,
            width,
        })
        .filter(|s| !s.is_empty()))
}

type BlockRow = (
    String,
    String,
    f64,
    f64,
    f64,
    String,
    f64,
    f64,
    String,
    Option<String>,
    String,
    String,
    i64,
    Option<String>,
    Option<String>,
    String,
);

/// Cover layout (`cover_block`, Round 27) in the order of the kinds.
async fn cover_blocks(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<ExchangeCoverBlock>, StoreError> {
    let rows: Vec<BlockRow> = sqlx::query_as(
        "SELECT uuid, kind, x_mm, y_mm, width_mm, align, size_pt, leading_em, color, font, style, backdrop, visible, \
         direction, text, updated FROM cover_block WHERE work_id = ?",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut blocks = Vec::with_capacity(rows.len());
    for row in rows {
        let (uuid, kind, x_mm, y_mm, width_mm, align, size_pt, leading_em, color, font, style, backdrop, visible) =
            (row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9, row.10, row.11, row.12);
        let (direction, text, updated) = (row.13, row.14, row.15);
        let kind = ExchangeCoverBlockKind::from_db(&kind).ok_or_else(|| broken(format!("Blockart {kind}")))?;
        let align = match align.as_str() {
            "left" => ExchangeCoverAlign::Left,
            "center" => ExchangeCoverAlign::Center,
            "right" => ExchangeCoverAlign::Right,
            "justify" => ExchangeCoverAlign::Justify,
            other => return Err(broken(format!("Ausrichtung {other}"))),
        };
        let style = match style.as_str() {
            "normal" => ExchangeFontStyle::Normal,
            "bold" => ExchangeFontStyle::Bold,
            "italic" => ExchangeFontStyle::Italic,
            other => return Err(broken(format!("Schnitt {other}"))),
        };
        let backdrop = match backdrop.as_str() {
            "none" => ExchangeBackdrop::None,
            "shadow" => ExchangeBackdrop::Shadow,
            "gradient" => ExchangeBackdrop::Gradient,
            other => return Err(broken(format!("Hintergrund {other}"))),
        };
        let direction = match direction.as_deref() {
            None => None,
            Some("down") => Some(ExchangeSpineDirection::Down),
            Some("up") => Some(ExchangeSpineDirection::Up),
            Some(other) => return Err(broken(format!("Laufrichtung {other}"))),
        };
        blocks.push(ExchangeCoverBlock {
            uuid,
            kind,
            x_mm,
            y_mm,
            width_mm,
            align,
            size_pt,
            leading_em,
            color,
            font,
            style,
            backdrop,
            visible: visible != 0,
            direction,
            text,
            updated,
        });
    }
    blocks.sort_by_key(|b| ExchangeCoverBlockKind::ALL.iter().position(|k| *k == b.kind));
    Ok(blocks)
}

fn origin_of(kind: &str) -> Result<ExchangeOriginKind, StoreError> {
    match kind {
        "human" => Ok(ExchangeOriginKind::Human),
        "persona" => Ok(ExchangeOriginKind::Persona),
        "mcp" => Ok(ExchangeOriginKind::Mcp),
        other => Err(broken(format!("Herkunft {other}"))),
    }
}

type LinkRow = (String, String, String, String, String, String, String, String, String, Option<String>);

/// Mind map: links between ideas of the work (also soft-deleted ones) and positions of its ideas.
async fn map(conn: &mut SqliteConnection, work: WorkId) -> Result<ExchangeMap, StoreError> {
    let rows: Vec<LinkRow> = sqlx::query_as(
        "SELECT l.uuid, a.uuid, b.uuid, l.kind, l.note, l.origin_kind, l.origin_name, l.created, l.updated, \
         l.deleted FROM idea_link l JOIN idea a ON a.id = l.from_id JOIN idea b ON b.id = l.to_id \
         WHERE a.work_id = ? AND b.work_id = ? ORDER BY l.id",
    )
    .bind(work)
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut links = Vec::with_capacity(rows.len());
    for (uuid, from, to, kind, note, origin, origin_name, created, updated, deleted) in rows {
        let kind = match kind.as_str() {
            "leads_to" => ExchangeLinkKind::LeadsTo,
            "belongs_to" => ExchangeLinkKind::BelongsTo,
            "example_of" => ExchangeLinkKind::ExampleOf,
            "contradicts" => ExchangeLinkKind::Contradicts,
            other => return Err(broken(format!("Verbindungsart {other}"))),
        };
        links.push(ExchangeLink {
            uuid,
            from,
            to,
            kind,
            note,
            origin_kind: origin_of(&origin)?,
            origin_name,
            created,
            updated,
            deleted,
        });
    }
    let positions = sqlx::query_as::<_, (String, String, f64, f64, String)>(
        "SELECT p.uuid, i.uuid, p.x, p.y, p.updated FROM idea_map_pos p JOIN idea i ON i.id = p.idea_id \
         WHERE p.work_id = ? AND i.work_id = ? ORDER BY p.id",
    )
    .bind(work)
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(uuid, idea, x, y, updated)| ExchangePosition { uuid, idea, x, y, updated })
    .collect();
    Ok(ExchangeMap { links, positions })
}

async fn book(conn: &mut SqliteConnection, work: WorkId) -> Result<Option<ExchangeBook>, StoreError> {
    type BookRow = (String, i64, i64, f64, f64, Option<String>, Option<String>, Option<String>);
    let row: Option<BookRow> = sqlx::query_as(
        "SELECT b.title_area, b.front_matter, b.cover_pages, b.paper_mm, b.bleed_mm, \
         (SELECT a.uuid FROM asset a WHERE a.id = b.cover_asset_id AND a.work_id = b.work_id), \
         (SELECT a.uuid FROM asset a WHERE a.id = b.author_photo_asset_id AND a.work_id = b.work_id), b.updated \
         FROM work_book b WHERE b.work_id = ?",
    )
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    let cover_blocks = cover_blocks(conn, work).await?;
    let Some((area, front, pages, paper, bleed, cover, photo, updated)) = row else {
        if cover_blocks.is_empty() {
            return Ok(None);
        }
        // Blocks without book details: the defaults of `BookMeta::default()`, so that `<umschlag>` has a `<buch>`.
        let meta = crate::BookMeta::default();
        return Ok(Some(ExchangeBook {
            title_area: ExchangeTitleArea::Top,
            front_matter: meta.front_matter,
            cover_pages: meta.cover_pages,
            paper_mm: meta.paper_mm,
            bleed_mm: meta.bleed_mm,
            cover_blocks,
            ..ExchangeBook::default()
        }));
    };
    let (publisher_address, printer_address): (String, String) =
        sqlx::query_as("SELECT publisher_address, printer_address FROM work_book WHERE work_id = ?")
            .bind(work)
            .fetch_one(&mut *conn)
            .await?;
    type S = String;
    type BookTexts = (S, S, S, S, S, S, S, S, S, S, S, S, S);
    let texts: BookTexts = sqlx::query_as(
        "SELECT publisher, place, year, edition, isbn, price, series, printer, bible_notice, image_credit, \
             dedication, blurb, bio FROM work_book WHERE work_id = ?",
    )
    .bind(work)
    .fetch_one(&mut *conn)
    .await?;
    let title_area = match area.as_str() {
        "top" => ExchangeTitleArea::Top,
        "middle" => ExchangeTitleArea::Middle,
        "bottom" => ExchangeTitleArea::Bottom,
        other => return Err(broken(format!("Titelbereich {other}"))),
    };
    let (
        publisher,
        place,
        year,
        edition,
        isbn,
        price,
        series,
        printer,
        bible_notice,
        image_credit,
        dedication,
        blurb,
        bio,
    ) = texts;
    Ok(Some(ExchangeBook {
        title_area,
        front_matter: front != 0,
        cover_pages: pages != 0,
        paper_mm: paper,
        bleed_mm: bleed,
        cover,
        author_photo: photo,
        updated,
        publisher,
        place,
        year,
        edition,
        isbn,
        price,
        series,
        printer,
        bible_notice,
        image_credit,
        dedication,
        blurb,
        bio,
        publisher_address,
        printer_address,
        cover_blocks,
    }))
}

/// Outline as a tree (children by `position`) with all revisions and posts; in addition key → UUID.
async fn outline(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<(Vec<ExchangeNode>, HashMap<i64, String>), StoreError> {
    let nodes: Vec<NodeRow> = sqlx::query_as(
        "SELECT id, seq, parent_seq, position, title, kind, numbered, uuid FROM node WHERE work_id = ? \
         ORDER BY position, seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let by_seq: HashMap<i64, String> = nodes.iter().map(|n| (n.1, n.7.clone())).collect();
    let by_key: HashMap<i64, String> = nodes.iter().map(|n| (n.0, n.7.clone())).collect();

    // Revisions per item (by number) and (item, number) → UUID for basis and references.
    let rows: Vec<RevisionRow> = sqlx::query_as(
        "SELECT r.node_id, r.uuid, r.number, r.status, r.author_kind, r.author_name, r.model, r.based_on, r.note, \
         r.body, r.created, r.edited, r.variant_group, r.variant_label, r.variant_summary, r.variant_findings \
         FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ? \
         ORDER BY r.node_id, r.number",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let revision_uuid: HashMap<(i64, i64), String> = rows.iter().map(|r| ((r.0, r.2), r.1.clone())).collect();
    let mut revisions: HashMap<i64, Vec<ExchangeRevision>> = HashMap::new();
    // Round 22 (minor version 8): state and state of the base per revision, history of the amendments (by step).
    let stands: HashMap<String, (i64, Option<i64>)> = sqlx::query_as::<_, (String, i64, Option<i64>)>(
        "SELECT r.uuid, r.amend_seq, r.based_on_amend FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND r.uuid IS NOT NULL",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(uuid, seq, based)| (uuid, (seq, based)))
    .collect();
    type AmendRow = (String, String, i64, String, Option<i64>, String, String, String, String);
    let amend_rows: Vec<AmendRow> = sqlx::query_as(
        "SELECT r.uuid, a.uuid, a.seq, a.kind, a.undo_of, a.author_name, a.created, a.body_before, a.body_after \
         FROM revision_amend a JOIN revision r ON r.id = a.revision_id JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND a.uuid IS NOT NULL AND r.uuid IS NOT NULL ORDER BY a.revision_id, a.seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut amends: HashMap<String, Vec<ExchangeAmend>> = HashMap::new();
    for (revision, uuid, seq, kind, undo_of, author, created, before, after) in amend_rows {
        amends.entry(revision).or_default().push(ExchangeAmend {
            uuid,
            seq: number(seq, "Schritt einer Fortschreibung")?,
            kind: ExchangeAmendKind::from_db(&kind).ok_or_else(|| broken(format!("Art der Fortschreibung {kind}")))?,
            undo_of: undo_of.map(|n| number(n, "undo_of einer Fortschreibung")).transpose()?,
            author,
            created,
            before,
            after,
        });
    }
    // Round 30b: findings of a variant as UUIDs (deleted findings are dropped).
    let finding_uuid: HashMap<i64, String> =
        sqlx::query_as::<_, (i64, String)>("SELECT id, uuid FROM finding WHERE work_id = ? AND uuid IS NOT NULL")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
    for (
        node,
        uuid,
        n,
        status,
        author_kind,
        author_name,
        model,
        based_on,
        note,
        body,
        created,
        edited,
        group,
        label,
        summary,
        variant_findings,
    ) in rows
    {
        let status = match status.as_str() {
            "current" => ExchangeRevisionStatus::Current,
            "proposed" => ExchangeRevisionStatus::Proposed,
            "rejected" => ExchangeRevisionStatus::Rejected,
            "superseded" => ExchangeRevisionStatus::Superseded,
            other => return Err(broken(format!("Status {other}"))),
        };
        let author = match (author_kind.as_str(), model) {
            ("assistant", Some(model)) => ExchangeAuthor::Assistant { persona: author_name, model },
            ("human", _) => ExchangeAuthor::Human { name: author_name },
            (other, _) => return Err(broken(format!("Autor {other}"))),
        };
        let based_on = based_on.and_then(|b| revision_uuid.get(&(node, b)).cloned());
        let variant = match crate::variants::from_columns(group, label, summary, &variant_findings)? {
            Some(v) => Some(ExchangeVariant {
                group: v.group,
                label: v.label,
                summary: v.summary,
                findings: v.findings.iter().filter_map(|id| finding_uuid.get(id).cloned()).collect(),
            }),
            None => None,
        };
        let (amend_seq, based_on_amend) = stands.get(&uuid).copied().unwrap_or((0, None));
        revisions.entry(node).or_default().push(ExchangeRevision {
            amend_seq: amend_of(Some(amend_seq))?.unwrap_or(0),
            based_on_amend: amend_of(based_on_amend)?,
            amends: amends.remove(&uuid).unwrap_or_default(),
            uuid,
            number: number(n, "Fassungsnummer")?,
            status,
            author,
            based_on,
            note,
            body,
            created,
            edited,
            variant,
        });
    }

    // Posts per item (by position) together with references and old versions.
    let mut versions = post_versions(conn, work, false).await?;
    type PostRow = (
        i64,
        i64,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<i64>,
        String,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );
    let post_rows: Vec<PostRow> = sqlx::query_as(
        "SELECT p.id, p.node_id, p.uuid, p.author, p.body, p.sources, p.created, p.edited, p.basis_rev, \
         p.basis_label, p.reply_to, p.repeat_of, p.relay_from FROM post p \
         JOIN node n ON n.id = p.node_id WHERE n.work_id = ? ORDER BY p.node_id, p.position",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let ref_rows: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "SELECT r.post_id, r.node_seq, r.rev, r.quote FROM post_ref r JOIN post p ON p.id = r.post_id \
         JOIN node n ON n.id = p.node_id WHERE n.work_id = ? ORDER BY r.post_id, r.position",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let key_of_seq: HashMap<i64, i64> = nodes.iter().map(|n| (n.1, n.0)).collect();
    let mut refs: HashMap<i64, Vec<ExchangePostRef>> = HashMap::new();
    for (post, seq, rev, quote) in ref_rows {
        // A reference to a place that no longer exists cannot be expressed through a UUID.
        let (Some(node), Some(key)) = (by_seq.get(&seq), key_of_seq.get(&seq)) else {
            continue;
        };
        let revision = revision_uuid.get(&(*key, rev)).cloned();
        refs.entry(post).or_default().push(ExchangePostRef { node: node.clone(), revision, quote });
    }
    let mut posts: HashMap<i64, Vec<ExchangePost>> = HashMap::new();
    // Round 30: posts per place (id → UUID) for reply to and repetition of.
    let mut post_uuid_by_node: HashMap<i64, HashMap<i64, String>> = HashMap::new();
    for row in &post_rows {
        post_uuid_by_node.entry(row.1).or_default().insert(row.0, row.2.clone());
    }
    let empty = HashMap::new();
    for (id, node, uuid, author, body, sources_json, created, edited, basis, label, reply_to, repeat_of, relay) in
        post_rows
    {
        let same = post_uuid_by_node.get(&node).unwrap_or(&empty);
        let basis = basis.and_then(|b| revision_uuid.get(&(node, b)).cloned());
        let meta = post_meta_of(basis, label, reply_to, repeat_of, relay, same);
        let list = posts.entry(node).or_default();
        list.push(ExchangePost {
            meta,
            uuid,
            position: number(list.len() as i64 + 1, "Position")?,
            author,
            body,
            sources: sources(&sources_json, "einem Beitrag")?,
            refs: refs.remove(&id).unwrap_or_default(),
            created,
            edited,
            versions: versions.remove(&id).unwrap_or_default(),
        });
    }

    // Tree: children per parent (seq) in the order of `position`.
    let mut children: HashMap<Option<i64>, Vec<&NodeRow>> = HashMap::new();
    for row in &nodes {
        children.entry(row.2).or_default().push(row);
    }
    fn build(
        parent: Option<i64>,
        children: &HashMap<Option<i64>, Vec<&NodeRow>>,
        revisions: &mut HashMap<i64, Vec<ExchangeRevision>>,
        posts: &mut HashMap<i64, Vec<ExchangePost>>,
        depth: usize,
    ) -> Result<Vec<ExchangeNode>, StoreError> {
        if depth > 16 {
            return Err(broken("Gliederung zu tief oder mit Kreis"));
        }
        let mut out = Vec::new();
        for (key, seq, _, _, title, kind, numbered, uuid) in children.get(&parent).into_iter().flatten().copied() {
            let kind = match kind.as_str() {
                "part" => ExchangeNodeKind::Part,
                "chapter" => ExchangeNodeKind::Chapter,
                "section" => ExchangeNodeKind::Section,
                other => return Err(broken(format!("Punktart {other}"))),
            };
            out.push(ExchangeNode {
                uuid: uuid.clone(),
                kind,
                numbered: *numbered != 0,
                title: title.clone(),
                // Round 30: fixed key for the cross-references `#link(<kN>)`.
                key: Some(*key),
                revisions: revisions.remove(key).unwrap_or_default(),
                posts: posts.remove(key).unwrap_or_default(),
                children: build(Some(*seq), children, revisions, posts, depth + 1)?,
            });
        }
        Ok(out)
    }
    let tree = build(None, &children, &mut revisions, &mut posts, 0)?;
    Ok((tree, by_key))
}

/// Ideas of the work, also deleted ones, by creation (id).
async fn ideas(
    conn: &mut SqliteConnection,
    work: WorkId,
    node_keys: &HashMap<i64, String>,
) -> Result<Vec<ExchangeIdea>, StoreError> {
    let rows: Vec<IdeaRow> = sqlx::query_as(
        "SELECT id, uuid, node_id, note, quote, source, bible, origin_kind, origin_name, status, done_as, done_ref, \
         created, updated, deleted FROM idea WHERE work_id = ? ORDER BY id",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    // Round 34 (a query of its own, because sqlx reads tuples only up to 16 columns): title, UUID of the parent node
    // (only if it belongs to the same work), order, main topic.
    let tree: Vec<(i64, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT i.id, i.title, (SELECT p.uuid FROM idea p WHERE p.id = i.parent_id AND p.work_id = i.work_id), \
         i.position, i.main_topic FROM idea i WHERE i.work_id = ?",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut tree_of: HashMap<i64, (String, Option<String>, i64, i64)> = tree
        .into_iter()
        .map(|(id, title, parent, position, main_topic)| (id, (title, parent, position, main_topic)))
        .collect();
    let words: Vec<(i64, String)> = sqlx::query_as(
        "SELECT w.idea_id, w.word FROM idea_word w JOIN idea i ON i.id = w.idea_id WHERE i.work_id = ? \
         ORDER BY w.idea_id, w.position",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut words_of: HashMap<i64, Vec<String>> = HashMap::new();
    for (idea, word) in words {
        words_of.entry(idea).or_default().push(word);
    }
    let mut out = Vec::with_capacity(rows.len());
    for (
        id,
        uuid,
        node,
        note,
        quote,
        source,
        bible,
        origin_kind,
        origin_name,
        status,
        done_as,
        done_ref,
        created,
        updated,
        deleted,
    ) in rows
    {
        let (title, parent, position, main_topic) = tree_of.remove(&id).unwrap_or_default();
        let origin_kind = origin_of(&origin_kind)?;
        let status = match status.as_str() {
            "open" => ExchangeIdeaStatus::Open,
            "done" => ExchangeIdeaStatus::Done,
            other => return Err(broken(format!("Einfall-Status {other}"))),
        };
        let done_as = match done_as.as_deref() {
            None => None,
            Some("outline") => Some(ExchangeIdeaUse::Outline),
            Some("post") => Some(ExchangeIdeaUse::Post),
            Some("persona") => Some(ExchangeIdeaUse::Persona),
            Some("note") => Some(ExchangeIdeaUse::Note),
            Some(other) => return Err(broken(format!("erledigt als {other}"))),
        };
        out.push(ExchangeIdea {
            uuid,
            node: node.and_then(|key| node_keys.get(&key).cloned()),
            words: words_of.remove(&id).unwrap_or_default(),
            note,
            quote,
            source,
            bible,
            origin_kind,
            origin_name,
            status,
            done_as,
            done_ref: done_ref.map(|text| translate_done_ref(&text, node_keys)),
            created,
            updated,
            deleted,
            title,
            // `main_topic` applies only without parent node and reference (that is how the store keeps it); here once
            // more, so that the output is unambiguous even for rows changed by hand.
            main_topic: main_topic != 0 && parent.is_none() && node.is_none(),
            parent,
            position,
        });
    }
    Ok(out)
}

/// "key N…" → "<UUID of the item>…" if the item exists in the work; otherwise verbatim.
fn translate_done_ref(text: &str, node_keys: &HashMap<i64, String>) -> String {
    let Some(rest) = text.strip_prefix("key ") else {
        return text.to_owned();
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    match rest[..digits].parse::<i64>().ok().and_then(|key| node_keys.get(&key)) {
        Some(uuid) if digits > 0 => format!("{uuid}{}", &rest[digits..]),
        _ => text.to_owned(),
    }
}

/// Threads without a section with their posts, by creation.
async fn threads(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<ExchangeThread>, StoreError> {
    let rows: Vec<(i64, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, uuid, title, created_by, created, updated FROM chat_thread WHERE work_id = ? ORDER BY id",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut versions = post_versions(conn, work, true).await?;
    type ThreadPostRow = (
        i64,
        i64,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );
    let post_rows: Vec<ThreadPostRow> = sqlx::query_as(
        "SELECT p.id, p.thread_id, p.uuid, p.author, p.body, p.sources, p.created, p.edited, p.basis_label, \
         p.reply_to, p.repeat_of, p.relay_from FROM chat_thread_post p \
         JOIN chat_thread t ON t.id = p.thread_id WHERE t.work_id = ? ORDER BY p.thread_id, p.position",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut posts: HashMap<i64, Vec<ExchangeThreadPost>> = HashMap::new();
    let mut uuid_by_thread: HashMap<i64, HashMap<i64, String>> = HashMap::new();
    for row in &post_rows {
        uuid_by_thread.entry(row.1).or_default().insert(row.0, row.2.clone());
    }
    let empty = HashMap::new();
    for (id, thread, uuid, author, body, sources_json, created, edited, label, reply_to, repeat_of, relay) in post_rows
    {
        // A thread post has no revisions: the basis stays empty (only the header line travels along).
        let meta = post_meta_of(None, label, reply_to, repeat_of, relay, uuid_by_thread.get(&thread).unwrap_or(&empty));
        let list = posts.entry(thread).or_default();
        list.push(ExchangeThreadPost {
            meta,
            uuid,
            position: number(list.len() as i64 + 1, "Position")?,
            author,
            body,
            sources: sources(&sources_json, "einem Fadenbeitrag")?,
            created,
            edited,
            versions: versions.remove(&id).unwrap_or_default(),
        });
    }
    Ok(rows
        .into_iter()
        .map(|(id, uuid, title, created_by, created, updated)| ExchangeThread {
            uuid,
            title,
            created_by,
            created,
            updated,
            posts: posts.remove(&id).unwrap_or_default(),
        })
        .collect())
}

/// Images of the work by creation, each with paths and all versions.
async fn assets(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<ExchangeAsset>, StoreError> {
    let rows: Vec<(i64, String, String, String)> =
        sqlx::query_as("SELECT id, uuid, name, created FROM asset WHERE work_id = ? ORDER BY id")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, uuid, name, created) in rows {
        let paths: Vec<String> =
            sqlx::query_scalar("SELECT path FROM asset_path WHERE work_id = ? AND asset_id = ? ORDER BY path")
                .bind(work)
                .bind(id)
                .fetch_all(&mut *conn)
                .await?;
        let revision_rows: Vec<AssetRevisionRow> = sqlx::query_as(
            "SELECT uuid, number, based_on, mime, encoding, content, caption, alt, author_name, note, status, \
             created, edited FROM asset_revision WHERE asset_id = ? ORDER BY number",
        )
        .bind(id)
        .fetch_all(&mut *conn)
        .await?;
        let by_number: HashMap<i64, String> = revision_rows.iter().map(|r| (r.1, r.0.clone())).collect();
        let mut revisions = Vec::with_capacity(revision_rows.len());
        for (rev_uuid, n, based_on, mime, encoding, content, caption, alt, author, note, status, rev_created, edited) in
            revision_rows
        {
            let data = match encoding.as_str() {
                "base64" => crate::decode_base64(&content)?,
                "svg" => content.into_bytes(),
                other => return Err(broken(format!("Bildkodierung {other}"))),
            };
            let status = match status.as_str() {
                "current" => ExchangeAssetStatus::Current,
                "superseded" => ExchangeAssetStatus::Superseded,
                other => return Err(broken(format!("Bildstatus {other}"))),
            };
            revisions.push(ExchangeAssetRevision {
                uuid: rev_uuid,
                number: number(n, "Bildfassung")?,
                based_on: based_on.and_then(|b| by_number.get(&b).cloned()),
                mime,
                sha256: sha256_hex(&data),
                data,
                caption,
                alt,
                author,
                note,
                status,
                created: rev_created,
                edited,
            });
        }
        out.push(ExchangeAsset { uuid, name, created, paths, revisions });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_ref_key_becomes_the_node_uuid() {
        let keys: HashMap<i64, String> = [(12, "26000000-0000-4000-8000-00000000000c".to_owned())].into();
        // "key 12" → UUID; the rest („, Beitrag 3“) stays verbatim, because it is not a numeric id.
        assert_eq!(translate_done_ref("key 12", &keys), "26000000-0000-4000-8000-00000000000c", "nur key");
        assert_eq!(
            translate_done_ref("key 12, Beitrag 3", &keys),
            "26000000-0000-4000-8000-00000000000c, Beitrag 3",
            "key mit Rest"
        );
        // An unknown item, "key 120" (not "key 12") and free text stay unchanged.
        assert_eq!(translate_done_ref("key 99", &keys), "key 99", "unbekannter Punkt bleibt Text");
        assert_eq!(translate_done_ref("key 120", &keys), "key 120", "keine Teilzahl");
        assert_eq!(translate_done_ref("Notiz", &keys), "Notiz", "freier Text");
        assert_eq!(translate_done_ref("key ", &keys), "key ", "ohne Zahl");
    }
}
