// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bilder und Grafiken als Text (Migration 0009, Vertrag Runde 6, Abschnitt 4) auf einer echten
//! SQLite-Datei im Temp-Ordner: Fassungen wie bei Abschnitten (genau eine aktuelle), Trigger,
//! Grenzen, Spiegel und Export. Alle Bilder sind im Test erzeugt (kein Binärfile im Repo).

use baruch_store::{AssetChangeKind, AssetEncoding, ImportAsset, MAX_ASSET_BYTES, NewAsset, Store, StoreError, WorkId};

const HOUSE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- erfundenes Haus -->\n\
                     <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect width=\"10\" height=\"8\"/></svg>\n";
const HOUSE_RED: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect fill=\"red\" width=\"10\" height=\"8\"/></svg>\n";

/// CRC-32 (PNG), bitweise.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Gültiges PNG mit einem Pixel in der Farbe `gray` (zlib-Block ohne Kompression).
fn tiny_png(gray: u8) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    let raw = [0u8, gray]; // Filter 0, ein Grauwert
    let adler = {
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    };
    let mut zlib = vec![0x78, 0x01, 0x01, 0x02, 0x00, 0xFD, 0xFF];
    zlib.extend_from_slice(&raw);
    zlib.extend_from_slice(&adler.to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

async fn fresh() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.default_work().await.unwrap();
    (dir, store, work)
}

fn svg<'a>(file_name: &'a str, text: &'a str) -> NewAsset<'a> {
    NewAsset {
        file_name,
        name: None,
        bytes: text.as_bytes(),
        as_base64: false,
        path: Some("/bilder/haus.svg"),
        caption: "Das Haus am Hang",
        alt: "Ein einfaches Haus",
        note: "erste Skizze",
    }
}

async fn count(store: &Store, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(store.pool()).await.unwrap()
}

async fn currents(store: &Store, work: WorkId, name: &str) -> Vec<u32> {
    store.asset_revisions(work, name).await.unwrap().iter().filter(|r| r.current).map(|r| r.number).collect()
}

#[tokio::test]
async fn adding_an_svg_stores_it_as_text_with_exactly_one_current_version() {
    let (_dir, store, work) = fresh().await;
    let change = store.add_asset(work, &svg("Haus.SVG", HOUSE), "Autor").await.unwrap();
    assert_eq!((change.name.as_str(), change.rev, change.kind), ("haus.svg", 1, AssetChangeKind::Added), "{change:?}");
    assert_eq!(currents(&store, work, "haus.svg").await, [1], "genau eine aktuelle Fassung nach dem Hinzufügen");

    let list = store.asset_list(work).await.unwrap();
    assert_eq!(list.len(), 1, "ein Bild im Werk: {list:?}");
    let summary = &list[0];
    assert_eq!(summary.mime, "image/svg+xml", "SVG am Inhalt erkannt");
    assert_eq!(summary.encoding, AssetEncoding::Svg, "SVG als Text gespeichert");
    assert_eq!(summary.bytes, HOUSE.len() as u64, "Größe der Datei");
    assert_eq!(summary.paths, ["/bilder/haus.svg"], "Pfad wie im Text");
    assert_eq!(
        (summary.caption.as_str(), summary.alt.as_str()),
        ("Das Haus am Hang", "Ein einfaches Haus"),
        "Bildunterschrift und Alternativtext wie angelegt"
    );

    let data = store.asset_data(work, "haus.svg", None).await.unwrap();
    assert_eq!(data.content, HOUSE, "SVG-Text unverändert in der Datenbank");
    assert_eq!(data.bytes().unwrap(), HOUSE.as_bytes(), "Bytes = SVG-Text");
    let by_path = store.asset_by_path(work, "/bilder/haus.svg").await.unwrap().expect("Bild zum Pfad");
    assert_eq!(by_path.name, "haus.svg", "Pfad findet das Bild");
    assert_eq!(
        store.asset_by_path(work, "bilder/haus.svg").await.unwrap().map(|d| d.name),
        Some("haus.svg".to_owned()),
        "auch ohne führendes /"
    );
    assert!(store.asset_by_path(work, "/bilder/fehlt.svg").await.unwrap().is_none(), "unbekannter Pfad → None");
}

