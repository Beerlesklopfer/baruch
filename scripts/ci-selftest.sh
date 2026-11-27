#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Selbsttest der Hilfsskripte für CI, Release und Webseite – ohne Netz, ohne GitHub, ohne echte Schlüssel.
#
#   scripts/ci-selftest.sh
#
# Alles spielt in einem Temp-Ordner mit erfundenen Daten: zwei erfundene .deb-Versionen, ein Wegwerfschlüssel
# in einem eigenen GNUPGHOME (wird mit dem Ordner gelöscht; der echte Archivschlüssel wird nie berührt), ein
# Nachbau von `gh`, eine erfundene Hugo-Seite. Jede Zusicherung nennt ihren Grund (Muster SKILLS.md).
# Fehlt ein Werkzeug (gpg, hugo), wird der Teil übersprungen und am Ende genannt – nie still.
# Exit 0 nur, wenn keine Zusicherung scheitert. HUGO nennt eine andere Binärdatei als `hugo`.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
scripts="$root/scripts"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failed=0
checks=0
skipped=""

# check BESCHREIBUNG BEGRÜNDUNG BEFEHL…: Der Befehl muss gelingen.
check() {
    what="$1"; why="$2"; shift 2
    checks=$((checks + 1))
    if "$@" >"$tmp/out.txt" 2>&1; then
        echo "ok    $what"
    else
        failed=$((failed + 1))
        echo "FEHLER $what"
        echo "       Grund der Zusicherung: $why"
        sed -n '1,12s/^/       | /p' "$tmp/out.txt"
    fi
}
# check_exit ERWARTET BESCHREIBUNG BEGRÜNDUNG BEFEHL…: Der Befehl muss mit genau diesem Exit-Code enden.
check_exit() {
    want="$1"; what="$2"; why="$3"; shift 3
    checks=$((checks + 1))
    got=0
    "$@" >"$tmp/out.txt" 2>&1 || got=$?
    if [ "$got" = "$want" ]; then
        echo "ok    $what"
    else
        failed=$((failed + 1))
        echo "FEHLER $what (Exit $got statt $want)"
        echo "       Grund der Zusicherung: $why"
        sed -n '1,12s/^/       | /p' "$tmp/out.txt"
    fi
}
has() { command -v "$1" >/dev/null 2>&1; }

# ---- 1. APT-Repository über zwei Versionen ---------------------------------------------------------------
# Erfundenes Paket „baruch“ in zwei Versionen (nur eine Textdatei; kein echtes Programm).
make_deb() {
    version="$1"
    pkg="$tmp/pkg-$version"
    mkdir -p "$pkg/DEBIAN" "$pkg/usr/share/doc/baruch"
    printf 'Erfundenes Paket für scripts/ci-selftest.sh, Version %s.\n' "$version" > "$pkg/usr/share/doc/baruch/PROBE"
    cat > "$pkg/DEBIAN/control" <<EOF
Package: baruch
Version: $version
Architecture: amd64
Maintainer: Selbsttest <selbsttest@example.invalid>
Description: erfundenes Paket (Selbsttest)
EOF
    dpkg-deb --root-owner-group --build "$pkg" "$tmp/baruch_${version}_amd64.deb" >/dev/null
}
versions_in() { sed -n 's/^Version: //p' "$1" | sort | tr '\n' ' '; }

