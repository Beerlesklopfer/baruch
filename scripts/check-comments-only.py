#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Checks that a change touched comments only, and lists German umlauts left in comments.

    python3 scripts/check-comments-only.py BASE HEAD [PATH...]
    python3 scripts/check-comments-only.py --umlaute [--allow FILE] PATH...
    python3 scripts/check-comments-only.py --selftest

BASE HEAD: for every file that differs between the two commits (`git diff --name-status`, contents through
`git show`) the text without comments must be the same. Understood file types: Rust (.rs), TypeScript (.ts),
templates (.html: `{# #}` and `<!-- -->`), Python (.py, module `tokenize`) and shell (.sh). The content of string
literals is compared byte for byte; outside of them only whitespace at the end of a line and empty lines are
ignored. For Rust, code blocks in doc comments (doctests) must be identical and the set of intra-doc link targets
must be the same. Any other file type and every added, deleted or renamed file is reported.
Exit 0 = comments only, 1 = deviation, 2 = usage error.

--umlaute: prints `file:line: text` for every comment line that contains one of ä ö ü Ä Ö Ü ß (same comment
recognition; other file types such as .sql are skipped). Lines matched by an entry of the allow list
(`scripts/comment-umlaut-allow.txt`: `path glob<TAB>substring<TAB>reason`, `#` starts a comment) are left out.
Exit 0 = nothing found, 1 = hits, 2 = usage error.

Known limits (reported honestly rather than guessed): Python docstrings are strings, not comments; indented
(unfenced) code blocks in Rust doc comments are not compared; a shell `#` counts as a trailing comment only if no
quote character precedes it on its line; a `/` after `}` in TypeScript is read as a division.

