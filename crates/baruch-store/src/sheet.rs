// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typeset view settings per work (Round 18c, migration `0044_work_sheet.sql`).
//!
//! - The settings override the presentation of the HTML typeset view derived from the work template, never Typst,
//!   export or PDF.
//! - `work_sheet`: at most one row per work; `NULL` = from the template. Without a row everything comes from the
//!   template ([`SheetSettings::default`]). A deleted work takes its row along (`ON DELETE CASCADE`).
//! - Not book text, no commit in the mirror (like `work_timing`), no uuid, not in the exchange format.
//! - Check before writing ([`SheetSettings::normalized`]): font lists at most [`MAX_SHEET_FONT_CHARS`] characters,
//!   families separated by commas, each a name of letters, digits, spaces, `-`, `_`, `.` (also in `"…"` or `'…'`) – so
//!   never control characters, `<`, `>`, `;`, `{`, `}` or `\`, nothing that could leave a CSS rule or the `<style>`
//!   element. Lengths as a number with the unit `pt`, `mm`, `cm` or `em` within limits (font size 6–48 pt, text width
//!   6–60 cm; `em` counts as 12 pt), colours `#rrggbb` (stored in lower case).

use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId};

/// Longest font list (characters) for `font`/`heading_font`.
pub const MAX_SHEET_FONT_CHARS: usize = 200;

/// Typeset view settings of a work (Round 18c, migration 0044 `work_sheet`): they override the presentation of the
/// typeset view derived from the work template (HTML typeset view only, never Typst/export/PDF). `None` = from the
/// template. Lengths as a CSS length with the unit pt|mm|cm|em (for example "11pt", "16cm"), colours as `#rrggbb`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SheetSettings {
    pub font: Option<String>,
    pub heading_font: Option<String>,
    pub size: Option<String>,
    pub fill: Option<String>,
    pub heading_fill: Option<String>,
    pub background: Option<String>,
    pub width: Option<String>,
}

/// Limits of the font size in pt.
const SIZE_PT: (f64, f64) = (6.0, 48.0);
/// Limits of the text width in pt (6 cm to 60 cm).
const WIDTH_PT: (f64, f64) = (6.0 * 72.0 / 2.54, 60.0 * 72.0 / 2.54);

impl SheetSettings {
    /// All fields `None`.
    pub fn is_empty(&self) -> bool {
        *self == SheetSettings::default()
    }

    /// Checked, normalised version (outer whitespace removed, empty fields `None`, colours lower case, unit lower
    /// case). Invalid values → `Refused` with a German message that names the field.
    pub fn normalized(&self) -> Result<SheetSettings, StoreError> {
        let text = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
        let font = |v: &Option<String>, message: &'static str| -> Result<Option<String>, StoreError> {
            text(v).map(|v| check_font(&v).then_some(v).ok_or(StoreError::Refused(message))).transpose()
        };
        let color = |v: &Option<String>, message: &'static str| -> Result<Option<String>, StoreError> {
            text(v).map(|v| check_color(&v).ok_or(StoreError::Refused(message))).transpose()
        };
        let length = |v: &Option<String>, range: (f64, f64), message: &'static str| {
            text(v).map(|v| check_length(&v, range).ok_or(StoreError::Refused(message))).transpose()
        };
        Ok(SheetSettings {
            font: font(
                &self.font,
                "Satzbild: Schrift – höchstens 200 Zeichen, Namen aus Buchstaben, Ziffern, Leerzeichen, - _ . \
                 (auch in Anführungszeichen), durch Kommas getrennt.",
            )?,
            heading_font: font(
                &self.heading_font,
                "Satzbild: Schrift der Überschriften – höchstens 200 Zeichen, Namen aus Buchstaben, Ziffern, \
                 Leerzeichen, - _ . (auch in Anführungszeichen), durch Kommas getrennt.",
            )?,
            size: length(
                &self.size,
                SIZE_PT,
                "Satzbild: Schriftgröße – eine Zahl mit Einheit pt, mm, cm oder em, erlaubt 6 bis 48 pt (z. B. 11pt).",
            )?,
            fill: color(&self.fill, "Satzbild: Textfarbe – als #rrggbb (z. B. #222222).")?,
            heading_fill: color(
                &self.heading_fill,
                "Satzbild: Farbe der Überschriften – als #rrggbb (z. B. #7a1f1f).",
            )?,
            background: color(&self.background, "Satzbild: Blattfarbe – als #rrggbb (z. B. #fbf8f1).")?,
            width: length(
                &self.width,
                WIDTH_PT,
                "Satzbild: Satzbreite – eine Zahl mit Einheit pt, mm, cm oder em, erlaubt 6 bis 60 cm (z. B. 16cm).",
            )?,
        })
    }
}

/// A family name: letters, digits, spaces, `-`, `_`, `.`; not empty.
fn family_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().all(|c| c.is_alphanumeric() || " -_.".contains(c))
}

/// Font list: at most [`MAX_SHEET_FONT_CHARS`] characters, families separated by commas, each a name ([`family_name`])
/// or a name in `"…"` or `'…'`. That way quotation marks stay paired, and no character can leave the CSS rule.
fn check_font(list: &str) -> bool {
    if list.chars().count() > MAX_SHEET_FONT_CHARS {
        return false;
    }
    list.split(',').all(|family| {
        let family = family.trim();
        let inner = ['"', '\'']
            .iter()
            .find_map(|q| family.strip_prefix(*q).and_then(|f| f.strip_suffix(*q)))
            .filter(|_| family.len() >= 2);
        match inner {
            Some(inner) => family_name(inner),
            None => !family.contains(['"', '\'']) && family_name(family),
        }
    })
}