if has apt-ftparchive && has dpkg-deb; then
    make_deb 0.8.0-1
    make_deb 0.9.0-1
    check "apt-repo.sh baut ein unsigniertes Repository aus einem Paket" \
        "Grundfall des ersten Release: ein Paket, kein Schlüssel hinterlegt." \
        sh "$scripts/apt-repo.sh" "$tmp/repo1" "$tmp/baruch_0.8.0-1_amd64.deb"
    packages1="$tmp/repo1/dists/stable/main/binary-amd64/Packages"
    check "Packages nennt nach dem ersten Lauf genau Version 0.8.0-1" \
        "Sonst wäre die Ausgangslage für die Übernahme unten nicht die behauptete." \
        test "$(versions_in "$packages1")" = "0.8.0-1 "
    check "ohne Schlüssel entsteht kein InRelease und kein Schlüsselbund" \
        "Ein unsigniertes Repository darf nicht wie ein signiertes aussehen (apt lehnt es dann ehrlich ab)." \
        sh -c "[ ! -e '$tmp/repo1/dists/stable/InRelease' ] && [ ! -e '$tmp/repo1/baruch-archive-keyring.gpg' ]"
    check "binary-arm64/Packages liegt leer vor" \
        "Release nennt arm64; ohne die (leere) Liste meldete apt dort einen Fehler 404." \
        sh -c "[ -f '$tmp/repo1/dists/stable/main/binary-arm64/Packages' ] && [ ! -s '$tmp/repo1/dists/stable/main/binary-arm64/Packages' ]"

    # Der Stand des ersten Release als Archiv, wie release.yml es ans Release hängt – mit Störkörpern im pool/.
    printf 'kein Paket\n' > "$tmp/repo1/pool/main/b/baruch/LIESMICH.txt"
    ln -s /etc/hostname "$tmp/repo1/pool/main/b/baruch/verweis.deb"
    tar -C "$tmp/repo1" -czf "$tmp/apt-repo-alt.tar.gz" .

    check "apt-repo.sh --base übernimmt das Archiv des vorigen Release und nimmt das neue Paket dazu" \
        "Kern von „ansammelnd“: release.yml ruft genau so auf." \
        sh "$scripts/apt-repo.sh" --base "$tmp/apt-repo-alt.tar.gz" "$tmp/repo2" "$tmp/baruch_0.9.0-1_amd64.deb"
    packages2="$tmp/repo2/dists/stable/main/binary-amd64/Packages"
    check "Packages nennt danach beide Versionen (0.8.0-1 und 0.9.0-1)" \
        "Ältere Versionen müssen installierbar bleiben (apt install baruch=0.8.0-1); fehlte die alte, hätte das Release sie gelöscht." \
        test "$(versions_in "$packages2")" = "0.8.0-1 0.9.0-1 "
    check "im pool/ liegen genau die zwei Pakete, kein Verweis und keine fremde Datei" \
        "Aus dem alten Archiv dürfen nur echte .deb-Dateien kommen; ein Verweis könnte auf beliebige Dateien des Runners zeigen." \
        sh -c "[ \"\$(find '$tmp/repo2/pool' -mindepth 1 ! -type d | sort | tr '\n' ' ')\" = '$tmp/repo2/pool/main/b/baruch/baruch_0.8.0-1_amd64.deb $tmp/repo2/pool/main/b/baruch/baruch_0.9.0-1_amd64.deb ' ] && [ -z \"\$(find '$tmp/repo2/pool' -type l)\" ]"
    check "die Prüfsumme von Packages steht in Release" \
        "apt prüft Packages gegen Release; ein Release vom alten Stand machte das Repository unbrauchbar." \
        grep -q "$(sha256sum "$packages2" | cut -d' ' -f1)" "$tmp/repo2/dists/stable/Release"
    check "die Prüfsumme des alten Pakets in Packages stimmt mit der Datei überein" \
        "Die Übernahme darf das Paket nicht verändern; apt verweigerte sonst die Installation." \
        grep -q "^SHA256: $(sha256sum "$tmp/baruch_0.8.0-1_amd64.deb" | cut -d' ' -f1)\$" "$packages2"
    check_exit 1 "apt-repo.sh --base bricht bei fehlendem Archiv ab" \
        "Ein Tippfehler im Pfad darf nicht still zu einem Repository ohne die alten Pakete führen." \
        sh "$scripts/apt-repo.sh" --base "$tmp/gibt-es-nicht.tar.gz" "$tmp/repo3" "$tmp/baruch_0.9.0-1_amd64.deb"

    # ---- 2. Signieren mit einem Wegwerfschlüssel ---------------------------------------------------------
    if has gpg && has gpgv; then
        GNUPGHOME="$tmp/gnupg"
        export GNUPGHOME
        mkdir -m 700 "$GNUPGHOME"
        printf 'erfundene-passphrase-nur-im-selbsttest' > "$tmp/passphrase"
        if gpg --batch --quiet --pinentry-mode loopback --passphrase-file "$tmp/passphrase" \
            --quick-generate-key "Selbsttest Wegwerfschlüssel <selbsttest@example.invalid>" ed25519 sign 1d \
            >"$tmp/keygen.txt" 2>&1; then
            fingerprint="$(gpg --batch --list-secret-keys --with-colons | awk -F: '$1 == "fpr" { print $10; exit }')"
            check "apt-repo.sh signiert mit Schlüssel und Passphrase aus einer Datei" \
                "So ruft release.yml auf (APT_GPG_KEY, APT_GPG_PASSPHRASE_FILE); die Passphrase steht nie in einer Befehlszeile." \
                env APT_GPG_KEY="$fingerprint" APT_GPG_PASSPHRASE_FILE="$tmp/passphrase" \
                sh "$scripts/apt-repo.sh" --base "$tmp/apt-repo-alt.tar.gz" "$tmp/repo-signiert" "$tmp/baruch_0.9.0-1_amd64.deb"
            check "InRelease lässt sich allein mit baruch-archive-keyring.gpg prüfen" \
                "Genau das tut apt mit Signed-By; die Schlüsseldatei muss darum binär sein und zur Signatur passen." \
                gpgv --keyring "$tmp/repo-signiert/baruch-archive-keyring.gpg" "$tmp/repo-signiert/dists/stable/InRelease"
            check "Release.gpg passt zu Release" \
                "Ältere apt-Fassungen lesen Release und Release.gpg statt InRelease." \
                gpgv --keyring "$tmp/repo-signiert/baruch-archive-keyring.gpg" \
                "$tmp/repo-signiert/dists/stable/Release.gpg" "$tmp/repo-signiert/dists/stable/Release"
            check "die Schlüsseldatei enthält keinen geheimen Schlüssel" \
                "Sie wird öffentlich unter /apt/ ausgeliefert." \
                sh -c "! gpg --batch --list-packets '$tmp/repo-signiert/baruch-archive-keyring.gpg' | grep -q 'secret key packet'"
            check_exit 1 "ein veränderter InRelease fällt bei der Prüfung durch" \
                "Gegenprobe: Die Prüfung oben wäre wertlos, wenn sie alles durchließe (gpgv: Exit 1 bei falscher Signatur)." \
                sh -c "sed 's/^Suite: stable/Suite: anders/' '$tmp/repo-signiert/dists/stable/InRelease' > '$tmp/InRelease.falsch' && gpgv --keyring '$tmp/repo-signiert/baruch-archive-keyring.gpg' '$tmp/InRelease.falsch'"
        else
            skipped="$skipped Signieren(Wegwerfschlüssel-ließ-sich-nicht-erzeugen)"
        fi
        gpgconf --kill gpg-agent >/dev/null 2>&1 || true
        unset GNUPGHOME
    else
        skipped="$skipped Signieren(gpg/gpgv-fehlt)"
    fi
else
    skipped="$skipped APT-Repository(apt-ftparchive/dpkg-deb-fehlt)"
fi

