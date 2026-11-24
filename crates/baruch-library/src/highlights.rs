// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hervorhebungen in Quellen der Bibliothek (Tabelle `library_highlight`, Migration
//! `0103_highlights.sql`): eine markierte Stelle eines Titels, optional in einem Zitat mit
//! Zeichenbereich, dazu Art (Zitat, Kernaussage, Widerspruch, zu prüfen), Notiz und Urheber.
//!
//! Der Mensch legt aktive Hervorhebungen an, Personas nur Vorschläge (auch die Datenbank prüft das).
//! Hervorhebungen stehen im Volltextindex (`kind = 'highlight'`); abgelehnte findet die Suche nicht.
//! [`cite_typst`] macht daraus ein Typst-Zitat für einen Abschnitt.

use sqlx::SqlitePool;
use unicode_normalization::UnicodeNormalization;

use crate::LibraryError;

/// Art der Hervorhebung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightKind {
    Quote,
    Core,
    Contradiction,
    Check,
}

impl HighlightKind {
    pub const ALL: [HighlightKind; 4] =
        [HighlightKind::Quote, HighlightKind::Core, HighlightKind::Contradiction, HighlightKind::Check];

    pub fn as_str(self) -> &'static str {
        match self {
            HighlightKind::Quote => "quote",
            HighlightKind::Core => "core",
            HighlightKind::Contradiction => "contradiction",
            HighlightKind::Check => "check",
        }
    }

    pub fn parse(s: &str) -> Option<HighlightKind> {
        HighlightKind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// Anzeige: Zitat, Kernaussage, Widerspruch, zu prüfen.
    pub fn label(self) -> &'static str {
        match self {
            HighlightKind::Quote => "Zitat",
            HighlightKind::Core => "Kernaussage",
            HighlightKind::Contradiction => "Widerspruch",
            HighlightKind::Check => "zu prüfen",
        }
    }
}

/// Stand der Hervorhebung: `active` (vom Menschen), `proposed` (Vorschlag einer Persona), `rejected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightStatus {
    Active,
    Proposed,
    Rejected,
}

impl HighlightStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            HighlightStatus::Active => "active",
            HighlightStatus::Proposed => "proposed",
            HighlightStatus::Rejected => "rejected",
        }
    }

    pub fn parse(s: &str) -> Option<HighlightStatus> {
        [HighlightStatus::Active, HighlightStatus::Proposed, HighlightStatus::Rejected]
            .into_iter()
            .find(|k| k.as_str() == s)
    }

    /// Anzeige: aktiv, Vorschlag, abgelehnt.
    pub fn label(self) -> &'static str {
        match self {
            HighlightStatus::Active => "aktiv",
            HighlightStatus::Proposed => "Vorschlag",
            HighlightStatus::Rejected => "abgelehnt",
        }
    }
}

/// Urheber: der Mensch oder eine Persona (Handle und Modell).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HighlightAuthor {
    Human { name: String },
    Persona { handle: String, model: String },
}

/// Neue Hervorhebung für [`add_highlight`]. Der Status folgt aus dem Urheber.
#[derive(Debug, Clone)]
pub struct NewHighlight {
    pub title_id: i64,
    pub quote_id: Option<i64>,
    pub locator: String,
    /// Zeichenbereich ab 0 (Unicode-Zeichen), beide `None` oder `start <= end`.
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub text: String,
    pub kind: HighlightKind,
    pub note: String,
    pub author: HighlightAuthor,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Highlight {
    pub id: i64,
    pub title_id: i64,
    pub quote_id: Option<i64>,
    pub locator: String,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub text: String,
    pub kind: HighlightKind,
    pub note: String,
    pub author: HighlightAuthor,
    pub status: HighlightStatus,
    pub created: String,
}

/// Fehler der Hervorhebungen. Bewusst eigener Typ: [`LibraryError`] bleibt, wie er ist.
#[derive(Debug, thiserror::Error)]
pub enum HighlightError {
    #[error(transparent)]
    Library(#[from] LibraryError),
    /// Unbekannte Hervorhebung bzw. ein Zitat, das es beim Titel nicht gibt.
    #[error("Hervorhebung bzw. Zitat {0} gibt es nicht.")]
    Unknown(i64),
    #[error("Eine Hervorhebung braucht Text.")]
    Empty,
    #[error(
        "Ungültiger Zeichenbereich: Anfang und Ende ab 0, Anfang nicht hinter dem Ende, Ende nicht hinter \
         dem Ende des Zitats."
    )]
    InvalidRange,
    /// Der Text entspricht nicht dem Ausschnitt `start..end` des Zitats.
    #[error("Der Text entspricht nicht dem markierten Ausschnitt des Zitats ({0:?}).")]
    TextMismatch(String),
}

