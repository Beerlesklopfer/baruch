// SPDX-License-Identifier: AGPL-3.0-or-later
//! Laptop-Modus (Runde 33, Schnittstelle R33-S1; Besitz: Aufgabe „laptop“).
//!
//! Zustand aus Adresse (`?laptop=auto|on|off`, `?lrail=0|1`, `?rrail=0|1`) und Cookies (`baruch_laptop`,
//! `baruch_lrail`, `baruch_rrail`, `baruch_vw`). Ein gültiger Schlüssel der Adresse gilt vor dem Cookie und wird
//! als `Set-Cookie` gemerkt. Die Schicht `consent::inject` schreibt den wirksamen Zustand als Attribute an
//! `<html>` (einzige Quelle für CSS und Widgets); `web/src/laptop.ts` hält sie aktuell.
//!
//! Stand S1: Typen, Schwellen und die Formel des wirksamen Zustands stehen; Adresse und Cookies wertet erst die
//! Aufgabe „laptop“ aus ([`LaptopState::from_request`] liefert die Vorgabe, [`set_cookies`] nichts).

/// Unterhalb dieser Fensterbreite (`window.innerWidth`) ist das Band kompakt.
pub const COMPACT_BELOW_PX: u32 = 1600;
/// Unterhalb dieser Fensterbreite sind die Seitenleisten eingeklappt.
pub const NARROW_BELOW_PX: u32 = 1280;
/// Schlüssel der Adresse, die diesem Modul gehören.
pub const QUERY_KEYS: [&str; 3] = ["laptop", "lrail", "rrail"];

/// Einstellung des Menschen (`?laptop=`, Cookie `baruch_laptop`; fehlt = automatisch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Auto,
    On,
    Off,
}

/// Breitenklasse des Fensters (Cookie `baruch_vw`, nur von laptop.ts gesetzt; fehlt = breit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Width {
    #[default]
    Wide,
    Medium,
    Small,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LaptopState {
    pub mode: Mode,
    pub width: Width,
    /// Gliederung (linke Spalte) vom Menschen eingeklappt.
    pub lrail: bool,
    /// Rechte Spalte vom Menschen eingeklappt.
    pub rrail: bool,
}

impl Mode {
    /// `"auto"` | `"on"` | `"off"` – Wert in Adresse, Cookie und `data-laptop`.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Auto => "auto",
            Mode::On => "on",
            Mode::Off => "off",
        }
    }
}

impl Width {
    /// Breitenklasse nach den Schwellen [`COMPACT_BELOW_PX`] und [`NARROW_BELOW_PX`].
    pub fn from_px(px: u32) -> Width {
        if px < NARROW_BELOW_PX {
            Width::Small
        } else if px < COMPACT_BELOW_PX {
            Width::Medium
        } else {
            Width::Wide
        }
    }

    /// `"w"` | `"m"` | `"s"` – Wert des Cookies `baruch_vw`.
    pub fn cookie(self) -> &'static str {
        match self {
            Width::Wide => "w",
            Width::Medium => "m",
            Width::Small => "s",
        }
    }
}

impl LaptopState {
    /// Zustand aus Cookie-Kopf und roher Abfrage der Anfrage.
    // S1-Körper: Adresse und Cookies werden noch nicht ausgewertet, es gilt immer die Vorgabe – ersetzt die Aufgabe „laptop“.
    pub fn from_request(cookie_header: Option<&str>, query: Option<&str>) -> LaptopState {
        let _ = (cookie_header, query);
        LaptopState::default()
    }

    /// Band kompakt: „an“ oder automatisch bei mittlerer bzw. schmaler Breite.
    pub fn compact(&self) -> bool {
        self.mode == Mode::On || (self.mode == Mode::Auto && matches!(self.width, Width::Medium | Width::Small))
    }

