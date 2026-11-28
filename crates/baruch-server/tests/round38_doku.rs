// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 38 "English for agents and developers", task regeln_doku, and Round 38c "developer docs in English, rest":
//! the rules (`AGENTS.md`), the developer docs and the history files.
//!
//! These tests only read files of the repository: no server, no network, no data folder. They check what a
//! translation can silently lose or leave half done: a rule missing from `AGENTS.md`, German prose left in a file
//! that counts as translated, a history file without the head paragraph, the work language missing from the
//! developer docs.

use std::path::PathBuf;

/// The state of an entry of [`FILES`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    /// English as a whole; German only quoted or as code.
    Translated,
    /// Not yet translated: still German, except for sections added since Round 38 (those are English).
    Pending,
}

/// The Markdown files of the translation tasks of Rounds 38 and 38c (the release notes in `release.yml` are covered
/// by [`the_release_notes_are_english`]). An entry flips from `Pending` to `Translated` in the commit that translates
/// its last line. In Round 38c `docs/Architektur.md` was translated in two halves split at a marker line; the
/// integrator joined them and removed the marker, so every entry now covers its whole file.
const FILES: [(&str, State); 12] = [
    // Round 38, regeln_doku (done).
    ("AGENTS.md", State::Translated),
    ("README.md", State::Translated),
    ("docs/Betrieb.md", State::Translated),
    // Round 38c, skills_plan.
    ("SKILLS.md", State::Translated),
    ("docs/Plan.md", State::Translated),
    ("docs/Verteilung.md", State::Translated),
    ("docs/Bedienung.md", State::Translated),
    // Round 38c, mcp_austausch_wysiwyg.
    ("docs/MCP.md", State::Translated),
    ("docs/Austauschformat.md", State::Translated),
    ("docs/WYSIWYG.md", State::Translated),
    // Round 38c, rpc_doku.
    ("docs/RPC.md", State::Translated),
    // Round 38c, architektur_1 and architektur_2 (joined by the integrator).
    ("docs/Architektur.md", State::Translated),
];

/// The split marker of Round 38c; it must not survive the join.
const OLD_SPLIT: &str = "R38c split";

/// The four history files (contract §3).
const HISTORY: [&str; 4] = ["docs/Backlog.md", "docs/Fahrplan.md", "HANDOFF.md", "CHANGELOG.md"];

/// The head paragraph of the history files, literally from the contract.
const HEAD: &str = "> From Round 38 on, new entries are written in English; older entries stay German.";

/// Umlauts that may stand in the prose of a translated file: `(file, text, reason)`. Everything else German must
/// be a quotation („…“ or "…") or code.
const ALLOWED: [(&str, &str, &str); 1] =
    [("README.md", "© Jörg Bernau", "the name of the author in the copyright line")];

fn repo_file(path: &str) -> String {
    let full = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display()))
}

fn is_german_letter(c: char) -> bool {
    matches!(c, 'ä' | 'ö' | 'ü' | 'Ä' | 'Ö' | 'Ü' | 'ß')
}

/// The prose of a Markdown file: fenced code blocks, code spans, German quotations („…“) and straight quotations
/// ("…") are blanked out, line by line, so that line numbers stay. A straight quotation ends at the end of its line
/// at the latest, a code span and a German quotation at the end of their paragraph – an unbalanced mark then cannot
/// hide the rest of the file.
fn prose(text: &str) -> Vec<String> {
    #[derive(Clone, Copy, PartialEq)]
    enum In {
        Prose,
        Code,
        German,
        Straight,
    }
    let mut out = Vec::new();
    let mut fenced = false;
    let mut state = In::Prose;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            state = In::Prose;
            out.push(String::new());
            continue;
        }
        if fenced {
            out.push(String::new());
            continue;
        }
        if line.trim().is_empty() || state == In::Straight {
            state = In::Prose;
        }
        let mut kept = String::new();
        for c in line.chars() {
            match (state, c) {
                (In::Prose, '`') => state = In::Code,
                (In::Prose, '„') => state = In::German,
                (In::Prose, '"') => state = In::Straight,
                (In::Prose, c) => kept.push(c),
                (In::Code, '`') | (In::German, '“') | (In::Straight, '"') => state = In::Prose,
                _ => {}
            }
        }
        out.push(kept);
    }
    out
}

