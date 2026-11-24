// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 33 – Übrige Seiten (Aufgabe „seiten“, Vertrag §7): Vorgabe-Reiter und Gruppen je Seite, Satz-Seite ohne edit_toolbar.
//!
//! Jede Zusicherung mit Begründung, nur erfundene Werke (`tests/data/gartenbuch.typ` und die Texte hier), kein Netz,
//! kein Eliezer. Die Tests hängen nicht am Aussehen von `base.html`: Sie prüfen den Wert von `data-band-default`
//! (den base.html aus `{% block band_tab %}` schreibt) und die Gruppen der Seite in der wörtlichen Form des Vertrags
//! – gleich, ob base.html die Blöcke noch in der alten Kopfzeile (Schnittstelle S1) oder schon in den Feldern des
//! Menübands zeigt.

use crate::common;

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const HOUSE: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Eine Gruppe des Menübands: Name und Inhalt (zwischen `<div class="band-items" …>` und `</div></details>`).
struct Group<'a> {
    name: &'a str,
    /// Text zwischen `class="band-group"` und ` data-band-group open>` (weitere Attribute, sonst leer).
    extra: &'a str,
    inner: &'a str,
}

const GROUP_OPEN: &str = "<details class=\"band-group\"";
const GROUP_TAG_END: &str = " data-band-group open>";
const GROUP_END: &str = "</div></details>";

/// Alle Gruppen eines HTML-Texts in der **wörtlichen Form des Vertrags** (§4):
/// `<details class="band-group" data-band-group open><summary class="band-group-label">Name</summary><div
/// class="band-items" role="group" aria-label="Name">…</div></details>`. Weicht eine Gruppe davon ab, scheitert der
/// Test mit der Stelle – `band::apply` ersetzt Zeichenketten und fände eine abweichende Gruppe nicht.
fn groups<'a>(html: &'a str, at: &str) -> Vec<Group<'a>> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find(GROUP_OPEN) {
        let tag = &rest[start + GROUP_OPEN.len()..];
        let tag_end = tag.find('>').unwrap_or_else(|| panic!("{at}: Gruppe ohne Ende des Tags"));
        let open = &tag[..=tag_end];
        assert!(
            open.ends_with(GROUP_TAG_END),
            "{at}: das Tag der Gruppe endet wörtlich mit „ data-band-group open>“ (kompakt ersetzt band::apply genau \
             diese Zeichen): <details class=\"band-group\"{open}"
        );
        let extra = &open[..open.len() - GROUP_TAG_END.len()];
        let body = &tag[tag_end + 1..];
        let label = body.strip_prefix("<summary class=\"band-group-label\">").unwrap_or_else(|| {
            panic!("{at}: direkt hinter dem Tag steht die Beschriftung als <summary>: {:.160}", body)
        });
        let name_end = label.find("</summary>").unwrap_or_else(|| panic!("{at}: <summary> ohne Ende"));
        let name = &label[..name_end];
        let items = &label[name_end + "</summary>".len()..];
        let head = format!("<div class=\"band-items\" role=\"group\" aria-label=\"{name}\">");
        let inner = items.strip_prefix(head.as_str()).unwrap_or_else(|| {
            panic!(
                "{at}: Gruppe „{name}“: hinter der Beschriftung steht {head} (gleicher Name für Hilfsmittel): {:.200}",
                items
            )
        });
        let end = inner.find(GROUP_END).unwrap_or_else(|| panic!("{at}: Gruppe „{name}“ endet mit </div></details>"));
        out.push(Group { name, extra, inner: &inner[..end] });
        rest = &inner[end + GROUP_END.len()..];
    }
    out
}

/// Adressen (`href`, `action`) eines HTML-Stücks, lesbar gemacht (minijinja maskiert `/` in Werten als `&#x2f;`).
fn addresses(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for attr in ["href=\"", "action=\""] {
        for rest in html.split(attr).skip(1) {
            let raw = rest.split('"').next().unwrap_or_default();
            out.push(raw.replace("&#x2f;", "/").replace("&amp;", "&"));
        }
    }
    out
}

fn band_default(html: &str) -> &str {
    let marker = "data-band-default=\"";
    let at = html.find(marker).unwrap_or_else(|| panic!("data-band-default fehlt: {:.400}", html));
    html[at + marker.len()..].split('"').next().unwrap()
}

async fn keys(server: &common::Server, work: i64) -> Vec<(i64, String)> {
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    result(&answer)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["key"].as_i64().unwrap(), n["title"].as_str().unwrap().to_owned()))
        .collect()
}

async fn get_with_cookie(server: &common::Server, uri: &str, cookie: &str) -> (StatusCode, String) {
    let request = Request::get(uri).header(header::COOKIE, cookie).body(Body::empty()).unwrap();
    let (status, _, html) = server.send(request).await;
    (status, html)
}

/// Ein erfundenes Werk (Gartenbuch) mit allem, was die Seiten brauchen: Bild, Einfall, Beitrag, Variantengruppe.
struct Fixture {
    server: common::Server,
    work: i64,
    key: i64,
    asset: String,
    idea: i64,
    title: i64,
    post: i64,
    group: &'static str,
}