#[tokio::test]
async fn a_png_is_stored_as_base64_text_and_the_content_is_always_text() {
    let (_dir, store, work) = fresh().await;
    let png = tiny_png(200);
    let new = NewAsset {
        file_name: "punkt.png",
        name: None,
        bytes: &png,
        as_base64: false,
        path: None,
        caption: "",
        alt: "",
        note: "",
    };
    store.add_asset(work, &new, "Autor").await.unwrap();
    let svg_b64 = NewAsset { as_base64: true, path: Some("/bilder/haus-b64.svg"), ..svg("haus.svg", HOUSE) };
    let change = store.add_asset(work, &svg_b64, "Autor").await.unwrap();
    assert_eq!(change.name, "haus.svg", "freier Name");

    let data = store.asset_data(work, "punkt.png", None).await.unwrap();
    assert_eq!((data.mime.as_str(), data.encoding), ("image/png", AssetEncoding::Base64), "PNG als Base64");
    assert!(
        !data.content.contains('\n') && data.content.ends_with('='),
        "Standard-Base64 ohne Zeilenumbruch: {}",
        data.content
    );
    assert_eq!(data.bytes().unwrap(), png, "entschlüsselt byte-gleich");
    let svg_data = store.asset_data(work, "haus.svg", None).await.unwrap();
    assert_eq!(svg_data.encoding, AssetEncoding::Base64, "SVG auf Wunsch als Base64");
    assert_eq!(svg_data.bytes().unwrap(), HOUSE.as_bytes(), "und byte-gleich zurück");

    assert_eq!(
        count(&store, "SELECT count(*) FROM asset_revision WHERE typeof(content) <> 'text'").await,
        0,
        "Inhalt ist für jede Fassung TEXT, nie BLOB"
    );
    let insert_blob = sqlx::query(
        "INSERT INTO asset_revision (asset_id, number, mime, encoding, content, bytes, sha256, author_name, status) \
         SELECT id, 99, 'image/png', 'base64', x'00', 1, '', 'Autor', 'superseded' FROM asset LIMIT 1",
    )
    .execute(store.pool())
    .await;
    assert!(insert_blob.is_err(), "ein BLOB als Inhalt scheitert am CHECK typeof(content) = 'text'");
}

