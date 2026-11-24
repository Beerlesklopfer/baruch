// SPDX-License-Identifier: AGPL-3.0-or-later
//! Buchangaben und Umschlag je Werk (Migration `0017_book.sql`, Vertrag Runde 11, Aufgabe cover).
//!
//! - Eine Zeile je Werk in `work_book`; keine Zeile = Vorgaben ([`BookMeta::default`]): kein Umschlag,
//!   Titel oben, ohne Titelei, Umschlagseiten im Satz an, 0,1 mm Papier je Seite, 3 mm Beschnitt.
//! - Umschlag und Autorenfoto sind Bilder **desselben** Werks (Trigger); nach außen über ihren
//!   Bildnamen. Wird das Bild gelöscht, ist der Verweis weg (`ON DELETE SET NULL`).
//! - Grenzen ([`Store::set_book_meta`]): einzeilige Felder höchstens 200 Zeichen ohne Zeilenumbruch;
//!   Klappentext 4000, Kurzbiografie 2000, Widmung, Rechtehinweis und Bildnachweis je 1000 Zeichen
//!   (mehrzeilig); Anschrift des Verlags und der Druckerei je 500 Zeichen (mehrzeilig, Runde 27); Jahr leer oder vier Ziffern; ISBN leer oder eine gültige ISBN-13 (Prüfziffer,
//!   [`baruch_typst::cover::isbn13_check`], Präfix 978/979 seit Runde 20c), gespeichert wie eingegeben (eine nur
//!   anders geschriebene ISBN ist eine Änderung); Papierdicke je Seite über 0 bis 1 mm (Oberfläche und RPC rechnen seit
//!   Runde 20c auch je Blatt = 2 Seiten, gespeichert bleibt die Dicke je Seite, keine Migration),
//!   Beschnitt 0 bis 10 mm. Verstoß → [`StoreError::Refused`] mit deutscher Meldung, nichts geändert.
//!   Unbekanntes Bild → [`StoreError::UnknownAsset`].
//! - Untertitel, Autor und Bibelübersetzung stehen weiter in `work` ([`Store::update_work`]).

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection};

use crate::{Store, StoreError, WorkId};

/// Wo Titel und Untertitel auf der Vorderseite stehen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TitleArea {
    #[default]
    Top,
    Middle,
    Bottom,
}

impl TitleArea {
    /// `top`, `middle`, `bottom` (Datenbank, RPC, Formulare).
    pub fn as_str(self) -> &'static str {
        match self {
            TitleArea::Top => "top",
            TitleArea::Middle => "middle",
            TitleArea::Bottom => "bottom",
        }
    }

    pub fn parse(s: &str) -> Option<TitleArea> {
        match s.trim() {
            "top" => Some(TitleArea::Top),
            "middle" => Some(TitleArea::Middle),
            "bottom" => Some(TitleArea::Bottom),
            _ => None,
        }
    }
}

/// Buchangaben eines Werks (Vorgaben, wenn es keine Zeile gibt).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookMeta {
    pub publisher: String,
    pub place: String,
    pub year: String,
    pub edition: String,
    pub isbn: String,
    pub price: String,
    pub series: String,
    pub printer: String,
    pub bible_notice: String,
    pub image_credit: String,
    pub dedication: String,
    pub blurb: String,
    pub bio: String,
    /// Bildname des Autorenfotos.
    pub author_photo: Option<String>,
    /// Bildname des Umschlags.
    pub cover: Option<String>,
    pub title_area: TitleArea,
    /// Titelei (Schmutztitel, Seite 2, Haupttitel, Impressum, Widmung).
    pub front_matter: bool,
    /// Vorder- und Rückseite des Umschlags im Satz (nur wirksam mit Umschlag).
    pub cover_pages: bool,
    /// Papierdicke in mm je Seite.
    pub paper_mm: f64,
    /// Beschnitt in mm.
    pub bleed_mm: f64,
    /// Letzte Änderung der Buchangaben bzw. des Umschlags.
    pub updated: Option<String>,
    /// Anschrift des Verlags bzw. bei Selbstverlag des Verfassers oder Herausgebers (Runde 27, mehrzeilig).
    pub publisher_address: String,
    /// Anschrift der Druckerei (Runde 27, mehrzeilig).
    pub printer_address: String,
}