impl From<sqlx::Error> for HighlightError {
    fn from(e: sqlx::Error) -> Self {
        HighlightError::Library(LibraryError::Db(e))
    }
}

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    title_id: i64,
    quote_id: Option<i64>,
    locator: String,
    start_char: Option<i64>,
    end_char: Option<i64>,
    text: String,
    kind: String,
    note: String,
    author_kind: String,
    author_name: String,
    model: Option<String>,
    status: String,
    created: String,
}

const COLUMNS: &str = "id, title_id, quote_id, locator, start_char, end_char, text, kind, note, \
                       author_kind, author_name, model, status, created";

fn highlight(row: Row) -> Result<Highlight, HighlightError> {
    let broken = |what: &str| HighlightError::from(sqlx::Error::Protocol(format!("Hervorhebung {}: {what}", row.id)));
    let kind = HighlightKind::parse(&row.kind).ok_or_else(|| broken("unbekannte Art"))?;
    let status = HighlightStatus::parse(&row.status).ok_or_else(|| broken("unbekannter Status"))?;
    let author = match (row.author_kind.as_str(), row.model) {
        ("human", None) => HighlightAuthor::Human { name: row.author_name },
        ("assistant", Some(model)) => HighlightAuthor::Persona { handle: row.author_name, model },
        _ => return Err(broken("unbekannter Urheber")),
    };
    Ok(Highlight {
        id: row.id,
        title_id: row.title_id,
        quote_id: row.quote_id,
        locator: row.locator,
        start: row.start_char,
        end: row.end_char,
        text: row.text,
        kind,
        note: row.note,
        author,
        status,
        created: row.created,
    })
}

/// Zeichenbereich prüfen: beide fehlen oder `0 <= start <= end`.
fn valid_range(start: Option<i64>, end: Option<i64>) -> Result<(), HighlightError> {
    match (start, end) {
        (None, None) => Ok(()),
        (Some(s), Some(e)) if 0 <= s && s <= e => Ok(()),
        _ => Err(HighlightError::InvalidRange),
    }
}

