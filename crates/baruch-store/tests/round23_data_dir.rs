// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 23 (integrator, code review quellen-5): a data directory that Baruch creates itself belongs only to the owner;
//! Baruch does not change an existing directory.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use baruch_store::Store;

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[tokio::test]
async fn a_new_data_dir_is_private_and_an_existing_one_keeps_its_rights() {
    let root = tempfile::tempdir().unwrap();

    // New, also with a missing parent directory: 0700, the database lies in it.
    let fresh = root.path().join("eltern").join("daten");
    let store = Store::open(&fresh).await.expect("neuer Datenordner lässt sich öffnen");
    assert_eq!(mode(&fresh), 0o700, "ein neu angelegter Datenordner ist nur für den Besitzer lesbar");
    assert!(fresh.join("baruch.sqlite").is_file(), "die Datenbank liegt im neuen Ordner");
    drop(store);

    // Existing with deliberately wider permissions: stays as the human created it.
    let shared = root.path().join("geteilt");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o750)).unwrap();
    Store::open(&shared).await.expect("vorhandener Datenordner lässt sich öffnen");
    assert_eq!(mode(&shared), 0o750, "die Rechte eines vorhandenen Ordners ändert Baruch nicht");

    // Second open of the own directory: still 0700, no error „existiert schon“.
    Store::open(&fresh).await.expect("zweites Öffnen");
    assert_eq!(mode(&fresh), 0o700, "auch nach dem zweiten Öffnen unverändert");
}
