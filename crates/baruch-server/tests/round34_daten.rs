// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 34 „Mindmap wie XMind“ – Daten (Besitz mindmap_daten): Einfälle als Baum über RPC und MCP, die Mindmap
//! als Baum (`idea_map::tree::tree`, RPC `idea.tree`) samt der Wolke daneben (Nachtrag des Nutzers vom 2026-11-25:
//! Einfälle ohne Platz liegen in der Wolke), Baum-Import in einer Transaktion mit einem Spiegel-Commit, Austausch
//! in Unterversion 9 und der Spiegel `einfaelle.md`. Werk und Einfälle sind frei erfunden; kein Modell, kein Netz.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::idea_map::MapFilter;
use baruch_server::idea_map::tree::{
    self, ImportLink, ImportNode, ImportTree, MAX_IMPORT_NODES, MAX_TREE_DEPTH, TreeKind, TreeNode,
};
use baruch_store::{IdeaOrigin, IdeaStatus};
use serde_json::{Value, json};

const BOOK: &str =
    "#set document(title: \"Leuchtturmbuch\")\n\n= Nebel\n\nText.\n\n== Horn\n\nText.\n\n= Brandung\n\nText.\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Fehlercode einer abgelehnten RPC-Anfrage samt Meldung.
fn error_of(answer: &Value) -> (i64, String) {
    assert!(answer["result"].is_null(), "RPC sollte scheitern: {answer}");
    (answer["error"]["code"].as_i64().unwrap(), answer["error"]["message"].as_str().unwrap_or_default().to_owned())
}

struct Book {
    server: common::Server,
    work: i64,
    nebel: i64,
    horn: i64,
    brandung: i64,
}

async fn book() -> Book {
    let server = common::open().await;
    let work = server.import(BOOK, "leuchtturmbuch.typ").await.work;
    let nodes = result(&server.rpc("work.get", json!({ "work": work })).await)["work"]["nodes"].clone();
    let key = |title: &str| {
        nodes.as_array().unwrap().iter().find(|n| n["title"] == title).unwrap_or_else(|| panic!("kein Punkt {title}"))
            ["key"]
            .as_i64()
            .unwrap()
    };
    Book { nebel: key("Nebel"), horn: key("Horn"), brandung: key("Brandung"), server, work }
}

impl Book {
    async fn create(&self, mut params: Value) -> Value {
        if params.get("work").is_none() && params.get("parent").is_none() {
            params["work"] = json!(self.work);
        }
        result(&self.server.rpc("idea.create", params).await)["idea"].clone()
    }

    async fn id(&self, params: Value) -> i64 {
        self.create(params).await["id"].as_i64().unwrap()
    }

    /// Spiegelordner des Leuchtturmbuchs (der leere Speicher bringt schon ein Beispielwerk mit).
    fn mirror(&self) -> std::path::PathBuf {
        self.server.mirror_dir_of("leuchtturmbuch")
    }

    fn commits(&self) -> usize {
        common::git(&self.mirror(), &["rev-list", "--count", "HEAD"]).map_or(0, |n| n.trim().parse().unwrap())
    }

    fn last_commit(&self) -> String {
        common::git(&self.mirror(), &["log", "-1", "--format=%an%n%B"]).unwrap_or_default()
    }

    async fn tree(&self, filter: &MapFilter) -> tree::MindMap {
        tree::tree(&self.server.state, self.work, filter).await.expect("tree gelingt für ein vorhandenes Werk")
    }
}

fn quiet() -> MapFilter {
    MapFilter { near: false, ..MapFilter::default() }
}

fn ids(nodes: &[TreeNode]) -> Vec<&str> {
    nodes.iter().map(|n| n.id.as_str()).collect()
}