/// `#rrggbb` (also upper case) → written in lower case.
fn check_color(value: &str) -> Option<String> {
    let hex = value.strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("#{}", hex.to_ascii_lowercase()))
}

/// Length `<number><unit>` with the unit pt|mm|cm|em (number: digits, at most one dot with up to three decimal places),
/// converted to pt within `range` (`em` = 12 pt). Result without whitespace, unit in lower case.
fn check_length(value: &str, range: (f64, f64)) -> Option<String> {
    let lower = value.trim().to_ascii_lowercase();
    let (unit, factor) = [("pt", 1.0), ("mm", 72.0 / 25.4), ("cm", 72.0 / 2.54), ("em", 12.0)]
        .into_iter()
        .find(|(u, _)| lower.ends_with(u))?;
    let number = lower[..lower.len() - unit.len()].trim_end();
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if whole.is_empty() || whole.len() > 4 || !digits(whole) || !digits(fraction) || fraction.len() > 3 {
        return None;
    }
    if number.ends_with('.') {
        return None;
    }
    let pt = number.parse::<f64>().ok()? * factor;
    // Small tolerance for rounding (for example 60cm exactly at the limit).
    (pt >= range.0 - 1e-6 && pt <= range.1 + 1e-6).then(|| format!("{number}{unit}"))
}

async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

type Row =
    (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>);

impl Store {
    /// Settings of the work; without a row `SheetSettings::default()`. Unknown work → `UnknownWork`.
    pub async fn sheet_settings(&self, work: WorkId) -> Result<SheetSettings, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let row: Option<Row> = sqlx::query_as(
            "SELECT font, heading_font, size, fill, heading_fill, background, width FROM work_sheet WHERE work_id = ?",
        )
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row
            .map(|(font, heading_font, size, fill, heading_fill, background, width)| SheetSettings {
                font,
                heading_font,
                size,
                fill,
                heading_fill,
                background,
                width,
            })
            .unwrap_or_default())
    }

    /// Set (whole row; everything `None` = delete the row). Invalid values → `Refused` with a German message, nothing
    /// changed; unknown work → `UnknownWork`. No mirror commit (like `work_timing`).
    pub async fn set_sheet_settings(&self, work: WorkId, settings: &SheetSettings) -> Result<(), StoreError> {
        let settings = settings.normalized()?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        work_exists(&mut tx, work).await?;
        if settings.is_empty() {
            sqlx::query("DELETE FROM work_sheet WHERE work_id = ?").bind(work).execute(&mut *tx).await?;
        } else {
            sqlx::query(
                "INSERT INTO work_sheet (work_id, font, heading_font, size, fill, heading_fill, background, width) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(work_id) DO UPDATE SET font = excluded.font, \
                 heading_font = excluded.heading_font, size = excluded.size, fill = excluded.fill, \
                 heading_fill = excluded.heading_fill, background = excluded.background, width = excluded.width, \
                 updated = strftime('%Y-%m-%dT%H:%M:%SZ','now')",
            )
            .bind(work)
            .bind(&settings.font)
            .bind(&settings.heading_font)
            .bind(&settings.size)
            .bind(&settings.fill)
            .bind(&settings.heading_fill)
            .bind(&settings.background)
            .bind(&settings.width)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_lengths_and_colors_are_checked_before_writing() {
        assert!(check_font("\"Erfundene Antiqua\", Georgia, serif"), "Liste mit Anführungszeichen und Namen");
        assert!(check_font("'Probe Grotesk', sans-serif"), "einfache Anführungszeichen");
        assert!(!check_font("\"Offen, serif"), "unpaarige Anführungszeichen ließen die CSS-Zeichenkette offen");
        assert!(!check_font("Serif; } body { color: red"), "Semikolon und Klammern verlassen die Regel");
        assert!(!check_font("A</style><script>"), "spitze Klammern verlassen das style-Element");
        assert!(!check_font("A\u{7}B"), "Steuerzeichen");
        assert!(!check_font("A,,B"), "leere Familie zwischen zwei Kommas");
        assert!(!check_font(&"a".repeat(MAX_SHEET_FONT_CHARS + 1)), "mehr als 200 Zeichen");
        assert!(check_font(&"a".repeat(MAX_SHEET_FONT_CHARS)), "genau 200 Zeichen gehen");
        assert_eq!(check_length("11PT", SIZE_PT).as_deref(), Some("11pt"), "Einheit klein");
        assert_eq!(check_length("0.9em", SIZE_PT).as_deref(), Some("0.9em"), "em zählt als 12 pt");
        assert_eq!(check_length("5pt", SIZE_PT), None, "unter 6 pt");
        assert_eq!(check_length("60cm", WIDTH_PT).as_deref(), Some("60cm"), "genau an der Grenze");
        assert_eq!(check_length("61cm", WIDTH_PT), None, "über 60 cm");
        assert_eq!(check_length("16", WIDTH_PT), None, "ohne Einheit");
        assert_eq!(check_length("1e2pt", SIZE_PT), None, "keine Exponenten");
        assert_eq!(check_length("-12pt", SIZE_PT), None, "keine Vorzeichen");
        assert_eq!(check_length("12.pt", SIZE_PT), None, "kein Punkt ohne Nachkommastelle");
        assert_eq!(check_length("1in", SIZE_PT), None, "nur pt, mm, cm, em");
        assert_eq!(check_color("#A1B2C3").as_deref(), Some("#a1b2c3"), "Farbe klein geschrieben");
        assert_eq!(check_color("#abc"), None, "nur #rrggbb");
        assert_eq!(check_color("red"), None, "keine Farbnamen");
    }
}
