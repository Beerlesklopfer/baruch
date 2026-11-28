// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 24 „Verteilung“ (ownership integrator): acceptance across the boundaries of the three tasks.
//!
//! About page (pakete) under an allowed host name (webseite) and the one version number in manifest, program, website
//! and change list (pakete, ci, webseite). Names invented (`*.example.org`); no network, no Eliezer, no MultiGPT. Every
//! assertion with a reason.

use crate::round24_betrieb::{mcp_ping, open_allowing};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::about;

const NAME: &str = "baruch.example.org";

fn get(uri: &str, host: &str) -> Request<Body> {
    Request::get(uri).header(header::HOST, host).body(Body::empty()).unwrap()
}

/// File of the repo, seen from the manifest of the crate.
fn repo_file(path: &str) -> String {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full} lesen: {e}"))
}

/// All pages of the help (`site/content/de/hilfe/*.md`) as paths from the repo – read in the directory, not listed
/// individually, so that a new help page falls under the bans by itself (Round 37: the manual for users has been there
/// since, `docs/Bedienung.md` is only a pointer and technical note any more).
fn help_pages() -> Vec<String> {
    let dir = format!("{}/../../site/content/de/hilfe", env!("CARGO_MANIFEST_DIR"));
    let mut pages: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir} lesen: {e}"))
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".md"))
        .map(|name| format!("site/content/de/hilfe/{name}"))
        .collect();
    pages.sort();
    assert!(
        pages.len() >= 15,
        "die Hilfe hat den Einstieg und ihre 14 Seiten – ein leeres Verzeichnis machte die Verbote wirkungslos: \
         {pages:?}"
    );
    pages
}

/// Behind a proxy a human reaches the about page and the licence text under the allowed name; a foreign name gets to
/// see nothing of it, and `/mcp` demands the token under the name.
#[tokio::test]
async fn behind_a_proxy_the_about_page_answers_under_the_allowed_name_only() {
    let server = open_allowing(NAME).await;

    let (status, _, page) = server.send(get("/ueber", NAME)).await;
    assert_eq!(status, StatusCode::OK, "unter dem erlaubten Namen lädt die Über-Seite: {page}");
    assert!(
        page.contains(&format!("data-about-version>{}<", about::VERSION)),
        "die Seite nennt die Version des Programms, auch über den Proxy: {page}"
    );
    assert!(
        page.contains("fragt nie von sich aus im Netz nach neuen Versionen"),
        "die Zusage zur Aktualisierung steht wörtlich da (Vertrag §1): {page}"
    );
    assert!(
        page.contains(">Über Baruch</a>"),
        "der Menüeintrag ist ein gewöhnlicher Link und damit ohne JavaScript erreichbar: {page}"
    );

    let (status, headers, text) = server.send(get("/ueber/lizenz", "baruch.example.org:8443")).await;
    assert_eq!(status, StatusCode::OK, "der Lizenztext lädt auch mit Port im Host");
    assert_eq!(
        headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
        Some("text/plain; charset=utf-8"),
        "reiner Text, kein HTML (Vertrag §1)"
    );
    assert_eq!(text, about::LICENSE_TEXT, "genau der Text der Datei LICENSE des Repos");
    assert!(text.contains("GNU AFFERO GENERAL PUBLIC LICENSE"), "es ist die AGPL");

    for uri in ["/ueber", "/ueber/lizenz"] {
        let (status, _, body) = server.send(get(uri, "fremd.example.org")).await;
        assert_eq!(status, StatusCode::MISDIRECTED_REQUEST, "{uri}: ein nicht erlaubter Name bleibt 421");
        assert!(
            !body.contains(about::VERSION) && !body.contains("GNU AFFERO"),
            "{uri}: die Ablehnung verrät weder Version noch Inhalt: {body}"
        );
    }

    let (status, _) = mcp_ping(&server, Some(NAME), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "lesende Seiten gehen unter dem Namen ohne Anmeldung (darum Zugangsschutz im Proxy), /mcp aber nur mit Token"
    );
}

