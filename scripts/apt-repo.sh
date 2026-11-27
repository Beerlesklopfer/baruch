#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Creates a static APT repository (served under https://baruch.bernau.family/apt/).
#
#   scripts/apt-repo.sh [--base ARCHIV.tar.gz] ZIELORDNER PAKET.deb [PAKET.deb …]
#
#   --base   earlier state of the repository (apt-repo.tar.gz of a release): its packages under pool/ are
#            taken over so that older versions stay installable. Only real .deb files are taken over
#            (no links, nothing outside pool/); dists/ and the key are always created anew.
#
# Layout:
#   ZIELORDNER/pool/main/b/baruch/*.deb
#   ZIELORDNER/dists/stable/main/binary-{amd64,arm64}/Packages{,.gz}
#   ZIELORDNER/dists/stable/Release            (with checksums)
#   ZIELORDNER/dists/stable/InRelease, Release.gpg   (only with a key)
#   ZIELORDNER/baruch-archive-keyring.gpg      (public key, only with a key)
#
# Signing: set APT_GPG_KEY to the fingerprint (or the key ID) of a secret key that lies in the
# GnuPG keyring (GNUPGHOME). A passphrase comes from the file APT_GPG_PASSPHRASE_FILE (never as an
# argument). Without APT_GPG_KEY an unsigned repository is created (only as a trial; apt rejects it without
# [trusted=yes]). The script never creates a key.
#
# Existing packages in ZIELORDNER/pool stay and are included. A new package replaces one
# of the same name from --base (the same file name = the same version).
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

# Puts a package into pool/<component>/<initial letter>/<name>/; the place follows from the fields of the package,
# not from a path in the archive.
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
        # -type f follows no links; the list stands in a file so that an error in add_deb aborts.
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
    # Empty list for architectures without a package: apt then reports no error 404.
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
    # Public key in binary format, as `Signed-By:` expects it.
    gpg --batch --yes --export "$APT_GPG_KEY" > baruch-archive-keyring.gpg
    echo "apt-repo.sh: signiert mit $APT_GPG_KEY (InRelease, Release.gpg, baruch-archive-keyring.gpg)."
else
    echo "apt-repo.sh: WARNUNG – kein APT_GPG_KEY gesetzt, Repository ist unsigniert (nur zur Probe)." >&2
fi

echo "apt-repo.sh: Repository in $repo"
