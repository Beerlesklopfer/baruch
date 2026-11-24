#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Prüft den veröffentlichten Fingerabdruck des APT-Archivschlüssels (docs/Verteilung.md, „Schlüssel für das
# APT-Repository“).
#
#   scripts/ci-fingerprint.sh [--root ORDNER] [--key FINGERABDRUCK]
#
#   --root  Wurzel des Repositorys (Vorgabe: der Ordner über diesem Skript)
#   --key   Fingerabdruck des Schlüssels, mit dem gleich signiert wird (release.yml, Job „publish“)
#
# Der Fingerabdruck steht an drei Stellen, die der Nutzer von Hand füllt: `params.aptFingerprint` in
# site/hugo.toml (Webseite), README.md und docs/Verteilung.md. Geprüft wird:
#   1. Ist er in site/hugo.toml eingetragen, sind es 40 Hex-Zeichen (Leerzeichen erlaubt), README.md und
#      docs/Verteilung.md nennen denselben, und der Platzhalter steht in keiner der beiden mehr.
#   2. Mit --key: Er ist eingetragen und gleich dem Schlüssel. Grund: Wer die Schlüsseldatei von der Webseite
#      lädt, kann sie nur mit einem Fingerabdruck vergleichen, den er auf einem zweiten Weg kennt (README im
#      Repository, Quelltext der Pakete). Ein signiertes Repository ohne oder mit falschem Fingerabdruck wäre
#      nicht prüfbar – darum scheitert dann das Release, bevor etwas signiert ist.
#
# Exit 0: eingetragen und stimmig. Exit 3: noch nicht eingetragen (nur ohne --key; der Aufrufer warnt).
# Exit 1: widersprüchlich, falsche Form oder (mit --key) fehlend bzw. ein anderer Schlüssel. Exit 2: Aufruf.
# Der Fingerabdruck ist öffentlich; das Skript liest keinen Schlüssel und kein Geheimnis.
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

# Ohne Leerzeichen, in Großbuchstaben: So lassen sich „ABCD 1234 …“ und „abcd1234…“ vergleichen.
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