/// The version has one source (`[workspace.package] version`); all places that must repeat it agree with it. If the
/// user changes the version, this test shows the places he must update.
#[test]
fn the_version_is_the_same_in_manifest_program_site_and_changelog() {
    let version = about::VERSION;

    let manifest = repo_file("Cargo.toml");
    let workspace = manifest.split("[workspace.package]").nth(1).expect("[workspace.package] im Manifest");
    let workspace = workspace.split("\n[").next().unwrap();
    assert!(
        workspace.lines().any(|l| l.trim() == format!("version = \"{version}\"")),
        "das Programm trägt die Version des Workspace-Manifests: {workspace}"
    );
    assert!(
        workspace.lines().any(|l| l.trim() == format!("repository = \"{}\"", about::REPOSITORY)),
        "Repo-Adresse im Manifest und auf der Über-Seite sind dieselbe: {workspace}"
    );

    let site = repo_file("site/hugo.toml");
    assert!(
        site.lines().any(|l| l.trim() == format!("version = \"{version}\"")),
        "die Webseite verlinkt die Pakete dieser Version (params.version; scripts/ci-site.sh prüft dasselbe in der CI)"
    );
    assert!(
        site.lines().any(|l| l.trim() == format!("baseURL = \"{}\"", about::HOMEPAGE)),
        "die Adresse der Webseite ist die, die die Über-Seite nennt"
    );
    assert!(
        site.lines().any(|l| l.trim() == format!("repo = \"{}\"", about::REPOSITORY)),
        "die Webseite verlinkt dasselbe Repo wie die Über-Seite"
    );
    // Follow-up fix R24: the former assertion demanded the empty value and would have failed as soon as the user enters
    // the fingerprint. Allowed is empty (still open) or a whole fingerprint; whether it fits the key is checked by
    // `scripts/ci-fingerprint.sh` in the release run.
    let fingerprint = site
        .lines()
        .find_map(|l| l.trim().strip_prefix("aptFingerprint = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("params.aptFingerprint steht in site/hugo.toml");
    let hex: String = fingerprint.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        hex.is_empty() || (hex.len() == 40 && hex.chars().all(|c| c.is_ascii_hexdigit())),
        "der Fingerabdruck des Archivschlüssels ist leer (trägt nur der Nutzer ein) oder vollständig (40 \
         Hex-Zeichen) – nie eine Kurz-ID oder ein Platzhaltertext: {fingerprint}"
    );

    let changelog = repo_file("CHANGELOG.md");
    // Round 32: above the version there may be a section „Unveröffentlicht“ (work since the release; the version number
    // is set only by the author) – then the section below it describes this version.
    let first = changelog
        .lines()
        .find(|l| l.starts_with("## ") && *l != "## Unveröffentlicht")
        .expect("Abschnitt in CHANGELOG.md");
    assert!(
        first == format!("## {version}") || first.starts_with(&format!("## {version} ")),
        "der oberste Abschnitt der Änderungsliste mit Versionsnummer beschreibt diese Version: {first}"
    );

    assert_eq!(
        about::release_url(),
        format!("{}/releases/tag/v{version}", about::REPOSITORY),
        "die Release-Seite der Über-Seite gehört zu dieser Version"
    );
    let readme = repo_file("README.md");
    assert!(readme.contains(&about::release_url()), "das README verlinkt dieselbe Release-Seite wie die Über-Seite");
    for file in [
        format!("baruch_{version}-1_amd64.deb"),
        format!("Baruch-{version}-x86_64.AppImage"),
        format!("Baruch-{version}-macos-arm64.dmg"),
        format!("Baruch-{version}-macos-x86_64.dmg"),
        format!("Baruch-{version}-x64.msi"),
    ] {
        assert!(readme.contains(&file), "das README nennt das Paket {file} mit dem Namen aus dem Vertrag (§1)");
    }
}

/// The private host name of the former MultiGPT default is no longer in any file that the program or the website
/// delivers (contract §0/§5). The search is assembled from parts, so that this test does not contain it itself.
#[test]
fn the_former_private_default_address_is_gone_from_what_ships() {
    let needle = ["gpt", "intranet"].join(".");
    for path in [
        "crates/baruch-store/src/connections.rs",
        "crates/baruch-server/templates/settings/verbindungen.html",
        "docs/Bedienung.md",
        "docs/Betrieb.md",
        "README.md",
        "CHANGELOG.md",
        "site/content/de/mcp.md",
    ]
    .into_iter()
    .map(String::from)
    // Round 37: the manual is in the help of the website; it is delivered like `site/content/de/mcp.md`.
    .chain(help_pages())
    {
        assert!(!repo_file(&path).contains(&needle), "{path} nennt die frühere Intranet-Adresse nicht mehr");
    }
    assert_eq!(
        baruch_store::ConnectionKind::Multigpt.default_config(),
        serde_json::json!({ "url": "" }),
        "eine neue Instanz hat für MultiGPT keine Adresse (Vertrag §4)"
    );
}

/// Follow-up fix R24 (review): what becomes public with the repo names neither the machine of the developer nor the
/// name of a third party or the title of his book. The search words are assembled, so that this test does not contain
/// them itself. Only today's state is checked; the user decides about the Git history (`docs/Backlog.md`,
/// „Schritte des Nutzers“).
#[test]
fn the_current_tree_names_no_private_machine_and_no_third_party() {
    let needles = [
        (["WS-", "Joerg"].concat(), "Rechnername des Entwicklers"),
        (["Drid", "iger"].concat(), "Name eines Dritten"),
        (["Depen", "denz"].concat(), "Titel des Buchs eines Dritten"),
        (["Narz", "issmus"].concat(), "Untertitel des Buchs eines Dritten"),
    ];
    for path in [
        "README.md",
        "CHANGELOG.md",
        "HANDOFF.md",
        "SKILLS.md",
        "AGENTS.md",
        "docs/Plan.md",
        "docs/Backlog.md",
        "docs/Fahrplan.md",
        "docs/Architektur.md",
        "docs/Bedienung.md",
        "docs/Betrieb.md",
        "docs/Verteilung.md",
        "docs/MCP.md",
        "docs/RPC.md",
        "docs/WYSIWYG.md",
        "docs/Austauschformat.md",
        "crates/baruch-core/src/sample.rs",
        "crates/baruch-server/src/assist/prompts.rs",
    ]
    .into_iter()
    .map(String::from)
    // Round 37: the help pages become public with the website; the same ban holds for them.
    .chain(help_pages())
    {
        let text = repo_file(&path);
        for (needle, what) in &needles {
            assert!(!text.contains(needle.as_str()), "{path} nennt nichts Privates oder Fremdes mehr ({what})");
        }
    }
}

/// Follow-up fix R24 (review): the access rule of `/mcp` has changed with `--allow-host`; `docs/MCP.md` and
/// `docs/RPC.md` must name it (AGENTS.md: docs in the same commit). The behaviour itself is checked by
/// `round24_betrieb::mcp_under_an_extra_name_needs_a_token_even_from_loopback`.
#[test]
fn the_interface_docs_name_the_token_rule_behind_a_proxy() {
    let mcp = repo_file("docs/MCP.md");
    let access = mcp.split("\n## Access").nth(1).expect("section „Access“ in docs/MCP.md");
    let access = access.split("\n## ").next().unwrap();
    assert!(
        access.contains("--allow-host") && access.contains("BARUCH_ALLOWED_HOSTS"),
        "the section Access names how additional host names are allowed"
    );
    assert!(
        access.contains("only with a token") && access.contains("docs/Betrieb.md"),
        "the section Access says that /mcp under such a name needs a token even from loopback, \
         and points to the operating guide"
    );
    let rpc = repo_file("docs/RPC.md");
    assert!(
        rpc.contains("--allow-host"),
        "docs/RPC.md nennt die erlaubten Rechnernamen (Host-Regel gilt für /rpc wie für /mcp)"
    );
}