Python 3, standard library only.
"""

import difflib
import fnmatch
import io
import os
import re
import subprocess
import sys
import tempfile
import tokenize

CODE = "code"
STR = "str"
COMMENT = "comment"
DOC = "doc"  # Rust doc comment (also a comment)

UMLAUTS = re.compile("[äöüÄÖÜß]")
ALLOW_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "comment-umlaut-allow.txt")
SKIP_DIRS = {".git", "target", "node_modules", ".claude"}


class ScanError(Exception):
    """The file could not be split into code, strings and comments."""


class Segments:
    """Collects (kind, text, line) pieces of a file in source order."""

    def __init__(self):
        self.items = []
        self.line = 1

    def add(self, kind, text):
        if not text:
            return
        if self.items and self.items[-1][0] == kind and kind in (CODE, STR):
            last = self.items[-1]
            self.items[-1] = (kind, last[1] + text, last[2])
        else:
            self.items.append((kind, text, self.line))
        self.line += text.count("\n")


# ---- Rust ----------------------------------------------------------------------------------------------------

RAW_START = re.compile(r'r(#*)"')
IDENT_CHAR = re.compile(r"[A-Za-z0-9_]")


def scan_rust(text):
    seg = Segments()
    i = 0
    n = len(text)
    start = 0  # start of the pending code run

    def flush(end):
        seg.add(CODE, text[start:end])

    while i < n:
        c = text[i]
        if c == "/" and text.startswith("//", i):
            flush(i)
            end = text.find("\n", i)
            end = n if end < 0 else end
            body = text[i:end]
            doc = (body.startswith("///") and not body.startswith("////")) or body.startswith("//!")
            seg.add(DOC if doc else COMMENT, body)
            i = start = end
        elif c == "/" and text.startswith("/*", i):
            flush(i)
            depth = 1
            j = i + 2
            while depth and j < n:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            if depth:
                raise ScanError("line %d: block comment is not closed" % seg.line)
            body = text[i:j]
            doc = (body.startswith("/**") and not body.startswith("/***") and body != "/**/") or body.startswith("/*!")
            seg.add(DOC if doc else COMMENT, body)
            i = start = j
        elif c == '"':
            flush(i)
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            if j >= n:
                raise ScanError("line %d: string is not closed" % seg.line)
            seg.add(STR, text[i : j + 1])
            i = start = j + 1
        elif c == "r" and (i == 0 or not IDENT_CHAR.match(text[i - 1]) or raw_prefix(text, i)):
            m = RAW_START.match(text, i)
            if not m:
                i += 1
                continue
            flush(i)
            closing = '"' + m.group(1)
            j = text.find(closing, m.end())
            if j < 0:
                raise ScanError("line %d: raw string is not closed" % seg.line)
            seg.add(STR, text[i : j + len(closing)])
            i = start = j + len(closing)
        elif c == "'":
            # Character literal or lifetime/label: 'x' and '\…' are literals, everything else a lifetime.
            if i + 1 < n and text[i + 1] == "\\":
                j = i + 3  # skip the escaped character, so that '\'' and '\\' end at the right quote
                while j < n and text[j] != "'":
                    j += 1
                if j >= n:
                    raise ScanError("line %d: character literal is not closed" % seg.line)
                flush(i)
                seg.add(STR, text[i : j + 1])
                i = start = j + 1
            elif i + 2 < n and text[i + 2] == "'" and text[i + 1] != "\n":
                flush(i)
                seg.add(STR, text[i : i + 3])
                i = start = i + 3
            else:
                i += 1
        else:
            i += 1
    flush(n)
    return seg.items


def raw_prefix(text, i):
    """Is the `r` at i the second letter of a `br`/`cr` literal prefix (not the end of an identifier)?"""
    return text[i - 1] in "bc" and (i < 2 or not IDENT_CHAR.match(text[i - 2]))


def rust_doc_lines(items):
    """Doc comment text as groups of lines; a group ends where code stands between two doc comments."""
    groups = []
    current = []
    for kind, body, line in items:
        if kind == DOC:
            if body.startswith("//"):
                current.append((body[3:], line))
            else:
                inner = body[3:-2]
                for offset, part in enumerate(inner.split("\n")):
                    current.append((part, line + offset))
        elif kind == COMMENT or (kind == CODE and not body.strip()):
            continue
        elif current:
            groups.append(current)
            current = []
    if current:
        groups.append(current)
    return groups


def rust_doc_blocks(items):
    """Fenced code blocks of the doc comments (fence lines included), each as (text, first line)."""
    blocks = []
    for group in rust_doc_lines(items):
        fence = None
        block = []
        first = 0
        for body, line in group:
            mark = body.strip()[:3]
            if fence is None:
                if mark in ("```", "~~~"):
                    fence = mark
                    block = [body.rstrip()]
                    first = line
            else:
                block.append(body.rstrip())
                if mark == fence:
                    blocks.append(("\n".join(block), first))
                    fence = None
        if fence is not None:
            blocks.append(("\n".join(block), first))
    return blocks


LINK_TARGETS = (
    re.compile(r"\[`([^`\]\n]+)`\]"),  # [`Type::method`]
    re.compile(r"\]\(([^)\s]+)\)"),  # [text](target)
    re.compile(r"\]\[([^\]\n]+)\]"),  # [text][target]
    # [label]: target – a label in backticks is an intra-doc link followed by a colon, not a link definition
    re.compile(r"^\s*\[(?!`)[^\]\n]+\]:\s*(\S+)"),
)


def rust_link_targets(items):
    targets = set()
    for group in rust_doc_lines(items):
        for body, _line in group:
            for pattern in LINK_TARGETS:
                targets.update(pattern.findall(body))
    return targets


# ---- TypeScript ----------------------------------------------------------------------------------------------

REGEX_KEYWORDS = {
    "return", "typeof", "case", "in", "of", "void", "delete", "throw", "new", "else", "do", "yield", "await",
    "instanceof",
}  # fmt: skip
WORD_BEFORE = re.compile(r"([A-Za-z_$][A-Za-z0-9_$]*)\s*$")


def regex_allowed(text, i):
    """Can a regular expression literal start at the `/` at i (rather than a division)?"""
    j = i - 1
    while j >= 0 and text[j] in " \t\r\n":
        j -= 1
    if j < 0:
        return True
    prev = text[j]
    if prev in ")]}":
        return False
    if re.match(r"[A-Za-z0-9_$]", prev):
        m = WORD_BEFORE.search(text, max(0, j - 20), j + 1)
        return bool(m) and m.group(1) in REGEX_KEYWORDS
    return True


def regex_end(text, i):
    """End (exclusive) of the regular expression literal that starts at i, or -1 if there is none on this line."""
    j = i + 1
    n = len(text)
    in_class = False
    while j < n and text[j] != "\n":
        c = text[j]
        if c == "\\":
            j += 2
            continue
        if c == "[":
            in_class = True
        elif c == "]":
            in_class = False
        elif c == "/" and not in_class:
            j += 1
            while j < n and re.match(r"[A-Za-z]", text[j]):
                j += 1
            return j
        j += 1
    return -1


def scan_ts(text):
    seg = Segments()
    i = 0
    n = len(text)
    start = 0
    braces = []  # one counter per open `${` of a template literal

    def flush(end):
        seg.add(CODE, text[start:end])

    def template(j):
        """Scans template text from j; returns the index after the closing backtick or after `${`."""
        while j < n:
            if text[j] == "\\":
                j += 2
            elif text[j] == "`":
                return j + 1, False
            elif text.startswith("${", j):
                return j + 2, True
            else:
                j += 1
        raise ScanError("line %d: template literal is not closed" % seg.line)

    while i < n:
        c = text[i]
        if c == "/" and text.startswith("//", i):
            flush(i)
            end = text.find("\n", i)
            end = n if end < 0 else end
            seg.add(COMMENT, text[i:end])
            i = start = end
        elif c == "/" and text.startswith("/*", i):
            flush(i)
            end = text.find("*/", i + 2)
            if end < 0:
                raise ScanError("line %d: block comment is not closed" % seg.line)
            seg.add(COMMENT, text[i : end + 2])
            i = start = end + 2
        elif c == "/" and regex_allowed(text, i) and regex_end(text, i) > 0:
            flush(i)
            end = regex_end(text, i)
            seg.add(STR, text[i:end])
            i = start = end
        elif c in "'\"":
            flush(i)
            j = i + 1
            while j < n and text[j] != c:
                if text[j] == "\n":
                    raise ScanError("line %d: string is not closed" % seg.line)
                j += 2 if text[j] == "\\" else 1
            if j >= n:
                raise ScanError("line %d: string is not closed" % seg.line)
            seg.add(STR, text[i : j + 1])
            i = start = j + 1
        elif c == "`":
            flush(i)
            end, opened = template(i + 1)
            seg.add(STR, text[i:end])
            if opened:
                braces.append(0)
            i = start = end
        elif c == "{" and braces:
            braces[-1] += 1
            i += 1
        elif c == "}" and braces:
            if braces[-1] == 0:
                braces.pop()
                flush(i)
                end, opened = template(i + 1)
                seg.add(STR, text[i:end])
                if opened:
                    braces.append(0)
                i = start = end
            else:
                braces[-1] -= 1
                i += 1
        else:
            i += 1
    flush(n)
    return seg.items


# ---- Templates -----------------------------------------------------------------------------------------------


def scan_html(text):
    seg = Segments()
    i = 0
    n = len(text)
    while i < n:
        a = text.find("{#", i)
        b = text.find("<!--", i)
        candidates = [p for p in (a, b) if p >= 0]
        if not candidates:
            break
        p = min(candidates)
        closing = "#}" if p == a else "-->"
        end = text.find(closing, p + 2)
        if end < 0:
            raise ScanError("line %d: comment is not closed" % (seg.line + text.count("\n", i, p)))
        seg.add(CODE, text[i:p])
        seg.add(COMMENT, text[p : end + len(closing)])
        i = end + len(closing)
    seg.add(CODE, text[i:])
    return seg.items


# ---- Python --------------------------------------------------------------------------------------------------


def scan_python(text):
    lines = text.split("\n")
    offsets = [0]
    for line in lines:
        offsets.append(offsets[-1] + len(line) + 1)

    def offset(position):
        return offsets[position[0] - 1] + position[1]

    spans = []  # (start, end, kind), in source order, not overlapping
    fstring_depth = 0
    fstring_start = 0
    fstring_types = {getattr(tokenize, name, -1) for name in ("FSTRING_START", "TSTRING_START")}
    fstring_ends = {getattr(tokenize, name, -1) for name in ("FSTRING_END", "TSTRING_END")}
    try:
        for token in tokenize.generate_tokens(io.StringIO(text).readline):
            if token.type in fstring_types:
                if fstring_depth == 0:
                    fstring_start = offset(token.start)
                fstring_depth += 1
            elif token.type in fstring_ends:
                fstring_depth -= 1
                if fstring_depth == 0:
                    spans.append((fstring_start, offset(token.end), STR))
            elif fstring_depth:
                continue
            elif token.type == tokenize.STRING:
                spans.append((offset(token.start), offset(token.end), STR))
            elif token.type == tokenize.COMMENT:
                first_line = token.start[0] == 1 and token.string.startswith("#!")
                if not first_line:
                    spans.append((offset(token.start), offset(token.end), COMMENT))
    except (tokenize.TokenError, IndentationError, SyntaxError) as error:
        raise ScanError("Python tokenizer: %s" % error)
    seg = Segments()
    position = 0
    for begin, end, kind in spans:
        seg.add(CODE, text[position:begin])
        seg.add(kind, text[begin:end])
        position = end
    seg.add(CODE, text[position:])
    return seg.items


# ---- Shell ---------------------------------------------------------------------------------------------------

HEREDOC = re.compile(r"<<(-?)\s*(?:'([^']+)'|\"([^\"]+)\"|\\?([A-Za-z_][A-Za-z0-9_]*))")


def shell_double_quote(text, j, line):
    """Index after the closing `"` of a double-quoted text whose content starts at j; `$(…)` inside may itself
    contain quotes."""
    n = len(text)
    while j < n:
        c = text[j]
        if c == "\\":
            j += 2
        elif c == '"':
            return j + 1
        elif text.startswith("$(", j):
            j = shell_substitution(text, j + 2, line)
        else:
            j += 1
    raise ScanError("line %d: double quote is not closed" % line)


def shell_substitution(text, j, line):
    """Index after the `)` that closes a `$(` whose content starts at j."""
    n = len(text)
    depth = 1
    while j < n:
        c = text[j]
        if c == "\\":
            j += 2
        elif c == "'":
            end = text.find("'", j + 1)
            if end < 0:
                break
            j = end + 1
        elif c == '"':
            j = shell_double_quote(text, j + 1, line)
        elif c == "(":
            depth += 1
            j += 1
        elif c == ")":
            depth -= 1
            j += 1
            if depth == 0:
                return j
        else:
            j += 1
    raise ScanError("line %d: command substitution is not closed" % line)


def scan_shell(text):
    seg = Segments()
    i = 0
    n = len(text)
    start = 0
    pending = []  # heredocs opened on the current line: (strip tabs, delimiter)
    quote_on_line = False
    line_start = True  # only whitespace so far on this line

    def flush(end):
        seg.add(CODE, text[start:end])

    while i < n:
        c = text[i]
        if c == "\n":
            i += 1
            quote_on_line = False
            line_start = True
            if pending:
                flush(i)
                start = i
                for strip_tabs, delimiter in pending:
                    while True:
                        if i >= n:
                            raise ScanError("line %d: heredoc %s is not closed" % (seg.line, delimiter))
                        end = text.find("\n", i)
                        end = n if end < 0 else end
                        body = text[i:end]
                        i = min(end + 1, n)
                        if (body.lstrip("\t") if strip_tabs else body) == delimiter:
                            break
                seg.add(STR, text[start:i])
                start = i
                pending = []
            continue
        if c in " \t":
            i += 1
            continue
        if c == "#":
            first_line = seg.line == 1 and text.startswith("#!") and i == 0
            before = text[i - 1] if i else "\n"
            if not first_line and (line_start or (before in " \t" and not quote_on_line)):
                flush(i)
                end = text.find("\n", i)
                end = n if end < 0 else end
                seg.add(COMMENT, text[i:end])
                i = start = end
                continue
            i += 1
            line_start = False
            continue
        line_start = False
        if c == "\\":
            i += 2
        elif c == "'":
            flush(i)
            end = text.find("'", i + 1)
            if end < 0:
                raise ScanError("line %d: single quote is not closed" % seg.line)
            seg.add(STR, text[i : end + 1])
            i = start = end + 1
            quote_on_line = True
        elif c == '"':
            flush(i)
            j = shell_double_quote(text, i + 1, seg.line)
            seg.add(STR, text[i:j])
            i = start = j
            quote_on_line = True
        elif c == "`":
            quote_on_line = True
            i += 1
        elif c == "<" and text.startswith("<<", i) and not text.startswith("<<<", i):
            m = HEREDOC.match(text, i)
            if m:
                pending.append((m.group(1) == "-", m.group(2) or m.group(3) or m.group(4)))
                i = m.end()
            else:
                i += 2
        else:
            i += 1
    flush(n)
    return seg.items


# ---- Comparison ----------------------------------------------------------------------------------------------

SCANNERS = {".rs": scan_rust, ".ts": scan_ts, ".html": scan_html, ".py": scan_python, ".sh": scan_shell}


def scanner_for(path):
    return SCANNERS.get(os.path.splitext(path)[1])


def code_lines(items):
    """Text without comments as (line text, source line). Whitespace at the end of a line and empty lines are
    dropped outside of strings; line breaks inside strings stay part of the line."""
    out = []
    pieces = []
    first = None

    def finish():
        nonlocal pieces, first
        while pieces and pieces[-1][0] == CODE:
            stripped = pieces[-1][1].rstrip()
            if stripped:
                pieces[-1] = (CODE, stripped)
                break
            pieces.pop()
        if pieces:
            out.append(("".join(part for _kind, part in pieces), first))
        pieces = []
        first = None

    for kind, body, line in items:
        if kind == STR:
            if first is None:
                first = line
            pieces.append((STR, body))
        elif kind == CODE:
            for offset, part in enumerate(body.split("\n")):
                if offset:
                    finish()
                if part:
                    if first is None and part.strip():
                        first = line + offset
                    pieces.append((CODE, part))
    finish()
    return out


def compare(path, base, head, limit=8):
    """Messages `path:line: …` for everything but comments that differs between base and head."""
    scan = scanner_for(path)
    if scan is None:
        return ["%s:1: file type is not checked for comment-only changes" % path]
    try:
        old = scan(base)
    except ScanError as error:
        return ["%s:1: base version cannot be read (%s)" % (path, error)]
    try:
        new = scan(head)
    except ScanError as error:
        return ["%s:1: cannot be read (%s)" % (path, error)]
    messages = []
    old_lines = code_lines(old)
    new_lines = code_lines(new)
    old_text = [text for text, _ in old_lines]
    new_text = [text for text, _ in new_lines]
    if old_text != new_text:
        matcher = difflib.SequenceMatcher(None, old_text, new_text, autojunk=False)
        for tag, a1, a2, b1, b2 in matcher.get_opcodes():
            if tag == "equal":
                continue
            if b1 < len(new_lines):
                line = new_lines[b1][1]
            else:
                line = new_lines[-1][1] if new_lines else 1
            was = " | ".join(old_text[a1:a2])[:160]
            now = " | ".join(new_text[b1:b2])[:160]
            messages.append("%s:%d: code or string changed: %r -> %r" % (path, line, was, now))
            if len(messages) >= limit:
                messages.append("%s:%d: further differences not shown" % (path, line))
                break
    if scan is scan_rust:
        old_blocks = rust_doc_blocks(old)
        new_blocks = rust_doc_blocks(new)
        if [text for text, _ in old_blocks] != [text for text, _ in new_blocks]:
            line = 1
            for index, (text, first) in enumerate(new_blocks):
                if index >= len(old_blocks) or old_blocks[index][0] != text:
                    line = first
                    break
            else:
                line = new_blocks[-1][1] if new_blocks else 1
            messages.append(
                "%s:%d: code block in a doc comment changed (%d before, %d now; doctests must stay identical)"
                % (path, line, len(old_blocks), len(new_blocks))
            )
        old_targets = rust_link_targets(old)
        new_targets = rust_link_targets(new)
        if old_targets != new_targets:
            gone = ", ".join(sorted(old_targets - new_targets)) or "-"
            added = ", ".join(sorted(new_targets - old_targets)) or "-"
            messages.append("%s:1: intra-doc link targets changed (missing: %s; new: %s)" % (path, gone, added))
    return messages


# ---- Umlauts -------------------------------------------------------------------------------------------------


def read_allow(path):
    """Entries (glob, substring) of the allow list; a malformed line is a usage error."""
    entries = []
    with open(path, encoding="utf-8") as handle:
        for number, raw in enumerate(handle, 1):
            line = raw.rstrip("\n")
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            fields = line.split("\t")
            if len(fields) != 3 or not all(field.strip() for field in fields):
                raise ValueError("%s:%d: expected `path glob<TAB>substring<TAB>reason`" % (path, number))
            entries.append((fields[0].strip(), fields[1]))
    return entries


def umlauts(path, text, allow):
    """Messages `path:line: text` for comment lines with an umlaut that no allow entry covers."""
    scan = scanner_for(path)
    if scan is None:
        return []
    try:
        items = scan(text)
    except ScanError as error:
        return ["%s:1: cannot be read (%s)" % (path, error)]
    messages = []
    for kind, body, line in items:
        if kind not in (COMMENT, DOC):
            continue
        for offset, part in enumerate(body.split("\n")):
            if not UMLAUTS.search(part):
                continue
            if any(fnmatch.fnmatch(path, glob) and needle in part for glob, needle in allow):
                continue
            messages.append("%s:%d: %s" % (path, line + offset, part.strip()))
    return messages


def walk(paths):
    for path in paths:
        if os.path.isdir(path):
            for folder, dirs, files in os.walk(path):
                dirs[:] = sorted(d for d in dirs if d not in SKIP_DIRS)
                for name in sorted(files):
                    yield os.path.join(folder, name)
        else:
            yield path


def run_umlauts(paths, allow_file):
    try:
        allow = read_allow(allow_file)
    except (OSError, ValueError) as error:
        print("check-comments-only.py: %s" % error, file=sys.stderr)
        return 2
    found = 0
    for path in walk(paths):
        if scanner_for(path) is None:
            continue
        try:
            with open(path, encoding="utf-8") as handle:
                text = handle.read()
        except (OSError, UnicodeDecodeError) as error:
            print("check-comments-only.py: %s: %s" % (path, error), file=sys.stderr)
            return 2
        shown = os.path.relpath(path).replace(os.sep, "/")
        for message in umlauts(shown, text, allow):
            print(message)
            found += 1
    return 1 if found else 0


# ---- git -----------------------------------------------------------------------------------------------------


def git(args, cwd=None):
    result = subprocess.run(["git"] + args, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        raise RuntimeError("git %s: %s" % (" ".join(args), result.stderr.decode("utf-8", "replace").strip()))
    return result.stdout


def run_compare(base, head, paths, cwd=None):
    try:
        listing = git(["diff", "--name-status", "--no-renames", "-z", base, head, "--"] + paths, cwd).decode("utf-8")
    except (RuntimeError, OSError) as error:
        print("check-comments-only.py: %s" % error, file=sys.stderr)
        return 2
    fields = [field for field in listing.split("\0") if field]
    deviations = 0
    for status, path in zip(fields[0::2], fields[1::2]):
        if status != "M":
            names = {"A": "added", "D": "deleted", "T": "type changed"}
            print("%s:1: file %s (only changed comments are allowed)" % (path, names.get(status, "status " + status)))
            deviations += 1
            continue
        try:
            old = git(["show", "%s:%s" % (base, path)], cwd).decode("utf-8")
            new = git(["show", "%s:%s" % (head, path)], cwd).decode("utf-8")
        except UnicodeDecodeError:
            print("%s:1: not UTF-8 text" % path)
            deviations += 1
            continue
        except (RuntimeError, OSError) as error:
            print("check-comments-only.py: %s" % error, file=sys.stderr)
            return 2
        for message in compare(path, old, new):
            print(message)
            deviations += 1
    return 1 if deviations else 0


# ---- Self-test -----------------------------------------------------------------------------------------------


def selftest():
    failed = []
    count = 0

    def check(what, why, ok):
        nonlocal count
        count += 1
        if not ok:
            failed.append(what)
            print("FAILED %s\n       reason for the assertion: %s" % (what, why))

    def same(what, why, path, base, head):
        messages = compare(path, base, head)
        check(what, why + " – got: %s" % messages, not messages)

    def differs(what, why, path, base, head, needle):
        messages = compare(path, base, head)
        check(what, why + " – got: %s" % messages, any(needle in m for m in messages))

    def comments(scan, text):
        return [body for kind, body, _line in scan(text) if kind in (COMMENT, DOC)]

    # Rust
    raw = 'let a = r#"// kein Kommentar "quoted" "#; // Kommentar\n'
    check(
        "rust: `//` inside a raw string is not a comment",
        "A raw string with `//` would otherwise lose its tail and a changed string would go unnoticed.",
        comments(scan_rust, raw) == ["// Kommentar"],
    )
    same(
        "rust: translating the comment behind a raw string is comment-only",
        "The comment after the string is the only thing that changed.",
        "a.rs", raw, raw.replace("// Kommentar", "// comment"),
    )  # fmt: skip
    differs(
        "rust: a changed raw string is reported",
        "Strings are compared byte for byte, also raw strings.",
        "a.rs", raw, raw.replace("kein Kommentar", "no comment"), "code or string changed",
    )  # fmt: skip
    quote = "let q = '\"'; let s = \"a\"; // Anführungszeichen\nfn f<'a>(x: &'a str) -> &'a str { x } // Lebenszeit\n"
    check(
        "rust: the character literal '\"' does not open a string, lifetimes do not open a literal",
        "Otherwise everything up to the next quote would count as a string and hide comments.",
        comments(scan_rust, quote) == ["// Anführungszeichen", "// Lebenszeit"],
    )
    check(
        "rust: escaped character literals end at the right quote",
        "'\\'' and '\\\\' must not swallow the following code.",
        comments(scan_rust, "let a = '\\''; let b = '\\\\'; let c = b'\\n'; // x\n") == ["// x"],
    )
    nested = "fn a() {} /* außen /* innen */ noch außen */ fn b() {}\n"
    check(
        "rust: nested block comments end at the outer `*/`",
        "Rust block comments nest; ending at the first `*/` would turn comment text into code.",
        comments(scan_rust, nested) == ["/* außen /* innen */ noch außen */"],
    )
    same(
        "rust: a comment may be reflowed over more or fewer lines",
        "Lines that only held a comment disappear; trailing whitespace is ignored.",
        "a.rs",
        "// eins\n// zwei\n// drei\nfn a() {} // Ende\n\nfn b() {}\n",
        "// one, two\n// three\nfn a() {} // end, but longer\nfn b() {}\n",
    )
    differs(
        "rust: a changed string is reported",
        "A translated message would change what users see.",
        "a.rs", 'fn a() -> &\'static str { "Fehler" } // x\n', 'fn a() -> &\'static str { "error" } // x\n',
        "code or string changed",
    )  # fmt: skip
    differs(
        "rust: whitespace at the end of a line inside a string counts",
        "The content of strings is compared byte for byte.",
        "a.rs", 'const A: &str = "a \nb";\n', 'const A: &str = "a\nb";\n', "code or string changed",
    )  # fmt: skip
    differs(
        "rust: changed code is reported with its line",
        "The message must lead to the place.",
        "a.rs", "// a\nfn a() {}\nfn b() { 1 }\n", "// a\nfn a() {}\nfn b() { 2 }\n", "a.rs:3:",
    )  # fmt: skip
    doctest = "/// Zählt.\n///\n/// ```\n/// assert_eq!(1, 1); // gleich\n/// ```\nfn a() {}\n"
    same(
        "rust: translating the prose of a doc comment keeps the doctest",
        "Only the text outside the fenced block changed.",
        "a.rs", doctest, doctest.replace("Zählt.", "Counts."),
    )  # fmt: skip
    differs(
        "rust: a changed doctest is reported",
        "Code blocks in doc comments are compiled and run as tests.",
        "a.rs", doctest, doctest.replace("// gleich", "// equal"), "code block in a doc comment changed",
    )  # fmt: skip
    differs(
        "rust: a removed doctest is reported",
        "The number of tests must stay the same.",
        "a.rs", doctest, "/// Counts.\nfn a() {}\n", "code block in a doc comment changed",
    )  # fmt: skip
    link = "/// Siehe [`Store::accept`] und [Plan](docs/Plan.md).\nfn a() {}\n"
    same(
        "rust: intra-doc links survive a translation of the text around them",
        "The targets are the same.",
        "a.rs", link, "/// See [`Store::accept`] and the [plan](docs/Plan.md).\nfn a() {}\n",
    )  # fmt: skip
    differs(
        "rust: a changed intra-doc link target is reported",
        "A broken link fails `cargo doc` and loses the reference.",
        "a.rs", link, link.replace("Store::accept", "Store::accepts"), "intra-doc link targets changed",
    )  # fmt: skip

    # TypeScript
    ts = "const a = `x // kein Kommentar ${b /* drin */ + `y`} z`; // Kommentar\nconst r = /\\/\\/[/]/g; // Regex\n"
    check(
        "ts: template literal and regex with `//` are not comments",
        "Otherwise the rest of the line would be dropped and a changed template would go unnoticed.",
        comments(scan_ts, ts) == ["/* drin */", "// Kommentar", "// Regex"],
    )
    same(
        "ts: translating the comments is comment-only",
        "Only comments changed.",
        "a.ts", ts, ts.replace("// Kommentar", "// comment").replace("drin", "inside").replace("// Regex", "// regex"),
    )  # fmt: skip
    differs(
        "ts: a changed template literal is reported",
        "Template literals are strings.",
        "a.ts", ts, ts.replace("kein Kommentar", "no comment"), "code or string changed",
    )  # fmt: skip
    check(
        "ts: a division is not a regular expression",
        "`a / b // c` must keep its comment.",
        comments(scan_ts, "const c = a / b; // geteilt\nconst d = (a) / 2 / b; // zwei\n") == ["// geteilt", "// zwei"],
    )

    # Templates
    html = "{# Kommentar für Entwickler #}\n<p>Text</p> <!-- HTML-Kommentar -->\n{{ wert }}\n"
    check(
        "html: `{# #}` and `<!-- -->` are comments",
        "Both kinds are translated by the comment tasks.",
        comments(scan_html, html) == ["{# Kommentar für Entwickler #}", "<!-- HTML-Kommentar -->"],
    )
    same(
        "html: translating template comments is comment-only",
        "Only comments changed.",
        "a.html", html, "{# comment for developers #}\n<p>Text</p> <!-- HTML comment -->\n{{ wert }}\n",
    )  # fmt: skip
    differs(
        "html: changed page text is reported",
        "The user interface stays German in Round 38.",
        "a.html", html, html.replace("<p>Text</p>", "<p>text</p>"), "code or string changed",
    )  # fmt: skip

    # Python
    py = '#!/usr/bin/env python3\n# Kommentar\nx = "# kein Kommentar"  # hinten\ny = f"{x} # auch nicht"\n'
    check(
        "python: `#` inside strings is not a comment, the shebang is code",
        "The shebang must stay literally; strings are compared byte for byte.",
        comments(scan_python, py) == ["# Kommentar", "# hinten"],
    )
    same(
        "python: translating comments is comment-only",
        "Only comments changed.",
        "a.py", py, py.replace("# Kommentar", "# comment").replace("# hinten", "# behind"),
    )  # fmt: skip
    differs(
        "python: a changed docstring is reported",
        "Docstrings are strings and can be visible at run time.",
        "a.py", '"""Hilfe."""\nx = 1\n', '"""Help."""\nx = 1\n', "code or string changed",
    )  # fmt: skip

    # Shell
    sh = (
        "#!/bin/sh\n# Kommentar\nx=1 # hinten\necho \"a # kein Kommentar\" # bleibt Code\ncat <<EOF\n"
        "# im Heredoc\nEOF\necho ${#x} $# # zählt\n"
    )
    check(
        "shell: heredoc lines, `#` after a quote and `$#` are not comments",
        "A heredoc line with `#` is data; after a quote the tool cannot be sure and keeps the text as code.",
        comments(scan_shell, sh) == ["# Kommentar", "# hinten", "# zählt"],
    )
    same(
        "shell: translating comments is comment-only",
        "Only comments changed.",
        "a.sh", sh, sh.replace("# Kommentar", "# comment").replace("# hinten", "# behind").replace("# zählt", "# n"),
    )  # fmt: skip
    differs(
        "shell: a changed heredoc is reported",
        "Heredoc content is data that the script writes.",
        "a.sh", sh, sh.replace("# im Heredoc", "# in the heredoc"), "code or string changed",
    )  # fmt: skip
    differs(
        "shell: a changed shebang is reported",
        "The shebang is not a comment.",
        "a.sh", sh, sh.replace("#!/bin/sh", "#!/bin/bash"), "code or string changed",
    )  # fmt: skip
    check(
        "shell: a multi-line quoted text hides its `#` lines",
        "An awk or sed program in quotes is a string, not a comment.",
        comments(scan_shell, "awk '\n# kein Kommentar\n{ print }' f\n# echt\n") == ["# echt"],
    )

    nested = 'root="$(cd "$(dirname "$0")/.." && pwd)" # Wurzel\nx="$(awk \'\n# kein "Kommentar"\n{ print }\' f)" # awk\n'
    check(
        "shell: quotes inside `$(…)` inside double quotes stay one string",
        "Otherwise the quote state flips and comments are taken for strings or the other way round.",
        comments(scan_shell, nested) == [],
    )
    check(
        "shell: after such a line the next comment is found",
        "The quote state must be balanced again at the end of the line.",
        comments(scan_shell, nested + "# danach\n") == ["# danach"],
    )

    # Other files
    differs(
        "an unknown file type is reported",
        "The tool must not wave through what it cannot read.",
        "a.sql", "-- a\n", "-- b\n", "file type is not checked",
    )  # fmt: skip

    # Umlauts
    source = '// Größe prüfen\nlet a = "Größe"; // size\n/// Zitat „Kritisch:“ bleibt wörtlich\nfn a() {}\n'
    check(
        "umlauts: comment lines with umlauts are listed, strings are not",
        "Strings of the user interface stay German; only comments are translated.",
        umlauts("crates/x/src/a.rs", source, [])
        == ["crates/x/src/a.rs:1: // Größe prüfen", "crates/x/src/a.rs:3: /// Zitat „Kritisch:“ bleibt wörtlich"],
    )
    check(
        "umlauts: an allow entry removes exactly the lines it matches",
        "A quoted German marker is a justified exception; the other line must still be listed.",
        umlauts("crates/x/src/a.rs", source, [("crates/x/*", "wörtlich")]) == ["crates/x/src/a.rs:1: // Größe prüfen"],
    )
    check(
        "umlauts: an allow entry for another path does not apply",
        "Exceptions are granted per file.",
        len(umlauts("crates/x/src/a.rs", source, [("crates/y/*", "wörtlich")])) == 2,
    )
    check(
        "umlauts: the line number is right behind a heredoc and a multi-line string",
        "The message must lead to the place.",
        umlauts("a.sh", "#!/bin/sh\ncat <<EOF\nx\ny\nEOF\nx='a\nb'\n# Größe\n", []) == ["a.sh:8: # Größe"]
        and umlauts("a.rs", 'const A: &str = "a\nb";\n/* x\n   Größe */\n', []) == ["a.rs:4: Größe */"],
    )
    check(
        "umlauts: .sql files are skipped",
        "Migrations are never changed (checksums).",
        umlauts("m/0001.sql", "-- Größe\n", []) == [],
    )

    # End to end in a throw-away repository (exit codes, added files, allow list file).
    with tempfile.TemporaryDirectory() as tmp:
        try:
            ident = ["-c", "user.name=selftest", "-c", "user.email=selftest@example.invalid", "-c", "commit.gpgsign=false"]
            git(["init", "-q", "."], tmp)

            def commit(files, message):
                for name, content in files.items():
                    with open(os.path.join(tmp, name), "w", encoding="utf-8") as handle:
                        handle.write(content)
                git(["add", "-A"], tmp)
                git(ident + ["commit", "-q", "-m", message], tmp)
                return git(["rev-parse", "HEAD"], tmp).decode().strip()

            first = commit({"a.rs": "// Größe\nfn a() {}\n", "b.sh": "#!/bin/sh\n# Länge\necho x\n"}, "base")
            second = commit({"a.rs": "// size\nfn a() {}\n", "b.sh": "#!/bin/sh\n# length\necho x\n"}, "comments")
            third = commit({"a.rs": "// size\nfn a() { }\n", "c.rs": "fn c() {}\n"}, "code")
            with open(os.devnull, "w") as null:
                stdout = sys.stdout
                sys.stdout = null
                try:
                    clean = run_compare(first, second, [], tmp)
                    dirty = run_compare(second, third, [], tmp)
                    only = run_compare(second, third, ["b.sh"], tmp)
                finally:
                    sys.stdout = stdout
            check("git: a commit that only translates comments gives exit 0", "This is the acceptance rule.", clean == 0)
            check("git: changed code and an added file give exit 1", "Both are deviations.", dirty == 1)
            check("git: a path limits the check", "Every task checks its own paths.", only == 0)
        except (RuntimeError, OSError) as error:
            check("git: throw-away repository", "git is needed for the comparison (%s)." % error, False)
        allow_path = os.path.join(tmp, "allow.txt")
        with open(allow_path, "w", encoding="utf-8") as handle:
            handle.write("# head\n\ncrates/*\twörtlich\tquoted marker\n")
        check(
            "allow list: comments and empty lines are skipped, entries are read",
            "The delivered list has only a head.",
            read_allow(allow_path) == [("crates/*", "wörtlich")],
        )
        with open(allow_path, "w", encoding="utf-8") as handle:
            handle.write("crates/* wörtlich without tabs\n")
        try:
            read_allow(allow_path)
            malformed = False
        except ValueError:
            malformed = True
        check("allow list: a line without tabs is an error", "A silently ignored entry would hide a typo.", malformed)

    print("check-comments-only.py --selftest: %d assertions, %d failed." % (count, len(failed)))
    return 1 if failed else 0


def main(argv):
    if argv == ["--selftest"]:
        return selftest()
    if argv and argv[0] == "--umlaute":
        rest = argv[1:]
        allow_file = ALLOW_FILE
        if len(rest) >= 2 and rest[0] == "--allow":
            allow_file = rest[1]
            rest = rest[2:]
        if not rest or any(arg.startswith("--") for arg in rest):
            print("usage: check-comments-only.py --umlaute [--allow FILE] PATH...", file=sys.stderr)
            return 2
        return run_umlauts(rest, allow_file)
    if len(argv) >= 2 and not any(arg.startswith("--") for arg in argv):
        return run_compare(argv[0], argv[1], argv[2:])
    print(__doc__.strip().split("\n\n")[1], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
