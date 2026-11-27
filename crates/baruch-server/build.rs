//! Compiles the TypeScript widgets (`web/src/*.ts`) with `tsc` to `$OUT_DIR/js` and generates
//! `$OUT_DIR/scripts.rs`, a list `(file name, content)` that the program embeds.
//! `tsc` comes from `web/node_modules` (run `npm ci` once in the folder `web`).

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

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
