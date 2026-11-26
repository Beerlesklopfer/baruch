# Runde 35 – konten (Baustein für RPC.md, Architektur.md, Bedienung/Betrieb)

Konten, Passwort-Hash, Sitzungen, Anmelde-Schicht (Vertrag §1 und die auth-Teile von S1). Was eine Rolle darf, steht
in `rollen.md`; Seiten und Menü in `oberflaeche_konten.md`.

## Grundsatz

- **Ohne Konten** (Tabelle `account` leer) bleibt Baruch offen wie vor Runde 35: `Who::Open`, kein Cookie, keine
  Umleitung, keine zusätzliche Kopfzeile.
- **Mit Konten** verlangen Seiten, Formulare und `/rpc` eine Sitzung – auch am eigenen Rechner (Vorgabe). `/mcp`
  bleibt bei Tokens; die Schicht lässt `/mcp` unberührt.
- Das erste Konto ist der Autor und entsteht nur am eigenen Rechner (`POST /einrichten`). Jedes weitere Konto ist
  ein Lektor. Konten werden nie gelöscht (das Handle `@lektorat:<slug>` bleibt vergeben), nur gesperrt.

## Aufbau (für Architektur.md)

| Datei | Inhalt |
|---|---|
| `baruch-store/src/accounts.rs` | Tabellen `account`, `account_session`, `login_attempt` (Migration 0053) und der Schalter `store_meta` `auth.local_open`. Der Store hasht nie und sieht nie ein Klartext-Passwort; `Account` trägt keinen Hash. |
| `baruch-server/src/auth/mod.rs` | `Who`, `Identity`, `current()`/`scope()` (task-lokal), `AuthState` (gemerktes „es gibt Konten“), `accounts_exist`, `identity_of`, `check_name`, `message` |
| `auth/gate.rs` | Schicht `gate` (im Router direkt außerhalb von `consent::inject`), `resolve`, `own_machine`, `peer` |
| `auth/password.rs` | Argon2id: `hash`, `hash_with`, `verify`, `policy`, `hash_blocking`, `verify_blocking`, `DUMMY_PHC` |
| `auth/session.rs` | Token, Cookie, `current_hash()` (Sitzung der laufenden Anfrage, task-lokal) |
| `auth/throttle.rs` | Sperre gegen Durchprobieren: `wait_seconds`, Bereiche, `blocked`/`failed`/`succeeded` |
| `auth/routes.rs` | Formulare (hinter `guard::same_origin`) und die gemeinsamen Bausteine `create_account`, `change_password`, `check_current`, `set_active` |
| `auth/rpc.rs` | `account.*` |
| `auth/cli.rs`, `main.rs` | Notweg `baruch-server passwort <name>` |
| `guard.rs` | `forwarded(headers)`, `FORWARDED_HEADERS` |

**Ablauf einer Anfrage** (`auth::gate`): (1) `/mcp` läuft als `Who::Open` durch. (2) `Who` aus den Extensions der
Anfrage (nur im Prozess setzbar: Tests), sonst `resolve`. (3) `Who::Anonymous`: `PUBLIC_PATHS` und `/static/js/…`
laufen durch; GET/HEAD → 303 auf `/anmelden?next=<Pfad+Query, prozentkodiert>`; `POST /rpc` → 401 mit JSON-RPC-Fehler
`-32002`; sonst 401 Text „Anmeldung erforderlich.“ (4) `roles::check_request`. (5) Die Anfrage läuft in
`auth::scope(who, …)` und `session::scope(<Token-Hash>, …)`.

**`resolve`:** keine Konten → `Open`. Sonst: gültige Sitzung eines aktiven Kontos → `Account`; sonst Schalter
„Am eigenen Rechner ohne Anmeldung“ an **und** `gate::own_machine` → `LocalAuthor`; sonst `Anonymous`. Ein
Datenbankfehler führt nie zu mehr Rechten (er steht auf stderr, die Anfrage gilt als nicht angemeldet; ist schon
die Zahl der Konten nicht lesbar, gilt „es gibt Konten“).