/// Leerraum vereinheitlichen: jede Folge von Unicode-Leerraum (Umbruch, Tab, geschütztes Leerzeichen
/// U+00A0, …) wird ein Leerzeichen, Anfang und Ende ohne Leerraum.
pub(crate) fn normalize_space(text: &str) -> String {
    text.split(char::is_whitespace).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Prüft einen Bereich in einem Zitat: `end` höchstens so lang wie das Zitat (in Unicode-Zeichen),
/// und `text` ist der Ausschnitt `quote[start..end]` (Zeilenenden vereinheitlicht). Unterscheiden sich
/// beide nur im Leerraum (eine Auswahl im Browser fasst Umbrüche und Doppel-Leerzeichen zusammen),
/// gilt der Text als gleich. Gibt den genauen Ausschnitt des Zitats zurück, der gespeichert wird.
fn check_excerpt(quote: &str, start: i64, end: i64, text: &str) -> Result<String, HighlightError> {
    let quote = quote.replace("\r\n", "\n");
    let (start, end) = (usize::try_from(start), usize::try_from(end));
    let (Ok(start), Ok(end)) = (start, end) else { return Err(HighlightError::InvalidRange) };
    if end > quote.chars().count() {
        return Err(HighlightError::InvalidRange);
    }
    let excerpt: String = quote.chars().skip(start).take(end - start).collect();
    if excerpt != text && normalize_space(&excerpt) != normalize_space(text) {
        return Err(HighlightError::TextMismatch(excerpt));
    }
    Ok(excerpt)
}

/// Legt eine Hervorhebung an und gibt ihre id zurück. Mensch → `active`, Persona → immer
/// `proposed`. Ohne Fundstelle (`locator` leer) gilt die Stelle des Zitats. Leerer Text → [`HighlightError::Empty`], ungültiger Bereich
/// (auch: Ende hinter dem Zitat) → [`HighlightError::InvalidRange`]. Mit Zitat und Bereich wird der
/// genaue Ausschnitt `quote[start..end]` gespeichert, wenn `text` ihm bis auf Leerraum gleicht
/// ([`normalize_space`]); sonst → [`HighlightError::TextMismatch`], unbekannter Titel → `LibraryError::UnknownTitle`, ein Zitat,
/// das nicht zum Titel gehört → [`HighlightError::Unknown`].
pub async fn add_highlight(pool: &SqlitePool, new: &NewHighlight) -> Result<i64, HighlightError> {
    if new.text.trim().is_empty() {
        return Err(HighlightError::Empty);
    }
    valid_range(new.start, new.end)?;
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let title: Option<i64> = sqlx::query_scalar("SELECT id FROM library_title WHERE id = ?1")
        .bind(new.title_id)
        .fetch_optional(&mut *tx)
        .await?;
    if title.is_none() {
        return Err(LibraryError::UnknownTitle(new.title_id).into());
    }
    let mut text = new.text.replace("\r\n", "\n");
    if let Some(quote) = new.quote_id {
        let found: Option<String> =
            sqlx::query_scalar("SELECT text FROM library_quote WHERE id = ?1 AND title_id = ?2")
                .bind(quote)
                .bind(new.title_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(quote_text) = found else { return Err(HighlightError::Unknown(quote)) };
        if let (Some(start), Some(end)) = (new.start, new.end) {
            text = check_excerpt(&quote_text, start, end, &text)?;
        }
    }
    // Ohne Zitat gilt hier nur 0 <= start <= end: Den Bereich gegen den Volltext einer verknüpften Quelle
    // prüft der Server danach ([`check_range`], [`set_highlight_check`]); die Hervorhebung des Menschen
    // wird in jedem Fall gespeichert.
    let (author_kind, author_name, model, status) = match &new.author {
        HighlightAuthor::Human { name } => ("human", name.as_str(), None, HighlightStatus::Active),
        HighlightAuthor::Persona { handle, model } => {
            ("assistant", handle.as_str(), Some(model.as_str()), HighlightStatus::Proposed)
        }
    };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO library_highlight \
           (title_id, quote_id, locator, start_char, end_char, text, kind, note, author_kind, author_name, model, status) \
         VALUES (?1, ?2, coalesce(nullif(?3, ''), (SELECT page FROM library_quote WHERE id = ?2), ''), \
                 ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) RETURNING id",
    )
    .bind(new.title_id)
    .bind(new.quote_id)
    .bind(new.locator.trim())
    .bind(new.start)
    .bind(new.end)
    .bind(&text)
    .bind(new.kind.as_str())
    .bind(new.note.trim())
    .bind(author_kind)
    .bind(author_name)
    .bind(model)
    .bind(status.as_str())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Alle Hervorhebungen (oder die eines Titels), nach Titel, Zitat, Anfang und id.
pub async fn list_highlights(pool: &SqlitePool, title_id: Option<i64>) -> Result<Vec<Highlight>, HighlightError> {
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM library_highlight WHERE ?1 IS NULL OR title_id = ?1 \
         ORDER BY title_id, quote_id IS NULL, quote_id, start_char IS NULL, start_char, id"
    ))
    .bind(title_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(highlight).collect()
}

/// Eine Hervorhebung; unbekannt → [`HighlightError::Unknown`].
pub async fn get_highlight(pool: &SqlitePool, id: i64) -> Result<Highlight, HighlightError> {
    let row: Option<Row> = sqlx::query_as(&format!("SELECT {COLUMNS} FROM library_highlight WHERE id = ?1"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    highlight(row.ok_or(HighlightError::Unknown(id))?)
}

/// Art, Notiz und/oder Status ändern (`None` = bleibt) und den neuen Stand zurückgeben. Eine
/// Persona-Hervorhebung lässt sich nicht auf `active` setzen (Datenbankprüfung, `Library(Db)`).
pub async fn update_highlight(
    pool: &SqlitePool,
    id: i64,
    kind: Option<HighlightKind>,
    note: Option<&str>,
    status: Option<HighlightStatus>,
) -> Result<Highlight, HighlightError> {
    let changed = sqlx::query(
        "UPDATE library_highlight SET kind = coalesce(?2, kind), note = coalesce(?3, note), \
         status = coalesce(?4, status) WHERE id = ?1",
    )
    .bind(id)
    .bind(kind.map(HighlightKind::as_str))
    .bind(note.map(str::trim))
    .bind(status.map(HighlightStatus::as_str))
    .execute(pool)
    .await?
    .rows_affected();
    if changed == 0 {
        return Err(HighlightError::Unknown(id));
    }
    get_highlight(pool, id).await
}

/// Hervorhebung löschen; unbekannt → [`HighlightError::Unknown`].
pub async fn delete_highlight(pool: &SqlitePool, id: i64) -> Result<(), HighlightError> {
    let changed =
        sqlx::query("DELETE FROM library_highlight WHERE id = ?1").bind(id).execute(pool).await?.rows_affected();
    if changed == 0 {
        return Err(HighlightError::Unknown(id));
    }
    Ok(())
}

// ---- Prüfung gegen den Volltext (Runde 13, Migration 0106) ---------------------------------------------

/// Ergebnis der Prüfung einer Hervorhebung ohne Zitat gegen den Volltext ihrer Quelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FulltextCheck {
    /// Der Text steht genau im Bereich `start..end` des Volltexts.
    Found,
    /// Im Bereich steht etwas anderes (oder der Bereich liegt hinter dem Ende).
    NotFound,
    /// Nicht prüfbar: keine Verknüpfung, Gegenstelle nicht erreichbar, kein Bereich.
    Unverifiable,
}

impl FulltextCheck {
    pub const ALL: [FulltextCheck; 3] = [FulltextCheck::Found, FulltextCheck::NotFound, FulltextCheck::Unverifiable];

    pub fn as_str(self) -> &'static str {
        match self {
            FulltextCheck::Found => "found",
            FulltextCheck::NotFound => "not_found",
            FulltextCheck::Unverifiable => "unverifiable",
        }
    }

    pub fn parse(s: &str) -> Option<FulltextCheck> {
        FulltextCheck::ALL.into_iter().find(|c| c.as_str() == s)
    }

    /// Anzeige: „im Volltext gefunden“, „im Volltext nicht gefunden“, „nicht prüfbar“.
    pub fn label(self) -> &'static str {
        match self {
            FulltextCheck::Found => "im Volltext gefunden",
            FulltextCheck::NotFound => "im Volltext nicht gefunden",
            FulltextCheck::Unverifiable => "nicht prüfbar",
        }
    }
}

