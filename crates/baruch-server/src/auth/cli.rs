// SPDX-License-Identifier: AGPL-3.0-or-later
//! Notweg bei vergessenem Passwort (Runde 35, Vertrag §1.7): `baruch-server passwort <name> [--data <Ordner>]`.

use std::path::Path;

/// Setzt das Passwort des Kontos `name` im Datenordner `data` und löscht dessen Sitzungen.
pub async fn reset_password(data: &Path, name: &str, password: &str) -> Result<(), String> {
    // STUB: Passwort eines Kontos von der Befehlszeile setzen, Sitzungen löschen (Runde 35)
    let _ = (data, name, password);
    Err("passwort: noch nicht umgesetzt (Konten, Runde 35)".to_owned())
}
