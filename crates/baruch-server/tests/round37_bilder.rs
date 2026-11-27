// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 37 „Hilfe auf der Webseite“, owner bilder: help link in the user interface and images of the help.
//!
//! The entry „Hilfe“ in the menu „Baruch ▾“ (contract §7), the line on „Über Baruch“ and the image list
//! `site/data/hilfebilder.toml` (contract §6). No network: the address of the help is only checked as text, never
//! fetched. Lektor and works come from the stage of `round35_rollen` (freely invented).

use axum::http::StatusCode;
use baruch_server::about;
use serde_json::json;

use crate::common;
use crate::round35_rollen::{get_as, stage};

/// The entry, verbatim according to contract §7.
const ENTRY: &str = "<a class=\"menu-item\" role=\"menuitem\" href=\"https://baruch.bernau.family/hilfe/\" \
                     target=\"_blank\" rel=\"noopener noreferrer\" title=\"Hilfe mit Bildern auf der Webseite von \
                     Baruch (öffnet ein neues Fenster, braucht das Internet)\">Hilfe</a>";

/// The entry „Über Baruch“, before which the help stands.
const ABOUT_ENTRY: &str = "<a class=\"menu-item\" role=\"menuitem\" href=\"/ueber\"";

/// The names of the image list from contract §6, in the order of the contract. The contract speaks of „95 Namen“, its
/// table (and the interface S1) has 94 – the table is authoritative.
const NAMES: [&str; 94] = [
    "start-werkliste",
    "start-neues-werk",
    "start-arbeitsplatz",
    "start-werk-bearbeiten",
    "start-archiv",
    "start-beenden",
    "start-ueber",
    "einlesen-bericht",
    "einlesen-pdf-epub",
    "einlesen-pdf-vorschau",
    "einlesen-word",
    "einlesen-word-vorschau",
    "band-start",
    "band-einfuegen",
    "band-pruefen",
    "band-buch",
    "band-ansicht",
    "band-menue-baruch",
    "band-eingeklappt",
    "band-kompakt",
    "band-laptop-arbeitsseite",
    "band-ohne-javascript",
    "gliederung-seite",
    "gliederung-leiste",
    "gliederung-bearbeiten",
    "gliederung-antraege",
    "gliederung-antrag-vorschau",
    "gliederung-schema",
    "schreiben-satzbild",
    "schreiben-quelltext",
    "schreiben-vorschau",
    "schreiben-fassungen",
    "schreiben-aeltere-fassung",
    "schreiben-fassungsverlauf",
    "schreiben-fassungen-ausblenden",
    "schreiben-vorschlag-lesefluss",
    "schreiben-bilder",
    "schreiben-bild-einfuegen",
    "schreiben-zeitwaechter",
    "assistenten-leiste",
    "assistenten-chat",
    "assistenten-beitraege",
    "assistenten-faeden",
    "assistenten-faden",
    "assistenten-befunde-liste",
    "assistenten-befunde-abschnitt",
    "assistenten-vorschlaege",
    "assistenten-websuche",
    "einfaelle-liste",
    "einfaelle-einfall",
    "einfaelle-festhalten",
    "mindmap-karte",
    "mindmap-eingeklappt",
    "mindmap-gliederung",
    "mindmap-einlesen",
    "mindmap-ohne-javascript",
    "bibel-einstellungen",
    "bibel-zitate-pruefen",
    "bibel-nachschlagen",
    "literatur-seite",
    "literatur-quelle",
    "abgleich-seite",
    "suchen-treffer",
    "suchen-ersetzen",
    "verweise-seite",
    "glossar-seite",
    "register-seite",
    "glossar-markierung",
    "satz-vorschau",
    "satz-einstellungen",
    "satz-buchangaben",
    "umschlag-editor",
    "epub-vorschau",
    "epub-pruefung",
    "satz-stichwortzettel",
    "satz-word-export",
    "austausch-ausgeben",
    "austausch-einlesen",
    "austausch-vorschau",
    "einstellungen-darstellung",
    "einstellungen-popup",
    "einstellungen-werk",
    "einstellungen-assistenten",
    "einstellungen-verbindungen",
    "einstellungen-quellen",
    "einstellungen-mcp",
    "einstellungen-daten",
    "konten-einrichten",
    "konten-anmelden",
    "konten-einstellungen",
    "konten-mein-konto",
    "konten-mitwirkende",
    "konten-lektor-arbeitsplatz",
    "installation-erster-start",
];

fn picture_list() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../site/data/hilfebilder.toml");
    std::fs::read_to_string(path).expect("site/data/hilfebilder.toml lesen")
}

/// The blocks `[[bild]]` of the list, each as lines (without comments and empty lines). Simple line search, no TOML
/// reader: the list writes every field on a line of its own.
fn blocks(list: &str) -> Vec<Vec<&str>> {
    let mut found: Vec<Vec<&str>> = Vec::new();
    for line in list.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if line == "[[bild]]" {
            found.push(Vec::new());
        } else if let Some(block) = found.last_mut() {
            block.push(line);
        }
    }
    found
}

fn is_field(line: &str, field: &str) -> bool {
    line.split('=').next().map(str::trim) == Some(field)
}