**Eigener Rechner** (`gate::own_machine`, Entscheidung 0.2): `shutdown::own_machine` (Gegenstelle bekannt und
Loopback, kein Proxy-Name, `Host` eine IP-Adresse oder `localhost`) **und** keine der Kopfzeilen `Forwarded`,
`X-Forwarded-For`, `X-Forwarded-Host`, `X-Forwarded-Proto`, `X-Real-IP`, `Via` (`guard::forwarded`; schon das
Vorhandensein zählt, der Inhalt wird nicht gelesen). Ohne `ConnectInfo`: nein. Sind Proxy-Namen erlaubt
(`--allow-host`), gilt auch eine Anfrage ohne `Host` nicht als eigener Rechner.

**Antworten:** Mit Konten bekommt jede Antwort (angemeldet oder abgewiesen) `Cache-Control: no-store`, wenn sie
selbst keine Angabe trägt (`/static/…` behält `no-cache`). Ohne Konten hängt die Schicht nichts an.

**Regel:** `auth::current()` und `auth::session::current_hash()` nie in abgespaltenen Aufgaben (`tokio::spawn`,
`spawn_blocking`) auswerten – vorher holen und mitgeben.

## Passwort

- Argon2id, **m = 19456 KiB, t = 2, p = 1**, Salz 16 Byte (getrandom), Ausgabe 32 Byte, gespeichert als PHC-Text
  (`$argon2id$v=19$m=19456,t=2,p=1$<Salz>$<Hash>`), Crate `argon2` 0.5. Hashen und Prüfen laufen in
  `spawn_blocking`.
- `verify` liest die Parameter aus dem PHC-Text: Wächst die Vorgabe später, bleiben alte Hashes prüfbar. Nur
  Argon2id gilt; ein unlesbarer Text passt zu keinem Passwort.
- Regel: 10 bis 200 Zeichen (Unicode-Zeichen, nicht Bytes), sonst keine.
- Unbekannter Name: Prüfung gegen den festen Schein-Hash `password::DUMMY_PHC` (gleiche Parameter, gleiche Dauer),
  gleiche Meldung „Name oder Passwort stimmt nicht.“ – auch für ein gesperrtes Konto und ein überlanges Passwort.
- Kein Klartext in Datenbank (CHECK `password LIKE '$argon2id$%'`), Logs, Fehlermeldungen oder Antworten; kein Typ
  mit Passwort leitet `Debug` ab; Meldungen zu falschen RPC-Parametern zitieren keinen Wert.

## Sitzung

- Token: 32 Zufallsbytes (getrandom) als Base64url (43 Zeichen). In der Datenbank steht nur der SHA-256 (hex,
  64 Zeichen, CHECK der Spalte). Wer nur die Datenbank kennt, kann sich damit nicht anmelden.
- Cookie: `baruch_session=<token>; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000`, zusätzlich `; Secure`, wenn
  `guard::via_proxy` wahr ist (Proxy-Name im `Host`, oder Proxy-Namen erlaubt und kein `Host`).
- Gültig 30 Tage ohne Benutzung, höchstens 90 Tage seit der Anmeldung (beides prüft der Server in SQL);
  `last_seen` wird höchstens einmal je Minute geschrieben. Abgelaufene Sitzungen aller Konten räumt die nächste
  Anmeldung weg.
- **Erneuern (Ergänzung zum Vertrag):** Liegt die vorige Benutzung länger als eine Stunde zurück, setzt die Antwort
  dasselbe Cookie erneut (frische 30 Tage im Browser) – sonst endete die Sitzung im Browser 30 Tage nach der
  Anmeldung, obwohl sie benutzt wird. Die Obergrenze von 90 Tagen bleibt.
