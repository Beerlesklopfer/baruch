<!-- translation-of: docs/Verteilung.md source-commit: c2fb812 -->

# Verteilung {#distribution}

> Dies ist die deutsche Übersetzung von `docs/Verteilung.md` (englisches Original, maßgeblich), Stand: Commit `c2fb812`.

Wie aus dem Quelltext Pakete entstehen, wie ein Release zustande kommt, wie die Website und das APT-Repository ausgeliefert werden
und wie Endnutzer Baruch installieren. Stand 2026-11-24 (Runde 24, Plan B5). Was geprüft ist und was
nicht, steht unter [Stand der Prüfung](#state-of-verification) und [Offen](#open).

**Kurz:** Ein Tag `v<Version>` genügt. GitHub baut die Pakete, führt das APT-Repository fort, erstellt das
Release und liefert die Website und das Repository über GitHub Pages aus. Einen manuellen Schritt „Hochladen“ gibt es nicht mehr.

## Deutscher Teil der Dokumentation {#german-section-of-the-documentation}

`site/tools/docs_pages.py` baut aus `docs/de/` auch den deutschen Abschnitt /docs/ (Architektur aus ihren Teilen
`docs/de/Architektur.part<N>.md`), neben dem englischen /en/docs/. `scripts/check-doc-lag.py` meldet Übersetzungen, die
einem älteren Commit ihres englischen Originals folgen (informativ), und `scripts/ci-site.sh` führt sowohl den Generator für
beide Sprachen als auch die Prüfung auf Rückstand aus.

## Überblick {#overview}

| Paket | Datei im Release | Gebaut von | Stand |
|---|---|---|---|
| Debian/Ubuntu | `baruch_<Version>-1_amd64.deb` | `scripts/build-deb.sh` (cargo-deb) | lokal gebaut, entpackt, gestartet |
| AppImage | `Baruch-<Version>-x86_64.AppImage` | `packaging/appimage/build-appimage.sh` | lokal gebaut und über FUSE gestartet; nie in der CI gelaufen |
| macOS | `Baruch-<Version>-macos-arm64.dmg`, `…-x86_64.dmg` | `packaging/macos/build-dmg.sh` | nur in der CI, unsigniert |
| Windows | `Baruch-<Version>-x64.msi` | `packaging/windows/build-msi.ps1` (WiX 3) | nur in der CI, unsigniert |
| APT-Repository | `apt-repo.tar.gz` | `scripts/apt-repo.sh` | lokal über zwei Versionen geprüft, auch signiert (Wegwerfschlüssel) |
| Prüfsummen | `SHA256SUMS` | `release.yml` | nur in der CI |

| Workflow | Auslöser | Was er tut |
|---|---|---|
| `.github/workflows/ci.yml` | Push auf `main`, von Hand | `cargo fmt --check`, Tests, clippy (`-D warnings`), `typst compile` eines Exports mit dem eigenen Programm (`baruch-typst`, ohne Download), `cargo deny check`, Bau der Website (ohne Auslieferung), Selbsttest der Hilfsskripte |
| `.github/workflows/release.yml` | Tag `v*`, von Hand (dann nur Pakete als Artefakte) | Pakete → APT-Repository → `SHA256SUMS` → GitHub-Release → Website einschließlich `/apt/` über GitHub Pages |
| `.github/workflows/pages.yml` | Push auf `main`, Veröffentlichen eines Releases, von Hand | die Website neu bauen, `/apt/` aus dem zuletzt veröffentlichten Release wieder einsetzen (Job `pages-build`), ausliefern (Job `pages-deploy`) |

Die Version steht an einer Stelle: `[workspace.package] version` in `Cargo.toml` (`scripts/version.sh` liest sie).
Ein Tag `v<Version>` muss dazu passen, sonst bricht der Bau ab; `params.version` in `site/hugo.toml` muss
ebenfalls passen, sonst bricht der Bau der Website ab (`scripts/ci-site.sh`). Versionsnummern setzt nur der
Nutzer.

Jedes Paket enthält die Lizenz (AGPL-3.0-or-later, `LICENSE`) und `THIRD-PARTY-LICENSES.txt`: alle in
`baruch-server` gelinkten Crates mit Version, Lizenz, Quelle und Lizenztexten. Die Datei entsteht beim Paketbau
aus `cargo metadata` (`scripts/third-party-licenses.py`, nur Python 3 und cargo) unter `packaging/generated/`
und wird nicht eingecheckt.

Baruch fragt nie von sich aus das Netz nach neuen Versionen. Aktualisiert wird über APT oder von Hand; die
Seite „Über“ (`/ueber`) verlinkt auf die Release-Seite.

## Ein Release machen {#making-a-release}

1. **Version setzen** (Nutzer): `version` in `Cargo.toml` und `params.version` in `site/hugo.toml`; ein Bau
   aktualisiert `Cargo.lock`; Commit auf `development`. Die Version am besten erst unmittelbar vor dem Tag erhöhen:
   ab dem nächsten Push auf `main` bewirbt die Website diese Version, auch wenn das Release noch
   nicht existiert.
2. **Fast-Forward nach `main`** (Nutzer): löst `ci.yml` und `pages.yml` aus. Erst weitermachen, wenn `ci.yml`
   grün ist.
3. **Tag setzen und pushen** (Nutzer):

   ```sh
   git tag -a v0.9.0 -m "Baruch 0.9.0"
   git push origin v0.9.0
   ```

4. **`release.yml` läuft** – ohne weiteres Zutun:
   1. baut die Pakete (Linux, macOS, Windows) und startet das entpackte .deb als Probe;
   2. holt `apt-repo.tar.gz` des zuletzt veröffentlichten Releases und übernimmt dessen Pakete (`pool/`), damit
      ältere Versionen installierbar bleiben (`scripts/ci-apt-fetch.sh`, `scripts/apt-repo.sh --base`); beim
      ersten Release beginnt das Repository leer;
   3. wartet im Job `publish` auf die **Freigabe** der Umgebung `release` (falls eingerichtet, siehe
      Einmalige Einrichtung), vergleicht den Schlüssel aus dem Secret mit dem eingetragenen Fingerabdruck
      (`scripts/ci-fingerprint.sh`, Abbruch bei Abweichung), erzeugt das APT-Repository neu und signiert es;
   4. schreibt `SHA256SUMS` und erstellt das GitHub-Release **als Entwurf**;
   5. baut die Website (`hugo --minify --panicOnWarning -s site`), legt das Repository in
      `site/public/apt/` ab (Job `pages-build`) und liefert alles über GitHub Pages aus (Job `pages-deploy`).

   Linux (.deb) ist Pflicht; scheitern AppImage, macOS oder Windows, entsteht das Release trotzdem und nennt in
   der Beschreibung, was fehlt.
5. **Den Entwurf ansehen und veröffentlichen** (Nutzer, auf GitHub unter Releases) – und zwar **vor dem nächsten
   Push auf `main`**, siehe [Grenze bei Entwürfen](#limit-with-drafts). Außerdem gilt: `main` erst pushen,
   wenn der Release-Lauf vollständig durch ist, siehe
   [Auslieferung über GitHub Pages](#delivery-through-github-pages). Wer das nicht will, setzt die
   Repository-Variable `RELEASE_DRAFT` auf `false`; dann ist das Release sofort öffentlich und die Grenze
   entfällt.

Von Hand gestartet (Actions → Release → „Run workflow“) baut `release.yml` die Pakete nur als Artefakte, ohne
Release und ohne Auslieferung – empfohlen als Generalprobe vor dem ersten Tag.

Ein zweiter Lauf für denselben Tag (Re-run) scheitert am Schritt „GitHub-Release“, weil das Release schon
existiert: vorher das Release (oder den Entwurf) löschen.

## Auslieferung über GitHub Pages {#delivery-through-github-pages}

Die Website (`https://baruch.bernau.family/`) und das APT-Repository (`https://baruch.bernau.family/apt/`) sind eine
einzige Pages-Site. Pages ersetzt bei jeder Auslieferung die ganze Site; deshalb muss **jede** Auslieferung das
Repository mitbringen:

- `release.yml` (Job `pages-build`) nimmt das soeben erzeugte Repository (als Artefakt aus dem Job `publish`).
- `pages.yml` holt `apt-repo.tar.gz` des jüngsten **veröffentlichten** Releases, das das Archiv trägt, prüft es
  gegen `SHA256SUMS` desselben Releases und setzt es wieder in `/apt/` ein.
- Gibt es noch kein solches Release, erscheint die Site ohne `/apt/`; der Lauf meldet das als Warnung.
- Scheitert das Holen aus einem anderen Grund (Netz, Rechte, Prüfsumme), bricht `pages.yml` ab und die alte
  Site bleibt – besser keine neue Site als eine ohne das Repository.

Beide Workflows teilen die Gruppe `pages` (`concurrency`, `cancel-in-progress: false`): Zwei Jobs der Gruppe laufen nie
gleichzeitig. Mehr sichert die Gruppe nicht zu:

- `pages.yml` hält die Gruppe für den ganzen Lauf (Gruppe am Workflow). `release.yml` kann das nicht – die Gruppe
  `release-<Ref>` steht dort schon am Workflow –, also tragen `pages-build` und `pages-deploy` sie je für sich
  und geben sie dazwischen frei.
- In einer Gruppe bei GitHub wartet höchstens **ein** Job; ein neu eintreffender verdrängt den wartenden, der
  dann als abgebrochen zählt.
- Folge: Löst ein Push auf `main` `pages.yml` aus, während das Release noch läuft, kann sich dessen Lauf zwischen
  `pages-build` und `pages-deploy` des Releases schieben, und `pages-deploy` des Releases kann verdrängt werden.
  Dann steht die Auslieferung von `pages.yml` zuletzt – mit `/apt/` aus dem letzten **veröffentlichten** Release,
  also ohne das neue.

Daher die Regel: **`main` erst pushen, wenn der Release-Lauf vollständig durch ist** (Job `pages-deploy` grün) und
der Entwurf veröffentlicht wurde. Ist es trotzdem passiert: den Entwurf veröffentlichen, dann `pages.yml` von Hand starten
(Actions → Webseite → „Run workflow“); ein abgebrochenes `pages-deploy` des Releases lässt sich auch
für sich wiederholen. **Nicht gemessen:** Das folgt aus dem von GitHub beschriebenen Verhalten der Gruppen; lokal lässt es sich
nicht prüfen, und beobachtet wurde es nicht.

**Bauen und Ausliefern sind zwei Jobs** (seit Runde 32), in beiden Workflows gleich:

- `pages-build` (nur `contents: read`): Checkout, Hugo, das Repository holen, die Site bauen,
  `actions/upload-pages-artifact`.
- `pages-deploy` (`needs: [pages-build]`, Umgebung `github-pages`, `pages: write` und `id-token: write`): nur
  `actions/deploy-pages`, kein Checkout, kein Befehl.

Grund (2026-11-24, beim ersten Release beobachtet): Mit beiden Actions im selben Job fand `deploy-pages` zweimal das soeben
hochgeladene Artefakt nicht, und das Wiederholen des Jobs erzeugte ein zweites Artefakt. GitHub
selbst sieht für Pages zwei Jobs vor. Außerdem führt der Job mit Schreibrecht und OIDC-Token so keinen Code des
Repositorys mehr aus, und eine gescheiterte Auslieferung lässt sich für sich wiederholen, ohne neu zu bauen. In
`release.yml` trägt `pages-deploy` dieselbe Bedingung wie `pages-build`
(`!cancelled() && needs.pages-build.result == 'success'`), sonst zählte es als übersprungen, wenn macOS oder
Windows scheitern. **Ungeprüft:** Die Trennung ist nie gelaufen (nichts ausgelöst, kein Push); ob sie den
beobachteten Fehler behebt, zeigt erst der nächste Lauf. Die zweite Beobachtung desselben Tages – ein Tag auf
demselben Commit wie der letzte Lauf von `main` wird nicht neu ausgeliefert – löst sie nicht; dafür gibt es seit
Runde 36 den Abschnitt „Auslieferung je Commit“ weiter unten.

Das Repository ist kumulativ: `Packages` nennt alle bisher veröffentlichten Versionen
(`apt install baruch=0.9.0-1` bleibt möglich). Die Kette läuft über die Releases – jedes `apt-repo.tar.gz`
enthält die Pakete aller Vorgänger. Ein Paket ganz zurückzuziehen heißt daher: das Archiv des
jüngsten Releases ohne dieses Paket neu erzeugen und es am Release ersetzen.

### Auslieferung je Commit {#delivery-per-commit}

Beobachtung vom 2026-11-24: Der Tag des Releases lag auf demselben Commit wie der letzte Lauf von `main`, und die
Auslieferung des Release-Laufs kam nicht an – die Site zeigte weiter den Stand von `pages.yml`. Die Ursache
ist nicht gemessen (Kandidaten sind die Gruppe `pages` – der wartende Job wird verdrängt, siehe oben – und der Fehler von
`deploy-pages`, der am selben Tag beobachtet wurde).

Vorgehen seit Runde 36: `pages.yml` hat zusätzlich den Auslöser `release: types: [published]`. Sobald der
Nutzer den Entwurf veröffentlicht, läuft `pages.yml` noch einmal und liefert die Site einschließlich `/apt/` aus dem soeben
veröffentlichten Release aus – ein eigener Lauf, unabhängig vom Release-Lauf und davon, auf welchem Commit der
Tag liegt.

- **Gebaut wird `main`, nicht der Tag.** Beim Ereignis `release` zeigt der Lauf auf den Commit des Tags; der
  Checkout nennt daher `ref: main`. Sonst würde ein spät veröffentlichtes Release die Site auf den Stand
  seines Tags zurücksetzen. Bei einem Push und von Hand bleibt es beim Commit des Laufs.
- **Warum so:** Es braucht keine weitere Berechtigung. Den Workflow aus `release.yml` heraus zu starten
  (`gh workflow run`) würde dort `actions: write` erfordern – eine weitere Schreibberechtigung, entgegen der Regel „nur wo es
  nötig ist“. Außerdem sitzt der Auslöser an der richtigen Stelle: Erst mit dem Veröffentlichen ist
  `apt-repo.tar.gz` für `pages.yml` abrufbar (Leserechte).
- **Grenze `RELEASE_DRAFT=false`:** Veröffentlicht sich das Release im Release-Lauf selbst, löst das nichts aus – Ereignisse des
  `GITHUB_TOKEN` starten keine Workflows. Dann bleibt es bei den Jobs `pages-build`/`pages-deploy` des
  Release-Laufs und dem manuellen Weg.
- **Umgebung `github-pages`:** Der Lauf kommt vom Tag (`refs/tags/v…`); die Umgebung muss Tags `v*` erlauben
  (wie schon für `release.yml`, „Einmalige Einrichtung“).
- **Manueller Weg**, wenn die Site nach einem Release den neuen Stand nicht zeigt (das Release muss veröffentlicht sein):
  `gh workflow run pages.yml --ref main` oder Actions → Webseite → „Run workflow“. Ein abgebrochenes
  `pages-deploy` des Release-Laufs lässt sich auch für sich wiederholen.
- Die Gruppe `pages` und die Regel „`main` erst nach dem Release-Lauf pushen“ gelten unverändert; der neue
  Lauf reiht sich in der Gruppe ein. `scripts/ci-check-workflows.sh` erlaubt den Auslöser `release` nur als
  `types: [published]`.

**Nie durch Ausführung geprüft:** der Auslöser, der Checkout von `main` beim Ereignis `release` und die Freigabe der
Umgebung für diesen Lauf sind nur gelesen (YAML, Regelprüfung, Selbsttest) – nichts wurde ausgelöst, kein Push,
kein Tag. Ob das die Beobachtung vom 2026-11-24 behebt, zeigt erst das nächste Release.

### Grenze bei Entwürfen {#limit-with-drafts}

Ein Token mit `contents: read` sieht keine Entwürfe. Solange das neue Release ein Entwurf ist,

- liefert ein **Push auf `main`** (`pages.yml`) das Repository des vorigen veröffentlichten Releases aus –
  `/apt/` fällt auf den alten Stand zurück (beim allerersten Release: `/apt/` verschwindet), bis der Entwurf
  veröffentlicht ist und `pages.yml` erneut gelaufen ist (von Hand: Actions → Webseite → „Run workflow“);
- würde ein **weiteres Release** das vorige veröffentlichte als Basis nehmen; die Pakete des Entwurfs würden dann im
  Repository fehlen;
- zeigen die Download-Links der Website ins Leere (die Dateien eines Entwurfs sind nicht öffentlich).

Daher: den Entwurf zügig und vor dem nächsten Push auf `main` veröffentlichen. Nach dem Veröffentlichen
ist kein weiterer Schritt nötig: `release.yml` hat die Site einschließlich `/apt/` bereits ausgeliefert, und das Veröffentlichen
löst `pages.yml` noch einmal aus („Auslieferung je Commit“).

## Einmalige Einrichtung {#one-time-setup}

Der Nutzer macht diese Schritte einmal; kein Skript und kein Agent nimmt sie ihm ab.

1. **DNS:** einen CNAME-Eintrag `baruch` (in der Zone `bernau.family`) auf `beerlesklopfer.github.io` setzen.
   Der Name steht zusätzlich in `site/static/CNAME` (genau `baruch.bernau.family`; `scripts/ci-site.sh` prüft
   das bei jedem Bau).
2. **Pages-Quelle:** im Repository unter Settings → Pages → „Build and deployment“ die Quelle **„GitHub
   Actions“** wählen, die eigene Domain `baruch.bernau.family` eintragen und „Enforce HTTPS“ einschalten,
   sobald GitHub das Zertifikat ausgestellt hat.
3. **Umgebung `github-pages`:** Settings → Environments → `github-pages` → „Deployment branches and tags“.
   GitHub erlaubt dort meist nur den Standardzweig; `release.yml` liefert aber von einem Tag aus.
   Eine Regel für Tags `v*` hinzufügen (neben `main`). *Annahme, nicht geprüft:* Ohne diese Regel verweigert GitHub
   den Job `pages-deploy` in `release.yml` („not allowed to deploy to github-pages“); der erste Lauf zeigt es.
4. **Umgebung `release` mit Schutzregeln:** Settings → Environments → „New environment“ → `release`. Dort
   „Required reviewers“ einschalten (der Nutzer selbst) und unter „Deployment branches and tags“ nur Tags
   `v*` erlauben. Der Job `publish` in `release.yml` läuft in dieser Umgebung und wartet dann bei jedem
   Release auf die Freigabe, bevor er an den Archivschlüssel kommt. *Annahme, nicht geprüft:* Schutzregeln für
   Umgebungen gibt es bei GitHub in öffentlichen Repositorys in jedem Tarif, in privaten nur in bezahlten Tarifen.
   Ohne diesen Schritt legt GitHub die Umgebung beim ersten Lauf leer an – der Workflow läuft dann, aber nichts
   schützt den Schlüssel.
5. **Secrets in dieser Umgebung** (Settings → Environments → `release` → „Add environment secret“), siehe
   den nächsten Abschnitt: `APT_GPG_PRIVATE_KEY` und `APT_GPG_PASSPHRASE`. **Nicht** als Repository-Secrets anlegen:
   ein Repository-Secret sieht jeder Job jedes Laufs, ein Secret der Umgebung nur der freigegebene
   Job `publish`.
6. **Den Fingerabdruck eintragen** – derselbe Wert an drei Stellen (40 Hex-Zeichen, Leerzeichen erlaubt):
   `params.aptFingerprint` in `site/hugo.toml` (ist er leer, zeigt die Site einen Platzhalterhinweis), in
   `README.md` und in dieser Datei den Platzhalter, der mit `FINGERABDRUCK-` beginnt, ersetzen (Suchen
   und Ersetzen; er steht hier zweimal, in der README einmal). `scripts/ci-fingerprint.sh` prüft, ob alle drei
   Stellen übereinstimmen; `release.yml` vergleicht den Wert vor dem Signieren mit dem Schlüssel aus dem Secret
   und **bricht ab**, wenn er fehlt oder abweicht – so entsteht ein signiertes Repository ohne prüfbaren Fingerabdruck
   nicht.
7. **Impressum:** `site/content/de/impressum.md` ist ein sichtbar gekennzeichneter Platzhalter (`platzhalter: true` im
   Kopf, keine Adresse). Der Betreiber trägt die Angaben ein und entfernt die Zeile `platzhalter: true`;
   solange sie steht, warnt jeder Bau der Site (`scripts/ci-site.sh`), liefert aber aus.

### Schlüssel für das APT-Repository {#key-for-the-apt-repository}

Das Repository wird mit einem Archivschlüssel signiert, dessen geheimer Teil als Secret der Umgebung `release`
bei GitHub liegt. Sein öffentlicher Teil heißt `baruch-archive-keyring.gpg` und liegt nach jedem Release unter
`https://baruch.bernau.family/apt/baruch-archive-keyring.gpg`. Die Workflows sind an keinen bestimmten Schlüssel gebunden:
Sie signieren mit dem im Secret und verlangen nur, dass dessen Fingerabdruck der
veröffentlichte ist.

**Welcher Schlüssel – Entscheidung des Nutzers vor dem ersten Release** (Review R24):

- **Empfohlen: ein eigener Schlüssel nur für Baruch** (nur zum Signieren, mit Ablaufdatum; der Hauptschlüssel
  bleibt auf dem eigenen Rechner, nur der Teil, der signieren darf, kommt ins Secret). Wird das Repository bei
  GitHub oder das Secret übernommen, ist dann nur Baruch betroffen, und wer die Paketquelle von Baruch hinzufügt,
  vertraut keinem Schlüssel, der auch andere Dinge signiert.
- **Bisher geplant: der Archivschlüssel von ForgeIEC** – derselbe wie für `apt.forgeiec.io`. Das spart einen
  Schlüssel, koppelt aber beide Projekte: Der geheime Schlüssel liegt dann zusätzlich in diesem Repository;
  ein Einbruch hier trifft auch `apt.forgeiec.io`, und ein Widerruf dort legt Baruch lahm. `Signed-By`
  begrenzt den Schlüssel für die Nutzer auf die jeweilige Paketquelle, nicht aber den Schaden, wenn der
  geheime Teil verloren geht.

Niemand außer dem Nutzer erzeugt, wählt oder hinterlegt den Schlüssel. Fällt die Wahl auf den gemeinsamen
Schlüssel, gehört der Satz dazu auf die Installationsseite (`site/content/de/installation.md`, Abschnitt
„Debian und Ubuntu über die APT-Quelle“).

Fingerabdruck: `BEBD FEF8 F980 2EC5 8A90  B818 9A9C 9199 6BB8 8DB4`

(Platzhalter. Niemand außer dem Nutzer trägt ihn ein; er wird nicht aus dem Netz geholt und nicht erraten.
Er steht auch in `README.md` und auf der Website – wer die Schlüsseldatei von der Website lädt, vergleicht
sie am besten mit dem Wert aus der README im Repository, also über einen zweiten Weg. Beide liegen aber
bei GitHub; ein davon unabhängiger Weg – etwa die persönliche Weitergabe des Fingerabdrucks oder ein signierter
Tag – existiert bisher nicht.)

- **`APT_GPG_PRIVATE_KEY`:** der geheime Schlüssel als ASCII (`gpg --armor --export-secret-keys …` auf dem
  Rechner, auf dem er liegt). Der Teil, der signieren darf, genügt.
- **`APT_GPG_PASSPHRASE`:** die Passphrase dazu (entfällt nur bei einem Schlüssel ohne Passphrase).
- Im Repository liegt kein Schlüssel, und kein Skript erzeugt einen. Beide Secrets gehören in die Umgebung
  `release` (Einmalige Einrichtung, Schritte 4 und 5); die Workflows reichen sie nur über `env` an den einen
  Schritt weiter, der signiert, und `scripts/ci-check-workflows.sh` verweigert `APT_GPG_*` in jedem Job ohne diese
  Umgebung; die Passphrase steht in einer Datei mit `umask 077` im
  Temp-Ordner des Runners (`--passphrase-file`), nie in einer Befehlszeile. Schlüsselbund und Datei werden am
  Ende des Schritts gelöscht.
- `release.yml` schreibt den öffentlichen Teil selbst aus dem importierten Schlüssel nach
  `baruch-archive-keyring.gpg` (binär, wie `Signed-By:` es erwartet).
- **Vor dem Signieren** vergleicht `scripts/ci-fingerprint.sh --key <Fingerabdruck des importierten
  Schlüssels>` den Schlüssel mit dem eingetragenen Fingerabdruck. Fehlt der Eintrag oder ist es ein anderer
  Schlüssel, endet der Job `publish` mit einem Fehler: kein Release, keine Auslieferung. Geprüft wird der
  Stand des Tags – ein späterer Eintrag auf `main` hilft dem laufenden Tag nicht. Dann: den Fingerabdruck eintragen,
  committen, nach `main` bringen, den Tag löschen und auf dem neuen Stand neu setzen (es gab noch kein
  Release). Der Job `linux` warnt deshalb schon zu Beginn des Laufs, wenn der Eintrag fehlt.
- **Ohne das Secret** erzeugt `release.yml` ein unsigniertes Repository und warnt; `pages-deploy` liefert es trotzdem aus
  und warnt ebenfalls. apt verweigert ein unsigniertes Repository – es taugt nur für eine Probe.
- **Schlüssel verlängert, ersetzt oder widerrufen:** Bei einem gemeinsamen Schlüssel betrifft das ForgeIEC
  und Baruch zusammen. Das Secret erneuern, den Fingerabdruck auf der Website und hier aktualisieren, ein neues Release machen (oder das Repository
  von Hand neu signieren und `apt-repo.tar.gz` am jüngsten Release ersetzen, dann `pages.yml` von Hand starten).
  Bei einem neuen Schlüssel müssen die Nutzer die Schlüsseldatei neu holen.

Von Hand, ohne CI (z. B. als Probe; unsigniert ohne `APT_GPG_KEY`):

```sh
scripts/apt-repo.sh --base apt-repo.tar.gz /pfad/zum/ordner baruch_0.9.1-1_amd64.deb
APT_GPG_KEY=<Fingerabdruck> scripts/apt-repo.sh /pfad/zum/ordner baruch_0.9.0-1_amd64.deb
```

### Secrets und Variablen in GitHub {#secrets-and-variables-in-github}

| Name | Art | Wofür | Nötig |
|---|---|---|---|
| `APT_GPG_PRIVATE_KEY` | Secret der Umgebung `release` | geheimer Archivschlüssel (ASCII) | für ein signiertes Repository |
| `APT_GPG_PASSPHRASE` | Secret der Umgebung `release` | Passphrase dazu | für einen Schlüssel mit Passphrase |
| `RELEASE_DRAFT` | Variable | `false` = Release sofort öffentlich | nein (Standard: Entwurf) |
| `MACOS_SIGN` | Variable | `true` schaltet Signieren und Notarisieren ein | nein (Standard: aus) |
| `MACOS_CERTIFICATE_P12_BASE64`, `MACOS_CERTIFICATE_PASSWORD` | Secrets | Zertifikat „Developer ID Application“ mit privatem Schlüssel (.p12, `base64 -w0`), Passwort – dieselben Werte wie für Onesimus | nur mit `MACOS_SIGN=true` |
| `MACOS_SIGN_IDENTITY` | Secret | Name oder SHA-1 der Identität | nein (Standard: erste „Developer ID Application“ des Zertifikats) |
| `APPSTORE_API_KEY_ID`, `APPSTORE_API_ISSUER_ID`, `APPSTORE_API_KEY_P8_BASE64` | Secrets | App-Store-Connect-Schlüssel für `notarytool` (`AuthKey_<id>.p8`, `base64 -w0`) – wie für Onesimus | ohne: signiert, nicht notarisiert (Warnung) |
| `WINDOWS_SIGN` | Variable | `true` schaltet das Signieren der .msi ein | nein (Standard: aus) |
| `WINDOWS_CERT_PFX_BASE64`, `WINDOWS_CERT_PASSWORD` | Secrets | Code-Signing-Zertifikat (.pfx, Base64), Passwort | nur mit `WINDOWS_SIGN=true` |

Das Signieren für macOS und Windows ist vorbereitet und **ausgeschaltet**; beide Pakete erscheinen unsigniert
(siehe [Offen](#open)).

## Regeln der Workflows {#rules-of-the-workflows}

`scripts/ci-check-workflows.sh` prüft sie bei jedem CI-Lauf (Job `skripte`) und lokal:

- Jede Action ist auf eine 40-stellige Commit-SHA festgelegt, mit der Version als Kommentar
  (`uses: actions/checkout@<SHA> # v4.2.2`). Ein Tag lässt sich verschieben, eine SHA nicht.
- Kein `pull_request_target`. Der Auslöser `release` nur als `types: [published]` (nur `pages.yml` verwendet ihn).
- Die Berechtigungen auf oberster Ebene sind genau `contents: read`. Nur der Job `publish` in `release.yml` hat `contents: write`;
  nur die Jobs `pages-deploy` (in `release.yml` und `pages.yml`) haben `pages: write` und `id-token: write`,
  beide in der Umgebung `github-pages`.
- Bauen und Ausliefern der Website sind getrennt: Der Job der Umgebung `github-pages` besteht nur aus
  `actions/deploy-pages` und hängt (`needs`) an einem Job derselben Datei, der `actions/upload-pages-artifact` ausführt
  und keine Schreibrechte hat. Die Prüfung verweigert beides im selben Job (Grund: „Auslieferung über
  GitHub Pages“; Gegenprobe in `scripts/ci-selftest.sh`).
- Der Archivschlüssel (`secrets.APT_GPG_*`) steht nur in einem Job der Umgebung `release` (Job `publish`).
  Grund: Die Schutzregeln der Umgebung (Freigabe, nur Tags `v*`) entscheiden, welcher Lauf an den Schlüssel kommt
  – nicht allein die Tatsache, dass jemand einen Tag setzen kann.
- `actions/checkout` läuft mit `persist-credentials: false`.
- In `run:` steht kein `${{ … }}`; Werte und Secrets kommen nur über `env` in den jeweiligen Schritt.
  Secrets stehen nirgends im Repository und nie in einer Befehlszeile.
- Rust ist in jedem Workflow in genau einer Variablen `RUST_VERSION` festgelegt (derselbe Wert wie
  `rustc --version` auf dem Entwicklungsrechner, überall gleich); keine `rust-toolchain.toml`, kein `stable`.

Festgelegte Werkzeuge (Stand 2026-11-24):

| Werkzeug | Version | Wo | Prüfsumme |
|---|---|---|---|
| Rust | 1.99.0 | `RUST_VERSION` in `ci.yml` und `release.yml` | über rustup |
| cargo-deny | 0.22.0 | `CARGO_DENY_VERSION` in `ci.yml` | Quellbau mit `--locked` (crates.io) |
| cargo-deb | 3.7.0 | `CARGO_DEB_VERSION` in `release.yml` | Quellbau mit `--locked` (crates.io) |
| Hugo | 0.168.0 (nicht „extended“) | `scripts/ci-tool.sh` | SHA-256 aus `hugo_0.168.0_checksums.txt` |
| appimagetool, type2-runtime | 1.9.1, 20251108 | `packaging/appimage/build-appimage.sh` | SHA-256 aus dem Feld `digest` |
| actions/checkout | v4.2.2 | alle Workflows | `11bd71901bbe5b1630ceea73d27597364c9af683` |
| actions/setup-node | v4.4.0 | `ci.yml`, `release.yml` | `49933ea5288caeca8642d1e84afbd3f7d6820020` |
| actions/upload-artifact | v4.6.2 | `release.yml` | `ea165f8d65b6e75b540449e92b4886f43607fa02` |
| actions/download-artifact | v4.3.0 | `release.yml` | `d3f86a106a0bac45b974a628896c90dbdf5c8093` |
| actions/upload-pages-artifact | v5.0.0 | `release.yml`, `pages.yml` | `fc324d3547104276b827a68afc52ff2a11cc49c9` |
| actions/deploy-pages | v5.0.1 | `release.yml`, `pages.yml` | `368f82528645a54fb793d4d04e342629a3f51346` |

Typst steht seit Runde 36 nicht mehr in der Tabelle: Die CI lädt keine Typst-Kommandozeile, sie setzt mit dem
eigenen Programm (`baruch-typst`, Abschnitt „Typst-Kommandozeile“).

Nicht festgelegt: `protoc`, `apt-utils`, `gnupg` und `python3-yaml` kommen aus der Paketquelle des Runners (apt,
Homebrew, Chocolatey), `gh` aus dem Image des Runners, Node ist die jeweils aktuelle 22, WiX das des
Windows-Runners.

## Installation für Endnutzer {#installation-for-end-users}

Nach dem Start läuft Baruch unter `http://127.0.0.1:8765/` und ist nur vom eigenen Rechner aus erreichbar. Die
Daten liegen im Ordner des Nutzers (`~/.local/share/baruch`, unter Windows `%LOCALAPPDATA%\baruch`); kein
Paket legt dort etwas an oder löscht dort beim Entfernen etwas.

Jedes Paket enthält den Lizenztext (`LICENSE`), die Lizenzen der mitgelieferten Bibliotheken
(`THIRD-PARTY-LICENSES.txt`) und das Angebot des Quelltexts (`QUELLTEXT.txt` mit der Adresse des Repos und dem Tag
der Version):

| Paket | Ort |
|---|---|
| .deb | `/usr/share/doc/baruch/` (dazu `copyright`, `README.Debian`, `CHANGELOG.md`) |
| AppImage | `usr/share/doc/baruch/` im Image (`./Baruch-…AppImage --appimage-extract`) |
| macOS | `Baruch.app/Contents/Resources/`; `LICENSE.txt` und `QUELLTEXT.txt` auch im geöffneten Image |
| Windows | Installationsordner (Standard `C:\Program Files\Baruch`) |

Im laufenden Programm zeigt **„Baruch ▾ → Über Baruch“** Version, Lizenz, Quelltext und den Ort der
Fremdlizenzen. Baruch fragt nie von sich aus das Netz nach neuen Versionen: Aktualisiert wird von Hand mit
einem neuen Paket von der Release-Seite, unter Debian/Ubuntu mit dem APT-Repository über `sudo apt upgrade`. Das Entfernen
eines Pakets (auch `apt purge`, Löschen von `Baruch.app`, Deinstallieren unter Windows) lässt den Datenordner
unberührt; wer die Daten loswerden will, löscht sie selbst.

### Debian und Ubuntu über das APT-Repository {#debian-and-ubuntu-through-the-apt-repository}

Für Debian 13 und Ubuntu 24.04 oder neuer (amd64; das Paket braucht `libc6 ≥ 2.39`).

```sh
sudo curl -fsSL -o /usr/share/keyrings/baruch-archive-keyring.gpg \
  https://baruch.bernau.family/apt/baruch-archive-keyring.gpg

# compare the fingerprint: BEBD FEF8 F980 2EC5 8A90  B818 9A9C 9199 6BB8 8DB4
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

`Signed-By` bindet den Schlüssel an genau dieses Repository; er gilt daher nicht für andere Paketquellen.
Neue Versionen kommen dann mit `sudo apt upgrade`; eine ältere Version: `sudo apt install baruch=0.9.0-1`.
Entfernen: `sudo apt remove baruch`, dazu die beiden Dateien oben löschen.

### .deb direkt {#deb-directly}

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

Ein Schlüsselbund (gnome-keyring, KWallet oder KeePassXC als Secret Service) wird für die Schlüssel der
Verbindungen empfohlen; `poppler-utils`, `libxml2-utils` und Java werden nur für zusätzliche Prüfungen gebraucht.

### AppImage {#appimage}

```sh
chmod +x Baruch-0.9.0-x86_64.AppImage
./Baruch-0.9.0-x86_64.AppImage            # start the server and open the browser
./Baruch-0.9.0-x86_64.AppImage --help     # arguments go to baruch-server
```

Das AppImage bringt `libdbus` nicht mit (für den Schlüsselbund; auf jedem üblichen Desktop vorhanden) und
braucht wie das .deb glibc ≥ 2.39. Aktualisiert wird von Hand: die neue Datei laden. Die Seite „Über Baruch“ nennt den Pfad der
Fremdlizenzen im eingehängten Image; er gilt nur, solange Baruch läuft.

### macOS (unsigniert) {#macos-unsigned}

`Baruch-<Version>-macos-arm64.dmg` (Apple Silicon) oder `…-x86_64.dmg` (Intel) öffnen und Baruch in den Ordner
„Programme“ ziehen. Seit 0.9.0 ist das Programm mit der Developer ID signiert
und von Apple notarisiert (Ticket angeheftet); macOS fragt nur beim ersten Start. Ohne die Secrets
(Abschnitt oben) entsteht ein unsigniertes Paket – dann: „Systemeinstellungen → Datenschutz & Sicherheit →
Dennoch öffnen“ oder `xattr -dr com.apple.quarantine /Applications/Baruch.app`. Die Prüfsumme vorher mit `SHA256SUMS` vergleichen.
Baruch hat kein Dock-Symbol; die Benutzeroberfläche ist der Browser (wenn möglich als eigenes App-Fenster, siehe
„Baruch beenden“). Beenden: „Baruch ▾ → Baruch beenden…“ in der Benutzeroberfläche; notfalls die Aktivitätsanzeige oder
`pkill baruch-server` (seit Runde 32 fährt auch das sauber herunter).

### Windows (unsigniert) {#windows-unsigned}

`Baruch-<Version>-x64.msi` ausführen (Administratorrechte, Installation nach `C:\Program Files\Baruch`). Das
Paket ist **nicht signiert**; SmartScreen warnt („Weitere Informationen“ → „Trotzdem ausführen“). Die
Prüfsumme vorher vergleichen (`Get-FileHash Baruch-0.9.0-x64.msi -Algorithm SHA256`). Start über „Baruch“ im
Startmenü: ein minimiertes Fenster mit dem Server, dazu der Browser (Edge als eigenes App-Fenster, sonst der
Standardbrowser). Beenden: „Baruch ▾ → Baruch beenden…“ in der Benutzeroberfläche oder das minimierte Fenster schließen.

## Typst-Kommandozeile {#typst-command-line}

Seit Runde 36 ist Baruch zugleich die Typst-Kommandozeile – dasselbe Programm, dieselbe eingebaute Engine (Typst
0.15.1) und dieselben eingebetteten Schriften wie im Server, ohne zweite Binärdatei:

```sh
baruch-typst compile buch.typ                 # → buch.pdf next to the input
baruch-typst compile buch.typ aus/buch.pdf --root . --font-path ~/schriften
baruch-typst compile buch.typ "seite-{0p}.png" --ppi 300      # one image per page
baruch-typst fonts                            # family names, one per line
baruch-typst --version                        # typst 0.15.1 (Baruch <Version>)
baruch-server typst compile buch.typ          # the same without a symlink
```

Das Programm wird zur Typst-Kommandozeile, wenn es unter dem Namen `baruch-typst` oder `typst` aufgerufen wird
(Symlink; unter Windows zählt auch `.exe`, Groß- und Kleinschreibung spielen keine Rolle) oder wenn das erste Argument `typst` ist
(`baruch-server typst …`). Es braucht dann keinen Datenordner, startet keinen Server und berührt das Netz nicht.
Gedacht ist es für den Typst-Ordner eines Exports (`baruch-server export <Werk> --out ORDNER` oder der
Git-Spiegel des Werks): Er lässt sich ohne ein eigenes Typst setzen.

| Option | Bedeutung |
|---|---|
| `compile EINGABE [AUSGABE]` (kurz `c`) | setzt die Datei; ohne AUSGABE entsteht die Eingabe mit der Endung des Formats |
| `--root ORDNER` | Wurzelordner des Projekts (Standard: Ordner der Eingabe). Dateien außerhalb sind gesperrt, auch über `../` oder einen Symlink |
| `--font-path ORDNER` | weitere Schriften (wiederholbar; auch die Umgebungsvariable `TYPST_FONT_PATHS`), zusätzlich zu den eingebetteten und den Systemschriften |
| `--format pdf\|png\|svg` (kurz `-f`) | ohne Angabe nach der Endung der Ausgabe, sonst PDF |
| `--ppi ZAHL` | Auflösung für PNG (Standard 144) |
| `{p}`, `{0p}`, `{t}` im Namen der Ausgabe | Seitenzahl, Seitenzahl mit führenden Nullen, Seitenanzahl. PNG und SVG mit mehreren Seiten verlangen eine dieser Marken, sonst Exit 1 |
| `fonts [--font-path ORDNER]` | Familiennamen der verfügbaren Schriften |
| `--version`, `--help` | `typst <Typst-Version> (Baruch <Version>)`; Hilfe auf Deutsch |

Exit-Codes: 0 Erfolg; 1 Satz- oder Dateifehler (Meldungen als `datei:zeile:spalte: error: text` auf der
Fehlerausgabe, Warnungen ebenso mit `warning:`); 2 falscher Aufruf oder nicht unterstützt.

Unterschiede zu Typst und zum Export von Baruch:

- **`compile` überschreibt** eine vorhandene Ausgabedatei, wie Typst. Der Export von Baruch
  (`baruch-server export … --pdf DATEI`) tut das nie. Geschrieben wird erst nach erfolgreichem Satz: Bei
  einem Fehler entsteht keine Datei, und eine vorhandene bleibt, wie sie war.
- **Keine Pakete, kein Netz:** `#import "@preview/…"` endet mit Exit 1 und der Meldung „Typst-Pakete werden
  nicht geladen (offline)“ – wie im Server.
- **Nicht unterstützt** (Exit 2, deutsche Meldung „… unterstützt Baruch als Typst-Alias nicht“, nichts wird
  gelesen oder geschrieben): `watch`, `query`, `init`, `update`, `eval`, `completions`, `info`, Eingabe von der
  Standardeingabe (`-`), Ausgabe auf die Standardausgabe, HTML und jeder weitere Schalter von Typst
  (`--input`, `--pages`, `--pdf-standard`, `--open`, `--jobs` …). Ein Schalter wird nie stillschweigend übersprungen.
- Baruch setzt ein Werk selbst (`export --pdf`, Satzvorschau) mit den Bildern aus der Datenbank; die
  Kommandozeile liest nur Dateien von der Platte.

Was die Pakete mitbringen – **nie eine Datei namens `typst`**, ein eigenes Typst bleibt unberührt:

| Paket | Typst-Kommandozeile |
|---|---|
| .deb | `/usr/bin/baruch-typst`, Symlink auf `baruch-server` (cargo-deb `preserve-symlinks`, Quelle `packaging/linux/baruch-typst`) |
| AppImage | im Image `usr/bin/baruch-typst` (Symlink). Von außen: `./Baruch-…AppImage typst compile …` oder ein eigener Symlink `ln -s /pfad/Baruch-…AppImage ~/bin/baruch-typst` – `AppRun` erkennt den Namen an `ARGV0` |
| macOS | `Baruch.app/Contents/MacOS/baruch-typst` (Symlink), Aufruf z. B. `/Applications/Baruch.app/Contents/MacOS/baruch-typst compile buch.typ` |
| Windows | `baruch-typst.cmd` neben `baruch-server.exe` (ruft `baruch-server.exe typst %*` auf); der Installationsordner liegt nicht im `PATH` |

Wer das Programm lieber als `typst` aufruft, legt den Symlink selbst an (`ln -s "$(command -v baruch-server)"
~/bin/typst`). Der Name `baruch-typst` meint hier den Befehl; die gleichnamige Crate im Quelltext
(`crates/baruch-typst`, Werkvorlage und Gliederung) ist etwas anderes.

Quelle: Argumente in `crates/baruch-server/src/typst_cli.rs`, Satz in `crates/baruch-render/src/cli.rs`;
`TYPST_VERSION` dort folgt der Crate `typst` in `Cargo.lock` (die Tests `baruch-render/tests/cli.rs` und
`round36_typst.rs` halten das fest).

## Baruch beenden {#quitting-baruch}

Seit Runde 32 beendet sich Baruch über die Benutzeroberfläche: **„Baruch ▾ → Baruch beenden…“** öffnet die Bestätigung
`/beenden`, der Knopf dort schickt ein gewöhnliches Formular ab (ohne JavaScript benutzbar). Danach zeigt das
Fenster „Baruch ist beendet – dieses Fenster kann geschlossen werden“ und versucht, sich selbst zu schließen.

- **Nur am eigenen Rechner:** Die Gegenstelle muss Loopback sein, und die Anfrage darf nicht unter einem
  zusätzlich erlaubten Rechnernamen kommen (`--allow-host`, Reverse-Proxy). Sonst antwortet Baruch mit 403 und
  einer Erklärung. Es gibt dafür weder eine RPC-Methode noch ein MCP-Werkzeug. **Grenze:** Ein Reverse-Proxy auf
  demselben Rechner, der `Host` auf `127.0.0.1` oder `localhost` umschreibt (bei nginx der Standard, siehe
  `docs/Betrieb.md`, „Host names and proxy“), sieht für Baruch wie der eigene Rechner aus – dann kann jeder,
  der durch den Proxy kommt, Baruch beenden. Der Proxy muss `Host` unverändert durchreichen (aus dem Quelltext gelesen,
  `shutdown::own_machine`; nicht nachgestellt).
- **Sauberes Herunterfahren** auch bei Signalen: Strg+C, unter Unix `SIGTERM` (`systemctl --user stop`, `kill`),
  unter Windows Strg+Pause, Schließen des Konsolenfensters und Herunterfahren des Systems. Laufende
  Antworten werden beendet (höchstens 10 Sekunden; offene Ströme wie der Live-Chat werden danach gekappt), die
  Datenbank wird geschlossen, die letzte Ausgabezeile lautet „Baruch beendet.“, der Prozess endet mit
  Status 0. `SIGHUP` fängt Baruch bewusst nicht ab (sonst ginge `nohup` nicht mehr).
- **Dienst (systemd):** Die Units (`packaging/systemd/baruch.service`, `packaging/debian/baruch.service`) haben
  `Restart=on-failure`. Ein Ende mit Status 0 ist kein Fehler: Nach „Baruch beenden“ bleibt der Dienst aus, bis
  `systemctl --user start baruch.service` (oder die nächste Anmeldung) ihn wieder startet. Wer `Restart=always`
  einträgt, macht den Knopf wirkungslos.

### Fenster und Starter {#window-and-launcher}

Ein Skript darf ein Browserfenster nur schließen, wenn der Browser es erlaubt – in der Regel bei Fenstern, die
ein Skript geöffnet hat, und bei App-Fenstern. Deshalb öffnen die Starter Baruch möglichst als **App-Fenster**
eines Chromium-artigen Browsers (`--app=<Adresse>`: ein eigenes Fenster ohne Adressleiste). Gesteuert wird das über
`BARUCH_BROWSER`:

| Wert | Wirkung |
|---|---|
| `app` (Standard) | App-Fenster, wenn ein passender Browser gefunden wird; sonst der Standardbrowser |
| `standard` | immer der Standardbrowser (`xdg-open`, `open`, `start`) |

`BARUCH_NO_BROWSER=1` öffnet weiterhin gar kein Fenster.

| Plattform | Starter | App-Fenster mit | Stand |
|---|---|---|---|
| Linux (.deb, AppImage) | `packaging/linux/baruch` (im AppImage hinter `AppRun`) | `chromium`, `chromium-browser`, `google-chrome`, `google-chrome-stable`, `brave-browser`, `microsoft-edge` aus dem `PATH` (in dieser Reihenfolge) | **Geprüft** mit Attrappen im `PATH`: Wahl des Programms, `--app=<Adresse>`, Rückfall auf `xdg-open`, beide Schalter (`tests/round32_beenden.rs`); `bash -n`. **Nicht geprüft:** ein echter Browser (in dieser Runde wurde keiner gestartet), also auch nicht, ob `window.close()` das App-Fenster wirklich schließt – eine Annahme auf Grundlage des bekannten Verhaltens von Chromium. `shellcheck` war nicht installiert. |
| macOS | dasselbe Skript wie `Contents/MacOS/baruch` | die Programme „Google Chrome“, „Chromium“, „Brave Browser“, „Microsoft Edge“ unter `/Applications` und `~/Applications` (`BARUCH_APPLICATIONS` nennt andere Ordner) | **Geprüft** nur unter Linux nachgestellt (Attrappe für `uname`, Programmordner im Temp-Verzeichnis). **Angenommen:** dass der direkte Aufruf des Programms im Bundle mit `--app=` ein App-Fenster öffnet, auch wenn der Browser bereits läuft. Auf keinem Mac gelaufen. Safari kennt kein App-Fenster dieser Art; ohne Chromium-Browser bleibt der Text stehen. |
| Windows | `packaging/windows/baruch-start.cmd` | Microsoft Edge (`msedge.exe` unter `%ProgramFiles(x86)%`, `%ProgramFiles%` oder `%LocalAppData%`) | **Nur gelesen** und am Text geprüft; auf keinem Windows gelaufen. Nach „Baruch beenden“ endet `baruch-server.exe`, und das minimierte Konsolenfenster schließt sich dadurch von selbst (Annahme). |

Wirkt `window.close()` nicht, bleibt der Satz „Baruch ist beendet – dieses Fenster kann geschlossen werden“
stehen. Das ist der Normalfall bei **Firefox**: Firefox schließt per Skript nur Fenster, die ein Skript selbst
geöffnet hat, und kennt kein `--app`. Ebenso bei jedem gewöhnlichen Tab und immer, wenn Baruch schon lief
(Dienst) und die Seite von Hand geöffnet wurde.

Stand der Prüfung des Herunterfahrens: unter Linux am echten Prozess geprüft (`tests/round32_beenden.rs`: Knopf,
`SIGTERM`, Strg+C – jeweils Status 0, „Baruch beendet.“, keine `-wal` neben der Datenbank zurückgeblieben). Die Signale von
Windows sind nur kompiliert (`cargo check --target x86_64-pc-windows-gnu` allein für `src/shutdown/signals.rs`),
nie ausgeführt; nach dem Schließen des Fensters lässt Windows dem Prozess nur wenige Sekunden, daher gilt dort eine Frist von 3 statt 10
Sekunden. Unter macOS gilt der Unix-Zweig (ungeprüft, da kein Mac vorhanden).

## Lokal bauen und prüfen {#building-and-checking-locally}

```sh
scripts/build-deb.sh                      # builds release (nice, -j 4) and the .deb under target/debian
scripts/build-deb.sh --no-build           # package only: target/release/baruch-server must exist
packaging/appimage/build-appimage.sh --appdir-only     # AppDir without a network
scripts/apt-repo.sh /pfad/zum/ordner target/debian/*.deb   # unsigned repository as a probe
cargo deny check licenses sources bans    # without a network; `cargo deny check` additionally fetches the RustSec data
nice -n 10 ionice -c3 cargo clippy --workspace --all-targets -j 4 -- -D warnings
scripts/ci-selftest.sh                    # helper scripts and rules of the workflows, without a network
scripts/ci-check-workflows.sh             # only the rules of the workflows
scripts/ci-fingerprint.sh                 # fingerprint of the archive key: exit 3 = not entered yet
scripts/ci-site.sh                        # version comparison and hugo --minify --panicOnWarning -s site
scripts/ci-typst.sh target/debug/baruch-server            # import, export, typesetting with the own program (baruch-typst)
scripts/ci-typst.sh target/debug/baruch-server typst      # counter-check with a foreign Typst from the PATH
scripts/ci-typst.sh target/debug/baruch-server -          # only import and export
```

`scripts/ci-selftest.sh` arbeitet nur mit erfundenen Daten in einem Temp-Ordner: zwei erfundene
.deb-Versionen, ein Wegwerfschlüssel in einem eigenen `GNUPGHOME`, eine Attrappe von `gh`, eine erfundene
Hugo-Site. Fehlt ein Werkzeug, nennt die letzte Ausgabe, was übersprungen wurde.

Ein Paket prüfen, ohne es zu installieren:

```sh
dpkg-deb -I paket.deb && dpkg-deb -c paket.deb
dpkg-deb -x paket.deb /pfad/zur/probe
/pfad/zur/probe/usr/bin/baruch-server --data /pfad/zu/probedaten --secrets memory --listen 127.0.0.1:18765
```

### Die Website lokal bauen {#building-the-website-locally}

Der Quelltext der Website liegt unter `site/` (Hugo, eigenes Theme `site/themes/baruch/`, kein Submodul).
Gebraucht wird Hugo ab 0.158 in der einfachen Ausgabe (nicht „extended“; geprüft mit 0.168.0).

```sh
hugo --minify --panicOnWarning -s site                    # output in site/public/ (not checked in)
hugo server -s site --bind 127.0.0.1 --port 1313          # preview under http://127.0.0.1:1313/
```

`--panicOnWarning` macht jede Warnung zum Fehler; derselbe Befehl läuft in der CI. Die Sprache steht in
`site/hugo.toml` als `locale = "de"` – der ältere Schlüssel `languageCode` ist seit Hugo 0.158 veraltet und
bricht den Bau mit `--panicOnWarning`. `params.version` bestimmt alle Dateinamen und Links der Installationsseite
und wird zusammen mit der Version in `Cargo.toml` aktualisiert (der Test `round24_abnahme` und
`scripts/ci-site.sh` melden eine Abweichung); der Nutzer trägt `params.aptFingerprint` ein (leer = sichtbarer
Platzhalter). Die sechs Bildschirmfotos unter `site/static/bilder/` entstehen mit
`site/tools/bildschirmfotos.py --fotos` aus einem erfundenen Werk; die Site baut auch ohne sie. Ausgeliefert wird
nur über die CI, nie von Hand. Einzelheiten: `site/README.md`.

## Stand der Prüfung {#state-of-verification}

Lokal geprüft (2026-11-24, Debian 13, Version 0.9.0; die Punkte zu Paketen, Website und Testlauf auf dem
zusammengeführten Stand durch den Integrator, im Release-Profil neu gebaut):

- `nice -n 10 ionice -c3 cargo test --workspace -j 4` nach `cargo clean -p` aller 15 Workspace-Crates: 197
  Ergebniszeilen, 3086 bestanden, 0 fehlgeschlagen, 2 ignoriert. Einschließlich `round24_abnahme` (Seite „Über“ unter einem
  erlaubten Rechnernamen; eine Version in Manifest, Programm, Website und Änderungsliste).
- .deb mit `scripts/build-deb.sh` gebaut (`cargo deb --locked`): `baruch_0.9.0-1_amd64.deb`, 75 MB.
  `dpkg-deb -I` zeigt Version `0.9.0-1`, `Homepage: https://baruch.bernau.family/`,
  `Depends: libc6 (>= 2.39), libdbus-1-3 (>= 1.10)`; `dpkg-deb -c` enthält `usr/bin/baruch-server`,
  `usr/bin/baruch` und unter `usr/share/doc/baruch/` `copyright`, `LICENSE` (byteidentisch mit der Datei des
  Repos), `THIRD-PARTY-LICENSES.txt`, `QUELLTEXT.txt`, `README.md`, `CHANGELOG.md`, `README.Debian`,
  `examples/baruch.service`. Das Control-Archiv enthält nur `control` – kein Maintainer-Skript.
- Server aus dem entpackten Paket (`dpkg-deb -x`) mit eigenem Datenordner gestartet
  (`--listen 127.0.0.1:0 --secrets memory`, `HOME` und `XDG_*` auf leere Ordner): `/`, `/ueber`,
  `/einstellungen/daten` HTTP 200, `/ueber/lizenz` als `text/plain`; `baruch-server --version` gibt
  `baruch-server 0.9.0` aus; außerhalb des Datenordners entstand nichts.
- AppImage: AppDir gebaut, `AppRun` gestartet (HTTP 200). Das Image `Baruch-0.9.0-x86_64.AppImage` (116 MB)
  entstand ebenfalls lokal (appimagetool 1.9.1 und type2-runtime 20251108 in `target/appimage/tools/`, beide
  Prüfsummen in Ordnung) und wurde über FUSE gestartet (HTTP 200, `/ueber` nennt den Pfad im eingehängten
  Image). Nicht geprüft: Start ohne FUSE, andere Distributionen.
- Website: `hugo --minify --panicOnWarning -s site` ohne Warnung (10 Seiten, mit den sechs Bildschirmfotos);
  alle 135 internen Links der gebauten HTML-Dateien führen auf vorhandene Dateien und Anker (`/apt/`
  ausgenommen – das entsteht erst beim Release), kein `<script>`, fremde Adressen nur `github.com/Beerlesklopfer/baruch`.
  In Firefox angesehen: Start, Funktionen, Installation, Betrieb, jeweils hell und dunkel, 1400 und 420 Pixel breit.
- `cargo check` für `x86_64-pc-windows-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin` scheitert lokal an
  der ersten C-Abhängigkeit (`zstd-sys`: kein `x86_64-w64-mingw32-gcc` oder kein Apple-Compiler). Grün ohne
  C-Compiler: `-p keyring` (Windows `windows-native`, Apple `apple-native`) und `-p baruch-epub`.

- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: ohne Befund (rustc/cargo 1.99.0). Dafür
  mechanische Korrekturen in 34 Dateien, Verhalten unverändert; die in
  `crates/baruch-rag/tests/port.rs` festgehaltene Prüfsumme von `baruch-rag/src/lib.rs` ist aktualisiert. Auf dem zusammengeführten Stand wiederholt
  (Exit 0), ebenso `cargo fmt --all -- --check`.
- `cargo deny check licenses sources bans`: in Ordnung (auch mit den Keyring-Abhängigkeiten für macOS und
  Windows), mit `wildcards = "deny"` und
  `allow-wildcard-paths = true` (alle Crates tragen `publish = false`).
- `scripts/ci-selftest.sh`: 67 Zusicherungen, alle erfüllt (Folgearbeit R24; davor 51). Darunter: APT-Repository über zwei erfundene
  Versionen (beide in `Packages`, Prüfsummen in `Release`), dasselbe signiert mit einem Wegwerfschlüssel einschließlich einer
  Passphrase-Datei (`gpgv` gegen `baruch-archive-keyring.gpg`, Gegenprobe mit verändertem `InRelease`), Holen
  des letzten Releases gegen eine Attrappe von `gh` (kein Release, Störung, falsche Prüfsumme), Site-Bau einer
  erfundenen Hugo-Site mit `/apt/` (einschließlich der Warnung bei fehlendem Fingerabdruck und bei einer als
  Platzhalter gekennzeichneten Seite), Regeln der Workflows mit acht Gegenproben (neu: Archivschlüssel ohne die Umgebung
  `release`), `scripts/ci-fingerprint.sh` an erfundenen Fingerabdrücken (offen, konsistent, anderer Schlüssel,
  vergessener Platzhalter, abweichende Stelle, falsche Form).
- Workflows: alle drei mit Python (PyYAML) gelesen; jede Action mit einer 40-stelligen SHA, über
  `git ls-remote` gegen die Tags geprüft (Tabelle oben).
- `scripts/ci-typst.sh … -`: Import und Export des erfundenen Werks `scripts/ci-data/leuchtturm.typ` mit dem
  lokal gebauten Binary.
- Die SHA-256-Summe des Hugo-Archivs ist von Hand neu berechnet (`sha256sum` auf der geladenen Datei) und
  stimmt mit `scripts/ci-tool.sh` überein, ebenso der Name im Archiv (`hugo`). (Der Download von Typst, damals ebenso
  geprüft, ist seit Runde 36 entfernt.)
**Nie gelaufen** (nur in der CI prüfbar; der erste Lauf über „Run workflow“ zeigt es):

- alle drei Workflows selbst, auch die Auslieferung über Pages (Umgebung `github-pages` von einem Tag aus,
  eigene Domain, HTTPS);
- `typst compile` des exportierten Werks mit einem **fremden** Typst (Gegenprobe `scripts/ci-typst.sh … typst`):
  nie gelaufen. Mit dem eigenen Programm läuft der Satz seit Runde 36 lokal (siehe unten);
- der Auslöser `release: published` von `pages.yml` einschließlich des Checkouts von `main` („Auslieferung je Commit“);
- `scripts/ci-tool.sh hugo` (Laden von Hugo; lokal ist Hugo 0.168.0 schon vorhanden – die Prüfsumme des
  Archivs ist neu berechnet, siehe oben);
- `scripts/ci-apt-fetch.sh` gegen das echte GitHub (`gh api`, `gh release download`);
- das Signieren mit dem echten Archivschlüssel;
- `cargo deny check advisories` (braucht die RustSec-Datenbank aus dem Netz);
- der Testlauf auf dem Runner: Plattenplatz und Laufzeit für Tests **und** clippy in einem Job (Grenze 240
  Minuten; die Schritte „Platz …“ zeigen den Verbrauch);
- .dmg und .msi (unsigniert; `candle`/`light`, `hdiutil`), der ganze Bau von `baruch-server` für Windows und
  macOS, der Schlüsselbund dort, die Programmsuche und das Abbrechen fremder Programme unter Windows (`taskkill`);
  der Bau des AppImage **in der CI** (lokal gebaut, siehe oben); `lintian`;
- `actionlint` und `shellcheck`: auf dem Rechner nicht vorhanden und sind **nicht** gelaufen. Die Skripte werden mit `sh -n` geprüft
  und – bis auf `ci-tool.sh hugo` – ausgeführt.

Runde 36 (2026-11-27, Typst-Kommandozeile und „Auslieferung je Commit“), lokal geprüft:

- Tests `crates/baruch-render/tests/cli.rs` (Engine: PDF, PNG, SVG, Wurzelordner einschließlich eines Symlinks nach außen,
  Schriftordner mit umbenannter Schrift, Meldungen mit Ort, keine Pakete) und
  `crates/baruch-server/tests/round36_typst.rs` (Kommandozeile im Prozess und über das gebaute Binary als
  Symlink `baruch-typst`, als Symlink `typst` und als `baruch-server typst`; Paketregeln, CI und Doku am
  Text; `AppRun` gegen eine Attrappe von `baruch-server`).
- `scripts/ci-typst.sh target/debug/baruch-server`: Import, Export und Satz des erfundenen Werks mit dem eigenen
  Programm über den Symlink `baruch-typst` (PDF mit 28928 Bytes), dazu die Gegenproben (`watch` → Exit 2,
  Satzfehler → Exit 1 ohne Datei).
- .deb mit `scripts/build-deb.sh --no-build` gepackt (der Debug-Bau diente als Binary, kein Release-Bau):
  `dpkg-deb -c` zeigt `./usr/bin/baruch-typst -> baruch-server` als Symlink und keine Datei `typst`; aus dem
  entpackten Paket meldet `usr/bin/baruch-typst --version` „typst 0.15.1 (Baruch 0.9.0)“.
- AppDir mit `packaging/appimage/build-appimage.sh --appdir-only`: `usr/bin/baruch-typst` ist der Symlink und
  antwortet ebenso. Das AppImage selbst (Runtime, `ARGV0`) wurde nicht gebaut.
- `scripts/ci-selftest.sh` (80 Zusicherungen, darunter der Ablauf von `ci-typst.sh` gegen eine Attrappe, „kein
  Typst-Download“ und der Auslöser `release`), `scripts/ci-check-workflows.sh`, alle drei Workflows mit
  Python `yaml.safe_load` gelesen, `hugo --minify --panicOnWarning -s site` ohne Warnung.

In Runde 36 nicht geprüft: AppImage, .dmg und .msi wurden nicht gebaut (die Regeln sind nur am Text geprüft
und `AppRun` gegen eine Attrappe); `baruch-typst.cmd` ist auf keinem Windows gelaufen; ob `codesign` und
die Notarisierung den Symlink `Contents/MacOS/baruch-typst` akzeptieren, zeigt erst der nächste Release-Lauf; die
Workflows wurden nicht ausgelöst. `actionlint`, `yamllint` und `shellcheck` fehlen weiterhin.

## Werkzeuge anheben {#raising-tools}

- **Actions:** SHA zu einem Tag: `git ls-remote https://github.com/actions/checkout 'refs/tags/v4.2.2*'`
  (bei einem annotierten Tag zählt die Zeile mit `^{}`); SHA und Kommentar in allen Workflows und in der Tabelle
  oben ändern. `scripts/ci-check-workflows.sh` prüft die Form, nicht ob die SHA zum Tag gehört.
- **Rust:** `RUST_VERSION` in `ci.yml` und `release.yml` auf denselben Wert; clippy vorher lokal mit dieser Version
  ausführen (neue Versionen bringen neue Warnungen).
- **Hugo:** Version und SHA-256 stehen oben in `scripts/ci-tool.sh`: Zeile
  `hugo_<Version>_linux-amd64.tar.gz` aus `hugo_<Version>_checksums.txt` des Releases.
- **Typst:** kein Download mehr, also keine Prüfsumme. Wer die Crates `typst*` in `crates/baruch-render/Cargo.toml` anhebt,
  aktualisiert `TYPST_VERSION` in `crates/baruch-render/src/cli.rs` (das meldet `typst --version`). Vergisst man
  es, scheitern `baruch-render/tests/cli.rs`, `round36_typst.rs` und in der CI `scripts/ci-typst.sh`
  (Vergleich mit `Cargo.lock`).
- **appimagetool und Runtime:** Version und SHA-256 stehen oben in `packaging/appimage/build-appimage.sh`
  (Feld `digest` unter `https://api.github.com/repos/AppImage/appimagetool/releases/tags/<Version>`, ebenso
  `AppImage/type2-runtime`).
- **cargo-deb, cargo-deny:** `CARGO_DEB_VERSION` in `release.yml`, `CARGO_DENY_VERSION` in `ci.yml`.

## Offen {#open}

- **Signieren macOS/Windows** (Nutzerfrage): vorbereitet, ausgeschaltet (`MACOS_SIGN`, `WINDOWS_SIGN`). macOS
  braucht eine Mitgliedschaft im Apple Developer Program (Developer-ID-Zertifikat, Notarisierung). Für Windows
  gibt es Code-Signing-Zertifikate inzwischen meist nur noch auf einem Token oder in der Cloud; der Schritt mit
  einer PFX-Datei muss dann ersetzt werden.
- **Fingerabdruck des Archivschlüssels:** Platzhalter, bis der Nutzer ihn einträgt (hier, in `README.md` und in
  `site/hugo.toml`; `scripts/ci-fingerprint.sh` prüft die drei Stellen, `release.yml` den Schlüssel dagegen).
- **Wahl des Archivschlüssels** (ein eigener Schlüssel für Baruch oder der gemeinsame mit ForgeIEC): Entscheidung
  des Nutzers, siehe „Key for the APT repository“.
- **Umgebung `release`:** ob Schutzregeln im gewählten Tarif verfügbar sind und ob der Job mit Freigabe wie
  beschrieben wartet, zeigt erst der erste Lauf (nie gelaufen). Die Secrets für das Signieren unter macOS und
  Windows sind noch an keine Umgebung gebunden (ausgeschaltet; vor dem Einschalten anpassen).
- **Impressum der Website:** Platzhalter, bis der Betreiber es füllt (Einmalige Einrichtung, Schritt 7).
- **Regel für Tags in der Umgebung `github-pages`:** Annahme, siehe Einmalige Einrichtung, Schritt 3.
- **Bauen Windows und macOS überhaupt?** In der CI nie versucht; der erste Lauf zeigt es.
- **Schlüsselbund unter macOS und Windows:** In der Vorarbeit wurde `keyring` nur mit dem Linux-Zugang
  (`sync-secret-service`) gebaut. Runde 24 (Aufgabe pakete) fügt die Features `apple-native` und `windows-native` hinzu;
  das ist erst geprüft, wenn die Pakete in der CI gebaut und auf den Systemen gestartet wurden.
- **Ältere Distributionen:** Gebaut wird auf Ubuntu 24.04 (glibc 2.39), damit `protoc` aus der Paketquelle neu
  genug ist. Für Debian 12 oder Ubuntu 22.04 müsste man auf einem älteren System bauen und `protoc` getrennt beschaffen.
- **arm64:** Das APT-Repository trägt `binary-arm64` (leer); ein arm64-Paket baut noch niemand.
- **Aktualisieren:** nur über APT. AppImage, macOS und Windows melden keine neue Version (gewollt: Baruch
  fragt nie von sich aus das Netz).
- **Übernommene Pakete werden nicht neu geprüft:** `apt-repo.sh --base` übernimmt die .deb-Dateien des vorigen
  Archivs anhand der Prüfsumme in `SHA256SUMS` desselben Releases, prüft aber deren Signatur nicht. Wer ein
  veröffentlichtes Release ändern darf, kann so ein Paket einschleusen – er könnte aber ebenso gut das
  Release selbst ersetzen.
- **Größe:** Das Repository wächst mit jeder Version um ein Paket; GitHub Pages erlaubt etwa 1 GB je Site.
  Alte Versionen vorher aus dem Archiv nehmen (siehe „Delivery through GitHub Pages“).
- **Fremdlizenzen:** 57 Crates (u. a. typst, lance) bringen keine Lizenzdatei mit; für sie steht nur der
  SPDX-Ausdruck in der Liste. Die Texte von MIT und Apache-2.0 stehen durch andere Crates ohnehin in der Datei.
- **Node und `protoc`** sind nicht auf eine genaue Version festgelegt (siehe „Rules of the workflows“).


## Runde 39c – docs_site: Entwicklerdokumentation auf der Website {#round-39c--docs_site-developer-documentation-on-the-website}

Die Entwicklerdokumentation ist seit Runde 39c Teil der Website: `docs/Bedienung.md`, `Architektur.md`, `RPC.md`,
`MCP.md`, `Austauschformat.md`, `WYSIWYG.md`, `Betrieb.md`, `Verteilung.md` und `Plan.md` erscheinen auf Englisch unter
<https://baruch.bernau.family/en/docs/> (`/en/docs/usage/`, `/architecture/`, `/rpc/`, `/mcp/`, `/exchange-format/`,
`/wysiwyg/`, `/operation/`, `/distribution/`, `/plan/`). Das Hauptmenü hat „Documentation“ (englisch) und
„Dokumentation (englisch)“ (deutsch, verweist auf den englischen Teil).

- **Bauschritt:** `site/tools/docs_pages.py` (Python-Standardbibliothek, mit `python3 -I` ausgeführt) erzeugt die Seiten
  in `site/generated/docs/`; `scripts/ci-site.sh` führt es vor Hugo aus, sodass `ci.yml`, `pages.yml` und `release.yml`
  den Abschnitt ohne eigenen Schritt erhalten (Python 3 ist auf den Runnern). `site/hugo.toml` bindet `generated/` nur als
  Inhalt der Sprache `en` ein, neben `content/de` und `content/en`.
- **Entscheidung: nicht eingecheckt.** `site/generated/` steht in `.gitignore`. Die Seiten werden bei jedem Bau der Site aus `docs/`
  neu erzeugt und können nie veralten; eine eingecheckte Kopie würde sich mit jedem Dokumentations-Commit ändern und bräuchte eine
  eigene Aktualitätsprüfung. `ci-site.sh` bricht ab, wenn nach dem Bau `public/en/docs/index.html` oder die Seite einer aufgelisteten Quelle
  fehlt (eine fehlende Einbindung würde sonst stillschweigend ohne den Abschnitt bauen).
- **Links:** zwischen den neun Dokumenten → Site-Adressen mit GitHub-kompatiblen Ankern (der Generator setzt jede
  Überschriften-Id ausdrücklich); zu anderen Dateien des Repositorys → `https://github.com/Beerlesklopfer/baruch/blob/main/<path>`;
  zu Seiten des Website-Quelltexts → die Seite auf der Site. Tote relative Links und Anker stoppen den Generator,
  `scripts/site-check.py` prüft die gebauten Links und Anker wie auf jeder Seite.
- **Pages liefert bei jedem Push auf `main` aus** (wie bisher), eine Änderung der Dokumentation erreicht die Site also mit dem nächsten
  Fast-Forward von `main`.
- **Architektur als eine Seite:** Der Bau der ganzen Site dauert mit Hugo 0.168 etwa 55–60 ms (etwa 30 ms ohne
  den Abschnitt); `Architektur.md` in 48 Teile zu zerlegen, maß dasselbe. Eine Seite hält die Suche des Browsers
  über das ganze Dokument funktionsfähig (die Site hat keine Suche). Einzelheiten: `site/README.md`, „Developer documentation“.
