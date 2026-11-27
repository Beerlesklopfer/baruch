// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 34 „Mindmap wie XMind“ – acceptance by the integrator, end to end across the merged branches (mindmap_daten,
//! mindmap_widget, mindmap_austausch) including the user's addendum of 2026-11-25: the **cloud** (ideas without a
//! place) lies next to the mind map and is connected with it – ideas move in both directions, free main topics are main
//! branches of their own.
//!
//! The „Halligbuch“ with two parts and four chapters and all ideas are freely invented; no model, no network. Keys and
//! dragging of the widget (`web/src/mindmap.ts`) run in no browser: checked are the RPC calls that the widget issues
//! for them (the same parameters) and the image that the server returns afterwards. The migration 0052 on a
//! reconstructed database in the state 0051 is checked by `crates/baruch-store/tests/migration_0052.rs`; here is that
//! ideas, links and positions of the old idea map arrive unchanged in the mind map.

use crate::common;

use std::net::SocketAddr;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::idea_map::files::import;
use baruch_server::idea_map::tree::ImportTree;
use serde_json::{Value, json};
use tower::ServiceExt;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

struct Book {
    server: common::Server,
    work: i64,
    slug: String,
    keys: Vec<(String, i64)>,
}

/// Invented Hallig book: part „Land“ (Warft, Priel) and part „See“ (Tide, Sturmflut).
async fn open_book(file: &str, title: &str) -> Book {
    let server = common::open().await;
    open_book_in(server, file, title).await
}

async fn open_book_in(server: common::Server, file: &str, title: &str) -> Book {
    let source = format!(
        "#set document(title: \"{title}\")\n\n{}\n\n= Warft\n\nText.\n\n= Priel\n\nText.\n\n{}\n\n= Tide\n\nText.\n\n\
         = Sturmflut\n\nText.\n",
        baruch_typst::heading_line(0, "Land", None),
        baruch_typst::heading_line(0, "See", None)
    );
    let report = server.import(&source, file).await;
    let answer = server.rpc("work.get", json!({ "work": report.work })).await;
    let keys = result(&answer)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["title"].as_str().unwrap().to_owned(), n["key"].as_i64().unwrap()))
        .collect();
    Book { server, work: report.work, slug: report.slug, keys }
}

impl Book {
    fn key(&self, title: &str) -> i64 {
        self.keys.iter().find(|(t, _)| t == title).unwrap_or_else(|| panic!("kein Punkt {title}: {:?}", self.keys)).1
    }

    fn n(&self, title: &str) -> String {
        format!("n:{}", self.key(title))
    }

    async fn call(&self, method: &str, params: Value) -> Value {
        result(&self.server.rpc(method, params).await).clone()
    }

    /// `idea.create` like the widget (Tab, Enter, Alt+E): the answer is the id.
    async fn create(&self, mut params: Value) -> i64 {
        if params.get("parent").is_none() {
            params["work"] = json!(self.work);
        }
        self.call("idea.create", params).await["idea"]["id"].as_i64().unwrap()
    }

    /// The image as the widget fetches it after every action (`data-bild`), without proximity.
    async fn image(&self, query: &str) -> String {
        let (status, svg) = self.server.get(&format!("/werke/{}/karte/bild?near=0{query}", self.work)).await;
        assert_eq!(status, StatusCode::OK, "das Bild antwortet: {svg}");
        svg
    }

    async fn act(&self, form: &str) -> String {
        let (status, location) = self
            .server
            .post_form(&format!("/werke/{}/karte/act", self.work), &format!("{form}&seite=gliederung&back=near%3D0"))
            .await;
        assert_eq!(status, StatusCode::SEE_OTHER, "das Formular „{form}“ leitet nach dem Speichern weiter");
        location.expect("303 mit Ziel")
    }

    async fn outline(&self, query: &str) -> String {
        let (status, html) = self.server.get(&format!("/werke/{}/karte/gliederung?near=0{query}", self.work)).await;
        assert_eq!(status, StatusCode::OK, "die Gliederungsansicht antwortet");
        html
    }

    /// Tree and cloud as a flat list: (identifier, title, identifier of the parent node or „wolke“).
    async fn places(&self) -> Vec<(String, String, String)> {
        fn fill(node: &Value, parent: &str, out: &mut Vec<(String, String, String)>) {
            let id = node["id"].as_str().unwrap().to_owned();
            out.push((id.clone(), node["title"].as_str().unwrap().to_owned(), parent.to_owned()));
            for child in node["children"].as_array().unwrap() {
                fill(child, &id, out);
            }
        }
        let answer = self.call("idea.tree", json!({ "work": self.work, "near": false })).await;
        let mut out = Vec::new();
        fill(&answer["map"]["root"], "", &mut out);
        for idea in answer["cloud"]["ideas"].as_array().unwrap() {
            fill(idea, "wolke", &mut out);
        }
        out
    }

    /// Identifier of the idea with this title and that of its parent node.
    async fn place_of(&self, title: &str) -> (String, String) {
        let places = self.places().await;
        let found = places.iter().find(|(_, t, _)| t == title).unwrap_or_else(|| panic!("kein Knoten „{title}“"));
        (found.0.clone(), found.2.clone())
    }