impl Default for BookMeta {
    fn default() -> Self {
        BookMeta {
            publisher: String::new(),
            place: String::new(),
            year: String::new(),
            edition: String::new(),
            isbn: String::new(),
            price: String::new(),
            series: String::new(),
            printer: String::new(),
            bible_notice: String::new(),
            image_credit: String::new(),
            dedication: String::new(),
            blurb: String::new(),
            bio: String::new(),
            author_photo: None,
            cover: None,
            title_area: TitleArea::Top,
            front_matter: false,
            cover_pages: true,
            paper_mm: 0.1,
            bleed_mm: 3.0,
            updated: None,
            publisher_address: String::new(),
            printer_address: String::new(),
        }
    }
}

/// Änderung der Buchangaben: jedes Feld `None` = bleibt. `author_photo: Some(None)` entfernt das
/// Foto. Den Umschlag setzt [`Store::set_work_cover`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookEdit {
    pub publisher: Option<String>,
    pub place: Option<String>,
    pub year: Option<String>,
    pub edition: Option<String>,
    pub isbn: Option<String>,
    pub price: Option<String>,
    pub series: Option<String>,
    pub printer: Option<String>,
    pub bible_notice: Option<String>,
    pub image_credit: Option<String>,
    pub dedication: Option<String>,
    pub blurb: Option<String>,
    pub bio: Option<String>,
    pub author_photo: Option<Option<String>>,
    pub title_area: Option<TitleArea>,
    pub front_matter: Option<bool>,
    pub cover_pages: Option<bool>,
    pub paper_mm: Option<f64>,
    pub bleed_mm: Option<f64>,
    /// Anschrift des Verlags (Runde 27).
    pub publisher_address: Option<String>,
    /// Anschrift der Druckerei (Runde 27).
    pub printer_address: Option<String>,
}

/// Ergebnis von [`Store::set_book_meta`]: geänderte Felder (Namen wie in [`BookMeta`]) und der Stand
/// vorher und nachher (für den Spiegel-Commit).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookChange {
    pub changed: Vec<&'static str>,
    pub before: BookMeta,
    pub after: BookMeta,
}

/// Ergebnis von [`Store::set_work_cover`]: Bildname vorher und nachher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CoverChange {
    pub before: Option<String>,
    pub after: Option<String>,
}

/// Ein Textfeld mit seinen Grenzen und festen Meldungen.
struct TextField {
    name: &'static str,
    max: usize,
    multiline: bool,
    too_long: &'static str,
    one_line: &'static str,
}

macro_rules! field {
    ($name:literal, $label:literal, $max:literal, $multi:literal) => {
        TextField {
            name: $name,
            max: $max,
            multiline: $multi,
            too_long: concat!($label, ": höchstens ", stringify!($max), " Zeichen."),
            one_line: concat!($label, ": bitte in einer Zeile."),
        }
    };
}

/// Die Textfelder in fester Reihenfolge (so stehen sie auch in `changed`).
const TEXT_FIELDS: [TextField; 15] = [
    field!("publisher", "Verlag", 200, false),
    field!("place", "Verlagsort", 200, false),
    field!("year", "Jahr", 200, false),
    field!("edition", "Auflage", 200, false),
    field!("isbn", "ISBN", 200, false),
    field!("price", "Preis", 200, false),
    field!("series", "Reihe", 200, false),
    field!("printer", "Druck", 200, false),
    field!("bible_notice", "Rechtehinweis der Bibelübersetzung", 1000, true),
    field!("image_credit", "Bildnachweis", 1000, true),
    field!("dedication", "Widmung", 1000, true),
    field!("blurb", "Klappentext", 4000, true),
    field!("bio", "Kurzbiografie", 2000, true),
    field!("publisher_address", "Anschrift des Verlags bzw. Herausgebers", 500, true),
    field!("printer_address", "Anschrift der Druckerei", 500, true),
];

fn text_of<'a>(meta: &'a BookMeta, name: &str) -> &'a str {
    match name {
        "publisher" => &meta.publisher,
        "place" => &meta.place,
        "year" => &meta.year,
        "edition" => &meta.edition,
        "isbn" => &meta.isbn,
        "price" => &meta.price,
        "series" => &meta.series,
        "printer" => &meta.printer,
        "bible_notice" => &meta.bible_notice,
        "image_credit" => &meta.image_credit,
        "dedication" => &meta.dedication,
        "blurb" => &meta.blurb,
        "publisher_address" => &meta.publisher_address,
        "printer_address" => &meta.printer_address,
        _ => &meta.bio,
    }
}

