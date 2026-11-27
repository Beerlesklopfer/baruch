---
title: "Installation"
description: "Baruch herunterladen und installieren: APT-Quelle für Debian und Ubuntu, .deb, AppImage, macOS und Windows – oder aus dem Quelltext bauen."
---

Baruch wird wie ein gewöhnliches Programm installiert und dann im Browser bedient. Unten stehen die Wege für
jedes System; die Daten bleiben dabei immer auf dem eigenen Rechner.

## Dateien zum Herunterladen

Aktuell ist Version {{< param "version" >}}. Alle Dateien liegen auf der {{< release >}} bei GitHub.

| System | Datei | Hinweis |
|---|---|---|
| Debian 13, Ubuntu 24.04 oder neuer (amd64) | {{< download "baruch_VERSION-1_amd64.deb" >}} | besser über die APT-Quelle, siehe unten |
| Linux allgemein (x86_64) | {{< download "Baruch-VERSION-x86_64.AppImage" >}} | braucht glibc 2.39 oder neuer |
| macOS, Apple Silicon | {{< download "Baruch-VERSION-macos-arm64.dmg" >}} | signiert und notarisiert |
| macOS, Intel | {{< download "Baruch-VERSION-macos-x86_64.dmg" >}} | signiert und notarisiert |
| Windows (x64) | {{< download "Baruch-VERSION-x64.msi" >}} | nicht signiert |
| Prüfsummen | {{< download "SHA256SUMS" >}} | SHA-256 aller Dateien |
| APT-Repository als Archiv | {{< download "apt-repo.tar.gz" >}} | für einen eigenen Spiegel |