    /// Seitenleisten eingeklappt: „an“ oder automatisch bei schmaler Breite.
    pub fn narrow(&self) -> bool {
        self.mode == Mode::On || (self.mode == Mode::Auto && self.width == Width::Small)
    }

    /// Gliederung eingeklappt.
    pub fn rail_left(&self) -> bool {
        self.narrow() || self.lrail
    }

    /// Rechte Spalte eingeklappt.
    pub fn rail_right(&self) -> bool {
        self.narrow() || self.rrail
    }

    /// z. B. `data-laptop="auto" data-compact data-rail-left` (ohne führendes Leerzeichen, feste Reihenfolge:
    /// data-laptop, data-compact, data-narrow, data-rail-left, data-rail-right)
    // S1-Körper: schreibt noch immer nur `data-laptop="auto"`, nicht den wirksamen Zustand – ersetzt die Aufgabe „laptop“.
    pub fn html_attrs(&self) -> String {
        "data-laptop=\"auto\"".to_owned()
    }
}

/// Fertige Set-Cookie-Werte für gültige Schlüssel laptop/lrail/rrail in der Adresse.
// S1-Körper: merkt noch nichts, liefert immer eine leere Liste – ersetzt die Aufgabe „laptop“.
pub fn set_cookies(query: Option<&str>) -> Vec<String> {
    let _ = query;
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_classes_follow_the_thresholds() {
        // Vertrag §2: w ab 1600 px, m von 1280 bis 1599, s darunter – die Grenzen selbst gehören zur breiteren Klasse.
        assert_eq!(Width::from_px(1600), Width::Wide, "1600 px ist nicht mehr „unter 1600“");
        assert_eq!(Width::from_px(1599), Width::Medium, "knapp unter der Schwelle wird das Band kompakt");
        assert_eq!(Width::from_px(1280), Width::Medium, "1280 px ist nicht mehr „unter 1280“");
        assert_eq!(Width::from_px(1279), Width::Small, "knapp unter der Schwelle klappen die Seitenleisten ein");
        assert_eq!(
            [Width::Wide.cookie(), Width::Medium.cookie(), Width::Small.cookie()],
            ["w", "m", "s"],
            "Cookie baruch_vw trägt genau diese Werte (laptop.ts schreibt sie)"
        );
    }

    #[test]
    fn effective_state_follows_mode_and_width() {
        let state = |mode, width| LaptopState { mode, width, lrail: false, rrail: false };
        // Automatisch: die Breite entscheidet.
        assert!(!state(Mode::Auto, Width::Wide).compact(), "breit und automatisch: volles Band");
        assert!(state(Mode::Auto, Width::Medium).compact(), "mittel und automatisch: Band kompakt");
        assert!(!state(Mode::Auto, Width::Medium).narrow(), "mittel: die Seitenleisten bleiben offen");
        assert!(state(Mode::Auto, Width::Small).narrow(), "schmal und automatisch: Seitenleisten eingeklappt");
        // „An“ und „Aus“ gelten unabhängig von der Breite (ohne JavaScript kennt der Server sie nicht).
        assert!(state(Mode::On, Width::Wide).compact() && state(Mode::On, Width::Wide).narrow(), "„an“ wirkt auch breit");
        assert!(
            !state(Mode::Off, Width::Small).compact() && !state(Mode::Off, Width::Small).narrow(),
            "„aus“ hält auch ein schmales Fenster im vollen Zustand"
        );
        // Einzeln eingeklappte Leisten gelten zusätzlich.
        let left = LaptopState { lrail: true, ..LaptopState::default() };
        assert!(left.rail_left() && !left.rail_right(), "nur die Gliederung ist eingeklappt");
        let narrow = state(Mode::On, Width::Wide);
        assert!(narrow.rail_left() && narrow.rail_right(), "schmal klappt beide Leisten ein");
        assert_eq!([Mode::Auto.name(), Mode::On.name(), Mode::Off.name()], ["auto", "on", "off"], "Werte von ?laptop=");
    }
}
