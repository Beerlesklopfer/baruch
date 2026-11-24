// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reader-Profile der EPUB-Vorschau (Besitz „reader“, Runde 20b). Typen und Slugs seit S1 eingefroren
//! (Reihenfolge = Anzeige); die Werte sind Annahmen ohne Gerät (siehe unten und docs/Architektur.md).
//!
//! Hersteller-Reader von Android sind ohne Netz und Gerät nicht prüfbar: Annahme/Nutzerfrage in der Doku, kein
//! erfundenes Profil.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ReaderSupport {
    pub popup_footnotes: bool,
    pub embedded_fonts: bool,
    pub css_backgrounds: bool,
    pub page_break_css: bool,
    pub hyphenation: bool,
    pub svg_images: bool,
    pub night_mode: bool,
    pub overrides_publisher_font: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ReaderProfile {
    pub slug: &'static str,
    pub name: &'static str,
    pub device: &'static str,
    pub width_px: u32,
    pub height_px: u32,
    pub required: bool,
    /// Amazon: das EPUB geht durch Send to Kindle (Umwandlung).
    pub via_conversion: bool,
    pub supports: ReaderSupport,
    pub note: &'static str,
}

// Werte der Profile (Runde 20b). Alles ohne echtes Gerät und ohne Netz zusammengestellt: Maße sind CSS-Pixel
// gängiger Geräte (Hochformat), Fähigkeiten nach dem bekannten Verhalten der Leseprogramme beim Stand des
// Wissens der Entwickler – **Annahmen**, in docs/Architektur.md („R20b reader“) so vermerkt. Standardschrift,
// Größe, Ränder, Zeilenabstand, Silbentrennung und Nachtmodus bildet je Profil `static/readers/<slug>.css` im
// Server nach.

/// Apple Books: Fußnoten als Popup (`epub:type="noteref"`/`"footnote"`), eigene Schriften nur mit
/// `com.apple.ibooks.display-options.xml` sicher, Silbentrennung über `hyphens`, Nachtmodus.
const APPLE: ReaderSupport = ReaderSupport {
    popup_footnotes: true,
    embedded_fonts: true,
    css_backgrounds: true,
    page_break_css: true,
    hyphenation: true,
    svg_images: true,
    night_mode: true,
    overrides_publisher_font: false,
};

/// Google Play Books: Vorgabe „Original“ (Schrift des Verlags), Popup-Fußnoten, Nachtmodus; Silbentrennung
/// übernimmt der Reader nach eigener Einstellung (Annahme).
const PLAY: ReaderSupport = ReaderSupport {
    popup_footnotes: true,
    embedded_fonts: true,
    css_backgrounds: true,
    page_break_css: true,
    hyphenation: true,
    svg_images: true,
    night_mode: true,
    overrides_publisher_font: false,
};

/// Kindle-Gerät (E-Ink) nach Send to Kindle: Umwandlung, Bookerly statt Verlagsschrift (außer „Verlagsschrift“
/// gewählt), keine Hintergrundfarben, SVG unsicher, Popup-Fußnoten, dunkler Modus.
const KINDLE: ReaderSupport = ReaderSupport {
    popup_footnotes: true,
    embedded_fonts: true,
    css_backgrounds: false,
    page_break_css: true,
    hyphenation: true,
    svg_images: false,
    night_mode: true,
    overrides_publisher_font: true,
};

/// Kindle-App (Telefon) nach Send to Kindle: wie das Gerät, aber farbig; Hintergründe entfernt die Umwandlung
/// meist (Annahme).
const KINDLE_APP: ReaderSupport = KINDLE;

/// tolino (E-Ink): Fußnoten als Sprung, nicht als Popup (Annahme); Verlagsschrift, Silbentrennung, dunkler Modus
/// erst in neuer Firmware (Annahme).
const TOLINO: ReaderSupport = ReaderSupport {
    popup_footnotes: false,
    embedded_fonts: true,
    css_backgrounds: false,
    page_break_css: true,
    hyphenation: true,
    svg_images: true,
    night_mode: true,
    overrides_publisher_font: false,
};

/// Kobo (E-Ink) mit gewöhnlichem EPUB (nicht KEPUB): Fußnoten als Sprung, Verlagsschrift wählbar.
const KOBO: ReaderSupport = ReaderSupport {
    popup_footnotes: false,
    embedded_fonts: true,
    css_backgrounds: false,
    page_break_css: true,
    hyphenation: true,
    svg_images: true,
    night_mode: true,
    overrides_publisher_font: false,
};

/// Readium-Leser am Rechner (Thorium) bzw. calibre-Betrachter: vollständiges EPUB 3.
const DESKTOP: ReaderSupport = ReaderSupport {
    popup_footnotes: true,
    embedded_fonts: true,
    css_backgrounds: true,
    page_break_css: true,
    hyphenation: true,
    svg_images: true,
    night_mode: true,
    overrides_publisher_font: false,
};

