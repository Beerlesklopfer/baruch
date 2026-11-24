// SPDX-License-Identifier: AGPL-3.0-or-later
//! Buchsatz in den Einstellungen (Runde 29): Satz-Einstellungen eines Werks ([`TypesetSettings`]) und was daraus
//! folgt – Typst-Regeln hinter der Werkvorlage ([`rules`]), Konflikte mit der Werkvorlage ([`template_conflicts`]),
//! Erkennen und Entfernen einer Kapitelanfangs-Regel der Vorlage ([`detect_chapter_break`], [`remove_rule`]),
//! Mustertext der Vorschau ([`preview_source`]), die EPUB-Abbildung als Datenmodell ([`epub_style`]) und der Rückweg des
//! Regelblocks beim Import ([`settings_from_rules`], Runde 21b) und das Wasserzeichen „Entwurf“ der Ausgaben
//! ([`draft_rule`], Runde 22).
//!
//! - `model` ist die eingefrorene Schnittstelle (Vertrag §2), alles Übrige gehört nach R29-S1 der Aufgabe satz-typst.
//! - „Satz“ meint Typst (Satz-Vorschau, Buch-PDF, Umschlag, Export/Spiegel); das HTML-„Satzbild“ (Runde 18c) bleibt
//!   davon unberührt.

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
