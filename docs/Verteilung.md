# Verteilung

Wie aus dem Quelltext Pakete werden, wie ein Release entsteht, wie Webseite und APT-Repository ausgeliefert
werden und wie Endnutzer Baruch installieren. Stand 2026-11-24 (Runde 24, Plan B5). Was geprüft ist und was
nicht, steht unter [Stand der Prüfung](#stand-der-prüfung) und [Offen](#offen).

**Kurz:** Ein Tag `v<Version>` genügt. GitHub baut die Pakete, führt das APT-Repository fort, legt das Release
an und liefert Webseite und Repository über GitHub Pages aus. Es gibt keinen Handschritt „hochladen“ mehr.

## Überblick

| Paket | Datei im Release | Gebaut von | Stand |
|---|---|---|---|
| Debian/Ubuntu | `baruch_<Version>-1_amd64.deb` | `scripts/build-deb.sh` (cargo-deb) | lokal gebaut, entpackt, gestartet |
| AppImage | `Baruch-<Version>-x86_64.AppImage` | `packaging/appimage/build-appimage.sh` | lokal gebaut und über FUSE gestartet; in der CI nie gelaufen |
| macOS | `Baruch-<Version>-macos-arm64.dmg`, `…-x86_64.dmg` | `packaging/macos/build-dmg.sh` | nur in der CI, unsigniert |
| Windows | `Baruch-<Version>-x64.msi` | `packaging/windows/build-msi.ps1` (WiX 3) | nur in der CI, unsigniert |
| APT-Repository | `apt-repo.tar.gz` | `scripts/apt-repo.sh` | lokal über zwei Versionen geprüft, auch signiert (Wegwerfschlüssel) |
| Prüfsummen | `SHA256SUMS` | `release.yml` | nur in der CI |

| Arbeitsablauf | Auslöser | Was er tut |
|---|---|---|
| `.github/workflows/ci.yml` | Push auf `main`, von Hand | `cargo fmt --check`, Tests, clippy (`-D warnings`), `typst compile` eines Exports, `cargo deny check`, Seitenbau (ohne Auslieferung), Selbsttest der Hilfsskripte |
| `.github/workflows/release.yml` | Tag `v*`, von Hand (dann nur Pakete als Artefakte) | Pakete → APT-Repository → `SHA256SUMS` → GitHub-Release → Webseite samt `/apt/` über GitHub Pages |
| `.github/workflows/pages.yml` | Push auf `main`, von Hand | Webseite neu bauen, `/apt/` aus dem letzten veröffentlichten Release wieder einspielen, ausliefern |

Die Version steht an einer Stelle: `[workspace.package] version` in `Cargo.toml` (`scripts/version.sh` liest sie).
Ein Tag `v<Version>` muss dazu passen, sonst bricht der Bau ab; `params.version` in `site/hugo.toml` muss
ebenfalls dazu passen, sonst bricht der Seitenbau ab (`scripts/ci-site.sh`). Versionsnummern legt nur der Nutzer
fest.

Jedes Paket enthält die Lizenz (AGPL-3.0-or-later, `LICENSE`) und `THIRD-PARTY-LICENSES.txt`: alle in
`baruch-server` gelinkten Crates mit Version, Lizenz, Quelle und Lizenztexten. Die Datei entsteht beim Paketbau
aus `cargo metadata` (`scripts/third-party-licenses.py`, nur Python 3 und cargo) unter `packaging/generated/`
und wird nicht eingecheckt.

Baruch fragt nie von sich aus im Netz nach neuen Versionen. Aktualisiert wird über APT oder von Hand; die
Über-Seite (`/ueber`) verlinkt die Release-Seite.

## Ein Release machen

1. **Version setzen** (Nutzer): `version` in `Cargo.toml` und `params.version` in `site/hugo.toml`; ein Bau
   zieht `Cargo.lock` nach; committen auf `development`. Die Version am besten erst unmittelbar vor dem Tag
   anheben: Die Webseite wirbt ab dem nächsten Push auf `main` für diese Version, auch wenn es das Release noch
   nicht gibt.
2. **Fast-Forward nach `main`** (Nutzer): löst `ci.yml` und `pages.yml` aus. Erst weiter, wenn `ci.yml` grün
   ist.
3. **Tag setzen und schieben** (Nutzer):

   ```sh
   git tag -a v0.9.0 -m "Baruch 0.9.0"
   git push origin v0.9.0
   ```

4. **`release.yml` läuft** – ohne weiteres Zutun:
   1. baut die Pakete (Linux, macOS, Windows) und startet das entpackte .deb zur Probe;
   2. holt `apt-repo.tar.gz` des letzten veröffentlichten Release und übernimmt dessen Pakete (`pool/`), damit
      ältere Versionen installierbar bleiben (`scripts/ci-apt-fetch.sh`, `scripts/apt-repo.sh --base`); beim
      ersten Release beginnt das Repository leer;
   3. wartet im Job `publish` auf die **Freigabe** der Umgebung `release` (sofern eingerichtet, siehe
      Einmalige Einrichtung), vergleicht den Schlüssel aus dem Secret mit dem eingetragenen Fingerabdruck
      (`scripts/ci-fingerprint.sh`, bei Abweichung Abbruch), erzeugt das APT-Repository neu und signiert es;
   4. schreibt `SHA256SUMS` und legt das GitHub-Release **als Entwurf** an;
   5. baut die Webseite (`hugo --minify --panicOnWarning -s site`), legt das Repository nach
      `site/public/apt/` und liefert alles über GitHub Pages aus (Job `pages`).

   Linux (.deb) ist Pflicht; scheitern AppImage, macOS oder Windows, entsteht das Release trotzdem und nennt in
   der Beschreibung, was fehlt.
5. **Entwurf ansehen und veröffentlichen** (Nutzer, auf GitHub unter Releases) – und zwar **vor dem nächsten
   Push auf `main`**, siehe [Grenze bei Entwürfen](#grenze-bei-entwürfen). Wer das nicht will, setzt die
   Repository-Variable `RELEASE_DRAFT` auf `false`; dann ist das Release sofort öffentlich und die Grenze
   entfällt.

Von Hand gestartet (Actions → Release → „Run workflow“) baut `release.yml` nur die Pakete als Artefakte, ohne
Release und ohne Auslieferung – als Generalprobe vor dem ersten Tag zu empfehlen.

Ein zweiter Lauf für dasselbe Tag (Re-run) scheitert am Schritt „GitHub-Release“, weil es das Release schon
gibt: vorher das Release (oder den Entwurf) löschen.

## Auslieferung über GitHub Pages

Webseite (`https://baruch.bernau.family/`) und APT-Repository (`https://baruch.bernau.family/apt/`) sind eine
einzige Pages-Seite. Pages ersetzt bei jeder Auslieferung die ganze Seite; darum muss **jede** Auslieferung das
Repository mitbringen:

- `release.yml` (Job `pages`) nimmt das eben erzeugte Repository (als Artefakt aus dem Job `publish`).
- `pages.yml` holt `apt-repo.tar.gz` des jüngsten **veröffentlichten** Release, das das Archiv trägt, prüft es
  gegen `SHA256SUMS` desselben Release und legt es wieder nach `/apt/`.
- Gibt es noch kein solches Release, erscheint die Seite ohne `/apt/`; der Lauf meldet das als Warnung.
- Scheitert der Abruf aus einem anderen Grund (Netz, Rechte, Prüfsumme), bricht `pages.yml` ab und die alte
  Seite bleibt stehen – lieber keine neue Seite als eine ohne Repository.

Beide Abläufe teilen die Gruppe `pages` (`concurrency`): Es laufen nie zwei Auslieferungen zugleich.

Das Repository ist ansammelnd: `Packages` nennt alle bisher veröffentlichten Versionen
(`apt install baruch=0.9.0-1` bleibt möglich). Die Kette läuft über die Releases – jedes `apt-repo.tar.gz`
enthält die Pakete aller Vorgänger. Ein Paket ganz zurückziehen heißt darum: das Archiv des jüngsten Release
ohne dieses Paket neu erzeugen und am Release ersetzen.

### Grenze bei Entwürfen

Ein Token mit `contents: read` sieht Entwürfe nicht. Solange das neue Release ein Entwurf ist,

- liefert ein **Push auf `main`** (`pages.yml`) das Repository des vorigen veröffentlichten Release aus –
  `/apt/` fällt auf den alten Stand zurück (beim allerersten Release: `/apt/` verschwindet), bis der Entwurf
  veröffentlicht und `pages.yml` erneut gelaufen ist (von Hand: Actions → Webseite → „Run workflow“);
- nähme ein **weiteres Release** das vorige veröffentlichte als Grundlage; die Pakete des Entwurfs fehlten
  dann im Repository;
- zeigen die Download-Links der Webseite ins Leere (die Dateien eines Entwurfs sind nicht öffentlich).

Darum: den Entwurf zeitnah und vor dem nächsten Push auf `main` veröffentlichen. Nach dem Veröffentlichen ist
kein weiterer Schritt nötig; die Seite samt `/apt/` hat `release.yml` schon ausgeliefert.

## Einmalige Einrichtung

Diese Schritte macht der Nutzer einmal; kein Skript und kein Agent nimmt sie ihm ab.

1. **DNS:** einen CNAME-Eintrag `baruch` (in der Zone `bernau.family`) auf `beerlesklopfer.github.io` setzen.
   Der Name steht zusätzlich in `site/static/CNAME` (genau `baruch.bernau.family`; `scripts/ci-site.sh` prüft
   das bei jedem Bau).
2. **Pages-Quelle:** im Repository unter Settings → Pages → „Build and deployment“ die Quelle **„GitHub
   Actions“** wählen, die eigene Domain `baruch.bernau.family` eintragen und „Enforce HTTPS“ einschalten,
   sobald GitHub das Zertifikat ausgestellt hat.
3. **Umgebung `github-pages`:** Settings → Environments → `github-pages` → „Deployment branches and tags“.
   GitHub lässt dort üblicherweise nur den Standardzweig zu; `release.yml` liefert aber von einem Tag aus.
   Eine Regel für Tags `v*` ergänzen (neben `main`). *Annahme, nicht geprüft:* Ohne diese Regel lehnt GitHub
   den Job `pages` in `release.yml` ab („not allowed to deploy to github-pages“); der erste Lauf zeigt es.
4. **Umgebung `release` mit Schutzregeln:** Settings → Environments → „New environment“ → `release`. Dort
   „Required reviewers“ (der Nutzer selbst) einschalten und unter „Deployment branches and tags“ nur Tags
   `v*` zulassen. Der Job `publish` in `release.yml` läuft in dieser Umgebung und wartet dann bei jedem
   Release auf die Freigabe, bevor er an den Archivschlüssel kommt. *Annahme, nicht geprüft:* Schutzregeln für
   Umgebungen gibt es bei GitHub in öffentlichen Repositorys in jedem Tarif, in privaten nur in bezahlten.
   Ohne diesen Schritt legt GitHub die Umgebung beim ersten Lauf leer an – der Ablauf läuft dann, aber nichts
   schützt den Schlüssel.
5. **Secrets in dieser Umgebung** (Settings → Environments → `release` → „Add environment secret“), siehe
   nächster Abschnitt: `APT_GPG_PRIVATE_KEY` und `APT_GPG_PASSPHRASE`. **Nicht** als Repository-Secret
   anlegen: Ein Repository-Secret sieht jeder Job jedes Laufs, ein Secret der Umgebung nur der freigegebene
   Job `publish`.
6. **Fingerabdruck eintragen** – an drei Stellen denselben Wert (40 Hex-Zeichen, Leerzeichen erlaubt):
   `params.aptFingerprint` in `site/hugo.toml` (leer zeigt die Seite einen Platzhalter-Hinweis), in
   `README.md` und in dieser Datei jeweils den Platzhalter ersetzen, der mit `FINGERABDRUCK-` beginnt (Suchen
   und Ersetzen; er steht hier zweimal, im README einmal). `scripts/ci-fingerprint.sh` prüft, ob alle drei
   Stellen übereinstimmen; `release.yml` vergleicht den Wert vor dem Signieren mit dem Schlüssel aus dem Secret
   und **bricht ab**, wenn er fehlt oder abweicht – ein signiertes Repository ohne prüfbaren Fingerabdruck
   entsteht so nicht.
7. **Impressum:** `site/content/impressum.md` ist ein sichtbar markierter Platzhalter (`platzhalter: true` im
   Kopf, keine Anschrift). Die Angaben trägt der Betreiber ein und entfernt die Zeile `platzhalter: true`;
   solange sie steht, warnt jeder Seitenbau (`scripts/ci-site.sh`), liefert aber aus.

### Schlüssel für das APT-Repository

Das Repository wird mit einem Archivschlüssel signiert, dessen geheimer Teil als Secret der Umgebung `release`
bei GitHub liegt. Sein öffentlicher Teil heißt `baruch-archive-keyring.gpg` und liegt nach jedem Release unter
`https://baruch.bernau.family/apt/baruch-archive-keyring.gpg`. Die Abläufe sind an keinen bestimmten Schlüssel
gebunden: Sie signieren mit dem, der im Secret liegt, und verlangen nur, dass sein Fingerabdruck der
veröffentlichte ist.

**Welcher Schlüssel – Entscheidung des Nutzers vor dem ersten Release** (Gutachten R24):

- **Empfohlen: ein eigener Schlüssel nur für Baruch** (nur zum Signieren, mit Ablaufdatum; der Hauptschlüssel
  bleibt auf dem eigenen Rechner, ins Secret kommt nur der Teil, der signieren darf). Wird das Repository bei
  GitHub oder das Secret übernommen, ist dann nur Baruch betroffen, und wer die Baruch-Paketquelle einträgt,
  vertraut keinem Schlüssel, der noch anderes signiert.
- **Bisher vorgesehen: der ForgeIEC-Archivschlüssel** – derselbe wie für `apt.forgeiec.io`. Das spart einen
  Schlüssel, koppelt aber beide Projekte: Der geheime Schlüssel liegt dann zusätzlich in diesem Repository;
  ein Einbruch hier trifft auch `apt.forgeiec.io`, und ein Widerruf dort legt Baruch still. `Signed-By`
  begrenzt den Schlüssel bei den Nutzern auf die jeweilige Paketquelle, nicht aber den Schaden, wenn der
  geheime Teil verloren geht.

Niemand außer dem Nutzer erzeugt, wählt oder hinterlegt den Schlüssel. Fällt die Wahl auf den gemeinsamen
Schlüssel, gehört der Satz dazu auf die Seite Installation (`site/content/installation.md`, Abschnitt
„Debian und Ubuntu über die APT-Quelle“).

Fingerabdruck: `FINGERABDRUCK-TRÄGT-DER-NUTZER-EIN`

(Platzhalter. Niemand außer dem Nutzer trägt ihn ein; er wird nicht aus dem Netz geholt und nicht geraten.
Er steht außerdem im `README.md` und auf der Webseite – wer die Schlüsseldatei von der Webseite lädt, vergleicht
sie am besten mit dem Wert aus dem README im Repository, also über einen zweiten Weg. Beides liegt allerdings
bei GitHub; einen davon unabhängigen Weg – etwa den Fingerabdruck persönlich oder über einen signierten
Tag weitergeben – gibt es bisher nicht.)

- **`APT_GPG_PRIVATE_KEY`:** der geheime Schlüssel als ASCII (`gpg --armor --export-secret-keys …` auf dem
  Rechner, auf dem er liegt). Es genügt der Teil, der signieren darf.
- **`APT_GPG_PASSPHRASE`:** die Passphrase dazu (entfällt nur bei einem Schlüssel ohne Passphrase).
- Im Repository liegt kein Schlüssel, und kein Skript erzeugt einen. Beide Secrets gehören in die Umgebung
  `release` (Einmalige Einrichtung, Schritte 4 und 5); die Abläufe reichen sie nur über `env` in den einen
  Schritt, der signiert, und `scripts/ci-check-workflows.sh` lehnt `APT_GPG_*` in jedem Job ohne diese
  Umgebung ab; die Passphrase steht in einer Datei mit `umask 077` im
  Temp-Ordner des Runners (`--passphrase-file`), nie in einer Befehlszeile. Schlüsselbund und Datei werden am
  Ende des Schritts gelöscht.
- `release.yml` schreibt den öffentlichen Teil selbst aus dem importierten Schlüssel nach
  `baruch-archive-keyring.gpg` (binär, wie `Signed-By:` ihn erwartet).
- **Vor dem Signieren** vergleicht `scripts/ci-fingerprint.sh --key <Fingerabdruck des importierten
  Schlüssels>` den Schlüssel mit dem eingetragenen Fingerabdruck. Fehlt der Eintrag oder ist es ein anderer
  Schlüssel, endet der Job `publish` mit einem Fehler: kein Release, keine Auslieferung. Geprüft wird der
  Stand des Tags – ein nachträglicher Eintrag auf `main` hilft dem laufenden Tag nicht. Dann: Fingerabdruck
  eintragen, committen, nach `main` bringen, das Tag löschen und auf dem neuen Stand neu setzen (ein Release
  gab es noch nicht). Der Job `linux` warnt deshalb schon am Anfang des Laufs, wenn der Eintrag fehlt.
- **Ohne das Secret** erzeugt `release.yml` ein unsigniertes Repository und warnt; `pages` liefert es trotzdem
  aus und warnt ebenfalls. apt lehnt ein unsigniertes Repository ab – es taugt nur zur Probe.
- **Schlüssel verlängert, ersetzt oder widerrufen:** Bei einem gemeinsamen Schlüssel betrifft das ForgeIEC
  und Baruch zusammen. Secret erneuern, Fingerabdruck auf der Webseite und hier nachziehen, ein neues Release machen (oder das Repository
  von Hand neu signieren und `apt-repo.tar.gz` am jüngsten Release ersetzen, dann `pages.yml` von Hand starten).
  Nutzer müssen bei einem neuen Schlüssel die Schlüsseldatei neu holen.

Von Hand, ohne CI (z. B. zur Probe; unsigniert ohne `APT_GPG_KEY`):

```sh
scripts/apt-repo.sh --base apt-repo.tar.gz /pfad/zum/ordner baruch_0.9.1-1_amd64.deb
APT_GPG_KEY=<Fingerabdruck> scripts/apt-repo.sh /pfad/zum/ordner baruch_0.9.0-1_amd64.deb
```

### Secrets und Variablen in GitHub

| Name | Art | Wofür | Nötig |
|---|---|---|---|
| `APT_GPG_PRIVATE_KEY` | Secret der Umgebung `release` | geheimer Archivschlüssel (ASCII) | für ein signiertes Repository |
| `APT_GPG_PASSPHRASE` | Secret der Umgebung `release` | Passphrase dazu | bei einem Schlüssel mit Passphrase |
| `RELEASE_DRAFT` | Variable | `false` = Release sofort öffentlich | nein (Vorgabe: Entwurf) |
| `MACOS_SIGN` | Variable | `true` schaltet Signieren und Notarisieren ein | nein (Vorgabe: aus) |
| `MACOS_CERT_P12_BASE64`, `MACOS_CERT_PASSWORD`, `MACOS_SIGN_IDENTITY` | Secrets | Developer-ID-Zertifikat (.p12, Base64), Passwort, Name der Identität | nur mit `MACOS_SIGN=true` |
| `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD` | Secrets | Notarisierung (`notarytool`) | nur mit `MACOS_SIGN=true` |
| `WINDOWS_SIGN` | Variable | `true` schaltet das Signieren der .msi ein | nein (Vorgabe: aus) |
| `WINDOWS_CERT_PFX_BASE64`, `WINDOWS_CERT_PASSWORD` | Secrets | Code-Signing-Zertifikat (.pfx, Base64), Passwort | nur mit `WINDOWS_SIGN=true` |

Die Signierung für macOS und Windows ist vorbereitet und **abgeschaltet**; beide Pakete erscheinen unsigniert
(siehe [Offen](#offen)).

## Regeln der Arbeitsabläufe

`scripts/ci-check-workflows.sh` prüft sie bei jedem CI-Lauf (Job `skripte`) und lokal:

- Jede Aktion ist auf einen 40-stelligen Commit-SHA festgelegt, mit der Version als Kommentar
  (`uses: actions/checkout@<SHA> # v4.2.2`). Ein Tag lässt sich verschieben, ein SHA nicht.
- Kein `pull_request_target`.
- Oberste Rechte sind genau `contents: read`. `contents: write` hat nur der Job `publish` in `release.yml`;
  `pages: write` und `id-token: write` haben nur die Jobs `pages` (in `release.yml` und `pages.yml`), beide in
  der Umgebung `github-pages`.
- Der Archivschlüssel (`secrets.APT_GPG_*`) steht nur in einem Job der Umgebung `release` (Job `publish`).
  Grund: Die Schutzregeln der Umgebung (Freigabe, nur Tags `v*`) entscheiden, welcher Lauf an den Schlüssel
  kommt – nicht schon der Umstand, dass jemand ein Tag setzen kann.
- `actions/checkout` läuft mit `persist-credentials: false`.
- In `run:` steht kein `${{ … }}`; Werte und Geheimnisse kommen nur über `env` in den jeweiligen Schritt.
  Geheimnisse stehen nirgends im Repository und nie in einer Befehlszeile.
- Rust ist in jedem Ablauf in genau einer Variablen `RUST_VERSION` festgelegt (derselbe Wert wie
  `rustc --version` auf dem Entwicklungsrechner, überall gleich); kein `rust-toolchain.toml`, kein `stable`.

Festgelegte Werkzeuge (Stand 2026-11-24):

| Werkzeug | Version | Wo | Prüfsumme |
|---|---|---|---|
| Rust | 1.99.0 | `RUST_VERSION` in `ci.yml` und `release.yml` | über rustup |
| cargo-deny | 0.22.0 | `CARGO_DENY_VERSION` in `ci.yml` | Quellbau mit `--locked` (crates.io) |
| cargo-deb | 3.7.0 | `CARGO_DEB_VERSION` in `release.yml` | Quellbau mit `--locked` (crates.io) |
| Hugo | 0.168.0 (nicht „extended“) | `scripts/ci-tool.sh` | SHA-256 aus `hugo_0.168.0_checksums.txt` |
| Typst | 0.15.1 (= Crate `typst` in `Cargo.lock`) | `scripts/ci-tool.sh` | SHA-256 aus dem Feld `digest` des GitHub-Release |
| appimagetool, type2-runtime | 1.9.1, 20251108 | `packaging/appimage/build-appimage.sh` | SHA-256 aus dem Feld `digest` |
| actions/checkout | v4.2.2 | alle Abläufe | `11bd71901bbe5b1630ceea73d27597364c9af683` |
| actions/setup-node | v4.4.0 | `ci.yml`, `release.yml` | `49933ea5288caeca8642d1e84afbd3f7d6820020` |
| actions/upload-artifact | v4.6.2 | `release.yml` | `ea165f8d65b6e75b540449e92b4886f43607fa02` |
| actions/download-artifact | v4.3.0 | `release.yml` | `d3f86a106a0bac45b974a628896c90dbdf5c8093` |
| actions/upload-pages-artifact | v5.0.0 | `release.yml`, `pages.yml` | `fc324d3547104276b827a68afc52ff2a11cc49c9` |
| actions/deploy-pages | v5.0.1 | `release.yml`, `pages.yml` | `368f82528645a54fb793d4d04e342629a3f51346` |

Nicht festgelegt: `protoc`, `apt-utils`, `gnupg` und `python3-yaml` kommen aus der Paketquelle des Runners (apt,
Homebrew, Chocolatey), `gh` aus dem Abbild des Runners, Node ist die jeweils aktuelle 22, WiX das des
Windows-Runners.

## Installation für Endnutzer

Nach dem Start läuft Baruch unter `http://127.0.0.1:8765/` und ist nur vom eigenen Rechner erreichbar. Die
Daten liegen im Ordner des Benutzers (`~/.local/share/baruch`, unter Windows `%LOCALAPPDATA%\baruch`); kein
Paket legt dort etwas an oder löscht beim Entfernen etwas.

Jedes Paket enthält den Lizenztext (`LICENSE`), die Lizenzen der eingebundenen Bibliotheken
(`THIRD-PARTY-LICENSES.txt`) und das Quellcode-Angebot (`QUELLTEXT.txt` mit der Adresse des Repos und dem Tag
der Version):

| Paket | Ort |
|---|---|
| .deb | `/usr/share/doc/baruch/` (dazu `copyright`, `README.Debian`, `CHANGELOG.md`) |
| AppImage | `usr/share/doc/baruch/` im Abbild (`./Baruch-…AppImage --appimage-extract`) |
| macOS | `Baruch.app/Contents/Resources/`; `LICENSE.txt` und `QUELLTEXT.txt` auch im geöffneten Abbild |
| Windows | Installationsordner (Vorgabe `C:\Program Files\Baruch`) |

Im laufenden Programm zeigt **Werkzeuge ▾ → Über Baruch** Version, Lizenz, Quellcode und den Ort der
Drittlizenzen. Baruch fragt nie von sich aus im Netz nach neuen Versionen: Aktualisiert wird von Hand mit
einem neuen Paket von der Release-Seite, unter Debian/Ubuntu mit dem APT-Repository über `sudo apt upgrade`. Das Entfernen
eines Pakets (auch `apt purge`, Löschen von `Baruch.app`, Deinstallation unter Windows) lässt den Datenordner
unberührt; wer die Daten loswerden will, löscht ihn selbst.

### Debian und Ubuntu über das APT-Repository

Für Debian 13 und Ubuntu 24.04 oder neuer (amd64; das Paket braucht `libc6 ≥ 2.39`).

```sh
sudo curl -fsSL -o /usr/share/keyrings/baruch-archive-keyring.gpg \
  https://baruch.bernau.family/apt/baruch-archive-keyring.gpg

# Fingerabdruck vergleichen: FINGERABDRUCK-TRÄGT-DER-NUTZER-EIN
gpg --show-keys --with-fingerprint /usr/share/keyrings/baruch-archive-keyring.gpg

sudo tee /etc/apt/sources.list.d/baruch.sources >/dev/null <<'EOF'
Types: deb
URIs: https://baruch.bernau.family/apt
Suites: stable
Components: main
Architectures: amd64
Signed-By: /usr/share/keyrings/baruch-archive-keyring.gpg
EOF

sudo apt update
sudo apt install baruch
```

`Signed-By` bindet den Schlüssel an genau dieses Repository; für andere Paketquellen gilt er dadurch nicht.
Neue Versionen kommen danach mit `sudo apt upgrade`; eine ältere Version: `sudo apt install baruch=0.9.0-1`.
Entfernen: `sudo apt remove baruch`, dazu die beiden Dateien oben löschen.

### .deb direkt

```sh
sha256sum --check --ignore-missing SHA256SUMS
sudo apt install ./baruch_0.9.0-1_amd64.deb
```

Starten: „Baruch“ im Startmenü oder `baruch` im Terminal (startet den Server, falls noch keiner läuft, und
öffnet den Browser); `baruch-server --help` zeigt Import und Export. Das Paket richtet keinen Dienst ein. Wer
Baruch dauerhaft laufen lassen will, nimmt das Beispiel für einen systemd-Benutzerdienst:

```sh
install -Dm644 /usr/share/doc/baruch/examples/baruch.service ~/.config/systemd/user/baruch.service
systemctl --user daemon-reload
systemctl --user enable --now baruch.service
```

Empfohlen ist ein Schlüsselbund (gnome-keyring, KWallet oder KeePassXC als Secret Service) für die Schlüssel der
Verbindungen; `poppler-utils`, `libxml2-utils` und Java sind nur für zusätzliche Prüfungen nötig.

### AppImage

```sh
chmod +x Baruch-0.9.0-x86_64.AppImage
./Baruch-0.9.0-x86_64.AppImage            # Server starten und Browser öffnen
./Baruch-0.9.0-x86_64.AppImage --help     # Argumente gehen an baruch-server
```

Das AppImage bringt `libdbus` nicht mit (für den Schlüsselbund; auf jedem üblichen Desktop vorhanden) und
braucht wie das .deb glibc ≥ 2.39. Aktualisiert wird von Hand: neue Datei laden. Die Seite Über Baruch nennt den Pfad der
Drittlizenzen im eingehängten Abbild; er gilt nur, solange Baruch läuft.

### macOS (unsigniert)

`Baruch-<Version>-macos-arm64.dmg` (Apple Silicon) oder `…-x86_64.dmg` (Intel) öffnen und Baruch in den Ordner
„Programme“ ziehen. Das Programm ist **nicht signiert und nicht notarisiert**; macOS verweigert darum den
ersten Start. Erlauben: Systemeinstellungen → Datenschutz & Sicherheit → „Dennoch öffnen“, oder im Terminal
`xattr -dr com.apple.quarantine /Applications/Baruch.app`. Vorher die Prüfsumme mit `SHA256SUMS` vergleichen.
Baruch hat kein Fenster und kein Dock-Symbol; die Oberfläche ist der Browser. Beenden: Aktivitätsanzeige oder
`pkill baruch-server`.

### Windows (unsigniert)

`Baruch-<Version>-x64.msi` ausführen (Administratorrechte, Installation nach `C:\Program Files\Baruch`). Das
Paket ist **nicht signiert**; SmartScreen warnt („Weitere Informationen“ → „Trotzdem ausführen“). Vorher die
Prüfsumme vergleichen (`Get-FileHash Baruch-0.9.0-x64.msi -Algorithm SHA256`). Start über „Baruch“ im
Startmenü: ein minimiertes Fenster mit dem Server, dazu der Browser. Beenden: das Fenster schließen.

## Lokal bauen und prüfen

```sh
scripts/build-deb.sh                      # baut release (nice, -j 4) und das .deb unter target/debian
scripts/build-deb.sh --no-build           # nur paketieren: target/release/baruch-server muss liegen
packaging/appimage/build-appimage.sh --appdir-only     # AppDir ohne Netz
scripts/apt-repo.sh /pfad/zum/ordner target/debian/*.deb   # unsigniertes Repository zur Probe
cargo deny check licenses sources bans    # ohne Netz; `cargo deny check` holt zusätzlich die RustSec-Daten
nice -n 10 ionice -c3 cargo clippy --workspace --all-targets -j 4 -- -D warnings
scripts/ci-selftest.sh                    # Hilfsskripte und Regeln der Arbeitsabläufe, ohne Netz
scripts/ci-check-workflows.sh             # nur die Regeln der Arbeitsabläufe
scripts/ci-fingerprint.sh                 # Fingerabdruck des Archivschlüssels: Exit 3 = noch nicht eingetragen
scripts/ci-site.sh                        # Versionsabgleich und hugo --minify --panicOnWarning -s site
scripts/ci-typst.sh target/debug/baruch-server            # Import, Export, typst compile (braucht typst)
scripts/ci-typst.sh target/debug/baruch-server -          # ohne Typst-Kommandozeile: nur Import und Export
```

`scripts/ci-selftest.sh` arbeitet nur mit erfundenen Daten in einem Temp-Ordner: zwei erfundene
.deb-Versionen, ein Wegwerfschlüssel in einem eigenen `GNUPGHOME`, ein Nachbau von `gh`, eine erfundene
Hugo-Seite. Fehlt ein Werkzeug, nennt die letzte Ausgabe, was übersprungen wurde.

Ein Paket prüfen, ohne es zu installieren:

```sh
dpkg-deb -I paket.deb && dpkg-deb -c paket.deb
dpkg-deb -x paket.deb /pfad/zur/probe
/pfad/zur/probe/usr/bin/baruch-server --data /pfad/zu/probedaten --secrets memory --listen 127.0.0.1:18765
```

### Webseite lokal bauen

Die Quelle der Webseite liegt unter `site/` (Hugo, eigenes Theme `site/themes/baruch/`, kein Submodul).
Gebraucht wird Hugo ab 0.158 in der einfachen Fassung (nicht „extended“; geprüft mit 0.168.0).

```sh
hugo --minify --panicOnWarning -s site                    # Ausgabe in site/public/ (nicht eingecheckt)
hugo server -s site --bind 127.0.0.1 --port 1313          # Vorschau unter http://127.0.0.1:1313/
```

`--panicOnWarning` macht jede Warnung zum Fehler; derselbe Befehl läuft in der CI. Die Sprache steht in
`site/hugo.toml` als `locale = "de"` – der ältere Schlüssel `languageCode` ist seit Hugo 0.158 veraltet und
bricht den Bau mit `--panicOnWarning` ab. `params.version` bestimmt alle Dateinamen und Links der Seite
Installation und wird mit der Version in `Cargo.toml` nachgezogen (der Test `round24_abnahme` und
`scripts/ci-site.sh` melden eine Abweichung); `params.aptFingerprint` trägt der Nutzer ein (leer = sichtbarer
Platzhalter). Die sechs Bildschirmfotos unter `site/static/bilder/` entstehen mit
`site/tools/bildschirmfotos.py --fotos` aus einem erfundenen Werk; die Seite baut auch ohne sie. Ausgeliefert
wird nur über die CI, nie von Hand. Einzelheiten: `site/README.md`.

## Stand der Prüfung

Lokal geprüft (2026-11-24, Debian 13, Version 0.9.0; die Punkte zu Paketen, Webseite und Testlauf am
zusammengeführten Stand durch den Integrator, Neubau im Release-Profil):

- `nice -n 10 ionice -c3 cargo test --workspace -j 4` nach `cargo clean -p` aller 15 Workspace-Crates: 197
  Ergebniszeilen, 3086 bestanden, 0 gescheitert, 2 ignoriert. Darin `round24_abnahme` (Über-Seite unter einem
  erlaubten Rechnernamen; eine Version in Manifest, Programm, Webseite und Änderungsliste).
- .deb mit `scripts/build-deb.sh` gebaut (`cargo deb --locked`): `baruch_0.9.0-1_amd64.deb`, 75 MB.
  `dpkg-deb -I` zeigt Version `0.9.0-1`, `Homepage: https://baruch.bernau.family/`,
  `Depends: libc6 (>= 2.39), libdbus-1-3 (>= 1.10)`; `dpkg-deb -c` enthält `usr/bin/baruch-server`,
  `usr/bin/baruch` und unter `usr/share/doc/baruch/` `copyright`, `LICENSE` (byte-gleich mit der Datei des
  Repos), `THIRD-PARTY-LICENSES.txt`, `QUELLTEXT.txt`, `README.md`, `CHANGELOG.md`, `README.Debian`,
  `examples/baruch.service`. Das Steuerarchiv enthält nur `control` – kein Wartungsskript.
- Server aus dem entpackten Paket (`dpkg-deb -x`) mit eigenem Datenordner gestartet
  (`--listen 127.0.0.1:0 --secrets memory`, `HOME` und `XDG_*` auf leere Ordner): `/`, `/ueber`,
  `/einstellungen/daten` HTTP 200, `/ueber/lizenz` als `text/plain`; `baruch-server --version` gibt
  `baruch-server 0.9.0` aus; außerhalb des Datenordners entstand nichts.
- AppImage: AppDir aufgebaut, `AppRun` gestartet (HTTP 200). Das Abbild `Baruch-0.9.0-x86_64.AppImage` (116 MB)
  ist auch lokal entstanden (appimagetool 1.9.1 und type2-runtime 20251108 in `target/appimage/tools/`, beide
  Prüfsummen in Ordnung) und über FUSE gestartet worden (HTTP 200, `/ueber` nennt den Pfad im eingehängten
  Abbild). Nicht geprüft: Start ohne FUSE, andere Distributionen.
- Webseite: `hugo --minify --panicOnWarning -s site` ohne Warnung (10 Seiten, mit den sechs Bildschirmfotos);
  alle 135 internen Verweise der gebauten HTML-Dateien führen auf vorhandene Dateien und Anker (`/apt/`
  ausgenommen – das entsteht erst beim Release), kein `<script>`, fremde Adressen nur `github.com/Beerlesklopfer/baruch`.
  In Firefox angesehen: Start, Funktionen, Installation, Betrieb, je hell und dunkel, 1400 und 420 Pixel breit.
- `cargo check` für `x86_64-pc-windows-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin` scheitert lokal an
  der ersten C-Abhängigkeit (`zstd-sys`: kein `x86_64-w64-mingw32-gcc` bzw. kein Apple-Compiler). Ohne
  C-Compiler grün: `-p keyring` (Windows `windows-native`, Apple `apple-native`) und `-p baruch-epub`.

- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: ohne Befund (rustc/cargo 1.99.0). Dafür
  mechanische Korrekturen in 34 Dateien, Verhalten unverändert; die in `crates/baruch-rag/tests/port.rs`
  festgeschriebene Prüfsumme von `baruch-rag/src/lib.rs` ist nachgezogen. Am zusammengeführten Stand wiederholt
  (Exit 0), ebenso `cargo fmt --all -- --check`.
- `cargo deny check licenses sources bans`: in Ordnung (auch mit den keyring-Abhängigkeiten für macOS und
  Windows), mit `wildcards = "deny"` und
  `allow-wildcard-paths = true` (alle Crates tragen `publish = false`).
- `scripts/ci-selftest.sh`: 67 Zusicherungen, alle erfüllt (Nachbesserung R24; davor 51). Darin: APT-Repository über zwei erfundene
  Versionen (beide in `Packages`, Prüfsummen in `Release`), dasselbe signiert mit einem Wegwerfschlüssel samt
  Passphrase-Datei (`gpgv` gegen `baruch-archive-keyring.gpg`, Gegenprobe mit verändertem `InRelease`), Abruf
  des letzten Release gegen einen Nachbau von `gh` (kein Release, Störung, falsche Prüfsumme), Seitenbau einer
  erfundenen Hugo-Seite mit `/apt/` (samt Warnung bei fehlendem Fingerabdruck und bei einer als Platzhalter
  markierten Seite), Regeln der Arbeitsabläufe mit acht Gegenproben (neu: Archivschlüssel ohne Umgebung
  `release`), `scripts/ci-fingerprint.sh` an erfundenen Fingerabdrücken (offen, stimmig, anderer Schlüssel,
  vergessener Platzhalter, abweichende Stelle, falsche Form).
- Arbeitsabläufe: alle drei mit Python (PyYAML) gelesen; jede Aktion mit 40-stelligem SHA, per
  `git ls-remote` gegen die Tags geprüft (Tabelle oben).
- `scripts/ci-typst.sh … -`: Import und Export des erfundenen Werks `scripts/ci-data/leuchtturm.typ` mit der
  lokal gebauten Binärdatei.
- `scripts/ci-tool.sh typst ORDNER`: geladen, Prüfsumme geprüft, entpackt, `typst --version` meldet 0.15.1.
  Die SHA-256-Summen beider Archive (Hugo, Typst) sind zusätzlich von Hand nachgerechnet (`sha256sum` auf den
  geladenen Dateien) und stimmen mit `scripts/ci-tool.sh` überein; die Namen im Archiv (`hugo` bzw.
  `typst-x86_64-unknown-linux-musl/typst`) ebenso.
**Nie gelaufen** (nur in der CI prüfbar; der erste Lauf per „Run workflow“ zeigt es):

- alle drei Arbeitsabläufe selbst, auch die Auslieferung über Pages (Umgebung `github-pages` von einem Tag aus,
  eigene Domain, HTTPS);
- `typst compile` des exportierten Werks mit der Typst-Kommandozeile: Die Arbeitsumgebung der Runde ließ den
  Aufruf nicht zu (vom Entwickler zweimal und vom Integrator einmal versucht, nicht umgangen). Ob der Export mit Typst 0.15.1 von der Kommandozeile
  setzt, zeigt erst der Job `test` in `ci.yml`;
- `scripts/ci-tool.sh hugo` (das Laden von Hugo; lokal liegt Hugo 0.168.0 schon vor – die Prüfsumme des
  Archivs ist nachgerechnet, siehe oben);
- `scripts/ci-apt-fetch.sh` gegen das echte GitHub (`gh api`, `gh release download`);
- das Signieren mit dem echten Archivschlüssel;
- `cargo deny check advisories` (braucht die RustSec-Datenbank aus dem Netz);
- der Testlauf auf dem Runner: Plattenplatz und Laufzeit für Tests **und** clippy in einem Job (Grenze 240
  Minuten; die Schritte „Platz …“ zeigen den Verbrauch);
- .dmg und .msi (unsigniert; `candle`/`light`, `hdiutil`), der ganze Bau von `baruch-server` für Windows und
  macOS, der Schlüsselbund dort, die Programmsuche und der Abbruch fremder Programme unter Windows (`taskkill`);
  der Bau des AppImage **in der CI** (lokal gebaut, siehe oben); `lintian`;
- `actionlint` und `shellcheck`: fehlen auf dem Rechner und liefen **nicht**. Die Skripte sind mit `sh -n`
  geprüft und – bis auf `ci-tool.sh hugo` und den Satz in `ci-typst.sh` – ausgeführt.

## Werkzeuge anheben

- **Aktionen:** SHA zu einem Tag: `git ls-remote https://github.com/actions/checkout 'refs/tags/v4.2.2*'`
  (bei einem Tag mit Anhang gilt die Zeile mit `^{}`); SHA und Kommentar in allen Abläufen und in der Tabelle
  oben ändern. `scripts/ci-check-workflows.sh` prüft die Form, nicht, ob der SHA zum Tag gehört.
- **Rust:** `RUST_VERSION` in `ci.yml` und `release.yml` auf denselben Wert; vorher lokal mit dieser Version
  clippy laufen lassen (neue Versionen bringen neue Warnungen).
- **Hugo, Typst:** Version und SHA-256 stehen oben in `scripts/ci-tool.sh`. Hugo: Zeile
  `hugo_<Version>_linux-amd64.tar.gz` aus `hugo_<Version>_checksums.txt` des Release. Typst: Feld `digest` von
  `typst-x86_64-unknown-linux-musl.tar.xz` unter
  `https://api.github.com/repos/typst/typst/releases/tags/v<Version>`. Typst folgt dem Crate `typst` in
  `Cargo.lock`; nach einem `cargo update` meldet `scripts/ci-selftest.sh` die Abweichung.
- **appimagetool und Laufzeit:** Version und SHA-256 stehen oben in `packaging/appimage/build-appimage.sh`
  (Feld `digest` unter `https://api.github.com/repos/AppImage/appimagetool/releases/tags/<Version>`, ebenso
  `AppImage/type2-runtime`).
- **cargo-deb, cargo-deny:** `CARGO_DEB_VERSION` in `release.yml`, `CARGO_DENY_VERSION` in `ci.yml`.

## Offen

- **Signierung macOS/Windows** (Nutzerfrage): vorbereitet, abgeschaltet (`MACOS_SIGN`, `WINDOWS_SIGN`). macOS
  braucht eine Mitgliedschaft im Apple-Entwicklerprogramm (Developer-ID-Zertifikat, Notarisierung). Für Windows
  gibt es Code-Signing-Zertifikate inzwischen meist nur noch auf einem Token oder in der Cloud; der Schritt mit
  einer PFX-Datei ist dann zu ersetzen.
- **Fingerabdruck des Archivschlüssels:** Platzhalter, bis der Nutzer ihn einträgt (hier, im `README.md` und in
  `site/hugo.toml`; `scripts/ci-fingerprint.sh` prüft die drei Stellen, `release.yml` den Schlüssel dagegen).
- **Wahl des Archivschlüssels** (eigener Schlüssel für Baruch oder der gemeinsame mit ForgeIEC): Entscheidung
  des Nutzers, siehe „Schlüssel für das APT-Repository“.
- **Umgebung `release`:** Ob Schutzregeln im gewählten Tarif verfügbar sind und ob der Job mit Freigabe wie
  beschrieben wartet, zeigt erst der erste Lauf (nie gelaufen). Die Secrets für das Signieren unter macOS und
  Windows sind noch an keine Umgebung gebunden (abgeschaltet; vor dem Einschalten nachziehen).
- **Impressum der Webseite:** Platzhalter, bis der Betreiber es füllt (Einmalige Einrichtung, Schritt 7).
- **Regel für Tags in der Umgebung `github-pages`:** Annahme, siehe Einmalige Einrichtung, Schritt 3.
- **Windows und macOS bauen überhaupt?** In der CI nie versucht; der erste Lauf zeigt es.
- **Schlüsselbund auf macOS und Windows:** In der Vorarbeit war `keyring` nur mit dem Linux-Zugang gebaut
  (`sync-secret-service`). Runde 24 (Aufgabe pakete) zieht die Features `apple-native` und `windows-native`
  nach; geprüft ist das erst, wenn die Pakete in der CI gebaut und auf den Systemen gestartet wurden.
- **Ältere Distributionen:** Gebaut wird auf Ubuntu 24.04 (glibc 2.39), damit `protoc` aus der Paketquelle neu
  genug ist. Für Debian 12 oder Ubuntu 22.04 müsste auf einem älteren System gebaut und `protoc` gesondert
  beschafft werden.
- **arm64:** Das APT-Repository führt `binary-arm64` (leer); ein arm64-Paket baut noch niemand.
- **Aktualisierung:** nur über APT. AppImage, macOS und Windows melden keine neue Version (gewollt: Baruch
  fragt nie von sich aus im Netz nach).
- **Übernommene Pakete werden nicht neu geprüft:** `apt-repo.sh --base` übernimmt die .deb-Dateien des vorigen
  Archivs nach der Prüfsumme in `SHA256SUMS` desselben Release, prüft aber nicht dessen Signatur. Wer ein
  veröffentlichtes Release ändern darf, kann so ein Paket einschleusen – er könnte allerdings ebenso das
  Release selbst ersetzen.
- **Größe:** Das Repository wächst mit jeder Version um ein Paket; GitHub Pages erlaubt rund 1 GB je Seite.
  Vorher alte Versionen aus dem Archiv nehmen (siehe „Auslieferung über GitHub Pages“).
- **Drittlizenzen:** 57 Crates (u. a. typst, lance) bringen keine Lizenzdatei mit; für sie steht nur der
  SPDX-Ausdruck in der Liste. Die Texte von MIT und Apache-2.0 stehen über andere Crates trotzdem in der Datei.
- **Node und `protoc`** sind nicht auf eine genaue Version festgelegt (siehe Regeln der Arbeitsabläufe).
