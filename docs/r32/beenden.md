# Runde 32, Aufgabe beenden – Bausteine für den Integrator

Diese Datei wird im Abschluss in die genannten Dokumente eingesetzt und danach gelöscht. Schon am Ort stehen:
`docs/Verteilung.md` (Abschnitt „Baruch beenden“, Starter je Plattform, Pages in zwei Jobs),
`site/content/installation.md`, `site/content/bedienung.md`, `packaging/systemd/README.md` bleibt unverändert
(die Units erklären den Neustart selbst).

## Bedienung

### Baruch beenden

**Baruch ▾ → „Baruch beenden…“** öffnet die Rückfrage `/beenden`. Der Knopf „Baruch beenden“ schickt ein
gewöhnliches Formular (`POST /beenden`), „Abbrechen“ führt zurück zum zuletzt geöffneten Werk. Alles geht ohne
JavaScript.

- Nach dem Knopf zeigt das Fenster „Baruch ist beendet – dieses Fenster kann geschlossen werden“. Mit JavaScript
  versucht die Seite, das Fenster zu schließen (`window.close()`). Browser lassen das nur bei Fenstern zu, die
  ein Skript oder der Starter als App-Fenster geöffnet hat; sonst – in Firefox immer – bleibt der Satz stehen.
- Die Starter (`baruch`, AppImage, macOS-Programm, Windows-Startmenü) öffnen Baruch deshalb, wenn ein
  Chromium-artiger Browser da ist, als eigenes Fenster ohne Adressleiste (`--app`). `BARUCH_BROWSER=standard`
  nimmt immer den Standardbrowser, `BARUCH_NO_BROWSER=1` öffnet keinen (Einzelheiten und Stand der Prüfung je
  Plattform: `docs/Verteilung.md`, „Baruch beenden“).
- **Nur am eigenen Rechner:** Von einem anderen Rechner oder unter dem Namen eines Reverse-Proxys
  (`--allow-host`) zeigt `/beenden` mit Status 403 eine Erklärung und kein Formular; auch das Formular selbst
  wird dann abgelehnt. Über RPC und MCP lässt sich Baruch nicht beenden.
- Gespeicherte Texte stehen in der Datenbank und im Git-Spiegel; beim Beenden geht nichts verloren. Laufende
  Antworten gehen noch zu Ende (höchstens 10 Sekunden), ein offener Live-Chat wird danach getrennt.
- Als Dienst (`systemd --user`, `Restart=on-failure`) bleibt Baruch nach dem Beenden aus, bis
  `systemctl --user start baruch.service` es wieder startet.
- Im Terminal beendet Strg+C; `systemctl --user stop`, `kill` (SIGTERM) und unter Windows Strg+Pause bzw. das
  Schließen des Konsolenfensters fahren genauso sauber herunter. Die letzte Zeile der Ausgabe heißt
  „Baruch beendet.“, der Status ist 0.

Routen (für die Tabelle der Seiten):

| Route | Zweck |
|---|---|
| `GET /beenden` | Rückfrage mit Formular; nicht vom eigenen Rechner 403 mit Erklärung |
| `POST /beenden` | hinter `guard::same_origin`; nur vom eigenen Rechner, sonst 403; Antwort 200 mit der Abschlussseite, 500 ms später fährt Baruch herunter |

## RPC

Keine Methode. Beenden ist bewusst weder RPC-Methode noch MCP-Werkzeug (`rpc::call` kennt die Gegenstelle nicht;
Test `there_is_no_rpc_method_and_no_mcp_tool_to_quit`). Vorschlag für einen Satz in `docs/RPC.md` und
`docs/MCP.md`:

> Baruch beenden gibt es weder über RPC noch über MCP: Das geht nur über die Seite `/beenden` am eigenen Rechner
> oder über ein Signal an den Prozess.

## Architektur

