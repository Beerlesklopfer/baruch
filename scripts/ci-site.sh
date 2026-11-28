#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Builds the website (Hugo) and checks it; on request the APT repository goes to <Ausgabe>/apt/.
#
#   scripts/ci-site.sh [--site ORDNER] [--apt ARCHIV.tar.gz]
#
#   --site  source of the site (default: site in the repository)
#   --apt   apt-repo.tar.gz (from release.yml or scripts/ci-apt-fetch.sh): is unpacked to public/apt/
#
# Checks:
#   1. params.version in hugo.toml = version in Cargo.toml (scripts/version.sh) – otherwise the site advertises
#      a version that does not exist, and the download links point into the void.
#   2. `hugo --minify --panicOnWarning -s ORDNER` without a warning.
#   3. public/index.html is there; public/CNAME names exactly baruch.bernau.family (GitHub Pages). For every
#      further language with content (content/en, Round 39b) public/<lang>/index.html is there as well – otherwise
#      hugo.toml lacks [languages.<lang>] and the English pages were silently not built.
#   3a. scripts/site-check.py over public/ (before unpacking /apt/): no dead internal links and anchors,
#      no image without alt text, no JavaScript, no foreign address for style, font or image, no link to
#      docs/Bedienung.md or docs/Betrieb.md at GitHub. If the site has a section content/hilfe (multilingual
#      content/de/hilfe), /bedienung/ must redirect to /hilfe/; without it (trial in ci-selftest.sh) only this check is
#      dropped. One run covers both languages (German in the root, English under public/en/): <html lang> per
#      language, pictures of English pages from /bilder/en/, every language switch resolves, /bedienung/ only German.
#   The hugo build itself stops for a help picture (hilfebild) or a screenshot (bild) that is missing in the
#   language of the page (errorf in the shortcodes, Round 39b).
#   4. with --apt: public/apt/dists/stable/Release and at least one package under public/apt/pool/.
#   5. Warning (no abort) as long as placeholders would be served: `aptFingerprint` in hugo.toml empty
#      (the page Installation then shows no fingerprint) or a page with `platzhalter: true` in the
#      header (Impressum). Only the user enters both; an abort would prevent the dress rehearsal.
#
# The output ORDNER/public is deleted beforehand (build result, not checked in). HUGO names a binary other
# than `hugo` from the PATH, PYTHON one other than `python3` (for site-check.py).
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
site="$root/site"
apt=""
while [ $# -gt 0 ]; do
    case "$1" in
        --site) [ $# -ge 2 ] || exit 2; site="$2"; shift 2 ;;
        --apt) [ $# -ge 2 ] || exit 2; apt="$2"; shift 2 ;;
        *) sed -n '3,8p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
    esac
done
hugo="${HUGO:-hugo}"
command -v "$hugo" >/dev/null 2>&1 || { echo "ci-site.sh: $hugo fehlt (scripts/ci-tool.sh hugo ORDNER)." >&2; exit 1; }
python="${PYTHON:-python3}"
command -v "$python" >/dev/null 2>&1 || { echo "ci-site.sh: $python fehlt (für scripts/site-check.py)." >&2; exit 1; }
[ -f "$site/hugo.toml" ] || { echo "ci-site.sh: $site/hugo.toml fehlt." >&2; exit 1; }
if [ -n "$apt" ]; then
    [ -f "$apt" ] || { echo "ci-site.sh: $apt nicht gefunden." >&2; exit 1; }
    case "$apt" in /*) ;; *) apt="$(pwd)/$apt" ;; esac
fi

version="$(sh "$root/scripts/version.sh")"
site_version="$(awk '
    /^\[params\]/ { inside = 1; next }
    /^\[/ { inside = 0 }
    inside && /^[ \t]*version[ \t]*=/ { gsub(/[" \t]/, "", $0); sub(/^version=/, "", $0); print; exit }
' "$site/hugo.toml")"
if [ "$site_version" != "$version" ]; then
    echo "ci-site.sh: [params] version in $site/hugo.toml ist „${site_version}“, Cargo.toml führt $version." >&2
    exit 1
fi

rm -rf "$site/public"
"$hugo" --minify --panicOnWarning -s "$site"

[ -s "$site/public/index.html" ] || { echo "ci-site.sh: public/index.html fehlt oder ist leer." >&2; exit 1; }
for lang in en; do
    if [ -d "$site/content/$lang" ] && [ ! -s "$site/public/$lang/index.html" ]; then
        echo "ci-site.sh: content/$lang ist da, aber public/$lang/index.html fehlt ([languages.$lang] in hugo.toml?)." >&2
        exit 1
    fi
done
if [ "$(cat "$site/public/CNAME" 2>/dev/null)" != "baruch.bernau.family" ]; then
    echo "ci-site.sh: public/CNAME muss genau baruch.bernau.family enthalten (site/static/CNAME)." >&2
    exit 1
fi

if [ -d "$site/content/hilfe" ] || [ -d "$site/content/de/hilfe" ]; then
    "$python" "$root/scripts/site-check.py" "$site/public"
else
    "$python" "$root/scripts/site-check.py" --ohne-hilfe "$site/public"
fi

fingerprint="$(awk '
    /^\[params\]/ { inside = 1; next }
    /^\[/ { inside = 0 }
    inside && /^[ \t]*aptFingerprint[ \t]*=/ { gsub(/[" \t]/, "", $0); sub(/^aptFingerprint=/, "", $0); print; exit }
' "$site/hugo.toml")"
if [ -z "$fingerprint" ]; then
    echo "::warning::Platzhalter: Der Fingerabdruck des Archivschlüssels fehlt (params.aptFingerprint in hugo.toml); die Seite Installation zeigt einen Platzhalter-Hinweis."
fi
if [ -d "$site/content" ]; then
    for page in $(grep -rlE '^platzhalter:[ \t]*true[ \t]*$' "$site/content" 2>/dev/null | sort); do
        echo "::warning::Platzhalter: ${page#"$site"/} ist als Platzhalter markiert (platzhalter: true) und würde so ausgeliefert."
    done
fi

if [ -n "$apt" ]; then
    if [ -e "$site/public/apt" ]; then
        echo "ci-site.sh: Die Seite bringt selbst einen Ordner apt/ mit; dort liegt nur das Repository." >&2
        exit 1
    fi
    mkdir -p "$site/public/apt"
    tar -xzf "$apt" -C "$site/public/apt" --no-same-owner --no-same-permissions
    [ -s "$site/public/apt/dists/stable/Release" ] \
        || { echo "ci-site.sh: apt/dists/stable/Release fehlt im Archiv." >&2; exit 1; }
    debs="$(find "$site/public/apt/pool" -type f -name '*.deb' 2>/dev/null | wc -l)"
    [ "$debs" -ge 1 ] || { echo "ci-site.sh: Das Archiv enthält kein Paket unter pool/." >&2; exit 1; }
    if [ ! -s "$site/public/apt/dists/stable/InRelease" ]; then
        echo "::warning::Das APT-Repository unter /apt/ ist unsigniert (InRelease fehlt); apt lehnt es ab."
    fi
    echo "ci-site.sh: /apt/ mit $debs Paket(en)."
else
    echo "ci-site.sh: Seite ohne /apt/ gebaut."
fi
echo "ci-site.sh: $site/public (Version $version)"
