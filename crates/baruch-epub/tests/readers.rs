// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reader-Profile (Runde 20b, Besitz „reader“): eingefrorene Slugs in Vertragsreihenfolge, Pflicht-Profile,
//! Kindle über EPUB und je Slug eine CSS-Datei im Server.

use baruch_epub::readers::{PROFILES, profile, required};

const SLUGS: [&str; 9] = [
    "apple-books-iphone",
    "apple-books-ipad",
    "google-play-books",
    "kindle",
    "kindle-app",
    "tolino",
    "kobo",
    "thorium",
    "calibre",
];

#[test]
fn the_profiles_are_exactly_the_nine_frozen_slugs_in_contract_order() {
    let slugs: Vec<&str> = PROFILES.iter().map(|p| p.slug).collect();
    assert_eq!(slugs, SLUGS, "Slugs und Reihenfolge sind seit S1 eingefroren (Reihenfolge = Anzeige)");
}

#[test]
fn exactly_the_five_store_readers_are_required() {
    let required_slugs: Vec<&str> = required().map(|p| p.slug).collect();
    assert_eq!(
        required_slugs,
        SLUGS[..5],
        "Pflicht sind Apple Books (iPhone, iPad), Google Play Books, Kindle und Kindle-App – sonst keiner"
    );
    for p in PROFILES {
        assert_eq!(p.required, SLUGS[..5].contains(&p.slug), "{}: required wie im Vertrag", p.slug);
    }
}

#[test]
fn kindle_goes_through_send_to_kindle_and_says_so() {
    let kindle = profile("kindle").expect("kindle");
    let app = profile("kindle-app").expect("kindle-app");
    assert_eq!(kindle.name, "Kindle (über EPUB)", "Name nach Vertrag §0.4: kein eigenes AZW/KF8");
    assert_eq!(app.name, "Kindle-App (über EPUB)", "Name nach Vertrag");
    assert!(kindle.via_conversion && app.via_conversion, "beide gehen durch Send to Kindle (Umwandlung)");
    assert!(kindle.note.contains("Send to Kindle"), "der Hinweis nennt Send to Kindle: {}", kindle.note);
    for p in PROFILES.iter().filter(|p| !p.slug.starts_with("kindle")) {
        assert!(!p.via_conversion, "{}: nur Amazon wandelt das EPUB um", p.slug);
    }
    assert!(
        kindle.supports.overrides_publisher_font && !kindle.supports.css_backgrounds,
        "Kindle-Gerät: Bookerly statt Verlagsschrift, keine Hintergründe (E-Ink)"
    );
}

#[test]
fn profile_finds_every_slug_and_nothing_else() {
    for slug in SLUGS {
        assert_eq!(profile(slug).map(|p| p.slug), Some(slug), "{slug} wird gefunden");
    }
    for unknown in ["", "Kindle", "kindle ", "nook", "../kindle"] {
        assert!(profile(unknown).is_none(), "{unknown:?} ist kein Profil (Groß-/Kleinschreibung zählt)");
    }
}

#[test]
fn every_profile_has_real_values_instead_of_placeholders() {
    for p in PROFILES {
        assert!(!p.device.is_empty(), "{}: Gerät genannt", p.slug);
        assert!(!p.note.contains("noch nicht umgesetzt"), "{}: kein S1-Platzhalter mehr: {}", p.slug, p.note);
        assert!(
            (300..=1400).contains(&p.width_px) && (500..=1400).contains(&p.height_px),
            "{}: Maße in CSS-Pixeln eines Geräts ({} × {})",
            p.slug,
            p.width_px,
            p.height_px
        );
    }
    let iphone = profile("apple-books-iphone").unwrap();
    let ipad = profile("apple-books-ipad").unwrap();
    assert!(ipad.width_px > iphone.width_px, "das iPad ist breiter als das iPhone");
    assert!(
        !profile("tolino").unwrap().supports.popup_footnotes && !profile("kobo").unwrap().supports.popup_footnotes,
        "tolino und Kobo (gewöhnliches EPUB) springen zur Fußnote statt eines Popups (Annahme)"
    );
}

#[test]
fn every_slug_has_a_css_file_in_the_server() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-server/static/readers");
    for slug in SLUGS {
        let path = dir.join(format!("{slug}.css"));
        let css = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(css.contains(&format!("„{slug}“")), "{slug}.css nennt im Kopf seinen Reader");
    }
    let files = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(files, SLUGS.len(), "keine verwaiste CSS-Datei ohne Profil");
}

#[test]
fn profiles_serialize_with_all_fields_for_rpc() {
    let value = serde_json::to_value(PROFILES).unwrap();
    let first = &value[0];
    for field in ["slug", "name", "device", "width_px", "height_px", "required", "via_conversion", "supports", "note"] {
        assert!(!first[field].is_null(), "{field} gehört zur Antwort von epub.readers: {first}");
    }
    assert!(first["supports"]["popup_footnotes"].is_boolean(), "Fähigkeiten als Wahrheitswerte");
}
