// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 37 „Hilfe auf der Webseite“, owner hilfe_text: the help pages under `site/content/de/hilfe/` and the entry
//! point `docs/Bedienung.md`.
//!
//! Since Round 37 the help is the only description of the operation for users (contract §1). These tests read only
//! files of the repository: no server, no network, no data directory, no Hugo. Whether the pages build is checked by
//! `hugo --minify --panicOnWarning -s site` (CI `scripts/ci-site.sh`); here is what Hugo does not notice – missing
//! pages, placeholders, headings without anchor or image, unused image names, dangling references, forbidden words.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// The 14 pages of the help with title and weight from the contract (§5); the entry point `_index.md` is added.
const PAGES: [(&str, u32, &str); 14] = [
    ("erste-schritte", 10, "Erste Schritte"),
    ("einlesen", 20, "Vorhandene Texte einlesen"),
    ("menueband", 30, "Menüband und Laptop-Modus"),
    ("gliederung", 40, "Gliederung"),
    ("schreiben", 50, "Schreiben, Bilder und Fassungen"),
    ("assistenten", 60, "Assistenten, Chat und Befunde"),
    ("einfaelle", 70, "Einfälle und Mindmap"),
    ("bibel-quellen", 80, "Bibel, Literatur und Quellen"),
    ("suchen", 90, "Suchen, Verweise, Register und Glossar"),
    ("buchsatz", 100, "Buchsatz: PDF, E-Book und Umschlag"),
    ("austausch-sicherung", 110, "Austausch, Sicherung und Sicherheit"),
    ("einstellungen", 120, "Einstellungen"),
    ("konten", 130, "Konten und Lektoren"),
    ("tasten", 140, "Tasten und Maus"),
];

/// Fixed anchors of the help (contract §5): the other pages of the website link to them.
const FIXED_ANCHORS: [(&str, &str); 19] = [
    ("erste-schritte", "starten"),
    ("erste-schritte", "beenden"),
    ("erste-schritte", "neues-werk"),
    ("einlesen", "word"),
    ("menueband", "laptop-modus"),
    ("assistenten", "chat"),
    ("assistenten", "befunde"),
    ("bibel-quellen", "bibeln-waehlen"),
    ("bibel-quellen", "quellen"),
    ("buchsatz", "pdf"),
    ("buchsatz", "epub"),
    ("buchsatz", "umschlag"),
    ("austausch-sicherung", "sicherung"),
    ("einstellungen", "verbindungen"),
    ("einstellungen", "assistenten"),
    ("einstellungen", "mcp-zugang"),
    ("einstellungen", "daten"),
    ("konten", "einrichten"),
    ("konten", "lektoren"),
];

/// Fixed anchors of the other pages (contract §5, guaranteed by the task „webseite“): only to them – or to the page
/// itself – may the help link.
const FOREIGN_TARGETS: [&str; 12] = [
    "/installation/",
    "/installation/#bibeln-einrichten",
    "/installation/#aktualisieren",
    "/mcp/",
    "/mcp/#assistenten-einrichten",
    "/mcp/#multigpt-einrichten",
    "/mcp/#mcp-clients",
    "/betrieb/",
    "/betrieb/#datenordner",
    "/betrieb/#sicherung",
    "/betrieb/#reverse-proxy",
    "/betrieb/#konten",
];

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn page(name: &str) -> String {
    read(&format!("site/content/de/hilfe/{name}.md"))
}

/// All pages of the help: the entry point and the 14 pages, as (name, text).
fn all_pages() -> Vec<(String, String)> {
    std::iter::once("_index").chain(PAGES.iter().map(|p| p.0)).map(|name| (name.to_string(), page(name))).collect()
}

/// Head (between the two `---`) and body of a page.
fn split(name: &str, text: &str) -> (String, String) {
    let rest = text.strip_prefix("---\n").unwrap_or_else(|| panic!("{name}.md beginnt mit dem Kopf (---)"));
    let (head, body) = rest.split_once("\n---\n").unwrap_or_else(|| panic!("{name}.md schließt den Kopf mit ---"));
    (head.to_string(), body.to_string())
}

