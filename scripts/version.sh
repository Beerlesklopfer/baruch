#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Prints the version of Baruch: [workspace.package] version in Cargo.toml.
# If the build runs for a tag v* (GITHUB_REF_NAME or first argument), the tag must match it –
# otherwise abort so that no package with a wrong number is created. Only the user sets version numbers.
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
