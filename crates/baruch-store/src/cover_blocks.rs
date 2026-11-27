// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cover layout per work (migration `0045_cover_block.sql`, contract Round 27, task umschlag-daten).
//!
//! - Per work and kind ([`CoverBlockKind`]) at most one row in `cover_block`; without a row the default block applies
//!   (`baruch_typst::cover_layout::default_blocks`). No row in the whole work = no layout of its own.
//! - [`check_cover_block`] checks a block against the limits from `baruch_typst::cover_layout` (equal to the CHECKs of
//!   the migration) and brings it into normal form; [`Store::set_cover_blocks`] replaces the whole block per named
//!   kind, [`Store::reset_cover_blocks`] deletes the rows of the named faces (tombstones) and on request sets the title
//!   area in the same transaction.
//! - A changed block keeps its row and with it its uuid (`INSERT … ON CONFLICT (work_id, kind) DO UPDATE`); only a
//!   reset deletes rows and leaves tombstones `cover_block` (exchange format: `umschlag-block`).
//! - Normal form: colour in lower case (`light`, `dark` or `#rrggbb`), font trimmed (empty = `None` = font of the work
//!   template), spine without a reading direction → `down`, custom text with `\n` instead of `\r\n` (kinds with custom
//!   text without a text → `""`, other kinds with an empty text → `None`).
//! - Violation → [`StoreError::CoverBlock`] (RPC -32602) with a German message that names kind and field, for example
//!   „Umschlag-Block „Titel (Vorderseite)“ (front_title), Feld x_mm: …“; nothing changed.
//! - Fingerprint of a layout ([`Store::cover_layout_fingerprint`]): SHA-256 (hex, lower case) over the JSON of the
//!   blocks in [`CoverBlockKind::ALL`] order. A request (`asset_request` op `cover_layout`) remembers it as
//!   `expected_layout`; if it no longer matches on confirmation, the request becomes `stale`.
//! - Only the human changes it (or the server when confirming a request); assistants file requests through
//!   `asset_request` (op `cover_layout`).

use serde::Serialize;
use sqlx::SqliteConnection;

use baruch_typst::cover_layout::{
    Backdrop, CoverAlign, FontStyle, LEADING_EM, MAX_FONT_CHARS, MAX_TEXT_CHARS, POS_MM, SIZE_PT, SpineDirection,
    WIDTH_MM,
};

use crate::{CoverBlock, CoverBlockKind, CoverFace, CoverLayout, Store, StoreError, TitleArea, WorkId, broken};

/// Result of [`Store::set_cover_blocks`] and [`Store::reset_cover_blocks`]: changed kinds and the stored layout before
/// and after (for the mirror commit).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoverLayoutChange {
    pub changed: Vec<CoverBlockKind>,
    pub before: CoverLayout,
    pub after: CoverLayout,
}

/// „Titel (Vorderseite)“.
fn kind_label(kind: CoverBlockKind) -> String {
    format!("{} ({})", kind.label(), kind.face().label())
}

fn field_error(kind: CoverBlockKind, field: &str, message: impl std::fmt::Display) -> StoreError {
    StoreError::CoverBlock(format!(
        "Umschlag-Block „{}“ ({}), Feld {field}: {message}",
        kind_label(kind),
        kind.as_str()
    ))
}

/// Number finite and in `[range.0, range.1]`.
fn check_number(
    kind: CoverBlockKind,
    field: &str,
    value: f64,
    range: (f64, f64),
    unit: &str,
) -> Result<(), StoreError> {
    if value.is_finite() && value >= range.0 && value <= range.1 {
        Ok(())
    } else {
        let shown = |v: f64| v.to_string().replace('.', ",");
        Err(field_error(
            kind,
            field,
            format!("erlaubt {} bis {} {unit}, nicht {}.", shown(range.0), shown(range.1), shown(value)),
        ))
    }
}

/// A family name after the rule of the typeset view fonts (`sheet::check_font`): letters, digits, spaces, `-`, `_`,
/// `.`; not empty. That way no character can leave a Typst string, a CSS rule or an HTML attribute (no `;`, `{`, `}`,
/// `<`, `>`, `\`, `"`, no control characters).
fn family_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().all(|c| c.is_alphanumeric() || " -_.".contains(c))
}

/// Colour: `light`, `dark` or `#rrggbb` (also upper case) → written in lower case.
fn check_color(kind: CoverBlockKind, value: &str) -> Result<String, StoreError> {
    let lower = value.trim().to_ascii_lowercase();
    let hex = lower.strip_prefix('#').filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()));
    if lower == "light" || lower == "dark" || hex.is_some() {
        Ok(lower)
    } else {
        Err(field_error(kind, "color", format!("„light“, „dark“ oder #rrggbb erwartet, nicht „{}“.", value.trim())))
    }
}