- Abmelden löscht die Sitzung und das Cookie (`Max-Age=0`). Passwortwechsel löscht alle **anderen** Sitzungen des
  Kontos (die ändernde bleibt; Zurücksetzen durch den Autor oder die Befehlszeile löscht alle). Sperren
  (`active = 0`) löscht alle. Beides wirkt bei der nächsten Anfrage.

## Sperre gegen Durchprobieren (`login_attempt`)

- Bereiche: `konto:<Namensschlüssel>` (immer, auch für unbekannte Namen; Namensschlüssel = ohne Rand, Leerraum
  einfach, klein, höchstens 40 Zeichen) und `adresse:<IP>` (nur mit bekannter Gegenstelle und nicht über den Proxy).
- Bis zum dritten Fehlversuch keine Wartezeit; ab dem vierten 2^(n−3) Sekunden (2, 4, 8 …), höchstens 900.
- Der Fehlversuch, der die Sperre auslöst, und jeder Versuch während der Sperre antworten **429** mit
  `Retry-After: <Sekunden>`; während der Sperre wird kein Passwort geprüft und nichts gezählt.
- Erfolg setzt die Bereiche des Versuchs zurück. Einträge, die älter als 24 Stunden sind, fallen beim nächsten
  Fehlversuch weg.
- Höchstens 1000 Zeilen: Ist die Tabelle voll (999 Bereiche), bekommt ein neuer Bereich keine Zeile; stattdessen
  wartet der Bereich `alle` fest 2 Sekunden – in dieser Zeit wartet jeder Anmeldeversuch.
- Auch das bisherige Passwort beim Ändern (`/konto/passwort`, `account.set_password`) zählt in `konto:<…>`: Wer
  eine Sitzung in der Hand hat, kann das Passwort nicht beliebig oft raten.

## Routen (Formulare, alle hinter `guard::same_origin`; kein RPC)

| Route | Felder | Antwort |
|---|---|---|
| `POST /anmelden` | `name`, `password`, `next` | 303 auf `konten_ui::safe_next(next)` mit Cookie; sonst Anmeldeseite mit 401, bei Sperre 429 mit `Retry-After`. Ohne Konten 303 `/` ohne Cookie. |
| `POST /abmelden` | – | 303 `/anmelden?ok=abgemeldet`, Cookie gelöscht. Ohne Konten 303 `/`. |
| `POST /einrichten` | `name`, `display`, `password`, `password2` | Nur ohne Konten und vom eigenen Rechner: Autor-Konto, Sitzung, 303 `/einstellungen/konten?ok=konto_angelegt`. Konto vorhanden oder nicht eigener Rechner → Seite mit 403; ungültige Eingabe → 422. |
| `POST /konto/passwort` | `current`, `password`, `password2` | 303 `/konto?ok=passwort_geaendert` bzw. `?fehler=passwort_falsch\|passwort_kurz\|passwort_lang\|passwort_ungleich\|unbekanntes_konto` (ohne eigenes Konto) |
| `POST /einstellungen/konten/act` | `action`, `id`, … | 303 `/einstellungen/konten?ok=…\|fehler=…`; nur volle Rechte (sonst 403 mit `x-baruch-abgelehnt: rolle`); unbekannte Aktion 422 |

Aktionen von `/einstellungen/konten/act`:

| `action` | Felder | ok | Fehler |
|---|---|---|---|
| `create` | `name`, `display`, `password`, `password2` – immer `kind = lektor` | `konto_angelegt` | `name_ungueltig`, `name_vergeben`, `passwort_kurz`, `passwort_lang`, `passwort_ungleich`; ohne jedes Konto 303 `/einrichten` |
| `deactivate` | `id` | `konto_gesperrt` | `letzter_autor`, `unbekanntes_konto` |
| `activate` | `id` | `konto_entsperrt` | `unbekanntes_konto` |
| `reset_password` | `id`, `password`, `password2` | `passwort_geaendert` | wie `create` (Passwort), `unbekanntes_konto` |
| `local_open` | `open` = `0` \| `1` | `lokal_offen` / `lokal_anmelden` | – |