async fn fixture() -> Fixture {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let nodes = keys(&server, work).await;
    let key = nodes[1].0;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "Haus.svg", "content": HOUSE, "encoding": "svg",
                    "path": "/bilder/haus.svg", "caption": "Das Haus", "alt": "Ein Haus mit Dach" }),
        )
        .await;
    let asset = result(&added)["name"].as_str().unwrap().to_owned();
    let idea = server.rpc("idea.create", json!({ "work": work, "key": key, "words": "Laube", "note": "Rosen." })).await;
    let idea = result(&idea)["idea"]["id"].as_i64().unwrap();
    let list = server.rpc("library.list", json!(null)).await;
    let title = result(&list)["titles"][0]["id"].as_i64().unwrap();
    let seq = server.state.store.node_seq(work, key).await.unwrap();
    let note = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "Autor".into(),
        body: "Die Laube braucht Schatten.".into(),
        sources: vec![],
        refs: vec![],
    };
    let post = server.state.store.add_post(work, seq, &note).await.unwrap();
    // Zwei Geschwister-Vorschläge als Variantengruppe: /varianten zeigt nur eine vorhandene Gruppe.
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = &loaded.nodes[seq];
    let base = node.current_number().unwrap();
    let current = node.revision(base).unwrap().body.clone();
    let group = "5d1e7c2a-3b4f-4a66-8c0d-91e2f3a4b5c6";
    for (label, extra) in [("A", "Kurz gefasst."), ("B", "Anders gefasst.")] {
        let info = baruch_store::VariantInfo {
            group: group.into(),
            label: label.into(),
            summary: format!("Variante {label}"),
            findings: vec![],
        };
        let body = format!("{}\n{extra}\n", current.trim_end());
        server.state.store.add_proposal_on(work, seq, &body, "@lektor", "m", "", base, Some(&info)).await.unwrap();
    }
    Fixture { server, work, key, asset, idea, title, post, group }
}

impl Fixture {
    async fn page(&self, uri: &str) -> String {
        let (status, html) = get_with_cookie(&self.server, uri, &format!("baruch_work={}", self.work)).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
        html
    }
}