- `crates/baruch-server/src/shutdown.rs` – Beenden (Runde 32).
  - `Shutdown` (in `AppState::shutdown`): Auslöser auf einem `watch`-Kanal; einmal verlangt, bleibt verlangt.
  - `own_machine(hosts, peer, headers)`: wahr nur, wenn die Gegenstelle (`ConnectInfo`) bekannt und Loopback ist
    (`to_canonical`, damit `::ffff:127.0.0.1` zählt), `guard::via_proxy` verneint und `Host` fehlt oder
    `guard::allowed_host` erfüllt. `guard.rs` blieb unberührt.
  - `serve` → `serve_until(listener, state, grace, signal)`: `axum::serve(…)` mit
    `into_make_service_with_connect_info::<SocketAddr>()` und `with_graceful_shutdown` auf das erste von Signal
    (`shutdown/signals.rs`) und `Shutdown::requested()`. Danach nimmt der Server keine Verbindung mehr an,
    ruhende Verbindungen schließen, laufende Anfragen werden beantwortet. Eine zweite Zukunft wartet ab dem
    Auslöser die Frist (`GRACE` = 10 s) ab; ist dann noch eine Verbindung offen, wird nicht länger gewartet
    (Meldung auf stderr). Zuletzt `store.pool().close()` (höchstens `CLOSE_WAIT` = 5 s): SQLite schreibt das
    Protokoll (`-wal`) zurück. `serve` schreibt dann „Baruch beendet.“; `main` kehrt zurück, Status 0.
  - `shutdown/signals.rs` (nur `tokio`, einzeln übersetzbar): Strg+C überall; Unix `SIGTERM`; Windows
    `ctrl_break`, `ctrl_close`, `ctrl_shutdown` – die letzten beiden mit Frist `WINDOWS_CLOSE_GRACE` = 3 s, weil
    Windows den Prozess kurz nach dem Ereignis selbst beendet. Kein `SIGHUP` (ein Empfänger höbe `nohup` auf).
  - Seiten: eigene Template-Umgebung (`base.html`, `shutdown.html`, `shutdown_done.html`). Die Abschlussseite
    ist in sich geschlossen (Stil inline, einziges Skript `window.close()`), weil der Server danach nichts mehr
    liefert. `POST /beenden` antwortet zuerst und löst `REPLY_DELAY` = 500 ms später aus.
  - Kein Widget, keine `.ts`-Datei: Das Inline-Skript der Abschlussseite genügt.
- Tests: `src/shutdown.rs` (Auslöser, Tabelle zu `own_machine`), `tests/round32_beenden.rs` (Seiten am echten
  Server im Testprozess, echter Prozess mit Knopf/SIGTERM/Strg+C, Starter mit Attrappen in abgeschlossenem
  `PATH`, Units, Workflows).
- CI: `pages.yml` und `release.yml` trennen `pages-build` und `pages-deploy`; Regel 11 in
  `scripts/ci-check-workflows.sh`, Gegenproben in `scripts/ci-selftest.sh`.

Bekannte Grenze: Läuft beim Beenden noch ein blockierender Auftrag (`spawn_blocking`, etwa ein Typst-Satz),
wartet die Tokio-Laufzeit beim Verlassen von `main` auf ihn – das Ende kann sich dann über die Frist hinaus
verzögern. Die Datenbank ist zu diesem Zeitpunkt bereits geschlossen. Nicht mit einem Test belegt.

## CHANGELOG

- **Baruch beenden:** Menü Baruch ▾ → „Baruch beenden…“ mit Rückfrage (`/beenden`, auch ohne JavaScript). Nur am
  eigenen Rechner, nie unter einem Proxy-Namen, nicht über RPC oder MCP. Danach „Baruch ist beendet – dieses
  Fenster kann geschlossen werden“; das Fenster schließt sich, wenn der Browser es zulässt.
- **Sauberes Herunterfahren** auch auf `SIGTERM` (`systemctl stop`, `kill`) und unter Windows auf Strg+Pause,
  das Schließen des Fensters und das Herunterfahren (Windows ungeprüft): laufende Antworten gehen zu Ende
  (höchstens 10 s), die Datenbank wird geschlossen, letzte Zeile „Baruch beendet.“, Status 0. Bisher nur Strg+C.
- **Starter:** öffnen, wenn vorhanden, ein eigenes Fenster eines Chromium-artigen Browsers (`--app`), unter
  Windows Edge; `BARUCH_BROWSER=standard` nimmt den Standardbrowser. Der Starter wartet nach einem Signal auf
  das saubere Ende des Servers.
