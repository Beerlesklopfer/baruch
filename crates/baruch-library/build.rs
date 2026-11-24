// SPDX-License-Identifier: AGPL-3.0-or-later
//! `sqlx::migrate!` bettet die Migrationen beim Übersetzen ein; eine neue Datei unter `migrations/`
//! muss daher einen Neubau auslösen.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
