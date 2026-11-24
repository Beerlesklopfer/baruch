// SPDX-License-Identifier: AGPL-3.0-or-later
//! Menüband (Runde 33, Schnittstelle R33-S1; Besitz: Aufgabe „menueband“).
//!
//! Das Band steht vollständig in `templates/base.html` (die Vorlage ist in vielen eigenen Umgebungen eingebettet,
//! ein Fragment müsste in allen registriert werden). Seiten kennen ihre Adresse und `?tab=` nicht; darum setzt die
//! Schicht `consent::inject` über [`apply`] den aktiven Reiter, die Adressen der Reiter und den Laptop-Zustand im
//! fertigen HTML ein – wie die Abzeichen seit Runde 23a.
//!
//! Stand S1: [`apply`] ersetzt nur die Marke [`FILL`] an `<html>`; Adresse und Cookie wertet erst die Aufgabe
//! „menueband“ aus ([`BandState::from_request`] liefert die Vorgabe, [`set_cookies`] nichts).

/// Marke an `<html>` in `base.html`; [`apply`] ersetzt sie durch die Attribute des Zustands.
pub const FILL: &str = " data-band-fill";
/// Reiter des Bands: (Kennung in `?tab=`, Beschriftung).
pub const TABS: [(&str, &str); 5] =
    [("start", "Start"), ("einfuegen", "Einfügen"), ("pruefen", "Prüfen"), ("buch", "Buch"), ("ansicht", "Ansicht")];
/// Schlüssel der Adresse, die diesem Modul gehören.
pub const QUERY_KEYS: [&str; 2] = ["tab", "band"];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BandState {
    /// Reiter aus `?tab=`, nur wenn er in TABS steht.
    pub tab: Option<&'static str>,
    /// Band zugeklappt (`?band=` vor Cookie `baruch_band`).
    pub closed: bool,
    /// Rohe Abfrage der Anfrage ohne die Schlüssel aus band::QUERY_KEYS und laptop::QUERY_KEYS (Paare mit `&`, unverändert, nicht dekodiert).
    pub rest: String,
}

impl BandState {
    /// Zustand aus Cookie-Kopf und roher Abfrage der Anfrage.
    // S1-Körper: Adresse und Cookie werden noch nicht ausgewertet, es gilt immer die Vorgabe – ersetzt die Aufgabe „menueband“.
    pub fn from_request(cookie_header: Option<&str>, query: Option<&str>) -> BandState {
        let _ = (cookie_header, query);
        BandState::default()
    }
}

/// Fertige Set-Cookie-Werte für einen gültigen Schlüssel `band` in der Adresse.
// S1-Körper: merkt noch nichts, liefert immer eine leere Liste – ersetzt die Aufgabe „menueband“.
pub fn set_cookies(query: Option<&str>) -> Vec<String> {
    let _ = query;
    Vec::new()
}

/// Setzt den Zustand in eine fertige Seite ein; eine Seite ohne [`FILL`] bleibt unverändert.
// S1-Körper: ersetzt nur die Marke an <html> (immer data-band="open"); Reiter, Adressen und Gruppen fehlen – ersetzt die Aufgabe „menueband“.
pub fn apply(page: &str, band: &BandState, laptop: &crate::laptop::LaptopState) -> String {
    let _ = band;
    if !page.contains(FILL) {
        return page.to_owned();
    }
    page.replacen(FILL, &format!(" {} data-band=\"open\"", laptop.html_attrs()), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laptop::LaptopState;

    #[test]
    fn apply_replaces_only_the_first_marker() {
        let page = "<html lang=\"de\" data-band-fill><body><p data-band-fill>x</p></body></html>";
        let out = apply(page, &BandState::default(), &LaptopState::default());
        assert_eq!(
            out, "<html lang=\"de\" data-laptop=\"auto\" data-band=\"open\"><body><p data-band-fill>x</p></body></html>",
            "nur die Marke an <html> wird ersetzt; ein zweites Vorkommen im Seiteninhalt bleibt, wie es ist"
        );
    }

    #[test]
    fn page_without_marker_stays_untouched() {
        // Fragmente und Seiten, die base.html nicht erweitern, tragen die Marke nicht.
        let page = "<html lang=\"de\"><body>ohne Band</body></html>";
        assert_eq!(apply(page, &BandState::default(), &LaptopState::default()), page, "ohne Marke keine Änderung");
    }

    #[test]
    fn tabs_are_the_five_from_the_contract() {
        let ids: Vec<&str> = TABS.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, ["start", "einfuegen", "pruefen", "buch", "ansicht"], "Kennungen und Reihenfolge der Reiter (§2/§3.2)");
    }
}
