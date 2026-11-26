# Betrieb

Wie Baruch dauerhaft läuft: Datenordner und Sicherung, Aktualisieren, der Betrieb hinter einem Reverse-Proxy mit
TLS und unter einem Rechnernamen, Konten und Anmeldung, `/mcp` mit Token, TLS zu pgvector. Stand Runde 35
(2026-11-26; das Proxy-Kapitel stammt aus Runde 24).

Was hier **geprüft** ist und was nur **Annahme**, steht jeweils dabei und gesammelt unter
[Stand der Prüfung](#stand-der-prüfung). Installation und Pakete: `docs/Verteilung.md`. Bedienung:
`docs/Bedienung.md`. Aufbau der Schutzschichten: `docs/Architektur.md`, Abschnitt „MCP“.

## Das Wichtigste zuerst

- **Ohne Konten hat Baruch keine Anmeldung.** Wer die Oberfläche erreicht, liest und ändert alles: Werke,
  Fassungen, Einstellungen, Verbindungen. Die Vorgabe ist darum: nur der eigene Rechner (`127.0.0.1:8765`).
- **Mit Konten (seit Runde 35) verlangt Baruch selbst eine Anmeldung** – für Seiten, Formulare und `/rpc`.
  `/mcp` bleibt bei seinen Tokens. Siehe [Anmeldung und Konten](#anmeldung-und-konten).
- Wer Baruch über das Netz erreichen will, stellt einen Reverse-Proxy davor, der **TLS** spricht. TLS ist
  Pflicht: Passwort und Sitzungs-Cookie gingen sonst im Klartext über die Leitung. **Ohne Konten** muss der Proxy
  außerdem **den Zugang schützen** (Basic-Auth, Client-Zertifikat oder VPN); mit Konten ist dieser zusätzliche
  Schutz kein Zwang mehr, bleibt aber eine sinnvolle zweite Schicht.
- Baruch fragt nie von sich aus im Netz nach neuen Versionen. Aktualisiert wird über APT oder von Hand.

## Datenordner

| System | Vorgabe |
|---|---|
| Linux | `~/.local/share/baruch` (bzw. `$XDG_DATA_HOME/baruch`) |
| macOS | `~/Library/Application Support/baruch` |
| Windows | `%LOCALAPPDATA%\baruch` (Local, nicht Roaming) |

Anderer Ordner: `--data ORDNER` oder `BARUCH_DATA`. Inhalt:

- `baruch.sqlite` (mit `-wal` und `-shm` im laufenden Betrieb): die Quelle aller Daten.
- `works/<slug>/`: je Werk der Git-Spiegel, ein gewöhnlicher Typst-Ordner. Nur lesen – den Stand bestimmt die
  Datenbank, beim Start werden die Spiegel nachgeführt.
- `lancedb/`: der Index der Bedeutungssuche (lässt sich neu aufbauen).

Legt Baruch den Ordner selbst an, bekommt er unter Linux und macOS die Rechte 0700. Einen vorhandenen Ordner
ändert Baruch nicht.

**Geheimnisse** (Schlüssel, Token, Passwörter der Verbindungen) liegen nicht im Datenordner, sondern im
Schlüsselbund des Systems (Dienst `baruch`) – und nur, wenn Baruch mit dem Vorgabe-Datenordner läuft oder mit
`--secrets keyring` (`BARUCH_SECRETS=keyring`) gestartet ist. Sonst bleiben sie im Speicher des Prozesses und
sind nach dem Beenden weg. Die Startausgabe nennt, was gilt („Geheimnisse: …“). Die Tokens für den MCP-Zugang
stehen als SHA-256-Hash in der Datenbank.

Kein Paket legt im Datenordner etwas an, und kein Entfernen löscht dort etwas.

## Sicherung

**Einstellungen → Daten → „Sicherung jetzt“** (Kästchen ankreuzen) legt neben dem Datenordner
`<Name>.sicherung-<JJJJ-MM-TT-HHMMSS>` an: die Datenbank als stimmiger Stand (`VACUUM INTO`), dazu Spiegel und
alles andere. Nichts wird überschrieben. Liegen schon zehn Sicherungen daneben, lehnt Baruch ab, bis alte
entfernt oder verschoben sind – Baruch löscht keine selbst. Einzelheiten: `docs/Bedienung.md`, Einstellungen →
Daten.

- **Vor jeder Aktualisierung** eine Sicherung anlegen. Migrationen der Datenbank laufen beim ersten Start der
  neuen Version automatisch; ein Zurück auf die alte Version gibt es danach nur über die Sicherung.
- **Sicherungen woanders hinlegen.** Sie liegen auf demselben Dateisystem wie die Daten; gegen einen Ausfall der
  Platte hilft nur eine Kopie an anderer Stelle (Backup-Programm, anderer Rechner).
- **Den Datenordner selbst kopieren** nur, wenn Baruch nicht läuft: SQLite schreibt im Betrieb in mehrere
  Dateien, eine Kopie mittendrin kann unstimmig sein.
- **Der Schlüsselbund gehört nicht zur Sicherung.** Nach einem Umzug auf einen anderen Rechner die Schlüssel der
  Verbindungen neu eintragen (Einstellungen → Verbindungen).
- **Zurückspielen** (Annahme, in dieser Runde nicht geprüft): Baruch beenden, den Datenordner beiseitelegen,
  die Sicherung an seine Stelle kopieren, starten. Vorher lässt sich eine Sicherung gefahrlos ansehen:
  `baruch-server --data <Kopie der Sicherung> --listen 127.0.0.1:0` (Geheimnisse bleiben dann im Speicher).

Ein einzelnes Werk trägt die Austauschdatei von einem Rechner zum anderen: `baruch-server export ID --baruch
DATEI.baruch` (alle Fassungen, Einfälle, Beiträge, Bilder; keine Schlüssel).

## Dauerbetrieb und Aktualisieren

Das Debian-Paket richtet keinen Dienst ein. Das Beispiel für einen systemd-Benutzerdienst liegt unter
`/usr/share/doc/baruch/examples/baruch.service` (im Repository `packaging/systemd/`, dort auch die Bedienung
des Dienstes). Baruch läuft als Dienst des angemeldeten Benutzers, nicht als Systemdienst – so erreicht er
Schlüsselbund und Datenordner.

Aktualisieren: Sicherung anlegen, neue Version einspielen (`sudo apt upgrade` bzw. neue Datei von der
Release-Seite), Dienst neu starten. Die Seite **Über Baruch** (`/ueber`) zeigt die laufende Version und
verlinkt die Release-Seite; der Klick dorthin ist die einzige „Nachfrage“ nach neuen Versionen.

## Erreichbarkeit: drei Stufen

| Stufe | Start | Erreichbar | Schutz |
|---|---|---|---|
| 1. Nur eigener Rechner (Vorgabe) | `baruch-server` | `http://127.0.0.1:8765/`, `localhost`, `[::1]` | Rechner und Benutzerkonto |
| 2. Nur `/mcp` aus dem eigenen Netz | `--listen 192.168.…:8765` | von anderen Rechnern nur `/mcp`, unter der IP-Adresse | Token; **kein TLS** – nur im vertrauenswürdigen Netz |
| 3. Hinter einem Reverse-Proxy | `--allow-host NAME` | alles unter `https://NAME/` | TLS im Proxy; Anmeldung in Baruch (Konten) oder Zugangsschutz im Proxy; `/mcp` mit Token |

Stufe 2 beschreibt `docs/Bedienung.md` („MCP-Zugang von anderen Rechnern“). Der Rest dieses Kapitels ist Stufe 3.

### Warum Baruch Rechnernamen sonst ablehnt

Ohne `--allow-host` antwortet Baruch nur unter einer IP-Adresse oder `localhost`; jeder andere Wert der
Kopfzeile `Host` bekommt **421**. Das schützt vor DNS-Rebinding: Eine fremde Webseite kann einen eigenen Namen
auf `127.0.0.1` zeigen lassen und so den Browser dazu bringen, Baruch unter diesem Namen anzusprechen. Eine
IP-Adresse lässt sich nicht umbinden. Ein erlaubter Name hebt diesen Schutz genau für diesen Namen auf – darum
wird er ausdrücklich genannt und nie mit Platzhalter.

## Hinter einem Reverse-Proxy

### 1. Namen erlauben

```sh
baruch-server --allow-host baruch.example.org
baruch-server --allow-host baruch.example.org --allow-host schreibstube.example.net
BARUCH_ALLOWED_HOSTS=baruch.example.org,schreibstube.example.net baruch-server
```

`--allow-host` gilt nur für `serve` und darf mehrfach stehen; `BARUCH_ALLOWED_HOSTS` trennt mit Komma oder
Leerraum. Beides zusammen ergibt die Vereinigung. Im systemd-Dienst:
`Environment=BARUCH_ALLOWED_HOSTS=baruch.example.org`.

Ein gültiger Name:

- 1 bis 253 Zeichen, Teile zwischen den Punkten 1 bis 63 Zeichen aus `a–z`, `0–9` und `-` (nicht am Rand);
  Großbuchstaben werden klein gelesen. Ein Name ohne Punkt (`baruch` im Intranet) gilt.
- **ohne Port** – der Port in `Host` ist beliebig (`baruch.example.org` und `baruch.example.org:8443` passieren
  beide);
- **kein Platzhalter** (`*.example.org`), jeder Name einzeln;
- **keine IP-Adresse** und nicht `localhost` – die gelten ohnehin. Auch Namen, deren letzter Teil eine Zahl ist
  (`127.1`, `0x7f.1`), sind abgelehnt, weil Browser sie als IP-Adresse lesen;
- Umlaute nur in der ASCII-Form (`xn--…`).

Ein ungültiger Name beendet den Start mit Exit 2 und nennt ihn („Ungültiger Rechnername „…“ (--allow-host): …“).
Die Startausgabe nennt die erlaubten Namen. Erlaubt ist genau der Name: `x.baruch.example.org` oder
`baruch.example.org.evil.example` bleiben 421.

### 2. Baruch bleibt auf Loopback, der Proxy auf demselben Rechner

Baruch lauscht weiter auf `127.0.0.1:8765` (Vorgabe von `--listen`); nur der Proxy öffnet einen Port nach
außen. So führt kein Weg am Proxy und seinem Zugangsschutz vorbei. Der Proxy muss **auf demselben Rechner**
laufen: Von einer Gegenstelle außerhalb von Loopback lässt Baruch nur `/mcp` durch (403 für alles andere),
gleich unter welchem Namen.

### 3. Regeln für den Proxy

1. **Zugangsschutz.** Ohne Konten Pflicht: Basic-Auth, Client-Zertifikat oder VPN vor allem außer `/mcp` –
   Baruch selbst prüft dann niemanden. Mit Konten prüft Baruch selbst (Anmeldeseite, Sitzungs-Cookie); der
   Schutz im Proxy ist dann freiwillig. Wer beides nutzt, meldet sich zweimal an.
2. **`Host` unverändert durchreichen.** Baruch vergleicht bei jeder schreibenden Anfrage `Origin` mit `Host`
   (`guard::check`). Schreibt der Proxy `Host` um (bei nginx die Vorgabe!), antworten Speichern und alle
   Formulare mit 403 „Origin passt nicht zu Host“. Schlimmer: Baruch hielte dann jede Anfrage für lokal –
   siehe die Probe unten.
3. **TLS endet im Proxy.** Baruch spricht nur HTTP. `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host`
   und `Forwarded` **wertet Baruch nicht aus** – weder für die Gegenstelle noch für das Schema. Einzige
   Ausnahme (Runde 35): Ist „Am eigenen Rechner ohne Anmeldung“ eingeschaltet, gilt eine Anfrage mit einer dieser
   Kopfzeilen (dazu `X-Real-IP`, `Via`) **nicht** als eigener Rechner. Sie zu setzen ist darum empfohlen.
4. **HTTP/1.1 zum Dienst** (Vorgabe bei nginx und Caddy). Eine Anfrage ohne `Host`-Kopfzeile gilt, sobald Namen
   erlaubt sind, nicht als lokal.
5. **Keine Zwischenspeicherung, keine Umschreibung** von Antworten; lange Antworten nicht puffern
   (Annahme: Live-Antworten im Chat kommen sonst verzögert an; mit einem echten Proxy nicht geprüft).

### Beispiel nginx

Nicht gegen einen laufenden nginx geprüft (siehe [Stand der Prüfung](#stand-der-prüfung)); geprüft ist, wie
Baruch auf die Kopfzeilen antwortet, die diese Einstellung schickt.

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name baruch.example.org;

    ssl_certificate     /etc/letsencrypt/live/baruch.example.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/baruch.example.org/privkey.pem;

    # Zugangsschutz: Pflicht, solange in Baruch keine Konten eingerichtet sind (ohne Konten hat Baruch keine
    # Anmeldung). Mit Konten kann er entfallen; TLS bleibt Pflicht.
    auth_basic           "Baruch";
    auth_basic_user_file /etc/nginx/baruch.htpasswd;

    client_max_body_size 64m;            # Bilder und Importe; an die eigenen Dateien anpassen (Annahme)

    location / {
        proxy_pass         http://127.0.0.1:8765;
        proxy_http_version 1.1;
        proxy_set_header   Host $http_host;   # unverändert, samt Port – nicht $proxy_host (Vorgabe)
        proxy_buffering    off;
        proxy_read_timeout 600s;              # Modellantworten und Buchsatz dauern
    }

    # /mcp schützt der Token (Authorization: Bearer …). Basic-Auth nutzt dieselbe Kopfzeile, beides zugleich
    # geht nicht – darum hier ausgenommen.
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

`$http_host` ist die Kopfzeile so, wie der Browser sie schickt (mit Port, falls nicht 443). `$host` lässt den
Port weg; auf einem anderen Port als 443 passte `Origin` dann nicht mehr zu `Host`.

### Beispiel Caddy

Ebenfalls nicht gegen einen laufenden Caddy geprüft. Caddy holt das Zertifikat selbst und reicht `Host`
in der Vorgabe unverändert durch (Annahme nach der Dokumentation von Caddy 2).

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

(`basic_auth` heißt in älteren Fassungen von Caddy 2 `basicauth`.)

### Probe nach dem Einrichten (Pflicht)

Von einem **anderen** Rechner, `NAME` = der erlaubte Name:

```sh
# 1. Ohne Anmeldung keine Oberfläche: 401 (vom Proxy)
curl -s -o /dev/null -w '%{http_code}\n' https://NAME/

# 2. /mcp ohne Token: 401 (von Baruch). Kommt hier 200, reicht der Proxy Host nicht durch – sofort abschalten!
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://NAME/mcp \
  -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"ping"}'

# 3. Mit Anmeldung: 200; danach im Browser einen Abschnitt speichern (kein 403)
curl -s -o /dev/null -w '%{http_code}\n' -u NUTZER https://NAME/
```

Zu 2: Schreibt der Proxy `Host` auf `127.0.0.1:8765` um, sieht Baruch eine Anfrage von Loopback unter einer
IP-Adresse – genau wie von einem Programm auf dem eigenen Rechner – und nimmt `/mcp` **ohne Token** an. Ist
`/mcp` dann noch vom Zugangsschutz ausgenommen, stünde der MCP-Zugang offen im Netz. Baruch kann diesen Fall
nicht erkennen (es wertet `X-Forwarded-*` bewusst nicht aus); die Probe ist darum Pflicht.

Auf dem Rechner selbst, ohne Proxy, zeigt dieselbe Prüfung das Verhalten von Baruch:

```sh
curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: baruch.example.org' http://127.0.0.1:8765/     # 200
curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: fremd.example.org'  http://127.0.0.1:8765/     # 421
```

## Anmeldung und Konten

Seit Runde 35 kennt Baruch Konten: einen **Autor** und beliebig viele **Lektoren**. Bedienung (Einrichten, Lektor
anlegen, Werk freigeben, was ein Lektor darf): `docs/Bedienung.md`, Abschnitt „Konten und Rollen“. Hier steht,
was das für den Betrieb heißt.

- **Ohne Konten ändert sich nichts.** Keine Anmeldung, keine Sitzungs-Cookies, keine Umleitung – Baruch verhält
  sich wie vor Runde 35. Dann gilt alles oben: nur der eigene Rechner, oder ein Proxy mit Zugangsschutz.
- **Das erste Konto** legt der Autor **am eigenen Rechner** an (`http://127.0.0.1:8765/einrichten`), nie über den
  Proxy: Über den Namen des Proxys oder mit einer Proxy-Kopfzeile antwortet `/einrichten` mit 403. Sonst könnte
  der Erste, der die Adresse erreicht, die Schreibwerkstatt übernehmen.
- **Mit Konten gilt die Anmeldung auch am eigenen Rechner** (Vorgabe, Entscheidung 0.2 der Runde 35). Grund: Ein
  Proxy auf demselben Rechner verbindet sich ebenfalls über Loopback. Schreibt er `Host` um, sähe er aus wie der
  eigene Rechner – eine Ausnahme für Loopback wäre dann eine vollständige Umgehung der Anmeldung. Der Autor kann
  unter Einstellungen → Konten „Am eigenen Rechner ohne Anmeldung“ einschalten. Die Ausnahme gilt nur für eine
  Gegenstelle auf Loopback, unter IP-Adresse oder `localhost` (nie unter einem Namen aus `--allow-host`) und nur
  ohne die Kopfzeilen `Forwarded`, `X-Forwarded-For`, `X-Forwarded-Host`, `X-Forwarded-Proto`, `X-Real-IP`,
  `Via`. **Nur einschalten, wenn der Proxy `Host` unverändert durchreicht** (Probe unten) – am besten setzt er
  zusätzlich `X-Forwarded-For`.
- **Direkter Zugriff von anderen Rechnern bleibt gesperrt** (Entscheidung 0.3): Von einer Gegenstelle außerhalb
  von Loopback lässt Baruch weiterhin nur `/mcp` durch. Ein Lektor kommt über den Reverse-Proxy auf demselben
  Rechner (`--allow-host`, TLS am Proxy) – nicht über `--listen` im Netz.
- **`/mcp` bleibt bei Tokens.** Die Anmeldung gilt für Seiten, Formulare und `/rpc`; MCP-Clients melden sich
  nicht an und brauchen kein Konto.
- **Sitzung:** Nach der Anmeldung setzt Baruch das Cookie `baruch_session` (`HttpOnly`, `SameSite=Strict`,
  über den Namen des Proxys zusätzlich `Secure`). Gespeichert ist nur ein Hash des Sitzungsschlüssels. Eine
  Sitzung endet nach 30 Tagen ohne Benutzung, spätestens nach 90 Tagen, beim Abmelden, beim Sperren des Kontos
  und – für alle anderen Sitzungen des Kontos – beim Passwortwechsel.
- **`SameSite=Strict` und Links von außen:** Folgt jemand einem Link aus einer E-Mail oder von einer anderen
  Seite, schickt der Browser das Sitzungs-Cookie bei diesem ersten Sprung nicht mit. Baruch zeigt dann die
  Anmeldeseite; der Link „Schon angemeldet? Weiter“ dort führt ohne erneute Anmeldung ans Ziel.
- **`Host` unverändert durchreichen** ist mit Konten doppelt wichtig: wegen der Prüfung schreibender Anfragen
  (`Origin` gegen `Host`), wegen `Secure` am Cookie und wegen der Ausnahme für den eigenen Rechner.
- **Durchprobieren von Passwörtern** bremst Baruch selbst: Ab dem vierten Fehlversuch je Konto (und je Adresse,
  wenn die Anfrage nicht über den Proxy kommt) wächst eine Wartezeit bis höchstens 15 Minuten; solange antwortet
  die Anmeldung mit 429 und `Retry-After`. Hinter dem Proxy sieht Baruch nur dessen Adresse – eine Bremse je
  Besucheradresse gehört darum in den Proxy (bei nginx `limit_req`), wenn sie gewünscht ist.
- **Passwörter** speichert Baruch nur als Argon2id-Prüfwert in der Datenbank (Tabelle `account`), nie im
  Klartext und nie in Protokollen. Die Sicherung des Datenordners enthält diese Prüfwerte und die Hashes der
  Sitzungen – sie gehört weiterhin unter Verschluss.
- **Konten und Freigaben sind Daten der Instanz:** Sie stehen nicht im Austauschformat und nicht im Git-Spiegel.
  Was ein Lektor schreibt (Vorschläge, Beiträge, Befunde), steht mit seiner Herkunft `@lektorat:<name>` im Werk
  und im Spiegel.
- **Passwort vergessen (Notweg):** am Rechner, auf dem Baruch läuft, als der Benutzer des Dienstes:

  ```sh
  baruch-server passwort NAME [--data ORDNER]     # liest das neue Passwort als eine Zeile von der Eingabe
  ```

  Das setzt das Passwort des Kontos und beendet seine Sitzungen. Der Befehl braucht keine Anmeldung – wer auf
  dem Rechner als dieser Benutzer arbeiten kann, hat ohnehin den Datenordner.

Probe mit Konten, von einem **anderen** Rechner (`NAME` = der erlaubte Name):

```sh
# 1. Ohne Anmeldung leitet Baruch auf die Anmeldeseite um: 303 mit Location /anmelden?next=…
curl -s -o /dev/null -w '%{http_code} %{redirect_url}\n' https://NAME/

# 2. /rpc ohne Anmeldung: 401
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://NAME/rpc -H 'Origin: https://NAME' \
  -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"work.list"}'

# 3. Einrichten über den Proxy ist gesperrt: 303 (es gibt schon Konten) bzw. 403 (noch keine)
curl -s -o /dev/null -w '%{http_code}\n' https://NAME/einrichten
```

Kommt bei 1 eine **200** mit der Arbeitsseite, hält Baruch den Proxy für den eigenen Rechner („Am eigenen Rechner
ohne Anmeldung“ ist eingeschaltet, und der Proxy schreibt `Host` um, ohne `X-Forwarded-For` zu setzen): sofort
den Schalter ausschalten und den Proxy berichtigen.

## `/mcp` mit Token

Unter einem zusätzlich erlaubten Namen gilt eine Anfrage **nie als lokal**: `/mcp` verlangt dort immer
`Authorization: Bearer <Token>`, auch wenn die Gegenstelle (der Proxy) Loopback ist. Ohne Token: 401.

1. Im Browser über den Proxy **Einstellungen → MCP-Zugang → Token erzeugen**. Baruch zeigt den Token genau
   einmal und speichert nur seinen Hash.
2. Die Seite zeigt unter dem erlaubten Namen gleich die passende Adresse und den Befehl:

   ```sh
   claude mcp add -s local --transport http baruch https://baruch.example.org/mcp \
     --header "Authorization: Bearer <Token>"
   ```

   Das Schema `https` nimmt Baruch dabei an (TLS endet im Proxy; Baruch kennt das Schema der Anfrage nicht).
3. Beiträge und Vorschläge dieses Zugangs stehen unter `@mcp:<Name des Tokens>`. Widerrufen gilt sofort.

Direkt auf dem Rechner (`http://127.0.0.1:8765/mcp`) bleibt `/mcp` wie bisher ohne Token erreichbar. Über MCP
lässt sich kein Buchtext ändern oder löschen (`docs/MCP.md`); lesen lässt sich alles.

Schützt der Proxy alles mit Client-Zertifikaten oder steht er nur im VPN, braucht `/mcp` keine Ausnahme: Dann
gelten beide Schutzschichten zugleich.

## Verbindungen nach außen

Baruch öffnet von sich aus nur Verbindungen, die unter **Einstellungen → Verbindungen** eingetragen sind
(Modell-Backend, MultiGPT, pgvector); Websuche und Seitenabruf laufen über MultiGPT und nur, wenn der Schalter der
Persona es erlaubt.
**MultiGPT hat seit Runde 24 keine Adresse als Vorgabe**: Ohne eingetragene Adresse ist der Weg „nicht
eingerichtet“ und wird nie angefragt.

### TLS zu pgvector und `PGSSLROOTCERT`

Der TLS-Modus der Verbindung pgvector (`disable`, `prefer`, `require`, `verify-full`) steht unter Einstellungen
→ Verbindungen. Dazu liest die Datenbankbibliothek (sqlx 0.8.6) beim Aufbau **Umgebungsvariablen** des
Dienstes: `PGSSLROOTCERT` (Wurzelzertifikat, PEM-Datei), `PGSSLCERT` und `PGSSLKEY` (Client-Zertifikat und
Schlüssel), `PGOPTIONS`. Nicht gelesen werden `PGPASSWORD` und `~/.pgpass` – das Passwort kommt nur aus dem
Schlüsselbund.

Befund aus dem Quelltext von sqlx 0.8.6 (`sqlx-postgres/src/connection/tls.rs`,
`sqlx-core/src/net/tls/tls_rustls.rs`), **gelesen, nicht gegen einen Server geprüft**:

| TLS-Modus | Zertifikat geprüft | Name geprüft | `PGSSLROOTCERT` |
|---|---|---|---|
| `disable` | – (kein TLS) | – | ohne Wirkung |
| `prefer` | nein | nein | ohne Wirkung; fällt ohne TLS auf Klartext zurück |
| `require` | **nein** | nein | **ohne Wirkung** – die Datei wird nicht einmal gelesen |
| `verify-full` | ja | ja | zusätzliche Wurzel neben denen des Systems |

- Das Backlog führte die Vermutung, mit `PGSSLROOTCERT` prüfe sqlx auch bei `require` (so verhält sich libpq:
  liegt eine Wurzeldatei vor, prüft es bei `require` wie bei `verify-ca`). **Für sqlx 0.8.6 trifft das nach dem
  Quelltext nicht zu:** Bei `require` nimmt sqlx jedes Zertifikat an. `require` verschlüsselt also, schützt
  aber nicht vor einem Angreifer im Netz, der sich als Datenbank ausgibt.
- Wer Schutz will, wählt **`verify-full`**. Hat der Server ein Zertifikat einer eigenen Zertifizierungsstelle,
  bekommt der Dienst ihre Wurzel über die Umgebung:

  ```ini
  # ~/.config/systemd/user/baruch.service, Abschnitt [Service]
  Environment=PGSSLROOTCERT=/etc/ssl/eigene-ca/wurzel.pem
  ```

  Mit `verify-full` und einer unlesbaren oder ungültigen Datei scheitert die Verbindung mit einer Meldung
  (Annahme nach dem Quelltext).
- `PGSSLCERT` und `PGSSLKEY` gelten in jedem TLS-Modus und nur zusammen; ist nur eine der beiden gesetzt,
  scheitert jede TLS-Verbindung zu pgvector (Annahme nach dem Quelltext).
- Der Hinweis auf der Seite Verbindungen („Passwort hinterlegt und TLS `prefer`/`disable`/`require`“) kennt
  diese Umgebungsvariablen nicht; er urteilt nur nach dem eingestellten Modus.

## Was hinter einem Proxy anders ist – und was nicht

- **Gleich bleibt:** der Schutz schreibender Anfragen (`Origin`/`Sec-Fetch-Site`), die Ablehnung lesender
  Anfragen fremder Seiten als Unterressource, das Einrahmverbot, alle Grenzen für Dateien und Importe.
- **Ohne Konten sind alle „der Autor“.** Baruch unterscheidet dann keine Benutzer. Wer am Zugangsschutz
  vorbeikommt, handelt in der Oberfläche als Autor – auch Übernehmen, Löschen, Einstellungen, Sicherung.
- **Mit Konten** unterscheidet Baruch Autor und Lektoren (siehe [Anmeldung und Konten](#anmeldung-und-konten)).
  Ein Lektor sieht nur freigegebene Werke und schreibt nur Vorschläge, Beiträge und Befunde; alles andere
  beantwortet Baruch mit 403.
- **Cookies** der Oberfläche (`baruch_*`, nur Einstellungen) setzt Baruch ohne `Secure`; sie enthalten keine
  Geheimnisse. Das Sitzungs-Cookie `baruch_session` trägt über den Namen des Proxys `Secure`.
- **Die Adresse des eigenen Rechners gilt weiter:** Unter `http://127.0.0.1:8765/` verhält sich Baruch wie
  ohne Proxy, `/mcp` ohne Token eingeschlossen. Mit Konten verlangt auch diese Adresse eine Anmeldung, solange
  „Am eigenen Rechner ohne Anmeldung“ ausgeschaltet ist.

## Stand der Prüfung

Geprüft (Tests `crates/baruch-server/tests/round24_betrieb.rs`, Unit-Tests in `guard.rs`, ein eigener Server mit
`curl`):

- Ohne `--allow-host` bleibt ein Name 421; mit erlaubtem Namen passieren `Host: name` und `Host: name:8443`,
  ein anderer Name, ein Name mit Leerzeichen oder Platzhalter nicht.
- Ein POST mit `Origin: https://name` und `Host: name` besteht die Prüfung; mit umgeschriebenem `Host` 403.
- `/mcp` unter dem erlaubten Namen von Loopback: ohne Token 401, mit Token 200; unter `127.0.0.1` wie bisher
  ohne Token.
- Ungültige Namen beenden `serve` mit Exit 2 und Meldung, als Angabe und als Umgebungsvariable.
- Die Seite MCP-Zugang zeigt unter dem erlaubten Namen Adresse und Befehl mit Token.

Nicht geprüft, also Annahme:

- die Beispiele für nginx und Caddy an einem laufenden Proxy, samt Zertifikat, Basic-Auth, Client-Zertifikat,
  HTTP/2 nach außen und Live-Antworten durch den Proxy;
- alles zu `PGSSLROOTCERT`, `PGSSLCERT`, `PGSSLKEY` gegen einen PostgreSQL-Server mit TLS (Befund nur aus dem
  Quelltext von sqlx 0.8.6);
- das Zurückspielen einer Sicherung;
- der Betrieb unter macOS und Windows hinter einem Proxy.

## Offen

- Baruch kann einen Proxy, der `Host` umschreibt und keine Proxy-Kopfzeile setzt, nicht erkennen (siehe Proben).
- Ein eigenes TLS in Baruch gibt es nicht; `Secure` trägt nur das Sitzungs-Cookie.
- Konten (Runde 35): Anmeldung, Sitzung und Sperren sind mit Tests gegen den Server geprüft, **nicht** an einem
  echten Proxy mit TLS und nicht in einem echten Browser (Handprüfliste: `docs/Architektur.md`, „Runde 35 – Konten und Rollen“).
  Zwei-Faktor-Anmeldung, Zurücksetzen per E-Mail und mehrere Autoren gibt es nicht.
- Der TLS-Hinweis der Verbindung pgvector kennt die Umgebungsvariablen nicht.
