#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Baut das Paket baruch_<Version>_<Architektur>.deb mit cargo-deb.
#
#   scripts/build-deb.sh [--no-build] [--out ORDNER] [weitere Argumente für cargo deb]
#
#   --no-build   nichts kompilieren: target/release/baruch-server muss schon liegen
#   --out        Zielordner des Pakets (Vorgabe target/debian)
#
# Vorher entstehen die Drittlizenzen unter packaging/generated/THIRD-PARTY-LICENSES.txt und daneben das
# Quellcode-Angebot QUELLTEXT.txt (ohne sie bricht cargo-deb ab). Rechnerlast: nice/ionice und -j 4
# (AGENTS.md); in der CI unschädlich.
#
# Das Paket richtet keinen Dienst ein und bringt keine Wartungsskripte mit (kein postinst/postrm): Es legt
# nichts im Datenordner des Benutzers an, und das Entfernen (auch `apt purge`) löscht dort nichts.
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
