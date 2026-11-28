// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39b „User interface German and English, outputs and help“, owner fotos_tool: screenshots in two languages, the
//! picture shortcodes and the site checks (contract §2.2, §2.5).
//!
//! The tests only read files of the repository: the picture list `site/data/hilfebilder.toml`, the shortcodes and the
//! partial of the theme, the i18n files, the screenshot tool `site/tools/bildschirmfotos.py`, its README and the two
//! sample works. They start neither Hugo nor Python nor a browser (the build checks with Hugo live in
//! `scripts/ci-selftest.sh`).

use std::collections::BTreeSet;

/// A file of the repository as text.
fn repo_file(path: &str) -> String {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{path} lesen: {e}"))
}

/// The blocks `[[bild]]` of the list, each as lines (without comments and empty lines). Simple line search like
/// `round37_bilder.rs`: the list writes every field on a line of its own.
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

/// Name of the field of a line `feld = …`.
fn field_name(line: &str) -> &str {
    line.split('=').next().map(str::trim).unwrap_or("")
}

/// Value of the field `field` in a block (text after the first `=`, trimmed), if the block has it.
fn value<'a>(block: &[&'a str], field: &str) -> Option<&'a str> {
    block.iter().find(|l| field_name(l) == field).and_then(|l| l.split_once('=')).map(|(_, v)| v.trim())
}

/// The strings of a TOML list value written on one line (`["a", "b"]`).
fn list_items(value: &str) -> Vec<&str> {
    value
        .trim()
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or_else(|| panic!("keine Liste auf einer Zeile: {value}"))
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.trim_matches('"'))
        .collect()
}

/// Fields of the list: those of Round 37 plus the additive fields of Round 39b (contract §2.5).
const KNOWN_FIELDS: [&str; 16] = [
    "name",
    "seite",
    "zeigt",
    "dunkel",
    "braucht",
    "adresse",
    "fenster",
    "thema",
    "ausschnitt",
    "schritt",
    "erwartet",
    "stufe",
    "sprachen",
    "adresse_en",
    "ausschnitt_en",
    "schritt_en",
];

#[test]
fn every_field_of_the_picture_list_is_known_and_the_new_ones_stay_additive() {
    let list = repo_file("site/data/hilfebilder.toml");
    let blocks = blocks(&list);
    assert!(!blocks.is_empty(), "die Liste hat Blöcke [[bild]] – sonst prüfte dieser Test nichts");
    for block in &blocks {
        let name = value(block, "name").unwrap_or("(ohne Namen)");
        for line in block {
            let field = field_name(line);
            assert!(
                KNOWN_FIELDS.contains(&field),
                "{name}: unbekanntes Feld „{field}“ – Vertrag §2.5 erlaubt nur die Felder der Runde 37 und \
                 sprachen/adresse_en/ausschnitt_en/schritt_en; ein Tippfehler bliebe sonst still wirkungslos"
            );
        }
        assert!(
            value(block, "adresse").is_some(),
            "{name}: adresse bleibt Pflicht – adresse_en ergänzt sie nur, sie ersetzt sie nicht (tests/round37_bilder.rs \
             liest adresse)"
        );
    }
}

#[test]
fn the_language_fields_of_the_picture_list_are_valid_for_both_languages() {
    let list = repo_file("site/data/hilfebilder.toml");
    let mut overrides = 0;
    for block in blocks(&list) {
        let name = value(&block, "name").unwrap_or("(ohne Namen)");
        if let Some(languages) = value(&block, "sprachen") {
            let items = list_items(languages);
            assert!(!items.is_empty(), "{name}: sprachen ist nicht leer – ein Bild ohne Sprache entstünde nie");
            for item in &items {
                assert!(
                    ["de", "en"].contains(item),
                    "{name}: sprachen nennt nur de und en (Sprachen der Webseite), nicht „{item}“"
                );
            }
        }
        if let Some(address) = value(&block, "adresse_en") {
            overrides += 1;
            let address = address.trim_matches('"');
            assert!(
                address.starts_with('/') && !address.contains("://") && !address.contains("8765"),
                "{name}: adresse_en ist wie adresse ein Pfad am Wegwerf-Server, nie ein anderer Rechner oder der \
                 Dienst des Nutzers: {address}"
            );
            assert_ne!(
                Some(format!("\"{address}\"").as_str()),
                value(&block, "adresse"),
                "{name}: adresse_en gleich adresse ist überflüssig – fehlt das Feld, gilt adresse ohnehin"
            );
        }
        if let Some(cut) = value(&block, "ausschnitt_en") {
            overrides += 1;
            let cut = cut.trim_matches('"');
            let fixed = cut.split(['x', '+']).count() == 4 && cut.split(['x', '+']).all(|n| n.parse::<u32>().is_ok());
            assert!(
                cut.starts_with("css:") || fixed,
                "{name}: ausschnitt_en hat die Formen von ausschnitt (BxH+X+Y oder css:SELEKTOR): {cut}"
            );
        }
        if let Some(steps) = value(&block, "schritt_en") {
            overrides += 1;
            for step in list_items(steps) {
                let kind = step.split(':').next().unwrap_or("");
                assert!(
                    ["cookie", "offen", "klick", "sitzung"].contains(&kind),
                    "{name}: schritt_en kennt nur die Schritte von schritt, nicht „{step}“"
                );
            }
        }
    }
    assert!(
        overrides >= 3,
        "die Bilder Suche, Ersetzen und Markierung tragen adresse_en – ihre Adressen enthalten Wörter des \
         Beispielwerks, die es im englischen Werk nicht gibt (gefunden: {overrides})"
    );
}

