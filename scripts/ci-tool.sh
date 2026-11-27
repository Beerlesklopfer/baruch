#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Downloads a tool of the CI in a fixed version and checks the SHA-256 checksum before it is unpacked.
#
#   scripts/ci-tool.sh hugo ZIELORDNER
#
# Puts ZIELORDNER/hugo in place (Linux x86_64, like the runners). Without a matching checksum the script aborts;
# nothing unchecked is executed. The version stands only here (one place for ci.yml, release.yml and pages.yml).
# Raising it: docs/Verteilung.md, „Werkzeuge anheben“.
#
# - Hugo: the same version as on the development machine (not "extended", the theme needs no SCSS).
#   Checksum from hugo_<Version>_checksums.txt of the release, queried on 2026-11-24.
# - The CI has not downloaded Typst since Round 36: `typst compile` is checked by scripts/ci-typst.sh with the own
#   program (baruch-typst, built-in engine). `ci-tool.sh typst` therefore ends like every unknown tool
#   with exit 2.
#
# With CI_TOOL_CHECK_ONLY=1 the script downloads nothing: it only checks the invocation (for scripts/ci-selftest.sh,
# without network).
set -eu

HUGO_VERSION="0.168.0"
HUGO_SHA256="fada83ad56bb27ed4cb7dd81590303bdac16140f944ac4e66fc248117e2d5594"

usage() {
    sed -n '3,6p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
}
[ $# -eq 2 ] || usage
tool="$1"
dest="$2"

case "$tool" in
    hugo)
        url="https://github.com/gohugoio/hugo/releases/download/v${HUGO_VERSION}/hugo_${HUGO_VERSION}_linux-amd64.tar.gz"
        sum="$HUGO_SHA256"
        member="hugo"
        strip=0
        show="version"
        ;;
    *) usage ;;
esac

if [ "${CI_TOOL_CHECK_ONLY:-}" = 1 ]; then
    echo "ci-tool.sh: $tool – nur geprüft, nichts geladen ($url)."
    exit 0
fi

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) ;;
    *) echo "ci-tool.sh: nur für Linux x86_64 vorgesehen (hier: $(uname -s) $(uname -m))." >&2; exit 1 ;;
esac
for needed in curl sha256sum tar install; do
    command -v "$needed" >/dev/null 2>&1 || { echo "ci-tool.sh: $needed fehlt." >&2; exit 1; }
done

mkdir -p "$dest"
dest="$(cd "$dest" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
archive="$work/${url##*/}"

curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$archive" "$url"
# Check before unpacking (two spaces: the format of `sha256sum --check`).
if ! printf '%s  %s\n' "$sum" "$archive" | sha256sum --check --status; then
    echo "ci-tool.sh: Prüfsumme von ${url##*/} stimmt nicht (erwartet $sum)." >&2
    exit 1
fi
tar -xf "$archive" -C "$work" --no-same-owner --strip-components="$strip" "$member"
install -m 755 "$work/$tool" "$dest/$tool"
# The line proves in the log what runs (`hugo version`).
echo "ci-tool.sh: $dest/$tool – $("$dest/$tool" "$show" | head -n 1)"