    async fn id_of(&self, title: &str) -> i64 {
        self.place_of(title).await.0.trim_start_matches("i:").parse().unwrap()
    }

    fn commits(&self) -> usize {
        common::git(&self.server.mirror_dir_of(&self.slug), &["rev-list", "--count", "HEAD"])
            .map_or(0, |n| n.trim().parse().unwrap())
    }

    fn last_commit(&self) -> String {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "-1", "--format=%B"]).unwrap_or_default()
    }
}

/// Opening tag of the node `id` in the image (`<g class="mm-node …" data-node="id" …>`).
fn tag<'a>(svg: &'a str, id: &str) -> &'a str {
    let at = svg.find(&format!(" data-node=\"{id}\"")).unwrap_or_else(|| panic!("Knoten {id} fehlt im Bild"));
    let start = svg[..at].rfind("<g ").unwrap();
    &svg[start..at + svg[at..].find('>').unwrap()]
}

fn drawn(svg: &str, id: &str) -> bool {
    svg.contains(&format!(" data-node=\"{id}\""))
}

fn attr<'a>(tag: &'a str, name: &str) -> &'a str {
    match tag.find(&format!(" {name}=\"")) {
        Some(at) => {
            let rest = &tag[at + name.len() + 3..];
            &rest[..rest.find('"').unwrap()]
        }
        None => "",
    }
}

fn i(id: i64) -> String {
    format!("i:{id}")
}

// ---- Image and actions of the widget -------------------------------------------------------------------------