#[tokio::test]
async fn replacing_switching_updating_and_deleting_keep_exactly_one_current() {
    let (_dir, store, work) = fresh().await;
    store.add_asset(work, &svg("haus.svg", HOUSE), "Autor").await.unwrap();

    let replaced = store
        .replace_asset(work, "haus.svg", "haus-rot.svg", HOUSE_RED.as_bytes(), false, "rot gestrichen", "Autor")
        .await
        .unwrap();
    assert_eq!((replaced.rev, replaced.kind), (2, AssetChangeKind::Replaced), "Ersetzen = neue Fassung 2");
    assert_eq!(replaced.before.as_ref().map(|b| b.number), Some(1), "vorher aktuell war 1");
    assert_eq!(currents(&store, work, "haus.svg").await, [2], "nach dem Ersetzen genau eine aktuelle (die neue)");
    let revisions = store.asset_revisions(work, "haus.svg").await.unwrap();
    assert_eq!(revisions[1].based_on, Some(1), "die neue Fassung beruht auf der bisher aktuellen");
    assert_eq!(revisions[1].caption, "Das Haus am Hang", "Bildunterschrift geht mit");
    assert_eq!(
        store.asset_data(work, "haus.svg", None).await.unwrap().content,
        HOUSE_RED,
        "aktuell ist der neue Inhalt"
    );
    assert_eq!(store.asset_data(work, "haus.svg", Some(1)).await.unwrap().content, HOUSE, "Fassung 1 bleibt erhalten");

    let unchanged =
        store.replace_asset(work, "haus.svg", "nochmal.svg", HOUSE_RED.as_bytes(), false, "", "Autor").await;
    assert!(
        matches!(&unchanged, Err(StoreError::Refused(m)) if *m == "Das Bild ist unverändert."),
        "gleiche Bytes ersetzt → abgelehnt: {unchanged:?}"
    );

    let switched = store.set_current_asset(work, "haus.svg", 1).await.unwrap();
    assert_eq!((switched.rev, switched.kind), (1, AssetChangeKind::Switched), "auf Fassung 1 umgeschaltet");
    assert_eq!(currents(&store, work, "haus.svg").await, [1], "nach dem Umschalten genau eine aktuelle");
    assert!(
        matches!(store.set_current_asset(work, "haus.svg", 1).await, Err(StoreError::Refused(_))),
        "schon aktuell → abgelehnt"
    );

    // Fassung 3 auf 2 aufbauen: erst 2 aktuell, dann ersetzen.
    store.set_current_asset(work, "haus.svg", 2).await.unwrap();
    let blue = HOUSE_RED.replace("red", "blue");
    let third = store.replace_asset(work, "haus.svg", "haus3.svg", blue.as_bytes(), false, "", "Autor").await.unwrap();
    assert_eq!(third.rev, 3, "dritte Fassung");

    let updated = store
        .update_asset_revision(work, "haus.svg", 2, Some("Rotes Haus"), Some("Haus mit rotem Dach"), None, "Autor")
        .await
        .unwrap();
    assert_eq!((updated.rev, updated.kind), (2, AssetChangeKind::Updated), "Fassung 2 geändert");
    assert!(
        updated.detail.contains("Bildunterschrift vorher: Das Haus am Hang"),
        "vorher im Detail: {}",
        updated.detail
    );
    let after = store.asset_revisions(work, "haus.svg").await.unwrap();
    let two = after.iter().find(|r| r.number == 2).unwrap();
    assert_eq!(
        (two.caption.as_str(), two.alt.as_str(), two.note.as_str()),
        ("Rotes Haus", "Haus mit rotem Dach", "rot gestrichen"),
        "Fassung 2 trägt die geänderten Felder"
    );
    assert!(two.edited.is_some(), "edited ist gesetzt");
    assert!(
        after.iter().filter(|r| r.number != 2).all(|r| r.edited.is_none()),
        "nur die geänderte Fassung trägt edited"
    );
    assert_eq!(currents(&store, work, "haus.svg").await, [3], "Ändern lässt die aktuelle, wie sie ist");
    assert!(
        matches!(
            store.update_asset_revision(work, "haus.svg", 2, Some("Rotes Haus"), None, None, "Autor").await,
            Err(StoreError::Refused(_))
        ),
        "nichts geändert → abgelehnt"
    );

    let refused = store.delete_asset_revision(work, "haus.svg", 3, "Autor").await;
    assert!(
        matches!(refused, Err(StoreError::Refused(_))),
        "die aktuelle Fassung lässt sich nicht löschen: {refused:?}"
    );
    let deleted = store.delete_asset_revision(work, "haus.svg", 2, "Autor").await.unwrap();
    assert_eq!((deleted.rev, deleted.kind), (2, AssetChangeKind::Deleted), "Fassung 2 gelöscht");
    assert_eq!(
        deleted.removed.as_ref().map(|d| d.content.as_str()),
        Some(HOUSE_RED),
        "gelöschter Inhalt für den Spiegel"
    );
    assert!(deleted.detail.contains("Umgehängt: 3 → Fassung 1"), "Kinder hängen am Elternteil: {}", deleted.detail);
    let left = store.asset_revisions(work, "haus.svg").await.unwrap();
    assert_eq!(left.iter().map(|r| r.number).collect::<Vec<_>>(), [1, 3], "Lücke bei 2");
    assert_eq!(left[1].based_on, Some(1), "Fassung 3 hängt jetzt an Fassung 1");
    assert_eq!(currents(&store, work, "haus.svg").await, [3], "nach dem Löschen einer Fassung genau eine aktuelle");
    let bigger = HOUSE.replace("10", "12");
    let fourth =
        store.replace_asset(work, "haus.svg", "haus4.svg", bigger.as_bytes(), false, "", "Autor").await.unwrap();
    assert_eq!(fourth.rev, 4, "Nummern werden nicht wiederverwendet (nicht 2)");

    let removed = store.delete_asset(work, "haus.svg", "Autor").await.unwrap();
    assert_eq!((removed.kind, removed.rev), (AssetChangeKind::Removed, 4), "ganzes Bild gelöscht, zuletzt aktuell 4");
    assert!(removed.removed.is_some(), "Inhalt der aktuellen Fassung für den Spiegel");
    assert!(store.asset_list(work).await.unwrap().is_empty(), "kein Bild mehr");
    for table in ["asset", "asset_revision", "asset_path"] {
        assert_eq!(count(&store, &format!("SELECT count(*) FROM {table}")).await, 0, "{table} ist leer");
    }
}