/// Lines of the prose of a file that still carry a German letter, without the allowed exceptions.
fn german_lines(file: &str) -> Vec<String> {
    let mut cleaned = repo_file(file);
    for (owner, allowed, _) in ALLOWED {
        if owner == file {
            cleaned = cleaned.replace(allowed, "");
        }
    }
    prose(&cleaned)
        .into_iter()
        .enumerate()
        .filter(|(_, line)| line.chars().any(is_german_letter))
        .map(|(at, line)| format!("{file}:{}: {line}", at + 1))
        .collect()
}

#[test]
fn the_prose_filter_keeps_prose_and_drops_quotations_and_code() {
    let lines = prose("Größe „Über\nBaruch“ über `für` \"Tür\" Maß\n```\nÄrger\n```\nöde\n");
    assert_eq!(lines[0], "Größe ", "prose before a German quotation stays; the quotation is dropped");
    assert_eq!(
        lines[1], " über   Maß",
        "a German quotation may run over a line break; code spans and \"…\" are dropped"
    );
    assert_eq!(lines[3], "", "a fenced block is code as a whole (commands and file contents stay as they are)");
    assert_eq!(lines[5], "öde", "after the fence prose counts again – the filter does not swallow the rest of a file");
    let open = prose("eine \"offene Klammer\nÄrger\n");
    assert_eq!(open[1], "Ärger", "an unbalanced straight quotation mark ends with its line and hides nothing below");
}

#[test]
fn every_file_of_the_task_is_translated_or_still_german() {
    for (at, (file, _)) in FILES.iter().enumerate() {
        assert!(
            !FILES[..at].iter().any(|(f, _)| f == file),
            "{file} is listed at most once in FILES (one state per file, contract §1)"
        );
        assert!(
            !repo_file(file).contains(OLD_SPLIT),
            "{file}: the split marker of Round 38c is gone after the join – a leftover marker would mean a half the \
             test no longer knows"
        );
    }
    for (file, state) in FILES {
        if state != State::Pending {
            continue;
        }
        let left = german_lines(file).len();
        assert!(
            left >= 20,
            "{file} is listed as not yet translated but has only {left} German lines of prose: a translated file \
             belongs to State::Translated, where the check for leftover German applies to it"
        );
    }
}

#[test]
fn translated_files_carry_german_only_as_quotation_or_code() {
    for (file, allowed, reason) in ALLOWED {
        assert!(
            repo_file(file).contains(allowed),
            "the exception „{allowed}“ ({reason}) still occurs in {file} – an exception nobody needs is removed"
        );
    }
    for (file, state) in FILES {
        if state != State::Translated {
            continue;
        }
        let left = german_lines(file);
        assert!(
            left.is_empty(),
            "{file} is English: umlauts and ß only inside „…“, \"…\" or code (quoted texts of the German user \
             interface, paths, commands). Leftover German prose:\n{}",
            left.join("\n")
        );
    }
}

#[test]
fn the_history_files_announce_english_and_stay_german_below() {
    for file in HISTORY {
        let text = repo_file(file);
        let mut lines = text.lines();
        assert!(lines.next().is_some_and(|l| l.starts_with("# ")), "{file} begins with its heading");
        assert_eq!(lines.next(), Some(""), "{file}: an empty line follows the heading");
        assert_eq!(
            lines.next(),
            Some(HEAD),
            "{file}: the head paragraph stands literally and directly under the first heading (contract §3)"
        );
        assert_eq!(text.matches(HEAD).count(), 1, "{file}: the head paragraph stands exactly once");
    }
    let changelog = repo_file("CHANGELOG.md");
    let unreleased = changelog
        .split("\n## Unveröffentlicht\n")
        .nth(1)
        .expect("CHANGELOG.md keeps the heading „Unveröffentlicht“ (other tests look for it)");
    let first = unreleased.lines().find(|l| l.starts_with("### ")).expect("a section below „Unveröffentlicht“");
    assert_eq!(
        first, "### User interface in English and German (Round 39a)",
        "the newest round is the first section below „Unveröffentlicht“"
    );
    assert!(
        unreleased.contains("\n### English as the primary language (Round 38)\n"),
        "the section of Round 38 stays below „Unveröffentlicht“, after the newer round"
    );
    assert!(
        changelog.contains("### Hilfe auf der Webseite (Runde 37)"),
        "older entries of the change log stay German – nothing is translated retroactively"
    );
}

