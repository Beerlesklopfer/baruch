#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Holt apt-repo.tar.gz des letzten veröffentlichten Release (Grundlage für das nächste APT-Repository in
# release.yml; in pages.yml der Stand, der unter /apt/ wieder ausgeliefert wird).
#
#   scripts/ci-apt-fetch.sh ZIELORDNER
#
# Umgebung: GH_REPO (Besitzer/Name), GH_TOKEN (nur Leserechte nötig); Werkzeug: gh.
# Ergebnis: ZIELORDNER/apt-repo.tar.gz, geprüft gegen SHA256SUMS desselben Release; das Tag steht auf der
# Standardausgabe.
#
# Exit 0  Archiv liegt vor.
# Exit 3  Es gibt kein veröffentlichtes Release mit apt-repo.tar.gz (erstes Release) – kein Fehler.
# Exit 1  alles andere (Netz, Rechte, Prüfsumme). Ein Fehler darf nie wie „kein Release“ aussehen, sonst
#         verschwände das Repository bei einer Störung von der Webseite.
#
# Grenze: Entwürfe und Vorabversionen zählen nicht; ein Token mit `contents: read` sieht Entwürfe gar nicht
# (docs/Verteilung.md).
set -eu

if [ $# -ne 1 ]; then
    sed -n '3,7p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
fi
dest="$1"
[ -n "${GH_REPO:-}" ] || { echo "ci-apt-fetch.sh: GH_REPO ist nicht gesetzt." >&2; exit 1; }
for needed in gh sha256sum python3; do
    command -v "$needed" >/dev/null 2>&1 || { echo "ci-apt-fetch.sh: $needed fehlt." >&2; exit 1; }
done

# Jüngstes veröffentlichtes Release, das das Archiv trägt (nicht bloß „latest“: ein Release ohne Archiv würde
# sonst das Repository leeren). Entwürfe und Vorabversionen zählen nicht.
if ! releases="$(gh api "repos/$GH_REPO/releases?per_page=100")"; then
    echo "ci-apt-fetch.sh: Die Liste der Releases von $GH_REPO ließ sich nicht lesen." >&2
    exit 1
fi
if ! tag="$(printf '%s' "$releases" | python3 -c '
import json, sys
found = [
    r for r in json.load(sys.stdin)
    if not r.get("draft") and not r.get("prerelease")
    and any(a.get("name") == "apt-repo.tar.gz" for a in r.get("assets") or [])
]
found.sort(key=lambda r: r.get("published_at") or "", reverse=True)
print(found[0]["tag_name"] if found else "")
')"; then
    echo "ci-apt-fetch.sh: Die Liste der Releases von $GH_REPO ist kein lesbares JSON." >&2
    exit 1
fi
if [ -z "$tag" ]; then
    echo "ci-apt-fetch.sh: kein veröffentlichtes Release mit apt-repo.tar.gz in $GH_REPO." >&2
    exit 3
fi

mkdir -p "$dest"
rm -f "$dest/apt-repo.tar.gz" "$dest/SHA256SUMS"
gh release download "$tag" --repo "$GH_REPO" --pattern apt-repo.tar.gz --pattern SHA256SUMS --dir "$dest" >&2
[ -f "$dest/apt-repo.tar.gz" ] || { echo "ci-apt-fetch.sh: apt-repo.tar.gz von $tag kam nicht an." >&2; exit 1; }
[ -f "$dest/SHA256SUMS" ] || { echo "ci-apt-fetch.sh: SHA256SUMS von $tag fehlt." >&2; exit 1; }
line="$(awk '$2 == "apt-repo.tar.gz" || $2 == "*apt-repo.tar.gz" { print $1; exit }' "$dest/SHA256SUMS")"
[ -n "$line" ] || { echo "ci-apt-fetch.sh: SHA256SUMS von $tag nennt apt-repo.tar.gz nicht." >&2; exit 1; }
if ! printf '%s  %s\n' "$line" "$dest/apt-repo.tar.gz" | sha256sum --check --status; then
    echo "ci-apt-fetch.sh: Prüfsumme von apt-repo.tar.gz ($tag) stimmt nicht." >&2
    exit 1
fi
echo "ci-apt-fetch.sh: apt-repo.tar.gz von $tag geprüft." >&2
printf '%s\n' "$tag"