Namen: 1 bis 40 Zeichen ohne Steuerzeichen; der Slug (Regel wie `docx_import::word_author`) darf nicht `unbekannt`
sein – ein Name braucht also mindestens einen Buchstaben a–z (auch ä, ö, ü, ß) oder eine Ziffer. Name
(ohne Rücksicht auf Groß-/Kleinschreibung und Leerraum) und Slug sind eindeutig, auch gegenüber gesperrten Konten.
Anzeigename höchstens 80 Zeichen.

## RPC `account.*` (für RPC.md)

Anmelden, Abmelden und Einrichten gibt es nur als Formular (Cookie). Passwörter, Hashes und Tokens stehen nie in
einer Antwort. Fehler: `-32602` falsche Parameter (die Meldung zitiert keinen Wert), `-32000` fachlich abgelehnt,
`-32002` nicht angemeldet (HTTP 401, schon in der Schicht), `-32003` nur der Autor.

| Methode | Parameter | Antwort |
|---|---|---|
| `account.me` | `{}` | `{accounts_exist, signed_in, local_open, id?, name?, display?, kind?, handle?}` – die Felder des Kontos nur mit Sitzung (`kind`: `author` \| `lektor`). Jeder, auch ohne Konten. |
| `account.list` | `{}` | `{accounts: [{id, name, display, kind, handle, active, created, last_login}]}` nach id. Nur volle Rechte. |
| `account.create` | `{name, display?, password}` | das Konto (wie in `account.list`), immer `kind: "lektor"`. Nur volle Rechte. Ohne jedes Konto `-32000` mit Hinweis auf `/einrichten`; Name ungültig oder vergeben, Passwort zu kurz oder zu lang → `-32000`. |
| `account.set_active` | `{id, active}` | das Konto. Nur volle Rechte. Letztes aktives Autor-Konto oder unbekanntes Konto → `-32000`. Sperren beendet alle Sitzungen des Kontos. |
| `account.set_local_open` | `{open}` | `{local_open}`. Nur volle Rechte. |
| `account.set_password` | `{current?, password, id?}` | `{id, changed: true}`. Eigenes Konto (ohne `id` oder mit der eigenen): `current` ist Pflicht (`-32602` ohne, `-32000` wenn falsch); die Sitzung der Anfrage bleibt, alle anderen enden. Fremdes Konto (`id`): nur volle Rechte, alle Sitzungen des Kontos enden. |
| `account.logout_all` | `{}` | `{sessions}` – beendet alle Sitzungen des eigenen Kontos, auch die der Anfrage. Ohne Sitzung `-32000`. |

`roles::check_rpc` entscheidet vor dem Dispatch, was ein Lektor aufrufen darf (`account.me`,
`account.set_password`, `account.logout_all`); die Methoden „nur volle Rechte“ prüfen `who.full_rights()` hier ein
zweites Mal.

## Befehlszeile

`baruch-server passwort <name> [--data <Ordner>]` – Notweg bei vergessenem Passwort. Liest das neue Passwort als
**eine Zeile von der Standardeingabe** (nie von der Befehlszeile: dort stünde es in der Prozessliste), setzt es für
das vorhandene Konto und beendet alle Sitzungen des Kontos. Exit 0 mit „Passwort für „…“ gesetzt …“; unbekanntes
Konto (nennt die vorhandenen), ungültige Länge, Ordner ohne Datenbank oder noch kein Konto → Exit 1 mit Grund auf
stderr; ohne Namen Exit 2. Legt kein Konto an und entsperrt keines. Läuft auch neben dem laufenden Server.

`serve` gibt beim Start eine Zeile „Anmeldung: aus (keine Konten …)“ bzw. „Anmeldung: an (n Konten)“ aus.

