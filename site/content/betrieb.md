---
title: "Betrieb"
description: "Datenordner und Sicherung, Dauerbetrieb als Benutzerdienst und der Betrieb hinter einem Reverse-Proxy mit TLS."
---

Diese Seite richtet sich an alle, die Baruch dauerhaft betreiben, sichern oder unter einem eigenen Namen im Netz
erreichbar machen wollen.

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

**Einstellungen → Daten → „Sicherung jetzt“** (Kästchen ankreuzen) legt **neben** dem Datenordner einen Ordner
`baruch.sicherung-JJJJ-MM-TT-HHMMSS` an – unter Linux also etwa
`~/.local/share/baruch.sicherung-2026-11-24-213000`. Darin liegen die Datenbank als stimmiger Stand, die
Spiegel und alles andere. Nichts wird überschrieben; bei zehn Sicherungen lehnt Baruch weitere ab, bis alte
verschoben oder gelöscht sind – selbst löscht Baruch keine.

- **Vor jeder neuen Version** eine Sicherung anlegen: Migrationen der Datenbank laufen beim ersten Start
  automatisch, ein Zurück gibt es danach nur über die Sicherung.
- **Sicherungen woanders hinlegen:** Sie liegen auf derselben Platte wie die Daten; gegen einen Plattenausfall
  hilft nur eine Kopie an anderer Stelle.
- **Den Datenordner selbst kopieren** nur, wenn Baruch nicht läuft – SQLite schreibt im Betrieb in mehrere
  Dateien.
- **Der Schlüsselbund gehört nicht zur Sicherung:** Nach einem Umzug die Schlüssel der Verbindungen neu
  eintragen (Einstellungen → Verbindungen).

### Wiederherstellen

1. Baruch beenden.
2. Den Datenordner umbenennen (etwa `baruch` → `baruch.alt`), nicht löschen.
3. Die Sicherung an seine Stelle kopieren und in `baruch` umbenennen.
4. Baruch starten.

Eine Sicherung lässt sich vorher ansehen, ohne den laufenden Bestand zu berühren – an einer **Kopie**, denn
Baruch schreibt beim Start in den Ordner, den es öffnet:

```sh
cp -a ~/.local/share/baruch.sicherung-2026-11-24-213000 /tmp/baruch-probe
baruch-server --data /tmp/baruch-probe --listen 127.0.0.1:8766
```

Dann `http://127.0.0.1:8766/` öffnen; Schlüssel bleiben dabei nur im Speicher. Das Zurückspielen ist so
beschrieben, aber noch nicht in einem echten Ernstfall erprobt – darum Schritt 2: den alten Ordner aufheben.

## Dauerbetrieb

Das Debian-Paket richtet keinen Dienst ein. Wer Baruch dauerhaft laufen lassen will, nimmt das Beispiel für
einen systemd-Benutzerdienst:

```sh
install -Dm644 /usr/share/doc/baruch/examples/baruch.service ~/.config/systemd/user/baruch.service
systemctl --user daemon-reload
systemctl --user enable --now baruch.service
```

## Schlüsselbund auf einem Server ohne Desktop

Schlüssel, Token und Passwörter der Verbindungen legt Baruch im Schlüsselbund des Systems ab – unter Linux über
den „Secret Service“ (gnome-keyring, KWallet, KeePassXC). Auf einem Server ohne grafische Oberfläche läuft der
oft nicht. Dann gilt:

- Schreiben, Satz und alles, was keine Verbindung braucht, geht weiter. Beim Lesen oder Speichern eines
  Schlüssels meldet Baruch „kein Zugriff auf den Schlüsselbund (gesperrt oder nicht erreichbar)“ – nie mit dem Schlüssel
  selbst im Text.
- **Umgebungsvariablen für Schlüssel gibt es bewusst nicht**; Schlüssel gehören nicht in Dienstdateien oder
  Prozesslisten.
- **Wege:** einen Secret-Service-Dienst auch ohne Desktop betreiben (etwa gnome-keyring oder KeePassXC in der
  Sitzung des Dienst-Benutzers) – oder Baruch mit `--secrets memory` starten. Dann bleiben die Schlüssel nur
  im Speicher und müssen nach jedem Neustart neu eingetragen werden. Was gilt, nennt die Startausgabe
  („Geheimnisse: …“).

## Nur auf dem eigenen Rechner – der Standard

Standardmäßig lauscht Baruch auf `127.0.0.1:8765` und antwortet nur unter einer IP-Adresse oder `localhost`.
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

