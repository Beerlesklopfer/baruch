# Betrieb

**Die Beschreibung für Betreiber steht auf der Webseite:** <https://baruch.bernau.family/betrieb/> – Quelle ist
`site/content/de/betrieb.md` (seit Runde 37). Dort stehen Datenordner, Sicherung und Wiederherstellen, Dauerbetrieb
und Aktualisieren, die drei Stufen der Erreichbarkeit, der Betrieb hinter einem Reverse-Proxy mit den Beispielen
für nginx und Caddy und der Pflichtprobe, Anmeldung und Konten, `/mcp` mit Token, Verbindungen nach außen und
TLS zu pgvector.

**Regel für Entwickler:** „Betrieb nachziehen“ heißt, `site/content/de/betrieb.md` im selben Commit zu ändern. Diese
Datei hier enthält **keine Schritte für Betreiber**, sondern nur, was dort niemand braucht: den Stand der Prüfung
mit den Tests, Offenes und technische Anmerkungen. Nichts steht in beiden Dateien.

Installation und Pakete: `docs/Verteilung.md`. Bedienung: die Hilfe unter `site/content/de/hilfe/` (Einstieg
`docs/Bedienung.md`). Aufbau der Schutzschichten: `docs/Architektur.md`, Abschnitt „MCP“.

## Wo steht was

| Thema | auf der Webseite |
|---|---|
| Das Wichtigste zuerst | `/betrieb/#das-wichtigste` |
| Datenordner, Rechte, Schlüsselbund | `/betrieb/#datenordner` |
| Sicherung, Wiederherstellen, ein Werk mitnehmen | `/betrieb/#sicherung`, `#wiederherstellen`, `#werk-mitnehmen` |
| Dauerbetrieb, Aktualisieren, Schlüsselbund ohne Desktop | `/betrieb/#dauerbetrieb`, `#schluesselbund` |
| Erreichbarkeit in drei Stufen, abgelehnte Rechnernamen | `/betrieb/#erreichbarkeit`, `#rechnernamen` |
| Hinter einem Reverse-Proxy: Namen erlauben, Regeln, nginx, Caddy, Probe | `/betrieb/#reverse-proxy`, `#namen-erlauben`, `#regeln`, `#nginx`, `#caddy`, `#probe` |
| Anmeldung und Konten, Passwort vergessen, Probe mit Konten | `/betrieb/#konten`, `#passwort-vergessen`, `#probe-konten` |
| `/mcp` mit Token | `/betrieb/#mcp-token` |
| Verbindungen nach außen, TLS zu pgvector | `/betrieb/#verbindungen`, `#pgvector-tls` |
| Was hinter einem Proxy anders ist | `/betrieb/#proxy-unterschiede` |
| Was nicht erprobt ist (für Betreiber) | `/betrieb/#nicht-erprobt` |

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
- Konten (Runde 35): Anmeldung, Sitzung und Sperren mit Tests gegen den Server.

Nicht geprüft, also Annahme (die Liste für Betreiber steht unter `/betrieb/#nicht-erprobt`; hier dazu das
Technische):

- die Beispiele für nginx und Caddy an einem laufenden Proxy, samt Zertifikat, Basic-Auth, Client-Zertifikat,
  HTTP/2 nach außen und Live-Antworten durch den Proxy;
- alles zu `PGSSLROOTCERT`, `PGSSLCERT`, `PGSSLKEY` gegen einen PostgreSQL-Server mit TLS (Befund nur aus dem
  Quelltext, siehe unten);
- das Zurückspielen einer Sicherung;
- der Betrieb unter macOS und Windows hinter einem Proxy;
- Konten an einem echten Proxy mit TLS und in einem echten Browser (Handprüfliste: `docs/Architektur.md`,
  „Runde 35 – Konten und Rollen“).

## Offen

- Baruch kann einen Proxy, der `Host` umschreibt und keine Proxy-Kopfzeile setzt, nicht erkennen (siehe die
  Proben auf der Webseite).
- Ein eigenes TLS in Baruch gibt es nicht; `Secure` trägt nur das Sitzungs-Cookie.
- Zwei-Faktor-Anmeldung, Zurücksetzen per E-Mail und mehrere Autoren gibt es nicht.
- Der TLS-Hinweis der Verbindung pgvector kennt die Umgebungsvariablen nicht.
- Die Hinweistexte im Programm (`src/main.rs`, Hilfe zu `--allow-host`; `templates/settings/mcp.html`) und
  `docs/MCP.md` nennen weiter `docs/Betrieb.md`; diese Datei führt von dort auf die Webseite. Ob sie künftig
  gleich die Adresse `/betrieb/` nennen, entscheidet der Nutzer.