/// Erwartung an eine ausgelieferte Seite: Vorgabe-Reiter und je Gruppe die Adressen, die vor dem Umbau (Stand
/// ed9d210) in `page_actions` der Seite standen. Eine Adresse mit `*` am Ende ist ein Anfang (der Rest hängt am
/// Werk, z. B. der erste Abschnitt).
struct Expect {
    name: &'static str,
    uri: String,
    tab: &'static str,
    groups: Vec<(&'static str, Vec<String>)>,
    why: &'static str,
}

fn matches_address(found: &str, expected: &str) -> bool {
    match expected.strip_suffix('*') {
        Some(prefix) => found.starts_with(prefix),
        None => found == expected,
    }
}

fn expectations(f: &Fixture) -> Vec<Expect> {
    let (w, k) = (f.work, f.key);
    let back = |addr: String| vec![("Zurück", vec![addr])];
    let work = format!("/?work={w}");
    let section = format!("/?work={w}&key={k}");
    let literature = format!("/literatur?work={w}&key={k}");
    vec![
        Expect {
            name: "Werke",
            uri: "/werke".into(),
            tab: "start",
            groups: vec![
                ("Werke", vec!["/werke/neu".into(), "/werke/archiv".into()]),
                ("Zurück", vec!["/?work=*".into()]),
            ],
            why: "die Werkliste erreicht man über Baruch ▾ – kein Werk-Reiter, also Start; Anlegen und Archiv sind ihre Aktionen",
        },
        Expect {
            name: "Neues Werk",
            uri: "/werke/neu".into(),
            tab: "start",
            groups: vec![("Zurück", vec!["/werke".into(), "/?work=*".into()])],
            why: "Neues Werk steht in Baruch ▾ (kein Reiter), darum Start",
        },
        Expect {
            name: "Archiv der gelöschten Werke",
            uri: "/werke/archiv".into(),
            tab: "start",
            groups: vec![("Zurück", vec!["/werke".into(), "/?work=*".into()])],
            why: "gehört zur Werkliste, darum Start",
        },
        Expect {
            name: "PDF oder EPUB importieren",
            uri: "/werke/import/quelle".into(),
            tab: "start",
            groups: vec![("Zurück", vec!["/werke".into(), "/?work=*".into()])],
            why: "der Import steht in Baruch ▾, darum Start",
        },
        Expect {
            name: "Werk löschen",
            uri: format!("/werke/{w}/loeschen"),
            tab: "start",
            groups: vec![("Zurück", vec!["/werke".into(), work.clone()])],
            why: "Löschen geht von der Werkliste aus, darum Start",
        },
        Expect {
            name: "Werk bearbeiten",
            uri: format!("/werke/{w}/bearbeiten"),
            tab: "buch",
            groups: vec![("Zurück", vec!["/werke".into(), work.clone()])],
            why: "Vertrag §7: Werk bearbeiten → Buch (Gruppe Werk)",
        },
        Expect {
            name: "Gliederung",
            uri: format!("/gliederung?work={w}"),
            tab: "start",
            groups: vec![("Zurück", vec![format!("/?work={w}&key=*")])],
            why: "Vertrag §7 nennt die Gliederung nicht unter Buch („sonst start“)",
        },
        Expect {
            name: "Gliederungspunkt bearbeiten",
            uri: format!("/gliederung/bearbeiten?work={w}&key={k}"),
            tab: "start",
            groups: vec![("Zurück", vec![format!("/gliederung?work={w}&key={k}"), section.clone()])],
            why: "Unterseite der Gliederung, wie diese Start",
        },
        Expect {
            name: "Gliederungsschema",
            uri: format!("/werke/{w}/nummerierung"),
            tab: "buch",
            groups: vec![("Siehe auch", vec![format!("/werke/{w}/bearbeiten")]), ("Zurück", vec![work.clone()])],
            why: "Vertrag §7: Gliederungsschema → Buch",
        },
        Expect {
            name: "Bilder",
            uri: format!("/werke/{w}/bilder"),
            tab: "buch",
            groups: back(work.clone()),
            why: "Vertrag §7: Bilder → Buch",
        },
        Expect {
            name: "Bild",
            uri: format!("/werke/{w}/bilder/{}", f.asset),
            tab: "buch",
            groups: vec![("Zurück", vec![format!("/werke/{w}/bilder"), work.clone()])],
            why: "Unterseite der Bilder, wie diese Buch",
        },
        Expect {
            name: "Befunde",
            uri: format!("/werke/{w}/befunde"),
            tab: "pruefen",
            groups: back(work.clone()),
            why: "Vertrag §7: Befunde → Prüfen",
        },
        Expect {
            name: "Prüfung (Zitate prüfen)",
            uri: format!("/werke/{w}/pruefung?key={k}"),
            tab: "pruefen",
            groups: back(format!("/?work={w}&key={k}&rev=1")),
            why: "Vertrag §7: Prüfung → Prüfen",
        },
        Expect {
            name: "Suche",
            uri: format!("/suche?work={w}&q=Laube"),
            tab: "pruefen",
            groups: back(work.clone()),
            why: "Vertrag §7: Suche → Prüfen",
        },
        Expect {
            name: "Verweise",
            uri: format!("/verweise?work={w}"),
            tab: "pruefen",
            groups: back(work.clone()),
            why: "Vertrag §7: Verweise → Prüfen",
        },
        Expect {
            name: "Literatur",
            uri: format!("/literatur?work={w}&key={k}"),
            tab: "pruefen",
            groups: back(section.clone()),
            why: "Vertrag §7: Literatur → Prüfen",
        },
        Expect {
            name: "Quelle",
            uri: format!("/quelle?title={}&work={w}&key={k}", f.title),
            tab: "pruefen",
            groups: vec![("Zurück", vec![literature.clone(), section.clone()])],
            why: "Unterseite der Literatur, wie diese Prüfen",
        },
        Expect {
            name: "Volltext aus MultiGPT",
            uri: format!("/quelle/multigpt?work={w}&key={k}&collection=Nachbau&document=doc-1"),
            tab: "pruefen",
            groups: vec![("Zurück", vec![literature.clone(), section.clone()])],
            why: "Unterseite der Literatur, wie diese Prüfen (ohne Verbindung ein ehrlicher Hinweis, kein Netz)",
        },
        Expect {
            name: "Bibel der Literatur",
            uri: format!("/literatur/bibel?work={w}&key={k}"),
            tab: "pruefen",
            groups: vec![("Zurück", vec![literature.clone(), section.clone()])],
            why: "Unterseite der Literatur, wie diese Prüfen",
        },
        Expect {
            name: "Einfälle",
            uri: format!("/einfaelle?work={w}"),
            tab: "start",
            groups: vec![("Siehe auch", vec![format!("/werke/{w}/karte")]), ("Zurück", vec![work.clone()])],
            why: "Einfälle stehen im Reiter Start (Gruppe Einfälle)",
        },
        Expect {
            name: "Einfall",
            uri: format!("/einfaelle/{}", f.idea),
            tab: "start",
            groups: vec![("Zurück", vec![format!("/einfaelle?work={w}"), section.clone()])],
            why: "Unterseite der Einfälle, wie diese Start",
        },
        Expect {
            name: "Gedankenkarte",
            uri: format!("/werke/{w}/karte"),
            tab: "start",
            groups: vec![("Zurück", vec![format!("/einfaelle?work={w}"), work.clone()])],
            why: "die Gedankenkarte steht im Reiter Start (Gruppe Einfälle)",
        },
        Expect {
            name: "Fäden",
            uri: format!("/faeden?work={w}"),
            tab: "start",
            groups: back(work.clone()),
            why: "Fäden gehören zum Chat der Arbeitsseite, kein eigener Reiter – Start",
        },
        Expect {
            name: "Abgleich",
            uri: format!("/abgleich?work={w}"),
            tab: "pruefen",
            groups: back(work.clone()),
            why: "Vertrag §7: Abgleich → Prüfen",
        },
        Expect {
            name: "Umschlag",
            uri: format!("/werke/{w}/umschlag"),
            tab: "buch",
            groups: vec![("Herunterladen", vec![format!("/werke/{w}/umschlag.pdf")]), ("Zurück", vec![work.clone()])],
            why: "Vertrag §7: Umschlag → Buch",
        },
        Expect {
            name: "EPUB-Vorschau",
            uri: format!("/werke/{w}/epub/vorschau"),
            tab: "buch",
            groups: vec![("EPUB", vec![format!("/werke/{w}/epub")]), ("Zurück", vec![work.clone()])],
            why: "Vertrag §7: EPUB → Buch",
        },
        Expect {
            name: "EPUB-Prüfung",
            uri: format!("/werke/{w}/epub/pruefung"),
            tab: "buch",
            groups: vec![
                ("EPUB", vec![format!("/werke/{w}/epub"), format!("/werke/{w}/epub/vorschau")]),
                ("Zurück", vec![work.clone()]),
            ],
            why: "Vertrag §7: EPUB → Buch",
        },
        Expect {
            name: "Austausch: Ausgeben",
            uri: format!("/werke/{w}/austausch"),
            tab: "buch",
            groups: back(work.clone()),
            why: "Vertrag §7: Export/Austausch → Buch",
        },
        Expect {
            name: "Austausch: Einlesen",
            uri: "/austausch/einlesen".into(),
            tab: "buch",
            groups: back("/werke".into()),
            why: "Vertrag §7: Austausch → Buch (ohne ?work= nur „Werke“ als Rückweg)",
        },
        Expect {
            name: "Glossar",
            uri: format!("/werke/{w}/glossar"),
            tab: "buch",
            groups: vec![("Siehe auch", vec![format!("/werke/{w}/register")]), ("Zurück", vec![work.clone()])],
            why: "Vertrag §7: Glossar → Buch",
        },
        Expect {
            name: "Register",
            uri: format!("/werke/{w}/register"),
            tab: "buch",
            groups: vec![("Siehe auch", vec![format!("/werke/{w}/glossar")]), ("Zurück", vec![work.clone()])],
            why: "Vertrag §7: Register → Buch",
        },
        Expect {
            name: "Stichwortzettel",
            uri: format!("/werke/{w}/stichwortzettel"),
            tab: "start",
            groups: back(work.clone()),
            why: "Vertrag §7 nennt den Stichwortzettel nicht unter Buch („sonst start“)",
        },
        Expect {
            name: "An der Markierung",
            uri: format!("/werke/{w}/markierung?key={k}"),
            tab: "start",
            groups: back(format!("/?work={w}&key={k}&rev=1")),
            why: "Rückfall des Kontextmenüs ohne JavaScript, gehört zum Editor – Start",
        },
        Expect {
            name: "Einstellungen",
            uri: format!("/einstellungen/darstellung?work={w}"),
            tab: "ansicht",
            groups: back(work.clone()),
            why: "Vertrag §7: Einstellungen → Ansicht",
        },
        Expect {
            name: "Belege der Websuche",
            uri: format!("/websuche/belege?work={w}&key={k}&post=1"),
            tab: "start",
            groups: back(section.clone()),
            why: "Belege eines Beitrags im Chat – Start",
        },
        Expect {
            name: "Chat-Historie",
            uri: format!("/beitraege?work={w}&key={k}"),
            tab: "start",
            groups: back(section.clone()),
            why: "Chat des Abschnitts – Start",
        },
        Expect {
            name: "Beitrag",
            uri: format!("/beitrag?work={w}&id={}", f.post),
            tab: "start",
            groups: vec![("Zurück", vec![format!("/beitraege?work={w}&key={k}#post-{}", f.post), section.clone()])],
            why: "Chat des Abschnitts – Start",
        },
        Expect {
            name: "Varianten",
            uri: format!("/varianten?work={w}&key={k}&group={}", f.group),
            tab: "start",
            groups: back(section.clone()),
            why: "Vorschläge des Abschnitts – Start",
        },
        Expect {
            name: "Verlauf einer Fassung",
            uri: format!("/werke/{w}/fassungen/verlauf?key={k}&rev=1"),
            tab: "start",
            groups: back(format!("/?work={w}&key={k}&rev=1")),
            why: "Fassungen des Abschnitts – Start",
        },
        Expect {
            name: "Über Baruch",
            uri: "/ueber".into(),
            tab: "start",
            groups: back(work.clone()),
            why: "Über Baruch steht in Baruch ▾ (kein Reiter), darum Start",
        },
    ]
}

#[tokio::test]
async fn every_page_names_its_default_tab_and_keeps_its_actions_in_groups() {
    let f = fixture().await;
    let pages = expectations(&f);
    // Die Liste der Abnahme (Vertrag „seiten“): jede dort genannte Seite kommt vor (die Satz-Seite hat ihren
    // eigenen Test unten, Export ist eine Datei und keine Seite).
    for wanted in [
        "Werke",
        "Neues Werk",
        "Werk bearbeiten",
        "Gliederung",
        "Gliederungsschema",
        "Bilder",
        "Befunde",
        "Prüfung (Zitate prüfen)",
        "Suche",
        "Verweise",
        "Literatur",
        "Einfälle",
        "Gedankenkarte",
        "Fäden",
        "Abgleich",
        "Umschlag",
        "EPUB-Vorschau",
        "EPUB-Prüfung",
        "Austausch: Ausgeben",
        "Austausch: Einlesen",
        "Glossar",
        "Register",
        "Stichwortzettel",
        "Einstellungen",
        "Über Baruch",
    ] {
        assert!(pages.iter().any(|p| p.name == wanted), "die Seite „{wanted}“ aus der Abnahme-Liste wird geprüft");
    }
    for page in &pages {
        let Expect { name, uri, tab, groups: expected, why } = page;
        let html = f.page(uri).await;
        assert_eq!(band_default(&html), *tab, "{name} ({uri}): Vorgabe-Reiter – {why}");
        assert_eq!(
            html.matches("data-band-default=\"").count(),
            1,
            "{name} ({uri}): genau eine Vorgabe (base.html schreibt sie aus {{% block band_tab %}})"
        );
        let found = groups(&html, uri);
        for (group, wanted) in expected {
            // Nach dem Zusammenführen bringt base.html eigene Gruppen mit (auch gleichen Namens): Es genügt, dass
            // eine Gruppe dieses Namens die Adressen der Seite in dieser Reihenfolge trägt.
            let ok = found.iter().filter(|g| g.name == *group).any(|g| {
                let have = addresses(g.inner);
                let mut rest = have.iter();
                wanted.iter().all(|w| rest.any(|h| matches_address(h, w)))
            });
            assert!(
                ok,
                "{name} ({uri}): Gruppe „{group}“ mit den Adressen {wanted:?} (standen vor dem Umbau in page_actions; \
                 keine Aktion darf verloren gehen). Gefunden: {:?}",
                found.iter().map(|g| (g.name, addresses(g.inner))).collect::<Vec<_>>()
            );
        }
        for g in &found {
            // Ohne JavaScript bedienbar: In den Gruppen dieser Seiten stehen nur gewöhnliche Links und Formulare.
            assert!(
                !g.inner.contains("data-widget=") && !g.inner.contains("type=\"button\""),
                "{name} ({uri}): Gruppe „{}“ braucht kein Widget und hat keinen Knopf, der nur mit JavaScript etwas \
                 täte: {}",
                g.name,
                g.inner
            );
        }
    }
}

fn group_named<'a, 'b>(found: &'a [Group<'b>], name: &str) -> &'a Group<'b> {
    found.iter().find(|g| g.name == name).unwrap_or_else(|| {
        panic!("Gruppe „{name}“ fehlt; vorhanden: {:?}", found.iter().map(|g| g.name).collect::<Vec<_>>())
    })
}

