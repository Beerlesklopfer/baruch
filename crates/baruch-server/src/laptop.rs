// SPDX-License-Identifier: AGPL-3.0-or-later
//! Laptop-Modus (Runde 33, Schnittstelle R33-S1; Besitz: Aufgabe „laptop“).
//!
//! Zustand aus Adresse (`?laptop=auto|on|off`, `?lrail=0|1`, `?rrail=0|1`) und Cookies (`baruch_laptop`,
//! `baruch_lrail`, `baruch_rrail`, `baruch_vw`). Ein gültiger Schlüssel der Adresse gilt vor dem Cookie und wird
//! als `Set-Cookie` gemerkt. Die Schicht `consent::inject` schreibt den wirksamen Zustand als Attribute an
//! `<html>` (einzige Quelle für CSS und Widgets); `web/src/laptop.ts` hält sie aktuell.
//!
//! Ungültige Werte (unbekannter Modus, fremde Zeichen) werden übergangen, als stünden sie nicht da; steht ein
//! Schlüssel mehrfach in der Adresse, gilt der letzte gültige. In die Seite gelangen nur feste Zeichenketten
//! ([`LaptopState::html_attrs`]), nie ein Wert aus Adresse oder Cookie.
//!
//! Ohne JavaScript fehlt `baruch_vw`: Der Server kennt die Fensterbreite dann nicht, „automatisch“ zeichnet breit,
//! und der Mensch stellt den Laptop-Modus von Hand „an“.

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

impl Mode {
    /// Wert aus `?laptop=` (`auto` | `on` | `off`); alles andere ist ungültig.
    fn from_query(value: &str) -> Option<Mode> {
        match value {
            "auto" => Some(Mode::Auto),
            "on" => Some(Mode::On),
            "off" => Some(Mode::Off),
            _ => None,
        }
    }
}

impl Width {
    /// Wert des Cookies `baruch_vw`; alles andere ist ungültig.
    fn from_cookie(value: &str) -> Option<Width> {
        match value {
            "w" => Some(Width::Wide),
            "m" => Some(Width::Medium),
            "s" => Some(Width::Small),
            _ => None,
        }
    }
}

/// `0` | `1` aus `?lrail=` bzw. `?rrail=`; alles andere ist ungültig.
fn rail_from_query(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

/// Wert des Cookies `name` im Kopf `Cookie:` (`a=1; b=2`); steht es mehrfach da, gilt das erste. Kein Dekodieren:
/// gültige Werte bestehen nur aus Buchstaben und Ziffern.
fn cookie<'a>(header: Option<&'a str>, name: &str) -> Option<&'a str> {
    header?.split(';').find_map(|pair| {
        let (key, value) = pair.trim().split_once('=')?;
        (key == name).then_some(value.trim())
    })
}

/// Letzter gültiger Wert des Schlüssels `key` in der rohen Abfrage (`a=1&b=2`, nicht dekodiert – gültige Werte
/// enthalten nichts, was kodiert würde).
fn query_value<T>(query: Option<&str>, key: &str, parse: impl Fn(&str) -> Option<T>) -> Option<T> {
    query?
        .split('&')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            if name == key { parse(value) } else { None }
        })
        .next_back()
}

