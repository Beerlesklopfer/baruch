#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Baut die Webseite (Hugo) und prüft sie; auf Wunsch kommt das APT-Repository nach <Ausgabe>/apt/.
#
#   scripts/ci-site.sh [--site ORDNER] [--apt ARCHIV.tar.gz]
#
#   --site  Quelle der Seite (Vorgabe: site im Repository)
#   --apt   apt-repo.tar.gz (aus release.yml bzw. scripts/ci-apt-fetch.sh): wird nach public/apt/ entpackt
#
# Prüfungen:
#   1. params.version in hugo.toml = Version in Cargo.toml (scripts/version.sh) – sonst wirbt die Seite für
#      eine Version, die es nicht gibt, und die Download-Links zeigen ins Leere.
#   2. `hugo --minify --panicOnWarning -s ORDNER` ohne Warnung.
#   3. public/index.html liegt vor; public/CNAME nennt genau baruch.bernau.family (GitHub Pages).
#   4. mit --apt: public/apt/dists/stable/Release und mindestens ein Paket unter public/apt/pool/.
#   5. Warnung (kein Abbruch), solange Platzhalter ausgeliefert würden: `aptFingerprint` in hugo.toml leer
#      (die Seite Installation zeigt dann keinen Fingerabdruck) oder eine Seite mit `platzhalter: true` im
#      Kopf (Impressum). Beides trägt nur der Nutzer ein; ein Abbruch verhinderte die Generalprobe.
#
# Die Ausgabe ORDNER/public wird vorher gelöscht (Bauergebnis, nicht eingecheckt). HUGO nennt eine andere
# Binärdatei als `hugo` aus dem PATH.
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
if [ "$(cat "$site/public/CNAME" 2>/dev/null)" != "baruch.bernau.family" ]; then
    echo "ci-site.sh: public/CNAME muss genau baruch.bernau.family enthalten (site/static/CNAME)." >&2
    exit 1
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
