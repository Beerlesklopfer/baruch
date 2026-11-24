---
title: "Installation"
description: "Baruch herunterladen und installieren: APT-Quelle für Debian und Ubuntu, .deb, AppImage, macOS und Windows – oder aus dem Quelltext bauen."
---

Nach dem Start läuft Baruch unter `http://127.0.0.1:8765/` und ist nur vom eigenen Rechner erreichbar. Die Daten
liegen im Ordner des Benutzers (Linux `~/.local/share/baruch`, macOS `~/Library/Application Support/baruch`,
Windows `%LOCALAPPDATA%\baruch`); kein Paket legt dort etwas an, und kein Entfernen löscht dort etwas.

## Dateien zum Herunterladen

Aktuell ist Version {{< param "version" >}}. Alle Dateien liegen auf der {{< release >}} bei GitHub.

| System | Datei | Hinweis |
|---|---|---|
| Debian 13, Ubuntu 24.04 oder neuer (amd64) | {{< download "baruch_VERSION-1_amd64.deb" >}} | besser über die APT-Quelle, siehe unten |
| Linux allgemein (x86_64) | {{< download "Baruch-VERSION-x86_64.AppImage" >}} | braucht glibc 2.39 oder neuer |
| macOS, Apple Silicon | {{< download "Baruch-VERSION-macos-arm64.dmg" >}} | nicht signiert |
| macOS, Intel | {{< download "Baruch-VERSION-macos-x86_64.dmg" >}} | nicht signiert |
| Windows (x64) | {{< download "Baruch-VERSION-x64.msi" >}} | nicht signiert |
| Prüfsummen | {{< download "SHA256SUMS" >}} | SHA-256 aller Dateien |
| APT-Repository als Archiv | {{< download "apt-repo.tar.gz" >}} | für einen eigenen Spiegel |

### Prüfsumme vergleichen

`SHA256SUMS` in denselben Ordner laden wie das Paket, dann:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

Unter Windows: `Get-FileHash {{< datei "Baruch-VERSION-x64.msi" >}} -Algorithm SHA256` und die Zeile mit
`SHA256SUMS` vergleichen.

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
sha256sum --check --ignore-missing SHA256SUMS
sudo apt install ./{{< datei "baruch_VERSION-1_amd64.deb" >}}
```

Starten: „Baruch“ im Startmenü oder `baruch` im Terminal – das startet den Server, falls noch keiner läuft, und
öffnet den Browser. Das Paket richtet keinen Dienst ein; ein Beispiel für einen systemd-Benutzerdienst liegt
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

## macOS (nicht signiert)

Die passende `.dmg`-Datei öffnen und Baruch in den Ordner „Programme“ ziehen. Das Programm ist **nicht
signiert und nicht notarisiert**; macOS verweigert darum den ersten Start. Erlauben: Systemeinstellungen →
Datenschutz & Sicherheit → „Dennoch öffnen“, oder im Terminal:

```sh
xattr -dr com.apple.quarantine /Applications/Baruch.app
```

Vorher die Prüfsumme mit `SHA256SUMS` vergleichen. Baruch hat kein Fenster und kein Dock-Symbol; die Oberfläche
ist der Browser. Beenden: Aktivitätsanzeige oder `pkill baruch-server`.

## Windows (nicht signiert)

Die `.msi`-Datei ausführen (Administratorrechte, Installation nach `C:\Program Files\Baruch`). Das Paket ist
**nicht signiert**; SmartScreen warnt („Weitere Informationen“ → „Trotzdem ausführen“). Vorher die Prüfsumme
vergleichen. Start über „Baruch“ im Startmenü: ein minimiertes Fenster mit dem Server, dazu der Browser.
Beenden: das Fenster schließen.

Die Pakete für macOS und Windows entstehen nur im automatischen Bau und sind weniger erprobt als die für
Linux. Was geprüft ist und was nicht, steht in {{< doku "docs/Verteilung.md" "docs/Verteilung.md" >}}.

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