fn text_mut<'a>(meta: &'a mut BookMeta, name: &str) -> &'a mut String {
    match name {
        "publisher" => &mut meta.publisher,
        "place" => &mut meta.place,
        "year" => &mut meta.year,
        "edition" => &mut meta.edition,
        "isbn" => &mut meta.isbn,
        "price" => &mut meta.price,
        "series" => &mut meta.series,
        "printer" => &mut meta.printer,
        "bible_notice" => &mut meta.bible_notice,
        "image_credit" => &mut meta.image_credit,
        "dedication" => &mut meta.dedication,
        "blurb" => &mut meta.blurb,
        "publisher_address" => &mut meta.publisher_address,
        "printer_address" => &mut meta.printer_address,
        _ => &mut meta.bio,
    }
}

fn wanted_text<'a>(edit: &'a BookEdit, name: &str) -> Option<&'a str> {
    let value = match name {
        "publisher" => &edit.publisher,
        "place" => &edit.place,
        "year" => &edit.year,
        "edition" => &edit.edition,
        "isbn" => &edit.isbn,
        "price" => &edit.price,
        "series" => &edit.series,
        "printer" => &edit.printer,
        "bible_notice" => &edit.bible_notice,
        "image_credit" => &edit.image_credit,
        "dedication" => &edit.dedication,
        "blurb" => &edit.blurb,
        "publisher_address" => &edit.publisher_address,
        "printer_address" => &edit.printer_address,
        _ => &edit.bio,
    };
    value.as_deref()
}

/// Eingabe eines Textfelds prüfen und in Normalform bringen (getrimmt, CRLF → LF).
fn clean_text(field: &TextField, value: &str) -> Result<String, StoreError> {
    let value = value.replace("\r\n", "\n");
    let value = value.trim();
    if !field.multiline && value.contains(['\n', '\r']) {
        return Err(StoreError::Refused(field.one_line));
    }
    if value.chars().count() > field.max {
        return Err(StoreError::Refused(field.too_long));
    }
    match field.name {
        "year" if !value.is_empty() && !(value.len() == 4 && value.bytes().all(|b| b.is_ascii_digit())) => {
            Err(StoreError::Refused("Jahr: leer lassen oder vier Ziffern (z. B. 2026)."))
        }
        "isbn" if !value.is_empty() => match baruch_typst::cover::isbn13_check(value) {
            Ok(_) => Ok(value.to_owned()),
            Err(e) => Err(StoreError::Refused(e.message())),
        },
        _ => Ok(value.to_owned()),
    }
}

/// Gleicher Text (Runde 20c: auch bei der ISBN zählt die Schreibweise – „978-3-…“ statt „9783…“ ist eine Änderung,
/// die gespeichert wird; früher galten gleiche Ziffern als unverändert).
fn same_text(_name: &str, a: &str, b: &str) -> bool {
    a == b
}

async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// id des Bilds `name` im Werk; unbekannt → `UnknownAsset`.
async fn asset_of(conn: &mut SqliteConnection, work: WorkId, name: &str) -> Result<i64, StoreError> {
    sqlx::query_scalar("SELECT id FROM asset WHERE work_id = ? AND name = ?")
        .bind(work)
        .bind(name)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| StoreError::UnknownAsset { work, name: name.to_owned() })
}

/// Buchangaben lesen (Werk muss es geben).
pub(crate) async fn read(conn: &mut SqliteConnection, work: WorkId) -> Result<BookMeta, StoreError> {
    work_exists(conn, work).await?;
    let row = sqlx::query(
        "SELECT b.*, (SELECT name FROM asset WHERE id = b.cover_asset_id) AS cover_name, \
                (SELECT name FROM asset WHERE id = b.author_photo_asset_id) AS photo_name \
         FROM work_book b WHERE b.work_id = ?",
    )
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else { return Ok(BookMeta::default()) };
    let mut meta = BookMeta::default();
    for field in &TEXT_FIELDS {
        *text_mut(&mut meta, field.name) = row.try_get(field.name)?;
    }
    meta.cover = row.try_get("cover_name")?;
    meta.author_photo = row.try_get("photo_name")?;
    let area: String = row.try_get("title_area")?;
    meta.title_area =
        TitleArea::parse(&area).ok_or_else(|| crate::broken(format!("unbekannter Titelbereich {area:?}")))?;
    meta.front_matter = row.try_get::<i64, _>("front_matter")? != 0;
    meta.cover_pages = row.try_get::<i64, _>("cover_pages")? != 0;
    meta.paper_mm = row.try_get("paper_mm")?;
    meta.bleed_mm = row.try_get("bleed_mm")?;
    meta.updated = row.try_get("updated")?;
    Ok(meta)
}

