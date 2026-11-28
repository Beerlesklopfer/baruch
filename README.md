# Baruch

**A free writing workshop for books and sermons.** Baruch keeps a work as an outline of parts,
chapters and sections, preserves every revision of every section, typesets the book with
[Typst](https://typst.app) and lets AI assistants read along, review and suggest – without taking the last
word away from the human.

> Named after Baruch, Jeremiah's scribe, who wrote the burnt scroll down again, and enlarged
> (Jer 36:32). Revisions are the core.

Baruch is licensed under the [AGPL-3.0](LICENSE) and is deliberately a free counterpart to paid
writing tools. It runs locally on your own machine, and the data stays there.

Website: <https://baruch.bernau.family>

> The website and the user interface are in German: <https://baruch.bernau.family/>. Menu entries, buttons and
> messages are quoted in German in this document, with an English explanation where needed.

**Status:** version 0.9.0. Baruch is used daily for a real book and for sermons. There are packages
for Debian and Ubuntu (also through an APT repository), an AppImage, and unsigned packages for macOS and
Windows; interfaces may still change (see [Status and limits](#status-and-limits), changes in
[CHANGELOG.md](CHANGELOG.md)).

## Contents

- [Principles](#principles)
- [Quick start](#quick-start) (installing, building from source, starting)
- [A tour](#a-tour)
- [What Baruch can do](#what-baruch-can-do)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Setting up assistants and models](#setting-up-assistants-and-models)
- [Baruch with Claude Code and other MCP clients](#baruch-with-claude-code-and-other-mcp-clients)
- [Command line](#command-line)
- [Data, backup and security](#data-backup-and-security)
- [Structure](#structure)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [Status and limits](#status-and-limits)
- [License](#license)

## Principles

- **The human decides.** Assistants write suggestions, posts, ideas and requests. Accepting,
  changing and deleting book text stay with the human. Whatever restructures the outline, assistants only file as
  a request that the human approves.
- **Nothing gets lost.** Each section has any number of revisions; exactly one is "current". Every
  change becomes a commit in a Git mirror of the work – with its origin (human, persona, MCP client).
- **Typst is the source.** The mirror is an ordinary Typst folder that can be typeset with
  `typst compile main.typ` even without Baruch – or with Baruch itself: `baruch-typst compile main.typ` is the
  same program as a Typst command line (built-in engine, without network and without packages; `compile`,
  `fonts`, `--version`; [docs/Verteilung.md](docs/Verteilung.md), section "Typst command line").
- **Usable without JavaScript.** Every page works with links and forms. JavaScript adds comfort:
  dragging, keyboard, popups, live updates.
- **Local and honest.** Baruch answers only on your own machine. Keys live in the keyring.
  External models exist only after explicit approval. What does not work, Baruch says – nothing is silently
  shortened, discarded or guessed.

## Quick start

### Installing

The packages are on the [release page](https://github.com/Beerlesklopfer/baruch/releases/tag/v0.9.0), together
with `SHA256SUMS` for comparison. The website <https://baruch.bernau.family/> walks through the installation
step by step; all details are in [docs/Verteilung.md](docs/Verteilung.md).

**Debian 13 and Ubuntu 24.04 or newer (amd64) through the APT repository** – this way new versions arrive
with `sudo apt upgrade` as well:

```sh
sudo curl -fsSL -o /usr/share/keyrings/baruch-archive-keyring.gpg \
  https://baruch.bernau.family/apt/baruch-archive-keyring.gpg

sudo tee /etc/apt/sources.list.d/baruch.sources >/dev/null <<'EOF'
Types: deb
URIs: https://baruch.bernau.family/apt
Suites: stable
Components: main
Architectures: amd64
Signed-By: /usr/share/keyrings/baruch-archive-keyring.gpg
EOF

sudo apt update
sudo apt install baruch
```

Fingerprint of the archive key (compare before `apt update` with
`gpg --show-keys --with-fingerprint /usr/share/keyrings/baruch-archive-keyring.gpg`):
`BEBD FEF8 F980 2EC5 8A90  B818 9A9C 9199 6BB8 8DB4`

If a placeholder still stands here instead of a fingerprint, the fingerprint has not been published and the
package source is not yet meant for end users: the key file comes from the same website as the packages and can
only be verified with a fingerprint that is known through a second channel (this README in the repository). A
signed release only comes about when the value here, on the website and in `docs/Verteilung.md` matches the key
(`scripts/ci-fingerprint.sh`).

| System | File | Installing |
|---|---|---|
| Debian, Ubuntu (amd64) | `baruch_0.9.0-1_amd64.deb` | `sudo apt install ./baruch_0.9.0-1_amd64.deb` |
| Linux in general (x86_64) | `Baruch-0.9.0-x86_64.AppImage` | `chmod +x Baruch-0.9.0-x86_64.AppImage`, then start it |
| macOS, Apple Silicon | `Baruch-0.9.0-macos-arm64.dmg` | open, drag Baruch to "Applications" (unsigned) |
| macOS, Intel | `Baruch-0.9.0-macos-x86_64.dmg` | as above (unsigned) |
| Windows (x64) | `Baruch-0.9.0-x64.msi` | run it (unsigned, SmartScreen warns) |

The packages for macOS are signed and notarized by Apple (since 0.9.0), the package for Windows is **not
signed**; both have not yet been tried on a real device. How to start them is described in
[docs/Verteilung.md](docs/Verteilung.md). Every package contains the license, the licenses of the bundled
libraries (`THIRD-PARTY-LICENSES.txt`) and the pointer to the source code. No package creates anything in the
data folder, and removing it deletes nothing there.

After the installation, "Baruch" in the start menu or `baruch` in the terminal starts the local server and opens
the user interface in the browser.

**Updating:** Baruch never asks the network for new versions on its own. With the APT repository
`sudo apt upgrade` is enough; otherwise you download the new package from the release page. Which version is
running is shown by „Baruch ▾ → Über Baruch“ (About Baruch) or `baruch-server --version`.

### Building from source

You need:

- Rust (edition 2024, i.e. a current stable toolchain);
- Node.js with npm (only for the TypeScript compiler of the user interface);
- `protoc` (Protocol Buffers, for LanceDB) in the `PATH`;
- optionally `xmllint`, `pdftotext` (Poppler) and Java with EPUBCheck for additional checks.

```sh
git clone https://github.com/Beerlesklopfer/baruch.git
cd baruch
(cd crates/baruch-server/web && npm ci)
nice -n 10 ionice -c3 cargo install --locked --path crates/baruch-server -j 4
```

The program ends up in `~/.cargo/bin/baruch-server`. `scripts/build-deb.sh` builds your own `.deb`
(needs `cargo-deb`), see [docs/Verteilung.md](docs/Verteilung.md).

### Starting

```sh
baruch                              # from a package: start the server and open the browser
baruch-server                       # only the server on http://127.0.0.1:8765/
baruch-server --data /tmp/probe     # for trying things out with a data folder of its own
```

Then open `http://127.0.0.1:8765/` in the browser. A new database comes with a sample work.

Quitting: menu „Baruch ▾ → Baruch beenden…“ (quit Baruch; only on your own machine), Ctrl+C in the terminal or
`systemctl --user stop baruch.service`. Baruch shuts down cleanly in each case.

### Your first own work

- „Baruch ▾ → Neues Werk / Importieren…“ (new work / import): create a book or a sermon (title, author,
  skeleton, audience, Bible translation) – or read in an existing file: Typst, Markdown (also as a ZIP with
  images), PDF, EPUB or a `.baruch` exchange file. When reading in, Baruch first shows a preview of the
  outline; nothing is created before you confirm.
- For permanent operation as a user service see [packaging/systemd/README.md](packaging/systemd/README.md);
  the `.deb` ships an example for this under `/usr/share/doc/baruch/examples/baruch.service`, but does not set
  up a service itself.

## A tour

The workplace has three areas whose width can be dragged:

- **The outline on the left:** parts, chapters and sections as a tree, with numbers according to the chosen
  scheme, speaking time, badges for open suggestions and findings. A click opens the section; renaming,
  moving, changing the level and "number on/off" go through the context menu or forms.
- **The editor in the middle:** the typeset view (the section as it looks under the work template) or the
  Typst source, with the ribbon above it.
- **The accordions on the right:** assistants, revisions, findings and the section chat.

At the top there is a header with the menus „Werk“ (work), „Ansicht“ (view) and „Werkzeuge“ (tools), the
light/dark switch and the „Einstellungen“ (settings; as a popup, without JavaScript as a page of its own).

## What Baruch can do

### Works, outline and revisions

- **Works:** books and sermons, any number side by side, each with title, subtitle, author,
  audience, Bible translation and book details.
- **Outline:** parts, chapters and sections, each item with or without a number. Markdown headings
  (`# `, `## ` …) become outline items while typing. A new part under which existing chapters
  would move needs the active approval of the human.
- **Outline schemes:** thirteen ways of numbering, switchable at any time and back again – decimal
  (1.1), Roman, „Teil I · Kapitel 1“, „Erstes Kapitel“, legal (§ 1 · Abs. 1), **sermon**
  (Einleitung · I. · II. · Schluss), **book** (like sermon, parts as „Erster Teil“) or a pattern of your own.
  Typst sets the same numbers as the outline.
- **Revisions:** each section has any number of revisions as a tree. Exactly one is current; switching
  is possible at any time, nothing is silently displaced. Suggestions of the assistants are revisions of their
  own, on request in several variants side by side.
- **Git mirror:** every change becomes a commit in the mirror of the work, with its origin in the commit.
- **Requests:** renaming, moving, changing the level, deleting, replacing images, setting the cover –
  assistants only file these as a request. The human confirms or declines, and both are logged.

### Writing

- **Typeset view:** the section appears the way it is typeset and is edited there – paragraphs,
  headings, emphasis, lists, term lists, quotations, footnotes, links, scenes, images. Whatever cannot be
  edited as a block stays a Typst island in place. The round trip Typst ↔ typeset view is
  lossless.
- **Source:** the same revision as Typst text, switchable.
- **Ribbon:** buttons for the formats of the work template (scene, assignment, footnote, quotation, list, term,
  break, link, Bible reference, literature reference, cross-reference) – a button only appears when the
  template knows the format – and "image at the cursor position".
- **Find and replace** as in VS Code: Ctrl+F and Ctrl+H in the section, Ctrl+Shift+F and Ctrl+Shift+H
  in the whole work, with an adjustable scope (selection, section, chapter, part, choice, work), whole word,
  case sensitivity and regular expressions. Replacing creates a new revision per section and can be
  undone with one click; Bible quotations are excluded by default.
- **Cross-references:** every outline item has a fixed jump mark. „Verweis“ (reference) inserts a reference,
  Ctrl+click jumps there, Alt+← back; in the PDF and EPUB references are links. Baruch finds phrases like
  „im dritten Kapitel“ (in the third chapter) and offers to convert them.
- **Context menu in the text:** „In Glossar aufnehmen“ (add to glossary), „In Register aufnehmen“ (add to
  index), „Im Glossar nachschlagen“ (look up in the glossary) – right click, menu key or Shift+F10.
- **Time keeper:** speaking time per outline item and for the work, target time with a warning – above all for
  sermons. In addition **cue sheets** as an A5 sheet or index cards.

### Assistants

- **Personas** with their own role, their own model and their own connection – „Lektor“ (editor), „Exeget“,
  „Schrift“ (scripture), „Belege“ (evidence), „Nathan“ (auditor), „Hörer“ (listener) or your own. Locally through
  an OpenAI-compatible service or Ollama; external providers only after approval.
- **The editor persona as a companion:** it first says what works and then names the most important points with
  a concrete next step. It reviews a section or a whole chapter; if the text is too long, Baruch
  splits it with overlap – nothing is ever silently shortened.
- **Solutions instead of mere criticism:** every point of criticism comes with a solution („statt … / besser …“,
  i.e. instead of … / better …) and the button „Als Vorschlag übernehmen“ (take over as a suggestion). On
  request the editor persona carries out the solutions in two to three variants that you compare side by side.
- **Chat at the section and threads:** `@lektor` or „Lektor, …“ calls a persona, „RFC“ or `@alle` calls all
  of them (the editor persona summarizes last). Answers appear live, structured, with verified jump marks into
  the text and a note on which revision was reviewed. Posts can be copied, quoted, repeated and
  deleted; your own posts can also be edited.
- **List of findings:** Baruch breaks answers down into individual findings. The human marks them as accepted,
  rejected (with a reason), postponed or done. Rejected findings go back to the personas as a decision
  and are not reported again. The tab „Prüfen → Befunde“ (review → findings) shows an overview of the whole
  work (chapter × persona), the outline shows „k offen“ (k open).
- **Evidence:** a persona searches your own library, SWORD commentaries and, on request, the web and
  names its sources.

### Bible

- **Recognizing and checking references:** „1. Kön 3,16–28“, „Ps 139,23“, lists like „Joh 3,16, 18“,
  „Psalm 139 … Vers 23“; times of day and number words are not mistaken for references.
- **Versifications:** all 18 versifications of the SWORD project with conversion between German and English
  numbering (e.g. Mal 3,19–24 ↔ Mal 4,1–6). Where no rule applies, Baruch says so.
- **SWORD modules:** local Bibles, commentaries and lexicons for reading, searching and quoting – also
  compressed (ZIP, BZIP2, XZ) and encrypted with the publisher's key. The license notice is shown.
- **Checking quotations:** in the editor, Bible quotations are marked as „stimmt“ (matches) or „weicht ab“
  (differs). If a quotation differs only through leftover whitespace in the module, it reads
  „stimmt (Leerraum im Modul)“.

### Ideas and mind map

- **Ideas:** notes with tags, thought, marked text, source and Bible reference, free or attached to
  a section. Quick capture with Alt+E. With one click an idea becomes an outline item,
  a post, a note or an assignment to a persona. Full-text and semantic search.
- **Mind map:** all ideas of a work as a mind map – the work in the middle, parts and chapters as branches,
  ideas as sub-branches of any depth; next to it the cloud for ideas without a place. Tab, Enter, F2, dragging
  and collapsing as in XMind; connections („führt weiter“, „gehört zu“, „Beispiel für“, „widerspricht“) as
  labelled lines, closeness from the semantic search as a suggestion „passt zu Ast …“ (fits branch …). Usable
  without JavaScript through the outline view. Output as XMind, FreeMind, SVG, PDF and Markdown; reading in
  `.xmind` and `.mm` (not yet verified in real XMind and Freeplane).

### Literature and search

- **Author's library:** titles with citation key, quotations and highlights, full-text search (SQLite FTS5)
  across work and literature; literature as a popup with Alt+L.
- **Semantic search (RAG):** local vector index (LanceDB) with embeddings through your own model backend,
  honest status, rebuild without a long lock, weak hits marked; optionally pgvector and
  MultiGPT as further sources. Synchronization with MultiGPT only at the push of a button.

### Typesetting, book and e-book

- **Book typesetting in the settings** („Einstellungen → Werk → Satz“): chapter start (continuous, new page,
  new right-hand page), part pages, page format, margins and gutter, font, paragraph, hyphenation,
  page numbers, running heads, footnotes, widows and orphans – with a preview and an explanation of the
  terms. The work template remains for special cases; Baruch shows contradictions.
- **Typesetting preview and book PDF** with front matter (half title, main title, imprint, dedication). Imprint
  and title pages can be designed as functions in the work template; the print check warns about missing
  mandatory details and about images below 300 dpi.
- **Cover editor:** move all text blocks of front, back and spine freely, with guides
  (bleed, safety margin, spine), contrast check and optional font. The cover for the
  printer calculates the spine width from the page count.
- **Indexes and glossary:** subject, person and Bible reference index (in biblical order) with
  page numbers, ranges, sub-entries and "see also"; glossary with variants and explanations, at the
  end of the book and in the EPUB.
- **EPUB 3:** every chapter and every part page as a file of its own, table of contents, footnotes as a popup or
  endnotes, cross-references, images with alternative text, cover, metadata from the book details. An
  **EPUB preview** shows nine reader profiles side by side (also "Kindle via EPUB"); a structure check
  always runs, EPUBCheck if it is present on the machine.

### Import, export and exchange

- **Import:** Typst, Markdown (also as a ZIP or folder with images), PDF (locally or through MultiGPT), EPUB and
  Word (.docx) – always with a preview of the outline and warnings, created only after confirmation.
- **Importing Word (.docx):** outline, footnotes, quotations, lists, tables and images become a work; tracked
  changes arrive per editor as open suggestions, comments as posts – the decisions are made in Baruch.
- **Export:** Typst file or folder with images, book PDF, cover PDF, EPUB, Word (.docx), cue sheets.
- **Word (.docx) for the publisher:** export of the whole work with styles, real footnotes and embedded
  images; open suggestions of the assistants on request as tracked changes (`/werke/<id>/word`,
  `baruch-server export ID --docx DATEI [--vorschlaege]`). This has not yet been verified in Microsoft Word
  itself.
- **Exchange between instances:** the `.baruch` file (ZIP with XML following its own
  [schema](docs/schema/)) carries a work completely – all revisions, ideas, connections, posts,
  findings, images, cover, typesetting settings – but no keys. Reading in works as a new work or
  as a **merge**: only what is missing is added; nothing is deleted or overwritten, differing
  revisions become further revisions, outline changes become requests.

## Keyboard shortcuts

Keys are named as on an English keyboard; the German user interface and help call Ctrl „Strg“ and Shift
„Umschalt“.

| Key | Effect |
|---|---|
| Alt+E | capture an idea |
| Alt+L | open/close literature |
| Alt+, | settings |
| Ctrl+S | save |
| Ctrl+I · Ctrl+B | emphasize · bold |
| Shift+Enter | line break in the typeset view |
| Ctrl+F · Ctrl+H | find · replace in the section |
| Ctrl+Shift+F · Ctrl+Shift+H | find · replace in the work |
| Enter/F3 · Shift+Enter/Shift+F3 | next · previous hit |
| Ctrl+Shift+1 · Ctrl+Alt+Enter | replace hit · replace all |
| Alt+C · Alt+W · Alt+R | case · whole word · regular expression |
| Ctrl+click · Alt+← | open reference · back |
| Alt+→ | next revision |
| Menu key · Shift+F10 | context menu in the text |
| Ctrl+Enter | send message in the thread |
| Ctrl+mouse wheel · Ctrl+0 | enlarge area · reset |

## Setting up assistants and models

„Einstellungen → Verbindungen“ (settings → connections) creates the model connections (OpenAI-compatible,
Ollama, optionally pgvector and MultiGPT), „Einstellungen → Assistenten“ (settings → assistants) the personas
with role, model, context length, colour and their own prompt. Keys and tokens live in the keyring of the
system (service `baruch`), never in files. An external provider must be approved explicitly. You choose Bibles,
commentaries and lexicons under „Einstellungen → Quellen“ (settings → sources).

## Baruch with Claude Code and other MCP clients

Baruch is an MCP server at `POST /mcp`. With Claude Code:

```sh
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

Through MCP, assistants read works, outline, revisions, ideas, findings and Bible texts, write
suggestions, posts and ideas, file requests and propose glossary terms. Whatever changes or deletes the book
text is not possible through MCP. From other machines MCP works only with a token. All tools are listed in
[docs/MCP.md](docs/MCP.md); the same functions exist as JSON-RPC at `POST /rpc`
([docs/RPC.md](docs/RPC.md)).

## Command line

```sh
baruch-server --version
baruch-server [serve] [--listen ADRESSE:PORT] [--data ORDNER] [--secrets keyring|memory] [--allow-host NAME]…
baruch-server import DATEI.typ [--title "TITEL"]
baruch-server import DATEI.md|DATEI.zip [--title "TITEL"]
baruch-server import DATEI.pdf|DATEI.epub [--uebernehmen] [--bibliothek] [--title "TITEL"]
baruch-server import DATEI.docx [--uebernehmen] [--title "TITEL"]
baruch-server import DATEI.baruch [--abgleichen] [--vorschau]
baruch-server export ID|SLUG [--out ORDNER | --pdf DATEI | --epub DATEI | --baruch DATEI]
baruch-server export ID|SLUG --docx DATEI [--vorschlaege]
baruch-server export ID|SLUG --stichwort DATEI [--format a5|karten] [--schrift PT]
```

Export never overwrites an existing file. When importing PDF, EPUB and Word, the command line without
`--uebernehmen` only shows the recognized outline (for Word also editors and notes). `baruch-server --help`
names all details (in German: `ADRESSE` = address, `ORDNER` = folder, `DATEI` = file).

## Data, backup and security

- **Data:** `~/.local/share/baruch` (macOS `~/Library/Application Support/baruch`, Windows
  `%LOCALAPPDATA%\baruch`). There you find `baruch.sqlite` as the source of all data and, under
  `works/<slug>/`, the Git mirror of every work. The data never lives in the installation directory; no package
  creates anything there, and removing Baruch deletes no data.
- **Backup:** „Einstellungen → Daten → Sicherung jetzt“ (settings → data → back up now). Before installing a
  new version of Baruch a copy of the data folder is recommended; migrations run automatically on first start.
- **Security:** the server answers only under `127.0.0.1`, `[::1]` or `localhost` (and under names that
  were allowed at start with `--allow-host`, see [docs/Betrieb.md](docs/Betrieb.md)). It accepts writing calls
  only from its own page. Files that are read in are checked against limits for size, depth and
  unpacked bytes. Details are in the help under
  [Austausch, Sicherung und Sicherheit](https://baruch.bernau.family/hilfe/austausch-sicherung/) and, for
  operators, under [Betrieb](https://baruch.bernau.family/betrieb/).

## Structure

A Rust server ([axum](https://github.com/tokio-rs/axum) and
[minijinja](https://github.com/mitsuhiko/minijinja)) serves pages and fragments. The widgets are written in
TypeScript (`crates/baruch-server/web/src`) and are compiled by `build.rs` during the build.

| Crate | Task |
|---|---|
| `baruch-core` | works, chapter tree, revisions per section, personas |
| `baruch-store` | data storage in SQLite (migrations, requests, findings, glossary …) |
| `baruch-gitmirror` | Git mirror of the Typst folders (pure Rust through gix) |
| `baruch-typst` | work template and outline, schemes, typesetting settings, cover, indexes |
| `baruch-render` | Typst typesetting for preview and PDF |
| `baruch-wysiwyg` | typeset-view editor: Typst into editable blocks and back |
| `baruch-epub` | creating and checking EPUB 3 |
| `baruch-exchange` | writing and safely reading the exchange format `.baruch` (XML in a ZIP container) |
| `baruch-library` | author's library and full-text search (FTS5) |
| `baruch-rag`, `baruch-lancedb` | interface of the search paths and local vector index |
| `baruch-llm` | OpenAI-compatible model client |
| `baruch-scripture` | recognizing Bible references, versifications, checking the wording |
| `baruch-sword` | reading local SWORD modules (also compressed and encrypted) |
| `baruch-server` | server, user interface, RPC and MCP |

## Documentation

- [Help on the website](https://baruch.bernau.family/hilfe/) (German): usage step by step, with images
  (source: `site/content/de/hilfe/`); [Betrieb](https://baruch.bernau.family/betrieb/) for operators
  (`site/content/de/betrieb.md`)
- [docs/Bedienung.md](docs/Bedienung.md): entry point for developers – "where is what" and technical notes on
  the help
- [docs/Verteilung.md](docs/Verteilung.md): packages, installation, APT repository, release, delivery through
  GitHub Pages
- [docs/Betrieb.md](docs/Betrieb.md): data folder, backup, permanent operation, operation behind a reverse proxy
- [CHANGELOG.md](CHANGELOG.md): changes per version
- [docs/Architektur.md](docs/Architektur.md): structure and decisions
- [docs/RPC.md](docs/RPC.md): JSON-RPC interface
- [docs/MCP.md](docs/MCP.md): MCP tools
- [docs/WYSIWYG.md](docs/WYSIWYG.md): typeset-view editor
- [docs/Austauschformat.md](docs/Austauschformat.md): exchange format `.baruch`, schema under
  [docs/schema/](docs/schema/)
- [docs/Plan.md](docs/Plan.md), [docs/Fahrplan.md](docs/Fahrplan.md), [docs/Backlog.md](docs/Backlog.md):
  plan, rounds and open points

## Contributing

The primary language is English for code, comments, commits, developer docs, MCP and RPC; the user interface is
German (German and English through catalogs from Round 39). Work happens on the branch
`development`; `main` follows after approval. Every change to the user interface, RPC or MCP updates the docs
in the same commit. The remaining rules are in [AGENTS.md](AGENTS.md).

Tests:

```sh
nice -n 10 ionice -c3 cargo test --workspace -j 4
```

The tests need no network and no credentials; models, MultiGPT and web pages are replaced by local stand-ins.
Bible tests only run with free or invented test modules, test texts are freely invented.

Before a commit also:

```sh
nice -n 10 ionice -c3 cargo clippy --workspace --all-targets -j 4 -- -D warnings
cargo deny check licenses sources bans
scripts/ci-selftest.sh
```

Clippy warnings are errors. An `#[allow(clippy::…)]` needs a reason as a comment.
`scripts/ci-selftest.sh` checks, without network, the helper scripts for release and website and the rules of
the workflows (actions only with commit SHA, minimal permissions, no secrets in command lines).

The CI runs on every push to `main` (`.github/workflows/ci.yml`): tests, clippy, `cargo deny check`,
typesetting an exported work with the program itself as Typst command line (`baruch-typst`, no
download) and the build of the website. A tag `v<Version>`
builds the packages, continues the APT repository and delivers website and repository through GitHub Pages
(`.github/workflows/release.yml`). Procedure, one-time setup and state of verification:
[docs/Verteilung.md](docs/Verteilung.md). The source of the website is under [site/](site/README.md).

## Status and limits

- **Version 0.9.0** – the first with packages. All functions described above are finished; what has happened
  since the beginning is in [CHANGELOG.md](CHANGELOG.md), what is open in
  [docs/Backlog.md](docs/Backlog.md), the timeline in [docs/Fahrplan.md](docs/Fahrplan.md).
- **Packages:** the `.deb` and the AppImage have been built and started on Debian 13. The packages for macOS
  and Windows are only built in the CI, are unsigned and have not yet been tried on a real device – the
  keyring there (macOS Keychain, Windows Credential Manager) is built as well, but unverified.
  For Linux there is only amd64/x86_64; the `.deb` needs glibc 2.39 or newer (Debian 13, Ubuntu 24.04).
- **Updating:** only through the APT repository or by hand. Baruch never checks for new versions itself.
- **Operation on a network:** on its own Baruch answers only on your own machine and has no login without
  accounts. It belongs on a network only behind a reverse proxy with TLS and with accounts set up
  („Einstellungen → Konten“, settings → accounts) or access protection in the proxy. Baruch answers under a host
  name only if it was allowed at start (`--allow-host NAME`); data folder, backup, permanent operation and
  operation behind a reverse proxy with TLS are described in [docs/Betrieb.md](docs/Betrieb.md).
- **Website:** <https://baruch.bernau.family/> – source under [site/](site/README.md), delivered only through
  the CI. No workflow has run so far (see [docs/Verteilung.md](docs/Verteilung.md), section
  "State of verification").
- **Interfaces** (RPC, MCP, exchange format) may still change until version 1.0.
- Baruch does not ship Bible texts, commentaries or literature; it only reads what is on your own
  machine and shows its license.
- Baruch checks legal details in the imprint for completeness, but does not replace legal advice.

## License

GNU Affero General Public License v3.0 or later (`AGPL-3.0-or-later`), see [LICENSE](LICENSE); in the
running program under „Baruch ▾ → Über Baruch“ (About Baruch). The source code of every version is at
<https://github.com/Beerlesklopfer/baruch> at the tag `v<Version>`. The packages contain third-party libraries;
their licenses are in `THIRD-PARTY-LICENSES.txt` in the respective package.
© Jörg Bernau