/// Value of a head line `feld: "wert"` or `feld: wert`.
fn field(head: &str, name: &str) -> Option<String> {
    head.lines()
        .find_map(|line| line.strip_prefix(&format!("{name}:")).map(|value| value.trim().trim_matches('"').to_string()))
}

/// Lines of the body without the lines in code blocks (there `#` are comments of the shell, not headings).
fn prose_lines(body: &str) -> Vec<&str> {
    let mut code = false;
    let mut lines = Vec::new();
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            code = !code;
            continue;
        }
        if !code {
            lines.push(line);
        }
    }
    lines
}

/// Anchor of a heading line (`## Titel {#anker}`) if it carries one.
fn anchor(line: &str) -> Option<&str> {
    let start = line.rfind("{#")?;
    let anchor = line[start + 2..].strip_suffix('}')?;
    let fine = !anchor.is_empty() && anchor.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    fine.then_some(anchor)
}

/// All anchors of a page.
fn anchors(body: &str) -> BTreeSet<String> {
    prose_lines(body)
        .iter()
        .filter(|line| line.starts_with('#'))
        .filter_map(|line| anchor(line))
        .map(String::from)
        .collect()
}

/// Parameter `name="wert"` of a shortcode call.
fn param<'a>(call: &'a str, name: &str) -> Option<&'a str> {
    let start = call.find(&format!("{name}=\""))? + name.len() + 2;
    let end = call[start..].find('"')?;
    Some(&call[start..start + end])
}

/// All calls `{{< hilfebild … >}}` of a page (the text between the brackets).
fn image_calls(body: &str) -> Vec<&str> {
    let mut calls = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("{{< hilfebild") {
        let tail = &rest[start + 3..];
        let end =
            tail.find(">}}").unwrap_or_else(|| panic!("hilfebild-Aufruf ohne Ende: {}", &tail[..tail.len().min(80)]));
        calls.push(&tail[..end]);
        rest = &tail[end..];
    }
    calls
}

/// The image list `site/data/hilfebilder.toml`: name → pages. Simple line search (`name = "…"`, `seite = […]`), so that
/// own fields of the task „bilder“ do not interfere.
fn image_list() -> BTreeMap<String, Vec<String>> {
    let text = read("site/data/hilfebilder.toml");
    let mut list = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("name = \"") {
            let name = value.trim_end().trim_end_matches('"').to_string();
            list.insert(name.clone(), Vec::new());
            current = Some(name);
        } else if let Some(value) = line.strip_prefix("seite = [") {
            let name = current.clone().expect("in der Bildliste steht „seite“ hinter „name“");
            let pages = value
                .trim_end()
                .trim_end_matches(']')
                .split(',')
                .map(|p| p.trim().trim_matches('"').to_string())
                .filter(|p| !p.is_empty())
                .collect();
            list.insert(name, pages);
        }
    }
    list
}

/// Targets of all Markdown links `](ziel)` of a page. Code blocks and code in the text (between backticks) do not
/// count: there Markdown stands as an example, not as a reference.
fn link_targets(body: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in prose_lines(body) {
        // Every second piece between backticks is code.
        let text: String = line.split('`').step_by(2).collect::<Vec<_>>().join(" ");
        let mut rest = text.as_str();
        while let Some(start) = rest.find("](") {
            let tail = &rest[start + 2..];
            let end = tail.find(')').unwrap_or(tail.len());
            targets.push(tail[..end].to_string());
            rest = &tail[end..];
        }
    }
    targets
}

#[test]
fn the_help_section_takes_over_the_old_address() {
    let index = page("_index");
    assert!(
        index.contains("aliases: [\"/bedienung/\"]"),
        "alte Verweise auf /bedienung/ müssen weiter ankommen: die Hilfe leitet die Adresse per aliases um"
    );
    assert!(
        !root().join("site/content/de/bedienung.md").exists(),
        "neben dem Alias gäbe eine Seite bedienung.md zwei Ausgaben auf demselben Pfad – die alte Seite ist gelöscht"
    );
}

