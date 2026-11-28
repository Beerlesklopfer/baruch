#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Generates the developer documentation pages of the website from docs/*.md (Round 39c, task docs_site).

    python3 -I site/tools/docs_pages.py [--site ORDNER] [--repo ORDNER] [--lang en|de|all]
    python3 -I site/tools/docs_pages.py --list [--lang en|de|all]
    python3 -I site/tools/docs_pages.py --selbsttest

Python standard library only, no network. The nine sources in DOCS (docs/Bedienung.md … docs/Plan.md) are the only
input; there is no hand-kept copy. The output goes to <site>/generated/docs/ – outside site/content, so that the help
tests (every English page has a German twin) do not see it. site/hugo.toml mounts site/generated as content of the
language en only, which gives the fixed addresses /en/docs/ (index) and /en/docs/<slug>/. The output is gitignored
(.gitignore `site/generated/`): scripts/ci-site.sh runs this script before every Hugo build, so the pages can never
fall behind the sources.

What the generator does with each source:

 1. Front matter: `title` from the first heading (level 1, removed from the body because the layout prints it),
    `weight` and `description` from DOCS, `quelle` = the source path (the layout names it).
 2. Every heading gets an explicit id `{#…}` computed like GitHub does (lower case, punctuation removed, spaces to
    hyphens, duplicates numbered -1, -2 …). So every anchor that works on GitHub works on the site, independent of
    Hugo's own id rules.
 3. Links (inline and reference definitions, outside code): to one of the nine sources → /en/docs/<slug>/#anchor (the
    anchor must exist in that source); to a page of the website source (site/content/de|en/…) → the address of that
    page on the site; to any other file or folder of the repository → <params.repo>/blob/main/<path> (folders:
    tree/main). A relative link to something that does not exist, an anchor that does not exist or a path outside
    the repository stops the run with exit 1 and the list of findings.
 4. Raw HTML in the running text (`<Titel>`, `<id>`) would be dropped by Hugo (markup.goldmark.renderer.unsafe =
    false); outside code spans and code blocks the `<` is written as `&lt;`, so the text reads as in the source.
    Autolinks (`<https://…>`) stay. HTML comments outside code (`<!-- End R25 … -->`, markers for the editors) are
    removed, as GitHub hides them too.
 5. Hugo shortcode syntax (`{{< … >}}`, `{{% … %}}`) is escaped everywhere, also in code, as `{{</* … */>}}`; Hugo
    then prints it literally instead of calling the shortcode.

German (Round 39d): the same nine documents exist in German under docs/de/ (docs/de/<Name>.md; the architecture as
docs/de/Architektur.part<N>.md, joined in numeric order with one blank line, the header comment of every part is
dropped). They give the section /docs/ of the German site (German is the default language, so no subfolder), written
to <site>/generated-de/docs/ with the same slugs as English, so that the language switch resolves. The English files
are the source of truth: every heading of a German document carries an explicit id `{#<English anchor>}` (kept,
removed from the title text), and the sequence of heading levels and ids must equal that of the English source;
relative links resolve against docs/de/ (siblings = the other German documents, `../schema/…` and `../Backlog.md`
= untranslated files in docs/); every link anchor must be one of the ids of its target. A missing German file or
part, a heading without id, a different sequence, a dead anchor or a link into the English original is a finding
(exit 1, nothing written). --lang selects the languages (default all).

Exit 0 on success, 1 with findings, 2 on a wrong call. --selbsttest runs the generator on an invented mini repository
and checks every rule above, each assertion with its reason.
"""

import argparse
import json
import os
import posixpath
import re
import shutil
import sys
import tempfile

# (source file in docs/, slug of the address /en/docs/<slug>/, weight, description). Order = weight.
DOCS = (
    ("Bedienung.md", "usage", 10,
     "Developer entry point for usage: where the user help lives, what moved where, technical remarks and the state "
     "of checking."),
    ("Architektur.md", "architecture", 20,
     "Structure of the code: crates, store, server, mirror, assistants and the design notes of every round."),
    ("RPC.md", "rpc", 30,
     "The JSON-RPC interface POST /rpc: every method with its parameters, results and errors."),
    ("MCP.md", "mcp", 40,
     "The MCP server POST /mcp for AI assistants: every tool, tokens, sessions and limits."),
    ("Austauschformat.md", "exchange-format", 50,
     "The exchange format that moves a work with all its revisions from one Baruch to another."),
    ("WYSIWYG.md", "wysiwyg", 60,
     "The typeset view editor: blocks, reading and writing Typst, proposals in place."),
    ("Betrieb.md", "operation", 70,
     "Developer entry point for operation: where the description for operators lives and technical remarks."),
    ("Verteilung.md", "distribution", 80,
     "Packages, releases, the website and the APT repository, and how the CI delivers them."),
    ("Plan.md", "plan", 90,
     "The plan of Baruch: context, decisions of the user and the stages of the work."),
)
# Address of the section and folder of the output below the site.
URL_BASE = "/en/docs/"
OUTPUT = os.path.join("generated", "docs")
URL_BASE_DE = "/docs/"
OUTPUT_DE = os.path.join("generated-de", "docs")
# German lead texts per slug (translations of the descriptions in DOCS).
DESCRIPTIONS_DE = {
    "usage": "Einstieg für Entwickler in die Bedienung: wo die Hilfe für Anwender liegt, was wohin gezogen ist, "
             "technische Anmerkungen und der Stand der Prüfung.",
    "architecture": "Aufbau des Codes: Crates, Speicher, Server, Spiegel, Assistenten und die Entwurfsnotizen jeder "
                    "Runde.",
    "rpc": "Die JSON-RPC-Schnittstelle POST /rpc: jede Methode mit Parametern, Ergebnissen und Fehlern.",
    "mcp": "Der MCP-Server POST /mcp für KI-Assistenten: jedes Werkzeug, Tokens, Sitzungen und Grenzen.",
    "exchange-format": "Das Austauschformat, das ein Werk mit allen Fassungen von einem Baruch in ein anderes bringt.",
    "wysiwyg": "Der Editor mit gesetzter Ansicht: Blöcke, Typst lesen und schreiben, Vorschläge an Ort und Stelle.",
    "operation": "Einstieg für Entwickler in den Betrieb: wo die Beschreibung für Betreiber liegt, und technische "
                 "Anmerkungen.",
    "distribution": "Pakete, Veröffentlichungen, die Webseite und das APT-Repository, und wie die CI sie ausliefert.",
    "plan": "Der Plan von Baruch: Zusammenhang, Entscheidungen des Nutzers und die Stufen der Arbeit.",
}
INDEX_TITLE_DE = "Dokumentation"
INDEX_DESCRIPTION_DE = ("Entwickler- und Betreiberdokumentation von Baruch auf Deutsch, bei jedem Bau der Webseite "
                        "aus den Dateien in docs/de/ des Repositorys erzeugt.")
INDEX_TEXT_DE = """Diese Seiten sind die Entwicklerdokumentation von Baruch in deutscher Übersetzung. Sie werden bei
jedem Bau der Webseite aus den Markdown-Dateien in `docs/de/` des Repositorys erzeugt; Änderungen gehören in diese
Dateien, nie hierher. Maßgeblich ist das englische Original in `docs/` (dieselben Seiten unter
[/en/docs/](/en/docs/)); die Übersetzung folgt ihm und kann zurückliegen (`scripts/check-doc-lag.py`).

