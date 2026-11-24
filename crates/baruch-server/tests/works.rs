// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mehrere Werke in Server und Oberfläche (Vertrag B1.5, Abschnitt 4; Runde 4): Werkwahl ohne
//! JavaScript, `work` in Adressen, Formularen und RPC, eigene Spiegel je Werk, Prüfung des
//! Spiegelordners; Migration 0005 auf einer Datenbank im Stand davor.

use crate::common;

use axum::http::StatusCode;
use baruch_server::rpc;
use serde_json::json;

#[tokio::test]
async fn the_page_of_an_imported_work_shows_its_title_and_outline() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let (status, html) = server.get(&format!("/?work={}", report.work)).await;
    assert_eq!(status, StatusCode::OK, "Seite des importierten Werks lädt");
    assert!(html.contains("<span class=\"work-title\">Das Gartenbuch</span>"), "Titel des Werks in der Kopfzeile");
    assert!(html.contains("· Grete Gärtner"), "Autor aus #set document");
    for title in ["Frühling", "Aussaat", "Tiefe", "Sommer"] {
        assert!(html.contains(title), "Gliederung zeigt „{title}“");
    }
    let work_link = format!("href=\"/?work={}&key=", report.work);
    assert!(html.contains(&work_link), "Links der Gliederung tragen das Werk und den Schlüssel: {work_link}");
    assert!(!html.contains("href=\"/?node="), "kein Link ohne Werk in Gliederung, Editor, Kopfzeile");
    assert!(html.contains(&format!("name=\"work\" value=\"{}\"", report.work)), "Formulare tragen das Werk");
    assert!(html.contains("Die Beete werden umgegraben."), "erster Abschnitt im Editor");
}

#[tokio::test]
async fn the_work_selector_is_a_plain_get_form_listing_all_works() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let (_, html) = server.get(&format!("/?work={}", report.work)).await;
    assert!(html.contains("<form class=\"work-select\" action=\"/\" method=\"get\""), "Werkwahl ist ein GET-Formular");
    assert!(html.contains("<select name=\"work\""), "Auswahlfeld heißt work, damit ?work= entsteht");
    assert!(
        html.contains(&format!(
            "<option value=\"{}\" selected>Das Gartenbuch · Grete Gärtner · 4 Abschnitte</option>",
            report.work
        )),
        "das gezeigte Werk ist gewählt und nennt Titel, Autor und Abschnitte: {html}"
    );
    assert!(html.contains(&format!("<option value=\"{}\">", server.state.work)), "Vorgabewerk steht zur Wahl");
    assert!(
        html.contains("<noscript><button class=\"btn\" type=\"submit\">Öffnen</button></noscript>"),
        "Knopf zum Öffnen nur ohne JavaScript (mit JavaScript öffnet die Auswahl sofort)"
    );
    assert!(html.contains("data-widget=\"work-select\""), "die Auswahl ist das Widget work-select");

    let (_, default) = server.get("/").await;
    assert!(
        default.contains(&format!("<option value=\"{}\" selected>", server.state.work)),
        "ohne ?work= das Vorgabewerk"
    );
    assert!(!default.contains("<span class=\"work-title\">Das Gartenbuch</span>"), "ohne ?work= nicht das neue Werk");
}

#[tokio::test]
async fn an_unknown_work_is_not_found() {
    let server = common::open().await;
    let (status, body) = server.get("/?work=999").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
    assert_eq!(body, "unbekanntes Werk", "deutscher Text im Rumpf der 404");
    let answer = server.rpc("work.get", json!({ "work": 999 })).await;
    assert_eq!(answer["error"]["code"], rpc::INVALID_PARAMS, "RPC: unbekanntes Werk → -32602: {answer}");
    let bad = server.rpc("node.get", json!({ "work": "eins", "node": 0 })).await;
    assert_eq!(bad["error"]["code"], rpc::INVALID_PARAMS, "work muss eine Zahl sein: {bad}");
}

#[tokio::test]
async fn work_list_names_every_work_with_its_outline_size() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let answer = server.rpc("work.list", json!({})).await;
    let works = answer["result"]["works"].as_array().unwrap();
    assert_eq!(works.len(), 2, "Beispielwerk und Import: {answer}");
    let garden = works.iter().find(|w| w["id"] == report.work).unwrap();
    assert_eq!(garden["slug"], "das-gartenbuch", "slug aus dem Titel: {answer}");
    assert_eq!(garden["author"], "Grete Gärtner", "Autor aus #set document: {answer}");
    assert_eq!(garden["nodes"], 4, "Anzahl der Knoten");
}