#[tokio::test]
async fn the_mindmap_has_the_work_in_the_middle_branches_on_both_sides_and_follows_every_widget_action() {
    let b = open_book("halligbuch.typ", "Halligbuch").await;
    let w = b.work;
    // Tab at the chapter, Tab at the idea, Enter (sibling before the next one), Tab at the work, Alt+E – with exactly
    // the parameters that mindmap.ts sends for them.
    let fenne = b.create(json!({ "title": "Fenne", "key": b.key("Warft") })).await;
    let zaun = b.create(json!({ "title": "Zaun", "parent": fenne })).await;
    let tor = b.create(json!({ "title": "Tor", "parent": fenne })).await;
    let graben = b.create(json!({ "title": "Graben", "parent": fenne, "before": tor })).await;
    let pfosten = b.create(json!({ "title": "Pfosten", "parent": zaun, "note": "aus Eiche" })).await;
    let vogelzug = b.create(json!({ "title": "Vogelzug", "cloud": false })).await;
    let treibholz = b.create(json!({ "title": "Treibholz" })).await;

    let svg = b.image("").await;
    // Central topic.
    let root = tag(&svg, "w");
    assert!(root.contains("mm-work"), "das Werk ist der Knoten in der Mitte: {root}");
    assert_eq!((attr(root, "data-side"), attr(root, "data-title")), ("", "Halligbuch"), "ohne Seite, mit Titel");
    // Main branches right and left: parts in outline order, after them the free main topic.
    let land = tag(&svg, &b.n("Land"));
    assert!(land.contains("mm-part") && land.contains("mm-main"), "der Teil ist ein Hauptast: {land}");
    assert_eq!((attr(land, "data-parent"), attr(land, "data-side")), ("w", "right"), "der erste Hauptast steht rechts");
    let topic = tag(&svg, &i(vogelzug));
    assert!(topic.contains("mm-main") && !topic.contains("mm-cloud-item"), "Tab am Werk: freies Hauptthema: {topic}");
    assert_eq!(
        (attr(topic, "data-parent"), attr(topic, "data-side")),
        ("w", "left"),
        "der letzte Hauptast steht links"
    );
    assert_ne!(attr(land, "data-branch"), attr(topic, "data-branch"), "jeder Hauptast hat seine eigene Farbe");
    // Sub-branches: chapters below the part, ideas below them, arbitrarily deep; side and colour are inherited by the
    // whole branch.
    let warft = tag(&svg, &b.n("Warft"));
    assert_eq!(
        (attr(warft, "data-parent"), attr(warft, "data-depth"), attr(warft, "data-editable")),
        (b.n("Land").as_str(), "2", "0"),
        "Kapitel hängt am Teil und ist in der Mindmap nicht änderbar (Gliederung = Antragsregel)"
    );
    let deep = tag(&svg, &i(pfosten));
    assert_eq!(
        (attr(deep, "data-parent"), attr(deep, "data-depth"), attr(deep, "data-side"), attr(deep, "data-editable")),
        (i(zaun).as_str(), "5", "right", "1"),
        "Werk → Teil → Kapitel → Einfall → Unterknoten → Unterknoten"
    );
    assert_eq!(attr(deep, "data-branch"), attr(land, "data-branch"), "Unteräste erben die Farbe ihres Hauptasts");
    assert!(
        svg.contains(&format!("data-from=\"{}\" data-to=\"{}\"", i(zaun), i(pfosten))),
        "ein Ast (mm-edge) verbindet Eltern- und Unterknoten"
    );
    let at = |id: i64| svg.find(&format!(" data-node=\"{}\"", i(id))).unwrap();
    assert!(
        at(zaun) < at(graben) && at(graben) < at(tor),
        "Enter legt das Geschwister vor das nächste: Zaun, Graben, Tor"
    );
    assert!(svg.contains("mm-note"), "der Einfall mit Notiz trägt das Notiz-Symbol");
    // Cloud: Alt+E drops without a place.
    let loose = tag(&svg, &i(treibholz));
    assert!(loose.contains("mm-cloud-item") && attr(loose, "data-cloud") == "1", "Alt+E: Kärtchen der Wolke: {loose}");
    assert!(svg.contains("data-cloud-count=\"1\"") && svg.contains("1 in der Wolke"), "Zähler der Wolke im Bild");

    // F2: rename, one commit; Ctrl+Z sends the old title back.
    let commits = b.commits();
    b.call("idea.update", json!({ "id": tor, "title": "Stöpe" })).await;
    assert_eq!(attr(tag(&b.image("").await, &i(tor)), "data-own"), "Stöpe", "der gespeicherte Titel steht am Knoten");
    assert_eq!(b.commits(), commits + 1, "Umbenennen ist ein Commit im Spiegel");

    // Dragging: from the cloud onto an idea, back (Ctrl+Z), onto a chapter, onto the work, between siblings.
    let moved = b.call("idea.move", json!({ "id": treibholz, "parent": tor })).await;
    assert_eq!(moved["changed"], true, "Umhängen ändert etwas: {moved}");
    let log = b.last_commit();
    assert!(
        log.contains("Baruch-Action: idea-move") && log.contains("Baruch-Origin: human"),
        "Commit mit Herkunft: {log}"
    );
    let svg = b.image("").await;
    let hung = tag(&svg, &i(treibholz));
    assert_eq!(
        (attr(hung, "data-parent"), attr(hung, "data-cloud")),
        (i(tor).as_str(), ""),
        "aus der Wolke in den Baum"
    );
    assert!(svg.contains("data-cloud-count=\"0\""), "die Wolke ist leer");
    b.call("idea.move", json!({ "id": treibholz, "cloud": true, "before": null })).await;
    assert_eq!(
        attr(tag(&b.image("").await, &i(treibholz)), "data-cloud"),
        "1",
        "Rückgängig legt ihn zurück in die Wolke"
    );
    b.call("idea.move", json!({ "id": treibholz, "parent": null, "key": b.key("Priel") })).await;
    assert_eq!(
        attr(tag(&b.image("").await, &i(treibholz)), "data-parent"),
        b.n("Priel"),
        "auf ein Kapitel gezogen hängt er dort"
    );
    b.call("idea.move", json!({ "id": treibholz, "parent": null, "key": null, "cloud": false })).await;
    let svg = b.image("").await;
    assert!(tag(&svg, &i(treibholz)).contains("mm-main"), "aufs Werk gezogen wird er freies Hauptthema");
    b.call("idea.move", json!({ "id": treibholz, "cloud": true })).await;
    b.call("idea.move", json!({ "id": tor, "before": zaun })).await;
    let svg = b.image("").await;
    let at = |id: i64| svg.find(&format!(" data-node=\"{}\"", i(id))).unwrap();
    assert!(at(tor) < at(zaun), "zwischen Geschwister gezogen ordnet neu");
    // A subtree moves as a whole into the cloud and back again (one step „Rückgängig“).
    b.call("idea.move", json!({ "id": zaun, "cloud": true })).await;
    let svg = b.image("").await;
    let card = tag(&svg, &i(zaun));
    assert_eq!(
        (attr(card, "data-cloud"), attr(card, "data-children")),
        ("1", "1"),
        "Zaun liegt mit Unterknoten in der Wolke"
    );
    assert!(!drawn(&svg, &i(pfosten)), "Unterknoten werden in der Wolke nicht einzeln gezeichnet");
    assert!(svg.contains("data-cloud-count=\"2\""), "der Zähler zählt die Kärtchen, nicht ihre Unterknoten");
    b.call("idea.move", json!({ "id": zaun, "parent": fenne, "key": null, "before": graben })).await;
    assert_eq!(attr(tag(&b.image("").await, &i(pfosten)), "data-parent"), i(zaun), "zurück im Baum, samt Unterknoten");
    // A cycle is rejected, nothing changes.
    let refused = b.server.rpc("idea.move", json!({ "id": fenne, "parent": pfosten })).await;
    assert_eq!(refused["error"]["code"], -32000, "eigener Nachfahre als Elternknoten: abgelehnt: {refused}");
    // Chapters and parts are not ideas: `idea.*` does not reach them.
    let refused = b.server.rpc("idea.update", json!({ "id": b.key("Warft") + 100_000, "title": "Umbenannt" })).await;
    assert!(refused["error"].is_object(), "eine Kennung der Gliederung ist kein Einfall");

    // Folding is pure display (`zu=`), nothing is stored.
    let closed = b.image(&format!("&zu={}", i(fenne))).await;
    assert!(tag(&closed, &i(fenne)).contains("mm-closed"), "der Ast ist eingeklappt");
    for hidden in [zaun, pfosten, graben, tor] {
        assert!(!drawn(&closed, &i(hidden)), "eingeklappt: Nachfahre {hidden} wird nicht gezeichnet");
    }
    assert!(closed.contains(">+ 4<"), "der Einklapper nennt die Zahl der verborgenen Knoten");
    assert!(drawn(&b.image("").await, &i(pfosten)), "ohne zu= steht wieder alles da");
    let (_, page) = b.server.get(&format!("/werke/{w}/karte?near=0&zu={}", i(fenne))).await;
    assert!(
        page.contains("class=\"mm-toggle\" href=\"?near=0\""),
        "ohne JavaScript klappt der Link am Knoten wieder auf (Adresse ohne zu=)"
    );

    // Link with a label; a hidden end ends at the visible ancestor; contradiction into the cloud.
    let link = b
        .call(
            "idea.link",
            json!({ "from": pfosten, "to": vogelzug, "kind": "leads_to", "note": "Rastplatz der Gänse" }),
        )
        .await["link"]["id"]
        .as_i64()
        .unwrap();
    b.call("idea.link", json!({ "from": graben, "to": treibholz, "kind": "contradicts", "note": "" })).await;
    let svg = b.image("").await;
    assert!(
        svg.contains(&format!(
            "mm-link mm-link-leads_to\" data-link=\"{link}\" data-from=\"{}\" data-to=\"{}\"",
            i(pfosten),
            i(vogelzug)
        )),
        "die Beziehung ist eine Linie zwischen den beiden Knoten"
    );
    assert!(svg.contains(">führt weiter: Rastplatz der Gänse<"), "Beschriftung = Art und Notiz");
    assert!(
        svg.contains("mm-link mm-link-contradicts mm-contradiction"),
        "auch eine Verbindung in die Wolke wird gezeichnet, der Widerspruch hervorgehoben"
    );
    let closed = b.image(&format!("&zu={}", i(zaun))).await;
    assert!(
        closed.contains(&format!("data-end-from=\"{}\"", i(zaun))),
        "ist ein Ende eingeklappt, endet die Linie am sichtbaren Vorfahren"
    );
    let (_, page) = b.server.get(&format!("/werke/{w}/karte?near=0")).await;
    let contradictions = &page[page.find("Widersprüche").expect("Liste Widersprüche")..];
    assert!(
        contradictions.contains("Graben") && contradictions.contains("Treibholz"),
        "Widerspruch steht in der Liste"
    );

    // Deleting takes the child nodes along (one commit), restoring brings back exactly them.
    let commits = b.commits();
    let gone = b.call("idea.delete", json!({ "id": fenne, "confirm": true })).await;
    assert_eq!(gone["deleted_ids"].as_array().map(Vec::len), Some(5), "Fenne, Zaun, Pfosten, Graben, Stöpe: {gone}");
    assert_eq!(b.commits(), commits + 1, "ein Commit für den ganzen Ast");
    let svg = b.image("").await;
    for id in [fenne, zaun, pfosten, graben, tor] {
        assert!(!drawn(&svg, &i(id)), "gelöscht: {id} steht nicht mehr im Bild");
    }
    assert!(!svg.contains(&format!("data-link=\"{link}\"")), "die Linie zu einem gelöschten Einfall verschwindet");
    let back = b.call("idea.restore", json!({ "id": fenne })).await;
    assert_eq!(back["restored"].as_array().map(Vec::len), Some(5), "Rückgängig bringt den ganzen Ast zurück: {back}");
    let svg = b.image("").await;
    assert_eq!(attr(tag(&svg, &i(pfosten)), "data-parent"), i(zaun), "an derselben Stelle");
    assert!(svg.contains(&format!("data-link=\"{link}\"")), "und die Verbindung ist wieder da");

    // The widget: delivered, and it confirms the deletion (the server demands `confirm`).
    let (status, js) = b.server.get("/static/js/mindmap.js").await;
    assert_eq!(status, StatusCode::OK, "das Widget wird ausgeliefert");
    let deletes: Vec<&str> = js.split("\"idea.delete\"").skip(1).map(|rest| &rest[..rest.find('}').unwrap()]).collect();
    assert!(
        !deletes.is_empty() && deletes.iter().all(|call| call.contains("confirm: true")),
        "jeder Aufruf von idea.delete im Widget schickt confirm: true mit: {deletes:?}"
    );
    assert!(
        js.contains("cloud: true") && js.contains("cloud: false"),
        "es kennt „in die Wolke“ und „Hauptthema am Werk“"
    );
}

