#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Builds the package baruch_<Version>_<Architektur>.deb with cargo-deb.
#
#   scripts/build-deb.sh [--no-build] [--out ORDNER] [further arguments for cargo deb]
#
#   --no-build   compile nothing: target/release/baruch-server must already be there
#   --out        target directory of the package (default target/debian)
#
# Before that the third-party licences are created under packaging/generated/THIRD-PARTY-LICENSES.txt and next to
# them the source code offer QUELLTEXT.txt (without them cargo-deb aborts). Machine load: nice/ionice and -j 4
# (AGENTS.md); harmless in the CI.
#
# The package sets up no service and brings no maintainer scripts (no postinst/postrm): it creates
# nothing in the data directory of the user, and removing it (also `apt purge`) deletes nothing there.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

no_build=0
out="target/debian"
while [ $# -gt 0 ]; do
    case "$1" in
        --no-build) no_build=1; shift ;;
        --out) out="$2"; shift 2 ;;
        *) break ;;
    esac
done

target="$(rustc -vV | sed -n 's/^host: //p')"
python3 scripts/third-party-licenses.py --target "$target" --out packaging/generated/THIRD-PARTY-LICENSES.txt

quiet=""
command -v ionice >/dev/null 2>&1 && quiet="ionice -c3"
if [ "$no_build" = 1 ]; then
    if [ ! -x target/release/baruch-server ]; then
        echo "build-deb.sh: target/release/baruch-server fehlt (--no-build)." >&2
        exit 1
    fi
    nice -n 10 $quiet cargo deb -p baruch-server --no-build --output "$out" "$@"
else
    if [ ! -x crates/baruch-server/web/node_modules/.bin/tsc ]; then
        (cd crates/baruch-server/web && npm ci)
    fi
    nice -n 10 $quiet cargo deb -p baruch-server --locked --output "$out" "$@" -- -j 4
fi
