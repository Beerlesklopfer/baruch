#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Shows which German translation of a developer document follows an older state than its English source (Round 39d).

    python3 -I scripts/check-doc-lag.py [--strict]
    python3 -I scripts/check-doc-lag.py --selftest

The nine documents (docs/Bedienung.md … docs/Plan.md) exist additionally in German under docs/de/. Line 1 of every
German file is the header `<!-- translation-of: docs/<Name>.md source-commit: <hash> -->` (the architecture, kept in
the parts docs/de/Architektur.part<N>.md, adds ` part: <N>` before ` -->` in every part). <hash> is the last commit
that changed the English source before the translation was made. This script compares the time of the commit
`git log -1 -- docs/<Name>.md` (the last change of the source) with the time of <hash>:

    lag: docs/de/<Name>.md follows <hash> (<date>), the source changed <date> in <hash2>   the translation is late
    missing: docs/de/<Name>.md (<reason>)                                                   no file, no header, unknown commit
    ok                                                                                      every document is current

The output is informational: the exit code is 0 in every case, also with lag or missing files, unless --strict is
given (then 1). Exit 2 on a wrong call. No network; only `git log` and `git show` of the local repository are used.
Python standard library only.
"""

import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# The nine documents; the architecture is translated in parts (docs/de/Architektur.part<N>.md).
DOCUMENTS = ("Bedienung.md", "Architektur.md", "RPC.md", "MCP.md", "Austauschformat.md", "WYSIWYG.md", "Betrieb.md",
             "Verteilung.md", "Plan.md")
PARTED = ("Architektur.md",)
HEADER = re.compile(r"^<!-- translation-of: (docs/[^\s]+\.md) source-commit: ([0-9a-f]{7,40})(?: part: (\d+))? -->\s*$")
USAGE = "usage: check-doc-lag.py [--strict]\n       check-doc-lag.py --selftest"


def git(root, *args):
    """Output of a git command in the repository, None if it fails."""
    try:
        done = subprocess.run(["git", "-C", root, *args], capture_output=True, text=True, check=False)
    except OSError:
        return None
    return done.stdout.strip() if done.returncode == 0 else None


def german_files(root, name):
    """[(path relative to the repository, expected part number or None)] of one document; [] if there is none."""
    folder = os.path.join(root, "docs", "de")
    if name in PARTED:
        stem = name[:-3]
        found = []
        if os.path.isdir(folder):
            for entry in os.listdir(folder):
                match = re.match(r"^" + re.escape(stem) + r"\.part(\d+)\.md$", entry)
                if match:
                    found.append((int(match.group(1)), f"docs/de/{entry}"))
        return [(path, number) for number, path in sorted(found)]
    return [(f"docs/de/{name}", None)] if os.path.isfile(os.path.join(folder, name)) else []


def first_line(root, path):
    try:
        with open(os.path.join(root, path), encoding="utf-8") as handle:
            return handle.readline().rstrip("\n")
    except OSError:
        return None


def check(root):
    """(lines to print, number of problems) for the repository at ROOT."""
    lines, problems = [], 0
    for name in DOCUMENTS:
        files = german_files(root, name)
        if not files:
            shown = f"{name[:-3]}.part1.md" if name in PARTED else name
            lines.append(f"missing: docs/de/{shown} (no such file)")
            problems += 1
            continue
        source_time = git(root, "log", "-1", "--format=%ct", "--", f"docs/{name}")
        source_hash = git(root, "log", "-1", "--format=%h", "--", f"docs/{name}")
        source_date = git(root, "log", "-1", "--format=%cs", "--", f"docs/{name}")
        if not source_time or not source_hash:
            lines.append(f"missing: docs/de/{name} (git does not know docs/{name}; not a repository or not committed)")
            problems += 1
            continue
        if name in PARTED and files[0][1] != 1:
            lines.append(f"missing: docs/de/{name[:-3]}.part1.md (no such file)")
            problems += 1
        for path, number in files:
            match = HEADER.match(first_line(root, path) or "")
            if not match:
                lines.append(f"missing: {path} (line 1 is not the header `<!-- translation-of: docs/<Name>.md "
                             "source-commit: <hash> -->`)")
                problems += 1
                continue
            if match.group(1) != f"docs/{name}":
                lines.append(f"missing: {path} (the header names {match.group(1)}, expected docs/{name})")
                problems += 1
                continue
            if number is not None and match.group(3) != str(number):
                lines.append(f"missing: {path} (the header names part {match.group(3)}, the file is part {number})")
                problems += 1
                continue
            hash_time = git(root, "show", "-s", "--format=%ct", match.group(2) + "^{commit}")
            if not hash_time:
                lines.append(f"missing: {path} (the header names the commit {match.group(2)}, which git does not know)")
                problems += 1
                continue
            hash_date = git(root, "show", "-s", "--format=%cs", match.group(2) + "^{commit}")
            late = int(source_time) > int(hash_time)
            if int(source_time) == int(hash_time):
                # The same second: the translation is current only if its commit contains the last change.
                late = git(root, "merge-base", "--is-ancestor", source_hash, match.group(2)) is None \
                    and source_hash != match.group(2)[:len(source_hash)]
            if late:
                lines.append(f"lag: {path} follows {match.group(2)} ({hash_date}), the source changed {source_date} "
                             f"in {source_hash}")
                problems += 1
    if not problems:
        lines.append("ok")
    return lines, problems


def selftest():
    count = [0]

    def ok(cond, why):
        count[0] += 1
        if not cond:
            raise AssertionError(why)

    def run(root, *args):
        subprocess.run(["git", "-C", root, *args], capture_output=True, text=True, check=True,
                       env={**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.org",
                            "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.org"})

    def commit(root, files, when):
        for path, text in files.items():
            target = os.path.join(root, *path.split("/"))
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with open(target, "w", encoding="utf-8") as handle:
                handle.write(text)
        run(root, "add", "-A")
        run(root, "-c", "commit.gpgsign=false", "commit", "-q", "-m", "x", "--date", f"{when} +0000")
        return git(root, "log", "-1", "--format=%h")

    with tempfile.TemporaryDirectory(prefix="doc-lag-") as root:
        run(root, "init", "-q")
        old = "2026-01-01T10:00:00"
        env_old = {"GIT_COMMITTER_DATE": f"{old} +0000"}
        os.environ.update(env_old)
        first = commit(root, {f"docs/{n}": "# T\n" for n in DOCUMENTS}, old)
        os.environ["GIT_COMMITTER_DATE"] = "2026-02-01T10:00:00 +0000"
        second = commit(root, {"docs/Plan.md": "# T\n\nchanged\n"}, "2026-02-01T10:00:00")
        header = "<!-- translation-of: docs/{0} source-commit: {1}{2} -->\n\n# T\n"
        german = {f"docs/de/{n}": header.format(n, first, "") for n in DOCUMENTS if n not in PARTED}
        german["docs/de/Architektur.part1.md"] = header.format("Architektur.md", first, " part: 1")
        german["docs/de/Architektur.part2.md"] = header.format("Architektur.md", first, " part: 2")
        os.environ["GIT_COMMITTER_DATE"] = "2026-03-01T10:00:00 +0000"
        commit(root, german, "2026-03-01T10:00:00")
        lines, problems = check(root)
        ok(problems == 1 and len(lines) == 1 and lines[0].startswith(f"lag: docs/de/Plan.md follows {first} (2026-01-01)")
           and lines[0].endswith(f"the source changed 2026-02-01 in {second}"),
           "a translation that names the commit before the last change of its source is late; the others are current")
        os.environ["GIT_COMMITTER_DATE"] = "2026-03-02T10:00:00 +0000"
        commit(root, {"docs/de/Plan.md": header.format("Plan.md", second, "")}, "2026-03-02T10:00:00")
        lines, problems = check(root)
        ok(lines == ["ok"] and problems == 0, "naming the commit of the last change makes the document current")
        os.remove(os.path.join(root, "docs", "de", "Betrieb.md"))
        lines, problems = check(root)
        ok(lines == ["missing: docs/de/Betrieb.md (no such file)"], "a missing German file is reported as missing")
        with open(os.path.join(root, "docs", "de", "MCP.md"), "w", encoding="utf-8") as handle:
            handle.write("# no header\n")
        lines, _ = check(root)
        ok(any(line.startswith("missing: docs/de/MCP.md (line 1 is not the header") for line in lines),
           "a file without the header line is reported as missing")
        with open(os.path.join(root, "docs", "de", "MCP.md"), "w", encoding="utf-8") as handle:
            handle.write(header.format("MCP.md", "deadbeef", ""))
        lines, _ = check(root)
        ok(any("names the commit deadbeef, which git does not know" in line for line in lines),
           "a header with an unknown commit is reported as missing, not as a crash")
        os.remove(os.path.join(root, "docs", "de", "Architektur.part1.md"))
        lines, _ = check(root)
        ok("missing: docs/de/Architektur.part1.md (no such file)" in lines,
           "the architecture needs part 1; the other parts alone are not enough")
    print("check-doc-lag.py --selftest: %d assertions passed" % count[0])
    return 0


def main(argv):
    if argv == ["--selftest"]:
        try:
            return selftest()
        except AssertionError as error:
            print(f"selftest failed: {error}", file=sys.stderr)
            return 1
    if argv not in ([], ["--strict"]):
        print(USAGE, file=sys.stderr)
        return 2
    lines, problems = check(ROOT)
    for line in lines:
        print(line)
    return 1 if problems and argv == ["--strict"] else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