# ---- 3. Abruf des letzten veröffentlichten Release gegen einen Nachbau von gh ----------------------------
# Der Nachbau liest sein Verhalten aus FAKE_GH_MODE; er spricht mit niemandem.
mkdir -p "$tmp/bin" "$tmp/release"
printf 'erfundenes Archiv\n' > "$tmp/release/apt-repo.tar.gz"
(cd "$tmp/release" && sha256sum apt-repo.tar.gz > SHA256SUMS)
cat > "$tmp/bin/gh" <<'EOF'
#!/bin/sh
# Nachbau von gh für scripts/ci-selftest.sh.
case "$FAKE_GH_MODE:$1" in
    none:api) echo "[]" ;;                   # noch kein Release
    draft:api)                               # nur ein Entwurf und ein Release ohne Archiv
        cat <<'JSON'
[{"tag_name": "v0.9.0", "draft": true, "prerelease": false, "published_at": null,
  "assets": [{"name": "apt-repo.tar.gz"}]},
 {"tag_name": "v0.8.0", "draft": false, "prerelease": false, "published_at": "2026-10-01T10:00:00Z",
  "assets": [{"name": "SHA256SUMS"}]}]
JSON
        ;;
    down:api) echo "HTTP 503" >&2; exit 1 ;; # Störung
    html:api) echo "<html>Wartung</html>" ;; # Antwort ist kein JSON
    *:api)
        # Erfundene Liste, absichtlich ungeordnet: Entwurf (neuer), Release ohne Archiv (neuer), Vorabversion
        # (neuer), zwei veröffentlichte mit Archiv. Richtig ist v0.9.0.
        cat <<'JSON'
[{"tag_name": "v0.9.2", "draft": true, "prerelease": false, "published_at": null,
  "assets": [{"name": "apt-repo.tar.gz"}]},
 {"tag_name": "v0.9.1", "draft": false, "prerelease": false, "published_at": "2027-01-05T10:00:00Z",
  "assets": [{"name": "SHA256SUMS"}]},
 {"tag_name": "v0.8.0", "draft": false, "prerelease": false, "published_at": "2026-10-01T10:00:00Z",
  "assets": [{"name": "apt-repo.tar.gz"}, {"name": "SHA256SUMS"}]},
 {"tag_name": "v0.9.0", "draft": false, "prerelease": false, "published_at": "2026-12-01T10:00:00Z",
  "assets": [{"name": "apt-repo.tar.gz"}, {"name": "SHA256SUMS"}]},
 {"tag_name": "v1.0.0-rc1", "draft": false, "prerelease": true, "published_at": "2027-02-01T10:00:00Z",
  "assets": [{"name": "apt-repo.tar.gz"}]}]
JSON
        ;;
    *:release)
        echo "$3" > "$FAKE_GH_FILES/gewaehlt.txt" # gh release download TAG …
        dir=""
        while [ $# -gt 0 ]; do
            [ "$1" = "--dir" ] && dir="$2"
            shift
        done
        cp "$FAKE_GH_FILES/apt-repo.tar.gz" "$dir/"
        case "$FAKE_GH_MODE" in
            badsum) echo "0000000000000000000000000000000000000000000000000000000000000000  apt-repo.tar.gz" > "$dir/SHA256SUMS" ;;
            nosum) echo "abc  anderes.deb" > "$dir/SHA256SUMS" ;;
            *) cp "$FAKE_GH_FILES/SHA256SUMS" "$dir/" ;;
        esac
        ;;
esac
EOF
chmod +x "$tmp/bin/gh"
fetch() {
    mode="$1"; shift
    env PATH="$tmp/bin:$PATH" GH_REPO="erfunden/probe" GH_TOKEN="kein-echtes-token" FAKE_GH_MODE="$mode" \
        FAKE_GH_FILES="$tmp/release" sh "$scripts/ci-apt-fetch.sh" "$@"
}
check_exit 0 "ci-apt-fetch.sh holt das Archiv und prüft es gegen SHA256SUMS" \
    "Normalfall ab dem zweiten Release." fetch ok "$tmp/fetch-ok"
check "das geholte Archiv liegt im Zielordner" \
    "release.yml und pages.yml lesen genau diesen Pfad." test -s "$tmp/fetch-ok/apt-repo.tar.gz"
check "gewählt ist v0.9.0: das jüngste veröffentlichte Release mit Archiv" \
    "Nicht der Entwurf v0.9.2 (mit Leserechten unsichtbar), nicht v0.9.1 ohne Archiv (leerte das Repository), nicht die Vorabversion, nicht das ältere v0.8.0 (verlöre 0.9.0)." \
    test "$(cat "$tmp/release/gewaehlt.txt")" = "v0.9.0"
check_exit 3 "ohne jedes Release endet ci-apt-fetch.sh mit Exit 3" \
    "Erstes Release: Die Abläufe unterscheiden „gibt es nicht“ (weiter ohne Grundlage) von einem Fehler." \
    fetch none "$tmp/fetch-none"
check_exit 3 "nur ein Entwurf und ein Release ohne Archiv: ebenfalls Exit 3" \
    "Die dokumentierte Grenze: Ein Entwurf zählt nicht; pages.yml liefert dann ohne /apt/ aus und warnt." \
    fetch draft "$tmp/fetch-draft"
check_exit 1 "eine Antwort, die kein JSON ist, ist ein Fehler" \
    "Eine Wartungsseite darf nicht als „kein Release“ gelten." fetch html "$tmp/fetch-html"
check_exit 1 "eine Störung beim Abruf ist ein Fehler, kein „kein Release“" \
    "Sonst lieferte pages.yml bei einem Ausfall die Seite ohne /apt/ aus und das Repository wäre weg." \
    fetch down "$tmp/fetch-down"
check_exit 1 "eine falsche Prüfsumme bricht ab" \
    "Ein beschädigtes Archiv darf nicht Grundlage des nächsten Repositorys werden." fetch badsum "$tmp/fetch-bad"
