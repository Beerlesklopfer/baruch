// SPDX-License-Identifier: AGPL-3.0-or-later
//! Requests concerning images (migration 0018, contract Round 11; pattern [`crate::OutlineRequest`]).
//!
//! Assistants and MCP clients create images at once (MCP `asset_add`), but **replacing**, **setting as cover or
//! removing the cover**, **deleting** and, since Round 27, **changing the cover layout** (MCP `cover_layout_set`, op
//! `cover_layout`) change nothing at once: [`Store::asset_request`] creates a request. The human confirms or rejects; a
//! confirmed request is executed in the server (image through the functions of the store, cover through
//! `work.set_cover`), afterwards [`Store::asset_request_decide`] records the decision.
//!
//! Rules:
//! - On the request it is checked what would not work even now (unknown image, broken or too large content, another
//!   image type, unchanged image, deleting the current version); then there is no request.
//! - The request remembers the state: `expected_sha256` (SHA-256 of the current or of the named version; passed empty =
//!   the store enters it) or `expected_cover` (cover at the time of the request) or `expected_layout` (fingerprint of
//!   the stored layout, [`Store::cover_layout_fingerprint`]; passed empty = the store enters it). A request concerning
//!   the layout checks every block like [`crate::check_cover_block`] (stored in normal form) and is rejected if it
//!   would change nothing. If it no longer matches on confirmation, the server closes the request as `stale` with a
//!   reason.
//! - An identical open request is not created twice; a request is decided exactly once. The origin is always a persona
//!   or an MCP client (the human changes directly).
//! - **Round 21c:** a request to replace may change the format if it says so explicitly (`rename`,
//!   [`Store::asset_request_with`]); on confirmation the image then gets the new extension
//!   ([`Store::replace_asset_renaming`]). The content of decided requests to replace is removed after
//!   [`ASSET_REQUEST_PURGE_DAYS`] days ([`Store::purge_decided_asset_requests`], `purged`; the size stays).
//!   [`Store::asset_mcp_usage`] counts from the existing rows what a requester has written in the last 24 hours (quota
//!   of the MCP tools, limits in the server).

use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use crate::{
    AssetEncoding, CoverBlock, Decision, IdeaOrigin, MAX_ASSET_BYTES, Store, StoreError, WorkId, broken, decode_base64,
    detect_mime,
};

/// What a request concerning an image wants.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AssetOp {
    /// Replace the image `name` with new content (new version, current at once on confirmation).
    Replace {
        name: String,
        /// MIME type of the new content (passed empty = detected).
        mime: String,
        encoding: AssetEncoding,
        /// Base64 or SVG text; left out in answers (up to 8 MiB), `bytes` stands there.
        #[serde(skip_serializing)]
        content: String,
        /// Size decoded (entered by the store).
        bytes: u64,
        caption: Option<String>,
        alt: Option<String>,
        expected_sha256: String,
    },
    /// Set the image `name` as the cover (`None` = remove the cover).
    SetCover { name: Option<String>, expected_cover: Option<String> },
    /// Delete the image `name` completely (`rev` = `None`) or a version that is not current.
    Delete { name: String, rev: Option<u32>, expected_sha256: String },
    /// Change the cover layout (Round 27): every block replaces its kind completely. `expected_layout` is the
    /// fingerprint of the stored layout at the time of the request (how it is formed: umschlag-daten).
    CoverLayout { blocks: Vec<CoverBlock>, expected_layout: String },
}

impl AssetOp {
    /// Value of the column `op`.
    pub fn name(&self) -> &'static str {
        match self {
            AssetOp::Replace { .. } => "replace",
            AssetOp::SetCover { .. } => "set_cover",
            AssetOp::Delete { .. } => "delete",
            AssetOp::CoverLayout { .. } => "cover_layout",
        }
    }

    /// Value of the column `asset_name` (empty for "remove the cover").
    pub fn asset_name(&self) -> &str {
        match self {
            AssetOp::Replace { name, .. } | AssetOp::Delete { name, .. } => name,
            AssetOp::SetCover { name, .. } => name.as_deref().unwrap_or(""),
            AssetOp::CoverLayout { .. } => "",
        }
    }
}