impl LaptopState {
    /// Zustand aus Cookie-Kopf und roher Abfrage der Anfrage: Ein gültiger Schlüssel der Adresse gilt vor dem
    /// Cookie; Ungültiges wird übergangen. Die Breite kommt nur aus dem Cookie `baruch_vw` (laptop.ts).
    pub fn from_request(cookie_header: Option<&str>, query: Option<&str>) -> LaptopState {
        let mode = query_value(query, "laptop", Mode::from_query).unwrap_or_else(|| {
            match cookie(cookie_header, "baruch_laptop") {
                Some("on") => Mode::On,
                Some("off") => Mode::Off,
                _ => Mode::Auto,
            }
        });
        let width = cookie(cookie_header, "baruch_vw").and_then(Width::from_cookie).unwrap_or_default();
        let rail = |key: &str, name: &str| {
            query_value(query, key, rail_from_query).unwrap_or_else(|| cookie(cookie_header, name) == Some("1"))
        };
        LaptopState { mode, width, lrail: rail("lrail", "baruch_lrail"), rrail: rail("rrail", "baruch_rrail") }
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
    /// Nur feste Zeichenketten – kein Wert aus Adresse oder Cookie gelangt in die Seite.
    pub fn html_attrs(&self) -> String {
        let mut attrs = format!("data-laptop=\"{}\"", self.mode.name());
        for (on, attr) in [
            (self.compact(), " data-compact"),
            (self.narrow(), " data-narrow"),
            (self.rail_left(), " data-rail-left"),
            (self.rail_right(), " data-rail-right"),
        ] {
            if on {
                attrs.push_str(attr);
            }
        }
        attrs
    }
}

/// Ein Jahr, wie die Cookies der Oberfläche (`store` in web/src/widgets.ts).
const COOKIE_MAX_AGE: u32 = 31_536_000;

fn keep(name: &str, value: &str) -> String {
    format!("{name}={value}; Path=/; Max-Age={COOKIE_MAX_AGE}; SameSite=Lax")
}

fn forget(name: &str) -> String {
    format!("{name}=; Path=/; Max-Age=0; SameSite=Lax")
}

/// Fertige Set-Cookie-Werte für gültige Schlüssel laptop/lrail/rrail in der Adresse.
///
/// `laptop=on|off` und `lrail=1`/`rrail=1` werden gemerkt; die Vorgaben (`laptop=auto`, `lrail=0`, `rrail=0`)
/// löschen das Cookie (`Max-Age=0`), denn „fehlt“ heißt automatisch bzw. sichtbar. Ungültiges liefert nichts.
/// Die Werte sind feste Zeichenketten, nie Text aus der Adresse.
pub fn set_cookies(query: Option<&str>) -> Vec<String> {
    let mut cookies = Vec::new();
    match query_value(query, "laptop", Mode::from_query) {
        Some(Mode::Auto) => cookies.push(forget("baruch_laptop")),
        Some(mode) => cookies.push(keep("baruch_laptop", mode.name())),
        None => {}
    }
    for (key, name) in [("lrail", "baruch_lrail"), ("rrail", "baruch_rrail")] {
        match query_value(query, key, rail_from_query) {
            Some(true) => cookies.push(keep(name, "1")),
            Some(false) => cookies.push(forget(name)),
            None => {}
        }
    }
    cookies
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
        assert!(
            state(Mode::On, Width::Wide).compact() && state(Mode::On, Width::Wide).narrow(),
            "„an“ wirkt auch breit"
        );
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

    #[test]
    fn the_request_is_read_from_all_four_cookies() {
        let state = LaptopState::from_request(
            Some("baruch_work=3; baruch_laptop=off; baruch_vw=s; baruch_lrail=1; baruch_rrail=1"),
            None,
        );
        assert_eq!(
            state,
            LaptopState { mode: Mode::Off, width: Width::Small, lrail: true, rrail: true },
            "Modus, Breitenklasse und beide Leisten stehen in je einem Cookie; fremde Cookies stören nicht"
        );
        assert_eq!(
            LaptopState::from_request(Some("baruch_laptop=on;baruch_vw=m"), None),
            LaptopState { mode: Mode::On, width: Width::Medium, lrail: false, rrail: false },
            "auch ohne Leerzeichen hinter dem Strichpunkt lesbar; fehlende Leisten-Cookies heißen „sichtbar“"
        );
        assert_eq!(
            LaptopState::from_request(None, None),
            LaptopState::default(),
            "ohne Cookies und Adresse gilt die Vorgabe: automatisch, breit, beide Spalten sichtbar"
        );
    }

    #[test]
    fn invalid_values_are_skipped() {
        // Werte aus Cookie und Adresse kommen vom Browser bzw. aus einem Link: Unbekanntes darf weder wirken noch
        // in die Seite gelangen.
        let state = LaptopState::from_request(
            Some("baruch_laptop=\"><b>; baruch_vw=x; baruch_lrail=ja; baruch_rrail=2"),
            Some("laptop=%22%3E%3Cscript%3E&lrail=2&rrail=on&laptop"),
        );
        assert_eq!(state, LaptopState::default(), "ungültige Cookies und Schlüssel gelten als nicht vorhanden");
        assert_eq!(state.html_attrs(), "data-laptop=\"auto\"", "nichts davon steht in den Attributen");
        assert_eq!(
            LaptopState::from_request(Some("baruch_laptop=auto"), None).mode,
            Mode::Auto,
            "„auto“ ist kein Cookie-Wert (fehlt = automatisch) und bleibt folgenlos"
        );
        assert_eq!(
            LaptopState::from_request(Some("xbaruch_laptop=on; baruch_laptop_alt=on"), None).mode,
            Mode::Auto,
            "nur der genaue Cookie-Name zählt, kein Name, der ihn bloß enthält"
        );
    }

    #[test]
    fn a_valid_key_in_the_address_wins_over_the_cookie() {
        let cookies = Some("baruch_laptop=off; baruch_lrail=1; baruch_rrail=1");
        let state = LaptopState::from_request(cookies, Some("work=3&laptop=on&lrail=0"));
        assert_eq!(state.mode, Mode::On, "?laptop=on gilt für diese Antwort vor dem Cookie „off“");
        assert!(!state.lrail, "?lrail=0 zeigt die Gliederung trotz Cookie wieder");
        assert!(state.rrail, "ohne Schlüssel in der Adresse bleibt es beim Cookie");
        assert_eq!(
            LaptopState::from_request(cookies, Some("laptop=auto")).mode,
            Mode::Auto,
            "?laptop=auto hebt ein gemerktes „aus“ in derselben Antwort auf"
        );
        assert_eq!(
            LaptopState::from_request(cookies, Some("laptop=quer&lrail=x")),
            LaptopState { mode: Mode::Off, width: Width::Wide, lrail: true, rrail: true },
            "ein ungültiger Schlüssel der Adresse verdrängt das Cookie nicht"
        );
        assert_eq!(
            LaptopState::from_request(None, Some("laptop=on&laptop=off")).mode,
            Mode::Off,
            "steht der Schlüssel mehrfach da, gilt der letzte gültige (das Band hängt ihn hinten an)"
        );
        assert_eq!(
            LaptopState::from_request(None, Some("vw=s&baruch_vw=s")).width,
            Width::Wide,
            "die Breite misst nur laptop.ts (Cookie); die Adresse kann sie nicht setzen"
        );
    }

    #[test]
    fn the_formula_holds_for_every_combination_of_mode_and_width() {
        // Vertrag §2, „Wirksamer Zustand“ – alle neun Kombinationen, je mit allen Stellungen der Leisten-Schalter.
        for mode in [Mode::Auto, Mode::On, Mode::Off] {
            for width in [Width::Wide, Width::Medium, Width::Small] {
                let compact = mode == Mode::On || (mode == Mode::Auto && width != Width::Wide);
                let narrow = mode == Mode::On || (mode == Mode::Auto && width == Width::Small);
                for (lrail, rrail) in [(false, false), (true, false), (false, true), (true, true)] {
                    let state = LaptopState { mode, width, lrail, rrail };
                    assert_eq!(state.compact(), compact, "compact für {state:?}: an, oder automatisch und nicht breit");
                    assert_eq!(state.narrow(), narrow, "narrow für {state:?}: an, oder automatisch und schmal");
                    assert_eq!(state.rail_left(), narrow || lrail, "rail_left für {state:?}: schmal oder eingeklappt");
                    assert_eq!(
                        state.rail_right(),
                        narrow || rrail,
                        "rail_right für {state:?}: schmal oder eingeklappt"
                    );
                }
            }
        }
    }

    #[test]
    fn html_attrs_keep_the_fixed_order_and_use_fixed_strings_only() {
        let attrs = |mode, width, lrail, rrail| LaptopState { mode, width, lrail, rrail }.html_attrs();
        assert_eq!(attrs(Mode::Auto, Width::Wide, false, false), "data-laptop=\"auto\"", "Vorgabe: nur der Modus");
        assert_eq!(
            attrs(Mode::On, Width::Wide, false, false),
            "data-laptop=\"on\" data-compact data-narrow data-rail-left data-rail-right",
            "„an“: alle Attribute in der Reihenfolge des Vertrags (band::apply und Tests vergleichen Zeichenketten)"
        );
        assert_eq!(
            attrs(Mode::Auto, Width::Medium, false, true),
            "data-laptop=\"auto\" data-compact data-rail-right",
            "mittel: kompakt, nicht schmal; nur die von Hand eingeklappte rechte Spalte"
        );
        assert_eq!(
            attrs(Mode::Off, Width::Small, true, false),
            "data-laptop=\"off\" data-rail-left",
            "„aus“ hält das Band voll; eine einzeln eingeklappte Leiste gilt trotzdem"
        );
        // Nur feste Zeichenketten: Jede Ausgabe besteht aus dem Modus und einer Teilmenge der vier Attribute.
        let allowed = ["data-compact", "data-narrow", "data-rail-left", "data-rail-right"];
        for mode in [Mode::Auto, Mode::On, Mode::Off] {
            for width in [Width::Wide, Width::Medium, Width::Small] {
                for (lrail, rrail) in [(false, false), (true, false), (false, true), (true, true)] {
                    let out = attrs(mode, width, lrail, rrail);
                    let mut parts = out.split(' ');
                    assert_eq!(
                        parts.next(),
                        Some(format!("data-laptop=\"{}\"", mode.name()).as_str()),
                        "data-laptop steht immer vorn: {out}"
                    );
                    let rest: Vec<&str> = parts.collect();
                    let expected: Vec<&str> = allowed.iter().copied().filter(|a| rest.contains(a)).collect();
                    assert_eq!(rest, expected, "nur bekannte Attribute, je höchstens einmal, feste Reihenfolge: {out}");
                }
            }
        }
    }

    #[test]
    fn set_cookies_remember_valid_keys_and_delete_the_defaults() {
        let year = "Path=/; Max-Age=31536000; SameSite=Lax";
        assert_eq!(
            set_cookies(Some("work=3&laptop=on")),
            [format!("baruch_laptop=on; {year}")],
            "?laptop=on wird ein Jahr gemerkt"
        );
        assert_eq!(set_cookies(Some("laptop=off")), [format!("baruch_laptop=off; {year}")], "?laptop=off ebenso");
        assert_eq!(
            set_cookies(Some("laptop=auto")),
            ["baruch_laptop=; Path=/; Max-Age=0; SameSite=Lax"],
            "„automatisch“ ist das fehlende Cookie: löschen statt merken"
        );
        assert_eq!(
            set_cookies(Some("lrail=1&rrail=1")),
            [format!("baruch_lrail=1; {year}"), format!("baruch_rrail=1; {year}")],
            "eingeklappte Leisten werden je für sich gemerkt"
        );
        assert_eq!(
            set_cookies(Some("lrail=0&rrail=0")),
            ["baruch_lrail=; Path=/; Max-Age=0; SameSite=Lax", "baruch_rrail=; Path=/; Max-Age=0; SameSite=Lax"],
            "sichtbar ist das fehlende Cookie: löschen"
        );
        for query in
            [None, Some(""), Some("laptop=an"), Some("laptop=%22%3E%3Cb%3E&lrail=2&rrail="), Some("work=3&tab=buch")]
        {
            assert!(
                set_cookies(query).is_empty(),
                "ungültige oder fremde Schlüssel setzen kein Cookie (nichts aus der Adresse landet im Kopf): {query:?}"
            );
        }
        for cookie in set_cookies(Some("laptop=on&lrail=1&rrail=0")) {
            assert!(
                cookie.bytes().all(|b| b.is_ascii_graphic() || b == b' '),
                "als HTTP-Kopfwert verwendbar (consent::inject hängt ihn mit HeaderValue::from_str an): {cookie}"
            );
        }
    }
}