/// Check a block and bring it into normal form; violation → [`StoreError::CoverBlock`] with a German message that names
/// kind and field (limits from `baruch_typst::cover_layout`, equal to the CHECKs of migration 0045).
pub fn check_cover_block(block: &CoverBlock) -> Result<CoverBlock, StoreError> {
    let kind = block.kind;
    let mut out = block.clone();
    check_number(kind, "x_mm", block.x_mm, POS_MM, "mm")?;
    check_number(kind, "y_mm", block.y_mm, POS_MM, "mm")?;
    check_number(kind, "width_mm", block.width_mm, WIDTH_MM, "mm")?;
    check_number(kind, "size_pt", block.size_pt, SIZE_PT, "pt")?;
    check_number(kind, "leading_em", block.leading_em, LEADING_EM, "em")?;
    out.color = check_color(kind, &block.color)?;
    out.font = match block.font.as_deref().map(str::trim).filter(|f| !f.is_empty()) {
        None => None,
        Some(font) if font.chars().count() > MAX_FONT_CHARS => {
            return Err(field_error(kind, "font", format!("höchstens {MAX_FONT_CHARS} Zeichen.")));
        }
        Some(font) if !family_name(font) => {
            return Err(field_error(
                kind,
                "font",
                "ein Schriftname aus Buchstaben, Ziffern, Leerzeichen, - _ . (ohne ; { } < > \\ und Steuerzeichen).",
            ));
        }
        Some(font) => Some(font.to_owned()),
    };
    out.direction = match (kind.face() == CoverFace::Spine, block.direction) {
        (true, None) => Some(SpineDirection::Down),
        (true, direction) => direction,
        (false, None) => None,
        (false, Some(_)) => {
            return Err(field_error(kind, "direction", "eine Laufrichtung gibt es nur auf dem Rücken."));
        }
    };
    out.text = match (kind.own_text(), block.text.as_deref()) {
        (true, None) => Some(String::new()),
        (true, Some(text)) => {
            let text = text.replace("\r\n", "\n");
            let chars = text.chars().count();
            if chars > MAX_TEXT_CHARS {
                return Err(field_error(kind, "text", format!("höchstens {MAX_TEXT_CHARS} Zeichen (jetzt {chars}).")));
            }
            if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
                return Err(field_error(kind, "text", "keine Steuerzeichen außer Zeilenumbruch und Tab."));
            }
            Some(text)
        }
        (false, None) => None,
        (false, Some(text)) if text.trim().is_empty() => None,
        (false, Some(_)) => {
            return Err(field_error(
                kind,
                "text",
                "eigenen Text haben nur die Freitexte und das Motto; dieser Block nimmt seinen Text aus Werk und \
                 Buchangaben.",
            ));
        }
    };
    Ok(out)
}

type BlockRow = (
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
);

const COLUMNS: &str =
    "kind, x_mm, y_mm, width_mm, align, size_pt, leading_em, color, font, style, backdrop, visible, direction, text";

fn block_of(row: BlockRow) -> Result<CoverBlock, StoreError> {
    let (
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
        visible,
        direction,
        text,
    ) = row;
    let bad = |what: &str, value: &str| broken(format!("cover_block: unbekannte {what} {value:?}"));
    Ok(CoverBlock {
        kind: CoverBlockKind::parse(&kind).ok_or_else(|| bad("Art", &kind))?,
        x_mm,
        y_mm,
        width_mm,
        align: CoverAlign::parse(&align).ok_or_else(|| bad("Ausrichtung", &align))?,
        size_pt,
        leading_em,
        color,
        font,
        style: FontStyle::parse(&style).ok_or_else(|| bad("Schnitt", &style))?,
        backdrop: Backdrop::parse(&backdrop).ok_or_else(|| bad("Hinterlegung", &backdrop))?,
        visible: visible != 0,
        direction: direction.map(|d| SpineDirection::parse(&d).ok_or_else(|| bad("Laufrichtung", &d))).transpose()?,
        text,
    })
}

async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Stored blocks of the work in [`CoverBlockKind::ALL`] order (the work must exist).
pub(crate) async fn read_layout(conn: &mut SqliteConnection, work: WorkId) -> Result<CoverLayout, StoreError> {
    work_exists(conn, work).await?;
    let rows: Vec<BlockRow> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM cover_block WHERE work_id = ?")))
            .bind(work)
            .fetch_all(&mut *conn)
            .await?;
    let mut blocks = rows.into_iter().map(block_of).collect::<Result<Vec<_>, _>>()?;
    blocks.sort_by_key(|b| b.kind);
    Ok(CoverLayout { blocks })
}