// ---- Without JavaScript: outline view ------------------------------------------------------------------------

#[tokio::test]
async fn without_javascript_the_outline_view_does_every_action_and_connects_cloud_and_mindmap() {
    let b = open_book("halligbuch.typ", "Halligbuch").await;
    let w = b.work;
    let (_, page) = b.server.get(&format!("/werke/{w}/karte")).await;
    assert!(
        page.contains(&format!("href=\"/werke/{w}/karte/gliederung\"")),
        "die Seite der Mindmap verweist auf die Gliederungsansicht"
    );

    // Create: at the chapter, as a main topic at the work, into the cloud, as a child node and as a sibling.
    let location = b.act(&format!("method=idea.create&title=Fenne&key={}", b.key("Warft"))).await;
    assert_eq!(location, format!("/werke/{w}/karte/gliederung?near=0&ok=angelegt"), "zurück in die Gliederungsansicht");
    b.act("method=idea.create&title=Vogelzug&ort=werk").await;
    b.act("method=idea.create&title=Treibholz").await;
    let fenne = b.id_of("Fenne").await;
    b.act(&format!("method=idea.create&title=Zaun&parent={fenne}")).await;
    b.act(&format!("method=idea.create&title=Tor&parent={fenne}")).await;
    let (zaun, tor) = (b.id_of("Zaun").await, b.id_of("Tor").await);
    b.act(&format!("method=idea.create&title=Graben&parent={fenne}&before={tor}")).await;
    assert_eq!(b.place_of("Fenne").await.1, b.n("Warft"), "Unterknoten am Kapitel");
    assert_eq!(b.place_of("Vogelzug").await.1, "w", "„Hauptthema anlegen“ hängt direkt ans Werk");
    assert_eq!(b.place_of("Treibholz").await.1, "wolke", "ohne Platz liegt der neue Einfall in der Wolke");
    let order: Vec<String> =
        b.places().await.into_iter().filter(|(_, _, parent)| *parent == i(fenne)).map(|(_, title, _)| title).collect();
    assert_eq!(order, vec!["Zaun", "Graben", "Tor"], "Geschwister steht vor dem genannten");

    // The page: tree as a nested list, cloud next to it, forms of the selected node – without script.
    let html = b.outline(&format!("&wahl={}", i(zaun))).await;
    let main = &html[html.find("<main").unwrap()..];
    assert!(!main.contains("<script"), "die Gliederungsansicht braucht kein Skript");
    let tree = &main[main.find("mmg-tree").unwrap()..main.find("id=\"wolke\"").unwrap()];
    for title in ["Land", "Warft", "Fenne", "Zaun", "Vogelzug"] {
        assert!(tree.contains(title), "„{title}“ steht im Baum");
    }
    assert!(!tree.contains("Treibholz"), "der Einfall ohne Platz steht nicht im Baum");
    let cloud = &main[main.find("id=\"wolke\"").unwrap()..];
    assert!(cloud.contains("1 in der Wolke") && cloud.contains("Treibholz"), "sondern in der Wolke, mit Zähler");
    for form in ["rename", "child", "sibling", "move", "main", "cloud", "order", "section", "link", "delete"] {
        assert!(main.contains(&format!("data-form=\"{form}\"")), "Formular „{form}“ für den gewählten Einfall");
    }
    assert!(
        main.contains("data-form=\"new-main\"") && main.contains("data-form=\"new\""),
        "Anlegen: Hauptthema und Wolke"
    );
    let chapter = b.outline(&format!("&wahl={}", b.n("Warft"))).await;
    assert!(
        chapter.contains("data-form=\"child\"") && !chapter.contains("data-form=\"rename\""),
        "Kapitel: nur „Unterknoten“, kein Umbenennen/Löschen (Gliederung = Antragsregel)"
    );

    // Rename, move, cloud ↔ mind map, main topic, order.
    assert!(b.act(&format!("method=idea.update&id={tor}&title=St%C3%B6pe")).await.contains("ok=umbenannt"));
    assert_eq!(b.id_of("Stöpe").await, tor, "umbenannt");
    let treibholz = b.id_of("Treibholz").await;
    assert!(b.act(&format!("method=idea.move&id={treibholz}&ziel=i:{zaun}")).await.contains("ok=umgehaengt"));
    assert_eq!(b.place_of("Treibholz").await.1, i(zaun), "„In die Mindmap unter …“ hängt den Wolken-Einfall ein");
    assert!(b.act(&format!("method=idea.move&id={zaun}&ziel=wolke")).await.contains("ok=in-wolke"));
    assert_eq!(b.place_of("Zaun").await.1, "wolke", "„In die Wolke“ legt den Knoten ab");
    assert_eq!(b.place_of("Treibholz").await.1, i(zaun), "sein Unterknoten bleibt an ihm");
    assert!(b.outline("").await.contains("1 in der Wolke"), "der Zähler zählt das Kärtchen");
    b.act(&format!("method=idea.move&id={zaun}&ziel=w")).await;
    assert_eq!(b.place_of("Zaun").await.1, "w", "„Als Hauptthema ans Werk“ macht ihn zum Hauptast");
    b.act(&format!("method=idea.move&id={zaun}&ziel={}", b.n("Tide"))).await;
    assert_eq!(b.place_of("Zaun").await.1, b.n("Tide"), "Umhängen an ein Kapitel");
    let graben = b.id_of("Graben").await;
    assert!(b.act(&format!("method=idea.move&id={tor}&before={graben}")).await.contains("ok=umgehaengt"));
    let order: Vec<String> =
        b.places().await.into_iter().filter(|(_, _, parent)| *parent == i(fenne)).map(|(_, title, _)| title).collect();
    assert_eq!(order, vec!["Stöpe", "Graben"], "„nach oben“ ordnet die Geschwister");

    // Link with a label, unlink, restore.
    let location =
        b.act(&format!("method=idea.link&from={graben}&to={tor}&kind=belongs_to&note=Entw%C3%A4sserung")).await;
    assert!(location.contains("ok=verbunden"), "verbunden: {location}");
    let html = b.outline("").await;
    let relations = &html[html.find("mmg-relations").unwrap()..];
    assert!(
        relations.contains("gehört zu") && relations.contains("Entwässerung"),
        "die Beziehung steht mit Art und Beschriftung in der Liste"
    );
    let link = result(&b.server.rpc("idea.links", json!({ "work": w })).await)["links"][0]["id"].as_i64().unwrap();
    let location = b.act(&format!("method=idea.unlink&id={link}")).await;
    assert!(
        location.contains("ok=geloest") && location.contains(&format!("undo={link}")),
        "gelöst mit Rückweg: {location}"
    );
    assert!(b.act(&format!("method=idea.link_restore&id={link}")).await.contains("ok=wiederhergestellt"));

    // Delete (including child nodes) and restore.
    let location = b.act(&format!("method=idea.delete&id={fenne}")).await;
    assert!(location.contains("ok=geloescht") && location.contains(&format!("wieder={fenne}")), "{location}");
    let html = b.outline(&format!("&ok=geloescht&wieder={fenne}")).await;
    assert!(html.contains("value=\"idea.restore\""), "die Meldung bietet „Rückgängig“ an");
    assert!(b.places().await.iter().all(|(_, title, _)| title != "Graben"), "die Unterknoten sind mitgelöscht");
    let deleted = &html[html.find("mmg-deleted").unwrap()..];
    assert!(deleted.contains("Fenne"), "„Gelöschte Einfälle“ nennt den Einfall mit seinem Titel");
    assert!(b.act(&format!("method=idea.restore&id={fenne}")).await.contains("ok=einfall-wieder"));
    assert_eq!(b.place_of("Graben").await.1, i(fenne), "wiederhergestellt samt Unterknoten");

    // Foreign things are rejected: idea of another work, unknown action.
    let other = result(&b.server.rpc("idea.create", json!({ "words": "Fremd" })).await)["idea"]["id"].as_i64().unwrap();
    let (status, _) =
        b.server.post_form(&format!("/werke/{w}/karte/act"), &format!("method=idea.delete&id={other}")).await;
    assert_eq!(status, StatusCode::CONFLICT, "ein Einfall, der nicht zum Werk der Adresse gehört");
    let (status, _) = b.server.post_form(&format!("/werke/{w}/karte/act"), "method=outline.delete&id=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "nur die Aktionen der Mindmap; die Gliederung ändert sie nie");
}

// ---- MCP -----------------------------------------------------------------------------------------------------

async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

#[tokio::test]
async fn mcp_adds_nodes_with_their_origin_and_page_outline_and_mirror_show_them() {
    let b = open_book("halligbuch.typ", "Halligbuch").await;
    let (headers, answer) = mcp(
        &b.server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Deichgraf", "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let call = |name: &'static str, arguments: Value| {
        let (server, session) = (&b.server, session.as_str());
        async move {
            let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                                  "params": { "name": name, "arguments": arguments } });
            let answer = mcp(server, Some(session), message).await.1;
            assert_eq!(answer["result"]["isError"], false, "{name} sollte gelingen: {answer}");
            answer["result"]["structuredContent"].clone()
        }
    };
    let commits = b.commits();
    let topic = call("idea_create", json!({ "work": b.work, "title": "Sielzug", "key": b.key("Priel") })).await["idea"]
        ["id"]
        .as_i64()
        .unwrap();
    let child = call("idea_create", json!({ "title": "Schöpfwerk", "parent": topic })).await;
    let child_id = child["idea"]["id"].as_i64().unwrap();
    assert_eq!(child["idea"]["origin"]["handle"], "@mcp:deichgraf", "Herkunft: der MCP-Client");
    let loose =
        call("idea_create", json!({ "work": b.work, "title": "Lahnung" })).await["idea"]["id"].as_i64().unwrap();
    assert_eq!(b.commits(), commits + 3, "jedes Anlegen ist ein Commit im Spiegel");
    let log = b.last_commit();
    assert!(
        log.contains("Baruch-Origin: mcp") && log.contains("Persona: @mcp:deichgraf"),
        "Herkunft im Spiegel: {log}"
    );
    call("idea_move", json!({ "id": loose, "parent": child_id })).await;
    assert!(b.last_commit().contains("Baruch-Action: idea-move"), "auch das Umhängen über MCP ist ein Commit");

    let tree = call("idea_tree", json!({ "work": b.work })).await;
    assert_eq!(tree["map"]["root"]["id"], "w", "idea_tree liest die Mindmap");
    assert_eq!(tree["cloud"]["count"], 0, "nach dem Umhängen ist die Wolke leer: {}", tree["cloud"]);
    // The human sees the same nodes in the image and in the outline view.
    let svg = b.image("").await;
    assert_eq!(attr(tag(&svg, &i(topic)), "data-parent"), b.n("Priel"), "der Knoten des Clients hängt am Kapitel");
    assert_eq!(
        attr(tag(&svg, &i(loose)), "data-parent"),
        i(child_id),
        "der umgehängte Knoten unter seinem neuen Elternknoten"
    );
    let html = b.outline("").await;
    assert!(html.contains("Sielzug") && html.contains("Schöpfwerk") && html.contains("Lahnung"), "Gliederungsansicht");
    // The outline stays a matter of the requests: no chapter can be changed through idea_*.
    let message = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                          "params": { "name": "idea_update", "arguments": { "id": 9_999_999, "title": "Umbenannt" } } });
    let refused = mcp(&b.server, Some(&session), message).await.1;
    assert!(
        refused["result"]["isError"] == true || refused["error"].is_object(),
        "idea_update erreicht nur Einfälle: {refused}"
    );
    let titles: Vec<String> = result(&b.server.rpc("work.get", json!({ "work": b.work })).await)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(titles, vec!["Land", "Warft", "Priel", "See", "Tide", "Sturmflut"], "die Gliederung ist unverändert");
}