/// Value of a line `feld = "wert"` of the block (without the quotation marks).
fn text_field<'a>(block: &[&'a str], field: &str) -> Option<&'a str> {
    let line = block.iter().find(|l| is_field(l, field))?;
    let value = line.split_once('=')?.1.trim();
    value.strip_prefix('"')?.strip_suffix('"')
}

/// The menu „Baruch ▾“ of a page: from `data-menu="baruch"` to the closing `</details>`.
fn baruch_menu(html: &str) -> &str {
    let start = html.find("data-menu=\"baruch\"").expect("die Seite hat das Menü „Baruch ▾“");
    let end = html[start..].find("</details>").expect("das Menü ist ein details-Element") + start;
    &html[start..end]
}

/// Checks the entry in a page; `page` names it in the messages.
fn assert_help_entry(html: &str, page: &str) {
    let menu = baruch_menu(html);
    assert_eq!(
        menu.matches(ENTRY).count(),
        1,
        "{page}: der Eintrag „Hilfe“ steht genau einmal und wörtlich nach Vertrag §7 im Menü „Baruch ▾“"
    );
    let at = menu.find(ENTRY).unwrap();
    let after = &menu[at + ENTRY.len()..];
    let about_at = after.find(ABOUT_ENTRY).unwrap_or_else(|| panic!("{page}: hinter „Hilfe“ folgt „Über Baruch“"));
    assert!(
        after[..about_at].trim().is_empty(),
        "{page}: „Hilfe“ steht unmittelbar vor „Über Baruch“, dazwischen kein anderer Eintrag: {:?}",
        &after[..about_at]
    );
}

#[test]
fn the_help_address_is_the_help_section_of_the_homepage() {
    assert_eq!(
        about::HELP_URL,
        format!("{}hilfe/", about::HOMEPAGE),
        "der Eintrag „Hilfe“ führt auf den Abschnitt /hilfe/ der Webseite – zieht die Webseite um, zieht die Hilfe mit"
    );
}

#[test]
fn the_help_entry_is_an_ordinary_link_that_needs_no_javascript() {
    assert!(
        ENTRY.contains(&format!(" href=\"{}\"", about::HELP_URL)),
        "das Ziel ist about::HELP_URL – Vorlage (fester Text) und Konstante dürfen nicht auseinanderlaufen"
    );
    assert!(
        ENTRY.starts_with("<a ") && ENTRY.ends_with(">Hilfe</a>"),
        "ein gewöhnliches a-Element mit der Beschriftung „Hilfe“ (kein Knopf, kein Formular)"
    );
    assert!(
        !ENTRY.contains("nur-autor") && !ENTRY.contains("nur-lektor"),
        "der Eintrag trägt keine Rollen-Klasse: Autor und Lektoren sehen ihn"
    );
    assert!(
        !ENTRY.contains(" data-") && !ENTRY.contains(" on") && !ENTRY.contains("javascript:"),
        "kein data-Attribut, kein Ereignis-Attribut, keine javascript:-Adresse – der Browser folgt dem Link selbst"
    );
    assert!(
        ENTRY.contains("target=\"_blank\"") && ENTRY.contains("rel=\"noopener noreferrer\""),
        "neues Fenster; die Webseite erfährt die Adresse der Instanz nicht und bekommt keinen Zugriff auf ihr Fenster"
    );
    assert!(
        ENTRY.contains("braucht das Internet"),
        "der Hinweistext sagt ehrlich, dass der Rechner dafür das Internet braucht – Baruch selbst ruft nichts ab"
    );
}