Die Beschreibung für Anwender ist die [Hilfe](/hilfe/), die für Betreiber die Seite [Betrieb](/betrieb/).
"""
HEADER_COMMENT = re.compile(r"\A<!--.*?-->[ \t]*\n?", re.S)
EXPLICIT_ID = re.compile(r"[ \t]*\{#([^{}\s]+)\}[ \t]*$")
LANGUAGES = ("en", "de")
INDEX_TITLE = "Documentation"
INDEX_DESCRIPTION = ("Developer and operator documentation of Baruch, generated from the files in docs/ of the "
                     "repository on every build of the website.")
INDEX_TEXT = """These pages are the developer documentation of Baruch in English. They are generated from the
Markdown files in `docs/` of the repository on every build of the website, so they always show the state of the
branch `main`; changes are made in those files, never here.

The description for users is the [help](/en/hilfe/), the one for operators the page [Operation](/en/betrieb/).
"""

FENCE = re.compile(r"^( {0,3})(`{3,}|~{3,})")
ATX = re.compile(r"^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$")
# A code span ends at its paragraph: it never crosses a blank line or a line that starts a block of its own (block
# quote, heading, list item, fence), as in CommonMark – otherwise a `<` after such a line would count as code here
# but as raw HTML for Hugo.
CODE_SPAN = re.compile(r"(?<!`)(`+)(?!`)((?:(?!\n[ \t]*\n)(?!\n {0,5}(?:>|#{1,6}[ \t]|[-*+][ \t]|\d{1,9}[.)][ \t]|`{3}|~{3}))"
                       r".)+?)(?<!`)\1(?!`)", re.S)
# [text](target "title") and ![alt](target); the text may contain code spans (placeholders by then) and one level
# of brackets.
INLINE_LINK = re.compile(r"(!?\[(?:[^\[\]]|\[[^\[\]]*\])*\]\()[ \t]*(<[^<>\n]*>|[^\s()<>]*(?:\([^\s()]*\)[^\s()<>]*)*)"
                         r"([ \t]+(?:\"[^\"\n]*\"|'[^'\n]*'))?[ \t]*\)")
REF_DEF = re.compile(r"^( {0,3}\[[^\]\n]+\]:[ \t]*)(<[^<>\n]*>|\S+)", re.M)
AUTOLINK = re.compile(r"<[A-Za-z][A-Za-z0-9+.-]{1,31}:[^<>\s]*>|<[^\s<>@]+@[^\s<>]+\.[^\s<>]+>")
HTML_COMMENT = re.compile(r"<!--.*?-->", re.S)
RAW_TAG = re.compile(r"<(?=[A-Za-z/!?])")
SHORTCODE = re.compile(r"\{\{([<%])(?!/\*)(.*?)([>%])\}\}", re.S)
PLACEHOLDER = "\x00{}\x00"


class Findings(Exception):
    pass


def read_text(path):
    with open(path, encoding="utf-8") as handle:
        return handle.read()


def write_text(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def repo_url(site):
    """params.repo of hugo.toml (top-level [params], a `repo = "…"` line; scripts/ci-site.sh reads it the same way)."""
    inside = False
    for line in read_text(os.path.join(site, "hugo.toml")).splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            inside = stripped == "[params]"
            continue
        match = re.match(r'repo\s*=\s*"([^"]+)"', stripped)
        if inside and match:
            return match.group(1).rstrip("/")
    raise Findings([("site/hugo.toml", "[params] repo is missing (needed for links into the repository)")])


def split_fences(text):
    """List of (is_code, chunk) for fenced code blocks and the text between them. Unclosed fences run to the end, as
    in CommonMark."""
    parts = []
    lines = text.splitlines(keepends=True)
    current, fence = [], None
    for line in lines:
        if fence is None:
            match = FENCE.match(line)
            if match:
                if current:
                    parts.append((False, "".join(current)))
                current, fence = [line], match.group(2)
                continue
            current.append(line)
        else:
            current.append(line)
            match = FENCE.match(line)
            # A closing fence: the same character, at least as long, nothing else on the line, at most 3 spaces in.
            if (match and match.group(2)[0] == fence[0] and len(match.group(2)) >= len(fence)
                    and line.strip() == match.group(2)):
                parts.append((True, "".join(current)))
                current, fence = [], None
    if current:
        parts.append((fence is not None, "".join(current)))
    return parts


def plain_heading(text):
    """Text of a heading as GitHub sees it for the anchor: code spans keep their content, links their text."""
    text = re.sub(r"!?\[((?:[^\[\]]|\[[^\[\]]*\])*)\]\([^)]*\)", r"\1", text)
    text = text.replace("`", "")
    text = re.sub(r"<[^>]*>", "", text)
    return text


def github_slug(text):
    slug = plain_heading(text).strip().lower()
    slug = re.sub(r"[^\w\- ]", "", slug)
    return slug.replace(" ", "-")


def headings(text):
    """[(line index in the text, level, heading text, anchor)] outside code blocks, anchors made unique as GitHub
    does (second "x" is "x-1")."""
    found, seen = [], {}
    offset = 0
    for is_code, chunk in split_fences(text):
        lines = chunk.splitlines()
        if not is_code:
            for index, line in enumerate(lines):
                match = ATX.match(line)
                if not match:
                    continue
                title = (match.group(2) or "").strip()
                base = github_slug(title)
                anchor = base
                if base in seen:
                    seen[base] += 1
                    anchor = f"{base}-{seen[base]}"
                    while anchor in seen:
                        seen[base] += 1
                        anchor = f"{base}-{seen[base]}"
                seen.setdefault(anchor, 0)
                found.append((offset + index, len(match.group(1)), title, anchor))
        offset += len(lines)
    return found


def site_page_url(path):
    """Address on the site for a file of the website source (site/content/<lang>/…), else None."""
    match = re.match(r"^site/content/(de|en)(/.*)?$", path)
    if not match:
        return None
    lang, rest = match.group(1), (match.group(2) or "/")
    prefix = "/" if lang == "de" else "/en/"
    rest = rest.lstrip("/")
    if rest.endswith(".md"):
        rest = rest[:-3]
        if rest == "_index" or rest.endswith("/_index"):
            rest = rest[: -len("_index")]
        elif rest:
            rest += "/"
    elif rest and not rest.endswith("/"):
        rest += "/"
    return prefix + rest


class Generator:
    def __init__(self, repo, site, docs=DOCS, lang="en"):
        self.repo = os.path.abspath(repo)
        self.site = os.path.abspath(site)
        self.docs = docs
        self.lang = lang
        self.german = lang == "de"
        self.base = "docs/de" if self.german else "docs"
        self.url_base = URL_BASE_DE if self.german else URL_BASE
        self.output = OUTPUT_DE if self.german else OUTPUT
        self.findings = []
        self.repo_link = repo_url(self.site)
        self.sources = {}
        self.anchors = {}
        for name, _slug, _weight, _description in docs:
            if self.german:
                self.load_german(name)
                continue
            path = os.path.join(self.repo, "docs", name)
            if not os.path.isfile(path):
                self.findings.append((f"docs/{name}", "source is missing"))
                continue
            text = read_text(path)
            self.sources[name] = text
            self.anchors[name] = {anchor for _i, _l, _t, anchor in headings(text)}

    # -- German sources (Round 39d) ------------------------------------------------------------------------

    def german_files(self, name):
        """([paths], [part numbers]) of the German source of one document: docs/de/<Name> (numbers [1]), else
        docs/de/<Stem>.part<N>.md in numeric order (the architecture is kept in parts so that merges stay
        conflict free); ([], []) if there is none."""
        folder = os.path.join(self.repo, "docs", "de")
        whole = os.path.join(folder, name)
        if os.path.isfile(whole):
            return [whole], [1]
        stem = name[:-3] if name.endswith(".md") else name
        pattern = re.compile(r"^" + re.escape(stem) + r"\.part(\d+)\.md$")
        parts = []
        if os.path.isdir(folder):
            for entry in os.listdir(folder):
                match = pattern.match(entry)
                if match:
                    parts.append((int(match.group(1)), os.path.join(folder, entry)))
        parts.sort()
        return [path for _n, path in parts], [n for n, _p in parts]

    def load_german(self, name):
        where = f"docs/de/{name}"
        english_path = os.path.join(self.repo, "docs", name)
        if not os.path.isfile(english_path):
            self.findings.append((f"docs/{name}", "source is missing (the English original is the source of truth)"))
            return
        english = read_text(english_path)
        files, numbers = self.german_files(name)
        if not files:
            self.findings.append((where, "German translation is missing (no docs/de/<Name> and no part files)"))
            return
        if numbers != list(range(1, len(numbers) + 1)):
            self.findings.append((where, f"part files are not numbered 1..N without a gap: {numbers}"))
            return
        pieces = [HEADER_COMMENT.sub("", read_text(path), count=1).strip("\n") for path in files]
        text = "\n\n".join(pieces) + "\n"
        self.sources[name] = text
        mine = headings(text)
        theirs = headings(english)
        ids = set()
        problems = []
        for position in range(max(len(mine), len(theirs))):
            if position >= len(mine):
                problems.append(f"heading {position + 1} is missing (English: {theirs[position][2]})")
                break
            index, level, title, _anchor = mine[position]
            match = EXPLICIT_ID.search(title)
            explicit = match.group(1) if match else None
            if explicit:
                ids.add(explicit)
            if position >= len(theirs):
                problems.append(f"extra heading beyond the English sequence (line {index + 1} of the joined text: "
                                f"{title})")
                break
            _eindex, elevel, etitle, eanchor = theirs[position]
            at = f"line {index + 1} of the joined text"
            if level != elevel:
                problems.append(f"{at}: level {level}, the English heading '{etitle}' has level {elevel}")
            if eanchor and explicit is None:
                problems.append(f"{at}: heading without explicit id, expected {{#{eanchor}}}")
            elif eanchor and explicit != eanchor:
                problems.append(f"{at}: id {{#{explicit}}}, expected {{#{eanchor}}}")
            elif not eanchor and explicit is not None:
                problems.append(f"{at}: id {{#{explicit}}} on a heading that has no anchor in English")
        for problem in problems[:10]:
            self.findings.append((where, problem))
        if len(problems) > 10:
            self.findings.append((where, f"... and {len(problems) - 10} more heading findings"))
        self.anchors[name] = ids

    def slug_of(self, name):
        return next(slug for doc, slug, _w, _d in self.docs if doc == name)

    def rewrite(self, source, target):
        """Target of a link in docs/<source> (German: docs/de/<source>) as the site needs it."""
        bracketed = target.startswith("<") and target.endswith(">")
        bare = target[1:-1] if bracketed else target
        if not bare or re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", bare) or bare.startswith("//"):
            return target
        path, _hash, fragment = bare.partition("#")
        where = f"{self.base}/{source}"
        if not path:
            if fragment and fragment not in self.anchors.get(source, set()):
                self.findings.append((where, f"anchor #{fragment} does not exist in this file"))
            return target
        joined = posixpath.normpath(posixpath.join(self.base, path))
        if joined == ".." or joined.startswith("../") or posixpath.isabs(path):
            self.findings.append((where, f"link leaves the repository: {target}"))
            return target
        anchor = f"#{fragment}" if fragment else ""
        name = joined[len(self.base) + 1:] if joined.startswith(self.base + "/") else None
        if name is not None and "/" in name:
            name = None
        if name in self.sources:
            if fragment and fragment not in self.anchors[name]:
                self.findings.append((where, f"anchor #{fragment} does not exist in {self.base}/{name}: {target}"))
            return f"{self.url_base}{self.slug_of(name)}/{anchor}"
        if self.german and joined.startswith("docs/") and joined[len("docs/"):] in {d[0] for d in self.docs}:
            self.findings.append((where, f"link into the English original {joined}; link to the German sibling "
                                         f"{joined[len('docs/'):]} instead: {target}"))
            return target
        full = os.path.join(self.repo, *joined.split("/"))
        if not os.path.exists(full):
            self.findings.append((where, f"link to a file that does not exist in the repository: {target}"))
            return target
        page = site_page_url(joined)
        if page is not None:
            if self.german and page.startswith("/en/"):
                # A German page links to the German twin of an English site page when there is one.
                twin = "site/content/de/" + joined[len("site/content/en/"):]
                if os.path.exists(os.path.join(self.repo, *twin.split("/"))):
                    page = site_page_url(twin)
            return page + anchor
        kind = "tree" if os.path.isdir(full) else "blob"
        return f"{self.repo_link}/{kind}/main/{joined}{anchor}"

    def convert_text(self, source, chunk):
        """Links and raw HTML of a chunk of running text (no code block)."""
        spans = []

        def keep(match):
            spans.append(match.group(0))
            return PLACEHOLDER.format(len(spans) - 1)

        work = CODE_SPAN.sub(keep, chunk)
        work = INLINE_LINK.sub(lambda m: m.group(1) + self.rewrite(source, m.group(2)) + (m.group(3) or "") + ")",
                               work)
        work = REF_DEF.sub(lambda m: m.group(1) + self.rewrite(source, m.group(2)), work)
        work = HTML_COMMENT.sub("", work)
        pieces = []
        last = 0
        for match in AUTOLINK.finditer(work):
            pieces.append(RAW_TAG.sub("&lt;", work[last:match.start()]))
            pieces.append(match.group(0))
            last = match.end()
        pieces.append(RAW_TAG.sub("&lt;", work[last:]))
        work = "".join(pieces)
        return re.sub(r"\x00(\d+)\x00", lambda m: spans[int(m.group(1))], work)

    def source_label(self, name):
        """What the front matter and the layout call the source file."""
        if not self.german:
            return "docs/" + name
        if os.path.isfile(os.path.join(self.repo, "docs", "de", name)):
            return f"docs/de/{name}"
        stem = name[:-3] if name.endswith(".md") else name
        return f"docs/de/{stem}.part*.md"

    def convert(self, name, weight, description):
        text = self.sources[name]
        heads = headings(text)
        title = None
        lines = text.splitlines()
        drop = None
        for index, level, heading, _anchor in heads:
            if level == 1:
                title, drop = heading, index
                break
        if title is None:
            self.findings.append((f"{self.base}/{name}", "no heading of level 1 (the title of the page)"))
            title = name
        if self.german:
            # The explicit ids are already in the text (checked against English in load_german).
            title = EXPLICIT_ID.sub("", title)
            ids = {}
        else:
            # A heading without letters or digits has no anchor (`{#}` would be invalid); Hugo then makes its own.
            ids = {index: anchor for index, _level, _heading, anchor in heads if anchor}
        out = []
        for index, line in enumerate(lines):
            if index == drop:
                continue
            if index in ids:
                line = line.rstrip()
                line = re.sub(r"[ \t]+#+$", "", line)
                line = f"{line} {{#{ids[index]}}}"
            out.append(line)
        body = "\n".join(out).lstrip("\n") + "\n"
        converted = []
        for is_code, chunk in split_fences(body):
            converted.append(chunk if is_code else self.convert_text(name, chunk))
        body = SHORTCODE.sub(lambda m: "{{" + m.group(1) + "/*" + m.group(2) + "*/" + m.group(3) + "}}",
                             "".join(converted))
        plain_title = re.sub(r"\s+", " ", plain_heading(title)).strip()
        if self.german:
            description = DESCRIPTIONS_DE.get(self.slug_of(name), description)
        front = ["---", f"title: {json.dumps(plain_title, ensure_ascii=False)}",
                 f"description: {json.dumps(description, ensure_ascii=False)}", f"weight: {weight}",
                 f"quelle: {json.dumps(self.source_label(name))}", "---", ""]
        return "\n".join(front) + body

    def index_page(self):
        title, description, text = ((INDEX_TITLE_DE, INDEX_DESCRIPTION_DE, INDEX_TEXT_DE) if self.german
                                    else (INDEX_TITLE, INDEX_DESCRIPTION, INDEX_TEXT))
        front = ["---", f"title: {json.dumps(title, ensure_ascii=False)}",
                 f"description: {json.dumps(description, ensure_ascii=False)}", "---", ""]
        return "\n".join(front) + text

    def build(self):
        """The pages as {file: content}; raises Findings instead of returning when a source has a finding."""
        pages = {}
        if not self.findings:
            pages["_index.md"] = self.index_page()
            for name, slug, weight, description in self.docs:
                pages[f"{slug}.md"] = self.convert(name, weight, description)
        if self.findings:
            raise Findings(self.findings)
        return pages

    def write(self, pages):
        output = os.path.join(self.site, self.output)
        if os.path.isdir(output):
            shutil.rmtree(output)
        for file, content in pages.items():
            write_text(os.path.join(output, file), content)
        return output, sorted(pages)

    def run(self):
        """Writes the pages; raises Findings instead of writing anything when a source has a finding."""
        return self.write(self.build())


# ---- Self-test ----------------------------------------------------------------------------------------------

def selbsttest_de(assertion):
    """The German run on an invented mini repository (Round 39d); each assertion with its reason."""
    docs = (("Eins.md", "one", 10, "First invented file."), ("Zwei.md", "two", 20, "Second invented file."),
            ("Gross.md", "big", 30, "Third invented file, translated in two parts."))
    header = "<!-- translation-of: docs/{0}.md source-commit: abc1234 -->\n\n"
    zwei = header.format("Zwei") + "# Zweite Datei {#second-file}\n\n## Zweiter Teil {#second-part}\n"
    files = {
        "site/hugo.toml": 'title = "Probe"\n[params]\nversion = "0.0.0"\nrepo = "https://example.org/probe"\n[markup]\n',
        "site/content/de/hilfe/moewen.md": "---\ntitle: Möwen\n---\n",
        "site/content/en/hilfe/moewen.md": "---\ntitle: Gulls\n---\n",
        "site/content/en/hilfe/nur-englisch.md": "---\ntitle: English only\n---\n",
        "crates/probe/src/lib.rs": "// invented\n",
        "docs/Backlog.md": "# Backlog\n",
        "docs/Eins.md": "# The first file\n\nSee [two](Zwei.md#second-part), [code](../crates/probe/src/lib.rs#L1), "
                        "[backlog](Backlog.md), [help](../site/content/en/hilfe/moewen.md), "
                        "[only](../site/content/en/hilfe/nur-englisch.md) and [here](#same-heading-1).\n\n"
                        "## Same heading\n\n## Same heading\n\n```sh\necho <id> \"{{< x >}}\"\n```\n",
        "docs/Zwei.md": "# Second file\n\n## Second part\n\nText.\n",
        "docs/Gross.md": "# Big\n\n## Alpha\n\n### Deep one\n\n## Beta\n",
        "docs/de/Eins.md": header.format("Eins") +
                           "# Die erste Datei {#the-first-file}\n\n> Dies ist die deutsche Übersetzung von `docs/Eins.md`.\n\n"
                           "Siehe [zwei](Zwei.md#second-part), [Code](../../crates/probe/src/lib.rs#L1), "
                           "[Rückstand](../Backlog.md), [Hilfe](../../site/content/en/hilfe/moewen.md), "
                           "[nur](../../site/content/en/hilfe/nur-englisch.md) und [hier](#same-heading-1) "
                           "mit <Titel>.\n\n"
                           "## Gleiche Überschrift {#same-heading}\n\n## Gleiche Überschrift {#same-heading-1}\n\n"
                           "```sh\necho <id> \"{{< x >}}\"\n```\n",
        "docs/de/Zwei.md": zwei + "\nText.\n",
        "docs/de/Gross.part1.md": header.format("Gross") + "# Groß {#big}\n\n## Alpha {#alpha}\n\n### Tief {#deep-one}\n",
        "docs/de/Gross.part2.md": header.format("Gross") + "## Beta {#beta}\n\nText.\n",
    }

    def make(root, change=None):
        for path, text in {**files, **(change or {})}.items():
            if text is None:
                target = os.path.join(root, *path.split("/"))
                if os.path.exists(target):
                    os.remove(target)
            else:
                write_text(os.path.join(root, *path.split("/")), text)

    def german(root):
        return Generator(root, os.path.join(root, "site"), docs, "de")

    with tempfile.TemporaryDirectory(prefix="docs-pages-de-") as root:
        make(root)
        output, written = german(root).run()
        one = read_text(os.path.join(output, "one.md"))
        big = read_text(os.path.join(output, "big.md"))
        assertion(written == ["_index.md", "big.md", "one.md", "two.md"],
                  "German: index and one page per document are written",
                  f"The section /docs/ needs its index and one page per listed document (written: {written}).")
        assertion(os.path.relpath(output, root) == os.path.join("site", "generated-de", "docs"),
                  "German: the output lies under site/generated-de/docs",
                  "Hugo mounts it as German content; the English output (generated/) must not be touched or mixed in.")
        assertion('title: "Die erste Datei"' in one and "{#the-first-file}" not in one.split("---", 2)[1],
                  "German: the title of the page is the level-1 heading without its explicit id",
                  "The id belongs to the heading line, not to the title text of the front matter.")
        assertion("# Die erste" not in one, "German: the level-1 heading is removed from the body",
                  "The layout prints the title as <h1>.")
        assertion(one.count("## Gleiche Überschrift {#same-heading-1}") == 1 and "{#same-heading-1} {#" not in one,
                  "German: explicit ids are kept exactly once",
                  "The English ids are the anchors of the site; a second generated id would break the heading.")
        assertion('quelle: "docs/de/Eins.md"' in one and 'quelle: "docs/de/Gross.part*.md"' in big,
                  "German: the front matter names the German source (parts as part*)",
                  "The layout prints the source file; the German page must not claim the English original.")
        assertion("[zwei](/docs/two/#second-part)" in one,
                  "German: a link to a sibling document goes to /docs/<slug>/ with its anchor",
                  "The German section lives at /docs/ (German is the default language, no subfolder).")
        assertion("(https://example.org/probe/blob/main/crates/probe/src/lib.rs#L1)" in one,
                  "German: ../../crates/... resolves against docs/de/ to a link into the repository",
                  "The extra ../ of the translation (D5) must lead to the same file as in the English text.")
        assertion("(https://example.org/probe/blob/main/docs/Backlog.md)" in one,
                  "German: ../Backlog.md (untranslated, in docs/) goes to the repository",
                  "Backlog, Fahrplan and schema are not translated and not on the site.")
        assertion("[Hilfe](/hilfe/moewen/)" in one,
                  "German: a link to an English site page goes to its German twin",
                  "A German page should not send the reader into the English help when a German page exists.")
        assertion("[nur](/en/hilfe/nur-englisch/)" in one,
                  "German: without a German twin the English page address stays",
                  "A link must not point to a page that does not exist.")
        assertion("mit &lt;Titel>." in one and "echo <id> \"{{</* x */>}}\"" in one,
                  "German: raw HTML is escaped, code keeps <, shortcodes are escaped",
                  "The same text rules as for the English pages apply.")
        assertion("## Alpha {#alpha}" in big and "### Tief {#deep-one}" in big and "## Beta {#beta}" in big
                  and "translation-of" not in big and big.index("Alpha") < big.index("Beta"),
                  "German: part files are joined in numeric order and their header comments are dropped",
                  "The architecture exists only as parts in git; the page must read as one document.")
        index = read_text(os.path.join(output, "_index.md"))
        assertion('title: "Dokumentation"' in index and "/en/docs/" in index,
                  "German: the index has the German title and points to the English original",
                  "The menu entry is Dokumentation; the English files are authoritative, which the reader should see.")
        english_output, _ = Generator(root, os.path.join(root, "site"), docs).run()
        assertion(os.path.exists(os.path.join(english_output, "one.md")) and os.path.exists(os.path.join(output, "one.md")),
                  "both languages can be written side by side",
                  "ci-site.sh builds both; one run must not remove the output of the other.")

    cases = (
        ("a heading without explicit id", {"docs/de/Zwei.md": zwei.replace(" {#second-part}", "")},
         "heading without explicit id, expected {#second-part}",
         "Without the English id the German anchors would differ and every link to them would break."),
        ("a wrong id", {"docs/de/Zwei.md": zwei.replace("{#second-part}", "{#zweiter-teil}")},
         "id {#zweiter-teil}, expected {#second-part}", "Link anchors stay the English slugs (D4)."),
        ("a missing heading", {"docs/de/Zwei.md": zwei.split("\n## ")[0] + "\n"},
         "heading 2 is missing", "A German document must have all headings of its English source."),
        ("an extra heading", {"docs/de/Zwei.md": zwei + "\n## Mehr {#more}\n"},
         "extra heading beyond the English sequence", "A heading the original does not have means the translation drifted."),
        ("a different level", {"docs/de/Zwei.md": zwei.replace("## Zweiter", "### Zweiter")},
         "level 3", "The table of contents of the page is built from the levels."),
        ("a dead anchor", {"docs/de/Zwei.md": zwei + "\n[x](Eins.md#gibt-es-nicht)\n"},
         "anchor #gibt-es-nicht does not exist in docs/de/Eins.md", "A dead link anchor ends at the top of the page."),
        ("a dead anchor in the same file", {"docs/de/Zwei.md": zwei + "\n[x](#nirgends)\n"},
         "anchor #nirgends does not exist in this file", "Same-page anchors are checked as well."),
        ("a link into the English original", {"docs/de/Zwei.md": zwei + "\n[x](../Eins.md)\n"},
         "link into the English original",
         "D5: links among the nine documents are siblings in docs/de/; a link to docs/ would leave the German section."),
        ("a dead file link", {"docs/de/Zwei.md": zwei + "\n[x](../crates/probe/src/lib.rs)\n"},
         "does not exist in the repository", "Without the extra ../ the link resolves to a path that is not there."),
        ("a missing part", {"docs/de/Gross.part2.md": None, "docs/de/Gross.part3.md": header.format("Gross") + "## Beta {#beta}\n"},
         "not numbered 1..N without a gap", "A lost part would silently drop a quarter of a document."),
        ("a missing translation", {"docs/de/Zwei.md": None},
         "German translation is missing", "A German page missing is a finding (exit 1), never an empty page."),
    )
    for was, change, expected, why in cases:
        with tempfile.TemporaryDirectory(prefix="docs-pages-de-") as root:
            make(root, change)
            try:
                german(root).run()
                found = []
            except Findings as error:
                found = error.args[0]
            written = os.path.exists(os.path.join(root, "site", "generated-de"))
            assertion(any(expected in message for _f, message in found) and not written,
                      f"German: {was} is a finding and nothing is written",
                      f"{why} (expected: {expected}; reported: {found})")


def selbsttest():
    failed = 0

    def assertion(ok, what, why):
        nonlocal failed
        print(f"{'ok ' if ok else 'FEHLER'}  {what}\n        Reason: {why}")
        if not ok:
            failed += 1

    docs = (("Eins.md", "one", 10, "First invented file."), ("Zwei.md", "two", 20, "Second invented file."))

    def mini_repo(root, eins=None):
        write_text(os.path.join(root, "site", "hugo.toml"),
                   'title = "Probe"\n[params]\nversion = "0.0.0"\nrepo = "https://example.org/probe"\n[markup]\n')
        write_text(os.path.join(root, "site", "content", "de", "hilfe", "moewen.md"), "---\ntitle: Möwen\n---\n")
        write_text(os.path.join(root, "crates", "probe", "src", "lib.rs"), "// invented\n")
        write_text(os.path.join(root, "docs", "Eins.md"), eins if eins is not None else (
            "# The first `file`\n\nIntro with [two](Zwei.md#second-part), [code](../crates/probe/src/lib.rs#L1), "
            "[folder](../crates/probe/), [help](../site/content/de/hilfe/moewen.md#zaehlen) and "
            "[here](#same-heading-1).\n\n"
            "## Same heading\n\nA `[not a link](Nirgends.md)` and a <Titel> and <https://example.org/x>.\n<!-- End of an invented part -->\n\n"
            "## Same heading\n\n```sh\necho <id> '{{< taste \"Strg+S\" >}}'\n[no](Nirgends.md)\n```\n\n"
            "Shortcode in code: `{{< hilfebild name=\"x\" >}}`.\n\n[ref]: Zwei.md\n"))
        write_text(os.path.join(root, "docs", "Zwei.md"), "# Second file\n\n## Second part\n\nText.\n")

    with tempfile.TemporaryDirectory(prefix="docs-pages-") as root:
        mini_repo(root)
        output, files = Generator(root, os.path.join(root, "site"), docs).run()
        one = read_text(os.path.join(output, "one.md"))
        assertion(files == ["_index.md", "one.md", "two.md"], "index and one page per source are written",
                  f"The section needs its index (/en/docs/) and one page per listed source. (written: {files})")
        assertion(os.path.relpath(output, root) == os.path.join("site", "generated", "docs"),
                  "the output lies under site/generated/docs, not under site/content",
                  "Generated pages under site/content would break the rule that every English help page has a German "
                  "twin (round39b_hilfe_en) and mix generated with hand-written content.")
        assertion('title: "The first file"' in one and "weight: 10" in one and 'quelle: "docs/Eins.md"' in one,
                  "front matter: title from the first heading, weight from the table, source path",
                  "Contract §2.3: title from the first heading, weight as listed; the layout names the source.")
        assertion("# The first" not in one, "the level-1 heading is removed from the body",
                  "The layout prints the title as <h1>; a second one would repeat it.")
        assertion("## Same heading {#same-heading}" in one and "## Same heading {#same-heading-1}" in one,
                  "headings get GitHub anchors, duplicates numbered",
                  "Anchors that work on GitHub must work on the site, independent of Hugo's id rules.")
        assertion("[two](/en/docs/two/#second-part)" in one, "a link to another source goes to its site address",
                  "Contract §2.4: links between the nine documents become site URLs with existing anchors.")
        assertion("(https://example.org/probe/blob/main/crates/probe/src/lib.rs#L1)" in one,
                  "a link to another repository file goes to <params.repo>/blob/main/<path>",
                  "Contract §2.4: other repo files are not on the site; the repository shows them.")
        assertion("(https://example.org/probe/tree/main/crates/probe)" in one,
                  "a link to a folder goes to tree/main",
                  "GitHub shows folders under tree/; blob/ would only work through a redirect.")
        assertion("[help](/hilfe/moewen/#zaehlen)" in one, "a link to a page of the website source goes to that page",
                  "The page is on the same site; its Markdown source on GitHub would show raw shortcodes.")
        assertion("[here](#same-heading-1)" in one and "[ref]: /en/docs/two/" in one,
                  "same-page anchors stay, reference definitions are rewritten",
                  "Both link forms of Markdown must follow the same rules.")
        assertion("`[not a link](Nirgends.md)`" in one and "[no](Nirgends.md)" in one,
                  "links in code spans and code blocks stay untouched (and are not checked)",
                  "Code shows Markdown as an example; rewriting it would change the example.")
        assertion("a &lt;Titel> and <https://example.org/x>" in one, "raw HTML is escaped, autolinks stay",
                  "Hugo drops raw HTML (unsafe = false); <Titel> would vanish from the sentence.")
        assertion("End of an invented part" not in one, "HTML comments in the text are removed",
                  "GitHub hides them; escaped, the markers for the editors would stand visibly on the page.")
        assertion("echo <id>" in one, "code blocks keep their < unescaped",
                  "In code the text is shown verbatim; &lt; would appear literally.")
        assertion("{{</* taste \"Strg+S\" */>}}" in one and "`{{</* hilfebild name=\"x\" */>}}`" in one,
                  "shortcode syntax is escaped everywhere, also in code",
                  "Hugo calls shortcodes even inside code; unescaped, the build would fail or insert a picture.")
        index = read_text(os.path.join(output, "_index.md"))
        assertion(f"title: \"{INDEX_TITLE}\"" in index, "the index page has its title",
                  "/en/docs/ is the fixed address of the section (contract §2.3).")
        stale = os.path.join(output, "stale.md")
        write_text(stale, "old")
        Generator(root, os.path.join(root, "site"), docs).run()
        assertion(not os.path.exists(stale), "a second run removes pages of sources that are gone",
                  "Otherwise a removed source would stay on the site as a stale page.")

    for eins, expected, why in (
            ("# T\n\n[x](Fehlt.md)\n", "does not exist in the repository",
             "A dead relative link would end at a 404 on GitHub; the generator is the only check for these."),
            ("# T\n\n[x](Zwei.md#gibt-es-nicht)\n", "anchor #gibt-es-nicht does not exist in docs/Zwei.md",
             "site-check.py would find it too, but the generator names the source file and line target."),
            ("# T\n\n[x](#nirgends)\n", "anchor #nirgends does not exist in this file",
             "Same-page anchors are checked as well."),
            ("# T\n\n[x](../../aussen.md)\n", "link leaves the repository",
             "A path outside the repository cannot be linked anywhere."),
            ("Text without a title.\n", "no heading of level 1", "The title of the page comes from it.")):
        with tempfile.TemporaryDirectory(prefix="docs-pages-") as root:
            mini_repo(root, eins)
            try:
                Generator(root, os.path.join(root, "site"), docs).run()
                found = []
            except Findings as error:
                found = error.args[0]
            written = os.path.exists(os.path.join(root, "site", "generated"))
            assertion(any(expected in message for _f, message in found) and not written,
                      f"finding reported and nothing written: {expected}", f"{why} (reported: {found})")

    selbsttest_de(assertion)

    print("docs_pages.py --selbsttest: " + ("alles in Ordnung" if not failed else f"{failed} assertion(s) failed"))
    return 1 if failed else 0


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    default_site = os.path.dirname(here)
    parser = argparse.ArgumentParser(description="Generates the documentation pages of the website from docs/*.md "
                                                 "(English) and docs/de/ (German).")
    parser.add_argument("--site", default=default_site, help="source of the site (default: site in the repository)")
    parser.add_argument("--repo", default=None, help="repository with docs/ (default: the folder above --site)")
    parser.add_argument("--lang", default="all", choices=("en", "de", "all"),
                        help="language of the section to generate or list (default: all)")
    parser.add_argument("--list", action="store_true", help="print the sources with address and weight, write nothing")
    parser.add_argument("--selbsttest", action="store_true", help="check the generator on an invented mini repository")
    options = parser.parse_args()
    if options.selbsttest:
        return selbsttest()
    languages = LANGUAGES if options.lang == "all" else (options.lang,)
    if options.list:
        for lang in languages:
            for name, slug, weight, _description in DOCS:
                if lang == "de":
                    print(f"docs/de/{name}\t{URL_BASE_DE}{slug}/\t{weight}")
                else:
                    print(f"docs/{name}\t{URL_BASE}{slug}/\t{weight}")
        return 0
    site = os.path.abspath(options.site)
    repo = os.path.abspath(options.repo) if options.repo else os.path.dirname(site)
    # Both languages are built first and written only when neither has a finding.
    built, findings = [], []
    for lang in languages:
        try:
            generator = Generator(repo, site, lang=lang)
            built.append((generator, generator.build()))
        except Findings as error:
            findings.extend(error.args[0])
    if findings:
        for where, message in findings:
            print(f"{where}: {message}")
        print(f"docs_pages.py: {len(findings)} finding(s), nothing written", file=sys.stderr)
        return 1
    for generator, pages in built:
        output, files = generator.write(pages)
        print(f"docs_pages.py: {len(files)} page(s) in {output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