/// Gespeicherte Prüfung einer Hervorhebung (`library_highlight_check`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HighlightCheck {
    pub result: FulltextCheck,
    pub label: &'static str,
    /// Grund bzw. Hinweis (bei „nicht prüfbar“ der Grund, bei „nicht gefunden“ der Text im Bereich).
    pub message: String,
    /// Zeitpunkt der Prüfung (UTC, ISO 8601).
    pub checked: String,
}

/// Steht `text` genau im Bereich `start..end` (Unicode-Zeichen ab 0) des Volltexts? Zeilenenden werden
/// vereinheitlicht (`\r\n` → `\n`), Leerraum zählt wie bei der Prüfung im Zitat nicht ([`normalize_space`]).
/// Ein Bereich außerhalb des Volltexts (negativ, Anfang hinter dem Ende, Ende hinter dem Text) ist
/// [`FulltextCheck::NotFound`]. Gibt nie [`FulltextCheck::Unverifiable`] zurück: Diese Funktion hat den
/// Volltext ja.
///
/// Runde 19b: Verglichen wird in Unicode-Normalform NFC – ein Zitat mit zerlegten Umlauten (`a` + U+0308, etwa aus
/// einem PDF) stimmt mit „ä“ im Volltext überein und umgekehrt. Der Bereich zählt in den Zeichen des Volltexts, wie er
/// gespeichert ist; passt er dort nicht, wird er noch in den Zeichen der NFC-Form des Volltexts versucht (so zählt
/// ein Browser, der den Text normalisiert anzeigt).
pub fn check_range(fulltext: &str, start: i64, end: i64, text: &str) -> FulltextCheck {
    let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end)) else {
        return FulltextCheck::NotFound;
    };
    let text: String = text.replace("\r\n", "\n").nfc().collect();
    if text.trim().is_empty() {
        return FulltextCheck::NotFound;
    }
    let fulltext = fulltext.replace("\r\n", "\n");
    let normalized: String = fulltext.nfc().collect();
    let same = |source: &str| {
        if start > end || end > source.chars().count() {
            return false;
        }
        let excerpt: String = source.chars().skip(start).take(end - start).nfc().collect();
        excerpt == text || normalize_space(&excerpt) == normalize_space(&text)
    };
    if same(&fulltext) || (normalized != fulltext && same(&normalized)) {
        FulltextCheck::Found
    } else {
        FulltextCheck::NotFound
    }
}