// ---- Baum ---------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_tree_has_the_work_in_the_middle_chapters_in_order_main_topics_and_children_by_position() {
    let b = book().await;
    assert!(b.mirror().is_dir(), "Vorbedingung: der Spiegel des Werks liegt unter seinem slug");
    let at_section = b.id(json!({ "key": b.horn, "words": "Signal", "note": "Zwei lange Töne." })).await;
    let topic = b.id(json!({ "title": "Seezeichen", "cloud": false })).await;
    let second = b.id(json!({ "title": "Gezeiten", "cloud": false })).await;
    let lamp = b.id(json!({ "title": "Lampe", "parent": topic })).await;
    let lens = b.id(json!({ "title": "Linse", "parent": topic, "before": lamp })).await;
    let wick = b.id(json!({ "title": "Docht", "parent": lamp, "key": b.brandung })).await;
    let loose = b.id(json!({ "words": "Laterne" })).await;

    let map = b.tree(&quiet()).await;
    assert_eq!(map.root.id, "w", "die Wurzel ist das Werk mit der festen Kennung w (Vertrag S1.2)");
    assert_eq!((map.root.kind, map.root.title.as_str()), (TreeKind::Work, "Leuchtturmbuch"), "Werk in der Mitte");
    assert_eq!(map.title, "Leuchtturmbuch", "MindMap::title ist der Werktitel");
    assert_eq!(
        ids(&map.root.children),
        [format!("n:{}", b.nebel), format!("n:{}", b.brandung), format!("i:{topic}"), format!("i:{second}")],
        "Hauptäste: Kapitel in Gliederungsreihenfolge, danach die freien Hauptthemen nach position, dann id"
    );
    let chapter = map.find(&format!("n:{}", b.nebel)).expect("das Kapitel ist ein Knoten n:<key>");
    assert_eq!(chapter.kind, TreeKind::Chapter, "Kapitel als Hauptast");
    assert!(!chapter.editable, "Kapitel sind in der Mindmap nicht änderbar (Gliederung = Antragsregel)");
    assert!(map.find(&format!("n:{}", b.horn)).is_none(), "ein Abschnitt unter einem Kapitel ist kein eigener Knoten");
    assert_eq!(ids(&chapter.children), [format!("i:{at_section}")], "ein Einfall mit Bezug hängt am Kapitel darüber");

    let idea = map.find(&format!("i:{at_section}")).unwrap();
    assert_eq!((idea.idea, idea.key), (Some(at_section), Some(b.horn)), "id und genauer Bezug stehen am Knoten");
    assert_eq!(idea.title, "Signal", "ohne eigenen Titel zeigt der Knoten die Schlagwörter");
    assert!(idea.own_title.is_empty(), "der gespeicherte Titel ist leer");
    assert!(idea.editable && idea.has_note, "Einfälle sind änderbar; die Notiz ergibt das Notiz-Symbol");
    assert_eq!(idea.status, Some(IdeaStatus::Open), "Status am Knoten");
    assert!(
        idea.section_title.as_deref().is_some_and(|t| t.contains("Horn")),
        "der Titel des Abschnitts im Bezug steht am Einfall: {:?}",
        idea.section_title
    );

    let main = map.find(&format!("i:{topic}")).unwrap();
    assert_eq!((main.title.as_str(), main.own_title.as_str()), ("Seezeichen", "Seezeichen"), "Titel = eigener Titel");
    assert_eq!(
        ids(&main.children),
        [format!("i:{lens}"), format!("i:{lamp}")],
        "Unterknoten nach position: die Linse wurde mit before vor die Lampe gesetzt"
    );
    assert_eq!(main.descendants, 3, "Nachfahren der Art Einfall, auch tiefere (Linse, Lampe, Docht)");
    let deep = map.find(&format!("i:{wick}")).unwrap();
    assert_eq!(deep.key, Some(b.brandung), "ein Unterknoten darf daneben einen Bezug tragen");
    assert!(
        map.find(&format!("n:{}", b.brandung)).unwrap().children.is_empty(),
        "mit Elternknoten ist der Bezug nur der Link zum Abschnitt: der Docht hängt nicht am Kapitel"
    );
    assert!(deep.section_title.as_deref().is_some_and(|t| t.contains("Brandung")), "Titel des verlinkten Abschnitts");
    let order: Vec<(&str, usize)> = map.walk().into_iter().map(|(n, d)| (n.id.as_str(), d)).collect();
    assert_eq!(order[0], ("w", 0), "walk beginnt mit der Wurzel");
    assert!(
        order.contains(&(format!("i:{wick}").as_str(), 3)),
        "walk nennt die Tiefe: Werk 0, Hauptthema 1, Lampe 2, Docht 3: {order:?}"
    );

    // Die Wolke: der Einfall ohne Platz steht nicht im Baum, sondern daneben.
    assert!(map.find(&format!("i:{loose}")).is_none(), "ein Einfall ohne Platz steht nicht im Baum");
    assert!(map.find("none").is_none(), "die Wolke none der flachen Karte gibt es im Baum nicht");
    assert_eq!(map.total_ideas, 6, "total_ideas zählt die gezeigten Einfälle im Baum, ohne die Wolke");
    assert_eq!(map.root.descendants, 6, "die Wurzel zählt alle Einfälle im Baum");
    let cloud = tree::cloud(&b.server.state, b.work, &quiet()).await.unwrap();
    assert_eq!(cloud.ideas.iter().map(|n| n.id.clone()).collect::<Vec<_>>(), [format!("i:{loose}")], "die Wolke");
    assert_eq!((cloud.count, cloud.count_with_children), (1, 1), "Zähler „1 in der Wolke“");
    assert!(!cloud.ideas[0].has_note, "ohne Notiz, Zitat, Quelle und Bibelstelle kein Notiz-Symbol");
}

