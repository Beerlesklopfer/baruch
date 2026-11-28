# Website

Source of the website `https://baruch.bernau.family/`, built with [Hugo](https://gohugo.io). This file describes
only **building locally and the preview**. The site is deployed exclusively by the CI (GitHub Pages,
`.github/workflows/`, described in `docs/Verteilung.md`) – nobody uploads anything by hand.

## Building

Hugo from 0.158 is needed (the key `locale` and the language key `label`), the plain edition (not "extended"): the
theme uses no SCSS and no image processing.

```sh
python3 -I site/tools/docs_pages.py         # developer documentation: docs/*.md to site/generated/docs/, docs/de/ to site/generated-de/docs/
hugo --minify --panicOnWarning -s site      # from the main folder; output in site/public/
sh scripts/ci-site.sh                       # both steps plus every check, as the CI does
```

Without the first step Hugo builds the site without the sections `/en/docs/` and `/docs/` (and the menu entries
"Documentation" and "Dokumentation" point nowhere); `scripts/ci-site.sh` runs it first and stops if the pages are missing afterwards.

`--panicOnWarning` turns every warning into an error; the CI checks the same way. `site/public/`,
`site/resources/`, `site/generated/`, `site/generated-de/` and `.hugo_build.lock` are in `.gitignore` and are never checked in.

After the build `scripts/site-check.py` checks the output (Python standard library only, no network):

```sh
python3 scripts/site-check.py               # over site/public; exit 1 with the list of findings
python3 scripts/site-check.py --selbsttest  # checks the script on an invented mini tree
```

It reports dead internal links and anchors (`/apt/` excepted), images without alt text, JavaScript, third-party
addresses for style, font or image, links to `docs/Bedienung.md` or `docs/Betrieb.md` on GitHub, links to
github.com in the text of the help and a missing redirect from `/bedienung/` to `/hilfe/`. `scripts/ci-site.sh`
calls it after the Hugo build; so the CI fails on a dead link, in either language.

## Preview

```sh
hugo server -s site --bind 127.0.0.1 --port 1313     # http://127.0.0.1:1313/ and http://127.0.0.1:1313/en/
```

The preview reloads the page on changes and inserts a script of its own for that (only in the preview; the built
site contains no JavaScript).

## Languages

The site is German and English (user decision 2026-11-27, Round 39b).

- **German** is the default language and has no subfolder in the address: `content/de/` → `/`, `/hilfe/…`. The
  German addresses have not changed since Round 24.
- **English** lives in `content/en/` → `/en/`, `/en/hilfe/…`. Every English page has **exactly the same relative
  file name** as its German original (Hugo pairs translations by path), the same `{#anchor}` ids, the same
  `weight` and the same non-text front matter (`inhalt`, `platzhalter`, `ohne_bild`, …); `title`, `description`,
  `claim` and `lead` are English. English pages carry **no `aliases`** (the old address `/bedienung/` stays
  German only).
- **Links in English pages** go to `/en/…/` with the German anchors (`/en/installation/#bibeln-einrichten`); static
  files (`/apt/…`) and external links stay as they are.
- **Interface words** quoted in the English help are the values of `crates/baruch-server/locales/en/*.ftl` for
  the key whose German value the German page quotes. English prose uses straight quotation marks or bold for them:
  the test `round39b_hilfe_en` rejects „ and “ (and German letters) in English prose; inline code is exempt.
- **Pictures:** German `static/bilder/<file>` and `static/bilder/hilfe/<name>.png`, English
  `static/bilder/en/<file>` and `static/bilder/en/hilfe/<name>.png`. Shortcode calls in English pages use the same
  `name`/`datei` with an English `alt` and `titel`; the partial `bildpfad.html` picks the folder of the language.
- **Theme strings** (menu labels aside) come from `themes/baruch/i18n/de.toml` and `en.toml` (`{{ T "key" }}`); the
  German values are the wording the layouts carried before, verbatim. Menus and the English site description are
  per language in `hugo.toml` (`[languages.<lang>.menus]`, `[languages.en.params]`).
- **Language switch:** a plain link in the header (`_partials/kopf.html`) to the translation of the page
  (`.Translations`), or to the home page of the other language if the page has no translation. `baseof.html`
  announces every translation as `<link rel="alternate" hreflang>` (German also as `x-default`); `<html lang>` is
  the locale of the language.

## Structure

| Path | Content |
|---|---|
| `hugo.toml` | address, languages with their menus, and the values `version`, `repo`, `aptFingerprint`, `description` under the top-level `[params]` (read by `scripts/ci-site.sh` and `tests/round24_abnahme.rs`) |
| `content/de/` | German content (default language, addresses without `/de/`): the seven fixed pages: start (`_index.md`), Funktionen, Installation, Betrieb, MCP und Assistenten, Lizenz, Impressum |
| `content/de/hilfe/` | the section **Hilfe** (help): start page (`_index.md`, takes over the old address `/bedienung/` with `aliases`) and one page per topic, ordered by `weight` |
| `content/en/`, `content/en/hilfe/` | the English translations under the same file names, addresses under `/en/` |
| `data/hilfebilder.toml` | the list of all pictures of the help (name, page, what the picture shows); only names from there are valid in the shortcode `hilfebild` |
| `themes/baruch/` | the theme, completely in the repository: layouts (`layouts/`), theme strings (`i18n/`), one stylesheet, the icon |
| `static/CNAME` | the host name for GitHub Pages |
| `static/bilder/` | screenshots for the home page and Features (1400×860, plus `arbeitsplatz-laptop.png` in 1366×768); English ones under `static/bilder/en/`; the site builds without them |
| `static/bilder/hilfe/` | pictures of the help: `<name>.png`, next to it optionally `<name>-dunkel.png` for the dark system; English ones under `static/bilder/en/hilfe/` |
| `tools/` | invented work and script for the screenshots, generator of the developer documentation (`tools/README.md`) |
| `generated/` | output of `tools/docs_pages.py` (`generated/docs/`), mounted as English content: `/en/docs/`; not checked in, rebuilt on every build |
| `generated-de/` | the same generator from `docs/de/` (`generated-de/docs/`), mounted as German content: `/docs/`; not checked in, rebuilt on every build |

## Source for usage and operation

Since Round 37 **the website is the source**: `content/de/hilfe/**` (and its English translation) is the only
description for users, `content/de/betrieb.md` the only one for operators. `docs/Bedienung.md` and `docs/Betrieb.md`
in the repository are only the entry point: reference to the website, "where is what", state of checking, open
points and technical remarks (addresses, cookie names, limits of the checks) – **no usage steps in either file**.

- "Update the usage docs" means: change the help page in the same commit – the German page and its English
  translation; "update operation": `content/de/betrieb.md` and `content/en/betrieb.md`.
- The **first setup** is described only once: Bibles in `installation.md` (`#bibeln-einrichten`), assistants,
  MultiGPT and AI programs in `mcp.md` (`#assistenten-einrichten`, `#multigpt-einrichten`, `#mcp-clients`). The help
  describes the pages and fields and links there.
- Pages link to each other only with ordinary links from `/` to pages or to fixed anchors
  (`[Text](/hilfe/konten/#lektoren)`, English `[Text](/en/hilfe/konten/#lektoren)`), not with `relref`. Fixed
  anchors stay, even when a heading changes.
- The shortcode `doku` (link into the repository) stays for files that really exist only there: `LICENSE`,
  `README.md`, `docs/Verteilung.md`, `docs/RPC.md`, `docs/MCP.md`, `docs/Architektur.md`.

## Writing

- **Everyday language.** Replace technical terms or explain them in a half sentence where they first appear
  (German: Fassung instead of Revision, Assistent instead of Persona, Bedeutungssuche instead of RAG; English:
  revision, assistant, semantic search; MCP, reverse proxy, token, SWORD, Typst, Git mirror, cookie). No round
  numbers, no file names of the source code, no routes – except addresses one really types
  (`http://127.0.0.1:8765/`).
- **Keys for both worlds:** `{{< taste "Strg+S" "⌘S" >}}` gives "Strg+S (Mac: ⌘S)" (English pages:
  `{{< taste "Ctrl+S" "⌘S" >}}`). Prove from the code what applies on the Mac; otherwise say honestly "unchecked on
  the Mac".
- **No raw HTML** in Markdown (`unsafe = false`), no HTML comments either – they break the build.
- **Every heading carries a fixed anchor** in lower case and ASCII: `## Sicherung {#sicherung}`; the English page
  keeps the German anchor (`## Backup {#sicherung}`).
- **Separate the checked from the assumed:** what nobody has tried (macOS, Windows, a real proxy) stays marked as
  "not checked".
- **No real names, works or data.** The sample work of the pictures is invented.
- Head of a page: `title`, `description` (one sentence; appears as the lead and in the list of the help), in the
  help `weight`; `inhalt: true` puts a table of contents under the lead on a fixed page (the help pages always have
  it).

## Layouts

| Layout | for |
|---|---|
| `layouts/baseof.html`, `_partials/kopf.html`, `_partials/fuss.html` | skeleton (with `hreflang` alternates), header with main menu and language switch, footer. In the main menu an entry is also marked on the subpages of its section (`aria-current="true"`, on the page itself `"page"`) |
| `layouts/home.html` | home page (picture through `_partials/bildpfad.html`) |
| `layouts/page.html` | fixed pages |
| `layouts/section.html` | start page of the help: list of the pages by `weight` with `description` |
| `layouts/docs/section.html`, `layouts/docs/page.html` | developer documentation (`/en/docs/`): index with the pages by `weight`; a page with the navigation of the section, its source file, table of contents (closed above 40 entries) and "previous/next page" |
| `layouts/hilfe/page.html` | pages of the help: navigation of the section (current page marked), table of contents from the headings of levels 2 and 3 (`markup.tableOfContents` in `hugo.toml`), "previous/next page" |
| `layouts/404.html` | page not found (one per language: `/404.html`, `/en/404.html`) |

There is deliberately no **search**: it would only work with JavaScript or through a third-party address. The
stylesheet (`static/css/baruch.css`) follows the light or dark system with `prefers-color-scheme`, lets tables and
the navigation of the help scroll sideways at phone width and hides the navigation in print.

## Rules

- **No third-party addresses:** no scripts, fonts, styles or pictures from other servers, no counters, no cookies.
  Fonts are system fonts. Links go only to the site itself, the repository and, in the running text, what the text
  names.
- **Without JavaScript:** the site contains none.
- **Identifiers in the layouts** (classes, i18n keys, partial names) are German, as before (it is content, not a
  program); comments in the layouts are English.
- **Version:** `params.version` in `hugo.toml` determines all file names and links of the Installation page
  (shortcodes `download`, `datei`, `release`). The only source of the version is `Cargo.toml`; whoever changes it
  there updates `hugo.toml`. Only the user sets version numbers.
- **Fingerprint:** `params.aptFingerprint` stays empty until the user enters the fingerprint of the archive key.
  Empty, the Installation page shows a visible placeholder notice. Do not fetch it from the net, do not guess.
- **Legal notice:** the details in `content/de/impressum.md` (and its English translation) are entered only by the
  operator. A page with `platzhalter: true` in its head is visibly marked as a placeholder.
- **Warnings during the build:** `scripts/ci-site.sh` warns (without aborting) as long as the fingerprint is empty
  or a page carries `platzhalter: true`. The fingerprint also stands in `README.md` and `docs/Verteilung.md`;
  `scripts/ci-fingerprint.sh` checks that all three places agree, and a signed release fails if it is missing or
  does not fit the key.
- **Language keys in `hugo.toml`:** `locale` per language (the older `languageCode` is deprecated since Hugo 0.158
  and breaks the build with `--panicOnWarning`), and `label` for the language name shown in the switch (the older
  `languageName` likewise).

## Developer documentation (generated, Rounds 39c and 39d)

The developer documentation in `docs/` is published in English under `/en/docs/`, its German translation in
`docs/de/` (Round 39d) under `/docs/` (German is the default language, so no language folder), with the same slugs,
so that the language switch of every page resolves. There is **no hand-kept copy**:
`tools/docs_pages.py` (Python standard library only, run with `python3 -I`) generates one page per source on every
build, `scripts/ci-site.sh` runs it before Hugo, and `.github/workflows/pages.yml` needs no step of its own.

| Source | Address | Weight |
|---|---|---|
| (index, generated) | `/en/docs/` | – |
| `docs/Bedienung.md` | `/en/docs/usage/` | 10 |
| `docs/Architektur.md` | `/en/docs/architecture/` | 20 |
| `docs/RPC.md` | `/en/docs/rpc/` | 30 |
| `docs/MCP.md` | `/en/docs/mcp/` | 40 |
| `docs/Austauschformat.md` | `/en/docs/exchange-format/` | 50 |
| `docs/WYSIWYG.md` | `/en/docs/wysiwyg/` | 60 |
| `docs/Betrieb.md` | `/en/docs/operation/` | 70 |
| `docs/Verteilung.md` | `/en/docs/distribution/` | 80 |
| `docs/Plan.md` | `/en/docs/plan/` | 90 |

- **Output:** `site/generated/docs/` – outside `content/`, so the rule "every English page has a German twin"
  (`tests/round39b_hilfe_en.rs`) and the fixed list of help pages (`tests/round37_hilfe.rs`) do not see it.
  `hugo.toml` mounts `generated/` as content of the language `en` only (`[module]`, `sites.matrix`); because a
  configured content mount replaces the defaults, `content/de` and `content/en` are mounted there too (the languages
  have no `contentDir` any more).
- **Not checked in** (`.gitignore`: `site/generated/`): the pages are rebuilt from `docs/` on every build, so they
  cannot fall behind; a committed copy would change with every documentation commit and need a currency check of
  its own.
- **Title and weight:** `title` from the first heading of the source (removed from the body; the layout prints it),
  `weight` and the one-sentence `description` from the table `DOCS` in the generator, `quelle` = source path.
- **Anchors:** every heading gets an explicit id computed like GitHub (`## Delivery through GitHub Pages` →
  `#delivery-through-github-pages`, duplicates `-1`, `-2` …), so an anchor that works in the repository works on
  the site.
- **Links:** to one of the nine sources → `/en/docs/<slug>/#anchor` (the anchor must exist); to a page of the website
  source (`site/content/de|en/…`) → that page on the site; to any other file of the repository →
  `<params.repo>/blob/main/<path>` (folders `tree/main`). A relative link to a missing file or anchor stops the
  generator (exit 1, nothing written) – it is the only check for links that leave the site.
- **Escaping:** raw HTML in the running text (`<Titel>`) becomes `&lt;Titel>` (Hugo would drop it, `unsafe = false`);
  Hugo shortcode syntax is escaped everywhere, also in code.
- **One page for the architecture:** measured with Hugo 0.168 on this machine, the whole site builds in about 55–60 ms
  with `Architektur.md` as one page (about 800 KB of HTML) and in about the same time split into 48 parts; the
  site without the documentation takes about 30 ms. The site has deliberately no search, so one page keeps the
  browser's find (Ctrl+F) working over the whole architecture, and every GitHub anchor maps 1:1. The table of
  contents of a page with more than 40 entries starts closed.
- **Checks:** `scripts/site-check.py` checks links and anchors of the section like every page, plus: no link into
  the generator output (`site/generated`, `site/generated-de`, `/generated/`, `/generated-de/`) and, under `/en/docs/`
  and `/docs/`, no link to one of the nine sources on GitHub; the language switch of a page under `/docs/` must lead to
  the same page under `/en/docs/`. `python3 -I site/tools/docs_pages.py --selbsttest` checks the generator; `scripts/ci-selftest.sh` runs
  it and checks that `ci-site.sh` calls it.
- **Menu:** English "Documentation" and German "Dokumentation", both `pageRef = "/docs"` and weight 60. Theme strings
  `docs_*` in `i18n/` (both languages).
- **German section (Round 39d):** source `docs/de/<Name>.md`; the architecture exists only as the four parts
  `docs/de/Architektur.part<N>.md` (no joined file in git, so that merges stay conflict free), which the generator
  joins in numeric order with one blank line. Line 1 of every file is the header comment
  `<!-- translation-of: docs/<Name>.md source-commit: <hash> -->` (parts add ` part: <N>`); the generator drops it.
  **The English file under `docs/` is the source of truth.** Every heading of a German file carries an explicit id
  `{#<English anchor>}` (`python3 -I scripts/docs-anchors.py FILE` lists the anchors, `--check GERMAN ENGLISH` verifies
  a translation); the generator fails (exit 1, nothing written) if a German heading has no explicit id, if the
  sequence of levels and ids differs from the English file, if a link anchor is not an id of its target, if a link
  leaves the German section for the English original, or if a German file or part is missing. Relative links resolve
  against `docs/de/`. Output `site/generated-de/docs/`, mounted as content of the language `de`, not checked in
  (`.gitignore`). `python3 -I scripts/check-doc-lag.py` (informational, called by `ci-site.sh`, exit 1 only with
  `--strict`) names every German file whose header commit is older than the last change of its English source.
  `docs/de/` is the allowed-German area of `scripts/check-doc-language.py` and of `tests/round38_doku.rs`.
- **Writing in `docs/`:** ordinary Markdown as on GitHub. Avoid a line inside a paragraph that starts with `>`,
  `#`, `- ` or `1. ` (it starts a new block on GitHub and on the site alike).

## Shortcodes

| Call | Result |
|---|---|
| `{{< bild datei="satz.png" alt="…" titel="…" >}}` | picture from `static/bilder/` (home page, Features); if the file is missing, nothing |
| `{{< hilfebild name="gliederung-seite" alt="…" titel="…" breite="640" dunkel="nein" >}}` | picture of the help from `static/bilder/hilfe/<name>.png`, with a dark system `<name>-dunkel.png` if present. `name` must be in `data/hilfebilder.toml`, `alt` is mandatory (otherwise the build aborts); `titel` is the caption, `breite` the largest display width in pixels. If the file is missing, nothing. The shortcode is frozen: changes only by the integrator |
| `{{< taste "Strg+S" "⌘S" >}}` | key "Strg+S (Mac: ⌘S)"; without a second value only the first key |
| `{{< download "Baruch-VERSION-x64.msi" >}}` | link to the file of the release (`VERSION` is replaced) |
| `{{< datei "Baruch-VERSION-x64.msi" >}}` | only the file name, for commands |
| `{{< release >}}` | link to the release page of the version; English pages pass the text: `{{< release "release page" >}}` |
| `{{< doku "docs/MCP.md" "MCP" >}}` | link to a file in the repository (branch `main`); not for usage and operation, see above |
| `{{< fingerabdruck >}}` | fingerprint of the archive key or the placeholder notice |