#[tokio::test]
async fn rpc_methods_act_on_the_named_work_and_default_to_the_first() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let node = server.rpc("node.get", json!({ "work": report.work, "node": 1 })).await;
    assert_eq!(node["result"]["title"], "Aussaat", "node.get im genannten Werk: {node}");
    let default = server.rpc("node.get", json!({ "node": 1 })).await;
    assert_ne!(default["result"]["title"], "Aussaat", "ohne work das Vorgabewerk");

    let added = server.rpc("post.add", json!({ "work": report.work, "node": 1, "body": "Wann säen?" })).await;
    assert_eq!(added["result"]["node"], 1, "post.add nennt den Abschnitt im genannten Werk: {added}");
    let node = server.rpc("node.get", json!({ "work": report.work, "node": 1 })).await;
    assert_eq!(node["result"]["posts"][0]["body"], "Wann säen?", "Beitrag landet im genannten Werk");
}

#[tokio::test]
async fn ui_render_returns_a_url_with_work_and_search() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let ui = server.rpc("ui.render", json!({ "work": report.work, "node": 2, "lq": "Zwei Zentimeter" })).await;
    let url = ui["result"]["url"].as_str().unwrap();
    let k = key_at(&server, report.work, 2).await;
    assert_eq!(
        url,
        format!("/?work={}&key={k}&rev=1&lq=Zwei%20Zentimeter", report.work),
        "Adresse trägt work, key und lq"
    );
    let tree = ui["result"]["tree"].as_str().unwrap();
    // Das erste „&“ steht wörtlich in der Vorlage, das vor lq kommt maskiert aus der Variablen.
    assert!(
        tree.contains(&format!(
            "href=\"/?work={}&key={}&amp;lq=Zwei%20Zentimeter\"",
            report.work,
            key_at(&server, report.work, 0).await
        )),
        "Links der Gliederung behalten die Suche: {tree}"
    );
    assert!(ui["result"]["editor"].as_str().unwrap().contains("Zwei Zentimeter."), "Unterabschnitt im Editor");
    assert_eq!(ui["result"]["title"], "Tiefe – Baruch", "Seitentitel aus dem gezeigten Abschnitt");
}

#[tokio::test]
async fn forms_without_javascript_keep_work_and_search() {
    let server = common::open().await;
    let report = server.import_garden().await;
    let form = format!("method=revision.save&work={}&node=3&body=%3D+Sommer%0A%0AHitze.%0A&lq=Hitze", report.work);
    let (status, location) = server.post_form("/act", &form).await;
    assert!(status.is_redirection(), "Weiterleitung erwartet: {status}");
    assert_eq!(
        location,
        Some(format!("/?work={}&key={}&rev=2&lq=Hitze", report.work, key_at(&server, report.work, 3).await)),
        "zurück ins selbe Werk, Suche bleibt"
    );
    let node = server.rpc("node.get", json!({ "work": report.work, "node": 3 })).await;
    assert_eq!(node["result"]["revisions"][1]["body"], "= Sommer\n\nHitze.\n", "gespeichert im genannten Werk");
    assert_eq!(common::currents(&node["result"]), 1, "genau eine aktuelle Revision");
}

#[tokio::test]
async fn opening_the_data_folder_mirrors_every_work() {
    let server = common::open().await;
    // Import nur in die Datenbank, ohne Spiegel: der nächste Start holt ihn nach.
    let imported = server.state.store.import_typst(common::GARDEN, "gartenbuch.typ", "Autor").await.unwrap();
    let dir = server.mirror_dir_of(&imported.slug);
    assert!(!dir.exists(), "vor dem Start gibt es den Spiegel noch nicht");
    drop(baruch_server::AppState::open(server.path()).await.unwrap());
    assert!(dir.join("main.typ").is_file(), "Start legt den Spiegel jedes Werks an");
    let Some(log) = common::git(&dir, &["log", "--format=%s"]) else { return };
    assert_eq!(log, "Stand der Datenbank gespiegelt\n", "Start-Commit im neuen Spiegel");
}