#[tokio::test]
async fn the_cloud_holds_ideas_without_a_place_and_moving_connects_it_with_the_mindmap() {
    let b = book().await;
    let topic = b.id(json!({ "title": "Hafen", "words": "Schiff", "cloud": false })).await;
    let a = b.id(json!({ "words": "Schiff, Tau", "note": "Festmachen." })).await;
    let c = b.id(json!({ "title": "Möwe" })).await;
    let below = b.id(json!({ "title": "Feder", "parent": c })).await;
    let state = &b.server.state;

    let cloud = tree::cloud(state, b.work, &quiet()).await.unwrap();
    assert_eq!(ids(&cloud.ideas), [format!("i:{a}"), format!("i:{c}")], "neue Einfälle ohne Platz landen in der Wolke");
    assert_eq!((cloud.count, cloud.count_with_children), (2, 3), "zwei in der Wolke, mit dem Unterknoten drei");
    assert_eq!(ids(&cloud.ideas[1].children), [format!("i:{below}")], "ein Einfall der Wolke behält seine Unterknoten");
    // Nähe als Hilfe: „passt zu Ast …“ aus gleichen Schlagwörtern, auch wenn der Filter die Nähe im Bild ausschaltet.
    let hint = cloud.hints.iter().find(|h| h.idea == a).expect("Vorschlag für den Einfall mit gleichem Schlagwort");
    assert_eq!(
        (hint.node.as_str(), hint.branch.as_str()),
        (format!("i:{topic}").as_str(), format!("i:{topic}").as_str()),
        "nah am Hauptthema"
    );
    assert_eq!(
        (hint.branch_title.as_str(), hint.shared_words.clone()),
        ("Hafen", vec!["Schiff".to_owned()]),
        "Ast und Grund"
    );
    assert!(cloud.hints.iter().all(|h| h.idea != c), "ohne Nähe kein Vorschlag");

    // Aus der Wolke auf einen Knoten ziehen: Unterknoten; die Wolke zählt einen weniger.
    let moved = result(&b.server.rpc("idea.move", json!({ "id": a, "parent": topic })).await).clone();
    assert_eq!((moved["changed"].as_bool(), moved["idea"]["parent"].as_i64()), (Some(true), Some(topic)), "{moved}");
    assert_eq!(moved["idea"]["cloud"], false, "nicht mehr in der Wolke");
    let (map, cloud) = tree::tree_and_cloud(state, b.work, &quiet()).await.unwrap();
    assert_eq!(
        ids(&map.find(&format!("i:{topic}")).unwrap().children),
        [format!("i:{a}")],
        "hängt jetzt in der Mindmap"
    );
    assert_eq!(cloud.count, 1, "„1 in der Wolke“");

    // Einen Knoten in die Wolke ziehen: Er löst sich mit seinen Unterknoten (der Teilbaum bleibt beisammen).
    let back = result(&b.server.rpc("idea.move", json!({ "id": topic, "cloud": true })).await).clone();
    assert_eq!(back["idea"]["cloud"], true, "{back}");
    let (map, cloud) = tree::tree_and_cloud(state, b.work, &quiet()).await.unwrap();
    assert!(
        map.find(&format!("i:{topic}")).is_none() && map.find(&format!("i:{a}")).is_none(),
        "samt Unterknoten aus dem Baum"
    );
    let in_cloud = cloud.ideas.iter().find(|n| n.id == format!("i:{topic}")).expect("liegt in der Wolke");
    assert_eq!(ids(&in_cloud.children), [format!("i:{a}")], "mit seinem Unterknoten");
    assert_eq!((cloud.count, cloud.count_with_children), (2, 4), "zwei in der Wolke, vier mit Unterknoten");

    // RPC idea.tree liefert beides in einer Antwort.
    let answer = result(&b.server.rpc("idea.tree", json!({ "work": b.work, "near": false })).await).clone();
    assert_eq!(answer["map"]["root"]["id"], "w", "idea.tree: die Mindmap");
    assert_eq!(answer["cloud"]["count"], 2, "idea.tree: die Wolke mit Zähler");
    assert_eq!(answer["cloud"]["ideas"][0]["kind"], "idea", "Knoten der Wolke sind Einfälle");
    let (code, _) = error_of(&b.server.rpc("idea.tree", json!({})).await);
    assert_eq!(code, -32602, "idea.tree ohne Werk: falsche Parameter");

    // Ohne JavaScript: das Formular der Seite des Einfalls legt in die Wolke bzw. hängt unter einen Einfall.
    let (status, page) = b.server.get(&format!("/einfaelle/{a}")).await;
    assert_eq!(status, StatusCode::OK, "Seite des Einfalls");
    assert!(page.contains("Platz in der Mindmap") && page.contains("In die Wolke legen"), "Formulare für den Platz");
    assert!(
        page.contains(&format!("Unterknoten von <a href=\"/einfaelle/{topic}\">Einfall {topic}</a>")),
        "Elternknoten angezeigt"
    );
    let (status, _) = b.server.post_form("/einfaelle/act", &format!("method=idea.move&id={a}&cloud=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "das Formular leitet nach dem Umhängen weiter");
    let stored = result(&b.server.rpc("idea.get", json!({ "id": a })).await)["idea"].clone();
    assert_eq!((stored["cloud"].as_bool(), stored["parent"].is_null()), (Some(true), true), "in der Wolke: {stored}");
    let (status, _) = b.server.post_form("/einfaelle/act", &format!("method=idea.move&id={a}&parent={c}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "„in die Mindmap unter …“");
    let stored = result(&b.server.rpc("idea.get", json!({ "id": a })).await)["idea"].clone();
    assert_eq!(stored["parent"].as_i64(), Some(c), "unter den gewählten Einfall gehängt: {stored}");
}

#[tokio::test]
async fn a_filter_keeps_ancestors_of_visible_ideas_as_ghosts_and_the_flat_map_still_works() {
    let b = book().await;
    let topic = b.id(json!({ "title": "Werft", "cloud": false })).await;
    let middle = b.id(json!({ "title": "Helling", "parent": topic })).await;
    let done = b.id(json!({ "title": "Kiel", "words": "Eiche", "parent": middle })).await;
    let other = b.id(json!({ "title": "Segel", "parent": topic })).await;
    result(&b.server.rpc("idea.update", json!({ "id": done, "status": "done" })).await);

    let filter = MapFilter { status: Some(IdeaStatus::Done), ..quiet() };
    let map = b.tree(&filter).await;
    let (top, mid, leaf) = (
        map.find(&format!("i:{topic}")).expect("Vorfahre bleibt stehen"),
        map.find(&format!("i:{middle}")).expect("auch der mittlere"),
        map.find(&format!("i:{done}")).expect("der passende Einfall"),
    );
    assert!(top.ghost && mid.ghost && !leaf.ghost, "Vorfahren eines sichtbaren Treffers bleiben als Geist stehen");
    assert!(map.find(&format!("i:{other}")).is_none(), "ein Zweig ohne passenden Einfall fällt weg");
    assert_eq!(map.total_ideas, 1, "gezählt wird nur, was zum Filter passt (keine Geister)");
    assert_eq!(top.descendants, 2, "descendants zählt, was im Baum steht (Helling und Kiel)");

    let by_word = b.tree(&MapFilter { word: Some("eiche".into()), ..quiet() }).await;
    assert!(by_word.find(&format!("i:{done}")).is_some_and(|n| !n.ghost), "Filter nach Schlagwort (ohne Groß/klein)");
    // q blendet nichts aus, es markiert Treffer – auch über den Titel (Volltext).
    let searched = b.tree(&MapFilter { q: Some("Helling".into()), ..quiet() }).await;
    assert!(searched.find(&format!("i:{middle}")).is_some_and(|n| n.hit), "Treffer der Suche über den Titel");
    assert!(
        searched.find(&format!("i:{other}")).is_some_and(|n| !n.hit && !n.ghost),
        "der Rest bleibt sichtbar, ohne hit"
    );

    // Die flache Sicht idea.map bleibt (Abwärtsverträglichkeit) und nennt je Einfall Titel und Platz im Baum.
    let flat = result(&b.server.rpc("idea.map", json!({ "work": b.work, "near": false })).await).clone();
    let ideas = flat["ideas"].as_array().expect("idea.map liefert die Einfälle");
    assert_eq!(ideas.len(), 4, "alle vier Einfälle in der flachen Sicht");
    let kiel = ideas.iter().find(|i| i["id"] == done).unwrap();
    assert_eq!(
        (kiel["title"].as_str(), kiel["parent"].as_i64(), kiel["label"].as_str()),
        (Some("Kiel"), Some(middle), Some("Kiel")),
        "MapIdea trägt title und parent; die Beschriftung ist der Titel: {kiel}"
    );
    assert!(kiel["position"].is_i64(), "und position: {kiel}");
    assert!(flat["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "w"), "die Knoten der flachen Karte sind da");
}

// ---- RPC ----------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn create_takes_title_parent_and_before_and_refuses_an_idea_without_title_and_words() {
    let b = book().await;
    let root = b.create(json!({ "title": "  Deich  ", "cloud": false })).await;
    assert_eq!(root["title"], "Deich", "der Titel wird gesäubert gespeichert");
    assert_eq!(root["words"], json!([]), "words darf fehlen, wenn ein Titel gesetzt ist");
    assert_eq!(
        (root["parent"].is_null(), root["main_topic"].as_bool()),
        (true, Some(true)),
        "freies Hauptthema: {root}"
    );
    let root_id = root["id"].as_i64().unwrap();
    let before = b.commits();
    // parent ohne work: das Werk kommt vom Elternknoten.
    let child = result(&b.server.rpc("idea.create", json!({ "title": "Schaf", "parent": root_id })).await).clone();
    assert_eq!(child["idea"]["work"].as_i64(), Some(b.work), "work aus dem Elternknoten: {child}");
    assert_eq!(child["idea"]["parent"].as_i64(), Some(root_id), "Unterknoten angelegt");
    assert!(child["idea"]["position"].is_i64(), "die Antwort trägt position: {child}");
    assert_eq!(b.commits(), before + 1, "ein Commit je Anlegen");
    let log = b.last_commit();
    assert!(
        log.contains("Baruch-Action: idea-create") && log.contains("Baruch-Origin: human"),
        "Herkunft Mensch: {log}"
    );
    let first = child["idea"]["id"].as_i64().unwrap();
    let front = b.create(json!({ "title": "Zaun", "parent": root_id, "before": first })).await;
    assert!(
        front["position"].as_i64()
            < result(&b.server.rpc("idea.get", json!({ "id": first })).await)["idea"]["position"].as_i64(),
        "before ordnet vor das Geschwister"
    );

    let (code, message) =
        error_of(&b.server.rpc("idea.create", json!({ "work": b.work, "note": "nur ein Gedanke" })).await);
    assert_eq!(code, -32602, "ohne Titel und Schlagwort abgelehnt");
    assert!(message.contains("Schlagwort") && message.contains("Titel"), "die Meldung nennt beides: {message}");
    let (code, _) = error_of(&b.server.rpc("idea.create", json!({ "work": b.work, "title": "x".repeat(201) })).await);
    assert_eq!(code, -32602, "ein Titel über 200 Zeichen wird abgelehnt");
    let (code, _) =
        error_of(&b.server.rpc("idea.create", json!({ "title": "Quer", "parent": root_id, "cloud": true })).await);
    assert_eq!(code, -32602, "in die Wolke und zugleich unter einen Knoten widerspricht sich");

    // Umbenennen ist idea.update mit title; die Antwort trägt den neuen Titel, der Commit nennt alt und neu.
    let renamed = result(&b.server.rpc("idea.update", json!({ "id": first, "title": "Lamm" })).await).clone();
    assert_eq!(
        (renamed["changed"].as_bool(), renamed["idea"]["title"].as_str()),
        (Some(true), Some("Lamm")),
        "{renamed}"
    );
    let log = b.last_commit();
    assert!(
        log.contains("Titel vorher: Schaf") && log.contains("Titel nachher: Lamm"),
        "Commit nennt die Änderung: {log}"
    );
}

#[tokio::test]
async fn move_rehangs_and_orders_in_one_commit_and_refuses_what_the_contract_forbids() {
    let b = book().await;
    let root = b.id(json!({ "title": "Fluss", "cloud": false })).await;
    let ferry = b.id(json!({ "title": "Fähre", "parent": root })).await;
    let rope = b.id(json!({ "title": "Seil", "parent": ferry })).await;
    let bank = b.id(json!({ "title": "Ufer", "parent": root })).await;
    let before = b.commits();

    let moved =
        result(&b.server.rpc("idea.move", json!({ "id": bank, "parent": root, "before": ferry })).await).clone();
    assert_eq!(moved["changed"], true, "neu geordnet: {moved}");
    assert!(moved["commit"].is_string(), "mit Commit: {moved}");
    assert_eq!(b.commits(), before + 1, "genau ein Commit für Umhängen und Ordnen");
    let log = b.last_commit();
    assert!(log.contains(&format!("Einfall {bank}: umgehängt")), "Betreff „umgehängt“: {log}");
    assert!(log.contains("Baruch-Action: idea-move") && log.contains("Baruch-Origin: human"), "Herkunft Mensch: {log}");
    let same = result(&b.server.rpc("idea.move", json!({ "id": bank, "before": ferry })).await).clone();
    assert_eq!(same["changed"], false, "steht schon dort: {same}");
    assert_eq!(b.commits(), before + 1, "ohne Änderung kein Commit");

    // Auf ein Kapitel: parent null, key gesetzt; aufs Werk: beides null.
    let at = result(&b.server.rpc("idea.move", json!({ "id": bank, "parent": null, "key": b.nebel })).await).clone();
    assert_eq!((at["idea"]["parent"].is_null(), at["idea"]["key"].as_i64()), (true, Some(b.nebel)), "{at}");
    let at = result(&b.server.rpc("idea.move", json!({ "id": bank, "parent": null, "key": null })).await).clone();
    assert_eq!(
        (at["idea"]["main_topic"].as_bool(), at["idea"]["cloud"].as_bool()),
        (Some(true), Some(false)),
        "aufs Werk: {at}"
    );

    // Ablehnungen (Code „abgelehnt“), nichts geändert.
    let second = b.server.import("#set document(title: \"Zweitwerk\")\n\n= Eins\n\nText.\n", "zweit.typ").await.work;
    let foreign = result(&b.server.rpc("idea.create", json!({ "work": second, "title": "Fremd" })).await)["idea"]["id"]
        .as_i64()
        .unwrap();
    let commits = b.commits();
    for (params, part, why) in [
        (json!({ "id": rope, "parent": foreign }), "anderen Werk", "Elternknoten in einem anderen Werk"),
        (json!({ "id": ferry, "parent": rope }), "Unterknoten", "eigener Nachfahre (Kreis)"),
        (json!({ "id": ferry, "parent": ferry }), "sich selbst", "sich selbst als Elternknoten"),
    ] {
        let (code, message) = error_of(&b.server.rpc("idea.move", params).await);
        assert_eq!(code, baruch_server::rpc::REFUSED, "{why}: abgelehnt, nicht „falsche Parameter“: {message}");
        assert!(message.contains(part), "{why}: Meldung nennt „{part}“: {message}");
    }
    result(&b.server.rpc("idea.delete", json!({ "id": rope, "confirm": true })).await);
    let commits = commits + 1;
    let (code, message) = error_of(&b.server.rpc("idea.move", json!({ "id": rope, "parent": root })).await);
    assert_eq!(code, baruch_server::rpc::REFUSED, "gelöschter Einfall: {message}");
    assert!(message.contains("gelöscht"), "{message}");
    // Tiefe: eine Kette bis 32, der Teilbaum „Fähre“ passt dann nicht mehr darunter.
    let mut last = b.id(json!({ "title": "Kette 1", "cloud": false })).await;
    for n in 2..=32 {
        last = b.id(json!({ "title": format!("Kette {n}"), "parent": last })).await;
    }
    let commits = commits + 32;
    let (code, message) = error_of(&b.server.rpc("idea.move", json!({ "id": ferry, "parent": last })).await);
    assert_eq!(code, baruch_server::rpc::REFUSED, "Tiefe über 32: {message}");
    assert!(message.contains("höchstens 32"), "{message}");
    assert_eq!(b.commits(), commits, "keine der Ablehnungen hat einen Commit hinterlassen");
    let unchanged = result(&b.server.rpc("idea.get", json!({ "id": ferry })).await)["idea"].clone();
    assert_eq!(unchanged["parent"].as_i64(), Some(root), "die Fähre hängt, wo sie hing");

    // Kapitel und Teile lassen sich über idea.* nicht ändern: Ihre Kennung ist kein Einfall.
    let (code, _) = error_of(&b.server.rpc("idea.move", json!({ "id": b.nebel + 100_000, "parent": null })).await);
    assert_eq!(code, -32602, "ein key ist keine Einfall-id: unbekannter Einfall");
    let (code, _) =
        error_of(&b.server.rpc("idea.move", json!({ "id": format!("n:{}", b.nebel), "parent": null })).await);
    assert_eq!(code, -32602, "die Kennung n:<key> eines Kapitels nimmt idea.move nicht an");
    let titles = result(&b.server.rpc("work.get", json!({ "work": b.work })).await)["work"]["nodes"].clone();
    assert!(titles.as_array().unwrap().iter().any(|n| n["title"] == "Nebel"), "die Gliederung ist unberührt");
}

#[tokio::test]
async fn delete_takes_the_children_along_and_restore_brings_exactly_them_back() {
    let b = book().await;
    let root = b.id(json!({ "title": "Mühle", "cloud": false })).await;
    let wheel = b.id(json!({ "title": "Rad", "parent": root })).await;
    let paddle = b.id(json!({ "title": "Schaufel", "parent": wheel })).await;
    let stone = b.id(json!({ "title": "Stein", "parent": root })).await;
    let link = result(
        &b.server.rpc("idea.link", json!({ "from": paddle, "to": stone, "kind": "leads_to", "note": "treibt" })).await,
    )["link"]["id"]
        .as_i64()
        .unwrap();
    result(&b.server.rpc("idea.map_pos_set", json!({ "work": b.work, "idea": paddle, "x": 5, "y": 7 })).await);
    let before = b.commits();

    let gone = result(&b.server.rpc("idea.delete", json!({ "id": wheel, "confirm": true })).await).clone();
    assert_eq!(gone["deleted"], true, "wie bisher: deleted true");
    assert_eq!(gone["deleted_ids"], json!([wheel, paddle]), "die Antwort nennt alle mitgelöschten Einfälle: {gone}");
    assert_eq!(b.commits(), before + 1, "ein Commit für den ganzen Teilbaum");
    let log = b.last_commit();
    assert!(log.contains(&format!("Mit gelöschte Unterknoten: {paddle}")), "der Commit nennt die Unterknoten: {log}");
    let child = result(&b.server.rpc("idea.get", json!({ "id": paddle })).await)["idea"].clone();
    assert_eq!(child["deleted"], gone["idea"]["deleted"], "derselbe Zeitstempel am Unterknoten");
    assert!(b.tree(&quiet()).await.find(&format!("i:{paddle}")).is_none(), "nicht mehr im Baum");

    let back = result(&b.server.rpc("idea.restore", json!({ "id": wheel })).await).clone();
    assert_eq!(back["restored"], json!([wheel, paddle]), "genau diese kommen zurück: {back}");
    assert!(back.get("detached").is_none(), "der Elternknoten ist da: kein detached");
    assert_eq!(b.commits(), before + 2, "ein Commit für das Wiederherstellen");
    let map = b.tree(&quiet()).await;
    assert_eq!(
        ids(&map.find(&format!("i:{wheel}")).unwrap().children),
        [format!("i:{paddle}")],
        "wieder am alten Platz"
    );
    assert!(
        map.links.iter().any(|l| l.id == link && l.note == "treibt"),
        "die Verbindung blieb erhalten und steht wieder in der Mindmap"
    );
    let positions =
        result(&b.server.rpc("idea.map", json!({ "work": b.work, "near": false })).await)["positions"].clone();
    assert!(positions.as_array().unwrap().iter().any(|p| p["idea"] == paddle), "die Lage blieb erhalten: {positions}");

    // Elternknoten noch gelöscht: detached, der Einfall liegt in der Wolke.
    result(&b.server.rpc("idea.delete", json!({ "id": root, "confirm": true })).await);
    let alone = result(&b.server.rpc("idea.restore", json!({ "id": wheel })).await).clone();
    assert_eq!(alone["detached"], true, "der Elternknoten ist noch gelöscht: {alone}");
    assert_eq!((alone["idea"]["parent"].is_null(), alone["idea"]["cloud"].as_bool()), (true, Some(true)), "{alone}");
}

// ---- Import -------------------------------------------------------------------------------------------------------

fn leaf(title: &str) -> ImportNode {
    ImportNode { title: title.to_owned(), ..ImportNode::default() }
}

#[tokio::test]
async fn import_creates_the_tree_in_one_transaction_with_one_commit_and_the_given_origin() {
    let b = book().await;
    let import = ImportTree {
        title: "Fremde Mindmap".into(),
        nodes: vec![
            ImportNode {
                title: "Küste".into(),
                note: "Salz in der Luft.".into(),
                words: vec!["Meer".into()],
                children: vec![ImportNode { children: vec![leaf("Strandhafer")], ..leaf("Düne") }, leaf("Watt")],
                cloud: false,
            },
            leaf("Insel"),
        ],
        links: vec![ImportLink { from: vec![0, 0, 0], to: vec![0, 1], note: "hält den Sand".into() }],
        warnings: vec!["nur ein Hinweis".into()],
    };
    let origin = IdeaOrigin::Persona { handle: "@lektor".into() };
    let before = b.commits();
    let report = tree::import(&b.server.state, b.work, None, &import, &origin).await.expect("der Baum wird angelegt");
    assert_eq!((report.ideas.len(), report.links), (5, 1), "fünf Einfälle, eine Verbindung");
    assert_eq!(b.commits(), before + 1, "genau ein Spiegel-Commit für den ganzen Baum");
    let log = b.last_commit();
    assert!(
        log.lines().next().is_some_and(|author| author.contains("@lektor")),
        "Autor des Commits ist die übergebene Herkunft: {log}"
    );
    assert!(log.contains("Mindmap eingelesen: 5 Einfälle"), "Betreff nennt den Umfang: {log}");
    assert!(log.contains("Baruch-Origin: persona") && log.contains("Baruch-Action: idea-import"), "Trailer: {log}");
    assert!(log.contains("Quelle: „Fremde Mindmap“"), "der Titel der Datei steht im Rumpf: {log}");

    let map = b.tree(&quiet()).await;
    let coast = map.root.children.iter().find(|n| n.title == "Küste").expect("oberste Knoten werden freie Hauptthemen");
    assert_eq!(
        (coast.note.as_str(), coast.words.clone()),
        ("Salz in der Luft.", vec!["Meer".to_owned()]),
        "Notiz, Schlagwörter"
    );
    assert_eq!(coast.origin, Some(origin.clone()), "die Herkunft steht am Einfall");
    assert_eq!(
        coast.children.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
        ["Düne", "Watt"],
        "Reihenfolge der Eingabe"
    );
    assert_eq!(coast.children[0].children[0].title, "Strandhafer", "die Verschachtelung bleibt");
    assert_eq!(map.links.len(), 1, "die Verbindung steht in der Mindmap");
    assert_eq!(
        (map.links[0].kind, map.links[0].note.as_str()),
        (baruch_store::LinkKind::LeadsTo, "hält den Sand"),
        "als leads_to mit der Beschriftung als Notiz"
    );
    let mirror = std::fs::read_to_string(b.mirror().join("einfaelle.md")).expect("einfaelle.md im Spiegel");
    assert!(mirror.contains("- Titel: Strandhafer\n"), "die neuen Einfälle stehen im Spiegel:\n{mirror}");

    // Unter einen Knoten.
    let under = tree::import(
        &b.server.state,
        b.work,
        coast.idea,
        &ImportTree { nodes: vec![leaf("Priel")], ..ImportTree::default() },
        &origin,
    )
    .await
    .unwrap();
    let map = b.tree(&quiet()).await;
    let coast = map.root.children.iter().find(|n| n.title == "Küste").unwrap();
    assert_eq!(coast.children.last().and_then(|n| n.idea), Some(under.ideas[0]), "hinter den vorhandenen Unterknoten");

    // Überschreitungen: abgelehnt, nichts angelegt, kein Commit.
    let (ideas, commits) = (map.total_ideas, b.commits());
    let too_many = ImportTree {
        nodes: (0..=MAX_IMPORT_NODES).map(|n| leaf(&format!("Knoten {n}"))).collect(),
        ..ImportTree::default()
    };
    let error = tree::import(&b.server.state, b.work, None, &too_many, &origin).await.unwrap_err();
    assert_eq!(error.code, baruch_server::rpc::REFUSED, "mehr als MAX_IMPORT_NODES Knoten: abgelehnt");
    assert!(error.message.contains("2000") && error.message.contains("Nichts angelegt"), "{}", error.message);
    let mut deep = leaf("unten");
    for n in 0..MAX_TREE_DEPTH {
        deep = ImportNode { children: vec![deep], ..leaf(&format!("Ebene {n}")) };
    }
    let error = tree::import(
        &b.server.state,
        b.work,
        None,
        &ImportTree { nodes: vec![deep], ..ImportTree::default() },
        &origin,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, baruch_server::rpc::REFUSED, "tiefer als MAX_TREE_DEPTH: abgelehnt");
    assert!(error.message.contains("32"), "{}", error.message);
    let half = ImportTree { nodes: vec![leaf("gültig"), leaf(&"x".repeat(201))], ..ImportTree::default() };
    let error = tree::import(&b.server.state, b.work, None, &half, &origin).await.unwrap_err();
    assert_eq!(error.code, baruch_server::rpc::REFUSED, "ein zu langer Titel lehnt den ganzen Baum ab");
    let empty = tree::import(&b.server.state, b.work, None, &ImportTree::default(), &origin).await.unwrap_err();
    assert_eq!(empty.code, baruch_server::rpc::REFUSED, "ein leerer Baum wird abgelehnt");
    assert_eq!(b.tree(&quiet()).await.total_ideas, ideas, "nichts angelegt – auch nicht der gültige Anfang");
    assert_eq!(b.commits(), commits, "und kein Commit");
}

// ---- MCP ----------------------------------------------------------------------------------------------------------

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

async fn tool(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    mcp(
        server,
        Some(session),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": name, "arguments": arguments } }),
    )
    .await
    .1
}

async fn tool_ok(server: &common::Server, session: &str, name: &str, arguments: Value) -> Value {
    let answer = tool(server, session, name, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{name} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

#[tokio::test]
async fn mcp_creates_subnodes_moves_them_and_reads_the_tree_with_its_origin_in_the_mirror() {
    let b = book().await;
    let (headers, answer) = mcp(
        &b.server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Probe Client", "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    let hint = answer["result"]["instructions"].as_str().unwrap_or_default();
    assert!(
        hint.contains("idea_tree") && hint.contains("idea_move") && hint.contains("cloud"),
        "der Hinweis nennt die Mindmap: {hint}"
    );
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let s = session.as_str();

    let (_, listed) = mcp(&b.server, None, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tools = listed["result"]["tools"].as_array().unwrap();
    let by_name =
        |name: &str| tools.iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("{name} fehlt in tools/list"));
    for name in ["idea_move", "idea_tree"] {
        assert!(by_name(name)["description"].as_str().is_some_and(|d| d.len() > 40), "{name} mit Beschreibung");
    }
    for name in ["idea_create", "add_idea"] {
        let properties = &by_name(name)["inputSchema"]["properties"];
        for field in ["title", "parent", "before", "cloud"] {
            assert!(properties[field].is_object(), "{name} kennt {field}: {properties}");
        }
    }
    assert!(by_name("idea_update")["inputSchema"]["properties"]["title"].is_object(), "idea_update kennt title");
    assert!(
        by_name("idea_delete")["description"].as_str().unwrap().contains("sub-nodes"),
        "idea_delete sagt, dass Unterknoten mitgelöscht werden"
    );
    assert!(
        by_name("idea_map")["description"].as_str().unwrap().contains("mind map"),
        "idea_map spricht von der Mindmap"
    );
    // Die Fragmente aus docs/r34 sind eingesetzt: MCP.md nennt die Werkzeuge, RPC.md die Methoden.
    for (file, names) in [("MCP.md", ["idea_move", "idea_tree"]), ("RPC.md", ["idea.move", "idea.tree"])] {
        let docs = std::fs::read_to_string(format!("{}/../../docs/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        for name in names {
            assert!(docs.contains(&format!("`{name}`")), "docs/{file} nennt {name}");
        }
    }

    let before = b.commits();
    let root = tool_ok(&b.server, s, "idea_create", json!({ "work": b.work, "title": "Markt", "cloud": false })).await;
    let root_id = root["idea"]["id"].as_i64().unwrap();
    assert_eq!(root["idea"]["origin"], json!({ "kind": "mcp", "handle": "@mcp:probe-client" }), "Client als Herkunft");
    assert_eq!(
        (root["idea"]["title"].as_str(), root["idea"]["main_topic"].as_bool()),
        (Some("Markt"), Some(true)),
        "{root}"
    );
    let child = tool_ok(&b.server, s, "add_idea", json!({ "title": "Stand", "parent": root_id })).await;
    let child_id = child["idea"]["id"].as_i64().unwrap();
    assert_eq!(
        child["idea"]["parent"].as_i64(),
        Some(root_id),
        "add_idea legt einen Unterknoten an (work vom Elternknoten)"
    );
    let loose = tool_ok(&b.server, s, "idea_create", json!({ "work": b.work, "words": ["Regen"] })).await;
    assert_eq!(loose["idea"]["cloud"], true, "idea_create ohne Platz landet in der Wolke: {loose}");
    let loose_id = loose["idea"]["id"].as_i64().unwrap();
    assert_eq!(b.commits(), before + 3, "je Anlegen ein Commit");

    let renamed = tool_ok(&b.server, s, "idea_update", json!({ "id": child_id, "title": "Obststand" })).await;
    assert_eq!(renamed["idea"]["title"], "Obststand", "idea_update setzt den Titel");
    let moved =
        tool_ok(&b.server, s, "idea_move", json!({ "id": loose_id, "parent": child_id, "persona": "lektor" })).await;
    assert_eq!((moved["changed"].as_bool(), moved["idea"]["parent"].as_i64()), (Some(true), Some(child_id)), "{moved}");
    assert_eq!(b.commits(), before + 5, "Umbenennen und Umhängen je ein Commit");
    let log = b.last_commit();
    assert!(log.contains("umgehängt") && log.contains("Baruch-Action: idea-move"), "Commit des Umhängens: {log}");
    assert!(log.contains("Baruch-Origin: persona") && log.contains("Persona: @lektor"), "Herkunft Persona: {log}");
    let by_client = tool_ok(&b.server, s, "idea_move", json!({ "id": loose_id, "cloud": true })).await;
    assert_eq!(by_client["idea"]["cloud"], true, "idea_move legt in die Wolke");
    let log = b.last_commit();
    assert!(log.contains("Baruch-Origin: mcp") && log.contains("Persona: @mcp:probe-client"), "Herkunft Client: {log}");
    let refused = tool(&b.server, s, "idea_move", json!({ "id": root_id, "parent": child_id })).await;
    assert_eq!(refused["result"]["isError"], true, "ein Kreis ist ein fachlicher Fehler: {refused}");

    let read = tool_ok(&b.server, s, "idea_tree", json!({ "work": b.work })).await;
    assert_eq!(read["map"]["root"]["id"], "w", "idea_tree liefert den Baum");
    let branch =
        read["map"]["root"]["children"].as_array().unwrap().iter().find(|n| n["title"] == "Markt").expect("Hauptthema");
    assert_eq!(branch["children"][0]["title"], "Obststand", "mit Unterknoten");
    assert_eq!(branch["url"], format!("/einfaelle/{root_id}"), "jeder Einfall trägt seine Adresse");
    assert_eq!(read["cloud"]["count"], 1, "und die Wolke daneben: {}", read["cloud"]);
    let commits = b.commits();
    tool_ok(&b.server, s, "idea_tree", json!({ "work": b.work, "status": "open" })).await;
    assert_eq!(b.commits(), commits, "idea_tree liest nur");
    let listed = tool_ok(&b.server, s, "idea_read", json!({ "id": child_id })).await;
    assert!(
        listed["idea"]["position"].is_i64() && listed["idea"]["parent"].is_i64(),
        "idea_read: parent, position: {listed}"
    );

    let gone = tool_ok(&b.server, s, "idea_delete", json!({ "id": root_id })).await;
    assert_eq!(gone["deleted_ids"], json!([root_id, child_id]), "idea_delete nimmt die Unterknoten mit: {gone}");
}

// ---- Austausch, Spiegel, Suche ------------------------------------------------------------------------------------

#[tokio::test]
async fn export_in_version_nine_and_reading_it_back_keeps_title_parent_and_order() {
    let b = book().await;
    let root = b.id(json!({ "title": "Kanal & <Schleuse>", "cloud": false })).await;
    let gate = b.id(json!({ "title": "Tor", "parent": root })).await;
    let lock = b.id(json!({ "title": "Kammer", "parent": root, "before": gate })).await;
    let loose = b.id(json!({ "words": "Treidelpfad" })).await;
    let exported =
        result(&b.server.rpc("work.exchange_export", json!({ "work": b.work, "format": "xml" })).await).clone();
    let xml = exported["xml"].as_str().expect("xml in der Antwort").to_owned();
    assert!(xml.contains("version=\"10\""), "Ausgabe in Unterversion 9");
    assert!(
        xml.contains("titel=\"Kanal &amp; &lt;Schleuse&gt;\"") && xml.contains("hauptthema=\"true\""),
        "Titel und Hauptthema: {xml}"
    );
    assert!(xml.contains(" eltern=\"") && xml.contains(" platz=\""), "Elternknoten und Platz stehen in der Datei");
    // Gültig nach dem XSD der Unterversion 9 (fehlt xmllint, ehrlich übersprungen).
    let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u10.xsd");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("werk.baruch.xml");
    std::fs::write(&file, &xml).unwrap();
    match std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&file)
        .output()
    {
        Ok(output) => assert!(output.status.success(), "xmllint: {}", String::from_utf8_lossy(&output.stderr)),
        Err(_) => eprintln!("xmllint fehlt: Prüfung gegen das XSD übersprungen."),
    }

    // Eine zweite Instanz liest die Datei als neues Werk: Titel, Elternknoten und Reihenfolge sind da.
    let other = common::open().await;
    let data = baruch_store::encode_base64(xml.as_bytes());
    let sha = baruch_exchange::sha256_hex(xml.as_bytes());
    let answer = other.rpc("work.exchange_import", json!({ "data": data, "sha256": sha, "mode": "new" })).await;
    let work = result(&answer)["work"].as_i64().expect("das eingelesene Werk");
    let map = tree::tree(&other.state, work, &quiet()).await.unwrap();
    let branch =
        map.root.children.iter().find(|n| n.title == "Kanal & <Schleuse>").expect("das Hauptthema ist wieder da");
    assert_eq!(
        branch.children.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
        ["Kammer", "Tor"],
        "Elternknoten und Reihenfolge überstehen die Rundreise (ids sind dort andere als {root}, {lock}, {gate})"
    );
    let cloud = tree::cloud(&other.state, work, &quiet()).await.unwrap();
    assert_eq!(
        cloud.ideas.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
        ["Treidelpfad"],
        "die Wolke auch (Einfall {loose})"
    );
}

#[tokio::test]
async fn the_mirror_shows_title_and_tree_and_the_search_finds_an_idea_by_its_title() {
    let b = book().await;
    let root = b.id(json!({ "title": "Glockenturm", "cloud": false })).await;
    let bell = b.id(json!({ "title": "Bronzeglocke", "parent": root, "note": "Gegossen im Regen." })).await;
    let loose = b.id(json!({ "words": "Seil" })).await;
    let mirror = std::fs::read_to_string(b.mirror().join("einfaelle.md")).expect("einfaelle.md im Spiegel des Werks");
    let tree = format!(
        "## Baum\n\n- Einfall {root}: Glockenturm (Hauptthema)\n  - Einfall {bell}: Bronzeglocke\n- Einfall {loose}: Seil (Wolke)\n"
    );
    assert!(mirror.contains(&tree), "der Spiegel zeigt den Baum eingerückt, mit Titeln und Platz:\n{mirror}");
    assert!(mirror.contains("- Titel: Bronzeglocke\n"), "und den Titel im Eintrag des Einfalls:\n{mirror}");
    assert!(mirror.contains(&format!("- Unterknoten von: Einfall {root}\n")), "und den Elternknoten:\n{mirror}");

    let found = result(&b.server.rpc("idea.search", json!({ "query": "Bronzeglocke", "work": b.work })).await).clone();
    let hits: Vec<i64> = found["ideas"].as_array().unwrap().iter().map(|i| i["id"].as_i64().unwrap()).collect();
    assert_eq!(hits, [bell], "idea.search findet den Einfall über seinen Titel");
    assert_eq!(found["ideas"][0]["title"], "Bronzeglocke", "die Liste trägt den Titel");
    let listed = result(&b.server.rpc("idea.list", json!({ "work": b.work })).await).clone();
    assert!(
        listed["ideas"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["title"].is_string() && i["position"].is_i64() && i.get("parent").is_some()),
        "idea.list: jede Antwort mit idea trägt title, parent, position"
    );

    // Die Seiten zeigen Titel und Platz, ohne JavaScript.
    let (status, page) = b.server.get(&format!("/einfaelle?work={}", b.work)).await;
    assert_eq!(status, StatusCode::OK, "Liste der Einfälle");
    assert!(page.contains("Bronzeglocke") && page.contains(">Wolke<"), "Titel und „Wolke“ an den Karten");
    assert!(page.contains(">Mindmap</a>"), "der Bandlink heißt „Mindmap“");
    assert!(page.contains("name=\"title\""), "das Formular „Neuer Einfall“ hat ein Titelfeld");
    let form = format!("method=idea.create&work={}&title=Kl%C3%B6ppel&words=", b.work);
    let (status, location) = b.server.post_form("/einfaelle/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "anlegen nur mit Titel über das Formular: {location:?}");
    let found = result(&b.server.rpc("idea.search", json!({ "query": "Klöppel", "work": b.work })).await).clone();
    assert_eq!(found["ideas"].as_array().unwrap().len(), 1, "der Einfall nur mit Titel ist angelegt und auffindbar");
}
