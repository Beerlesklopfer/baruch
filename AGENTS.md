# Baruch – Regeln für Agenten

Freie Schreibwerkstatt (AGPL-3.0) für Bücher und Predigten. Plan: `docs/Plan.md`.

- Oberfläche und Dokumentation Deutsch, Bezeichner Englisch.
- Nie Tokens oder Keys committen, veröffentlichen oder ausgeben; Keys gehören in den Schlüsselbund (`keyring`).
- Versionsnummern legt nur der Nutzer fest.
- Push nur nach ausdrücklichem „ja“ des Nutzers.
- Strom (2026-11-24, zwei Ausfälle des Netzteils unter Volllast): Bauen und Testen immer auf dieselben acht
  Threads gebunden und mit begrenzter Testparallelität, z. B.
  `RUST_TEST_THREADS=4 nice -n 10 ionice -c3 taskset -c 0-7 cargo test --workspace -j 4`. `-j 4` begrenzt nur
  das Bauen, die Tests liefen sonst auf allen 32 Threads. Weil alle Läufe auf dieselben Kerne `0-7` gebunden
  sind, bleibt die Gesamtlast auch bei parallelen Agenten gedeckelt. Nie mehrere volle Testläufe gleichzeitig.
- Rechnerlast: Bauen und Testen immer mit `nice`/`ionice` und `-j 4`. Agenten dürfen parallel arbeiten
  (je eigener Worktree, Zweig `team/<id>`), bauen aber jeweils in ihr **eigenes** `target/` im Worktree –
  kein gemeinsames `CARGO_TARGET_DIR`: Die Workspace-Crates bekommen in allen Worktrees dieselben
  Artefakt-Hashes, cargo hält dann den Bau eines anderen Worktrees für aktuell und testet fremden Code
  (in Runde B1.5 nachgewiesen). Firefox nie parallel zu Testläufen.
- Zweige: gearbeitet und committet wird auf `development`; Fast-Forward nach `main` löst die CI aus und
  macht nur der Nutzer.
- Nicht raten: Belege (Ausgabe, Code, Doku) vor Behauptungen.
- Stubs: Geplante Werkzeuge (MCP, RPC) gibt es sofort als Stub. Ein Stub antwortet ehrlich („noch nicht
  umgesetzt“, mit Hinweis, was fehlt) und trägt im Code den Vermerk `// STUB: <was fehlt> (<Stufe>)`;
  auch die Beschreibung des Werkzeugs nennt es. Offene Stubs: `grep -rn "STUB:" crates`.
- Fahrplan: Der PM pflegt `docs/Fahrplan.md` (Mermaid-Gantt und Tabelle) am Ende jeder Runde – erledigte Runde
  mit Beginn/Ende aus `git log` und Commits eintragen, laufende/geplante Runden nach `docs/Backlog.md` neu
  schätzen, Gantt vor dem Commit auf gültige Syntax prüfen (keine Doppelpunkte in Aufgabennamen).
- Doku wächst mit dem Code (Deutsch, unter `docs/`): Bedienung (`docs/Bedienung.md`), Aufbau
  (`docs/Architektur.md`), Schnittstellen (`docs/RPC.md`, `docs/MCP.md` mit jedem Werkzeug, Stubs markiert).
  Jede Änderung an Oberfläche, RPC oder MCP zieht die Doku im selben Commit nach.
- Revisionen: je Abschnitt genau eine aktuelle Fassung (Markierung, jederzeit umschaltbar); Ändern/Löschen nur Mensch, protokolliert im Spiegel; Assistenten nur Vorschläge.
  Ausnahme Gliederung (2026-11-06): Anlegen von Gliederungspunkten dürfen auch Assistenten (MCP `outline_add`, Herkunft im Spiegel); Umbenennen, Verschieben, Umstufen, Nummer an/aus und Löschen nur nach Bestätigung eines Antrags durch den Menschen.
  Ergänzung (2026-11-07): Ein neuer Teil, unter den vorhandene Kapitel wandern würden, ist kein bloßes Anlegen, sondern ein Antrag, dem der Mensch aktiv zustimmt (Popup, ohne JavaScript Hinweisleiste und Formular); nur ein Teil ohne Folgekapitel bleibt Anlegen.
- Lektor nach Einstellung (Einstellungen → Assistenten): „fest <Persona>“ oder „automatisch nach Kontext“ (aktive Persona mit dem größten Kontext; eine inaktive feste Persona fällt auf diese Regel zurück). Versuch seit 2026-11-06: fest @lektor (alle Personas auf einem Modell).
- Oberfläche: `baruch-server` (axum + minijinja) liefert Seiten und Fragmente aus, der Kern ist über
  JSON-RPC (`POST /rpc`) erreichbar; Widgets in TypeScript (`crates/baruch-server/web/src`,
  `data-widget`), übersetzt von `build.rs`. Alles muss auch ohne JavaScript bedienbar bleiben. Kein Iced.
