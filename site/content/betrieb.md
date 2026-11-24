---
title: "Betrieb"
description: "Datenordner und Sicherung, Dauerbetrieb als Benutzerdienst und der Betrieb hinter einem Reverse-Proxy mit TLS."
---

Diese Seite fasst zusammen; verbindlich und ausführlich ist {{< doku "docs/Betrieb.md" "docs/Betrieb.md" >}}.

## Datenordner

| System | Ordner |
|---|---|
| Linux | `~/.local/share/baruch` (bzw. `$XDG_DATA_HOME/baruch`) |
| macOS | `~/Library/Application Support/baruch` |
| Windows | `%LOCALAPPDATA%\baruch` |

Dort liegen `baruch.sqlite` – die Quelle aller Daten – und unter `works/<Name>/` der Git-Spiegel jedes Werks als
gewöhnlicher Typst-Ordner. Ein anderer Ordner: `baruch-server --data ORDNER` oder die Umgebungsvariable
`BARUCH_DATA`. Schlüssel, Token und Passwörter der Verbindungen stehen nicht im Datenordner, sondern im
Schlüsselbund des Systems.

## Sicherung

**Einstellungen → Daten → „Sicherung jetzt“** legt eine Sicherung an. Wer den Datenordner selbst kopiert, hält
Baruch dafür an oder nimmt die Sicherung aus den Einstellungen: SQLite schreibt im laufenden Betrieb in mehrere
Dateien. Vor dem Einspielen einer neuen Version empfiehlt sich immer eine Sicherung; Migrationen laufen beim
ersten Start automatisch. Die Schlüssel im Schlüsselbund gehören nicht zur Sicherung des Datenordners.

## Dauerbetrieb

Das Debian-Paket richtet keinen Dienst ein. Wer Baruch dauerhaft laufen lassen will, nimmt das Beispiel für
einen systemd-Benutzerdienst:

```sh
install -Dm644 /usr/share/doc/baruch/examples/baruch.service ~/.config/systemd/user/baruch.service
systemctl --user daemon-reload
systemctl --user enable --now baruch.service
```

## Nur auf dem eigenen Rechner – die Vorgabe

In der Vorgabe lauscht Baruch auf `127.0.0.1:8765` und antwortet nur unter einer IP-Adresse oder `localhost`.
Ein fremder Rechnername, der per DNS auf den eigenen Rechner zeigt, bekommt die Antwort 421 (Schutz vor
DNS-Rebinding). Schreibende Anfragen nimmt Baruch nur von der eigenen Seite an.

## Hinter einem Reverse-Proxy

Wer Baruch unter einem Rechnernamen und mit TLS erreichen will, stellt einen Reverse-Proxy (nginx, Caddy) auf
denselben Rechner und erlaubt den Namen beim Start:

```sh
baruch-server --allow-host baruch.example.org
```

Mehrere Namen: `--allow-host` mehrfach oder `BARUCH_ALLOWED_HOSTS=name1,name2`. Erlaubt ist genau der genannte
Name, ohne Platzhalter.

**Baruch hat keine Anmeldung.** Wer die Seite erreicht, kann alles lesen und ändern. Darum ist ein
Zugangsschutz im Proxy **Pflicht**: Basic-Auth, Client-Zertifikat oder ein VPN. Ohne ihn gehört Baruch nicht
ins Netz.

Dazu drei Regeln:

- Der Proxy reicht die Kopfzeile `Host` unverändert durch. Sonst lehnt Baruch Formulare und Speichern ab.
- TLS endet im Proxy; Baruch selbst spricht nur HTTP und bleibt auf `127.0.0.1`.
- Unter dem erlaubten Namen nimmt `/mcp` Anfragen nur mit Token an (Einstellungen → MCP-Zugang).

Fertige Beispiele für nginx und Caddy, die Liste der Fallstricke und der Hinweis zu `PGSSLROOTCERT` für
pgvector stehen in {{< doku "docs/Betrieb.md" "docs/Betrieb.md" >}}.
