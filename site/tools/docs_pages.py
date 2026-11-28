#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Generates the developer documentation pages of the website from docs/*.md (Round 39c, task docs_site).

    python3 -I site/tools/docs_pages.py [--site ORDNER] [--repo ORDNER]
    python3 -I site/tools/docs_pages.py --list
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
    def __init__(self, repo, site, docs=DOCS):
        self.repo = os.path.abspath(repo)
        self.site = os.path.abspath(site)
        self.docs = docs
        self.findings = []
        self.repo_link = repo_url(self.site)
        self.sources = {}
        self.anchors = {}
        for name, slug, _weight, _description in docs:
            path = os.path.join(self.repo, "docs", name)
            if not os.path.isfile(path):
                self.findings.append((f"docs/{name}", "source is missing"))
                continue
            text = read_text(path)
            self.sources[name] = text
            self.anchors[name] = {anchor for _i, _l, _t, anchor in headings(text)}

    def slug_of(self, name):
        return next(slug for doc, slug, _w, _d in self.docs if doc == name)

    def rewrite(self, source, target):
        """Target of a link in docs/<source> as the site needs it."""
        bracketed = target.startswith("<") and target.endswith(">")
        bare = target[1:-1] if bracketed else target
        if not bare or re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", bare) or bare.startswith("//"):
            return target
        path, _hash, fragment = bare.partition("#")
        where = f"docs/{source}"
        if not path:
            if fragment and fragment not in self.anchors.get(source, set()):
                self.findings.append((where, f"anchor #{fragment} does not exist in this file"))
            return target
        joined = posixpath.normpath(posixpath.join("docs", path))
        if joined == ".." or joined.startswith("../") or posixpath.isabs(path):
            self.findings.append((where, f"link leaves the repository: {target}"))
            return target
        anchor = f"#{fragment}" if fragment else ""
        name = joined[len("docs/"):] if joined.startswith("docs/") else None
        if name in self.sources:
            if fragment and fragment not in self.anchors[name]:
                self.findings.append((where, f"anchor #{fragment} does not exist in docs/{name}: {target}"))
            return f"{URL_BASE}{self.slug_of(name)}/{anchor}"
        full = os.path.join(self.repo, *joined.split("/"))
        if not os.path.exists(full):
            self.findings.append((where, f"link to a file that does not exist in the repository: {target}"))
            return target
        page = site_page_url(joined)
        if page is not None:
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
            self.findings.append((f"docs/{name}", "no heading of level 1 (the title of the page)"))
            title = name
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
        front = ["---", f"title: {json.dumps(plain_title, ensure_ascii=False)}",
                 f"description: {json.dumps(description, ensure_ascii=False)}", f"weight: {weight}",
                 f"quelle: {json.dumps('docs/' + name)}", "---", ""]
        return "\n".join(front) + body

    def index_page(self):
        front = ["---", f"title: {json.dumps(INDEX_TITLE)}", f"description: {json.dumps(INDEX_DESCRIPTION)}", "---", ""]
        return "\n".join(front) + INDEX_TEXT

    def run(self):
        """Writes the pages; raises Findings instead of writing anything when a source has a finding."""
        pages = {}
        if not self.findings:
            pages["_index.md"] = self.index_page()
            for name, slug, weight, description in self.docs:
                pages[f"{slug}.md"] = self.convert(name, weight, description)
        if self.findings:
            raise Findings(self.findings)
        output = os.path.join(self.site, OUTPUT)
        if os.path.isdir(output):
            shutil.rmtree(output)
        for file, content in pages.items():
            write_text(os.path.join(output, file), content)
        return output, sorted(pages)


# ---- Self-test ----------------------------------------------------------------------------------------------

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

    print("docs_pages.py --selbsttest: " + ("alles in Ordnung" if not failed else f"{failed} assertion(s) failed"))
    return 1 if failed else 0


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    default_site = os.path.dirname(here)
    parser = argparse.ArgumentParser(description="Generates the documentation pages of the website from docs/*.md.")
    parser.add_argument("--site", default=default_site, help="source of the site (default: site in the repository)")
    parser.add_argument("--repo", default=None, help="repository with docs/ (default: the folder above --site)")
    parser.add_argument("--list", action="store_true", help="print the sources with slug and weight, write nothing")
    parser.add_argument("--selbsttest", action="store_true", help="check the generator on an invented mini repository")
    options = parser.parse_args()
    if options.selbsttest:
        return selbsttest()
    if options.list:
        for name, slug, weight, _description in DOCS:
            print(f"docs/{name}\t{URL_BASE}{slug}/\t{weight}")
        return 0
    site = os.path.abspath(options.site)
    repo = os.path.abspath(options.repo) if options.repo else os.path.dirname(site)
    try:
        output, files = Generator(repo, site).run()
    except Findings as error:
        for where, message in error.args[0]:
            print(f"{where}: {message}")
        print(f"docs_pages.py: {len(error.args[0])} finding(s), nothing written", file=sys.stderr)
        return 1
    print(f"docs_pages.py: {len(files)} page(s) in {output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