/// Days after the decision after which the content of a request to replace is removed (Round 21c).
pub const ASSET_REQUEST_PURGE_DAYS: u32 = 30;

/// What a requester (persona or MCP client, `origin_name`) has written in a work through MCP in the last 24 hours
/// ([`Store::asset_mcp_usage`], Round 21c), counted from existing rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct AssetUsage {
    /// Image versions created with this author (`asset_add`) plus requests to replace (`asset_replace`).
    pub writes: usize,
    /// Decoded bytes of these versions and requests.
    pub bytes: u64,
    /// Open requests to replace by this requester (no matter how old).
    pub open_replace: usize,
}

/// A request concerning an image.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssetRequest {
    pub id: i64,
    pub work: WorkId,
    #[serde(flatten)]
    pub op: AssetOp,
    /// Requester (persona or MCP client).
    pub origin: IdeaOrigin,
    pub note: String,
    pub created: String,
    /// `None` = open.
    pub decided: Option<String>,
    pub decision: Option<Decision>,
    pub decided_by: Option<String>,
    pub message: Option<String>,
    /// German for list and commit, for example „„haus.png“ durch eine neue Fassung ersetzen (PNG, 3,2 KiB)“.
    pub summary: String,
    /// Round 21c: replacing with a change of type (the image gets the new extension on confirmation).
    pub rename: bool,
    /// Round 21c: content of a decided request to replace removed after [`ASSET_REQUEST_PURGE_DAYS`] days.
    pub purged: bool,
}

impl AssetRequest {
    /// Not decided yet?
    pub fn is_open(&self) -> bool {
        self.decision.is_none()
    }
}

// ---- Stored form in `params` (contract §1) ------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct ReplaceParams {
    mime: String,
    encoding: AssetEncoding,
    content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alt: Option<String>,
    expected_sha256: String,
    /// Round 21c: change of type allowed (missing in older rows = no).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    rename: bool,
    /// Round 21c: content removed (then `content` is empty and `bytes` the size before).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    purged: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bytes: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct DeleteParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rev: Option<u32>,
    expected_sha256: String,
}

#[derive(Serialize, Deserialize)]
struct CoverParams {
    expected_cover: Option<String>,
}

/// Stored form of a request concerning the cover layout (Round 27).
#[derive(Serialize, Deserialize)]
struct LayoutParams {
    blocks: Vec<CoverBlock>,
    expected_layout: String,
}

fn params_of(op: &AssetOp, rename: bool) -> Result<String, StoreError> {
    let text = match op {
        AssetOp::Replace { mime, encoding, content, bytes, caption, alt, expected_sha256, .. } => {
            serde_json::to_string(&ReplaceParams {
                mime: mime.clone(),
                encoding: *encoding,
                content: content.clone(),
                caption: caption.clone(),
                alt: alt.clone(),
                expected_sha256: expected_sha256.clone(),
                rename,
                purged: false,
                // Round 21c: write the size along right away (quota without reading the content, stays when cleaning
                // up).
                bytes: Some(*bytes),
            })
        }
        AssetOp::Delete { rev, expected_sha256, .. } => {
            serde_json::to_string(&DeleteParams { rev: *rev, expected_sha256: expected_sha256.clone() })
        }
        AssetOp::SetCover { expected_cover, .. } => {
            serde_json::to_string(&CoverParams { expected_cover: expected_cover.clone() })
        }
        AssetOp::CoverLayout { blocks, expected_layout } => {
            serde_json::to_string(&LayoutParams { blocks: blocks.clone(), expected_layout: expected_layout.clone() })
        }
    };
    text.map_err(|e| broken(format!("Bildantrag: {e}")))
}

/// `(rename, purged)` from the stored parameters (only set for `replace`).
fn replace_flags(op: &str, params: &str) -> (bool, bool) {
    if op != "replace" {
        return (false, false);
    }
    serde_json::from_str::<ReplaceParams>(params).map(|p| (p.rename, p.purged)).unwrap_or((false, false))
}