check_exit 1 "fehlt das Archiv in SHA256SUMS, bricht ci-apt-fetch.sh ab" \
    "Ohne Eintrag gäbe es nichts zu prüfen; ungeprüft wird nichts übernommen." fetch nosum "$tmp/fetch-nosum"
check_exit 1 "ohne GH_REPO bricht ci-apt-fetch.sh ab" \
    "Ohne Ziel fragte gh das Repository des Arbeitsverzeichnisses – nicht unbedingt das gemeinte." \
    env -u GH_REPO PATH="$tmp/bin:$PATH" sh "$scripts/ci-apt-fetch.sh" "$tmp/fetch-norepo"

# ---- 4. Werkzeugversionen --------------------------------------------------------------------------------
check "ci-tool.sh kennt Hugo (nur der Aufruf, nichts wird geladen)" \
    "Hugo ist das einzige Werkzeug, das die CI noch lädt; der Aufruf muss stimmen, bevor ein Lauf am Netz hängt." \
    env CI_TOOL_CHECK_ONLY=1 sh "$scripts/ci-tool.sh" hugo "$tmp/werkzeug"
check_exit 2 "ci-tool.sh lädt kein Typst mehr" \
    "Seit Runde 36 setzt die CI mit dem eigenen Programm (baruch-typst); ein Download wäre ein zweiter Compiler." \
    env CI_TOOL_CHECK_ONLY=1 sh "$scripts/ci-tool.sh" typst "$tmp/werkzeug"
no_typst_download() {
    ! grep -n 'typst/typst/releases\|typst-x86_64' "$root/.github/workflows/"*.yml "$scripts/ci-tool.sh" \
        "$scripts/ci-typst.sh" || return 1
    ! grep -n 'ci-tool.sh typst' "$root/.github/workflows/"*.yml
}
check "kein Arbeitsablauf und kein Skript lädt Typst" \
    "Gegenprobe über die Dateien: Der Download darf nicht an anderer Stelle wiederkommen." no_typst_download
typst_step_uses_own_program() {
    grep -q 'sh scripts/ci-typst.sh target/debug/baruch-server$' "$root/.github/workflows/ci.yml"
}
check "ci.yml ruft ci-typst.sh ohne fremdes Typst auf" \
    "Ohne zweites Argument setzt das eigene Programm über den Symlink baruch-typst – das ist die Prüfung der Runde 36." \
    typst_step_uses_own_program
# Nachbau von baruch-server für ci-typst.sh: import/export wie das echte Programm, dazu die Typst-Kommandozeile
# unter dem Namen baruch-typst. So läuft der Ablauf des Skripts hier ohne gebaute Binärdatei.
typst_version="$(awk '
    $0 == "name = \"typst\"" { hit = 1; next }
    hit && /^version = / { gsub(/[" ]/, "", $0); sub(/^version=/, "", $0); print; exit }
' "$root/Cargo.lock")"
fake_server() {
    mkdir -p "$tmp/$1"
    cat > "$tmp/$1/baruch-server" <<EOF
#!/bin/sh
case "\$(basename "\$0")" in
    baruch-typst)
        case "\$1" in
            --version) echo "typst $2 (Baruch 0.0.0)"; exit 0 ;;
            watch) echo "typst: Den Unterbefehl watch unterstützt Baruch als Typst-Alias nicht." >&2; exit 2 ;;
            compile)
                shift
                [ "\$1" = --root ] && shift 2
                if grep -q gibt_es_nicht "\$1"; then
                    echo "\$1:1:12: error: unknown variable: gibt_es_nicht" >&2
                    exit $3
                fi
                printf '%%PDF-1.7 Nachbau\n' > "\$2"
                exit 0 ;;
        esac
        exit 2 ;;
esac
case "\$1" in
    import) printf 'Werk: 1\nOrdnername: der-leuchtturm\nAbschnitte: 6\n' ;;
    export) mkdir -p "\$4"; cp "$scripts/ci-data/leuchtturm.typ" "\$4/der-leuchtturm.typ" ;;
esac
EOF
    chmod +x "$tmp/$1/baruch-server"
}
fake_server typst-gut "$typst_version" 1
check "ci-typst.sh setzt mit dem eigenen Programm unter dem Namen baruch-typst (Nachbau)" \
    "Ablauf des Skripts ohne gebaute Binärdatei: Symlink, Versionsabgleich mit Cargo.lock, Satz, Gegenproben." \
    sh "$scripts/ci-typst.sh" "$tmp/typst-gut/baruch-server"
fake_server typst-alt 0.0.0 1
check_exit 1 "ci-typst.sh scheitert, wenn --version eine andere Typst-Version nennt als Cargo.lock" \
    "TYPST_VERSION in baruch-render muss dem Crate folgen; sonst belöge --version die Nutzer." \
    sh "$scripts/ci-typst.sh" "$tmp/typst-alt/baruch-server"
fake_server typst-still "$typst_version" 0
check_exit 1 "ci-typst.sh scheitert, wenn ein Satzfehler mit Exit 0 endet" \
    "Gegenprobe: Ein Programm, das alles durchwinkt, darf die Prüfung nicht bestehen." \
    sh "$scripts/ci-typst.sh" "$tmp/typst-still/baruch-server"
check_exit 2 "ci-tool.sh lehnt ein unbekanntes Werkzeug ab" \
    "Es lädt nur, wofür eine Prüfsumme hinterlegt ist." sh "$scripts/ci-tool.sh" anderes "$tmp/werkzeug"