/// Bild-id zu einem Namen (oder keins).
async fn optional_asset(
    conn: &mut SqliteConnection,
    work: WorkId,
    name: Option<&str>,
) -> Result<Option<i64>, StoreError> {
    match name {
        Some(name) => asset_of(conn, work, name).await.map(Some),
        None => Ok(None),
    }
}

/// Ganze Zeile schreiben (anlegen oder ersetzen).
pub(crate) async fn write(conn: &mut SqliteConnection, work: WorkId, meta: &BookMeta) -> Result<(), StoreError> {
    let cover = optional_asset(conn, work, meta.cover.as_deref()).await?;
    let photo = optional_asset(conn, work, meta.author_photo.as_deref()).await?;
    let mut query = sqlx::query(
        "INSERT INTO work_book (work_id, cover_asset_id, title_area, front_matter, cover_pages, publisher, place, \
            year, edition, isbn, price, series, printer, bible_notice, image_credit, dedication, blurb, bio, \
            publisher_address, printer_address, author_photo_asset_id, paper_mm, bleed_mm, updated) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, \
            strftime('%Y-%m-%dT%H:%M:%fZ','now')) \
         ON CONFLICT (work_id) DO UPDATE SET cover_asset_id = excluded.cover_asset_id, \
            title_area = excluded.title_area, front_matter = excluded.front_matter, \
            cover_pages = excluded.cover_pages, publisher = excluded.publisher, place = excluded.place, \
            year = excluded.year, edition = excluded.edition, isbn = excluded.isbn, price = excluded.price, \
            series = excluded.series, printer = excluded.printer, bible_notice = excluded.bible_notice, \
            image_credit = excluded.image_credit, dedication = excluded.dedication, blurb = excluded.blurb, \
            bio = excluded.bio, publisher_address = excluded.publisher_address, \
            printer_address = excluded.printer_address, author_photo_asset_id = excluded.author_photo_asset_id, \
            paper_mm = excluded.paper_mm, bleed_mm = excluded.bleed_mm, updated = excluded.updated",
    )
    .bind(work)
    .bind(cover)
    .bind(meta.title_area.as_str())
    .bind(i64::from(meta.front_matter))
    .bind(i64::from(meta.cover_pages));
    for field in &TEXT_FIELDS {
        query = query.bind(text_of(meta, field.name).to_owned());
    }
    query.bind(photo).bind(meta.paper_mm).bind(meta.bleed_mm).execute(&mut *conn).await?;
    Ok(())
}

impl Store {
    /// Buchangaben des Werks; ohne Zeile die Vorgaben. Unbekanntes Werk → `UnknownWork`.
    pub async fn book_meta(&self, work: WorkId) -> Result<BookMeta, StoreError> {
        let mut conn = self.pool.acquire().await?;
        read(&mut conn, work).await
    }

