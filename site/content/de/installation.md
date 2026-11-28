---
title: "Installation"
description: "Baruch herunterladen, installieren und einrichten: Pakete für Debian und Ubuntu, AppImage, macOS und Windows – und danach Schritt für Schritt Bibeln und Assistenten."
inhalt: true
---

Baruch wird wie ein gewöhnliches Programm installiert und dann im Browser bedient. Unten stehen die Wege für
jedes System; die Daten bleiben dabei immer auf dem eigenen Rechner. Was nach dem ersten Start einzurichten
ist – Bibeln und Assistenten –, steht unter [Nach der Installation](#nach-der-installation).

## Dateien zum Herunterladen {#dateien}

Aktuell ist Version {{< param "version" >}}. Alle Dateien liegen auf der {{< release >}} bei GitHub.

| System | Datei | Hinweis |
|---|---|---|
| Debian 13, Ubuntu 24.04 oder neuer (amd64) | {{< download "baruch_VERSION-1_amd64.deb" >}} | besser über die Paketquelle, siehe unten |
| Linux allgemein (x86_64) | {{< download "Baruch-VERSION-x86_64.AppImage" >}} | braucht ein aktuelles System (glibc 2.39 oder neuer) |
| macOS, Apple Silicon | {{< download "Baruch-VERSION-macos-arm64.dmg" >}} | signiert und notarisiert |
| macOS, Intel | {{< download "Baruch-VERSION-macos-x86_64.dmg" >}} | signiert und notarisiert |
| Windows (x64) | {{< download "Baruch-VERSION-x64.msi" >}} | nicht signiert |
| Prüfsummen | {{< download "SHA256SUMS" >}} | SHA-256 aller Dateien |
| Paketquelle als Archiv | {{< download "apt-repo.tar.gz" >}} | für eine eigene Kopie der Paketquelle |

**Direkt zu:** [Windows](#windows) · [macOS](#macos) · [Debian und Ubuntu](#debian-ubuntu) ·
[Linux (AppImage)](#appimage) · [Aus dem Quelltext](#aus-dem-quelltext) ·
[Nach der Installation](#nach-der-installation)

Nach dem Start öffnet sich der Browser mit Baruch unter `http://127.0.0.1:8765/`; diese Adresse ist nur vom
eigenen Rechner aus erreichbar.

## Debian und Ubuntu über die Paketquelle {#debian-ubuntu}

Für Debian 13 und Ubuntu 24.04 oder neuer (amd64; das Paket braucht `libc6` ab 2.39). Eine Paketquelle
(APT-Quelle) ist eine Adresse, von der die Paketverwaltung des Systems Programme holt. Einmal eingetragen,
kommen auch neue Versionen von Baruch mit den gewöhnlichen Aktualisierungen des Systems.

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

## Das Paket (.deb) direkt {#deb-direkt}

```sh
sudo apt install ./{{< datei "baruch_VERSION-1_amd64.deb" >}}
```

Starten: „Baruch“ im Startmenü oder `baruch` im Terminal – das startet den Server, falls noch keiner läuft, und
öffnet den Browser (Chromium, Chrome, Brave oder Edge als eigenes Fenster ohne Adressleiste, sonst den
Standardbrowser; `BARUCH_BROWSER=standard baruch` nimmt immer den Standardbrowser). Beenden: Menü **Baruch ▾** →
„Baruch beenden…“. Das Paket richtet keinen Dienst ein; ein Beispiel für einen systemd-Benutzerdienst liegt
unter `/usr/share/doc/baruch/examples` (siehe [Betrieb](/betrieb/#dauerbetrieb)).

Empfohlen ist ein Schlüsselbund – der Passwortspeicher des Systems (gnome-keyring, KWallet oder KeePassXC) –
für die Schlüssel der Verbindungen.

## AppImage {#appimage}

```sh
chmod +x {{< datei "Baruch-VERSION-x86_64.AppImage" >}}
./{{< datei "Baruch-VERSION-x86_64.AppImage" >}}            # Server starten und Browser öffnen
./{{< datei "Baruch-VERSION-x86_64.AppImage" >}} --help     # Argumente gehen an baruch-server
```

Ein AppImage ist eine einzelne Datei, die ohne Installation läuft. Es bringt `libdbus` nicht mit (für den
Schlüsselbund; auf jedem üblichen Desktop vorhanden) und braucht wie das .deb glibc ab 2.39. Aktualisiert wird von Hand: neue Datei laden.

## macOS {#macos}

Die passende `.dmg`-Datei öffnen (Apple Silicon oder Intel) und Baruch in den Ordner „Programme“ ziehen. Das
Programm ist mit einer Developer ID signiert und von Apple notarisiert; macOS fragt beim ersten Start nur, ob
ein aus dem Internet geladenes Programm geöffnet werden soll.

Baruch hat kein Dock-Symbol; die Oberfläche ist der Browser. Ist Chrome, Chromium, Brave oder Edge installiert,
öffnet Baruch ein eigenes Fenster ohne Adressleiste, sonst den Standardbrowser. Das Browserfenster zu schließen
beendet Baruch nicht – es läuft im Hintergrund weiter. Beendet wird in der Oberfläche: Menü **Baruch ▾** →
„Baruch beenden…“ und die Rückfrage bestätigen (siehe
[Hilfe: Baruch beenden](/hilfe/erste-schritte/#beenden)). Ihre Texte
sind dabei sicher – jede gespeicherte Änderung steht bereits in der Datenbank.

## Windows (nicht signiert) {#windows}

Die `.msi`-Datei ausführen (Administratorrechte, Installation nach `C:\Program Files\Baruch`). Das Paket ist
**nicht signiert**; SmartScreen warnt („Weitere Informationen“ → „Trotzdem ausführen“). Start über „Baruch“ im Startmenü: ein minimiertes Fenster mit dem Server, dazu der Browser (Microsoft Edge als
eigenes Fenster ohne Adressleiste, sonst der Standardbrowser).
Beenden: in der Oberfläche Menü **Baruch ▾** → „Baruch beenden…“; das minimierte Fenster schließt sich dann von
selbst. Das minimierte Fenster zu schließen beendet Baruch ebenfalls.

Die Pakete für macOS und Windows entstehen nur im automatischen Bau und sind weniger erprobt als die für
Linux. Was geprüft ist und was nicht, steht in {{< doku "docs/Verteilung.md" "docs/Verteilung.md" >}}.

## Typst-Kommandozeile {#typst-kommandozeile}

**Typst** ist das Satzprogramm, mit dem Baruch Bücher setzt; seine Dateien enden auf `.typ`. Baruch bringt die
Typst-Kommandozeile gleich mit – dasselbe Programm unter dem Namen `baruch-typst`, mit dem eingebauten
Satzprogramm und den eingebetteten Schriften. Ein eigenes Typst ist für den Typst-Ordner eines Exports
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
{{< doku "docs/Verteilung.md" "docs/Verteilung.md" >}}, Abschnitt „Typst command line“.

## Optional: Prüfsumme vergleichen {#pruefsumme}

Wer sichergehen will, dass die Datei unverändert ist, vergleicht ihre Prüfsumme mit `SHA256SUMS` von der
Release-Seite. Unter Debian und Ubuntu über die APT-Quelle geschieht das automatisch.

- **Linux:** `SHA256SUMS` in denselben Ordner laden wie das Paket, dann
  `sha256sum --check --ignore-missing SHA256SUMS`.
- **macOS:** im Terminal `shasum -a 256 {{< datei "Baruch-VERSION-macos-arm64.dmg" >}}` und das Ergebnis mit der
  passenden Zeile in `SHA256SUMS` vergleichen.
- **Windows:** in der **PowerShell** (nicht in der Eingabeaufforderung `cmd.exe`)
  `Get-FileHash {{< datei "Baruch-VERSION-x64.msi" >}} -Algorithm SHA256` und das Ergebnis mit der passenden Zeile
  in `SHA256SUMS` vergleichen (Groß- und Kleinschreibung spielt keine Rolle).

## Nach der Installation {#nach-der-installation}

Schreiben, Gliederung, Fassungen und Buchsatz gehen sofort. **Bibeltexte und KI-Modelle bringt Baruch nicht
mit** – beim ersten Start sind darum die Auswahl der Bibel und die Assistenten noch leer. Das ist kein Fehler.
Beides richtet man einmal ein, in beliebiger Reihenfolge, oder lässt es weg.

### 1. Der erste Start {#erster-start}

Baruch starten (Startmenü oder `baruch` im Terminal). Der Browser öffnet `http://127.0.0.1:8765/` und zeigt ein
mitgeliefertes Beispielwerk: links die Gliederung, in der Mitte der Text, rechts Assistenten, Fassungen und
Chat. Darin lässt sich gefahrlos alles ausprobieren; {{< taste "Strg+S" "⌘S" >}} speichert. Den Rundgang
beschreibt die Hilfe unter [Erste Schritte](/hilfe/erste-schritte/#starten).

{{< hilfebild name="installation-erster-start" alt="Baruch direkt nach dem ersten Start: links die Gliederung des mitgelieferten Beispielwerks, in der Mitte ein Abschnitt, rechts die Bereiche für Assistenten, Fassungen und Chat." titel="Der erste Start zeigt das mitgelieferte Beispielwerk." >}}

### 2. Bibeln einrichten {#bibeln-einrichten}

Baruch liest Bibeln, Kommentare und Lexika im Format des freien **SWORD-Projekts** – einer Sammlung frei
verfügbarer Bibelausgaben, die auch Programme wie BibleTime und Xiphos benutzen. Eine solche Ausgabe heißt
**Modul**. Mit einer gewählten Bibel prüft Baruch nicht nur, ob es eine Stelle gibt, sondern auch, ob ein
Zitat im Wortlaut stimmt.

**Woher die Module kommen.** Freie Module gibt es bei CrossWire, dem Träger des Projekts
(<https://www.crosswire.org/sword/modules/>). Am einfachsten holt man sie mit einem Programm, das sie gleich an
den richtigen Ort legt:

- **BibleTime** oder **Xiphos** – Bibelprogramme mit eingebauter Modulverwaltung;
- unter Debian und Ubuntu das Paket `sword-utils` mit dem Befehl `installmgr`.

Ohne ein solches Programm: das Modul als ZIP-Datei laden und in einen eigenen Ordner entpacken. Welche Bibel
Sie verwenden dürfen, regelt ihre Lizenz; Baruch zeigt den Lizenzvermerk jedes Moduls an.

**Wohin der Ordner gehört.** Ein Modulordner ist der Ordner, der die beiden Unterordner `mods.d` (die
Beschreibungen) und `modules` (die Texte) enthält. Baruch sucht von selbst in dieser Reihenfolge:

1. in den Ordnern der Umgebungsvariable `SWORD_PATH`, falls gesetzt,
2. in `~/.sword` – dort legen die genannten Programme unter Linux ihre Module ab,
3. in `/usr/share/sword`.

Liegen die Module dort, ist nichts weiter zu tun. **Nicht geprüft:** wo BibleTime und Xiphos ihre Module unter
macOS und Windows ablegen. Dort den Ordner im Programm nachsehen und in Baruch eintragen (nächster Schritt).

**Wo man den Ordner in Baruch einträgt.** **Baruch ▾ → Einstellungen → Quellen**, Abschnitt **Bibeln (SWORD)**:

1. Im Feld **Modulordner** je Zeile einen Ordner eintragen (vollständiger Pfad oder `~/…`) und **Ordner
   speichern**. „Vorgabe“ stellt die Liste oben wieder her.
2. Die Tabelle darunter zeigt jedes gefundene Modul mit Art (Bibel, Kommentar, Lexikon), Lizenzvermerk und der
   Angabe, ob Baruch es lesen kann – sonst mit dem Grund. Fehlt ein Ordner oder sein `mods.d`, steht das als
   Hinweis unter dem Formular.
3. Manche Module gibt der Herausgeber nur verschlüsselt heraus. Den Schlüssel bekommt man von ihm; eingetragen
   wird er unter der Tabelle bei **Schlüssel verschlüsselter Module**. Er kommt nur in den Schlüsselbund des
   Systems.

{{< hilfebild name="bibel-einstellungen" alt="Einstellungen, Abschnitt Bibeln: das Feld für die Modulordner, die Tabelle der gefundenen Module mit Lizenzvermerk und die Auswahl „Bibel des Werks“." titel="Bibeln einrichten: Modulordner eintragen, Module prüfen, Bibel des Werks wählen." >}}

**Wie man die Bibel für ein Werk wählt.** Auf derselben Seite, unter der Tabelle: in der Auswahl **Bibel des
Werks** die Übersetzung wählen und **Bibel wählen**. Die Wahl gilt für das Werk, das gerade geöffnet ist; jedes
Werk hat seine eigene. „keine“ schaltet die Prüfung des Wortlauts für dieses Werk ab. Kommentare und Lexika
wählt man je Werk unter **Einstellungen → Werk → Kommentare und Lexika**.

Wie Baruch Stellen erkennt, Zitate prüft und Bibeln aufschlägt, beschreibt die Hilfe unter
[Bibel, Literatur und Quellen](/hilfe/bibel-quellen/#bibeln-waehlen).

**Geprüft und nicht geprüft:** Das Lesen der Module ist mit eigens erzeugten Testmodulen geprüft. Mit den
Bibelausgaben, die Sie tatsächlich installieren, ist es nicht einzeln geprüft; kann Baruch ein Modul nicht
lesen, nennt die Tabelle den Grund.

### 3. Assistenten einrichten {#assistenten}

Wer mit KI-Assistenten arbeiten will, braucht einen Modelldienst – ein Programm, das Sprachmodelle
bereitstellt, am besten auf dem eigenen Rechner. Die Schritte (Adresse eintragen, Schlüssel hinterlegen, Modell
wählen, ausprobieren) stehen unter [Assistenten einrichten](/mcp/#assistenten-einrichten). Ohne Modell ist
Baruch eine vollständige Schreib- und Satzumgebung.

### 4. Das erste eigene Werk {#erstes-werk}

**Baruch ▾ → Neues Werk / Importieren…** legt ein Buch oder eine Predigt an oder liest einen vorhandenen Text
ein. Weiter in der Hilfe: [Ein neues Werk](/hilfe/erste-schritte/#neues-werk) und
[Vorhandene Texte einlesen](/hilfe/einlesen/).

## Wo die Daten liegen {#daten}

Linux `~/.local/share/baruch`, macOS `~/Library/Application Support/baruch`, Windows `%LOCALAPPDATA%\baruch`.
Kein Paket legt dort etwas an, und kein Deinstallieren löscht dort etwas. Sicherung und Umzug beschreibt
[Betrieb](/betrieb/#datenordner).

## Aktualisieren {#aktualisieren}

Baruch fragt nie von sich aus im Netz nach neuen Versionen. Welche Version läuft, zeigt **Baruch ▾ → Über
Baruch**; dort steht auch der Link zur Release-Seite.

1. **Sicherung anlegen:** Einstellungen → Daten → „Sicherung jetzt“ (siehe
   [Hilfe: Sicherung](/hilfe/austausch-sicherung/#sicherung)). Beim ersten Start einer neuen Version baut Baruch
   seine Datenbank von selbst auf den neuen Stand um; zurück zur alten Version geht es danach nur über die
   Sicherung.
2. **Neue Version einspielen:**
   - Debian und Ubuntu mit Paketquelle: `sudo apt update && sudo apt upgrade`.
   - Sonst: die neue Datei von der {{< release >}} laden und wie oben installieren – AppImage ersetzen, `.dmg`
     öffnen und Baruch erneut in „Programme“ ziehen, `.msi` ausführen.
3. **Baruch neu starten.** Läuft es noch, vorher beenden (Baruch ▾ → „Baruch beenden…“); als Dienst siehe
   [Betrieb](/betrieb/#dauerbetrieb).

Der Datenordner bleibt bei jeder Aktualisierung unberührt. **Nicht geprüft:** das Aktualisieren unter macOS und
Windows über eine vorhandene Installation.

## Aus dem Quelltext {#aus-dem-quelltext}

Gebraucht werden eine aktuelle stabile Rust-Toolchain, Node.js mit npm (nur für den TypeScript-Compiler der
Oberfläche) und `protoc` im `PATH`.

```sh
git clone {{< param "repo" >}}.git
cd baruch
(cd crates/baruch-server/web && npm ci)
cargo install --locked --path crates/baruch-server
```

Das Programm landet in `~/.cargo/bin/baruch-server`.