#[test]
fn all_fifteen_pages_exist_and_none_is_a_placeholder_any_more() {
    let on_disk: BTreeSet<String> = std::fs::read_dir(root().join("site/content/de/hilfe"))
        .expect("site/content/de/hilfe lesen")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let expected: BTreeSet<String> =
        std::iter::once("_index").chain(PAGES.iter().map(|p| p.0)).map(|name| format!("{name}.md")).collect();
    assert_eq!(
        on_disk, expected,
        "die Hilfe besteht aus dem Einstieg und genau den 14 Seiten des Vertrags (§5) – eine weitere Seite bräuchte \
         einen Eintrag in PAGES, eine fehlende ließe einen Menüpunkt ins Leere zeigen"
    );
    for (name, text) in all_pages() {
        let (_, body) = split(&name, &text);
        assert!(
            !text.contains("entsteht in Runde 37") && !text.contains("noch Platzhalter"),
            "{name}.md ist ausgeschrieben; der Platzhalter der Schnittstelle S1 ist ersetzt"
        );
        let least = if name == "_index" { 600 } else { 3000 };
        assert!(
            body.chars().count() >= least,
            "{name}.md beschreibt ihr Thema wirklich ({} Zeichen, mindestens {least}) – ein Satz wäre wieder ein \
             Platzhalter",
            body.chars().count()
        );
    }
    let index = page("_index");
    for (target, why) in [
        ("/hilfe/einlesen/", "Text hinein"),
        ("/hilfe/schreiben/#vorschlaege", "Vorschläge annehmen"),
        ("/hilfe/buchsatz/#pdf", "PDF heraus"),
    ] {
        assert!(
            index.contains(&format!("]({target})")),
            "der Einstieg nennt den schnellsten Weg mit Verweis ({why}: {target})"
        );
    }
}

#[test]
fn every_page_has_title_description_and_weight_from_the_contract() {
    let (head, _) = split("_index", &page("_index"));
    assert_eq!(field(&head, "title").as_deref(), Some("Hilfe"), "der Einstieg heißt wie der Menüeintrag „Hilfe“");
    assert!(
        field(&head, "description").is_some_and(|d| d.chars().count() >= 20),
        "der Einstieg hat eine Beschreibung (Vorspann der Seite und Text für Suchmaschinen)"
    );
    for (name, weight, title) in PAGES {
        let (head, _) = split(name, &page(name));
        assert_eq!(
            field(&head, "title").as_deref(),
            Some(title),
            "{name}.md trägt den Titel aus dem Vertrag (§5); die Liste der Seiten und fremde Verweise nennen ihn"
        );
        assert_eq!(
            field(&head, "weight"),
            Some(weight.to_string()),
            "{name}.md trägt das Gewicht aus dem Vertrag (§5); danach ordnet die Vorlage die Liste der Seiten"
        );
        let description = field(&head, "description").unwrap_or_default();
        assert!(
            description.chars().count() >= 30 && description.ends_with('.'),
            "{name}.md hat als description einen ganzen Satz (er steht in der Liste der Seiten hinter dem Titel): \
             „{description}“"
        );
    }
}

#[test]
fn every_help_image_names_a_listed_picture_and_describes_it() {
    let list = image_list();
    assert!(
        list.len() >= 90,
        "die Bildliste ist gelesen ({} Namen) – eine leere Liste ließe jeden Namen durch",
        list.len()
    );
    let mut calls = 0;
    for (name, text) in all_pages() {
        let (_, body) = split(&name, &text);
        for call in image_calls(&body) {
            calls += 1;
            let picture = param(call, "name").unwrap_or_else(|| panic!("{name}.md: hilfebild ohne name: {call}"));
            assert!(
                list.contains_key(picture),
                "{name}.md: der Bildname „{picture}“ steht in site/data/hilfebilder.toml (sonst bricht der Bau ab; \
                 neue Namen trägt nur der Integrator nach)"
            );
            let alt = param(call, "alt").unwrap_or_else(|| panic!("{name}.md: hilfebild „{picture}“ ohne alt"));
            assert!(
                alt.chars().count() >= 40,
                "{name}.md: der alt-Text von „{picture}“ beschreibt das Bild für Menschen, die es nicht sehen \
                 ({} Zeichen sind zu knapp): „{alt}“",
                alt.chars().count()
            );
            assert!(
                !alt.contains('<') && !alt.contains('>'),
                "{name}.md: der alt-Text von „{picture}“ enthält keine spitzen Klammern (er steht in einem Attribut)"
            );
            if let Some(titel) = param(call, "titel") {
                assert!(
                    !titel.trim().is_empty(),
                    "{name}.md: eine angegebene Bildunterschrift von „{picture}“ ist nicht leer"
                );
            }
        }
    }
    assert!(
        calls >= 94,
        "die Hilfe zeigt ihre Bilder ({calls} Aufrufe) – die Zählung hat die Aufrufe wirklich gefunden"
    );
}

