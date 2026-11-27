//! `sqlx::migrate!` embeds `migrations/` at compile time; so that a new migration arrives even without a change to
//! `src/`, cargo rebuilds as soon as the folder changes.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