// ---- Round trip .mm and .xmind -------------------------------------------------------------------------------

async fn bytes_of(server: &common::Server, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = server.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    (status, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

fn upload(work: i64, bytes: &[u8]) -> Request<Body> {
    let boundary = "----baruch-r34-abnahme";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"karte\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
    Request::post(format!("/werke/{work}/karte/einlesen"))
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap()
}

fn form_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The tree as nested titles: `[titel, [kinder…]]`.
fn shape(node: &Value) -> Value {
    json!([node["title"], node["children"].as_array().unwrap().iter().map(shape).collect::<Vec<_>>()])
}

#[tokio::test]
async fn mm_and_xmind_travel_out_and_back_in_with_the_same_structure() {
    let a = open_book("halligbuch.typ", "Halligbuch").await;
    let fenne = a.create(json!({ "title": "Fenne", "key": a.key("Warft"), "note": "nasse Wiese" })).await;
    let zaun = a.create(json!({ "title": "Zaun", "parent": fenne, "words": "Holz, Draht" })).await;
    a.create(json!({ "title": "Pfosten", "parent": zaun })).await;
    let graben = a.create(json!({ "title": "Graben", "parent": fenne })).await;
    let vogelzug = a.create(json!({ "title": "Vogelzug", "cloud": false })).await;
    a.create(json!({ "title": "Ringelgans", "parent": vogelzug })).await;
    let treibholz = a.create(json!({ "title": "Treibholz" })).await;
    a.create(json!({ "title": "Strandgut", "parent": treibholz })).await;
    a.call("idea.link", json!({ "from": graben, "to": vogelzug, "kind": "leads_to", "note": "Tränke" })).await;

    // What the output carries: the main branches (parts with number and chapters, main topic), after them the cloud.
    let source = a.call("idea.tree", json!({ "work": a.work, "near": false })).await;
    let numbered = |node: &Value| -> Value {
        fn walk(node: &Value) -> Value {
            let number = node["number"].as_str().unwrap_or_default();
            let title = node["title"].as_str().unwrap();
            let shown = if number.is_empty() { title.to_owned() } else { format!("{number} {title}") };
            json!([shown, node["children"].as_array().unwrap().iter().map(walk).collect::<Vec<_>>()])
        }
        walk(node)
    };
    let expected: Vec<Value> = source["map"]["root"]["children"].as_array().unwrap().iter().map(numbered).collect();
    // Round 36: the cloud of the own file comes back into the cloud (before: as main topics into the tree).
    let expected_cloud: Vec<Value> = source["cloud"]["ideas"].as_array().unwrap().iter().map(numbered).collect();
    assert_eq!(
        (expected.len(), expected_cloud.len()),
        (3, 1),
        "zwei Teile und ein Hauptthema im Baum, ein Einfall in der Wolke: {expected:?} {expected_cloud:?}"
    );

    for ending in ["mm", "xmind"] {
        let request = Request::get(format!("/werke/{}/karte.{ending}?near=0", a.work)).body(Body::empty()).unwrap();
        let (status, file) = bytes_of(&a.server, request).await;
        assert_eq!(status, StatusCode::OK, "karte.{ending} antwortet");
        let read: ImportTree =
            import::read(&file).unwrap_or_else(|e| panic!("die eigene Ausgabe .{ending} ist lesbar: {e}"));
        assert_eq!(read.title, "Halligbuch", ".{ending}: die Wurzel wird Titel, nicht Knoten");
        assert_eq!(read.links.len(), 1, ".{ending}: die Beziehung reist mit");
        assert_eq!(read.links[0].note, "führt weiter: Tränke", ".{ending}: mit Art und Notiz als Beschriftung");

        // Import into a second, empty work: the preview creates nothing, the confirmation everything.
        let b = open_book(&format!("zweitbuch-{ending}.typ"), "Zweitbuch").await;
        let (status, preview) = bytes_of(&b.server, upload(b.work, &file)).await;
        let preview = String::from_utf8(preview).unwrap();
        assert_eq!(status, StatusCode::OK, ".{ending}: die Vorschau antwortet: {preview}");
        assert!(preview.contains("data-import-preview"), ".{ending}: Vorschau des gelesenen Baums");
        assert!(
            b.places().await.iter().all(|(id, ..)| !id.starts_with("i:")),
            ".{ending}: die Vorschau legt nichts an"
        );
        let commits = b.commits();
        let body = format!("tree={}&parent=", form_encode(&serde_json::to_string(&read).unwrap()));
        let (status, location) = b.server.post_form(&format!("/werke/{}/karte/einlesen/anlegen", b.work), &body).await;
        assert_eq!(status, StatusCode::SEE_OTHER, ".{ending}: die Bestätigung legt an");
        assert_eq!(location.as_deref(), Some(format!("/werke/{}/karte?ok=eingelesen", b.work).as_str()));
        assert_eq!(b.commits(), commits + 1, ".{ending}: das Einlesen ist ein Commit im Spiegel");

        // Same structure: the imported nodes are free main topics (ideas) in the same nesting.
        let imported = b.call("idea.tree", json!({ "work": b.work, "near": false })).await;
        let got: Vec<Value> = imported["map"]["root"]["children"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["kind"] == "idea")
            .map(shape)
            .collect();
        assert_eq!(got, expected, ".{ending}: Ausgabe → Einlesen ergibt dieselbe Struktur in derselben Reihenfolge");
        // Round 36: what lay in the cloud lies in the cloud again (the round trip of the own file preserves it).
        let got_cloud: Vec<Value> = imported["cloud"]["ideas"].as_array().unwrap().iter().map(shape).collect();
        assert_eq!(got_cloud, expected_cloud, ".{ending}: die Einfälle der Wolke liegen wieder in der Wolke");
        assert_eq!(imported["cloud"]["count"], 1, ".{ending}: sonst liegt nichts in der Wolke");
        let links = result(&b.server.rpc("idea.links", json!({ "work": b.work })).await)["links"].clone();
        assert_eq!(links.as_array().map(Vec::len), Some(1), ".{ending}: eine Verbindung angelegt");
        assert_eq!(
            links[0]["note"], "führt weiter: Tränke",
            ".{ending}: die Beschriftung ist die Notiz der Verbindung"
        );
        let zaun = b.id_of("Zaun").await;
        let idea = b.call("idea.get", json!({ "id": zaun })).await["idea"].clone();
        assert_eq!(idea["words"], json!(["Holz", "Draht"]), ".{ending}: Schlagwörter reisen mit");
        let fenne = b.id_of("Fenne").await;
        assert_eq!(b.call("idea.get", json!({ "id": fenne })).await["idea"]["note"], "nasse Wiese", ".{ending}: Notiz");
        assert_eq!(
            b.keys.len(),
            result(&b.server.rpc("work.get", json!({ "work": b.work })).await)["work"]["nodes"]
                .as_array()
                .unwrap()
                .len(),
            ".{ending}: das Einlesen legt nie Gliederungspunkte an"
        );
    }
}

// ---- Existing data of the idea map ---------------------------------------------------------------------------

#[tokio::test]
async fn ideas_links_and_positions_of_the_old_map_arrive_unchanged_and_loose_ideas_lie_in_the_cloud() {
    // Ideas as they existed before Round 34: only keywords, with or without reference, plus link and position.
    let b = open_book("halligbuch.typ", "Halligbuch").await;
    let w = b.work;
    let placed = b.create(json!({ "words": "Flutmarke, Pegel", "key": b.key("Tide"), "note": "am Sielhaus" })).await;
    let loose = b.create(json!({ "words": "Seenebel" })).await;
    let other = b.create(json!({ "words": "Möwen" })).await;
    let link =
        b.call("idea.link", json!({ "from": placed, "to": loose, "kind": "example_of", "note": "" })).await["link"]["id"]
            .as_i64()
            .unwrap();
    b.call("idea.map_pos_set", json!({ "work": w, "idea": loose, "x": 120.5, "y": -40.0 })).await;

    // The flat view stays (backward compatibility): ideas, link, position.
    let flat = b.call("idea.map", json!({ "work": w, "near": false })).await;
    assert_eq!(flat["ideas"].as_array().map(Vec::len), Some(3), "idea.map nennt weiter alle Einfälle");
    assert_eq!(flat["links"][0]["id"], link, "die Verbindung bleibt");
    assert_eq!(
        (flat["positions"][0]["idea"].as_i64(), flat["positions"][0]["x"].as_f64()),
        (Some(loose), Some(120.5)),
        "die gespeicherte Lage bleibt (in der Mindmap ohne Wirkung)"
    );
    let stored = b.call("idea.get", json!({ "id": placed })).await["idea"].clone();
    assert_eq!(
        (
            stored["title"].as_str(),
            stored["parent"].is_null(),
            stored["position"].as_i64(),
            stored["main_topic"].as_bool()
        ),
        (Some(""), true, Some(0), Some(false)),
        "ein alter Einfall hat keinen Titel, keinen Elternknoten, keine Reihenfolge und ist kein Hauptthema"
    );
    // In the mind map: with reference at the chapter, without reference in the cloud – nothing becomes a main branch
    // unasked.
    assert_eq!(b.place_of("Flutmarke, Pegel").await, (i(placed), b.n("Tide")), "angezeigter Titel = Schlagwörter");
    assert_eq!(b.place_of("Seenebel").await.1, "wolke", "lose Einfälle liegen in der Wolke");
    assert_eq!(b.place_of("Möwen").await.1, "wolke", "alle");
    let svg = b.image("").await;
    assert!(svg.contains("data-cloud-count=\"2\""), "„2 in der Wolke“");
    let mains = svg.matches("mm-main").count();
    assert_eq!(mains, 2, "Hauptäste sind nur die beiden Teile");
    assert!(
        svg.contains(&format!("data-link=\"{link}\" data-from=\"{}\" data-to=\"{}\"", i(placed), i(loose))),
        "die alte Verbindung ist als Beziehungslinie gezeichnet, auch in die Wolke"
    );
    let _ = other;
}
