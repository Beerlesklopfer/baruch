#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Lädt ein Werkzeug der CI in fester Version und prüft die SHA-256-Prüfsumme, bevor es entpackt wird.
#
#   scripts/ci-tool.sh hugo ZIELORDNER
#
# Legt ZIELORDNER/hugo ab (Linux x86_64, wie die Runner). Ohne passende Prüfsumme bricht das Skript ab; nichts
# Ungeprüftes wird ausgeführt. Die Version steht nur hier (eine Stelle für ci.yml, release.yml und pages.yml).
# Anheben: docs/Verteilung.md, „Werkzeuge anheben“.
#
# - Hugo: dieselbe Version wie auf dem Entwicklungsrechner (nicht „extended“, das Theme braucht kein SCSS).
#   Prüfsumme aus hugo_<Version>_checksums.txt des Release, abgefragt am 2026-11-24.
# - Typst lädt die CI seit Runde 36 nicht mehr: `typst compile` prüft scripts/ci-typst.sh mit dem eigenen
#   Programm (baruch-typst, eingebaute Engine). `ci-tool.sh typst` endet darum wie jedes unbekannte Werkzeug
#   mit Exit 2.
#
# Mit CI_TOOL_CHECK_ONLY=1 lädt das Skript nichts: Es prüft nur den Aufruf (für scripts/ci-selftest.sh, ohne
# Netz).
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
# Prüfen vor dem Entpacken (zwei Leerzeichen: das Format von `sha256sum --check`).
if ! printf '%s  %s\n' "$sum" "$archive" | sha256sum --check --status; then
    echo "ci-tool.sh: Prüfsumme von ${url##*/} stimmt nicht (erwartet $sum)." >&2
    exit 1
fi
tar -xf "$archive" -C "$work" --no-same-owner --strip-components="$strip" "$member"
install -m 755 "$work/$tool" "$dest/$tool"
# Die Zeile belegt im Protokoll, was läuft (`hugo version`).
echo "ci-tool.sh: $dest/$tool – $("$dest/$tool" "$show" | head -n 1)"