#[tokio::test]
async fn the_typeset_page_carries_its_controls_as_groups_of_the_book_tab_without_any_script() {
    // Vorher (Runde 18a) stand die Bedienung in einem eigenen Menüband unter der Kopfzeile
    // (fragments/edit_toolbar.html, Zweig satz_page; geprüft in round18_menueband und round9_menu). Seit Runde 33
    // steht sie im Reiter „Buch“; die Zusicherungen von dort gelten hier an der neuen Stelle weiter.
    let f = common::open().await;
    let source = "= Eins\n\nErster Text.\n\n#pagebreak()\n\n= Zwei\n\nZweiter.\n\n// Test: runde33-seiten-satz\n";
    let report = f.import(source, "runde33-seiten-satz.typ").await;
    let work = report.work;
    let nodes = keys(&f, work).await;
    let key = nodes.iter().find(|(_, title)| title == "Eins").expect("Abschnitt Eins").0;
    let uri = format!("/werke/{work}/satz?key={key}");
    let (status, html) = f.get(&uri).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite lädt: {html}");
    assert!(!html.contains("main.js"), "die Satz-Seite lädt kein main.js (sie braucht kein Widget): {html}");
    assert!(!html.contains("<script"), "überhaupt kein Skript: Reiter, Gruppen, Links und ein Formular genügen");
    assert_eq!(band_default(&html), "buch", "Vertrag §7: die Satz-Seite öffnet im Reiter Buch");
    assert!(
        !html.contains("aria-label=\"Menüband der Satz-Ansicht\"") && !html.contains("ribbon-satz\""),
        "das frühere eigene Menüband unter der Kopfzeile (edit_toolbar.html, satz_page) ist nicht mehr eingebunden"
    );
    assert_eq!(html.matches("data-ribbon-satz").count(), 1, "der Haken data-ribbon-satz steht genau einmal: {html}");

    let found = groups(&html, &uri);
    let back = group_named(&found, "Zurück");
    assert_eq!(
        back.extra, " data-ribbon-satz",
        "der Haken sitzt an der ersten Gruppe, zwischen class und data-band-group"
    );
    assert!(
        back.inner.contains(&format!("<a class=\"btn\" href=\"/?work={work}&amp;key={key}\">Zurück zum Werk</a>")),
        "Zurück zum Werk führt an den Abschnitt: {}",
        back.inner
    );
    let download = group_named(&found, "Herunterladen");
    assert!(
        download.inner.contains(&format!("<a class=\"btn\" href=\"/werke/{work}/pdf\" download"))
            && download.inner.contains(">PDF herunterladen</a>"),
        "PDF herunterladen als Anhang: {}",
        download.inner
    );
    let pages = group_named(&found, "Seiten");
    assert!(
        pages.inner.contains(
            "<span class=\"btn\" aria-disabled=\"true\" aria-label=\"Vorige Seite (dies ist die erste)\">‹</span>"
        ),
        "‹ auf Seite 1 gesperrt, mit Beschriftung für Bildschirmleser: {}",
        pages.inner
    );
    assert!(
        pages.inner.contains("<span class=\"typeset-at\">Seite 1 von 2</span>"),
        "Seitenangabe mit der Klasse typeset-at: {}",
        pages.inner
    );
    assert!(
        pages.inner.contains(&format!(
            "<a class=\"btn\" href=\"/werke/{work}/satz?key={key}&amp;page=2\" rel=\"next\" aria-label=\"Nächste Seite\""
        )),
        "› mit rel=next und aria-label: {}",
        pages.inner
    );
    let run = group_named(&found, "Satzlauf");
    assert!(
        run.inner.contains(&format!("method=\"post\" action=\"/werke/{work}/satz/abbrechen\">")),
        "„Satz abbrechen“ ist ein Formular (ohne JavaScript): {}",
        run.inner
    );
    assert!(run.inner.contains(&format!("name=\"key\" value=\"{key}\"")), "der Abschnitt reist mit: {}", run.inner);
    assert!(
        run.inner.contains("<button class=\"btn\" type=\"submit\"") && run.inner.contains(">Satz abbrechen</button>"),
        "abgeschickt wird mit einem gewöhnlichen Knopf: {}",
        run.inner
    );

    // Auf der zweiten Seite: ‹ als Link mit rel=prev, › gesperrt.
    let (status, second) = f.get(&format!("/werke/{work}/satz?key={key}&page=2")).await;
    assert_eq!(status, StatusCode::OK, "zweite Seite lädt: {second}");
    assert!(
        second.contains(&format!(
            "<a class=\"btn\" href=\"/werke/{work}/satz?key={key}&amp;page=1\" rel=\"prev\" aria-label=\"Vorige Seite\""
        )),
        "‹ mit rel=prev und aria-label: {second}"
    );
    assert!(
        second.contains("aria-disabled=\"true\" aria-label=\"Nächste Seite (dies ist die letzte)\">›</span>"),
        "› auf der letzten Seite gesperrt: {second}"
    );
    assert!(second.contains("<span class=\"typeset-at\">Seite 2 von 2</span>"), "Seitenangabe: {second}");
    // Das Formular führt weiter ohne neuen Lauf zurück auf die Seite (Route unverändert).
    let (status, location) = f.post_form(&format!("/werke/{work}/satz/abbrechen"), &format!("key={key}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Abbrechen → 303");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/satz?abgebrochen=0&key={key}").as_str()),
        "lief kein Satz: abgebrochen=0, der Abschnitt bleibt"
    );
}

