// SPDX-License-Identifier: AGPL-3.0-or-later
//! Über Baruch (Runde 24): Version, Lizenz und Quellcode-Angebot (AGPL-3.0, §13) – `GET /ueber` und der
//! Lizenztext unter `GET /ueber/lizenz`. Beide Seiten lesen nichts aus der Datenbank und fragen nie im Netz
//! nach: Baruch prüft nicht von sich aus auf neue Versionen. Die Seite nennt nur die Adresse der Release-Seite
//! als gewöhnlichen Link; erst der Klick des Menschen öffnet sie im Browser.
//!
//! Drittlizenzen: `THIRD-PARTY-LICENSES.txt` entsteht beim Paketbau (`scripts/third-party-licenses.py`) und liegt
//! in jedem Paket an einem festen Ort ([`THIRD_PARTY_PLACES`]). Die Seite nennt alle Orte und – wenn die Datei
//! neben dem laufenden Programm liegt ([`find_third_party`]) – den Pfad auf diesem Rechner. Ein aus dem Quelltext
//! gebautes Programm hat die Datei nicht; das sagt die Seite. Läuft Baruch aus einem AppImage (die Laufzeit setzt
//! `APPIMAGE`), liegt der Pfad im eingehängten Abbild und gilt nur, solange Baruch läuft; auch das sagt die Seite.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use minijinja::value::Serde;
use minijinja::{Value as Jinja, context};

use crate::AppState;
use crate::view::Settings;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const LICENSE_ID: &str = "AGPL-3.0-or-later";
pub const REPOSITORY: &str = "https://github.com/Beerlesklopfer/baruch";
pub const HOMEPAGE: &str = "https://baruch.bernau.family/";
/// Hilfe auf der Webseite (Runde 37): Ziel des Eintrags „Hilfe“ im Menü Baruch ▾ und auf der Seite „Über Baruch“.
pub const HELP_URL: &str = "https://baruch.bernau.family/hilfe/";
pub const LICENSE_TEXT: &str = include_str!("../../../LICENSE");

/// Dateiname der Drittlizenzen in jedem Paket.
pub const THIRD_PARTY_FILE: &str = "THIRD-PARTY-LICENSES.txt";

/// Wo die Drittlizenzen je Paket liegen: (Paket, Ort) – dieselben Orte wie in `packaging/` und
/// `docs/Verteilung.md`.
pub const THIRD_PARTY_PLACES: &[(&str, &str)] = &[
    ("Debian/Ubuntu (.deb)", "/usr/share/doc/baruch/THIRD-PARTY-LICENSES.txt"),
    ("AppImage", "usr/share/doc/baruch/THIRD-PARTY-LICENSES.txt im Abbild (entpacken mit --appimage-extract)"),
    ("macOS (.dmg)", "Baruch.app/Contents/Resources/THIRD-PARTY-LICENSES.txt"),
    ("Windows (.msi)", "THIRD-PARTY-LICENSES.txt im Installationsordner (Vorgabe C:\\Program Files\\Baruch)"),
];

/// Adresse der Release-Seite dieser Version (`…/releases/tag/v<Version>`). Baruch ruft sie nie selbst ab.
pub fn release_url() -> String {
    format!("{REPOSITORY}/releases/tag/v{VERSION}")
}

/// Orte relativ zum Programm `exe`, an denen ein Paket die Drittlizenzen ablegt: `../share/doc/baruch/` (.deb
/// unter `/usr/bin`, AppImage unter `usr/bin`), `../Resources/` (macOS, `Contents/MacOS`) und der Ordner des
/// Programms selbst (Windows).
pub fn third_party_candidates(exe: &Path) -> Vec<PathBuf> {
    let Some(dir) = exe.parent() else { return Vec::new() };
    let mut out = Vec::new();
    if let Some(up) = dir.parent() {
        out.push(up.join("share").join("doc").join("baruch").join(THIRD_PARTY_FILE));
        out.push(up.join("Resources").join(THIRD_PARTY_FILE));
    }
    out.push(dir.join(THIRD_PARTY_FILE));
    out
}

/// Die Drittlizenzen neben dem Programm `exe`, wenn sie dort als gewöhnliche Datei liegen (kein Ordner; einer
/// symbolischen Verknüpfung wird nicht gefolgt – angezeigt wird nur der Pfad, gelesen wird die Datei nie).
pub fn find_third_party(exe: &Path) -> Option<PathBuf> {
    third_party_candidates(exe)
        .into_iter()
        .find(|path| std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file()))
}

/// `GET /ueber`, `GET /ueber/lizenz`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/ueber", get(page)).route("/ueber/lizenz", get(license))
}