/// Ergebnis der Prüfung speichern (ersetzt eine frühere Prüfung; Zeitpunkt = jetzt). Unbekannte
/// Hervorhebung → [`HighlightError::Unknown`].
pub async fn set_highlight_check(
    pool: &SqlitePool,
    highlight_id: i64,
    result: FulltextCheck,
    message: &str,
) -> Result<(), HighlightError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM library_highlight WHERE id = ?1")
        .bind(highlight_id)
        .fetch_optional(&mut *tx)
        .await?;
    if exists.is_none() {
        return Err(HighlightError::Unknown(highlight_id));
    }
    sqlx::query(
        "INSERT INTO library_highlight_check (highlight_id, result, message) VALUES (?1, ?2, ?3) \
         ON CONFLICT (highlight_id) DO UPDATE SET result = excluded.result, message = excluded.message, \
         checked = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(highlight_id)
    .bind(result.as_str())
    .bind(message.trim())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Gespeicherte Prüfungen (aller Hervorhebungen bzw. der eines Titels) mit Zeitpunkt, je Hervorhebung.
pub async fn highlight_check_entries(
    pool: &SqlitePool,
    title_id: Option<i64>,
) -> Result<std::collections::HashMap<i64, HighlightCheck>, HighlightError> {
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT c.highlight_id, c.result, c.message, c.checked FROM library_highlight_check c \
           JOIN library_highlight h ON h.id = c.highlight_id WHERE ?1 IS NULL OR h.title_id = ?1",
    )
    .bind(title_id)
    .fetch_all(pool)
    .await?;
    let mut out = std::collections::HashMap::new();
    for (id, result, message, checked) in rows {
        let result = FulltextCheck::parse(&result).ok_or_else(|| {
            HighlightError::from(sqlx::Error::Protocol(format!("Prüfung der Hervorhebung {id}: unbekanntes Ergebnis")))
        })?;
        out.insert(id, HighlightCheck { result, label: result.label(), message, checked });
    }
    Ok(out)
}

/// Gespeicherte Prüfungen (aller Hervorhebungen bzw. der eines Titels): Ergebnis und Meldung.
pub async fn highlight_checks(
    pool: &SqlitePool,
    title_id: Option<i64>,
) -> Result<std::collections::HashMap<i64, (FulltextCheck, String)>, HighlightError> {
    Ok(highlight_check_entries(pool, title_id)
        .await?
        .into_iter()
        .map(|(id, check)| (id, (check.result, check.message)))
        .collect())
}

/// Gespeicherte Prüfung einer Hervorhebung; `None`, wenn sie nie geprüft wurde.
pub async fn highlight_check(pool: &SqlitePool, highlight_id: i64) -> Result<Option<HighlightCheck>, HighlightError> {
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT result, message, checked FROM library_highlight_check WHERE highlight_id = ?1")
            .bind(highlight_id)
            .fetch_optional(pool)
            .await?;
    row.map(|(result, message, checked)| {
        let result = FulltextCheck::parse(&result).ok_or_else(|| {
            HighlightError::from(sqlx::Error::Protocol(format!(
                "Prüfung der Hervorhebung {highlight_id}: unbekanntes Ergebnis"
            )))
        })?;
        Ok(HighlightCheck { result, label: result.label(), message, checked })
    })
    .transpose()
}

