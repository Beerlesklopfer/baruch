---
title: "Betrieb"
description: "Für alle, die Baruch dauerhaft betreiben: Datenordner, Sicherung und Wiederherstellen, Aktualisieren, Erreichbarkeit im Netz mit vorgeschaltetem Server, Konten und Zugang für KI-Programme."
inhalt: true
---

Diese Seite richtet sich an Betreiber: an alle, die Baruch dauerhaft laufen lassen, sichern, auf einen anderen
Rechner umziehen oder unter einem eigenen Namen im Netz erreichbar machen. Wie man mit Baruch schreibt, steht in
der [Hilfe](/hilfe/).

Bei jedem Schritt steht dabei, ob er **geprüft** ist oder nur **angenommen**. Was noch niemand an einem echten
Aufbau erprobt hat, sammelt der Abschnitt [Was nicht erprobt ist](#nicht-erprobt).

## Das Wichtigste zuerst {#das-wichtigste}

- **Ohne Konten hat Baruch keine Anmeldung.** Wer die Oberfläche erreicht, liest und ändert alles: Werke,
  Fassungen, Einstellungen, Verbindungen. Darum ist Baruch nach der Installation nur vom eigenen Rechner aus
  erreichbar, unter `http://127.0.0.1:8765/`.
- **Mit Konten verlangt Baruch selbst eine Anmeldung** – für alle Seiten und Formulare. Programme, die über die
  Schnittstelle für KI-Programme zugreifen (MCP, siehe [unten](#mcp-token)), melden sich nicht mit einem Konto
  an, sondern mit einem **Token**: einem langen, zufälligen Schlüsselwort, das Baruch erzeugt.
- **Wer Baruch über das Netz erreichen will, stellt einen Reverse-Proxy davor.** Das ist ein Webserver (etwa
  nginx oder Caddy), der die Anfragen aus dem Netz annimmt und an Baruch weiterreicht. Er muss **TLS** sprechen,
  also die Verbindung verschlüsseln (das „https“ in der Adresse) – sonst gingen Passwort und Sitzung lesbar über
  die Leitung. Ohne Konten muss er außerdem **den Zugang schützen**; mit Konten ist dieser zusätzliche Schutz
  freiwillig, bleibt aber eine sinnvolle zweite Schicht.
- **Baruch fragt nie von sich aus im Netz nach neuen Versionen.** Aktualisiert wird über die Paketverwaltung
  oder von Hand.

## Datenordner {#datenordner}

| System | Ordner |
|---|---|
| Linux | `~/.local/share/baruch` (bzw. `$XDG_DATA_HOME/baruch`) |
| macOS | `~/Library/Application Support/baruch` |
| Windows | `%LOCALAPPDATA%\baruch` (Local, nicht Roaming) |

Ein anderer Ordner: `baruch-server --data ORDNER` oder die Umgebungsvariable `BARUCH_DATA`. Im Ordner liegen:

- `baruch.sqlite` – die Datenbank und damit **die Quelle aller Daten**. Solange Baruch läuft, liegen daneben
  zwei Hilfsdateien mit den Endungen `-wal` und `-shm`.
- `works/<Name>/` – je Werk der **Git-Spiegel**: eine automatische Mitschrift aller Änderungen als gewöhnliche
  Textdateien, die sich mit dem Versionsprogramm Git und auch ohne Baruch lesen lässt. Nur lesen, nichts darin
  ändern: Den Stand bestimmt die Datenbank, und beim Start führt Baruch die Spiegel nach. Im Spiegel steht auch,
  was beim Löschen eines Gliederungspunkts verloren ging (die aktuelle Fassung, die übrigen Fassungen, die
  Beiträge) – ein versehentlich gelöschter Punkt lässt sich daraus von Hand wiederherstellen.
- `lancedb/` – der Suchindex der Bedeutungssuche (der Suche nach dem Sinn statt nach dem Wortlaut). Er lässt
  sich jederzeit neu aufbauen.

**Rechte:** Legt Baruch den Ordner selbst an, dürfen unter Linux und macOS nur Sie selbst hinein (Rechte 0700).
Einen vorhandenen Ordner ändert Baruch nie. Sind dessen Rechte offener, steht das als Hinweis unter
**Einstellungen → Daten**; Abhilfe von Hand: `chmod 700 <Datenordner>`.

**Schlüssel, Token und Passwörter** der Verbindungen liegen nicht im Datenordner, sondern im **Schlüsselbund**
des Systems, dem Passwortspeicher des Betriebssystems. Das gilt, wenn Baruch mit dem üblichen Datenordner läuft
oder mit `--secrets keyring` gestartet ist (auch `BARUCH_SECRETS=keyring`). Mit einem eigenen Datenordner ohne
diese Angabe bleiben sie nur im Arbeitsspeicher und sind nach dem Beenden weg. Was gilt, sagt Baruch beim Start
in der Zeile „Geheimnisse: …“. Von den Tokens für KI-Programme steht in der Datenbank nur ein Prüfwert, nie der
Token selbst.

Kein Paket legt im Datenordner etwas an, und kein Deinstallieren löscht dort etwas.

{{< hilfebild name="einstellungen-daten" alt="Einstellungen, Seite Daten: Pfad des Datenordners, Name der Instanz und der Knopf „Sicherung jetzt“ mit dem Kästchen zur Bestätigung." titel="Einstellungen → Daten zeigt den Datenordner und legt Sicherungen an." >}}

## Sicherung {#sicherung}

**Einstellungen → Daten → „Sicherung jetzt“** (Kästchen ankreuzen) legt **neben** dem Datenordner einen Ordner
`baruch.sicherung-JJJJ-MM-TT-HHMMSS` an – unter Linux also etwa
`~/.local/share/baruch.sicherung-2026-11-24-213000`. Darin liegen die Datenbank als in sich stimmiger Stand, die
Spiegel und alles andere. Nichts wird überschrieben. Liegen schon zehn Sicherungen daneben, lehnt Baruch
weitere ab, bis alte verschoben oder gelöscht sind – selbst löscht Baruch keine. Die Meldung „Sicherung
angelegt“ erscheint einmal; nach einem Neustart steht sie nicht mehr da, die Sicherung selbst bleibt liegen.
Die Bedienung aus Sicht der Schreibenden beschreibt die
[Hilfe zur Sicherung](/hilfe/austausch-sicherung/#sicherung).

- **Vor jeder neuen Version** eine Sicherung anlegen. Beim ersten Start einer neuen Version baut Baruch die
  Datenbank von selbst auf den neuen Stand um; ein Zurück auf die alte Version gibt es danach nur über die
  Sicherung.
- **Sicherungen woanders hinlegen.** Sie liegen auf derselben Platte wie die Daten; gegen einen Ausfall der
  Platte hilft nur eine Kopie an anderer Stelle (Backup-Programm, anderer Rechner).
- **Den Datenordner selbst kopieren** nur, wenn Baruch nicht läuft: Die Datenbank schreibt im Betrieb in
  mehrere Dateien, eine Kopie mittendrin kann unstimmig sein.
- **Der Schlüsselbund gehört nicht zur Sicherung.** Nach einem Umzug auf einen anderen Rechner die Schlüssel
  der Verbindungen neu eintragen ([Einstellungen → Verbindungen](/hilfe/einstellungen/#verbindungen)).
- **Sicherungen unter Verschluss halten.** Mit Konten enthält die Datenbank die Prüfwerte der Passwörter und der
  laufenden Anmeldungen – keine Passwörter im Klartext, aber auch nichts für fremde Augen.

### Wiederherstellen {#wiederherstellen}

**Angenommen, nicht erprobt:** Dieser Weg ist so beschrieben, aber noch nicht in einem echten Ernstfall
durchgespielt. Darum Schritt 2 – den alten Ordner aufheben.

1. Baruch beenden ([Hilfe: Baruch beenden](/hilfe/erste-schritte/#beenden)).
2. Den Datenordner umbenennen (etwa `baruch` in `baruch.alt`), nicht löschen.
3. Die Sicherung an seine Stelle kopieren und in `baruch` umbenennen.
4. Baruch starten.

Eine Sicherung lässt sich vorher ansehen, ohne den laufenden Bestand zu berühren – an einer **Kopie**, denn
Baruch schreibt beim Start in den Ordner, den es öffnet:

```sh
cp -a ~/.local/share/baruch.sicherung-2026-11-24-213000 ~/baruch-probe
baruch-server --data ~/baruch-probe --listen 127.0.0.1:8766
```

Dann `http://127.0.0.1:8766/` öffnen. Schlüssel bleiben dabei nur im Arbeitsspeicher.

### Ein einzelnes Werk mitnehmen {#werk-mitnehmen}

Ein einzelnes Werk trägt die Austauschdatei von einem Rechner zum anderen – mit allen Fassungen, Einfällen,
Beiträgen und Bildern, aber ohne Schlüssel:

```sh
baruch-server export ID --baruch DATEI.baruch
```

`ID` ist die Nummer des Werks. Dasselbe geht in der Oberfläche; siehe
[Hilfe: Austausch](/hilfe/austausch-sicherung/). Konten und Freigaben für Lektoren gehören zur Installation,
nicht zum Werk: Sie stehen weder in der Austauschdatei noch im Git-Spiegel.

## Dauerbetrieb und Aktualisieren {#dauerbetrieb}

Das Debian-Paket richtet keinen Dienst ein. Wer Baruch dauerhaft laufen lassen will, nimmt das mitgelieferte
Beispiel für einen Dienst des angemeldeten Benutzers (systemd):

```sh
install -Dm644 /usr/share/doc/baruch/examples/baruch.service ~/.config/systemd/user/baruch.service
systemctl --user daemon-reload
systemctl --user enable --now baruch.service
```

Baruch läuft dabei als Dienst **des Benutzers**, nicht als Dienst des Systems – nur so erreicht es dessen
Schlüsselbund und Datenordner.

**Aktualisieren:** Sicherung anlegen, neue Version einspielen (`sudo apt upgrade` bzw. die neue Datei von der
Release-Seite, siehe [Installation](/installation/#aktualisieren)), Dienst neu starten
(`systemctl --user restart baruch.service`). Die Seite **Baruch ▾ → Über Baruch** zeigt die laufende Version und
verlinkt die Release-Seite; dieser Klick ist die einzige „Nachfrage“ nach neuen Versionen.

### Schlüsselbund auf einem Server ohne Desktop {#schluesselbund}

Unter Linux spricht Baruch den Schlüsselbund über den „Secret Service“ an (gnome-keyring, KWallet, KeePassXC).
Auf einem Server ohne grafische Oberfläche läuft der oft nicht. Dann gilt:

- Schreiben, Buchsatz und alles, was keine Verbindung braucht, geht weiter. Beim Lesen oder Speichern eines
  Schlüssels meldet Baruch „kein Zugriff auf den Schlüsselbund (gesperrt oder nicht erreichbar)“ – nie mit dem
  Schlüssel selbst im Text.
- **Umgebungsvariablen für Schlüssel gibt es bewusst nicht**; Schlüssel gehören nicht in Dienstdateien oder
  Prozesslisten.
- **Wege:** einen Secret-Service-Dienst auch ohne Desktop betreiben (etwa gnome-keyring oder KeePassXC in der
  Sitzung des Dienst-Benutzers) – oder Baruch mit `--secrets memory` starten. Dann bleiben die Schlüssel nur im
  Arbeitsspeicher und müssen nach jedem Neustart neu eingetragen werden.

## Erreichbarkeit: drei Stufen {#erreichbarkeit}

| Stufe | Start | Erreichbar | Schutz |
|---|---|---|---|
| 1. Nur der eigene Rechner (so wird Baruch installiert) | `baruch-server` | `http://127.0.0.1:8765/`, `localhost`, `[::1]` | der Rechner und das eigene Benutzerkonto |
| 2. Nur KI-Programme aus dem eigenen Netz | `baruch-server --listen 192.168.1.20:8765` | von anderen Rechnern nur die Adresse `/mcp`, unter der IP-Adresse | Token; **keine Verschlüsselung** – nur im vertrauenswürdigen Netz |
| 3. Hinter einem Reverse-Proxy | `baruch-server --allow-host NAME` | alles unter `https://NAME/` | TLS im Proxy; Anmeldung in Baruch (Konten) oder Zugangsschutz im Proxy; `/mcp` mit Token |

Stufe 2 beschreibt [MCP und Assistenten](/mcp/#mcp-clients). Der Rest dieser Seite ist Stufe 3.

### Warum Baruch Rechnernamen sonst ablehnt {#rechnernamen}

Ohne `--allow-host` antwortet Baruch nur unter einer IP-Adresse oder `localhost`; unter jedem anderen Namen
kommt die Fehlerantwort **421**. Das schützt vor einem Angriff namens DNS-Rebinding: Eine fremde Webseite kann
einen eigenen Namen auf `127.0.0.1` zeigen lassen und so den Browser dazu bringen, Baruch unter diesem Namen
anzusprechen. Eine IP-Adresse lässt sich nicht umbiegen. Ein erlaubter Name hebt diesen Schutz genau für diesen
Namen auf – darum wird er ausdrücklich genannt.

## Hinter einem Reverse-Proxy {#reverse-proxy}

### 1. Namen erlauben {#namen-erlauben}

```sh
baruch-server --allow-host baruch.example.org
baruch-server --allow-host baruch.example.org --allow-host schreibstube.example.net
BARUCH_ALLOWED_HOSTS=baruch.example.org,schreibstube.example.net baruch-server
```

`--allow-host` darf mehrfach stehen; `BARUCH_ALLOWED_HOSTS` trennt mit Komma oder Leerzeichen. Beides zusammen
ergibt die Vereinigung. In der Dienstdatei: `Environment=BARUCH_ALLOWED_HOSTS=baruch.example.org`.

Ein gültiger Name (**geprüft**):

- 1 bis 253 Zeichen; die Teile zwischen den Punkten 1 bis 63 Zeichen aus `a–z`, `0–9` und `-` (nicht am Rand).
  Großbuchstaben werden klein gelesen. Ein Name ohne Punkt (`baruch` im eigenen Netz) gilt.
- **Ohne Port** – der Port in der Anfrage ist beliebig (`baruch.example.org` und `baruch.example.org:8443`
  gehen beide durch).
- **Kein Platzhalter** (`*.example.org`), jeder Name einzeln. Erlaubt ist genau der Name: `x.baruch.example.org`
  bleibt abgelehnt.
- **Keine IP-Adresse** und nicht `localhost` – die gelten ohnehin. Auch Namen, deren letzter Teil eine Zahl ist
  (`127.1`), sind abgelehnt, weil Browser sie als IP-Adresse lesen.
- Umlaute nur in der ASCII-Form (`xn--…`).

Ein ungültiger Name beendet den Start mit einer Meldung, die ihn nennt („Ungültiger Rechnername „…“
(--allow-host): …“). Beim Start nennt Baruch die erlaubten Namen.

### 2. Baruch bleibt lokal, der Proxy steht auf demselben Rechner {#baruch-bleibt-lokal}

Baruch lauscht weiter auf `127.0.0.1:8765`; nur der Proxy öffnet einen Port nach außen. So führt kein Weg am
Proxy vorbei. Der Proxy muss **auf demselben Rechner** laufen: Kommt eine Anfrage direkt von einem anderen
Rechner, lässt Baruch nur die Adresse `/mcp` durch und lehnt alles andere ab (Fehler 403), gleich unter welchem
Namen.

### 3. Regeln für den Proxy {#regeln}

1. **Zugangsschutz.** Ohne Konten Pflicht: Passwortabfrage des Proxys (Basic-Auth), Client-Zertifikat oder ein
   VPN vor allem außer `/mcp` – Baruch selbst prüft dann niemanden. Mit Konten prüft Baruch selbst; der Schutz
   im Proxy ist dann freiwillig. Wer beides nutzt, meldet sich zweimal an.
2. **Die Kopfzeile `Host` unverändert durchreichen.** In `Host` steht, unter welchem Namen der Browser die
   Seite aufgerufen hat. Baruch vergleicht sie bei jedem Speichern mit der Herkunft der Anfrage. Schreibt der
   Proxy `Host` um (bei nginx ist das die Voreinstellung!), lehnt Baruch Speichern und alle Formulare mit dem
   Fehler 403 „Origin passt nicht zu Host“ ab. Schlimmer: Baruch hielte dann jede Anfrage für eine vom eigenen
   Rechner – siehe die [Probe](#probe).
3. **TLS endet im Proxy.** Baruch selbst spricht nur unverschlüsseltes HTTP. Die Kopfzeilen, mit denen ein
   Proxy die ursprüngliche Anfrage beschreibt (`X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host`,
   `Forwarded`), **wertet Baruch nicht aus**. Einzige Ausnahme: Ist „Am eigenen Rechner ohne Anmeldung“
   eingeschaltet ([Konten](#konten)), gilt eine Anfrage mit einer dieser Kopfzeilen (dazu `X-Real-IP` und `Via`)
   **nicht** als eigener Rechner. Sie zu setzen ist darum empfohlen.
4. **HTTP/1.1 zu Baruch** (bei nginx und Caddy üblich).
5. **Nichts zwischenspeichern, nichts umschreiben**, lange Antworten nicht puffern. (**Angenommen:** Antworten
   der Assistenten, die Wort für Wort erscheinen, kommen sonst verzögert an; mit einem echten Proxy nicht
   geprüft.)

### Beispiel nginx {#nginx}

**Nicht an einem laufenden nginx erprobt.** Geprüft ist, wie Baruch auf die Kopfzeilen antwortet, die diese
Einstellung schickt.

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
        proxy_set_header   Host $http_host;   # unverändert, samt Port – nicht $proxy_host (Voreinstellung)
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
Port weg; auf einem anderen Port als 443 passte die Herkunft der Anfrage dann nicht mehr zu `Host`, und Baruch
lehnte das Speichern ab.

### Beispiel Caddy {#caddy}

**Ebenfalls nicht an einem laufenden Caddy erprobt.** Caddy holt das Zertifikat selbst und reicht `Host`
unverändert durch (Annahme nach der Dokumentation von Caddy 2).

```caddyfile
baruch.example.org {
    # Pflicht ohne Konten: Zugangsschutz. /mcp schützt der Token.
    @oberflaeche not path /mcp
    basic_auth @oberflaeche {
        autorin HASH-AUS-caddy-hash-password
    }
    reverse_proxy 127.0.0.1:8765
}
```

In älteren Fassungen von Caddy 2 heißt `basic_auth` noch `basicauth`.

### Probe nach dem Einrichten (Pflicht) {#probe}

Von einem **anderen** Rechner, `NAME` = der erlaubte Name. Die Befehle geben jeweils nur die Nummer der Antwort
aus.

```sh
# 1. Ohne Anmeldung keine Oberfläche: 401 (vom Proxy)
curl -s -o /dev/null -w '%{http_code}\n' https://NAME/

# 2. /mcp ohne Token: 401 (von Baruch). Kommt hier 200, reicht der Proxy Host nicht durch – sofort abschalten!
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://NAME/mcp \
  -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"ping"}'

# 3. Mit Anmeldung: 200; danach im Browser einen Abschnitt speichern (kein Fehler 403)
curl -s -o /dev/null -w '%{http_code}\n' -u NUTZER https://NAME/
```

Zu 2: Schreibt der Proxy `Host` auf `127.0.0.1:8765` um, sieht Baruch eine Anfrage vom eigenen Rechner unter
einer IP-Adresse – genau wie von einem Programm, das dort läuft – und nimmt `/mcp` **ohne Token** an. Ist `/mcp`
dann noch vom Zugangsschutz ausgenommen, stünde dieser Zugang offen im Netz. Baruch kann diesen Fall nicht
erkennen; die Probe ist darum Pflicht.

Mit Konten antwortet die erste Probe nicht mit 401 vom Proxy, sondern mit einer Umleitung auf die Anmeldeseite;
siehe die [Probe mit Konten](#probe-konten).

Auf dem Rechner selbst, ohne Proxy, zeigt dieselbe Prüfung das Verhalten von Baruch (**geprüft**):

```sh
curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: baruch.example.org' http://127.0.0.1:8765/     # 200
curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: fremd.example.org'  http://127.0.0.1:8765/     # 421
```

## Anmeldung und Konten {#konten}

Baruch kennt zwei Arten von Konten: einen **Autor** und beliebig viele **Lektoren**. Wie man Konten einrichtet,
Lektoren anlegt und ihnen Werke freigibt, steht in der Hilfe unter
[Konten einrichten](/hilfe/konten/#einrichten) und [Lektoren](/hilfe/konten/#lektoren). Hier steht, was das für
den Betrieb heißt.

{{< hilfebild name="konten-einrichten" alt="Seite „Konten einrichten“: Felder für den Namen und das Passwort des ersten Kontos, des Autors." titel="Das erste Konto entsteht am eigenen Rechner." breite="640" >}}

- **Ohne Konten ändert sich nichts.** Keine Anmeldung, keine Umleitung. Dann gilt alles oben: nur der eigene
  Rechner, oder ein Proxy mit Zugangsschutz.
- **Das erste Konto** legt der Autor **am eigenen Rechner** an (`http://127.0.0.1:8765/einrichten` oder
  **Baruch ▾ → Konten einrichten…**), nie über den Proxy: Dort ist das Einrichten gesperrt. Sonst könnte der
  Erste, der die Adresse erreicht, die Schreibwerkstatt übernehmen.
- **Mit Konten gilt die Anmeldung auch am eigenen Rechner.** Der Grund: Ein Proxy auf demselben Rechner
  verbindet sich ebenfalls von dort. Schreibt er `Host` um, sähe er aus wie der eigene Rechner – eine Ausnahme
  wäre dann eine vollständige Umgehung der Anmeldung. Der Autor kann unter **Einstellungen → Konten** den
  Schalter „Am eigenen Rechner ohne Anmeldung“ einschalten. Die Ausnahme gilt nur für Anfragen vom eigenen
  Rechner unter IP-Adresse oder `localhost` (nie unter einem erlaubten Namen) und nur ohne die Kopfzeilen eines
  Proxys (`Forwarded`, `X-Forwarded-For`, `X-Forwarded-Host`, `X-Forwarded-Proto`, `X-Real-IP`, `Via`). **Nur
  einschalten, wenn der Proxy `Host` unverändert durchreicht** (Probe unten) – am besten setzt er zusätzlich
  `X-Forwarded-For`.
- **Direkter Zugriff von anderen Rechnern bleibt gesperrt.** Ein Lektor kommt über den Reverse-Proxy auf
  demselben Rechner (Stufe 3), nicht über `--listen` im Netz.
- **KI-Programme bleiben bei Tokens.** Die Anmeldung gilt für Seiten und Formulare; Programme an der Adresse
  `/mcp` brauchen kein Konto. Das heißt auch: Ein Programm auf dem eigenen Rechner erreicht `/mcp` weiter ohne
  Token und ohne Anmeldung und kann dort schreiben, was über diesen Weg immer erlaubt war (Vorschläge, Beiträge,
  Einfälle, Gliederungspunkte, Anträge – nie Buchtext ändern oder löschen). Konten schützen diesen Weg nicht.
- **Sitzung:** Nach der Anmeldung merkt sich der Browser die Sitzung in einem Cookie, einem kleinen Merkzettel,
  den er bei jeder Anfrage mitschickt. Skripte einer Seite können ihn nicht lesen, fremde Seiten lösen ihn nicht
  aus, und über den Namen des Proxys geht er nur über verschlüsselte Verbindungen. Eine Sitzung endet nach
  30 Tagen ohne Benutzung, spätestens nach 90 Tagen, beim Abmelden, beim Sperren des Kontos und – für alle
  anderen Sitzungen des Kontos – beim Wechsel des Passworts.
- **Links von außen:** Folgt jemand einem Link aus einer E-Mail oder von einer anderen Seite, schickt der
  Browser die Sitzung bei diesem ersten Sprung nicht mit. Baruch zeigt dann die Anmeldeseite; der Link „Schon
  angemeldet? Weiter“ dort führt ohne erneute Anmeldung ans Ziel.
- **`Host` unverändert durchreichen** ist mit Konten doppelt wichtig: wegen der Prüfung beim Speichern, wegen
  des Schutzes der Sitzung und wegen der Ausnahme für den eigenen Rechner.
- **Durchprobieren von Passwörtern** bremst Baruch selbst: Ab dem vierten Fehlversuch je Konto wächst eine
  Wartezeit bis höchstens 15 Minuten; solange lehnt die Anmeldung ab (Fehler 429) und nennt die Wartezeit.
  Hinter dem Proxy sieht Baruch nur dessen Adresse – eine Bremse je Besucher gehört darum in den Proxy (bei
  nginx `limit_req`), wenn sie gewünscht ist.
- **Die Sperre kann den Autor aussperren:** Wer den Kontonamen kennt, hält das Konto mit falschen Anmeldungen in
  der Wartezeit; solange hilft auch das richtige Passwort nicht. An Daten kommt er damit nicht. Abhilfe: einen
  Kontonamen wählen, der sich nicht raten lässt, die Bremse im Proxy einrichten – und der Notweg unten.
- **Passwörter** speichert Baruch nur als Prüfwert, aus dem sich das Passwort nicht zurückrechnen lässt – nie
  im Klartext und nie in Protokollen.
- **Was ein Lektor schreibt** (Vorschläge, Beiträge, Befunde), steht mit seiner Herkunft `@lektorat:<name>` im
  Werk und im Git-Spiegel.

{{< hilfebild name="konten-einstellungen" alt="Einstellungen, Seite Konten: das Konto der Autorin, ein erfundener Lektor und der Schalter „Am eigenen Rechner ohne Anmeldung“." titel="Einstellungen → Konten: Autorin, Lektoren und die Ausnahme für den eigenen Rechner." >}}

### Passwort vergessen (Notweg) {#passwort-vergessen}

Am Rechner, auf dem Baruch läuft, als der Benutzer, dem der Dienst gehört:

```sh
baruch-server passwort NAME [--data ORDNER]     # liest das neue Passwort als eine Zeile von der Eingabe
```

Das setzt das Passwort des Kontos, beendet seine Sitzungen und hebt eine Sperre nach Fehlversuchen für dieses
Konto auf: Die Anmeldung mit dem neuen Passwort gelingt sofort. Eine Sperre je Besucheradresse (die gibt es nur
ohne Proxy) läuft nach höchstens 15 Minuten ohne weiteren Fehlversuch von selbst ab. Der Befehl braucht keine
Anmeldung – wer auf dem Rechner als dieser Benutzer arbeiten kann, hat ohnehin den Datenordner.

### Probe mit Konten {#probe-konten}

Von einem **anderen** Rechner (`NAME` = der erlaubte Name):

```sh
# 1. Ohne Anmeldung leitet Baruch auf die Anmeldeseite um: 303 und die Adresse …/anmelden?next=…
curl -s -o /dev/null -w '%{http_code} %{redirect_url}\n' https://NAME/

# 2. Die Programmierschnittstelle /rpc ohne Anmeldung: 401
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://NAME/rpc -H 'Origin: https://NAME' \
  -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"work.list"}'

# 3. Einrichten über den Proxy ist gesperrt: 303 (es gibt schon Konten) bzw. 403 (noch keine)
curl -s -o /dev/null -w '%{http_code}\n' https://NAME/einrichten
```

Kommt bei 1 eine **200** mit der Arbeitsseite, hält Baruch den Proxy für den eigenen Rechner („Am eigenen
Rechner ohne Anmeldung“ ist eingeschaltet, und der Proxy schreibt `Host` um, ohne `X-Forwarded-For` zu setzen):
sofort den Schalter ausschalten und den Proxy berichtigen.

## Zugang für KI-Programme: `/mcp` mit Token {#mcp-token}

**MCP** (Model Context Protocol) ist die Schnittstelle, über die KI-Programme wie Claude Code mit Baruch
arbeiten; sie liegt unter der Adresse `/mcp`. Wie man ein solches Programm verbindet, steht unter
[MCP und Assistenten](/mcp/#mcp-clients).

Unter einem zusätzlich erlaubten Namen gilt eine Anfrage **nie als eine vom eigenen Rechner**: `/mcp` verlangt
dort immer einen Token, auch wenn der Proxy auf demselben Rechner steht. Ohne Token: Fehler 401 (**geprüft**).

1. Im Browser über den Proxy [Einstellungen → MCP-Zugang](/hilfe/einstellungen/#mcp-zugang) öffnen und einen
   Token erzeugen. Baruch zeigt ihn genau einmal und speichert nur einen Prüfwert.
2. Die Seite zeigt unter dem erlaubten Namen gleich die passende Adresse und den Befehl:

   ```sh
   claude mcp add -s local --transport http baruch https://baruch.example.org/mcp \
     --header "Authorization: Bearer <Token>"
   ```

   Das „https“ nimmt Baruch dabei an: TLS endet im Proxy, Baruch selbst kann es nicht sehen.
3. Beiträge und Vorschläge dieses Zugangs stehen unter `@mcp:<Name des Tokens>`. Widerrufen gilt sofort.

Direkt auf dem Rechner (`http://127.0.0.1:8765/mcp`) bleibt der Zugang ohne Token. Über diesen Weg lässt sich
kein Buchtext ändern oder löschen; lesen lässt sich alles.

Schützt der Proxy alles mit Client-Zertifikaten oder steht er nur im VPN, braucht `/mcp` keine Ausnahme: Dann
gelten beide Schutzschichten zugleich.

## Verbindungen nach außen {#verbindungen}

Baruch öffnet von sich aus nur Verbindungen, die unter
[Einstellungen → Verbindungen](/hilfe/einstellungen/#verbindungen) eingetragen sind: zum Modelldienst für die
Assistenten, zu MultiGPT (einem Dienst für Dokumentensuche und Websuche) und zu einer Datenbank für die
Bedeutungssuche. Im Web sucht nur der Assistent für Belege, nur über MultiGPT und nur, wenn sein Schalter es
erlaubt. Für MultiGPT ist keine Adresse voreingestellt: Ohne eingetragene Adresse ist der Weg „nicht
eingerichtet“ und wird nie angefragt. Die Einrichtung beschreibt
[MCP und Assistenten](/mcp/#multigpt-einrichten).

### Verschlüsselung zur Datenbank der Bedeutungssuche (pgvector) {#pgvector-tls}

Wer die Bedeutungssuche mit einer PostgreSQL-Datenbank und deren Erweiterung pgvector betreibt, wählt unter
Einstellungen → Verbindungen, wie die Verbindung gesichert wird (`disable`, `prefer`, `require`,
`verify-full`). Dazu liest Baruch beim Verbinden **Umgebungsvariablen** des Dienstes: `PGSSLROOTCERT` (das
Wurzelzertifikat als PEM-Datei), `PGSSLCERT` und `PGSSLKEY` (Zertifikat und Schlüssel des Clients) und
`PGOPTIONS`. Nicht gelesen werden `PGPASSWORD` und `~/.pgpass` – das Passwort kommt nur aus dem Schlüsselbund.

**Aus dem Quelltext der benutzten Datenbankbibliothek gelesen, nicht gegen einen Server geprüft:**

| Einstellung | Zertifikat geprüft | Name geprüft | `PGSSLROOTCERT` |
|---|---|---|---|
| `disable` | – (unverschlüsselt) | – | ohne Wirkung |
| `prefer` | nein | nein | ohne Wirkung; fällt ohne TLS auf Klartext zurück |
| `require` | **nein** | nein | **ohne Wirkung** – die Datei wird nicht einmal gelesen |
| `verify-full` | ja | ja | zusätzliche Wurzel neben denen des Systems |

- **`require` verschlüsselt, schützt aber nicht** vor einem Angreifer im Netz, der sich als Datenbank ausgibt:
  Baruch nimmt dabei jedes Zertifikat an. Das ist anders als bei den üblichen PostgreSQL-Programmen, die mit
  einer Wurzeldatei auch bei `require` prüfen.
- Wer Schutz will, wählt **`verify-full`**. Hat der Server ein Zertifikat einer eigenen Zertifizierungsstelle,
  bekommt der Dienst deren Wurzel über die Umgebung:

  ```ini
  # ~/.config/systemd/user/baruch.service, Abschnitt [Service]
  Environment=PGSSLROOTCERT=/etc/ssl/eigene-ca/wurzel.pem
  ```

  Mit `verify-full` und einer unlesbaren oder ungültigen Datei scheitert die Verbindung mit einer Meldung
  (Annahme nach dem Quelltext).
- `PGSSLCERT` und `PGSSLKEY` gelten bei jeder verschlüsselten Einstellung und nur zusammen; ist nur eine der
  beiden gesetzt, scheitert jede verschlüsselte Verbindung zur Datenbank (Annahme nach dem Quelltext).
- Der Hinweis auf der Seite Verbindungen (zu einem hinterlegten Passwort bei `prefer`, `disable` oder
  `require`) kennt diese Umgebungsvariablen nicht; er urteilt nur nach der gewählten Einstellung.

## Was hinter einem Proxy anders ist – und was nicht {#proxy-unterschiede}

- **Gleich bleibt:** der Schutz beim Speichern gegen fremde Webseiten, das Verbot, Baruch in fremde Seiten
  einzubetten, und alle Grenzen für Dateien und Importe.
- **Ohne Konten sind alle „der Autor“.** Baruch unterscheidet dann keine Benutzer. Wer am Zugangsschutz
  vorbeikommt, handelt in der Oberfläche als Autor – auch beim Übernehmen, Löschen, in den Einstellungen und
  bei der Sicherung.
- **Mit Konten** unterscheidet Baruch Autor und Lektoren. Ein Lektor sieht nur freigegebene Werke und schreibt
  nur Vorschläge, Beiträge und Befunde; alles andere lehnt Baruch ab (Fehler 403).
- **Die Merkzettel der Oberfläche** (Thema, Spaltenbreiten und Ähnliches) enthalten keine Geheimnisse. Nur der
  Merkzettel der Sitzung ist besonders geschützt.
- **Die Adresse des eigenen Rechners gilt weiter:** Unter `http://127.0.0.1:8765/` verhält sich Baruch wie ohne
  Proxy, der Zugang für KI-Programme ohne Token eingeschlossen. Mit Konten verlangt auch diese Adresse eine
  Anmeldung, solange „Am eigenen Rechner ohne Anmeldung“ ausgeschaltet ist.
- **Baruch beenden** lässt sich nur am Rechner, auf dem es läuft – nicht über den Proxy.

## Was nicht erprobt ist {#nicht-erprobt}

Geprüft ist das Verhalten von Baruch selbst: Rechnernamen, die Prüfung beim Speichern, `/mcp` mit und ohne
Token, Anmeldung, Sitzung und Sperren – mit automatischen Tests und Proben am eigenen Rechner. **Nicht erprobt**,
also Annahme:

- die Beispiele für nginx und Caddy an einem laufenden Proxy, samt Zertifikat, Passwortabfrage,
  Client-Zertifikat und Antworten der Assistenten durch den Proxy;
- die Anmeldung mit Konten an einem echten Proxy mit TLS und in einem echten Browser;
- alles zu `PGSSLROOTCERT`, `PGSSLCERT` und `PGSSLKEY` gegen einen PostgreSQL-Server mit TLS;
- das Wiederherstellen einer Sicherung;
- der Betrieb unter macOS und Windows hinter einem Proxy.

Was es nicht gibt: ein eigenes TLS in Baruch, Anmeldung mit zweitem Faktor, Zurücksetzen des Passworts per
E-Mail und mehrere Autoren. Baruch kann außerdem einen Proxy, der `Host` umschreibt und keine eigene Kopfzeile
setzt, nicht erkennen – darum die Proben.
