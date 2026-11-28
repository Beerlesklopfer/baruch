# Distribution

How packages come from the source, how a release comes about, how the website and the APT repository are delivered
and how end users install Baruch. State 2026-11-24 (Round 24, plan B5). What is verified and what is
not is under [State of verification](#state-of-verification) and [Open](#open).

**In short:** a tag `v<Version>` suffices. GitHub builds the packages, continues the APT repository, creates the
release and delivers the website and the repository through GitHub Pages. There is no manual step "upload" any more.

## Overview

| Package | File in the release | Built by | State |
|---|---|---|---|
| Debian/Ubuntu | `baruch_<Version>-1_amd64.deb` | `scripts/build-deb.sh` (cargo-deb) | built locally, unpacked, started |
| AppImage | `Baruch-<Version>-x86_64.AppImage` | `packaging/appimage/build-appimage.sh` | built locally and started through FUSE; never run in the CI |
| macOS | `Baruch-<Version>-macos-arm64.dmg`, `…-x86_64.dmg` | `packaging/macos/build-dmg.sh` | only in the CI, unsigned |
| Windows | `Baruch-<Version>-x64.msi` | `packaging/windows/build-msi.ps1` (WiX 3) | only in the CI, unsigned |
| APT repository | `apt-repo.tar.gz` | `scripts/apt-repo.sh` | checked locally over two versions, also signed (throwaway key) |
| Checksums | `SHA256SUMS` | `release.yml` | only in the CI |

| Workflow | Trigger | What it does |
|---|---|---|
| `.github/workflows/ci.yml` | push to `main`, by hand | `cargo fmt --check`, tests, clippy (`-D warnings`), `typst compile` of an export with the own program (`baruch-typst`, without a download), `cargo deny check`, site build (without delivery), self-test of the helper scripts |
| `.github/workflows/release.yml` | tag `v*`, by hand (then only packages as artifacts) | packages → APT repository → `SHA256SUMS` → GitHub release → website including `/apt/` through GitHub Pages |
| `.github/workflows/pages.yml` | push to `main`, publishing a release, by hand | rebuild the website, put `/apt/` back from the last published release (job `pages-build`), deliver (job `pages-deploy`) |

The version stands in one place: `[workspace.package] version` in `Cargo.toml` (`scripts/version.sh` reads it).
A tag `v<Version>` must match it, otherwise the build aborts; `params.version` in `site/hugo.toml` must
match it as well, otherwise the site build aborts (`scripts/ci-site.sh`). Only the user sets version
numbers.

Every package contains the licence (AGPL-3.0-or-later, `LICENSE`) and `THIRD-PARTY-LICENSES.txt`: all crates linked
into `baruch-server` with version, licence, source and licence texts. The file arises during the package build
from `cargo metadata` (`scripts/third-party-licenses.py`, only Python 3 and cargo) under `packaging/generated/`
and is not checked in.

Baruch never asks the network for new versions on its own. Updating happens through APT or by hand; the
about page (`/ueber`) links to the release page.

## Making a release

1. **Set the version** (user): `version` in `Cargo.toml` and `params.version` in `site/hugo.toml`; a build
   updates `Cargo.lock`; commit on `development`. Best raise the version only right before the tag:
   from the next push to `main` on the website advertises this version, even if the release does not
   exist yet.
2. **Fast-forward to `main`** (user): triggers `ci.yml` and `pages.yml`. Only continue when `ci.yml` is
   green.
3. **Set and push the tag** (user):

   ```sh
   git tag -a v0.9.0 -m "Baruch 0.9.0"
   git push origin v0.9.0
   ```

4. **`release.yml` runs** – without anything further:
   1. builds the packages (Linux, macOS, Windows) and starts the unpacked .deb as a probe;
   2. fetches `apt-repo.tar.gz` of the last published release and takes over its packages (`pool/`), so that
      older versions stay installable (`scripts/ci-apt-fetch.sh`, `scripts/apt-repo.sh --base`); at the
      first release the repository starts empty;
   3. waits in the job `publish` for the **approval** of the environment `release` (if set up, see
      One-time setup), compares the key from the secret with the entered fingerprint
      (`scripts/ci-fingerprint.sh`, abort on a deviation), regenerates the APT repository and signs it;
   4. writes `SHA256SUMS` and creates the GitHub release **as a draft**;
   5. builds the website (`hugo --minify --panicOnWarning -s site`), puts the repository into
      `site/public/apt/` (job `pages-build`) and delivers everything through GitHub Pages (job `pages-deploy`).

   Linux (.deb) is mandatory; if AppImage, macOS or Windows fail, the release comes about anyway and names in
   the description what is missing.
5. **Look at the draft and publish it** (user, on GitHub under Releases) – and that **before the next
   push to `main`**, see [Limit with drafts](#limit-with-drafts). Otherwise, too: push `main` only
   when the release run is completely through, see
   [Delivery through GitHub Pages](#delivery-through-github-pages). Whoever does not want that sets the
   repository variable `RELEASE_DRAFT` to `false`; then the release is public immediately and the limit
   does not apply.

Started by hand (Actions → Release → "Run workflow"), `release.yml` only builds the packages as artifacts, without a
release and without delivery – recommended as a dress rehearsal before the first tag.

A second run for the same tag (re-run) fails at the step „GitHub-Release“, because the release already
exists: delete the release (or the draft) beforehand.

## Delivery through GitHub Pages

The website (`https://baruch.bernau.family/`) and the APT repository (`https://baruch.bernau.family/apt/`) are one
single Pages site. Pages replaces the whole site at every delivery; that is why **every** delivery must bring the
repository along:

- `release.yml` (job `pages-build`) takes the repository just generated (as an artifact from the job `publish`).
- `pages.yml` fetches `apt-repo.tar.gz` of the youngest **published** release that carries the archive, checks it
  against `SHA256SUMS` of the same release and puts it back into `/apt/`.
- If there is no such release yet, the site appears without `/apt/`; the run reports that as a warning.
- If the fetch fails for another reason (network, permissions, checksum), `pages.yml` aborts and the old
  site stays – better no new site than one without the repository.

Both workflows share the group `pages` (`concurrency`, `cancel-in-progress: false`): two jobs of the group never
run at the same time. The group assures nothing more:

- `pages.yml` holds the group for the whole run (group at the workflow). `release.yml` cannot do that – the group
  `release-<Ref>` already stands at the workflow there –, so `pages-build` and `pages-deploy` carry it each on their own
  and release it in between.
- In a group at GitHub at most **one** job waits; a newly arriving one displaces the waiting one, which
  then counts as cancelled.
- Consequence: if a push to `main` triggers `pages.yml` while the release is still running, its run can slip in between
  `pages-build` and `pages-deploy` of the release, and `pages-deploy` of the release can be displaced.
  Then the delivery of `pages.yml` stands last – with `/apt/` from the last **published** release,
  that is, without the new one.

Hence the rule: **push `main` only when the release run is completely through** (job `pages-deploy` green) and
the draft has been published. If it happened anyway: publish the draft, then start `pages.yml` by hand
(Actions → Webseite → "Run workflow"); a cancelled `pages-deploy` of the release can also be repeated
on its own. **Unmeasured:** this follows from the behaviour of the groups described by GitHub; locally it
cannot be checked and it was not observed.

**Building and delivering are two jobs** (since Round 32), the same in both workflows:

- `pages-build` (only `contents: read`): checkout, Hugo, fetch the repository, build the site,
  `actions/upload-pages-artifact`.
- `pages-deploy` (`needs: [pages-build]`, environment `github-pages`, `pages: write` and `id-token: write`): only
  `actions/deploy-pages`, no checkout, no command.

Reason (2026-11-24, observed at the first release): with both actions in the same job `deploy-pages` twice did not find the
artifact just uploaded, and repeating the job created a second artifact. GitHub
itself intends two jobs for Pages. Besides, the job with write permission and OIDC token thus no longer runs any code of the
repository, and a failed delivery can be repeated on its own without building anew. In
`release.yml`, `pages-deploy` carries the same condition as `pages-build`
(`!cancelled() && needs.pages-build.result == 'success'`), otherwise it would count as skipped when macOS or
Windows fail. **Unchecked:** the separation has never run (nothing triggered, no push); whether it fixes the
observed error only the next run shows. The second observation of the same day – a tag on
the same commit as the last run of `main` is not delivered anew – it does not solve; for that, since
Round 36, there is the section "Delivery per commit" below.

The repository is cumulative: `Packages` names all versions published so far
(`apt install baruch=0.9.0-1` stays possible). The chain runs through the releases – every `apt-repo.tar.gz`
contains the packages of all predecessors. Withdrawing a package completely therefore means: regenerate the archive of
the youngest release without this package and replace it at the release.

### Delivery per commit

Observation of 2026-11-24: the tag of the release lay on the same commit as the last run of `main`, and the
delivery of the release run did not arrive – the site kept showing the state of `pages.yml`. The cause
is not measured (candidates are the group `pages` – the waiting job is displaced, see above – and the error of
`deploy-pages` observed on the same day).

Procedure since Round 36: `pages.yml` additionally has the trigger `release: types: [published]`. As soon as the
user publishes the draft, `pages.yml` runs once more and delivers the site including `/apt/` from the just
published release – a run of its own, independent of the release run and of which commit the
tag lies on.

- **`main` is built, not the tag.** For the event `release` the run points to the commit of the tag; the
  checkout therefore names `ref: main`. Otherwise a release published late would reset the site to the state of
  its tag. For a push and by hand it stays with the commit of the run.
- **Why this way:** it needs no further permission. Starting the workflow from within `release.yml`
  (`gh workflow run`) would require `actions: write` there – one more write permission, against the rule "only where it
  is needed". Besides, the trigger sits at the right place: only with the publishing is
  `apt-repo.tar.gz` fetchable for `pages.yml` (read permissions).
- **Limit `RELEASE_DRAFT=false`:** if the release run publishes itself, that triggers nothing – events of the
  `GITHUB_TOKEN` start no workflows. Then it stays with the jobs `pages-build`/`pages-deploy` of the
  release run and the manual way.
- **Environment `github-pages`:** the run comes from the tag (`refs/tags/v…`); the environment must allow tags `v*`
  (as already for `release.yml`, "One-time setup").
- **Manual way**, when the site does not show the new state after a release (the release must be published):
  `gh workflow run pages.yml --ref main` or Actions → Webseite → "Run workflow". A cancelled
  `pages-deploy` of the release run can also be repeated on its own.
- The group `pages` and the rule "push `main` only after the release run" hold unchanged; the new
  run queues up in the group. `scripts/ci-check-workflows.sh` allows the trigger `release` only as
  `types: [published]`.

**Never checked by execution:** the trigger, the checkout of `main` for the event `release` and the approval of the
environment for this run are only read (YAML, rule check, self-test) – nothing was triggered, no push,
no tag. Whether this fixes the observation of 2026-11-24 only the next release shows.

### Limit with drafts

A token with `contents: read` does not see drafts. As long as the new release is a draft,

- a **push to `main`** (`pages.yml`) delivers the repository of the previous published release –
  `/apt/` falls back to the old state (at the very first release: `/apt/` disappears) until the draft is
  published and `pages.yml` has run again (by hand: Actions → Webseite → "Run workflow");
- a **further release** would take the previous published one as its base; the packages of the draft would then be
  missing from the repository;
- the download links of the website point into the void (the files of a draft are not public).

Hence: publish the draft promptly and before the next push to `main`. After publishing
no further step is needed: `release.yml` has already delivered the site including `/apt/`, and publishing
triggers `pages.yml` once more ("Delivery per commit").

## One-time setup

The user does these steps once; no script and no agent takes them off his hands.

1. **DNS:** set a CNAME record `baruch` (in the zone `bernau.family`) to `beerlesklopfer.github.io`.
   The name additionally stands in `site/static/CNAME` (exactly `baruch.bernau.family`; `scripts/ci-site.sh` checks
   that at every build).
2. **Pages source:** in the repository under Settings → Pages → "Build and deployment" choose the source **"GitHub
   Actions"**, enter the own domain `baruch.bernau.family` and switch on "Enforce HTTPS"
   as soon as GitHub has issued the certificate.
3. **Environment `github-pages`:** Settings → Environments → `github-pages` → "Deployment branches and tags".
   GitHub usually allows only the default branch there; `release.yml`, however, delivers from a tag.
   Add a rule for tags `v*` (beside `main`). *Assumption, not checked:* without this rule GitHub refuses
   the job `pages-deploy` in `release.yml` ("not allowed to deploy to github-pages"); the first run shows it.
4. **Environment `release` with protection rules:** Settings → Environments → "New environment" → `release`. There
   switch on "Required reviewers" (the user himself) and under "Deployment branches and tags" allow only tags
   `v*`. The job `publish` in `release.yml` runs in this environment and then waits at every
   release for the approval before it gets to the archive key. *Assumption, not checked:* protection rules for
   environments exist at GitHub in public repositories in every plan, in private ones only in paid plans.
   Without this step GitHub creates the environment empty at the first run – the workflow then runs, but nothing
   protects the key.
5. **Secrets in this environment** (Settings → Environments → `release` → "Add environment secret"), see
   the next section: `APT_GPG_PRIVATE_KEY` and `APT_GPG_PASSPHRASE`. Do **not** create them as repository secrets:
   a repository secret is seen by every job of every run, a secret of the environment only by the approved
   job `publish`.
6. **Enter the fingerprint** – the same value in three places (40 hex characters, spaces allowed):
   `params.aptFingerprint` in `site/hugo.toml` (when empty the site shows a placeholder hint), in
   `README.md` and in this file replace the placeholder that begins with `FINGERABDRUCK-` (search
   and replace; it stands here twice, in the README once). `scripts/ci-fingerprint.sh` checks whether all three
   places agree; `release.yml` compares the value with the key from the secret before signing
   and **aborts** if it is missing or deviates – so a signed repository without a checkable fingerprint
   does not come about.
7. **Imprint:** `site/content/de/impressum.md` is a visibly marked placeholder (`platzhalter: true` in the
   head, no address). The operator enters the details and removes the line `platzhalter: true`;
   as long as it stands, every site build warns (`scripts/ci-site.sh`), but delivers.

### Key for the APT repository

The repository is signed with an archive key whose secret part lies as a secret of the environment `release`
at GitHub. Its public part is called `baruch-archive-keyring.gpg` and lies after every release under
`https://baruch.bernau.family/apt/baruch-archive-keyring.gpg`. The workflows are bound to no particular key:
they sign with the one in the secret and only demand that its fingerprint is the
published one.

**Which key – decision of the user before the first release** (review R24):

- **Recommended: a key of its own only for Baruch** (only for signing, with an expiry date; the primary key
  stays on the own machine, only the part that may sign goes into the secret). If the repository at
  GitHub or the secret is taken over, only Baruch is affected then, and whoever adds the Baruch package source
  trusts no key that also signs other things.
- **Planned so far: the ForgeIEC archive key** – the same as for `apt.forgeiec.io`. That saves a
  key, but couples both projects: the secret key then additionally lies in this repository;
  a break-in here also hits `apt.forgeiec.io`, and a revocation there shuts Baruch down. `Signed-By`
  limits the key for the users to the respective package source, but not the damage when the
  secret part is lost.

Nobody but the user generates, chooses or deposits the key. If the choice falls on the shared
key, the sentence about it belongs on the installation page (`site/content/de/installation.md`, section
„Debian und Ubuntu über die APT-Quelle“).

Fingerprint: `BEBD FEF8 F980 2EC5 8A90  B818 9A9C 9199 6BB8 8DB4`

(Placeholder. Nobody but the user enters it; it is not fetched from the network and not guessed.
It also stands in `README.md` and on the website – whoever loads the key file from the website best compares
it with the value from the README in the repository, that is, through a second way. Both, however, lie
at GitHub; a way independent of it – such as passing on the fingerprint in person or through a signed
tag – does not exist so far.)

- **`APT_GPG_PRIVATE_KEY`:** the secret key as ASCII (`gpg --armor --export-secret-keys …` on the
  machine where it lies). The part that may sign suffices.
- **`APT_GPG_PASSPHRASE`:** the passphrase for it (omitted only for a key without a passphrase).
- No key lies in the repository, and no script generates one. Both secrets belong into the environment
  `release` (One-time setup, steps 4 and 5); the workflows pass them only through `env` into the one
  step that signs, and `scripts/ci-check-workflows.sh` refuses `APT_GPG_*` in every job without this
  environment; the passphrase stands in a file with `umask 077` in the
  temp folder of the runner (`--passphrase-file`), never in a command line. Keyring and file are deleted at the
  end of the step.
- `release.yml` writes the public part itself from the imported key to
  `baruch-archive-keyring.gpg` (binary, as `Signed-By:` expects it).
- **Before signing**, `scripts/ci-fingerprint.sh --key <Fingerabdruck des importierten
  Schlüssels>` compares the key with the entered fingerprint. If the entry is missing or it is another
  key, the job `publish` ends with an error: no release, no delivery. What is checked is the
  state of the tag – a later entry on `main` does not help the running tag. Then: enter the fingerprint,
  commit, bring it to `main`, delete the tag and set it anew on the new state (there was no release
  yet). The job `linux` therefore already warns at the start of the run when the entry is missing.
- **Without the secret** `release.yml` generates an unsigned repository and warns; `pages-deploy` delivers it anyway
  and warns as well. apt refuses an unsigned repository – it is only good for a probe.
- **Key extended, replaced or revoked:** with a shared key this affects ForgeIEC
  and Baruch together. Renew the secret, update the fingerprint on the website and here, make a new release (or re-sign the repository
  by hand and replace `apt-repo.tar.gz` at the youngest release, then start `pages.yml` by hand).
  With a new key users must fetch the key file anew.

By hand, without CI (e.g. as a probe; unsigned without `APT_GPG_KEY`):

```sh
scripts/apt-repo.sh --base apt-repo.tar.gz /pfad/zum/ordner baruch_0.9.1-1_amd64.deb
APT_GPG_KEY=<Fingerabdruck> scripts/apt-repo.sh /pfad/zum/ordner baruch_0.9.0-1_amd64.deb
```

### Secrets and variables in GitHub

| Name | Kind | What for | Needed |
|---|---|---|---|
| `APT_GPG_PRIVATE_KEY` | secret of the environment `release` | secret archive key (ASCII) | for a signed repository |
| `APT_GPG_PASSPHRASE` | secret of the environment `release` | passphrase for it | for a key with a passphrase |
| `RELEASE_DRAFT` | variable | `false` = release public immediately | no (default: draft) |
| `MACOS_SIGN` | variable | `true` switches on signing and notarising | no (default: off) |
| `MACOS_CERTIFICATE_P12_BASE64`, `MACOS_CERTIFICATE_PASSWORD` | secrets | certificate "Developer ID Application" with private key (.p12, `base64 -w0`), password – the same values as for Onesimus | only with `MACOS_SIGN=true` |
| `MACOS_SIGN_IDENTITY` | secret | name or SHA-1 of the identity | no (default: first "Developer ID Application" of the certificate) |
| `APPSTORE_API_KEY_ID`, `APPSTORE_API_ISSUER_ID`, `APPSTORE_API_KEY_P8_BASE64` | secrets | App Store Connect key for `notarytool` (`AuthKey_<id>.p8`, `base64 -w0`) – as for Onesimus | without: signed, not notarised (warning) |
| `WINDOWS_SIGN` | variable | `true` switches on signing of the .msi | no (default: off) |
| `WINDOWS_CERT_PFX_BASE64`, `WINDOWS_CERT_PASSWORD` | secrets | code signing certificate (.pfx, Base64), password | only with `WINDOWS_SIGN=true` |

Signing for macOS and Windows is prepared and **switched off**; both packages appear unsigned
(see [Open](#open)).

## Rules of the workflows

`scripts/ci-check-workflows.sh` checks them at every CI run (job `skripte`) and locally:

- Every action is pinned to a 40-digit commit SHA, with the version as a comment
  (`uses: actions/checkout@<SHA> # v4.2.2`). A tag can be moved, a SHA cannot.
- No `pull_request_target`. The trigger `release` only as `types: [published]` (only `pages.yml` uses it).
- The top-level permissions are exactly `contents: read`. Only the job `publish` in `release.yml` has `contents: write`;
  only the jobs `pages-deploy` (in `release.yml` and `pages.yml`) have `pages: write` and `id-token: write`,
  both in the environment `github-pages`.
- Building and delivering the website are separate: the job of the environment `github-pages` consists only of
  `actions/deploy-pages` and hangs (`needs`) on a job of the same file that runs `actions/upload-pages-artifact`
  and has no write permissions. The check refuses both in the same job (reason: "Delivery through
  GitHub Pages"; counter-check in `scripts/ci-selftest.sh`).
- The archive key (`secrets.APT_GPG_*`) stands only in a job of the environment `release` (job `publish`).
  Reason: the protection rules of the environment (approval, only tags `v*`) decide which run gets to the key
  – not merely the fact that somebody can set a tag.
- `actions/checkout` runs with `persist-credentials: false`.
- No `${{ … }}` stands in `run:`; values and secrets come into the respective step only through `env`.
  Secrets stand nowhere in the repository and never in a command line.
- Rust is fixed in every workflow in exactly one variable `RUST_VERSION` (the same value as
  `rustc --version` on the development machine, the same everywhere); no `rust-toolchain.toml`, no `stable`.

Fixed tools (state 2026-11-24):

| Tool | Version | Where | Checksum |
|---|---|---|---|
| Rust | 1.99.0 | `RUST_VERSION` in `ci.yml` and `release.yml` | through rustup |
| cargo-deny | 0.22.0 | `CARGO_DENY_VERSION` in `ci.yml` | source build with `--locked` (crates.io) |
| cargo-deb | 3.7.0 | `CARGO_DEB_VERSION` in `release.yml` | source build with `--locked` (crates.io) |
| Hugo | 0.168.0 (not "extended") | `scripts/ci-tool.sh` | SHA-256 from `hugo_0.168.0_checksums.txt` |
| appimagetool, type2-runtime | 1.9.1, 20251108 | `packaging/appimage/build-appimage.sh` | SHA-256 from the field `digest` |
| actions/checkout | v4.2.2 | all workflows | `11bd71901bbe5b1630ceea73d27597364c9af683` |
| actions/setup-node | v4.4.0 | `ci.yml`, `release.yml` | `49933ea5288caeca8642d1e84afbd3f7d6820020` |
| actions/upload-artifact | v4.6.2 | `release.yml` | `ea165f8d65b6e75b540449e92b4886f43607fa02` |
| actions/download-artifact | v4.3.0 | `release.yml` | `d3f86a106a0bac45b974a628896c90dbdf5c8093` |
| actions/upload-pages-artifact | v5.0.0 | `release.yml`, `pages.yml` | `fc324d3547104276b827a68afc52ff2a11cc49c9` |
| actions/deploy-pages | v5.0.1 | `release.yml`, `pages.yml` | `368f82528645a54fb793d4d04e342629a3f51346` |

Typst is no longer in the table since Round 36: the CI loads no Typst command line, it typesets with the
own program (`baruch-typst`, section "Typst command line").

Not fixed: `protoc`, `apt-utils`, `gnupg` and `python3-yaml` come from the package source of the runner (apt,
Homebrew, Chocolatey), `gh` from the image of the runner, Node is the current 22 at the time, WiX that of the
Windows runner.

## Installation for end users

After the start Baruch runs under `http://127.0.0.1:8765/` and is reachable only from the own machine. The
data lie in the folder of the user (`~/.local/share/baruch`, under Windows `%LOCALAPPDATA%\baruch`); no
package creates anything there or deletes anything there on removal.

Every package contains the licence text (`LICENSE`), the licences of the bundled libraries
(`THIRD-PARTY-LICENSES.txt`) and the source code offer (`QUELLTEXT.txt` with the address of the repo and the tag
of the version):

| Package | Place |
|---|---|
| .deb | `/usr/share/doc/baruch/` (plus `copyright`, `README.Debian`, `CHANGELOG.md`) |
| AppImage | `usr/share/doc/baruch/` in the image (`./Baruch-…AppImage --appimage-extract`) |
| macOS | `Baruch.app/Contents/Resources/`; `LICENSE.txt` and `QUELLTEXT.txt` also in the opened image |
| Windows | installation folder (default `C:\Program Files\Baruch`) |

In the running program **„Baruch ▾ → Über Baruch“** shows version, licence, source code and the place of the
third-party licences. Baruch never asks the network for new versions on its own: updating happens by hand with
a new package from the release page, under Debian/Ubuntu with the APT repository through `sudo apt upgrade`. Removing
a package (also `apt purge`, deleting `Baruch.app`, uninstalling under Windows) leaves the data folder
untouched; whoever wants to get rid of the data deletes it themselves.

### Debian and Ubuntu through the APT repository

For Debian 13 and Ubuntu 24.04 or newer (amd64; the package needs `libc6 ≥ 2.39`).

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

`Signed-By` binds the key to exactly this repository; it therefore does not hold for other package sources.
New versions then come with `sudo apt upgrade`; an older version: `sudo apt install baruch=0.9.0-1`.
Removing: `sudo apt remove baruch`, plus delete the two files above.

### .deb directly

```sh
sha256sum --check --ignore-missing SHA256SUMS
sudo apt install ./baruch_0.9.0-1_amd64.deb
```

Starting: „Baruch“ in the start menu or `baruch` in the terminal (starts the server if none is running yet, and
opens the browser); `baruch-server --help` shows import and export. The package sets up no service. Whoever
wants to let Baruch run permanently takes the example for a systemd user service:

```sh
install -Dm644 /usr/share/doc/baruch/examples/baruch.service ~/.config/systemd/user/baruch.service
systemctl --user daemon-reload
systemctl --user enable --now baruch.service
```

A keyring (gnome-keyring, KWallet or KeePassXC as Secret Service) is recommended for the keys of the
connections; `poppler-utils`, `libxml2-utils` and Java are only needed for additional checks.

### AppImage

```sh
chmod +x Baruch-0.9.0-x86_64.AppImage
./Baruch-0.9.0-x86_64.AppImage            # start the server and open the browser
./Baruch-0.9.0-x86_64.AppImage --help     # arguments go to baruch-server
```

The AppImage does not bring `libdbus` along (for the keyring; present on every usual desktop) and,
like the .deb, needs glibc ≥ 2.39. Updating happens by hand: load the new file. The page „Über Baruch“ names the path of the
third-party licences in the mounted image; it only holds as long as Baruch runs.

### macOS (unsigned)

Open `Baruch-<Version>-macos-arm64.dmg` (Apple Silicon) or `…-x86_64.dmg` (Intel) and drag Baruch into the folder
„Programme“. Since 0.9.0 the program is signed with the Developer ID
and notarised by Apple (ticket stapled); macOS only asks at the first start. Without the secrets
(section above) an unsigned package comes about – then: „Systemeinstellungen → Datenschutz & Sicherheit →
Dennoch öffnen“ or `xattr -dr com.apple.quarantine /Applications/Baruch.app`. Compare the checksum with `SHA256SUMS` beforehand.
Baruch has no Dock icon; the user interface is the browser (if possible as an app window of its own, see
"Quitting Baruch"). Quitting: „Baruch ▾ → Baruch beenden…“ in the user interface; if need be Activity Monitor or
`pkill baruch-server` (since Round 32 that also shuts down cleanly).

### Windows (unsigned)

Run `Baruch-<Version>-x64.msi` (administrator rights, installation to `C:\Program Files\Baruch`). The
package is **not signed**; SmartScreen warns („Weitere Informationen“ → „Trotzdem ausführen“). Compare the
checksum beforehand (`Get-FileHash Baruch-0.9.0-x64.msi -Algorithm SHA256`). Start through „Baruch“ in the
start menu: a minimised window with the server, plus the browser (Edge as an app window of its own, otherwise the
default browser). Quitting: „Baruch ▾ → Baruch beenden…“ in the user interface or close the minimised window.

## Typst command line

Since Round 36 Baruch is also the Typst command line – the same program, the same built-in engine (Typst
0.15.1) and the same embedded fonts as in the server, without a second binary:

```sh
baruch-typst compile buch.typ                 # → buch.pdf next to the input
baruch-typst compile buch.typ aus/buch.pdf --root . --font-path ~/schriften
baruch-typst compile buch.typ "seite-{0p}.png" --ppi 300      # one image per page
baruch-typst fonts                            # family names, one per line
baruch-typst --version                        # typst 0.15.1 (Baruch <Version>)
baruch-server typst compile buch.typ          # the same without a symlink
```

The program becomes the Typst command line when it is called under the name `baruch-typst` or `typst`
(symlink; under Windows `.exe` counts as well, upper and lower case do not matter) or when the first argument is `typst`
(`baruch-server typst …`). It then needs no data folder, starts no server and does not touch the network.
It is meant for the Typst folder of an export (`baruch-server export <Werk> --out ORDNER` or the
Git mirror of the work): that can be typeset without a Typst of its own.

| Option | Meaning |
|---|---|
| `compile EINGABE [AUSGABE]` (short `c`) | typesets the file; without AUSGABE the input with the extension of the format arises |
| `--root ORDNER` | root folder of the project (default: folder of the input). Files outside are locked, also through `../` or a symlink |
| `--font-path ORDNER` | further fonts (repeatable; also the environment variable `TYPST_FONT_PATHS`), in addition to the embedded and the system fonts |
| `--format pdf\|png\|svg` (short `-f`) | without it by the extension of the output, otherwise PDF |
| `--ppi ZAHL` | resolution for PNG (default 144) |
| `{p}`, `{0p}`, `{t}` in the name of the output | page number, page number with leading zeros, page count. PNG and SVG with several pages require one of these marks, otherwise exit 1 |
| `fonts [--font-path ORDNER]` | family names of the available fonts |
| `--version`, `--help` | `typst <Typst-Version> (Baruch <Version>)`; help in German |

Exit codes: 0 success; 1 typesetting or file error (messages as `datei:zeile:spalte: error: text` on the
error output, warnings likewise with `warning:`); 2 wrong call or unsupported.

Differences from Typst and from the export of Baruch:

- **`compile` overwrites** an existing output file, like Typst. The export of Baruch
  (`baruch-server export … --pdf DATEI`) never does that. Writing happens only after successful typesetting: on an
  error no file arises, and an existing one stays as it was.
- **No packages, no network:** `#import "@preview/…"` ends with exit 1 and the message „Typst-Pakete werden
  nicht geladen (offline)“ – as in the server.
- **Not supported** (Exit 2, German message „… unterstützt Baruch als Typst-Alias nicht“, nothing is
  read or written): `watch`, `query`, `init`, `update`, `eval`, `completions`, `info`, input from
  standard input (`-`), output to standard output, HTML and every further switch of Typst
  (`--input`, `--pages`, `--pdf-standard`, `--open`, `--jobs` …). A switch is never silently skipped.
- Baruch typesets a work itself (`export --pdf`, typesetting preview) with the images from the database; the
  command line reads only files from the disk.

What the packages bring along – **never a file named `typst`**, a Typst of your own stays untouched:

| Package | Typst command line |
|---|---|
| .deb | `/usr/bin/baruch-typst`, symlink to `baruch-server` (cargo-deb `preserve-symlinks`, source `packaging/linux/baruch-typst`) |
| AppImage | in the image `usr/bin/baruch-typst` (symlink). From outside: `./Baruch-…AppImage typst compile …`, or a symlink of your own `ln -s /pfad/Baruch-…AppImage ~/bin/baruch-typst` – `AppRun` recognises the name from `ARGV0` |
| macOS | `Baruch.app/Contents/MacOS/baruch-typst` (symlink), call e.g. `/Applications/Baruch.app/Contents/MacOS/baruch-typst compile buch.typ` |
| Windows | `baruch-typst.cmd` next to `baruch-server.exe` (calls `baruch-server.exe typst %*`); the installation folder is not in the `PATH` |

Whoever prefers to call the program as `typst` creates the symlink themselves (`ln -s "$(command -v baruch-server)"
~/bin/typst`). The name `baruch-typst` here means the command; the crate of the same name in the source
(`crates/baruch-typst`, work template and outline) is something else.

Source: arguments in `crates/baruch-server/src/typst_cli.rs`, typesetting in `crates/baruch-render/src/cli.rs`;
`TYPST_VERSION` there follows the crate `typst` in `Cargo.lock` (tests `baruch-render/tests/cli.rs` and
`round36_typst.rs` pin that).

## Quitting Baruch

Since Round 32 Baruch quits through the user interface: **„Baruch ▾ → Baruch beenden…“** opens the confirmation
`/beenden`, the button there submits an ordinary form (usable without JavaScript). Afterwards the
window shows „Baruch ist beendet – dieses Fenster kann geschlossen werden“ and tries to close itself.

- **Only at the own machine:** the peer must be loopback, and the request must not come under an
  additionally allowed host name (`--allow-host`, reverse proxy). Otherwise Baruch answers with 403 and
  an explanation. There is neither an RPC method nor an MCP tool for it. **Limit:** a reverse proxy on
  the same machine that rewrites `Host` to `127.0.0.1` or `localhost` (the default with nginx, see
  `docs/Betrieb.md`, "Host names and proxy") looks like the own machine to Baruch – then anybody who
  comes through the proxy can quit Baruch. The proxy must pass `Host` through unchanged (read from the source,
  `shutdown::own_machine`; not reproduced).
- **Clean shutdown** on signals as well: Ctrl+C, under Unix `SIGTERM` (`systemctl --user stop`, `kill`),
  under Windows Ctrl+Break, closing the console window and shutting down the system. Running
  responses finish (at most 10 seconds; open streams such as the live chat are cut off afterwards), the
  database is closed, the last line of the output reads „Baruch beendet.“, the process ends with
  status 0. Baruch deliberately does not catch `SIGHUP` (otherwise `nohup` would no longer work).
- **Service (systemd):** the units (`packaging/systemd/baruch.service`, `packaging/debian/baruch.service`) have
  `Restart=on-failure`. An end with status 0 is not an error: after „Baruch beenden“ the service stays off until
  `systemctl --user start baruch.service` (or the next login) starts it again. Whoever enters `Restart=always`
  makes the button ineffective.

### Window and launcher

A script may close a browser window only when the browser allows it – as a rule for windows that
a script opened, and for app windows. That is why the launchers open Baruch, where possible, as an **app window**
of a Chromium-like browser (`--app=<Adresse>`: a window of its own without an address bar). This is controlled through
`BARUCH_BROWSER`:

| Value | Effect |
|---|---|
| `app` (default) | app window if a fitting browser is found; otherwise the default browser |
| `standard` | always the default browser (`xdg-open`, `open`, `start`) |

`BARUCH_NO_BROWSER=1` still opens no window at all.

| Platform | Launcher | App window with | State |
|---|---|---|---|
| Linux (.deb, AppImage) | `packaging/linux/baruch` (in the AppImage behind `AppRun`) | `chromium`, `chromium-browser`, `google-chrome`, `google-chrome-stable`, `brave-browser`, `microsoft-edge` from the `PATH` (in this order) | **Checked** with dummies in the `PATH`: choice of the program, `--app=<Adresse>`, fallback to `xdg-open`, both switches (`tests/round32_beenden.rs`); `bash -n`. **Not checked:** a real browser (none was started in this round), so also not whether `window.close()` really closes the app window – an assumption based on the known behaviour of Chromium. `shellcheck` was not installed. |
| macOS | the same script as `Contents/MacOS/baruch` | the programs "Google Chrome", "Chromium", "Brave Browser", "Microsoft Edge" under `/Applications` and `~/Applications` (`BARUCH_APPLICATIONS` names other folders) | **Checked** only reproduced under Linux (dummy for `uname`, program folder in the temp directory). **Assumed:** that the direct call of the program in the bundle with `--app=` opens an app window even when the browser is already running. Run on no Mac. Safari knows no app window of this kind; without a Chromium browser the text stays. |
| Windows | `packaging/windows/baruch-start.cmd` | Microsoft Edge (`msedge.exe` under `%ProgramFiles(x86)%`, `%ProgramFiles%` or `%LocalAppData%`) | **Only read** and checked on the text; run on no Windows. After „Baruch beenden“, `baruch-server.exe` ends, and the minimised console window thereby closes by itself (assumption). |

If `window.close()` does not take effect, the sentence „Baruch ist beendet – dieses Fenster kann geschlossen werden“
stays. That is the normal case with **Firefox**: Firefox closes by script only windows that a script itself
opened, and knows no `--app`. Likewise with every ordinary tab and always when Baruch was already running
(service) and the page was opened by hand.

State of verification of the shutdown: checked under Linux on the real process (`tests/round32_beenden.rs`: button,
`SIGTERM`, Ctrl+C – each status 0, „Baruch beendet.“, no `-wal` left next to the database). The signals of
Windows are only compiled (`cargo check --target x86_64-pc-windows-gnu` for `src/shutdown/signals.rs` alone),
never executed; after the window is closed Windows leaves the process only a few seconds, so a deadline of 3 instead of 10
seconds holds there. Under macOS the Unix branch holds (unchecked, as there is no Mac).

## Building and checking locally

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

`scripts/ci-selftest.sh` works only with invented data in a temp folder: two invented
.deb versions, a throwaway key in a `GNUPGHOME` of its own, a mock of `gh`, an invented
Hugo site. If a tool is missing, the last output names what was skipped.

Checking a package without installing it:

```sh
dpkg-deb -I paket.deb && dpkg-deb -c paket.deb
dpkg-deb -x paket.deb /pfad/zur/probe
/pfad/zur/probe/usr/bin/baruch-server --data /pfad/zu/probedaten --secrets memory --listen 127.0.0.1:18765
```

### Building the website locally

The source of the website lies under `site/` (Hugo, own theme `site/themes/baruch/`, no submodule).
Hugo from 0.158 in the plain edition is needed (not "extended"; checked with 0.168.0).

```sh
hugo --minify --panicOnWarning -s site                    # output in site/public/ (not checked in)
hugo server -s site --bind 127.0.0.1 --port 1313          # preview under http://127.0.0.1:1313/
```

`--panicOnWarning` makes every warning an error; the same command runs in the CI. The language stands in
`site/hugo.toml` as `locale = "de"` – the older key `languageCode` is deprecated since Hugo 0.158 and
breaks the build with `--panicOnWarning`. `params.version` determines all file names and links of the installation
page and is updated together with the version in `Cargo.toml` (the test `round24_abnahme` and
`scripts/ci-site.sh` report a deviation); the user enters `params.aptFingerprint` (empty = visible
placeholder). The six screenshots under `site/static/bilder/` arise with
`site/tools/bildschirmfotos.py --fotos` from an invented work; the site builds without them as well. Delivery happens
only through the CI, never by hand. Details: `site/README.md`.

## State of verification

Checked locally (2026-11-24, Debian 13, version 0.9.0; the points about packages, website and test run on the
merged state by the integrator, rebuilt in the release profile):

- `nice -n 10 ionice -c3 cargo test --workspace -j 4` after `cargo clean -p` of all 15 workspace crates: 197
  result lines, 3086 passed, 0 failed, 2 ignored. Including `round24_abnahme` (about page under an
  allowed host name; one version in manifest, program, website and change list).
- .deb built with `scripts/build-deb.sh` (`cargo deb --locked`): `baruch_0.9.0-1_amd64.deb`, 75 MB.
  `dpkg-deb -I` shows version `0.9.0-1`, `Homepage: https://baruch.bernau.family/`,
  `Depends: libc6 (>= 2.39), libdbus-1-3 (>= 1.10)`; `dpkg-deb -c` contains `usr/bin/baruch-server`,
  `usr/bin/baruch` and under `usr/share/doc/baruch/` `copyright`, `LICENSE` (byte-identical with the file of the
  repo), `THIRD-PARTY-LICENSES.txt`, `QUELLTEXT.txt`, `README.md`, `CHANGELOG.md`, `README.Debian`,
  `examples/baruch.service`. The control archive contains only `control` – no maintainer script.
- Server from the unpacked package (`dpkg-deb -x`) started with its own data folder
  (`--listen 127.0.0.1:0 --secrets memory`, `HOME` and `XDG_*` on empty folders): `/`, `/ueber`,
  `/einstellungen/daten` HTTP 200, `/ueber/lizenz` as `text/plain`; `baruch-server --version` prints
  `baruch-server 0.9.0`; nothing arose outside the data folder.
- AppImage: AppDir built, `AppRun` started (HTTP 200). The image `Baruch-0.9.0-x86_64.AppImage` (116 MB)
  also arose locally (appimagetool 1.9.1 and type2-runtime 20251108 in `target/appimage/tools/`, both
  checksums fine) and was started through FUSE (HTTP 200, `/ueber` names the path in the mounted
  image). Not checked: start without FUSE, other distributions.
- Website: `hugo --minify --panicOnWarning -s site` without a warning (10 pages, with the six screenshots);
  all 135 internal links of the built HTML files lead to existing files and anchors (`/apt/`
  excepted – that only arises at the release), no `<script>`, foreign addresses only `github.com/Beerlesklopfer/baruch`.
  Viewed in Firefox: start, features, installation, operation, each light and dark, 1400 and 420 pixels wide.
- `cargo check` for `x86_64-pc-windows-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin` fails locally at
  the first C dependency (`zstd-sys`: no `x86_64-w64-mingw32-gcc` or no Apple compiler). Green without a
  C compiler: `-p keyring` (Windows `windows-native`, Apple `apple-native`) and `-p baruch-epub`.

- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: without a finding (rustc/cargo 1.99.0). For that,
  mechanical corrections in 34 files, behaviour unchanged; the checksum of `baruch-rag/src/lib.rs` pinned in
  `crates/baruch-rag/tests/port.rs` is updated. Repeated on the merged state
  (exit 0), likewise `cargo fmt --all -- --check`.
- `cargo deny check licenses sources bans`: fine (also with the keyring dependencies for macOS and
  Windows), with `wildcards = "deny"` and
  `allow-wildcard-paths = true` (all crates carry `publish = false`).
- `scripts/ci-selftest.sh`: 67 assertions, all met (follow-up R24; before that 51). Among them: APT repository over two invented
  versions (both in `Packages`, checksums in `Release`), the same signed with a throwaway key including a
  passphrase file (`gpgv` against `baruch-archive-keyring.gpg`, counter-check with a changed `InRelease`), fetching
  the last release against a mock of `gh` (no release, disturbance, wrong checksum), site build of an
  invented Hugo site with `/apt/` (including the warning for a missing fingerprint and for a page marked as a
  placeholder), rules of the workflows with eight counter-checks (new: archive key without the environment
  `release`), `scripts/ci-fingerprint.sh` on invented fingerprints (open, consistent, other key,
  forgotten placeholder, deviating place, wrong form).
- Workflows: all three read with Python (PyYAML); every action with a 40-digit SHA, checked through
  `git ls-remote` against the tags (table above).
- `scripts/ci-typst.sh … -`: import and export of the invented work `scripts/ci-data/leuchtturm.typ` with the
  locally built binary.
- The SHA-256 sum of the Hugo archive is recomputed by hand (`sha256sum` on the loaded file) and
  agrees with `scripts/ci-tool.sh`, the name in the archive (`hugo`) as well. (The download of Typst, checked
  likewise at the time, has been removed since Round 36.)
**Never run** (checkable only in the CI; the first run through "Run workflow" shows it):

- all three workflows themselves, also the delivery through Pages (environment `github-pages` from a tag,
  own domain, HTTPS);
- `typst compile` of the exported work with a **foreign** Typst (counter-check `scripts/ci-typst.sh … typst`):
  never run. With the own program the typesetting runs locally since Round 36 (see below);
- the trigger `release: published` of `pages.yml` including the checkout of `main` ("Delivery per commit");
- `scripts/ci-tool.sh hugo` (loading Hugo; locally Hugo 0.168.0 is already present – the checksum of the
  archive is recomputed, see above);
- `scripts/ci-apt-fetch.sh` against the real GitHub (`gh api`, `gh release download`);
- signing with the real archive key;
- `cargo deny check advisories` (needs the RustSec database from the network);
- the test run on the runner: disk space and run time for tests **and** clippy in one job (limit 240
  minutes; the steps „Platz …“ show the consumption);
- .dmg and .msi (unsigned; `candle`/`light`, `hdiutil`), the whole build of `baruch-server` for Windows and
  macOS, the keyring there, the program search and the abort of foreign programs under Windows (`taskkill`);
  the build of the AppImage **in the CI** (built locally, see above); `lintian`;
- `actionlint` and `shellcheck`: missing on the machine and did **not** run. The scripts are checked with `sh -n`
  and – except for `ci-tool.sh hugo` – executed.

Round 36 (2026-11-27, Typst command line and "Delivery per commit"), checked locally:

- Tests `crates/baruch-render/tests/cli.rs` (engine: PDF, PNG, SVG, root folder including a symlink to the outside,
  font folder with a renamed font, messages with location, no packages) and
  `crates/baruch-server/tests/round36_typst.rs` (command line in the process and through the built binary as
  symlink `baruch-typst`, as symlink `typst` and as `baruch-server typst`; package rules, CI and docs on the
  text; `AppRun` against a mock of `baruch-server`).
- `scripts/ci-typst.sh target/debug/baruch-server`: import, export and typesetting of the invented work with the own
  program through the symlink `baruch-typst` (PDF with 28928 bytes), plus the counter-checks (`watch` → exit 2,
  typesetting error → exit 1 without a file).
- .deb packaged with `scripts/build-deb.sh --no-build` (the debug build served as the binary, no release build):
  `dpkg-deb -c` shows `./usr/bin/baruch-typst -> baruch-server` as a symlink and no file `typst`; from the
  unpacked package `usr/bin/baruch-typst --version` reports „typst 0.15.1 (Baruch 0.9.0)“.
- AppDir with `packaging/appimage/build-appimage.sh --appdir-only`: `usr/bin/baruch-typst` is the symlink and
  answers likewise. The AppImage itself (runtime, `ARGV0`) has not been built.
- `scripts/ci-selftest.sh` (80 assertions, among them the flow of `ci-typst.sh` against a mock, "no
  Typst download" and the trigger `release`), `scripts/ci-check-workflows.sh`, all three workflows read with
  Python `yaml.safe_load`, `hugo --minify --panicOnWarning -s site` without a warning.

Not checked in Round 36: AppImage, .dmg and .msi have not been built (the rules are only checked on the text
and `AppRun` against a mock); `baruch-typst.cmd` has run on no Windows; whether `codesign` and
the notarisation accept the symlink `Contents/MacOS/baruch-typst` only the next release run shows; the
workflows have not been triggered. `actionlint`, `yamllint` and `shellcheck` are still missing.

## Raising tools

- **Actions:** SHA for a tag: `git ls-remote https://github.com/actions/checkout 'refs/tags/v4.2.2*'`
  (for an annotated tag the line with `^{}` holds); change SHA and comment in all workflows and in the table
  above. `scripts/ci-check-workflows.sh` checks the form, not whether the SHA belongs to the tag.
- **Rust:** `RUST_VERSION` in `ci.yml` and `release.yml` to the same value; run clippy locally with this version
  beforehand (new versions bring new warnings).
- **Hugo:** version and SHA-256 stand at the top of `scripts/ci-tool.sh`: line
  `hugo_<Version>_linux-amd64.tar.gz` from `hugo_<Version>_checksums.txt` of the release.
- **Typst:** no download any more, so no checksum. Whoever raises the crates `typst*` in `crates/baruch-render/Cargo.toml`
  updates `TYPST_VERSION` in `crates/baruch-render/src/cli.rs` (that is what `typst --version` reports). If one forgets
  it, `baruch-render/tests/cli.rs`, `round36_typst.rs` and in the CI `scripts/ci-typst.sh` fail
  (comparison with `Cargo.lock`).
- **appimagetool and runtime:** version and SHA-256 stand at the top of `packaging/appimage/build-appimage.sh`
  (field `digest` under `https://api.github.com/repos/AppImage/appimagetool/releases/tags/<Version>`, likewise
  `AppImage/type2-runtime`).
- **cargo-deb, cargo-deny:** `CARGO_DEB_VERSION` in `release.yml`, `CARGO_DENY_VERSION` in `ci.yml`.

## Open

- **Signing macOS/Windows** (user question): prepared, switched off (`MACOS_SIGN`, `WINDOWS_SIGN`). macOS
  needs a membership in the Apple Developer Program (Developer ID certificate, notarisation). For Windows
  code signing certificates are by now mostly only available on a token or in the cloud; the step with
  a PFX file then has to be replaced.
- **Fingerprint of the archive key:** placeholder until the user enters it (here, in `README.md` and in
  `site/hugo.toml`; `scripts/ci-fingerprint.sh` checks the three places, `release.yml` the key against it).
- **Choice of the archive key** (a key of its own for Baruch or the shared one with ForgeIEC): decision
  of the user, see "Key for the APT repository".
- **Environment `release`:** whether protection rules are available in the chosen plan and whether the job with approval waits as
  described only the first run shows (never run). The secrets for signing under macOS and
  Windows are not yet bound to any environment (switched off; update before switching on).
- **Imprint of the website:** placeholder until the operator fills it (One-time setup, step 7).
- **Rule for tags in the environment `github-pages`:** assumption, see One-time setup, step 3.
- **Do Windows and macOS build at all?** Never tried in the CI; the first run shows it.
- **Keyring on macOS and Windows:** in the preliminary work `keyring` was only built with the Linux access
  (`sync-secret-service`). Round 24 (task pakete) adds the features `apple-native` and `windows-native`;
  that is only checked when the packages have been built in the CI and started on the systems.
- **Older distributions:** building happens on Ubuntu 24.04 (glibc 2.39), so that `protoc` from the package source is new
  enough. For Debian 12 or Ubuntu 22.04 one would have to build on an older system and obtain `protoc` separately.
- **arm64:** the APT repository carries `binary-arm64` (empty); nobody builds an arm64 package yet.
- **Updating:** only through APT. AppImage, macOS and Windows report no new version (intended: Baruch
  never asks the network on its own).
- **Taken-over packages are not checked anew:** `apt-repo.sh --base` takes over the .deb files of the previous
  archive by the checksum in `SHA256SUMS` of the same release, but does not check its signature. Whoever may change a
  published release can smuggle in a package that way – he could, however, just as well replace the
  release itself.
- **Size:** the repository grows by one package with every version; GitHub Pages allows about 1 GB per site.
  Take old versions out of the archive before that (see "Delivery through GitHub Pages").
- **Third-party licences:** 57 crates (among others typst, lance) bring no licence file; for them only the
  SPDX expression stands in the list. The texts of MIT and Apache-2.0 stand in the file anyway through other crates.
- **Node and `protoc`** are not fixed to an exact version (see "Rules of the workflows").

