//! Compiles the TypeScript widgets (`web/src/*.ts`) with `tsc` to `$OUT_DIR/js` and generates
//! `$OUT_DIR/scripts.rs`, a list `(file name, content)` that the program embeds.
//! `tsc` comes from `web/node_modules` (run `npm ci` once in the folder `web`).
//!
//! Round 39a (interface R39a-S1): also lists the catalogs of the user interface `locales/<lang>/*.ftl` (lang `en`,
//! `de`) as `$OUT_DIR/locales.rs`, `&[(lang, stem, include_str!(…)), …]` sorted by language and stem, which
//! `src/i18n.rs` embeds. A new area file therefore needs no code change.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let web = manifest.join("web");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("js");

    println!("cargo:rerun-if-changed=web/src");
    println!("cargo:rerun-if-changed=web/tsconfig.json");
    println!("cargo:rerun-if-changed=web/package-lock.json");
    println!("cargo:rerun-if-changed=locales");
    locales(&manifest);

    let tsc = web.join("node_modules/.bin").join(if cfg!(windows) { "tsc.cmd" } else { "tsc" });
    if !tsc.exists() {
        panic!("TypeScript fehlt: {} nicht gefunden. Einmalig `npm ci` in {} ausführen.", tsc.display(), web.display());
    }
    let _ = fs::remove_dir_all(&out);
    let status = Command::new(&tsc)
        .arg("-p")
        .arg(web.join("tsconfig.json"))
        .arg("--outDir")
        .arg(&out)
        .status()
        .expect("tsc ließ sich nicht starten");
    if !status.success() {
        panic!("tsc meldet Fehler (siehe oben)");
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&out)
        .expect("tsc hat keine Ausgabe erzeugt")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "js"))
        .collect();
    files.sort();
    let mut list = String::from("&[\n");
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy();
        list.push_str(&format!("    ({name:?}, include_str!({:?})),\n", path_str(file)));
    }
    list.push_str("]\n");
    fs::write(out.parent().unwrap().join("scripts.rs"), list).unwrap();
}

/// Writes `$OUT_DIR/locales.rs` (see the module head). Every catalog file is also watched on its own, so that a
/// changed text is embedded anew.
fn locales(manifest: &Path) {
    let mut list = String::from("&[\n");
    for lang in ["de", "en"] {
        let dir = manifest.join("locales").join(lang);
        println!("cargo:rerun-if-changed={}", path_str(&dir));
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("catalog folder {} missing: {e}", dir.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
            .collect();
        files.sort();
        for file in &files {
            println!("cargo:rerun-if-changed={}", path_str(file));
            let stem = file.file_stem().unwrap().to_string_lossy();
            list.push_str(&format!("    ({lang:?}, {stem:?}, include_str!({:?})),\n", path_str(file)));
        }
    }
    list.push_str("]\n");
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("locales.rs"), list).unwrap();
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