// ---- Vorlagen (Quelltext) ------------------------------------------------------------------------------------

fn templates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{} lässt sich nicht lesen: {e}", path.display()))
}

/// Inhalt von `{% block NAME %}…{% endblock %}` (die Blöcke des Bands enthalten keine weiteren Blöcke).
fn block<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("{{% block {name} %}}");
    let start = source.find(&open)? + open.len();
    let end = source[start..].find("{% endblock %}").unwrap_or_else(|| panic!("Block {name} ohne Ende"));
    Some(&source[start..start + end])
}

/// Vorlagentext ohne Gruppen, ohne minijinja-Anweisungen und -Kommentare – was übrig bleibt, stünde lose im Band.
fn loose(content: &str) -> String {
    let mut text = String::new();
    let mut rest = content;
    while let Some(start) = rest.find(GROUP_OPEN) {
        text.push_str(&rest[..start]);
        let end = rest[start..].find(GROUP_END).expect("Gruppe ohne Ende") + GROUP_END.len();
        rest = &rest[start + end..];
    }
    text.push_str(rest);
    for (open, close) in [("{#", "#}"), ("{%", "%}")] {
        while let Some(start) = text.find(open) {
            let end = text[start..].find(close).expect("Anweisung ohne Ende") + close.len();
            text.replace_range(start..start + end, "");
        }
    }
    text.trim().to_owned()
}

