#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Fetches apt-repo.tar.gz of the last published release (basis for the next APT repository in
# release.yml; in pages.yml the state that is served again under /apt/).
#
#   scripts/ci-apt-fetch.sh ZIELORDNER
#
# Environment: GH_REPO (owner/name), GH_TOKEN (only read rights needed); tool: gh.
# Result: ZIELORDNER/apt-repo.tar.gz, checked against SHA256SUMS of the same release; the tag is printed on
# standard output.
#
# Exit 0  the archive is there.
# Exit 3  there is no published release with apt-repo.tar.gz (first release) – not an error.
# Exit 1  everything else (network, rights, checksum). An error must never look like "no release", otherwise
#         the repository would disappear from the website on a disruption.
#
# Limit: drafts and pre-releases do not count; a token with `contents: read` does not see drafts at all
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

# Youngest published release that carries the archive (not merely "latest": a release without the archive would
# otherwise empty the repository). Drafts and pre-releases do not count.
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
