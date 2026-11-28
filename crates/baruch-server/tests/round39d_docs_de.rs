// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 39d "German translation of the developer docs", task site_de: the generator, Hugo, the checks and the rule
//! texts for the German section `/docs/`.
//!
//! The nine developer documents exist in German under `docs/de/` (the architecture in four parts); the generator
//! `site/tools/docs_pages.py` turns them into `site/generated-de/docs/`, which `site/hugo.toml` mounts as German
//! content, so that `/docs/` has the same slugs as `/en/docs/` and the language switch resolves. These tests only run
//! the Python scripts of the repository and read files: no server, no network, no Hugo.
//!
//! Tests over the real German files (`anchors_…`, `the_generator_builds_…`) are strict: the integrator runs them
//! after all translations are merged. The generator, the anchor tool and the lag check are tested on invented
//! repositories by their own selftests and by the tests below, independent of the real translations.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repo_file(path: &str) -> String {
    std::fs::read_to_string(repo().join(path)).unwrap_or_else(|e| panic!("{path} is readable: {e}"))
}

/// Runs `python3 -I <script> <args>` in the repository; (exit code, stdout, stderr).
fn python(script: &str, args: &[&str]) -> (i32, String, String) {
    let output = Command::new("python3")
        .arg("-I")
        .arg(repo().join(script))
        .args(args)
        .current_dir(repo())
        .output()
        .unwrap_or_else(|e| panic!("python3 -I {script} starts: {e}"));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The nine documents with the slug of their page, in the order of the weights.
const DOCUMENTS: [(&str, &str); 9] = [
    ("Bedienung", "usage"),
    ("Architektur", "architecture"),
    ("RPC", "rpc"),
    ("MCP", "mcp"),
    ("Austauschformat", "exchange-format"),
    ("WYSIWYG", "wysiwyg"),
    ("Betrieb", "operation"),
    ("Verteilung", "distribution"),
    ("Plan", "plan"),
];

#[test]
fn the_generator_passes_its_selftest_with_the_german_cases() {
    let (code, out, err) = python("site/tools/docs_pages.py", &["--selbsttest"]);
    assert_eq!(code, 0, "the selftest of docs_pages.py passes (exit 0): {out}{err}");
    assert!(
        out.contains("German: a heading without explicit id is a finding and nothing is written")
            && out.contains("German: part files are joined in numeric order"),
        "the selftest covers the German run (explicit ids, parts); a selftest without these cases would let the \
         German generator rot unnoticed:\n{out}"
    );
    assert!(!out.contains("FEHLER"), "no assertion of the selftest failed: {out}");
}

#[test]
fn the_german_list_has_the_nine_pages_with_the_english_slugs() {
    let (code, german, err) = python("site/tools/docs_pages.py", &["--list", "--lang", "de"]);
    assert_eq!(code, 0, "--list --lang de works: {err}");
    let (_, english, _) = python("site/tools/docs_pages.py", &["--list", "--lang", "en"]);
    let german: Vec<Vec<&str>> = german.lines().map(|l| l.split('\t').collect()).collect();
    let english: Vec<Vec<&str>> = english.lines().map(|l| l.split('\t').collect()).collect();
    assert_eq!(german.len(), 9, "nine German pages: contract §site_de, one per document: {german:?}");
    for ((name, slug), (de, en)) in DOCUMENTS.iter().zip(german.iter().zip(english.iter())) {
        assert_eq!(de[0], format!("docs/de/{name}.md"), "the German source of {name} lies in docs/de/");
        assert_eq!(
            de[1],
            format!("/docs/{slug}/"),
            "German is the default language, so its section is /docs/ without a language folder"
        );
        assert_eq!(
            en[1],
            format!("/en/docs/{slug}/"),
            "the English section keeps its fixed address (other pages link to it)"
        );
        assert_eq!(de[2], en[2], "the weights are the same in both languages: the order of the pages does not change");
    }
    let (_, all, _) = python("site/tools/docs_pages.py", &["--list"]);
    assert_eq!(all.lines().count(), 18, "--list without --lang names both languages (9 + 9 rows)");
}

#[test]
fn the_anchor_tool_checks_what_the_translators_must_keep() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let english = dir.path().join("En.md");
    let good = dir.path().join("De.md");
    let missing_id = dir.path().join("DeNoId.md");
    let wrong_level = dir.path().join("DeLevel.md");
    std::fs::write(&english, "# Title\n\n## Same\n\n## Same\n\n```\n# not a heading\n```\n").unwrap();
    std::fs::write(&good, "# Titel {#title}\n\n## Gleich {#same}\n\n## Gleich {#same-1}\n").unwrap();
    std::fs::write(&missing_id, "# Titel {#title}\n\n## Gleich {#same}\n\n## Gleich\n").unwrap();
    std::fs::write(&wrong_level, "# Titel {#title}\n\n## Gleich {#same}\n\n### Gleich {#same-1}\n").unwrap();
    let (e, g, m, l) = (
        english.to_str().unwrap(),
        good.to_str().unwrap(),
        missing_id.to_str().unwrap(),
        wrong_level.to_str().unwrap(),
    );
    let (code, out, _) = python("scripts/docs-anchors.py", &[e]);
    assert_eq!(code, 0, "listing the anchors of a file works");
    assert_eq!(
        out.lines().map(|l| l.split('\t').nth(2).unwrap_or("")).collect::<Vec<_>>(),
        ["title", "same", "same-1"],
        "duplicates are numbered GitHub style and a heading inside a code block is not a heading"
    );
    let (code, out, _) = python("scripts/docs-anchors.py", &["--check", g, e]);
    assert_eq!(code, 0, "a translation with the same levels and the English ids passes: {out}");
    let (code, out, _) = python("scripts/docs-anchors.py", &["--check", m, e]);
    assert_eq!(code, 1, "a heading without explicit id is a finding (exit 1): the German anchors would differ");
    assert!(out.contains("no explicit id, expected {#same-1}"), "the finding names the expected id: {out}");
    let (code, out, _) = python("scripts/docs-anchors.py", &["--check", l, e]);
    assert_eq!(code, 1, "a different heading level is a finding: {out}");
    let (code, _, _) = python("scripts/docs-anchors.py", &["--check", g]);
    assert_eq!(code, 2, "a call without the English file is a usage error (exit 2)");
}

