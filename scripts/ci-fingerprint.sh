#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Checks the published fingerprint of the APT archive key (docs/Verteilung.md, „Schlüssel für das
# APT-Repository“).
#
#   scripts/ci-fingerprint.sh [--root ORDNER] [--key FINGERABDRUCK]
#
#   --root  root of the repository (default: the directory above this script)
#   --key   fingerprint of the key that is about to sign (release.yml, job „publish“)
#
# The fingerprint stands in three places that the user fills by hand: `params.aptFingerprint` in
# site/hugo.toml (website), README.md and docs/Verteilung.md. Checked is:
#   1. If it is entered in site/hugo.toml, it is 40 hex characters (spaces allowed), README.md and
#      docs/Verteilung.md name the same one, and the placeholder no longer stands in either of the two.
#   2. With --key: it is entered and equal to the key. Reason: whoever loads the key file from the website
#      can only compare it with a fingerprint known by a second way (README in the
#      repository, source text of the packages). A signed repository without or with a wrong fingerprint would
#      not be verifiable – therefore the release then fails before anything is signed.
#
# Exit 0: entered and consistent. Exit 3: not entered yet (only without --key; the caller warns).
# Exit 1: contradictory, wrong form or (with --key) missing or another key. Exit 2: usage.
# The fingerprint is public; the script reads no key and no secret.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
key=""
with_key=0
while [ $# -gt 0 ]; do
    case "$1" in
        --root) [ $# -ge 2 ] || exit 2; root="$2"; shift 2 ;;
        --key) [ $# -ge 2 ] || exit 2; key="$2"; with_key=1; shift 2 ;;
        *) sed -n '3,9p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
    esac
done
placeholder="FINGERABDRUCK-TRÄGT-DER-NUTZER-EIN"
toml="$root/site/hugo.toml"
others="README.md docs/Verteilung.md"
[ -f "$toml" ] || { echo "ci-fingerprint.sh: $toml fehlt." >&2; exit 1; }

# Without spaces, in upper case: this way „ABCD 1234 …“ and „abcd1234…“ can be compared.
normal() { printf '%s' "$1" | tr -d ' \t' | tr 'abcdef' 'ABCDEF'; }
is_fingerprint() { printf '%s' "$1" | grep -Eq '^[0-9A-F]{40}$'; }

entered="$(awk '
    /^\[params\]/ { inside = 1; next }
    /^\[/ { inside = 0 }
    inside && /^[ \t]*aptFingerprint[ \t]*=/ {
        sub(/^[^=]*=[ \t]*"/, "", $0); sub(/".*$/, "", $0); print; exit
    }
' "$toml")"
entered="$(normal "$entered")"

if [ "$with_key" = 1 ]; then
    key="$(normal "$key")"
    is_fingerprint "$key" || { echo "ci-fingerprint.sh: --key ist kein Fingerabdruck (40 Hex-Zeichen)." >&2; exit 1; }
fi

if [ -z "$entered" ]; then
    if [ "$with_key" = 1 ]; then
        echo "ci-fingerprint.sh: Der Fingerabdruck des Archivschlüssels ist nicht eingetragen (params.aptFingerprint in site/hugo.toml, dazu $others). Gleich signiert würde mit $key – vor dem Release eintragen (docs/Verteilung.md)." >&2
        exit 1
    fi
    echo "ci-fingerprint.sh: Der Fingerabdruck des Archivschlüssels ist noch nicht eingetragen (params.aptFingerprint in site/hugo.toml, dazu $others)."
    exit 3
fi

if ! is_fingerprint "$entered"; then
    echo "ci-fingerprint.sh: params.aptFingerprint in site/hugo.toml ist kein Fingerabdruck (40 Hex-Zeichen, Leerzeichen erlaubt)." >&2
    exit 1
fi
failed=0
for file in $others; do
    path="$root/$file"
    [ -f "$path" ] || { echo "ci-fingerprint.sh: $file fehlt." >&2; failed=1; continue; }
    if grep -q "$placeholder" "$path"; then
        echo "ci-fingerprint.sh: $file enthält noch den Platzhalter $placeholder; die Webseite nennt schon $entered." >&2
        failed=1
    fi
    if ! tr -d ' \t' < "$path" | tr 'abcdef' 'ABCDEF' | grep -q "$entered"; then
        echo "ci-fingerprint.sh: $file nennt den Fingerabdruck $entered der Webseite nicht." >&2
        failed=1
    fi
done
if [ "$with_key" = 1 ] && [ "$entered" != "$key" ]; then
    echo "ci-fingerprint.sh: Eingetragen ist $entered, signiert würde mit $key. Entweder liegt der falsche Schlüssel im Secret, oder der Fingerabdruck ist nicht nachgezogen." >&2
    failed=1
fi
[ "$failed" = 0 ] || exit 1
echo "ci-fingerprint.sh: Fingerabdruck $entered stimmt in site/hugo.toml, README.md und docs/Verteilung.md überein."