# ---- 5. Seitenbau an einer erfundenen Hugo-Seite ---------------------------------------------------------
hugo="${HUGO:-hugo}"
if has "$hugo"; then
    version="$(sh "$scripts/version.sh")"
    make_site() {
        site="$tmp/$1"
        mkdir -p "$site/layouts" "$site/static" "$site/content"
        cat > "$site/hugo.toml" <<EOF
baseURL = "https://baruch.bernau.family/"
locale = "de"
title = "Erfundene Probe"
disableKinds = ["taxonomy", "term", "RSS", "sitemap", "section", "page", "404"]

[params]
  version = "$2"
EOF
        printf '<!doctype html><title>Probe</title><p>Version {{ site.Params.version }}</p>\n' > "$site/layouts/index.html"
        printf '%s' "$3" > "$site/static/CNAME"
    }
    make_site seite-gut "$version" "baruch.bernau.family"
    check "ci-site.sh baut eine Seite, deren Version zu Cargo.toml passt" \
        "Normalfall in ci.yml, release.yml und pages.yml." \
        env HUGO="$hugo" sh "$scripts/ci-site.sh" --site "$tmp/seite-gut"
    check "ohne Fingerabdruck warnt ci-site.sh, bricht aber nicht ab" \
        "Die Seite zeigte sonst unbemerkt den Platzhalter; ein Abbruch verhinderte die Generalprobe vor dem Eintragen." \
        sh -c "env HUGO='$hugo' sh '$scripts/ci-site.sh' --site '$tmp/seite-gut' | grep -q '^::warning::Platzhalter: Der Fingerabdruck'"
    make_site seite-fertig "$version" "baruch.bernau.family"
    printf '  aptFingerprint = "0123 4567 89AB CDEF 0123 4567 89AB CDEF 0123 4567"\n' >> "$tmp/seite-fertig/hugo.toml"
    printf -- '---\ntitle: "Impressum"\nplatzhalter: true\n---\nErfundene Probe.\n' > "$tmp/seite-fertig/content/impressum.md"
    check "eine als Platzhalter markierte Seite meldet ci-site.sh mit ihrem Pfad" \
        "Ein Impressum ohne Angaben soll niemand versehentlich ausliefern." \
        sh -c "env HUGO='$hugo' sh '$scripts/ci-site.sh' --site '$tmp/seite-fertig' | grep -q '^::warning::Platzhalter: content/impressum.md'"
    check "mit eingetragenem Fingerabdruck entfällt dessen Warnung" \
        "Gegenprobe: Eine Warnung, die immer kommt, liest niemand mehr." \
        sh -c "! env HUGO='$hugo' sh '$scripts/ci-site.sh' --site '$tmp/seite-fertig' | grep -q 'Fingerabdruck'"
    rm "$tmp/seite-fertig/content/impressum.md"
    check "ohne markierte Seite und mit Fingerabdruck warnt ci-site.sh gar nicht" \
        "Gegenprobe zur Meldung über Platzhalter-Seiten." \
        sh -c "! env HUGO='$hugo' sh '$scripts/ci-site.sh' --site '$tmp/seite-fertig' | grep -q '::warning::'"
    make_site seite-alt "0.0.0" "baruch.bernau.family"
    check_exit 1 "ci-site.sh bricht bei abweichender Version ab" \
        "Die Seite würbe sonst für eine Version, deren Download-Links ins Leere zeigen." \
        env HUGO="$hugo" sh "$scripts/ci-site.sh" --site "$tmp/seite-alt"
    make_site seite-cname "$version" "anderswo.example"
    check_exit 1 "ci-site.sh bricht bei falschem CNAME ab" \
        "GitHub Pages stellte die Seite sonst auf einen anderen Namen um." \
        env HUGO="$hugo" sh "$scripts/ci-site.sh" --site "$tmp/seite-cname"
    check "site-check.py besteht seinen Selbsttest" \
        "ci-site.sh verlässt sich auf die Linkprüfung; der Selbsttest zeigt an einem erfundenen Baum, dass sie tote Links, Anker und fremde Adressen wirklich findet." \
        python3 "$scripts/site-check.py" --selbsttest
    make_site seite-tot "$version" "baruch.bernau.family"
    printf '<!doctype html><title>Probe</title><p>Version {{ site.Params.version }}</p><a href="/gibt-es-nicht/">tot</a>\n' > "$tmp/seite-tot/layouts/index.html"
    check_exit 1 "ci-site.sh bricht bei einem toten internen Link ab" \
        "Gegenprobe zum Normalfall oben: Ruft ci-site.sh die Linkprüfung nicht mehr auf, bliebe das hier unbemerkt." \
        env HUGO="$hugo" sh "$scripts/ci-site.sh" --site "$tmp/seite-tot"
    check "ci-site.sh nennt den toten Link mit Datei und Adresse" \
        "Exit 1 allein könnte auch von einer anderen Prüfung stammen (Version, CNAME)." \
        sh -c "env HUGO='$hugo' sh '$scripts/ci-site.sh' --site '$tmp/seite-tot' 2>&1 | grep -q '^index.html: toter Verweis <a href>: /gibt-es-nicht/'"
    mkdir -p "$tmp/seite-tot/content/hilfe"
    printf -- '---\ntitle: "Hilfe"\n---\nErfundene Probe.\n' > "$tmp/seite-tot/content/hilfe/_index.md"
    printf '<!doctype html><title>Probe</title><p>Version {{ site.Params.version }}</p>\n' > "$tmp/seite-tot/layouts/index.html"
    check "mit einem Abschnitt content/hilfe verlangt ci-site.sh die Weiterleitung von /bedienung/" \
        "Alte Links auf /bedienung/ sollen in der Hilfe ankommen; die Probe hat den Abschnitt, aber keine Weiterleitung." \
        sh -c "env HUGO='$hugo' sh '$scripts/ci-site.sh' --site '$tmp/seite-tot' 2>&1 | grep -q 'bedienung/index.html: die alte Adresse /bedienung/ fehlt'"
    if [ -f "$tmp/repo2/dists/stable/Release" ]; then
        tar -C "$tmp/repo2" -czf "$tmp/apt-repo-neu.tar.gz" .
        check "ci-site.sh --apt legt das Repository nach public/apt/" \
            "So liefern release.yml und pages.yml /apt/ aus." \
            env HUGO="$hugo" sh "$scripts/ci-site.sh" --site "$tmp/seite-gut" --apt "$tmp/apt-repo-neu.tar.gz"
        check "unter public/apt/ liegen Release und beide Pakete" \
            "Der Seitenbau darf das angesammelte Repository nicht kürzen." \
            sh -c "[ -s '$tmp/seite-gut/public/apt/dists/stable/Release' ] && [ \"\$(find '$tmp/seite-gut/public/apt/pool' -name '*.deb' | wc -l)\" = 2 ]"
        printf 'kein Archiv\n' > "$tmp/kaputt.tar.gz"
        check_exit 2 "ci-site.sh --apt bricht bei einem kaputten Archiv ab" \
            "Eine Seite mit halbem Repository darf nicht ausgeliefert werden (tar: Exit 2)." \
            env HUGO="$hugo" sh "$scripts/ci-site.sh" --site "$tmp/seite-gut" --apt "$tmp/kaputt.tar.gz"
    fi