## Technische Anmerkungen

Was auf der Webseite in Alltagssprache steht, hier mit den Namen aus dem Programm.

### Datenordner und Sicherung

- Die Sicherung schreibt die Datenbank mit `VACUUM INTO` (stimmiger Stand trotz laufendem Betrieb); `-wal` und
  `-shm` sind die Begleitdateien von SQLite.
- Die Tokens für `/mcp` stehen als SHA-256-Hash in der Datenbank.
- Geheimnisse: Schlüsselbund-Dienst `baruch`; Schalter `--secrets keyring|memory` bzw. `BARUCH_SECRETS`.
- Neue Datenordner entstehen mit 0700; `baruch.sqlite*` und `works/` entstehen mit der umask des Dienstes.
- „Sicherung angelegt“ ist ein einmaliger Vermerk im laufenden Prozess, kein Wert in der Datenbank.
- Das Beispiel für den Dienst liegt im Repository unter `packaging/systemd/` (dort auch die Bedienung des
  Dienstes), im Paket unter `/usr/share/doc/baruch/examples/baruch.service`.
- Die Seite „Über Baruch“ ist `/ueber`.

### Rechnernamen und Proxy

- Die Prüfung schreibender Anfragen ist `guard::check`: `Origin` gegen `Host`, dazu `Sec-Fetch-Site`; lesende
  Anfragen fremder Seiten als Unterressource werden abgelehnt, Einrahmen ist verboten.
- `--allow-host` gilt nur für den Befehl `serve`. Abgelehnt werden auch Namen wie `0x7f.1` (letzter Teil eine
  Zahl in anderer Schreibweise) und `baruch.example.org.evil.example` (nur der genaue Name gilt).
- Eine Anfrage ohne `Host`-Kopfzeile gilt, sobald Namen erlaubt sind, nicht als lokal.
- `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host` und `Forwarded` wertet Baruch weder für die
  Gegenstelle noch für das Schema aus; das Schema `https` in der Anzeige der Seite MCP-Zugang ist eine Annahme.
- Von einer Gegenstelle außerhalb von Loopback lässt Baruch nur `/mcp` durch (403 für alles andere).

### Konten

- Entscheidungen der Runde 35: 0.2 (mit Konten gilt die Anmeldung auch auf Loopback; Ausnahme nur als Schalter
  des Autors) und 0.3 (direkter Zugriff von anderen Rechnern bleibt gesperrt).
- Sitzungs-Cookie `baruch_session`: `HttpOnly`, `SameSite=Strict`, über einen Namen aus `--allow-host`
  zusätzlich `Secure`; gespeichert ist nur ein Hash des Sitzungsschlüssels. Die übrigen Cookies der Oberfläche
  (`baruch_*`, nur Einstellungen) setzt Baruch ohne `Secure`.
- Die Anmelde- und die Einrichtungsseite folgen dem Cookie `baruch_theme` (hell/dunkel), ohne JavaScript.
- Die Bremse antwortet mit 429 und `Retry-After`; sie zählt je Konto und – wenn die Anfrage nicht über den
  Proxy kommt – je Adresse.
- Passwörter: Argon2id-Prüfwert in der Tabelle `account`.
- `/einrichten` antwortet über den Namen des Proxys oder mit einer Proxy-Kopfzeile mit 403; gibt es schon
  Konten, mit 303.
- Die Anmeldung gilt für Seiten, Formulare und `/rpc`; die einzige Stelle der Rechteprüfung für Lektoren ist
  `roles` (`LEKTOR_ROUTES`, `LEKTOR_RPC`).

### TLS zu pgvector

- Der Befund auf der Webseite stammt aus dem Quelltext von sqlx 0.8.6 (`sqlx-postgres/src/connection/tls.rs`,
  `sqlx-core/src/net/tls/tls_rustls.rs`), **gelesen, nicht gegen einen Server geprüft**. Seit Runde 36 läuft
  Baruch mit sqlx 0.9.1; die Stelle in `sqlx-postgres-0.9.1/src/connection/tls.rs` wurde am 2026-11-27
  nachgelesen und verhält sich gleich (`accept_invalid_certs` gilt für alles außer `verify-ca` und
  `verify-full`, `accept_invalid_hostnames` für alles außer `verify-full`).
- Das Backlog führte die Vermutung, mit `PGSSLROOTCERT` prüfe sqlx auch bei `require` (so verhält sich libpq:
  liegt eine Wurzeldatei vor, prüft es bei `require` wie bei `verify-ca`). Nach dem Quelltext trifft das für sqlx
  nicht zu.
- `PGPASSWORD` und `~/.pgpass` liest Baruch nicht; das Passwort kommt nur aus dem Schlüsselbund.
