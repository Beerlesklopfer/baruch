#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Heading anchors of the developer docs and the check of a German translation (Round 39d, S1).

    python3 -I scripts/docs-anchors.py FILE [--range FIRST LAST]
        one line per heading outside code: <line>\\t<level>\\t<anchor>\\t<heading text>
    python3 -I scripts/docs-anchors.py --check GERMAN.md ENGLISH.md [--range FIRST LAST]
        the German file (several parts: GERMAN1.md,GERMAN2.md concatenated in order) must have the same sequence of
        heading levels as the English file or range, and every German heading must carry an explicit id
        {#<english anchor>} equal to the English anchor at the same position.

Anchors are the GitHub slugs with duplicate numbering over the WHOLE file, also with --range. Exit 0 ok, 1 findings
(file:line: reason), 2 usage. Python standard library only; headings and github_slug come from
site/tools/docs_pages.py (no copy).
"""
import importlib.util
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
USAGE = __doc__.split("\n\n")[1]
ID = re.compile(r"[ \t]*\{#([^{}\s]+)\}[ \t]*$")


def load():
    spec = importlib.util.spec_from_file_location("docs_pages", ROOT / "site" / "tools" / "docs_pages.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def usage():
    print("usage:\n" + USAGE, file=sys.stderr)
    return 2


def parse_range(args):
    if not args:
        return None, []
    if len(args) == 3 and args[0] == "--range" and args[1].isdigit() and args[2].isdigit():
        return (int(args[1]), int(args[2])), []
    return None, ["bad"]


def english(docs, path, rng):
    """[(line number 1-based, level, anchor, title)] of the English file, range applied after numbering."""
    rows = [(i + 1, level, anchor, title) for i, level, title, anchor in docs.headings(Path(path).read_text(encoding="utf-8"))]
    if rng:
        rows = [r for r in rows if rng[0] <= r[0] <= rng[1]]
    return rows


def german(docs, paths):
    """[(file, line, level, explicit id or None, title)] of the German parts in order."""
    rows = []
    for path in paths:
        for i, level, title, _anchor in docs.headings(Path(path).read_text(encoding="utf-8")):
            match = ID.search(title)
            rows.append((path, i + 1, level, match.group(1) if match else None, ID.sub("", title)))
    return rows


def main(argv):
    if not argv:
        return usage()
    try:
        docs = load()
    except Exception as error:
        print(f"docs-anchors.py: cannot load site/tools/docs_pages.py: {error}", file=sys.stderr)
        return 2
    if argv[0] == "--check":
        if len(argv) < 3:
            return usage()
        rng, bad = parse_range(argv[3:])
        if bad:
            return usage()
        paths = argv[1].split(",")
        try:
            en = english(docs, argv[2], rng)
            de = german(docs, paths)
        except OSError as error:
            print(f"docs-anchors.py: {error}", file=sys.stderr)
            return 2
        findings = []
        for index in range(max(len(en), len(de))):
            if index >= len(de):
                findings.append(f"{paths[-1]}: missing heading {index + 1} (English line {en[index][0]}: {en[index][3]})")
                break
            file, line, level, ident, title = de[index]
            if index >= len(en):
                findings.append(f"{file}:{line}: extra heading beyond the English sequence ({title})")
                break
            _eline, elevel, eanchor, etitle = en[index]
            if level != elevel:
                findings.append(f"{file}:{line}: level {level}, English line {en[index][0]} has level {elevel} ({etitle})")
            if ident is None:
                findings.append(f"{file}:{line}: no explicit id, expected {{#{eanchor}}}")
            elif ident != eanchor:
                findings.append(f"{file}:{line}: id {{#{ident}}}, expected {{#{eanchor}}}")
        for finding in findings:
            print(finding)
        return 1 if findings else 0
    if argv[0].startswith("--"):
        return usage()
    rng, bad = parse_range(argv[1:])
    if bad:
        return usage()
    try:
        rows = english(docs, argv[0], rng)
    except OSError as error:
        print(f"docs-anchors.py: {error}", file=sys.stderr)
        return 2
    for line, level, anchor, title in rows:
        print(f"{line}\t{level}\t{anchor}\t{title}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