/// Typst-Zitat aus einer Hervorhebung:
/// `#quote(block: true, attribution: [@key])[Text]`, mit Fundstelle `[@key[S. 12]]`.
///
/// Text und Fundstelle sind für Typst-Markup maskiert: Zeichen mit Bedeutung (`\ # [ ] * _ `` ` ``
/// `$ < > @ ~ = - + /`) bekommen einen Backslash, eine Aufzählung wie „1.“ am Zeilenanfang ebenso.
/// Eine Fundstelle aus Ziffern (auch „12–14“) bekommt „S. “ davor; sonst bleibt sie, wie sie ist.
/// CRLF wird LF, Leerzeilen bleiben Absätze.
pub fn cite_typst(highlight: &Highlight, key: &str) -> String {
    let locator = highlight.locator.trim();
    let attribution = if locator.is_empty() {
        format!("@{key}")
    } else {
        let pages = locator.chars().all(|c| c.is_ascii_digit() || matches!(c, '-' | '–' | ',' | ' ' | 'f'));
        let locator = if pages && locator.starts_with(|c: char| c.is_ascii_digit()) {
            format!("S. {locator}")
        } else {
            locator.to_owned()
        };
        format!("@{key}[{}]", escape_markup(&locator))
    };
    let text = escape_markup(highlight.text.replace("\r\n", "\n").trim());
    format!("#quote(block: true, attribution: [{attribution}])[{text}]")
}

