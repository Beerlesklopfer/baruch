// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abnahme der Runde 32 (Integrator): Rundreise Word über den Server, Menüeinträge, kein MCP-Werkzeug, Doku.

/// Schnittstelle R32-S1: Die drei Einträge stehen an den Plätzen des Menübands (Vertrag Runde 32 §0.5) – wörtlich, als
/// gewöhnliche Links (ohne JavaScript bedienbar). Die Marken selbst prüft Runde 33.
#[test]
fn the_three_entries_stand_at_their_places_in_the_ribbon() {
    let base = include_str!("../templates/base.html");
    let import = "<a class=\"menu-item\" role=\"menuitem\" href=\"/werke/import/word\" title=\"Word-Datei lesen, \
                  Gliederung, Bearbeiter und Warnungen prüfen und als Werk übernehmen\">Word (.docx) importieren…</a>";
    let quit = "<a class=\"menu-item\" role=\"menuitem\" href=\"/beenden\" title=\"Baruch auf diesem Rechner beenden \
                (mit Rückfrage)\">Baruch beenden…</a>";
    let export = "<a class=\"btn\" href=\"/werke/{{ w }}/word\" title=\"Das Werk als Word-Datei für den Verlag \
                  (Formatvorlagen, Fußnoten, Bilder; auf Wunsch offene Vorschläge als Änderungsverfolgung)\">Word …</a>";
    for entry in [import, quit, export] {
        assert_eq!(base.matches(entry).count(), 1, "der Eintrag steht genau einmal und wörtlich in base.html: {entry}");
    }
    let after = |mark: &str| base.split(mark).nth(1).unwrap_or_else(|| panic!("Marke fehlt: {mark}")).trim_start();
    assert!(
        after("{#- R32-PLATZ import-word -#}").starts_with(import),
        "„Word (.docx) importieren…“ folgt direkt auf seine Marke (bei den Importen im Menü Baruch ▾)"
    );
    assert!(
        after("{#- R32-PLATZ export-word -#}").starts_with(export),
        "„Word …“ folgt direkt auf seine Marke (Gruppe Ausgabe, hinter EPUB)"
    );
    let before_quit = base.split("{#- R32-PLATZ beenden -#}").next().unwrap().trim_end();
    assert!(
        before_quit.ends_with(quit),
        "„Baruch beenden…“ steht direkt vor seiner Marke – dahinter folgt im Menü kein Eintrag mehr (letzter Eintrag)"
    );
}