fn op_of(id: i64, op: &str, name: String, params: &str) -> Result<AssetOp, StoreError> {
    let unreadable = |e: serde_json::Error| broken(format!("Bildantrag {id}: params unlesbar: {e}"));
    match op {
        "replace" => {
            let p: ReplaceParams = serde_json::from_str(params).map_err(unreadable)?;
            let bytes = match p.bytes {
                Some(bytes) => bytes,
                None => content_bytes(&p.content, p.encoding).map(|b| b.len() as u64).unwrap_or(0),
            };
            Ok(AssetOp::Replace {
                name,
                mime: p.mime,
                encoding: p.encoding,
                content: p.content,
                bytes,
                caption: p.caption,
                alt: p.alt,
                expected_sha256: p.expected_sha256,
            })
        }
        "delete" => {
            let p: DeleteParams = serde_json::from_str(params).map_err(unreadable)?;
            Ok(AssetOp::Delete { name, rev: p.rev, expected_sha256: p.expected_sha256 })
        }
        "set_cover" => {
            let p: CoverParams = serde_json::from_str(params).map_err(unreadable)?;
            Ok(AssetOp::SetCover { name: (!name.is_empty()).then_some(name), expected_cover: p.expected_cover })
        }
        "cover_layout" => {
            let p: LayoutParams = serde_json::from_str(params).map_err(unreadable)?;
            Ok(AssetOp::CoverLayout { blocks: p.blocks, expected_layout: p.expected_layout })
        }
        other => Err(broken(format!("Bildantrag {id}: unbekannte Art {other:?}"))),
    }
}

/// Bytes of the content (Base64 decoded or SVG text).
fn content_bytes(content: &str, encoding: AssetEncoding) -> Result<Vec<u8>, StoreError> {
    match encoding {
        AssetEncoding::Base64 => decode_base64(content),
        AssetEncoding::Svg => Ok(content.as_bytes().to_vec()),
    }
}

fn type_word(mime: &str) -> &'static str {
    match mime {
        "image/png" => "PNG",
        "image/jpeg" => "JPEG",
        "image/gif" => "GIF",
        "image/webp" => "WebP",
        "image/svg+xml" => "SVG",
        _ => "Bild",
    }
}

/// "512 Bytes", "3,2 KiB", "1,5 MiB".
fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} Bytes")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0).replace('.', ",")
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0)).replace('.', ",")
    }
}

/// Summary in German.
pub(crate) fn summary(op: &AssetOp) -> String {
    match op {
        AssetOp::Replace { name, mime, bytes, .. } => {
            format!("„{name}“ durch eine neue Fassung ersetzen ({}, {})", type_word(mime), size_text(*bytes))
        }
        AssetOp::SetCover { name: Some(name), .. } => format!("„{name}“ als Umschlag setzen"),
        AssetOp::SetCover { name: None, .. } => "Umschlag entfernen".to_owned(),
        AssetOp::Delete { name, rev: None, .. } => format!("Bild „{name}“ mit allen Fassungen löschen"),
        AssetOp::Delete { name, rev: Some(rev), .. } => format!("Fassung {rev} von „{name}“ löschen"),
        AssetOp::CoverLayout { blocks, .. } if blocks.len() == 1 => "Umschlag-Anordnung ändern (1 Block)".to_owned(),
        AssetOp::CoverLayout { blocks, .. } => format!("Umschlag-Anordnung ändern ({} Blöcke)", blocks.len()),
    }
}

fn decision_column(decision: Decision) -> &'static str {
    match decision {
        Decision::Confirmed => "confirmed",
        Decision::Rejected => "rejected",
        Decision::Stale => "stale",
    }
}

fn decision_of(text: &str) -> Result<Decision, StoreError> {
    match text {
        "confirmed" => Ok(Decision::Confirmed),
        "rejected" => Ok(Decision::Rejected),
        "stale" => Ok(Decision::Stale),
        other => Err(broken(format!("unbekannte Entscheidung {other:?}"))),
    }
}