else
    skipped="$skipped Seitenbau(hugo-fehlt)"
fi

# ---- 6. Regeln der Arbeitsabläufe ------------------------------------------------------------------------
if python3 -c 'import yaml' >/dev/null 2>&1; then
    check "die Arbeitsabläufe im Repository halten die Regeln ein" \
        "SHAs, Rechte, Geheimnisse, feste Rust-Version (scripts/ci-check-workflows.sh nennt je Regel den Grund)." \
        sh "$scripts/ci-check-workflows.sh"
    # Gegenproben: je ein erfundener Ablauf mit genau einem Verstoß.
    bad_workflow() {
        mkdir -p "$tmp/wf-$1"
        cat > "$tmp/wf-$1/probe.yml"
    }
    bad_workflow tag <<'EOF'
on: push
permissions:
  contents: read
jobs:
  a:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          persist-credentials: false
EOF
    bad_workflow prt <<'EOF'
on:
  pull_request_target:
permissions:
  contents: read
jobs:
  a:
    runs-on: ubuntu-24.04
    steps:
      - run: echo hallo
EOF
    bad_workflow release-typen <<'EOF'
on:
  release:
permissions:
  contents: read
jobs:
  a:
    runs-on: ubuntu-24.04
    steps:
      - run: echo hallo
EOF
    mkdir -p "$tmp/wf-release-published"
    sed 's/^  release:$/  release:\n    types: [published]/' "$tmp/wf-release-typen/probe.yml" \
        > "$tmp/wf-release-published/probe.yml"
    bad_workflow secret <<'EOF'
on: push
permissions:
  contents: read
jobs:
  a:
    runs-on: ubuntu-24.04
    steps:
      - run: gpg --passphrase "${{ secrets.APT_GPG_PASSPHRASE }}" --sign datei
EOF
    bad_workflow token <<'EOF'
on: push
permissions:
  contents: read
jobs:
  a:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2
EOF
    bad_workflow pages <<'EOF'
on: push
permissions:
  contents: read
jobs:
  a:
    runs-on: ubuntu-24.04
    permissions:
      pages: write
    steps:
      - run: echo hallo
EOF
    # Hochladen und Ausliefern im selben Job (Stand bis Runde 32).
    bad_workflow pages-ein-job <<'EOF'
on: push
permissions:
  contents: read
jobs:
  pages:
    runs-on: ubuntu-24.04
    permissions:
      contents: read
      pages: write
      id-token: write
    environment:
      name: github-pages
    steps:
      - uses: actions/upload-pages-artifact@fc324d3547104276b827a68afc52ff2a11cc49c9 # v5.0.0
      - uses: actions/deploy-pages@368f82528645a54fb793d4d04e342629a3f51346 # v5.0.1
EOF
    # Dasselbe in zwei Jobs: bauen ohne Schreibrechte, ausliefern nur mit deploy-pages.
    bad_workflow pages-zwei-jobs <<'EOF'
on: push
permissions:
  contents: read
jobs:
  pages-build:
    runs-on: ubuntu-24.04
    permissions:
      contents: read
    steps:
      - run: echo bauen
      - uses: actions/upload-pages-artifact@fc324d3547104276b827a68afc52ff2a11cc49c9 # v5.0.0
  pages-deploy:
    needs: [pages-build]
    runs-on: ubuntu-24.04
    permissions:
      contents: read
      pages: write
      id-token: write
    environment:
      name: github-pages
    steps:
      - uses: actions/deploy-pages@368f82528645a54fb793d4d04e342629a3f51346 # v5.0.1
EOF
    mkdir -p "$tmp/wf-pages-ohne-needs" "$tmp/wf-pages-baut-mit" "$tmp/wf-pages-schreibt"
    # Zwei Jobs, aber der ausliefernde hängt nicht am bauenden.
    grep -v '^    needs: \[pages-build\]$' "$tmp/wf-pages-zwei-jobs/probe.yml" > "$tmp/wf-pages-ohne-needs/probe.yml"
    # Zwei Jobs, aber der ausliefernde führt zusätzlich einen Befehl aus.
    sed 's/^      - uses: actions\/deploy-pages@/      - run: echo nebenbei\n&/' \
        "$tmp/wf-pages-zwei-jobs/probe.yml" > "$tmp/wf-pages-baut-mit/probe.yml"
    # Zwei Jobs, aber der bauende hat ein Schreibrecht (und ist darum auch kein Pages-Job mehr ohne Umgebung).
    sed 's/^      - run: echo bauen$/      - run: echo bauen mit Schreibrecht/' "$tmp/wf-pages-zwei-jobs/probe.yml" |
        awk '{ print } /^      contents: read$/ && !done { print "      pages: write"; done = 1 }' \
        > "$tmp/wf-pages-schreibt/probe.yml"
    bad_workflow write <<'EOF'