#[test]
fn mirror_dir_accepts_only_plain_slugs() {
    let data = tempfile::tempdir().unwrap();
    assert_eq!(
        baruch_server::mirror_dir(data.path(), "das-gartenbuch-2").unwrap(),
        data.path().join("works/das-gartenbuch-2"),
        "ein gültiger slug liegt unter works/"
    );
    for bad in ["", "../x", "a--b", "-a", "a-", "Gross", "a/b", "ä", &"a".repeat(49)] {
        assert!(baruch_server::mirror_dir(data.path(), bad).is_err(), "Ordnername {bad:?} muss abgelehnt werden");
    }
    assert!(baruch_server::mirror_dir(data.path(), &"a".repeat(48)).is_ok(), "48 Zeichen sind erlaubt");
}

#[cfg(unix)]
#[test]
fn mirror_dir_refuses_symlinked_folders() {
    let data = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), data.path().join("works")).unwrap();
    let err = baruch_server::mirror_dir(data.path(), "werk").unwrap_err();
    assert!(err.contains("symbolische Verknüpfung"), "works/ als Verknüpfung abgelehnt: {err}");

    let data = tempfile::tempdir().unwrap();
    std::fs::create_dir(data.path().join("works")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), data.path().join("works/werk")).unwrap();
    let err = baruch_server::mirror_dir(data.path(), "werk").unwrap_err();
    assert!(err.contains("symbolische Verknüpfung"), "works/<slug> als Verknüpfung abgelehnt: {err}");
}

// ---- Migration 0005 auf einer Datenbank im Stand davor -------------------------------------------

/// Titel, den beim Nutzer das gesäte Beispielwerk und das importierte Buch gemeinsam hatten.
const OLD_TITLE: &str = "Mövenoog";

/// Ein kleines „importiertes“ Buch, von Hand zerlegt wie `baruch_typst::parse`: Vorlage und drei
/// Überschriften (Titel, Nummer, Elternteil, Stelle unter dem Elternteil, Art, Text).
const OLD_TEMPLATE: &str = "#set document(title: \"Mövenoog\", author: \"Jemand\")\n\n";
type OldHeading = (&'static str, &'static str, Option<i64>, i64, &'static str, &'static str);
const OLD_HEADINGS: [OldHeading; 3] = [
    ("Eins", "1", None, 0, "chapter", "= Eins\n\nText eins.\n\n"),
    ("Eins a", "1.1", Some(0), 0, "section", "== Eins a\n\nMehr.\n\n"),
    ("Zwei", "2", None, 1, "chapter", "= Zwei\n\nText zwei.\n"),
];

/// Migrator mit den eingebetteten Migrationen bis einschließlich `last` (Stand vor einer Migration).
fn partial(migrator: &sqlx::migrate::Migrator, last: i64) -> sqlx::migrate::Migrator {
    sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrator.migrations.iter().filter(|m| m.version <= last).cloned().collect(),
        ),
        ignore_missing: true,
        locking: true,
        no_tx: false,
    }
}

/// Ein importiertes Buch, wie `Store::import_typst` es vor Runde 4 schrieb (ohne Schlüssel).
async fn insert_imported(pool: &sqlx::SqlitePool, slug: &str, sha: &str) -> i64 {
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256) \
         VALUES (?, 'book', ?, 'Jemand', ?, 'moevenoog.typ', ?) RETURNING id",
    )
    .bind(slug)
    .bind(OLD_TITLE)
    .bind(OLD_TEMPLATE)
    .bind(sha)
    .fetch_one(pool)
    .await
    .unwrap();
    for (seq, (title, number, parent, position, kind, body)) in OLD_HEADINGS.iter().enumerate() {
        let node: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent)
        .bind(position)
        .bind(number)
        .bind(title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', 'current', 'Import aus moevenoog.typ')",
        )
        .bind(node)
        .bind(body)
        .execute(pool)
        .await
        .unwrap();
    }
    work
}