type RequestRow = (
    i64,
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

const COLUMNS: &str = "id, work_id, op, asset_name, params, origin_kind, origin_name, note, created, decided, \
                       decision, decided_by, message";

fn from_row(row: RequestRow) -> Result<AssetRequest, StoreError> {
    let (id, work, op, name, params, origin_kind, origin_name, note, created, decided, decision, decided_by, message) =
        row;
    let (rename, purged) = replace_flags(&op, &params);
    let op = op_of(id, &op, name, &params)?;
    let origin = match origin_kind.as_str() {
        "persona" => IdeaOrigin::Persona { handle: origin_name },
        "mcp" => IdeaOrigin::Mcp { handle: origin_name },
        other => return Err(broken(format!("Bildantrag {id}: unbekannte Herkunft {other:?}"))),
    };
    let decision = decision.as_deref().map(decision_of).transpose()?;
    let mut summary = summary(&op);
    if rename {
        summary.push_str(" – mit neuer Endung");
    }
    Ok(AssetRequest {
        id,
        work,
        op,
        origin,
        note,
        created,
        decided,
        decision,
        decided_by,
        message,
        summary,
        rename,
        purged,
    })
}

async fn fetch(conn: &mut SqliteConnection, work: WorkId, id: i64) -> Result<AssetRequest, StoreError> {
    let row: Option<RequestRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM asset_request WHERE id = ? AND work_id = ?"
    )))
    .bind(id)
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    from_row(row.ok_or(StoreError::UnknownRequest { work, id })?)
}

async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Versions of an image (through the public query of the store, in the same connection).
async fn revisions(
    conn: &mut SqliteConnection,
    work: WorkId,
    name: &str,
) -> Result<Vec<(u32, bool, String, String)>, StoreError> {
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT r.number, r.status, r.sha256, r.mime FROM asset_revision r JOIN asset a ON a.id = r.asset_id \
         WHERE a.work_id = ? AND a.name = ? ORDER BY r.number",
    )
    .bind(work)
    .bind(name)
    .fetch_all(&mut *conn)
    .await?;
    if rows.is_empty() {
        work_exists(conn, work).await?;
        return Err(StoreError::UnknownAsset { work, name: name.to_owned() });
    }
    rows.into_iter()
        .map(|(number, status, sha, mime)| {
            let number = u32::try_from(number).map_err(|_| broken(format!("Bild {name}: Fassung {number}")))?;
            Ok((number, status == "current", sha, mime))
        })
        .collect()
}

/// Check or enter the expected state.
fn expect(expected: &mut String, actual: &str, name: &str) -> Result<(), StoreError> {
    if expected.trim().is_empty() {
        *expected = actual.to_owned();
        return Ok(());
    }
    if !expected.trim().eq_ignore_ascii_case(actual) {
        return Err(StoreError::Asset(format!(
            "Das Bild „{name}“ hat inzwischen einen anderen Inhalt (expected_sha256 passt nicht); bitte neu lesen."
        )));
    }
    *expected = actual.to_owned();
    Ok(())
}

