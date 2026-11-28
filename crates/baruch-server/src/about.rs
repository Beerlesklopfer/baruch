// SPDX-License-Identifier: AGPL-3.0-or-later
//! About Baruch (Round 24): version, licence and source code offer (AGPL-3.0, §13) – `GET /ueber` and the
//! licence text under `GET /ueber/lizenz`. Both pages read nothing from the database and never ask the
//! network: Baruch does not check for new versions on its own. The page only names the address of the release page
//! as an ordinary link; only the human's click opens it in the browser.
//!
//! Third-party licences: `THIRD-PARTY-LICENSES.txt` is created by the package build (`scripts/third-party-licenses.py`)
//! and sits at a fixed place in every package ([`THIRD_PARTY_PLACES`]). The page names all places and – if the file
//! lies next to the running program ([`find_third_party`]) – the path on this machine. A program built from
//! source does not have the file; the page says so. If Baruch runs from an AppImage (the runtime sets
//! `APPIMAGE`), the path lies in the mounted image and is valid only while Baruch runs; the page says that too.

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
/// Help on the website (Round 37): target of the entry „Hilfe“ in the menu Baruch ▾ and on the page „Über Baruch“.
pub const HELP_URL: &str = "https://baruch.bernau.family/hilfe/";
/// English help on the website (Round 39b): the same pages under `/en/` (`site/content/en/hilfe/`).
pub const HELP_URL_EN: &str = "https://baruch.bernau.family/en/hilfe/";

/// Address of the help in the interface language: German [`HELP_URL`], English [`HELP_URL_EN`] (Round 39b).
pub fn help_url(lang: crate::i18n::Lang) -> &'static str {
    match lang {
        crate::i18n::Lang::De => HELP_URL,
        crate::i18n::Lang::En => HELP_URL_EN,
    }
}
pub const LICENSE_TEXT: &str = include_str!("../../../LICENSE");

/// File name of the third-party licences in every package.
pub const THIRD_PARTY_FILE: &str = "THIRD-PARTY-LICENSES.txt";

/// Where the third-party licences lie per package: (package, place) – the same places as in `packaging/` and
/// `docs/Verteilung.md`.
pub const THIRD_PARTY_PLACES: &[(&str, &str)] = &[
    ("Debian/Ubuntu (.deb)", "/usr/share/doc/baruch/THIRD-PARTY-LICENSES.txt"),
    ("AppImage", "usr/share/doc/baruch/THIRD-PARTY-LICENSES.txt im Abbild (entpacken mit --appimage-extract)"),
    ("macOS (.dmg)", "Baruch.app/Contents/Resources/THIRD-PARTY-LICENSES.txt"),
    ("Windows (.msi)", "THIRD-PARTY-LICENSES.txt im Installationsordner (Vorgabe C:\\Program Files\\Baruch)"),
];

/// A place of [`THIRD_PARTY_PLACES`] in the language of the request (the list keeps the German wording; places that
/// are only a path stay as they are).
fn place_text(place: &str) -> String {
    match THIRD_PARTY_PLACES.iter().position(|(_, p)| *p == place) {
        Some(1) => crate::i18n::tc("system_msg.about.appimage_place", &[]),
        Some(3) => crate::i18n::tc("system_msg.about.windows_place", &[]),
        _ => place.to_owned(),
    }
}

/// Address of the release page of this version (`…/releases/tag/v<version>`). Baruch never fetches it itself.
pub fn release_url() -> String {
    format!("{REPOSITORY}/releases/tag/v{VERSION}")
}

/// Places relative to the program `exe` where a package puts the third-party licences: `../share/doc/baruch/` (.deb
/// under `/usr/bin`, AppImage under `usr/bin`), `../Resources/` (macOS, `Contents/MacOS`) and the folder of the
/// program itself (Windows).
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

/// The third-party licences next to the program `exe` if they lie there as a regular file (not a folder; a
/// symbolic link is not followed – only the path is shown, the file is never read).
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
    // Path of the running program; behind a link (e.g. ~/.local/bin/baruch-server) its target counts.
    let third_party = std::env::current_exe()
        .ok()
        .map(|exe| std::fs::canonicalize(&exe).unwrap_or(exe))
        .and_then(|exe| find_third_party(&exe))
        .map(|path| path.display().to_string());
    let places: Vec<Jinja> = THIRD_PARTY_PLACES
        .iter()
        .map(|(package, place)| context! { package => *package, place => place_text(place) })
        .collect();
    let ctx = context! {
        settings => Jinja::from(Serde(settings)),
        version => VERSION,
        license_id => LICENSE_ID,
        repository => REPOSITORY,
        homepage => HOMEPAGE,
        help => help_url(crate::i18n::current()),
        release_url => release_url(),
        data_dir => state.data.display().to_string(),
        default_data_dir => baruch_store::default_data_dir().display().to_string(),
        third_party => third_party,
        // The runtime of an AppImage sets APPIMAGE (path of the image file): the path above then lies in the
        // mounted image and is valid only while Baruch runs.
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
        // .deb and AppImage: <root>/usr/bin/baruch-server and <root>/usr/share/doc/baruch/…
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
        // macOS: Baruch.app/Contents/MacOS/baruch-server and Contents/Resources/…
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
        // Windows: next to baruch-server.exe in the installation folder.
        let win = root.join("Baruch");
        std::fs::create_dir_all(&win).unwrap();
        let file = win.join(THIRD_PARTY_FILE);
        std::fs::write(&file, "erfundene Liste\n").unwrap();
        assert_eq!(
            find_third_party(&win.join("baruch-server.exe")),
            Some(file.clone()),
            "unter Windows liegt die Datei neben dem Programm"
        );
        // A folder of this name is not a licence file.
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir(&file).unwrap();
        assert_eq!(find_third_party(&win.join("baruch-server.exe")), None, "ein Ordner gleichen Namens zählt nicht");
    }

    #[test]
    fn the_help_follows_the_interface_language() {
        assert_eq!(help_url(crate::i18n::Lang::De), HELP_URL, "German interface: the German help pages");
        assert_eq!(help_url(crate::i18n::Lang::En), HELP_URL_EN, "English interface: the English help pages");
        assert_eq!(
            HELP_URL_EN,
            format!("{HOMEPAGE}en/hilfe/"),
            "Hugo serves the English translation of /hilfe/ under /en/hilfe/ (same path, language prefix)"
        );
    }

    #[test]
    fn every_package_place_names_the_file() {
        for (package, place) in THIRD_PARTY_PLACES {
            assert!(place.contains(THIRD_PARTY_FILE), "{package}: der Ort nennt die Datei beim Namen: {place}");
        }
        assert_eq!(THIRD_PARTY_PLACES.len(), 4, "vier Paketformate: .deb, AppImage, .dmg, .msi");
    }
}
