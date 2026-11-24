// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23 (Integrator, Codereview quellen-5): Ein Datenordner, den Baruch selbst anlegt, gehört nur dem Besitzer;
//! einen vorhandenen Ordner ändert Baruch nicht.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use baruch_store::Store;

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[tokio::test]
async fn a_new_data_dir_is_private_and_an_existing_one_keeps_its_rights() {
    let root = tempfile::tempdir().unwrap();

    // Neu, auch mit fehlendem Elternordner: 0700, die Datenbank liegt darin.
    let fresh = root.path().join("eltern").join("daten");
    let store = Store::open(&fresh).await.expect("neuer Datenordner lässt sich öffnen");
    assert_eq!(mode(&fresh), 0o700, "ein neu angelegter Datenordner ist nur für den Besitzer lesbar");
    assert!(fresh.join("baruch.sqlite").is_file(), "die Datenbank liegt im neuen Ordner");
    drop(store);

    // Vorhanden mit bewusst weiteren Rechten: bleibt, wie der Mensch ihn angelegt hat.
    let shared = root.path().join("geteilt");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o750)).unwrap();
    Store::open(&shared).await.expect("vorhandener Datenordner lässt sich öffnen");
    assert_eq!(mode(&shared), 0o750, "die Rechte eines vorhandenen Ordners ändert Baruch nicht");

    // Zweites Öffnen des eigenen Ordners: weiter 0700, kein Fehler „existiert schon“.
    Store::open(&fresh).await.expect("zweites Öffnen");
    assert_eq!(mode(&fresh), 0o700, "auch nach dem zweiten Öffnen unverändert");
}