#[test]
fn the_list_header_and_the_tool_readme_document_the_new_fields() {
    let list = repo_file("site/data/hilfebilder.toml");
    let header: String = list.lines().take_while(|l| l.trim() != "[[bild]]").collect::<Vec<_>>().join("\n");
    let readme = repo_file("site/tools/README.md");
    for field in ["sprachen", "adresse_en", "ausschnitt_en", "schritt_en"] {
        assert!(
            header.contains(field),
            "der Kopf von hilfebilder.toml erklärt {field} – er ist die Beschreibung der Liste für alle Aufgaben"
        );
        assert!(readme.contains(&format!("`{field}`")), "site/tools/README.md erklärt {field} in der Feldtabelle");
    }
    assert!(
        readme.contains("--lang") && readme.contains("site/static/bilder/en"),
        "das README nennt --lang und den englischen Zielordner – sonst fotografierte der Integrator in den deutschen"
    );
}

#[test]
fn both_picture_shortcodes_choose_the_language_folder_and_stop_for_a_missing_picture() {
    for shortcode in ["hilfebild", "bild"] {
        let text = repo_file(&format!("site/themes/baruch/layouts/_shortcodes/{shortcode}.html"));
        assert!(
            text.contains("partial \"bildpfad.html\""),
            "{shortcode}: der Pfad kommt aus dem Partial bildpfad.html – nur so liegen englische Bilder unter \
             bilder/en/ und deutsche unverändert unter bilder/"
        );
        assert!(
            !text.contains("printf \"bilder/"),
            "{shortcode}: kein fest gebauter Pfad bilder/… neben dem Partial – er wäre auf englischen Seiten deutsch"
        );
        let missing = text.find("is missing").unwrap_or_else(|| {
            panic!(
                "{shortcode}: eine Meldung für ein fehlendes Bild – der offene Befund aus Runde 37 (still ausgelassen)"
            )
        });
        let line = text[..missing].rsplit('\n').next().unwrap_or("");
        assert!(
            line.contains("errorf") && line.contains("fileExists"),
            "{shortcode}: ein fehlendes helles Bild bricht den Bau mit errorf ab, geprüft mit fileExists – sonst fiele \
             ein fehlendes Foto erst dem Leser auf: {line}"
        );
    }
    let help = repo_file("site/themes/baruch/layouts/_shortcodes/hilfebild.html");
    assert!(
        help.contains("i18n \"hilfebild_gross\"") && !help.contains("Bild in voller Größe öffnen"),
        "hilfebild: der Titel des Links auf das große Bild kommt aus i18n hilfebild_gross, nicht als fester deutscher \
         Text (sonst stünde er deutsch auf englischen Seiten)"
    );
    let dark = help
        .lines()
        .find(|l| l.contains("$nacht") && l.contains("if "))
        .expect("hilfebild bindet die dunkle Fassung ($nacht) bedingt ein");
    assert!(
        dark.contains("fileExists") && !dark.contains("errorf"),
        "hilfebild: die dunkle Fassung bleibt freiwillig (nur mit fileExists eingebunden) – nur drei Bilder haben eine"
    );
}

