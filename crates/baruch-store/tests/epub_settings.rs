// SPDX-License-Identifier: AGPL-3.0-or-later
//! Setting for EPUBCheck in the store (Round 20c, ownership „epub-pruefung“): path to the epubcheck.jar in
//! `store_meta` (key `epubcheck_jar`), without a migration; set, replace, remove, still there after reopening.

use baruch_store::Store;

#[tokio::test]
async fn the_epubcheck_jar_is_kept_in_store_meta_and_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(store.epubcheck_jar().await.unwrap(), None, "frischer Store: kein Pfad eingestellt");
    store.set_epubcheck_jar(Some("/opt/epubcheck/epubcheck.jar")).await.unwrap();
    assert_eq!(store.epubcheck_jar().await.unwrap().as_deref(), Some("/opt/epubcheck/epubcheck.jar"), "gesetzt");
    store.set_epubcheck_jar(Some("/srv/werkzeuge/epubcheck.jar")).await.unwrap();
    assert_eq!(
        store.epubcheck_jar().await.unwrap().as_deref(),
        Some("/srv/werkzeuge/epubcheck.jar"),
        "ein zweites Setzen ersetzt den Pfad (eine Zeile je Schlüssel)"
    );
    let value: String = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'epubcheck_jar'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(value, "/srv/werkzeuge/epubcheck.jar", "der Schlüssel heißt laut Vertrag epubcheck_jar");
    drop(store);
    let store = Store::open(dir.path()).await.unwrap();
    assert_eq!(
        store.epubcheck_jar().await.unwrap().as_deref(),
        Some("/srv/werkzeuge/epubcheck.jar"),
        "die Einstellung überlebt das Neuöffnen"
    );
    store.set_epubcheck_jar(None).await.unwrap();
    assert_eq!(store.epubcheck_jar().await.unwrap(), None, "None entfernt den Pfad");
    store.set_epubcheck_jar(None).await.unwrap();
    assert_eq!(store.epubcheck_jar().await.unwrap(), None, "zweimal entfernen schadet nicht");
}
