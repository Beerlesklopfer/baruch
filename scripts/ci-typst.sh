#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Prüft, dass ein Typst-Export von Baruch mit der Typst-Kommandozeile setzbar ist (Backlog „typst compile“).
#
#   scripts/ci-typst.sh BARUCH_SERVER [TYPST]
#
#   BARUCH_SERVER  gebaute Binärdatei (z. B. target/debug/baruch-server)
#   TYPST          Typst-Kommandozeile. Ohne Angabe ist es das eigene Programm unter dem Namen baruch-typst
#                  (Symlink im Temp-Ordner – so wie die Pakete es ausliefern; nichts wird geladen). Mit einer
#                  Angabe setzt diese Kommandozeile (z. B. ein Typst aus dem PATH als Gegenprobe). Mit
#                  TYPST = "-" entfällt der Satz: nur Import und Export werden geprüft.
#
# Ablauf: das erfundene Werk scripts/ci-data/leuchtturm.typ in einen frischen Datenordner importieren,
# mit `export --out` in einen Ordner schreiben, die exportierte .typ-Datei mit `typst compile` setzen.
# Mit dem eigenen Programm zusätzlich: `--version` nennt die Typst-Version aus Cargo.lock, `watch` wird ehrlich
# abgelehnt (Exit 2), und ein Satzfehler endet mit Exit 1 ohne Ausgabedatei.
# Kein Server, kein Netz, keine Verbindung zu einem Modell; der Datenordner ist ein Temp-Ordner und wird
# am Ende gelöscht.
set -eu

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
    sed -n '3,11p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
fi
root="$(cd "$(dirname "$0")/.." && pwd)"
server="$1"
typst="${2:-}"
[ -x "$server" ] || { echo "ci-typst.sh: $server fehlt oder ist nicht ausführbar." >&2; exit 1; }
case "$server" in /*) ;; *) server="$(pwd)/$server" ;; esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
data="$work/daten"
out="$work/export"

own=0
if [ -z "$typst" ]; then
    # Das eigene Programm als Typst: derselbe Name und derselbe Weg (Symlink) wie in den Paketen.
    own=1
    mkdir "$work/bin"
    ln -s "$server" "$work/bin/baruch-typst"
    typst="$work/bin/baruch-typst"
elif [ "$typst" != "-" ]; then
    command -v "$typst" >/dev/null 2>&1 || { echo "ci-typst.sh: $typst fehlt." >&2; exit 1; }
fi

"$server" import "$root/scripts/ci-data/leuchtturm.typ" --title "Der Leuchtturm am Nebelkap" --data "$data" \
    > "$work/import.txt"
cat "$work/import.txt"
slug="$(sed -n 's/^Ordnername: //p' "$work/import.txt")"
[ -n "$slug" ] || { echo "ci-typst.sh: der Import nennt keinen Ordnernamen." >&2; exit 1; }
# Die Probe hat sechs Überschriften; kämen weniger an, prüfte der Satz unten nicht das ganze Werk.
grep -qx 'Abschnitte: 6' "$work/import.txt" \
    || { echo "ci-typst.sh: erwartet waren 6 Abschnitte (scripts/ci-data/leuchtturm.typ)." >&2; exit 1; }

"$server" export "$slug" --out "$out" --data "$data"
count="$(find "$out" -maxdepth 1 -name '*.typ' | wc -l)"
[ "$count" -eq 1 ] || { echo "ci-typst.sh: erwartet war genau eine .typ-Datei in $out, gefunden: $count." >&2; exit 1; }
main="$(find "$out" -maxdepth 1 -name '*.typ')"
# Der Export muss den Text tragen, nicht nur die Vorlage.
grep -q 'einhundertvierzehn' "$main" || { echo "ci-typst.sh: der Export enthält den Text der Probe nicht." >&2; exit 1; }

if [ "$typst" = "-" ]; then
    echo "ci-typst.sh: ${main##*/} exportiert; NICHT gesetzt (keine Typst-Kommandozeile angegeben)."
    exit 0
fi
"$typst" --version
if [ "$own" = 1 ]; then
    # Die Zeile muss die Version des eingebetteten Compilers nennen (Crate `typst` in Cargo.lock).
    locked="$(awk '
        $0 == "name = \"typst\"" { hit = 1; next }
        hit && /^version = / { gsub(/[" ]/, "", $0); sub(/^version=/, "", $0); print; exit }
    ' "$root/Cargo.lock")"
    [ -n "$locked" ] || { echo "ci-typst.sh: Cargo.lock führt kein Crate typst." >&2; exit 1; }
    case "$("$typst" --version)" in
        "typst $locked (Baruch "*")") ;;
        *) echo "ci-typst.sh: --version nennt nicht typst $locked (Cargo.lock)." >&2; exit 1 ;;
    esac
fi
"$typst" compile --root "$out" "$main" "$work/leuchtturm.pdf"
# `typst compile` endet bei einem Satzfehler mit Exit ≠ 0 (set -e); zusätzlich: Es ist wirklich ein PDF entstanden.
[ "$(head -c 5 "$work/leuchtturm.pdf")" = "%PDF-" ] || { echo "ci-typst.sh: Ergebnis ist kein PDF." >&2; exit 1; }
if [ "$own" = 1 ]; then
    # Gegenproben: Ein grüner Satz oben wäre wertlos, wenn das Programm alles mit Exit 0 quittierte.
    rc=0
    "$typst" watch "$main" > "$work/watch.txt" 2>&1 || rc=$?
    [ "$rc" = 2 ] || { echo "ci-typst.sh: watch müsste mit Exit 2 abgelehnt werden (Exit $rc)." >&2; exit 1; }
    printf 'Der Nebel #gibt_es_nicht() hob sich.\n' > "$work/kaputt.typ"
    rc=0
    "$typst" compile "$work/kaputt.typ" "$work/kaputt.pdf" > "$work/kaputt.txt" 2>&1 || rc=$?
    [ "$rc" = 1 ] || { echo "ci-typst.sh: ein Satzfehler müsste mit Exit 1 enden (Exit $rc)." >&2; exit 1; }
    grep -q 'kaputt.typ:1:12: error: ' "$work/kaputt.txt" \
        || { echo "ci-typst.sh: die Meldung nennt nicht datei:zeile:spalte." >&2; cat "$work/kaputt.txt" >&2; exit 1; }
    [ ! -e "$work/kaputt.pdf" ] || { echo "ci-typst.sh: nach einem Satzfehler liegt eine Ausgabedatei." >&2; exit 1; }
fi
echo "ci-typst.sh: ${main##*/} gesetzt ($(wc -c < "$work/leuchtturm.pdf") Bytes PDF)."
