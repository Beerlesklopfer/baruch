<!-- translation-of: docs/Betrieb.md source-commit: 8e9823b -->

# Betrieb {#operation}

> Dies ist die deutsche Übersetzung von `docs/Betrieb.md` (englisches Original, maßgeblich), Stand: Commit `8e9823b`.

**Die Beschreibung für Betreiber steht auf der Website:** <https://baruch.bernau.family/betrieb/> (auf Deutsch) – die
Quelle ist `site/content/de/betrieb.md` (seit Runde 37). Sie behandelt den Datenordner, Sicherung und Wiederherstellung, den
Dauerbetrieb und das Aktualisieren, die drei Stufen der Erreichbarkeit, den Betrieb hinter einem Reverse-Proxy mit den Beispielen
für nginx und Caddy und der verbindlichen Probe, Anmeldung und Konten, `/mcp` mit Token, ausgehende Verbindungen und
TLS zu pgvector.

**Regel für Entwickler:** „Die Betriebsdoku aktualisieren“ heißt, `site/content/de/betrieb.md` im selben
Commit zu ändern. Diese Datei hier enthält **keine Schritte für Betreiber**, nur das, was dort niemand braucht: den Stand der
Prüfung mit den Tests, offene Punkte und technische Anmerkungen. Nichts steht in beiden Dateien.

Installation und Pakete: `docs/Verteilung.md`. Bedienung: die Hilfe unter `site/content/de/hilfe/` (Einstiegspunkt
`docs/Bedienung.md`). Aufbau der Schutzschichten: `docs/Architektur.md`, Abschnitt „Protection of the interfaces“.

## Was steht wo {#where-is-what}

| Thema | auf der Website |
|---|---|
| Das Wichtigste zuerst | `/betrieb/#das-wichtigste` |
| Datenordner, Rechte, Schlüsselbund | `/betrieb/#datenordner` |
| Sicherung, Wiederherstellung, ein Werk mitnehmen | `/betrieb/#sicherung`, `#wiederherstellen`, `#werk-mitnehmen` |
| Dauerbetrieb, Aktualisieren, Schlüsselbund ohne Desktop | `/betrieb/#dauerbetrieb`, `#schluesselbund` |
| Erreichbarkeit in drei Stufen, abgelehnte Rechnernamen | `/betrieb/#erreichbarkeit`, `#rechnernamen` |
| Hinter einem Reverse-Proxy: Namen erlauben, Regeln, nginx, Caddy, Probe | `/betrieb/#reverse-proxy`, `#namen-erlauben`, `#regeln`, `#nginx`, `#caddy`, `#probe` |
| Anmeldung und Konten, vergessenes Passwort, Probe mit Konten | `/betrieb/#konten`, `#passwort-vergessen`, `#probe-konten` |
| `/mcp` mit Token | `/betrieb/#mcp-token` |
| Ausgehende Verbindungen, TLS zu pgvector | `/betrieb/#verbindungen`, `#pgvector-tls` |
| Was hinter einem Proxy anders ist | `/betrieb/#proxy-unterschiede` |
| Was nicht erprobt wurde (für Betreiber) | `/betrieb/#nicht-erprobt` |

## Stand der Prüfung {#state-of-verification}

Geprüft (Tests `crates/baruch-server/tests/round24_betrieb.rs`, Unit-Tests in `guard.rs`, ein eigener Server
mit `curl`):

- Ohne `--allow-host` bleibt ein Name bei 421; mit einem erlaubten Namen kommen `Host: name` und `Host: name:8443` durch,
  ein anderer Name, ein Name mit Leerzeichen oder ein Platzhalter nicht.
- Ein POST mit `Origin: https://name` und `Host: name` besteht die Prüfung; mit umgeschriebenem `Host` 403.
- `/mcp` unter dem erlaubten Namen von Loopback aus: 401 ohne Token, 200 mit Token; unter `127.0.0.1` wie
  bisher ohne Token.
- Ungültige Namen beenden `serve` mit Exit 2 und einer Meldung, sowohl als Argument als auch als Umgebungsvariable.
- Die Seite für den MCP-Zugang zeigt Adresse und Befehl mit einem Token unter dem erlaubten Namen.
- Konten (Runde 35): Anmeldung, Sitzung und Sperren mit Tests gegen den Server.

Nicht geprüft, also eine Annahme (die Liste für Betreiber steht unter `/betrieb/#nicht-erprobt`; hier die
technische Seite davon):

- die Beispiele für nginx und Caddy an einem laufenden Proxy, einschließlich Zertifikat, Basic-Auth, Client-Zertifikat,
  HTTP/2 nach außen und Live-Antworten durch den Proxy;
- alles zu `PGSSLROOTCERT`, `PGSSLCERT`, `PGSSLKEY` gegen einen PostgreSQL-Server mit TLS (Befund nur
  aus dem Quelltext, siehe unten);
- das Wiederherstellen einer Sicherung;
- der Betrieb unter macOS und Windows hinter einem Proxy;
- Konten an einem echten Proxy mit TLS und in einem echten Browser (manuelle Checkliste: `docs/Architektur.md`,
  „Round 35 – Accounts and roles“).

## Offen {#open}

- Baruch kann einen Proxy nicht erkennen, der `Host` umschreibt und keinen Proxy-Header setzt (siehe die
  Tests auf der Website).
