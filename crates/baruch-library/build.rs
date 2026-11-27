// SPDX-License-Identifier: AGPL-3.0-or-later
//! `sqlx::migrate!` embeds the migrations at compile time; a new file under `migrations/` must therefore trigger a
//! rebuild.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