on: push
permissions:
  contents: write
jobs:
  a:
    runs-on: ubuntu-24.04
    steps:
      - run: echo hallo
EOF
    bad_workflow rust <<'EOF'
on: push
permissions:
  contents: read
jobs:
  a:
    runs-on: ubuntu-24.04
    steps:
      - run: rustup default stable && cargo build
EOF
    bad_workflow archivschluessel <<'EOF'
on: push
permissions:
  contents: read
jobs:
  a:
    runs-on: ubuntu-24.04
    steps:
      - run: echo hallo
        env:
          APT_GPG_PRIVATE_KEY: ${{ secrets.APT_GPG_PRIVATE_KEY }}
EOF
    mkdir -p "$tmp/wf-umgebung"
    sed 's/^    runs-on: ubuntu-24.04$/    runs-on: ubuntu-24.04\n    environment: release/' \
        "$tmp/wf-archivschluessel/probe.yml" > "$tmp/wf-umgebung/probe.yml"
    check_exit 1 "Regelprüfung lehnt den Archivschlüssel in einem Job ohne Umgebung release ab" \
        "Ohne Umgebung käme jeder Lauf auf einem Tag an den Schlüssel, ohne Freigabe." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-archivschluessel"
    check "derselbe Job mit environment release besteht die Regelprüfung" \
        "Gegenprobe: Abgelehnt wird wegen der fehlenden Umgebung, nicht wegen des Secrets an sich." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-umgebung"
    check_exit 1 "Regelprüfung lehnt eine Aktion mit Tag statt SHA ab" \
        "Ein Tag lässt sich verschieben; die Prüfung muss das erkennen, sonst wäre sie wertlos." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-tag"
    check_exit 1 "Regelprüfung lehnt pull_request_target ab" \
        "Fremder Code liefe mit den Rechten des Repositorys." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-prt"
    check_exit 1 "Regelprüfung lehnt den Auslöser release ohne types: [published] ab" \
        "Sonst lieferte pages.yml bei jedem Ereignis eines Release aus, auch bei einem Entwurf ohne abrufbares Archiv." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-release-typen"
    check "derselbe Ablauf mit types: [published] besteht die Regelprüfung" \
        "Gegenprobe: Abgelehnt wird die fehlende Einschränkung, nicht der Auslöser an sich." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-release-published"
    pages_follow_a_published_release() {
        python3 - "$root/.github/workflows/pages.yml" <<'PY'
import sys
import yaml

doc = yaml.safe_load(open(sys.argv[1], encoding="utf-8"))
on = doc.get("on", doc.get(True))
assert on.get("release") == {"types": ["published"]}, on
assert "workflow_dispatch" in on and on.get("push") == {"branches": ["main"]}, on
assert doc["concurrency"] == {"group": "pages", "cancel-in-progress": False}, doc["concurrency"]
PY
    }
    check "pages.yml läuft auch beim Veröffentlichen eines Release, in der Gruppe pages" \
        "Runde 36: So kommt die Seite samt /apt/ auch an, wenn der Tag auf dem Commit des letzten main-Laufs liegt; der Handweg (workflow_dispatch) und der Push auf main bleiben." \
        pages_follow_a_published_release
    check_exit 1 "Regelprüfung lehnt ein Geheimnis in einer Befehlszeile ab" \
        "Es stünde in der Prozessliste und im Protokoll des Befehls." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-secret"
    check_exit 1 "Regelprüfung lehnt checkout ohne persist-credentials: false ab" \
        "Das Token läge sonst für alle späteren Schritte in .git/config." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-token"
    check_exit 1 "Regelprüfung lehnt pages: write ohne Umgebung github-pages ab" \
        "Ausliefern darf nur der dafür bestimmte Job." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-pages"
    check_exit 1 "Regelprüfung lehnt upload-pages-artifact und deploy-pages im selben Job ab" \
        "Am 2026-11-24 fand deploy-pages so das frische Artefakt nicht; Bauen und Ausliefern sind zwei Jobs." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-pages-ein-job"
    check "dieselben Schritte in zwei Jobs (bauen ohne Schreibrechte, dann ausliefern) bestehen die Regelprüfung" \
        "Gegenprobe: Abgelehnt wird die Lage im selben Job, nicht die Aktionen an sich." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-pages-zwei-jobs"
    check_exit 1 "Regelprüfung lehnt einen Pages-Job ohne needs auf den bauenden Job ab" \
        "Ohne needs liefe das Ausliefern neben dem Bauen her und fände kein Artefakt." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-pages-ohne-needs"
    check_exit 1 "Regelprüfung lehnt einen Befehl im ausliefernden Pages-Job ab" \
        "Der Job mit pages: write und id-token: write soll keinen Code des Repositorys ausführen." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-pages-baut-mit"
    check_exit 1 "Regelprüfung lehnt Schreibrechte im Job mit upload-pages-artifact ab" \
        "Der bauende Job führt Hugo und Skripte aus; er darf weder Seite noch Release ändern." \
        sh "$scripts/ci-check-workflows.sh" "$tmp/wf-pages-schreibt"
    check_exit 1 "Regelprüfung lehnt Schreibrechte auf oberster Ebene ab" \
        "Jeder Job ohne eigene Angabe erbte sie." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-write"
    check_exit 1 "Regelprüfung lehnt rustup stable ohne RUST_VERSION ab" \
        "Der Compiler der CI soll der lokal geprüfte sein." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-rust"