#[test]
fn the_rules_for_agents_are_english_and_none_is_lost() {
    let rules = repo_file("AGENTS.md");
    // One keyword per rule of the German file (state 3fa5806), in its order.
    for (keyword, rule) in [
        ("keyring", "keys belong in the keyring"),
        ("version numbers", "only the user sets version numbers"),
        ("Push only after", "push only after the user's explicit yes"),
        ("taskset -c 0-7", "power: building and testing bound to eight threads"),
        ("RUST_TEST_THREADS=4", "power: limited test parallelism"),
        ("Never run several full test runs", "power: no parallel full runs"),
        ("own** target/", "machine load: every worktree builds into its own target directory"),
        ("CARGO_TARGET_DIR", "machine load: no shared build directory, with the reason"),
        ("Firefox", "machine load: no Firefox in parallel with test runs"),
        ("development/main", "branches: work on development, main only by the user"),
        ("Do not guess", "evidence before claims"),
        ("STUB:", "stubs answer honestly and are marked"),
        ("docs/Fahrplan.md", "the PM maintains the roadmap"),
        ("docs/Bedienung.md", "docs grow with the code: usage"),
        ("docs/Architektur.md", "docs grow with the code: structure"),
        ("docs/RPC.md", "docs grow with the code: RPC"),
        ("docs/MCP.md", "docs grow with the code: MCP"),
        ("in the same commit", "docs are updated in the same commit"),
        ("exactly one current revision", "revisions: one current revision per section"),
        ("logged in the mirror", "revisions: changes by the author are logged"),
        ("only make suggestions", "revisions: assistants and editors only suggest"),
        ("outline_add", "outline exception: assistants may create items"),
        ("2026-11-07", "outline addition: a new part that takes over chapters is a request"),
        ("LEKTOR_ROUTES", "accounts: the route list of editors"),
        ("LEKTOR_RPC", "accounts: the RPC list of editors"),
        ("`/mcp` stays with tokens", "accounts: MCP keeps its tokens"),
        ("„fest <Persona>“", "editor persona by setting, the German label quoted"),
        ("„automatisch nach Kontext“", "editor persona by setting, the second German label quoted"),
        ("without JavaScript", "everything stays usable without JavaScript"),
        ("No Iced", "user interface: no Iced"),
    ] {
        assert!(rules.contains(keyword), "AGENTS.md still names the rule „{rule}“ (keyword „{keyword}“)");
    }
    // The only rule that changed, and the new line (contract §3).
    for (keyword, why) in [
        ("primary language is English for code, comments, commits, developer docs, MCP and RPC", "what is English"),
        ("German and English through catalogs from Round 39 (until then German)", "the language of the user interface"),
        ("work language", "each work has its own language"),
        ("(`de`|`en`)", "the two work languages"),
        ("book output and persona answers follow it", "what the work language governs"),
        ("site/content/de/hilfe/", "where the user help lives"),
        ("site/content/de/betrieb.md", "where the description for operators lives"),
        ("developer entry points", "what docs/Bedienung.md and docs/Betrieb.md are since Round 37"),
        ("Exceptions that stay German", "the rule names its exceptions (finding 4 of Round 38)"),
        ("catalogs of Round 39a", "the user interface stays German until the catalogs of Round 39a"),
        ("its English translation lives in", "the German user help has its English translation since Round 39b"),
        ("`site/content/en/hilfe/`", "Round 39b: where the English user help lives"),
        ("`crates/baruch-server/locales/en/*.ftl` (base)", "Round 39b: the English catalog is the base"),
        ("translation keeping the old German wording", "Round 39b: German is the translation, wording kept"),
        ("one file per area", "Round 39b: keys by area"),
        ("`crates/baruch-core/locales/{en,de}/book.ftl`", "Round 39b: book words live in baruch-core"),
        ("follow the work language", "Round 39b: book words follow the work language, not the interface"),
        ("`docs/schema/`", "the schema docs stay German"),
    ] {
        assert!(rules.contains(keyword), "AGENTS.md states the new rule – {why}: „{keyword}“");
    }
    assert!(
        !rules.contains("Oberfläche und Dokumentation Deutsch"),
        "the old language rule is replaced, not kept beside the new one (two rules would contradict each other)"
    );
}