/// Jede Vorlage, die `base.html` erweitert: Vorgabe-Reiter und die Adressen, die vor dem Umbau (Stand ed9d210) in
/// ihrem Block `page_actions` standen – wörtlich wie im Vorlagentext. So sind auch Seiten erfasst, die nur als
/// Antwort auf ein Formular entstehen (Import-Bericht, Austausch-Vorschau, Hinweisseite der Assistenten).
const TEMPLATES: &[(&str, &str, &[&str])] = &[
    ("about.html", "start", &["href=\"/{% if settings.work %}?work={{ settings.work }}{% endif %}\""]),
    ("asset.html", "buch", &["href=\"/werke/{{ work.id }}/bilder\"", "href=\"/?work={{ work.id }}\""]),
    ("assets.html", "buch", &["href=\"/?work={{ work.id }}\""]),
    ("assist_notice.html", "start", &["href=\"{{ back }}\" data-back>{{ back_label }}</a>"]),
    ("befunde.html", "pruefen", &["href=\"{{ workspace }}\""]),
    ("cover.html", "buch", &["href=\"/werke/{{ work.id }}/umschlag.pdf\" download", "href=\"/?work={{ work.id }}\""]),
    (
        "epub_check.html",
        "buch",
        &[
            "href=\"/werke/{{ work.id }}/epub\" download",
            "href=\"/werke/{{ work.id }}/epub/vorschau\"",
            "href=\"/?work={{ work.id }}\"",
        ],
    ),
    ("epub_preview.html", "buch", &["href=\"/?work={{ work.id }}\"", "href=\"{{ download }}\" download"]),
    ("exchange.html", "buch", &["href=\"{{ back|safe }}\""]),
    ("exchange_done.html", "buch", &["href=\"/?work={{ work.id }}\">Zum Werk</a>"]),
    ("exchange_import.html", "buch", &["href=\"/werke\"", "href=\"/?work={{ work.id }}\""]),
    ("exchange_preview.html", "buch", &["href=\"/austausch/einlesen{% if work %}?work={{ work.id }}{% endif %}\""]),
    ("glossary.html", "buch", &["href=\"/?work={{ work.id }}\"", "href=\"/werke/{{ work.id }}/register\""]),
    ("idea.html", "start", &["href=\"{{ list_href }}\"", "href=\"{{ idea.work_href }}\""]),
    ("ideas.html", "start", &["href=\"{{ map_href }}\"", "href=\"{{ workspace }}\"", "href=\"/\">Zurück</a>"]),
    ("import_report.html", "start", &["href=\"/werke\"", "href=\"{{ work_url|safe }}\""]),
    ("index_page.html", "buch", &["href=\"/?work={{ work.id }}\"", "href=\"/werke/{{ work.id }}/glossar\""]),
    ("karte.html", "start", &["href=\"{{ ideas_href }}\"", "href=\"{{ workspace }}\""]),
    ("literature.html", "pruefen", &["href=\"{{ work_url }}\" data-back"]),
    ("literature_bible.html", "pruefen", &["href=\"{{ literature_url }}\"", "href=\"{{ work_url }}\" data-back"]),
    (
        "marking.html",
        "start",
        &[
            "href=\"/?work={{ work.id }}{% if section %}&amp;key={{ section.key }}&amp;rev={{ section.rev }}{% endif %}\"",
        ],
    ),
    ("notes.html", "start", &["href=\"{{ back }}\""]),
    ("outline.html", "start", &["href=\"{{ work_url }}\""]),
    // Rückfrage- und Fehlerseite beim Speichern ohne JavaScript: hatten nie Aktionen in der Kopfzeile (ihre Knöpfe
    // stehen im Formular der Seite).
    ("outline_confirm.html", "start", &[]),
    ("outline_edit.html", "start", &["href=\"{{ manager_url }}\"", "href=\"{{ work_url }}\""]),
    ("outline_retry.html", "start", &[]),
    ("post.html", "start", &["href=\"{{ history_url }}\"", "href=\"{{ node_url }}\" data-back"]),
    ("posts.html", "start", &["href=\"{{ node_url }}\" data-back"]),
    ("revision_history.html", "start", &["href=\"{{ back }}\""]),
    ("scripture_check.html", "pruefen", &["href=\"{{ back }}\""]),
    ("search.html", "pruefen", &["href=\"{{ back_url }}\""]),
    ("settings.html", "ansicht", &["href=\"/?work={{ work.id }}\"", "href=\"/werke\""]),
    ("source.html", "pruefen", &["href=\"{{ literature_url }}\"", "href=\"{{ work_url }}\" data-back"]),
    ("source_import.html", "start", &["href=\"/werke\"", "href=\"{{ back_url }}\""]),
    ("source_multigpt.html", "pruefen", &["href=\"{{ literature_url }}\"", "href=\"{{ work_url }}\" data-back"]),
    ("sync.html", "pruefen", &["href=\"/{% if selected %}?work={{ selected }}{% endif %}\""]),
    ("thread.html", "start", &["href=\"{{ list_url }}\"", "href=\"{{ work_url }}\" data-back"]),
    ("threads.html", "start", &["href=\"{{ work_url }}\" data-back"]),
    // Die Satz-Seite hatte seit Runde 18a keine page_actions mehr; ihre Bedienung stand im Fragment
    // edit_toolbar.html (Zweig satz_page) und steht jetzt hier.
    (
        "typeset.html",
        "buch",
        &[
            "href=\"{{ back }}\">Zurück zum Werk</a>",
            "href=\"/werke/{{ work.id }}/pdf\" download",
            "href=\"{{ previous }}\" rel=\"prev\" aria-label=\"Vorige Seite\"",
            "<span class=\"typeset-at\">Seite {{ shown }} von {{ total }}</span>",
            "href=\"{{ next }}\" rel=\"next\" aria-label=\"Nächste Seite\"",
            "method=\"post\" action=\"/werke/{{ work.id }}/satz/abbrechen\"",
        ],
    ),
    ("variants.html", "start", &["href=\"{{ back }}\""]),
    ("websearch_evidence.html", "start", &["href=\"{{ back }}\""]),
    ("work_delete.html", "start", &["href=\"/werke\"", "href=\"{{ back_url }}\""]),
    ("work_edit.html", "buch", &["href=\"/werke\"", "href=\"{{ back_url }}\""]),
    ("work_new.html", "start", &["href=\"/werke\"", "href=\"{{ back_url }}\""]),
    ("work_numbering.html", "buch", &["href=\"/werke/{{ work.id }}/bearbeiten\"", "href=\"/?work={{ work.id }}\""]),
    ("works.html", "start", &["href=\"/werke/neu\"", "href=\"/werke/archiv\"", "href=\"{{ back_url }}\""]),
    ("works_archive.html", "start", &["href=\"/werke\"", "href=\"{{ back_url }}\""]),
    ("xrefs.html", "pruefen", &["href=\"/?work={{ work.id }}{% if key %}&amp;key={{ key }}{% endif %}\""]),
];