/// Check and complete a request (MIME, size, expected state). Error = no request.
async fn checked(conn: &mut SqliteConnection, work: WorkId, op: &AssetOp, rename: bool) -> Result<AssetOp, StoreError> {
    let mut op = op.clone();
    match &mut op {
        AssetOp::Replace { name, mime, encoding, content, bytes, caption, alt, expected_sha256 } => {
            let list = revisions(conn, work, name).await?;
            let (_, _, sha, current_mime) = list
                .iter()
                .find(|r| r.1)
                .cloned()
                .ok_or_else(|| broken(format!("Bild {name}: keine aktuelle Fassung")))?;
            let data = content_bytes(content, *encoding)?;
            if data.len() > MAX_ASSET_BYTES {
                return Err(StoreError::Asset("Der neue Inhalt ist größer als 8 MiB.".into()));
            }
            let Some(found) = detect_mime(&data) else {
                return Err(StoreError::Asset(
                    "Der neue Inhalt ist kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG).".into(),
                ));
            };
            if *encoding == AssetEncoding::Svg && found != "image/svg+xml" {
                return Err(StoreError::Asset("encoding „svg“ verlangt SVG-Text.".into()));
            }
            if !mime.trim().is_empty() && mime.trim() != found {
                return Err(StoreError::Asset(format!("mime „{}“ passt nicht zum Inhalt ({found}).", mime.trim())));
            }
            if found != current_mime && !rename {
                return Err(StoreError::Asset(format!(
                    "Der neue Inhalt ist {found}, „{name}“ ist {current_mime}; ein anderes Format nur mit rename: true \
                     (das Bild bekommt beim Bestätigen die neue Endung und behält seine Pfade) oder als neues Bild."
                )));
            }
            if content_sha256(&data) == sha {
                return Err(StoreError::Refused("Das Bild ist unverändert."));
            }
            *mime = found.to_owned();
            *bytes = data.len() as u64;
            for t in [caption, alt].into_iter().flatten() {
                *t = t.replace("\r\n", "\n");
            }
            expect(expected_sha256, &sha, name)?;
        }
        AssetOp::Delete { name, rev, expected_sha256 } => {
            let list = revisions(conn, work, name).await?;
            let (_, current, sha, _) = match rev {
                None => list.iter().find(|r| r.1).cloned(),
                Some(rev) => list.iter().find(|r| r.0 == *rev).cloned(),
            }
            .ok_or_else(|| {
                let numbers: Vec<String> = list.iter().map(|r| r.0.to_string()).collect();
                StoreError::Asset(format!(
                    "Fassung {} von „{name}“ gibt es nicht; vorhanden: {}",
                    rev.unwrap_or(0),
                    numbers.join(", ")
                ))
            })?;
            if rev.is_some() && current {
                return Err(StoreError::Refused(
                    "Die aktuelle Fassung lässt sich nicht löschen; ohne rev wird das ganze Bild beantragt.",
                ));
            }
            expect(expected_sha256, &sha, name)?;
        }
        AssetOp::SetCover { name, .. } => {
            if let Some(name) = name {
                revisions(conn, work, name).await?;
            } else {
                work_exists(conn, work).await?;
            }
        }
        AssetOp::CoverLayout { blocks, expected_layout } => {
            if blocks.is_empty() {
                return Err(StoreError::Asset("blocks: mindestens ein Block der Anordnung erwartet.".into()));
            }
            *blocks = crate::cover_blocks::check_blocks(blocks)?;
            blocks.sort_by_key(|b| b.kind);
            let stored = crate::cover_blocks::read_layout(conn, work).await?;
            let now = Store::cover_layout_fingerprint(&stored);
            if expected_layout.trim().is_empty() {
                *expected_layout = now;
            } else if !expected_layout.trim().eq_ignore_ascii_case(&now) {
                return Err(StoreError::Asset(
                    "Die Umschlag-Anordnung hat sich inzwischen geändert (expected_layout passt nicht); bitte neu \
                     lesen (cover_layout)."
                        .into(),
                ));
            } else {
                *expected_layout = now;
            }
            if blocks.iter().all(|b| stored.get(b.kind) == Some(b)) {
                return Err(StoreError::Refused("Die Umschlag-Anordnung ist so schon gespeichert; nichts zu ändern."));
            }
        }
    }
    Ok(op)
}

/// SHA-256 (hex, lower case) as in `asset_revision.sha256`.
fn content_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

impl Store {
    /// Create a request concerning an image (atomic); origin persona or MCP client (the human → `Refused`). Checks what
    /// would not work even now (see the module documentation) and enters `bytes`, `mime` and `expected_sha256`; an
    /// identical open request → `Refused`. Changes nothing about the images. Result: the id of the request.
    pub async fn asset_request(
        &self,
        work: WorkId,
        op: &AssetOp,
        origin: &IdeaOrigin,
        note: &str,
    ) -> Result<i64, StoreError> {
        self.asset_request_with(work, op, origin, note, false).await
    }

