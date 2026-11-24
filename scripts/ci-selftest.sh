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
check "ci-tool.sh: die Typst-Version des Skripts ist die aus Cargo.lock" \
    "Die CI soll mit demselben Compiler setzen wie Baruch; nach einem Anheben von typst muss das Skript nachziehen." \
    env CI_TOOL_CHECK_ONLY=1 sh "$scripts/ci-tool.sh" typst "$tmp/werkzeug"
printf 'name = "typst"\nversion = "0.0.0"\n' > "$tmp/Cargo.lock.falsch"
check_exit 1 "ci-tool.sh bricht ab, wenn Cargo.lock eine andere Typst-Version führt" \
    "Gegenprobe zur Zeile darüber." \
    env CI_TOOL_CHECK_ONLY=1 CI_TOOL_LOCK="$tmp/Cargo.lock.falsch" sh "$scripts/ci-tool.sh" typst "$tmp/werkzeug"
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
    check_exit 1 "Regelprüfung lehnt ein Geheimnis in einer Befehlszeile ab" \
        "Es stünde in der Prozessliste und im Protokoll des Befehls." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-secret"
    check_exit 1 "Regelprüfung lehnt checkout ohne persist-credentials: false ab" \
        "Das Token läge sonst für alle späteren Schritte in .git/config." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-token"
    check_exit 1 "Regelprüfung lehnt pages: write ohne Umgebung github-pages ab" \
        "Ausliefern darf nur der dafür bestimmte Job." sh "$scripts/ci-check-workflows.sh" "$tmp/wf-pages"
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