## Store (nur von `auth` gerufen)

`account_count`, `account_list`, `account_get`, `account_create` (Lektor nur, wenn es ein Autor-Konto gibt),
`account_create_first` (Prüfung „Tabelle leer“ und Einfügen in einer Anweisung: von zwei gleichzeitigen Aufrufen
gewinnt genau einer), `account_for_login`, `account_password`, `account_set_password(id, phc, keep)`,
`account_set_active` (letzter aktiver Autor bleibt – Prüfung und Änderung in einer Anweisung), `session_create`,
`session_account`, `session_delete`, `session_delete_all`, `login_wait`, `login_failure`, `login_reset`,
`auth_local_open`, `set_auth_local_open`; Hilfen `Store::account_name_key`, `Store::session_token_hash`.
`lib.rs` reicht aus dem Modul nur `Account` und `NewAccount` heraus; Meldungen und Grenzen hängen darum als
zugehörige Konstanten an `Store` (`ACCOUNT_TAKEN`, `ACCOUNT_NOT_FIRST`, `ACCOUNT_NEEDS_AUTHOR`,
`ACCOUNT_LAST_AUTHOR`, `LOGIN_ATTEMPT_ROWS`, `LOGIN_FULL_WAIT`).

## Annahmen und Abweichungen (ehrlich)

1. **„Es gibt Konten“ wird nur als Ja gemerkt** (Vertrag §1.4 sagt: Ergebnis zwischenspeichern und nach dem Anlegen
   ungültig machen). Konten werden nie gelöscht, ein Ja bleibt wahr; das Nein wird je Anfrage nachgesehen (ein
   `SELECT count(*)` auf der leeren Tabelle). Grund: Ein gemerktes Nein ließe Anfragen ohne Anmeldung durch, wenn
   ein Konto an der Ablage vorbei entsteht (Tests der Kollegen legen Konten per SQL an; zweiter Prozess).
2. **Cookie erneuern** nach mehr als einer Stunde Pause (siehe Sitzung) – im Vertrag nicht genannt.
3. **Der vierte Fehlversuch antwortet selbst schon 429** mit `Retry-After: 2` (Abnahme: „ab dem vierten Fehlversuch
   429“), nicht erst der fünfte Versuch.
4. **`alle`** sperrt, solange es wartet, jeden Anmeldeversuch (auch bekannte Bereiche) für höchstens 2 Sekunden.
5. **Bisheriges Passwort beim Ändern** zählt in der Sperre des Kontos mit; während der Sperre lautet der Code
   `passwort_falsch` (die Tabelle der Meldungen hat keinen eigenen Code für „gesperrt“).
6. **`POST /einrichten` mit vorhandenem Konto** zeigt die Seite mit 403 und dem Satz „Es gibt schon ein Konto.“
   (kein Code der Tabelle §3.4); für Unangemeldete endet die Anfrage schon in der Schicht mit 401.
7. **`/einstellungen/konten/act` `create` ohne jedes Konto** leitet auf `/einrichten` (303): Ein Lektor ohne Autor
   wäre ein Zustand, in dem niemand volle Rechte hat; der Store lehnt ihn ebenfalls ab.
8. **Sperren des eigenen Kontos** ist einem Autor erlaubt, solange ein anderer aktiver Autor bleibt (in dieser Runde
   gibt es über die Oberfläche nur einen Autor – praktisch greift immer `letzter_autor`).
9. **`resolve` nimmt `&http::request::Parts`** statt `&Request` (schon in S1 so festgelegt und begründet).
10. Lektoren erreichen `/abmelden`, `/konto/passwort` und `/rpc` erst, wenn „rollen“ `LEKTOR_ROUTES` gefüllt hat;
    bis dahin lehnt `roles` (S1-Vorgabe) jede Route eines Lektors ab. Die Tests hier prüfen Lektor-Sitzungen darum
    über `auth::gate::resolve` und `auth::rpc::call` in `auth::scope`.