#[test]
fn anchors_of_all_nine_documents_are_identical_in_both_languages() {
    for (name, _) in DOCUMENTS {
        let english = format!("docs/{name}.md");
        let german = if name == "Architektur" {
            (1..=4).map(|n| format!("docs/de/Architektur.part{n}.md")).collect::<Vec<_>>().join(",")
        } else {
            format!("docs/de/{name}.md")
        };
        let (code, out, err) = python("scripts/docs-anchors.py", &["--check", &german, &english]);
        assert_eq!(
            code, 0,
            "{german} has the same heading levels as {english} and the explicit English ids (D4): every link anchor \
             stays the English slug, so both sections link alike:\n{out}{err}"
        );
    }
}

#[test]
fn the_generator_builds_both_sections_from_the_real_files() {
    // A site folder of its own with only hugo.toml (the generator reads params.repo from it), so that nothing is
    // written into the repository.
    let site = tempfile::tempdir().expect("a temporary site folder");
    std::fs::write(site.path().join("hugo.toml"), repo_file("site/hugo.toml")).unwrap();
    let (code, out, err) = python(
        "site/tools/docs_pages.py",
        &["--site", site.path().to_str().unwrap(), "--repo", repo().to_str().unwrap(), "--lang", "all"],
    );
    assert_eq!(
        code, 0,
        "the generator accepts the real documents of both languages (explicit ids, same heading sequence, no dead \
         anchor or file link):\n{out}{err}"
    );
    for (folder, base) in [("generated", "en"), ("generated-de", "de")] {
        let docs = site.path().join(folder).join("docs");
        assert!(docs.join("_index.md").is_file(), "{base}: the section has its index page");
        for (_, slug) in DOCUMENTS {
            assert!(docs.join(format!("{slug}.md")).is_file(), "{base}: the page {slug} is written");
        }
    }
    let rpc = std::fs::read_to_string(site.path().join("generated-de/docs/rpc.md")).unwrap();
    assert!(
        rpc.starts_with("---\ntitle: "),
        "the German page starts with its front matter: {}",
        &rpc[..40.min(rpc.len())]
    );
    assert!(rpc.contains("quelle: \"docs/de/RPC.md\""), "the front matter names the German source file");
}

#[test]
fn the_lag_check_runs_informationally_and_fails_only_with_strict() {
    let (code, out, err) = python("scripts/check-doc-lag.py", &[]);
    assert_eq!(code, 0, "check-doc-lag.py exits 0 without --strict, whatever it reports (informational): {out}{err}");
    assert!(
        out.lines().all(|l| l == "ok" || l.starts_with("lag: ") || l.starts_with("missing: ")) && !out.is_empty(),
        "every output line is `ok`, `lag: …` or `missing: …` (the format of the contract): {out}"
    );
    let (strict, _, _) = python("scripts/check-doc-lag.py", &["--strict"]);
    assert_eq!(
        strict,
        if out.trim() == "ok" { 0 } else { 1 },
        "--strict turns lag or a missing file into exit 1 and nothing else"
    );
    let (usage, _, _) = python("scripts/check-doc-lag.py", &["--unknown"]);
    assert_eq!(usage, 2, "a wrong call is a usage error (exit 2)");
    let (code, out, err) = python("scripts/check-doc-lag.py", &["--selftest"]);
    assert_eq!(code, 0, "the selftest of the lag check passes (invented git repository): {out}{err}");
}