#[test]
fn the_readme_says_that_website_and_interface_are_german() {
    let readme = repo_file("README.md");
    assert!(
        readme.contains("The user interface is German and English")
            && readme.contains("English <https://baruch.bernau.family/en/hilfe/>"),
        "an English reader learns before installing which languages the program and its help speak (contract §3; \
         Round 39b: both languages)"
    );
    for (anchor, heading) in [
        ("(#principles)", "\n## Principles\n"),
        ("(#quick-start)", "\n## Quick start\n"),
        ("(#data-backup-and-security)", "\n## Data, backup and security\n"),
        ("(#status-and-limits)", "\n## Status and limits\n"),
        ("(#license)", "\n## License\n"),
    ] {
        assert!(
            readme.contains(anchor) && readme.contains(heading),
            "the table of contents links {anchor} and the heading for it exists – translated headings change \
             their anchors"
        );
    }
    assert!(
        !readme.contains("(#grundsätze)") && !readme.contains("(#stand-und-grenzen)"),
        "no link to an anchor of a German heading is left"
    );
}

#[test]
fn the_release_notes_are_english() {
    let release = repo_file(".github/workflows/release.yml");
    for wanted in [
        "a free writing workshop for sermons, talks and books (AGPL-3.0-or-later). The user interface is in German.",
        "Installation and help: https://baruch.bernau.family/installation/",
        "Checksums: SHA256SUMS.",
        "- macOS: not signed and not notarized (Gatekeeper will warn; see the installation page).",
        "- macOS: signed (Developer ID) and notarized by Apple.",
        "- Windows: not signed (SmartScreen will warn; see the installation page).",
        "- Windows: signed.",
    ] {
        assert!(release.contains(wanted), "release.yml writes the release notes in English: „{wanted}“");
    }
    let notes = release
        .split("} > notes.md")
        .next()
        .and_then(|before| before.rsplit("version=\"$(sh scripts/version.sh \"$TAG\")\"").next())
        .expect("the block that writes notes.md");
    assert!(
        !notes.chars().any(is_german_letter),
        "no German sentence is left in the text of the release notes (they are read on GitHub by everyone): {notes}"
    );
}

#[test]
fn the_developer_docs_describe_the_work_language() {
    for (file, needles) in [
        (
            "docs/Architektur.md",
            &[
                "\n## Round 38 – English as the primary language, work language\n",
                "0055_work_language.sql",
                "CHECK (language IN ('de', 'en'))",
                "`crates/baruch-core/src/language.rs`",
                "`crates/baruch-store/src/language.rs`",
                "`crates/baruch-server/src/work_language.rs`",
                "`crates/baruch-server/src/assist/lang.rs`",
                "Work language is not the language of the user interface.",
                "`TypesetSettings.lang`",
                "`work.language_set {work, language}`",
                "`POST /einstellungen/werk/sprache`",
                "every detection accepts both languages",
                "| `critical` | „Kritisch:“ | `Critical:` |",
                "| `my_proposal` | „Mein Vorschlag:“ | `My proposal:` |",
                "`/einstellungen/werk?sprachfehler=werk#sprache`",
                "„Die Werksprache ließ sich nicht speichern: dieses Werk gibt es nicht; nichts geändert.“",
                "`work_language::fallback(work, read)`",
                "`work language of work <id> unreadable, answering in German: <error>`",
            ][..],
        ),
        (
            "docs/Austauschformat.md",
            &[
                "\n## Work language (minor version 10, Round 38)\n",
                "`version=\"10\"`",
                "docs/schema/baruch-werk-1-u10.xsd",
                "| `sprache` | work language, `de` or `en` (`work.language`) | `de` |",
                "`<angaben>` carries one more attribute",
                "every file up to minor version 9 – is read as `de`",
                "`ExchangeLanguage { De, En }`",
            ][..],
        ),
        (
            "docs/Bedienung.md",
            &[
                "block „Werksprache“",
                "`POST /einstellungen/werk/sprache`",
                "`ok=sprache#sprache`",
                "`sprachfehler=1#sprache`",
                "`sprachfehler=werk#sprache`",
                "`work.language_set`",
                "usable without JavaScript",
                "not in `LEKTOR_ROUTES`",
            ][..],
        ),
    ] {
        let text = repo_file(file);
        for needle in needles {
            assert!(
                text.contains(needle),
                "{file} documents the work language (contract §3 and §4; docs grow with the code): „{needle}“"
            );
        }
    }
}