#[tokio::test]
async fn the_help_entry_stands_in_the_menu_on_the_desk_and_on_pages_without_a_work() {
    let server = common::open().await;
    let works = server.rpc("work.list", json!({})).await;
    let work = works["result"]["works"][0]["id"].as_i64().expect("das Beispielwerk der neuen Instanz");

    let (status, desk) = server.get(&format!("/?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite des Beispielwerks kommt");
    assert_help_entry(&desk, "Arbeitsseite");

    for page in ["/werke", "/ueber", "/einstellungen/darstellung"] {
        let (status, html) = server.get(page).await;
        assert_eq!(status, StatusCode::OK, "{page} kommt ohne Angabe eines Werks");
        assert_help_entry(&html, page);
    }
}

#[tokio::test]
async fn a_lektor_sees_the_help_entry() {
    let s = stage().await;
    let (status, _, desk) = get_as(&s.server, &s.erika, &format!("/?work={}", s.granted)).await;
    assert_eq!(status, StatusCode::OK, "die Lektorin erreicht die Arbeitsseite des freigegebenen Werks");
    assert!(
        desk.contains("<body data-rolle=\"lektor\">"),
        "Vorbedingung: die Seite ist die Fassung für Lektoren (Rolle am body, konten.css blendet nur-autor aus)"
    );
    assert_help_entry(&desk, "Arbeitsseite der Lektorin");

    let (status, _, about_page) = get_as(&s.server, &s.erika, "/ueber").await;
    assert_eq!(status, StatusCode::OK, "„Über Baruch“ ist für Lektoren erlaubt");
    assert_help_entry(&about_page, "/ueber als Lektorin");
}

#[tokio::test]
async fn the_about_page_names_the_help_with_its_address() {
    let server = common::open().await;
    let (status, raw) = server.get("/ueber").await;
    assert_eq!(status, StatusCode::OK, "/ueber kommt");
    // The template escapes slashes in inserted values (&#x2f;); the browser reads them as „/“.
    let html = raw.replace("&#x2f;", "/");
    let link = format!("<a href=\"{0}\" rel=\"noopener noreferrer\" data-about-help>{0}</a>", about::HELP_URL);
    assert_eq!(
        html.matches(&link).count(),
        1,
        "„Über Baruch“ nennt die Hilfe einmal als gewöhnlichen Link mit der Adresse aus about::HELP_URL (Kontextwert help)"
    );
    let main = &html[html.find("<main").expect("die Seite hat einen Hauptteil")..];
    let homepage = main.find("<dt>Webseite</dt>").expect("die Zeile „Webseite“");
    let data = main.find("<dt>Datenordner</dt>").expect("die Zeile „Datenordner“");
    let help = main.find(&link).expect("der Link der Hilfe steht im Hauptteil");
    assert!(homepage < help && help < data, "die Hilfe steht unter der Webseite, vor dem Datenordner");
    assert!(
        main[homepage..data].contains("braucht das Internet"),
        "auch hier der ehrliche Hinweis: die Hilfe liegt auf der Webseite, nicht im Programm"
    );
}

#[test]
fn the_picture_list_has_unique_plain_names() {
    let list = picture_list();
    let blocks = blocks(&list);
    let mut seen = std::collections::BTreeSet::new();
    for block in &blocks {
        let name =
            text_field(block, "name").unwrap_or_else(|| panic!("jeder Block [[bild]] hat einen Namen: {block:?}"));
        assert!(
            !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{name}: nur Kleinbuchstaben, Ziffern und Bindestrich – der Name ist Dateiname und Teil der Adresse"
        );
        assert!(
            !name.ends_with("-dunkel"),
            "{name}: die Endung -dunkel gehört der dunklen Fassung (<name>-dunkel.png), kein eigener Name"
        );
        assert!(seen.insert(name), "{name}: jeder Name steht genau einmal – zwei Blöcke schrieben dieselbe Datei");
    }
    assert!(!blocks.is_empty(), "die Liste hat Blöcke [[bild]]");
}

#[test]
fn every_picture_names_its_page_what_it_shows_and_its_address() {
    let list = picture_list();
    for block in blocks(&list) {
        let name = text_field(&block, "name").expect("Name");
        assert!(
            block.iter().any(|l| is_field(l, "seite") && l.contains('"')),
            "{name}: seite fehlt – ohne sie weiß niemand, auf welche Hilfeseite das Bild gehört"
        );
        assert!(
            text_field(&block, "zeigt").is_some_and(|s| !s.trim().is_empty()),
            "{name}: zeigt fehlt – die Beschreibung des Motivs"
        );
        let address = text_field(&block, "adresse")
            .unwrap_or_else(|| panic!("{name}: adresse fehlt – das Werkzeug wüsste nicht, was es fotografieren soll"));
        assert!(
            address.starts_with('/'),
            "{name}: die Adresse ist ein Pfad am Wegwerf-Server (beginnt mit /), nie ein anderer Rechner: {address}"
        );
        assert!(
            !address.contains("://") && !address.contains("8765"),
            "{name}: keine vollständige Adresse und nie der Dienst des Nutzers: {address}"
        );
    }
}

#[test]
fn the_picture_list_keeps_every_name_of_the_contract() {
    let list = picture_list();
    let blocks = blocks(&list);
    let names: Vec<&str> = blocks.iter().filter_map(|b| text_field(b, "name")).collect();
    for name in NAMES {
        assert!(
            names.contains(&name),
            "{name}: der Name aus Vertrag §6 steht in der Liste – die Hilfeseiten verwenden ihn, ohne ihn bräche der \
             Bau der Webseite ab"
        );
    }
    for name in &names {
        assert!(
            NAMES.contains(name),
            "{name}: steht nicht in Vertrag §6 – neue Namen trägt nur der Integrator nach (dann auch hier)"
        );
    }
}

/// Integration Round 37: the screenshot of the page „Literatur“ showed the word „None“ in the search field –
/// `default('')` in minijinja replaces only missing values, not `none` (no search word).
#[tokio::test]
async fn the_literature_search_field_starts_empty() {
    let server = common::open().await;
    for path in ["/literatur?work=1", "/?work=1"] {
        let (status, html) = server.get(path).await;
        assert_eq!(status, StatusCode::OK, "{path} kommt");
        let field = html.find("<input type=\"search\" name=\"lq\"").expect("das Suchfeld der Literatur");
        let tag = &html[field..field + html[field..].find('>').expect("Ende des Felds")];
        assert!(
            tag.contains("value=\"\""),
            "{path}: ohne Suchwort ist das Feld leer – sonst sucht ein Klick auf „Suchen“ nach „None“: {tag}"
        );
    }
}