#[test]
fn every_template_puts_its_actions_as_literal_groups_into_the_block_of_its_tab() {
    let dir = templates_dir();
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("Ordner templates") {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_str().unwrap().to_owned();
        if path.is_dir() || !file.ends_with(".html") {
            continue;
        }
        let source = read(&path);
        // base.html selbst und die Arbeitsseite (workspace.html, Aufgaben menueband und laptop) gehören nicht hierher.
        if !source.contains("{% extends \"base.html\" %}") || file == "workspace.html" {
            continue;
        }
        seen += 1;
        let (_, tab, actions) = TEMPLATES.iter().find(|(name, _, _)| *name == file).unwrap_or_else(|| {
            panic!(
                "{file} erweitert base.html, steht aber nicht in der Liste: Vorgabe-Reiter festlegen (Vertrag §7) und \
                 die Aktionen der Seite als Gruppen in den Block des Reiters setzen"
            )
        });
        let named = block(&source, "band_tab").map(str::trim).unwrap_or("start");
        assert_eq!(named, *tab, "{file}: {{% block band_tab %}} (fehlt er, gilt die Vorgabe start aus base.html)");
        let own = match *tab {
            "start" => "page_actions",
            "pruefen" => "band_pruefen",
            "buch" => "band_buch",
            "ansicht" => "band_ansicht",
            other => panic!("{file}: unbekannter Reiter {other}"),
        };
        for name in ["page_actions", "band_einfuegen", "band_pruefen", "band_buch", "band_ansicht"] {
            if name != own {
                assert!(
                    block(&source, name).is_none(),
                    "{file}: die Aktionen stehen nur im Block des eigenen Reiters ({own}), nicht auch in {name} – \
                     sonst wären sie beim Öffnen der Seite verborgen"
                );
            }
        }
        let Some(content) = block(&source, own) else {
            assert!(actions.is_empty(), "{file}: Block {own} fehlt, obwohl die Seite Aktionen hatte: {actions:?}");
            continue;
        };
        let found = groups(content, &file);
        assert!(!found.is_empty(), "{file}: Block {own} enthält mindestens eine Gruppe");
        assert_eq!(
            loose(content),
            "",
            "{file}: im Block {own} steht nichts lose neben den Gruppen (jede Aktion gehört in eine Gruppe mit Namen)"
        );
        let inside: String = found.iter().map(|g| g.inner).collect::<Vec<_>>().join("\n");
        for action in *actions {
            assert!(
                inside.contains(action),
                "{file}: die Aktion {action} stand vor dem Umbau in der Kopfzeile der Seite und steht jetzt in einer \
                 Gruppe des Blocks {own}: {content}"
            );
        }
        for g in &found {
            assert!(
                !g.name.is_empty() && !g.name.contains('<'),
                "{file}: Gruppenname ist schlichter Text: {:?}",
                g.name
            );
            assert!(
                g.inner.contains("<a ") || g.inner.contains("<form "),
                "{file}: Gruppe „{}“ enthält gewöhnliche Links oder ein Formular (ohne JavaScript bedienbar): {}",
                g.name,
                g.inner
            );
        }
    }
    assert_eq!(
        seen,
        TEMPLATES.len(),
        "jede Vorlage der Liste gibt es noch (eine entfernte Seite auch aus der Liste nehmen)"
    );
}

