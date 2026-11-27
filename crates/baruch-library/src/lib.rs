// SPDX-License-Identifier: AGPL-3.0-or-later
//! Author's library and full-text search for Baruch.
//!
//! - **Library:** titles (with citation key for Typst `@cite`) and quotes from them (tables `library_title`,
//!   `library_quote`, migration `0101_library.sql`).
//! - **Search:** one shared FTS5 index `search_fts` over revisions and posts of the work as well as titles and
//!   quotes of the library (`unicode61 remove_diacritics 2`, so „langmutig“ also finds „langmütig“). Triggers keep
//!   it up to date; `0102_search.sql` fills it on the first run from existing data, `0104_revision_index.sql`
//!   follows changed and deleted revisions, `0105_post_index.sql` deleted and moved posts,
//!   `0107_post_edit_index.sql` edited posts (Round 28).
//! - **Highlights:** marked places in sources with kind and note, by the human or as a proposal of a persona (table
//!   `library_highlight`, migration `0103_highlights.sql`), in the index as `highlight`; [`cite_typst`] turns one
//!   into a Typst quote. Errors through the separate [`HighlightError`].
//! - **Sources outside (Round 13):** link of a title with a document in MultiGPT ([`RemoteRef`], table
//!   `library_title_remote`) and the result of checking a highlight without a quote against its full text
//!   ([`FulltextCheck`], [`check_range`], table `library_highlight_check`, migration `0106_source_remote.sql`).
//! - **Local full text (Round 21a):** text of a PDF or EPUB file imported as a source, read without MultiGPT
//!   ([`TitleText`], table `library_title_text`, migration `0108_title_text.sql`); document view and check of
//!   highlights use it like the full text of a link.
//!
//! The crate only knows `sqlx::SqlitePool` and the core schema (`work`, `node`, `revision`, `post`) from
//! `baruch-store`. Order at start: first the core migration, then [`migrate`], then [`seed_if_empty`]. Both
//! migrators share `_sqlx_migrations` and run with `ignore_missing`, so that neither reports the versions of the
//! other as missing.

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

/// Migrations of the library (versions 0101–0199), `ignore_missing` is set.
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

/// Runs the migrations of the library (after the core migration; can be called several times).
pub async fn migrate(pool: &sqlx::SqlitePool) -> Result<(), LibraryError> {
    debug_assert!(MIGRATOR.ignore_missing);
    MIGRATOR.run(pool).await?;
    Ok(())
}
