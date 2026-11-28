#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Baut Baruch-<Version>-<Architektur>.AppImage.
#
#   packaging/appimage/build-appimage.sh [--appdir-only] [--binary DATEI] [--out ORDNER]
#
#   --appdir-only  nur das AppDir aufbauen (lädt nichts aus dem Netz; für die Prüfung auf dem eigenen Rechner)
#   --binary       fertige Binärdatei (Vorgabe target/release/baruch-server)
#   --out          Zielordner (Vorgabe target/appimage)
#
# appimagetool und die Laufzeit (type2-runtime) werden in der CI in fester Version geladen und gegen die
# SHA-256-Prüfsummen unten geprüft; ohne passende Prüfsumme bricht das Skript ab. Die Prüfsummen stammen aus
# den Angaben der GitHub-Releases (Feld `digest`, abgefragt am 2026-11-24); beim Anheben der Version neu
# abfragen (docs/Verteilung.md, "Raising tools").
#
# Voraussetzung: packaging/generated/THIRD-PARTY-LICENSES.txt und QUELLTEXT.txt (beide von
# scripts/third-party-licenses.py). Im AppDir liegen Lizenz, Drittlizenzen und Quellcode-Angebot unter
# usr/share/doc/baruch/. Das AppImage schreibt nichts in den Datenordner außer dem, was Baruch selbst beim
# Start anlegt; löschen der Datei entfernt keine Daten.
set -euo pipefail

APPIMAGETOOL_VERSION="${APPIMAGETOOL_VERSION:-1.9.1}"
RUNTIME_VERSION="${RUNTIME_VERSION:-20251108}"
declare -A APPIMAGETOOL_SHA256=(
    [x86_64]="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
    [aarch64]="f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158"
)
declare -A RUNTIME_SHA256=(
    [x86_64]="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"
    [aarch64]="00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444"
)

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

appdir_only=0
binary="target/release/baruch-server"
out="target/appimage"
while [ $# -gt 0 ]; do
    case "$1" in
        --appdir-only) appdir_only=1; shift ;;
        --binary) binary="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        *) echo "build-appimage.sh: unbekanntes Argument $1" >&2; exit 2 ;;
    esac
done

arch="$(uname -m)"
version="$(sh scripts/version.sh)"
licenses="packaging/generated/THIRD-PARTY-LICENSES.txt"
offer="packaging/generated/QUELLTEXT.txt"
[ -x "$binary" ] || { echo "build-appimage.sh: Binärdatei $binary fehlt." >&2; exit 1; }
[ -f "$licenses" ] || { echo "build-appimage.sh: $licenses fehlt (scripts/third-party-licenses.py)." >&2; exit 1; }
[ -f "$offer" ] || { echo "build-appimage.sh: $offer fehlt (scripts/third-party-licenses.py)." >&2; exit 1; }

appdir="$out/Baruch.AppDir"
rm -rf "$appdir"
install -Dm755 "$binary" "$appdir/usr/bin/baruch-server"
if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "$appdir/usr/bin/baruch-server"
fi
install -Dm755 packaging/linux/baruch "$appdir/usr/bin/baruch"
# Typst-Kommandozeile (Runde 36): nur der Name baruch-typst, als Symlink im selben Ordner; nie eine Datei „typst“.
ln -s baruch-server "$appdir/usr/bin/baruch-typst"
install -Dm755 packaging/appimage/AppRun "$appdir/AppRun"
install -Dm644 packaging/linux/baruch.desktop "$appdir/usr/share/applications/baruch.desktop"
install -Dm644 packaging/icons/baruch.svg "$appdir/usr/share/icons/hicolor/scalable/apps/baruch.svg"
install -Dm644 packaging/icons/baruch-256.png "$appdir/usr/share/icons/hicolor/256x256/apps/baruch.png"
install -Dm644 LICENSE "$appdir/usr/share/doc/baruch/LICENSE"
install -Dm644 "$licenses" "$appdir/usr/share/doc/baruch/THIRD-PARTY-LICENSES.txt"
install -Dm644 "$offer" "$appdir/usr/share/doc/baruch/QUELLTEXT.txt"
install -Dm644 README.md "$appdir/usr/share/doc/baruch/README.md"
install -Dm644 CHANGELOG.md "$appdir/usr/share/doc/baruch/CHANGELOG.md"
# Was appimagetool an der Wurzel erwartet: .desktop, Symbol, .DirIcon.
cp packaging/linux/baruch.desktop "$appdir/baruch.desktop"
cp packaging/icons/baruch-256.png "$appdir/baruch.png"
ln -s baruch.png "$appdir/.DirIcon"
# Version im Desktop-Eintrag (appimagetool übernimmt sie in den Dateinamen der Aktualisierungsangabe).
printf 'X-AppImage-Version=%s\n' "$version" >> "$appdir/baruch.desktop"

echo "AppDir: $appdir (Version $version, $arch)"
if [ "$appdir_only" = 1 ]; then
    exit 0
fi

tool_sum="${APPIMAGETOOL_SHA256[$arch]:-}"
runtime_sum="${RUNTIME_SHA256[$arch]:-}"
if [ -z "$tool_sum" ] || [ -z "$runtime_sum" ]; then
    echo "build-appimage.sh: keine Prüfsummen für $arch hinterlegt." >&2
    exit 1
fi

tools="$out/tools"
mkdir -p "$tools"
fetch() { # fetch URL DATEI SHA256
    curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 --output "$2" "$1"
    echo "$3  $2" | sha256sum --check --strict -
}
fetch "https://github.com/AppImage/appimagetool/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-${arch}.AppImage" \
    "$tools/appimagetool" "$tool_sum"
fetch "https://github.com/AppImage/type2-runtime/releases/download/${RUNTIME_VERSION}/runtime-${arch}" \
    "$tools/runtime" "$runtime_sum"
chmod +x "$tools/appimagetool"

target="$out/Baruch-${version}-${arch}.AppImage"
# Ohne FUSE (CI-Runner) entpackt sich appimagetool selbst.
ARCH="$arch" APPIMAGE_EXTRACT_AND_RUN=1 "$tools/appimagetool" --no-appstream \
    --runtime-file "$tools/runtime" "$appdir" "$target"
chmod +x "$target"
echo "AppImage: $target"
