#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Baut Baruch-<Version>-macos-<Architektur>.dmg mit Baruch.app (läuft nur auf macOS: sips, iconutil, hdiutil).
#
#   packaging/macos/build-dmg.sh --binary DATEI --arch arm64|x86_64 [--out ORDNER]
#
# Signieren (abschaltbar): Ist MACOS_SIGN_IDENTITY gesetzt (z. B. "Developer ID Application: Name (TEAMID)")
# und liegt das Zertifikat im Schlüsselbund, werden Programm und Abbild mit codesign signiert. Ohne die
# Variable bleibt alles unsigniert. Notarisieren ist ein eigener Schritt in release.yml.
#
# Im Programm liegen unter Contents/Resources: LICENSE, THIRD-PARTY-LICENSES.txt, QUELLTEXT.txt
# (Quellcode-Angebot mit Adresse des Repos) und README.md; LICENSE.txt und QUELLTEXT.txt stehen zusätzlich
# sichtbar im Abbild. Das Programm schreibt nichts in sich selbst: Die Daten liegen unter
# ~/Library/Application Support/baruch, und das Löschen von Baruch.app entfernt sie nicht.
#
# STAND: noch nie gelaufen (lokal kein macOS); erster Lauf in der CI. Unsigniert, solange MACOS_SIGN_IDENTITY
# fehlt.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

binary=""
arch=""
out="dist"
while [ $# -gt 0 ]; do
    case "$1" in
        --binary) binary="$2"; shift 2 ;;
        --arch) arch="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        *) echo "build-dmg.sh: unbekanntes Argument $1" >&2; exit 2 ;;
    esac
done
[ -n "$binary" ] && [ -x "$binary" ] || { echo "build-dmg.sh: --binary fehlt oder ist nicht ausführbar." >&2; exit 1; }
[ -n "$arch" ] || { echo "build-dmg.sh: --arch fehlt." >&2; exit 1; }
licenses="packaging/generated/THIRD-PARTY-LICENSES.txt"
offer="packaging/generated/QUELLTEXT.txt"
[ -f "$licenses" ] || { echo "build-dmg.sh: $licenses fehlt (scripts/third-party-licenses.py)." >&2; exit 1; }
[ -f "$offer" ] || { echo "build-dmg.sh: $offer fehlt (scripts/third-party-licenses.py)." >&2; exit 1; }

version="$(sh scripts/version.sh)"
work="target/macos-$arch"
app="$work/stage/Baruch.app"
rm -rf "$work"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$out"

install -m755 "$binary" "$app/Contents/MacOS/baruch-server"
strip -x "$app/Contents/MacOS/baruch-server" || true
install -m755 packaging/linux/baruch "$app/Contents/MacOS/baruch"
# Typst-Kommandozeile (Runde 36): nur der Name baruch-typst, als Symlink im selben Ordner; nie eine Datei „typst“.
# Aufruf: /Applications/Baruch.app/Contents/MacOS/baruch-typst compile … (docs/Verteilung.md).
ln -s baruch-server "$app/Contents/MacOS/baruch-typst"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist.in > "$app/Contents/Info.plist"
install -m644 LICENSE "$app/Contents/Resources/LICENSE"
install -m644 "$licenses" "$app/Contents/Resources/THIRD-PARTY-LICENSES.txt"
install -m644 "$offer" "$app/Contents/Resources/QUELLTEXT.txt"
install -m644 README.md "$app/Contents/Resources/README.md"

# Symbol: .icns aus dem 1024er-PNG.
iconset="$work/baruch.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" packaging/icons/baruch-1024.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" packaging/icons/baruch-1024.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/baruch.icns"

if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
    codesign --force --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$app/Contents/MacOS/baruch-server"
    codesign --force --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$app"
    codesign --verify --strict --verbose=2 "$app"
else
    echo "build-dmg.sh: MACOS_SIGN_IDENTITY nicht gesetzt – Programm bleibt unsigniert."
fi

ln -s /Applications "$work/stage/Applications"
cp LICENSE "$work/stage/LICENSE.txt"
cp "$offer" "$work/stage/QUELLTEXT.txt"
dmg="$out/Baruch-${version}-macos-${arch}.dmg"
rm -f "$dmg"
hdiutil create -volname "Baruch $version" -srcfolder "$work/stage" -ov -format UDZO "$dmg"
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$dmg"
fi
echo "DMG: $dmg"
