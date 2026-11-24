//! `sqlx::migrate!` bettet `migrations/` beim Übersetzen ein; damit eine neue Migration auch ohne
//! Änderung an `src/` ankommt, baut cargo neu, sobald sich der Ordner ändert.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