#[tokio::test]
async fn the_content_of_a_version_cannot_be_changed_in_the_database() {
    let (_dir, store, work) = fresh().await;
    store.add_asset(work, &svg("haus.svg", HOUSE), "Autor").await.unwrap();
    for (column, value) in
        [("content", "'<svg/>'"), ("mime", "'image/png'"), ("bytes", "1"), ("sha256", "'00'"), ("encoding", "'base64'")]
    {
        let err = sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE asset_revision SET {column} = {value}")))
            .execute(store.pool())
            .await
            .expect_err("Inhalt unveränderlich");
        assert!(err.to_string().contains("unveränderlich"), "{column}: Trigger meldet: {err}");
    }
    for (column, value) in [("number", "7"), ("author_name", "'@lektor'"), ("created", "'gestern'")] {
        let err = sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE asset_revision SET {column} = {value}")))
            .execute(store.pool())
            .await;
        assert!(err.is_err(), "Herkunft ({column}) unveränderlich");
    }
    let err = sqlx::query("DELETE FROM asset_revision").execute(store.pool()).await;
    assert!(err.unwrap_err().to_string().contains("nicht löschen"), "die aktuelle Fassung lässt sich nicht löschen");
    let err = sqlx::query("UPDATE asset_revision SET status = 'proposed'").execute(store.pool()).await;
    assert!(err.is_err(), "kein Status außer current/superseded");
    assert_eq!(store.asset_data(work, "haus.svg", None).await.unwrap().content, HOUSE, "alles beim Alten");
}