**Ohne Konten hat Baruch keine Anmeldung.** Wer die Seite erreicht, kann dann alles lesen und ändern. Darum ist
ohne Konten ein Zugangsschutz im Proxy **Pflicht**: Basic-Auth, Client-Zertifikat oder ein VPN. Mit
[Konten](#anmeldung-mit-konten) meldet man sich bei Baruch selbst an; der Schutz im Proxy ist dann freiwillig.

Dazu drei Regeln:

- Der Proxy reicht die Kopfzeile `Host` unverändert durch. Sonst lehnt Baruch Formulare und Speichern ab.
- TLS endet im Proxy und ist Pflicht – auch mit Konten, sonst gingen Passwort und Sitzung im Klartext über die
  Leitung. Baruch selbst spricht nur HTTP und bleibt auf `127.0.0.1`.
- Unter dem erlaubten Namen nimmt `/mcp` Anfragen nur mit Token an (Einstellungen → MCP-Zugang).

### Anmeldung mit Konten

- Das erste Konto (der Autor) entsteht **am eigenen Rechner**: Baruch ▾ → „Konten einrichten…“. Über den Proxy ist
  das Einrichten gesperrt.
- Danach verlangt Baruch für Seiten, Formulare und `/rpc` eine Anmeldung – auch am eigenen Rechner. `/mcp` bleibt
  bei seinen Tokens.
- „Am eigenen Rechner ohne Anmeldung“ (Einstellungen → Konten) nur einschalten, wenn der Proxy `Host` unverändert
  durchreicht. Ein Proxy auf demselben Rechner, der `Host` umschreibt, sähe sonst aus wie der eigene Rechner.
- Direkt von anderen Rechnern bleibt alles außer `/mcp` gesperrt. Lektoren kommen über den Proxy.
- Das Sitzungs-Cookie ist `HttpOnly` und `SameSite=Strict`, über den Namen des Proxys auch `Secure`. Nach einem
  Link von außen (etwa aus einer E-Mail) zeigt Baruch darum einmal die Anmeldeseite; „Schon angemeldet? Weiter“
  führt ohne neue Anmeldung ans Ziel.
- Passwort vergessen: `baruch-server passwort NAME` am Rechner, auf dem Baruch läuft.

Mit Konten antwortet die erste Probe unten nicht mit 401 vom Proxy, sondern mit einer Umleitung (303) auf
`/anmelden`.

### Beispiel nginx

Die Beispiele sind gegen das Verhalten von Baruch geprüft, aber noch nicht mit einem laufenden nginx oder Caddy.

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name baruch.example.org;

    ssl_certificate     /etc/letsencrypt/live/baruch.example.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/baruch.example.org/privkey.pem;

    # Pflicht: Zugangsschutz. Baruch hat keine Anmeldung.
    auth_basic           "Baruch";
    auth_basic_user_file /etc/nginx/baruch.htpasswd;

    client_max_body_size 64m;            # Bilder und Importe

    location / {
        proxy_pass         http://127.0.0.1:8765;
        proxy_http_version 1.1;
        proxy_set_header   Host $http_host;   # unverändert, samt Port
        proxy_buffering    off;
        proxy_read_timeout 600s;              # Modellantworten und Buchsatz dauern
    }

    # /mcp schützt der Token; Basic-Auth nutzt dieselbe Kopfzeile, darum hier ausgenommen.
    location = /mcp {
        auth_basic         off;
        proxy_pass         http://127.0.0.1:8765;
        proxy_http_version 1.1;
        proxy_set_header   Host $http_host;
        proxy_buffering    off;
        proxy_read_timeout 600s;
    }
}

server {
    listen 80;
    server_name baruch.example.org;
    return 301 https://$host$request_uri;
}
```

Wichtig ist `Host $http_host`: Die Vorgabe von nginx schreibt `Host` um, dann lehnt Baruch Speichern und
Formulare ab.

### Beispiel Caddy

Caddy holt das Zertifikat selbst und reicht `Host` unverändert durch.

```caddyfile
baruch.example.org {
    # Pflicht: Zugangsschutz. /mcp schützt der Token.
    @oberflaeche not path /mcp
    basic_auth @oberflaeche {
        autorin HASH-AUS-caddy-hash-password
    }
    reverse_proxy 127.0.0.1:8765
}
```

### Probe nach dem Einrichten (Pflicht)

Von einem **anderen** Rechner, `NAME` = der erlaubte Name:

```sh
# 1. Ohne Anmeldung keine Oberfläche: 401 (vom Proxy)
curl -s -o /dev/null -w '%{http_code}\n' https://NAME/

# 2. /mcp ohne Token: 401 (von Baruch). Kommt hier 200, reicht der Proxy Host nicht durch – sofort abschalten!
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://NAME/mcp \
  -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"ping"}'

# 3. Mit Anmeldung: 200; danach im Browser einen Abschnitt speichern (kein Fehler 403)
curl -s -o /dev/null -w '%{http_code}\n' -u NUTZER https://NAME/
```

Zu 2: Schreibt der Proxy `Host` auf `127.0.0.1` um, hält Baruch jede Anfrage für lokal und nimmt `/mcp` ohne
Token an. Baruch kann das nicht erkennen; die Probe ist darum Pflicht.

## Verbindung zu pgvector mit TLS

Wer die Bedeutungssuche mit einer PostgreSQL-Datenbank (pgvector) betreibt, wählt unter Einstellungen →
Verbindungen den TLS-Modus. Nur **`verify-full`** prüft Zertifikat und Namen des Servers; `require`
verschlüsselt zwar, nimmt aber jedes Zertifikat an. Hat der Server ein Zertifikat einer eigenen
Zertifizierungsstelle, bekommt der Dienst deren Wurzel über die Umgebungsvariable `PGSSLROOTCERT` (Pfad zu
einer PEM-Datei). Das Passwort kommt nur aus dem Schlüsselbund, nie aus `PGPASSWORD` oder `~/.pgpass`.