/// Write a block (create it or replace the row of the kind; the uuid of the row stays).
async fn upsert(conn: &mut SqliteConnection, work: WorkId, block: &CoverBlock) -> Result<(), StoreError> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO cover_block (work_id, {COLUMNS}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (work_id, kind) DO UPDATE SET x_mm = excluded.x_mm, y_mm = excluded.y_mm, \
         width_mm = excluded.width_mm, align = excluded.align, size_pt = excluded.size_pt, \
         leading_em = excluded.leading_em, color = excluded.color, font = excluded.font, style = excluded.style, \
         backdrop = excluded.backdrop, visible = excluded.visible, direction = excluded.direction, \
         text = excluded.text, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now')"
    )))
    .bind(work)
    .bind(block.kind.as_str())
    .bind(block.x_mm)
    .bind(block.y_mm)
    .bind(block.width_mm)
    .bind(block.align.as_str())
    .bind(block.size_pt)
    .bind(block.leading_em)
    .bind(&block.color)
    .bind(&block.font)
    .bind(block.style.as_str())
    .bind(block.backdrop.as_str())
    .bind(i64::from(block.visible))
    .bind(block.direction.map(SpineDirection::as_str))
    .bind(&block.text)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Check all blocks (normal form) and reject duplicate kinds.
pub(crate) fn check_blocks(blocks: &[CoverBlock]) -> Result<Vec<CoverBlock>, StoreError> {
    let mut out: Vec<CoverBlock> = Vec::with_capacity(blocks.len());
    for block in blocks {
        if out.iter().any(|b| b.kind == block.kind) {
            return Err(field_error(block.kind, "kind", "dieselbe Art steht zweimal in der Liste."));
        }
        out.push(check_cover_block(block)?);
    }
    Ok(out)
}