#[tokio::test]
async fn limits_types_base64_and_names_are_checked() {
    let (_dir, store, work) = fresh().await;
    let mut big = tiny_png(1);
    big.resize(MAX_ASSET_BYTES + 1, 0);
    let new = NewAsset { file_name: "gross.png", bytes: &big, path: None, ..svg("x.svg", HOUSE) };
    let err = store.add_asset(work, &new, "Autor").await.unwrap_err();
    assert!(matches!(&err, StoreError::Asset(m) if m.contains("8 MiB")), "über 8 MiB → Asset: {err}");
    let mut exact = tiny_png(1);
    exact.resize(MAX_ASSET_BYTES, 0);
    let new = NewAsset { file_name: "genau.png", bytes: &exact, path: None, ..svg("x.svg", HOUSE) };
    assert!(store.add_asset(work, &new, "Autor").await.is_ok(), "genau 8 MiB sind erlaubt");

    let new = NewAsset { file_name: "text.txt", bytes: b"Kein Bild.", path: None, ..svg("x.svg", HOUSE) };
    let err = store.add_asset(work, &new, "Autor").await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "„text.txt“ ist kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG).",
        "falscher Typ → Asset mit deutscher Meldung"
    );
    assert!(matches!(err, StoreError::Asset(_)), "Asset-Fehler");

    let err = baruch_store::decode_base64("iVBORw0KGgo@").unwrap_err();
    assert!(matches!(err, StoreError::Asset(_)), "kaputtes Base64 → Asset: {err}");

    for bad in ["Haus.svg", "haus.png", "haus", "-haus.svg"] {
        let new = NewAsset { name: Some(bad), path: None, ..svg("haus.svg", HOUSE) };
        let err = store.add_asset(work, &new, "Autor").await.unwrap_err();
        assert!(matches!(err, StoreError::Asset(_)), "ungültiger Name {bad} → Asset: {err}");
    }
    let new = NewAsset { name: Some("haus.svg"), ..svg("haus.svg", HOUSE) };
    store.add_asset(work, &new, "Autor").await.unwrap();
    let err = store.add_asset(work, &NewAsset { path: None, ..new.clone() }, "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::Refused(_)), "doppelter eigener Name → Refused: {err}");
    let derived = store.add_asset(work, &NewAsset { name: None, path: None, ..new.clone() }, "Autor").await.unwrap();
    assert_eq!(derived.name, "haus-2.svg", "Name aus dem Dateinamen weicht auf -2 aus");
    let err = store.add_asset(work, &NewAsset { name: None, ..new.clone() }, "Autor").await.unwrap_err();
    assert!(matches!(&err, StoreError::Asset(m) if m.contains("gehört schon")), "Pfad eines anderen Bilds: {err}");
    assert_eq!(store.asset_list(work).await.unwrap().len(), 3, "abgelehnte Bilder legen nichts an");

    let err = store.replace_asset(work, "haus.svg", "punkt.png", &tiny_png(3), false, "", "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::Asset(_)), "PNG passt nicht zur Endung .svg: {err}");

    let unknown = store.asset_data(work, "fehlt.svg", None).await.unwrap_err();
    assert!(
        matches!(&unknown, StoreError::UnknownAsset { name, .. } if name == "fehlt.svg"),
        "unbekanntes Bild → UnknownAsset: {unknown}"
    );
    assert!(matches!(store.asset_list(999).await, Err(StoreError::UnknownWork(999))), "unbekanntes Werk");
    let err = store.set_current_asset(work, "haus.svg", 9).await.unwrap_err();
    assert!(matches!(&err, StoreError::Asset(m) if m.contains("vorhanden: 1")), "unbekannte Fassung: {err}");
}

#[tokio::test]
async fn paths_can_be_replaced_and_export_writes_the_current_bytes_per_path() {
    let (_dir, store, work) = fresh().await;
    store.add_asset(work, &svg("haus.svg", HOUSE), "Autor").await.unwrap();
    let png = tiny_png(9);
    let new = NewAsset { file_name: "punkt.png", bytes: &png, path: Some("bilder/punkt.png"), ..svg("x.svg", HOUSE) };
    store.add_asset(work, &new, "Autor").await.unwrap();
    let change =
        store.set_asset_paths(work, "haus.svg", &["/bilder/haus.svg\n/titel/haus.svg".to_owned()]).await.unwrap();
    assert_eq!(change.kind, AssetChangeKind::Updated, "Pfade ändern ist eine Änderung");
    assert!(change.detail.contains("Pfade nachher: /bilder/haus.svg, /titel/haus.svg"), "{}", change.detail);
    assert!(
        matches!(
            store.set_asset_paths(work, "haus.svg", &["/titel/haus.svg".into(), "/bilder/haus.svg".into()]).await,
            Err(StoreError::Refused(_))
        ),
        "gleiche Pfade → unverändert"
    );
    let files = store.export_assets(work).await.unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        ["bilder/haus.svg", "bilder/punkt.png", "titel/haus.svg"],
        "je Pfad eine Datei, ohne führendes /"
    );
    assert_eq!(files[0].bytes, HOUSE.as_bytes(), "SVG byte-gleich");
    assert_eq!(files[1].bytes, png, "PNG byte-gleich");
}

