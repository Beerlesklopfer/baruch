#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Checks that a Typst export of Baruch can be typeset with the Typst command line (backlog „typst compile“).
#
#   scripts/ci-typst.sh BARUCH_SERVER [TYPST]
#
#   BARUCH_SERVER  built binary (e.g. target/debug/baruch-server)
#   TYPST          Typst command line. Without it, it is the own program under the name baruch-typst
#                  (symlink in the temp directory – as the packages ship it; nothing is downloaded). With a
#                  value this command line typesets (e.g. a Typst from the PATH as a cross-check). With
#                  TYPST = "-" the typesetting is dropped: only import and export are checked.
#
# Flow: import the invented work scripts/ci-data/leuchtturm.typ into a fresh data directory,
# write it into a directory with `export --out`, typeset the exported .typ file with `typst compile`.
# With the own program additionally: `--version` names the Typst version from Cargo.lock, `watch` is honestly
# refused (exit 2), and a typesetting error ends with exit 1 without an output file.
# No server, no network, no connection to a model; the data directory is a temp directory and is
# deleted at the end.
set -eu

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
    sed -n '3,11p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
fi
root="$(cd "$(dirname "$0")/.." && pwd)"
server="$1"
typst="${2:-}"
[ -x "$server" ] || { echo "ci-typst.sh: $server fehlt oder ist nicht ausführbar." >&2; exit 1; }
case "$server" in /*) ;; *) server="$(pwd)/$server" ;; esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
data="$work/daten"
out="$work/export"

own=0
if [ -z "$typst" ]; then
    # The own program as Typst: the same name and the same way (symlink) as in the packages.
    own=1
    mkdir "$work/bin"
    ln -s "$server" "$work/bin/baruch-typst"
    typst="$work/bin/baruch-typst"
elif [ "$typst" != "-" ]; then
    command -v "$typst" >/dev/null 2>&1 || { echo "ci-typst.sh: $typst fehlt." >&2; exit 1; }
fi

"$server" import "$root/scripts/ci-data/leuchtturm.typ" --title "Der Leuchtturm am Nebelkap" --data "$data" \
    > "$work/import.txt"
cat "$work/import.txt"
slug="$(sed -n 's/^Ordnername: //p' "$work/import.txt")"
[ -n "$slug" ] || { echo "ci-typst.sh: der Import nennt keinen Ordnernamen." >&2; exit 1; }
# The sample has six headings; if fewer arrived, the typesetting below would not check the whole work.
grep -qx 'Abschnitte: 6' "$work/import.txt" \
    || { echo "ci-typst.sh: erwartet waren 6 Abschnitte (scripts/ci-data/leuchtturm.typ)." >&2; exit 1; }

"$server" export "$slug" --out "$out" --data "$data"
count="$(find "$out" -maxdepth 1 -name '*.typ' | wc -l)"
[ "$count" -eq 1 ] || { echo "ci-typst.sh: erwartet war genau eine .typ-Datei in $out, gefunden: $count." >&2; exit 1; }
main="$(find "$out" -maxdepth 1 -name '*.typ')"
# The export must carry the text, not only the template.
grep -q 'einhundertvierzehn' "$main" || { echo "ci-typst.sh: der Export enthält den Text der Probe nicht." >&2; exit 1; }

if [ "$typst" = "-" ]; then
    echo "ci-typst.sh: ${main##*/} exportiert; NICHT gesetzt (keine Typst-Kommandozeile angegeben)."
    exit 0
fi
"$typst" --version
if [ "$own" = 1 ]; then
    # The line must name the version of the embedded compiler (crate `typst` in Cargo.lock).
    locked="$(awk '
        $0 == "name = \"typst\"" { hit = 1; next }
        hit && /^version = / { gsub(/[" ]/, "", $0); sub(/^version=/, "", $0); print; exit }
    ' "$root/Cargo.lock")"
    [ -n "$locked" ] || { echo "ci-typst.sh: Cargo.lock führt kein Crate typst." >&2; exit 1; }
    case "$("$typst" --version)" in
        "typst $locked (Baruch "*")") ;;
        *) echo "ci-typst.sh: --version nennt nicht typst $locked (Cargo.lock)." >&2; exit 1 ;;
    esac
fi
"$typst" compile --root "$out" "$main" "$work/leuchtturm.pdf"
# `typst compile` ends with exit ≠ 0 on a typesetting error (set -e); additionally: a PDF has really been created.
[ "$(head -c 5 "$work/leuchtturm.pdf")" = "%PDF-" ] || { echo "ci-typst.sh: Ergebnis ist kein PDF." >&2; exit 1; }
if [ "$own" = 1 ]; then
    # Cross-checks: a green typesetting above would be worthless if the program acknowledged everything with exit 0.
    rc=0
    "$typst" watch "$main" > "$work/watch.txt" 2>&1 || rc=$?
    [ "$rc" = 2 ] || { echo "ci-typst.sh: watch müsste mit Exit 2 abgelehnt werden (Exit $rc)." >&2; exit 1; }
    printf 'Der Nebel #gibt_es_nicht() hob sich.\n' > "$work/kaputt.typ"
    rc=0
    "$typst" compile "$work/kaputt.typ" "$work/kaputt.pdf" > "$work/kaputt.txt" 2>&1 || rc=$?
    [ "$rc" = 1 ] || { echo "ci-typst.sh: ein Satzfehler müsste mit Exit 1 enden (Exit $rc)." >&2; exit 1; }
    grep -q 'kaputt.typ:1:12: error: ' "$work/kaputt.txt" \
        || { echo "ci-typst.sh: die Meldung nennt nicht datei:zeile:spalte." >&2; cat "$work/kaputt.txt" >&2; exit 1; }
    [ ! -e "$work/kaputt.pdf" ] || { echo "ci-typst.sh: nach einem Satzfehler liegt eine Ausgabedatei." >&2; exit 1; }
fi
echo "ci-typst.sh: ${main##*/} gesetzt ($(wc -c < "$work/leuchtturm.pdf") Bytes PDF)."