async fn page(axum::extract::State(state): axum::extract::State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    // Pfad des laufenden Programms; hinter einer Verknüpfung (z. B. ~/.local/bin/baruch-server) zählt ihr Ziel.
    let third_party = std::env::current_exe()
        .ok()
        .map(|exe| std::fs::canonicalize(&exe).unwrap_or(exe))
        .and_then(|exe| find_third_party(&exe))
        .map(|path| path.display().to_string());
    let places: Vec<Jinja> =
        THIRD_PARTY_PLACES.iter().map(|(package, place)| context! { package => *package, place => *place }).collect();
    let ctx = context! {
        settings => Jinja::from(Serde(settings)),
        version => VERSION,
        license_id => LICENSE_ID,
        repository => REPOSITORY,
        homepage => HOMEPAGE,
        release_url => release_url(),
        data_dir => state.data.display().to_string(),
        default_data_dir => baruch_store::default_data_dir().display().to_string(),
        third_party => third_party,
        // Die Laufzeit eines AppImage setzt APPIMAGE (Pfad der Abbild-Datei): Der Pfad oben liegt dann im
        // eingehängten Abbild und gilt nur, solange Baruch läuft.
        appimage => std::env::var_os("APPIMAGE").map(|path| path.to_string_lossy().into_owned()),
        third_party_file => THIRD_PARTY_FILE,
        third_party_places => places,
    };
    match state.templates.get_template("about.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Vorlage about.html: {e}")).into_response(),
    }
}

async fn license() -> Response {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], LICENSE_TEXT).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_address_names_this_version_in_the_repository() {
        assert_eq!(
            release_url(),
            format!("https://github.com/Beerlesklopfer/baruch/releases/tag/v{}", env!("CARGO_PKG_VERSION")),
            "Release-Seite = Repo + Tag v<Version>; die Version kommt aus dem Manifest, nie aus einer zweiten Quelle"
        );
    }

    #[test]
    fn third_party_licenses_are_found_where_each_package_puts_them() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        // .deb und AppImage: <Wurzel>/usr/bin/baruch-server und <Wurzel>/usr/share/doc/baruch/…
        let deb = root.join("deb").join("usr");
        std::fs::create_dir_all(deb.join("bin")).unwrap();
        std::fs::create_dir_all(deb.join("share").join("doc").join("baruch")).unwrap();
        let exe = deb.join("bin").join("baruch-server");
        assert_eq!(find_third_party(&exe), None, "ohne Datei (Bau aus dem Quelltext) wird nichts behauptet");
        let file = deb.join("share").join("doc").join("baruch").join(THIRD_PARTY_FILE);
        std::fs::write(&file, "erfundene Liste\n").unwrap();
        assert_eq!(
            find_third_party(&exe),
            Some(file),
            "im .deb und im AppImage liegt die Datei unter share/doc/baruch"
        );
        // macOS: Baruch.app/Contents/MacOS/baruch-server und Contents/Resources/…
        let app = root.join("Baruch.app").join("Contents");
        std::fs::create_dir_all(app.join("MacOS")).unwrap();
        std::fs::create_dir_all(app.join("Resources")).unwrap();
        let file = app.join("Resources").join(THIRD_PARTY_FILE);
        std::fs::write(&file, "erfundene Liste\n").unwrap();
        assert_eq!(
            find_third_party(&app.join("MacOS").join("baruch-server")),
            Some(file),
            "im macOS-Programm liegt die Datei unter Contents/Resources"
        );
        // Windows: neben baruch-server.exe im Installationsordner.
        let win = root.join("Baruch");
        std::fs::create_dir_all(&win).unwrap();
        let file = win.join(THIRD_PARTY_FILE);
        std::fs::write(&file, "erfundene Liste\n").unwrap();
        assert_eq!(
            find_third_party(&win.join("baruch-server.exe")),
            Some(file.clone()),
            "unter Windows liegt die Datei neben dem Programm"
        );
        // Ein Ordner dieses Namens ist keine Lizenzdatei.
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert_eq!(find_third_party(&win.join("baruch-server.exe")), None, "ein Ordner gleichen Namens zählt nicht");
    }

    #[test]
    fn every_package_place_names_the_file() {
        for (package, place) in THIRD_PARTY_PLACES {
            assert!(place.contains(THIRD_PARTY_FILE), "{package}: der Ort nennt die Datei beim Namen: {place}");
        }
        assert_eq!(THIRD_PARTY_PLACES.len(), 4, "vier Paketformate: .deb, AppImage, .dmg, .msi");
    }
}