/// Das Beispielwerk mit Teilen, Vorschlägen und Beiträgen, wie eine ältere Saat es schrieb (ohne
/// Schlüssel, ohne Quelle), aber mit dem alten Titel, den auch das Buch trägt.
async fn insert_seeded(pool: &sqlx::SqlitePool) -> i64 {
    let sample = baruch_core::sample::work();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author) \
         VALUES ('moevenoog-ein-ganzes-jahr-auf-der', 'book', ?, ?) RETURNING id",
    )
    .bind(OLD_TITLE)
    .bind(&sample.author)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut next_position: std::collections::HashMap<Option<usize>, i64> = std::collections::HashMap::new();
    for node in &sample.nodes {
        let position = next_position.entry(node.parent).or_insert(0);
        let kind = serde_json::to_value(node.kind).unwrap().as_str().unwrap().to_owned();
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind, findings) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(node.id as i64)
        .bind(node.parent.map(|p| p as i64))
        .bind(*position)
        .bind(&node.number)
        .bind(&node.title)
        .bind(kind)
        .bind(node.findings as i64)
        .fetch_one(pool)
        .await
        .unwrap();
        *position += 1;
        for r in &node.revisions {
            let (author_kind, name, model) = match &r.author {
                baruch_core::Author::Human { name } => ("human", name.clone(), None),
                baruch_core::Author::Assistant { persona, model } => {
                    ("assistant", persona.clone(), Some(model.clone()))
                }
            };
            let status = serde_json::to_value(r.status).unwrap().as_str().unwrap().to_owned();
            sqlx::query(
                "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(r.number as i64)
            .bind(&r.body)
            .bind(author_kind)
            .bind(name)
            .bind(model)
            .bind(status)
            .bind(&r.note)
            .execute(pool)
            .await
            .unwrap();
        }
        for (position, post) in node.posts.iter().enumerate() {
            let post_id: i64 = sqlx::query_scalar(
                "INSERT INTO post (node_id, position, author, body, sources) VALUES (?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(id)
            .bind(position as i64)
            .bind(&post.author)
            .bind(&post.body)
            .bind(serde_json::to_string(&post.sources).unwrap())
            .fetch_one(pool)
            .await
            .unwrap();
            for (i, r) in post.refs.iter().enumerate() {
                sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, ?, ?, ?, ?)")
                    .bind(post_id)
                    .bind(i as i64)
                    .bind(r.node as i64)
                    .bind(r.rev as i64)
                    .bind(&r.quote)
                    .execute(pool)
                    .await
                    .unwrap();
            }
        }
    }
    work
}

/// Export eines Werks ohne Vorlage, wie `Store::export_typst` ihn bildet: aktuelle Texte in
/// Baumreihenfolge, ein Zeilenende zwischen Texten, die keins haben.
fn expected_sample_export() -> String {
    fn walk(work: &baruch_core::Work, parent: Option<usize>, out: &mut Vec<usize>) {
        for child in work.children(parent) {
            out.push(child.id);
            walk(work, Some(child.id), out);
        }
    }
    let sample = baruch_core::sample::work();
    let mut order = Vec::new();
    walk(&sample, None, &mut order);
    let bodies: Vec<String> = order
        .iter()
        .filter_map(|&id| {
            let body = sample.nodes[id].current_index().map(|i| sample.nodes[id].revisions[i].body.clone())?;
            // Das Vorwort (id 0) zählt seit Migration 0006 nicht mit; seit Runde 14 (Vertrag §4) projiziert der
            // Export „ohne Nummer“ als Markierung in der ersten Überschriftszeile (die DB bleibt unverändert).
            Some(if id == 0 { baruch_typst::set_numbered_line(&body, false).unwrap() } else { body })
        })
        .collect();
    let last = bodies.len().saturating_sub(1);
    bodies
        .into_iter()
        .enumerate()
        .map(|(i, mut body)| {
            if i < last && !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            body
        })
        .collect()
}

/// Anzahl der Werke, Knoten, Revisionen, Beiträge und Verweise.
async fn counts(pool: &sqlx::SqlitePool) -> Vec<i64> {
    let mut out = Vec::new();
    for table in ["work", "node", "revision", "post", "post_ref"] {
        out.push(sqlx::query_scalar(&format!("SELECT count(*) FROM {table}")).fetch_one(pool).await.unwrap());
    }
    out
}