else
    skipped="$skipped Arbeitsabläufe(PyYAML-fehlt)"
fi

# ---- 7. Fingerabdruck des Archivschlüssels (erfundene Werte, kein Schlüssel) ------------------------------
fp_root() {
    dir="$tmp/fp-$1"
    mkdir -p "$dir/site" "$dir/docs"
    printf '[params]\nversion = "0.0.0"\naptFingerprint = "%s"\n\n[menus]\n' "$2" > "$dir/site/hugo.toml"
    printf 'Fingerabdruck: `%s`\n' "$3" > "$dir/README.md"
    printf 'Fingerabdruck: `%s`\n' "$4" > "$dir/docs/Verteilung.md"
}
fp_a="0123456789ABCDEF0123456789ABCDEF01234567"
fp_a_spaced="0123 4567 89AB CDEF 0123  4567 89ab cdef 0123 4567"
fp_b="FEDCBA9876543210FEDCBA9876543210FEDCBA98"
fp_placeholder="FINGERABDRUCK-TRÄGT-DER-NUTZER-EIN"
check_exit 3 "ci-fingerprint.sh meldet den heutigen Stand des Repositorys als offen oder stimmig" \
    "Solange der Nutzer nichts eingetragen hat, ist Exit 3 richtig; danach muss es Exit 0 sein – nie ein Widerspruch." \
    sh -c "sh '$scripts/ci-fingerprint.sh' && exit 3 || exit \$?"
fp_root leer "" "$fp_placeholder" "$fp_placeholder"
check_exit 3 "ohne Eintrag endet ci-fingerprint.sh mit Exit 3" \
    "ci.yml warnt dann nur; der Stand vor dem Eintragen ist kein Fehler." \
    sh "$scripts/ci-fingerprint.sh" --root "$tmp/fp-leer"
check_exit 1 "ohne Eintrag, aber mit Schlüssel scheitert ci-fingerprint.sh" \
    "Ein signiertes Repository ohne veröffentlichten Fingerabdruck könnte niemand prüfen." \
    sh "$scripts/ci-fingerprint.sh" --root "$tmp/fp-leer" --key "$fp_a"
fp_root gut "$fp_a_spaced" "$fp_a" "$fp_a_spaced"
check "ein überall gleicher Fingerabdruck besteht, auch in Vierergruppen und Kleinbuchstaben" \
    "gpg zeigt ihn in Gruppen; die Schreibweise darf keinen Fehlalarm auslösen." \
    sh "$scripts/ci-fingerprint.sh" --root "$tmp/fp-gut"
check "passt der Schlüssel zum Eintrag, besteht ci-fingerprint.sh --key" \
    "Normalfall im Job publish." sh "$scripts/ci-fingerprint.sh" --root "$tmp/fp-gut" --key "$fp_a"
check_exit 1 "ein anderer Schlüssel als der eingetragene scheitert" \
    "Sonst signierte ein vertauschtes oder untergeschobenes Secret unbemerkt; Nutzer verglichen gegen den falschen Wert." \
    sh "$scripts/ci-fingerprint.sh" --root "$tmp/fp-gut" --key "$fp_b"
fp_root readme "$fp_a" "$fp_placeholder" "$fp_a"
check_exit 1 "steht in README.md noch der Platzhalter, scheitert ci-fingerprint.sh" \
    "Das README ist der zweite Weg zum Fingerabdruck neben der Webseite; es darf nicht vergessen werden." \
    sh "$scripts/ci-fingerprint.sh" --root "$tmp/fp-readme"
fp_root abweichend "$fp_a" "$fp_a" "$fp_b"
check_exit 1 "nennt docs/Verteilung.md einen anderen Fingerabdruck, scheitert ci-fingerprint.sh" \
    "Zwei verschiedene veröffentlichte Werte machten den Vergleich wertlos." \
    sh "$scripts/ci-fingerprint.sh" --root "$tmp/fp-abweichend"
fp_root form "0123-kein-fingerabdruck" "$fp_a" "$fp_a"
check_exit 1 "ein Eintrag, der kein Fingerabdruck ist, scheitert" \
    "Ein Tippfehler oder eine Kurz-ID (16 Zeichen) taugt nicht zum Vergleich." \
    sh "$scripts/ci-fingerprint.sh" --root "$tmp/fp-form"

# ---- 8. Skripte selbst -----------------------------------------------------------------------------------
for script in "$scripts"/ci-*.sh "$scripts/apt-repo.sh" "$scripts/version.sh"; do
    check "sh -n ${script##*/}" "Ein Syntaxfehler fiele sonst erst im Release-Lauf auf." sh -n "$script"
done
probe_ok() {
    grep -q 'Frei erfunden' "$scripts/ci-data/leuchtturm.typ" || return 1
    [ "$(grep -c '^==* [^ ]' "$scripts/ci-data/leuchtturm.typ")" = 6 ]
}
check "die Typst-Probe ist frei erfunden gekennzeichnet und hat sechs Überschriften" \
    "scripts/ci-typst.sh erwartet sechs Abschnitte; so fällt eine geänderte Probe hier auf, nicht erst in der CI." \
    probe_ok

echo
[ -z "$skipped" ] || echo "Übersprungen (Werkzeug fehlt):$skipped"
echo "ci-selftest.sh: $checks Zusicherungen, $failed gescheitert."
[ "$failed" = 0 ]