**Direkt zu:** [Windows](#windows-nicht-signiert) · [macOS](#macos-nicht-signiert) ·
[Debian und Ubuntu](#debian-und-ubuntu-über-die-apt-quelle) · [Linux (AppImage)](#appimage) ·
[Aus dem Quelltext](#aus-dem-quelltext)

Nach dem Start öffnet sich der Browser mit Baruch unter `http://127.0.0.1:8765/`; diese Adresse ist nur vom
eigenen Rechner aus erreichbar.

## Debian und Ubuntu über die APT-Quelle

Für Debian 13 und Ubuntu 24.04 oder neuer (amd64; das Paket braucht `libc6` ab 2.39). So kommen auch neue
Versionen über die Paketverwaltung.

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

`Signed-By` bindet den Schlüssel an genau diese Paketquelle; er gilt für keine andere. Die Datei
`baruch-archive-keyring.gpg` enthält den öffentlichen Archivschlüssel, mit dem die Paketquelle signiert ist.

Den Schlüssel vor `apt update` prüfen:

```sh
gpg --show-keys --with-fingerprint /usr/share/keyrings/baruch-archive-keyring.gpg
```

{{< fingerabdruck >}}

Aktualisieren: `sudo apt update && sudo apt upgrade`. Entfernen: `sudo apt remove baruch`, dazu die beiden
Dateien oben löschen. Der Datenordner bleibt dabei unberührt.

## .deb direkt

```sh
sudo apt install ./{{< datei "baruch_VERSION-1_amd64.deb" >}}
```

Starten: „Baruch“ im Startmenü oder `baruch` im Terminal – das startet den Server, falls noch keiner läuft, und
öffnet den Browser (Chromium, Chrome, Brave oder Edge als eigenes Fenster ohne Adressleiste, sonst den
Standardbrowser; `BARUCH_BROWSER=standard baruch` nimmt immer den Standardbrowser). Beenden: Menü **Baruch ▾** →
„Baruch beenden…“. Das Paket richtet keinen Dienst ein; ein Beispiel für einen systemd-Benutzerdienst liegt
unter `/usr/share/doc/baruch/examples` (siehe [Betrieb](/betrieb/)).

Empfohlen ist ein Schlüsselbund (gnome-keyring, KWallet oder KeePassXC als Secret Service) für die Schlüssel
der Verbindungen.

## AppImage

```sh
chmod +x {{< datei "Baruch-VERSION-x86_64.AppImage" >}}
./{{< datei "Baruch-VERSION-x86_64.AppImage" >}}            # Server starten und Browser öffnen
./{{< datei "Baruch-VERSION-x86_64.AppImage" >}} --help     # Argumente gehen an baruch-server
```

Das AppImage bringt `libdbus` nicht mit (für den Schlüsselbund; auf jedem üblichen Desktop vorhanden) und
braucht wie das .deb glibc ab 2.39. Aktualisiert wird von Hand: neue Datei laden.

## macOS

Die passende `.dmg`-Datei öffnen (Apple Silicon oder Intel) und Baruch in den Ordner „Programme“ ziehen. Das
Programm ist mit einer Developer ID signiert und von Apple notarisiert; macOS fragt beim ersten Start nur, ob
ein aus dem Internet geladenes Programm geöffnet werden soll.

Baruch hat kein Dock-Symbol; die Oberfläche ist der Browser. Ist Chrome, Chromium, Brave oder Edge installiert,
öffnet Baruch ein eigenes Fenster ohne Adressleiste, sonst den Standardbrowser. Das Browserfenster zu schließen
beendet Baruch nicht – es läuft im Hintergrund weiter. Beendet wird in der Oberfläche: Menü **Baruch ▾** →
„Baruch beenden…“ und die Rückfrage bestätigen (siehe [Bedienung](/bedienung/)). Ihre Texte
sind dabei sicher – jede gespeicherte Änderung steht bereits in der Datenbank.

## Windows (nicht signiert)

Die `.msi`-Datei ausführen (Administratorrechte, Installation nach `C:\Program Files\Baruch`). Das Paket ist
**nicht signiert**; SmartScreen warnt („Weitere Informationen“ → „Trotzdem ausführen“). Start über „Baruch“ im Startmenü: ein minimiertes Fenster mit dem Server, dazu der Browser (Microsoft Edge als
eigenes Fenster ohne Adressleiste, sonst der Standardbrowser).
Beenden: in der Oberfläche Menü **Baruch ▾** → „Baruch beenden…“; das minimierte Fenster schließt sich dann von
selbst. Das minimierte Fenster zu schließen beendet Baruch ebenfalls.

Die Pakete für macOS und Windows entstehen nur im automatischen Bau und sind weniger erprobt als die für
Linux. Was geprüft ist und was nicht, steht in {{< doku "docs/Verteilung.md" "docs/Verteilung.md" >}}.

## Typst-Kommandozeile

Baruch bringt die Typst-Kommandozeile gleich mit – dasselbe Programm unter dem Namen `baruch-typst`, mit der
eingebauten Engine und den eingebetteten Schriften. Ein eigenes Typst ist für den Typst-Ordner eines Exports
nicht nötig:

```sh
baruch-typst compile buch.typ               # → buch.pdf
baruch-typst compile buch.typ --root . --font-path ~/schriften
baruch-typst fonts
baruch-server typst --help                   # dasselbe ohne den Namen baruch-typst
```

Unter Debian und Ubuntu liegt `baruch-typst` in `/usr/bin`, unter macOS in `Baruch.app/Contents/MacOS/`, unter
Windows als `baruch-typst.cmd` im Installationsordner; beim AppImage: `./Baruch-…AppImage typst compile …`. Ein
Programm namens `typst` installiert kein Paket – ein eigenes Typst bleibt unberührt.

Der Umfang ist bewusst klein: `compile` (PDF, PNG, SVG), `fonts`, `--version`. Eine vorhandene Ausgabedatei wird
überschrieben, wie bei Typst. Pakete aus dem Netz (`@preview/…`), `watch` und `query` gibt es nicht; Baruch
sagt das ehrlich und beendet sich mit einem Fehler. Einzelheiten:
{{< doku "docs/Verteilung.md" "docs/Verteilung.md" >}}, Abschnitt „Typst-Kommandozeile“.

## Optional: Prüfsumme vergleichen

Wer sichergehen will, dass die Datei unverändert ist, vergleicht ihre Prüfsumme mit `SHA256SUMS` von der
Release-Seite. Unter Debian und Ubuntu über die APT-Quelle geschieht das automatisch.

- **Linux:** `SHA256SUMS` in denselben Ordner laden wie das Paket, dann
  `sha256sum --check --ignore-missing SHA256SUMS`.
- **macOS:** im Terminal `shasum -a 256 {{< datei "Baruch-VERSION-macos-arm64.dmg" >}}` und das Ergebnis mit der
  passenden Zeile in `SHA256SUMS` vergleichen.
- **Windows:** in der **PowerShell** (nicht in der Eingabeaufforderung `cmd.exe`)
  `Get-FileHash {{< datei "Baruch-VERSION-x64.msi" >}} -Algorithm SHA256` und das Ergebnis mit der passenden Zeile
  in `SHA256SUMS` vergleichen (Groß- und Kleinschreibung spielt keine Rolle).

## Wo die Daten liegen

Linux `~/.local/share/baruch`, macOS `~/Library/Application Support/baruch`, Windows `%LOCALAPPDATA%\baruch`.
Kein Paket legt dort etwas an, und kein Deinstallieren löscht dort etwas. Sicherung und Umzug beschreibt
[Betrieb](/betrieb/).

## Aktualisieren

Baruch fragt nie von sich aus im Netz nach neuen Versionen. Neue Versionen kommen über die APT-Quelle oder von
Hand: Datei von der Release-Seite laden und wie oben installieren. Vor dem Einspielen einer neuen Version
empfiehlt sich eine Sicherung des Datenordners (Einstellungen → Daten → „Sicherung jetzt“); Migrationen der
Datenbank laufen beim ersten Start automatisch.

## Aus dem Quelltext

Gebraucht werden eine aktuelle stabile Rust-Toolchain, Node.js mit npm (nur für den TypeScript-Compiler der
Oberfläche) und `protoc` im `PATH`.

```sh
git clone {{< param "repo" >}}.git
cd baruch
(cd crates/baruch-server/web && npm ci)
cargo install --locked --path crates/baruch-server
```

Das Programm landet in `~/.cargo/bin/baruch-server`.
