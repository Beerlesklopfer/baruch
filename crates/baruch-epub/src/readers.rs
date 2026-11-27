// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reader profiles of the EPUB preview (ownership "reader", Round 20b). Types and slugs frozen since S1 (order =
//! display); the values are assumptions without a device (see below and docs/Architektur.md).
//!
//! Vendor readers of Android cannot be checked without network and device: assumption/question to the user in
//! the documentation, no invented profile.

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
    /// Amazon: the EPUB goes through Send to Kindle (conversion).
    pub via_conversion: bool,
    pub supports: ReaderSupport,
    pub note: &'static str,
}

// Values of the profiles (Round 20b). Everything compiled without a real device and without network: dimensions
// are CSS pixels of common devices (portrait), capabilities according to the known behaviour of the reading
// programs to the best of the developers' knowledge – **assumptions**, noted as such in docs/Architektur.md
// ("R20b reader"). Default font, size, margins, line spacing, hyphenation and night mode are reproduced per
// profile by `static/readers/<slug>.css` in the server.

/// Apple Books: footnotes as a popup (`epub:type="noteref"`/`"footnote"`), own fonts only safe with
/// `com.apple.ibooks.display-options.xml`, hyphenation through `hyphens`, night mode.
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

/// Google Play Books: default "Original" (font of the publisher), popup footnotes, night mode; hyphenation is
/// done by the reader according to its own setting (assumption).
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

/// Kindle device (e-ink) after Send to Kindle: conversion, Bookerly instead of the publisher font (unless
/// "publisher font" is chosen), no background colours, SVG unsafe, popup footnotes, dark mode.
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

/// Kindle app (phone) after Send to Kindle: like the device, but in colour; the conversion mostly removes
/// backgrounds (assumption).
const KINDLE_APP: ReaderSupport = KINDLE;

/// tolino (e-ink): footnotes as a jump, not as a popup (assumption); publisher font, hyphenation, dark mode only
/// in new firmware (assumption).
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

/// Kobo (e-ink) with ordinary EPUB (not KEPUB): footnotes as a jump, publisher font selectable.
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

/// Readium reader on the computer (Thorium) or calibre viewer: complete EPUB 3.
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

/// All profiles in display order: the mandatory profiles first.
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

/// The mandatory profiles (default of the preview), in display order.
pub fn required() -> impl Iterator<Item = &'static ReaderProfile> {
    PROFILES.iter().filter(|p| p.required)
}

/// Profile for `slug`.
pub fn profile(slug: &str) -> Option<&'static ReaderProfile> {
    PROFILES.iter().find(|p| p.slug == slug)
}