/// Alle Profile in Anzeige-Reihenfolge: zuerst die Pflicht-Profile.
pub static PROFILES: &[ReaderProfile] = &[
    ReaderProfile {
        slug: "apple-books-iphone",
        name: "Apple Books (iPhone)",
        device: "iPhone mit 6,1-Zoll-Bildschirm",
        width_px: 390,
        height_px: 844,
        required: true,
        via_conversion: false,
        supports: APPLE,
        note: "Vorgabeschrift „Original“ (Schrift des Buchs, sonst Georgia), etwa 17 px, schmale Ränder, \
               Fußnoten als Popup. Eigene Schriften sicher nur mit com.apple.ibooks.display-options.xml (Annahme).",
    },
    ReaderProfile {
        slug: "apple-books-ipad",
        name: "Apple Books (iPad)",
        device: "iPad mit 10,9-Zoll-Bildschirm",
        width_px: 820,
        height_px: 1180,
        required: true,
        via_conversion: false,
        supports: APPLE,
        note: "Wie auf dem iPhone, größere Schrift und breite Ränder; im Querformat zwei Spalten (Annahme).",
    },
    ReaderProfile {
        slug: "google-play-books",
        name: "Google Play Books",
        device: "Android-Telefon mit 6,3-Zoll-Bildschirm",
        width_px: 412,
        height_px: 915,
        required: true,
        via_conversion: false,
        supports: PLAY,
        note: "Vorgabe „Original“ (Schrift des Buchs, sonst Literata), Fußnoten als Popup, Nachtmodus. \
               Hersteller-Reader auf Android sind ohne Gerät nicht geprüft (Annahme, Nutzerfrage).",
    },
    ReaderProfile {
        slug: "kindle",
        name: "Kindle (über EPUB)",
        device: "Kindle Paperwhite, 6,8 Zoll, E-Ink",
        width_px: 456,
        height_px: 608,
        required: true,
        via_conversion: true,
        supports: KINDLE,
        note: "Kein eigenes Kindle-Format: Send to Kindle nimmt EPUB an und wandelt es um. Vorgabeschrift \
               Bookerly (die Schrift des Buchs nur mit „Verlagsschrift“), Graustufen, keine Hintergrundfarben.",
    },
    ReaderProfile {
        slug: "kindle-app",
        name: "Kindle-App (über EPUB)",
        device: "Telefon mit Kindle-App, 6,1 Zoll",
        width_px: 390,
        height_px: 844,
        required: true,
        via_conversion: true,
        supports: KINDLE_APP,
        note: "Send to Kindle nimmt EPUB an und wandelt es um; in der App Bookerly, farbig, ohne \
               Hintergrundfarben (Annahme).",
    },
    ReaderProfile {
        slug: "tolino",
        name: "tolino",
        device: "tolino shine, 6 Zoll, E-Ink",
        width_px: 400,
        height_px: 540,
        required: false,
        via_conversion: false,
        supports: TOLINO,
        note: "Schrift des Buchs (sonst eine Serifenschrift des Geräts), Graustufen, Fußnoten als Sprung zum \
               Notentext statt Popup (Annahme).",
    },
    ReaderProfile {
        slug: "kobo",
        name: "Kobo",
        device: "Kobo Clara, 6 Zoll, E-Ink",
        width_px: 400,
        height_px: 540,
        required: false,
        via_conversion: false,
        supports: KOBO,
        note: "Gewöhnliches EPUB (nicht KEPUB): Fußnoten als Sprung, Schrift des Buchs wählbar, Graustufen \
               (Annahme).",
    },
    ReaderProfile {
        slug: "thorium",
        name: "Thorium Reader",
        device: "Rechner (Fenster)",
        width_px: 800,
        height_px: 1000,
        required: false,
        via_conversion: false,
        supports: DESKTOP,
        note: "Readium-Leser am Rechner: EPUB 3 vollständig, Fußnoten als Popup, Nachtmodus.",
    },
    ReaderProfile {
        slug: "calibre",
        name: "calibre (E-Book-Betrachter)",
        device: "Rechner (Fenster)",
        width_px: 800,
        height_px: 1000,
        required: false,
        via_conversion: false,
        supports: DESKTOP,
        note: "Betrachter von calibre: Schrift des Buchs, Fußnoten als Popup, dunkles Farbschema wählbar.",
    },
];

/// Die Pflicht-Profile (Vorgabe der Vorschau), in Anzeige-Reihenfolge.
pub fn required() -> impl Iterator<Item = &'static ReaderProfile> {
    PROFILES.iter().filter(|p| p.required)
}

/// Profil zu `slug`.
pub fn profile(slug: &str) -> Option<&'static ReaderProfile> {
    PROFILES.iter().find(|p| p.slug == slug)
}