    /// Like [`Store::asset_request`]; `rename` (only for `Replace`, Round 21c) allows a change of type, which on
    /// confirmation switches the name to the new extension.
    pub async fn asset_request_with(
        &self,
        work: WorkId,
        op: &AssetOp,
        origin: &IdeaOrigin,
        note: &str,
        rename: bool,
    ) -> Result<i64, StoreError> {
        let rename = rename && matches!(op, AssetOp::Replace { .. });
        let (kind, name) = match origin {
            IdeaOrigin::Persona { handle } => ("persona", handle),
            IdeaOrigin::Mcp { handle } => ("mcp", handle),
            IdeaOrigin::Human { .. } => {
                return Err(StoreError::Refused("Der Mensch ändert Bilder direkt; Anträge stellen nur Assistenten."));
            }
        };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let op = checked(&mut tx, work, op, rename).await?;
        let params = params_of(&op, rename)?;
        let twin: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM asset_request WHERE work_id = ? AND decided IS NULL AND op = ? AND asset_name = ? AND params = ?",
        )
        .bind(work)
        .bind(op.name())
        .bind(op.asset_name())
        .bind(&params)
        .fetch_optional(&mut *tx)
        .await?;
        if twin.is_some() {
            return Err(StoreError::Refused("Diesen Antrag gibt es schon (offen); der Mensch entscheidet darüber."));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO asset_request (work_id, op, asset_name, params, origin_kind, origin_name, note) \
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(op.name())
        .bind(op.asset_name())
        .bind(&params)
        .bind(kind)
        .bind(name)
        .bind(note.trim())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// A request (open or decided). Unknown → `UnknownRequest`.
    pub async fn asset_request_get(&self, work: WorkId, id: i64) -> Result<AssetRequest, StoreError> {
        let mut conn = self.pool.acquire().await?;
        fetch(&mut conn, work, id).await
    }

    /// Requests of a work by id; `open_only` only the open ones.
    pub async fn asset_requests(&self, work: WorkId, open_only: bool) -> Result<Vec<AssetRequest>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let filter = if open_only { " AND decided IS NULL" } else { "" };
        let rows: Vec<RequestRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM asset_request WHERE work_id = ?{filter} ORDER BY id"
        )))
        .bind(work)
        .fetch_all(&mut *conn)
        .await?;
        rows.into_iter().map(from_row).collect()
    }

    /// Remove the content of decided requests to replace whose decision lies more than `days` days back (Round 21c,
    /// default [`ASSET_REQUEST_PURGE_DAYS`]): `content` becomes empty, `purged` true, `bytes` keeps the size before;
    /// everything else (decision, origin, reasoning) stays. The server calls this when listing and deciding. Result:
    /// number of requests cleaned.
    pub async fn purge_decided_asset_requests(&self, work: WorkId, days: u32) -> Result<usize, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, params FROM asset_request WHERE work_id = ? AND op = 'replace' AND decided IS NOT NULL \
             AND decided < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?) ORDER BY id",
        )
        .bind(work)
        .bind(format!("-{days} days"))
        .fetch_all(&mut *tx)
        .await?;
        let mut purged = 0;
        for (id, params) in rows {
            let mut p: ReplaceParams =
                serde_json::from_str(&params).map_err(|e| broken(format!("Bildantrag {id}: params unlesbar: {e}")))?;
            if p.purged {
                continue;
            }
            p.bytes = Some(content_bytes(&p.content, p.encoding).map(|b| b.len() as u64).unwrap_or(0));
            p.content = String::new();
            p.purged = true;
            let text = serde_json::to_string(&p).map_err(|e| broken(format!("Bildantrag {id}: {e}")))?;
            sqlx::query("UPDATE asset_request SET params = ? WHERE id = ?")
                .bind(text)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            purged += 1;
        }
        tx.commit().await?;
        Ok(purged)
    }

    /// What `origin_name` (persona handle or `@mcp:<Client>`) has written in the work in the last 24 hours (Round 21c,
    /// quota of the MCP image tools), counted from existing rows: image versions with this author (`asset_add`;
    /// confirmed replacements are written by the human) and requests to replace (whatever the decision). Deleted images
    /// no longer count. In addition the open requests to replace.
    pub async fn asset_mcp_usage(&self, work: WorkId, origin_name: &str) -> Result<AssetUsage, StoreError> {
        let names = serde_json::to_string(&[origin_name]).map_err(|e| broken(e.to_string()))?;
        let own = |column: &str| format!("{column} IN (SELECT value FROM json_each(?))");
        self.asset_usage(work, &own("r.author_name"), &own("origin_name"), &names).await
    }

    /// Round 23: shared quota of **all MCP clients without a token** in a work. Counts like [`Store::asset_mcp_usage`],
    /// but across all requesters except the human (image versions whose author begins with `@` – personas and
    /// `@mcp:<Client>` –, and requests by personas or MCP clients) **without** the names in `token_handles` (handles
    /// and legacy handles of the tokens, also revoked ones: those have a quota of their own per token). That way a new
    /// `clientInfo.name` or another `persona` opens no new quota.
    pub async fn asset_mcp_usage_without_token(
        &self,
        work: WorkId,
        token_handles: &[String],
    ) -> Result<AssetUsage, StoreError> {
        let names = serde_json::to_string(token_handles).map_err(|e| broken(e.to_string()))?;
        let rest = |column: &str, all: &str| format!("{all} AND {column} NOT IN (SELECT value FROM json_each(?))");
        self.asset_usage(
            work,
            &rest("r.author_name", "r.author_name LIKE '@%'"),
            &rest("origin_name", "origin_kind IN ('persona', 'mcp')"),
            &names,
        )
        .await
    }

    /// Counts for the quota: `authors` restricts the image versions (alias `r`), `origins` the requests; both
    /// conditions bind exactly one parameter, the JSON list `names`.
    async fn asset_usage(
        &self,
        work: WorkId,
        authors: &str,
        origins: &str,
        names: &str,
    ) -> Result<AssetUsage, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let since = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day')";
        let (revisions, revision_bytes): (i64, i64) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT count(*), coalesce(sum(r.bytes), 0) FROM asset_revision r JOIN asset a ON a.id = r.asset_id \
             WHERE a.work_id = ? AND {authors} AND r.created >= {since}"
        )))
        .bind(work)
        .bind(names)
        .fetch_one(&mut *conn)
        .await?;
        // Size from `params.bytes` (always written since Round 21c); only older rows without `bytes` return the whole
        // content for recomputing.
        let requests: Vec<(Option<i64>, Option<String>)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT json_extract(params, '$.bytes'), CASE WHEN json_extract(params, '$.bytes') IS NULL THEN params END \
             FROM asset_request WHERE work_id = ? AND op = 'replace' AND {origins} AND created >= {since}"
        )))
        .bind(work)
        .bind(names)
        .fetch_all(&mut *conn)
        .await?;
        let mut bytes = u64::try_from(revision_bytes).unwrap_or(0);
        for (stored, params) in &requests {
            bytes += match (stored, params) {
                (Some(n), _) => u64::try_from(*n).unwrap_or(0),
                (None, Some(params)) => serde_json::from_str::<ReplaceParams>(params)
                    .ok()
                    .and_then(|p| content_bytes(&p.content, p.encoding).ok())
                    .map_or(0, |b| b.len() as u64),
                (None, None) => 0,
            };
        }
        let open: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM asset_request WHERE work_id = ? AND op = 'replace' AND {origins} AND decided IS NULL"
        )))
        .bind(work)
        .bind(names)
        .fetch_one(&mut *conn)
        .await?;
        Ok(AssetUsage {
            writes: crate::to_usize(revisions, "Bildfassungen")? + requests.len(),
            bytes,
            open_replace: crate::to_usize(open, "Bildanträge")?,
        })
    }

    /// Number of open requests concerning images (hints in the user interface).
    pub async fn open_asset_requests(&self, work: WorkId) -> Result<usize, StoreError> {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM asset_request WHERE work_id = ? AND decided IS NULL")
            .bind(work)
            .fetch_one(&self.pool)
            .await?;
        crate::to_usize(count, "Bildanträge")
    }

    /// Record the decision on an open request (only the human `human`; with `Confirmed` the server has already executed
    /// it). `message` is the reason for `Stale`. Already decided → `Refused`, unknown → `UnknownRequest`. Result: the
    /// request afterwards.
    pub async fn asset_request_decide(
        &self,
        work: WorkId,
        id: i64,
        decision: Decision,
        human: &str,
        message: Option<&str>,
    ) -> Result<AssetRequest, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let request = fetch(&mut tx, work, id).await?;
        if !request.is_open() {
            return Err(StoreError::Refused("Über diesen Antrag ist schon entschieden."));
        }
        sqlx::query(
            "UPDATE asset_request SET decided = strftime('%Y-%m-%dT%H:%M:%fZ','now'), decision = ?, decided_by = ?, \
             message = ? WHERE id = ? AND decided IS NULL",
        )
        .bind(decision_column(decision))
        .bind(human)
        .bind(message)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        let request = fetch(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_follow_the_contract_and_read_back_unchanged() {
        let op = AssetOp::Replace {
            name: "haus.svg".into(),
            mime: "image/svg+xml".into(),
            encoding: AssetEncoding::Svg,
            content: "<svg xmlns=\"http://www.w3.org/2000/svg\"/>".into(),
            bytes: 41,
            caption: Some("Das Haus".into()),
            alt: None,
            expected_sha256: "ab".into(),
        };
        let text = params_of(&op, false).unwrap();
        for key in ["\"mime\"", "\"encoding\"", "\"content\"", "\"caption\"", "\"expected_sha256\""] {
            assert!(text.contains(key), "params von replace tragen {key} (Vertrag §1): {text}");
        }
        assert!(!text.contains("\"alt\""), "ohne alt kein Schlüssel alt: {text}");
        assert_eq!(op_of(1, "replace", "haus.svg".into(), &text).unwrap(), op, "verlustfrei zurückgelesen");
        let cover = AssetOp::SetCover { name: None, expected_cover: Some("alt.png".into()) };
        let text = params_of(&cover, false).unwrap();
        assert_eq!(text, r#"{"expected_cover":"alt.png"}"#, "set_cover trägt nur den Umschlag beim Antrag");
        assert_eq!(op_of(2, "set_cover", String::new(), &text).unwrap(), cover, "leerer Name = Umschlag entfernen");
    }

    #[test]
    fn answers_leave_out_the_content_but_name_the_size() {
        let op = AssetOp::Replace {
            name: "a.png".into(),
            mime: "image/png".into(),
            encoding: AssetEncoding::Base64,
            content: "AAAA".into(),
            bytes: 3,
            caption: None,
            alt: None,
            expected_sha256: "00".into(),
        };
        let json = serde_json::to_value(&op).unwrap();
        assert!(json.get("content").is_none(), "der Inhalt (bis 8 MiB) steht nie in Antworten: {json}");
        assert_eq!(json["bytes"], 3, "statt dessen die Größe");
        assert_eq!(json["op"], "replace", "die Art steht als op");
    }

    #[test]
    fn summaries_say_in_german_what_is_wanted() {
        let delete = AssetOp::Delete { name: "a.png".into(), rev: Some(2), expected_sha256: String::new() };
        assert_eq!(summary(&delete), "Fassung 2 von „a.png“ löschen", "eine Fassung");
        let all = AssetOp::Delete { name: "a.png".into(), rev: None, expected_sha256: String::new() };
        assert_eq!(summary(&all), "Bild „a.png“ mit allen Fassungen löschen", "das ganze Bild");
        let cover = AssetOp::SetCover { name: Some("u.png".into()), expected_cover: None };
        assert_eq!(summary(&cover), "„u.png“ als Umschlag setzen", "Umschlag");
        let off = AssetOp::SetCover { name: None, expected_cover: Some("u.png".into()) };
        assert_eq!(summary(&off), "Umschlag entfernen", "Umschlag weg");
    }
}