- Baruch hat kein eigenes TLS; nur das Sitzungs-Cookie trägt `Secure`.
- Es gibt keine Zwei-Faktor-Anmeldung, kein Zurücksetzen per E-Mail und keine Unterstützung für mehrere Autoren.
- Der TLS-Hinweis der pgvector-Verbindung kennt die Umgebungsvariablen nicht.
- Die Hinweistexte im Programm (`src/main.rs`, Hilfe zu `--allow-host`; `templates/settings/mcp.html`) und
  `docs/MCP.md` nennen noch `docs/Betrieb.md`; diese Datei führt von dort zur Website. Ob sie künftig
  die Adresse `/betrieb/` direkt nennen, entscheidet der Nutzer.

## Technische Anmerkungen {#technical-notes}

Was die Website in Alltagssprache sagt, hier mit den Namen aus dem Programm.

### Datenordner und Sicherung {#data-folder-and-backup}

- Die Sicherung schreibt die Datenbank mit `VACUUM INTO` (ein konsistenter Stand trotz laufendem Betrieb); `-wal` und
  `-shm` sind die Begleitdateien von SQLite.
- Die Token für `/mcp` werden als SHA-256-Hashes in der Datenbank gespeichert.
- Geheimnisse: Schlüsselbund-Dienst `baruch`; Schalter `--secrets keyring|memory` oder `BARUCH_SECRETS`.
- Neue Datenordner werden mit 0700 angelegt; `baruch.sqlite*` und `works/` werden mit der umask des Dienstes angelegt.
- „Sicherung angelegt“ ist ein einmaliger Hinweis im laufenden Prozess, kein Wert in der Datenbank.
- Das Beispiel für den Dienst liegt im Repository unter `packaging/systemd/` (die Handhabung des
  Dienstes ist dort ebenfalls beschrieben), im Paket unter `/usr/share/doc/baruch/examples/baruch.service`.
- Die Seite „Über Baruch“ ist `/ueber`.

### Rechnernamen und Proxy {#host-names-and-proxy}

- Die Prüfung schreibender Anfragen ist `guard::check`: `Origin` gegen `Host`, dazu `Sec-Fetch-Site`; lesende
  Anfragen von fremden Seiten als Unterressource werden abgelehnt, Einbetten in Frames ist verboten.
- `--allow-host` gilt nur für den Befehl `serve`. Namen wie `0x7f.1` (letzter Teil eine Zahl in anderer
  Schreibweise) und `baruch.example.org.evil.example` (nur der genaue Name zählt) werden ebenfalls abgelehnt.
- Eine Anfrage ohne `Host`-Header gilt nicht als lokal, sobald Namen erlaubt sind.
- Baruch wertet `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host` und `Forwarded` weder für die
  Gegenstelle noch für das Schema aus; das auf der Seite für den MCP-Zugang gezeigte Schema `https` ist eine Annahme.
- Von einer Gegenstelle außerhalb von Loopback lässt Baruch nur `/mcp` durch (403 für alles andere).

### Konten {#accounts}

- Entscheidungen der Runde 35: 0.2 (mit Konten gilt die Anmeldung auch auf Loopback; Ausnahme nur als Schalter
  der Autorenschaft) und 0.3 (der direkte Zugriff von anderen Rechnern bleibt gesperrt).
- Sitzungs-Cookie `baruch_session`: `HttpOnly`, `SameSite=Strict`, über einen Namen aus `--allow-host`
  zusätzlich `Secure`; gespeichert wird nur ein Hash des Sitzungsschlüssels. Die übrigen Cookies der Benutzeroberfläche
  (`baruch_*`, nur Einstellungen) setzt Baruch ohne `Secure`.
- Die Anmeldeseite und die Einrichtungsseite folgen dem Cookie `baruch_theme` (hell/dunkel), ohne JavaScript.
- Die Drosselung antwortet mit 429 und `Retry-After`; sie zählt je Konto und – wenn die Anfrage nicht über den Proxy
  kommt – je Adresse.
- Passwörter: Argon2id-Prüfwert in der Tabelle `account`.
- `/einrichten` antwortet über den Namen des Proxys oder mit einem Proxy-Header mit 403; gibt es bereits Konten,
  mit 303.
- Die Anmeldung gilt für Seiten, Formulare und `/rpc`; die einzige Stelle der Rechteprüfung für Lektoren ist
  `roles` (`LEKTOR_ROUTES`, `LEKTOR_RPC`).

### TLS zu pgvector {#tls-to-pgvector}

- Der Befund auf der Website stammt aus dem Quelltext von sqlx 0.8.6 (`sqlx-postgres/src/connection/tls.rs`,
  `sqlx-core/src/net/tls/tls_rustls.rs`), **gelesen, nicht gegen einen Server geprüft**. Seit Runde 36
  läuft Baruch mit sqlx 0.9.1; die Stelle in `sqlx-postgres-0.9.1/src/connection/tls.rs` wurde am
  2026-11-27 erneut gelesen und verhält sich gleich (`accept_invalid_certs` gilt für alles außer `verify-ca` und
  `verify-full`, `accept_invalid_hostnames` für alles außer `verify-full`).
- Das Backlog trug die Annahme, dass sqlx mit `PGSSLROOTCERT` auch unter `require` prüft (so verhält sich
  libpq: Ist eine Root-Datei vorhanden, prüft es unter `require` wie unter `verify-ca`). Laut
  Quelltext gilt das für sqlx nicht.
- Baruch liest `PGPASSWORD` und `~/.pgpass` nicht; das Passwort kommt nur aus dem Schlüsselbund.