/// Maskiert Text für Typst-Markup (siehe [`cite_typst`]).
pub fn escape_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let indent = line.len() - line.trim_start().len();
        let rest = &line[indent..];
        out.push_str(&line[..indent]);
        // „1.“ am Zeilenanfang wäre eine nummerierte Liste.
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let numbered = digits > 0 && rest[digits..].starts_with('.');
        for (i, c) in rest.char_indices() {
            if matches!(
                c,
                '\\' | '#' | '[' | ']' | '*' | '_' | '`' | '$' | '<' | '>' | '@' | '~' | '=' | '-' | '+' | '/'
            ) || (numbered && i == digits)
            {
                out.push('\\');
            }
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(text: &str, locator: &str) -> Highlight {
        Highlight {
            id: 1,
            title_id: 1,
            quote_id: None,
            locator: locator.into(),
            start: None,
            end: None,
            text: text.into(),
            kind: HighlightKind::Quote,
            note: String::new(),
            author: HighlightAuthor::Human { name: "Autor".into() },
            status: HighlightStatus::Active,
            created: String::new(),
        }
    }

    #[test]
    fn markup_characters_are_escaped() {
        assert_eq!(
            escape_markup("a #b [c] *d* _e_ `f` $g$ <h> @i ~ = - + / \\"),
            "a \\#b \\[c\\] \\*d\\* \\_e\\_ \\`f\\` \\$g\\$ \\<h\\> \\@i \\~ \\= \\- \\+ \\/ \\\\",
            "jedes Zeichen mit Bedeutung im Typst-Markup bekommt einen Backslash"
        );
        assert_eq!(
            escape_markup("1. Kor 13,4\n  2. Zeile"),
            "1\\. Kor 13,4\n  2\\. Zeile",
            "Aufzählung am Zeilenanfang"
        );
        assert_eq!(escape_markup("Kap. 1. Satz"), "Kap. 1. Satz", "ein Punkt mitten im Satz bleibt");
        assert_eq!(escape_markup("Liebe, Glaube"), "Liebe, Glaube", "gewöhnlicher Text bleibt, wie er ist");
    }

    #[test]
    fn whitespace_runs_become_one_space() {
        assert_eq!(
            normalize_space("  Die\nLiebe \t ist\u{a0}\u{a0}langmütig\r\n"),
            "Die Liebe ist langmütig",
            "Umbruch, Tab, geschütztes Leerzeichen und Folgen davon werden ein Leerzeichen, Ränder entfallen"
        );
        assert_eq!(normalize_space(""), "", "leerer Text bleibt leer");
    }

    #[test]
    fn an_excerpt_matches_up_to_whitespace_and_keeps_the_quote_text() {
        let quote = "Die Liebe ist\nlangmütig  und freundlich.";
        assert_eq!(
            check_excerpt(quote, 4, 23, "Liebe ist langmütig").unwrap(),
            "Liebe ist\nlangmütig",
            "Auswahl mit Leerzeichen statt Umbruch gilt; gespeichert wird der Ausschnitt mit Umbruch"
        );
        assert_eq!(
            check_excerpt(quote, 14, 28, "langmütig und").unwrap(),
            "langmütig  und",
            "Doppel-Leerzeichen im Zitat, eines in der Auswahl"
        );
        assert!(
            matches!(check_excerpt(quote, 4, 23, "Liebe ist geduldig"), Err(HighlightError::TextMismatch(_))),
            "inhaltlich anderer Text bleibt abgelehnt"
        );
        assert!(
            matches!(check_excerpt(quote, 4, 23, "Liebeist langmütig"), Err(HighlightError::TextMismatch(_))),
            "fehlender Leerraum zwischen Wörtern ist eine inhaltliche Abweichung"
        );
    }

    #[test]
    fn a_range_in_the_fulltext_is_found_only_with_the_same_words() {
        let full = "Der Leuchtturm steht am Kai.\r\nÜber dem Wasser kreist das Licht.";
        // Begründung: „Ü“ zählt als ein Zeichen, `\r\n` als eines (vereinheitlicht) – sonst verschöben sich
        // alle Bereiche hinter der ersten Zeile.
        let start = "Der Leuchtturm steht am Kai.\n".chars().count() as i64;
        let end = start + "Über dem Wasser".chars().count() as i64;
        assert_eq!(check_range(full, start, end, "Über dem Wasser"), FulltextCheck::Found, "genauer Bereich");
        assert_eq!(
            check_range(full, start, end, "  Über\tdem \n Wasser "),
            FulltextCheck::Found,
            "Leerraum zählt nicht (wie bei der Auswahl im Zitat)"
        );
        assert_eq!(
            check_range(full, start, end, "Unter dem Wasser"),
            FulltextCheck::NotFound,
            "anderer Wortlaut im selben Bereich"
        );
        assert_eq!(
            check_range(full, start + 1, end + 1, "Über dem Wasser"),
            FulltextCheck::NotFound,
            "um ein Zeichen verschoben ist nicht derselbe Bereich"
        );
        let len = full.replace("\r\n", "\n").chars().count() as i64;
        assert_eq!(check_range(full, len - 6, len, "Licht."), FulltextCheck::Found, "Bereich bis genau ans Ende");
        assert_eq!(check_range(full, len - 6, len + 1, "Licht."), FulltextCheck::NotFound, "Ende hinter dem Text");
        assert_eq!(check_range(full, -1, 3, "Der"), FulltextCheck::NotFound, "negativer Anfang");
        assert_eq!(check_range(full, 5, 4, "x"), FulltextCheck::NotFound, "Anfang hinter dem Ende");
        assert_eq!(check_range(full, 0, 0, ""), FulltextCheck::NotFound, "leerer Text ist kein Beleg");
        assert_eq!(check_range("", 0, 0, "x"), FulltextCheck::NotFound, "leerer Volltext");
        for result in FulltextCheck::ALL {
            assert_eq!(FulltextCheck::parse(result.as_str()), Some(result), "{result:?} hin und zurück");
        }
        assert_eq!(FulltextCheck::Unverifiable.label(), "nicht prüfbar", "Etikett wörtlich nach Vertrag");
    }

    #[test]
    fn a_citation_names_key_and_locator() {
        assert_eq!(
            cite_typst(&sample("Die Liebe ist langmütig", "1. Kor 13,4"), "luther1912"),
            "#quote(block: true, attribution: [@luther1912[1\\. Kor 13,4]])[Die Liebe ist langmütig]",
            "Fundstelle als Ergänzung zu @key, „1.“ maskiert"
        );
        assert_eq!(
            cite_typst(&sample("Text", "12–14"), "k"),
            "#quote(block: true, attribution: [@k[S. 12–14]])[Text]",
            "reine Seitenzahl bekommt „S. “"
        );
        assert_eq!(
            cite_typst(&sample(" Text ", ""), "k"),
            "#quote(block: true, attribution: [@k])[Text]",
            "ohne Fundstelle"
        );
    }
}