#[tokio::test]
async fn the_mirror_has_svg_text_and_base64_in_lines_of_76() {
    let (_dir, store, work) = fresh().await;
    store.add_asset(work, &svg("haus.svg", HOUSE), "Autor").await.unwrap();
    let mut png = tiny_png(4);
    png.extend(std::iter::repeat_n(7u8, 300)); // länger als eine Zeile
    let new = NewAsset { file_name: "punkt.png", bytes: &png, path: None, ..svg("x.svg", HOUSE) };
    store.add_asset(work, &new, "Autor").await.unwrap();
    let files = store.mirror_snapshot(work).await.unwrap();
    let svg_file = files.iter().find(|f| f.path == "bilder/haus.svg").expect("bilder/haus.svg im Spiegel");
    assert_eq!(svg_file.contents, HOUSE, "SVG als Text");
    let b64 = files.iter().find(|f| f.path == "bilder/punkt.png.b64").expect("bilder/punkt.png.b64 im Spiegel");
    let lines: Vec<&str> = b64.contents.split_inclusive('\n').collect();
    assert!(lines.len() > 1, "mehrere Zeilen");
    assert!(lines.iter().all(|l| l.ends_with('\n')), "jede Zeile mit \\n");
    assert!(lines[..lines.len() - 1].iter().all(|l| l.len() == 77), "76 Zeichen je Zeile");
    assert_eq!(baruch_store::decode_base64(&b64.contents.replace('\n', "")).unwrap(), png, "zusammengefügt die Bytes");
    assert!(
        files[0].path == "main.typ",
        "main.typ bleibt vorn: {:?}",
        files.iter().map(|f| &f.path).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn deleting_a_work_removes_all_its_image_rows() {
    let (_dir, store, work) = fresh().await;
    let source = "= Eins\n\n#image(\"/bilder/haus.svg\")\n";
    let image = ImportAsset { path: "/bilder/haus.svg".into(), file_name: "haus.svg".into(), bytes: HOUSE.into() };
    let (imported, names) = store
        .import_typst_with_assets(source, "eins.typ", "Autor", "Import aus eins.typ", None, &[image])
        .await
        .unwrap();
    assert_eq!(names, ["haus.svg"], "ein Bild angelegt");
    store.add_asset(work, &svg("haus.svg", HOUSE), "Autor").await.unwrap();
    store.replace_asset(imported.work, "haus.svg", "rot.svg", HOUSE_RED.as_bytes(), false, "", "Autor").await.unwrap();

    store.delete_work(imported.work, &imported.title).await.unwrap();
    for table in ["asset", "asset_path"] {
        let left: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table} WHERE work_id = ?")))
                .bind(imported.work)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(left, 0, "{table}: keine Zeile des gelöschten Werks");
    }
    assert_eq!(
        count(&store, "SELECT count(*) FROM asset_revision").await,
        1,
        "nur die Fassung des anderen Werks bleibt"
    );
    assert_eq!(store.asset_list(work).await.unwrap().len(), 1, "das Bild des anderen Werks bleibt");
}

#[tokio::test]
async fn image_paths_with_dot_parts_are_found_and_exported_in_normal_form() {
    // Runde 7: `./bilder/a.png` wurde vorher beim Speichern abgelehnt und beim Import übersprungen.
    let (_dir, store, work) = fresh().await;
    assert_eq!(
        baruch_store::normalize_asset_path("./bilder/laube.svg").as_deref(),
        Some("bilder/laube.svg"),
        "Normalform ohne ./"
    );
    assert!(baruch_store::normalize_asset_path("../laube.svg").is_none(), "hinaus geht es nicht");
    assert_eq!(
        baruch_store::check_asset_path(" ./bilder/laube.svg ").unwrap(),
        "./bilder/laube.svg",
        "erlaubt, gespeichert wie im Text (getrimmt)"
    );
    for bad in ["../laube.svg", "bilder/../laube.svg"] {
        assert!(
            matches!(baruch_store::check_asset_path(bad), Err(StoreError::Asset(_))),
            "{bad} bleibt abgelehnt (aus dem Ordner hinaus)"
        );
    }
    let new = NewAsset { path: Some("./bilder/laube.svg"), ..svg("laube.svg", HOUSE) };
    store.add_asset(work, &new, "Anna").await.expect("./ im Pfad ist erlaubt");
    let summary = store.asset_list(work).await.unwrap().into_iter().find(|a| a.name == "laube.svg").unwrap();
    assert_eq!(summary.paths, ["./bilder/laube.svg"], "asset_path hält den Pfad wie im Text");
    for written in ["./bilder/laube.svg", "bilder/laube.svg", "/bilder/laube.svg", "bilder//./laube.svg"] {
        let found = store.asset_by_path(work, written).await.unwrap().map(|d| d.name);
        assert_eq!(found.as_deref(), Some("laube.svg"), "{written} findet dasselbe Bild (Normalform)");
    }
    assert!(store.asset_by_path(work, "../bilder/laube.svg").await.unwrap().is_none(), "mit .. kein Treffer");
    let other = NewAsset { path: Some("bilder/laube.svg"), ..svg("zweite.svg", HOUSE_RED) };
    let taken = store.add_asset(work, &other, "Anna").await.unwrap_err();
    assert!(
        matches!(&taken, StoreError::Asset(m) if m.contains("gehört schon zum Bild laube.svg")),
        "derselbe Pfad in anderer Schreibweise gehört schon einem Bild: {taken}"
    );
    let files = store.export_assets(work).await.unwrap();
    let laube = files.iter().find(|f| f.bytes == HOUSE.as_bytes()).expect("Datei im Export");
    assert_eq!(laube.path, "bilder/laube.svg", "der Export schreibt den Pfad in Normalform");
}

#[tokio::test]
async fn searching_images_matches_name_caption_and_alt_without_case_and_takes_percent_literally() {
    let (_dir, store, work) = fresh().await;
    let add = |file_name: &'static str, path: &'static str, caption: &'static str, alt: &'static str| NewAsset {
        file_name,
        name: None,
        bytes: HOUSE.as_bytes(),
        as_base64: false,
        path: Some(path),
        caption,
        alt,
        note: "",
    };
    for new in [
        add("garten.svg", "/bilder/garten.svg", "Der Garten im Mai", "Beete mit Kohl"),
        add("hof.svg", "/bilder/hof.svg", "Rabatt 50% im Hofladen", "Ein Tor"),
        add("ufer_nord.svg", "/bilder/ufer.svg", "", "Schilf am ÄUSSEREN Ufer"),
    ] {
        store.add_asset(work, &new, "Mensch").await.unwrap();
    }
    let names = |found: Vec<baruch_store::AssetSummary>| found.into_iter().map(|a| a.name).collect::<Vec<_>>();
    let none: Vec<String> = Vec::new();
    assert_eq!(names(store.asset_search(work, "GARTEN").await.unwrap()), ["garten.svg"], "Name ohne Groß/Klein");
    assert_eq!(names(store.asset_search(work, "im mai").await.unwrap()), ["garten.svg"], "Bildunterschrift gefunden");
    assert_eq!(names(store.asset_search(work, "kohl").await.unwrap()), ["garten.svg"], "Alternativtext gefunden");
    assert_eq!(
        names(store.asset_search(work, "äusseren").await.unwrap()),
        ["ufer_nord.svg"],
        "Ä und ä gelten gleich (SQLite-LIKE allein faltet nur ASCII)"
    );
    assert_eq!(names(store.asset_search(work, "50%").await.unwrap()), ["hof.svg"], "% ist ein gewöhnliches Zeichen");
    assert_eq!(names(store.asset_search(work, "%").await.unwrap()), ["hof.svg"], "„%“ allein ist kein Platzhalter");
    assert_eq!(names(store.asset_search(work, "t_r").await.unwrap()), none, "_ ist kein Platzhalter (sonst „Tor“)");
    assert_eq!(names(store.asset_search(work, "r_n").await.unwrap()), ["ufer_nord.svg"], "_ wörtlich im Namen");
    assert_eq!(store.asset_search(work, "  ").await.unwrap().len(), 3, "leere Suche zeigt alle Bilder");
    assert!(
        matches!(store.asset_search(9999, "x").await, Err(StoreError::UnknownWork(_))),
        "ein unbekanntes Werk wird gemeldet, nicht als leere Liste"
    );
}

/// Runde 18c (Backlog Runde 11, „Bild einfügen“): Pfad und Fassung in einer Transaktion. Scheitert das Speichern
/// der Fassung (hier: eine zweite Überschrift derselben Ebene), bleibt auch kein Pfad zurück; gelingt es, hat das
/// Bild danach den Pfad, und die Fassung ist aktuell.
#[tokio::test]
async fn an_image_path_is_only_given_together_with_the_saved_revision() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let source = "= Haus\n\nEs steht am Hang.\n";
    let work = store.import_typst(source, "haus.typ", "Autor").await.unwrap().work;
    store.add_asset(work, &NewAsset { path: None, ..svg("haus.svg", HOUSE) }, "Autor").await.unwrap();
    let paths = |store: &Store| {
        let store = store.clone();
        async move { store.asset_list(work).await.unwrap()[0].paths.clone() }
    };
    assert!(paths(&store).await.is_empty(), "das Bild hat noch keinen Pfad");

    let broken = "= Haus\n\n#figure(image(\"bilder/haus.svg\"))\n\n= Noch ein Haus\n";
    let refused = store.save_human_with_asset_path(work, 0, broken, "Autor", 1, "haus.svg", "bilder/haus.svg").await;
    assert!(refused.is_err(), "zwei Überschriften derselben Ebene: Speichern scheitert: {refused:?}");
    assert!(paths(&store).await.is_empty(), "kein Pfad zurückgeblieben (eine Transaktion)");
    assert_eq!(store.load_work(work).await.unwrap().nodes[0].revisions.len(), 1, "keine neue Fassung");

    let body = "= Haus\n\nEs steht am Hang.\n\n#figure(image(\"bilder/haus.svg\"))\n";
    let (change, added) =
        store.save_human_with_asset_path(work, 0, body, "Autor", 1, "haus.svg", "bilder/haus.svg").await.unwrap();
    assert!(change.is_some() && added, "gespeichert und Pfad neu vergeben");
    assert_eq!(paths(&store).await, ["bilder/haus.svg"], "der Pfad steht beim Bild");
    let node = &store.load_work(work).await.unwrap().nodes[0];
    let current = node.current_index().map(|i| &node.revisions[i]).unwrap();
    assert_eq!(current.body, body, "die neue Fassung ist aktuell");

    let again = format!("{body}\nNoch ein Satz.\n");
    let (_, added) =
        store.save_human_with_asset_path(work, 0, &again, "Autor", 2, "haus.svg", "/bilder/haus.svg").await.unwrap();
    assert!(!added, "derselbe Pfad in anderer Schreibweise (führendes /) wird nicht doppelt vergeben");
    let (change, added) =
        store.save_human_with_asset_path(work, 0, &again, "Autor", 3, "haus.svg", "bilder/neu.svg").await.unwrap();
    assert!(change.is_none() && !added, "unveränderter Text: nichts gespeichert und auch kein neuer Pfad");
    assert_eq!(paths(&store).await, ["bilder/haus.svg"], "der zweite Pfad ist nicht dazugekommen");
}
