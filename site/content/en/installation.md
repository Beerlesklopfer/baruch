---
title: "Installation"
description: "Download, install and set up Baruch: packages for Debian and Ubuntu, AppImage, macOS and Windows – and afterwards, step by step, Bibles and assistants."
inhalt: true
---

Baruch is installed like an ordinary program and then used in the browser. Below are the ways for every system;
the data always stays on your own computer. What to set up after the first start – Bibles and assistants – is
described under [After the installation](#nach-der-installation).

## Files to download {#dateien}

The current version is {{< param "version" >}}. All files are on the {{< release "release page" >}} on GitHub.

| System | File | Note |
|---|---|---|
| Debian 13, Ubuntu 24.04 or newer (amd64) | {{< download "baruch_VERSION-1_amd64.deb" >}} | better through the package repository, see below |
| Linux in general (x86_64) | {{< download "Baruch-VERSION-x86_64.AppImage" >}} | needs a recent system (glibc 2.39 or newer) |
| macOS, Apple Silicon | {{< download "Baruch-VERSION-macos-arm64.dmg" >}} | signed and notarised |
| macOS, Intel | {{< download "Baruch-VERSION-macos-x86_64.dmg" >}} | signed and notarised |
| Windows (x64) | {{< download "Baruch-VERSION-x64.msi" >}} | not signed |
| Checksums | {{< download "SHA256SUMS" >}} | SHA-256 of all files |
| Package repository as an archive | {{< download "apt-repo.tar.gz" >}} | for your own copy of the package repository |

**Go to:** [Windows](#windows) · [macOS](#macos) · [Debian and Ubuntu](#debian-ubuntu) ·
[Linux (AppImage)](#appimage) · [From source](#aus-dem-quelltext) ·
[After the installation](#nach-der-installation)

After the start the browser opens with Baruch at `http://127.0.0.1:8765/`; this address can only be reached from
your own computer.

## Debian and Ubuntu through the package repository {#debian-ubuntu}

For Debian 13 and Ubuntu 24.04 or newer (amd64; the package needs `libc6` from 2.39). A package repository (APT
source) is an address from which the package manager of the system fetches programs. Once it is entered, new
versions of Baruch also arrive with the ordinary updates of the system.

```sh
sudo curl -fsSL -o /usr/share/keyrings/baruch-archive-keyring.gpg \
  https://baruch.bernau.family/apt/baruch-archive-keyring.gpg

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

`Signed-By` binds the key to exactly this package repository; it applies to no other. The file
`baruch-archive-keyring.gpg` contains the public archive key with which the package repository is signed.

Check the key before `apt update`:

```sh
gpg --show-keys --with-fingerprint /usr/share/keyrings/baruch-archive-keyring.gpg
```

{{< fingerabdruck >}}

Update: `sudo apt update && sudo apt upgrade`. Remove: `sudo apt remove baruch`, and delete the two files above.
The data folder stays untouched.

## The package (.deb) directly {#deb-direkt}

```sh
sudo apt install ./{{< datei "baruch_VERSION-1_amd64.deb" >}}
```

Start: "Baruch" in the start menu or `baruch` in the terminal – this starts the server if none is running yet and
opens the browser (Chromium, Chrome, Brave or Edge as a window of its own without an address bar, otherwise the
default browser; `BARUCH_BROWSER=standard baruch` always takes the default browser). Quit: menu **Baruch ▾** →
**Quit Baruch…**. The package sets up no service; an example of a systemd user service is in
`/usr/share/doc/baruch/examples` (see [Operation](/en/betrieb/#dauerbetrieb)).

A keyring – the password store of the system (gnome-keyring, KWallet or KeePassXC) – is recommended for the keys
of the connections.

## AppImage {#appimage}

```sh
chmod +x {{< datei "Baruch-VERSION-x86_64.AppImage" >}}
./{{< datei "Baruch-VERSION-x86_64.AppImage" >}}            # start the server and open the browser
./{{< datei "Baruch-VERSION-x86_64.AppImage" >}} --help     # arguments go to baruch-server
```

An AppImage is a single file that runs without installation. It does not bring `libdbus` (for the keyring;
present on every usual desktop) and, like the .deb, needs glibc from 2.39. Updating is done by hand: download the
new file.

## macOS {#macos}

Open the matching `.dmg` file (Apple Silicon or Intel) and drag Baruch into the "Applications" folder. The
program is signed with a Developer ID and notarised by Apple; on the first start macOS only asks whether a
program downloaded from the Internet should be opened.

Baruch has no Dock icon; the interface is the browser. If Chrome, Chromium, Brave or Edge is installed, Baruch
opens a window of its own without an address bar, otherwise the default browser. Closing the browser window does
not quit Baruch – it keeps running in the background. Quitting is done in the interface: menu **Baruch ▾** →
**Quit Baruch…** and confirm the question (see
[Help: Quitting Baruch](/en/hilfe/erste-schritte/#beenden)). Your texts are safe – every saved change is already
in the database.

## Windows (not signed) {#windows}

Run the `.msi` file (administrator rights, installation into `C:\Program Files\Baruch`). The package is **not
signed**; SmartScreen warns ("More info" → "Run anyway"). Start through "Baruch" in the start menu: a minimised
window with the server, and the browser (Microsoft Edge as a window of its own without an address bar, otherwise
the default browser).
Quit: in the interface, menu **Baruch ▾** → **Quit Baruch…**; the minimised window then closes by itself. Closing
the minimised window quits Baruch as well.

The packages for macOS and Windows are only built automatically and are less tried than those for Linux. What is
checked and what is not is described in {{< doku "docs/Verteilung.md" "docs/Verteilung.md" >}}.

## Typst command line {#typst-kommandozeile}

**Typst** is the typesetting program with which Baruch typesets books; its files end in `.typ`. Baruch brings
the Typst command line along – the same program under the name `baruch-typst`, with the built-in typesetting
engine and the embedded fonts. A Typst of your own is not needed for the Typst folder of an export:

```sh
baruch-typst compile buch.typ               # → buch.pdf
baruch-typst compile buch.typ --root . --font-path ~/fonts
baruch-typst fonts
baruch-server typst --help                   # the same without the name baruch-typst
```

Under Debian and Ubuntu `baruch-typst` is in `/usr/bin`, under macOS in `Baruch.app/Contents/MacOS/`, under
Windows as `baruch-typst.cmd` in the installation folder; with the AppImage: `./Baruch-…AppImage typst compile …`.
No package installs a program called `typst` – a Typst of your own stays untouched.

The scope is deliberately small: `compile` (PDF, PNG, SVG), `fonts`, `--version`. An existing output file is
overwritten, as with Typst. Packages from the net (`@preview/…`), `watch` and `query` do not exist; Baruch says
so honestly and exits with an error. Details:
{{< doku "docs/Verteilung.md" "docs/Verteilung.md" >}}, section "Typst command line".

## Optional: compare the checksum {#pruefsumme}

To be sure that the file is unchanged, compare its checksum with `SHA256SUMS` from the release page. Under
Debian and Ubuntu with the APT source this happens automatically.

- **Linux:** download `SHA256SUMS` into the same folder as the package, then
  `sha256sum --check --ignore-missing SHA256SUMS`.
- **macOS:** in the terminal `shasum -a 256 {{< datei "Baruch-VERSION-macos-arm64.dmg" >}}` and compare the
  result with the matching line in `SHA256SUMS`.
- **Windows:** in **PowerShell** (not in the command prompt `cmd.exe`)
  `Get-FileHash {{< datei "Baruch-VERSION-x64.msi" >}} -Algorithm SHA256` and compare the result with the matching
  line in `SHA256SUMS` (upper and lower case do not matter).

## After the installation {#nach-der-installation}

Writing, outline, revisions and book typesetting work right away. **Baruch ships no Bible texts and no AI
models** – on the first start the choice of Bible and the assistants are therefore still empty. That is not an
error. You set up both once, in any order, or leave them out.

### 1. The first start {#erster-start}

Start Baruch (start menu or `baruch` in the terminal). The browser opens `http://127.0.0.1:8765/` and shows a
bundled sample work: on the left the outline, in the middle the text, on the right assistants, revisions and
chat. You can safely try everything in it; {{< taste "Ctrl+S" "⌘S" >}} saves. The tour is described in the help
under [First steps](/en/hilfe/erste-schritte/#starten).

{{< hilfebild name="installation-erster-start" alt="Baruch right after the first start: on the left the outline of the bundled sample work, in the middle a section, on the right the panels for assistants, revisions and chat." titel="The first start shows the bundled sample work." >}}

### 2. Set up Bibles {#bibeln-einrichten}

Baruch reads Bibles, commentaries and lexicons in the format of the free **SWORD project** – a collection of
freely available Bible editions that programs such as BibleTime and Xiphos use as well. Such an edition is called
a **module**. With a chosen Bible, Baruch checks not only whether a reference exists, but also whether a
quotation is worded correctly.

**Where the modules come from.** Free modules are available from CrossWire, the body behind the project
(<https://www.crosswire.org/sword/modules/>). The easiest way is to fetch them with a program that puts them in
the right place right away:

- **BibleTime** or **Xiphos** – Bible programs with built-in module management;
- under Debian and Ubuntu the package `sword-utils` with the command `installmgr`.

Without such a program: download the module as a ZIP file and unpack it into a folder of its own. Which Bible you
may use is governed by its licence; Baruch shows the licence note of every module.

**Where the folder belongs.** A module folder is the folder that contains the two subfolders `mods.d` (the
descriptions) and `modules` (the texts). Baruch searches by itself in this order:

1. in the folders of the environment variable `SWORD_PATH`, if set,
2. in `~/.sword` – the programs named above put their modules there under Linux,
3. in `/usr/share/sword`.

If the modules are there, nothing else needs to be done. **Not checked:** where BibleTime and Xiphos put their
modules under macOS and Windows. There, look up the folder in the program and enter it in Baruch (next step).

**Where to enter the folder in Baruch.** **Baruch ▾ → Settings → Sources**, section **Bibles (SWORD)**:

1. In the field **Module folders** enter one folder per line (full path or `~/…`) and press **Save folders**.
   **Default** restores the list above.
2. The table below shows every module found, with its kind (Bible, commentary, lexicon), licence note and whether
   Baruch can read it – otherwise with the reason. If a folder or its `mods.d` is missing, a note under the form
   says so.
3. Some modules are only published encrypted. You get the key from the publisher; it is entered below the table
   under **Keys of encrypted modules**. It goes only into the keyring of the system.

{{< hilfebild name="bibel-einstellungen" alt="Settings, section Bibles: the field for the module folders, the table of the modules found with their licence notes and the choice Bible of the work." titel="Setting up Bibles: enter module folders, check modules, choose the Bible of the work." >}}

**How to choose the Bible for a work.** On the same page, below the table: choose the translation in the list
**Bible of the work** and press **Choose Bible**. The choice applies to the work that is currently open; every
work has its own. **none** switches off the check of the wording for this work. Commentaries and lexicons are
chosen per work under **Settings → Work → Commentaries and lexicons**.

How Baruch recognises references, checks quotations and looks up Bibles is described in the help under
[Bible, literature and sources](/en/hilfe/bibel-quellen/#bibeln-waehlen).

**Checked and not checked:** reading the modules is checked with test modules made for the purpose. It is not
checked one by one with the Bible editions you actually install; if Baruch cannot read a module, the table names
the reason.

### 3. Set up assistants {#assistenten}

To work with AI assistants you need a model service – a program that provides language models, best on your own
computer. The steps (enter the address, store a key, choose a model, try it out) are under
[Set up assistants](/en/mcp/#assistenten-einrichten). Without a model, Baruch is a complete writing and
typesetting environment.

### 4. Your first own work {#erstes-werk}

**Baruch ▾ → New work / Import…** creates a book or a sermon or imports an existing text. More in the help:
[A new work](/en/hilfe/erste-schritte/#neues-werk) and [Importing existing texts](/en/hilfe/einlesen/).

## Where the data lives {#daten}

Linux `~/.local/share/baruch`, macOS `~/Library/Application Support/baruch`, Windows `%LOCALAPPDATA%\baruch`.
No package creates anything there, and no uninstall deletes anything there. Backup and moving are described in
[Operation](/en/betrieb/#datenordner).

## Updating {#aktualisieren}

Baruch never asks the net for new versions on its own. Which version is running is shown by **Baruch ▾ → About
Baruch**; the link to the release page is there as well.

1. **Create a backup:** Settings → Data → **Back up now** (see
   [Help: Backup](/en/hilfe/austausch-sicherung/#sicherung)). On the first start of a new version Baruch converts
   its database to the new state by itself; going back to the old version is then only possible through the
   backup.
2. **Install the new version:**
   - Debian and Ubuntu with the package repository: `sudo apt update && sudo apt upgrade`.
   - Otherwise: download the new file from the {{< release "release page" >}} and install it as above – replace
     the AppImage, open the `.dmg` and drag Baruch into "Applications" again, run the `.msi`.
3. **Restart Baruch.** If it is still running, quit it first (Baruch ▾ → **Quit Baruch…**); for a service see
   [Operation](/en/betrieb/#dauerbetrieb).

The data folder stays untouched by every update. **Not checked:** updating under macOS and Windows over an
existing installation.

## From source {#aus-dem-quelltext}

You need a current stable Rust toolchain, Node.js with npm (only for the TypeScript compiler of the interface)
and `protoc` in the `PATH`.

```sh
git clone {{< param "repo" >}}.git
cd baruch
(cd crates/baruch-server/web && npm ci)
cargo install --locked --path crates/baruch-server
```

The program ends up in `~/.cargo/bin/baruch-server`.
