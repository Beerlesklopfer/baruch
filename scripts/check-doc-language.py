#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Find German prose in a Markdown file that counts as English (Round 38c).

    python3 scripts/check-doc-language.py [--part above|below] [--hints] FILE...
    python3 scripts/check-doc-language.py --selftest

The prose filter is the same as `prose` in crates/baruch-server/tests/round38_doku.rs: fenced code blocks,
`code spans`, German quotations („…“, up to the end of their paragraph at the latest) and straight quotations
("…", up to the end of their line at the latest) are blanked out line by line, so that line numbers stay.
German terms of the user interface are allowed only quoted like that; there is no separate allow file. The one
exception is ALLOWED below (keep it in sync with ALLOWED in round38_doku.rs).

Every prose line that still carries ä ö ü Ä Ö Ü ß is printed as `path:line: prose`. `--part above|below`
checks only the text before or after the split marker (exit 2 if the marker is missing or not unique); line
numbers stay those of the file. `--hints` also prints `hint: path:line: prose` for prose lines with common
German words without umlauts; hints never change the exit code.

Allowed-German area (Round 39d): the files under docs/de/ (the German translations of the developer docs) are German
by design and are skipped; the check is a path prefix of the path relative to the repository, so nothing else is
exempt (not docs/ itself, not a folder that merely ends in de/).

Exit codes: 0 nothing found, 1 German prose found, 2 usage error or missing file.
Python 3 standard library only.
"""

import os
import sys

# The marker that split docs/Architektur.md between two tasks in Round 38c (removed after the join; kept for
# --part, which can split any file that carries it).
SPLIT = "<!-- R38c split: architektur_1 above, architektur_2 below -->"

# Umlauts allowed in the prose of a translated file: (file name relative to the repository, text, reason).
# Keep in sync with ALLOWED in crates/baruch-server/tests/round38_doku.rs.
ALLOWED = [("README.md", "© Jörg Bernau", "the name of the author in the copyright line")]

# Folder (relative to the repository, with the trailing slash) whose Markdown files are German by design. Keep in
# sync with GERMAN_AREA in crates/baruch-server/tests/round38_doku.rs.
GERMAN_AREA = "docs/de/"

GERMAN_LETTERS = set("äöüÄÖÜß")

# Common German words without umlauts; a prose line containing one is only a hint.
HINT_WORDS = [" und ", " der ", " die ", " das ", " nicht ", " ist ", " mit ", " wird ", " oder ", " auch ",
              " eine ", " sich "]

USAGE = "usage: check-doc-language.py [--part above|below] [--hints] FILE...\n" \
        "       check-doc-language.py --selftest"


def rust_lines(text):
    """Lines as Rust's str::lines gives them (split at \\n, a trailing \\r dropped, no empty last line), so that
    line numbers agree with round38_doku.rs; str.splitlines would also split at form feeds and U+2028."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    return [line[:-1] if line.endswith("\r") else line for line in lines]


def prose(text):
    """The prose of a Markdown text, one entry per line, quotations and code blanked out."""
    out = []
    fenced = False
    state = "prose"
    for line in rust_lines(text):
        if line.lstrip().startswith("```"):
            fenced = not fenced
            state = "prose"
            out.append("")
            continue
        if fenced:
            out.append("")
            continue
        if line.strip() == "" or state == "straight":
            state = "prose"
        kept = []
        for c in line:
            if state == "prose":
                if c == "`":
                    state = "code"
                elif c == "„":
                    state = "german"
                elif c == '"':
                    state = "straight"
                else:
                    kept.append(c)
            elif (state, c) in (("code", "`"), ("german", "“"), ("straight", '"')):
                state = "prose"
        out.append("".join(kept))
    return out


def allowed_for(path):
    """The ALLOWED texts that apply to this path (matched by its path relative to the repository)."""
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    rel = os.path.relpath(os.path.abspath(path), root).replace(os.sep, "/")
    return [text for (name, text, _) in ALLOWED if name == rel]


def in_german_area(path):
    """True for a file below docs/de/ of this repository (decided by its path relative to the repository)."""
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    rel = os.path.relpath(os.path.abspath(path), root).replace(os.sep, "/")
    return rel.startswith(GERMAN_AREA)


def split_part(text, part):
    """(text of the part, number of file lines before it); None if the marker is missing or not unique."""
    if part is None:
        return text, 0
    lines = rust_lines(text)
    at = [i for i, line in enumerate(lines) if line.strip() == SPLIT]
    if len(at) != 1:
        return None
    at = at[0]
    if part == "above":
        return "\n".join(lines[:at]), 0
    return "\n".join(lines[at + 1:]), at + 1


def findings(path, text, part, hints, allowed):
    """(German lines, hint lines) as printable strings; None if the split marker is missing."""
    for a in allowed:
        text = text.replace(a, "")
    split = split_part(text, part)
    if split is None:
        return None
    body, offset = split
    german, hinted = [], []
    for i, line in enumerate(prose(body)):
        number = offset + i + 1
        if any(c in GERMAN_LETTERS for c in line):
            german.append("%s:%d: %s" % (path, number, line))
        elif hints:
            padded = " " + line.lower() + " "
            if any(w in padded for w in HINT_WORDS):
                hinted.append("hint: %s:%d: %s" % (path, number, line))
    return german, hinted