#[tokio::test]
async fn migration_0005_gives_unique_titles_and_keeps_everything_else() {
    let data = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(data.path().join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
    // Stand beim Nutzer vor Runde 4: Kern 0001–0004, Bibliothek 0101–0104, dann die Werke.
    partial(&baruch_store::MIGRATOR, 4).run(&pool).await.unwrap();
    partial(&baruch_library::MIGRATOR, 104).run(&pool).await.unwrap();
    let seeded = insert_seeded(&pool).await;
    let imported = insert_imported(&pool, "moevenoog", "aa").await;
    let again = insert_imported(&pool, "moevenoog-2", "bb").await;
    let before = counts(&pool).await;
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version").fetch_all(&pool).await.unwrap();
    assert_eq!(versions, [1, 2, 3, 4, 101, 102, 103, 104], "Testaufbau: Stand vor 0005");
    pool.close().await;
    // Die Umwandlung alter Teiltexte beim Start (Nutzerfrage 8) prüft `round14_conversion.rs`; hier geht es nur um
    // die Migrationen. `store_meta` gibt es erst ab 0007: Store::open migriert (0005 ff.), dann gilt die Umwandlung
    // als erledigt, danach startet der Server wie bisher.
    let store = baruch_store::Store::open(data.path()).await.expect("Migration 0005 ff.");
    sqlx::query("INSERT OR REPLACE INTO store_meta (key, value) VALUES (?, '1')")
        .bind(baruch_store::CONVERTED_KEY)
        .execute(store.pool())
        .await
        .unwrap();
    store.pool().close().await;

    let state = baruch_server::AppState::open(data.path()).await.expect("Start mit Migration 0005");
    let pool = state.store.pool();
    let works = state.store.works().await.unwrap();
    assert_eq!(
        works,
        [
            (
                seeded,
                "moevenoog-ein-ganzes-jahr-auf-der".to_owned(),
                "Beispielwerk: Leise Abhängigkeit (Auszug)".to_owned()
            ),
            (imported, "moevenoog".to_owned(), OLD_TITLE.to_owned()),
            (again, "moevenoog-2".to_owned(), format!("{OLD_TITLE} (2)")),
        ],
        "gesätes Werk heißt wieder Beispielwerk, das erste Buch behält den Titel, das zweite bekommt „(2)“; slugs bleiben"
    );
    assert_eq!(counts(pool).await, before, "gleich viele Werke, Knoten, Revisionen, Beiträge und Verweise");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&state.store.export_typst(seeded).await.unwrap()).into_owned(),
        expected_sample_export(),
        "Export des Beispielwerks byte-gleich"
    );
    let book: String = OLD_HEADINGS.iter().map(|h| h.5).collect();
    for work in [imported, again] {
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&state.store.export_typst(work).await.unwrap()).into_owned(),
            format!("{OLD_TEMPLATE}{book}"),
            "Export des Buchs {work} byte-gleich"
        );
    }
    let wrong: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM node n \
          WHERE (SELECT count(*) FROM revision r WHERE r.node_id = n.id AND r.status = 'current') <> 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(wrong, 0, "genau eine aktuelle Fassung je Abschnitt");
    let templates: Vec<(i64, i64, String, String)> =
        sqlx::query_as("SELECT work_id, number, body, note FROM work_template_revision ORDER BY work_id")
            .fetch_all(pool)
            .await
            .unwrap();
    let note = "Stand vor Migration 0005".to_owned();
    assert_eq!(
        templates,
        [
            (seeded, 1, String::new(), note.clone()),
            (imported, 1, OLD_TEMPLATE.to_owned(), note.clone()),
            (again, 1, OLD_TEMPLATE.to_owned(), note),
        ],
        "je Werk Fassung 1 der Werkvorlage aus work.template"
    );
    let missing: i64 =
        sqlx::query_scalar("SELECT count(*) FROM work WHERE title_key IS NULL").fetch_one(pool).await.unwrap();
    assert_eq!(missing, 0, "jeder Titel hat seinen Schlüssel");
    for (_, slug, _) in &works {
        assert!(
            data.path().join("works").join(slug).join("main.typ").is_file(),
            "Spiegel unter dem alten Ordnernamen {slug}"
        );
    }
    drop(state);

    let state = baruch_server::AppState::open(data.path()).await.unwrap();
    assert_eq!(state.store.works().await.unwrap(), works, "zweiter Start ändert keinen Titel und keinen slug");
    assert_eq!(counts(state.store.pool()).await, before, "zweiter Start ändert nichts an den Daten");
    let revisions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM work_template_revision").fetch_one(state.store.pool()).await.unwrap();
    assert_eq!(revisions, 3, "zweiter Start legt keine weitere Fassung der Werkvorlage an");
    let book_again = baruch_typst::xref::strip_key_labels(&state.store.export_typst(again).await.unwrap()).into_owned();
    assert_eq!(book_again, format!("{OLD_TEMPLATE}{book}"), "Export nach dem zweiten Start unverändert");
}

/// Fester Schlüssel (`Node::key`) des Abschnitts an Stelle `seq`: Erzeugte Adressen tragen ihn statt
/// der Stelle (`/?work=W&key=K`, Vertrag Runde 8).
async fn key_at(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}