#[test]
fn the_other_scripts_pass_their_selftests_with_the_german_section() {
    let (code, out, err) = python("scripts/site-check.py", &["--selbsttest"]);
    assert_eq!(code, 0, "site-check.py passes its selftest, German /docs/ cases included: {out}{err}");
    assert!(out.contains("unter /docs/ wird gefunden"), "the selftest has cases for the German section: {out}");
    let (code, out, err) = python("scripts/check-doc-language.py", &["--selftest"]);
    assert_eq!(code, 0, "check-doc-language.py passes its selftest (docs/de/ is the allowed-German area): {out}{err}");
}

#[test]
fn hugo_mounts_the_german_output_and_names_the_menu_entry() {
    let toml = repo_file("site/hugo.toml");
    let module = toml.split_once("\n[module]\n").map(|(_, m)| m).expect("hugo.toml has a [module] table");
    let mounts: Vec<&str> = module.split("[[module.mounts]]").skip(1).collect();
    let german = mounts
        .iter()
        .find(|m| m.contains("source = \"generated-de\""))
        .expect("hugo.toml mounts site/generated-de (the German output of the generator)");
    assert!(
        german.contains("target = \"content\"") && german.contains("languages = [\"de\"]"),
        "generated-de/ is content of the language de only: its pages are German, English has its own output"
    );
    let english = mounts.iter().find(|m| m.contains("source = \"generated\"")).expect("the English mount stays");
    assert!(english.contains("languages = [\"en\"]"), "generated/ stays English only: the two outputs must not mix");
    assert!(
        toml.contains("      name = \"Dokumentation\"\n      pageRef = \"/docs\"\n      weight = 60"),
        "the German menu entry is „Dokumentation“ with pageRef /docs and weight 60, the same place as English"
    );
    assert!(
        !toml.contains("Dokumentation (englisch)") && !toml.contains("url = \"/en/docs/\""),
        "the old entry that led to the English section by address is gone"
    );
    assert!(
        toml.contains("name = \"Documentation\"\n      pageRef = \"/docs\"\n      weight = 60"),
        "the English entry is unchanged"
    );
    let ignore = repo_file(".gitignore");
    assert!(
        ignore.lines().any(|l| l.trim() == "site/generated-de/"),
        ".gitignore names site/generated-de/: the pages are rebuilt on every site build, a committed copy would fall \
         behind the translations"
    );
}

#[test]
fn ci_site_generates_both_languages_shows_the_lag_and_checks_the_german_pages() {
    let script = repo_file("scripts/ci-site.sh");
    let lag = script.find("check-doc-lag.py").expect("ci-site.sh calls the lag check");
    let generator = script.find("\"$docs_generator\" --site").expect("ci-site.sh runs the generator");
    let hugo = script.find("\"$hugo\" --minify --panicOnWarning -s \"$site\"").expect("ci-site.sh builds with Hugo");
    assert!(lag < generator && generator < hugo, "order: lag check (informational), generator, Hugo");
    assert!(
        script.contains("check-doc-lag.py\" || true"),
        "the lag check never stops the run (informational): without `|| true` a late translation would fail the \
         site build under `set -e`"
    );
    assert!(script.contains("--lang all"), "the generator runs for both languages");
    assert!(
        script.contains("/en/docs/ /docs/ $(") && script.contains("--list --lang all | cut -f2"),
        "after the build ci-site.sh checks /en/docs/, /docs/ and every page of both languages: a missing mount \
         builds silently"
    );
}

#[test]
fn the_rule_is_described_where_developers_look() {
    for (file, needles) in [
        ("AGENTS.md", &["`docs/de/`", "`scripts/check-doc-lag.py`", "Round 39d"][..]),
        ("docs/Bedienung.md", &["`docs/de/`", "`scripts/check-doc-lag.py`"][..]),
        ("docs/Verteilung.md", &["`docs/de/`", "`scripts/check-doc-lag.py`", "`site/tools/docs_pages.py`"][..]),
        ("site/README.md", &["site/generated-de/", "docs/de/", "check-doc-lag.py", "not checked in", "/docs/"][..]),
        ("site/tools/README.md", &["docs/de/", "--lang", "generated-de", "explicit id"][..]),
    ] {
        let text = repo_file(file);
        for needle in needles {
            assert!(
                text.contains(needle),
                "{file} describes the German developer docs (docs grow with the code, AGENTS.md): „{needle}“"
            );
        }
    }
}