#[test]
fn every_listed_picture_of_a_help_page_is_used_on_that_page() {
    let help: BTreeSet<&str> = PAGES.iter().map(|p| p.0).collect();
    let used: BTreeMap<String, BTreeSet<String>> = all_pages()
        .into_iter()
        .map(|(name, text)| {
            let (_, body) = split(&name, &text);
            let names = image_calls(&body).iter().filter_map(|call| param(call, "name")).map(String::from).collect();
            (name, names)
        })
        .collect();
    let mut checked = 0;
    for (picture, pages) in image_list() {
        for target in pages.iter().filter(|p| help.contains(p.as_str())) {
            checked += 1;
            assert!(
                used[target].contains(&picture),
                "die Bildliste nennt „{picture}“ für die Seite {target}; dort wird es auch gezeigt – sonst nähme \
                 der Integrator ein Foto auf, das niemand sieht"
            );
        }
    }
    assert!(checked >= 90, "die Zuordnung Bild → Hilfeseite ist gelesen ({checked} Paare)");
}

#[test]
fn every_heading_has_an_anchor_and_every_section_a_picture_or_an_exemption() {
    for (name, text) in all_pages() {
        let (head, body) = split(&name, &text);
        let exempt: BTreeSet<String> = field(&head, "ohne_bild")
            .map(|list| {
                list.trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .map(|a| a.trim().trim_matches('"').to_string())
                    .filter(|a| !a.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let lines = prose_lines(&body);
        let mut seen = BTreeSet::new();
        // Sections of the second level: (anchor, lines up to the next `## `).
        let mut sections: Vec<(String, Vec<&str>)> = Vec::new();
        for line in &lines {
            if line.starts_with('#') {
                assert!(
                    !line.starts_with("# "),
                    "{name}.md: der Seitentitel kommt aus dem Kopf; im Text beginnt die Gliederung mit ##: {line}"
                );
                let id = anchor(line).unwrap_or_else(|| {
                    panic!(
                        "{name}.md: jede Überschrift trägt einen festen Anker {{#kleinbuchstaben-ascii}}, damit \
                         Verweise eine Umformulierung überleben: {line}"
                    )
                });
                assert!(seen.insert(id.to_string()), "{name}.md: der Anker #{id} steht nur einmal auf der Seite");
            }
            if line.starts_with("## ") {
                sections.push((anchor(line).unwrap().to_string(), Vec::new()));
            } else if let Some(section) = sections.last_mut() {
                section.1.push(line);
            }
        }
        if name != "_index" {
            assert!(sections.len() >= 3, "{name}.md gliedert ihr Thema in Abschnitte ({} gefunden)", sections.len());
        }
        for id in &exempt {
            assert!(
                sections.iter().any(|(anchor, _)| anchor == id),
                "{name}.md: ohne_bild nennt nur Abschnitte, die es gibt (#{id})"
            );
        }
        for (id, content) in &sections {
            let pictured = content.iter().any(|line| line.contains("{{< hilfebild "));
            assert!(
                pictured || exempt.contains(id),
                "{name}.md: der Abschnitt #{id} zeigt mindestens ein Bild oder steht im Kopf unter ohne_bild (nur \
                 reine Tabellen- oder Verweisabschnitte)"
            );
        }
    }
}

#[test]
fn the_fixed_anchors_of_the_contract_exist() {
    for (name, id) in FIXED_ANCHORS {
        let (_, body) = split(name, &page(name));
        assert!(
            anchors(&body).contains(id),
            "/hilfe/{name}/#{id} ist ein fester Anker des Vertrags (§5); Installation, MCP und Betrieb verlinken \
             darauf"
        );
    }
}

#[test]
fn links_lead_to_existing_help_anchors_or_to_the_fixed_anchors_of_the_other_pages() {
    let known: BTreeMap<String, BTreeSet<String>> = PAGES
        .iter()
        .map(|p| {
            let (_, body) = split(p.0, &page(p.0));
            (p.0.to_string(), anchors(&body))
        })
        .collect();
    let mut links = 0;
    for (name, text) in all_pages() {
        let (_, body) = split(&name, &text);
        for target in &link_targets(&body) {
            links += 1;
            assert!(
                target.starts_with('/'),
                "{name}.md: Verweise sind gewöhnliche Links ab „/“ – keine fremden Adressen, kein relref: {target}"
            );
            if let Some(rest) = target.strip_prefix("/hilfe/") {
                let (page_name, id) = rest.split_once("/#").unwrap_or((rest.trim_end_matches('/'), ""));
                let ids = known
                    .get(page_name)
                    .unwrap_or_else(|| panic!("{name}.md verweist auf eine Hilfeseite, die es nicht gibt: {target}"));
                assert!(
                    rest.ends_with('/') || !id.is_empty(),
                    "{name}.md: die Adresse einer Seite endet mit „/“: {target}"
                );
                assert!(
                    id.is_empty() || ids.contains(id),
                    "{name}.md verweist auf einen Anker, den es auf der Zielseite nicht gibt: {target}"
                );
            } else {
                assert!(
                    FOREIGN_TARGETS.contains(&target.as_str()),
                    "{name}.md verweist außerhalb der Hilfe nur auf Installation, MCP oder Betrieb und dort nur auf \
                     die Seite oder einen festen Anker des Vertrags (§5): {target}"
                );
            }
        }
    }
    assert!(links >= 60, "die Seiten der Hilfe sind untereinander verbunden ({links} Verweise gefunden)");
}

#[test]
fn the_help_speaks_plainly_and_contains_nothing_forbidden() {
    // (search text, reason). The names of third parties are assembled so that this test does not contain them itself.
    let forbidden: Vec<(String, &str)> = vec![
        ("{{< doku".into(), "kein doku-Shortcode: die Hilfe verweist nicht ins Repository"),
        ("doku ".into(), "kein doku-Shortcode und kein Verweis auf „die Doku“ im Repository"),
        ("github.com".into(), "kein Link zu GitHub in der Hilfe"),
        ("<!--".into(), "kein HTML-Kommentar: er erzeugt in Hugo eine Warnung und bricht den Bau ab"),
        ("relref".into(), "Verweise nur als gewöhnliche Links ab „/“"),
        ("docs/".into(), "keine Dateinamen des Repositorys"),
        (".rs".into(), "keine Dateinamen des Quelltexts"),
        ("Revision".into(), "Alltagssprache: „Fassung“ statt „Revision“"),
        ("Akkordeon".into(), "Alltagssprache: „aufklappbarer Bereich“ statt „Akkordeon“"),
        ("Widget".into(), "Alltagssprache: kein „Widget“"),
        ("Loopback".into(), "Alltagssprache: „der eigene Rechner“ statt „Loopback“"),
        ("RPC".into(), "die Programmierschnittstelle gehört in die technischen Anmerkungen"),
        ("Cookie".into(), "Alltagssprache: „Baruch merkt sich …“ statt „Cookie“"),
        ("Commit".into(), "Alltagssprache: „Eintrag im Verlauf“ statt „Commit“"),
        ("Git-Spiegel".into(), "Alltagssprache: „Verlauf des Werks“ statt „Git-Spiegel“"),
        ("RAG".into(), "Alltagssprache: „Bedeutungssuche“ statt „RAG“"),
        ("Runde ".into(), "keine Rundennummern (die Hilfe beschreibt den Stand, nicht seine Geschichte)"),
        ("Leise Abhängigkeit".into(), "Beispiele nennen das erfundene Werk „Ein Jahr auf Mövenoog“"),
        (["gpt", "intranet"].join("."), "die frühere private Adresse steht nirgends"),
        (["WS-", "Joerg"].concat(), "kein Rechnername des Entwicklers"),
        (["Drid", "iger"].concat(), "kein Name eines Dritten"),
        (["Depen", "denz"].concat(), "kein Titel des Buchs eines Dritten"),
        (["Narz", "issmus"].concat(), "kein Untertitel des Buchs eines Dritten"),
    ];
    // Routes of the user interface as an operating instruction („`/werke/…` aufrufen“) do not belong in the help; only
    // the address of the program itself is typed in.
    let routes = [
        "/werke/",
        "/einstellungen/",
        "/gliederung",
        "/suche",
        "/literatur",
        "/einfaelle",
        "/quelle",
        "/faden",
        "/beitrag",
        "/abgleich",
        "/verweise",
        "/anmelden",
        "/einrichten",
        "/beenden",
        "/ueber",
        "/rpc",
        "/act",
        "?tab=",
        "?work=",
        "?band=",
        "?laptop=",
    ];
    for (name, text) in all_pages() {
        let (_, body) = split(&name, &text);
        for (needle, why) in &forbidden {
            assert!(!text.contains(needle.as_str()), "{name}.md enthält „{needle}“ nicht – {why}");
        }
        for line in prose_lines(&body) {
            assert!(
                !line.trim_start().starts_with('<') && !line.contains("</") && !line.contains("<br"),
                "{name}.md enthält kein rohes HTML (unsafe = false; Hugo ließe es weg und warnte): {line}"
            );
            // References starting with „/hilfe/…“, „/installation/…“, „/mcp/…“, „/betrieb/…“ are links, not routes.
            let without_links: String = {
                let mut rest = line;
                let mut out = String::new();
                while let Some(start) = rest.find("](") {
                    out.push_str(&rest[..start]);
                    let tail = &rest[start + 2..];
                    rest = tail.find(')').map_or("", |end| &tail[end + 1..]);
                }
                out.push_str(rest);
                out
            };
            // Code that begins with „/“ and a letter would be a route (`/werke`); `// Stichwort: …` is none.
            let coded_route = without_links
                .match_indices("`/")
                .any(|(at, _)| without_links[at + 2..].chars().next().is_some_and(|c| c.is_ascii_alphabetic()));
            assert!(
                !coded_route,
                "{name}.md nennt keine Route der Oberfläche als Code (`/…`) – Adressen stehen in den technischen \
                 Anmerkungen von docs/Bedienung.md: {line}"
            );
            for route in routes {
                assert!(
                    !without_links.contains(route),
                    "{name}.md nennt keine Route der Oberfläche („{route}“) – die Hilfe führt über Menü und Knöpfe, \
                     Adressen stehen in den technischen Anmerkungen von docs/Bedienung.md: {line}"
                );
            }
        }
    }
    let band = page("menueband");
    assert!(
        band.contains(
            "**Baruch ▾ → Hilfe** öffnet diese Hilfe auf der Webseite in einem neuen Fenster; dafür braucht der \
             Rechner das Internet."
        ),
        "menueband.md beschreibt den Eintrag „Hilfe“ mit dem Wortlaut des Vertrags (§7) – so passen Text und Foto \
         des offenen Menüs zusammen"
    );
}

#[test]
fn keys_are_given_for_both_worlds() {
    let keys = page("tasten");
    let (_, body) = split("tasten", &keys);
    let lines = prose_lines(&body);
    let headers: Vec<&&str> = lines.iter().filter(|line| line.starts_with("| Aktion |")).collect();
    assert!(headers.len() >= 8, "tasten.md sammelt die Tasten je Bereich in Tabellen ({} gefunden)", headers.len());
    for header in headers {
        assert_eq!(
            *header, "| Aktion | Windows/Linux | Mac |",
            "jede Tastentabelle hat die Spalten „Windows/Linux“ und „Mac“ (Vertrag §5)"
        );
    }
    assert!(lines.iter().any(|line| line.starts_with("| Geste |")), "tasten.md hat auch die Tabelle der Mausgesten");
    for (windows, mac) in [("Strg+S", "⌘S"), ("Alt+E", "⌥E"), ("Strg+Umschalt+F", "⌘⇧F"), ("Strg+0", "⌘0")] {
        assert!(
            lines.iter().any(|line| line.contains(&format!("| {windows} | {mac} |"))),
            "tasten.md nennt {windows} mit der Mac-Entsprechung {mac} (belegt im Code: ctrlKey || metaKey bzw. \
             event.code)"
        );
    }
    assert!(
        keys.contains("am Mac ebenfalls Ctrl+Mausrad") && keys.contains("ungeprüft"),
        "wo der Code nur Strg bzw. nur das erzeugte Zeichen kennt, steht das ehrlich da (Mausrad: nur ctrlKey; Alt+L \
         und Alt+, werten nur event.key aus)"
    );
    // On the other pages every Strg key is followed by the Mac key in the same line or the line after it.
    for (name, _, _) in PAGES.iter().filter(|p| p.0 != "tasten") {
        let (_, body) = split(name, &page(name));
        let lines = prose_lines(&body);
        for (number, line) in lines.iter().enumerate() {
            // Headings name the key only as the name of the section; the Mac key is in the text below.
            if !line.contains("Strg+") || line.starts_with('#') {
                continue;
            }
            let next = lines.get(number + 1).copied().unwrap_or("");
            let around = format!("{line} {next}");
            assert!(
                around.contains("Mac") || around.contains('⌘'),
                "{name}.md nennt zu einer Strg-Taste die Mac-Entsprechung (Schreibweise „Strg+S (Mac: ⌘S)“): {line}"
            );
        }
    }
}

#[test]
fn the_developer_guide_points_to_the_help_and_maps_every_old_section() {
    let guide = read("docs/Bedienung.md");
    assert!(
        guide.contains("https://baruch.bernau.family/hilfe/") && guide.contains("site/content/de/hilfe/"),
        "docs/Bedienung.md verweist auf die Hilfe – auf der Webseite und im Repository (Vertrag §1 a)"
    );
    for heading in ["\n## Regel für Entwickler\n", "\n## Wo steht was\n", "\n## Technische Anmerkungen\n"] {
        assert!(guide.contains(heading), "docs/Bedienung.md hat den Abschnitt {}", heading.trim());
    }
    assert!(
        guide.contains("im selben Commit ändern"),
        "die Regel für Entwickler sagt, was „Bedienung nachziehen“ jetzt heißt (Vertrag §1 c)"
    );
    assert!(
        guide.lines().count() < 1500,
        "docs/Bedienung.md ist geschrumpft ({} Zeilen): Verweis, Tabelle, Regel und technische Anmerkungen – keine \
         zweite Bedienungsanleitung",
        guide.lines().count()
    );
    assert!(
        !guide.contains("So geht es"),
        "Bedienschritte („So geht es“) stehen nur in der Hilfe, nie in beiden Dateien (Vertrag §1)"
    );
    assert_eq!(
        guide.matches("{{< hilfebild name=\"").count(),
        guide.matches("{{< hilfebild name=\"…\"").count(),
        "docs/Bedienung.md zeigt den Aufruf hilfebild nur als Muster (name=\"…\") in der Regel für Entwickler; \
         Bilder mit echtem Namen stehen nur in der Hilfe"
    );

    let table = guide.split("\n## Wo steht was\n").nth(1).unwrap().split("\n## ").next().unwrap();
    // Every `##` heading of the former file (state 00537dd) – verbatim, so that none is lost in the distribution.
    let old_sections = [
        "Starten",
        "Typst- oder Markdown-Datei importieren",
        "PDF oder EPUB importieren (Runde 21a)",
        "Werke",
        "Bildschirm",
        "Menüband",
        "Zeitangaben",
        "Adresse",
        "Gliederung",
        "Zeitwächter",
        "Satzbild",
        "Bilder",
        "Revisionen",
        "Vorschläge im Lesefluss",
        "Abschnitts-Chat",
        "Assistenten fragen",
        "Befunde (Runde 28)",
        "Chat bedienen und Aufträge (Runde 28, chat-auftrag)",
        "Suchen und Ersetzen (Runde 30a)",
        "Querverweise (Runde 30a)",
        "Lösungsvorschläge und Varianten (Runde 30b)",
        "Chat: Wiederholen, Löschen, Anrede, Live, gegliederte Beiträge, Sprungmarken (Runde 30c)",
        "Bibelstellen",
        "Satz (Typst-Vorschau)",
        "Buch als PDF, Umschlag und Titelei",
        "Stichwortzettel",
        "Literatur",
        "Einfälle",
        "Mindmap",
        "Fäden",
        "Abgleich mit MultiGPT",
        "Vergrößern mit Strg+Mausrad",
        "Einstellungen",
        "Sicherheit",
        "Konten und Rollen (Runde 35)",
        "Austausch zwischen Instanzen",
        "Umschlag-Editor (Runde 27)",
        "Runde 19 – RAG-Pflege, Reste der Stellen",
        "EPUB (Runde 20)",
        "Runde 31 – Register und Glossar",
        "Runde 22 – Revisionen und Spiegel",
        "Runde 23 – Härtung",
        "Runde 32 – Word und Beenden",
        "Runde 36 – Reste",
    ];
    let known: BTreeMap<String, BTreeSet<String>> = std::iter::once("_index")
        .chain(PAGES.iter().map(|p| p.0))
        .map(|name| {
            let (_, body) = split(name, &page(name));
            (name.to_string(), anchors(&body))
        })
        .collect();
    for section in old_sections {
        let row = table.lines().find(|line| line.starts_with(&format!("| {section} | "))).unwrap_or_else(|| {
            panic!("„Wo steht was“ führt den früheren Abschnitt „{section}“ (lückenlos, Vertrag §1 b)")
        });
        assert!(
            row.contains("(../site/content/de/hilfe/"),
            "der frühere Abschnitt „{section}“ führt zu mindestens einer Hilfeseite: {row}"
        );
    }
    // Every named page exists, and every named anchor is on the page that is named in the line before it.
    let mut pairs = 0;
    for row in table.lines().filter(|line| line.starts_with("| ")) {
        let mut current: Option<&str> = None;
        let mut rest = row;
        loop {
            let link = rest.find("(../site/content/de/hilfe/");
            let mark = rest.find("`#");
            match (link, mark) {
                (Some(l), m) if m.is_none_or(|m| l < m) => {
                    let tail = &rest[l + "(../site/content/de/hilfe/".len()..];
                    let end = tail
                        .find(".md)")
                        .unwrap_or_else(|| panic!("Verweis auf eine Hilfeseite endet mit .md): {row}"));
                    let name = &tail[..end];
                    assert!(known.contains_key(name), "„Wo steht was“ nennt nur Hilfeseiten, die es gibt: {name}.md");
                    current = Some(name);
                    rest = &tail[end..];
                }
                (_, Some(m)) => {
                    let tail = &rest[m + 2..];
                    let end = tail.find('`').unwrap_or(tail.len());
                    let id = &tail[..end];
                    if let Some(name) = current {
                        pairs += 1;
                        assert!(
                            known[name].contains(id),
                            "„Wo steht was“ nennt für {name}.md nur Anker, die es dort gibt: #{id}"
                        );
                    }
                    rest = &tail[end..];
                }
                _ => break,
            }
        }
    }
    assert!(pairs >= 100, "die Tabelle nennt die Abschnitte mit Anker ({pairs} Paare geprüft)");

    let notes = guide.split("\n## Technische Anmerkungen\n").nth(1).unwrap();
    for (name, _, title) in PAGES {
        assert!(
            notes.contains(&format!("\n### {title} (`{name}.md`)\n")),
            "die technischen Anmerkungen haben je Hilfeseite einen Abschnitt (Vertrag §1 d): {title}"
        );
    }
}