    /// Buchangaben ändern (atomar, nur der Mensch). Prüft alle Felder (siehe Moduldoku), schreibt nur,
    /// wenn sich etwas ändert, und gibt geänderte Felder sowie den Stand vorher und nachher zurück.
    pub async fn set_book_meta(&self, work: WorkId, edit: &BookEdit) -> Result<BookChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = read(&mut tx, work).await?;
        let mut after = before.clone();
        let mut changed: Vec<&'static str> = Vec::new();
        for field in &TEXT_FIELDS {
            if let Some(wanted) = wanted_text(edit, field.name) {
                let wanted = clean_text(field, wanted)?;
                if !same_text(field.name, text_of(&before, field.name), &wanted) {
                    *text_mut(&mut after, field.name) = wanted;
                    changed.push(field.name);
                }
            }
        }
        if let Some(photo) = &edit.author_photo {
            let photo = photo.as_deref().map(str::trim).filter(|p| !p.is_empty());
            if let Some(name) = photo {
                asset_of(&mut tx, work, name).await?;
            }
            if photo != before.author_photo.as_deref() {
                after.author_photo = photo.map(str::to_owned);
                changed.push("author_photo");
            }
        }
        if let Some(area) = edit.title_area.filter(|&a| a != before.title_area) {
            after.title_area = area;
            changed.push("title_area");
        }
        if let Some(on) = edit.front_matter.filter(|&on| on != before.front_matter) {
            after.front_matter = on;
            changed.push("front_matter");
        }
        if let Some(on) = edit.cover_pages.filter(|&on| on != before.cover_pages) {
            after.cover_pages = on;
            changed.push("cover_pages");
        }
        if let Some(paper) = edit.paper_mm {
            if !(paper.is_finite() && paper > 0.0 && paper <= 1.0) {
                return Err(StoreError::Refused("Papierdicke: mehr als 0 und höchstens 1 mm je Seite."));
            }
            if paper != before.paper_mm {
                after.paper_mm = paper;
                changed.push("paper_mm");
            }
        }
        if let Some(bleed) = edit.bleed_mm {
            if !(bleed.is_finite() && (0.0..=10.0).contains(&bleed)) {
                return Err(StoreError::Refused("Beschnitt: 0 bis 10 mm."));
            }
            if bleed != before.bleed_mm {
                after.bleed_mm = bleed;
                changed.push("bleed_mm");
            }
        }
        if changed.is_empty() {
            return Ok(BookChange { changed, after: before.clone(), before });
        }
        write(&mut tx, work, &after).await?;
        let after = read(&mut tx, work).await?;
        tx.commit().await?;
        Ok(BookChange { changed, before, after })
    }

    /// Bildname des Umschlags (oder keiner).
    pub async fn work_cover(&self, work: WorkId) -> Result<Option<String>, StoreError> {
        Ok(self.book_meta(work).await?.cover)
    }

    /// Umschlag setzen (`Some(name)`, ein Bild des Werks) oder entfernen (`None` bzw. leerer Name).
    /// Unbekanntes Bild → `UnknownAsset`, nichts geändert. Gleicher Umschlag → keine Änderung
    /// (`before == after`).
    pub async fn set_work_cover(&self, work: WorkId, name: Option<&str>) -> Result<CoverChange, StoreError> {
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = read(&mut tx, work).await?;
        if let Some(name) = name {
            asset_of(&mut tx, work, name).await?;
        }
        if name == before.cover.as_deref() {
            return Ok(CoverChange { before: before.cover.clone(), after: before.cover });
        }
        let mut after = before.clone();
        after.cover = name.map(str::to_owned);
        write(&mut tx, work, &after).await?;
        let cover = read(&mut tx, work).await?.cover;
        tx.commit().await?;
        Ok(CoverChange { before: before.cover, after: cover })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_field_and_its_limit() {
        assert_eq!(TEXT_FIELDS[11].too_long, "Klappentext: höchstens 4000 Zeichen.", "Grenze steht in der Meldung");
        assert_eq!(TEXT_FIELDS[0].one_line, "Verlag: bitte in einer Zeile.", "einzeilig");
        assert_eq!(
            TEXT_FIELDS[14].too_long, "Anschrift der Druckerei: höchstens 500 Zeichen.",
            "Anschriften (Runde 27) mit ihrer Grenze"
        );
        assert!(TEXT_FIELDS[13].multiline && TEXT_FIELDS[14].multiline, "Anschriften sind mehrzeilig");
    }

    #[test]
    fn an_isbn_written_differently_is_a_change() {
        assert!(
            !same_text("isbn", "978-3-16-148410-0", "9783161484100"),
            "nur Bindestriche anders ist trotzdem eine Änderung (Runde 20c: die Schreibweise wird gespeichert)"
        );
        assert!(same_text("isbn", "978-3-16-148410-0", "978-3-16-148410-0"), "gleich geschrieben bleibt unverändert");
        assert!(!same_text("isbn", "", "9783161484100"), "leer → gesetzt ist eine Änderung");
    }

    #[test]
    fn an_isbn_without_978_or_979_is_refused_with_its_own_message() {
        let field = TEXT_FIELDS.iter().find(|f| f.name == "isbn").unwrap();
        match clean_text(field, "400-1-23-456789-3") {
            Err(StoreError::Refused(message)) => {
                assert!(message.contains("978 oder 979"), "eigene Meldung für das Präfix: {message}")
            }
            other => panic!("Präfix 400 muss abgelehnt werden: {other:?}"),
        }
        assert_eq!(
            clean_text(field, "isbn-13 978-3-16-148410-0").ok().as_deref(),
            Some("isbn-13 978-3-16-148410-0"),
            "„isbn-13 …“ in Kleinschrift gilt und wird wie eingegeben gespeichert"
        );
    }
}
