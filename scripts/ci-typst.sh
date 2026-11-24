#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Prüft, dass ein Typst-Export von Baruch mit der Typst-Kommandozeile setzbar ist (Backlog „typst compile“).
#
#   scripts/ci-typst.sh BARUCH_SERVER [TYPST]
#
#   BARUCH_SERVER  gebaute Binärdatei (z. B. target/debug/baruch-server)
#   TYPST          Typst-Kommandozeile (Vorgabe: typst aus dem PATH), in der Version des eingebetteten
#                  Compilers (scripts/ci-tool.sh typst ORDNER). Mit TYPST = "-" entfällt der Satz: nur
#                  Import und Export werden geprüft (auf Rechnern ohne Typst-Kommandozeile).
#
# Ablauf: das erfundene Werk scripts/ci-data/leuchtturm.typ in einen frischen Datenordner importieren,
# mit `export --out` in einen Ordner schreiben, die exportierte .typ-Datei mit `typst compile` setzen.
# Kein Server, kein Netz, keine Verbindung zu einem Modell; der Datenordner ist ein Temp-Ordner und wird
# am Ende gelöscht.
set -eu

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
    sed -n '3,10p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
fi
root="$(cd "$(dirname "$0")/.." && pwd)"
server="$1"
typst="${2:-typst}"
[ -x "$server" ] || { echo "ci-typst.sh: $server fehlt oder ist nicht ausführbar." >&2; exit 1; }
if [ "$typst" != "-" ]; then
    command -v "$typst" >/dev/null 2>&1 \
        || { echo "ci-typst.sh: $typst fehlt (scripts/ci-tool.sh typst ORDNER)." >&2; exit 1; }
fi
case "$server" in /*) ;; *) server="$(pwd)/$server" ;; esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
data="$work/daten"
out="$work/export"

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
"$typst" compile --root "$out" "$main" "$work/leuchtturm.pdf"
# `typst compile` endet bei einem Satzfehler mit Exit ≠ 0 (set -e); zusätzlich: Es ist wirklich ein PDF entstanden.
[ "$(head -c 5 "$work/leuchtturm.pdf")" = "%PDF-" ] || { echo "ci-typst.sh: Ergebnis ist kein PDF." >&2; exit 1; }
echo "ci-typst.sh: ${main##*/} gesetzt ($(wc -c < "$work/leuchtturm.pdf") Bytes PDF)."
