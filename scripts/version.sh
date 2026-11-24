#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Gibt die Version von Baruch aus: [workspace.package] version in Cargo.toml.
# Läuft der Bau für ein Tag v* (GITHUB_REF_NAME oder erstes Argument), muss das Tag dazu passen –
# sonst Abbruch, damit kein Paket mit falscher Nummer entsteht. Versionsnummern legt nur der Nutzer fest.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(awk '
    /^\[workspace\.package\]/ { inside = 1; next }
    /^\[/ { inside = 0 }
    inside && /^version[ \t]*=/ { gsub(/[" \t]/, "", $0); sub(/^version=/, "", $0); print; exit }
' "$root/Cargo.toml")"
if [ -z "$version" ]; then
    echo "version.sh: keine Version unter [workspace.package] in Cargo.toml gefunden." >&2
    exit 1
fi

tag="${1:-${GITHUB_REF_NAME:-}}"
case "$tag" in
    v[0-9]*)
        if [ "${tag#v}" != "$version" ]; then
            echo "version.sh: Tag $tag passt nicht zur Version $version in Cargo.toml." >&2
            exit 1
        fi
        ;;
esac
printf '%s\n' "$version"
