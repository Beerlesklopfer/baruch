#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Erzeugt ein statisches APT-Repository (ausgeliefert unter https://baruch.bernau.family/apt/).
#
#   scripts/apt-repo.sh [--base ARCHIV.tar.gz] ZIELORDNER PAKET.deb [PAKET.deb …]
#
#   --base   früherer Stand des Repositorys (apt-repo.tar.gz eines Release): Seine Pakete unter pool/ werden
#            übernommen, damit ältere Versionen installierbar bleiben. Übernommen werden nur echte .deb-Dateien
#            (keine Verweise, nichts außerhalb von pool/); dists/ und der Schlüssel entstehen immer neu.
#
# Aufbau:
#   ZIELORDNER/pool/main/b/baruch/*.deb
#   ZIELORDNER/dists/stable/main/binary-{amd64,arm64}/Packages{,.gz}
#   ZIELORDNER/dists/stable/Release            (mit Prüfsummen)
#   ZIELORDNER/dists/stable/InRelease, Release.gpg   (nur mit Schlüssel)
#   ZIELORDNER/baruch-archive-keyring.gpg      (öffentlicher Schlüssel, nur mit Schlüssel)
#
# Signieren: APT_GPG_KEY auf den Fingerabdruck (oder die Schlüssel-ID) eines geheimen Schlüssels setzen, der im
# GnuPG-Schlüsselbund (GNUPGHOME) liegt. Eine Passphrase kommt aus der Datei APT_GPG_PASSPHRASE_FILE (nie als
# Argument). Ohne APT_GPG_KEY entsteht ein unsigniertes Repository (nur zur Probe; apt lehnt es ohne
# [trusted=yes] ab). Das Skript erzeugt nie einen Schlüssel.
#
# Vorhandene Pakete im ZIELORDNER/pool bleiben liegen und werden mit aufgenommen. Ein neues Paket ersetzt ein
# gleichnamiges aus --base (derselbe Dateiname = dieselbe Version).
set -eu

SUITE="${APT_SUITE:-stable}"
COMPONENT="main"
ARCHS="amd64 arm64"
ORIGIN="Baruch"
LABEL="Baruch"

usage() {
    sed -n '3,9p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
}

base=""
if [ "${1:-}" = "--base" ]; then
    [ $# -ge 2 ] || usage
    base="$2"
    shift 2
fi
[ $# -ge 2 ] || usage
repo="$1"
shift

for tool in apt-ftparchive dpkg-deb gzip; do
    command -v "$tool" >/dev/null 2>&1 || { echo "apt-repo.sh: $tool fehlt." >&2; exit 1; }
done

mkdir -p "$repo"
repo="$(cd "$repo" && pwd)"

# Legt ein Paket nach pool/<Komponente>/<Anfangsbuchstabe>/<Name>/; der Ort folgt aus den Feldern des Pakets,
# nicht aus einem Pfad im Archiv.
add_deb() {
    deb="$1"
    name="$(dpkg-deb -f "$deb" Package)"
    arch="$(dpkg-deb -f "$deb" Architecture)"
    case "$name" in
        "" | *[!a-z0-9+.-]*) echo "apt-repo.sh: Paketname von $deb ist ungültig." >&2; exit 1 ;;
    esac
    case " $ARCHS all " in
        *" $arch "*) ;;
        *) echo "apt-repo.sh: Architektur $arch von $deb ist nicht vorgesehen ($ARCHS)." >&2; exit 1 ;;
    esac
    prefix="$(printf '%s' "$name" | cut -c1)"
    mkdir -p "$repo/pool/$COMPONENT/$prefix/$name"
    cp -f "$deb" "$repo/pool/$COMPONENT/$prefix/$name/"
}

if [ -n "$base" ]; then
    [ -f "$base" ] || { echo "apt-repo.sh: $base nicht gefunden." >&2; exit 1; }
    command -v tar >/dev/null 2>&1 || { echo "apt-repo.sh: tar fehlt." >&2; exit 1; }
    old="$(mktemp -d)"
    trap 'rm -rf "$old"' EXIT
    tar -xzf "$base" -C "$old" --no-same-owner --no-same-permissions
    taken=0
    if [ -d "$old/pool" ]; then
        # -type f folgt keinen Verweisen; die Liste steht in einer Datei, damit ein Fehler in add_deb abbricht.
        find "$old/pool" -type f -name '*.deb' | sort > "$old/list"
        while IFS= read -r deb; do
            add_deb "$deb"
            taken=$((taken + 1))
        done < "$old/list"
    fi
    echo "apt-repo.sh: $taken Paket(e) aus $base übernommen."
    rm -rf "$old"
    trap - EXIT
fi

for deb in "$@"; do
    [ -f "$deb" ] || { echo "apt-repo.sh: $deb nicht gefunden." >&2; exit 1; }
    add_deb "$deb"
done

cd "$repo"
for arch in $ARCHS; do
    dir="dists/$SUITE/$COMPONENT/binary-$arch"
    mkdir -p "$dir"
    # Leere Liste für Architekturen ohne Paket: apt meldet dann keinen Fehler 404.
    apt-ftparchive --arch "$arch" packages "pool/$COMPONENT" > "$dir/Packages"
    gzip -9 -n -c "$dir/Packages" > "$dir/Packages.gz"
done

rm -f "dists/$SUITE/Release" "dists/$SUITE/Release.gpg" "dists/$SUITE/InRelease"
apt-ftparchive \
    -o "APT::FTPArchive::Release::Origin=$ORIGIN" \
    -o "APT::FTPArchive::Release::Label=$LABEL" \
    -o "APT::FTPArchive::Release::Suite=$SUITE" \
    -o "APT::FTPArchive::Release::Codename=$SUITE" \
    -o "APT::FTPArchive::Release::Architectures=$ARCHS" \
    -o "APT::FTPArchive::Release::Components=$COMPONENT" \
    -o "APT::FTPArchive::Release::Description=Baruch – freie Schreibwerkstatt (https://baruch.bernau.family/)" \
    release "dists/$SUITE" > "dists/$SUITE/Release.tmp"
mv "dists/$SUITE/Release.tmp" "dists/$SUITE/Release"

if [ -n "${APT_GPG_KEY:-}" ]; then
    command -v gpg >/dev/null 2>&1 || { echo "apt-repo.sh: gpg fehlt." >&2; exit 1; }
    set -- --batch --yes --local-user "$APT_GPG_KEY" --digest-algo SHA512
    if [ -n "${APT_GPG_PASSPHRASE_FILE:-}" ]; then
        set -- "$@" --pinentry-mode loopback --passphrase-file "$APT_GPG_PASSPHRASE_FILE"
    fi
    gpg "$@" --clearsign --output "dists/$SUITE/InRelease" "dists/$SUITE/Release"
    gpg "$@" --armor --detach-sign --output "dists/$SUITE/Release.gpg" "dists/$SUITE/Release"
    # Öffentlicher Schlüssel im Binärformat, wie ihn `Signed-By:` erwartet.
    gpg --batch --yes --export "$APT_GPG_KEY" > baruch-archive-keyring.gpg
    echo "apt-repo.sh: signiert mit $APT_GPG_KEY (InRelease, Release.gpg, baruch-archive-keyring.gpg)."
else
    echo "apt-repo.sh: WARNUNG – kein APT_GPG_KEY gesetzt, Repository ist unsigniert (nur zur Probe)." >&2
fi

echo "apt-repo.sh: Repository in $repo"
