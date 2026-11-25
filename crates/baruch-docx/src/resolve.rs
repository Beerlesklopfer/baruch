// SPDX-License-Identifier: AGPL-3.0-or-later
//! Änderungsverfolgung auflösen (Aufgabe docx_import, Vertrag Runde 32 §2): ein [`DocxDocument`] mit
//! [`crate::Inline::Tracked`] und `mark` wird zu einem Stand ohne beides.

use crate::DocxDocument;

/// Welcher Stand entsteht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View<'a> {
    /// Stand vor allen Änderungen (Löschungen bleiben, Einfügungen entfallen).
    Original,
    /// Stand nach allen Änderungen (Einfügungen bleiben, Löschungen entfallen).
    Final,
    /// Original, auf das nur die Änderungen dieses Bearbeiters (wörtlicher Name) angewandt sind.
    FinalBy(&'a str),
}

/// Löst die Änderungen auf; das Ergebnis enthält weder [`crate::Inline::Tracked`] noch ein `mark`.
pub fn resolve(document: &DocxDocument, view: View<'_>) -> DocxDocument {
    // STUB: Auflösen fehlt – liefert ein leeres Dokument statt eines Stands (R32 docx_import)
    let _ = (document, view);
    DocxDocument::default()
}

/// Bearbeiter aller verfolgten Änderungen: wörtliche Namen in der Reihenfolge des ersten Auftretens.
pub fn change_authors(document: &DocxDocument) -> Vec<String> {
    // STUB: Bearbeiter sammeln fehlt – liefert keine (R32 docx_import)
    let _ = document;
    Vec::new()
}

/// Geänderte Überschrift: `section` ist die Stelle des Abschnitts in der Reihenfolge des Originals (ab 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingChange {
    pub section: usize,
    pub before: String,
    pub after: String,
}

/// Stand für den Vorschlag eines Bearbeiters: wie [`View::FinalBy`], aber die Überschriften bleiben wie im Original
/// (Änderungen an Überschriften ändern nie die Gliederung); die geänderten Überschriften kommen getrennt zurück.
pub fn proposal_view(document: &DocxDocument, author: &str) -> (DocxDocument, Vec<HeadingChange>) {
    // STUB: Vorschlagsstand fehlt – liefert ein leeres Dokument ohne Überschriftsänderungen (R32 docx_import)
    let _ = (document, author);
    (DocxDocument::default(), Vec::new())
}