#[test]
fn the_typeset_page_no_longer_includes_the_edit_toolbar_fragment() {
    let page = read(&templates_dir().join("typeset.html"));
    assert!(
        !page.contains("{% include \"fragments/edit_toolbar.html\" %}") && !page.contains("satz_page = true"),
        "typeset.html bindet das Fragment edit_toolbar.html nicht mehr ein (Vertrag §7)"
    );
    assert!(
        page.contains("{% block scripts %}{% endblock %}"),
        "der Block scripts bleibt leer: die Satz-Seite lädt kein main.js"
    );
    let band = block(&page, "band_buch").expect("Block band_buch der Satz-Seite");
    for hook in [
        "data-ribbon-satz",
        ">Zurück zum Werk</a>",
        ">PDF herunterladen</a>",
        "rel=\"prev\" aria-label=\"Vorige Seite\"",
        "aria-label=\"Vorige Seite (dies ist die erste)\"",
        "class=\"typeset-at\"",
        "rel=\"next\" aria-label=\"Nächste Seite\"",
        "aria-label=\"Nächste Seite (dies ist die letzte)\"",
        "action=\"/werke/{{ work.id }}/satz/abbrechen\"",
        ">Satz abbrechen</button>",
    ] {
        assert!(band.contains(hook), "der Haken {hook} steht wörtlich im Block band_buch der Satz-Seite: {band}");
    }
    let server = read(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/typeset.rs"));
    assert!(
        !server.contains("include_str!(\"../templates/fragments/edit_toolbar.html\")"),
        "die Umgebung der Satz-Seite lädt das Fragment nicht mehr (es gehört nur noch zum Editor)"
    );
}

#[test]
fn the_user_guide_describes_the_band_and_names_no_old_menu_path() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let guide = read(&root.join("docs/Bedienung.md"));
    assert!(guide.contains("\n## Menüband\n"), "docs/Bedienung.md hat den Abschnitt „Menüband“");
    assert!(
        !guide.contains("## Kopfzeile und Menüs") && !guide.contains("### Menüband (Runde 18a)"),
        "„Kopfzeile und Menüs“ ist zum Abschnitt „Menüband“ geworden, „Menüband (Runde 18a)“ ist darin aufgegangen"
    );
    for words in
        ["**Baruch ▾**", "| **Start** |", "| **Einfügen** |", "| **Prüfen** |", "| **Buch** |", "| **Ansicht** |"]
    {
        assert!(guide.contains(words), "der Abschnitt „Menüband“ nennt {words}");
    }
    for file in ["docs/Bedienung.md", "docs/Betrieb.md", "docs/Verteilung.md", "docs/Austauschformat.md"] {
        let text = read(&root.join(file));
        for (number, line) in text.lines().enumerate() {
            for old in ["Werk ▾", "Ansicht ▾", "Werkzeuge ▾"] {
                assert!(
                    !line.contains(old) || line.contains("früher"),
                    "{file}, Zeile {}: der alte Menüpfad „{old}“ ist auf das Menüband umgestellt (erlaubt nur in einem \
                     ausdrücklich als „früher“ bezeichneten Satz): {line}",
                    number + 1
                );
            }
        }
    }
}
