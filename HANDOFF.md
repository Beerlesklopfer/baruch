# Übergabe (Stand 2026-11-27, Runde 37 „Hilfe auf der Webseite“ **integriert** (Hilfe mit 94 Bildern, Betrieb und Ersteinrichtung auf der Webseite; Prüfung und PM-Abschluss stehen aus); Runde 36 „Reste“ **abgeschlossen** (Typst-Kommandozeile, sqlx 0.9.1, Reste; Integration `f2bb112`, PM-Abschluss mit vollem Lauf); Runde 35 „Konten und Rollen“ **abgeschlossen** (Integration `bae1816`, Nachbesserung `40d6591`, PM-Abschluss mit vollem Lauf); davor: Runde 34 „Mindmap wie XMind“ und Runde 32 „Lektorat I – Word und Beenden“ abgeschlossen; lokal zeigen `main`, `origin/main` und der Tag `v0.9.0` auf `35140b5`; nächste Version nach dem Backlog 0.9.1)

Arbeitsordner `~/Dokumente/Projekte/baruch`, Zweig `development`. Austausch über dieses Repo (Issues/Commits).

## Stand
- Oberfläche: Server mit Browser-Oberfläche (Vorbild David, siehe `docs/Plan.md`), alles ohne JavaScript bedienbar.
- **Runde 37 „Hilfe auf der Webseite“ integriert** (2026-11-27, Integrator, auf `development`; Schnittstelle R37-S1
  `4a5b9d2`; Zweige `team/bilder-r37` `c694a70`, `team/webseite` `b0f5266`, `team/hilfe_text` `999d243` mit `--no-ff`,
  **ohne Konflikte**; danach der Integrations-Commit; nicht gepusht; Worktrees und team-Zweige stehen noch). Version
  unverändert 0.9.0, CHANGELOG unter „Unveröffentlicht“. **Keine Migration**, keine neue Route, RPC-Methode oder
  MCP-Werkzeug, kein neuer Stub. Voller Lauf im Hauptordner nach `cargo clean -p` aller Workspace-Crates
  (`RUST_TEST_THREADS=4 … taskset -c 0-7 cargo test --workspace -j 4`): Exit 0, 215 Ergebnisse „ok“, 3546 bestanden,
  0 gescheitert, 2 ignoriert (Server-Bündel 1587; pgvector-Tests liefen mit). Danach geändert und gezielt
  nachgeprüft: Suchfeld der Literatur („None“), Bildliste, alt-Texte, Shortcode (siehe unten).
  - **Die Webseite ist die Quelle der Bedienung:** `site/content/hilfe/` (14 Seiten), `site/content/betrieb.md`,
    Ersteinrichtung in `installation.md` und `mcp.md`. `docs/Bedienung.md` und `docs/Betrieb.md` sind Einstieg für
    Entwickler („Wo steht was“, technische Anmerkungen). Wer Oberfläche ändert, ändert die Hilfeseite im selben
    Commit; `tests/round37_hilfe.rs` setzt die Regeln durch.
  - **Bilder:** 94 Bilder der Hilfe (drei auch dunkel) und die sieben der Seite „Funktionen“ sind neu aufgenommen
    (`site/tools/bildschirmfotos.py --fotos --server target/debug/baruch-server --sword <Beispielbibel> --temp
    /var/tmp/baruch-hilfe`), zusammen 4,2 MB. Beim ersten Durchgang standen der Heimatordner und der Name des
    Nutzers in drei Bildern (Datenordner, Modulordner, „Über Baruch“); die Bilder sind verworfen, das Werkzeug
    bricht bei solchen Pfaden jetzt ab und gibt dem Wegwerf-Server ein eigenes `HOME`. Kein Bild und keine
    PNG-Datei enthält den Namen (`grep -a`). Die Bilder zur Bibel zeigen eine erzeugte „Beispielbibel“ mit einem
    Vers – Nutzerfrage.
  - **Prüfungen:** `hugo --minify --panicOnWarning -s site` ohne Warnung (24 Seiten, 1 Alias), `scripts/site-check.py`
    ohne Fund, `scripts/ci-site.sh` und `scripts/ci-selftest.sh` (84 Zusicherungen) grün,
    `bildschirmfotos.py --abgleich`: 94 Namen, alle verwendet, keine Datei fehlt; `--pruefen`: alle 104 Aufnahmen
    antworten mit 200 und Inhalt.
  - **Sichtprüfung:** `.claude/sicht/r37/` (nicht im Repo): Kontaktbögen aller Bilder unter `bogen/`, dazu die
    Webseite (`seite-*.png`: Start, Hilfe, zwei Hilfeseiten, Installation, Betrieb, MCP – hell, dunkel, 1280 und
    390 px). Die Fotos der Webseite stammen von einer Kopie ohne `loading=lazy` (headless lädt solche Bilder nicht).
  - **Offen:** `docs/Backlog.md`, „Runde 37 – integriert“ (dort auch die Bausteine der drei Aufgaben). Fahrplan
    führt der PM nach.
- **Runde 36 abgeschlossen (PM, 2026-11-27):** Prüfung mit drei niedrigen Befunden, keine Nachbesserung; sie stehen
  in `docs/Backlog.md`, „Nach Runde 36 offen“ (`pages.yml` bei `release: published` braucht Tags `v*` in der
  Umgebung `github-pages`; `baruch-typst compile --format pdf a.typ a.typ` überschreibt wie Typst die Quelle;
  `TYPST_VERSION` steht wörtlich im Code). Voller Lauf des PM im Hauptordner nach `cargo clean -p` aller
  Workspace-Crates: Exit 0, 215 Ergebnisse „ok“, 3527 bestanden, 0 gescheitert, 2 ignoriert; Server-Bündel 1568 bestanden; pgvector-Tests liefen mit.
  `grep -rn "STUB:" crates` findet nur vier Zusicherungen in Tests, dass kein Vermerk mehr da ist – kein offener
  Stub. Die 73 Vermerke „R36: nicht bearbeitet, <Grund>“ im Backlog sind gegengelesen. Worktrees der Runde samt
  `target/` entfernt, die Zweige `team/typst_alias`, `team/sqlx09`, `team/reste` bleiben; Fahrplan nachgeführt
  (als Nächstes Runde 37 „Hilfe auf der Webseite“, dann 0.9.1). Nicht gepusht. Bildschirmfotos der Sichtprüfung:
  `.claude/sicht/r36/` (9 Bilder, nicht im Repo; der PM hat 02 und 05 nachgesehen).
- **Runde 36 „Reste“ integriert** (2026-11-27, Integrator, auf `development`; Schnittstelle R36-S1 `d03be8d`;
  Zweige `team/typst_alias` `fdfe844`, `team/reste` `252b7fa` und zuletzt `team/sqlx09` `350cb5e` (vorher
  `development` in den Zweig geholt, `2c06f76`) mit `--no-ff`, **ohne Textkonflikte**; danach der
  Integrations-Commit; nicht gepusht; Worktrees und team-Zweige stehen noch). Version unverändert 0.9.0, CHANGELOG
  unter „Unveröffentlicht“. **Keine Migration** (0055, 0056 und 0109 blieben unbenutzt), keine neue Route,
  RPC-Methode oder MCP-Werkzeug, kein offener Stub. Voller Lauf im Hauptordner nach `cargo clean -p` aller
  Workspace-Crates (`RUST_TEST_THREADS=4 … taskset -c 0-7 cargo test --workspace -j 4 --no-fail-fast`): Exit 0,
  215 Ergebnisse „ok“, 3527 bestanden, 0 gescheitert, 2 ignoriert (Server-Bündel 1568 bestanden; pgvector-Tests
  liefen mit, `timeout 20 podman info` → 0); clippy `-D warnings` und `cargo fmt --check` sauber; `cargo deny
  check`: advisories, bans, licenses, sources ok – ohne die Ausnahme RUSTSEC-2026-0459.
  - **Typst-Kommandozeile:** `baruch-server typst …` bzw. Alias `baruch-typst` (`compile` zu PDF/PNG/SVG, `fonts`,
    `--version`; offline; Nicht-Unterstütztes wie `watch` mit Exit 2). Die Pakete liefern nur `baruch-typst`
    (das .deb lokal mit der Debug-Binärdatei paketiert: `./usr/bin/baruch-typst -> baruch-server`, keine Datei
    `typst`). Abnahme `tests/round36_abnahme.rs`: `baruch-typst compile` und `baruch-server typst compile` auf der
    Datei aus `export --out` ergeben Byte für Byte die PDF von `export --pdf` (Werk ohne Umschlag/Titelei).
    Doku: `docs/Verteilung.md`, „Typst-Kommandozeile“.
  - **sqlx 0.9.1** (libsqlite3-sys 0.40): Migrationen und Prüfsummen unverändert; zusammengesetztes SQL nur über
    `AssertSqlSafe` (Regel im Modulkommentar von `baruch-store/src/lib.rs`, Merksätze in `SKILLS.md`, „Runde 36“).
    **Vor dem Einspielen beim Nutzer den Datenordner sichern:** Eine echte, von 0.8.6 geschriebene Datenbank hat
    in der Runde niemand mit 0.9 geöffnet (nur nachgebaute).
  - **Reste:** `passwort` hebt die Sperre auf; Lektoren mit Anzeigename, „Im Werk suchen“, Darstellung ohne Werk;
    Anmeldeseite folgt dem Thema; Mindmap-Rundreise erhält die Wolke, Beschriftungen frei; Band-Zähler mit
    „(+ n ausgeblendet)“; Spiegel-Commit nennt beim Löschen eines Gliederungspunkts die Verluste; u. a.
    (`docs/Architektur.md`, „Runde 36 – Reste“).
  - **CI/Pages:** `ci.yml` setzt mit dem eigenen Programm (`scripts/ci-typst.sh`), `pages.yml` läuft auch bei
    `release: published`. **Nie gelaufen** – das zeigt erst der nächste Fast-Forward nach `main` bzw. das nächste
    Release. AppImage, .dmg, .msi nicht gebaut.
  - **Sichtprüfung** (Firefox headless, eigener Wegwerf-Server): 9 Fotos unter `.claude/sicht/r36/`; dabei
    behoben: Kopfzeile des Vorschlags im Lesefluss ohne Anzeigenamen. Offenes: `docs/Backlog.md`, „Runde 36 –
    integriert“.
- **Runde 35 abgeschlossen (PM, 2026-11-27):** Prüfung mit vier Befunden, der mittlere behoben (`40d6591`: Die Sperre
  gegen Durchprobieren zählt jetzt vor der Passwortprüfung, `Store::login_reserve`); die drei niedrigen stehen in
  `docs/Betrieb.md` und in `docs/Backlog.md`, „Nach Runde 35 offen“. Voller Lauf des PM im Hauptordner nach
  `cargo clean -p` aller Workspace-Crates: Exit 0, 211 Ergebnisse „ok“, 3452 bestanden, 0 gescheitert, 2 ignoriert; Server-Bündel 1519 bestanden; pgvector-Tests liefen mit. Worktrees der Runde samt `target/` entfernt, die Zweige
  `team/konten`, `team/rollen`, `team/oberflaeche_konten` bleiben; Fahrplan nachgeführt. Nicht gepusht. Der Worktree
  `.claude/worktrees/strg_s` (Zweig `team/strg_s`, offene Änderungen) gehört nicht zur Runde und ist unberührt.
  Bildschirmfotos der Sichtprüfung: `.claude/sicht/r35/` (20 Bilder, nicht im Repo).
- **Runde 35 „Konten und Rollen (Lektorat II)“ integriert** (2026-11-27, Integrator, auf `development`;
  Schnittstelle R35-S1 `e56c3f3`; Zweige `team/konten` `cc89a28`, `team/rollen` `03359d2`,
  `team/oberflaeche_konten` `3ee5b32` mit `--no-ff` in dieser Reihenfolge, **ohne Textkonflikte**; danach der
  Integrations-Commit; nicht gepusht; Worktrees und team-Zweige stehen noch). Version unverändert 0.9.0, CHANGELOG
  unter „Unveröffentlicht“. **Migrationen 0053** (`account`, `account_session`, `login_attempt`) **und 0054**
  (`work_member`), beide nur `CREATE TABLE` – vor dem Einspielen beim Nutzer den Datenordner sichern. Neue
  Abhängigkeit `argon2` 0.5. Kein neues MCP-Werkzeug. Kein Stub der Runde ist offen
  (`grep -rn "STUB:" crates` findet nur vier Zusicherungen in Tests, dass kein Vermerk mehr da ist). Voller Lauf im
  Hauptordner nach `cargo clean -p` aller Workspace-Crates (`RUST_TEST_THREADS=4 … taskset -c 0-7 cargo test
  --workspace -j 4`, 2026-11-27): 211 Testgruppen „ok“, 3449 bestanden, 0 gescheitert, 2 ignoriert (Server-Bündel
  1518 bestanden, pgvector-Tests liefen mit); clippy `-D warnings` und `cargo fmt --check` sauber.
  - **Ohne Konten bleibt alles wie bisher** (keine Anmeldung, kein Cookie, keine Umleitung); neu ist nur der Eintrag
    „Konten einrichten…“ im Menü „Baruch ▾“ und die achte Einstellungsseite „Konten“. Der Dienst des Nutzers ändert
    sein Verhalten also erst, wenn er ein Konto einrichtet.
  - **Mit Konten** verlangen Seiten, Formulare und `/rpc` eine Sitzung (Cookie `baruch_session`, HttpOnly,
    SameSite=Strict, hinter dem Proxy Secure; Argon2id; Sperre gegen Durchprobieren). `/mcp` bleibt bei Tokens. Das
    erste Konto ist der Autor und entsteht nur am eigenen Rechner (`/einrichten`); weitere Konten sind Lektoren.
    Notweg bei vergessenem Passwort: `baruch-server passwort <name> [--data <Ordner>]` (Passwort von stdin).
  - **Lektoren** (`@lektorat:<slug>`) sehen nur freigegebene Werke (Reiter Buch → Mitwirkende) und schreiben nur
    Vorschläge (`revision.propose`), Beiträge im Abschnitts-Chat und Befunde (`finding.add`, auch „Anmerkung zum
    Buch“) – mit Herkunft im Spiegel. Alles andere ist gesperrt: Für Lektoren gilt nur, was in
    `roles::LEKTOR_ROUTES` und `roles::LEKTOR_RPC` steht (die einzige Stelle der Rechteprüfung); neue Routen und
    Methoden künftiger Runden sind ohne Zutun gesperrt, zwei Tests zählen sie aus dem Quelltext nach.
  - **Vom PM entschieden, der Nutzer kann es umstoßen** (Vertrag §0): Anmeldung auch am eigenen Rechner (Schalter
    „Am eigenen Rechner ohne Anmeldung“ unter Einstellungen → Konten, Vorgabe aus); direkter Zugriff von anderen
    Rechnern bleibt gesperrt (Lektor kommt über den Reverse-Proxy auf demselben Rechner); Kontoart global, Rolle je
    Werk; Lektoren lösen keine Modellaufträge aus, kein Export (auch kein PDF), keine Literatur, keine Einfälle.
  - **Vom Integrator ergänzt:** zuletzt geöffnetes Werk bei der Umleitung eines Lektors (Cookie `baruch_work`, nur
    wenn freigegeben); `AppState::default_work` und das Cookie der Anfrage zeigen einem Lektor nie auf ein fremdes
    Werk (Sichtprüfung: „Zurück zum Werk“ auf `/werke` endete mit 403); Kopf eines Vorschlags ohne doppelten Handle;
    Formular „Anmerkung zum Buch“ linksbündig. Abnahme `tests/round35_abnahme.rs` mit echten Sitzungen. Einzelheiten:
    `docs/Architektur.md`, „Runde 35 – Konten und Rollen“ (dort auch die Handprüfliste mit 24 Punkten).
  - **Nicht geprüft:** echter Browser mit Passwortmanager, echter Proxy mit TLS (Secure-Cookie, SameSite=Strict bei
    Links von außen), der Vergleich Byte für Byte „ohne Konten“ gegen ein Programm vom Stand vor S1 (nur vom
    Entwickler der Oberfläche gemacht; der Integrator prüft die Startseite ohne den einen Menüeintrag auf Spuren).
  - Fotos der Sichtprüfung: `.claude/sicht/r35/` (20 Bilder: Einrichten, Anmelden, Konten, Mitwirkende, Menü und
    Arbeitsseite als Autor und Lektor, Befunde, Mein Konto, Ablehnung; hell/dunkel, mit/ohne JavaScript, 1024 px).
- **Runde 34 „Mindmap wie XMind“ abgeschlossen** (PM-Abschluss 2026-11-26; ein erster Durchgang am 2026-11-25
  wurde vor dem Lauf unterbrochen): Prüfung mit zwei niedrigen Hinweisen
  (Beschriftungen der Verbindungen überdecken Knoten; Tasten weichen leicht von XMind ab und stehen auf der Seite),
  keine Nachbesserung. Voller Lauf im Hauptordner nach `cargo clean -p` aller Workspace-Crates: 209 Testgruppen „ok“, 3334 bestanden, 0 gescheitert, 2 ignoriert (Server-Bündel 1460 bestanden, pgvector-Tests liefen mit; 2026-11-26).
  `grep -rn "STUB:" crates` findet nur vier Zusicherungen in Tests, dass kein Vermerk mehr da ist – kein offener
  Stub. Der PM hat die sechs nur erzeugten Bilder der Sichtprüfung nachgesehen (02, 03, 08, 11, 13; `10-ausgabe.svg`
  ist wohlgeformt): heller Streifen wie ein Rollbalken im dunklen Modus (02, 08), Beschriftung „widerspricht …“
  überdeckt „Springtide“ (02, 03, 11) – beides niedrig, im Backlog. **Vom PM entschieden** (der Nutzer kann es
  umstoßen): Unterknoten wandern mit in die Wolke; Einlesen ohne Zielknoten legt Hauptthemen an; dass der eigene Ast
  „Wolke“ beim Wiedereinlesen nicht in die Wolke zurückkommt, ist eine Aufgabe für die Runde „Reste“.
  `docs/Fahrplan.md` nachgeführt (Runde 34 = Mindmap; Vorschlag: 35 Konten und Rollen, 36 Reste, 37 Hilfe, danach
  0.9.1). Die Worktrees `.claude/worktrees/wf_f112b584-168-3/-4/-5` samt `target/` sind entfernt, die Zweige
  `team/mindmap_*` stehen noch. Nicht gepusht.
- **Runde 34 „Mindmap wie XMind“ integriert** (2026-11-25, Integrator, auf `development`; Schnittstelle R34-S1
  `67faa2f`; Zweige `team/mindmap_daten` `38f4022`, `team/mindmap_widget` `d47d3af`, `team/mindmap_austausch`
  `7513039` mit `--no-ff` in dieser Reihenfolge, **ohne Textkonflikte**; danach der Integrations-Commit; nicht
  gepusht; die Worktrees hat der PM-Abschluss entfernt, die team-Zweige stehen noch). Version unverändert
  0.9.0, CHANGELOG unter „Unveröffentlicht“. **Migration 0052** (`idea`: `title`, `parent_id`, `position`,
  `main_topic`; nur `ADD COLUMN`) – vor dem Einspielen beim Nutzer den Datenordner sichern. Austauschformat
  Unterversion 9. Neue RPC `idea.move`, `idea.tree`; neue MCP-Werkzeuge `idea_move`, `idea_tree`; neue Seiten
  `/werke/{id}/karte/gliederung`, `/werke/{id}/karte/einlesen`, Bild `/werke/{id}/karte/bild`, Ausgabe `.xmind`.
  Kein Stub der Runde ist offen.
  - **Nachtrag des Nutzers (2026-11-25): „Es macht Sinn eine Verbindung zwischen der Wolke und der Mindmap zu
    machen.“** Umgesetzt als eine Regel mit vier Plätzen: Elternknoten → Unterknoten; sonst Bezug → am Kapitel;
    sonst Hauptthema am Werk; sonst **Wolke** (Ablage unter dem Baum, „n in der Wolke“). Einfälle wandern aus der
    Wolke auf einen Knoten, ein Kapitel oder aufs Werk und zurück (samt Unterknoten); die Nähe schlägt am Kärtchen
    „passt zu Ast …“ vor. Bestehende lose Einfälle liegen in der Wolke, nichts wird ungefragt Hauptast.
  - **Gefunden an der Nahtstelle:** (1) Widget und Austausch hatten gegen S1 angenommen, „Einfälle direkt an der
    Wurzel“ seien die Wolke; die Daten liefern die Wolke getrennt und führen direkt am Werk die freien Hauptthemen.
    Ohne Eingriff wären Hauptthemen als Kärtchen der Wolke gezeichnet worden und die echte Wolke gar nicht. Gelöst
    über die Sicht `tree::Scene` (`tree::scene`), die Seite, Bild, Gliederungsansicht und Ausgaben jetzt lesen.
    (2) Das Widget rief `idea.delete` ohne `confirm: true`: Löschen und das Rückgängig eines Anlegens wären im
    Browser immer abgelehnt worden. (3) „In die Wolke“ im Formular hätte den Einfall zum Hauptthema gemacht
    (`parent: null, key: null`); jetzt `cloud: true`, dazu „Als Hauptthema ans Werk“ (`ziel=w`) und „Neues
    Hauptthema“ (`ort=werk`). (4) Einlesen ohne Zielknoten legt Hauptthemen an; die Seite sagte „in die Wolke“.
    Alle vier behoben und mit Test belegt (`round34_abnahme`).
  - **Was der Integrator sonst änderte:** `bild/layout.rs`, `bild/svg.rs`, `bild::load`, `export::tree_order`,
    `web/src/idea_map.ts` und `tree::NOT_YET` gelöscht; `PdfKind::Map` → „Mindmap“; `notice_of` kennt die neuen
    Rückmeldungen; zwei Vorlagen in `TEMPLATES` (`round33_seiten.rs`); Vorschlag der Wolke nennt den Hauptast mit
    Nummer; „Gelöschte Einfälle“ nennt den Titel; Doku aus `docs/r34/` eingesetzt (gelöscht). Angepasste ältere
    Tests: elf Rundreisen auf `version="9"`, `round25_abnahme`, `round25_bild`, `round18_menueband`,
    `round28_abnahme`, `round28_uebersicht`, `round9_menu`.
  - **Tests:** `round34_abnahme` (5 Tests: Bild mit Werk in der Mitte, Ästen rechts/links, Unterästen und allen
    Aktionen des Widgets über RPC – Tab, Enter, F2, Ziehen, Wolke ↔ Baum, Einklappen, Verbindung mit Beschriftung,
    Löschen und Wiederherstellen; Gliederungsansicht ohne JavaScript mit allen Aktionen; MCP legt Knoten mit
    Herkunft an; Rundreise `.mm` und `.xmind` in ein zweites Werk mit gleicher Struktur; alte Einfälle, Verbindungen
    und Lagen unverändert). Die Migration auf einer nachgebauten Datenbank im Stand 0051 prüft
    `crates/baruch-store/tests/migration_0052.rs`. Voller Lauf nach `cargo clean -p` aller 16 Workspace-Crates
    (`RUST_TEST_THREADS=4 nice -n 10 ionice -c3 taskset -c 0-7 cargo test --workspace -j 4 --no-fail-fast`), zuletzt
    nach den Korrekturen aus der Sichtprüfung: **Exit 0, 209 Testgruppen, 3334 bestanden, 0 gescheitert, 2
    ignoriert**; Server-Bündel 1460 bestanden; pgvector-Tests liefen mit (podman antwortete, 25 Zeilen „pgvector …
    ok“). clippy `-D warnings` und `cargo fmt --check` grün.
  - **Sichtprüfung** (Firefox headless nach dem Testlauf, eigener Server auf `127.0.0.1:0` mit erfundenem
    „Halligbuch“, Verbindungen auf `127.0.0.1:9`, kein Eliezer): Bilder unter `.claude/sicht/r34/`. `01`–`04` Mindmap breit und
    Laptop (1280 px), hell und dunkel; `05` zwei Äste eingeklappt („+ 5“, „+ 2“); `06` Mindmap ohne JavaScript;
    `07`–`09` Gliederungsansicht ohne JavaScript (gewählter Knoten mit allen Formularen, dunkel, schmal mit einem
    Einfall der Wolke); `10`/`11` Ausgabe SVG und PDF; `12` Vorschau beim Einlesen der eigenen `.xmind`; `13`
    Formular „Einlesen“; `14` die Mindmap, nachdem ein eingeschleustes Skript das Widget in Firefox bedient hat
    (Klick, Tab, Enter, F2, Entf mit Rückfrage, Strg+Z, Ziehen aus der Wolke auf einen Knoten, aufs Werk und in die
    Wolke, Leertaste) – jede Aktion kam an, der Stand danach über RPC geprüft. **Behoben nach der Sicht:**
    Beschriftungen der Beziehungen lagen unter den Knoten (jetzt eigene Gruppe `g.mm-link-labels` am Ende); der Knopf
    „Löschen“ der Gliederungsansicht war rot auf rot; alle Einfälle waren dort ohne Suche als Treffer hinterlegt;
    Schlagwörter standen doppelt, wenn sie schon der Titel sind. Server per PID beendet (Exit 0).
  - **Nicht geprüft:** das Widget von Hand mit echter Maus und Tastatur (die Probe schickt künstliche Ereignisse;
    Zoom, Verschieben der Fläche, Pfeiltasten, Alt+L mit Dialog und Alt+E liefen nicht mit); echtes XMind und Freeplane (Aufbau der `.xmind`-Datei aus dem
    Gedächtnis, Annahmen in `docs/Bedienung.md`); Werk 3 des Nutzers; macOS und Windows. Handprüfung und Reste:
    `docs/Backlog.md`, „Nach Runde 34 offen“.
- **Runde 32 „Lektorat I – Word und Beenden“ integriert** (2026-11-25, Integrator, auf `development`; Schnittstelle
  R32-S1 `647ae42`; Zweige `team/docx_import` `6a2f506`, `team/docx_export` `2e3246b`, `team/beenden` `08fd6c5` mit
  `--no-ff` in dieser Reihenfolge, **ohne Konflikte**; danach der Integrations-Commit; nicht gepusht, Worktrees
  `.claude/worktrees/wf_e1ad26c7-75a-3/-4/-5` und team-Zweige stehen noch). Version unverändert 0.9.0, CHANGELOG unter
  „Unveröffentlicht“. **Keine Migration** (höchste weiter Store `0051`, Bibliothek `0108`), kein neues MCP-Werkzeug;
  neue RPC-Methoden `docx_import.preview/get/apply/discard`, `work.docx_export`; neue Seiten `/werke/import/word`,
  `/werke/{id}/word`, `/beenden`; neues Crate `baruch-docx`. Kein Stub der Runde ist offen
  (`grep -rn "STUB:" crates | grep R32` leer).
  - **Was der Integrator änderte:** drei Zeilen in `TEMPLATES` (`round33_seiten.rs`, von allen drei Entwicklern
    erbeten); Leser: `w:startOverride` als erste Nummer einer Liste, Bildpfad aus `wp:docPr name`, Bilder in
    Typst-Inseln; `to_typst`: ausgeschriebene Nummern bei Listen ab ≠ 1; Signale werden vor der Zeile „Baruch läuft“
    eingerichtet (`system_signal` richtet beim Aufruf ein, `shutdown::serve_with`), `main` endet nach dem
    Herunterfahren mit `exit(0)`; `docx_export.css` (Kasten endet am Knopf); Doku aus `docs/r32/` eingesetzt
    (gelöscht).
  - **Gefunden an der Nahtstelle:** (1) Eine Liste ab 3 kam nach Export und Import als zwei Listen ab 1 zurück.
    (2) Das Bild einer Abbildung fehlte nach der Rundreise im Werk, obwohl der Text es nannte. (3) Ein `SIGTERM`
    unmittelbar nach „Baruch läuft“ beendete den Prozess hart (Empfänger noch nicht eingerichtet). Alle drei behoben
    und mit Test belegt.
  - **Tests:** `crates/baruch-docx/tests/roundtrip.rs` (Modell → Datei → Modell, 2 Tests), `round32_abnahme`
    (7 Tests: Menüeinträge; Word → Werk → Word → Werk mit gleicher Gliederung und gleichem Text; Vorschläge als
    `w:ins`/`w:del` mit Persona und zurück als Vorschlag; echter Prozess nur über Seiten und Formulare bis zum
    Beenden mit Exit 0 und `PRAGMA integrity_check`; SIGTERM; Regelprüfung der Arbeitsabläufe; Doku und Stubs).
    Voller Lauf nach `cargo clean -p` aller Workspace-Crates: **206 Testgruppen, 3272 bestanden, 0 gescheitert,
    2 ignoriert** (mit pgvector-Tests; podman antwortete); clippy `-D warnings`, `cargo fmt --check` und `cargo deny check`
    grün.
  - **Sichtprüfung** (Firefox headless nach den gezielten Läufen, eigener Server auf `127.0.0.1:0` mit erfundener
    Datei „Die Fähre von Almsund“): Bilder unter `.claude/sicht/r32/` (Vorschau, Werk mit Vorschlägen, Reiter Buch mit
    „Word …“, Export-Seite, Menü Baruch, Rückfrage und Abschlussseite; hell/dunkel, mit/ohne JavaScript) und die
    exportierte Datei über LibreOffice ohne Fenster als PDF (`12-export-libreoffice-1.png`).
  - **Nicht geprüft:** Microsoft Word (weder Öffnen des Exports noch eine von Word geschriebene Datei), echter
    Browser beim Beenden (`window.close()` im App-Fenster), macOS, Windows, ein Lauf der getrennten Pages-Jobs.
    Handprüfung und Reste: `docs/Backlog.md`, „Nach Runde 32 offen“.
  - **Prüfung und Nachbesserung (2026-11-25, `a9a1ffb`):** drei Befunde. (mittel) Die Gruppe `pages` hält seit der
    Trennung in zwei Jobs nicht mehr die ganze Auslieferung – über die Doku behoben (`docs/Verteilung.md`: Grenze,
    Regel „`main` erst nach dem Ende des Release-Laufs schieben“, Test `pages_group_limit_is_documented`),
    technisch nicht geschlossen und ungemessen. (niedrig) `docs/RPC.md` nannte bei `docx_import.preview` die Grenze
    von `/rpc` nicht (413 ab etwa 24 MiB). (niedrig) Ein Proxy auf demselben Rechner, der `Host` auf `localhost`
    umschreibt, gilt für „Baruch beenden“ als eigener Rechner. Die beiden niedrigen hat der PM-Abschluss in der
    Doku nachgetragen (`docs/RPC.md`, `docs/Verteilung.md` „Baruch beenden“).
  - **PM-Abschluss (2026-11-25):** nach `cargo clean -p` für alle 16 Workspace-Crates
    `RUST_TEST_THREADS=4 nice -n 10 ionice -c3 taskset -c 0-7 cargo test --workspace -j 4 --no-fail-fast`: Exit 0,
    206 Ergebnisse „ok“, 3273 bestanden, 0 fehlgeschlagen, 2 ignoriert; Server-Bündel 1423 bestanden; pgvector-Tests
    liefen mit (podman antwortete, 25 Zeilen „pgvector … ok“). Nach den Doku-Nachträgen des Abschlusses lief das
    Server-Bündel noch einmal (es liest die Doku).
    `grep -rn "STUB:" crates`: 4 Treffer wie nach Runde 33, alle in Tests, die das Fehlen eines Stub-Vermerks
    zusichern (`round9_connections.rs`, `round13_abgleich.rs`, zweimal `literature.rs`); kein offener Stub im
    Quelltext, keiner aus Runde 32. `docs/Fahrplan.md` (Runde 32 erledigt, Release
    0.9.0 als nächster Schritt, Runden 34–36 unverändert geschätzt) und `docs/Backlog.md` („Nach Runde 32 offen“,
    Vorhaben Lektorat: Word und Beenden erledigt, erledigte PM-Punkte aus Runde 33) nachgeführt. Die vier bisher
    nur erzeugten Bilder (06, 13, 15, 16) sind angesehen. Aufgeräumt: Worktrees
    `.claude/worktrees/wf_e1ad26c7-75a-3/-4/-5` samt `target/` entfernt (zusammen 59 GiB; die team-Zweige bleiben),
    Zweig `runde33` gelöscht (war `1cfafa6`, in `development` enthalten). Nicht gepusht.
  - **Offene Entscheidung des Nutzers:** Der Tag `v0.9.0` zeigt auf `35140b5` (ohne Runde 32). Schiebt der Nutzer
    nach dem Release `development` nach `main`, liefert `pages.yml` eine Webseite aus, die Word-Import, Word-Export
    und „Baruch beenden“ nennt – in den Paketen 0.9.0 steckt das nicht. Wege in `docs/Backlog.md`, „Nach Runde 32
    offen“ (nur bis `35140b5` schieben, alles schieben, oder eine nächste Version festlegen).
- **Runde 33 „Menüband und Laptop-Modus“ integriert** (2026-11-25, Integrator; Hauptordner der Runde
  `.claude/worktrees/r33`, Zweig **`runde33`** – nicht `development`, weil Runde 32 dort parallel läuft; Schnittstelle
  R33-S1 `f7493c2`; Zweige `team/menueband-r33` `e9481ea`, `team/laptop` `d5f8fc0`, `team/seiten` `fdfc3fb` mit
  `--no-ff` in dieser Reihenfolge, **ohne Konflikte**; danach der Integrations-Commit; nicht gepusht, Worktrees und
  team-Zweige stehen noch). Version unverändert 0.9.0. Keine Migration, keine neue Route, keine neue RPC-Methode,
  kein neues MCP-Werkzeug; `grep -rn "STUB:" crates` unverändert 4.
  - **Was geht:** Auf jeder Seite das Menüband (`header.band`): Menü „Baruch ▾“, Reiter Start · Einfügen · Prüfen ·
    Buch · Ansicht (Arbeitsseite fünf, mit Werk vier, ohne Werk zwei), Gruppen mit Beschriftung, Abzeichen an den
    Reitern, Band auf/zu (▴/▾, Klick auf den gewählten Reiter, Cookie `baruch_band`). Die Knöpfe des Editors docken
    mit JavaScript an (Start: Text, Absatz, Fassung, Assistenten; Einfügen). Laptop-Modus Automatisch/An/Aus: unter
    1600 px kompakt (Gruppen als Popups), unter 1280 px Gliederung und Assistenten als Leisten mit Schublade; Spalten
    einzeln einklappbar; Zoom zurücksetzen. Ohne JavaScript: Reiter `?tab=`, `?band=`, `?laptop=`, `?lrail=`,
    `?rrail=` als Links, Gruppen als `<details>`, Schublade über `#pane-tree`/`#pane-assistants`.
  - **Arbeit des Integrators:** (1) Tests in fremden Dateien nachgezogen, die jede Aufgabe rot hinterlassen hatte:
    `round8_posts.rs` („Satz-Vorschau“), `round18_menueband.rs` (Satz-Seite: Feld Buch statt eigenem Band; Helfer
    `ribbon_of` entfällt), `round33_seiten.rs` (Gruppe „Darstellung“ aus base.html trägt den Thema-Knopf). Die beiden
    `.topbar`-Tests in `round9_menu.rs` hatte menueband schon umgeschrieben. (2) Toter Zweig `satz_page` aus
    `edit_toolbar.html` und `.ribbon-satz` aus `ribbon.css` entfernt, `karte.css` druckt ohne `.band` statt `.topbar`.
    (3) `docs/r33/*` in `docs/Bedienung.md` (Platzhalter) und `docs/Architektur.md` eingesetzt, Ordner gelöscht;
    alte Menüpfade in README, RPC.md, Architektur.md, CHANGELOG korrigiert; CHANGELOG-Abschnitt „Menüband und
    Laptop-Modus (Runde 33)“. (4) Shortcode `bild.html` mit `breite`/`hoehe`. (5) `tests/round33_abnahme.rs`
    (5 Tests: 18 Seiten mit Band, Reiter, Laptop-Zustand und jedem Reiter als Link; Liste alt → neu auf einer
    Unterseite; Schlüssel und Cookies zusammen; Abzeichen auf einer Unterseite; keine Reste). (6) Drei Korrekturen
    aus der Sichtprüfung: Mindestbreite der Gruppen (Beschriftung wurde gekappt: „Ansicht des Te…“), Zeile 1 im
    kompakten Band einzeilig (Autor entfällt, Titel kürzt – brach bei 1024 px um), „Einklappen ›“ der rechten Spalte
    mit Abstand zum Rahmen. (7) `bildschirmfotos.py`: `arbeitsplatz.png` mit `&laptop=off`, damit die 1400er-Bilder
    das volle Band zeigen; sieben Bilder neu unter `site/static/bilder/` (neu: `arbeitsplatz-laptop.png`).
  - **Tests:** nach `cargo clean -p` für alle 15 Workspace-Crates
    `RUST_TEST_THREADS=4 nice -n 10 ionice -c3 taskset -c 0-7 cargo test --workspace -j 4`: Exit 0, 197 Ergebnisse
    „ok“, 3147 bestanden, 0 fehlgeschlagen, 2 ignoriert; Server-Bündel 1374 bestanden. pgvector-Tests liefen mit
    (podman antwortet, ist aber langsam). `cargo clippy --workspace --all-targets -- -D warnings`: Exit 0.
    `cargo fmt --all --check`: sauber. `hugo --minify --panicOnWarning -s site`: ohne Warnung.
  - **Sichtprüfung** (Firefox headless, Wegwerf-Server mit dem erfundenen Beispielwerk, nie parallel zu einem
    Testlauf): 34 Bilder unter `.claude/sicht/r33/` (nicht im Repo): Arbeitsseite 1920 hell/dunkel mit allen fünf
    Reitern, Menü „Baruch ▾“ offen, Band zu, Leisten zu; 1366 hell/dunkel mit Popups (per Klick im Skript); 1024 mit
    Leisten; Satz, Einstellungen, Befunde, Gedankenkarte, Gliederung; ohne JavaScript: Arbeitsseite, Laptop „An“,
    Popup, Schubladen links/rechts, Menü, Satz, Band zu, `/werke` ohne Werk. Nicht geprüft: Tastatur (Strg+S, Alt+L,
    Alt+E, Alt+,) und Fokus im echten Browser – nur im Node-Nachbau der Widgets; Fenster schmaler als 1024 px.
  - **PM-Abschluss (2026-11-25, ≈ 01:30):** Die Prüfung nach der Integration fand keinen belegten Fehler (zwei
    Hinweise: Cookie `baruch_vw` wird beim ersten Aufruf nach einer Größenänderung noch alt gelesen; „Satz
    abbrechen“ nicht nachgesehen – Beleg nachgereicht: `form.typeset-cancel` im Block `band_buch` von
    `typeset.html`). Keine Nachbesserung. Voller Lauf nach `cargo clean -p` für alle 15 Workspace-Crates:
    Exit 0, 197 Ergebnisse „ok“, 3147 bestanden, 0 fehlgeschlagen, 2 ignoriert; Server-Bündel 1374 bestanden; pgvector-Tests liefen mit. `grep -rn "STUB:" crates`: 4. `docs/Fahrplan.md` nachgeführt (Runde 33, 0.9.0,
    Schätzung für Runde 32, das Zusammenführen und die Runden 34–36 als Vorschlag). Entschieden: Vorgabe-Reiter
    von Gliederung und Stichwortzettel bleibt „Start“ (Vertrag §7). Aufgeräumt: die drei Worktrees der Runde
    (`wf_d9752058-7ed-3/-4/-5`) samt `target/` entfernt, die Zweige `team/menueband-r33`, `team/laptop`,
    `team/seiten` bleiben. **Nicht angefasst:** die drei Worktrees von Runde 32 (`wf_e7da3bae-4ce-3/-4/-5`,
    uncommittete Arbeit) und der Hauptordner auf `development`.
  - **Offen (siehe `docs/Backlog.md`, „Nach Runde 33 offen“):** Zusammenführen mit `development` (drei Commits
    seit `ed9d210`, Probe: ein Konflikt in `docs/Architektur.md`) und mit Runde 32 (Probe gegen R32-S1: `lib.rs`,
    `base.html` an den Marken `R32-PLATZ`, `tests/main.rs`, `round9_menu.rs`), danach die Gegenprobe durch einen
    eigenen Gutachter (Vertrag §11) und das Auflösen des Zweigs `runde33`. **Push erst nach ausdrücklichem „ja“ des
    Nutzers.**
  - **Handprüfung durch den Nutzer** (eigener Wegwerf-Server aus `runde33`, nicht der Dienst auf 8765): Band breit
    (≥ 1600 px) – Reiter wechseln, Speichern im Band und mit Strg+S, Prüfen/Vorschlag; Fenster auf Laptop-Breite –
    Gruppen als Popups, unter 1280 px Leisten und Schubladen (Esc, Klick daneben); Ansicht → Laptop-Modus An/Aus;
    ohne JavaScript – Reiter, Gruppen und `?laptop=on` als Links.
- **Runde 24 „Verteilung (B5)“ integriert** (2026-11-24; Schnittstelle R24-S1 `6f97294` direkt auf `development`;
  Zweige `team/pakete` `dfbf20a`, `team/ci` `ff0db95`, `team/webseite` `285b58e` mit `--no-ff` in dieser Reihenfolge,
  **ohne Konflikte**; danach der Integrations-Commit; nicht gepusht, kein Tag, kein Arbeitsablauf ausgelöst; die
  Worktrees der Runde sind im PM-Abschluss entfernt, die team-Zweige bleiben wie bei allen Runden). Version **0.9.0**. Keine
  Migration (Store 0050, Bibliothek 0108), keine neuen RPC-Methoden oder MCP-Werkzeuge. Gutachten, Nachbesserung
  und PM-Abschluss (Fahrplan) sind erfolgt – siehe die Blöcke unten.
  - **Was geht:** Über-Seite `/ueber` und `/ueber/lizenz` (Werkzeuge ▾ → Über Baruch, Link unter Einstellungen →
    Daten; keine Aktualisierungsprüfung); `baruch-server --version`; `--allow-host NAME`/`BARUCH_ALLOWED_HOSTS` für
    den Betrieb hinter einem Reverse-Proxy (unter einem solchen Namen `/mcp` nur mit Token, `docs/Betrieb.md`);
    MultiGPT ohne Vorgabe-Adresse (Platzhalter `https://multigpt.example/mcp/`); Pakete .deb und AppImage (lokal
    gebaut und gestartet), .dmg/.msi als CI-Aufträge (nie gelaufen, unsigniert), je mit `LICENSE`,
    `THIRD-PARTY-LICENSES.txt`, `QUELLTEXT.txt`; Arbeitsabläufe `ci.yml`, `release.yml`, `pages.yml` (nie gelaufen);
    Webseite `site/` (Hugo) mit sechs Bildschirmfotos aus einem erfundenen Werk; clippy mit `-D warnings` grün.
  - **Integrator:** Prüfsumme in `baruch-rag/tests/port.rs` nachgezogen; `round12_lancedb.rs` an die leere
    MultiGPT-Vorgabe angepasst; Abnahme `tests/round24_abnahme.rs` (drei Tests; Helfer `open_allowing`, `mcp_ping`
    aus `round24_betrieb` als `pub(crate)`); `cargo fmt --all`; Doku-Bausteine der drei Berichte nach `README.md`,
    `docs/Verteilung.md`, `docs/Bedienung.md`, `docs/Architektur.md`, `docs/Backlog.md`, `docs/Plan.md`, `SKILLS.md`
    übertragen, `docs/runde24/` gelöscht (Berichte: `git show dfbf20a:docs/runde24/pakete.md`,
    `git show ff0db95:docs/runde24/ci.md`, `git show 285b58e:docs/runde24/webseite.md`); sechs Fotos unter
    `site/static/bilder/`; im Stil der Webseite `scrollbar-color` für Codeblöcke; in dieser Datei die lokalen
    Scratchpad-Pfade durch `<Scratchpad>` ersetzt (nur im aktuellen Stand – in der Historie stehen sie weiter).
  - **Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **197 Läufe „test result: ok“, 3086 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel 1339 in 202 s). `cargo clippy --workspace --all-targets -j 4 -- -D warnings` EXIT 0,
    `cargo fmt --all -- --check` nach dem Formatieren ohne Meldung, `cargo deny check licenses sources bans` in
    Ordnung, `scripts/ci-selftest.sh` 51 Zusicherungen, `scripts/ci-check-workflows.sh` drei Abläufe in Ordnung,
    `hugo --minify --panicOnWarning -s site` ohne Warnung, 135 interne Verweise gültig.
  - **Pakete Ende zu Ende:** `scripts/build-deb.sh` → `target/debian/baruch_0.9.0-1_amd64.deb` (75 MB, Steuerarchiv
    nur `control`), mit `dpkg-deb -x` entpackt; Server daraus, aus dem AppDir (`AppRun`) und aus
    `target/appimage/Baruch-0.9.0-x86_64.AppImage` (116 MB, FUSE) je mit `--data <Probeordner> --secrets memory
    --listen 127.0.0.1:0` gestartet: `--version` „baruch-server 0.9.0“, `/`, `/ueber`, `/einstellungen/daten` 200,
    `/ueber/lizenz` `text/plain`; `HOME`/`XDG_*` (leere Ordner) danach leer. Probeordner und Logs unter
    `~/.cache/baruch-runde/integrator24_*` (können weg).
  - **Sichtprüfung** (Firefox headless nach dem Lauf, eigener Server aus dem entpackten Paket mit dem erfundenen Werk
    „Ein Jahr auf Mövenoog“, Verbindungen auf `127.0.0.1:9`, kein Eliezer): 24 Bilder unter `.claude/sicht/r24/` –
    Webseite Start, Funktionen, Installation, Betrieb je hell/dunkel und 1400/420 Pixel breit (`seite-*.png`; die
    Bilder der Seite Funktionen in `seite-funktionen-*-bilder.png`), Über-Seite hell, dunkel, schmal, ohne JavaScript
    und mit offenem Menü Werkzeuge (`ueber-*.png`), Einstellungen → Daten ohne JavaScript. Behoben: heller
    Rollbalken-Streifen unter Codeblöcken im dunklen Thema der Webseite. Sonst nichts Auffälliges.
  - **Offen / nicht geprüft:** kein Arbeitsablauf ist je gelaufen (auch nicht Pages, .dmg, .msi, `cargo deny check
    advisories`, Signieren mit dem echten Schlüssel); `typst compile` mit der Typst-Kommandozeile lief lokal nicht
    (Rechteprüfung der Sitzung lehnte ab, nicht umgangen); Windows/macOS nie gebaut oder gestartet; Proxy-Beispiele
    an keinem echten Proxy; `PGSSLROOTCERT` nur aus dem Quelltext von sqlx (prüft nur bei `verify-full`/`verify-ca`,
    nicht bei `require` – die frühere Annahme war falsch). Alles in `docs/Backlog.md`, „Nach Runde 24 offen“.
  - **Schritte des Nutzers** (Reihenfolge und Einzelheiten: `docs/Backlog.md`, „Nach Runde 24 offen“ → „Schritte
    des Nutzers“): 1. über die **Historie** entscheiden (so veröffentlichen, umschreiben oder frisches Repo – in
    älteren Commits stehen private Autoren-Adresse, Rechnername, Scratchpad-Pfade, die frühere Intranet-Adresse
    sowie Name und Buchtitel eines Dritten); 2. **Archivschlüssel** wählen (eigener für Baruch empfohlen),
    **Fingerabdruck** in `site/hugo.toml`, `README.md`, `docs/Verteilung.md` eintragen, **Impressum** füllen;
    3. erster Push von `development`, Fast-Forward nach `main`; 4. Repo **öffentlich** schalten, DNS-CNAME
    `baruch` → `beerlesklopfer.github.io`, Pages-Quelle „GitHub Actions“, Umgebungen `github-pages` (Tags `v*`)
    und `release` (Freigabe, Tags `v*`, darin die Secrets `APT_GPG_PRIVATE_KEY`/`APT_GPG_PASSPHRASE`);
    5. Generalprobe per „Run workflow“; 6. Tag, Freigabe im Lauf, Entwurf vor dem nächsten Push auf `main`
    veröffentlichen; 7. im eigenen Dienst nach Sicherung einspielen und die MultiGPT-Adresse prüfen.
  - **Nachbesserung nach dem Gutachten** (2026-11-24, acht mittlere Befunde, alle geprüft und zutreffend):
    - *Archivschlüssel:* Job `publish` läuft in der Umgebung `release`; `scripts/ci-check-workflows.sh` lehnt
      `secrets.APT_GPG_*` außerhalb dieser Umgebung ab (Regel 10). Die Wahl des Schlüssels (eigener oder der
      gemeinsame mit ForgeIEC) bleibt beim Nutzer; `docs/Verteilung.md` nennt die Abwägung und empfiehlt den eigenen.
    - *Fingerabdruck:* neues `scripts/ci-fingerprint.sh` (drei Stellen müssen übereinstimmen; mit `--key` gleich
      dem Schlüssel). `release.yml` bricht vor dem Signieren ab, wenn er fehlt oder abweicht, und warnt schon im
      Job `linux`; `ci.yml` warnt bzw. scheitert bei Widerspruch; `scripts/ci-site.sh` warnt bei leerem
      Fingerabdruck und bei Seiten mit `platzhalter: true` (Impressum). README erklärt den Platzhalter. Der Test
      `round24_abnahme` verlangte bisher den **leeren** Fingerabdruck und wäre beim Eintragen gescheitert – jetzt
      leer oder 40 Hex-Zeichen.
    - *Historie:* nichts umgeschrieben. Im heutigen Stand sind Rechnername (Titelzeile dieser Datei), der
      Anmeldename in `docs/Plan.md` und alle Nennungen des Dritten entfernt; die Wege für die Historie stehen im
      Backlog.
    - *Beispielwerk:* `sample::work()` heißt „Beispielwerk: Leise Abhängigkeit (Auszug)“ von „Erika Mustermann“,
      Kapitel 2 „Das heimliche Muster“; Beispielwort im Auftrag des Hörers „Perikope“; Tests und Doku nennen als
      langen Titel das erfundene „Mövenoog – ein ganzes Jahr auf der kleinen Insel“. Keine Migration: vorhandene
      Werke behalten ihren Titel.
    - *Backlog:* Schritte zur Veröffentlichung mit Reihenfolge, Sichtbarkeit des Repos und den drei Commits; die
      66 Marken für Runde 24 heißen „offen nach R24“ (nichts davon wurde in R24 umgesetzt), die sechs Marken
      R24b/R24c tragen ihr Ergebnis.
    - *Doku:* `docs/MCP.md` „Zugang“ und `docs/RPC.md` nennen `--allow-host` und die Token-Pflicht von `/mcp` unter
      einem erlaubten Namen.
    - *Tests:* `baruch-core` `sample::tests::the_sample_work_names_no_real_author_and_no_real_book`;
      `round24_abnahme` `the_current_tree_names_no_private_machine_and_no_third_party`,
      `the_interface_docs_name_the_token_rule_behind_a_proxy`; `scripts/ci-selftest.sh` 67 statt 51 Zusicherungen.
    - *Fahrplan:* im PM-Abschluss nachgeführt (unten).
    - *Lauf der Nachbesserung:* `nice -n 10 ionice -c3 cargo test --workspace -j 4 --no-fail-fast`: 197 Läufe „test
      result: ok“, 3089 bestanden, 0 fehlgeschlagen, 2 ignoriert, EXIT 0; `cargo fmt --all -- --check` ohne Meldung,
      `cargo clippy --workspace --all-targets -j 4 -- -D warnings` EXIT 0, `cargo deny check licenses sources bans`
      in Ordnung, `scripts/ci-selftest.sh` 67 Zusicherungen, `scripts/ci-check-workflows.sh` drei Abläufe in Ordnung,
      `scripts/ci-site.sh` baut mit den zwei erwarteten Platzhalter-Warnungen. Handprüfung Teil A, Schritte 1–8, an
      einer Wegwerf-Instanz mit `curl` nachgestellt (Verbindung `eliezer` auf `127.0.0.1:9`); Teil B nicht (Pakete
      nicht neu gebaut, eigener Dienst nicht angefasst).
  - **PM-Abschluss** (2026-11-24, ≈ 18:50): Gutachten R24 mit 16 Befunden (8 mittel, 8 niedrig); die mittleren
    hat die Nachbesserung bearbeitet, von den niedrigen sind sechs hier behoben:
    - Test `round24_betrieb::the_program_prints_its_version_and_starts_nothing` für `--version`/`-V`;
    - Über-Seite: Aktualisiert wird „von Hand mit einem neuen Paket“, APT nur unter Debian/Ubuntu (vorher nannte
      der Satz APT für alle Pakete); `docs/Bedienung.md` und `docs/Verteilung.md` ebenso;
    - `ci.yml`: `cargo fmt --all -- --check` als erster Schritt im Job `test`;
    - `docs/Verteilung.md`: Überblickstabelle zum AppImage ohne Widerspruch zum „Stand der Prüfung“;
    - `docs/Fahrplan.md` (Runde 24 erledigt, keine Runde mehr geplant), Backlog (Rundenplan, Nutzerfrage 17 mit den
      drei verbleibenden Entscheidungen, neue Nutzerfrage 18, „Aus dem PM-Abschluss“), `docs/Plan.md`.
    - *Offen aus dem Gutachten:* Nutzerfrage 18 (`/mcp` fällt hinter einem Proxy, der `Host` umschreibt, offen aus –
      geschlossen ausfallen lassen?) und die Lieferkette in `release.yml` (ungepinnte `brew`/`choco`-Pakete).
    - *Lauf:* nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
      --no-fail-fast`: **197 Läufe „test result: ok“, 3090 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0 (Server-Bündel 1342 in 212 s; Bau nach dem Neustart des Rechners in 71 s). `cargo fmt --all -- --check` ohne Meldung, `cargo clippy --workspace
      --all-targets -j 4 -- -D warnings` EXIT 0, `scripts/ci-check-workflows.sh` drei Abläufe in Ordnung,
      `scripts/ci-selftest.sh` 67 Zusicherungen, 0 gescheitert.
    - *Nicht neu gebaut:* .deb und AppImage unter `target/` stammen aus der Integration (vor der Umbenennung des
      Beispielwerks und dem neuen Satz der Über-Seite). Vor einer Weitergabe neu bauen; die CI baut aus dem Tag.
    - *Aufgeräumt:* Worktrees `.claude/worktrees/wf_07f7058f-3ee-3/4/5` samt `target/` entfernt (59 GB; die Zweige
      `team/pakete`, `team/ci`, `team/webseite` bleiben, alle in `development` enthalten); Probeordner der Runde
      unter `~/.cache/baruch-runde/` gelöscht, Logs und die geprüfte Typst-Kommandozeile (`r24-ci-werkzeug/`) bleiben. Die 24 Bilder der Sichtprüfung liegen unter
      `.claude/sicht/r24/`, die sechs Fotos der Webseite unter `site/static/bilder/`.
    - *Wie weiter:* Keine Runde ist mehr geplant. Als Nächstes die Schritte des Nutzers (oben); mögliche Arbeit
      danach nur auf Wunsch und geschätzt in `docs/Fahrplan.md`, „Geplant“.
  - **Handprüfung durch den Nutzer, Teil A** – auf einer Wegwerf-Instanz, ohne Modell (nichts geht an Eliezer,
    wenn die Verbindungen wie in `SKILLS.md` „Wegwerf-Server“ umgestellt sind; `<Port>` nennt die Startzeile):
    1. `target/debug/baruch-server --version`: Erwartet genau „baruch-server 0.9.0“, kein Server startet.
    2. `target/debug/baruch-server serve --listen 127.0.0.1:0 --data "$(mktemp -d)" --secrets memory`, im Browser
       **Werkzeuge ▾ → Über Baruch** (`/ueber`): Version 0.9.0, Lizenz AGPL-3.0-or-later, Verweise auf Quellcode,
       Webseite und Release-Seite, Ort der Drittlizenzen, Datenordner, der Satz „fragt nie von sich aus im Netz
       nach neuen Versionen“. Der Verweis zum Lizenztext (`/ueber/lizenz`) zeigt den Text der AGPL als reinen Text.
       Derselbe Verweis steht unter **Einstellungen → Daten**. Ohne JavaScript ebenso.
    3. Neue Instanz, Werkliste: Das Beispielwerk heißt „Beispielwerk: Leise Abhängigkeit (Auszug)“, Autor „Erika
       Mustermann“ (Einstellungen → Werk); nirgends ein echter Name.
    4. Einstellungen → Verbindungen → MultiGPT: Das Adressfeld ist **leer**, der Platzhalter zeigt
       `https://multigpt.example/mcp/`; nichts wird abgefragt.
    5. Server beenden und mit einem Namen neu starten: `… serve --listen 127.0.0.1:0 --data <derselbe Ordner>
       --secrets memory --allow-host baruch.example.org`. Dann:
       `curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: baruch.example.org' http://127.0.0.1:<Port>/ueber` →
       **200**; mit `-H 'Host: fremd.example.org'` → **421**; ohne `-H` (also `127.0.0.1:<Port>`) → **200**.
    6. `/mcp` unter dem erlaubten Namen ohne Token:
       `curl -s -o /dev/null -w '%{http_code}\n' -H 'Host: baruch.example.org' -H 'Content-Type: application/json'
       -d '{"jsonrpc":"2.0","id":1,"method":"ping"}' http://127.0.0.1:<Port>/mcp` → **401**. Dieselbe Zeile ohne
       die `Host`-Angabe → **200** (lokal weiter ohne Token). Mit einem Token aus Einstellungen → MCP-Zugang
       (`-H 'Authorization: Bearer <Token>'`) unter dem Namen → **200**.
    7. Einstellungen → MCP-Zugang unter dem Namen aufrufen (`curl -s -H 'Host: baruch.example.org'
       http://127.0.0.1:<Port>/einstellungen/mcp | grep -o 'https://baruch.example.org/mcp' | head -1`): Die Seite
       nennt die Adresse `https://baruch.example.org/mcp` und den Befehl mit Token statt des Satzes über Loopback.
    8. Ungültiger Name: `target/debug/baruch-server serve --allow-host localhost --listen 127.0.0.1:0 --data
       "$(mktemp -d)" --secrets memory` → endet sofort mit Exit 2 und der Meldung „Ungültiger Rechnername …“.
    9. Skripte ohne Netz: `scripts/ci-selftest.sh` → „67 Zusicherungen, 0 gescheitert“; `scripts/ci-fingerprint.sh`
       → vor dem Eintragen Exit 3 mit dem Hinweis, wo der Fingerabdruck fehlt, danach Exit 0.
  - **Handprüfung durch den Nutzer, Teil B – Pakete und eigener Dienst:**
    1. .deb bauen und ansehen, ohne zu installieren: `scripts/build-deb.sh`, dann
       `dpkg-deb -c target/debian/baruch_0.9.0-1_amd64.deb | grep -E 'LICENSE|THIRD-PARTY|QUELLTEXT'` → alle drei
       Dateien unter `/usr/share/doc/baruch/`; `dpkg-deb -I …` nennt Version 0.9.0-1 und kein `postinst`.
    2. .deb installieren (`sudo apt install ./target/debian/baruch_0.9.0-1_amd64.deb`, nur der Nutzer):
       `baruch-server --version` → „baruch-server 0.9.0“; im Startmenü „Baruch“. **Achtung:** Der Starter nimmt
       Port 8765 und den echten Datenordner – läuft der eigene Dienst schon dort, vorher die Sicherung (Punkt 4)
       machen und nicht beides zugleich starten. Der Datenordner ist nach Installation und nach `sudo apt remove
       baruch` unverändert.
    3. AppImage: `packaging/appimage/build-appimage.sh`, dann
       `target/appimage/Baruch-0.9.0-x86_64.AppImage --version` → „baruch-server 0.9.0“; gestartet mit
       `--listen 127.0.0.1:0 --data "$(mktemp -d)" --secrets memory` zeigt `/ueber` als Ort der Drittlizenzen den
       Pfad im Abbild.
    4. Eigener Dienst (keine Migration, Store 0050, Bibliothek 0108): **vorher** Einstellungen → Daten →
       „Sicherung jetzt“ (oder den Datenordner bei gestopptem Dienst kopieren), dann das neue Programm einspielen
       und den Dienst neu starten. Erwartet: alle Werke unverändert, ein noch vorhandenes Beispielwerk behält
       seinen alten Titel.
    5. Danach **Einstellungen → Verbindungen → MultiGPT** ansehen: Die Vorgabe ist jetzt leer. Steht dort keine
       Adresse mehr (die Verbindung war nie gespeichert und lief nur über die frühere Vorgabe), die eigene
       Adresse einmal eintragen und speichern; „Quellen“ und Websuche melden bis dahin „nicht eingerichtet“ statt
       eines Fehlers. Steht die Adresse noch da, ist nichts zu tun.
    6. Webseite lokal: `scripts/ci-site.sh` → baut ohne Fehler und warnt zweimal („Fingerabdruck … fehlt“,
       „content/impressum.md ist als Platzhalter markiert“), solange beides offen ist; `site/public/index.html`
       im Browser ansehen (Start, Installation mit Platzhalter-Hinweis, Impressum).
- **Runde 23 „Härtung“ abgeschlossen** (2026-11-24 13:19 – ≈ 16:00; Schnittstellen R23-S1 `0fded6e` und R23-S1b `486da08` direkt auf
  `development`; Zweige `team/web` `c96476e`, `team/mcp-namen` `da6807a`, `team/geheimnisse` `bc298bd` mit `--no-ff` in
  dieser Reihenfolge, **ohne Konflikte**; Integration `2bfd677`, Nachbesserung `af2628e`, PM-Abschluss danach; nicht
  gepusht; Worktrees `.claude/worktrees/wf_20e1b3fd-3f9-3/4/5` samt `target/` entfernt, team-Zweige bleiben).
  Gutachten, Nachbesserung und PM-Abschluss sind erfolgt (unten). Keine Migration (Store 0050, Bibliothek 0108), keine
  neue Unterversion des Austauschformats, keine neuen RPC-Methoden oder MCP-Werkzeuge.
  - **PM-Abschluss:** Gutachten 14 Befunde (7 mittel, 7 niedrig); sechs mittlere behob die Nachbesserung, der siebte
    (Vorbedingungen der Veröffentlichung) ist Sache des Nutzers. Der PM behob drei niedrige: Begründung der
    Zusicherung in `baruch-epub/tests/round23.rs`; `fetch_allowed` lehnt auch `198.18.0.0/15` und `64:ff9b:1::/48` ab
    (Tabellentest, `docs/Architektur.md`); die Werkzeugbeschreibungen von `ask_persona` und `chat` nennen „alle Clients
    ohne Token zusammen höchstens vier“. Bewusst offen (offen nach R24, Backlog „Aus dem PM-Abschluss“): `ribbon::code_lines`
    bei unausgeglichenen Klammern im Markup, `?ok=sicherung` als Meldung ohne Beleg, Umbruch in der Token-Liste, kein
    DNS bei `fetch_allowed`, Rechte eines vorhandenen Datenordners. Fahrplan, Backlog und Plan nachgeführt.
    **Lauf des PM** nach `cargo clean -p` aller 15 Workspace-Crates, `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **197 Läufe „test result: ok“, 3057 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel 1318 in 229 s); `cargo fmt --all -- --check` ohne Meldung.
  - **Was geht:** Knopf „Zitat“ fügt `#quote(block: true)[]` mit Folgezeile „– “ ein, das Satzbild zeigt Zitatblock
    und Stellenangabe als eigenen Absatz (keine Code-Insel); „Fassung ändern“ über den Baum legt einen Schritt im
    Verlauf an und prüft den Stand (auch ohne JavaScript); `own_reads` lehnt lesende Unterressourcen fremder Seiten ab;
    Meldungen nur über feste Codes; Abzeichen und „Anträge zur Gliederung“ ohne JavaScript auf jeder Seite. MCP:
    Autoren `@mcp:<handle>` mit Umlaut-Umschreibung, eindeutige Token-Handles, `lokal-` für Clients ohne Token mit
    Token-Namen, gemeinsames Bildkontingent ohne Token, Grenzen für `add_post`/`propose_revision`/`index_propose`/
    Gliederungsanträge, `ask_persona`/`persona.chat` mit `done_proposals`/`done_notes`/`all_done`. Geheimnisse:
    `baruch_llm::scrub` schwärzt auch Base64 und Prozentform, Prüfungen folgen keiner Weiterleitung, Treffer der
    Websuche im eigenen Netz werden nicht abgerufen, `model.connection_delete*` mit `confirm: true`, Grenzen beim
    Einlesen (Glossar, Registertitel), EPUBCheck-Temp-Ordner mit Zufallsnamen und 0700.
  - **Integrator:** `/act` reicht `based_on_amend` durch; eigene Größengrenze `MCP_BODY_LIMIT` (12 MiB) für `/mcp`
    (Codereview ki-2); neuer Datenordner mit 0700 (Codereview quellen-5); Re-Exporte der drei Obergrenzen in
    `baruch_store`; ältere Tests an das Vertragsverhalten angepasst (`round21_bilder.rs` gemeinsames Kontingent,
    `round31_glossar.rs` -32602, `round15_anbieter.rs` `confirm`, `round26_roundtrip.rs` alter Text im Verlauf,
    XSD u8 in `round27_daten.rs`/`round28_befunde.rs`/`round30_varianten.rs`); Abnahme `tests/round23_abnahme.rs`
    (drei Tests), `baruch-store/tests/round23_data_dir.rs`; Doku der drei Berichte in `docs/RPC.md`, `docs/MCP.md`,
    `docs/Bedienung.md`, `docs/Architektur.md`, `docs/WYSIWYG.md`, `docs/Austauschformat.md`, `SKILLS.md` (je Abschnitt
    „Runde 23 – Härtung“), überholte Stellen berichtigt, `docs/runde23/` gelöscht (Berichte mit den Tabellen „Punkte“
    und „Codereview“: `git show team/<id>:docs/runde23/<id>.md`).
  - **Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **197 Läufe „test result: ok“, 3054 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel 1316).
  - **Sichtprüfung** (Firefox headless nach dem Lauf, Wegwerf-Server mit erfundenem Werk, Verbindungen auf
    `127.0.0.1:9`, kein Eliezer): zwölf Bilder unter `.claude/sicht/r23/` – Editor mit Zitatblock und Stellenangabe
    (Satzbild und Quelltext, hell/dunkel, mit und ohne JavaScript), `/gliederung` mit zwei Anträgen ohne JavaScript,
    Einstellungen → Daten nach „Sicherung jetzt“ (Ordnername vom Server, `ziel` aus der Adresse erscheint nicht),
    Token-Liste mit `@mcp:laptop-kueche`, `@mcp:laptop-kuche`, `@mcp:buero` samt „früher …“. Nichts Auffälliges; klein:
    „(früher `@mcp:…`)“ bricht in der schmalen Spalte mitten im Namen um.
  - **Nachbesserung nach dem Gutachten** (2026-11-24, auf `development`, keine Migration):
    - *Handles im Bestand:* Zwei Tokens aus der Zeit vor Runde 23, die erst die neue Regel auf denselben Autor legte
      („Büro“ bis R22 `@mcp:b-ro`, „buero“ `@mcp:buero`), bleiben getrennt: Der Token mit Umlaut behält seinen
      Alt-Handle (`mcp::session::keeps_legacy`, bei jeder Anfrage aus der Token-Liste bestimmt), besitzt nur diesen,
      und die Seite „MCP-Zugang“ nennt den Grund. Tokens ohne solchen Zusammenfall verhalten sich wie integriert.
    - *Kontingente ohne Token:* Ein neuer Name öffnet kein neues Kontingent mehr – Modellaufträge aller Clients ohne
      Token zusammen höchstens vier (`quota::MAX_WITHOUT_TOKEN`, je Name weiter zwei), `add_post` (300/24 h) und
      offene Vorschläge (100) zählen für alle Clients ohne Token zusammen (`mcp/tools.rs::Counted`); Tokens unberührt.
    - *Doku:* überholte Stellen berichtigt (`?ok=sicherung` ohne `ziel`, `MCP_BODY_LIMIT`, `defined_functions`,
      Verweise auf die abgeschlossene Runde), README „Stand und Grenzen“ auf dem Stand nach Runde 23, Webseite
      `https://baruch.bernau.family` in README und `Cargo.toml` (`homepage`), MCP.md und Bedienung.md um die beiden
      Regeln oben ergänzt.
    - *Backlog:* alle 110 Marken für Runde 23 an ihrer alten Stelle tragen ihr Ergebnis; „Nach Runde 23 offen“ ist um
      die fehlenden offenen Punkte ergänzt, Belege zeigen auf die Commits `c96476e`, `da6807a`, `bc298bd`;
      Nutzerfragen 14–17 neu (Befund-Blöcke, `persona` ohne Token, Befunde vorspulen, Veröffentlichung).
    - *Tests:* `mcp/session.rs` (`tokens_that_only_the_new_rule_would_merge_keep_their_old_handles`),
      `round23_mcp_namen.rs` (`two_old_tokens_that_only_the_new_rule_would_merge_stay_apart`,
      `clients_without_token_share_the_model_slots_whatever_they_call_themselves`, erweitert
      `add_post_and_propose_revision_have_size_limits_and_quotas`).
  - **Handprüfung durch den Nutzer, Teil A** – auf einer Wegwerf-Instanz (`target/debug/baruch-server serve --listen
    127.0.0.1:0 --data "$(mktemp -d)" --secrets memory`; ohne Modell, nichts geht an Eliezer, wenn vorher die
    Verbindungen wie in `SKILLS.md` „Wegwerf-Server“ umgestellt sind):
    1. Werk importieren, Abschnitt im **Quelltext** öffnen, Cursor in einen Absatz, Knopf „Zitat“: Erwartet ein Block
       `#quote(block: true)[]` mit je einer Leerzeile davor und danach, darunter eine Zeile „– “; der Cursor steht im
       Zitat. Zitat und Stellenangabe füllen, auf „Satzbild“ schalten: Zitatblock und darunter die Stellenangabe als
       eigener Absatz – **keine Code-Insel**.
    2. Im **Satzbild** Cursor ohne Auswahl in einen Absatz, Knopf „Zitat“: neuer Zitatblock hinter dem Absatz. Danach
       ein Wort markieren und „Zitat“: die Auswahl wird wie bisher ein Zitat im Fließtext.
    3. Speichern und gleich weitertippen bzw. zweimal schnell Strg+S: Der Editor ist während des Speicherns kurz
       gesperrt; nach dem Speichern fehlt kein getipptes Zeichen, es entsteht keine doppelte Fassung.
    4. Fokus per Tab ins Menüband: ← und → wandern zwischen den Knöpfen (mit Umlauf), Pos1/Ende springen an den Rand,
       Tab verlässt die Leiste; in der Liste „Absatzformat“ gehören die Pfeile der Liste.
    5. Umschlag-Editor (Werk ▾ → „Umschlag“, `/werke/<id>/umschlag`) bei schmalem Rücken: Ein
       Namensschild erscheint nur am gewählten Block (Fokus, Maus oder Auswahl), nichts überlagert sich.
    6. EPUB-Vorschau: Die Liste „Dateien und Kapitelgrenzen“ verteilt sich gleichmäßig auf die Spalten; Pfeiltasten in
       einer Auswahlliste laden nicht bei jedem Tastendruck neu (Eingabetaste oder „Anzeigen“ bestätigt).
    7. Einstellungen → Daten → „Sicherung jetzt“: Die Seite nennt den Ordner der neuesten Sicherung, die Adresse endet
       auf `?ok=sicherung` (ohne `ziel`).
    8. Einstellungen → MCP-Zugang: Token „Büro“ erzeugen – Autor `@mcp:buero`; danach „buero“ erzeugen: abgelehnt
       („ergibt denselben Autor“). Ein Programm ohne Token, das sich „Büro“ nennt, schreibt als `@mcp:lokal-buero`.
  - **Handprüfung durch den Nutzer, Teil B – mit Werk 3** (der Anlass der Runde: Zitat mit Stellenangabe wurde am
    echten Buch zur Code-Insel), nach Sicherung von `~/.local/share/baruch` und Neustart des Dienstes (keine Migration):
    1. Den Abschnitt mit dem Zitat aus Ps 139 öffnen (Satzbild): Erwartet der Zitatblock und darunter die
       Stellenangabe als Absatz, keine Code-Insel. Steht die Stellenangabe dort noch in derselben Zeile wie die
       schließende Klammer des Zitats, im Quelltext in die Zeile darunter setzen („– Ps 139,…“) und speichern.
    2. An anderer Stelle im Satzbild mit „Zitat“ ein neues Zitat samt Stellenangabe einfügen, speichern, Satz-Ansicht
       öffnen: Zitat und Stellenangabe sind gesetzt wie im Satzbild; Buch-PDF ebenso.
    3. Fassung über den Baum ändern („Fassung ändern“): Die Änderung steht als Schritt im Verlauf der Fortschreibungen
       (mit „Rückgängig“).
    4. Einstellungen → MCP-Zugang: Die vorhandenen Tokens zeigen ihren Autor; steht bei einem „(früher `@mcp:…`)“,
       gehören ihm auch die alten Beiträge. Steht bei einem „behält den Autor von vor Runde 23“, schreibt er bewusst
       weiter unter dem alten Namen. Mit Claude Code (ohne Token) einen Beitrag schreiben lassen: Autor wie bisher,
       bei Umlaut im Namen jetzt umschrieben (`ü` → `ue`).
    5. Nur wenn Eliezer dafür frei ist: Lektor im Chat ansprechen (keine Prüfung) – er geht zuerst auf die Nachricht
       ein und beginnt nicht mit „Was trägt:“; bei „prüfen“ beginnt er damit (Regel 6). Auf den Ton achten.
  - **Offen** (Backlog „Nach Runde 23 offen“): `persona` ohne Token vom eigenen Rechner bleibt erlaubt
    (Vertrag §4, → Nutzerfrage 15); vor einer Veröffentlichung entscheidet der Nutzer über Zielversion, Adresse des
    Repos, die Intranet-Vorgabe der MultiGPT-Verbindung, Prüfung der Git-Historie, CI und Pakete (Backlog „Vor einer
    Veröffentlichung“, Nutzerfrage 17); `cross_site_subresource` ohne `Sec-Fetch-Mode`/`-Dest` gilt nicht als
    Unterressource; Klick auf „Zitat“ im Satzbild, Sperre beim Speichern, Pfeiltasten im Menüband, Namensschilder am
    Rücken und EPUB-Dateiliste nicht im Browser durchgeklickt (→ Nutzer); Ton des Lektors nach Regel 6 am echten Modell.
- **Runde 22 „Revisionen und Spiegel“ abgeschlossen** (2026-11-10 09:19 – ≈ 11:55 ab Schnittstelle R22-S1 `dfff467`
  direkt auf `development`; Zweige `team/fassungen` `a809403`, `team/befund-rueckfrage` `18d666a`,
  `team/spiegel-entwurf` `e39eac8` mit `--no-ff` in dieser Reihenfolge, **ohne Konflikte**; Integration `15165ce`,
  Nachbesserung `be1584e`, PM-Abschluss danach; nicht gepusht; Worktrees `wf_87fd2316-901-3/4/5` samt `target/`
  entfernt, team-Zweige bleiben). **Danach Pause** – keine neue Runde vorbereitet; R23/R24 nur auf Wunsch.
  - **PM-Abschluss:** Gutachten 15 Befunde (5 mittel, 10 niedrig); die fünf mittleren behob die Nachbesserung, der PM
    die niedrigen: Seitenzweige schreiben Bildpfade ab der Wurzel wie der HEAD (`tree.rs`), Vorschlag auf älterem Stand
    der Basis heißt „veraltet: Fassung n ist inzwischen bei Stand k“ (`view.rs`), „Erledigt: Befund n“ nur zu Befunden,
    die im Auftrag standen (`assist::propose_done`, sonst Vermerk), `post.repeat` überspringt nur offene Rückfragen
    „erledigt“ (`posts.rs`), Autor einer `<fortschreibung>` 1–200 Zeichen schon beim Einlesen (`check.rs`), Archivseite
    nennt die verlorene Werkart, Begründungen an sieben Zusicherungen; Doku: Architektur `<kN>`, MCP-Tabelle
    `revision_amends` und Anweisungen, RPC/Bedienung. Neue Tests: `round22_befunde.rs` (anderer Abschnitt),
    `round22_fassungen.rs` („veraltet“), `round22_seitenzweige.rs` (Bildpfad), `baruch-exchange/tests/read.rs` (Autor).
    Rest → R23a (Backlog „Nach Runde 22 offen“): Ändern über den Baum legt keinen Verlaufsschritt an.
  - **PM-Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast` auf `be1584e`: **190 Läufe „test result: ok“, 2919 bestanden, 0 fehlgeschlagen, 2 ignoriert**,
    EXIT 0 (Server-Bündel 1238); nach den PM-Korrekturen erneut voll: **190 Läufe ok, 2922 bestanden, 0
    fehlgeschlagen, 2 ignoriert**, EXIT 0; `cargo fmt --all` ohne Änderungen an fremden Dateien.
  - **Was geht:** „Speichern“ im Editor schreibt die eigene aktuelle Fassung fort („Fassung 1 · Stand 3“), je Schritt
    Text vorher/nachher und ein Spiegel-Commit; neue Fassung mit Grund (Knopf „Als neue Fassung speichern“,
    Persona-Fassung, andere Basis, Überschrift/Gliederung, große Änderung > 20 %, nach 30 min, ausgeschaltet).
    Verlauf mit Wort-Unterschied und „Bis hier zurück“ (neuer Schritt), Einstellungen → Werk → Fassungen,
    „weitere Fassung(en)“ im Editor-Kopf/Baum/Vorschau, Seitenzweige `refs/revisions/<key>/<n>` im Spiegel. Personas
    fragen beim erneuten Prüfen „Befund n scheint erledigt?“ (Ja/Nein durch den Menschen), übernommene Befunde zeigen
    „Fassung n · Stand k“. Archiv gelöschter Werke (`/werke/archiv`) mit Wiederherstellen. Wasserzeichen „Entwurf“
    (Einstellungen → Werk → Satz, Download „Als Entwurf herunterladen“ im Menü Werk, auf der Satz-Seite und am
    Stichwortzettel, ohne die Einstellung zu ändern); Austausch Unterversion 8. Migration **0050**.
  - **Integrator:** `<kN>`-Regel kommt beim Re-Import zurück („ohne Nummer“, kein toter Block in der Vorlage);
    Seitenzweig entsteht gleich nach jedem neuen Vorschlag (Personas, MCP, Zitate, Varianten, Lösungen); `node.get`
    trägt `amend_seq`/`based_on_amend`; `post.repeat` schlägt Befunde mit offener Rückfrage nicht noch einmal vor;
    Menüeintrag „Als Entwurf herunterladen“; `/werke/archiv` und Verlauf in `round9_menu.rs` (47 Seiten); MCP-Anweisungen
    nennen `revision_amends`, `ask_persona` nennt die Rückfrage; `lib.rs` exportiert `PartIdentity`/`PartRevision`/
    `PartAmend`. Befund beim Zusammenführen: fünf ältere Austausch-Tests (XSD u4/u5/u7) fielen, weil Stand 0
    (`beruht-stand`, `stand` an Befunden) geschrieben wurde – der Schreiber lässt Stand 0 jetzt weg (Austauschformat.md).
    Doku aus `docs/runde22/` in RPC/MCP/Bedienung/Architektur/SKILLS eingefügt, Ordner gelöscht; Plan, Backlog
    („Nach Runde 22 offen“), Fahrplan nachgeführt. `grep -rn "STUB:" crates` nennt nichts für R22.
  - **Abnahme** `tests/round22_abnahme.rs` (7 Tests, erfundene Werke „Die Salzwiese“, „Das Fährhaus am Deich“,
    „Deichtage“, „Rast“): dreimal fortschreiben, Rücknahme per Formular, neue Fassung auf Wunsch/große Änderung/Zeit/
    ausgeschaltet; Persona-Fassung → `not_own`, Seitenzweig sofort; Rückfrage nach ↻ Wiederholen mit MockEliezer und
    „Ja, erledigt“ per Formular; Wasserzeichen mit Datum auf jeder PDF-Seite (pdftotext), Stichwortzettel, Umschlag nur
    auf Wunsch mit Druckprüfungs-Warnung, EPUB, byte-gleich ohne; Austausch zwischen zwei Wegwerf-Instanzen mit
    Vorspulen und Teil-Antrag mit Kennung; Archiv; `<kN>` über Export → Import.
  - **Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: 5 Fehlschläge (die XSD-Tests oben); nach der Korrektur voller Lauf **190 Läufe „test result: ok“,
    2913 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0 (Server-Bündel 1237). `cargo fmt --all -- --check` ohne
    Meldung.
  - **Sichtprüfung** (`.claude/sicht/r22/`, Wegwerf-Server mit Modell-Nachbau auf 127.0.0.1, hell/dunkel, mit/ohne
    JavaScript): Editor mit beiden Knöpfen und Akkordeon Fassungen, Verlauf, Einstellungen → Fassungen und → Satz
    (Entwurf), Befund-Rückfrage (Befundseite und Chat), Satz-Vorschau mit Wasserzeichen, Stichwortzettel, Menü „Als
    Entwurf herunterladen“, Buch-PDF-Seite, Archiv. Befund: Knopf „Fassungen speichern“ lief über die ganze Breite
    (behoben in `revisions.css`).
  - **Nachbesserung nach dem Gutachten** (2026-11-10, auf `development`): Ein veralteter Editor-Reiter überschreibt
    keine zwischenzeitliche Fortschreibung mehr (Formularfeld `based_on_amend`, RPC `outline.save`/`revision.save`
    `based_on_amend?`; anderer Stand → neue Fassung „nicht auf der aktuellen Fassung bearbeitet“). Vorspulen beim
    Austausch nur auf der aktuellen Fassung des Menschen und nur, wenn die Schritte der Datei den neuen Text lückenlos
    ergeben – sonst wie vor Runde 22 „abweichende Fassung“ (Persona-/MCP-Fassungen, Vorschläge, ersetzte Fassungen
    bleiben unberührt). Bedienung.md (Statuszeile, Satzbild, Speichern, Löschen/Archiv, Zeitgrenze, zwei Reiter) und
    Austauschformat.md (Regel d0) nachgeführt. Tests: `revision_amend.rs`, `exchange_u8.rs`, `round22_fassungen.rs`.
  - **Handprüfung durch den Nutzer, Teil A** – auf einer Wegwerf-Instanz (`target/debug/baruch-server serve --listen
    127.0.0.1:0 --data "$(mktemp -d)" --secrets memory`); der Dienst auf 8765 bekommt Migration 0050 erst beim
    nächsten Neustart, davor Sicherung von `~/.local/share/baruch`.
    1. Werk importieren, einen Abschnitt im Editor zweimal klein ändern und „Speichern“: Erwartet „Gespeichert:
       fortgeschrieben (Stand 2)“, im Kopf „Stand 2“ mit Link auf den Verlauf; dort zwei Schritte mit Unterschied.
    2. „Bis hier zurück“ am Schritt 2: Text wie Stand 1, Stand 3, Verlauf vollständig.
    3. Einen ganzen Absatz neu schreiben und speichern: „neue Fassung 2 (große Änderung)“. „Als neue Fassung
       speichern“: neue Nummer.
    4. Persona prüfen lassen, Befund umsetzen, ↻ Wiederholen: Rückfrage „Befund n scheint erledigt?“ mit
       „Ja, erledigt“/„Nein, noch offen“ (Eliezer nur, wenn freigegeben).
    5. Einstellungen → Werk → Satz → Entwurf an (mit Datum): Satz-Vorschau und Buch-PDF tragen „ENTWURF“; Menü Werk →
       „Als Entwurf herunterladen“ geht auch bei ausgeschaltetem Entwurf; Umschlag-PDF nur mit Häkchen (dann Warnung).
    6. Werk löschen, Werke → Archiv → Wiederherstellen: aktuelle Texte als Fassung 1 zurück.
  - **Handprüfung durch den Nutzer, Teil B – mit Werk 3** (der eigentliche Wunsch: kleine Korrektur ohne neue Fassung,
    Entwurfs-PDF), nach Sicherung von `~/.local/share/baruch` und Neustart des Dienstes mit Migration 0050:
    1. In Werk 3 einen Abschnitt öffnen, ein Wort korrigieren, „Speichern“. **Zeitgrenze beachten:** Die 30 Minuten
       zählen ab dem letzten Schreiben der Fassung (Anlage oder letzte Fortschreibung). Am echten Werk sind die
       Fassungen älter, die **erste** kleine Korrektur ergibt deshalb erwartungsgemäß „Gespeichert: neue Fassung n
       (Zeitgrenze überschritten).“ – kein Fehler.
    2. Gleich danach ein zweites Wort korrigieren, „Speichern“: jetzt „Gespeichert: fortgeschrieben (Stand 1).“, im
       Kopf „Fassung n · Stand 1“ mit Link auf den Verlauf (ein Schritt mit Wort-Unterschied).
    3. Wer die Zeitgrenze für Korrekturrunden nicht will: Einstellungen → Werk → Fassungen → „Neue Fassung, wenn seit
       dem letzten Speichern … Minuten vergangen sind“ auf 0 (keine Zeitgrenze); dann schreibt schon die erste kleine
       Korrektur fort.
    4. Zwei Reiter auf demselben Abschnitt: in A klein ändern und speichern (fortgeschrieben), danach in B (alter Text)
       klein ändern und speichern → „neue Fassung … (nicht auf der aktuellen Fassung bearbeitet)“; der Text aus A bleibt
       als vorige Fassung erhalten.
    5. Werk ▾ → „Als Entwurf herunterladen“: Buch-PDF von Werk 3 mit Wasserzeichen „ENTWURF“ auf jeder Seite, ohne
       dass sich die Einstellung „Entwurf“ ändert; danach normaler Download ohne Wasserzeichen.
- **Runde 21 „Import und Bilder“ abgeschlossen** (2026-11-09 18:58 – ≈ 21:50, Beginn nach `716b076`; Schnittstelle
  R21-S1 `842b0ed` direkt auf `development`, Zweige `team/pdf-import` `198ebcd`, `team/markdown-import` `c758be1`,
  `team/bilder` `02fe1e6` mit `--no-ff` in dieser Reihenfolge, **ohne Konflikte**; Integration `3d50805`, Nachbesserung
  `fc6b3f6`, PM-Abschluss danach; nicht gepusht; Worktrees `wf_9464aa87-6e1-3/4/5` samt `target/` entfernt,
  team-Zweige bleiben).
  - **PM-Abschluss:** Gutachten 17 Befunde (1 hoch, 5 mittel, 11 niedrig); den hohen und die fünf mittleren behob die
    Nachbesserung (Backlog/Fahrplan der PM), der PM sechs niedrige: CLI „1 Seite/1 Abschnitt/1 Bild“, Fehlerausgabe von
    Poppler wird ganz geleert (vorher Hänger bis 120 s bei > 128 KiB stderr), „Pfade mit zwei Bildern“ ohne
    vorgewähltes Bild, Kasten „Lokal lesen“ mit `--warning` statt des fehlenden `--accent`, Exit 2 in Bedienung,
    Begründungen an drei Zusicherungen in `round21_pdf_import.rs`; dazu die Grenze des MCP-Kontingents in `docs/MCP.md`.
    Übrige niedrige Befunde → R23a/R23b (Backlog „Nach Runde 21 offen“). Abweichungen der Entwickler bestätigt.
    Fahrplan/Backlog nachgeführt (R22 160, R23 200 min; 28 „→ R21“-Punkte erledigt oder neu zugeordnet).
  - **PM-Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast` auf `fc6b3f6`: **184 Läufe „test result: ok“, 2813 bestanden, 0 fehlgeschlagen, 2 ignoriert**,
    EXIT 0 (Server-Bündel 1192); nach den PM-Korrekturen `cargo fmt --all -- --check` ohne Meldung und
    `cargo test -p baruch-server`: 1558 bestanden, 0 fehlgeschlagen.
  - **Was geht:** PDF/EPUB importieren (`Werk ▾ → PDF oder EPUB importieren…`, `/werke/import/quelle`, RPC
    `source_import.*`, CLI `import DATEI.pdf|.epub [--uebernehmen] [--bibliothek]`): Vorschau mit bearbeitbarer
    Gliederung, angelegt erst nach Bestätigung – als Werk, als Quelle der Bibliothek (lokaler Volltext, Migration
    **0108** in der Bibliothek) oder beides; PDF lokal (`pdftotext`) oder über MultiGPT, EPUB immer lokal.
    Markdown, ZIP und Ordner mit Bildern über „Neues Werk / Importieren…“, `work.import_archive`, CLI; Warnungen statt
    stillem Verwerfen (Berichtsseite). `export --out` prüft Ziele vorab. Bilder: Typwechsel beim Ersetzen („Endung
    anpassen“), Altbestand `/x.png` gegen `x.png` auflösen, MCP-Kontingent je Client und Werk, Aufräumen entschiedener
    Anträge nach 30 Tagen (410), Bilder in der Literatur, EPUB-Cover als Rasterbild der Vorderseite mit Titel.
  - **Integrator:** fremde Tests nachgezogen (`migration_0048/0049.rs` „≥ 107“, `round12_multigpt.rs` mit
    `rank_score`, `work_crud.rs` neues Datei-Feld); `/quelle` fragt `fulltext::has_fulltext` (lokaler Volltext sichtbar);
    RPC.md „Score“ berichtigt; `cargo fmt --all`. Sichtprüfung (`.claude/sicht/r21/`, mit und ohne JavaScript):
    Kollisionskasten auf „Bilder“ wurde auf 320 px gestreckt (behoben), Link „PDF oder EPUB importieren …“ stand neben
    statt unter dem Import-Formular (behoben), „1 Seiten“ in der PDF-Vorschau (behoben).
  - **Abnahme** `tests/round21_abnahme.rs` (8 Tests, „Der Apfelgarten am Deich“, frei erfunden).
  - **Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **184 Läufe „test result: ok“, 2808 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel 1189); nach den Korrekturen der Sichtprüfung betroffene Server-Tests erneut grün (169).
  - **Nachbesserung nach Gutachten** (2026-11-09): PDF/EPUB-Import setzt hinter eingebettete Ausdrücke
    (`#linebreak()`, `#emph[…]`, `#raw(…)`, `#footnote[…]` …) ein Schutz-`;`, wenn `(`, `[`, `;` oder `.Name` folgt
    (vorher scheiterte der Satz mit „expected function, found content“); eine führende Zahl in einer EPUB-Fußnote fällt
    nur noch weg, wenn sie gleich dem Fußnotenzeichen ist und kein Rücksprung die Nummer trug (Warnung
    `import-epub-footnote-number`; „1 Kor 13,4“ bleibt); Bilder in einer EPUB-Fußnote bleiben als `#image(…)` in ihr;
    der EPUB-Export meldet den Bildtyp nach dem Inhalt (PNG nach Typwechsel nicht mehr als `image/svg+xml`, Datei im
    EPUB mit passender Endung); der lokale PDF-Weg liest höchstens 2000 Seiten (Warnung `import-pdf-pages-cut`),
    höchstens 128 MiB Ausgabe ohne zweite Kopie und höchstens zwei PDFs zugleich. Tests
    `tests/round21_nachbesserung.rs` und Einheitstests in `typst_out.rs`/`pdf.rs`.
  - **Handprüfung durch den Nutzer** – auf einer **Wegwerf-Instanz**, der Dienst auf 8765 und
    `~/.local/share/baruch` bleiben unberührt (Migration 0108 erst beim nächsten Neustart des Dienstes; davor wie
    immer Sicherung von `~/.local/share/baruch`). Eigene Dateien (PDF, EPUB, Markdown) sind hier gefahrlos.
    1. **Instanz starten:** `cargo build -p baruch-server`, dann
       `target/debug/baruch-server serve --listen 127.0.0.1:0 --data "$(mktemp -d)" --secrets memory`. Erwartet: Zeile
       „Baruch läuft: http://127.0.0.1:<Port>/“; diese Adresse im Browser öffnen. Beenden am Ende mit Strg+C (bzw. per
       eigener PID); den Temp-Ordner danach löschen.
    2. **PDF lokal:** Werk ▾ → „PDF oder EPUB importieren…“, eine eigene PDF wählen, Weg „lokal“, „Lesen und Gliederung
       zeigen“. Erwartet: Vorschau mit „Weg: lokal“ und Grund, Hinweis `import-pdf-plain` (keine Hervorhebungen,
       Fußnoten, Bilder), ggf. „n Seitenzahl(en) entfernt“, je Punkt Zeichenzahl und Textprobe. Eine Zeile leeren,
       eine Ebene ändern (`==`), „Übernehmen als Werk“. Erwartet: Weiterleitung (303) auf das neue Werk; die Gliederung
       ist genau die bestätigte; der Text des gestrichenen Punkts steht beim vorigen (Bericht `import-outline-merged`).
       Werk ▾ → PDF: Das Buch-PDF entsteht ohne Satzfehler.
    3. **PDF über MultiGPT** (prüft die Annahmen `upload_document`, `run_status`, `document_info` mit
       Inhaltsverzeichnis, `read_document` seitenweise): Einstellungen → Verbindungen → MultiGPT eintragen (Schlüssel
       bleibt im Speicher dieser Instanz), dieselbe PDF mit Weg „MultiGPT“, Sammlung und Häkchen „in MultiGPT
       hochladen“. Erwartet: „Weg: multigpt“, Gliederung aus dem Inhaltsverzeichnis. Scheitert es: der Grund und der
       Knopf „Lokal lesen“, kein stiller Wechsel. Abweichende Werkzeugnamen oder Felder bitte melden.
    4. **EPUB:** eine EPUB mit Fußnoten und Bildern importieren. Erwartet: Gliederung aus dem Inhaltsverzeichnis,
       Fußnoten als `#footnote[…]` (Zahlen wie „1 Kor“ bleiben; entfernt Baruch eine Fußnotennummer, steht der Hinweis
       `import-epub-footnote-number` da), Bilder im Werk (Seite „Bilder“), übersprungene mit Grund; das Buch-PDF
       entsteht ohne Satzfehler.
    5. **Markdown:** Werk ▾ → „Neues Werk / Importieren…“, eine `.md`-Datei. Erwartet: ohne Hinweise öffnet sich das
       Werk; sonst Berichtsseite mit `md-…`-Codes und Zeile (eine einzelne Datei bringt keine Bilder mit:
       `md-image…`-Hinweise) und Link zum Werk.
    6. **ZIP und Ordner:** dieselbe Markdown-Datei mit ihren Bildern als ZIP hochladen, danach als *Ordner* (Feld
       „Ordner“; braucht einen Browser, der Ordner-Auswahl kann). Erwartet: Bilder kommen mit (Seite „Bilder“),
       fehlende oder Pfade mit `..` stehen mit Grund im Bericht. Kommandozeile: `target/debug/baruch-server import
       heft.zip --data <Temp-Ordner>` nennt Werk, Abschnitte, Bilder und Hinweise.
    7. **EPUB-Cover und Typwechsel:** in einem Werk mit Umschlagbild Werk ▾ → EPUB. Erwartet: das Cover zeigt die
       Vorderseite mit Titel. Dann auf „Bilder“ ein SVG mit „Endung anpassen“ durch ein PNG ersetzen und erneut das
       EPUB laden: Das Bild erscheint im Lesegerät (im Manifest `image/png`).
- **Runde 31 „Register und Glossar“ abgeschlossen** (2026-11-09 15:46 – ≈ 19:00, Beginn nach `0f12415`, vorgezogen vor
  R21; Schnittstelle R31-S1 `5f97a8b` direkt auf `development`, Zweige `team/glossar-daten` `4e724ba`,
  `team/register-satz` `d926236`, `team/kontextmenue` `a9ad284` mit `--no-ff` in dieser Reihenfolge, **ohne
  Konflikte** (`7ae30ed` `ecc38ab` `8e4e6ed`); Integrationscommit `5bf301b`, Nachbesserung `877d485`, PM-Abschluss
  danach; nicht gepusht; Worktrees `wf_60db2559-d5c-3/4/5` samt `target/` entfernt, team-Zweige bleiben).
  - **PM-Abschluss:** Gutachten 11 Befunde (4 mittel, 7 niedrig); die vier mittleren behob die Nachbesserung, der PM
    zwei niedrige (Rustdoc von `back_matter` stand vor `strip_back_matter`; Abschnitt „Integration“ in `docs/MCP.md`
    war Platzhalter) und die falsche Zusage zu Grenzen beim Einlesen in `docs/Austauschformat.md`; übrige niedrige
    Befunde → R21b/R23a/R23b/R23c/R24b (Backlog „Nach Runde 31 offen“). PM-Entscheidungen: Stichwörter nur bei
    wörtlicher Gleichheit zusammen; kein MCP-Werkzeug für `glossary.suggest`; Registervorschläge nicht im
    Austauschformat. Fahrplan/Backlog nachgeführt (R21 und R23 je 180 min).
  - **PM-Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **180 Läufe „test result: ok“, 2703 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel „1138 passed“); `cargo fmt --check` sauber; `grep -rn "STUB:" crates` nur in Tests (Zusicherungen
    „kein STUB“).
  - **Was geht:** Glossar je Werk (Migration 0049; Begriffe mit Varianten, kurzer/ausführlicher Erklärung, Quelle,
    Bibelstellen), eigene Register, Registermarken im Text (`#metadata((…))<baruch-idx>`, Setzen/Entfernen = neue
    Fassung des Menschen), Register Stichwort/Personen/Bibelstellen (automatisch, biblische Reihenfolge) + bis zu 13
    eigene; Registerseiten mit Seitenzahlen aus Typst (Bereiche, Unterstichwörter, „siehe/siehe auch“), Glossarseite,
    erste Erwähnung je Kapitel – alles nur Projektion (Export, Spiegel, PDF, EPUB), ohne Einstellungen byte-gleich.
    Kontextmenü im Satzbild und Quelltext (Rechtsklick, Kontextmenü-Taste, Umschalt+F10), ohne JavaScript Link
    „Markierung …“ und Seiten Glossar/Register. Personas/MCP nur Vorschläge. Austausch Unterversion 7.
  - **Integrator:** `instructions` nennen die drei Glossar-Werkzeuge; S1-Ausnahmen in `tests/mcp.rs` und
    `tests/round29_oberflaeche.rs` entfernt; Import eines Exports mit Buchende nimmt Buchende und Projektion heraus
    (`strip_back_matter`, `strip_projection`, offener Punkt von register-satz); Sonde in `round31_kontextmenue.rs`
    lesend gemacht (sie verdeckte den Datenweg); Sichtprüfung: Registerseite nennt Abschnitte mit Nummer und Titel
    statt „k13“, Fundstelle deutsch, Glossar-Seite „Vorgeschlagen von“ nur bei Assistenten.
  - **Abnahme** `tests/round31_abnahme.rs` (5 Tests, „Die Wärterin vom Kap“, frei erfunden).
  - **Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: Server-Bündel zuerst 1 Fehlschlag (Sonde in `round31_kontextmenue.rs`, behoben); Endstand nach den
    Korrekturen der Sichtprüfung: **180 Läufe „test result: ok“, 2700 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel 1136). `cargo fmt --check` sauber;
    `grep -rn "STUB:" crates` außerhalb der Tests leer. `typst` (CLI) fehlt auf dem Rechner: die `typst compile`-Teile
    sind übersprungen, der Export wird mit dem eingebetteten Compiler gesetzt.
  - **Sichtprüfung** (Bilder `.claude/sicht/r31/`): `kontextmenue-satzbild`, `glossar-formular`, `glossar-seite`,
    `register-seite`, `pdf-registerseite-07`, `pdf-bibelstellenregister-09`, `pdf-glossarseite-10`,
    `pdf-erste-erwaehnung-02`, `epub-vorschau-glossar`, `epub-vorschau-bibelstellen`.
  - **Nachbesserung nach Gutachten** (2026-11-09): `mark_word` setzt dieselbe Marke nie zweimal (eine Fundstelle mit
    derselben Marke in der Kette hinter dem Wort bleibt; „erste“/„n-te“ schon markiert bzw. bei „alle“ nichts mehr
    offen → Meldung „trägt … schon diese Registermarke“, keine neue Fassung; ein Registervorschlag dafür wird
    „veraltet“); ebenso das Kontextmenü im Quelltext (`markInSource`). „Begriffe vorschlagen lassen“ ist jetzt ein
    Formular (ohne JavaScript) auf der Glossarseite (ganzes Werk) und auf „An der Markierung“ (Abschnitt), vorher nur
    RPC. Die S1-Zweige „noch nicht umgesetzt“ in `round31_kontextmenue.rs`, `round31_glossar.rs` und
    `baruch-store/tests/glossary.rs` sind entfernt (422-Fälle prüfen echte Fehler), `IndexError::NotYet` ersetzt
    durch `AlreadyMarked`. Handprüfung unten mit Werk 3 und erwarteten Ergebnissen.
  - **Handprüfung durch den Nutzer mit Werk 3** (Dienst 127.0.0.1:8765; **Migration 0049 – vorher Sicherung** von
    `~/.local/share/baruch`). Die Begriffe „blinder Fleck“ und „Scham“ nur dort wählen, wo sie im Fließtext eines
    Abschnitts stehen (nicht in einer Überschrift):
    1. **Glossar über das Kontextmenü:** Werk 3 öffnen, einen Abschnitt mit „blinder Fleck“ im Satzbild öffnen, die
       beiden Wörter markieren → Rechtsklick (oder Umschalt+F10) → „In Glossar aufnehmen …“, kurze Erklärung
       eintragen, „Aufnehmen“. Erwartet: Dialog schließt ohne Meldung, der Abschnitt bleibt gespeichert (keine neue
       Fassung, kein „ungespeichert“); im Spiegel ein Commit `glossary-add`.
    2. **Glossar ohne JavaScript:** JavaScript im Browser abschalten (oder `/werke/3/markierung?key=<Abschnitt>`
       direkt öffnen), in der Statuszeile „Markierung …“, als Wort „Scham“ eintragen → „Übernehmen“ → bei „In Glossar
       aufnehmen“ Erklärung und gern eine Bibelstelle (z. B. „Gen 3,7“) eintragen → „In Glossar aufnehmen“.
       Erwartet: zurück auf der Seite mit „Begriff ins Glossar aufgenommen.“; eine unbekannte Bibelstelle wird mit
       Meldung abgelehnt und die Eingaben bleiben stehen.
    3. **Glossarseite:** Werk ▾ → Glossar. Erwartet: „blinder Fleck“ und „Scham“ alphabetisch unter „Begriffe“,
       Bibelstelle einheitlich geschrieben („Gen 3,7“); „Nachschlagen“ findet beide (auch über eine eingetragene
       Variante, z. B. „blinden Fleck“).
    4. **Register:** an der Markierung von „Scham“ „In Register aufnehmen“ (Stichwortregister, Fundstelle „erste“) →
       „Marke setzen“; dasselbe ein zweites Mal. Erwartet: beim ersten Mal neue Fassung und „Registermarke gesetzt“,
       beim zweiten Mal die Meldung „… trägt an der gewählten Fundstelle schon diese Registermarke; nichts geändert“
       und keine weitere Fassung. Werk ▾ → Register: „Scham“ steht einmal im Stichwortregister mit Verweis auf den
       Abschnitt (Nummer und Titel).
    5. **Begriffe vorschlagen lassen** (fragt den Lektor nach Einstellung; nur wenn ein Modell erreichbar ist):
       Glossarseite → „Begriffe vorschlagen lassen“. Erwartet: nach einer Weile „Der Lektor hat N Begriffe
       vorgeschlagen …“, die Vorschläge stehen gekennzeichnet unter „Vorschläge der Assistenten“ (nichts ist
       übernommen); ohne Modell eine ehrliche Meldung, nichts angelegt.
    6. **Satz:** Einstellungen → Werk → Satz → Register und Glossar: Glossarseite „nach den Registern“, Registerseiten
       „Stichwortregister“ und „Bibelstellenregister“ ankreuzen, speichern.
    7. **PDF:** Werk 3 als PDF setzen. Erwartet: am Buchende das Stichwortregister mit „Scham“ und Seitenzahl (die
       Seite, auf der das Wort steht, Zählung wie in der Fußzeile), das Bibelstellenregister mit den Bibelstellen des
       Buchs in biblischer Reihenfolge (Gen vor Ps vor Mt …, je Buch nach Kapitel und Vers), danach die Glossarseite
       mit „blinder Fleck“ und „Scham“ samt Erklärung.
    8. **EPUB:** Werk → „Als EPUB herunterladen“ bzw. Ansicht → „EPUB-Vorschau“. Erwartet: am Ende eigene Dateien
       „Stichwortregister“, „Bibelstellenregister“ und „Glossar“; die Einträge der Register verlinken auf die
       Stellen im Text, die Glossarseite zeigt beide Begriffe.
    9. **Rückmeldung:** je Schritt „ok“ oder Abweichung mit Seite/Abschnitt und Bildschirmfoto (Issue oder Notiz im
       Repo).
- **Runde 20 „EPUB-Vorschau“ abgeschlossen** (2026-11-09 13:55 – ≈ 15:45, Beginn nach `aeab543`; Schnittstelle
  R20-S1 `8da1f7a` direkt auf `development`, Zweige `team/epub` `e4c85c1`, `team/reader` `d06bb4d`,
  `team/epub-pruefung` `605dea6` mit `--no-ff` in dieser Reihenfolge, **ohne Konflikte** (`7b4e368` `2402601`
  `00ff0d5`); Integrationscommit `b1a8788`, Nachbesserung und PM-Abschluss in einem Commit danach; nicht gepusht;
  Worktrees `wf_7850dc30-9bc-3/4/5` samt `target/` entfernt, team-Zweige bleiben).
  - **Gutachten** (15 Befunde: 7 mittel, 8 niedrig). Nachbesserung: Endnoten-Eintrag auf oberster Ebene, Label in
    Fußnote im Endnoten-Modus, `READER_PUBLISHER_FONT`-Fehlalarm, 403 der Prüfseite für fremde Seiten (je ein Test),
    Handprüfung mit neun Schritten, Wunsch „Einleitung blinder Fleck“ festgehalten, EPUB-Cover ohne Titel als „noch
    nicht“ (→ R21c). **PM-Abschluss:** `epub_export`/`epub_check` in die Werkzeugtabelle von `docs/MCP.md`;
    Themenfarben `--warning`/`--success`/`--line` statt nicht vorhandener `--warn`/`--ok`/`--border` (Kontrast im
    Dunkelmodus); Begründung an der letzten Zusicherung von `round20_abnahme.rs`; Abweichungen §0.6/§0.7 und die
    Änderungen des Integrators an Dateien von reader/epub-pruefung bestätigt (Backlog R20); Nutzerfrage 6 mit
    PM-Vorgabe (Kindle als Annäherung), neue Nutzerfrage 13 (Rückenbreite); übrige niedrige Befunde → R23a/R23c
    (Backlog „Nach Runde 20 offen“); Fahrplan/Backlog nachgeführt.
  - **PM-Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **171 Läufe „test result: ok“, 2592 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel „1100 passed“); kein ÜBERSPRUNGEN; `cargo fmt --check` sauber; `grep -rn "STUB:" crates` nur in
    Tests (Zusicherungen „kein STUB“).
  - **Was geht:** EPUB 3 (EPUB-2-Rückfall) aus den aktuellen Fassungen: je Teil/Kapitel eine Datei, Fußnoten Popup
    oder Endnoten, Querverweise als Links, Bilder mit alt, Umschlag als Cover, Titelei, Metadaten maskiert,
    Satz-Einstellungen als CSS, gleiche Bytes bei gleichem Stand; Menü Werk → „Als EPUB herunterladen“, RPC
    `work.epub`, CLI `export --epub`, MCP `epub_export`. Vorschau (Ansicht → „EPUB-Vorschau“) mit neun
    Reader-Profilen nebeneinander (Pflicht: Apple Books iPhone/iPad, Google Play Books, „Kindle (über EPUB)“,
    „Kindle-App (über EPUB)“), Nachtmodus, Kapitelgrenzen, ohne JavaScript. Prüfung (`/werke/{id}/epub/pruefung`):
    eigene Struktur-/Barrierefreiheitsprüfung (xmllint), Reader-Warnungen, EPUBCheck aus PATH oder eingestelltem
    epubcheck.jar (Einstellungen → Daten; nie Download, nur der Mensch startet es). Buchangaben-Reste: ISBN-Präfix,
    `paper_sheet_mm`, Commit „Umschlag entfernt“, Druckerei-Umschlag mit Textregeln der Vorlage. Keine Migration.
  - **Integrator:** `instructions` nennen `epub_export`/`epub_check`; S1-Ausnahme in `tests/mcp.rs` entfernt; Prüfseite
    in `round9_menu` (42 Seiten); `cover.rs`-Modulkopf; S1-Stand-Sätze der Doku ersetzt; nach dem Zusammenführen drei
    Testerwartungen an R20a angepasst (nummerierte Titel, Bilder im Testwerk). `grep -rn "STUB:" crates` außerhalb der
    Tests: **leer**.
  - **Sichtprüfung** (Wegwerf-Server, `--secrets memory`, Verbindungen auf 127.0.0.1:9, keine Anfrage an Eliezer oder
    ins Netz; Bilder `.claude/sicht/r20/`): `vorschau-zwei-reader-hell`/`-dunkel` (Apple Books iPhone + Kindle, Bild
    und Popup-Fußnoten), `inhaltsverzeichnis` (nav mit Teilen/Kapiteln), `fussnote-sprung-tolino`,
    `kapitel-neue-seite` (Teil-Seite eigene Datei, Grenzen markiert), `querverweis-stuerme`, `pruefbericht` („Keine
    Fehler“, „EPUBCheck nicht gefunden“, Knopf), `menu-ansicht-epub`, `menu-werk-epub`. **Behoben:** Bilder und
    `book.css` luden im Rahmen nicht (Firefox kürzt den `Referer` aus `sandbox`-Rahmen auf die Herkunft;
    `from_own_frame` nimmt den gekürzten Referer desselben Hosts an, Test); E-Ink-Profile mit Graustufen am `body`.
  - **Abnahme** `tests/round20_abnahme.rs` (7 Tests, „Das Feuer am Kap“, frei erfunden).
  - **Lauf:** nach `cargo clean -p` aller 15 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **171 Läufe „test result: ok“, 2588 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel 1099); `cargo fmt --check` sauber; xmllint vorhanden (kein ÜBERSPRUNGEN).
  - **Offen für den PM:** Fahrplan/Backlog (R20 erledigt, Abweichungen §0.6 Druckerei-Umschlag nach R20c und §0.7
    `paper_sheet_mm` ohne Migration), Nutzerfragen: Vorgabe 0,2 mm je Blatt, Java für echten EPUBCheck-Lauf,
    Hersteller-Reader auf Android. Niedrig: `check_mimetype` prüft den Inhalt eines *komprimierten* mimetype nicht
    (nur „komprimiert“ wird gemeldet).
  - **Nachbesserung nach Gutachten** (2026-11-09): Endnoten-Eintrag „Anmerkungen“ im Inhaltsverzeichnis auf oberster
    Ebene (nicht mehr unter dem letzten Teil; nav und NCX); Verweis `@name` auf eine Marke in einer Fußnote zeigt im
    Endnoten-Modus auf `notes.xhtml` (kein toter Link mehr); `READER_PUBLISHER_FONT` nur, wenn das Werk eine Schrift
    einstellt (der Grundstil zählte mit); `GET /werke/{id}/epub/pruefung` lehnt fremde Seiten mit 403 ab. Je ein Test.
    EPUB-Cover ohne Titel der Umschlag-Anordnung als „noch nicht“ in Bedienung und Backlog („Nach Runde 20 offen“,
    → R21c). Nutzerwunsch „Einleitung: der blinde Fleck“ (Biologie des Auges → Analogie des Sehens → Seele des
    Menschen) als Buchtext-Wunsch im Backlog „Nach Runde 20 offen“ festgehalten – nur als Vorschlag umsetzbar, kein
    Agent schreibt am echten Werk.
  - **Offen für den Nutzer (Buchtext):** Einleitung „Der blinde Fleck“ – Gliederungspunkt anlegen (oder von einem
    Assistenten per `outline_add` anlegen lassen) und den Text selbst schreiben oder von einer Persona als Vorschlag
    erarbeiten lassen (`ask_persona` mit `propose`); übernehmen nur Sie.
  - **Handprüfung durch den Nutzer mit Werk 3** (Dienst 127.0.0.1:8765, keine Migration; trotzdem **vorher
    Sicherung** von `~/.local/share/baruch` – das Löschen des Umschlagbilds schreibt jetzt einen eigenen Commit):
    1. **Herunterladen:** Werk 3 → Werk → „Als EPUB herunterladen“. Erwartet: `<slug>.epub`, ein zweiter Download
       ergibt dieselben Bytes (z. B. `sha256sum` beider Dateien gleich).
    2. **Am Rechner öffnen** in einem eigenen Reader (Thorium Reader oder calibre-Betrachter). Erwartet: Umschlagbild
       als erste Seite, Titelseite/Impressum/Widmung (bei eingeschalteter Titelei), Inhaltsverzeichnis mit Teilen und
       Kapiteln in Gliederungsreihenfolge, jedes Kapitel auf neuer Seite, Fußnoten anklickbar mit Rücksprung,
       Querverweise springen ans Ziel, Bilder mit Bildunterschrift.
    3. **Endnoten:** Download mit `/werke/3/epub?fussnoten=endnoten`. Erwartet: „Anmerkungen“ am Ende und im
       Inhaltsverzeichnis auf oberster Ebene (neben den Teilen, nicht unter dem letzten).
    4. **Vorschau:** Ansicht → „EPUB-Vorschau“. Erwartet: Pflicht-Reader nebeneinander, Bilder im Rahmen sichtbar,
       Nachtmodus dunkel, Kapitelgrenzen in der Dateiliste; auch ohne JavaScript bedienbar („Anzeigen“).
    5. **Prüfseite** `/werke/3/epub/pruefung`. Erwartet: keine Fehler; als Hinweise/Warnungen bei Werk 3
       voraussichtlich `TEMPLATE_PAGEBREAK_MAPPED` (Kapitelregel der Werkvorlage auf die Kapiteldateien abgebildet),
       `ISLAND_OMITTED` für Typst-Inseln (`#set`-Zeilen, eigene Funktionen), `IMAGE_NO_ALT`/`IMG_ALT_EMPTY` für Bilder
       ohne Alternativtext – diese Bilder unter Bilder mit Alternativtext versehen. `READER_PUBLISHER_FONT` nur, wenn
       in den Satz-Einstellungen eine Schrift steht. Bitte jede Meldung notieren, die unerwartet ist.
    6. **EPUBCheck (optional, Java nötig):** epubcheck.jar unter Einstellungen → Daten eintragen, auf der Prüfseite
       „EPUBCheck ausführen“. Erwartet: „bestanden“ oder Meldungen „EPUBCheck: …“ mit Pfad; ohne Jar ehrlich
       „EPUBCheck nicht gefunden“.
    7. **Apple Books / Google Play Books:** EPUB per AirDrop/Dateien bzw. play.google.com/books → „Hochladen“.
       Erwartet: Cover in der Bibliothek (nur das Bild – Titel aus dem Umschlag-Editor fehlt noch, siehe Backlog),
       Popup-Fußnoten bei Apple Books, Inhaltsverzeichnis wie in Schritt 2.
    8. **Send to Kindle:** EPUB per Mail an die Send-to-Kindle-Adresse (Betreff leer) oder mit der Send-to-Kindle-App
       bzw. amazon.com/sendtokindle. Erwartet: nach einigen Minuten in der Bibliothek von Kindle-Gerät und
       Kindle-App; Amazon wandelt um, Schrift ist Bookerly bis „Schrift des Verlags“ gewählt wird; Fußnoten als
       Popup oder Sprung; schlägt die Umwandlung fehl, kommt eine Mail von Amazon (Text bitte aufheben).
    9. **Rückmeldung:** je Schritt „ok“ oder Abweichung mit Gerät/App, Seite/Kapitel und Bildschirmfoto, am besten
       als Issue oder Notiz im Repo; unerwartete Prüfmeldungen mit Code.
- **Runde 19 „RAG-Pflege, Reste der Stellen“ abgeschlossen** (2026-11-09 08:54 – ≈ 11:20; Schnittstelle R19-S1 `800da6a`
  direkt auf `development`, Zweige `team/r19-index` `dc363fd`, `team/r19-pgvector` `df6a93a`, `team/r19-treffer`
  `6333449` mit `--no-ff` in dieser Reihenfolge, **ohne Konflikte** (`ba32ec2` `afb8400` `d8af5ec`); Integrationscommit
  `8cced30`, Nachbesserung `ec25ac9`, PM-Abschluss; nicht gepusht; Worktrees `wf_c758e4da-8a5-3/4/5` samt `target/`
  entfernt, team-Zweige bleiben).
  - **Gutachten** (11 Befunde: 3 mittel, 8 niedrig). Nachbesserung `ec25ac9`: die drei mittleren (unterbrochener
    Auftrag eines anderen Werks, TLS bei `require`, Handprüfung mit Schritten). **PM-Abschluss:** veralteter
    Stub-Vermerk zur Gedankenkarte in `docs/Architektur.md` berichtigt, rohe U+2028/U+2029 im Regex-Hinweis von
    `SKILLS.md` durch den Text `\u2028`/`\u2029` ersetzt; MCP `idea_search` mit `mode: "rag"` (nicht bestellt, nur
    lesend) als PM-Vorgabe im Backlog; Erledigt-Eintrag R29 nachgetragen; übrige niedrige Befunde → R21a/R23a/R24
    (Backlog „Nach Runde 19 offen“); Fahrplan/Backlog nachgeführt.
  - **PM-Lauf:** nach `cargo clean -p` aller 14 Workspace-Crates (10,9 GiB entfernt) auf Stand `ec25ac9`
    `nice -n 10 ionice -c3 cargo test --workspace -j 4 --no-fail-fast`: **162 Läufe „test result: ok“, 2457
    bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0 (Server-Bündel „1061 passed“); kein ÜBERSPRUNGEN;
    `cargo fmt --check` sauber; `grep -rn "STUB:" crates` nur `cover.rs:65` und `base.html:103` (EPUB, R20).
  - **Was geht:** Index ohne Versuchszyklus je Suche, ehrlicher Status (`index.stale`, „n Aufträge liegengeblieben …“
    in Einstellungen → Verbindungen), „Index neu aufbauen“ antwortet sofort (`deferred`), Einfälle gelöschter Werke
    sofort im gemeinsamen Bereich, Start mit eigenem Datenordner ohne gespeicherte Verbindung Eliezer ohne
    Einbettungsanfrage, Abgleich mit MultiGPT gehärtet; Aufgaben-Präfixe (LanceDB Anfrage/Dokument, pgvector Anfrage),
    pgvector kürzt Felder in SQL, quotet Schemanamen, TLS-Hinweis; „schwach“ (unter 0,45) in Literatur, Einfällen,
    Belegen, `rag.search`/`rag_search`, `idea.search` (`weak`, Ausfall -32001); Gedankenkarte „Gruppen benennen
    (Assistent)“ als Vorschlag, Übernehmen nur Mensch (Formular), MCP `idea_map` mit `near_limit`/`near_threshold`;
    Menüband „Bibelstelle“/„Literaturverweis“ aus der Vorlage; Wortlaut gegen Module mit Silbentrennresten
    („Wortlaut stimmt (Leerraum im Modul)“, `module_note`), „Mal“ = Maleachi vor Kapitel und Vers, Zitat-Bindeglied,
    Lemma-Probe der Lexika, NFC. Keine Migration, keine neue Route, kein neues MCP-Werkzeug; Karten-Stubs entfernt
    (`grep -rn "STUB:" crates` zeigt nur `cover.rs`/`base.html` EPUB).
  - **Integrator:** Kurzbeschreibung von MCP `idea_search` nennt `mode` (offener Punkt von treffer). Doku
    „### Integration“ in RPC/MCP/Bedienung/Architektur/SKILLS, Plan, Backlog-Stand.
  - **Abnahme** `tests/round19_abnahme.rs` (5 Tests, „Die Laternen am Kai“, nur Nachbauten): Präfixe in LanceDB über
    `connection.set`, schwache Treffer auf Literaturseite, `/einfaelle`, `idea.search` und im Belegblock von @belege
    (`persona.review` mit `ScriptedModel`); scheiternder Einbetter → `stale` auf der Seite, fünf Suchen ohne neuen
    Einbettungsversuch, `rag.reindex` während hängender Einbettung < 2 s mit `deferred`, danach geheilt; gelöschtes
    Werk → Einfall sofort „ohne Werk“ auffindbar; pgvector-Container: Tabelle in einem Schema mit Anführungszeichen
    und SQL im Namen, Treffer über `rag.search` gekürzt (500 Zeichen), Anfrage mit `query_prefix`, `weak`, Opfertabelle
    steht; Gruppen benennen per Formular, Vorschlag ohne Commit, MCP ohne Übernahme, Übernahme per Formular mit
    Herkunft Mensch; erfundenes SWORD-Modul mit Trennresten: `match` + `module_spacing`, `module_note`, Prüfanzeige,
    MCP `scripture_check`, „Joh 3,16 Mal 3,20“ ergibt beide Stellen.
  - **Lauf:** nach `cargo clean -p` aller 14 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **162 Läufe „test result: ok“, 2456 bestanden, 0 fehlgeschlagen, 2 ignoriert**, EXIT 0
    (Server-Bündel 1060); `cargo fmt --check` sauber; pgvector-Container-Tests liefen (kein ÜBERSPRUNGEN), Container
    entfernt.
  - **Sichtprüfung** (Wegwerf-Server auf freiem Port, `--secrets memory`, eigener Modell-Nachbau auf 127.0.0.1 für
    Einbettungen und Chat, MultiGPT auf 127.0.0.1:9, keine Anfrage an Eliezer oder ins Netz): Bilder `.claude/sicht/r19/`
    – `rag-status-neuaufbau` (Neuaufbau läuft, „2 Aufträge ausstehend“), `rag-status-liegengeblieben` (Backend weg:
    „1 Auftrag liegengeblieben … Suchen stoßen keinen neuen Versuch an“), `verbindungen-lancedb-praefixe`,
    `literatur-schwach`, `einfaelle-schwach` (aufgeklappte `<details>`), `pruefung-leerraum-im-modul` (Hinweis zum
    Modul, „Wortlaut stimmt (Leerraum im Modul)“, Joh 3,16 und Mal 3,20), `karte-gruppen-vorschlag`,
    `karte-gruppen-liste`. Nichts zu beheben.
  - **Nachbesserung nach dem Gutachten:** Ein vom Neuaufbau eines Werks unterbrochener Auftrag eines anderen Werks
    (oder das Aufräumen) kommt wieder in die Warteschlange (vorher blieb das andere Werk halb abgeglichen, ohne
    `pending`/`stale`; Test `reindexing_one_work_requeues_the_interrupted_job_of_another_work`). TLS-Hinweis auch bei
    `require` (sqlx prüft dort das Zertifikat nicht), empfohlen wird nur noch `verify-full`.
  - **Handprüfung durch den Nutzer** nach dem Einspielen (Dienst 127.0.0.1:8765; keine Migration, trotzdem **vorher
    Sicherung** von `~/.local/share/baruch` oder „Sicherung jetzt“ – Übernehmen in der Gedankenkarte schreibt Commits in
    den Spiegel, „Index neu aufbauen“ legt den LanceDB-Index im Datenordner neu an):
    1. **RAG-Status bei kurz fehlendem Eliezer:** Eliezer kurz stoppen (oder das Einbettungsmodell entladen), etwas am
       Buch ändern; Einstellungen → Verbindungen zeigt bei LanceDB „n Aufträge liegengeblieben …“. Mehrmals in Literatur
       oder Einfällen suchen: Die Suche meldet LanceDB ehrlich als ausgefallen, und Eliezer bekommt dabei **keine**
       neuen Einbettungsanfragen (im Protokoll von Eliezer nachsehen). Eliezer wieder starten, eine Änderung oder
       „Index neu aufbauen“ holt die Aufträge zurück; danach steht „liegengeblieben“ nicht mehr da.
    2. **Index neu aufbauen am echten Werk:** Knopf „Index neu aufbauen“ antwortet sofort (läuft gerade ein Auftrag:
       Rücksetzwunsch, `deferred`), der Status zeigt „n Aufträge ausstehend“. Der Neuaufbau bettet **das ganze Werk
       neu über Eliezer** ein (eine Anfrage zur Zeit, je 32 Texte) – bitte festhalten, wie lange Eliezer danach
       belastet ist, bis „ausstehend“ verschwindet.
    3. **Präfixe für nomic-embed-text** (Einstellungen → Verbindungen, LanceDB: Anfrage `search_query: `, Dokument
       `search_document: `, Leerzeichen am Ende gehört dazu): Ein geändertes **Dokument-Präfix legt LanceDB still**
       (Status „ausgefallen“, „Aufgaben-Präfix geändert; Index neu aufbauen.“), bis „Index neu aufbauen“ den ganzen
       Index neu aufgebaut hat – also erst eintragen, wenn Eliezer die Zeit dafür hat (siehe 2.).
    4. **Eigenes pgvector:** „Verbindung prüfen“ – das Anfrage-Präfix muss zur Einbettung passen, mit der die Daten
       in der Tabelle eingebettet wurden (bei nomic-embed-text mit `search_document: ` also `search_query: `, sonst
       leer lassen); Sammlungen und Modelle erscheinen gekürzt (200 Zeichen), Treffertexte höchstens 500 Zeichen;
       Tabelle in einem eigenen Schema (`schema.tabelle`) wird gefunden. TLS-Hinweis: bei Passwort mit `prefer` oder
       `disable` deutlich, bei `require` milder (verschlüsselt, aber Zertifikat ungeprüft), bei `verify-full` keiner –
       `verify-full` braucht ein Zertifikat, dem der Rechner vertraut, und den passenden Rechnernamen.
    5. Prüfmodul mit Trennresten in „Zitate prüfen“ (Hinweis und „Leerraum im Modul“, die Faustregel ist gegen das
       echte Modul ungeprüft).
    6. MultiGPT-Annahmen (Rechte-Wörter in `isError`, `upload_document` ohne ID, stilles Kappen von `limit`) gegen das
       echte MultiGPT.
    7. Gedankenkarte „Gruppen benennen (Assistent)“ mit dem echten Lektor; Übernehmen schreibt einen Commit.
- **Runde 29 „Buchsatz in den Einstellungen“ abgeschlossen** (2026-11-09 06:16 – ≈ 09:15; Schnittstelle R29-S1
  `09f4f9d` direkt auf `development`, Zweige `team/satz-daten` `3f4ee48`, `team/satz-typst` `fc81f39`,
  `team/satz-oberflaeche` `d54255a` mit `--no-ff` in dieser Reihenfolge, ohne Konflikte (`aa2fb29` `cc1b386`
  `b770eae`); Integrationscommit `d39fc3f`, Nachbesserung `066ab17`, PM-Abschluss; nicht gepusht; Worktrees
  `wf_35540f3d-793-3/4/5` samt `target/` entfernt, team-Zweige bleiben).
  - **Gutachten** (17 Befunde: 1 hoch, 3 mittel, 13 niedrig). **Nachbesserung `066ab17`:** Leerseite vor einem Teil
    ohne Einstellung „Teil-Seite“ trug Seitenzahl und Kolumnentitel (hoch; `part_page_rule_for`), Messtests prüften
    Leerseiten nicht wirklich (`blank()` nach Satzspiegel, neuer Test mit zweitem Teil, Mutationsprobe), „Vorlagenregel
    übernehmen“ verlor im Export den Umbruch vor Teilen (die Baruch-Kapitelregel ist jetzt wörtlich die übernehmbare
    Regel – im Export bricht ein Teil nun wie ein Kapitel um, Buchsatz unverändert), Satzbegriffe Bundsteg,
    Schusterjunge/Hurenkind und Kolumnentitel im Block und in Bedienung.md erklärt. **PM-Abschluss:** Rundung der
    Zahlen ohne Gleitkomma-Reste (5,3 mm bleibt 5,3), „Gespeichert …“ in Ortszeit, Fehler-/Hinweisfarben über
    `--danger`/`--warning`, Hinweis `NO_DOCUMENT_TITLE` erkennt `#set document(` über mehrere Zeilen und ignoriert
    Kommentare, Kopf von `docs/Austauschformat.md` (u6), RPC.md zu `work.typeset_adopt_rule` (Bestätigung ist Sache des
    Aufrufers wie bei `work.typeset_reset`), Backlog-Abweichungen von R29, Ersatzweg in der Handprüfung;
    Fahrplan/Backlog nachgeführt. Übrige niedrige Befunde → R23a (Backlog „Nach Runde 29 offen“).
  - **PM-Lauf:** nach `cargo clean -p` aller Workspace-Crates (13,7 GiB entfernt) auf Stand `066ab17`
    `nice -n 10 ionice -c3 cargo test --workspace -j 4 --no-fail-fast`: **160 Läufe „test result: ok“, 2383 bestanden,
    0 fehlgeschlagen, 2 ignoriert**, EXIT 0 (Server-Bündel „1021 passed“). Nach den PM-Änderungen
    `cargo test -p baruch-typst -p baruch-server -p baruch-render -p baruch-store -p baruch-exchange`: 112 Läufe, 2076
    bestanden, 0 fehlgeschlagen, EXIT 0 (neu: `normalized_values_carry_no_floating_point_residue`,
    `document_title_is_found_across_lines_and_not_in_comments`); `cargo fmt --check` sauber; keine R29-Stubs.
  - **Was geht:** Einstellungen → Werk → Satz (ohne JavaScript, Vorschau mit Mustertext, Widget ohne Neuladen):
    Kapitelanfang, Teil-Seite, Seitenformat, Ränder mit Bundsteg, Schrift, Größe, Zeilenabstand, Einzug/Abstand,
    Blocksatz, Silbentrennung/Sprache, Seitenzahlen (Lage, ab erstem Kapitel), Kolumnentitel, Fußnoten,
    Schusterjungen/Hurenkinder, „Verweis mit Seitenzahl“. Regelblock „// Baruch: Satz“ hinter der Werkvorlage (Haken
    `snapshot::head`) in Export, Spiegel, Satz, PDF und Umschlag; ohne Einstellungen byte-gleich. Hinweise bei
    Widerspruch zur Werkvorlage (Block und Satz-Seite), Angebot „Kapitelanfang aus der Werkvorlage übernehmen“ (neue
    Vorlagenfassung, ein Commit). RPC `work.typeset_get|set|reset|adopt_rule|preview`, MCP `typeset_settings` nur
    lesend. Migration 0048 (`work_typeset`, Grabsteinart), Austausch Unterversion 6 (`<satz>`).
  - **Integrator:** Befund behoben – ohne Kapitelregel hielt der Regelblock alle Seiten zwischen zwei
    Kapitelüberschriften für leer (keine Seitenzahl, kein Kolumnentitel), weil die Marke `<baruch-satz-end>` dann ihrer
    Überschrift in der Abfrage folgt (`rules.rs` `helpers`, Test `without_a_chapter_rule_every_text_page_keeps_number_and_head`
    in `baruch-render`). Hinweis `NO_DOCUMENT_TITLE` bei Kolumnentitel „Werktitel“ ohne `#set document(title: …)`;
    Austauschseite nennt die Satz-Einstellungen; Vorschau zeigt „Ungespeicherte Werte“ nur bei Abweichung vom
    gespeicherten Stand; Vorschaubilder haben vor dem Laden schon Seitengröße (`aspect-ratio`). Doku: Austauschformat
    (u6 maßgeblich, Grabsteinart `satz`, „heute 6“), Bedienung (Querverweise), MCP (`exchange_export` Unterversion 6),
    RPC (Hinweise), Architektur/SKILLS „Integration“, Plan, Backlog „Nach Runde 29 offen“. Die fünf älteren Tests mit
    `version="6"` (satz-daten) sind bestätigt.
  - **Abnahme** `tests/round29_abnahme.rs` (8 Tests, „Fährhausbuch“ mit Titelei, 2 Teilen, 4 Kapiteln, Fußnoten,
    Querverweis): ohne Einstellungen unverändert (Setzen + Zurücksetzen per Formular → Export und Satzquelle
    byte-gleich); Formular speichern → 303 → `work.typeset_get` → Export/`main.typ` mit Block → PDF aus derselben
    Quelle gemessen (A5 auf jeder Seite auch per `pdfinfo`, Kapitel rechts, jede Textseite mit Zahl außen ab dem ersten
    Kapitel, Kolumnentitel = laufendes Kapitel, Bundsteg 25/15 mm, „S. n“ = gedruckte Zahl auch per `pdftotext`),
    Umschlag A5 mit Rücken aus der neuen Seitenzahl; jede Einstellung einzeln über den Server (12 × 19, eigenes Format,
    neue Seite, Schrift/Größe, Einzug/Abstand, Zeilenabstand, Silbentrennung, verborgene Zahlen, Werktitel, Fußnoten
    a–d bzw. je Kapitel); Vorlagenregel: Angebot, 422 ohne Häkchen, Übernahme mit einem Commit, Kapitelseiten vorher =
    nachher; Konflikt `#set page(paper: "a4")` in Block und Satz-Seite, PDF A5; MCP liest, kein Schreibweg; Austausch
    u6 über drei Instanzen mit Grabstein.
  - **Lauf:** nach `cargo clean -p` aller 14 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **160 Läufe „test result: ok“, 2379 bestanden, 0 fehlgeschlagen, 2 ignoriert** (Goldstand-Schreiber),
    EXIT 0 (Server-Bündel 1020); `cargo fmt --check` sauber; keine R29-Stubs (`grep -rn "STUB:" crates`).
    Nach den Änderungen aus der Sichtprüfung `cargo test -p baruch-server -p baruch-typst -p baruch-render`: 1567
    bestanden, 0 fehlgeschlagen, EXIT 0; `cargo fmt --check` sauber.
  - **Sichtprüfung** (eigener Server auf freiem Port, `--secrets memory`, Verbindungen auf 127.0.0.1:9, keine Anfrage an
    Eliezer oder ins Netz): Bilder `.claude/sicht/r29/` – `einstellungen-satz-vorschau-hell`/`-dunkel` (Block mit
    Konflikt-Hinweis beim Seitenformat, Angebot zur Vorlagenregel, 13 Vorschauseiten), `einstellungen-satz-angebot`,
    `satz-druckpruefung` (Konflikt und Angebot auf der Satz-Seite), `satz-kapitel-rechts` (Seite 7), `satz-kolumnentitel`
    (Seite 8), `pdf-doppelseite-8-9` (Kolumnentitel, Zahlen außen, Bundsteg). Behoben: „Ungespeicherte Werte“ bei
    unveränderter Vorschau, Vorschaubilder ohne Platzhalter. Auffällig, nicht behoben: `pdfinfo` meldet zum Buch-PDF
    „Syntax Error: Suspects object is wrong type (boolean)“ (Ausgabe von typst-pdf, das PDF ist lesbar).
  - **Handprüfung durch den Nutzer** nach dem Einspielen (Dienst 127.0.0.1:8765; **vorher Sicherung** von
    `~/.local/share/baruch` oder „Sicherung jetzt“ – beim ersten Start läuft Migration 0048: neue Tabelle `work_typeset`
    und Umbau von `uuid_tombstone` wie 0046, verlustfrei getestet in `migration_0048.rs`; ohne Einstellungen ändern sich
    Spiegel und Satz nicht):
    1. Werk 3: Einstellungen → Werk → Satz zeigt das Angebot zur Show-Regel aus Vorlagenfassung 2 (Kapitelanfang
       „neue rechte Seite“); Häkchen setzen, „Als Einstellung übernehmen“ → neue Vorlagenfassung ohne die Zeile, ein
       Commit; im Satz stehen die Kapitel weiter auf rechten Seiten. **Erscheint kein Angebot** (erkannt wird nur eine
       Regel, die ausschließlich aus Umbruch und Überschrift besteht): Kapitelanfang „neue rechte Seite“ von Hand
       wählen und speichern; die Show-Regel bleibt dann als Hinweis „Die Werkvorlage setzt … selbst“ stehen und lässt
       sich über eine neue Vorlagenfassung (Einstellungen → Werk → Werkvorlage) entfernen. Bitte den Wortlaut der Regel
       melden, damit die Erkennung ihn aufnimmt.
    2. Seitenformat (z. B. A5), Ränder mit Bundsteg, Seitenzahlen „unten außen“ ab dem ersten Kapitel und Kolumnentitel „Kapitel“
       wählen, „Vorschau“, dann „Speichern“; Satz-Seite und PDF prüfen; Umschlag-Rücken folgt der neuen Seitenzahl.
    3. Setzt die Werkvorlage etwas selbst (z. B. `#set page(…)`), steht ein Hinweis beim Feld und unter Druckprüfung.
    4. Menüband „Verweis“: Häkchen „mit Seitenzahl“ startet mit der Werk-Einstellung.
- **Runde 30 „Suchen/Ersetzen, Querverweise, Chat: Lösungsvorschläge, Varianten, Bedienung“ abgeschlossen**
  (2026-11-09 03:09 – ≈ 06:35; Schnittstelle R30-S1 `aa6808c` direkt auf `development`, Zweige
  `team/suchen-verweise` `4a209fc`, `team/chat-loesungen` `d3a2a8f`, `team/chat-bedienung` `6d92305` mit `--no-ff` in
  dieser Reihenfolge, ohne Konflikte (`29a7efa` `37240d1` `62947a8`); Integrationscommit `0043b88`, Nachbesserung
  `88df145`, PM-Abschluss; nicht gepusht; Worktrees `wf_467ea785-a97-3/4/5` samt `target/` entfernt, team-Zweige
  bleiben).
  - **Gutachten** (15 Befunde: 0 hoch, 6 mittel, 9 niedrig). **Nachbesserung `88df145`:** Abgleich (Merge) schlüsselt
    Querverweise vor dem Vergleich um (keine Scheinabweichung mit fremden `<kN>`), Lösung nach einer Leerzeile gehört
    weiter zum kritischen Punkt, MCP-Relay im Abschnitts-Chat nur eine Stufe und gerahmt, Ersetzen-Vorschau mit
    8-MiB-Budget, Kürzung auf 2000 Stellen und 403 bei fremder Herkunft per GET, `/suche` mit Bereich „Markierung“
    und Wahl der Fassung (bleibt in allen Formularen erhalten), Editor-Knopf „⌶“ (Alt+L). **PM-Abschluss:**
    doppelte `picks` in `solution.adopt` zählen einmal, Varianten-Etiketten rücken bei einem Ausfall nicht mehr nach,
    Querverweis-Farbe im dunklen Thema (`--primary`), Werkzeugtabelle `text_search` (MCP.md), Backlog (Alt+→ =
    nächste Fassung), Fälle aus Werk 3 in der Handprüfung unten; Fahrplan/Backlog nachgeführt. Übrige niedrige
    Befunde → R23a (Backlog „Nach Runde 30 offen“). Voller Lauf nach `cargo clean -p` aller 14 Crates vor den
    PM-Änderungen (Stand `88df145`): 154 Läufe ok, 2262 bestanden, 0 fehlgeschlagen, 1 ignoriert, EXIT 0; danach
    `cargo test -p baruch-server`: 295 + 989 bestanden (neuer Test `a_doubled_pick_sets_its_solution_only_once`),
    `cargo fmt --check` sauber; keine R30-Stubs.
  - **Integrator:** MCP `ask_persona` liefert `basis`, `post_delete {thread}` löscht eigene Fadenbeiträge
    (`threads::delete_post_as`), Stub-Vermerke von `text_search`/`solution_variants` aus Werkzeugtabelle und
    `initialize` entfernt; `round9_menu::all_pages` um `/suche`, `/verweise`, `/varianten` (40 Seiten);
    `basis_select` bekam im Chat die ganze Fassung statt der Nummer (der ganze Text stand zweimal im Auswahlfeld –
    `round18_satzbild` fiel darüber); Befundtext im Chat nicht mehr doppelt (`finding_block(card, body)`); doppelte
    Lösungen (unter dem Befund und unter „Mein Vorschlag:“) nur einmal in der Lösungsliste (Unit-Test); Chat-Eingabe
    bricht um (Feld, darunter „als Auftrag“, Prüfgrundlage, Senden); kein `pre-line` um Befund-Blöcke. Tests von
    chat-bedienung nachgezogen, die erst mit den anderen Zweigen brachen: `round30_bedienung` (Lösungszeilen in
    Skripten wegen der Lösungspflicht, `&at=` in Sprung-Adressen aus `check_marker`), `round30_live` (Listenpunkte sind
    Befund-Blöcke). Doku: MCP.md, Bedienung (Menü), Architektur (`finding_block`), Plan, SKILLS „Runde 30
    (Integration)“, Store-Moduldoku zu `export_typst`, Backlog „Nach Runde 30 offen“.
  - **Abnahme** `tests/round30_abnahme.rs` (6 Tests, Küstenbuch mit 3 Teilen, 12 Kapiteln, 48 Abschnitten, 6 Personas,
    ScriptedModel): Regex-Suche = Trefferliste auf `/suche`, Zeitgrenze, 50 Abschnitte ersetzt (50 Fassungen vom
    Menschen, ein Commit, Bibelzitat unverändert), Rückgängig per Formular (ein Commit); Verweis über `xref.targets`
    → PDF mit internem Link und „die Einleitung, S. n“ (`pdftotext`), Export mit `<kN>`, Einlesen als neues Werk und
    Austausch u5 in eine zweite Instanz schlüsseln um, `xref.convert` per Formular, kaputter Verweis in der
    Druckprüfung; RFC mit allen sechs Personas (Nathan einmal nachgefordert → „(ohne Lösungsvorschlag)“, alle Köpfe
    „geprüft: aktuelle Fassung 1“, nur der Lektor sammelt, ein Sammel-Vorschlag), „Als Vorschlag übernehmen“ mit 2
    picks per Formular samt Statusvorschlägen, „Lektor, … Varianten“ im Chat → 2 Geschwister, `solution.variants`
    count 3 → A/B/C auf gleicher Basis, „übrige verwerfen“ per Formular; Chat: „Lektor, was meinst du?“, Relay
    „@schrift:“ genau einmal mit „antwortet auf Beitrag n“, Befund-Block mit Lösung und Knopf, geprüfte Sprungmarke
    als Link und ungeprüfte als Text, `<script>` maskiert, `post.since` mit „schreibt …“ und neuen Beiträgen, ↻ per
    Formular mit `repeat_of` und „erledigt?“-Vorschlag, ✕ mit einem Commit; alle neuen Formulare 403 bei fremder
    Herkunft; MCP `ask_persona` mit `basis`, `post_delete {thread}` nur eigene.
  - **Lauf:** nach `cargo clean -p` aller 14 Workspace-Crates `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **154 Läufe „test result: ok“, 2257 bestanden, 0 fehlgeschlagen, 1 ignoriert**, EXIT 0; nach den
    Änderungen aus der Sichtprüfung `cargo test -p baruch-server`: 295 + 984 bestanden, 0 fehlgeschlagen;
    `cargo fmt --check` sauber; `grep -rn "STUB:" crates`: keine R30-Stubs.
  - **Sichtprüfung** (eigener Server auf freiem Port, `--secrets memory`, Modell-Nachbau in Python auf 127.0.0.1,
    `multigpt` auf 127.0.0.1:9, keine Anfrage an Eliezer oder ins Netz): Bilder `.claude/sicht/r30/` –
    `suchleiste-strg-h` (Strg+H, Ersetzen-Zeile offen, Treffer im Satzbild markiert), `werk-suche-seitenleiste`
    (Strg+Umschalt+F, Trefferbaum), `werk-suche` und `ersetzen-vorschau` (`/suche` mit Gliederungs-Häkchen und Diff je
    Abschnitt), `querverweis-satzbild` (Verweis als Block mit „S. …“), `verweise-liste`, `chat-befund-bloecke`
    (Kopf „geprüft: aktuelle Fassung 1“, Befund-Blöcke, Lösung mit „Als Vorschlag übernehmen“, geprüfter und
    ungeprüfter Sprung), `varianten-vergleich` (A/B nebeneinander), `chat-schreibt` („@lektor schreibt …“). Behoben:
    Chat-Eingabe gequetscht, Befund-Blöcke mit Leerzeilen und doppeltem Rahmen, doppelte Lösungen. Auffällig, nicht
    behoben: der Chat behält seine feste Höhe aus R28 (unter den Beiträgen bleibt Platz frei); im Trefferumfeld der
    Werk-Suche wird der Text hinter einem Treffer mitten im Wort abgeschnitten.
  - **Handprüfung durch den Nutzer** nach dem Einspielen (Dienst 127.0.0.1:8765; **vorher Sicherung** von
    `~/.local/share/baruch` oder „Sicherung jetzt“ – beim ersten Start läuft Migration 0047 (nur angehängte Spalten und
    `replace_batch`, verlustfrei getestet in `migration_0047.rs`), und **jeder Spiegel ändert sich einmal** durch die
    Sprungmarken `<kN>` an den Überschriften (ein Commit je Werk durch `sync_all`):
    1. Im Abschnitt Strg+F bzw. Strg+H (Suchleiste oben rechts, Pfeil klappt Ersetzen auf), Enter/F3 springt, Alt+C/W/R
       schalten um; Ersetzen ändert nur den Editortext, gespeichert wird von Hand.
    2. Strg+Umschalt+F: Seitenleiste mit Trefferbaum; „Vorschau und Ersetzen …“ → `/suche`, Abschnitte abwählen,
       „Alle ersetzen“ → je Abschnitt eine neue Fassung, ein Commit; „Rückgängig“ stellt alle zurück. Bibelzitate
       bleiben stehen.
    3. Menüband „Verweis“ → Ziel suchen, mit/ohne Seitenzahl einfügen, speichern; Strg+Klick springt, Alt+← zurück;
       Satz/PDF zeigt „S. n“ als Link; Werkzeuge ▾ → Querverweise prüfen.
    4. „RFC“: alle Köpfe „geprüft: …“ gleich, kritische Punkte mit `statt`/`besser` bzw. Schritt, Lektor „Mein
       Vorschlag“ wird ein Sammel-Vorschlag; „Als Vorschlag übernehmen“; „Lektor, setze die Lösungen als Varianten
       um.“ → Varianten A/B, Vergleich `/varianten`, „übrige verwerfen“.
    5. Chat: „Lektor, …“ ohne @; neue Antworten erscheinen ohne Neuladen, „@lektor schreibt …“; ↻ nach einer Korrektur,
       ✕ löschen (ein Commit); Sprungmarken in Antworten springen in den Text.
    6. Die Anlässe in Werk 3 gezielt nachprüfen: (a) den Satz, der sich auf die Einleitung bezieht (Anlass im Backlog,
       R30, „Zusatz R30a“): Werkzeuge ▾ → `/verweise` → „mögliche Querverweise“ muss ihn anbieten, „umwandeln“ legt
       eine neue Fassung mit `#link(<kN>)` an, Strg+Klick springt zur Einleitung, im PDF „S. n“. (b) Abschnitt key 126
       (Fassung 1 nur Überschrift, Vorschlag 2 mit Text; Beiträge 59–61): RFC erneut – alle Köpfe „geprüft: Fassung 2
       (Vorschlag …)“, kein Befund „Abschnitt leer“, @exeget/@schrift kommentieren nicht den Befund des Lektors.
       (c) In diesem Abschnitt „Lektor, was meinst du?“ ohne @ – Antwort ohne Neuladen.
- **Runde 28 „Befunde: Befundliste, Übersicht übers Werk, Aufträge im Chat“ abgeschlossen** (2026-11-09 00:59 –
  ≈ 03:20, Plan ab 00:45; Schnittstelle R28-S1 `cb7b169` direkt auf `development`, Zweige `team/befunde` `f46a7a5`,
  `team/befund-uebersicht` `5e46999`, `team/chat-auftrag` `07a986f` mit `--no-ff` in dieser Reihenfolge
  (`ee7a5af` `5f3f656` `afe1399`, Konflikt nur in der Werkzeugtabelle von `docs/MCP.md`), danach der
  Integrationscommit `77ef49a`, Nachbesserung `1796940`, PM-Abschluss; nicht gepusht; Worktrees
  `wf_0da9d3cb-892-3/4/5` samt `target/` entfernt, team-Zweige bleiben).
  - **Gutachten** (19 Befunde: 0 hoch, 5 mittel, 14 niedrig). **Nachbesserung `1796940`:** Statusvorschlag aus einem
    Schreibauftrag nur für Befunde des eigenen Abschnitts, Ähnlichkeit nur über Inhaltswörter (Schwelle 0,8), über
    2000 Befunden die neuesten statt der ältesten mit `truncated`/`total` und Hinweis „Gekappt“ auf der Matrix,
    Lektor-Ausnahme im Schreibauftrag (und Lektor-Zeilen aus dem Vorschlag entfernt), Gliederung ohne JavaScript
    (Werkzeuge in eigener Zeile, Titel mindestens 8em). **PM-Abschluss:** Persona im Kopf einer Entscheidungszeile
    entschärft (Einfuhr u4, Test in `round28_chat`), S1-Ausweichzweige in `round28_chat.rs` hart, vier Zusicherungen
    in `round28_uebersicht.rs` begründet, `cargo fmt` über die Nachbesserung, Backlog/Fahrplan/„0107 frei“/Bedienung
    nachgezogen. Übrige niedrige Befunde → R23 (Backlog „Nach Runde 28 offen“). Voller Lauf nach `cargo clean -p`
    aller 14 Crates vor den PM-Änderungen: 146 Läufe ok, 2111 bestanden, 0 fehlgeschlagen, 1 ignoriert; danach
    `cargo test -p baruch-server -p baruch-store`: 70 Läufe ok, 1571 bestanden, 0 fehlgeschlagen.
  - **Integrator:** Platzhalter-Zweige aus `round28_uebersicht` entfernt (Statusformular bis 303 und MCP-Vorschlag mit
    Herkunft jetzt hart geprüft, Fehlerweg 200 mit Hinweis), `round9_menu` nachgezogen (`menu.badges` mit
    `open_findings`, verborgenes Abzeichen am Menü Werk, Seiten `/werke/<id>/befunde`, `/beitraege`, `/beitrag` in
    `all_pages`), Austauschseite zeigt Befunde (offen nach Zählregel 1.5) und alte Beitragsfassungen, Moduldoku der
    Bibliothek nennt 0107, RPC.md ohne S1-Vermerk; Abnahme `tests/round28_abnahme.rs` (2 Tests: RFC mit allen sechs
    Personas → je ein Befund und zwei Sammelvorschläge des Lektors mit `[B<id>]`-Ursprüngen, Beiträge byte-gleich;
    übernommen/verworfen/verschoben über Formulare ohne JavaScript, je ein Commit; nächste Prüfung bekommt die
    Entscheidungen gerahmt, die Wiederholung hängt am verworfenen Befund; dieselbe Zahl in `finding_open_counts`,
    Matrix, Gliederung, Editorkopf, `/gliederung` und Gedankenkarte; „setzt Befund x um“; Lektor ohne „Kritisch:“
    ohne, Nathan mit Nachforderung; Rundreise u4; Schreibauftrag mit Statusvorschlag und Entscheidung, Beitrag in
    Befunde umwandeln, eigener Beitrag bearbeitet mit Verlauf und Commit, Persona-Beitrag nicht, Chat rollt, Faden-RFC
    ergibt Befunde). SKILLS.md „Befunde und Chat (Runde 28)“ um RFC im Test und Statuswege ergänzt.
    Voller Lauf nach `cargo clean -p` aller 14 Workspace-Crates, `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **146 Läufe „test result: ok“, 2107 bestanden, 0 fehlgeschlagen, 1 ignoriert**
    (Goldstand-Schreiber), EXIT 0, keine Warnung; `cargo fmt --check` sauber. `STUB:`: keine R28-Stubs mehr.
    Sichtprüfung (eigener Server auf freiem Port mit `--secrets memory`, Verbindungen auf 127.0.0.1:9, erfundenes
    Brückenbuch; `findings::backfill` zerlegte beim Start 14 Befunde): Bilder `.claude/sicht/r28/` – `matrix-hell`,
    `matrix-dunkel`, `akkordeon-befunde`, `akkordeon-statusformulare` (ohne JavaScript), `chat-lang` (gerollt, am
    neuesten Beitrag, „bearbeitet“, Kopieren/Zitieren), `historie-ohne-js`, `beitrag-eigener`, `lektor-neuer-ton`,
    `fassungsbaum-setzt-befund-um`, `gedankenkarte-befunde`. Zahlen stimmen überall überein (9 offen im Werk, 4 an
    der Brücke). Auffällig nur: ohne JavaScript verdrängte das Abzeichen in der linken Gliederung den Titel (in der Nachbesserung
    behoben, im Browser noch nicht angesehen).
  - **Handprüfung durch den Nutzer mit Werk 3** nach dem Einspielen (Dienst 127.0.0.1:8765; **vorher Sicherung** von
    `~/.local/share/baruch` oder „Sicherung jetzt“ – beim ersten Start läuft Migration 0046 (baut `uuid_tombstone` um,
    verlustfrei getestet in `migration_0046.rs`) und `findings::backfill` zerlegt einmal alle vorhandenen
    Persona-Beiträge, ohne sie zu ändern; Protokollzeile im Journal):
    1. **Werk ▾ → Befunde** (Werk 3). Erwartet: Matrix Kapitel × Persona mit Zeichen und Zahl; im Kapitel des ersten
       RFC (Beiträge 41–49) stehen Befunde der Personas, Lob ausgeblendet (Schalter „Lob zeigen“).
    2. Eine Zelle anklicken → genau deren Befunde; einen Befund **verwerfen** (Begründung Pflicht), einen
       **übernehmen** (Fassung wählen), einen **verschieben** (Abschnitt oder Einfall). Erwartet: Abzeichen „k offen“
       in Gliederung, Editorkopf und am Menü sinken; je Entscheidung ein Commit im Spiegel (`git log` in
       `works/<slug>`), `befunde.md` im Spiegel.
    3. Im selben Kapitel den Lektor fragen. Erwartet: beginnt mit „Was trägt:“, höchstens zwei „Vorschlag:“, kein
       erzwungenes „Kritisch:“; der verworfene Punkt kommt nicht wieder (oder hängt am verworfenen Befund).
    4. **„RFC“** im Abschnitts-Chat. Erwartet: alle aktiven Personas antworten, der Lektor zuletzt mit „Jetzt wichtig“ /
       „Kann warten“; auf der Befundseite „Sammelvorschlag von @lektor“ mit Links auf die Ursprünge.
    5. **Chat:** lange Historie rollt im Akkordeon und steht beim Öffnen am neuesten Beitrag; Text markieren und
       kopieren (Strg+C) sowie „Kopieren“/„Zitieren“; eigenen Beitrag bearbeiten → „bearbeitet“, Verlauf auf der
       Beitragsseite; ein Persona-Beitrag hat kein Bearbeiten.
    6. „@lektor Schreibe den Absatz knapper“ → ein Vorschlag (Fassung), nichts gelöscht.
    7. Gegenprobe ohne JavaScript: `/beitraege?work=3&key=<k>` zeigt die ganze Historie, Statusformulare auf der
       Befundseite gehen.
- **Runde 27 „Umschlag-Editor und Impressum in der Werkvorlage“ abgeschlossen** (2026-11-08 22:40 – 2026-11-09 ≈ 00:50;
  Schnittstelle R27-S1 `cd54cbb` direkt auf `development`, Zweige `team/umschlag-daten` `f9cb541`,
  `team/umschlag-satz` `d39145f`, `team/umschlag-widget` `1d72a86` mit `--no-ff` in dieser Reihenfolge ohne Konflikte,
  danach der Integrationscommit `d2f19b9`, Nachbesserung `e2fe08a`, PM-Abschluss; nicht gepusht; Worktrees
  `wf_69268c89-5e4-3/4/5` nach dem PM-Abschluss entfernt, die Zweige `team/umschlag-daten`, `team/umschlag-satz`,
  `team/umschlag-widget` bleiben, bis der Nutzer sie löscht).
  - **PM-Abschluss:** Gutachten 13 Befunde (0 hoch, 2 mittel, 11 niedrig); die zwei mittleren in `e2fe08a` behoben
    (Grundschrift der Werkvorlage wie Typst über `baruch_wysiwyg::template_font`, Handprüfung in neun Schritten),
    drei niedrige vom PM (MCP-Hinweis in `initialize` nennt `cover_layout`/`cover_layout_set` samt Zusicherung in der
    Abnahme und `docs/MCP.md`; „im Aufbau“ aus Bedienung; Begründung einer Zusicherung), einer irrig (Herkunftstest
    der neuen Routen besteht in `round27_widget.rs`), die übrigen eingeplant (Backlog „Nach Runde 27 offen“: Zeitgrenze,
    Pixelgrenze, Normalform beim Einlesen, Namensschilder → R23a; Textregeln der Vorlage im Druck-Umschlag → R20a;
    Vorgabeblöcke auf 0,01 mm vom PM entschieden → R23a). Fahrplan mit R27 (≈ 130 min), R23 jetzt 100 min.
    Voller Lauf nach `cargo clean -p` aller 14 Workspace-Crates, `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast` (mit den PM-Änderungen): **143 Läufe „test result: ok“, 2038 bestanden, 0 fehlgeschlagen,
    1 ignoriert** (Goldstand-Schreiber), EXIT 0, keine Warnung; `cargo fmt --check` sauber. `STUB:`: unverändert
    (EPUB ×2 → R20, Cluster ×2 → R19, übrige Treffer Wächtertests).
  - **Integrator:** Store-Tests zur Buchvorlage auf `imprint_template()` nachgezogen, `round27_daten` (leerer Block hat
    Höhe 0), Platzhalter-Zweige aus `round27_widget` entfernt, `describe_exchange` zählt Umschlag-Blöcke; Abnahme
    `tests/round27_abnahme.rs` (5 Tests: Lage in Umschlag- und Buch-PDF mit `pdftotext -bbox` ≤ 0,5 mm, gleiche Quelle
    von Vorschau und PDF, Formular = RPC, Druckprüfung, Schnellwahl/Zurücksetzen bis byte-gleich, Impressum aus der
    Vorlage samt Knopf, MCP-Antrag, Austausch zwischen zwei Instanzen); SKILLS.md „Umschlag-Editor (Runde 27)“ und
    XSD u3. Sichtprüfung (Bilder `.claude/sicht/r27/`): Rahmen schwächer getönt, Druckprüfung ohne doppelten Blocknamen.
    Voller Lauf nach `cargo clean -p` aller 14 Workspace-Crates, `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **143 Läufe „test result: ok“, 2036 bestanden, 0 fehlgeschlagen, 1 ignoriert** (Goldstand-Schreiber),
    EXIT 0, keine Warnung; nach den Sicht-Korrekturen `cargo test -p baruch-server` erneut grün (246 + 879),
    `cargo fmt --check` sauber. `STUB:`: keine R27-Stubs mehr (EPUB ×2 → R20, Cluster ×2 → R19).
  - **Handprüfung durch den Nutzer mit Werk 3** nach dem Einspielen (Dienst 127.0.0.1:8765; **vorher Sicherung** von
    `~/.local/share/baruch` oder „Sicherung jetzt“ – Migration 0045 läuft beim ersten Start und baut
    `uuid_tombstone` und `asset_request` um, verlustfrei getestet in `migration_0045.rs`). Werk 3 hat noch keine
    eigene Anordnung; was probeweise geändert wird, nimmt Schritt 9 wieder zurück. Die Abnahme aus dem Plan
    (`docs/Backlog.md`, Runde 27) prüft der Abnahmetest mit erfundenem Werk; hier geht es um das echte Buch.
    1. **Werk ▾ → Umschlag** (Werk 3), bevor etwas geändert ist. Erwartet: Vorschau Rückseite | Rücken | Vorderseite
       wie bisher, alle Rahmen mit „(Vorgabe)“, Hilfslinien (Beschnitt rot gestrichelt, Schnittkante, Sicherheits-
       abstand gepunktet, Rückenkanten, Mitten). **Umschlag-PDF** herunterladen und aufheben (Vergleich in Schritt 9).
    2. **Titel ins dunkle obere Drittel:** Rahmen „Titel (Vorderseite)“ nach oben ziehen, an einer Ecke breiter
       ziehen, **Hell/Dunkel** auf hell, **Speichern**. Erwartet: Die Vorschau setzt den Titel nach kurzer Pause an
       der neuen Stelle in heller Schrift; die Druckprüfung meldet für den Titel keinen Kontrast mehr (oder nur, wenn
       das Bild dort doch hell ist – dann Schatten oder Verlauf als Hintergrund wählen, die Warnung verschwindet).
    3. **Autor nach unten**, dann **Klappentext auf der Rückseite** verschieben und über die Ecke schmaler machen,
       **Speichern**. Erwartet: Der Klappentext bricht in der Vorschau schmaler um und wird länger; die Höhe des
       Rahmens wächst mit; läuft er über den Strichcode, meldet die Druckprüfung „Strichcode“.
    4. **Gleiche Lage in beiden PDFs:** **Umschlag-PDF** herunterladen und Satz (Seite 1, Vorderseite; letzte Seite,
       Rückseite) bzw. Buch-PDF öffnen. Erwartet: Titel, Autor und Klappentext stehen in beiden an derselben Stelle
       relativ zur Schnittkante (im Umschlag-PDF kommt außen der Beschnitt dazu) und in derselben Schrift.
    5. **Warnungen auslösen:** einen Block mit Pfeiltasten (Umschalt = 5 mm) über die Schnittkante schieben; einen
       hellen Block auf eine helle Bildstelle legen; bei einem Block eine nicht vorhandene Schrift eintragen
       (z. B. „Gibt Es Nicht“). Erwartet in der Druckprüfung: „… ragt über die Schnittkante … in den Beschnitt“,
       „Sicherheitsabstand“, „Kontrast … ist zu gering“ und „Schrift „Gibt Es Nicht“ fehlt; gesetzt mit der Vorgabe
       „…““ – die Vorgabe ist die Grundschrift der Werkvorlage (letzte `#set text(font: …)` auf oberster Ebene).
       Danach **Strg+Z** bzw. die Werte zurückstellen und speichern.
    6. **Buchangaben** (Umschlagseite bzw. Einstellungen → Werk): Anschrift des Verlags/Herausgebers und der Druckerei
       eintragen, speichern. Erwartet: Die Pflichtangaben-Warnungen in der Druckprüfung (Titelei eingeschaltet)
       verschwinden für die gefüllten Angaben (Hinweis, keine Rechtsberatung).
    7. **Impressum aus der Werkvorlage:** **„Impressum-Vorlage in die Werkvorlage übernehmen“**. Erwartet: neue Fassung
       der Werkvorlage mit den fehlenden Funktionen `schmutztitel`, `haupttitel`, `impressum`, `widmung` (Commit im
       Spiegel); ein zweiter Klick sagt, dass alles da ist. Dann in **Werk ▾ → Werk bearbeiten** die Funktion
       `impressum(buch)` ändern, etwa eine eigene Zeile `[Eigene Angabe: #buch.verlag, #buch.verlag_anschrift]`
       ergänzen, speichern. Erwartet im **Satz**: Die Impressumsseite (Rückseite des Haupttitels) zeigt die eigene
       Zeile mit den Anschriften aus Schritt 6 an fester Stelle. Danach die vorige Vorlagenfassung wieder herstellen,
       falls die Änderung nicht bleiben soll.
    8. **Gegenprobe ohne JavaScript** (JavaScript im Browser abschalten oder `/werke/3/umschlag` in einem Browser ohne
       JS): Zahlenfelder eines Blocks ändern, **Vorschau ohne Speichern**. Erwartet: Adresse trägt die Werte
       (`?b.front_title.y_mm=…`), Vorschau und Rahmen zeigen die neue Lage, gespeichert ist nichts; ein ungültiger
       Wert (z. B. y = 5000) bleibt mit Meldung „Titel (Vorderseite), y: Zahl zwischen -50 und 1000 mm erwartet …“
       stehen. **Anordnung übernehmen** speichert; Schnellwahl und Zurücksetzen gehen als eigene Formulare.
    9. **Zurücksetzen:** **Anordnung zurücksetzen** ohne Häkchen (alle Flächen). Erwartet: alle Rahmen wieder
       „(Vorgabe)“, das Umschlag-PDF entspricht wieder dem aus Schritt 1 (Werk ohne Anordnung setzt unverändert).
- **Runde 18 „Editor und Menüband“ abgeschlossen** (2026-11-08, 20:34–≈ 22:50; Schnittstelle R18-S1 `a1e03d8` direkt auf
  `development`, Zweige `team/konstrukte` `645e361`, `team/satzbild-werk` `3de9585`, `team/menueband` `7d4319d` mit
  `--no-ff` in dieser Reihenfolge ohne Konflikte, Merges `f9cf8b4` `4b6d4d0` `c04ae85`, danach der Integrationscommit;
  Integration `b7e5362`, Nachbesserung `d721b1c`, PM-Abschluss; nicht gepusht; Worktrees `wf_0dcac4b5-0a3-3/4/5`
  nach dem PM-Abschluss entfernt, die Zweige `team/menueband`, `team/konstrukte`, `team/satzbild-werk` bleiben, bis
  der Nutzer sie löscht).
  - **PM-Abschluss:** Gutachten 13 Befunde (0 hoch, 4 mittel, 9 niedrig); die vier mittleren in `d721b1c` behoben,
    drei niedrige vom PM (Modulkopf `typeset.rs` beschrieb den Abbruch über Welt-Zugriffe falsch; „Werkzeugleiste“
    in Bedienung/Architektur; `aria-label` an K/F, Tastatur des Menübands dokumentiert), sechs eingeplant (Backlog
    „Nach Runde 18 offen“: `defined_functions` ohne Blockkommentare, `work.sheet_set` ohne Transaktion, Pfeiltasten
    → R23a; `offset:` in mehrzeiligem Inline-Konstrukt → R21c; Bibelstelle/`@cite` → R19c; zu früh als erledigt
    markierte Reste → R22b/R23a). Die fünf Reste „→ R18 oder später“ aus Runde 17 gehen nach R19c (R19 jetzt 120
    min). Fahrplan mit R18 (≈ 135 min) und berichtigter R25-Zeile (laut Git 18:48–20:34). Voller Lauf nach `cargo
    clean -p` aller 14 Workspace-Crates, `nice -n 10 ionice -c3 cargo test --workspace -j 4 --no-fail-fast`: **139
    Läufe „test result: ok“, 1951 bestanden, 0 fehlgeschlagen, 0 ignoriert**, EXIT 0, keine Warnung; nach den
    PM-Änderungen `cargo test -p baruch-server` erneut grün (237 + 848), `cargo fmt --check` sauber. `STUB:`:
    unverändert (EPUB ×2 → R20, Cluster ×2 → R19, übrige Treffer Wächtertests).
  - **menueband (18a):** Menüband mit Formaten der Werkvorlage (`ribbon.formats`), Satz-Seite ohne JavaScript mit
    „Satz abbrechen“ (`preview.cancel`, `baruch_render::Cancel`), fremde Seiten lösen keinen Satz/kein PDF aus,
    Schriftsuche beim Start, Kopfzeile bricht unter 1000 px um, „neu“ als Wort, Kanten der Gedankenkarte mit
    Beschriftung, Pfeil und Strichart, geteilte Seite zählt zu beiden Kapiteln.
  - **konstrukte (18b):** Aufzählungen, Begriffslisten, geschachtelte Listen, `\`, `#link`, Verweise im Satzbild;
    Inseln an Ort und Stelle (`{replaces, raw}`); mehrere Bilder je Block; Markdown bei `revision.update`; Teil-Block im
    Kapiteltext abgelehnt; Lektor-Reste (ausgelassene Teile in einer Zeile, Zwischenspeicher der Zusammenfassungen,
    „an Persona geben“ auf `/quelle/multigpt`, Ereignis `piece`).
  - **satzbild-werk (18c):** Migration 0044 `work_sheet`, Einstellungen → Werk → Satzbild, RPC `work.sheet`/
    `work.sheet_set`, Stil schon im HTML; Bild an der Cursorstelle (`place` = end|start|block:n|offset:n, Pfad und
    Fassung in einer Transaktion); Vorschläge-Reste; Literatur-Treffer im Satzbild; Verhaltenstest in Node.
  - **Integrator:** `typeset::warm_fonts()` in `main.rs`; fremde Zusicherungen nachgezogen (`migration_0040/0041`
    wegen `work_sheet`, `round15_nachbesserung` Ablehnung jetzt bei 3000 Tokens, `round18_menueband` Ziel des Knopfs
    „Bild“); `piece` in `docs/RPC.md`; Abnahmetest `tests/round18_abnahme.rs` (5 Tests); nach der Sichtprüfung
    gestapelte Bereiche unter 900 px (Editor ragte unter „Assistenten“) und Beschriftungen der Karte über den Knoten.
    Voller Lauf nach `cargo clean -p` aller 14 Workspace-Crates, `nice -n 10 ionice -c3 cargo test --workspace -j 4
    --no-fail-fast`: **139 Läufe, 1946 bestanden, 0 fehlgeschlagen, 0 ignoriert**, keine Warnung, EXIT 0; nach den
    Fixes der Sichtprüfung `round25`/`round18`/`editor_layout`/`round9_menu` erneut grün (90), `cargo fmt --check`
    sauber. Offene Stubs: nur die zwei EPUB-Vermerke (R20) und „Cluster … (R19)“.
  - **Sichtprüfung** (Firefox headless, Wegwerf-Server mit erfundenem Werk „Der Hafen am Morgen“): Bilder unter
    `.claude/sicht/r18/` (`editor-hell`, `editor-dunkel`, `editor-1024`, `editor-schmal-800` (nach dem Fix),
    `satzbild-listen-inseln`, `satzbild-einstellungen-nachher` (vorher = `editor-hell`), `einstellungen-satzbild-block`,
    `einstellungen-werk-satzbild`, `bild-einfuegen-ohne-js`, `bild-an-cursorstelle`, `satz-seite`,
    `satz-abgebrochen`, `karte-kanten` (nach dem Fix), `karte-kanten-dunkel` (vor dem Fix)).
  - **Nachbesserung nach dem Gutachten:** Rohblock-Insel (```` ``` ````) im Rumpf einer Szene behält beim Ändern ihren
    Einzug und bleibt unverändert byte-gleich (`dedent` entscheidet für den ganzen Text, `indent_lines`);
    `Sec-Fetch-Site`-Schutz auch für Umschlag (Seite, SVG, PDF), Stichwortzettel-PDF und Karte-PDF; Kopfzeile der
    Satz-Seite ohne Widget (Thema als Link auf Einstellungen → Darstellung); Handprüfung unten als nummerierter
    Ablauf mit Werk 3. Voller Lauf: 139 Läufe, 1951 bestanden, 0 fehlgeschlagen.
  - **Handprüfung durch den Nutzer mit Werk 3** nach dem Einspielen (Dienst 127.0.0.1:8765, Editor mit Menüband;
    **vorher Sicherung** des Datenordners oder „Sicherung jetzt“; beim ersten Start läuft Migration 0044): Was im
    echten Buch probeweise geändert wird, danach mit Strg+Z bzw. „Verwerfen“ zurücknehmen oder in **Fassungen** die
    vorige Fassung wieder als aktuell markieren.
    1. Werk 3 → **Werk ▾ → Werk bearbeiten** → Werkvorlage: nachsehen, ob sie `#let szene` und `#let arbeit`
       definiert (Baruch kennt die Vorlage des Nutzers nicht; darum hier nachsehen). Erwartet im Editor: Das
       Menüband zeigt **Szene** bzw. **Arbeit** genau dann, wenn die Vorlage sie definiert, und immer **Fußnote,
       Zitat, Liste, Aufzählung, Begriff, Umbruch, Link** sowie **Bild**.
    2. Werk 3, ein Abschnitt in Kapitel I, **Quelltext**: Cursor in einen Absatz, **Fußnote** klicken. Erwartet:
       `#footnote[]` steht an der Cursorstelle, der Cursor darin; **Strg+Z** nimmt es zurück. Eine Markierung vorher
       wandert in die Fußnote.
    3. Derselbe Abschnitt im **Satzbild**: **Liste**, **Aufzählung** und **Begriff** am Cursor. Erwartet: Punkt,
       Nummer bzw. `Begriff: …` erscheinen im Blatt; ein Format, das das Satzbild nicht kann (etwa **Szene**),
       meldet unten rechts „… geht im Satzbild noch nicht – bitte im Quelltext einfügen“ und ändert nichts.
    4. Ein Abschnitt von Werk 3 mit gelber Insel (z. B. ein Arbeitsauftrag oder Code): **Doppelklick** auf die
       Insel. Erwartet: ein Quelltextfeld an ihrer Stelle; **Escape** stellt sie wieder her; **Übernehmen** ohne
       Änderung und Speichern legt keine neue Fassung an (der Text bleibt byte-gleich, auch im Rumpf einer Szene).
    5. **⚙ Einstellungen → Werk → Satzbild** (Werk 3): Schrift, Farben und Satzbreite ändern, speichern. Erwartet: Das
       Satzbild im Editor zeigt die neuen Werte, Satz und PDF bleiben unverändert (die Einstellung gilt nur für das
       Satzbild). **Alles aus der Vorlage** setzt alle Werte zurück.
    6. **Bild einfügen …** (Statuszeile oder Knopf **Bild**) mit „an der Cursorstelle“, z. B. `umschlag.png`.
       Erwartet: das Bild steht hinter dem Block mit dem Cursor, eine neue Fassung ist angelegt; danach die vorige
       Fassung wieder als aktuell markieren.
    7. Werk 3 → **Ansicht ▾ → Satz**: während er setzt (erster Satz nach dem Start dauert) **Satz abbrechen**.
       Erwartet: „Der Satz wurde abgebrochen.“ mit **Neu setzen**; dieser Link setzt neu. Das Zeichen ◐ oben rechts
       führt auf dieser Seite zu Einstellungen → Darstellung (die Satz-Seite hat kein JavaScript).
    8. Werk 3 → **Werkzeuge ▾ → Gedankenkarte**: Verbindungen tragen ihre Art als Beschriftung, gerichtete Arten
       eine Pfeilspitze, „widerspricht“ (etwa 48 ↔ 79, falls nach Runde 25 angelegt) keine, jede Art ihre Strichart.
       Dabei in hellem und dunklem Thema prüfen, ob Beschriftungen naher Einfälle deren Nummern zu sehr verdecken
       (seit der Sichtprüfung liegen sie über den Knoten; ein dunkles Bild nach dem Fix fehlt).
- **Runde 25 „Gedankenkarte: Mindmap der Einfälle“ abgeschlossen** (2026-11-08, 18:48–≈ 20:55; vorgezogen vor R18;
  Schnittstelle R25-S1 `c0e5e8a` direkt auf `development`, Zweige `team/karte-daten` `be8c243`, `team/karte-bild`
  `0cb92f6`, `team/karte-widget` `8baa26a` mit `--no-ff` in dieser Reihenfolge, Merges `1d5ec7f` `4e6d9e7` `9d70fec`,
  Integration `ff8d3d6`, Sichtprüfung `61eae1b`, Nachbesserung `fc2ca78`, PM-Abschluss; nicht gepusht; Worktrees
  `wf_cbb5c75b-f78-3/4/5` nach dem PM-Abschluss entfernt, die Zweige `team/karte-*` bleiben, bis der Nutzer sie löscht).
  - **karte-daten:** Migration 0043 (`idea_link`, `idea_map_pos`, Grabsteinarten), Store `idea_links.rs`, RPC
    `idea.map/links/link/unlink/link_restore/map_pos_set/map_pos_reset`, MCP `idea_map`, `idea_links`, `idea_link`,
    `idea_unlink`, Spiegelzeile „Verbindungen“ in `einfaelle.md`, Nähe aus gespeicherten Vektoren (`rag/neighbors.rs`).
  - **karte-bild:** Anordnung (300 Einfälle ≈ 1 ms im Debug-Bau, ohne Überlappung), Seite `/werke/{id}/karte` ohne
    JavaScript mit Gliederungsliste, Widersprüchen und Formularen (`POST …/karte/act`), Ausgaben SVG/PDF/Markdown/
    FreeMind, Menüpunkt, Verbindungen auf `/einfaelle/{id}`; Stub „Gruppen benennen (Assistent)“ (R19).
  - **karte-widget:** `web/src/idea_map.ts` (Zoom, Verschieben, Ziehen, Tastatur, Seitenleiste, Alt+E),
    Austauschformat Unterversion 2 (`<karte>`, XSD `docs/schema/baruch-werk-1-u2.xsd`, v1-XSD eingefroren).
  - **Integrator:** Trigger `idea_link_cascade_uuid_tombstone` in 0043 (Grabstein ohne Werk bei hart gelöschtem
    `from_id`), Vorgabewerte der Nähe nicht mehr in den Links der Karte, Abnahmetest `tests/round25_abnahme.rs`,
    Doku-Einleitungen, SKILLS (u2-XSD). Details: `docs/Architektur.md` „Integration Runde 25“.
  - **PM-Abschluss:** Fahrplan (Runde 25 mit Zeiten aus `git log`, Gantt ohne Doppelpunkte in Aufgabennamen),
    Backlog („Nach Runde 25 offen“, „neuer Teil als Antrag“ → R22, `GrabsteinArt` erledigt, Reste nach R18a/R19a/
    R21c/R23a). Niedrige Gutachtenbefunde behoben: Legende „widerspricht“ Strich-Punkt wie die Kante, Nichtzeichen
    U+FFFE/U+FFFF in SVG- und FreeMind-Ausgabe entfernt (Unit-Tests), Daten-Hinweis in `idea_map`/`idea_links` und
    `docs/MCP.md`, Bedienung „Nähe fein gepunktet“, Begründungen an fünf Zusicherungen. Voller Lauf nach `cargo clean
    -p` aller 14 Workspace-Crates, `nice -n 10 ionice -c3 cargo test --workspace -j 4 --no-fail-fast`: **135 Läufe,
    1868 bestanden, 0 fehlgeschlagen, 0 ignoriert**, keine Warnung, EXIT 0; nach den PM-Änderungen `baruch-server`
    und `baruch-store` erneut grün, `cargo fmt --check` sauber. Offene Stubs (`grep -rn "STUB:" crates`): die zwei
    EPUB-Vermerke (R20) und „Cluster durch einen Assistenten benennen (R19)“ (Code und Vorlage).
  - **Offen für den Nutzer:** Nutzerfragen 1 (werkübergreifende Karte, Vorgabe später) und 2 (Gruppen durch einen
    Assistenten benennen, Vorgabe ja in R19); die Handprüfung unten (steht auch im Backlog „Beim Nutzer“).
  - **Sichtprüfung** (Firefox headless, Wegwerf-Server): Bilder unter `.claude/sicht/r25/` (`karte-hell`,
    `karte-dunkel`, `karte-ohne-js`, `karte-formulare-ohne-js`, `karte-seitenleiste`, `karte-widersprueche`,
    `karte-filter`, `karte-pdf-1`, `menue-werkzeuge-gedankenkarte`); sie zeigen den Stand vor der Nachbesserung
    (Teile noch blass, leere Kapitel noch ohne „leer“).
  - **Nachbesserung nach dem Gutachten** (siehe `docs/Architektur.md` „Integration Runde 25“): Werkwechsel löst die
    Verbindungen eines Einfalls und entfernt seine Lagen; Teile zählen die Einfälle ihrer Kapitel; leere Kapitel
    hervorgehoben (Wort „leer“).
  - **Handprüfung durch den Nutzer** nach dem Einspielen von Runde 25 (beim Einspielen läuft Migration 0043;
    **vorher Sicherung** des Datenordners oder „Sicherung jetzt“): 1. Werk 3 → Werkzeuge ▾ → Gedankenkarte: alle 33
    Einfälle erscheinen, die 5 ohne Abschnitt in der Wolke „Ohne Kapitel“; Teile tragen die Zahl der offenen
    Einfälle ihrer Kapitel, wirklich leere Kapitel sind mit „leer“ hervorgehoben. 2. Einfälle per Ziehen ordnen
    (mit JavaScript) und „Bezug setzen“ ohne JavaScript; Lage bleibt nach dem Neuladen, „Anordnung zurücksetzen“
    verwirft sie. 3. Den Widerspruch 48 ↔ 79 von Hand anlegen („Verbinden mit Einfall“, Art „widerspricht“); er
    steht in der Liste „Widersprüche“ und in `einfaelle.md`. 4. „Nähe zeigen“ mit Bedeutungsindex (Einfälle
    eingebettet) und ohne (Hinweis, Ersatz über Schlagwörter). 5. Ausgeben als FreeMind (`.mm`) in Freeplane öffnen,
    dazu SVG, PDF und Markdown. 6. Einen verbundenen Einfall in ein anderes Werk umziehen lassen: Seine Verbindungen
    sind danach gelöst (Commit nennt sie).
- **Runde 26 „Austauschformat zwischen Instanzen (XML)“ abgeschlossen** (2026-11-08, 16:37–≈ 18:50; vorgezogen vor
  R25 und R18; Schnittstelle R26-S1 `a0c0c86` direkt auf `development`, Zweige `team/ids` `59ff36f`, `team/export`
  `255cb6a`, `team/import` `14a5af1` mit `--no-ff` in dieser Reihenfolge, Merges `0a1e010` `e24bcdf` `ea6ed61`,
  Integration `8d0fa86`, Nachbesserung `1c4a3ca`, PM-Abschluss; nicht gepusht; Worktrees `wf_43812325-507-3/4/5`
  nach dem PM-Abschluss entfernt, die Zweige `team/ids|export|import` bleiben, bis der Nutzer sie löscht).
  - **Gutachten** (11 Befunde: 1 hoch, 6 mittel, 4 niedrig), alle behoben: gelöschtes Werk ließ sich wegen seiner
    verwaisten Einfälle nicht aus der eigenen Datei zurückspielen (hoch; jetzt `adopt_orphan_idea`); doppelte
    Fassungen beim Hin-und-her-Abgleich nach einer Bearbeitung an Ort und Stelle (`same_text_number`); Instanzname
    aus der Datei mit `<`/`>`/Zeilenumbruch verhinderte den Spiegel-Commit (`clean_instance_name`); unbekannte
    Grabstein-Art bei `version` > 1 lehnte die ganze Datei ab; Rundreise-Test hatte einen Ausweg „ÜBERSPRUNGEN“;
    Startbefehl der Wegwerf-Instanz fehlte (Nachbesserung `1c4a3ca`). Niedrig, im PM-Abschluss: Besitzvermerke aus
    Doku und Dateiköpfen entfernt, RPC-Einleitung zum Vorgabewerk berichtigt, „Ältere Datei“ (Rücknahme-Anträge
    ohne gemeinsame Basis) in `docs/Austauschformat.md` §4, Fahrplan mit Zeiten aus `git log`.
  - **PM-Abschluss:** voller Lauf nach `cargo clean -p` aller 14 Workspace-Crates, `nice -n 10 ionice -c3 cargo test
    --workspace -j 4 --no-fail-fast`: **132 Läufe, 1794 bestanden, 0 fehlgeschlagen, 0 ignoriert**, keine Warnung,
    EXIT 0; nach den Kommentar-Änderungen `baruch-exchange`, `baruch-store` und `round26*` erneut grün,
    `cargo fmt --check` sauber. Offene Stubs (`grep -rn "STUB:" crates`): nur die zwei EPUB-Vermerke (R20).
  - **ids:** Spalte `uuid` in zehn Tabellen mit Fülltriggern, Grabsteine `uuid_tombstone` (Migration 0041; 0042 für
    die Kaskade beim Löschen eines Werks), Instanzkennung und -name (Einstellungen → Daten „Diese Instanz“, RPC
    `instance.get/set_name`), UUID in „Werk ändern“ und `work.meta`.
  - **export:** Crate `baruch-exchange` (Modell, eigener Schreiber, deterministisches ZIP `mimetype`/`werk.xml`/
    `bilder/`/`MANIFEST.xml`), XSD `docs/schema/baruch-werk-1.xsd`, `Store::exchange_dump`, Seite
    `/werke/{id}/austausch` mit beiden Downloads, RPC `work.exchange_export`, CLI `export ID --baruch DATEI`, MCP
    `exchange_export` (nur lesend, 4 MiB).
  - **import:** sicherer Leser (keine DTD/Entitäten, Grenzen, ZIP-Pfade, Manifest-Prüfsummen, ZIP-Bombe), `validate`,
    `exchange_apply` (Vorschau = derselbe Code mit Rücksprung; neu bzw. abgleichen nach F6: nur hinzufügen, nie
    löschen/überschreiben/umschalten, Gliederungsänderungen als Anträge mit Herkunft „Austausch: <Instanz>“, ein
    Spiegel-Commit), Seiten `/austausch/einlesen`, `/austausch/vorschau`, `/austausch/uebernehmen` ohne JavaScript,
    RPC `work.exchange_preview/_import`, CLI `import DATEI.baruch[.xml] [--abgleichen] [--vorschau] [--title]`.
  - **Integrator:** Wächtertest „nur exchange_apply schreibt uuid“ schlug auf `HashSet::insert` und minijinja an
    (auf SQL beschränkt); Rundreise-Test lief erst nach dem Merge wirklich und scheiterte an der Überschriftsebene im
    Test (berichtigt); neuer Test Umbenennen auf B → Hinweis „Umschalten genügt“; MCP-Hinweis nennt
    `exchange_export`; Einzahl/Mehrzahl, Formular-Kasten und lange Begründungen in der Antragsliste. Details:
    `docs/Architektur.md` „Integration Runde 26“.
  - **Ende-zu-Ende** (zwei Wegwerf-Server, CLI auf drittem Ordner; das Prüfskript lag nur im Scratchpad unter
    `/tmp` und ist **vergänglich** – es brauchte von Hand gestartete Server; dieselben Wege decken dauerhaft
    `tests/round26_*.rs` und `baruch-store/tests/exchange_apply.rs` ab): 75 Prüfungen grün (Rundreise ohne Verlust bis auf `<ausgabe>`,
    Abgleich mit weiteren Fassungen, Einfall, Antrag, „aktuell“ unverändert, zweiter Durchgang leer, sechs
    manipulierte Dateien ohne Schreiben abgelehnt, keine Geheimnisse in der Ausgabe).
  - **Voller Lauf** nach `cargo clean -p` aller 14 Workspace-Crates, `nice -n 10 ionice -c3 cargo test --workspace -j 4`:
    **132 Läufe, 1789 bestanden, 0 fehlgeschlagen, 0 ignoriert**, keine Warnung, EXIT 0; `grep -rn 'STUB:' crates | grep R26` leer.
  - **Sichtprüfung** (Firefox headless): Bilder unter `.claude/sicht/r26/` (`werk-menue`, `ausgeben`, `einlesen`,
    `vorschau-neu`, `vorschau-abgleich`, `ergebnis-antraege`, `gliederung-antraege`, `einstellungen-instanz`).
  - **Offen für den Nutzer:** Umbenennen auf der anderen Instanz wird nach F6 (b) Hinweis statt Antrag (Backlog
    „Nach Runde 26 offen“, Nutzerfrage); Nutzerfragen 1 (Bibliotheksformat, Vorgabe nein) und 2 (Netzabgleich, später).
  - **Handprüfung durch den Nutzer** (beim Einspielen laufen die Migrationen 0041 und 0042; vorher Sicherung): 1.
    Einstellungen → Daten → „Diese Instanz“ einen Namen geben. 2. Ein eigenes Werk über Werk ▾ → Austausch:
    Ausgeben herunterladen (`.baruch`). 3. Wegwerf-Instanz in einem eigenen Ordner auf einem **anderen Port**
    starten (ohne `--listen` nähme sie 8765, wo der Dienst läuft, und der Start scheiterte):
    `baruch-server --listen 127.0.0.1:8766 --data ~/baruch-probe --secrets memory`. Mit `--secrets memory` liest sie
    keinen Schlüssel aus dem Schlüsselbund; ihre Bedeutungssuche bettet über die Vorgabe-Verbindung
    `127.0.0.1:1234` ohne Schlüssel ein und bekommt dort nur 401 (harmlos; wer das nicht will, stellt unter
    Einstellungen → Verbindungen auf `:8766` Eliezer und MultiGPT vor dem Einlesen auf `http://127.0.0.1:9`).
    Im Browser `http://127.0.0.1:8766/` → Werk ▾ → Austausch: Einlesen… die Datei einlesen, Vorschau und Ergebnis
    ansehen. 4. Auf `:8766` etwas ändern (neue Fassung, Einfall, „mit Nummer“), dort über Werk ▾ → Austausch
    ausgeben, die Datei herunterladen und auf der eigenen Instanz (`:8765`) einlesen („abgleichen“): weitere
    Fassung, Anträge unter Gliederung → Anträge, „aktuell“ bleibt. 5. Aufräumen: die Wegwerf-Instanz mit Strg+C
    beenden und `~/baruch-probe` löschen (`rm -r ~/baruch-probe`).
- **Runde 17 „Bibelstellen (SWORD-Rest), Reste Lektor“ integriert** (2026-11-08, Integrator; vom Stand `f8bf209`,
  Zweige `team/verszaehlung` `99e0811` (S1 `c9777da`), `team/stellen` `85ce28b` (S2 `2e12c10`), `team/pruefanzeige`
  `13a18b3` (S3 `8b6f65e`), mit `--no-ff` in dieser Reihenfolge; nicht gepusht; Worktrees `wf_ed399bc3-042-2/3/4` und
  Zweige bleiben bis zum PM-Abschluss).
  - **verszaehlung:** alle 18 SWORD-Verszählungen, Abbildung mit geprüfter Regeltabelle (Drehpunkt KJV), RPC
    `scripture.map`/`scripture.versifications`, MCP `scripture_map`; Lektor-Reste L1–L5, L9 (Zitatprüfung nur gegen
    den gezeigten Text, `assist/ref_claims.rs`, Fäden, wirksame Kontextlänge ohne Netz, Einfälle gerahmt).
  - **stellen:** Parser (Komma-Listen, Uhrzeit, Anzahl, „Psalm 139, … Vers 23“), `crate::quotes::quote_spans`,
    Lexikon-Stichwörter über Strong-Nummern bzw. Lemma-Probe, Predigttext über `read_passage` (400 Verse), L6–L8.
  - **pruefanzeige:** „Zitate prüfen“ im Editor (Highlight-API, Liste; ohne JavaScript `/werke/{id}/pruefung`), RPC
    `bible.marks`, verschlüsselte Module (`bible.cipher_key_set/_delete`, nur Mensch, Schlüssel nur im
    Schlüsselbund), BZIP2/XZ, Lizenzvermerk überall, Kommentar-Widget.
  - **Integrator:** Ende-zu-Ende `tests/round17.rs` (5 Tests); behoben: 4./5. Mose wurden nie als Stelle erkannt,
    Zitat-Zuordnung über eine Leerzeile, Akkordeon der Arbeitsseite mit wirksamem Lektor/Kontext, „(nicht im Text
    belegt)“ galt als Stellenaussage, Liste „Zitate prüfen“ links abgeschnitten, MCP-Hinweis nennt `scripture_map`.
    Details: `docs/Architektur.md` „Integration Runde 17“.
  - **Handprüfung mit Eliezer** (zwei Anfragen an `google/gemma-4-26b-a4b` über den Pförtner, `--secrets memory`,
    erfundene „Möwenpredigt“ aus einer erfundenen BZIP2-Bibel in KJV-Zählung): „Prüfen, ganze Predigt“ 10,5 s, vier
    „Kritisch:“-Befunde hinter der Teil-Angabe erkannt, alle Zitate im Text; eine Frage nach Lk 15,5/15,6 1,8 s.
    „Lk 15,5 nicht belegt“ trat nicht wieder auf (Vermerk nur offline getestet).
  - **Sichtprüfung** (Wegwerf-Server, Firefox headless, Vorschalt-Server für Klicks): Bilder unter `.claude/sicht/r17/`
    (`editor-deutung-markiert2` – Mal 3,20 gelesen als Mal 4,2 (KJV), grün –, `editor-anwendung-markiert2` – rot
    gewellt mit Unterschieden –, `pruefseite`, `quellen-gesperrt`, `quellen-falsch`, `quellen-entsperrt-ausschnitt`,
    `literatur-kommentare-offen-ausschnitt`, `lektor-befund-ausschnitt`, `editor-deutung-liste-abgeschnitten-vorher`).
  - **Handprüfung durch den Nutzer** (nach dem Einspielen; keine Migration): 1. Eine eigene Predigt öffnen, im
    Editor unten **Zitate prüfen** → Liste über der Statuszeile, Zitate im Satzbild grün/rot; ohne JavaScript führt
    der Link auf die Prüfseite. 2. **Einstellungen → Quellen → Bibeln**: Spalten „Lizenz (laut conf)“ und
    „Verschlüsselung“; ein verschlüsseltes Modul (falls vorhanden) mit dem Schlüssel des Herausgebers freischalten –
    ein falscher Schlüssel meldet „Der Schlüssel passt nicht zum Modul; nichts gespeichert.“ 3. Eine Stelle wie
    „4. Mose 6,24“ oder „Psalm 139, besonders Vers 23“ im Text: erscheint als Stelle. 4. Lektor „Prüfen, ganze
    Predigt“: Behauptet ein Befund, eine Stelle aus dem Predigttext fehle, steht dahinter „(Stelle steht im Text: …)“.
    5. Akkordeon „Assistenten“ auf der Arbeitsseite und Einstellungen → Assistenten zeigen denselben Lektor und
    dieselbe Kontextlänge. 6. **Andere Verszählung an echten Daten** (bisher nur mit selbst erzeugten Modulen
    geprüft): eine eigene Bibel in KJV- oder Luther-Zählung als Bibel des Werks wählen und Stellen mit Abbildung
    zitieren – „Mal 3,19–24“ (in KJV Mal 4,1–6), „Joel 3,1–5“ (KJV Joel 2,28–32), „Ps 51,3“ (KJV Ps 51,1, Überschrift
    als Vers 1–2), „Ps 13,2“ (gleich viele Verse, trotzdem verschoben). In der Liste „Zitate prüfen“ steht „gelesen:
    … (KJV)“; stimmt der Wortlaut? Eine Bibel in Catholic/Segond/Calvin-Zählung meldet bei den Psalmen ehrlich
    „Versifikation passt nicht“. 7. **BZIP2/XZ an echten Daten:** ein eigenes Modul mit `CompressType=BZIP2` oder
    `XZ` (steht in dessen conf, Einstellungen → Quellen zeigt es) nachschlagen und „Zitate prüfen“ damit. 8.
    **Verschlüsselung:** Geprüft ist sie gegen libsword (`osis2mod`/`diatheke` mit erfundenem Text,
    `crates/baruch-sword/tests/libsword.rs`), aber noch an keinem echten Modul eines Herausgebers; wer eines mit
    Schlüssel hat, schaltet es frei und vergleicht einen Vers mit einer anderen Ausgabe.
- **Nachbesserung Runde 17** (Gutachten nach der Integration, 2026-11-08): Abbildung rät in Büchern ohne Regel nicht
  mehr (Catholic, Catholic2, Segond, Calvin: Psalmen „Versifikation passt nicht“ statt still verschoben);
  Stellenvermerk nur bei „fehlt“ mit Ort und ganzen Wörtern; vom Modell geschriebene Prüfvermerke werden entfernt und
  neu geprüft; Probelesen erkennt falsche Schlüssel auch bei Latin-1/RawText; sword-modules unter
  `vendor/sword-modules` mit Obergrenze 64 MiB je entpacktem Block (Dekompressionsbombe); Satzbild markiert Zitate
  nur als ganze Wörter und an ihrer Stelle (`web/src/quote_find.ts`); Abgleich mit libsword (`tests/libsword.rs`).
  Details: `docs/Architektur.md` (Abbildung, Stellenaussagen, Prüfanzeige, Verschlüsselte Module, Test-Module,
  Obergrenze beim Entpacken).
- **PM-Abschluss Runde 17** (2026-11-08): voller Lauf nach `cargo clean -p` aller 13 Workspace-Crates (7,2 GiB
  entfernt), `nice -n 10 ionice -c3 cargo test --workspace -j 4`: **122 Läufe, 1684 bestanden, 0 fehlgeschlagen,
  0 ignoriert** (Server-Bündel 750, 61,9 s), EXIT 0. Vorher `cargo fmt --all` (die Nachbesserung war nicht
  fmt-sauber: neun Dateien, nur Zeilenumbrüche und die Reihenfolge `mod quote_find` in `tests/main.rs`) und
  Begründungen an zwei Zusicherungen (`round17.rs`, `round17_pruefanzeige.rs`). `STUB:`-Zeilen: 6, davon 2 echte
  Stubs (`cover.rs` EPUB-Umschlag, `base.html` EPUB-Vorschau), die übrigen sind Wächtertests. Gutachten: 16 Befunde
  (1 hoch, 8 mittel, 7 niedrig); der hohe und alle mittleren in `322c889` behoben; von den niedrigen im PM-Abschluss
  Begründungen, Backlog (Status „→ R17“ auf erledigt) und Fahrplan, die übrigen („Mal“ als Zahlwort, GET-Abfrage der
  Modellliste, 2. Kön 15, Hilfsskript der Regeltabelle) unter „Nach Runde 17 offen“; „Hervorheben an Bibeltexten“
  ist eine Nutzerfrage. Fahrplan: Runde 17 ≈ 105 min (13:35–15:20). Worktrees `wf_ed399bc3-042-2/3/4` samt
  `target/` entfernt, Zweige `team/*` bleiben (gemergt). Keine Migration; nach dem Einspielen genügt Neubau und
  Neustart des Dienstes. Nächste Runde: R18 „Editor und Menüband“.
- **Runde 16 „Predigt (B4)“ integriert** (2026-11-08, Integrator; vom Stand `f80f7d2`, Zweige `team/zeitwaechter`
  `1d6178a` (S1 `1db283d`), `team/predigt-vorlage` `136bffd`, `team/stichwortzettel` `7f0a579` (S1 per cherry-pick
  `8061b24`), mit `--no-ff` in dieser Reihenfolge; nicht gepusht; Worktrees `wf_930e8bdf-c1d-2/3/4` und Zweige
  bleiben bis zum PM-Abschluss).
  - **predigt-vorlage:** Neues Werk → Predigt (Predigttext · Einleitung · I.–III. · Anwendung · Schluss, Schema
    `sermon`), Predigttext aus SWORD als Zitat mit Stelle und Modul (ohne Bibel ehrlich nur die Stelle), Prompts
    „Predigt“, fremde Befunde/Relay/Fäden gerahmt als Daten, Belegpflicht „Kritisch:“ mit „(nicht im Text belegt)“,
    wirksame Kontextlänge für Fäden und „Lektor automatisch“, Einstellung „Chat nur Abschnitt“.
  - **zeitwaechter:** Sprechdauer (Migration 0039, `baruch_typst::timing`), Zielzeit, Anzeige in Baum, Editorkopf
    (Live über `timing.count`), Einstellungen → Werk → Predigtzeit, RPC `work.timing*`, `timing.count`, MCP
    `work_timing`; lokal/extern im Akkordeon „Assistenten“.
  - **stichwortzettel:** Stichwortzettel A5/Karten (Seite, `notes.get`/`notes.pdf`, Download, CLI `--stichwort`),
    Protokoll-Commit beim Ablehnen/Veralten jeder Antragsart, Vorschau „neuer Teil“ mit Art und Nummer, Zeiten auf
    `/gliederung`.
  - **Integrator:** Ende-zu-Ende `tests/round16.rs` (6 Tests); „Kritisch:“ hinter der Teil-Angabe zählt
    (`prompts::critical_finding`, Fund der Handprüfung), Zitatprüfung mit `[…]`-Einfügungen, „die ganze Predigt“ im
    Auftrag, sticky Kartenkopf im Stichwortzettel, `static/predigt.css`. Details: `docs/Architektur.md` „Integration
    Runde 16“.
  - **Handprüfung mit Eliezer** (zwei Anfragen an `google/gemma-4-26b-a4b` über einen Pförtner, Baruch mit
    `--secrets memory`, erfundene „Laternenpredigt“ ≈ 5:30): Prompt sagt „Lektor dieser Predigt“ samt Predigt-Hinweis,
    ganze Predigt im Wortlaut (1 928 Tokens), 11,8 s; zwei „Kritisch:“-Befunde mit Zitat aus dem Text, aber hinter der
    Teil-Angabe → Baruch erkannte sie nicht und forderte nach (zweite Anfrage, 3,1 s). Behoben, offline mit den echten
    Antwortformen getestet; eine erneute Prüfung gegen Eliezer steht aus (→ Backlog „Nach Runde 16 offen“).
  - **Sichtprüfung** (Wegwerf-Server, Firefox headless): Bilder unter `.claude/sicht/r16/` (`neu`, `arbeit` – Baum
    und Editorkopf mit Zeiten und Warnung –, `gliederung`, `einst-zeit`, `zettel`, `zettel-pdf` – A5 Seite 1/2 und
    eine Karte –, `antrag-neuer-teil`, `antrag-abgelehnt`). CLI `export … --stichwort` geprüft (Exit 0, 1, 2).
  - **Nachbesserung nach dem Gutachten:** Stichwortzettel überspringt Hervorhebungen in `#footnote`/`#arbeit`,
    Verschachtelung `#name[…]` höchstens 32 Ebenen (vorher Stacküberlauf, Server brach ab); „Kritisch:“ auch hinter
    fett gesetzter Teil-Angabe, mit Zusatz in Klammern und nach römischer Nummer. Details: `docs/Architektur.md`
    „Nachbesserung Runde 16“.
  - **Handprüfung durch den Nutzer mit einer eigenen Predigt** (nach dem Einspielen, Dienst 127.0.0.1:8765): Der
    erste Start führt die **Migration 0039** (Predigtzeit) aus – **vorher wie gewohnt sichern**
    (`~/.local/share/baruch` kopieren). 1. **Werk ▾ → Neues Werk / Importieren…** (ohne JavaScript `/werke/neu`):
    Titel, Art **Predigt**, Grundgerüst „passend zur Art“, im Feld **Predigttext** eine Stelle (z. B. „Lk 15,4–6“),
    unter **SWORD-Bibel für den Wortlaut** eine eigene Bibel wählen → Anlegen. Erwartet: Gliederung Predigttext ·
    Einleitung · I.–III. · Anwendung · Schluss, im Predigttext der Wortlaut als Zitat mit Stelle und Modul (ohne
    Bibel nur die Stelle). 2. In jeden Punkt einige eigene Sätze schreiben (in einen eine Fußnote mit `*…*` darin,
    in einen eine Zeile `// Stichwort: …`). 3. **⚙ Einstellungen → Werk → Predigtzeit**: Wörter je Minute lassen,
    Zielzeit knapp unter die angezeigte Gesamtdauer setzen → Baum, Editorkopf und `/gliederung` zeigen Zeiten und
    „Zielzeit überschritten um m:ss“; Zielzeit danach wieder passend setzen. 4. Akkordeon **Assistenten**, Persona
    Lektor, Umfang **ganzes Kapitel** (= ganze Predigt), **Prüfen** → Befunde je Teil mit „Kritisch:“, der Lektor
    spricht von der „Predigt“; unter dem Beitrag darf **kein** Hinweis „nennt keinen mit „Kritisch:“ markierten
    Befund“ stehen (Backlog „Nach Runde 16 offen“); erfundene Zitate tragen „(nicht im Text belegt)“. 5. **Werk ▾ →
    Stichwortzettel …**: Vorschau prüfen (Markierung, Hervorhebungen außerhalb der Fußnote, sonst erste Sätze;
    Zeitmarken wie im Baum), Format **A5** und **Karten**, Schriftgröße ändern, **PDF herunterladen**. 6. Kommandozeile:
    `baruch-server export <id|slug> --stichwort ~/predigt-zettel.pdf --format
    karten` → „Stichwortzettel geschrieben: …“; ein zweiter Aufruf mit derselben Datei endet mit Exit 1, ohne zu
    überschreiben. 7. Einen Antrag auf der Predigt (z. B. Umbenennen) ablehnen → im Spiegel ein Protokoll-Commit,
    die Gliederung bleibt.
- **PM-Abschluss Runde 16** (2026-11-08): voller Lauf nach `cargo clean -p` aller 13 Workspace-Crates (8,1 GiB
  entfernt), `nice -n 10 ionice -c3 cargo test --workspace -j 4 --no-fail-fast`: **118 Läufe, 1582 bestanden,
  0 fehlgeschlagen, 0 ignoriert** (Server-Bündel 707, Server-Einheitentests 188), EXIT 0, 101 s. Danach nur
  Begründungen zweier Zusicherungen und die Beschreibung von `outline_requests` geändert: Server-Tests erneut grün
  (707 + 188), `cargo fmt --check` sauber. `STUB:`-Zeilen: 9 (unverändert). Gutachten: 13 Befunde (0 hoch, 4 mittel,
  9 niedrig); alle mittleren in `7dc257c` behoben, drei niedrige im PM-Abschluss (Doku `outline_requests`,
  Zusicherungen, Backlog), sechs niedrige unter „Nach Runde 16 offen“ (→ R17/R23). Fahrplan: Runde 16 ≈ 125 min
  (03:07–05:10), Runde 17 auf 150 min. Worktrees `wf_930e8bdf-c1d-2/3/4` samt `target/` entfernt, Zweige `team/*`
  bleiben (gemergt). Nächste Runde: R17 „Bibelstellen (SWORD-Rest), Lektor-Reste“.
- **PM-Abschluss Runde 15** (2026-11-08): voller Lauf nach `cargo clean -p` aller 13 Workspace-Crates (8,7 GiB
  entfernt), `nice -n 10 ionice -c3 cargo test --workspace -j 4 --no-fail-fast`: **114 Läufe, 1490 bestanden,
  0 fehlgeschlagen, 0 ignoriert** (Server-Bündel 661), EXIT 0. `STUB:`-Zeilen: 9, davon 4 echte Stubs (`cover.rs` EPUB-Umschlag,
  `base.html` EPUB-Vorschau, `versification.rs` Verszählungen, `baruch-sword` verschlüsselte Module – dazu Tests, die
  das Fehlen alter Stubs prüfen). Gutachten: 15 Befunde (0 hoch, 7 mittel, 8 niedrig); alle mittleren in `ace0e36`
  behoben, `docs/MCP.md` (`instructions`) im Abschluss berichtigt, die übrigen sieben niedrigen im Backlog „Nach Runde
  15 offen“ (→ R16/R23). Fahrplan (Runde 15 mit Vorlauf 21:43–00:14 und Runde 00:14–02:45), Backlog und Plan
  nachgezogen; Worktrees `wf_e01bf6b0-7bf-3/4/5` samt `target/` entfernt (Zweige `team/lektor`, `team/anbieter`,
  `team/auftraege` bleiben, alle in `development` gemergt). Nächste Runde: 16 „Predigt (B4)“.
- **Nachbesserung nach dem Gutachten Runde 15** (2026-11-08, auf `development`): gestückelter Kontext wird je Aufruf
  gemessen (Kopf, Auftragszusatz, „wird geprüft, steht unten“ eingerechnet), zu viele Teile → ehrliche Ablehnung vor
  jedem Modellaufruf; Ollama meldet die wirksame Kontextlänge (`num_ctx`, sonst `/api/ps`, sonst 4096), nie den
  Höchstwert; keine Einbettung über ein als extern markiertes Eliezer; neue Modell-Verbindungen „nach der Adresse“
  lokal/extern, Warnung bei nicht erkennbar lokaler Adresse als lokal; `X-Frame-Options: DENY` und
  `frame-ancestors 'none'` auf jeder Antwort; Backlog-Marken „→ R15“ nachgezogen, Plan um Runde 15 ergänzt. Tests
  `tests/round15_nachbesserung.rs`.
- **Runde 15 „Lektor und Personas“ integriert** (2026-11-08, Integrator; vom Stand `ca04c58`, Zweige `team/anbieter`
  `8699f18`, `team/lektor` `9ee4362`, `team/auftraege` `dfda162`, mit `--no-ff` in dieser Reihenfolge; Schnittstellen
  R15-S1 (`6a7a6c1`) und R15-S2 (`70a085f`) lagen per cherry-pick in den anderen Zweigen; nicht gepusht; Worktrees und
  Zweige bleiben bis zum PM-Abschluss).
  - **lektor:** ganzes Kapitel (bei Predigten das Werk) im Wortlaut, Befunde der anderen Personas, `scope`
    section|chapter (Prüfen/Fragen über das Kapitel, Vorschlag je Teil nacheinander, `proposals`), Budget nur aus
    `state.model.context`, Stückeln/Komprimieren statt still kürzen (`Progress::Compressed`, Vermerk „Komprimiert: …“),
    „Kritisch:“ im Code (einmal nachgefordert, Ereignis `retry`), Vorschlag ohne „Kritisch:“-Zeilen, SWORD-Faktenblock
    (`assist/bible_facts.rs`), Toast-Baustein und Umfangsauswahl.
  - **anbieter:** `baruch_llm::Client` (Eliezer, OpenAI-kompatibel, Ollama), Verbindungen je Persona (Migration 0036),
    Freigabe „extern“ in `EliezerPort::chat`, Kontextabgleich (min aus Einstellung und Meldung), Modellliste aus dem
    abgelaufenen Zwischenspeicher, Kurztest gespeichert; SWORD-Kommentare und -Lexika (Migration 0037, RPC
    `commentary.*`/`lexicon.*`, MCP `commentary_lookup`/`lexicon_*`), Bausteine Kommentar-Panel/Treffergruppe,
    `bible_research::research`.
  - **auftraege:** SSE `posted`/`retry`/`compressed`, Hinweisseite statt 422, Abbrechen, `scope` im Formular, Einfall an
    gewählte Persona mit Herkunft, Literatur „an Persona geben“ (`literature.to_persona`), Hervorhebung übernehmen
    (`highlight.own`), Fäden ohne Abschnitt (Migration 0038, `/faeden`, `/faden`, `thread.*`), MCP `chat` ohne `key` mit
    Relay und Warteschlange je Client, Bibeln in der Literatur (`/literatur/bibel`, Gruppen „Bibel“/„Kommentar“, Zitat
    als Vorschlag).
  - **Integrator:** Konflikte nur in Verdrahtungs- und S1/S2-Dateien (Fassung des Besitzers). Behoben: `.toast` war
    doppelt belegt (Fehlermeldungen in `app.css` vs. Hinweis des Lektors – CSS nur noch für
    `[data-widget="toast"]`), zitierte `[Sn]` werden Quelle des Beitrags, OpenAI-Verbindungen melden nie eine
    Kontextlänge (Zwischenspeicher je Adresse), Grenze der MultiGPT-Prüfung je Anfrage über der gemeinsamen (flackernder
    Test `round13_quellen`), Hinweisseite mit Rand, MCP-`instructions`, `round9_menu` um `/faeden` und
    `/literatur/bibel`. Ende-zu-Ende `tests/round15.rs` (10 Tests, siehe `docs/Architektur.md` „Integration Runde 15“).
  - **Test:** `cargo clean -p` aller 13 Crates, dann voller Lauf; zuletzt `cargo test --workspace -j 4 --no-fail-fast`:
    **1483 bestanden, 0 fehlgeschlagen, 0 ignoriert** (114 Läufe, Server-Bündel 654); `cargo fmt --check`, clippy
    `-D warnings`, `tsc --noEmit` sauber; `grep -rn "SHIM R15" crates` leer; 9 `STUB:`-Zeilen; `target/` 13 GB.
  - **Handprüfung mit Eliezer** (Integrator, genau drei Anfragen an `google/gemma-4-26b-a4b` über einen Pförtner, der
    nur Chat und die lesende Modellliste durchließ, Einbettungen beim Start abwies und den Schlüssel selbst setzte;
    Baruch mit `--secrets memory`; erfundenes Kapitel „Der Wärter“, 3 Abschnitte, ≈ 2100 Wörter): (1) Prüfen ganzes
    Kapitel: 12,5 s, Kontext vollständig (alle drei Abschnitte im Wortlaut, nicht das nächste Kapitel, keine Kürzung,
    14 393 Zeichen Prompt), Antwort 1 423 Zeichen, je Abschnitt ein „Kritisch:“, nicht komprimiert; Eliezer meldet
    262 144 Tokens, wirksam bleibt die Einstellung 32 768. (2) Vorschlag für I.3: 11,3 s, 4 125 Zeichen, beginnt mit
    der Überschrift, keine „Kritisch:“-Zeile, keine verbotene Einleitung, gezielte Änderung nur an der verlangten
    Stelle, aktuelle Fassung bleibt. (3) „Passt das so?“: 3,1 s, mit „Kritisch:“ (keine Nachforderung nötig), aber der
    Befund stützt sich auf eine erfundene Tatsache (→ Backlog R16).
  - **Sichtprüfung** (Wegwerf-Server, Nachbau-Modell, erfundene SWORD-Testmodule): Bilder unter `.claude/sicht/r15/`
    (`assistenten`, `assistenten-nathan-unten` – Verbindung „Fernrechner“ extern ohne Freigabe mit Warnung und
    Kontextabgleich –, `assistenten-umfang-rechts`, `arbeit-13` – Lektor-Befund zum Kapitel und Chat –, `arbeit-14` –
    @belege mit [S1]/[S2] und Quellen, „an Persona geben“ mit Quelle „Bibel Klippenbibel, Ps 139,23“ –,
    `hinweis-komprimiert-js`/`-ohnejs`, `bibel` – Bibelseite mit Kommentar daneben –, `literatur-suche` – Gruppen
    „Bibel“ und „Kommentar“ –, `antrag-zustimmen` – Popup aus Nutzerfrage 12). Befunde daraus behoben (Toast-Klasse,
    Quellen für [Sn], OpenAI-Kontext, Rand der Hinweisseite); Rest im Backlog „Nach Runde 15 offen“.
  - **Handprüfung durch den Nutzer mit Werk 3** (nach dem Einspielen, Dienst 127.0.0.1:8765): Der erste Start führt
    die Migrationen 0035–0038 aus (vorher wie gewohnt sichern). 1. Einstellungen → Assistenten, Tab Lektor: unten
    „Modell-Verbindung“ = Eliezer (lokal), „Wirksamer Kontext“ prüfen (Einstellung 32768, Eliezer meldet mehr →
    Einstellung bleibt). 2. Werk 3, ein Abschnitt in Kapitel I, Akkordeon „Assistenten“: Persona Lektor, Umfang
    „Abschnitt“, „Prüfen“ → Befund mit „Kritisch:“, Bezüge auf Nachbarabschnitte. 3. Umfang „ganzes Kapitel“,
    „Prüfen“ → Befunde je Teil; Dauer notieren. 4. Kontext von @lektor testweise 8192, Schritt 3 wiederholen →
    mehrere Zusammenfassungs-Aufrufe, Hinweis „Komprimiert: …“ (mit JavaScript unten rechts mit ×, ohne JavaScript
    als Kasten auf der Hinweisseite) und Absatz „Komprimiert: …“ am Ende des Beitrags; danach 32768 zurück.
    5. „ganzes Kapitel“ + „Vorschlag“ → je geändertem Abschnitt ein Vorschlag (Notiz „… (ganzes Kapitel)“), keine
    „Kritisch:“-Zeile im Text, aktuelle Fassung bleibt; dauert etwa so viele Aufrufe, wie das Kapitel Teile hat.
    6. Frage an @nathan „Passt das so?“ → „Kritisch:“ (bei Fehlen kurz sichtbare Neuanforderung). 7. Einstellungen →
    Werk → „Kommentare und Lexika“ wählen (eigene SWORD-Module), dann Literatur → Bibeln → Stelle aufschlagen
    (Kommentar daneben), Suchfeld mit Gruppe „Bibel“/„Kommentar“, „An Persona geben“. 8. @belege nach einer Stelle
    fragen → [Sn] in der Antwort, darunter „Quelle: SWORD [Sn] …“. 9. Im Chat tippen: eigener Beitrag erscheint
    sofort, „Abbrechen“ meldet, was gespeichert ist.
- **Nachgang zu Runde 14 (2026-11-07, drei Nutzerentscheidungen)**, ein Entwickler im Hauptordner, nicht gepusht:
  - `a54523e` **Ortszeit statt UTC:** `baruch-server/src/localtime.rs` (chrono::Local, lag schon im Lock); alle
    Anzeigen (Fassungen, Einfälle, Bilder, Werke, Verbindungsprüfung, Abgleich, Hervorhebungen, MCP-Clients/Tokens,
    Abrufdatum der Websuche, Papierkorb-Ordner) rechnen erst beim Anzeigen um; gespeichert und RPC weiter UTC.
    Tests mit `FixedOffset`, `Local` im Kindprozess mit `TZ=Europe/Berlin`.
  - `36b7300` **Alte Teiltexte umwandeln** (Nutzerfrage 8 entschieden): Startschritt `Store::convert_legacy_parts`
    (`legacy_parts.rs`, Merker `store_meta.parts_marked_r14`, eine Transaktion) nach `mirror::sync_all`; leere Teile
    und reine „= …“-Zeilen (Titel des Teils oder beginnt mit „Teil“) → neue Fassung mit Teil-Markierung, Notiz
    „Umwandlung Runde 14 (Nutzerentscheidung)“, alte bleibt; eigene Texte bleiben (stderr nur Anzahl und ids); je
    Werk ein Spiegel-Commit. Neue Datenbanken setzen den Merker bei der Saat. **Beim Nutzer:** Der erste Start nach
    dem Einspielen wandelt einmal um; wie viele Teile, ist an seinen Daten nicht geprüft (nicht gelesen) – die
    stderr-Zeile im Journal des Dienstes nennt es. Sein Werk 1 (Beispielwerk, Saat vor diesem Stand) bekommt dabei
    ebenfalls Markierungen für seine leeren Teile.
  - `323e094` **Markdown-Überschriften:** `baruch_typst::markdown` (Regel, Tabellentest), angewandt in
    `outline.save`/`outline.check`/Rückfrage/`revision.save` und beim Übernehmen (`revision.accept`, Vorschlag
    bleibt, danach Fassung des Menschen, Feld `markdown`); Widgets `markdown.ts`, `panes.ts`, `wysiwyg.ts`.
    Ebene 4–6 → 3 (dokumentiert). Live-Umwandlung im Browser nur per `tsc` geprüft (→ Nutzer: im Browser tippen).
  - Voller Lauf `cargo test --workspace -j 4 --no-fail-fast`: **1375 bestanden, 0 fehlgeschlagen, 0 ignoriert**
    (108 Läufe, Server-Bündel 587); `cargo fmt --check`, clippy `-D warnings`, `tsc --noEmit` sauber.
- **Runde 14 „Teile und Gliederung“ abgeschlossen** (Integration, Gutachten, Nachbesserung `bba23db`, PM-Abschluss) (vom Stand `6906036`, Zweige `team/teile-typst` `74647cc`,
  `team/nummerierung` `67c8437`, `team/teile-gliederung` `5e06eb3`, mit `--no-ff` in dieser Reihenfolge; Schnittstellen-
  Commit `234c047` per cherry-pick in beiden anderen Zweigen; Konflikt nur in `tests/main.rs` (drei `mod`-Zeilen,
  alphabetisch wegen rustfmt); nicht gepusht; Worktrees nach dem PM-Abschluss gelöscht, Zweige `team/*` bleiben).
  - **Gutachten** (13 Befunde: 0 hoch, 5 mittel, 8 niedrig), **Nachbesserung `bba23db`:** Titel mit Code (`#emph[…]`)
    werden nicht zur Markierung „ohne Nummer“ verfälscht, sondern fallen auf die Show-Regel zurück; „Teil ohne Nummer“
    wird abgelehnt (RPC und Antrag), weil die Markierung es nicht tragen kann; `* k` in Funktionen der Werkvorlage
    höchstens 16 (vorher ließ `* 40000000000` den Prozess abbrechen); Architektur.md „Export und Spiegel“ nachgezogen;
    neuer Teil über MCP übernimmt vorhandene Kapitel – dokumentiert, Entscheidung beim Nutzer (Nutzerfrage 12).
  - **PM-Abschluss:** `cargo fmt` für drei Dateien der Nachbesserung, Begründung an der letzten Zusicherung ohne Meldung
    (`round14.rs`), Test `parts_without_chapters_and_a_part_at_the_end_survive_the_round_trip`
    (`baruch-store/tests/round14_parts.rs`), Backlog (veralteter Satz „aus der Werkvorlage byte-gleich“, Gutachten-Reste,
    Erledigt), Fahrplan; Bilder der Sichtprüfung dauerhaft unter `.claude/sicht/r14/` (nicht im Repo, `.gitignore`).
    Voller Lauf nach `cargo clean -p` aller 13 Crates: **1360 bestanden, 0 fehlgeschlagen, 0 ignoriert** (108 Läufe,
    Server-Bündel 582); danach mit den PM-Tests baruch-typst + baruch-store 366 grün, `round14` 19 grün, clippy
    `-D warnings` und `fmt --check` sauber; 13 `STUB:`-Zeilen.
  - **teile-typst:** `baruch_typst::marker` (Teil `#heading(level: 1, numbering: none)[T] <baruch-part>`, „ohne Nummer“
    `… <baruch-unnumbered>`, nur kanonische Form), `Heading.numbered`, Import (Teil → `part`, „ohne Nummer“ →
    `numbered = 0`), Satzbild-Blöcke `part`/`unnumbered`, Teil-Seite im Satz (`part_page_rule`, rechte Seite).
  - **teile-gliederung:** Teile anlegen auch mit Werkvorlage (`outline.add`/`outline_add` mit `level`, `adopted`,
    Formular, Kontextmenü), Umbenennen schreibt die Markierungszeile, Abgleich beim Speichern/Umschalten, Anträge mit
    Vorschau (`outline_request_preview`, `preview` in RPC/MCP, `<details>` auf `/gliederung#antraege`),
    `idea_to_outline` in einer Transaktion.
  - **nummerierung:** `Numbering::from_template` (Strings wörtlich, Wort-Schemata, Funktionen je Ebene), Projektion
    „ohne Nummer“ in Export, Export-Karte, Spiegel und Satz (`snapshot::project`, DB bleibt), Kopf der Seite
    Gliederungsschema.
  - **Integrator:** Import setzt `numbered` nach dem Einfügen ausdrücklich aus der Textform (der Trigger
    `node_numbered_default` aus 0006 machte Ebenen mit leerer Nummer – Predigt-Abschnitte – zu „ohne Nummer“, der Export
    projizierte sie; `round14_numbering` schlug nach dem Zusammenführen deshalb fehl). Fünf Tests auf die Projektion
    nach Vertrag §4 umgestellt (`works` 0005, `store` Spiegel, `outline` 0006, `round8_typst`, `round11_images`: auch
    bei „aus der Werkvorlage“ zählt Typst das Vorwort jetzt nicht mehr – wie Baruch). Entschieden: Werke „aus der
    Werkvorlage“ ohne Regel werden ebenfalls projiziert (Export → Import behält „ohne Nummer“). Ende-zu-Ende
    `tests/round14.rs` (5: Teil + Vorwort ohne Nummer → Export → Re-Import → Satz/PDF mit `#context assert.eq` und
    Gegenprobe, `counted_headings`; Teil über MCP, Umbenennen per Formular, Umschalten, Spiegel, Re-Import ohne Vorlage;
    Antrag mit Vorschau → Bestätigung ohne JS; `idea.process` atomar; Juristisch/Predigt Baum = Vorschau = Re-Import).
    Keine Migration, keine neue Abhängigkeit, `grep -rn "SHIM R14a" crates` leer, 13 `STUB:`-Zeilen (unverändert).
  - **Test:** `cargo clean -p` aller 13 Crates, dann `cargo test --workspace -j 4 --no-fail-fast`: **1355 bestanden,
    0 fehlgeschlagen, 0 ignoriert** (108 Läufe, Server-Bündel 582); `cargo fmt --check`, clippy `-D warnings`,
    `tsc --noEmit` sauber; `target/` 11 GB.
  - **Sichtprüfung** (eigener Server, `--secrets memory`, erfundenes Werk „Das Jahr am Leuchtturm“): Bilder unter `.claude/sicht/r14/` im Arbeitsordner (gliederung, teil-editor, bearbeiten-ebene, antraege-vorschau, gliederung-nach-antrag, nummerierung, satz, pdf-teil-3, pdf-buch-alle, gliederung-juristisch, pdf-juristisch-alle; je `.png`).
    Baum mit Teilen I/II und Vorwort ohne Nummer, Teil-Block im Satzbild, Feld „Ebene“ im Formular, zwei Anträge mit
    Vorschau (Marken „umbenannt“, „neue Ebene“), Bestätigung per Formular, PDF mit Teil-Seiten auf den Seiten 3 und 5
    (rechts), Kapitel 1, 1.1, 1.1.1, 2, 2.1, 3; unter „Juristisch“ § 1/Abs. 1/Nr. 1, Teile ohne Nummer in Baum und Satz.
    Nichts behoben nötig; aufgefallen: heller Streifen rechts auf „Satz“ und „Gliederungsschema“ (alt, Backlog).
    **Vorfall:** Der erste Start des Wegwerf-Servers bettete über die Vorgabe-Verbindung `127.0.0.1:1234` ein – eine
    Anfrage an Eliezer ohne Schlüssel (401), sofort beendet; danach Verbindungen auf `127.0.0.1:9` umgestellt
    (Umweg in `SKILLS.md`, Backlog → R19a).
- **Runde 13 „Quellen über MultiGPT, Websuche, Abgleich“ abgeschlossen** (Integration, Gutachten, Nachbesserung `1251a5b`, PM-Abschluss) (vom Stand `9b9d01c`, Zweige `team/quellen`
  `4c5f5e9`, `team/websuche` `d1b64bb`, `team/abgleich` `a58fb76`, mit `--no-ff` in dieser Reihenfolge; Konflikte nur
  in `tests/main.rs`, `static/app.css` (alle Blöcke behalten), `lib.rs` und `rpc.rs` (Zeilen aus §6 vereinigt);
  nicht gepusht; Worktrees und Zweige bleiben).
  - **quellen:** gemeinsame `connections::multigpt::Session` mit Vorrat, Volltext (`/quelle/multigpt`, `source.read`,
    MCP `source_read`), `source.adopt`, Prüfung von Hervorhebungen gegen den Volltext (Migration 0106).
  - **websuche:** Websuche für `@belege` (Schalter, Vorgabe aus), Prüfung der Zitate im Code, Quellenzeile, Belege
    (Migration 0034), Belegseite mit Übernahme in die Bibliothek; kein MCP-Werkzeug.
  - **abgleich:** `sync/` (Migration 0033), Hochladen je Kapitel nur nach Bestätigung, Holen gewählter Sammlungen
    als lokale Kopie, Art `remote` im lokalen Index, Seite `/abgleich`, Menü „Abgleichen“; kein MCP-Werkzeug.
  - **Integrator:** Shims gelöscht, Websuche und Abgleich auf `Session` umgestellt (`Session::call` gibt Daten ohne
    Token zurück, HTTP-Obergrenze der Sitzung 30 s wegen 30 s je Abgleich-Aufruf); `local_copy` →
    `sync::local_document` (ohne Sammlung nur eindeutige Dokument-ID); `web_evidence.page_hash` = `sync_hash`;
    `round9_menu` mit 27 Seiten; Ende-zu-Ende `tests/round13.rs` (4, eine MCP-Vorderseite über die drei Nachbauten).
    Keine `STUB:`-Zeile „Runde 13“ mehr, `open_part(Multigpt)` ist `None`.
  - **Test:** `cargo clean -p` aller 13 Crates, dann `cargo test --workspace -j 4 --no-fail-fast`: **1267 bestanden,
    0 fehlgeschlagen** (102 Läufe, Server-Bündel 558); nach der Nachbesserung Server erneut grün (+1 Einheitentest);
    fmt, clippy `-D warnings`, `tsc --noEmit` sauber; kein pgvector-Container übrig; `target/` 9,2 GB.
  - **Handprüfung mit dem echten Einbettungsmodell** (`text-embedding-nomic-embed-text-v1.5`, 768 Dimensionen, nur
    `/v1/embeddings`, Schlüssel nur im Programm; MultiGPT als Python-Nachbau): Sammlungen „Hausbuch“ (5) und
    „Leuchtfeuer“ (1) über den Abgleich geholt, Art `remote` eingebettet. „Imkerei, Wachs und süßer Ertrag“ → Die
    Bienen 0,687 (vor Schmied 0,668); „Astronomie: Planeten durch das Teleskop betrachten“ → Die Sterne 0,738 (vor
    0,608); „Glut und Hammerschläge in der Schmiede“ → Der Schmied 0,703 (vor Bienen 0,668). Rang 1 stimmt jedes Mal,
    bei zwei Anfragen knapp (Präfixe/Untergrenze → R19). Zwei Einbettungen beim Start schlugen mit 401 fehl, bevor
    der Schlüssel gesetzt war (erwartet, Wegwerf-Server).
  - **Sichtprüfung** (Nachbau, `--secrets memory`): `<Scratchpad>/hand13/bilder/dokumentansicht.png`, `<Scratchpad>/hand13/bilder/belege-chat.png`,
    `<Scratchpad>/hand13/bilder/belegseite.png`, `<Scratchpad>/hand13/bilder/abgleich.png`, `<Scratchpad>/hand13/bilder/assistenten-websuche.png`, `<Scratchpad>/hand13/bilder/einstellungen-werk-lang.png`.
    Danach behoben: Zeitpunkte auf `/abgleich`, Einstellungen → Werk und im Hinweis „lokale Kopie vom …“ als
    ISO-Stempel → `TT.MM.JJJJ HH:MM UTC` (`sync::display_time`), „1 Dokument“ statt „1 Dokumente“.
  - **Gutachten** (19 Befunde: 0 hoch, 7 mittel, 12 niedrig) und **Nachbesserung `1251a5b`**: gescheitertes Löschen
    der alten Fassung wird im nächsten Lauf nachgeholt (Stand `stale:`), der Abgleich holt seitenweise Dokumente
    ganz (`multigpt::read_all_pages`, gleich dem Online-Volltext), keine Wiederholung eines Aufrufs nach
    Zeitlimit/Abbruch (kein doppeltes `upload_document`), keine Websuche über MCP (`Order.web`), MCP `source_read`
    nur für Dokumente aus gewählten Sammlungen, lokaler Kopie oder verknüpftem Titel, Zitate mit „…“ nur aus Stücken
    von mindestens drei Wörtern an Wortgrenzen mit höchstens 400 Zeichen Lücke; Kommentare/Befunde im Abgleich als
    Abweichung dokumentiert (Nutzerfrage 11 im Backlog).
  - **PM-Abschluss:** `cargo clean -p` aller 13 Crates, dann `cargo test --workspace -j 4 --no-fail-fast`: **1274
    bestanden, 0 fehlgeschlagen** (102 Läufe, Server-Bündel 563). Danach behoben (niedrige Befunde): Doc-Kommentar
    von `collection_for`, „1 Dokument bei MultiGPT“, sieben Zusicherungen mit Begründung, Hinweis am Schalter der
    Websuche, dass die Anfrage (bis 200 Zeichen Buchtext) an die Suchmaschine geht, ein Clippy-Befund in
    `sync/mock.rs`; Server erneut 563 + 147 grün, clippy `-D warnings` und fmt sauber. Fahrplan, Backlog („Nach
    Runde 13 offen“, sechs niedrige Befunde nach R19/R23), Plan nachgezogen. 13 `STUB:`-Zeilen, keine für Runde 13.
    Worktrees und Zweige `team/quellen`, `team/websuche`, `team/abgleich` entfernt.
  - **Vom PM entschieden (Einspruch jederzeit):** @belege sucht beim Vorschlag nicht im Web; Daten der Websuche in
    UTC; Ausschalten des Hochladens lässt Dokumente bei MultiGPT stehen. Offen beim Nutzer: Annahmen über das echte
    MultiGPT in `docs/Architektur.md` prüfen; Kommentare/Befunde hochladen (Nutzerfrage 11).
- **Runde 12 „Verbindungen nutzen, RAG“ integriert** (vom Stand `cfad08b`, Zweige `team/lancedb` `32b6fc9`,
  `team/pgvector` `8ad37df`, `team/multigpt` `b7352a7`, mit `--no-ff` in dieser Reihenfolge; Konflikte nur in den
  Vertragszeilen der Workspace-`Cargo.toml`; `baruch-rag` in allen drei Zweigen byte-gleich (`git diff` leer);
  nicht gepusht; Worktrees und Zweige bleiben).
  - **lancedb:** Crates `baruch-rag` und `baruch-lancedb` (lancedb 0.41, Apache-2.0, braucht `protoc`), Einbettungen
    über Eliezer (`baruch-llm` `embed.rs`, eine Anfrage zur Zeit), Index im Datenordner, Dienst `src/rag/`
    (RRF-Zusammenführung, Status ohne Netz, RPC `rag.*`, `check_lancedb`).
  - **pgvector:** Suchweg direkt (READ ONLY, 10 s, Zuordnung der Tabelle), Sammlungen je Werk (Migration 0031),
    Einstellungen → Verbindungen mit Sammlungen, Status der Wege, „Index neu aufbauen“; Umschalter Wortlaut/Bedeutung
    in Literatur und Einfällen.
  - **multigpt:** MCP-Client (Streamable HTTP, `nextCursor`, `list_collections`), Suchweg MultiGPT, MCP
    `rag_search`/`rag_status`, Treffer als Belege `[Rn]` für @belege und den Lektor (Migration 0032).
  - **Integrator:** Haken ersetzt (`rag::hooks` → Store/`pgvector::open_way`/`multigpt::open_way`, `search_hooks` und
    `assist::hooks` → `crate::rag`), `run_check` LanceDB → `rag::check_lancedb` (STUB und Code `stub_lancedb` weg),
    `serve` ruft `rag::catch_up`, `work.delete` → `rag::notify`, Hervorhebungen → `notify(None)` und Werk; Zwischenspeicher
    der Einbettung nach Alias geschlüsselt (LanceDB und pgvector teilen die Anfrage auch mit fremdem Einbetter);
    der Satz „Die Wortlaut-Suche (FTS) geht weiter.“ steht auf den Seiten nur einmal; Ausschnitte ohne
    Überschriftszeile und Leerzeilen; MCP-`instructions` nennen `rag_*`; STUBs „Abgleichen“ und `highlights.rs` auf
    Runde 13. Tests der Zweige auf den verdrahteten Stand umgestellt (round9_connections, round12_lancedb,
    round12_multigpt, round12_search_ui, round12_personas_rag). Ende-zu-Ende `tests/round12.rs` (6, mit eigenem
    pgvector-Container `docker.io/pgvector/pgvector:pg17`, danach entfernt). `docs/runde12/` in Bedienung, Architektur,
    RPC, MCP, Plan, SKILLS, Backlog übertragen und gelöscht.
  - **Test:** `cargo clean -p` aller 13 Workspace-Crates, dann `cargo test --workspace -j 4 --no-fail-fast`:
    **1187 bestanden, 0 fehlgeschlagen** (159 Läufe), danach mit einem weiteren Einheitentest 1188; clippy `-D warnings`
    sauber, `cargo fmt --check` sauber, `tsc --noEmit` ohne Fehler. 21 `STUB:`-Zeilen (vorher 24: drei
    Nutzungstexte und LanceDB-Prüfung weg, `MultigptWay::read_full` neu). `target/` ≈ 40 GB.
  - **Handprüfung mit dem echten Einbettungsmodell** (`text-embedding-nomic-embed-text-v1.5`, 768 Dimensionen,
    nur `/v1/embeddings`, Schlüssel nur im Programm): erfundenes Werk mit fünf Abschnitten (Bienen, Schmied,
    Seefahrt, Sterne, Abendgebet), Index in 2 s. „Imkerei, Wachs und süßer Ertrag“ → Bienen 0,711 (vor Schmied
    0,678); „Astronomie: Planeten durch das Teleskop betrachten“ → Sterne 0,761 (vor 0,620); „Schiffbruch auf hoher
    See“ → Seefahrt 0,671 (vor Bienen 0,666). Rang 1 stimmt jedes Mal, der Abstand ist bei zwei Anfragen knapp
    (Anfrage ohne `search_query:`-Präfix, Backlog).
  - **Sichtprüfung** (Nachbau von Eliezer und MultiGPT in Python, `--secrets memory`): `<Scratchpad>/hand12/bilder/literatur-rag.png`,
    `<Scratchpad>/hand12/bilder/einfaelle-rag.png`, `<Scratchpad>/hand12/bilder/verbindungen.png`, `<Scratchpad>/hand12/bilder/belege-chat.png`. Danach behoben: Überschriftszeile und
    Leerzeilen in den Ausschnitten. Aufgefallen, offen: Treffer mit 0–4 % Ähnlichkeit füllen die Liste auf (keine
    Untergrenze).
  - **Gutachten** (18 Befunde: 0 hoch, 6 mittel, 12 niedrig) und **Nachbesserung `77330aa`**: abgelehnte
    Hervorhebungen fehlen im Index, SSE-Stückgrenze mitten in einem Umlaut bricht MultiGPT nicht mehr ab, der
    geprüfte Abschnitt ist nie sein eigener Beleg, `GET /literatur?…&lmode=rag` und `/einfaelle?…&mode=rag` von
    fremden Seiten suchen nicht (`guard::check`), höchstens 4 gleichzeitige Suchen auf pgvector/MultiGPT; Größe von
    `target/` und fehlende Revisionen/Befunde im Index ins Backlog. Die niedrigen Reste stehen im Backlog „Nach
    Runde 12 offen“; im PM-Abschluss richtiggestellt: Nutzungstext von Eliezer (Einbettungen), Bedienung zur Suche
    der Arbeitsseite, Backlog zur MultiGPT-Prüfung, Fahrplan.
  - **PM-Abschluss:** `cargo clean -p` aller 13 Workspace-Crates, dann `cargo test --workspace -j 4
    --no-fail-fast`: **1195 bestanden, 0 fehlgeschlagen, 0 ignoriert** (159 Läufe), kein pgvector-Container übrig,
    `target/` 40 GB; nach dem Nutzungstext round9_connections/round10_models/round12 erneut grün, fmt und clippy
    sauber; 21 `STUB:`-Zeilen. Worktrees `wf_0ad2ab4d-e28-2/3/4` samt `target/` danach gelöscht.
- **Runde 11 „Bilder und Umschlag“ integriert** (Stand `59c8c45`, Zweige `team/mcp-assets` `2ce37f8`,
  `team/cover` `3320137`, `team/images` `0bf6f75`, mit `--no-ff` in dieser Reihenfolge; ein Konflikt in `rpc.rs`
  (Dispatch-Zeilen beider Zweige) gelöst; nicht gepusht; Worktrees und Zweige bleiben).
  - **mcp-assets:** Buch als PDF (`baruch-render::compile_pdf`, `src/pdf.rs`, RPC `work.pdf`, `/werke/{id}/pdf`,
    CLI `export --pdf`), Druckprüfung (`PrintWarning`, 300 dpi), MCP `asset_*` mit Anträgen (Migration 0018).
  - **cover:** Buchangaben und Umschlag (Migration 0017), Rahmen im Satz (Umschlag-Vorder-/Rückseite, Titelei),
    Umschlag-PDF mit Rücken/Beschnitt, Umschlagseite, Einstellungen → Werk.
  - **images:** Einstellungen → Assistenten mit Tab je Persona (Wunsch des Nutzers: „Jede Rolle ein Tab und
    Textfelder“), „@…“-Vervollständigung, Bilder-Suche, Vorschaubilder, Bild einfügen, Kapitelbild.
  - **Integrator:** Haken ersetzt (`typeset::prepare` → `cover::frame`, `cover::to_pdf` → `pdf::source_to_pdf`,
    Druckprüfung der Umschlagseite aus `Rendered.print`, Bilder-Seite → `cover::cover_name`/`assets_panel`,
    `asset_requests::panel`, `asset_requests::hook_cover_name` → `cover::cover_name`), `LibraryView.cover` +
    Kontext `cover` für `tree.html`, `/mcp` mit `RPC_BODY_LIMIT` (8 MiB über `asset_add`), Werkzeugnamen-Tests nehmen
    `asset_*` aus (Beschreibung beginnt mit „Antrag“), `back` im Umschlag-Block unmaskiert. Ende-zu-Ende
    `tests/round11.rs` (5). `docs/runde11/` in Bedienung, Architektur, RPC, MCP, Plan, SKILLS übertragen und gelöscht.
  - **Test:** `cargo clean -p` aller zehn Crates, dann `cargo test --workspace -j 4 --no-fail-fast`: **1046 Tests,
    0 fehlgeschlagen** (136 Läufe); clippy `-D warnings` sauber, `cargo fmt --check` sauber, `tsc --noEmit` ohne
    Fehler. 23 `STUB:`-Zeilen (neu: Umschlag als cover-image im EPUB).
  - **Sichtprüfung** (eigener Server `--secrets memory`, erfundenes Werk „Das Jahr am Leuchtturm“, selbst
    erzeugtes dunkles Hochformat 832 × 1216): `<Scratchpad>/sicht11/bilder/werke.png`, `<Scratchpad>/sicht11/bilder/arbeitsbereich.png`, `<Scratchpad>/sicht11/bilder/bilder.png`,
    `<Scratchpad>/sicht11/bilder/einstellungen-werk-unten.png`, `<Scratchpad>/sicht11/bilder/einstellungen-werk-lang.png`, `<Scratchpad>/sicht11/bilder/satz-1.png`,
    `<Scratchpad>/sicht11/bilder/satz-titelei-rueckseite.png`, `<Scratchpad>/sicht11/bilder/umschlag.png`, `<Scratchpad>/sicht11/bilder/pdf-buch-alle.png`, `<Scratchpad>/sicht11/bilder/pdf-umschlag-1.png`,
    `<Scratchpad>/sicht11/bilder/assistenten.png`.
  - **Offen:** Vorschaubilder mit `loading="lazy"` erscheinen im Headless-Bildschirmfoto teils leer (Firefox
    lädt sie dort nicht vor dem Foto; Adresse liefert 200); poppler meldet bei typst-PDFs „Suspects object is
    wrong type“ (harmlos, Seiten korrekt); `ImageView` reist noch unter `IMAGE_VIEW_KEY` in `image_alts` statt in
    einem eigenen Feld; Bild einfügen nur am Abschnittsende; persona_tabs/mention im Browser nur als Bild geprüft.
- **Runde 10 (B2) „backend · personas · scripture“ erledigt** (Team-Runde vom Stand `c1e56ed`, Zweige
  `team/backend` `d82fc61`, `team/scripture` `3d81ddd`, `team/personas` `54daa76`; mit `--no-ff` in der
  Reihenfolge backend → scripture → personas zusammengeführt, ohne Konflikte; nicht gepusht; Worktrees und
  Zweige bleiben). Personas rufen jetzt wirklich ein Modell auf (Eliezer).
  - **backend:** Crate `baruch-llm` (gestreamtes `chat/completions`, SSE-Parser, Grenzen, `Queue` = eine Anfrage
    zur Zeit, Aliasse, `mock::MockEliezer`), `src/models.rs` (RPC `model.list/status/probe`, Schlüssel nur bei
    `secret_bound`), Kurztest und Persona-Abgleich auf der Verbindungsseite.
  - **personas:** Port `assist::ModelPort`, Prüfen/Vorschlag/„@persona fragen“, `/assistenten/act` (ohne JS) und
    `/assistenten/stream` (Wort für Wort, Abbrechen), Rollen-Prompts mit harten Regeln (Migration 0015),
    `LectorChoice` (Vorgabe fest `@lektor`, Versuch), Lektor kennt die Einfälle, MCP `ask_persona`/`chat`,
    Einfall an Persona; Migrationstests 0011–0014 zukunftsfest.
  - **scripture:** Crate `baruch-scripture` (66 Bücher, deutsche Verszählung der Schlachter 1951 nach SWORD
    „German“ – die eBible-Tabelle war englisch und ist ersetzt –, nur Zahlen), RPC `scripture.check`, MCP `scripture_check`, Faktenblock für @schrift/@belege; MCP-Tokens
    (Migration 0016, nur SHA-256-Hash), Seite „MCP-Zugang“, `guard::remote_only_mcp`.
  - **Integrator:** Dispatch in `rpc.rs`, `assist/eliezer.rs` (`EliezerPort`, in `serve`), Hooks →
    `crate::scripture`, `scripture_check` in TOOLS, `remote_only_mcp` im Router; Ende-zu-Ende
    `tests/round10.rs` (7: Lektor fest per Formular, @schrift mit Faktenblock und „Wortlaut: ungeprüft“, Vorschlag
    nur proposed, „Als Assistent“ bereinigt, Strom, Modellliste aus `/v1/models` auf der Einstellungsseite, MCP mit
    Token von 192.0.2.44 inkl. `scripture_check`, zwei gleichzeitige Aufrufe beim Nachbau nacheinander). Nach der
    Sichtprüfung behoben: Zeilenumbrüche in Beiträgen (`.post-body`, `pre-line`), Regel 4 nennt die Ausnahme für
    @schrift, Vorschläge ohne „Kritisch:“-Anmerkung (Regel 6 und Auftrag), neue Umgebungsvariable
    `BARUCH_MAX_TOKENS`. `docs/runde10/` in Bedienung, Architektur, RPC, MCP, Plan, SKILLS übertragen und gelöscht.
  - **Test:** `cargo clean -p` aller zehn Workspace-Crates, dann `cargo test --workspace -j 4 --no-fail-fast`:
    **950 Tests, 0 fehlgeschlagen** (123 Läufe); nach den Nachbesserungen erneut **951, 0 fehlgeschlagen**; clippy
    `-D warnings` ohne Meldung, `cargo fmt --check` sauber, `tsc --noEmit` ohne Fehler. 22 `STUB:`-Zeilen (Soll §8).
  - **Sichtprüfung mit Nachbau** (eigener Server `--secrets memory`, Python-Nachbau von Eliezer, Firefox headless
    über Marionette, eigenes Profil): Chat mit gestreamter Antwort und „Abbrechen“, fertige Beiträge von @lektor und
    @schrift, Einstellungen Assistenten (Lektor-Wahl „Versuch“, VRAM-Hinweis, Alias-Warnungen, Rollen-Prompt),
    MCP-Zugang mit einmal gezeigtem Token, Verbindungen mit Kurztest. Bilder: `<Scratchpad>/sicht10/bilder/chat-strom-1400.png`,
    `<Scratchpad>/sicht10/bilder/chat-fertig-1400.png`, `<Scratchpad>/sicht10/bilder/assistenten-1400.png`, `<Scratchpad>/sicht10/bilder/assistenten-prompt-1400.png`, `<Scratchpad>/sicht10/bilder/mcp-token-1400.png`,
    `<Scratchpad>/sicht10/bilder/verbindungen-1400.png`.
  - **Handprüfung mit echtem Eliezer** (Freigabe des Nutzers 2026-11-06; eigener Server `--secrets keyring`,
    Wegwerf-Ordner, nur lesend, kein `set_secret`, Adresse Vorgabe, nur `google/gemma-4-26b-a4b`, nacheinander,
    `BARUCH_MAX_TOKENS=300`, frei erfundenes „Senfkornbuch“): `connection.check` ok in 26 ms (16 Modelle), Kurztest
    60 ms („OK“), je Persona eine Frage 0,6–1,3 s, Antworten 190–480 Zeichen; Regeltreue gut (Deutsch, keine
    verbotene Einleitung, „Kritisch:“ bei Lektor/Exeget/Nathan, Nathan ohne Freigabe, Hörer als Leser, Schrift und
    Belege nur Zuordnung, Ps 151,1 ungültig, Wortlaut ungeprüft). Befund (behoben, erneut geprüft): Vorschlag des
    Lektors enthielt eine „Kritisch:“-Anmerkung im Abschnittstext. Browser-Runde: `<Scratchpad>/sicht10/bilder/echt-strom-1.png`,
    `<Scratchpad>/sicht10/bilder/echt-fertig-unten.png` (drei Personas im Chat). Details im Backlog „Nach Runde 10 offen“.
  - **Gutachten und Nachbesserung** (`6e1e268`): drei Blickwinkel (Korrektheit, Sicherheit, Plan), 20 Befunde
    (2 hoch – beide dieselbe Sache –, 8 mittel, 10 niedrig). Behoben: Verszählung war durchgehend englisch (KJV,
    31102 Verse) → deutsche Zählung „German“ nach SWORD (31172 Verse, Tests mit Ps 51,21, Neh 3,38, Joel 4,1,
    3. Joh 15 …); `finish_reason` „length“ → Beitrag mit Hinweis und `cut: true`, abgeschnittener Vorschlag wird
    abgelehnt; Kürzen des Chatverlaufs nimmt jetzt die ältesten Beiträge zuerst; Komma vor nummeriertem Buch
    („Joh 3, 1. Joh 4“); @lektor im Beispiel 32768 statt 65536; MCP mit Token darf nicht als Persona handeln;
    Lektor-Prompt ehrlich („nicht das ganze Werk“); Hinweise „Stufe B2“ auf die offenen Runden umgestellt.
    Nicht behoben, niedrig, im Backlog „Nach Runde 10 offen“: Abbruchmeldung im Chat, Uhrzeiten als Stelle,
    Handle-Gleichheit bei Token-Namen, Warteschlange ohne Grenze je Token, Prüfung verbotener Einleitungen auch
    beim Vorschlag, „Kritisch:“ im Vorschlag nur per Prompt, neun Zusicherungen ohne Begründung, Schema von
    `chat` (`key` Pflicht trotz STUB-Zweig). Fahrplan und Bilder-Hinweis im PM-Abschluss erledigt. Danach
    `cargo test`: **959, 0 fehlgeschlagen**; PM-Abschluss: `cargo clean -p` aller Workspace-Crates, dann
    `cargo test --workspace -j 4 --no-fail-fast`: EXIT 0, 123 Läufe, **959 bestanden, 0 fehlgeschlagen, 0 ignoriert**;
    22 `STUB:`-Zeilen.
  - **Bilder** liegen im Sitzungs-Scratchpad unter `/tmp` (RAM-Dateisystem): Sie verfallen mit der Sitzung; der
    dauerhafte Beleg ist die Beschreibung hier und im Backlog.
  - **Offen:** Musterpersonas noch mit alten Modellnamen; Lektor mit ganzem Kapitel (Plan);
    Versuch mit dem echten Buch (Werk 3, Kapitel I) beim Nutzer.
- **Runde 9 „menu · settings · connections“ erledigt** (Team-Runde, Vertrag nach Backlog `596ba72`, Zweige von
  `f851000`: `team/menu` `b239701`, `team/settings` `ed8a456`, `team/connections` `21c7305`; mit `--no-ff` in der
  Reihenfolge menu → settings → connections zusammengeführt; Konflikte nur in den Vertragszeilen von
  `server/src/lib.rs` und `rpc.rs` sowie `workspace.html` (Zoom-Zonen von development behalten); nicht
  gepusht; Worktrees und Zweige bleiben). **Achtung:** Der Merge-Commit von connections ist `937f8fa` und trägt
  den Betreff eines gleichzeitigen Backlog-Commits einer anderen Sitzung („Backlog: Modelle der Personas über
  Eliezer …“); Inhalt ist der Merge samt Verdrahtung, erklärt in `9111c35`.
  - **menu:** eine Kopfzeile in `base.html` (`{% block topbar %}`, Seiten nur `page_title`/`page_actions`),
    Menüs Werk/Ansicht/Werkzeuge als `<details>`, `menu.ts` (ARIA-Tastatur), Abzeichen (`menu.badges`),
    Thema-Umschalter klein, „⚙ Einstellungen“ (Alt+,), Hülle des Einstellungs-Dialogs am Seitenende.
  - **settings:** `src/settings/` mit `registry()` (sieben Seiten), Rahmen `/einstellungen/<seite>` und
    `?fragment=1`, `settings.ts` (Popup), Darstellung (Cookies, neu `baruch_pv`), Werk (Blöcke aus
    `work_edit.html`/`work_numbering.html`, Feld `back`), Assistenten (`persona.update`, Lektor-Anzeige),
    MCP-Zugang (`session::clients()`), Daten (Sicherung jetzt, `VACUUM INTO`); RPC `settings.*`.
  - **connections:** Migration 0013 (`connection`, `work_rag`, `preferred_source`), `SecretStore`
    (`KeyringSecrets` im Dienst, `MemorySecrets` in allen Tests), Prüfen von Eliezer/MultiGPT/pgvector,
    RPC `connection.*`/`sources.*`, Formulare, MCP `sources_list`; neue Crates keyring 3, reqwest 0.12,
    sqlx-postgres.
  - **Integrator:** `settings::registry()` zeichnet Verbindungen/Quellen mit `crate::connections::render_*`
    (`pending_page` entfernt); Kopfzeilen-Test (`round9_menu.rs`) auf Werk bearbeiten, Gliederungsschema und
    alle sieben Einstellungsseiten ausgedehnt, dazu frühere Kopfzeilen-Adressen dieser Seiten; Ende-zu-Ende
    `tests/round9_integration.rs` (4): jede Einstellungsseite als Fragment, ohne JavaScript und über
    `settings.render`; Geheimnis per Formular → nur „hinterlegt“, Prüfen gegen Test-Server; Quellen-CRUD und
    `sources_list`; Darstellung (Cookie) und Assistenten (SQLite, Lektor) wirken. `docs/runde9/` in Bedienung,
    Architektur, RPC, MCP, SKILLS übertragen und gelöscht; `cargo fmt` über den Workspace (`fb98dc9`).
  - **Test:** `cargo clean -p` aller acht Workspace-Crates, dann `cargo test --workspace -j 4 --no-fail-fast`:
    **833 Tests, 0 fehlgeschlagen, 0 ignoriert** (105 Läufe, der Wegwerf-Cluster PostgreSQL lief); clippy
    (`--all-targets -D warnings`) ohne Warnungen, `cargo fmt --check` sauber, `tsc --noEmit` ohne Fehler.
  - **Sichtprüfung** (eigener Server auf freiem Port, Temp-Daten mit dem erfundenen „Gartenbuch“, Firefox
    headless, eigenes Profil) bei 1400×860 und 1024×768: Kopfzeile einzeilig, nichts läuft aus dem Bild
    (Titel und Autor gekürzt), Menü „Werk“ aufgeklappt (über eine gespeicherte Seite mit `<details open>`),
    Einstellungen Darstellung, Verbindungen, Assistenten ohne Auffälligkeiten; keine Änderung nötig. Der
    Server las über den echten Schlüsselbund nur den Zustand („baruch/eliezer: hinterlegt“), nie ein
    Geheimnis. Bilder (Scratchpad, nicht im Repo): `<Scratchpad>/sicht9/bilder/arbeit-1024.png`, `<Scratchpad>/sicht9/bilder/arbeit-1400.png`,
    `<Scratchpad>/sicht9/bilder/menu-offen-1024.png`, `<Scratchpad>/sicht9/bilder/menu-offen-1400.png`, `<Scratchpad>/sicht9/bilder/darstellung-1024.png`,
    `<Scratchpad>/sicht9/bilder/verbindungen-1024.png`, `<Scratchpad>/sicht9/bilder/assistenten-1400.png` (je Seite beide Größen vorhanden).
  - **Gutachten und Nachbesserung** (`8f74107`): 17 Befunde (1 hoch, 6 mittel, 10 niedrig); hoch und mittel
    behoben: Geheimnis an das Ziel gebunden (Migration 0014 `connection.secret_target`; nach Adressänderung
    schickt „Verbindung prüfen“ nichts, `secret_withheld`; Diebstahl über `connection.set`+`check` für Bearer und
    Klartext-Passwort getestet), Geheimnis vor dem Speichern geprüft (`gespeichert_ohne_geheimnis`), abgelehnte
    Werk-Formulare im Popup behalten Rahmen und Eingabe (`settings::work_retry_page`), „Sicherung jetzt“ höchstens
    10 Sicherungen und 512 MiB Rücklage (`statvfs`), `serve --secrets keyring|memory` (Vorgabe Schlüsselbund nur
    mit dem Vorgabe-Datenordner), „Fassungen“ unter Darstellung, `--warning` hell #9a5800 (5,07 : 1). Niedrige
    Befunde im Backlog; PM hat „Vergrößern mit Strg+Mausrad“ in Bedienung.md aus dem Kapitel Einstellungen gelöst.
  - **Test nach der Nachbesserung (PM):** `cargo clean -p` aller acht Workspace-Crates, dann
    `cargo test --workspace -j 4 --no-fail-fast`: **843 Tests, 0 fehlgeschlagen, 0 ignoriert** (106 Läufe,
    EXIT 0); `cargo fmt --check` sauber. 31 `STUB:`-Zeilen in `crates` (B2, Runde „Verbindungen“, EPUB-Vorschau).
  - **Offen** (Backlog „Nach Runde 9 offen“): Maus-/Tastenbedienung von Menü und Popup im Browser des Nutzers,
    echter Schlüsselbund einmal prüfen, MultiGPT-`nextCursor`, Nutzung der Verbindungen (Runde „Verbindungen“).
- **Runde 8 „inline · posts · typst“ erledigt** (Team-Runde vom Stand `f601260`, Worktrees ab 17:05, Zweige
  `team/inline` `326e259`, `team/posts` `b4976fa`, `team/typst` `0bb0263`; zusammengeführt mit `--no-ff` in der
  Reihenfolge posts → inline → typst ohne Konflikte; Worktrees und Zweige bleiben; nicht gepusht; keine
  Migration):
  - **inline:** `baruch_wysiwyg::align`/`hunk_html_with`; Vorschläge an ihrer Stelle im Lesefluss
    (`section.pv-flow`, je Stück `details.pv[data-pv-at]`, im Satzbild per `wysiwyg.ts` zwischen die Blöcke
    gesetzt), Basis = gezeigte Fassung, ein Kopf, „Teil i/k“; Formulare je Vorschlag außerhalb der Stücke
    (`form=`). **Ursache „Quelltext nach dem Laden“ belegt** (`docs/WYSIWYG.md`): leeres Blatt vom Server, das
    Widget schaltete erst auf Quelltext und öffnete das Satzbild nach zwei RPC-Antworten; jetzt
    `Settings.editor` aus Cookie `baruch_editor` und fertiges Satzbild im HTML (Test scheiterte vorher).
    Linien `--pc-line`/`--pc-line-dark` ≥ 3 : 1.
  - **posts:** `/?work=W&key=K` überall (Auflösung mit Rückfall und Hinweis), keine erzeugten `node=`-Links;
    `Post.id/created`; Store `post_get/delete/move/to_idea`; RPC `post.*`, Formular `POST /beitraege/act`,
    MCP `post_move/post_delete/post_to_idea` (nur eigene); `url` in MCP-Antworten, `SearchHit::node_key`;
    Chat-Abzeichen und `chat.ts`; unveränderte leere/überlange Titel speicherbar; `href()` im Release geprüft.
  - **typst:** Crate `baruch-render` (typst 0.15.1, dazu bewusst `typst-layout` für `PagedDocument`, kein Netz),
    `Store::export_map` (byte-gleich mit `export_typst`), `src/typeset.rs` (`preview.typst`,
    `/werke/{id}/satz`, SVG-Route mit CSP `sandbox`, statischer Zwischenspeicher 8, eine Semaphore, 60 s),
    MCP `typst_check`; die zwölf Gliederungsschemata setzen fehlerfrei.
  - **Integrator:** `typst_check` in den Werkzeuglisten-Tests (`mcp.rs`, `round4.rs`), Weiterleitungen in
    `tests/proposals.rs` streng auf die Key-Form; Ende-zu-Ende `baruch-server/tests/round8.rs` (5): MCP-`url`
    öffnet nach dem Umstellen den richtigen Abschnitt und alle Links nutzen `key`; zwei Persona-Vorschläge
    zwischen den richtigen Absätzen mit je einer Kopfzeile, einer per Cookie eingeklappt, einer per Formular
    übernommen (ein Commit); Beitrag per MCP umgewandelt (ein Commit mit `einfaelle.md`), fremde verweigert,
    vom Menschen verschoben und (mit Bestätigung) gelöscht; unveränderter leerer Titel speicherbar, neu
    geleerter nicht; Typst-Seite mit Bild als SVG und Fehler mit Abschnitt und Key-Link, `typst_check`.
    Doku-Entwürfe aus `docs/runde8/` in Bedienung, Architektur, RPC, MCP, SKILLS übernommen, `docs/runde8/`
    gelöscht; Plan (Ausnahme Beiträge), Backlog („Nach Runde 8 offen“), Fahrplan.
  - **Test:** `cargo clean -p` aller acht Workspace-Crates, dann `cargo test --workspace -j 4 --no-fail-fast`:
    **728 Tests, 0 fehlgeschlagen, 0 ignoriert** (94 Läufe); clippy (`--all-targets -D warnings`) ohne
    Warnungen, `cargo fmt --check` sauber, `tsc --noEmit` ohne Fehler.
  - **Sichtprüfung** (eigener Server auf freiem Port, Temp-Daten, erfundenes „Teichbuch“ mit Bild, Vorschläge
    von @lektor und @nathan über MCP, drei Beiträge; Firefox headless, eigenes Profil, Cookie `baruch_folded`
    in `cookies.sqlite`). **Nach dem Laden ist das Satzbild aktiv** (alle Fotos). Behoben: Kopfzeile lief mit
    dem neuen Knopf „Satz“ bei 1400 px aus dem Fenster („Thema“ abgeschnitten, Werktitel Wort für Wort
    umgebrochen) – Knöpfe behalten ihre Breite, Titel und Autor kürzen sich (`static/app.css`); der Hinweis
    „Vorschau (Platzhalter – Typst-Satz folgt)“ verweist jetzt auf „Satz“ (`editor.html`). Bildschirmfotos
    (Scratchpad, nicht im Repo): `<Scratchpad>/sicht/bilder/1-lesefluss-offen.png` (beide offen, Satzbild),
    `<Scratchpad>/sicht/bilder/2-editor-lektor-eingeklappt.png` (Lektor als blaue Linie zwischen Frosch und Libellen, Nathan offen
    nach dem Schilf; Chat mit Abzeichen 3 und „Verwalten“), `<Scratchpad>/sicht/bilder/3-vorschau-eingeklappt.png`,
    `<Scratchpad>/sicht/bilder/4-satz-typst.png` (Typst-Seite mit Bild und Abbildungsunterschrift), `<Scratchpad>/sicht/bilder/8-endstand.png` (Endstand
    der Kopfzeile). Nicht im Bild: die aufgeklappten Formulare unter „Verwalten“ (brauchen einen Klick).
  - **Offen** (Backlog „Nach Runde 8 offen“): Maus-/Tastenbedienung der Stücke im Satzbild, „Alle einklappen“
    im Satzbild, leere „−“-Zeile im Zeilen-Diff der Vorschau, PDF, `pick` mit Stellen; Gutachten zu Runde 8 erledigt – Nachbesserung `6615e1f` (Vorlage ohne Zeilenende, Linienkontrast auf dem Blatt, MCP-Eigentum dokumentiert), niedrige Punkte im Backlog.
- **Runde 7 „bugs · proposals · ideas“ erledigt** (Team-Runde vom Stand `e8d896d`, zusammengeführt auf
  `development` in der Reihenfolge bugs → proposals → ideas; Konflikte nur in den Vertragszeilen von
  `store/src/lib.rs`, `rpc.rs` und `server/src/lib.rs`, alle Seiten übernommen; nicht gepusht):
  - **bugs – Ursache „Absatz wird Titel“ (belegt):** Das Satzbild schickte eine Überschrift mit einfachem
    Zeilenumbruch hinter dem Titel (Umschalt+Enter bzw. `<br>` des Browsers, danach der Absatz) als einen
    Überschriftsblock; `baruch_wysiwyg::emit` machte aus dem Umbruch ein Leerzeichen, `outline.save` nahm die
    ganze Zeile als Titel. `tests/round7_titles.rs` scheiterte vor dem Fix mit „Laub Anna's Laube steht im
    Herbst …“. Jetzt endet eine Überschrift am ersten Umbruch (`emit::split_heading`, im Widget
    `splitHeading`/`headingEdits`/`guardHeadingMerge`). Titelregeln `check_title` (nicht leer, einzeilig,
    ≤ 200 Zeichen) auf allen Wegen, -32602 / MCP `isError` / 422; vorhandene leere Titel bleiben lesbar
    (der leere Abschnitt im Buch des Nutzers bleibt stehen, bis er ihn umbenennt). `normalize_asset_path`
    (`./`, `//`), `NumberingInfo.label` („Nummern: Nur Kapitel römisch“), Bilder im Satzbild als `ws-figure`.
  - **proposals:** Migration 0010 (`persona.color`), Seite `/einstellungen/personas` und RPC `persona.*`,
    Kontrast ≥ 4,5:1 in hell und dunkel (Text, gedämpfter Text und Links), Farbe in Assistentenliste, Chat, Fassungsliste und Fassungskopf;
    offene Vorschläge als `<details class="pv">` im Lesefluss (Editor und Vorschau) mit Satzbild- bzw.
    Zeilen-Diff, Übernehmen/Verwerfen ohne JavaScript, einklappbar (Cookie `baruch_folded`). Vorschlag-Karte
    und Widget `wysiwyg-diff` entfernt (RPC `wysiwyg.diff` bleibt).
  - **ideas:** Migration 0011 (`idea`, `idea_word`, `idea_fts`), RPC `idea.*`, Seiten `/einfaelle…`,
    Schnellerfassung „Einfall“ (Alt+E), MCP `idea_create/read/list/search/update/delete` mit vollem CRUD auch
    für Assistenten (IDEA_CRUD), Commits mit Herkunft im Werkspiegel (`einfaelle.md`) bzw. in
    `<data>/einfaelle/`. Neue STUBs (B2): Lektor kennt die Einfälle, Persona-Auftrag aus einem Einfall.
  - **Integrator:** totes Widget `wysiwyg-diff` aus `wysiwyg.ts` entfernt; Alternativtexte im Satzbild-Diff
    der Vorschläge in Normalform nachgeschlagen (`view.rs`, sonst fehlte er bei `image("./…")`); Methodenliste
    im Kopf von `rpc.rs`; Doku-Entwürfe aus `docs/runde7/` in Bedienung, Architektur, RPC, MCP und SKILLS
    eingefügt, `docs/runde7/` gelöscht; Plan (Ausnahme Einfälle), Backlog (Runde 7 unter „Erledigt“, neu „Nach
    Runde 7 offen“), Fahrplan (Runde 7 00:28–01:29, Typst-Vorschau in Runde 8, Runden neu geschätzt).
    Ende-zu-Ende `baruch-server/tests/round7.rs` (4): neues Kapitel und Abschnitt, Absätze im Satzbild (mit
    der Anfrage des alten Widgets), Ebene über Gliederung und Format-Auswahl → Titel bleiben; leerer,
    mehrzeiliger und 201-Zeichen-Titel über Baum-Formular (422), RPC (-32602) und MCP (`isError`) abgelehnt,
    ohne Commit; zwei Persona-Vorschläge in zwei Farben, einer gefaltet (Cookie), einer per Formular
    übernommen (ein Commit); Einfall per MCP als Persona, in der Oberfläche als Gliederungspunkt verarbeitet
    (mehrzeiliger Titel → 422, Einfall bleibt offen), per MCP gelöscht und per Formular wiederhergestellt,
    `einfaelle.md` und Commits mit Trailern; Migrationen 0010+0011 gemeinsam auf einer Datenbank im Stand
    development (0001–0009 + 0101–0105, Werk mit Vorlage, Personas, Vorschlag, Beitrag, Bild) über einen
    vollen Serverstart verlustfrei, zweiter Start ändert nichts.
  - **Test:** `cargo clean -p` aller sieben Workspace-Crates, dann `cargo test --workspace -j 4 --no-fail-fast`:
    **646 Tests, 0 fehlgeschlagen, 0 ignoriert** (80 Läufe); nach den CSS-Korrekturen der Sichtprüfung erneut
    646/0. clippy (`--all-targets -D warnings`) ohne Warnungen, `cargo fmt --check` sauber, `tsc` ohne Fehler.
  - **Sichtprüfung** (eigener Server auf einem freien Port von 127.0.0.1, Temp-Daten, erfundenes Werk „Annas
    Laube“ mit `#szene`, `#figure(image("./bilder/laube.svg"))`, Vorschläge von @lektor und @nathan, vier
    Einfälle; Firefox headless mit eigenem Profil, Cookie `baruch_folded` direkt in `cookies.sqlite`).
    Behoben: Mit Vorschlägen unter dem Editor schrumpfte das Editor-Formular, und Satzbild und Fuß lagen über
    der Vorschlagsliste (`#pane-editor` ist eine Flex-Spalte) – jetzt schrumpft kein Kind des Bereichs, und das
    Formular behält mit Vorschlägen `min(34em, 72vh)` (`static/proposals.css`); die Beschriftung eines
    eingeklappten Vorschlags stand unter der farbigen Linie (eigene Schriftgröße) – jetzt nur für
    Screenreader; Marke im Baum „2 Vorschlag“ → „2 Vorschläge“. Firefox headless zeigte den Editor bei
    manchen Läufen im Quelltext statt im Satzbild, auch ohne Cookie (vermutlich Foto vor der Antwort von
    `wysiwyg.parse`, nicht belegt). Bildschirmfotos (Scratchpad, nicht im Repo):
    `<Scratchpad>/integ7-browser/r7-lesefluss-1.png` (Lektor offen in Blau, Nathan als magentafarbene Linie),
    `<Scratchpad>/integ7-browser/r7-ohne-cookie.png` (Satzbild mit Bild und Unterschrift, beide Vorschläge offen),
    `<Scratchpad>/integ7-browser/r7-einfaelle.png` (Seite „Einfälle“ mit Wolke und Herkunft),
    `<Scratchpad>/integ7-browser/r7-vorschau.png` (Vorschau). Die Schnellerfassung (Dialog nach Klick/Alt+E) lässt sich mit
    `--screenshot` nicht öffnen und ist nicht im Bild geprüft.
  - **Offen** (Backlog „Nach Runde 7 offen“): Tastenbedienung von Satzbild-Überschriften, Schnellerfassung und
    Einklappen im Browser des Nutzers; Spiegel `<data>/einfaelle/` beim Start nicht nachgeführt; mehrere Bilder
    in einem Block; `/x.png` und `x.png` als zwei Bilder.
- **PM-Abschluss Runde 7** (nach `bb6d04b`): `cargo clean -p` aller sieben Workspace-Crates (rund 14 GiB), dann
  `cargo test --workspace -j 4 --no-fail-fast` grün: **649 Tests, 0 fehlgeschlagen, 0 ignoriert** (80 Läufe).
  STUBs unverändert bis auf die neuen B2-STUBs der Einfälle. Nachbesserung `bb6d04b` nach dem Gutachten: Umbruch in
  einer Hervorhebung (`*…*`, `_…_`) beendet die Überschrift ebenfalls (`emit::split_heading`/`cutAtBreak`), und die
  Persona-Tönungen halten 4,5 : 1 auch für gedämpften Text und Links. Kleine Gutachten-Punkte erledigt: tote
  CSS-Regeln `.ws-diff` und Kommentare zur Vorschlag-Karte entfernt, Begründungen an den neuen Zusicherungen in
  `tests/ideas.rs`, `round7_titles.rs` und Store-`titles.rs`, Bedienung (Einklappen über die farbige Linie, nicht
  den Kopf), Backlog (Doppel `/x.png` entfernt, übrige niedrige Befunde unter „Nach Runde 7 offen“), Fahrplan
  (Reihenfolge der Tabelle, Runde 7 00:28–01:50).
- **PM-Abschluss Runde 6** (nach `c827486`): `cargo clean -p` aller sieben Workspace-Crates (29515 Dateien,
  14,3 GiB), dann `cargo test --workspace -j 4 --no-fail-fast` grün: **572 Tests, 0 fehlgeschlagen, 0 ignoriert**
  (69 Läufe). clippy (`--all-targets`) ohne Warnungen, `cargo fmt --check` sauber. STUBs unverändert (alle B2).
  Niedrige Gutachten-Punkte erledigt: Bearbeitungsrest in `docs/Architektur.md` entfernt, Begründungen an den
  Zusicherungen in `tests/assets.rs` (Server und Store) ergänzt, Backlog aufgeräumt (oben nur Offenes, der
  Wortlaut der erledigten Wünsche unter „Erledigt“), Fahrplan nach `git log` (Runde 6 17:02–19:30, Backlog-Commits
  18:31–18:37, Runde 7 neu auf 150 Minuten geschätzt), HANDOFF: Ursache der leeren Bilder im Foto als
  unbelegt gekennzeichnet. Offen und im Backlog: `export --out` folgt Ordner-Symlinks im Ziel, Datei/Ordner-
  Kollision bei Bildpfaden, `/x.png` und `x.png` als zwei Bilder, Maskierungstests der Seiten „Bilder“/„Bild“.
- **Nachbesserung nach dem Gutachten zu Runde 6:** Satzbild: Der Einzug der ersten Rumpfzeile gehört zum ersten
  Block und wandert beim Einfügen davor und beim Umstellen mit (vorher wurde eine Liste zur geschachtelten Liste
  bzw. Insel). Ordner-Import: Bildpfade, die der Store ablehnt (`./bilder/a.svg`, `a//b.png`), werden mit Grund
  übersprungen statt den ganzen Import zu verwerfen (`baruch_store::check_asset_path`). Export/`main.typ` mit
  Gliederungsschema: je Knoten mit `numbered = 0` eine Show-Regel `numbering: none` (an Ebene und Titel), damit
  Typst wie Baruch zählt (nicht mit `typst compile` geprüft). Backlog: Bildersuche, Base64-Prüfung, Sichtprüfung
  `loading="lazy"` und `./`-Pfade nachgetragen. `cargo test --workspace -j 4`: **572 Tests, 0 fehlgeschlagen**
  (69 Läufe), clippy ohne Warnungen.
- **Runde 6 „numbering · satzbild · images“ erledigt** (Team-Runde vom Stand `5cefac6`, zusammengeführt auf
  `development` in der Reihenfolge satzbild → numbering → images; Konflikte nur in den Vertragszeilen von
  `server/src/lib.rs`, `rpc.rs` und `store/src/lib.rs`, beide Seiten übernommen):
  - **numbering:** Migration 0008 (`work.numbering_scheme`, zwölf Schemata, NULL → `template`, `''` → `none`,
    Muster → `custom`; `work.numbering` bleibt). `baruch_typst::scheme` (`Scheme`, `Numbering`, `export_template`),
    Mustersprache `P`, `P|k`, `P1;P2;…`, `aa`; `heading_numbering` liest nur die letzte Regel der obersten Ebene und
    erkennt Tiefenbedingungen. Store `set_numbering_scheme`, `numbering_preview`, `numbering_rule`,
    `StoreError::Numbering` (-32602). Server `src/numbering.rs`: RPC `numbering.schemes/preview/set`, Seite
    `/werke/{id}/nummerierung`, Formular `POST /werke/nummerierung/act`. Export und `main.typ` bekommen bei
    anderen Schemata als `template` die Regel angehängt; `template` bleibt byte-gleich.
  - **satzbild:** `BlockKind::Scene/Work { args, body }` mit Rumpf-Anweisung `{replaces, body}`, Label an der
    Überschrift (`Heading.label`), `sheet_style` nur oberste Ebene, `image_path`/`render_html_with`/
    `diff_html_with`; Widget: Vorgabe Satzbild nach jedem Neuzeichnen, `data-ws-open` gegen die leere Fläche,
    Einfügen mehrerer Absätze. `docs/WYSIWYG.md` nachgezogen.
  - **images:** Migration 0009 (`asset`, `asset_path`, `asset_revision`, Inhalt TEXT, Trigger wie 0004),
    `baruch_store::assets` (8 MiB je Fassung, MIME aus den ersten Bytes), `baruch_typst::images::image_paths`,
    Import über die CLI liest Bilder relativ zum Ordner (nicht hinaus), `export --out`, Server `src/assets.rs`
    (RPC `asset.*`, Seiten `/werke/{id}/bilder…`, Hochladen, `…/pfad?p=`, `nosniff` und CSP sandbox), Spiegel
    `bilder/<name>` bzw. `.b64`, ein Commit je Änderung (`Baruch-Action: asset-*`), MCP nur lesend
    (`list_assets`, `read_asset`). Neue Abhängigkeiten: `base64 0.22` (Store), `baruch-typst` im Server.
  - **Integrator:** Alternativtext aus `Store::asset_list` in den Bild-Rückruf des Satzbilds
    (`wysiwyg::image_alts`, Pfad mit oder ohne `/`); Methodenliste im Kopf von `rpc.rs`; gemeldete Testbrüche
    behoben: `mcp.rs` und `round4.rs` (Werkzeugliste um `list_assets`/`read_asset`), dazu
    `migration_0008.rs` (vergleicht jetzt nach einem Migrator bis 0008, sonst stören die Tabellen aus 0009).
    Doku-Entwürfe aus `docs/runde6/` in Bedienung, Architektur, RPC, MCP und SKILLS eingefügt, `docs/runde6/`
    gelöscht; Backlog aufgeräumt (offen oben, „Erledigt“ am Ende), Plan verweist auf den Fahrplan, Fahrplan mit
    Runde 6 und neu geschätzten Runden 7–12. Ende-zu-Ende `baruch-server/tests/round6.rs` (4): Schema per
    Formular → Gliederung, Editor-Kopf, `/gliederung`, Export und `main.typ` folgen, zurück auf „aus der
    Werkvorlage“ byte-gleich; Satzbild eines Abschnitts mit `#szene` und Bild-Insel zeigt `<img>` mit
    maskiertem Alternativtext, Bearbeiten eines Absatzes in der Szene → Export genau mit dieser Änderung;
    Ordner-Import mit SVG und PNG → Seite „Bilder“, Ersetzen per Hochladen = Fassung 2 mit einem Commit,
    `export --out` byte-gleich; Migrationen 0008 und 0009 gemeinsam auf einer Datenbank im Stand 0001–0007 +
    0101–0105 (mit Beispielbibliothek) über einen vollen Serverstart verlustfrei, zweiter Start ändert nichts.
  - **PM-Abschluss:** `cargo clean -p` aller sieben Workspace-Crates, `cargo test --workspace -j 4` grün:
    **567 Tests, 0 fehlgeschlagen** (69 Läufe). `cargo clippy --workspace --all-targets` ohne Warnungen,
    `cargo fmt --check` sauber. Keine neuen STUBs.
  - **Sichtprüfung im Browser** (eigener Server auf einem freien Port von 127.0.0.1, Temp-Daten, erfundenes Werk
    mit `#szene`, `#arbeit` und SVG; Firefox headless mit eigenem Profil): Satzbild offen mit Szene als Kasten,
    Label an der Überschrift, Bild-Insel; Quelltext über `#fundstelle` ohne leere Fläche; Baum mit „Nummern:
    Alphanumerisch“ (I. · A. · 1.); Seite „Gliederungsschema“ mit Vorschau je Schema; Seiten „Bilder“ und
    „Bild“. Behoben: Auf der Seite „Bilder“ zog das Formular „Bild hinzufügen“ die Spalte in die Höhe
    (`.work-form` hat `flex: 1` aus `works.css`) – `assets.css` setzt es für `.assets-page` auf `flex: 0 0 auto`.
    Firefox headless lädt Bilder mit `loading="lazy"` in einem iframe nicht vor dem Bildschirmfoto (Vorschau
    in der Bilderliste und Bild-Insel leer); Chromium headless zeigt das Bild in der Insel, und dieselben Bilder
    laden in Firefox ohne iframe (Probe mit eager/lazy/datei). Ursache nicht belegt (vermutlich die Foto-Hülle,
    nicht Baruch); im Browser des Nutzers einmal ansehen (Backlog „Nach Runde 6 offen“).
  - **Offen** (Backlog „Nach Runde 6 offen“): Regeln der Schemata mit Wörtern nicht mit `typst compile`
    geprüft (typst fehlt); Re-Import dieser Regeln nur näherungsweise; Widget-Verhalten (Einfügen, Enter in
    Szenen, Markieren) nur per Test und `tsc` geprüft; Literatursuche markiert die Fundstelle im verborgenen
    Textfeld.
- **PM-Abschluss Runde 5** (nach 119b859): `cargo clean -p` aller sieben Workspace-Crates, dann
  `cargo test --workspace -j 4` grün, **471 Tests, 0 fehlgeschlagen** (57 Läufe; gegenüber der Integration
  +1 `switch_level` = 8, +3 `serialize` = 9). `cargo clippy --workspace --all-targets` ohne Warnungen;
  `cargo fmt --check` schlug nach 119b859 in `baruch-wysiwyg/src/parse.rs` an (eine Zeile), mit `cargo fmt`
  nachgezogen. STUBs unverändert (alle B2, siehe unten).
- **Nachbesserung nach dem Gutachten zu Runde 5** (119b859): Maskierungen, die als Text eine Kurzschreibweise
  ergäben (`\'`, `\"`, `\-` vor `-`/`?`/Ziffer, `\.` neben Punkten), bleiben im Satzbild als Inline-Insel
  stehen (`escape_matters`), harmlose bleiben bearbeitbarer Text; `= Anna\'s Laube <laube>` bleibt byte-gleich.
  Umschalten, das Unterpunkte unter Ebene 3 drücken würde, wird abgelehnt (`StoreError::Heading`, „Erst
  Unterpunkte höher stufen“) statt zu kappen; hin und zurück stellt die Gliederung wieder her (Test).
  `baruch_typst::is_newline` (alle Typst-Zeilenenden) in `escape_title`, `emit` und `wysiwyg.ts`.
  `docs/Fahrplan.md`: Runde 5 erledigt.
- **Runde 5 „tree · wysiwyg · r4“ erledigt** (Team-Runde, zusammengeführt auf `development` in der Reihenfolge
  r4 → tree → wysiwyg, ohne Konflikte):
  - **tree:** Der Baum links ist die einzige Gliederung (Ziehen und Ablegen, Alt+Umschalt+Pfeile, Kontextmenü
    mit Umbenennen per F2/Doppelklick an Ort und Stelle, Anlegen davor/danach/darunter, Umstufen, Löschen mit
    `confirm: true`, Nummerierung im Kopf; `/gliederung…` nur noch ohne JavaScript verlinkt). Einträge öffnen
    immer den Editor. `Store::switch_to` (hinter `set_current`/`accept`): Die Gliederung folgt Ebene und Titel
    der Fassung, Unterpunkte werden mitgestuft (neue Fassungen nur mit geänderter Überschriftszeile), die
    Fassung bleibt byte-gleich (Nutzerentscheidung „Nein“); `mirror::record_switch` = ein Commit
    „… · Ebene a → b“. `outline.move` behält die Ebene. `heading_numbering` erkennt `numbering("…")` in
    Funktionen. `view.rs` gibt `node.key` in den Kontext. `outline.ts` löst `baruch:before-save` aus.
  - **wysiwyg:** neue Crate `baruch-wysiwyg` (Blöcke, Typst-Inseln, `parse`/`serialize`/`emit`, HTML, Stil aus
    der Vorlage), Server-Modul `src/wysiwyg.rs` (RPC `wysiwyg.parse/serialize/style/diff`, nur lesend), Widget
    `wysiwyg.ts` mit Umschalter „Satzbild | Quelltext“ (Cookie `baruch_editor`), Satzbild-Diff der
    Vorschlag-Karte, `static/wysiwyg.css`, `docs/WYSIWYG.md`. Rundreise byte-gleich (alle Testdateien, eigene
    `satzbild.typ`, alle Fassungen des Beispielwerks, Eigenschaftstest 2000 Fälle).
  - **r4:** Migration 0007 (`work.id AUTOINCREMENT`, `store_meta`, `work_retired`), Saat nur ohne
    `store_meta.seeded` (Teile des Beispielwerks ohne Text), slugs gelöschter Werke dauerhaft reserviert
    (PM-Entscheidung, weil die Datenbank den Archivordner nicht kennt; die Löschseite sagt es), MCP
    `propose_revision`/`add_post`/`refs` und `highlight.cite` bzw. `/literatur/act cite` nur mit `key`
    (`node` → -32602 bzw. 422), `works::RPC_BODY_LIMIT`, Tests für jeden Schutzzweig von `move_to_trash`.
    **Abweichung vom Vertrag mit Beleg:** sqlx-sqlite 0.8.6 führt jede Migration in einer Transaktion aus und
    ignoriert `-- no-transaction` (`migrate.rs`, `apply`); 0007 nutzt darum `PRAGMA defer_foreign_keys=ON` und
    die Wache `fk_guard` statt `foreign_keys=OFF`/`BEGIN IMMEDIATE`/`RENAME` (Test
    `sqlx_runs_even_a_no_transaction_migration_inside_a_transaction`).
  - **Integrator:** die zwei Vertragszeilen (`rpc.rs`: `wysiwyg::handles` im `match`; `lib.rs`:
    `DefaultBodyLimit::max(works::RPC_BODY_LIMIT)` an `/rpc`), gemeldeter Testbruch
    `revisions.rs::the_tree_view_indents_by_parent_and_escapes_authors` behoben (sucht jetzt nur in der
    Baumliste, weil der Satzbild-Bereich auch `data-rev` trägt), Doku-Entwürfe aus `docs/runde5/` in
    Bedienung, RPC, Architektur, Backlog, SKILLS, Plan und den Doku-Kommentar von `Store::open` eingefügt,
    `docs/runde5/` gelöscht. Ende-zu-Ende `baruch-server/tests/round5.rs` (7): Klick → Editor mit
    Satzbild-Bereich (ohne JavaScript `hidden`, nur das Textfeld), Satzbild bearbeiten → `outline.save` →
    Fassung 2 mit `= Sommer\n\nGießen am _Morgen_.\n` (Export, Spiegel); Laden/Speichern ohne Änderung über
    jede Fassung des Beispielwerks byte-gleich, keine neue Fassung; Ebene ändern → alte Fassung → Gliederung
    folgt, ein Commit, Fassung unverändert; MCP ohne `key` → -32602; `work.import` mit 3 MiB über `/rpc`,
    über `RPC_BODY_LIMIT` → 413; neues Werk nach dem Löschen bekommt weder id noch slug; Migration 0007 auf
    einer Datenbank im Stand 0001–0006 + 0101–0105.
  - **Probe von 0007** auf einer nachgebauten Datenbank (Temp-Ordner, eigene Server auf Port 0, nicht auf echten
    Daten): altes Programm (development 4828d6e) säte Werk 1, importierte ein frei erfundenes Buch mit 87
    Knoten als Werk 2, dazu Fassungen und Beiträge über RPC (105 Fassungen, 9 Beiträge); neues Programm: alle
    16 Tabellen Zeile für Zeile gleich, `pragma_foreign_key_check` leer, je Abschnitt genau eine `current`,
    Export von Werk 2 byte-gleich, `store_meta.seeded = 1`, zweiter Start ändert nichts, Werk 2 löschen und
    neu anlegen → id 3, slug `…-2`.
  - `cargo clean -p` aller sieben Workspace-Crates, dann `cargo test --workspace` grün, **467 Tests, 0
    fehlgeschlagen**: core 6; gitmirror 5 + mirror 12; library 7 + highlights 9, library 19, revision_index 6;
    server 27 + concurrency 1, git_mirror 8, headings 3, import 13, library_search 6, literature 14, mcp 19,
    node_keys 4, origin 9, outline 13, pages 9, revisions 21, round3 5, round4 7, round5 7, round5_fixes 9,
    rpc 6, tree 8, work_crud 13, works 11, wysiwyg 7; store 11 + concurrency 1, import 13, migration_0007 4,
    mirror_template 5, outline 18, revision_status 9, revision_tree 26, store 14, switch_level 7, works 11;
    typst 6 + numbering 8 + roundtrip 18; wysiwyg 13 + emit 3, html 4, roundtrip 4, serialize 6, style 2.
    `cargo clippy --workspace --all-targets` ohne Warnungen, `cargo fmt --check` sauber, tsc ohne Fehler.
  - Zweige `team/tree`, `team/wysiwyg`, `team/r4` und ihre Worktrees unter `.claude/worktrees/wf_4841fe08-aec-*`
    bestehen noch.
- **PM-Abschluss Runde 4** (nach 0ca7428): `cargo clean -p` aller Workspace-Crates, dann `cargo test --workspace`
  grün, **384 Tests, 0 fehlgeschlagen**. Probe der Migrationen 0005/0006/0105 auf einer **Kopie** der echten
  Datenbank (Stand 0001–0004 + 0101–0104, angelegt 14:47; `~/.local/share/baruch` selbst nicht angefasst) mit
  eigenem Server (Port 0): alle Migrationen laufen, 2 Werke, 11 + 87 Knoten, 103 Fassungen, 7 Beiträge, 5 Zitate
  unverändert, jede aktuelle Fassung und die Werkvorlagen byte-gleich, je Abschnitt genau eine `current`, Export
  von Werk 2 byte-gleich mit der Quelldatei, zweiter Start ändert nichts. Titel bleiben eindeutig (beide Werke
  trugen schon verschiedene Titel); geändert hat sich genau ein Titel in Werk 2 (Maskierung `\'` aufgelöst), dessen
  Spiegeldatei darum anders heißt.
- **Nachbesserung nach dem Gutachten zu Runde 4** (auf `development`):
  - **Feste Schlüssel:** `baruch_core::Node::key` (= `node.id`, keine Migration), `Store::node_seq`,
    `StoreError::GoneNode`; `rpc::resolve_keys` übersetzt `key`/`target_key` unter `write_lock` (RPC, `/act`,
    `/gliederung/act`, Rückfrageseite, MCP `read_node`/`propose_revision`/`add_post` samt `refs`). Alle Formulare
    (`tree.html`, `outline.html`, `outline_edit.html` – Zielauswahl jetzt `target_key` –, `outline_confirm.html`,
    `editor.html`, `assistants.html`) und `outline.ts` schicken ihn mit. Ein veralteter Tab bzw. Assistent schreibt
    so nicht mehr in einen fremden Abschnitt und löscht keinen fremden Punkt.
  - **Überschriften außerhalb des Editors:** `outline::check_current_heading` (statt `sync_node_heading`) lehnt beim
    Übernehmen, Umschalten, `revision.save` und Ändern der aktuellen Fassung Texte mit anderer Ebene, weiteren
    oder fehlender Überschrift ab (`StoreError::Heading`, -32000, zurückgerollt); `Store::add_proposal` prüft vorher
    (`check_proposal_heading`, also auch MCP und `highlight.cite`). Ein neuer Titel derselben Ebene benennt weiter um.
    Ändern einer nicht aktuellen Fassung prüft nichts. Bedienung, Backlog, RPC, MCP, Architektur nachgezogen.
  - **Ungespeicherter Text:** `leaveEditor()` gemeinsam in `widgets.ts`; `workspace.ts` fragt vor jedem Formular außer
    dem Editor und bei `baruch:navigate` (außer `detail.asked`), `outline.ts` fragt vor dem RPC-Aufruf.
  - **Fremder Buchtitel entfernt:** die aus `docs/buch.typ` übernommene Überschrift in Testdaten, Tests, Kommentar und
    Doku durch den erfundenen Titel „Anna\'s Laube“ ersetzt. In der Git-Historie (a231aff, e4c5999, 9f3034c) steht sie
    weiter; Umschreiben der Historie entscheidet der Nutzer.
  - Tests neu: `baruch-server/tests/node_keys.rs` (4), `headings.rs` (3), `mcp.rs` +2, `revisions.rs` +1,
    `baruch-store/tests/outline.rs` +2; `cargo test --workspace` grün, **384 Tests**, clippy ohne Warnungen, fmt sauber.
- **Runde 4 „works · outline · fixes“ erledigt** (Team-Runde, zusammengeführt auf `development` in der Reihenfolge
  fixes → works → outline):
  - **works:** Store-Migration 0005 (`work.title_key` eindeutig, `subtitle`, `audience`, `bible`, `updated`,
    Tabelle `work_template_revision`), Doppel-Auflösung in Rust beim Start (`works::ensure_title_keys`: gesätes
    Werk gegen importiertes → Titel des Beispielwerks (`sample::work().title`), sonst „(2)“, „(3)“ …; nur Titel ändern sich,
    slug und Spiegel bleiben). `baruch-store/src/works.rs` (`create_work`, `import_typst_titled`, `work_meta`,
    `update_work`, `delete_work`), Server `src/works.rs` mit RPC `work.list/meta/create/import/update/delete`,
    Seiten `/werke…`, Upload `POST /werke/import` (multipart, 16 MiB), `POST /werke/act`, Export-Download,
    Werkwahl öffnet sofort (`works.ts`), Cookie `baruch_work`, CLI `import --title`. Löschen archiviert den
    Spiegel nach `works/.geloescht/<slug>-<datum>/`. Nicht über MCP.
  - **outline:** Store-Migration 0006 (`node.numbered`, `work.numbering`, aufgelöste Typst-Maskierungen in
    `node.title`), `baruch-typst` mit `unescape_title`/`escape_title`/`heading_line`/`heading_numbering`/
    `format_number`, Nummern beim Laden (`load.rs`), `baruch-store/src/outline.rs` (`outline_move/set_level/
    rename/add/delete/check/save`, `set_numbering`, `sync_heading`), Server `src/outline.rs` (RPC `outline.*`,
    `/gliederung`, `/gliederung/bearbeiten`, `POST /gliederung/act`, Rückfrageseite), `mirror::record_outline`
    (ein Commit je Änderung), Formulare ↑ ↓ ← → in der linken Spalte, Widget `outline.ts`.
  - **fixes:** `revision.delete` verlangt `confirm: true` (RPC) bzw. das Kästchen (`/act`, 422); `/literatur/act`
    `cite` ohne Abschnitt → 422; `StoreError::UnknownRevision` nennt die vorhandenen Fassungen; Hervorhebungen
    nehmen Text mit anderem Leerraum an (`normalize_space`, DOM-Ranges in `literature.ts`); Bibliotheks-
    Migration 0105 (`post_fts_delete`, `post_fts_move`, verwaiste Beitragszeilen weg); Tests zur Gleichzeitigkeit;
    `workspace.ts` hört auf `baruch:navigate`, Alt+Umschalt+Pfeil blättert nicht; Versionslisten in Tests robust.
  - **Integrator:** Konflikte in `docs/Architektur.md`, `docs/RPC.md` und `crates/baruch-server/src/lib.rs`
    (STYLE, Router) gelöst; `state.work` wird nicht mehr gelesen (`/act`, `literature::resolve`, `mirror::sync`
    → `state.default_work()`); `outline::sync_node_heading` nach jeder Änderung der aktuellen Fassung
    (`Store::accept`, `save_human`, `set_current`, `save_human_on`, `update_revision`) in derselben Transaktion –
    ein übernommener Vorschlag mit neuer Überschrift derselben Ebene benennt den Punkt um; doppelte
    Index-Bereinigung in `delete_work` entfernt (0105 erledigt das); Doku nachgezogen (`rpc.rs`-Kopf, RPC.md
    „Werk:“ und `revision.save`, Architektur, Bedienung, SKILLS, Plan, Backlog); Ende-zu-Ende-Tests
    `baruch-server/tests/round4.rs` (7).
  - `cargo test --workspace` grün (frische Artefakte aller Workspace-Crates), **372 Tests**: core 6; gitmirror 5 +
    mirror 12; library 7 + highlights 9, library 19, revision_index 6; server 26 + concurrency 1, git_mirror 8,
    import 13, library_search 6, literature 13, mcp 15, origin 9, outline 13, pages 9, revisions 20, round3 5,
    round4 7, rpc 6, work_crud 13, works 11; store 11 + concurrency 1, import 13, mirror_template 5, outline 16,
    revision_status 9, revision_tree 26, store 14, works 8; typst 6 + numbering 6 + roundtrip 18.
    `cargo clippy --workspace --all-targets` ohne Warnungen, `cargo fmt --check` sauber.
  - Zweige `team/works`, `team/outline`, `team/fixes` und ihre Worktrees unter `.claude/worktrees/wf_7df27d73-23d-*`
    bestehen noch.

Frühere Runden:
- **Runde 3 „revtree · litpopup · small“ erledigt** (Team-Runde, zusammengeführt auf `development`):
  - **revtree:** Store-Migration 0004 (Elternteil `based_on` nach Regel R nachgetragen, `edited`,
    `node.last_rev`, neue Trigger), Bibliotheks-Migration 0104 (Volltextindex bei UPDATE/DELETE),
    `baruch-store/src/tree.rs` (`set_current`, `save_human_on`, `update_revision`, `delete_revision`,
    `revision_tree`), `CommitRequest::allow_empty`, Spiegel-Protokoll mit `Baruch-Action`, RPC
    `revision.set_current/update/delete/tree`, `revision.save {based_on?}`, `ui.render {pick?}`, Editor mit
    „‹ n/m ›“, `is-current`, „Diese Fassung verwenden“, „Fassung ändern“, „Löschen“ und Baumansicht
    (`fragments/revtree.html`). Nicht über MCP.
  - **litpopup:** Literatur als Popup (Kopfzeile, Alt+L; ohne JavaScript `/literatur`), Dokumentansicht
    `/quelle`, Formulare an `POST /literatur/act`, Hervorhebungen (Migration 0103, `baruch-library/src/highlights.rs`,
    RPC `highlight.*`), `highlight.cite` legt ein Zitat als Vorschlag an (@literatur, baruch-zitat),
    Suche mit Art `highlight` und `quote_id`/`highlight_id`; `baruch-library/build.rs`.
  - **small:** Port 8765, `default_data_dir()` = `dirs::data_local_dir()/baruch`, `import_typst` aus dem MCP
    entfernt (nur noch CLI), MCP liest Revisionen über Nummern, `vorlage.typ` im Spiegel mit `#import` je
    Abschnittsdatei, slug an Wortgrenzen, Beispielwerk mit eigenem Titel „Beispielwerk: …“ (nur neue DB), FIFO
    im Import, Parser-Grenzen.
  - Integrator: Konflikt in `docs/Bedienung.md` gelöst, Versionslisten der Bibliothekstests um 0104 ergänzt,
    Anker `id="fundstelle"` am Textfeld des Editors (Treffer-Link auch ohne JavaScript), Doku nachgezogen
    (`rpc.rs`-Kopf, Architektur Server, SKILLS), Ende-zu-Ende-Tests `baruch-server/tests/round3.rs`.
  - Abweichung revtree, angenommen: auch **Umschalten** schreibt mit `allow_empty` (jede Umschaltung genau ein
    Commit, auch zwischen Fassungen mit gleichem Text).
  - Nachbesserung nach dem Gutachten (7bb1c70): `add_highlight` prüft den Bereich gegen das Zitat (Ende im Zitat,
    Text = `quote[start..end]`, sonst `InvalidRange` bzw. neu `HighlightError::TextMismatch` → -32602);
    Plan.md nennt die Regeln aus 0004 statt „nur anhängen“; Backlog nennt Baum über das ganze Werk und Hinweis
    auf Alternativen im Lesefluss als offen.
  - `cargo test --workspace` grün (frische Artefakte, PM-Abschluss nach 7bb1c70), **266 Tests**: core 6; gitmirror 5 + mirror 12;
    library 5 + highlights 8, library 19, revision_index 3; server 22 + git_mirror 7, import 9,
    library_search 6, literature 10, mcp 14, origin 9, pages 7, revisions 16, round3 5, rpc 6, works 10;
    store 2 + import 10, mirror_template 4, revision_status 9, revision_tree 25, store 14; typst 6 + roundtrip 17.
  - Zweige `team/revtree`, `team/litpopup`, `team/small` und ihre Worktrees unter `.claude/worktrees/wf_75cc2d82-f11-*`
    bestehen noch.

- **B1 (Ausschnitt) erledigt:** SQLite ist die Quelle, Git spiegelt, Autorenbibliothek mit Volltextsuche.
- **B1.5 „MCP, Typst-Import, Nacharbeit“ erledigt** (Team-Runde, zusammengeführt auf `development`):
  - `crates/baruch-typst` (neu, rein): `parse` zerlegt eine Typst-Datei in Werkvorlage (alles vor der ersten
    Überschrift) und `=`/`==`/`===`-Abschnitte, `join` setzt sie Byte für Byte wieder zusammen.
  - `crates/baruch-store`: `import_typst`/`export_typst`/`work_template`/`work_by_slug`/`work_list`,
    Migration 0002 (Werkvorlage, `source_sha256` eindeutig → zweiter Import abgelehnt), Migration 0003
    (Trigger: Statuswechsel nur proposed → current/rejected, current → superseded).
  - `crates/baruch-server`:
    - Mehrere Werke: `AppState::work` (Vorgabewerk) und `AppState::mirror(work)` (ein Spiegel je Werk,
      lazy); alle RPC-Methoden nehmen `work?`, neu `work.list` und `work.export`; Werkwahl in der Kopfzeile
      (GET-Formular), Links tragen `work` und `lq`.
    - CLI: `baruch-server import <datei> [--data DIR]` und `baruch-server export <id|slug> [--data DIR]`.
    - MCP (`POST /mcp`, `docs/MCP.md`): `list_works`, `outline`, `read_node`, `search`, `propose_revision`
      (nur `proposed`), `add_post`, `list_personas`, `import_typst`; `ask_persona`/`chat` STUB (B2).
      Ohne Token nur Loopback, sonst 401; fremder `Origin` 403.
  - Nacharbeit: NUL/Steuerzeichen in der Suche → leere Suche; Ausschnitte als HTML mit `<mark>`;
    Treffer-Links behalten Werk und Suche (`urlencode`), Titel mit Treffern aufgeklappt; ein
    `hashchange`-Listener in `library.ts`; Spiegel lehnt Symlinks in Zwischenordnern ab; Test „Spiegel holt
    nach Fehler nach“; `rustfmt.toml` (120 Zeichen) und `cargo fmt --all`.
- **Nacharbeit B1.5 (Befunde):** UTF-8-BOM verschluckt das erste Kapitel nicht mehr (BOM bleibt Vorlage,
  Rundreise byte-gleich); `guard::local_host` vor allen Routen lehnt fremde `Host`-Namen mit 421 ab
  (DNS-Rebinding); MCP `import_typst {path}` nur noch für `.typ` (auch Verknüpfungsziel) mit einheitlicher
  Meldung ohne Existenz-Auskunft; Notiz und Commit des MCP-Imports nennen den Client. Zugriff unter einem
  Rechnernamen (Proxy) geht damit vorerst nicht.
- Probe mit dem echten Buch (nur lokal, Wegwerf-Ordner, nichts kopiert): 8 Kapitel, 79 Abschnitte,
  Export byte-gleich. `docs/buch.typ` steht in `.git/info/exclude` und wird nie committet.
- `cargo test --workspace` grün, **163 Tests**: core 4; gitmirror 5 + mirror 11; library 3 + library 19;
  server 13 + git_mirror 7, import 6, library_search 5, mcp 15, origin 9, pages 7, rpc 6, works 10;
  store 2 + import 10, revision_status 5, store 14; typst 3 + roundtrip 9.
- Commits auf `development` (nicht gepusht): Merges b56c729 (import), be3585b (cleanup), 6958cc4 (mcp),
  Integration ebba2c5, Formatierung 27a1297, Übergabe 7902878, Nacharbeit 5d49a3a, dazu dieser
  Übergabe-Commit. Zweige `team/import`, `team/cleanup`, `team/mcp` und die Worktrees
  `.claude/worktrees/wf_17efd605-3ec-2/3/4` bestehen noch.
- STUBs im Code (Stand Runde 4, unverändert seit Runde 3): `mcp.rs` Token-Verwaltung; `mcp/tools.rs` `ask_persona` und `chat`;
  `literature.rs` Dokumentansicht über `read_document` und Persona-Auftrag (auch `web/src/literature.ts`);
  `highlights.rs` Bereichsprüfung ohne Zitat gegen den Volltext – alle B2.

## Nächste Schritte
0. Nach Runde 37 (integriert, **keine Migration**): Der Dienst des Nutzers wird nicht angefasst. Es fehlen Prüfung
   und PM-Abschluss (Fahrplan). Die Hilfe erscheint erst mit dem Fast-Forward nach `main` auf der Webseite; bis
   dahin führt der neue Eintrag Baruch ▾ → Hilfe auf eine Adresse, die es noch nicht gibt (`/hilfe/`; die alte
   Seite `/bedienung/` bleibt bis dahin online). Vor der Veröffentlichung im eigenen Browser ansehen: die Hilfe
   (`hugo server -s site`), besonders die Bilder zur Bibel, die Tasten am Mac und die Druckansicht. Danach die
   Veröffentlichung 0.9.1 durch den Nutzer.
0. Nach Runde 36 (abgeschlossen, **keine Migration**, aber **sqlx 0.9.1**): Der Dienst des Nutzers wird nicht
   angefasst. Vor dem Einspielen den Datenordner sichern – eine echte, von sqlx 0.8.6 geschriebene Datenbank hat
   in der Runde niemand mit 0.9 geöffnet (nur nachgebaute; die Prüfsummen der Migrationen sind nachgerechnet).
   Ansehen im eigenen Browser: `docs/Backlog.md`, „Runde 36 – Reste“ (Mindmap dunkel im breiten Fenster,
   Token-Liste in schmaler Spalte, erster Aufruf nach einer Größenänderung, Anmeldeseite dunkel). Als Nächstes
   nach Vorschlag des PM: Runde 37 „Hilfe auf der Webseite“, danach die Veröffentlichung 0.9.1 durch den Nutzer
   (`docs/Fahrplan.md`, „Geplant“). Bei der Veröffentlichung zeigt sich erstmals, ob `scripts/ci-typst.sh` in
   `ci.yml`, `pages.yml` bei `release: published` und `baruch-typst` in AppImage, .dmg und .msi laufen.
0. Nach Runde 35 (integriert, **Migrationen 0053 und 0054**, nur neue Tabellen): Der Dienst des Nutzers wird nicht
   angefasst; vor dem Einspielen den Datenordner sichern. Ohne Konten ändert sich nach dem Einspielen nichts. Wer
   Konten will: am Rechner selbst (`http://127.0.0.1:8765/einrichten`) das Autor-Konto anlegen, dann Einstellungen →
   Konten (Lektor anlegen) und Reiter Buch → Mitwirkende (Werk freigeben). Der Proxy muss `Host` unverändert
   durchreichen und TLS sprechen; `auth_basic` im Proxy kann mit Konten entfallen (`docs/Betrieb.md`). Handprüfung:
   `docs/Architektur.md`, „Runde 35 – Konten und Rollen“, „Handprüfung für den Nutzer“. Offene Fragen und Reste:
   `docs/Backlog.md`, „Nach Runde 35 offen“. Prüfung, Nachbesserung (`40d6591`) und PM-Abschluss sind erledigt; es
   folgen nach Vorschlag des PM 36 „Reste“ und 37 „Hilfe auf der Webseite“ (`docs/Fahrplan.md`, „Geplant“).
0. Nach Runde 34 (abgeschlossen, **Migration 0052**): Nach dem Stand der Übergabe läuft der Dienst des Nutzers mit
   `35140b5` (vom PM nicht nachgesehen – der Dienst wird nicht angefasst); die Runden 32 und 34 sind dort nicht
   eingespielt. Vor dem Einspielen den Datenordner sichern. Danach die Handprüfung aus `docs/Backlog.md`, „Nach
   Runde 34 offen“: Werk 3 (33 Einfälle; lose Einfälle in der Wolke, Verbindungen als Linien), Tasten und Ziehen im
   eigenen Browser, `.xmind` in XMind und `.mm` in Freeplane öffnen. Nächste Runde nach Vorschlag des PM: 35 „Konten
   und Rollen“, dann 36 „Reste“ und 37 „Hilfe auf der Webseite“ (`docs/Fahrplan.md`, „Geplant“). Offen für den
   Nutzer: ob „schlanker Einstieg, Befunde nach Gewicht, Anmerkungen zum ganzen Buch“ (Vorhaben Lektorat) vor oder
   nach 0.9.1 kommt – in der Reihenfolge vom 2026-11-25 ist es nicht genannt.
0. Nach Runde 32 (abgeschlossen, **keine Migration** gegenüber dem eingespielten Stand `35140b5`): Der Dienst des
   Nutzers läuft mit `35140b5`; Runde 32 ist dort **nicht** eingespielt. Reihenfolge beim Nutzer: (1) Release
   0.9.0 – nur den Tag `v0.9.0` schieben, Release-Lauf abwarten, Signaturen von macOS und APT prüfen, Release
   veröffentlichen, erst danach `main` (`docs/Verteilung.md`, „Ein Release machen“ und die Grenze der Gruppe
   `pages`); (2) entscheiden, welcher Stand nach `main` geht (siehe oben); (3) für Runde 32 den Datenordner sichern,
   Dienst neu bauen und starten, dann die Handprüfung aus `docs/Backlog.md`, „Nach Runde 32 offen“ (Word-Datei aus
   echtem Word und LibreOffice, Export in Word öffnen, Beenden im App-Fenster und als Dienst). Nächste Runde nach
   Vorschlag des PM: 34 „Konten und Rollen“ (`docs/Fahrplan.md`, „Geplant“).
0. Nach Runde 23 (abgeschlossen). Beim Nutzer: Dienst neu bauen und starten (**keine Migration**;
   vorher wie immer Datenordner sichern), dann die Handprüfung oben unter „Runde 23 … abgeschlossen“ (Teil A
   Wegwerf-Instanz, **Teil B mit Werk 3: Zitat mit Stellenangabe**). Vorhandene MCP-Tokens behalten Namen und
   Beiträge; Clients ohne Token mit Umlaut im Namen schreiben ab jetzt unter dem neuen Handle (`ü` → `ue`).
   Veröffentlichung (Fast-Forward nach `main`, Push) nur durch den Nutzer – vorher die Punkte unter Backlog „Vor einer
   Veröffentlichung“ entscheiden (Zielversion, Repo-Adresse, Git-Historie, MultiGPT-Vorgabe, CI, Pakete).
0. Nach Runde 22 (mit Runde 21): **Datenordner `~/.local/share/baruch` sichern** (oder „Sicherung jetzt“), Dienst neu
   bauen und starten. Beim Start laufen Migration **0050** (Fortschreibungen, Einstellung „Fassungen“) und aus Runde 21
   **0108** in der Bibliothek. Danach die Handprüfung oben unter „Runde 22 … abgeschlossen“, vor allem **Teil B mit
   Werk 3** (erste Korrektur an einem älteren Abschnitt = neue Fassung wegen der Zeitgrenze, zweite = fortgeschrieben;
   Entwurfs-PDF), dazu die Handprüfung von Runde 21 (PDF/EPUB-Import, Markdown/ZIP). Danach Pause; R23/R24 nur auf
   Wunsch (`docs/Backlog.md`, „Nach Runde 22 offen“).
0. Nach Runde 20: **Datenordner `~/.local/share/baruch` sichern** (oder „Sicherung jetzt“), Dienst neu bauen und
   starten (keine Migration). Danach die Handprüfung mit Werk 3 oben unter „Runde 20 … abgeschlossen“ (neun Schritte:
   Download, eigener Reader, Endnoten, Vorschau, Prüfseite, EPUBCheck, Apple/Google, Send to Kindle, Rückmeldung).
   Buchtext-Wunsch „Einleitung: der blinde Fleck“ selbst anstoßen (Gliederungspunkt, Text als Vorschlag einer Persona).
   Nächste Runde: 21 „Import und Bilder (B3)“, danach 22 mit den beiden Wünschen zu Fassungen und erledigten Befunden.
0. Nach Runde 19: **Datenordner `~/.local/share/baruch` sichern** (oder „Sicherung jetzt“), Dienst neu bauen und
   starten (keine Migration). Danach die Handprüfung oben unter „Runde 19 … abgeschlossen“ (RAG-Status, Neuaufbau mit
   Dauer der Last auf Eliezer, Präfixe – ein geändertes Dokument-Präfix legt LanceDB bis zum vollständigen Neuaufbau
   still –, eigenes pgvector mit TLS-Hinweis, MultiGPT-Annahmen, Gruppen benennen). Nächste Runde: 20 „EPUB-Vorschau“.
0. Nach Runde 15: **Datenordner `~/.local/share/baruch` sichern**, Dienst neu bauen und starten. Beim Start laufen
   Migration 0035 (`outline_request` mit Art `add`, neu angelegt, alle Zeilen samt id bleiben), 0036
   (`model_connection`, `persona_backend`; Verbindung Eliezer aus der bisherigen), 0037 (`work_sword_source`,
   `persona_bible`) und 0038 (`chat_thread`, `chat_thread_post`), geprobt in
   `crates/baruch-store/tests/migration_0035.rs` bis `migration_0038.rs`. Danach die Handprüfung mit Werk 3
   (oben unter „Runde 15 integriert“), das Zustimmungs-Popup (ein `outline_add` mit `level: 0` mitten im Werk über
   MCP) und – falls vorhanden – ein echter Ollama (`model.context`).
0. Nach Runde 13: **Datenordner `~/.local/share/baruch` sichern** (oder „Sicherung jetzt“), Dienst neu bauen und
   starten. Beim Start laufen Migration 0033 (`sync_*`), 0034 (`web_evidence`) und 0106 (`library_title_remote`,
   `library_highlight_check`), alle ohne Datenänderung, geprobt in `crates/baruch-store/tests/migration_0033.rs`,
   `migration_0034.rs` und `crates/baruch-library/tests/migration_0106.rs`. Nichts geht ohne Knopfdruck hinaus: Websuche (Einstellungen →
   Assistenten → @belege) und Hochladen (Einstellungen → Werk, mit Bestätigung) sind aus. Dann selbst gegen das
   echte MultiGPT prüfen: Volltext eines Treffers, Abgleich einer Sammlung, eine Websuche – und die Annahmen in
   `docs/Architektur.md` abgleichen. Nächste Runde: 14 „Teile und Gliederung“.
- **Rundenplan (2026-11-07):** Runden 13–24 mit je drei Aufgaben in `docs/Backlog.md` („Rundenplan“), Gantt und
  Tabelle in `docs/Fahrplan.md`; jeder offene Backlog-Punkt trägt seine Runde („→ R15b“). Runde 13 beginnt mit dem
  Vorlauf `bau` (Server-Tests bündeln) vor den Worktrees. EPUB ist auf Runde 20 verschoben. Zehn Nutzerfragen stehen
  dort, nicht eingeplant.
0. Nach Runde 12: **Datenordner `~/.local/share/baruch` sichern** (oder „Sicherung jetzt“), Dienst neu bauen
   (der Bau braucht jetzt `protoc`, hier unter `~/.local/bin`) und starten. Beim Start laufen Migration 0031
   (Tabelle `work_rag_collection`) und 0032 (Tabelle `persona_rag`), beide ohne Datenänderung, geprobt in
   `crates/baruch-store/tests/migration_0031.rs` und `migration_0032.rs`. Danach baut `rag::catch_up` den
   LanceDB-Index im Datenordner auf und bettet dafür **alle aktuellen Fassungen, Einfälle, Hervorhebungen und die
   Bibliothek über Eliezer** ein (`text-embedding-nomic-embed-text-v1.5` muss in Eliezer geladen sein; eine
   Anfrage zur Zeit, je 32 Texte – beim echten Buch einige Minuten Last auf Eliezer). Dann selbst prüfen:
   MultiGPT und das eigene pgvector unter Einstellungen → Verbindungen („Verbindung prüfen“, Zuordnung der
   Tabelle, Sammlungen je Werk), Bedeutungssuche in Literatur und Einfällen, @belege mit Belegen.
0. Nach Runde 10: **Datenordner `~/.local/share/baruch` sichern** (oder „Sicherung jetzt“), Dienst neu bauen und
   starten. Beim Start laufen Migration 0015 (Spalte `persona.prompt`, leer = Vorgabe-Prompt) und 0016 (neue
   Tabelle `mcp_token`), beide ohne Datenänderung, geprobt in `crates/baruch-store/tests/migration_0015.rs` und
   `migration_0016.rs`. Der Lektor ist danach **fest @lektor** (Versuch; Einstellungen → Assistenten →
   „automatisch“ stellt zurück). Dann der Versuch mit Werk 3, Kapitel I: Prüfen/Vorschlag/„@persona fragen“
   gegen Eliezer (Gemma, 32768). Ohne `BARUCH_MAX_TOKENS` gelten die Grenzen der Aufträge (bis 4096).
0. Nach Runde 9: **Datenordner `~/.local/share/baruch` sichern** (oder danach „Sicherung jetzt“ unter
   Einstellungen → Daten), Dienst neu bauen und starten. Beim Start laufen Migration 0013 (drei neue Tabellen)
   und 0014 (Spalte `connection.secret_target`), beide ohne Datenänderung, geprobt in
   `crates/baruch-store/tests/migration_0013.rs` und `migration_0014.rs`. Geheimnisse sind an ihr Ziel
   gebunden: Wer die Eliezer-Adresse ändert, trägt den Schlüssel neu ein. Ein Server mit anderem `--data` als
   der Vorgabe nimmt nicht den Schlüsselbund (`--secrets keyring` erzwingt ihn); der Dienst (ohne `--data`)
   bleibt beim Schlüsselbund. Danach im Browser: Menüs mit
   Tastatur, Alt+, (Popup), Einstellungen → Verbindungen „Verbindung prüfen“ für Eliezer (Schlüssel aus dem
   Schlüsselbund `baruch/eliezer`).
0. Nach Runde 7: **Datenordner `~/.local/share/baruch` sichern**, Dienst neu bauen und starten. Beim Start laufen
   Migration 0010 (`persona.color`, keine Datenänderung) und 0011 (neue Tabellen der Einfälle), geprobt auf
   einer nachgebauten Datenbank im Stand development (`tests/round7.rs`, `migration_0010.rs`,
   `migration_0011.rs`). Danach im Buch den leeren Abschnitt (`== `) umbenennen oder löschen; im Browser
   Umschalt+Enter in Überschriften, Alt+E und das Einklappen der Vorschläge ausprobieren.
1. Nutzer: **vorher den Datenordner `~/.local/share/baruch` sichern**, dann den Dienst auf 8765 neu bauen und neu
   starten. Beim Start läuft Migration 0007 (Tabelle `work` neu gebaut mit `AUTOINCREMENT`, `store_meta`,
   `work_retired`; ids, Titel und alle Zeilen bleiben, geprobt auf einer nachgebauten Datenbank). Hinweis von
   r4: Ids von Werken, die schon vor 0007 gelöscht wurden und größer als die heutige größte id waren, kennt
   `sqlite_sequence` nicht und kann sie noch einmal vergeben – nach dem Start
   `SELECT seq FROM sqlite_sequence WHERE name='work'` mit den Ordnern unter `works/.geloescht/` vergleichen.
   Schon früher (Runde 4) liefen beim Start die Migrationen 0005 (eindeutige Titel: das gesäte Werk 1 heißt danach
   den Titel aus `sample::work()`, sofern es denselben Titel wie das Buch trägt; slug und Spiegelordner
   bleiben), 0006 (aufgelöste Maskierungen in Titeln, Nummerierung) und 0105 (Volltextindex der Beiträge).
   Geprobt auf nachgebauten Datenbanken und auf einer Kopie der echten (siehe PM-Abschluss). Danach schreibt der Start
   einen Commit „Stand der Datenbank gespiegelt“: **Dateinamen im Spiegel können sich ändern**, weil sie jetzt dem
   aufgelösten Titel folgen (z. B. „Anna's Laube“ statt „Anna\'s Laube“), und bei Werken ohne
   Vorlage ändert sich die Kommentarzeile `// Baruch: <Titel>` in `main.typ`.
2. Im Browser prüfen (hier ohne Firefox nur mit tsc und Textprüfungen getestet): **Satzbild** (Enter,
   Umschalt+Enter, Strg+I/B/S, Löschen über Blockgrenzen, Umschalter, Satzbild-Diff), **Baum** (Kontextmenü,
   Umbenennen an Ort und Stelle, Doppelklick-Zeitfenster 300 ms, Anlegen, Fokus nach dem Neuzeichnen),
   Werkwahl öffnet sofort,
   Import-Dialog mit vorbelegtem Titel, Ziehen und Ablegen in der Gliederung, Alt+Umschalt+Pfeile,
   Rückfrage beim Teilen/Zusammenführen im Editor, Markieren mit Umbruch im Literatur-Popup.
3. `typst compile main.typ` im Spiegel prüfen (typst hier nicht installiert).
4. Backlog: Satzbild-Einstellungen je Werk und Formate der Vorlage als Knöpfe, Seitenzweige im Spiegel, Baum über das ganze Werk, Alternativen im Lesefluss, Kapitelbilder,
   Export als PDF/EPUB, Gliederungs-Vorschläge der Assistenten.
5. B2: Modell-Backends, `ask_persona`/`chat`, Token für `/mcp`, Literatur-Stubs (Volltext, an Persona geben).

## Offene Fragen an den Nutzer
- **Nach Runde 37 (Hilfe auf der Webseite):** (1) AGENTS.md: Die Zeile „Bedienung (`docs/Bedienung.md`)“ sollte
  künftig „Bedienung (`site/content/hilfe/`, Einstieg `docs/Bedienung.md`), Betrieb (`site/content/betrieb.md`)“
  lauten – ändern? (2) Bilder zur Bibel: die erzeugte „Beispielbibel“ mit einem Vers behalten oder mit einem
  gemeinfreien Modul neu aufnehmen (`--sword ORDNER`)? (3) Hinweistexte in `src/main.rs` (`--allow-host`),
  `templates/settings/mcp.html` und `docs/MCP.md` nennen `docs/Betrieb.md` – künftig gleich
  `https://baruch.bernau.family/betrieb/`? (4) Die sieben Bilder der Seite „Funktionen“ sind mit erneuert – recht so?
  (5) MCP-Zugang im Bild ohne Token – soll ein Beispiel-Token in der Liste stehen (sein Anfang wäre lesbar)?
  (6) Kommandozeile (`baruch-server import/export/passwort`) in der Hilfe für Anwender lassen oder ganz nach
  `/betrieb/`? (7) Lektoren und PDF: Die Hilfe sagt, Lektoren können das Buch zurzeit nicht als PDF abrufen – soll
  das so bleiben? (8) `mcp.md` nennt für Eliezer und MultiGPT keine Bezugsquelle – gewollt? (9) Alt+L und Alt+,
  greifen am Mac vermutlich nicht (nur `event.key`); reparieren (Zweizeiler) und kann jemand die Tasten am Mac
  durchgehen? (10) Einstellungen → Werk ist sehr lang (über 6000 px) – aufteilen oder Inhaltsverzeichnis?

- **Nach Runde 35 (Konten und Rollen):** (1) Soll ein Lektor das Buch als PDF lesen dürfen (heute gesperrt, wie
  jeder Export)? (2) Bleibt es dabei, dass mit Konten auch am eigenen Rechner eine Anmeldung nötig ist (Schalter
  unter Einstellungen → Konten, Vorgabe aus)? (3) Reicht „Lektor kommt über den Reverse-Proxy auf demselben Rechner“,
  oder soll Baruch selbst von anderen Rechnern erreichbar werden? (4) Braucht es ein zweites Autor-Konto oder
  MCP-Tokens je Lektor und Werk? (5) Soll ein Lektor in Fäden ohne Abschnitt schreiben dürfen (heute nur lesen)?
  (6) Aus der Prüfung: `/mcp` ist von der Anmeldung ausgenommen – ein Programm am eigenen Rechner schreibt auch mit
  Konten ohne Token Vorschläge, Beiträge und Einfälle. Soll MCP mit Konten immer ein Token verlangen?
- **Nach Runde 34:** Kommt „schlanker Einstieg, Befunde nach Gewicht, Anmerkungen zum ganzen Buch“ (Vorhaben
  Lektorat) vor oder nach 0.9.1? Die Reihenfolge vom 2026-11-25 nennt nur Konten und Rollen, Reste und Hilfe.
  Zur Kenntnis, vom PM entschieden: Beim Ziehen in die Wolke wandern die Unterknoten mit; Einlesen ohne Zielknoten
  legt Hauptthemen am Werk an.
- **Nach Runde 20:** Java installieren, damit EPUBCheck einmal echt mit dem jar laufen kann (Baruch lädt nichts
  herunter)? Welche Hersteller-Reader auf Android sollen ein Profil bekommen (heute keines erfunden)? Vorgabe
  Rückenbreite 0,2 mm je Blatt beibehalten (Backlog Nutzerfrage 13)?
- **Nach Runde 28:** Soll ein Schreibauftrag im Chat („@lektor Erstelle ein Vorwort“) auch einen **neuen Abschnitt**
  anlegen dürfen (nach den Regeln von `outline_add`, Herkunft im Spiegel), oder bleibt es beim Vorschlag für den
  gezeigten Abschnitt (heutige Vorgabe)?
- **Nach Runde 17:** Hervorheben an Bibeltexten (eine Bibelstelle ist kein Titel der Bibliothek) bräuchte eine
  eigene Tabelle (Migration 0108 der Bibliothek ist frei; 0107 belegt seit Runde 28). Gewünscht, und wenn ja, in welche Runde?
- **Nach Runde 17:** Einstellungen → Assistenten fragt die Modellliste nur noch auf Knopfdruck ab („Modellliste
  abfragen“), damit beim Laden keine Anfrage an Eliezer geht; die Warnung „Modell unbekannt“ erscheint erst danach.
  So lassen, oder die Liste nach dem Laden per JavaScript nachholen?
- ~~Nach Runde 10: Musterpersonas, Verszahlen-Lizenz~~ **entschieden 2026-11-06:** Modellnamen bleiben in
  `sample.rs`, nur die LLM-Einstellungen gehören in die Einstellungen, Anbieter frei je Persona (Backlog „Nach Runde 10 offen“);
  Verszahlen aus `canon_german.h` als Tatsachen übernehmen – so fangen wir an.
- **Zur Kenntnis (PM-Entscheidung Runde 5):** slugs gelöschter Werke bleiben dauerhaft reserviert (ein neues Werk
  mit demselben Titel heißt `<slug>-2`), Werk-ids werden nie wieder vergeben, nach dem Löschen aller Werke wird
  nicht neu gesät. Einverstanden?
- Teile schalten ohne Gliederungsabgleich um (sie haben keine Überschriftszeile); der Vertrag nannte Teile unter
  den Ablehnungen. tree hat es so gelöst, damit die Teile vorhandener Werke umschaltbar bleiben. So lassen?
- `revision.delete` nimmt über RPC nur das JSON-`true` als `confirm`, `outline.delete` auch `"1"`/`"on"`
  (Formulare). Vereinheitlichen?
- Umbenennen eines Teils ändert seinen Text nicht (Teile ohne Überschriftszeile; neue Saat: Teile ohne Text,
  vorhandene Datenbanken behalten „= Teil …“). Bestehende Teile-Texte bereinigen?
- `update_work` lehnt eine Werkvorlage mit Überschrift (`=` am Zeilenanfang) ab, damit Export und Import dieselbe
  Gliederung ergeben. Recht so?
- Umschalten schreibt wie Ändern und Löschen auch ohne Dateiänderung einen Commit (`allow_empty`). Behalten?
- Persona-Hervorhebungen bleiben laut Vertrag immer Vorschlag; soll es „Vorschlag übernehmen“ (als eigene
  Hervorhebung kopieren) geben?
- `#heading(level: 1, numbering: none)[…]` vor der ersten `=`-Überschrift bleibt Teil der Werkvorlage.
  Sollen solche Überschriften künftig Kapitel werden?

## Offene Punkte
- Aus dem Gutachten Runde 5, noch nicht behoben (niedrig):
  - `emit` trennt eine Inline-Insel (`@abb`, `#f`) nicht vom folgenden Text: tippt man direkt dahinter einen
    Buchstaben, verschmilzt er mit ihr (`@abbs`); `parse(emit(k)) == k` gilt dafür nicht.
  - Bearbeiten in `#szene`/`#arbeit`/`#quote` erzeugt den ganzen Rumpf neu (Einzug weg, `#emph[x]` → `_x_`);
    „unberührt bleibt wörtlich“ gilt nur auf oberster Ebene (`wysiwyg.ts` markiert nur den obersten Block).
  - `serialize` lehnt Inseln nur auf oberster Ebene ab; `Inline::Island` in neuen Absätzen und Inseln im Rumpf
    gehen wörtlich durch. `wysiwyg.ts` hat keinen `drop`-Handler (fremdes HTML mit `data-raw` per Ziehen).
  - `DefaultBodyLimit` (~32 MiB) gilt für ganz `/rpc`, nicht nur `work.import`; nur lokal erreichbar.
  - Doppelklick benennt nur den schon gewählten Eintrag um (Bedienung.md sagt es, Backlog nicht).
  - Einfügen im Satzbild macht aus mehreren Absätzen einen (Umbrüche → Leerzeichen).
- Umschalten über MCP gibt es nicht (Entscheidung Runde 3, Backlog); die frühere Rückfrage dazu ist erledigt.
- Runde 5, nicht im Browser geprüft: Satzbild (`wysiwyg.ts`; Enter, Löschen über Blockgrenzen und
  `execCommand` verhalten sich je Browser leicht anders, die Umwandlung nimmt `i`/`b` und `span` mit
  `font-style`/`font-weight` an), Kontextmenü, Umbenennen an Ort und Stelle, Doppelklick-Zeitfenster und
  Fokus nach dem Neuzeichnen (`outline.ts`) – nur tsc und Textprüfungen.
- `wysiwyg.ts` fängt zusätzlich zu `baruch:before-save` das Absenden des Editor-Formulars ab (Rückfall, falls
  das Ereignis fehlt); beides verträgt sich, ist aber doppelt.
- Vertragsabweichungen zur Kenntnis: `SheetStyle` hat zusätzlich `heading_font`/`heading_fill`; `Block`
  serialisiert flach (`{"kind": …, "raw": …}`); `serde_json` in baruch-wysiwyg nur Testabhängigkeit;
  Ablehnungen beim Umschalten als `StoreError::Heading` (-32000) statt `Refused`; `rpc::LibraryView` hat das
  Feld `numbering`; `outline::outline_answer` ist `pub(crate)`.
- Nummerierung aus einer Funktion: das erste `numbering("…")`-Muster gilt für alle Ebenen (Backlog).
- MCP-Clients, die noch `node` schicken, bekommen jetzt -32602 (bewusst, Runde 5); Links `/?node=…` tragen
  weiter nur die Stelle (reine Anzeige).
- Aus dem Gutachten Runde 3: in Runde 4 behoben (Löschbestätigung auf dem Server, `cite` ohne Abschnitt,
  Leerraum beim Markieren, Meldung bei Nummernlücken, Löschen ganzer Werke/Punkte trotz
  `revision_no_delete_current`, Test zur Gleichzeitigkeit, Begründungen an Zusicherungen, Doku zur Literatur).
- Runde 4, nicht im Browser geprüft: Werkwahl/Import-Dialog (`works.ts`), Ziehen und Ablegen und Rückfrage
  (`outline.ts`), Zeichenstellen über DOM-Ranges (`literature.ts`; stimmen nur, solange `source.html` keinen
  zusätzlichen Leerraum in `<p class="source-text">` setzt).
- Linke Spalte: bis zu vier versteckte Formulare je Eintrag (beim echten Buch mit 87 Knoten etwa 80 KB HTML).
- Die Vorschau zeigt Überschriften wörtlich (mit Maskierung); `baruch-server` hängt nicht von `baruch-typst` ab.
- Erster Commit von team/works (4f38448) baut allein nicht, erst mit „Verdrahtung works“ (Folge der
  Commit-Aufteilung).
- Schon falsch gespeicherte Hervorhebungsbereiche werden nicht bereinigt (Prüfung nur beim Anlegen).
- `?lq=%00` zeigt „Treffer für „\0““ mit rohem NUL im HTML (200, „Keine Treffer.“, harmlos);
  `rpc::library_view` könnte Steuerzeichen vor der Anzeige entfernen.
- Ein Revisionstext mit U+E000/U+E001 kann die Hervorhebung verschieben (Ausgabe bleibt maskiert).
- Zitierschlüssel wird nicht auf `[a-z0-9_-:]+` geprüft.
- Regelverstöße in der DB (z. B. Trigger aus 0003/0004) kommen als `StoreError::Db` und damit als `INTERNAL`,
  sofern der Store sie nicht vorher als `Refused` abfängt.
- Literatur: Dialog, Alt+L, Kontextmenü und Markieren der Fundstelle nur mit tsc und Textprüfungen getestet,
  nicht im Browser. Parser-Grenze: `] else [` am Anfang einer Folgezeile gilt als Fließtext.
- `import_file`: Restfenster zwischen `metadata` und `open` (blockiert dann nur einen `spawn_blocking`-Faden).
- Beide Migratoren setzen `ignore_missing` über ein semver-exemptes Feld von `sqlx::migrate::Migrator`.
- Trigger aus 0102 hängen an `revision`, `post`, `node`; baut store diese Tabellen um, braucht library 0103.
- DNS-Rebinding: `guard::local_host` lässt nur IP-Adressen und `localhost` als `Host` zu; eine
  einstellbare Liste erlaubter Namen (Proxy, Rechnername) fehlt. MCP-Token für andere Rechner fehlt (STUB, B2).
- MCP-Sitzungen leben nur im Prozess (nach Neustart „unbekannt“ bis zum nächsten `initialize`).
- `gix-transport` steht (ohne Client-Features) im Abhängigkeitsbaum, weil gix es fest mitbringt.
- Team-Runden: gemeinsamer `CARGO_TARGET_DIR` vermischt die Worktrees (siehe `SKILLS.md`, Tests).

## Offen beim Nutzer
- Fast-Forward `development` → `main` (löst CI aus) und Push nur durch den Nutzer.
- Aufräumen der Team-Zweige und Worktrees aus B1, B1.5, Runde 3, Runde 4 und Runde 5, dazu `team/bugs`,
  `team/proposals`, `team/ideas` mit `.claude/worktrees/wf_77286a38-f7b-2/3/4` (Runde 7).