11. Anzeige beim Start und Hilfetext zu `--allow-host` sagen jetzt „ohne Konten hat Baruch keine Anmeldung“.
    Die Sätze „Baruch hat keine Anmeldung“ in `README.md`, `packaging/systemd/README.md`, `docs/Betrieb.md`,
    `docs/Bedienung.md`, `docs/Architektur.md`, `site/content/betrieb.md` gehören anderen Besitzern und sind
    nachzuführen.

## Tests

- `crates/baruch-store/tests/migration_0053.rs`: 0053 auf nachgebauter Datenbank im Stand 0052 (alle Zeilen
  byte-gleich, Schema unverändert, drei leere Tabellen, CHECKs, Datei = S1 per SHA-256); Store: erstes Konto nur
  einmal (auch gleichzeitig), Eindeutigkeit, letzter Autor, Sitzungen (nur Hash, Fristen, Passwortwechsel, Sperren),
  Sperre (Wartezeiten, 24 Stunden, 1000 Zeilen), Schalter.
- `crates/baruch-server/tests/round35_konten.rs`: ohne Konten wie bisher; Einrichten (eigener Rechner, sechs
  Kopfzeilen, Proxy-Name, gleichzeitig); jede Route aus `common::registered_routes()` ohne Sitzung (303 bzw. 401,
  `/rpc` −32002); `/mcp` unverändert; Anmelden (Cookie, Secure, `next`, gleiche Antwort für falsch und unbekannt);
  Sperre; Sitzung (Hash, Abmelden, Fristen, Erneuern, Passwortwechsel, Sperren); Ausnahme eigener Rechner;
  Verwaltung über Formular und RPC; kein Geheimnis in Antworten, Debug-Ausgaben und der Ausgabe des echten
  Programms; Befehl `passwort`.
- Modultests in `auth/*.rs` und `guard.rs`.

## Handprüfung (Integrator bzw. Nutzer)

Wegwerf-Server nach dem Umweg in `SKILLS.md` (`--listen 127.0.0.1:0 --data <Temp-Ordner>`, Verbindungen vorher auf
eine tote Adresse):

1. Start: Zeile „Anmeldung: aus“. `/` öffnet ohne Anmeldung, kein Cookie `baruch_session`.
2. `/einrichten` im Browser dieses Rechners: Konto anlegen → angemeldet, Cookie `HttpOnly`, `SameSite=Strict`.
3. Privates Fenster: `/` leitet auf `/anmelden?next=/`; falsches Passwort viermal → Wartezeit; nach Ablauf anmelden.
4. `curl -i -X POST <url>/rpc -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"work.list"}'`
   → 401 mit Code −32002. `curl -i <url>/mcp …` verhält sich wie vor Runde 35.
5. Einstellungen → Konten: Lektor anlegen, sperren (dessen Fenster fällt bei der nächsten Anfrage auf die
   Anmeldung zurück), entsperren; Schalter „Am eigenen Rechner ohne Anmeldung“ ein → privates Fenster öffnet `/`
   ohne Anmeldung; `curl -H 'X-Forwarded-For: 1.2.3.4' <url>/` → 303 auf die Anmeldung.
6. Hinter dem Proxy (`--allow-host`): Anmelden setzt das Cookie mit `Secure`; die Ausnahme „eigener Rechner“ gilt
   dort nie.
7. `baruch-server passwort <name> --data <Ordner>` mit dem neuen Passwort auf stdin → altes Passwort und alte
   Fenster gelten nicht mehr.
8. `sqlite3 'file:<Ordner>/baruch.sqlite?mode=ro' 'select password from account; select token_hash from
   account_session'` → nur `$argon2id$v=19$m=19456,t=2,p=1$…` und 64 Hex-Zeichen.
