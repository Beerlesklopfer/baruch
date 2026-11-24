// SPDX-License-Identifier: AGPL-3.0-or-later
//! Autorenbibliothek und Volltextsuche für Baruch.
//!
//! - **Bibliothek:** Titel (mit Zitierschlüssel für Typst-`@cite`) und Zitate daraus
//!   (Tabellen `library_title`, `library_quote`, Migration `0101_library.sql`).
//! - **Suche:** ein gemeinsamer FTS5-Index `search_fts` über Revisionen und Beiträge des Werks
//!   sowie Titel und Zitate der Bibliothek (`unicode61 remove_diacritics 2`, also findet
//!   „langmutig“ auch „langmütig“). Trigger halten ihn aktuell; `0102_search.sql` füllt ihn beim
//!   ersten Lauf aus vorhandenen Daten nach, `0104_revision_index.sql` folgt geänderten und gelöschten
//!   Revisionen, `0105_post_index.sql` gelöschten und verschobenen Beiträgen, `0107_post_edit_index.sql`
//!   bearbeiteten Beiträgen (Runde 28).
//! - **Hervorhebungen:** markierte Stellen in Quellen mit Art und Notiz, vom Menschen oder als
//!   Vorschlag einer Persona (Tabelle `library_highlight`, Migration `0103_highlights.sql`), im
//!   Index als `highlight`; [`cite_typst`] macht daraus ein Typst-Zitat. Fehler über den eigenen
//!   [`HighlightError`].
//! - **Quellen außerhalb (Runde 13):** Verknüpfung eines Titels mit einem Dokument in MultiGPT
//!   ([`RemoteRef`], Tabelle `library_title_remote`) und das Ergebnis der Prüfung einer Hervorhebung ohne
//!   Zitat gegen dessen Volltext ([`FulltextCheck`], [`check_range`], Tabelle `library_highlight_check`,
//!   Migration `0106_source_remote.sql`).
//! - **Lokaler Volltext (Runde 21a):** Text einer als Quelle importierten PDF- oder EPUB-Datei, gelesen ohne
//!   MultiGPT ([`TitleText`], Tabelle `library_title_text`, Migration `0108_title_text.sql`); Dokumentansicht und
//!   Prüfung von Hervorhebungen nutzen ihn wie den Volltext einer Verknüpfung.
//!
//! Die Crate kennt nur `sqlx::SqlitePool` und das Kernschema (`work`, `node`, `revision`, `post`)
//! aus `baruch-store`. Reihenfolge beim Start: erst die Kernmigration, dann [`migrate`], dann
//! [`seed_if_empty`]. Beide Migratoren teilen `_sqlx_migrations` und laufen mit
//! `ignore_missing`, damit keiner die Versionen des anderen als fehlend meldet.

mod highlights;
mod remote;
mod search;
mod seed;
mod text;
mod titles;

pub use highlights::{
    FulltextCheck, Highlight, HighlightAuthor, HighlightCheck, HighlightError, HighlightKind, HighlightStatus,
    NewHighlight, add_highlight, check_range, cite_typst, delete_highlight, escape_markup, get_highlight,
    highlight_check, highlight_check_entries, highlight_checks, list_highlights, set_highlight_check, update_highlight,
};
pub use remote::{RemoteRef, set_title_remote, title_for_remote, title_remote};
pub use search::{HitKind, SearchHit, SearchOptions, fts_query, search};
pub use seed::seed_if_empty;
pub use text::{TitleText, set_title_text, title_for_text_sha256, title_text, title_text_created};
pub use titles::{NewTitle, Quote, Title, TitleEntry, add_quote, add_title, get_title, list_titles};

use sqlx::migrate::Migrator;

/// Migrationen der Bibliothek (Versionen 0101–0199), `ignore_missing` ist gesetzt.
pub static MIGRATOR: Migrator = Migrator { ignore_missing: true, ..sqlx::migrate!("./migrations") };

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("Datenbank: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("Leere Suche.")]
    EmptyQuery,
    #[error("unbekannter Titel {0}")]
    UnknownTitle(i64),
}

/// Führt die Migrationen der Bibliothek aus (nach der Kernmigration; mehrfach aufrufbar).
pub async fn migrate(pool: &sqlx::SqlitePool) -> Result<(), LibraryError> {
    debug_assert!(MIGRATOR.ignore_missing);
    MIGRATOR.run(pool).await?;
    Ok(())
}