def run(args, out=sys.stdout, err=sys.stderr):
    part = None
    hints = False
    files = []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--part":
            if i + 1 >= len(args) or args[i + 1] not in ("above", "below"):
                print(USAGE, file=err)
                return 2
            part = args[i + 1]
            i += 2
            continue
        if a == "--hints":
            hints = True
        elif a.startswith("-"):
            print(USAGE, file=err)
            return 2
        else:
            files.append(a)
        i += 1
    if not files:
        print(USAGE, file=err)
        return 2
    found = False
    for path in files:
        if in_german_area(path):
            continue
        try:
            with open(path, encoding="utf-8") as f:
                text = f.read()
        except OSError as e:
            print("%s: %s" % (path, e), file=err)
            return 2
        result = findings(path, text, part, hints, allowed_for(path))
        if result is None:
            print("%s: the split marker %s must stand exactly once on a line of its own" % (path, SPLIT), file=err)
            return 2
        german, hinted = result
        for line in german:
            print(line, file=out)
        for line in hinted:
            print(line, file=out)
        found = found or bool(german)
    return 1 if found else 0


def selftest():
    import io
    import tempfile

    count = [0]

    def check(cond, why):
        count[0] += 1
        if not cond:
            raise AssertionError(why)

    lines = prose("Größe „Über\nBaruch“ über `für` \"Tür\" Maß\n```\nÄrger\n```\nöde\n")
    check(lines[0] == "Größe ", "prose before a German quotation stays; the quotation is dropped")
    check(lines[1] == " über   Maß", "a German quotation runs over a line break; code span and \"…\" are dropped")
    check(lines[3] == "", "a fenced block is code as a whole")
    check(lines[5] == "öde", "after the fence prose counts again")
    open_quote = prose("eine \"offene Klammer\nÄrger\n")
    check(open_quote[1] == "Ärger", "an unbalanced straight quotation mark ends with its line")

    with tempfile.TemporaryDirectory() as tmp:
        def write(name, text):
            p = os.path.join(tmp, name)
            with open(p, "w", encoding="utf-8") as f:
                f.write(text)
            return p

        def call(*args):
            out, err = io.StringIO(), io.StringIO()
            code = run(list(args), out, err)
            return code, out.getvalue(), err.getvalue()

        clean = write("clean.md", "# Title\n\nThe button „Speichern“ saves; `für` is code.\n")
        code, out, _ = call(clean)
        check(code == 0 and out == "", "quoted UI terms and code spans are no German prose (exit 0)")

        dirty = write("dirty.md", "# Title\n\nDas Werk wird gespeichert, für immer.\n")
        code, out, _ = call(dirty)
        check(code == 1, "German prose gives exit 1")
        check(out.strip() == "%s:3: Das Werk wird gespeichert, für immer." % dirty, "path:line: prose is printed")

        split = write("split.md", "oben für\n\n%s\n\nunten über\nfine\n" % SPLIT)
        code, out, _ = call("--part", "above", split)
        check(code == 1 and out.strip() == "%s:1: oben für" % split, "--part above checks only the text above")
        code, out, _ = call("--part", "below", split)
        check(code == 1 and out.strip() == "%s:5: unten über" % split,
              "--part below reports the line numbers of the file, not of the part")

        nomarker = write("nomarker.md", "no marker here\n")
        code, _, err = call("--part", "below", nomarker)
        check(code == 2 and "split marker" in err, "a missing marker is a usage error (exit 2)")
        code, _, _ = call(os.path.join(tmp, "missing.md"))
        check(code == 2, "a missing file is a usage error (exit 2)")
        code, _, _ = call("--part", "middle", clean)
        check(code == 2, "an unknown part is a usage error (exit 2)")
        code, _, _ = call()
        check(code == 2, "no file is a usage error (exit 2)")

        hinted = write("hinted.md", "This is fine.\nDas ist nicht gut und so.\n")
        code, out, _ = call(hinted)
        check(code == 0 and out == "", "without --hints, German words without umlauts are not reported")
        code, out, _ = call("--hints", hinted)
        check(code == 0, "hints never change the exit code")
        check(out.strip() == "hint: %s:2: Das ist nicht gut und so." % hinted, "a hint names path and line")

    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    check(in_german_area(os.path.join(root, "docs", "de", "Plan.md")) and
          in_german_area(os.path.join(root, "docs", "de", "sub", "x.md")),
          "files below docs/de/ are in the allowed-German area")
    check(not in_german_area(os.path.join(root, "docs", "Plan.md")) and
          not in_german_area(os.path.join(root, "docs", "schema", "de", "x.md")) and
          not in_german_area(os.path.join(root, "docs", "dex", "x.md")) and
          not in_german_area(os.path.join(root, "README.md")),
          "nothing else is exempt: not docs/ itself, not another folder called de, not a longer name")
    check(allowed_for(os.path.join(root, "README.md")) == ["© Jörg Bernau"],
          "the copyright line of README.md is the one allowed exception")
    check(allowed_for(os.path.join(root, "SKILLS.md")) == [], "no exception applies to other files")
    print("check-doc-language.py --selftest: %d assertions passed" % count[0])
    return 0


def main():
    args = sys.argv[1:]
    if args == ["--selftest"]:
        try:
            return selftest()
        except AssertionError as e:
            print("selftest failed: %s" % e, file=sys.stderr)
            return 1
    return run(args)


if __name__ == "__main__":
    sys.exit(main())