- **CI:** Webseite wird in zwei Jobs gebaut (`pages-build`) und ausgeliefert (`pages-deploy`).

## README

Vorschlag (Abschnitt Starten/Betrieb):

> Beenden: Menü **Baruch ▾ → „Baruch beenden…“** (nur am eigenen Rechner), Strg+C im Terminal oder
> `systemctl --user stop baruch.service`. Baruch fährt dabei sauber herunter.

## Bitten an den Integrator

0. **Nötig, damit der volle Lauf grün ist:** Der Wächter
   `round33_seiten::every_template_puts_its_actions_as_literal_groups_into_the_block_of_its_tab` verlangt jede
   Vorlage, die `base.html` erweitert, in seiner Liste `TEMPLATES` (`tests/round33_seiten.rs`, gehört dem
   Integrator). Ohne die Zeile scheitert genau dieser eine Test am Zweig `team/beenden`; mit ihr ist der volle
   Lauf grün (so geprüft, die Zeile aber nicht committet). Wörtlich, hinter der Zeile für `about.html`:
   ```rust
       ("shutdown.html", "start", &["href=\"/{% if settings.work %}?work={{ settings.work }}{% endif %}\""]),
   ```
   Dasselbe trifft vermutlich `docx_import.html` und `docx_export.html` der beiden anderen Aufgaben.
1. **Kein Widget zu registrieren.** `web/src/shutdown.ts` gibt es nicht; `main.ts`/`widgets.ts` bleiben, wie sie
   sind.
2. **`Store::close` ist nicht nötig:** `serve_until` schließt über das vorhandene `Store::pool()`
   (`state.store.pool().close().await`). Wer die Absicht im Store sichtbar haben will, kann ergänzen und in
   `shutdown.rs` (`close_database`) einsetzen:
   ```rust
   /// Schließt die Datenbank (wartet auf zurückgegebene Verbindungen); danach schlägt jede Abfrage fehl.
   pub async fn close(&self) {
       self.pool.close().await;
   }
   ```
3. **`docs/MCP.md`, `docs/RPC.md`:** der Satz oben unter „RPC“.
4. **`docs/Bedienung.md`, `docs/Architektur.md`, `CHANGELOG.md`, `README.md`:** die Abschnitte oben.
5. **`docs/Backlog.md` (PM):** „Baruch beenden aus der Oberfläche“ ist umgesetzt. Offen bleiben: echte
   Browserprobe (`window.close()` im App-Fenster), macOS und Windows auf echten Geräten, die Beobachtung „Pages
   je Commit“ (von der Trennung der Jobs nicht berührt).
6. **`main.rs` (optional):** Soll ein laufender blockierender Auftrag das Ende nie über die Frist hinaus
   verzögern, kann `main` nach erfolgreichem `shutdown::serve` mit `std::process::exit(0)` enden (die Datenbank
   ist dann schon geschlossen). Nicht umgesetzt, da `main.rs` nicht mir gehört; siehe „Bekannte Grenze“.
7. **`packaging/systemd/README.md`** gehört zu meinem Bereich und blieb unverändert; die Tabelle „Bedienen“ dort
   stimmt weiter (`stop` fährt jetzt sauber herunter).

## Handprüfung durch den Nutzer

- `baruch` starten (mit Chromium/Chrome installiert): eigenes Fenster ohne Adressleiste? Baruch ▾ → „Baruch
  beenden…“ → schließt sich das Fenster von selbst?
- Dasselbe mit `BARUCH_BROWSER=standard` und Firefox: Der Satz bleibt stehen, Baruch ist beendet.
- Dienst: nach „Baruch beenden“ zeigt `systemctl --user status baruch.service` „inactive (dead)“, kein Neustart.
- macOS und Windows: Start, App-Fenster, Beenden, Schließen des Konsolenfensters (alles ungeprüft).
- Nächster Lauf von `pages.yml` bzw. `release.yml`: Findet `pages-deploy` das Artefakt aus `pages-build`?