/// Fingerprint of a layout (see the module documentation).
fn fingerprint(layout: &CoverLayout) -> String {
    use sha2::{Digest, Sha256};
    let mut blocks: Vec<&CoverBlock> = layout.blocks.iter().collect();
    blocks.sort_by_key(|b| b.kind);
    let json = serde_json::to_string(&blocks).unwrap_or_default();
    Sha256::digest(json.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

impl Store {
    /// Stored layout of the work (blocks in [`CoverBlockKind::ALL`] order); empty without rows. Unknown work →
    /// `UnknownWork`.
    pub async fn cover_layout(&self, work: WorkId) -> Result<CoverLayout, StoreError> {
        let mut conn = self.pool.acquire().await?;
        read_layout(&mut conn, work).await
    }

    /// Fingerprint of a layout: SHA-256 (hex, lower case) over the JSON of the blocks in [`CoverBlockKind::ALL`] order
    /// (`expected_layout` of the requests).
    pub fn cover_layout_fingerprint(layout: &CoverLayout) -> String {
        fingerprint(layout)
    }

    /// Every named block replaces its kind completely, unnamed kinds stay (atomic, only the human or a confirmed
    /// request). Invalid → `Asset` with kind and field, nothing changed. Unchanged blocks do not count as a change
    /// (`changed` empty if nothing differs).
    pub async fn set_cover_blocks(&self, work: WorkId, blocks: &[CoverBlock]) -> Result<CoverLayoutChange, StoreError> {
        let blocks = check_blocks(blocks)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = read_layout(&mut tx, work).await?;
        let mut changed = Vec::new();
        for block in &blocks {
            if before.get(block.kind) != Some(block) {
                upsert(&mut tx, work, block).await?;
                changed.push(block.kind);
            }
        }
        if changed.is_empty() {
            return Ok(CoverLayoutChange { changed, after: before.clone(), before });
        }
        changed.sort();
        let after = read_layout(&mut tx, work).await?;
        tx.commit().await?;
        Ok(CoverLayoutChange { changed, before, after })
    }

    /// Delete the rows of the faces `faces` (empty = all faces; tombstones `cover_block`) and set the title area with
    /// `area`, in one transaction. `changed` names the deleted kinds.
    pub async fn reset_cover_blocks(
        &self,
        work: WorkId,
        faces: &[CoverFace],
        area: Option<TitleArea>,
    ) -> Result<CoverLayoutChange, StoreError> {
        let faces: Vec<CoverFace> = if faces.is_empty() { CoverFace::ALL.to_vec() } else { faces.to_vec() };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = read_layout(&mut tx, work).await?;
        let changed: Vec<CoverBlockKind> =
            before.blocks.iter().map(|b| b.kind).filter(|k| faces.contains(&k.face())).collect();
        for kind in &changed {
            sqlx::query("DELETE FROM cover_block WHERE work_id = ? AND kind = ?")
                .bind(work)
                .bind(kind.as_str())
                .execute(&mut *tx)
                .await?;
        }
        if let Some(area) = area {
            let meta = crate::book::read(&mut tx, work).await?;
            if meta.title_area != area {
                let mut meta = meta;
                meta.title_area = area;
                crate::book::write(&mut tx, work, &meta).await?;
            }
        }
        let after = read_layout(&mut tx, work).await?;
        tx.commit().await?;
        Ok(CoverLayoutChange { changed, before, after })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(kind: CoverBlockKind) -> CoverBlock {
        CoverBlock {
            kind,
            x_mm: 10.0,
            y_mm: 20.0,
            width_mm: 100.0,
            align: CoverAlign::Center,
            size_pt: 24.0,
            leading_em: 0.65,
            color: "light".into(),
            font: None,
            style: FontStyle::Normal,
            backdrop: Backdrop::None,
            visible: true,
            direction: None,
            text: None,
        }
    }

    #[test]
    fn normal_form_fills_direction_and_text_and_lowers_the_color() {
        let mut spine = block(CoverBlockKind::SpineFree);
        spine.color = " #A1B2C3 ".into();
        spine.font = Some("  ".into());
        spine.text = Some("Zeile 1\r\nZeile 2".into());
        let checked = check_cover_block(&spine).unwrap();
        assert_eq!(checked.direction, Some(SpineDirection::Down), "Rücken ohne Laufrichtung läuft wie bisher");
        assert_eq!(checked.color, "#a1b2c3", "Farbe klein, ohne Leerraum (CHECK verlangt Länge 7)");
        assert_eq!(checked.font, None, "leere Schrift = Schrift der Werkvorlage");
        assert_eq!(checked.text.as_deref(), Some("Zeile 1\nZeile 2"), "CRLF wird LF");
        let mut title = block(CoverBlockKind::FrontTitle);
        title.text = Some("  ".into());
        assert_eq!(check_cover_block(&title).unwrap().text, None, "leerer Text bei Art ohne eigenen Text fällt weg");
        let motto = check_cover_block(&block(CoverBlockKind::BackMotto)).unwrap();
        assert_eq!(motto.text.as_deref(), Some(""), "Motto ohne Text bekommt den leeren eigenen Text");
    }

    #[test]
    fn violations_name_kind_and_field() {
        let mut b = block(CoverBlockKind::FrontTitle);
        b.x_mm = 1000.5;
        let message = check_cover_block(&b).unwrap_err().to_string();
        assert!(message.contains("Titel (Vorderseite)") && message.contains("front_title"), "Art: {message}");
        assert!(message.contains("x_mm") && message.contains("1000,5"), "Feld und Wert: {message}");
        b.x_mm = f64::NAN;
        assert!(check_cover_block(&b).is_err(), "NaN liegt in keinem Bereich");
        let mut b = block(CoverBlockKind::BackBlurb);
        b.direction = Some(SpineDirection::Up);
        assert!(check_cover_block(&b).unwrap_err().to_string().contains("direction"), "Laufrichtung nur am Rücken");
        for font in ["A;B", "A{", "A}", "<b>", "A\\B", "A\u{7}", "\"A\""] {
            let mut b = block(CoverBlockKind::FrontAuthor);
            b.font = Some(font.into());
            let message = check_cover_block(&b).unwrap_err().to_string();
            assert!(message.contains("font"), "Schriftname {font:?} abgelehnt: {message}");
        }
        let mut b = block(CoverBlockKind::FrontAuthor);
        b.font = Some("Libertinus Serif-2_a.b".into());
        assert!(check_cover_block(&b).is_ok(), "Buchstaben, Ziffern, Leerzeichen, - _ . gehen");
        for color in ["red", "#abc", "#12345g", "lightish"] {
            let mut b = block(CoverBlockKind::FrontAuthor);
            b.color = color.into();
            assert!(check_cover_block(&b).unwrap_err().to_string().contains("color"), "Farbe {color:?}");
        }
    }

    #[test]
    fn the_fingerprint_ignores_order_but_not_values() {
        let a = block(CoverBlockKind::FrontTitle);
        let b = block(CoverBlockKind::FrontAuthor);
        let one = CoverLayout { blocks: vec![a.clone(), b.clone()] };
        let two = CoverLayout { blocks: vec![b.clone(), a.clone()] };
        assert_eq!(fingerprint(&one), fingerprint(&two), "Reihenfolge der Liste zählt nicht");
        let mut moved = a;
        moved.x_mm += 0.5;
        let three = CoverLayout { blocks: vec![moved, b] };
        assert_ne!(fingerprint(&one), fingerprint(&three), "ein verschobener Block ändert den Fingerabdruck");
        assert_eq!(fingerprint(&CoverLayout::default()).len(), 64, "SHA-256 als Hex");
    }
}
