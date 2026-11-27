// SPDX-License-Identifier: AGPL-3.0-or-later
//! Book typesetting in the settings (Round 29): typesetting settings of a work ([`TypesetSettings`]) and what
//! follows from them – Typst rules behind the work template ([`rules`]), conflicts with the work template
//! ([`template_conflicts`]), detecting and removing a chapter-start rule of the template ([`detect_chapter_break`],
//! [`remove_rule`]), sample text of the preview ([`preview_source`]), the EPUB mapping as a data model
//! ([`epub_style`]) and the way back of the rule block on import ([`settings_from_rules`], Round 21b) and the
//! watermark „Entwurf“ of the outputs ([`draft_rule`], Round 22).
//!
//! - `model` is the frozen interface (contract §2), everything else belongs to the task satz-typst after R29-S1.
//! - „Satz“ means Typst (typesetting preview, book PDF, cover, export/mirror); the HTML „Satzbild“ (typeset view,
//!   Round 18c) stays untouched by it.

mod draft;
mod epub;
mod model;
mod preview;
mod read;
mod rules;
mod template;

pub use draft::{DRAFT_MARK, draft_line, draft_rule};
pub use epub::*;
pub use model::*;
pub use preview::preview_source;
pub use read::settings_from_rules;
pub use rules::{part_page_rule_for, part_page_rule_with, rules};
pub use template::{
    CANONICAL_CHAPTER_BREAK, ChapterBreakRule, ConflictEffect, TemplateConflict, detect_chapter_break, remove_rule,
    template_conflicts,
};