#[test]
fn the_picture_path_partial_keeps_german_paths_and_gives_other_languages_a_folder() {
    let partial = repo_file("site/themes/baruch/layouts/_partials/bildpfad.html");
    assert!(
        partial.contains("printf \"bilder/%s\" $datei"),
        "Deutsch behält bilder/<datei> – die eingecheckten deutschen Bilder bleiben, wo sie sind"
    );
    assert!(
        partial.contains("printf \"bilder/%s/%s\" $lang $datei") && partial.contains("ne $lang \"de\""),
        "jede andere Sprache bekommt bilder/<lang>/<datei> (Vertrag §2.2)"
    );
    for lang in ["de", "en"] {
        let i18n = repo_file(&format!("site/themes/baruch/i18n/{lang}.toml"));
        assert!(
            i18n.contains("[hilfebild_gross]"),
            "i18n/{lang}.toml hat hilfebild_gross – ohne den Schlüssel bliebe der Titel leer bzw. bräche der Bau mit \
             --panicOnWarning ab"
        );
    }
}

#[test]
fn the_tool_has_both_languages_and_writes_english_pictures_to_their_own_folder() {
    let tool = repo_file("site/tools/bildschirmfotos.py");
    for (needle, reason) in [
        ("LANGS = (\"de\", \"en\")", "die beiden Sprachen der Webseite"),
        ("\"--lang\", choices=LANGS, default=DEFAULT_LANG", "--lang mit genau diesen Werten, Vorgabe de"),
        ("DEFAULT_LANG = \"de\"", "ohne Angabe bleibt alles wie bisher deutsch"),
        (
            "TARGET if lang == DEFAULT_LANG else TARGET / lang",
            "Englisch nach site/static/bilder/en, Deutsch unverändert",
        ),
        ("LANG_COOKIE = \"baruch_lang\"", "das Cookie der Oberflächensprache aus crates/baruch-server/src/i18n.rs"),
        ("intl.accept_languages", "Firefox schickt Accept-Language in der Sprache der Bilder"),
        ("\"Accept-Language\"", "die Anfragen des Aufbaus und des Vermittlers tragen die Sprache"),
        ("work.language_set", "die Werksprache des Beispielwerks folgt der Sprache der Bilder"),
        ("beispielwerk.en.typ", "für Englisch das englische Beispielwerk"),
        ("EXPECTED_EN", "die Texte aus erwartet bekommen für Englisch ihre Entsprechung"),
    ] {
        assert!(tool.contains(needle), "bildschirmfotos.py enthält {needle:?}: {reason}");
    }
    assert!(
        tool.contains("\"baruch_rrail\", \"baruch_session\", LANG_COOKIE,"),
        "baruch_lang ist ein verwaltetes Cookie des Vermittlers – sonst setzte ein altes Cookie im Browser die \
         falsche Sprache"
    );
}

/// The heading lines (`=`, `==`, `===` …) of a Typst file as their levels.
fn heading_levels(typst: &str) -> Vec<usize> {
    typst.lines().filter(|l| l.starts_with('=')).map(|l| l.chars().take_while(|c| *c == '=').count()).collect()
}

#[test]
fn the_english_sample_work_mirrors_the_german_one_and_contains_no_german_text() {
    let german = repo_file("site/tools/beispielwerk.typ");
    let english = repo_file("site/tools/beispielwerk.en.typ");
    assert_eq!(
        heading_levels(&english),
        heading_levels(&german),
        "das englische Beispielwerk hat dieselben Teile, Kapitel und Abschnitte – jedes Bild findet so in beiden \
         Sprachen dasselbe Motiv (Schlüssel, Fassungen, Befunde je Abschnitt)"
    );
    assert!(english.contains("#set text(lang: \"en\")"), "das englische Werk setzt die Satzsprache Englisch");
    for (index, line) in english.lines().enumerate().filter(|(_, l)| !l.starts_with("//")) {
        let german_marks: BTreeSet<char> = line.chars().filter(|c| "äöüÄÖÜß„“".contains(*c)).collect();
        assert!(
            german_marks.is_empty(),
            "beispielwerk.en.typ:{}: kein deutscher Text im englischen Werk (Vertrag: englische erfundene Daten) – \
             gefunden {german_marks:?} in: {line}",
            index + 1
        );
    }
    for feature in ["#footnote[", "#quote(block: true", "Psalm 119"] {
        assert!(
            english.contains(feature) == german.contains(feature),
            "{feature}: was das deutsche Werk für die Bilder mitbringt (Fußnote, Zitat, Bibelstelle), hat das \
             englische auch"
        );
    }
}
