# Baruch – Fertigkeiten für Agenten

Regeln stehen in `AGENTS.md`, der Plan in `docs/Plan.md`, der Übergabestand in `HANDOFF.md`.

## Bauen und prüfen
- Kern testen: `nice -n 10 ionice -c3 cargo test -p baruch-core`
- Einmalig TypeScript holen: `cd crates/baruch-server/web && npm ci`
- Server bauen und testen: `nice -n 10 ionice -c3 cargo test -p baruch-server -j 4`; nur ein Fachgebiet der
  Integrationstests: `… --test server <gebiet>::` (z. B. `round12::`)
- Alles testen: `nice -n 10 ionice -c3 cargo test --workspace -j 4` (im Hauptordner bzw. im eigenen Worktree)
- Starten: `cargo run -p baruch-server -- --data DIR` → http://127.0.0.1:8765/ (Vorgabe von `--listen`)
  - `--secrets keyring|memory` oder `BARUCH_SECRETS`: Vorgabe ist der Schlüsselbund nur mit dem
    Vorgabe-Datenordner; mit eigenem `--data` (Wegwerf-Server) bleiben Geheimnisse im Speicher, der
    Schlüsselbund des Nutzers bleibt unberührt. Agenten nie `--secrets keyring` mit einem Wegwerf-Ordner
    (einzige Ausnahme: die vom Nutzer freigegebene, nur lesende Handprüfung gegen Eliezer, siehe unten).
  - `--listen ADRESSE:PORT` oder `BARUCH_LISTEN`; Agenten nur `--listen 127.0.0.1:0` (8765 gehört dem
    Dienst des Nutzers, `packaging/systemd/`) und den eigenen Prozess per PID beenden. Dienst und
    `~/.local/share/baruch` nie anfassen (kein `systemctl`, `pkill`, `killall`).
  - `--data DIR` oder `BARUCH_DATA`, Vorgabe `baruch_store::default_data_dir()` = `dirs::data_local_dir()` +
    `baruch`: Linux `~/.local/share/baruch` (`$XDG_DATA_HOME/baruch`), macOS
    `~/Library/Application Support/baruch`, Windows `%LOCALAPPDATA%\baruch` (Local, nicht Roaming:
    servergespeicherte Profile würden SQLite-WAL und Spiegel bei jeder An-/Abmeldung kopieren).
    Zum Ausprobieren immer einen Wegwerf-Ordner nehmen, nicht die echten Daten.
- Datenordner:
  - `baruch.sqlite` (WAL): die Quelle. Beim ersten Start Kernschema und Beispielwerk
    „Beispielwerk: Leise Abhängigkeit (Auszug)“ (`baruch-core/src/sample.rs`, nur in eine leere Datenbank gesät),
    dann Bibliothek (Literatur, FTS5) mit Beispieltiteln.
  - `works/<slug>/`: Git-Spiegel je Werk (`main.typ` = Werkvorlage + `#include`, `abschnitte/NNN-titel.typ`;
    mit Werkvorlage zusätzlich `vorlage.typ`, und jede Abschnittsdatei beginnt mit
    `#import "../vorlage.typ": *`), ein Commit je Übernahme/Speichern/Import mit Trailern `Persona`, `Model`,
    `Baruch-Node`, `Baruch-Revision`. Nur lesen; den Stand bestimmt die Datenbank, beim Start werden alle
    Werke nachgeführt. slug: höchstens 40 Zeichen, an der letzten Wortgrenze gekürzt, Kollision `-2`, `-3`.
- Typst-Buch importieren (nur über die Kommandozeile, nicht über MCP):
  `cargo run -p baruch-server -- import buch.typ --data DIR` (neues Werk, Ausgabe
  Werk/slug/Abschnitte/Spiegel/Commit; zweiter Import derselben Datei → Exit 1; FIFO oder Ordner →
  „… ist keine Datei“); zurück: `cargo run -p baruch-server -- export <id|slug> --data DIR` (Typst auf
  stdout, nach Import byte-gleich, ohne die `#import`-Zeilen des Spiegels). Der Import liest auch die Bilder aus
  `image("…")` relativ zum Ordner der Datei (nicht hinaus), zurück mit `export <id|slug> --out ORDNER` (Text
  und Bilder byte-gleich, nichts überschreiben).
  `docs/buch.typ` ist das Buch eines Dritten: nie committen, nie in Tests/Doku kopieren; Tests nutzen
  eigene Dateien unter `crates/<crate>/tests/data/`.
- MCP in Claude Code: `claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp`
  (gilt für Handstart und Benutzerdienst). Ohne Token nur Loopback; von anderen Rechnern mit
  `--header "Authorization: Bearer <Token>"` (Einstellungen → MCP-Zugang); Werkzeuge in `docs/MCP.md`.
- Modell-Backend (Runde 10): `serve` bindet Eliezer über `assist::eliezer::EliezerPort` an. Handprüfung gegen den
  echten Eliezer nur, wenn der Nutzer sie für die Runde freigibt: eigener Server
  `serve --listen 127.0.0.1:0 --data <Wegwerf-Ordner> --secrets keyring`, nur lesend (kein
  `connection.set_secret`/`delete_secret`, Adresse nicht ändern), nur das freigegebene Modell, nie
  `/api/v1/models/load|unload|estimate`, eine Anfrage zur Zeit, kurze Ausgaben (`BARUCH_MAX_TOKENS=300` beim
  Start klemmt `max_tokens` jedes Modellaufrufs); den Schlüssel liest nur Baruch. Persona-Modelle im
  Wegwerf-Ordner per `persona.update` auf das freigegebene Modell stellen.
- Bildschirmfoto: `firefox --headless --screenshot bild.png --window-size=1400,860 http://127.0.0.1:PORT/`
- Wegwerf-Server für die Sichtprüfung ohne Anfrage an Eliezer: erst den Datenordner per
  `baruch-server import DATEI.typ --data DIR` anlegen, dann in `DIR/baruch.sqlite` die Verbindungen `eliezer` und
  `multigpt` auf `http://127.0.0.1:9` setzen (`INSERT INTO connection (kind, config) … ON CONFLICT(kind) DO UPDATE`),
  erst dann `serve --listen 127.0.0.1:0 --data DIR --secrets memory` – sonst bettet `rag::catch_up` beim Start über
  die Vorgabe `127.0.0.1:1234` ein. Zugeklappte `<details>` für ein Foto: Seite per `curl` holen, `open` setzen,
  `<base href>` einfügen und die Datei fotografieren.
  (Port des eigenen Wegwerf-Servers; `--no-remote --profile <eigener Ordner>`). Cookies wie `baruch_folded` für
  ein Foto: nach dem ersten Foto eine Zeile in `<profil>/cookies.sqlite` (`moz_cookies`) nach dem Muster von
  `baruch_work` einfügen.
- **Handprüfung gegen Eliezer mit Pförtner** (Integrator Runde 15, statt `--secrets keyring`): ein kleines Programm
  auf `127.0.0.1:0` (Python, `http.server`) leitet nur `POST /v1/chat/completions` (höchstens die freigegebene Zahl,
  eine zur Zeit, nur das freigegebene Modell) und die lesenden Listen `GET /v1/models`, `GET /api/v1/models` an
  `127.0.0.1:1234` weiter, alles andere (Einbettungen beim Start, load/unload) bekommt 403. Den Schlüssel liest nur
  der Pförtner (`keyring.get_password("baruch", "eliezer")`) und setzt ihn in den Authorization-Kopf; nie ausgeben,
  nie protokollieren. Baruch läuft mit `--secrets memory`, die Verbindung `eliezer` zeigt auf den Pförtner,
  `multigpt` auf `127.0.0.1:9`. Das Protokoll des Pförtners (Dauer, Länge, Nachrichten) zeigt, ob der Kontext
  vollständig war.
- Fotos nach einem Klick (Integrator Runde 17, z. B. „Zitate prüfen“ mit Markierung im Satzbild, aufgeklappte
  Kommentare): kleiner Vorschalt-Server derselben Herkunft, der alles weiterreicht und in HTML mit `?klick=SELEKTOR`
  ein Skript (Klick nach `DOMContentLoaded`) und ein Bild einfügt, das erst nach 3 s kommt – das `load`-Ereignis,
  auf das `firefox --screenshot` wartet, kommt so erst nach dem Ergebnis des Klicks.
- Gemeinsames Scratchpad: Alle Agenten einer Runde teilen es. Hilfsdateien und Logs nur mit der eigenen
  Aufgaben-ID im Namen (`verszaehlung_…`, `integrator17_…`), sonst überschreibt ein anderer Agent sie.
- Seiten mit JavaScript, die nur als POST-Antwort entstehen (z. B. Hinweisseite nach `/assistenten/act`): Antwort per
  `curl` speichern und über einen kleinen Vorschalt-Server derselben Herkunft ausliefern (alles andere an den
  Wegwerf-Server weiterreichen), sonst laden die Module von `file://` nicht.
- Nie parallel zu Firefox, anderen Agenten oder Testläufen anderer Projekte.

## Oberfläche
- Vorlagen: `crates/baruch-server/templates` (`base.html`, `workspace.html`,
  `fragments/{tree,editor,edit_toolbar,revisions,revtree,assistants,library,proposals,idea_capture}.html`), eingebettet in `view.rs`; für das
  Literatur-Popup und seine Seiten außerdem `literature.html`, `source.html`, `fragments/highlights.html`
  (eigene Umgebung in `literature.rs`); für die Werkverwaltung `works.html`, `work_new.html`,
  `work_edit.html`, `work_delete.html` (eigene Umgebung in `works.rs`, `fragments/work_new.html` auch in
  `view.rs` für den Import-Dialog der Kopfzeile); für die Gliederung `outline.html`, `outline_edit.html`,
  `outline_confirm.html` (eigene Umgebung in `outline.rs`); für das Gliederungsschema `work_numbering.html`
  (eigene Umgebung in `numbering.rs`); für die Bilder `assets.html`, `asset.html` (eigene Umgebung in
  `assets.rs`); für die Einstellungen `settings.html` und `settings/*.html` (eigene Umgebung in `settings/mod.rs`,
  dazu `settings/{verbindungen,quellen}.html` in `connections/pages.rs`); für die Einfälle
  `ideas.html`, `idea.html` (eigene Umgebung in `ideas.rs`, `fragments/idea_capture.html` auch in `view.rs`
  für den Dialog der Kopfzeile). CSS je Gebiet in
  `static/{app,library,works,outline,wysiwyg,numbering,assets,proposals,ideas,typeset,menu,settings,connections}.css`,
  zusammengefügt in `lib.rs` (`STYLE`). minijinja 3 nimmt eigene Typen über `Value::from(Serde(x))`.
- Kopfzeile (Runde 9): nur in `base.html` (`{% block topbar %}`, öffnendes Tag exakt `<header class="topbar">`);
  Seiten liefern `{% block page_title %}` und `{% block page_actions %}`, nie eine eigene Kopfzeile. Menüs
  Werk/Ansicht/Werkzeuge als `<details class="menu">` (ohne JavaScript bedienbar), Widget `menubar`
  (`web/src/menu.ts`, Abzeichen über RPC `menu.badges`), Daten der Arbeitsseite aus
  `crate::menu::workspace_topbar`. Je Seite höchstens ein `data-literature-open`, `data-idea-open`,
  `data-work-import-open`, `data-view-toggle`. Test `tests/round9_menu.rs` (neue Seiten dort in `all_pages`
  aufnehmen). Das Thema-Widget bleibt in `panes.ts` und hört auf `baruch:settings-changed`.
- Einstellungen: `src/settings/` (Registrierung, Rahmen, Seiten), Vorlagen `templates/settings.html` und
  `templates/settings/*.html` (eigene Umgebung in `settings/mod.rs`), Widget `web/src/settings.ts`, Stil
  `static/settings.css`. Neue Seite: Eintrag in `PAGES` mit einer `PageRender`-Funktion, die nur
  `<section class="settings-page" data-settings-page-body="<slug>">…` liefert; Cookies/Host über
  `settings::request()`. Formularblöcke anderer Seiten nie kopieren, sondern als `{% block content %}` mit
  herausgezogenem Kontextaufbau wiederverwenden (Muster `work_edit.html`/`work_numbering.html`).
- Tests mit Sicherung legen den Datenordner in einen Unterordner eines tempdir (`round9_settings.rs`,
  `open_in_subdir`), sonst landet die Sicherung im Temp-Ordner selbst.
- Verbindungen und Quellen: `src/connections/` (RPC `connection.*`, `sources.*`, Formulare
  `POST /einstellungen/{verbindungen,quellen}/act`, MCP `sources_list`). Geheimnisse nur über
  `AppState::secrets` (`SecretStore`); Tests nehmen immer `MemorySecrets` (`AppState::open`), nie den echten
  Schlüsselbund; Prüfen nur gegen eigene Test-Server auf `127.0.0.1:0`. Geheimnisse sind an ihr Ziel gebunden
  (Migration 0014): Nach `connection.set` mit neuer Adresse im Test erst `connection.set_secret`, sonst hält
  `connection.check` das Geheimnis zurück (`tests/round9_connections.rs`, `confirm_secret`). Kein Geheimnis in HTML, RPC, Meldungen,
  Adressen oder Prüfergebnissen (Test mit eindeutigem Test-Geheimnis in `tests/round9_connections.rs`).
- Werke: Werkwahl `<select data-widget="work-select">` in der Kopfzeile (`web/src/works.ts`), Cookie
  `baruch_work`; `/werke…` ohne JavaScript. Gliederung: der Baum in `fragments/tree.html` (Nummerierung im Kopf,
  Formulare ↑ ↓ ← → und Links `nojs-only` als Rückfall ohne JavaScript), Widget `outline` (`web/src/outline.ts`:
  Ziehen und Ablegen, Kontextmenü, Umbenennen per Doppelklick/F2, Anlegen, Löschen mit `confirm: true`); nach
  einer Änderung ohne Neuladen `baruch:navigate` (hört `workspace.ts`). Jede schreibende Baumaktion ruft vorher
  `leaveEditor()` (`widgets.ts`), die einzige Rückfrage bei ungespeichertem Text.
- Satzbild: Bereich `div.wysiwyg[data-widget=wysiwyg]` im Editor-Formular (`fragments/editor.html`, ohne
  JavaScript `hidden`), Widget `web/src/wysiwyg.ts`, Stil `static/wysiwyg.css`, RPC `wysiwyg.*` in
  `src/wysiwyg.rs` (`docs/WYSIWYG.md`). Abgeschickt wird immer das Textfeld: `outline.ts` löst vor
  `new FormData` `baruch:before-save {wait}` aus, `wysiwyg.ts` füllt das Textfeld. Kein Import zwischen
  `outline.ts` und `wysiwyg.ts`. Cookie `baruch_editor` = `satz` | `quelltext`. Eine Überschrift ist im
  Satzbild immer eine Zeile (`splitHeading`, `headingEdits`); Titelregeln prüft der Client mit derselben
  Meldung, maßgeblich bleibt der Server (`check_title`).
- Vorschläge im Lesefluss: `fragments/proposals.html`, Widget `proposals` (`web/src/proposals.ts`), Cookie
  `baruch_folded`. Seit Runde 8 an ihrer Stelle: `baruch_wysiwyg::align` gegen die **gezeigte** Fassung, je
  Stück ein `details.pv[data-pv-at]`; `wysiwyg.ts` setzt die Stücke ins Satzbild und übergeht `.pv` beim
  Speichern. Persona-Farben nur als geprüfte `#rrggbb` über `crate::personas::style` ins
  `style`-Attribut (`--pc-light`, `--pc-dark`, `--pc-line`, `--pc-line-dark`), nie aus Nutzereingaben ungeprüft.
- Adressen: kanonisch `/?work=W&key=K[&rev][&view=preview][&pick][&lq]` (`view::node_url`); neue Links und
  Weiterleitungen nie mit `node=` (Test `round8_posts::no_generated_link_or_redirect_carries_node_any_more`).
- Beiträge verwalten: `src/posts.rs` (`post.*`, `POST /beitraege/act`), Widget `chat` (`web/src/chat.ts`,
  LocalStorage `baruch_chat_seen` nur in try/catch). MCP darf nur eigene Beiträge (Persona bzw. `@mcp:<name>`).
- Typst-Satz: Crate `baruch-render` (offline, SVG), `src/typeset.rs` (`preview.typst`, `/werke/{id}/satz`,
  statischer Zwischenspeicher, eine Semaphore, 60 s), `src/mcp/typst.rs` (`typst_check`). Tests, die den
  statischen Zwischenspeicher nutzen, hängen eine eigene Kommentarzeile ans Werk (eigener Stand).
- Buch als PDF und Umschlag (Runde 11): `typeset::prepare` legt den Rahmen aus `cover::frame` um die Quelle
  (Umschlag-Vorder-/Rückseite, Titelei; ohne beides byte-gleich) – Satz-Ansicht, `work.pdf`, `/werke/{id}/pdf` und
  `export --pdf` nutzen dasselbe `prepare` (gleiche Seitenzahl). PDF nur über `crate::pdf::source_to_pdf`
  (`baruch_render::compile_pdf`, eine Erzeugung je Werk, Sperre `typeset::RUNS`, 120 s). Die Markierung „PDF
  entsteht“ ist prozessweit je Werk-id: Tests, die PDFs erzeugen oder `pdf::claim` setzen, halten vorher
  `common::PDF_SERIAL` (alle Server-Tests laufen in einem Prozess; `round11_pdf.rs`, `round11.rs`,
  `round11_cover.rs`). PDF-Antworten sind Bytes: in Tests
  nicht über `Server::send` (UTF-8), sondern `oneshot` + `to_bytes`. Umschlag/Buchangaben: Store `src/book.rs`
  (Migration 0017), `baruch-typst/src/cover.rs` (rein; Nutzertexte nur als String-Literale), Server `src/cover.rs`.
  Druckprüfung `Rendered.print` (Rasterbilder unter 300 dpi), nicht in `diagnostics`.
- Einfälle: Dialog `<dialog data-widget="idea-capture">` in der Kopfzeile (`web/src/ideas.ts`: Alt+E,
  Markierung als `quote`, Fokus zurück), ohne JavaScript die Seiten `/einfaelle…` mit Formularen an
  `POST /einfaelle/act`.
- Literatur: Popup `<dialog data-widget="literature">` in der Kopfzeile von `workspace.html` (Inhalt
  `fragments/library.html`), ohne JavaScript die Seiten `/literatur` und `/quelle` aus `src/literature.rs`
  mit eigener Template-Umgebung (`literature.html`, `source.html`, `fragments/highlights.html`) und
  Formularen an `POST /literatur/act`. RPC `literature.render` und `highlight.*` (`docs/RPC.md`).
  minijinja maskiert `/` in Werten als `&#x2f;`; selbst gebaute Adressen gibt `literature::href` sicher weiter.
- Personas (Runde 10): `src/assist/` (Port `port.rs`, Adapter `eliezer.rs`, Kontext `context.rs`, Rollen-Prompts
  `prompts.rs`, Hooks `hooks.rs` → `crate::scripture`, RPC `rpc.rs`, Routen `routes.rs` mit
  `POST /assistenten/act` und `/assistenten/stream`), Modell-Backend `src/models.rs` (RPC `model.*`, prozessweite
  Warteschlange, Zwischenspeicher der Modellliste), Widget `web/src/assist.ts`, Stil `static/assist.css`.
  Modellaufrufe nur über `crate::models::chat` bzw. `state.model` (Warteschlange, Bindung des Schlüssels, Alias),
  nie `baruch_llm::chat_stream` direkt. Modell **ohne** `write_lock` aufrufen, erst zum Schreiben sperren und
  `key` neu auflösen. Die Chat-Eingabe ist ein Formular an `/assistenten/act` mit `method=persona.chat`, Feld
  `message` (Tests suchen `value="persona.chat"`).
- Runde 15: Chat in `assist/chat.rs`, Stückeln `assist/chunk.rs`, SWORD-Faktenblock `assist/bible_facts.rs` (Daten
  aus `src/bible_research.rs`), Fortschritt `assist::Progress` (`Posted`, `Retry`, `Compressed`) → SSE in
  `routes.rs`; Budget nur über `state.model.context(state, &persona)` (`ModelPort::context`, `models::context`).
  Toast `fragments/toast.html` + `web/src/toast.ts` (Ereignis `baruch:toast`) – CSS nur für
  `.toast[data-widget="toast"]`, denn die bloße Klasse `.toast` ist die rote Fehlermeldung aus `app.css`.
  Hinweisseite ohne JavaScript `assist_notice.html` (`routes::notice_page`). Verbindungen je Persona: Store
  `model_connections.rs` (Migration 0036), `baruch_llm::Client` (`client.rs`), Freigabe „extern“ in
  `EliezerPort::chat`. SWORD-Kommentare/Lexika `baruch-sword/src/reference.rs`, Auswahl je Werk
  `sword_sources.rs` (0037), Bausteine `bibles::commentary_panel_html`/`commentary_hits_html`. Bibeln in der
  Literatur `literature/bibles.rs` (`/literatur/bibel`), Fäden ohne Abschnitt `src/threads.rs` (0038), MCP-Warteschlange
  je Client `mcp/quota.rs`.
- Runde 16 („Predigt“): Werkart Predigt nur über `work.kind == WorkKind::Sermon`; Vorlage in `store/works.rs`
  (`Preset::Sermon`, `create_work_with_text`, Predigttext aus SWORD in `server/works.rs`). Zeitwächter: Zählregel nur
  `baruch_typst::timing::spoken_words` (das Editor-Widget `web/src/timing.ts` zählt über RPC `timing.count`, nie in
  TypeScript), Einstellungen `Store::timing_settings` (0039, nur SQLite), Server `src/timing.rs`
  (`work_timing`/`timing_json`; `WorkTiming::shown` = Predigt oder Zielzeit). Stichwortzettel: Satz
  `baruch-render/src/notes.rs` (Nutzertexte nur als Typst-Zeichenketten), Server `src/pdf/notes.rs` (hängt an
  `pdf::handles`/`pages`, PDFs unter `common::PDF_SERIAL`), Seite `templates/notes.html`. Stil aller R16-Bausteine in
  `static/predigt.css`. „Kritisch:“: `prompts::critical_finding` (Zeilenanfang oder hinter der Teil-Angabe) für
  Prüfung und Zitatprüfung, `critical_line` (nur Zeilenanfang) zum Entfernen aus Vorschlägen.
- Handprüfung mit SWORD-Testbibel: Ein Wegwerf-Server braucht echte Moduldateien. Dafür vorübergehend ein Beispiel
  `crates/baruch-sword/examples/<name>.rs` schreiben (`baruch_sword::fixture`, frei erfundener Text), mit
  `cargo run -p baruch-sword --example <name> -- <Ordner im Scratchpad>` ausführen, die Datei wieder löschen (nie
  committen) und den Ordner per RPC `bible.set_dirs` setzen. Gliederungsanträge im Wegwerf-Server über MCP
  (`initialize` → `Mcp-Session-Id`, dann `tools/call outline_rename`/`outline_add`).
- Runde 11 Oberfläche: Umschlagseite `templates/cover.html` und Block `fragments/cover_panel.html` (eigene
  Umgebung in `cover.rs`, Stil `static/cover.css`; auch in Einstellungen → Werk und oben auf der Seite „Bilder“),
  Anträge zu Bildern `fragments/asset_requests.html` (eigene Umgebung in `asset_requests.rs`, unten auf
  „Bilder“). Einstellungen → Assistenten: je Persona ein Tab (`?persona=<handle>`, Widget `persona-tabs` in
  `web/src/persona_tabs.ts`), ein Formular `action=save`; Chat-Vervollständigung „@…“ `web/src/mention.ts`.
  Vom Server gebaute Adressen in Vorlagen (`back`, `url`) mit `|safe`, sonst schreibt minijinja `/` als `&#x2f;`
  (Test `tests/assets.rs`).
- Bibelstellen: Crate `baruch-scripture`, Server `src/scripture.rs` (RPC `scripture.check`, `facts_for`,
  `mark_answer`), MCP `src/mcp/scripture.rs`. Runde 17: Verszählungen `versification.rs` (alle 18 von
  `sword-modules`, German/KJV aus `data/*.tsv`) und Abbildung `versification/mapping.rs` mit Regeltabelle
  `data/versification-map.tsv` (Drehpunkt KJV, nie raten), RPC/MCP `scripture.map`/`scripture_map`
  (`src/scripture_map.rs`, `src/mcp/scripture_map.rs`); Zitat-Spans nur über `crate::quotes::quote_spans`
  (inline + Blockzitate, auch in `#szene`), Zuordnung `wording::quote_for`; Lesen in der Zählung des Moduls nur
  über `bibles::read_passage` (nie `bible.verse` mit deutscher Stelle); Prüfanzeige RPC `bible.marks`, Seite
  `src/scripture_check.rs`, Widget `web/src/scripture_check.ts` (CSS Custom Highlight API, keine DOM-Änderung);
  Schlüssel verschlüsselter Module nur in `AppState::secrets` (Konto `sword-cipher:<modul>`), Tests mit
  `MemorySecrets` und `Fixture::enciphered(<erfundener Schlüssel>)`, BZIP2/XZ über `Fixture::compression`.
  Stellenaussagen in Befunden: `assist/ref_claims.rs` (nach `mark_unbacked_criticals`). MCP-Tokens: Store `src/mcp_tokens.rs` (Migration 0016, nur Hash),
  Server `src/mcp/tokens.rs` (RPC `mcp_token.*`), Seite `settings/access.rs`, `guard::remote_only_mcp`.
  Seiten, die ein Geheimnis einmal zeigen: 200 direkt (keine Weiterleitung), `Cache-Control: no-store`, Wert über
  ein eigenes `task_local` nur in diese Antwort.
- RPC-Methoden stehen oben in `src/rpc.rs`; `ui.render` liefert das HTML der drei Bereiche,
  `library.search` ist das Suchwerkzeug des Lektors (Werk und Bibliothek). Alle Methoden nehmen `work?`
  (fehlt → Vorgabewerk). MCP in `src/mcp.rs`, Werkzeuge in `src/mcp/tools.rs`, Sitzungen in
  `src/mcp/session.rs`, Einfälle in `src/mcp/ideas.rs` (`docs/MCP.md`); Fehler immer über `rpc::RpcError::from`. Schreibende Routen hinter `guard::same_origin`; Testanfragen mit fremdem `Origin`
  bekommen 403, `/rpc` braucht `Content-Type: application/json`.
- Doku: Bedienung = Hilfeseite unter `site/content/de/hilfe/` (seit Runde 37; `docs/Bedienung.md` ist Einstieg und
  trägt nur Technisches), `docs/Architektur.md`, `docs/RPC.md`, `docs/MCP.md` – im selben Commit nachziehen.
  Formulare (`form[data-rpc]`, Feld `method`) gehen ohne JavaScript an `POST /act`, mit JavaScript über RPC.
- Widgets: `web/src/widgets.ts` (`register`/`mount`), `workspace.ts` (Navigation, Formulare, Alt+←/→ ohne
  Umschalt, `confirm: true` beim Löschen, Ereignis `baruch:navigate` mit `detail.url` → Neuzeichnen),
  `panes.ts` (Spaltengrenzen, Thema, Editor, Gliederung, Chat), `library.ts` (Titel filtern), `menu.ts`
  (Menüleiste), `settings.ts` (Einstellungs-Popup, Alt+,),
  `literature.ts` (Popup mit Alt+L, Fundstelle im Editor, Kontextmenü der Dokumentansicht).
- Vorbild: David (`~/Dokumente/Projekte/Psalm/server`).

## Crates
- `baruch-core`: Typen, Beispieldaten, Lektor-Wahl, Diff. `Post` hat `id` (0 = neu) und `created`.
- `baruch-render`: Typst-Satz (Runde 8), rein: `Source`, `stand`, `compile` → SVG-Seiten, Überschriften-Seiten,
  Diagnosen. Einzige Crate mit `typst*`-Abhängigkeiten (0.15.1, `typst-kit` nur `embedded-fonts`/`scan-fonts`);
  Tests `tests/{compile,stand,schemes,lines,pdf,print}.rs`; Runde 11: `compile_pdf` (typst-pdf 0.15.1), `PrintWarning`.
- `baruch-typst`: rein, ohne DB: `parse` (Werkvorlage + Überschriften `=`/`==`/`===`, `title` aufgelöst, `raw_title`
  wörtlich) und `join` (byte-gleich zurück); `escape_title`/`unescape_title`, `is_newline`, `heading_line`,
  `heading_numbering` (letzte Regel der obersten Ebene, Mustersprache mit `P|k` und `P1;P2;…`), `scheme`
  (Gliederungsschemata: `Scheme`, `Numbering`, `export_template`), `images::image_paths` (Bildpfade aus
  `image("…")`), `format_number` (Tests `tests/{roundtrip,numbering,scheme,images}.rs`, Daten `tests/data/`). Öffentliche Funktionen nicht
  ändern, ohne `baruch-wysiwyg` mitzuziehen. Runde 14: `marker` (Teil `#heading(level: 1, numbering: none)[T]
  <baruch-part>`, „ohne Nummer“ `… <baruch-unnumbered>`; `part_line`, `unnumbered_line`, `relevel_line`,
  `set_numbered_line`, `part_page_rule`; `Heading.level` 0 = Teil, `Heading.numbered`), Tests `tests/parts.rs`
  (Daten `tests/data/teile.typ`); `Numbering::from_template` (Re-Import, Funktionen je Ebene, Wort-Schemata).
  Markierungen nie als Literal schreiben, immer über diese Funktionen. Text eines neuen Teils =
  `heading_line(0, Titel, None) + "\n"`; Export/Spiegel/Satz projizieren `node.numbered` per
  `set_numbered_line` (`store/snapshot.rs`), die DB-Texte bleiben. Der Trigger `node_numbered_default` (0006)
  setzt bei leerer Nummer `numbered = 0` – wer Knoten einfügt, setzt `numbered` danach ausdrücklich (Import)
  bzw. fügt mit vorläufiger Nummer `'0'` ein (`outline.rs`).
- `baruch-wysiwyg`: rein, ohne DB und Dateisystem: `parse` (scheitert nie; Verkettung der `raw` = Text),
  `serialize` (Keep wörtlich, bearbeitete Blöcke kanonisch, Inseln nur als Ganzes löschbar), `emit`,
  `render_html`, `diff_html` (mit Bildern: `render_html_with`/`diff_html_with` und `image_path`; Inseln mit
  Bild als `ws-figure`), `emit`/`serialize` beenden eine Überschrift am ersten Zeilenumbruch
  (`split_heading`, Runde 7),
  `sheet_style`/`sheet_css` (nur Regeln der obersten Ebene). Tests
  `tests/{roundtrip,emit,serialize,html,style,blocks,align,parts,konstrukte}.rs`, Daten `tests/data/satzbild.typ` (frei erfunden, alle
  Konstrukte und Inseln) und `tests/data/szenen.typ` (Szenen, Arbeitsaufträge, Labels, Bildinsel); der Eigenschaftstest läuft mit
  fester Saat und lässt sich über `WS_SEED`/`WS_CASES` erweitern.
- `baruch-store`: SQLite (sqlx, nur Laufzeit-API, Migrationen 0001–0099); `accept`/`reject`/`save_human`/
  `add_proposal`/`add_post`/`import_typst` und der Revisionsbaum (`src/tree.rs`: `set_current`, `save_human_on`,
  `update_revision`, `delete_revision`, `revision_tree`; `switch_to` hinter `set_current`/`accept`), Werke (`src/works.rs`: `create_work`, `update_work`,
  `delete_work`, eindeutige Titel über `title_key`) und Gliederung (`src/outline.rs`: `outline_*`,
  `set_numbering`, `set_numbering_scheme`, `numbering_preview`, `numbering_rule`, `sync_heading`,
  `check_title`/`MAX_TITLE_CHARS` = Titelregeln für alle Wege, `StoreError::Title` → -32602) und Bilder
  (`src/assets.rs`, Migration 0009: `asset_*`, `add_asset`, `replace_asset`, `import_typst_with_assets`,
  `export_assets`, `normalize_asset_path` = Normalform für jeden Pfadvergleich; Tests
  `tests/{assets,migration_0009}.rs`), Persona-Farben (`src/personas.rs`, Migration 0010:
  `persona_colors`, `set_persona_color`; Runde 9 `persona_list`, `update_persona` mit `PersonaEdit::check`;
  Tests `tests/{personas,migration_0010,round9_personas}.rs`), Verbindungen und Quellen (`src/connections.rs`,
  Migration 0013: `connection_*`, `work_rag`/`set_work_rag`, `source_*` mit lückenloser `position`; Tests
  `tests/{connections,migration_0013,migration_0014}.rs`; 0014: Bindung `secret_target`), Rollen-Prompt und
  Lektor-Wahl (Migration 0015, `persona_prompt`/`set_persona_prompt`, `lector_choice`/`set_lector_choice` in
  `store_meta` `lector`; Tests `tests/{round10_personas,migration_0015}.rs`), MCP-Tokens (`src/mcp_tokens.rs`,
  Migration 0016; Tests `tests/{mcp_tokens,migration_0016}.rs`) und Einfälle
  (`src/ideas.rs`, Migration 0011: `idea_*`, `Idea::entry_markdown`, `einfaelle.md` über `snapshot::files` nur
  bei vorhandenen Einfällen; Tests `tests/{ideas,migration_0011}.rs`), Beiträge (`src/posts.rs`, Runde 8:
  `post_get/delete/move/to_idea`, `PostInfo::entry_markdown`; Tests `tests/posts.rs`), Export-Karte
  (`src/export_map.rs`: `export_map(w).typst == export_typst(w)` byte-gleich; Test `tests/export_map.rs`) je eine Transaktion (`BEGIN IMMEDIATE`),
  `mirror_snapshot` für Git, `export_typst`. Neue Migration: `build.rs` hat `rerun-if-changed=migrations`;
  bestehende nie ändern (Prüfsumme). sqlx-sqlite führt jede Migration in einer eigenen Transaktion aus
  (0.8.6 ignorierte `-- no-transaction`, **0.9 beachtet es** – die Markierung nie setzen, Test
  `migration_0007`): kein `BEGIN`/`COMMIT` und kein `PRAGMA foreign_keys` in einer
  Migration, stattdessen `PRAGMA defer_foreign_keys=ON` und eine Wache über `pragma_foreign_key_check` (0007).
- `baruch-library`: Titel, Zitate, Hervorhebungen (`src/highlights.rs`, 0103), Volltextsuche `search_fts`
  (Migrationen 0101–0199; 0104 hält den Index bei geänderten und gelöschten Revisionen aktuell, 0105 bei
  gelöschten und verschobenen Beiträgen); `build.rs` hat
  `rerun-if-changed=migrations`. Beide Migratoren
  teilen `_sqlx_migrations` mit `ignore_missing`; Reihenfolge `Store::open` → `migrate` → `seed_if_empty`
  (`AppState::open` im Server).
- `baruch-llm` (Runde 10): Client für Eliezer (`models`, `chat_stream`, `Limits`, `LlmError`, `Queue`, `alias`),
  rein; Nachbau `mock::MockEliezer` (öffentlich, für Tests auch des Servers); Tests `tests/{client,queue}.rs`,
  Einheitentests in `src/{lib,sse}.rs`.
- `baruch-scripture` (Runde 10): rein (std + serde), `parse`/`scan`/`find_book`/`books`, `RefStatus`, `NoBible`
  (STUB Wortlaut); Daten `data/versification.tsv` (nur Zahlen, Quelle/Lizenz im Kopf, nie von Hand ändern – neu
  erzeugen aus der Quelle und Kopf nachziehen); Tests `tests/{references,versification}.rs`. Seit 2026-11-06
  `versification` (German, KJV aus `data/versification-kjv.tsv`) und `wording` (Zitat finden und vergleichen),
  Test `tests/wording.rs`.
- `baruch-sword` (2026-11-06): SWORD-Bibeln lesen über `sword-modules` (CC0, nur Feature `zip`; nie libsword oder
  GPL-Crates wie `rsword_chirho`). Testbibeln immer selbst schreiben (`baruch_sword::fixture`, frei erfundener
  Text, Temp-Ordner); Server-Tests setzen ihre Modulordner per `bible.set_dirs` (`tests/sword_bibles.rs`), nie die
  echten Ordner des Rechners. Nie fremden Bibeltext in Repo, Tests oder Doku.
- `baruch-rag` (Runde 12): gemeinsame Schnittstelle der RAG-Wege (Vertrag, wörtlich; `tests/port.rs` prüft den
  SHA-256 der beiden Dateien – eine Änderung zieht den Test nach). `baruch-lancedb`: lokaler Index über `lancedb`
  0.41, rein (keine SQLite, kein Netz), eigene Tokio-Laufzeit, Tests `tests/index.rs`. `baruch-llm` dazu
  `embed.rs` (`POST /v1/embeddings`, Alias `nomic-embed-text`), Test `tests/embeddings.rs`.
- `baruch-gitmirror`: Git-Spiegel über gix (reines Rust), synchron, im Server in `spawn_blocking`;
  `CommitRequest::allow_empty` erzwingt einen Commit bei unverändertem Baum (Protokoll).
- `baruch-server`: Bibliothek (`src/lib.rs`, `rpc.rs`, `view.rs`, `mirror.rs`, `import.rs`, `mcp.rs`, `literature.rs`,
  `works.rs`, `outline.rs`, `wysiwyg.rs`, `numbering.rs`, `assets.rs`, `personas.rs`, `ideas.rs`, `posts.rs`,
  `typeset.rs`, `menu.rs`, `settings/`, `connections/`, `assist/`, `models.rs`, `scripture.rs`, `mcp/ideas.rs`,
  `mcp/typst.rs`, `mcp/scripture.rs`, `mcp/tokens.rs`, Runde 11 `pdf.rs`, `cover.rs`, `asset_requests.rs`, `mcp/assets.rs`,
  Runde 12 `rag/`, `search_hooks.rs`, `connections/{pgvector,multigpt,collections}.rs`, `mcp/rag.rs`,
  `assist/rag_facts.rs`) und dünnes `main.rs`
  (`serve`/`import [--title]`/`export [--out | --pdf DATEI]`). Vorgabewerk zur Laufzeit
  `state.default_work()` (kleinste id jetzt; `state.work` ist nur der Stand beim Start und wird nicht gelesen),
  Spiegel je Werk (`state.mirror(work)`, beim ersten Gebrauch geöffnet). Tests (Module der einen
  Binärdatei `tests/main.rs`, siehe „Tests“) in
  `tests/{rpc,pages,library_search,literature,git_mirror,import,works,work_crud,outline,node_keys,headings,tree,wysiwyg,satzbild,numbering,assets,mcp_assets,origin,mcp,revisions,round3,round4,round5_fixes,round5,round6,round7_titles,proposals,personas,ideas,mcp_ideas,round7,concurrency,outline_mcp,round9_menu,round9_settings,round9_connections,round10,round10_models,round10_personas,round10_scripture,round10_tokens,round11,round11_pdf,round11_mcp_assets,round11_cover,round11_images,round11_personas,round12…,round13…,round14,round14_parts,round14_outline,round14_numbering}.rs`
  mit Temp-Datenordner (`tests/common`:
  `open()`, `import_garden()`, `rpc()`, `git()`). MCP-Tests setzen die Gegenstelle mit
  `request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127,0,0,1], 40000))))`.

## Revisionen
- Revisionsbaum (Migration 0004): jede Revision hat ein Elternteil `based_on` (Wurzel = NULL), auch eigene
  Speicherungen (`save_human_on` nimmt die gezeigte Fassung, `save_human` die aktuelle).
- Genau eine `current` je Abschnitt (Teilindex `revision_one_current`); sie ist eine **Markierung**, umschaltbar
  auf jede andere Fassung (`Store::set_current`, RPC `revision.set_current`). Übernehmen = Umschalten auf einen
  Vorschlag; Verwerfen bleibt Status für Vorschläge. Statuswechsel nur → current (aus proposed, rejected,
  superseded), current → superseded, proposed → rejected (Trigger `revision_status_transition`).
- Ändern (`update_revision`: Text, Notiz, setzt `edited`) und Löschen (`delete_revision`) nur durch den
  Menschen, nie über MCP. Löschen braucht eine Bestätigung, die der Server prüft (`/act`: Kästchen
  `confirm`, RPC: `confirm: true`). Herkunft (Abschnitt, Nummer, Autor, Modell, Zeit) ist per Trigger unveränderlich, die
  aktuelle Fassung nicht löschbar, Kinder einer gelöschten Fassung hängen an deren Elternteil.
- Nummern sind eindeutig und steigend, nach dem Löschen mit Lücken, nie wiederverwendet (`node.last_rev`).
  Überall über die **Nummer** zugreifen (`Node::revision`, `position_of`, `current_number`), nie über Stelle ± 1.
- Abschnitte: `Node::id` ist die Stelle (seq), die jede Gliederungsänderung neu vergibt; `Node::key` (= `node.id`
  in SQLite) bleibt. Schreibende Formulare, RPC und MCP schicken `key` mit (`rpc::resolve_keys` unter
  `write_lock`, Schlüssel geht vor). Neue Schreibwege genauso.
- Aktuelle Fassung über `revision.save` oder Ändern der aktuellen und jeder Vorschlag: genau eine Überschrift der
  Ebene des Punkts, sonst `StoreError::Heading`. Umschalten und Übernehmen (`Store::switch_to`): die Gliederung
  folgt der Fassung, auch in der Ebene (Unterpunkte mitgestuft, ein Commit); die Fassung wird nie umgeschrieben.
- Werk-ids nie wiederverwendet (`AUTOINCREMENT`, Migration 0007), slugs gelöschter Werke in `work_retired`
  dauerhaft reserviert, gesät wird nur ohne `store_meta.seeded`.
- MCP schreibt (`propose_revision`, `add_post`, `refs`, `outline_*`) und `highlight.cite` zitiert nur mit `key`; `node`
  → -32602.
- Gliederung über MCP (Wunsch 2026-11-06, `src/mcp/outline.rs`, Store `outline_add_many`, `src/requests.rs`,
  Migration 0012): `outline_add` legt sofort an (Autor Persona/Client, ein Commit); Umbenennen, Verschieben,
  Umstufen, Nummer an/aus, Löschen nur als Antrag, den der Mensch bestätigt. Werkzeugnamen-Prüfungen nehmen
  `outline_*` aus.
- Assistenten und MCP-Aufrufer schreiben nur Vorschläge (`propose_revision`, `Store::add_proposal`) und Beiträge;
  eigene Beiträge dürfen sie verschieben, löschen und in Einfälle umwandeln (Runde 8, je ein Commit
  `Baruch-Action: post-*`), fremde nie.
- Jede Übernahme, Umschaltung, Speicherung, Änderung und Löschung ist ein Commit im Spiegel (Trailer
  `Baruch-Action: switched|updated|deleted`); Umschalten, Ändern und Löschen auch als leerer Protokoll-Commit
  (`allow_empty`), Ändern mit vorher/nachher, Löschen mit dem gelöschten Text.
- Bilder haben Fassungen wie Abschnitte (genau eine `current`, umschaltbar, Inhalt per Trigger unveränderlich –
  Ersetzen ist eine neue Fassung; Ändern nur Bildunterschrift/Alternativtext/Notiz; Löschen nur Mensch mit
  `confirm`; Spiegel-Commit `Baruch-Action: asset-*`). Inhalt immer TEXT (Base64 bzw. SVG), nie BLOB. Über MCP
  lesen (`list_assets`, `read_asset`) und sofort anlegen (`asset_add`, Herkunft im Spiegel); Ersetzen, Umschlag
  setzen/entfernen und Löschen nur als Antrag (`asset_replace`, `asset_set_cover`, `asset_delete`; Store
  `src/asset_requests.rs`, Migration 0018, Server `src/asset_requests.rs`), den der Mensch bestätigt (Runde 11).
- Einfälle ändern keinen Buchtext: volles CRUD auch für Assistenten und MCP (IDEA_CRUD), jede Aktion ein
  Commit mit `Baruch-Action: idea-*` und `Baruch-Origin`; Einfälle ohne Werk im Git-Ordner
  `<data>/einfaelle/`. Persona-Farben sind global und stehen nicht im Spiegel.
- Titel von Gliederungspunkten: nicht leer, ohne Zeilenumbruch, höchstens 200 Zeichen (`check_title`) auf
  jedem Weg (Gliederung, Speichern, Umschalten, Vorschläge); geprüft wird nur ein geänderter Titel (Runde 8:
  `check_chunk_titles(chunks, old_title)`); Import/Export prüfen nicht (byte-gleich).
- Ein Fehler im Git-Spiegel rollt nichts zurück; die RPC-Antwort trägt dann `mirror` statt `commit`.

## Austauschformat zwischen Instanzen (Runde 26)
- Format, Elemente, Kennungen, Einlesen und Grenzen: `docs/Austauschformat.md`; XSD `docs/schema/baruch-werk-1-u3.xsd`
  (Namensraum `urn:baruch:werk:1`, Unterversion `version="3"` seit Runde 27 mit `<umschlag>` (Blöcke, Grabsteinart
  `umschlag-block`), `<verlag-anschrift>`, `<druckerei-anschrift>` und `<satzbild>`; Unterversion 2 seit Runde 25 mit
  `<karte>`). `docs/schema/baruch-werk-1.xsd` (Unterversion 1) und `baruch-werk-1-u2.xsd` bleiben eingefroren – nie
  ändern (ein Test hält ihre SHA-256 fest); eine neue Unterversion bekommt ein eigenes XSD.
- Crate `baruch-exchange` (rein, ohne DB): `model.rs` (Typen `Exchange*`, eingefroren, nur Integrator), `write.rs`
  (eigener Schreiber, deterministisches ZIP), `read.rs` mit `read/{dom,map,archive,check}.rs` (sicherer Leser:
  keine DTD, Grenzen aus `limits.rs`, Pfad- und Prüfsummenprüfung, `validate`), `sample.rs` (Beispielwerk,
  frei erfunden). Tests `tests/{write,xsd,read}.rs`, Daten `tests/data/import/*.baruch.xml`.
- Store: `uuids.rs` (Instanz, `new_uuid`, Abfragen über UUID), `exchange_dump.rs` (Werk → `ExchangeWork`),
  `exchange_apply.rs` (Vorschau und Übernehmen durch denselben Code, Vorschau rollt zurück; `derive_uuid`).
  Migrationen 0041 (Spalten `uuid`, Fülltrigger, Grabsteine `uuid_tombstone`, `store_meta.instance_uuid`) und 0042
  (Grabsteine bei der Kaskade); 0043 und 0045 bauen `uuid_tombstone` für neue Grabsteinarten um (alle Trigger wörtlich
  neu, Vorbild `0045_cover_block.sql`). **Nur `exchange_apply` setzt eine uuid**, und nur auf eben angelegten Zeilen
  (Wächtertest `uuids.rs::no_code_path_but_exchange_apply_writes_the_uuid_column`). Wer später eine der zehn
  Tabellen umbaut, nimmt Spalte, Index und beide Trigger mit. Tests `tests/{migration_0041,uuids,exchange_dump,
  exchange_apply}.rs`.
- Server: `src/exchange/{mod,export,import}.rs`, `src/instance.rs`, `src/mcp/exchange.rs`; Routen
  `GET /werke/{id}/austausch[.baruch|.xml]`, `GET /austausch/einlesen`, `POST /austausch/vorschau` (multipart, Feld
  `datei`, Eingang `<data>/austausch-eingang/<sha256>`, nach 24 h weggeräumt), `POST /austausch/uebernehmen`
  (urlencoded `sha256`, `mode`, `title`), `POST /einstellungen/daten/instanz`; RPC `work.exchange_*`,
  `instance.*`; MCP nur `exchange_export`; CLI `export ID --baruch DATEI`, `import DATEI.baruch[.xml]
  [--abgleichen] [--vorschau] [--title]`. Tests `tests/round26_{ids,export,import,roundtrip}.rs`.
- XSD-Prüfung in Tests: `xmllint --noout --nonet --schema docs/schema/baruch-werk-1-u3.xsd DATEI`; fehlt `xmllint`, wird
  ehrlich mit Meldung übersprungen (eine Rust-Prüfung der Pflichtelemente läuft trotzdem).
- Ende-zu-Ende von Hand (Integrator R26): zwei Wegwerf-Server A/B nach dem Umweg oben, Werk per RPC aufbauen
  (Vorschlag über MCP `propose_revision` mit `model`), Ausgabe per `GET`, Einlesen per RPC und Formular, Kommandozeile
  auf einem dritten Ordner ohne laufenden Server; Zeilenzahlen per `sqlite3`-Lesezugriff
  (`file:…?mode=ro`) vor und nach manipulierten Dateien vergleichen. Vergleich zweier Ausgaben: alles außer
  `<ausgabe …/>` muss byte-gleich sein.

## Umschlag-Editor (Runde 27)
- Typen eingefroren in `baruch_typst::cover_layout` (`CoverBlock`, 15 `CoverBlockKind`, `CoverGeometry` mit
  `face_origin`/`face_size`, Grenzen = CHECKs von Migration 0045); Koordinaten in mm ab der Schnittkante oben links der
  eigenen Fläche, im Buch-PDF ist die Umschlag-Vorderseite eine Seite W × H mit Ursprung an der Schnittkante.
  `default_blocks` ahmt das alte Bild je Titelbereich nach, `effective_blocks` = gespeichert oder Vorgabe je Art.
- Ohne Zeile in `cover_block` und ohne Titelei-Funktionen der Vorlage bleiben `frame_parts`/`wrap_source` byte-gleich
  (Goldstände `baruch-typst/tests/data/cover_r26/`, SHA-256 in `round27_satz.rs`); wer den Satz ohne Anordnung ändert,
  muss die Goldstände bewusst neu schreiben (ignorierter Schreiber in `baruch-typst/tests/cover_layout.rs`).
- Store `cover_blocks.rs` (`set_cover_blocks` Upsert je Art ohne Grabstein, `reset_cover_blocks` mit Grabstein und
  Titelbereich in einer Transaktion, `cover_layout_fingerprint` für `expected_layout`); Server `cover_layout.rs` (RPC
  `cover.layout|layout_set|layout_reset|fonts`, `layout_view` = gespeichert + Vorschau → Maße, Höhen, Druckprüfung),
  `cover_print.rs` (Codes `CODES`, Kontrast im Rasterbild, SVG-Umschläge nicht gemessen, `imprint_checks`),
  `cover.rs` in zwei Bereichen (Satz: `book_info` … `cover_pdf`; Seiten/Formulare/RPC: der Rest), `mcp/cover.rs`
  (nur lesen bzw. Antrag über `asset_request` op `cover_layout`). Widget `web/src/cover_editor.ts`, Stil
  `static/cover.css`.
- Formulare ohne JavaScript: Vorschau `GET /werke/{id}/umschlag[.svg]?b.<kind>.<feld>=…` (speichert nichts),
  `POST /werke/umschlag/anordnung` (`action=save|quick|reset`), `POST /werke/umschlag/impressum`.
- Titelei aus der Werkvorlage: `#let schmutztitel|haupttitel|impressum|widmung(buch)` liefern Inhalt; Baruch setzt
  jede auf eine eigene Seite und schreibt dann einmal `#let baruch_buch = (…)` (nur String-Literale). Neue Bücher
  bekommen `imprint_template()`; Store-Tests zur Buchvorlage vergleichen mit `imprint_template()`.
- Lage messen: `pdftotext -bbox` (Wort, `xMin`/`yMin` in pt) in Umschlag- und Buch-PDF, Ursprung der Fläche aus
  `CoverGeometry::face_origin` abziehen (`baruch-render/tests/cover_layout.rs`, `tests/round27_abnahme.rs`); fehlt
  `pdftotext`, ehrlich „ÜBERSPRUNGEN“. Schriften: `baruch_render::font_families()` (blockierend, `spawn_blocking`).
- Tests: `tests/round27_{daten,satz,widget,abnahme}.rs`, Store `tests/{cover_blocks,migration_0045}.rs`,
  `baruch-typst/tests/cover_layout.rs`, `baruch-render/tests/cover_layout.rs`; PNG mit dunklem oberem Drittel aus
  `round27_satz::dark_top_png`.

## Befunde und Chat (Runde 28)
- Daten: Migration 0046 (`finding`, `finding_status`, `post_version`, Spalten `post.edited`/`chat_thread_post.edited`,
  Grabstein-Arten `finding`/`finding_status`/`post_version`, Kaskaden-Trigger wie 0042/0045). Migrationstests, die das
  Schema nach `Store::open` vergleichen (0016–0018, 0040, 0041), nehmen `, edited TEXT` nur bei `post` und
  `chat_thread_post` heraus (`revision` hat schon eine Spalte `edited`) und die 0046-Objekte in die Liste der neuen.
- Store `src/findings.rs` (Typen, `finding_insert`/`finding_get`/`finding_list`/`finding_history`/
  `finding_open_counts`/`findings_adopted_in`) und `src/post_edit.rs` (`post_versions`, `thread_post_versions`,
  `post_edited_map`); Tests mit echten Befunden legen sie über `Store::finding_insert` an.
- Server `src/findings/{mod,parse,rpc,page,view}.rs`: Zerlegen nur über `findings::ingest` (nimmt nie die
  Schreibsperre, der Aufrufer darf sie halten; kein Commit), aufgerufen in `assist::save` (nicht bei Propose) und
  `threads.rs::answer`; Fehler dort nur auf stderr, der Beitrag bleibt. MCP `mcp/findings.rs`, `mcp/posts.rs`;
  Spiegel `mirror::describe_finding`/`record_finding`, Beitrag bearbeiten `PostAction::Edit`.
- Tests: `tests/round28_{befunde,uebersicht,chat,abnahme}.rs`, Store `tests/{findings,post_edit,migration_0046}.rs`.
- RFC im Test (Integrator R28): Die Reihenfolge der Personas nicht raten, sondern vor dem Skript über
  `assist::chat::rfc_personas(&state)` holen und die Antworten danach in den `ScriptedModel` schieben (`push`); jede
  Antwort mit genau einer Marke ergibt genau einen Befund, die ids folgen dann der Aufrufreihenfolge (`max(id)`
  vorher nachsehen), und das Skript des Lektors kann `[B<id>]` zitieren. Vorbild `tests/round28_abnahme.rs`.
- Statuswege der Befunde ohne JavaScript: `POST /werke/befunde/act` (`method=finding.set_status|finding.decide`,
  `back` als relative Adresse) → 303; Fehler → 200 mit `role="alert"` und Entwurf. Neue Seiten gehören in
  `tests/round9_menu.rs` (`all_pages`); `menu.badges` liefert seit R28 `open_findings`.

## Runde 30 (Suchen/Ersetzen, Querverweise, Chat-Lösungen)
- Schnittstelle R30-S1: Migration 0047 (`docs/Architektur.md`, „Schnittstelle Runde 30“), Migrationstest
  `crates/baruch-store/tests/migration_0047.rs`; Migrationstests, die das Schema nach `Store::open` vergleichen, nehmen
  die angehängten 0047-Spalten und `replace_batch`/`replace_batch_work`/`revision_variant_group` heraus. Neue
  `Order`-Felder in Tests: `links: Default::default(), basis: None`; `ChatOrder` bekommt `basis: Default::default()`.

### Runde 30a
- Sprungmarken `<kN>` stehen in Export, Export-Karte, Spiegel und Satz, nie in SQLite.
  - Tests, die einen Export, eine Spiegeldatei oder `export --out` mit der Quelle vergleichen, vergleichen
    `baruch_typst::xref::strip_key_labels(&…)`. Nach einem Re-Import haben die Werke neue Schlüssel, darum beide
    Seiten strippen.
  - Neue Satz-Goldstände (`round27_satz.rs`) misst der Testlauf selbst (Ausgabe „gemessen:“).
- Satzquelle: Teile haben im Satz die Ankerzeile vor der Teil-Zeile (`part_anchors_for_typesetting`), und kaputte
  Verweise sind dort nur Text. Wer Bereiche von `prepare` mit `export_map` vergleicht, wendet dieselbe Funktion an
  (`round14_parts.rs`).
- Suche: `baruch_server::search::find_within(&state, work, &query, Duration::ZERO)` prüft die Zeitgrenze ohne
  prozessweiten Zustand. Ein katastrophaler Ausdruck bleibt mit dem Crate `regex` linear.
- Store-Ersetzen: Ein Abschnitt, dessen Überschrift nicht passt, rollt nur seinen `SAVEPOINT` zurück
  (`round30_replace.rs`). `replace_batch` kommt nie ins Austauschformat.
- Node-Proben: `search_keys.js` und `jump.js` haben keine bzw. nur reine Importe. Für `xref.js` schreibt der Test alle
  `SCRIPTS` samt `package.json` (`"type": "module"`) in einen Ordner (`round30_verweise.rs`). Fehlt `node`, melden die
  Tests „ÜBERSPRUNGEN“.

### Runde 30b
- Lösungspflicht in Tests: Jede Vorgabe verlangt unter „Kritisch: …“ eine Lösung; fehlt sie, fordert `assist::run`
  **einmal** nach (eigener Retry-Grund `prompts::SOLUTION_RETRY_REASON`) – das verbraucht einen weiteren Schritt des
  `ScriptedModel` bzw. eine Antwort des `MockEliezer` (leer → „OK“). Skriptierte Antworten mit „Kritisch:“ bekommen
  darum eine Lösungszeile (`\n- Schritt: prüfen.` bzw. `- statt: „…“`/`- besser: „…“`), sonst verschieben sich alle
  folgenden Schritte; ein Test, der nur Marken prüft, darf den Vermerk `prompts::NO_SOLUTION_NOTE` erwarten (dann
  scheitert die Nachforderung am Skriptende). Abgeschnittene Antworten (`Step::cut`) fordern nichts nach.
- Prompts enthalten jetzt die Gliederung mit `[key:N]` (wenn Platz ist) und `JUMP_RULE`; Zusicherungen auf
  Gliederungszeilen prüfen `"] 1.1 Titel"` statt `"- 1.1 Titel"`. Fremde Befunde sieht nur der Lektor unter
  `FINDINGS_TITLE`, alle anderen unter `KNOWN_TITLE`.
- Lösungen, Varianten und RFC-Sammel-Vorschlag im Test: `tests/round30_loesungen.rs`, `tests/round30_varianten.rs`
  (Varianten-Antworten in der Form `Zeile: …\nBefunde: [B1]\nText:\n= …`; Teilvorschlag mit kleiner Kontextlänge
  über `persona.update {context_length}` und `[[Absatz n]]`-Antworten). Store: `tests/round30_variants.rs`
  (Austausch u5 mit `baruch_exchange::sample`, Abgleich ändert bekannte Zeilen nie). Austausch:
  `baruch-exchange/tests/{read,xsd,write}.rs` (u5-Rundreise, u4-Dateien, eingefrorenes u4-XSD per SHA-256).
- `remap_keys` (R30a) ist seit der Integration umgesetzt; der Store-Test zum Umsetzen der Querverweise in
  `exchange_apply` prüft das Umschlüsseln jetzt wirklich (die Ausweichmeldung „übersprungen“ greift nur, wenn
  `remap_keys` nichts ändert).

### Runde 30c
- Chat-Bedienung: Store `post_meta.rs` (`thread_add_post_meta`), `threads.rs` (`thread_post_delete`); Server
  `assist/chat.rs` (`addressed`, `line_addresses`, `relay_text`, `basis_param`, `MAX_RELAY_ANSWERS`), `posts.rs`
  (`post.repeat`, `post.since`, `propose_done`), `threads.rs` (`thread.post_delete`, `thread.repeat`, `thread.since`,
  `delete_post_as`), `chat_render.rs` (Markdown-Teilset, Links, Segmente), `chat_live.rs` (schreibt …/Fehler),
  Vorlage `fragments/post_item.html` (`post_item`, `post_entry`, `thread_entry`).
- Tests: `tests/round30_bedienung.rs`, `tests/round30_live.rs`, Store `tests/round30_post_meta.rs`. Relay im Test: die
  Antwort der ersten Persona mit Zeilen „@handle: …“ ins Skript, danach je Relay-Antwort ein Schritt; Reihenfolge der
  Aufrufe über `ScriptedModel::calls()` (`persona`) prüfen. „schreibt …“ im Test mit `Step::Wait` und
  `model.started().await`, dann `post.since` abfragen, danach `model.release()`. Die Live-Tabellen sind prozessweit je
  (Werk-id, Schlüssel): Integrationstests prüfen nur eigene Einträge (`find`, Zeit `since` merken), Abwesenheit nur in
  Unit-Tests mit eigener Werk-id (`chat_live::tests`).
- Reine Funktionen der Widgets (chat.ts `typingLine`/`freshFailures`/`deleteQuestion`, mention.ts
  `addressAt`/`filterAddresses`/`applyAddress`) unter Node wie `tests/quote_find.rs`, aber mit allen `SCRIPTS` als `.mjs`
  in einem Ordner (Importe auf `.mjs` umgeschrieben), weil chat.js `rpc.js` und `widgets.js` importiert.
- Vorlagen-Makros sehen den Seitenkontext nicht: alles über Argumente (`at = dict(...)` in der Vorlage). Einzelne
  Beiträge für `post.since` rendert `Environment::render_str` mit `{% from … import post_entry %}`.

### Runde 30 (Integration)
- Abnahme `tests/round30_abnahme.rs`: ein Küstenbuch (3 Teile, 12 Kapitel, 48 Abschnitte, 6 Personas) für alle drei
  Teilrunden. Zusammenspiel, das erst nach dem Zusammenführen geht: Befund-Blöcke im Chat brauchen das echte
  `finding_spans` – ein Listenpunkt einer Persona-Antwort ist dann ein eigener Block (`post-finding` mit `finding_block`),
  nicht mehr Teil einer gemeinsamen `<ul>`; Sprung-Adressen aus `check_marker` tragen bei wörtlichem Zitat `&at=<Byte>`.
- Skriptierte Antworten mit „Kritisch:“ in Chat-Tests brauchen eine Lösungszeile (siehe Runde 30b), sonst verschiebt
  die Nachforderung alle folgenden Schritte – so in `round30_bedienung.rs` nachgezogen.
- `finding_block(card, body)`: Der Chat reicht sein Segment als `body` herein, damit der Befundtext nicht doppelt
  erscheint (Test: Text im Block genau einmal).

## Satz-Einstellungen (Runde 29)
- Schnittstelle R29-S1: `baruch_typst::typeset::model` ist eingefroren (Änderung nur über Integrator/PM). Der einzige
  Ort der Satz-Regeln ist der Haken in `baruch_store::snapshot::head`; ein Werk ohne Einstellungen exportiert
  byte-gleich. Testdateien `crates/baruch-server/tests/round29_*.rs` sind in `tests/main.rs` schon eingetragen.

### satz-daten
- Store `src/typeset.rs` (Tabelle `work_typeset`, Migration 0048), Tests `crates/baruch-store/tests/{typeset,
  migration_0048}.rs`. Migrationstests, die das Schema nach `Store::open` vergleichen (0040, 0041), nehmen
  `work_typeset` samt Index und Triggern heraus (0016–0018 vergleichen nur die eigenen Tabellen).
- Golden des Exports ohne Einstellungen: `crates/baruch-store/tests/data/satz_golden_{export,main}.typ` aus dem Stand
  R29-S1 (Quelle `satz_quelle.typ`, Schema „decimal“). Neu schreiben nur bewusst mit dem ignorierten Test
  `write_goldens_from_the_current_state` – und nur, wenn sich der Export ohne Einstellungen gewollt ändert.
- Tests „mit Einstellungen“ rechnen den erwarteten Export als alter Export + `baruch_typst::typeset::rules(&s)` an der
  Stelle, wo der erste Abschnitt beginnt (`export_map().sections[0].start`); so bleiben sie gültig, was immer
  satz-typst in die Regeln schreibt (keine Ausweichzweige).
- Austausch: `baruch_exchange::typeset` (Wörter, `TYPESET_ATTRIBUTES`, `full_sample` mit allen sechzehn Feldern), das
  Beispielwerk trägt Satz-Einstellungen (`sample_uuid(400)`) und einen Grabstein `satz` (`sample_uuid(401)`). Tests,
  die ältere Unterversionen aus dem Beispiel bauen, nehmen beides über `without_u6` heraus.
- Server: `crate::typeset_settings` (RPC, `load`, `ADOPT_NOTE`, `NO_RULE`), `mirror::describe_typeset`; Test
  `tests/round29_daten.rs` (RPC, Commits samt Trailer und Rumpf, MCP, Austausch über drei Instanzen).

### satz-typst
- Regeln: `baruch_typst::typeset::rules` (`rules.rs`), Vorlage `template.rs` (`template_conflicts`,
  `detect_chapter_break`, `remove_rule`), Vorschau `preview.rs`, EPUB `epub.rs`. Tests
  `baruch-typst/tests/typeset_{rules,template,epub}.rs`; die Wirkung jeder Einstellung misst
  `baruch-render/tests/typeset_measure.rs` mit `baruch_render::page_texts` (Quelle wie im Server: Vorlage, `rules`,
  `part_page_rule_with`, Text).
- Messen: Zeilen = Textstücke mit gleicher Grundlinie; Fußnotenzeichen sind eigene Stücke aus einem Zeichen; ein
  Trennstrich ist ein Stück „\u{ad}“; rechte Kanten nur aus Zeilen, die auf einen Buchstaben enden (Satzzeichen hängen
  in den Rand); Leerseiten haben keine Textstücke und keine `page_lines`.
- Neue Regeln, die an der Überschrift hängen, stehen hinter der Kapitelregel (Typst wendet die zuletzt definierte
  Show-Regel zuerst an): so liegt die Marke `<baruch-satz-end>` vor dem Umbruch. Nichts, was für `counter(page).at(…)`
  gelten soll, gehört in die Show-Regel einer Überschrift (es läge hinter ihrer Startmarke) – dafür der Seitenhintergrund.
- Server: `typeset::prepare` reicht nur `part_page` weiter (`state.store.work_typeset`), alles andere steht schon in
  `export_map`. Der Goldstand „Werk ohne Einstellungen“ in `tests/round29_typst.rs` ist mit `typeset.rs`/`cover.rs` aus S1
  gemessen (Ausgabe „gemessen:“).

### satz-oberflaeche
- Block „Satz“: `src/settings/work_typeset.rs` + `templates/settings/werk_satz.html` (in `settings::templates()`
  registriert). Makros der Vorlage bekommen Meldungen und Hinweise als Argumente (`notes(id, errors[id], hints[id])`),
  sie sehen den Seitenkontext nicht. Neue Formularfelder immer in `settings_from_form`, `form_from_settings`,
  `formToSettings` (TS) und im Rundreise-Test zugleich nachziehen.
- Tests `tests/round29_oberflaeche.rs`: Ausschnitt des Blocks mit `satz_block` (bis `id="abgleich"`); Vorschau-Tests
  holen die Bilder sofort nach dem Setzen (prozessweiter Zwischenspeicher mit acht Ständen) und nutzen je Test einen
  eigenen Werktitel bzw. eigene Werte (eigener Stand). Erfolgswege Speichern/Zurücksetzen/Übernehmen prüft die Abnahme
  (`round29_abnahme.rs`), nicht diese Datei.
- Reine Funktionen der Widgets (`formToSettings`, `debounce`, `Newest`, `xref.ts` `initialPage`) unter Node wie in
  `round30_live.rs` (alle `SCRIPTS` als `.mjs`); `formToSettings` gegen `settings_from_form` über
  `serde_json::from_value::<TypesetSettings>` vergleichen (JS schreibt `20` statt `20.0`).

### Runde 29 (Integration)
- Abnahme `tests/round29_abnahme.rs`: Wirkung der Einstellungen im Server messen mit `typeset::typeset` (Kapitelseiten
  über `page_of`, Seitenmaße über `page_sizes_pt`) und `baruch_render::page_texts(&typeset.prepared.source)` in
  `spawn_blocking` – das ist genau die Quelle des PDFs (`PdfOut.stand == prepared.stand`); das PDF selbst mit `pdfinfo`
  (Seitenmaße) und `pdftotext` („S. n“). Seitenzahlen und Kolumnentitel immer auf **allen** Textseiten prüfen, nicht nur
  auf einer: Teil- und Leerseiten haben höchstens drei Textstücke (`page.len() <= 3`).
- Kolumnentitel/Seitenzahl ohne Kapitelregel: Die Marke `<baruch-satz-end>` folgt dann ihrer Überschrift in der Abfrage
  (gleiche Lage) und darf nicht als „Leerseite bis zur nächsten Überschrift“ zählen (Befund der Integration, Test
  `without_a_chapter_rule_every_text_page_keeps_number_and_head` in `baruch-render`).
- Fußnotenzeichen messen: einzelne Textstücke aus einem Zeichen (die Größe des Hochstellens steht nicht in `size_pt`).

## Gedankenkarte (Runde 25)
- **Seit Runde 34 Mindmap** (siehe „Runde 34 – Mindmap“ am Ende): `bild/{layout,svg}.rs` und `web/src/idea_map.ts`
  gibt es nicht mehr; Daten, Verbindungen, Nähe und Gruppen gelten weiter.
- Daten: Migration 0043 (`idea_link`, `idea_map_pos`, Grabsteinarten `idea_link`/`idea_map_pos`), Store
  `idea_links.rs`; Server `src/idea_map/{mod,links,near}.rs` (RPC `idea.map` … `idea.map_pos_reset`, `build` liefert
  die ganze Karte), `src/idea_map/bild/{mod,layout,svg,export}.rs` (Seite, Formular `/werke/{id}/karte/act`,
  Ausgaben), `src/mcp/idea_map.rs`, `rag/neighbors.rs` (liest nur gespeicherte Vektoren), Widget
  `web/src/idea_map.ts`. Verbindungen: jede Änderung ein Spiegel-Commit; Lagen ohne Commit; Nähe nie gespeichert.
- DOM-Vertrag der Karte (Klassen `im-*`, `data-node`, `data-x/y/r`) lesen Widget und Tests; wer ihn ändert, ändert
  `svg.rs`, `idea_map.ts` und die Tests `round25_{bild,widget}.rs` zusammen.
- `build` gibt den angewandten Filter mit eingesetzten Vorgaben zurück; Links der Seite baut `bild::filter_query`
  ohne Vorgabewerte. Tests: `tests/round25_{daten,bild,widget,abnahme}.rs`, Store `tests/{idea_links,
  migration_0043,exchange_map}.rs`.

## Editor und Menüband (Runde 18)
- Menüband: `templates/fragments/edit_toolbar.html` (Editor: `data-edit-toolbar hidden`; Satz-Seite: ohne `hidden`, ohne
  JavaScript bedienbar), Formate aus `src/ribbon.rs` (`formats(template)`, RPC `ribbon.formats`), Widget
  `web/src/ribbon.ts`, Stil `static/ribbon.css`. Befehle ans Satzbild nur über `web/src/ws_command.ts`
  (`baruch:ws-command`, `sendWsCommand`; Cursorstelle `baruch:ws-caret`) – eingefroren; `wysiwyg.ts` führt sie aus,
  niemand sonst ändert `wysiwyg.ts` dafür. Knöpfe tragen `data-ribbon-cmd`, „Bild“ `data-ribbon-image` (öffnet
  `details[data-image-insert]` in `fragments/editor.html`).
- Satz abbrechen: `baruch_render::Cancel` + `compile_cancellable` (`compile`/`compile_pdf` unverändert); Server
  `typeset::cancel(work)`/`cancel_in`, RPC `preview.cancel`, Formular `POST /werke/{id}/satz/abbrechen`. Zähler für
  Tests: `typeset::runs_of(stand)`, `pdf::pdf_runs()`. GET von Satz, Seiten-SVG und PDF mit
  `guard::cross_site_get` → 403 ohne Lauf. `typeset::warm_fonts()` läuft beim Start von `serve` (`main.rs`). Typst
  lässt sich nicht hart unterbrechen (Messung in `baruch-render/tests/cancel.rs`); ein Abbruch über die Welt
  (keine Dateien mehr liefern) bricht comemo („found differing return values“) – nicht wieder versuchen.
- Satzbild-Einstellungen je Werk: Store `src/sheet.rs` (Migration 0044 `work_sheet`, kein Spiegel, kein uuid; seit
  Runde 27 im Austauschformat als `<satzbild>`, Unterversion 3), `baruch_wysiwyg::sheet_style_over`, Server `src/sheet.rs` (`sheet_style_for`/`sheet_css_for`, RPC
  `work.sheet`/`work.sheet_set`), Formular `POST /einstellungen/werk/satzbild` (`settings/work.rs`), Stil im HTML als
  `<style id="ws-style" data-work="W">` (das Widget übernimmt ihn ohne RPC). Satzbild-Einstellungen ändern Typst,
  Export und PDF nie.
- Migrationstests 0040/0041 vergleichen Schemaobjekte nach `Store::open`: eine neue Tabelle (wie `work_sheet`) dort in
  `without_0041`/`new_objects` und bei Verweisen auf `work` in `references_after` ausnehmen.
- Verhaltenstest ohne Browser (`tests/round18_satzbild.rs`, `HARNESS`): die übersetzten Widgets laufen in Node 20 auf
  einem kleinen DOM-Nachbau mit nachgebautem `fetch`; fehlt `node`, meldet der Test „ÜBERSPRUNGEN“. Braucht ein
  Widget beim Einhängen neue DOM-Teile, den Nachbau ergänzen, nicht die Zusicherungen.
- Bild an der Cursorstelle: `POST /werke/bilder/einfuegen` mit `place` = `end|start|block:<n>|offset:<n>` und optional
  `body`; Pfad und Fassung in einer Transaktion (Store `assets.rs`).
- Abnahme: `tests/round18_abnahme.rs` (ein erfundenes Werk mit allen Konstrukten; jeder Block, den das Widget neu
  schickt, bleibt bei kanonischem Text byte-gleich).

## Austausch zwischen Rechnern
- Über GitHub: Commits auf Zweigen, Rückmeldungen als Issue-Kommentare.
- Push nur nach „ja“ des Nutzers; keine Tokens in Commits, Issues oder Dokumenten.

## Tests (Vorbild: David, `~/Dokumente/Projekte/Psalm/server/tests`)
- **Jede Crate hat eigene Tests**, und jede Crate ist Bibliothek + dünnes Programm: Logik in `src/lib.rs`,
  `main.rs` nur Start und Argumente – so lassen sich Server, Speicher und Spiegel aus `tests/` heraus prüfen.
- **Integrationstests je Fachgebiet** unter `crates/<crate>/tests/<gebiet>.rs` (z. B. `revisions.rs`,
  `rpc.rs`, `pages.rs`, `library_search.rs`, `git_mirror.rs`, `cookies.rs`), gemeinsame Helfer in
  `tests/common/mod.rs` (`mod common;`), z. B. `open() -> (TempDir, Store)` mit echter SQLite-Datei im
  Temp-Ordner, Beispielwerk, Hilfen für RPC-Aufrufe. Testdaten (Typst-Kapitel, Repos) in `tests/data/`.
- **`baruch-server`: eine Test-Binärdatei** (seit Runde 13): Jede Test-Binärdatei linkt den Server samt LanceDB
  (≈ 500 MB); darum `autotests = false` und `[[test]] name = "server"` mit `tests/main.rs`, jede Datei
  `tests/<gebiet>.rs` ist dort ein Modul. **Neue Testdatei:** anlegen und in `tests/main.rs` `mod <gebiet>;`
  eintragen (sonst schlägt `every_test_file_is_bundled` an), Hilfen mit `use crate::common;` (bzw.
  `use crate::pgvector_support;`) statt `mod common;`. Keine neuen eigenständigen Testbinärdateien im Server.
  Alle Module laufen in **einem** Prozess: prozessweite Zustände (Markierung „PDF entsteht“, Warteschlange der
  Modelle, MCP-Sitzungen, Satz-Semaphore, Index-Laufzeiten) teilen sich alle Server-Tests. Darum: PDFs nur unter
  `common::PDF_SERIAL`; Tests, die die Modell-Warteschlange füllen oder ihren Zustand prüfen, beginnen mit
  `let _turn = common::model_queue(module_path!()).await;` (nur Tests eines Moduls zugleich; u. a.
  `round10.rs`, `round10_models.rs`, `round15_anbieter.rs`, `round15.rs`); nie Umgebungsvariablen setzen oder das Arbeitsverzeichnis wechseln;
  Zusicherungen auf prozessweite Listen (z. B. gesehene MCP-Clients) mit `find`, nie auf die ganze Liste.
  **Index nach jeder Änderung abwarten:** Jede Änderung an Einfällen oder Text reiht den Index neu ein
  (`rag::notify`); auch wenn sich nur Metadaten ändern (z. B. der Bezug bei `idea.move`), ersetzt die
  Hintergrund-Aufgabe die Zeile in LanceDB – löschen, dann neu schreiben, nicht in einem Schritt. Wer danach etwas
  prüft, das gespeicherte Vektoren liest (Nähe, „passt zu Ast“, Bedeutungssuche), ruft vorher
  `rag::wait_idle(&server.state).await`; sonst fehlt der Vektor unter Last gelegentlich (Runde 38,
  `round25_abnahme.rs`: 5 von 10 Läufen rot bei vier Test-Threads).
- **Echte Bausteine statt Attrappen:** echte SQLite-Datei, echtes Git-Repo im Temp-Ordner, Router über
  `tower::ServiceExt::oneshot`; nachgebildet werden nur fremde Dienste (MultiGPT, Modelle).
- **Modelle in Tests** (Runde 10): nie der echte Eliezer, nie Netz außer 127.0.0.1:0. Personas allein:
  `AppState::open_with(dir, Arc::new(MemorySecrets::default()), Arc::new(model.clone()))` mit
  `assist::mock::ScriptedModel` (`tests/round10_personas.rs`). Ende-zu-Ende mit HTTP: `Arc::new(EliezerPort)` und
  `baruch_llm::mock::MockEliezer::start()`, Adresse per `connection.set {kind: "eliezer", config: {url:
  mock.url()}}` (mit Schlüssel danach `connection.set_secret`, Bindung); Antworten mit `mock.push(Reply::text(&[…]))`,
  prüfen mit `chat_bodies()`, `forbidden_paths()`, `context_fields()`, `max_active()`, `cancelled()`
  (`tests/round10.rs`). `AppState::open` hat `NoModel`. Jede neue Testdatei vorher prüfen:
  `grep -n 1234 crates/*/tests/<datei>.rs`. Die Warteschlange in `models` ist prozessweit: Tests derselben Datei
  teilen sie (keine Zusicherung auf `waiting == 0`; im Bündel siehe `common::model_queue` oben). Die Vorgabe des Lektors ist fest `@lektor`; Tests, in denen
  der Lektor dem Kontext folgen soll, setzen vorher `persona.set_lector {mode: "auto"}`.
  Seit Runde 15: Verbindung je Persona über `model.connection_set {name, kind: "eliezer"|"openai"|"ollama", url,
  external}` und `persona.backend_set {persona, connection, external_allowed?}`; der Nachbau spielt die Art mit
  `MockEliezer::set_flavor(Flavor::Ollama | Flavor::Openai)` und meldet Kontextlängen mit `set_context_length`.
  Kontext des Lektors im Test: `ScriptedModel::with_reported_context(n)` bzw. `persona.update {context_length}`;
  Stückeln/Komprimieren mit eigenem `ModelPort`, der Aufrufe mit „Zusammenfassung“ in `label` kurz beantwortet
  (`tests/round15_lektor.rs` `Responder`, `tests/round15.rs`). Faktenblock ohne SWORD-Module über
  `BibleInput::Given`, mit Modulen über `baruch_sword::fixture` (`write`, `write_lexicon`) und `bible.set_dirs`,
  `bible.set_work`, `bible.set_work_sources`. Der Lektor ruft das Modell nie parallel (`max_active() == 1`).
- **Bedeutungssuche in Tests** (Runde 12): nie Eliezer, nie MultiGPT, nie das pgvector des Nutzers. Ohne
  `rag::set_embedders`/`rag::catch_up` gibt es keinen Einbetter (Suche meldet `NO_EMBEDDER`, nie Netz). Einbetter:
  `rag::set_embedders(&state, rag::eliezer_embedders(&state))` nach `connection.set` auf `MockEliezer` (512
  Dimensionen, `set_embedding_dimension`, Gleichzeitigkeit mit `embed_max_active()`), oder ein eigener
  `EmbedderFactory` mit festem Vektorraum (`tests/round12.rs` `Concepts`); danach `rag::catch_up` und
  `rag::wait_idle`. MultiGPT: `connections::multigpt::mock::MockMultigpt`. pgvector: `use crate::pgvector_support;` →
  `start("<aufgabe>")` (eigener podman-Container mit lokalem Abbild `docker.io/pgvector/pgvector:pg17`/`:pg16`,
  Port nur auf 127.0.0.1, Name `baruch-r12-<aufgabe>-<pid>-<n>`; gleichzeitig laufende Tests teilen sich einen
  Container, jeder bekommt eine eigene Datenbank `rag_<n>` (`Pg::database`, `config()`); mit dem letzten `Pg`
  entfernt `Drop` nur diesen Container; sonst initdb-Cluster ohne pgvector, sonst `ÜBERSPRUNGEN`). Nie fremde Container
  oder Abbilder anfassen, kein `podman rmi`.
- **MultiGPT-Nachbauten** (Runde 13; nie das echte MultiGPT, nie SearXNG, nie Internet):
  - `connections::multigpt::mock::MockMultigpt` (Suche, `document_info`, `read_document`): seit Runde 13 auch
    `DELETE /mcp` (in `requests()` mit `"method": "DELETE"`), seitenweises `read_document` (`Behavior.pages`),
    Verzögerung je Werkzeug (`Behavior.tool_delays`), eigene `session_id`.
  - `websearch::mock::{MockWebMcp, MockSite}`: `web_search` (Filter „site:“), `fetch_url` holt nur `*.example` von
    der lokalen `MockSite` (`with_fixtures`: Lichtkunde, Hafenchronik, Gästebuch mit Prompt-Injektion, Seite über
    1 MiB, langsame Seite); `set_results`, `tool_calls`, `configure(|b| b.search_delay = …)`.
  - `sync::mock::MockDocs`: alle sechs Dokument-Werkzeuge, `put_document`/`remove_document`/`set_url`,
    `documents(collection)`, `deny_write(Some(Deny::Forbidden | Deny::ToolError))`, `clear_requests`.
  - Jeder Nachbau beantwortet `DELETE /mcp` mit 200. **Ein** MultiGPT für alles: die Vorderseite `OneMultigpt` in
    `tests/round13.rs` gibt jedes Werkzeug an seinen Nachbau weiter.
  - **Sitzungsvorrat:** `connections::multigpt::Session` legt ruhende Sitzungen prozessweit je (Adresse,
    Token-Hash, Tokio-Laufzeit) ab, höchstens zwei, Werkzeugliste `TOOLS_TTL`. Jeder Test hat einen eigenen
    Nachbau (eigene Adresse), darum teilen sich Tests nie eine Sitzung. Zusicherungen auf die Zahl der
    `initialize`/`tools/list` gelten nur innerhalb eines Tests; ein zweiter Lauf nutzt die Sitzung aus dem Vorrat
    (kein neues `initialize`). Ruhende Sitzungen beendet `multigpt::expire_idle(url, Dauer)` sofort (mit DELETE).
    Nach Gebrauch immer `release` oder `close`; wer eine Sitzung fallen lässt, löst ein DELETE im Hintergrund aus.
  - Abgleich in Tests: `sync::wait_idle(&state)`; Grenzen verkleinern mit `sync::set_limits` (je Datenordner).
- **Bauen mit LanceDB** (Runde 12): `lance` braucht `protoc` im `PATH` (hier `~/.local/bin/protoc`). Erster Bau
  der Abhängigkeiten ≈ 15 min; die eine Server-Test-Binärdatei ≈ 530 MB, `target/` nach `cargo clean` und vollem
  Lauf ≈ 9 GB (vor dem Bündeln in Runde 13: 61 Server-Binärdateien zu je ≈ 500 MB, ≈ 38–40 GB).
- MCP-Tests mit Token: `Authorization: Bearer <secret>` und `ConnectInfo(SocketAddr::from(([192, 0, 2, 7],
  50000)))` (TEST-NET-1) als fremde Gegenstelle; Token über `mcp_token.create`. Nie einen Token in Ausgaben
  oder Meldungen von Tests (Zusicherungen prüfen nur auf Abwesenheit).
- **Versionslisten** von Migrationen nie vollständig festschreiben (`vec![101, 102, 103, 104]` bricht mit der
  nächsten Migration), sondern mit `starts_with` bzw. gegen `MIGRATOR.iter()` prüfen.
- **Migrationstests** vergleichen nach einem Migrator bis zur eigenen Version (`version <= n`) und öffnen
  danach mit `Store::open`; so brechen sie nicht an späteren Migrationen (`migration_0007.rs` bis
  `migration_0016.rs`, `migrate_upto`; `migration_0009.rs` seit Runde 7, weil 0010 die Tabelle `persona` ändert).
- **Fehler aus der Arbeit nachstellen:** erst den Ablauf über RPC so nachbauen, wie die Oberfläche ihn schickt
  (z. B. `wysiwyg.parse` → `wysiwyg.serialize` mit den Anweisungen des Widgets → `outline.check` →
  `outline.save`), den Test **vor** dem Fix scheitern lassen und die Ausgabe im Bericht nennen
  (`tests/round7_titles.rs`).
- Prüfungen, die MCP-Werkzeugnamen auf `update`/`delete`/`create` absuchen (`outline.rs`, `revisions.rs`,
  `work_crud.rs`), nehmen `idea_*`, `post_*`, `outline_*` und `asset_*` aus (`asset_delete` u. a. sind Anträge,
  `outline.rs` prüft, dass ihre Beschreibung mit „Antrag“ beginnt) – Einfälle und Beiträge sind die gewollte Ausnahme. Neue
  MCP-Werkzeuge in die Werkzeuglisten-Tests `mcp.rs`, `round4.rs` und `mcp_ideas.rs` eintragen.
- **Bilder im Test erzeugen** (kleines PNG mit CRC/zlib-Block im Test, SVG als Text), keine Binärdateien im Repo.
- **Testnamen als Satz**, der das Verhalten beschreibt (`accepting_a_proposal_keeps_exactly_one_current`,
  `the_literature_list_shows_titles_without_javascript`), und jede Zusicherung mit Begründung im Text
  (`assert!(html.contains(..), "Vorschlag-Karte fehlt: {html}")`).
- **Gerenderte Seiten prüfen** wie David: konkrete Ausschnitte im HTML (Escaping, Deutsch, keine Reste von
  Platzhaltern), auch ohne JavaScript (Formulare an `/act`).
- **Unit-Tests** für kleine Logik direkt im Modul (`#[cfg(test)] mod tests`), Integrationstests für Abläufe.
- Lauf: `nice -n 10 ionice -c3 cargo test --workspace -j 4` im eigenen Ordner (Worktrees: eigenes `target/`, siehe AGENTS.md).
- **Team-Runden mit Worktrees:** Ein gemeinsamer `CARGO_TARGET_DIR` vermischt die Workspace-Crates der
  Worktrees (gleiche Artefakt-Hashes), ein Lauf kann dann fremde Quellen testen. Entweder je Worktree ein
  eigener Zielordner oder, wie in B1.5, `--config profile.dev.package.<crate>.codegen-units=<n>` je Worktree;
  vor dem Zusammenführen den Lauf im Hauptordner wiederholen.

## Runde 19

### index

- Tests des Index-Workers mit eigenem `EmbedderFactory` (Muster `Recording` in `tests/round19_index.rs`: zeichnet
  jeden Aufruf auf, Vektoren nach Schlüsselwörtern, Modi „gelingt“, „scheitert“, „hängt“). Anfragen der Suche
  von Index-Einbettungen trennen (`index_calls`): Die Suche bettet ihre Anfrage immer ein, nur der Index darf nach
  dem Rückzug nichts mehr einbetten.
- Nach einer Suche erst `rag::wait_idle` abwarten, bevor man die Schreibsperre braucht (`rag.reindex` mit
  `deferred: false`): Jede Suche reiht still `Prune` (und alle 30 s `Shared`) ein.
- Ein `rag::catch_up` ohne vorher gesetzten Einbetter bindet in Tests (eigener Datenordner) **keinen** Einbetter
  mehr, außer die Verbindung `eliezer` ist gespeichert; Tests setzen den Nachbau wie bisher vorher
  (`rag::set_embedders`) bzw. speichern `eliezer` auf `MockEliezer`.
- Präfixe der Verbindung LanceDB setzen Tests über `connection.set` (seit der Zusammenführung mit pgvector) oder
  `Store::connection_set` (voller Satz mit `dir` und `embedding_model`), **vor** dem Import (sonst bettet der erste
  Auftrag ohne Präfix ein).
- `MockDocs` (Abgleich) kennt `upload_without_id`, `reject_empty` (Inhalt nur aus Überschriften), `silent_cap`
  und `lose_upload_answer` (HTTP 502 nach dem Anlegen). Die Werkzeugliste merkt sich der Sitzungsvorrat:
  Schemas (etwa `maximum` von `limit`) vor dem ersten Lauf ändern.
- Zum Nachweis „scheitert vor dem Fix“ dreht ein Hilfsskript im Scratchpad die Kernzeilen zurück; nach dem
  Zurückkopieren (`shutil.copy2` behält die Änderungszeit) die Dateien mit `touch` anfassen, sonst baut cargo
  nicht neu.

### pgvector

- Tests `tests/round19_pgvector.rs`; Container-Tests über `pgvector_support::start("r19pgvector")` (eigene Datenbank je
  Test, `pg.database`). Ein Schema für den Suchpfad gilt je Datenbank: `ALTER DATABASE <pg.database> SET search_path
  …` (nie `ALTER ROLE` – die Rolle teilen sich alle Tests des Containers). Längen in Zeichen messen
  (`chars().count()`), die Grenzen stehen als `pgvector::MAX_*_CHARS`.
- Aufgaben-Präfixe im Test: `connection.set` nimmt `query_prefix`/`document_prefix` jetzt an; was eingebettet wurde,
  zeigt `FixedFactory::last` → `FixedEmbedder::calls` (Texte je Aufruf).
- Gruppen der Karte: ohne Einbetter entstehen die Nähe-Paare nur aus gleichen Schlagwörtern – damit lassen sich
  Gruppen ohne LanceDB bauen (`idea.create` mit passenden `words`). Benennen braucht ein `ScriptedModel`
  (`AppState::open_with`) und `common::model_queue(module_path!())`; die Antwort im Skript in der Form
  „g<id>: Name“. Vorschläge sind prozessweit je (Datenordner, Werk) – jeder Test hat seinen Temp-Ordner, darum keine
  Überschneidung.
- Bausteine des Satzbilds (`ws_command.ts` importiert nichts): unter Node `ws_command.js` aus
  `baruch_server::SCRIPTS` als `.mjs` schreiben und importieren; fehlt `node`, „ÜBERSPRUNGEN“.
- In TypeScript keine `\u2028`/`\u2029`-Escapes in Regex-Literalen über das Edit-Werkzeug schreiben (es setzt die
  echten Zeichen ein, die ein Regex-Literal beenden); stattdessen `\p{Zl}`/`\p{Zp}` mit Schalter `u`.

### treffer
- Schwache Treffer nur über `baruch_rag::is_weak` (nie eigene Schwellen); Seiten zeigen sie in `<details
  class="weak-hits" data-weak-hits>`, Tests suchen `data-weak-label`. Feste Vektoren: eigener `EmbedderFactory` mit
  Begriffsachsen (Vorbild `tests/round19_treffer.rs`, `rag::set_embedders`, dann `rag::catch_up`/`wait_idle`).
- Module mit Trennresten nur erfunden: `baruch_sword::fixture::hyphen_remnants(text, every)`; damit die Schätzung
  anschlägt, müssen die ganzen Wörter in anderen Versen vorkommen (Vorbild `psalm()` in `round19_treffer.rs`).
- `bibles::module_note` merkt sich das Ergebnis je conf-Datei und Änderungszeit; Tests mit eigenem Temp-Ordner sind
  davon unabhängig.
- Lemma-Probe: `Lexicon::lookup_lemma`; ein Testlexikon mit `StrongsPadding=false` zeigt, dass „G26“ ohne Probe
  nicht trifft.

### Integration
- Abnahme `tests/round19_abnahme.rs`: ein Nachbau-Einbetter für alle Wege (`Space`: Begriffsachsen, Aufzeichnung,
  Modi gelingt/scheitert/hängt) über `rag::set_embedders` – pgvector bekommt denselben `EmbedderFactory` wie LanceDB,
  darum zeigt `texts().last()` nach `rag.search` die Anfrage samt Präfix des gefragten Wegs. pgvector über
  `rag.search` statt `open_way`: `connection.rag_set {ways: ["pgvector"]}`, `connection.check` und
  `connection.collections_set`, Tabelle mit `vector(4)` passend zur Dimension des Nachbaus.
- Die Literaturseite zeigt höchstens `limit` Treffer, auch aus der beim Start gesäten Bibliothek (Beispieltitel);
  Zusicherungen auf einen bestimmten schwachen Abschnitt nur für einen, der sicher in der Liste steht.
- SWORD-Fixture: Bücher mit OSIS-Namen (`John`, nicht `Joh`; `Ps` und `Mal` sind gleich), sonst „Schlüssel … gibt
  es nicht“.
- Belegblock Ende-zu-Ende: `store.set_persona_rag("@belege", true)`, `ScriptedModel` mit einem Schritt,
  `persona.review` über RPC; der Block steht in der letzten Nutzer-Nachricht (`model.calls()`).

## EPUB (Runde 20)

Gemeinsam: Testwerk `baruch_epub::sample::input()` (frei erfunden, PNGs im Code erzeugt), Bau und Test nur mit
eigenem `target/`; EPUBCheck in Tests nur als selbst geschriebenes Skript über `path_env`, nie PATH des Prozesses
ändern; xmllint mit `--noout --nonet`.

### epub

- EPUB-Tests in der Crate: `cargo test -p baruch-epub` (ohne Server, schnell). Abwandlungen des Testwerks direkt an
  `sample::input()` (Abschnitt anhängen, Vorlage ergänzen, `footnotes` umstellen); Schlüssel in `sample::KEYS`.
- Große, nicht komprimierbare Bilder (z. B. für die 8-MiB-Grenze von `epub_export`): `sample::png_with(w, h, |x, y|
  …)` mit Rauschen; drei Bilder à ≈ 3,3 MiB (1050 × 1050) bleiben je unter `MAX_ASSET_BYTES`.
- Wohlgeformtheit: jede `.xhtml`/`.ncx`/`.opf`/`.xml` aus dem ZIP in einen Temp-Ordner schreiben und
  `/usr/bin/xmllint --noout --nonet` aufrufen (`tests/package.rs`), sonst quick-xml und „ÜBERSPRUNGEN“.
- Werk ohne Abschnitt im Server-Test: Fassungen und Knoten direkt in SQLite löschen (Muster `outline_mcp.rs`).
- Gleiche Bytes prüfen: RPC `work.epub {inline: true}` gegen den Download vergleichen (gleicher Stand).

### reader

- Reader-Profile in `baruch_epub::readers` (Slugs eingefroren), CSS je Profil in
  `crates/baruch-server/static/readers/<slug>.css` – ein neues Profil braucht beides und einen Zweig in
  `epub_preview::reader_css`; `baruch-epub/tests/readers.rs` prüft, dass es zu jedem Slug genau eine Datei gibt.
- Reader-CSS ohne `<` und `&` (steht als `<style>` in XHTML) und ohne Adressen; Nachtmodus hinter `/* @night */`.
- Reader-Dateien im Test: `GET /werke/{w}/epub/leser/{slug}/{href}`; `href` aus `work.epub` (`files`). Bilder gibt
  der S1-Bau nicht aus – `epub_preview::reader_file` direkt auf einem `EpubBook` mit eigenem `EpubItem` prüfen.
- Unteranfragen aus dem Rahmen im Test nachstellen: `sec-fetch-site: cross-site` + `host` + `referer` auf eine
  Reader-Datei desselben Werks.
- Sichtprüfung (nur Integrator/Nutzer, nie parallel zu Tests): Wegwerf-Server nach „Bauen und prüfen“, Seite
  `/werke/{w}/epub/vorschau` fotografieren; Bilder und `book.css` müssen im Rahmen laden. Firefox kürzt den
  `Referer` aus `sandbox`-Dokumenten auf die Herkunft (Integrator R20). Für jedes Foto ein **frisches**
  Firefox-Profil nehmen (sonst zeigt der Zwischenspeicher alte Antworten) und den Bildpfad **absolut** angeben
  (`--screenshot` mit relativem Pfad schreibt nichts). Kopfzeilen der Unteranfragen sieht man über einen kleinen
  protokollierenden Python-Vermittler auf `127.0.0.1:0` vor dem Wegwerf-Server.

### epub-pruefung

- Eigene Prüfung testen: `crates/baruch-epub/tests/check.rs` baut das Testwerk (`build` + `package`) und schreibt für
  jede kaputte Variante das ZIP selbst neu (`zip::ZipWriter`, Einträge in Reihenfolge, `mimetype` wahlweise DEFLATE);
  OPF/XHTML werden als Text geändert (`replace`). Jede Variante prüft genau ihren Code (`codes::…`). Neue Codes in
  `baruch_epub::check::codes` und in `docs/RPC.md` (epub.check) eintragen.
- Gültiges Testwerk = kein Befund mit `Severity::Error`; Hinweise (`NCX_MISSING`, `A11Y_METADATA_MISSING`,
  `IMG_ALT_EMPTY`) sind erlaubt. Ändert R20a das Testwerk oder den Bau, muss `the_valid_sample_has_no_error` grün
  bleiben – sonst ist der Bau oder die Prüfung falsch.
- xmllint: Tests nehmen ihn nur über `find_xmllint(std::env::var_os("PATH"))`, fehlt er, „ÜBERSPRUNGEN“ für die
  xmllint-Zusicherungen; der Rückfall auf quick-xml hat einen eigenen Test (Pfad, der nicht startet).
- EPUBCheck in Tests nie echt: ein Skript `epubcheck` (`#!/bin/sh`, ausführbar) in einem Temp-Ordner, übergeben als
  `path_env` (`find_epubcheck(Some(dir), jar)`, im Server `epub_check::run_check(…, Some(dir))`); ein Marker-File im
  Skript zeigt, ob es lief. Hängendes Skript (`sleep 30`) mit kurzer Zeitgrenze prüft den Abbruch samt Enkel. Den
  PATH des Prozesses nie setzen. Steht auf dem Rechner ein echtes `epubcheck` im PATH, überspringen die Tests zu
  „nicht gefunden“ (`ÜBERSPRUNGEN`).
- Jar-Einstellung: Testdateien `epubcheck.jar` mit beliebigem Inhalt genügen (Java wird nie gestartet); MCP darf
  keinen Weg dazu haben (Test über `tools/list`).
- Buchangaben-Reste: Commits zählen mit `git rev-list --count HEAD` im Spiegel (`round20_pruefung.rs`), Reihenfolge
  mit `git log -2 --format=%s` („Umschlag entfernt“ vor „Bild …: gelöscht“). Umschlag-Quellen und Rücken nur unter
  `common::PDF_SERIAL` (Typst).
- Goldstände R26/R27 (`baruch-typst/tests/cover_layout.rs`, `round27_satz.rs`) bleiben unverändert: Textregeln der
  Werkvorlage kommen nur hinzu, wenn die Vorlage mehr als `#set text(lang: "de")` setzt.

### Integration

- Ende-zu-Ende-Abnahme nach dem Muster `tests/round20_abnahme.rs`: Werk importieren, Bilder/Umschlag/Buchangaben/
  Satz-Einstellungen per RPC, Querverweis über `add_proposal` + `revision.accept` (Schlüssel erst nach dem Import
  bekannt). Kaputte EPUBs aus den echten Bytes mit `zip::ZipWriter` nachbauen; für „kaputtes XHTML“ eine Datei mit
  `</p>` wählen (Teil-Seiten haben keinen Absatz).
- CLI gegen einen Datenordner erst, wenn der `AppState` im Test wieder freigegeben ist (Block), dann Bytes vergleichen.

## Runde 31 – Register und Glossar

### Schnittstelle R31-S1

- Marke im Text: `#metadata((register: "…", term: "…"))<baruch-idx>` (`baruch_typst::index::mark_source`,
  `parse_mark`); Test-Hilfe für die Rundreise im Satzbild: `crates/baruch-wysiwyg/tests/index_mark.rs`.
- `term_key` steht in der Datenbank: nie ändern. `sort_key` wird nie gespeichert.
- Export, Karte und Spiegel bleiben ohne R31-Einstellungen byte-gleich (`BackMatter::is_active`); wer das Buchende
  anhängt, nimmt `snapshot::append_tail` (Karte = Export).
- Server-Testdateien: `round31_glossar.rs` (a), `round31_register.rs` (b), `round31_kontextmenue.rs` (c); die Abnahme
  `round31_abnahme.rs` legt der Integrator an.
- Die drei MCP-Werkzeuge waren in S1 STUB; seit R31a umgesetzt, die Ausnahme in `tests/mcp.rs` ist entfallen.

### 31a glossar-daten

- Store `src/glossary.rs`, Migration 0049, Tests `crates/baruch-store/tests/{glossary,migration_0049}.rs`; Migrationstests
  0040/0041/0048 nehmen die 0049-Objekte heraus (`R31_OBJECTS`). Testwerke frei erfunden.
- Übernahme eines Registervorschlags hängt an `baruch_typst::index::mark_word` (register-satz). Tests prüfen mit einer
  Sonde (`mark_word(…)` → `IndexError::NotYet`?) das ehrliche S1-Verhalten bzw. danach die echte Übernahme
  (`glossary.rs::accepting_an_index_proposal_is_a_new_human_revision_or_stale_without_change`,
  `round31_glossar.rs::mcp_only_proposes_and_the_human_decides`).
- Austausch Unterversion 7: Beispielwerk (`baruch_exchange::sample`) mit zwei Registern (`sample_uuid(1110/1111)`), drei
  Begriffen (`1100`–`1102`) und Grabsteinen `glossar-begriff`/`register` (`1103`/`1112`). Tests, die ältere
  Unterversionen aus dem Beispiel bauen, nehmen die u7-Teile über `without_u7` heraus (in `without_u6` enthalten).
  `docs/schema/baruch-werk-1-u6.xsd` ist eingefroren (SHA-256 im Test).
- Server-Test `tests/round31_glossar.rs` (RPC samt Commits und Trailern, MCP, `glossary.suggest` mit `ScriptedModel`
  unter `common::model_queue`, Austausch über zwei Instanzen mit xmllint gegen u7). Ältere Server-Tests erwarten
  `version="7"` bzw. das u7-XSD.

### 31b register-satz

- Marken und Projektion: `baruch_typst::index` mit privaten Untermodulen `index/{scan,project,back}.rs`; Tests
  `crates/baruch-typst/tests/index_{marks,back_matter}.rs` (Text), gemessen im Satz in
  `crates/baruch-render/tests/index_measure.rs` (`page_texts`: Seitenzahl = Ziffern in der Fußzeile der Seite, auf der
  das markierte Wort steht; Registerzeilen = Textstücke einer Grundlinie, nach Spalten getrennt). Ohne Silbentrennung
  setzen (`#set text(hyphenate: false)`), sonst zerfallen Stichwörter.
- Quelle im Messtest wie im Store: Vorlage + `rules` + je Abschnitt `Projector::project` + `back_matter`. Mehrspaltig
  fließt Typst erst in die zweite Spalte, wenn die erste voll ist – für den Spaltentest viele Stichwörter erzeugen.
- Store: `crates/baruch-store/tests/index_export.rs` rechnet den erwarteten Export aus der alten Karte (Abschnitte durch
  `Projector`, dahinter `back_matter`); Glossarbegriffe kommen bis glossar-daten leer aus dem Store, die Glossarseite
  sagt dann „Keine Begriffe.“.
- EPUB: `crates/baruch-epub/tests/index.rs` projiziert `sample::input()` selbst (im Server kommen die Abschnitte schon
  projiziert aus `export_map`); Links der Register auf `baruch-idx-{key}-{n}` gegen die Dateien prüfen.
- Server: `tests/round31_register.rs`; PDF nur unter `common::PDF_SERIAL`, Text mit `pdftotext` je Seite (`\x0c`).
  `typst` gibt es auf dem Entwicklungsrechner nicht im PATH: der Test meldet „ÜBERSPRUNGEN“ und setzt den Export
  zusätzlich mit dem eingebetteten Compiler (`baruch_render::page_texts`) als eigenständige Datei.

### 31c kontextmenue

- Marke im Satzbild: `span.ws-index-mark` ohne Textknoten (Zeichen nur im Stil); wer das HTML ändert, zieht
  `render_index_mark` (baruch-wysiwyg), `fillMark`/`markOf` (wysiwyg.ts), `markLabel` (ws_command.ts) und die Tests
  `baruch-wysiwyg/tests/index_mark.rs` und `tests/round31_kontextmenue.rs` zusammen nach.
- TypeScript-Gegenstücke zu `baruch_typst::index` (`markSource`, `parseMarkAt`, `validateMark`) und `index_mark_label`
  (`markLabel`) prüft `round31_kontextmenue.rs` unter Node gegen die Rust-Funktionen; jede Änderung dort zieht das TS nach.
- Seiten-Tests gegen Aufgaben, die noch fehlen können: erst per RPC fragen, ob die Methode „noch nicht umgesetzt“ meldet
  (`not_yet`), dann genau diesen Zustand prüfen (Meldung samt erhaltener Eingaben bzw. Weg bis zur Anzeige) – so bleiben
  die Tests vor und nach dem Zusammenführen von 31a/31b gültig.
- Neue Formularfelder des Satz-Blocks: `index_register` (mehrfach) ist ein Feld je Häkchen; `block_context` braucht dafür
  die Vec der Formularpaare (die `BTreeMap` `v` hält nur einen Wert je Name).
- Vom Server gebaute Rücksprung-Adressen (`back`) in Vorlagen mit `|safe` und vorher `&` → `&amp;` (minijinja schreibt
  `/` sonst als `&#x2f;`); `glossary_page::back_to` folgt nur Pfaden dieses Servers.
- `round29_oberflaeche.rs` nimmt die drei R31-Felder noch von der Bezeichnungsprüfung aus (Notiz an den Integrator: die
  Ausnahme kann jetzt fallen; die Prüfung steckt auch in `round31_kontextmenue.rs`).

### Integration

- Abnahme `tests/round31_abnahme.rs`: Werk mit 70 gleichen Absätzen im mittleren Kapitel, damit ein Stichwort über
  mehrere Seiten läuft (Bereich „3–6“ gegen pdftotext gemessen); `fill()` legt Glossar und Marken genau so an, wie die
  Oberfläche sie schickt (Formulare ohne JavaScript bzw. RPC wie die Dialoge). Seitenzahl = Index der pdftotext-Seite + 1
  bei `PageNumberStart::FirstPage`.
- „siehe auch“ einer Marke mit Unterstichwort steht in der Zeile des Unterstichworts hinter der Seitenzahl
  („Leuchtturm 2; siehe auch Nebel“).
- Ein Export trägt Kennungs-Labels (`<kN>`, `#metadata("baruch-key")`) hinter Überschriften und Teilen, die in der
  Importdatei fehlen: Byte-Gleichheit ohne R31-Einstellungen gegen den Export vorher prüfen, nicht gegen die
  Importdatei.
- Import eines Exports mit Buchende: `strip_back_matter`/`strip_projection` (Unit-Test in
  `baruch-typst/tests/index_back_matter.rs`, Ende-zu-Ende in der Abnahme).

## Runde 21 – Import und Bilder

### Schnittstelle R21-S1

- Abhängigkeiten stehen seit S1 fest (`zip`, `quick-xml` 0.38, `pulldown-cmark` 0.13 ohne Standard-Features im Server,
  `typst-render` 0.15.1 in baruch-render; typst-render ließ sich laden, der Ersatz resvg/image war nicht nötig).
  Entwickler ändern keine Cargo.toml/Cargo.lock; Bedarf an den Integrator.
- ZIP lesen nur über `baruch_server::archive::read_zip` (Grenzen `IMPORT_LIMITS`). Testarchive im Test mit
  `zip::ZipWriter` bauen; gleiche Namen lässt der Schreiber nicht zu – im Unit-Test wird der zweite Name in den Bytes
  umgeschrieben (Kopf und Zentralverzeichnis, gleiche Länge), Sonderdateien über die äußeren Attribute (Offset 38 im
  Zentralverzeichnis, Modus in den oberen 16 Bit), weil `unix_permissions` nur die Rechte-Bits nimmt.
- Hinweise eines Imports als `import::ImportWarning {code, message, line}` in `ImportReport.warnings`; Codes beginnen
  mit `md-`, `archive-` bzw. `import-`. PDF/EPUB legen das Werk nur über `import::import_generated` an.
- Server-Testdateien: `round21_pdf_import.rs` (a), `round21_markdown_import.rs` (b), `round21_bilder.rs` (c), in S1 nur
  mit Kopf; die Abnahme `round21_abnahme.rs` legt der Integrator an. Keine S1-Zweige in Tests (Lehre R31).

### 21a pdf-import

- Testdokumente nur im Test erzeugen: PDF über `baruch_render::compile_pdf` aus erfundenem Typst unter
  `common::PDF_SERIAL` (Typst-Vorgabe: Ebene 1 setzt größer als Ebene 2 – daraus liest die lokale Heuristik die Ebenen),
  EPUB von Hand mit `zip::ZipWriter` (mimetype STORED, `container.xml`, OPF, `nav.xhtml`, XHTML), PNG über
  `baruch_epub::sample::png`. Symlink im Archiv mit `ZipWriter::add_symlink`, zu großer Eintrag als STORED-Rauschen
  über 16 MiB, Bombe als 8 MiB Leerzeichen DEFLATE.
- Lokaler PDF-Weg braucht Poppler (`pdftotext`, `pdfinfo`); Tests prüfen das mit `poppler()` und melden sonst
  „ÜBERSPRUNGEN“. „pdftotext fehlt“ ohne Umgebungsvariablen: `pdf_import::cli_import_with`/`preview_with` mit
  `PdfTools { pdftotext: <nicht vorhandener Pfad>, … }`.
- „Kein stiller Wechsel“ prüfen: `pdf_import::local_pdf_reads(&state)` zählt pdftotext-Läufe je Datenordner; nach einem
  Fehler von MultiGPT muss er 0 bleiben.
- MultiGPT-Nachbau mit `mock::full_tools()`, Dokument über `configure(|b| { b.upload_pages = …; b.upload_toc = …;
  b.index_polls = 1; })`; Aufrufe über `tool_calls("upload_document" | "read_document" | "run_status")` prüfen
  (`page_from`/`page_to` in den Argumenten). `default_tools()` nie ändern.
- Entwürfe liegen unter `<data>/import-drafts/<32 hex>/`; Alter/Anzahl testen, indem der Test Ordner mit
  `draft.json {"created": <Sekunden>}` anlegt (die Aufräumung liest nur `created`) und dann `source_import.preview` ruft.
- Formulare im Test als multipart von Hand (Grenze, `Content-Disposition`); 413 mit einer Datei über
  `pdf_import::UPLOAD_LIMIT`.
- `/quelle?title=…` zeigt den lokalen Volltext erst, wenn `literature.rs` `fulltext::has_fulltext` nutzt (Integrator);
  bis dahin prüfen die Tests den Volltext über RPC `source.read {title}` und `highlight.add`.

### 21b markdown-import

- Umwandlung nur in `src/markdown_import/convert.rs` (`to_typst`, rein); Unit-Tests dort für jede Form. Neue Form =
  Zeile in der Tabelle `docs/Architektur.md` („21b markdown-import“) und ein `md-…`-Code, nie still verwerfen.
- Testtexte in Rust-Strings: Eine Zeilenfortsetzung `\` am Zeilenende verschluckt den Einzug der nächsten Zeile –
  eingerückte Markdown-Zeilen (verschachtelte Listen) mit `\n  - …` in **derselben** Quellzeile schreiben.
- Archive im Test mit `zip::ZipWriter` bauen (`tests/round21_markdown_import.rs`, `zip_of`); große Bilder im Archiv aus
  Pseudozufall, sonst hält `read_zip` sie für eine Zip-Bombe (Verhältnis > 200). Ordner-Upload = mehrere Teile `files`
  mit relativem Pfad im `filename` (so schicken es Browser bei `webkitdirectory`).
- Pfade im Bericht: minijinja schreibt `/` als `&#x2f;`, `'` als `&#x27;` – Zusicherungen auf HTML entsprechend.
- Zeilen der Hinweise sind 1-basiert in der **Markdown**-Datei (nicht im erzeugten Typst).
- Rundreisen Export → Import im Test: Titel kommen aus `#set document` – beim Re-Import `title` mitgeben, sonst
  „Titel vergeben“; Vergleiche mit `baruch_typst::xref::strip_key_labels` auf beiden Seiten.
- `numbering_scheme.rs`/`round14_numbering.rs` (fremd) prüfen unter „Ohne Nummern“ nur, dass die `=`-Zeile bleibt; seit
  21b steht dort zusätzlich die Show-Regel des Rückfalls für `numbered = 0` in der Vorlage des Exports.
- Fremder Test, der an der alten Form hängt (Meldung an den Integrator): `tests/work_crud.rs:424` erwartet das alte
  Datei-Feld `<input type="file" name="file" accept=".typ" required>`; seit 21b `accept=".typ,.md,.markdown,.zip"` ohne
  `required` (der Ordner ist die Alternative) plus `<input type="file" name="files" multiple webkitdirectory>`.

### 21c bilder

- Server-Tests in `tests/round21_bilder.rs`; Store in `crates/baruch-store/tests/assets_r21c.rs`. Testbilder mit
  `baruch_epub::sample::png(w, h, [r, g, b])`, SVG als Text; große Inhalte (8 MiB) = PNG-Kennung + Füllbytes (der Typ
  wird nur an den ersten Bytes erkannt).
- Altbestand `/x.png`/`x.png` lässt sich über den Store nicht mehr anlegen: im Test per `sqlx::query("INSERT INTO
  asset_path …")` auf `server.state.store.pool()` nachbauen. Alter von Entscheidungen ebenso per `UPDATE
  asset_request SET decided = strftime(…, 'now', '-31 days')` (kein Schutz-Trigger); `asset_revision.created` ist
  dagegen per Trigger unveränderlich.
- Kontingent im Test: je Client eine eigene MCP-Sitzung (`clientInfo.name` → `@mcp:<name>`), gezählt wird über
  `Store::asset_mcp_usage`. 30 kleine `asset_add` sind schnell; für 64 MiB acht `asset_replace` à 8 MiB (nur
  Datenbank, kein Git).
- PDF-Satz messen: `pdfimages -list` (Spalten page, num, type, width, height); fehlt es → „ÜBERSPRUNGEN“. `pdfinfo`
  meldet bei jedem Typst-PDF die Suspects-Zeile (Ursache in Architektur „21c bilder“); Tests vergleichen gegen eine
  Kopie mit `/Xuspects false`.
- EPUB-Cover messen: `image::load_from_memory` auf `OEBPS/images/cover.png`, Vergleich mit
  `baruch_render::render_png` auf einer Quelle nur mit dem Umschlagbild (gleiche Seitengröße → gleiche Pixelgröße);
  Blockrahmen in mm × (Breite / Seitenbreite). Alles mit Typst unter `common::PDF_SERIAL`.
- Wer das EPUB-Cover ändert: Server-Tests, die die Dateiliste zählen, sehen seit R21c immer ein `cover` vorn.

### Integration

- Zusammenführen pdf-import → markdown-import → bilder mit `--no-ff` lief ohne Konflikte; nachgezogen hat der
  Integrator nur fremde Tests, die vertragsgemäße Änderungen trafen: `migration_0048/0049.rs` prüfen die höchste
  Bibliotheksmigration nur noch als „≥ 107“ (0108 kam dazu – wer eine Migration ergänzt, prüft so, nicht auf
  Gleichheit), `round12_multigpt.rs` vergleicht mit `multigpt::rank_score(2)` statt 1/Rang, `work_crud.rs` mit dem
  neuen Datei-Feld (`accept=".typ,.md,.markdown,.zip"` ohne `required`, dazu `name="files" multiple webkitdirectory`).
- `/quelle?title=…` fragt `literature::fulltext::has_fulltext` (MultiGPT-Verknüpfung **oder** lokaler Volltext aus
  0108), nicht mehr nur `title_remote`. Prüfen: EPUB mit `target: "library"` übernehmen, dann steht der Text auf der
  Seite und nicht `data-fulltext="none"`.
- Abnahme `tests/round21_abnahme.rs` (8 Tests, „Der Apfelgarten am Deich“, frei erfunden): Zeilen der Gliederung aus
  der Vorschau-Seite über `name="src" value="…"`/`name="line" value="…"` lesen und als Formular wiederholen (Reihenfolge
  `line`, `src` je Punkt). EPUB-Cover belegen ohne Koordinaten: dasselbe Werk mit `cover.layout_set` und je einem
  unsichtbaren Block rastern und Pixel vergleichen (Titel, Untertitel, Autor einzeln nachweisbar).
- Re-Import eines `export --out`-Ordners: Ordner als ZIP packen und auf einem **frischen** Datenordner
  (`common::open()`) über `work.import_archive` einlesen, sonst lehnt der vergebene Titel ab; Vergleich
  `strip_key_labels(export_typst)` und `format!("{:?}", export_assets)`.
- `markdown_import::cli_import` gibt Hinweise im Bericht auf **stdout** aus (`Hinweis (<code>, Zeile n): …`), der
  Typst-Import (`main.rs::import`) auf stderr.
- `cargo fmt --all` nach dem Zusammenführen: die Zweige waren nicht durchgehend formatiert.

## Runde 22 – Revisionen und Spiegel

### fassungen

#### Revisionen

- **Fortschreiben (Runde 22):** Nur `outline.save`/`revision.save` mit `mode: "auto"` schreiben fort (Store
  `outline_save_mode`, `save_human_mode`/`save_human_with_reason`); fehlt `mode`, bleibt alles „neue Fassung“ – alte
  Tests und Clients brauchen nichts. Jeder Schritt ist genau ein Spiegel-Commit (`Baruch-Action: amended|amend-undone`,
  `Baruch-Amend`), Rückgängig nur als neuer Schritt (`revision.amend_undo`), nie Verlaufszeilen löschen.
  Neue Schreibwege legen weiter neue Fassungen an, bis sie ausdrücklich `SaveMode::Auto` bekommen.
- Zeitgrenze im Test ohne Schlafen: `revision.created` ist per Trigger unveränderlich – im Test den Trigger
  `revision_origin_immutable` kurz herausnehmen, `created` zurücksetzen, den Trigger wörtlich wieder anlegen
  (`crates/baruch-store/tests/revision_amend.rs`, `age_revision`); maßgeblich ist max(Anlage, letzte Fortschreibung).
- `change_percent` zählt Leerräume als gleiche Teile (ein völlig neuer Satz ≈ 50 %); Testtexte für „große Änderung“
  deutlich umschreiben, für „kleine Änderung“ ein Wort einfügen.
- Seitenzweige `refs/revisions/<key>/<n>` lesen: `state.mirror(work).await?.side_refs("refs/revisions/")`,
  `read_file(name, pfad)`, `parents(name)` oder `git log <ref>` im Spiegelordner. Ein neuer Vorschlag ist kein Commit;
  seine Referenz legt `mirror::side_refs_after_proposal` gleich nach dem Anlegen an (alle Vorschlagswege im Server).
- Adressen in ausgelieferten Seiten prüfen: minijinja maskiert `/` als `&#x2f;` und `&` als `&amp;`; vor dem Suchen
  zurückwandeln (`round22_fassungen.rs`, `readable`).

### befund-rueckfrage

- Runde 22 (befund-rueckfrage): Tests `tests/round22_befunde.rs` (MockEliezer unter `common::model_queue`: Kontext des
  erneuten Prüfens über `chat_bodies()`, Rückfrage, Pflichtbefund, Formular „Ja/Nein“, Stand per SQL
  `UPDATE revision SET amend_seq`), `tests/round22_archiv.rs` (anlegen → löschen → `/werke/archiv` →
  wiederherstellen, Symlink im Archiv, `mirror::sync_all` für Einfälle ohne Werk), Store `tests/works_restore.rs` und
  die Stand-Tests in `tests/findings.rs`. Persona-Antworten mit „Erledigt: Befund n“ im Test nur für Befunde
  derselben Persona; `mirror_ideas` ist privat – im Test über `mirror::sync_all` bzw. `work.delete` prüfen.
- `/werke/archiv` und der Verlauf einer Fassung stehen in `tests/round9_menu.rs` (`all_pages`, jetzt 47 Seiten).

### spiegel-entwurf

#### Runde 22 – spiegel-entwurf

- Entwurf: Regel `baruch_typst::typeset::draft_rule` (Tests `typeset/draft.rs`, `baruch-render/tests/draft.rs` mit
  `page_texts`); Server `typeset::prepare_with`/`effective_draft`/`cover_draft`. Tests `tests/round22_entwurf.rs` unter
  `common::PDF_SERIAL`. **pdftotext und Wasserzeichen:** Kopf-/Fußzeile ist gesperrt gesetzt (pdftotext trennt die
  Buchstaben mit Leerzeichen) – vor dem Suchen allen Leerraum entfernen; diagonalen Text zerlegt pdftotext zwischen die
  Zeilen des Buchtexts, darum dort die Satzquelle (`prepare_with`) mit `baruch_render::page_texts` prüfen.
- Ohne Entwurf ist die Satzquelle genau `export_map().typst` (Test vergleicht direkt); PDF-Vergleiche über `sha256` der
  RPC-Antwort (`inline: true`).
- Austausch u8: Beispiel mit u8-Teilen `baruch_exchange::sample::sample_document_u8()`; `sample_document()` bleibt u7
  (ältere Tests bauen daraus Dateien der Unterversionen 1–7). XSD-Prüfung `docs/schema/baruch-werk-1-u8.xsd`; ältere
  Server-Tests prüfen ihre Ausgaben weiter gegen u7 (ohne u8-Teile gültig) und erwarten seit R22 `version="8"`.
- Store-Tests `crates/baruch-store/tests/exchange_u8.rs` (Vorspulen, Antrag mit Kennung zustimmen/ablehnen); Server
  `tests/round22_abgleich.rs` (zwei Instanzen, Zustimmung mit Spiegel-Commit, `<kN>`-Regel, Spiegel setzen mit
  `Store::mirror_snapshot` + `baruch_render::compile`).

### Integration

- Abnahme `tests/round22_abnahme.rs`: Fortschreiben dreimal, Rücknahme per Formular, „Als neue Fassung speichern“,
  große Änderung, Zeitgrenze (gealterte Fassung, Einstellung per Formular), `policy_off`; Persona-Fassung → `not_own`,
  Seitenzweig sofort nach MCP-Vorschlag; Rückfrage nach ↻ Wiederholen mit MockEliezer, „Ja, erledigt“ per Formular;
  Wasserzeichen mit Datum je Seite gemessen (pdftotext, Seiten durch `\x0c` getrennt, Leerraum entfernen), Umschlag,
  EPUB, byte-gleich ohne; Austausch zwischen zwei Wegwerf-Instanzen mit Vorspulen und Teil-Antrag; Archiv;
  `<kN>`-Regel über Export → Import.
- `node.get` trägt seit dem Integrator `amend_seq` je Fassung (vorher nur `revision.tree`).

## Runde 23 – Härtung

### web

- **Verhaltenstests der Widgets ohne Browser.** Der DOM-Nachbau steht in `tests/round18_satzbild.rs` (`DOM`) und ist
  über `run_harness(scene, &scenario)` für jede Testdatei nutzbar. Die Szenen dieser Runde stehen in
  `tests/round23_web/widgets.mjs` (Ergänzungen: `FormData`, `<dialog>`, `DOMParser`, Verlauf, `requestSubmit(submitter)`,
  Felder mit `form="<id>"`; nachgebautes Netz für RPC und Seiten). Eine Szene wählt `scenario.scene`; reine Funktionen
  eines Moduls ruft die Szene `pure` (`module`, `calls`). Zum schnellen Ausprobieren ohne `cargo`: Widgets mit
  `web/node_modules/.bin/tsc -p web/tsconfig.json --outDir <ordner>` übersetzen und `node harness.mjs scenario.json
  file://<ordner>` aufrufen.
- **Nachbau prüfen, bevor man ihm glaubt.** Der Selektor „Nachfahre“ (`a b`) traf im Nachbau bis Runde 22 nie
  (der Vorfahre fiel beim Zerlegen weg); `:checked` fehlte. Beides ist behoben. Trifft ein `querySelector` im Nachbau
  nichts, zuerst den Selektor im Nachbau prüfen.
- **Typst merkt sich Funktionsaufrufe.** Ein rekursives `fib(n)` ist im eingebetteten Typst sofort fertig (comemo); für
  einen Test, der Rechenzeit braucht, eine Schleife nehmen (`for i in range(1500000) { s += i }`).
- **Feature-Vereinigung.** `cargo test -p <crate>` baut die Abhängigkeiten mit anderen Features neu (lange Wartezeit);
  für einzelne Server-Tests `cargo test --workspace --test server -- <filter>` nehmen (das Testziel heißt `server`).
- **Prozessweite Zähler in Tests.** Zähler für „wie oft“ (z. B. `cover_print::contrast_decodes`) je Schlüssel führen
  (Prüfsumme) und im Test einmalige Daten verwenden; alle Server-Tests laufen in einem Prozess nebeneinander.
- **Schutz des Arbeitsbaums.** Shell-Aufrufe mit Hier-Dokument, in denen ein Wort wie `commit` vorkommt, lehnt der
  Schutz des Worktrees ab; Hilfsskripte als Datei im Scratchpad anlegen und aufrufen.

### mcp-namen

- MCP-Tests mit Namen: Ein Client ohne Token, der wie ein Token heißt, schreibt als `@mcp:lokal-<handle>`. Tests, die einen Token und einen gleichnamigen Client ohne Token mischen, müssen das erwarten.
- Ein Token mit dem Namen eines widerrufenen lässt sich über RPC und Formular nicht mehr anlegen; Tests nehmen einen neuen Namen oder `Store::mcp_token_create` (Bestand ohne Handle-Prüfung).
- Bildkontingent im Test: Alle Sitzungen ohne Token teilen je Werk ein Kontingent. Wer „ein anderer Client schreibt weiter“ prüfen will, braucht dafür einen Token (`mcp_token.create`, fremde Gegenstelle mit `Authorization: Bearer`).
- Obergrenzen testen, ohne hunderte Commits zu erzeugen: einen echten Eintrag anlegen und per `WITH RECURSIVE … INSERT … SELECT` vervielfachen (Muster in `tests/round23_mcp_namen.rs`).
- Echte Gegenstelle für `/mcp`: `axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())` auf `127.0.0.1:0` und `reqwest` mit `no_proxy()` (Muster `listen`/`Wire` in derselben Datei).
- `ChatOrder` hat das Pflichtfeld `relay` (Oberfläche `false`, MCP `true`); `PostRequest` in Tests mit `..Default::default()` schreiben.

### geheimnisse

- `baruch_llm::scrub` ist die eine Stelle zum Schwärzen; neue Wege rufen sie auf, nie `text.replace(secret, "***")`.
- Nachbau für Weiterleitungen: `Recorder` in `crates/baruch-llm/tests/redirect.rs` bzw. `round23_geheimnisse.rs` (roher
  TCP-Server auf 127.0.0.1:0, merkt sich die Anfragen; zwei Stück: der erste antwortet 30x auf den zweiten).
- Websuche gegen 127.0.0.1 im Test: `websearch::fetch::set_fetch_exceptions(&state, &["127.0.0.1"])`, danach wieder
  leeren. Die Nachbauten nutzen `*.example` und brauchen das nicht.
- `baruch_exchange::sample::sample_document()` trägt Kapitel mit `==` unter Teilen; seit Runde 23 meldet jedes
  Einlesen dieses Beispiels dafür Hinweise in `deviations` (Zusicherungen auf `deviations` mit `find`/`any`, nie auf
  „leer“).
- Der Leser ist streng: Eine Datei mit `version="1"` darf keine Teile späterer Unterversionen tragen (ein
  heruntergesetztes `version` an der aktuellen Ausgabe wird abgelehnt).

### Integration

- Abnahme `tests/round23_abnahme.rs` nutzt die Helfer der drei Aufgaben (`round23_mcp_namen::{mill, local, remote,
  create_token, …}`, `round23_geheimnisse::{with_models, redirecting}`; dafür `pub(crate)`). Neue Abnahmen dürfen
  solche Helfer teilen, statt sie zu kopieren.
- Seiten außerhalb der Arbeitsseite zeigen ohne `?work=` das Vorgabewerk (Abzeichen im Menü): Tests auf Abzeichen
  hängen `?work=<id>` an.
- `own_reads` im Test: `Sec-Fetch-Site: cross-site` **mit** `Sec-Fetch-Mode` und `Sec-Fetch-Dest` schicken; nur
  `Sec-Fetch-Site` allein gilt nicht als Unterressource (ältere Tests bilden so den Link von außen nach).
- Ein Test, der „ein anderer Client ohne Token schreibt weiter“ erwartet, ist seit Runde 23 falsch (gemeinsames
  Kontingent); `round21_bilder.rs` erwartet jetzt die Ablehnung.
  Seit der Nachbesserung gilt das auch für `add_post` (300/24 h), offene Vorschläge (100) und die Modellaufträge
  (alle Clients ohne Token zusammen vier, `quota::MAX_WITHOUT_TOKEN`; je Name weiter zwei): Ein Test, der mehr als
  vier hängende Modellaufträge über Loopback ohne Token braucht, nimmt Tokens (`store.mcp_token_create`, Anfrage mit
  `Authorization: Bearer …`). Bestand von vor Runde 23 bildet ein Test mit `store.mcp_token_create` nach (ohne
  Handle-Prüfung), siehe `round23_mcp_namen::two_old_tokens_that_only_the_new_rule_would_merge_stay_apart`.
- Bildschirmfotos mit Thema und JavaScript (Integrator R23): Mit JavaScript liest das Thema-Widget `document.cookie`;
  ein Vorschalt-Server, der nur den `Cookie`-Kopf der Anfrage setzt, ergibt darum mit JavaScript das helle Thema. Der
  Vorschalt-Server setzt die Cookies zusätzlich als `Set-Cookie` in jeder Antwort, und jede Variante (Thema, mit/ohne
  JavaScript) bekommt ein eigenes Firefox-Profil (Cookies gelten je Rechner, nicht je Port); vor dem eigentlichen Foto
  einmal eine beliebige Seite laden. Ohne JavaScript: `user_pref("javascript.enabled", false);` in `user.js` des Profils.

## Runde 24 – Verteilung

### pakete

- **Version an einer Stelle.** Quelle ist `[workspace.package] version` in `Cargo.toml`; Skripte lesen sie über
  `scripts/version.sh`, das Programm über `about::VERSION`. Wiederholt wird sie nur in `site/hugo.toml`
  (`params.version`), `README.md` und `CHANGELOG.md`; `tests/round24_abnahme.rs` und `scripts/ci-site.sh` melden eine
  Abweichung. Die Nummer ändert nur der Nutzer.
- **Paket prüfen, ohne es zu installieren:** `scripts/build-deb.sh` (Release-Bau, etwa 12 Minuten beim ersten Mal),
  `dpkg-deb -I/-c`, `dpkg-deb -x paket.deb <Probeordner>`, dann den Server daraus mit `HOME` und `XDG_*` auf leeren
  Ordnern starten (Datenordner vorher per `import` anlegen, Verbindungen auf `127.0.0.1:9`, siehe „Bauen und prüfen“).
  Den Starter `baruch` ohne Argumente nie starten: Er nimmt Port 8765 und die Vorgabe-Verbindungen.
- **AppImage lokal:** `packaging/appimage/build-appimage.sh` lädt appimagetool und die Laufzeit mit Prüfsumme nach
  `target/appimage/tools/` (keine Systeminstallation); `--appdir-only` kommt ohne Netz aus. Probeordner und Abbilder
  gehören nach `/home` (z. B. `~/.cache/baruch-runde/`), nicht ins RAM-Dateisystem `/tmp`: die Binärdatei hat 340 MB.
- **Kreuzprüfung für Windows/macOS** scheitert lokal an der ersten C-Abhängigkeit (`zstd-sys`, kein Kreuzcompiler).
  Was ohne C geht: `cargo check --target <Ziel> -p keyring` und `-p baruch-epub`.
- **Umgebungsvariablen im Testprozess nicht umstellen** (alle Server-Tests teilen einen Prozess): Zweige, die an
  einer Variablen hängen (`APPIMAGE` auf der Über-Seite), über eine reine Funktion mit Parameter prüfen oder von Hand.

### ci

- **Vor dem Commit:** `nice -n 10 ionice -c3 cargo clippy --workspace --all-targets -j 4 -- -D warnings` ist grün
  (die CI behandelt Warnungen als Fehler; ein `#[allow(clippy::…)]` braucht eine Begründung), dazu
  `cargo fmt --all -- --check`, `cargo deny check licenses sources bans` und `scripts/ci-selftest.sh` (ohne Netz:
  Hilfsskripte und Regeln der Arbeitsabläufe).
- **Festgeschriebene Prüfsummen:** `crates/baruch-rag/tests/port.rs` hält die SHA-256 von `baruch-rag/src/lib.rs` und
  `Cargo.toml` fest. Wer eine der beiden Dateien ändert (auch nur ein `#[allow]`), zieht die Konstante im selben
  Commit nach (`sha256sum`).
- **Arbeitsabläufe ändern:** Aktionen nur mit 40-stelligem SHA (`git ls-remote <Repo> 'refs/tags/<Tag>*'`), Werte und
  Geheimnisse nur über `env`, nie `${{ … }}` in `run:`; `scripts/ci-check-workflows.sh` prüft die Regeln. Werkzeuge
  mit Version und Prüfsumme stehen in `scripts/ci-tool.sh`. `actionlint`/`shellcheck` fehlen auf dem Rechner.
- **Kein Arbeitsablauf läuft ohne den Nutzer:** kein Tag, kein Push, kein „Run workflow“.
- **Archivschlüssel und Fingerabdruck (Nachbesserung):** `secrets.APT_GPG_*` nur in einem Job mit
  `environment: release` (Regel 10 in `scripts/ci-check-workflows.sh`). Der Fingerabdruck steht in `site/hugo.toml`,
  `README.md` und `docs/Verteilung.md`; `scripts/ci-fingerprint.sh` prüft die drei Stellen (Exit 3 = noch offen) und
  mit `--key` gegen den Schlüssel. Nie einen Fingerabdruck eintragen, raten oder aus dem Netz holen – auch nicht
  „zum Testen“; Tests nehmen erfundene Werte in einem Temp-Ordner (`--root`).
- **Zusicherungen auf Platzhalter:** Ein Test darf einen Wert, den der Nutzer später einträgt, nicht auf „leer“
  festnageln (`round24_abnahme` tat das mit `aptFingerprint` und wäre beim Eintragen gescheitert): „leer oder
  gültig“ prüfen.
- **Nichts Privates oder Fremdes im Baum:** Rechnername, Anmeldename, Name und Buchtitel Dritter gehören in keine
  Datei; `round24_abnahme::the_current_tree_names_no_private_machine_and_no_third_party` sucht danach (Suchwörter
  im Test zusammengesetzt). Beispieltitel in Tests und Doku: das erfundene „Mövenoog“.

### webseite

- **Hugo:** `hugo --minify --panicOnWarning -s site` (0.168.0, nicht „extended“ – kein SCSS). `languageCode` ist seit
  Hugo 0.158 veraltet und bricht mit `--panicOnWarning` ab; es steht `locale = "de"`. Ausgabe `site/public/` ist
  ignoriert. Die Seite kommt ohne JavaScript und ohne fremde Adressen für Stil, Schrift oder Bild aus.
- **Bildschirmfotos der Webseite:** `site/tools/bildschirmfotos.py --pruefen` (ohne Firefox) bzw. `--fotos` (nur
  Integrator, nie parallel zu Testläufen); erfundenes Werk `site/tools/beispielwerk.typ`.
- **Proxy im Test nachstellen:** Anfrage an den Router mit der `Host`-Kopfzeile, die der Proxy durchreicht, und
  `ConnectInfo` Loopback (`round24_betrieb::{open_allowing, mcp_ping}`); `AppState::allowed_hosts` nach dem Öffnen
  setzen. Namen erfunden (`*.example.org`).
- **Fremde Annahmen nachlesen:** „`PGSSLROOTCERT` lässt sqlx auch bei `require` prüfen“ stand im Backlog und im
  Vertrag und war falsch (sqlx 0.8.6 prüft nur bei `verify-ca`/`verify-full`, Quelltext unter
  `~/.cargo/registry/src`). Vor dem Dokumentieren den Quelltext lesen.

### Integration

- **Tests, die an einer Vorgabe hängen:** Die leere MultiGPT-Vorgabe ließ `round12_lancedb` scheitern (erwartete
  einen geöffneten Weg „mit der Vorgabe-Adresse“). Ändert eine Aufgabe eine Vorgabe, vorher
  `grep -rn "<Funktion>" crates/*/tests` und die Stellen im Bericht nennen.
- **`cargo fmt` nach dem Zusammenführen:** Neue Dateien der Zweige waren nicht durchgehend formatiert; der Integrator
  formatiert einmal (`cargo fmt --all`) vor clippy und Testlauf.
- **Webseite fotografieren:** `site/public` über `python3 -m http.server --bind 127.0.0.1 <Port>` ausliefern (die
  Verweise der Seite sind absolut ab `/`). Dunkles Thema: eigenes Firefox-Profil, in `user.js`
  `user_pref("layout.css.prefers-color-scheme.content-override", 0);` **und** `user_pref("ui.systemUsesDarkTheme", 1);`
  (die erste Zeile allein blieb headless hell); schmal: `--window-size=420,2600`. Bilder mit `loading=lazy` fehlen
  im Foto (es entsteht beim `load`-Ereignis): für das Foto eine Kopie von `site/public` nehmen und dort
  `loading=lazy decoding=async` entfernen. Dieselben Einstellungen wirken auch auf Baruch selbst (ohne Thema-Cookie
  folgt die Oberfläche dem System).
- **Interne Verweise der Webseite prüfen:** über die gebauten HTML-Dateien (jedes `href`/`src` ohne fremden Rechner
  muss als Datei unter `site/public` liegen, Anker als `id`); `/apt/` entsteht erst beim Release.

## Runde 33 – Menüband und Laptop-Modus

### Band (`templates/base.html`, `src/band.rs`, `web/src/band.ts`)

- **Das Band steht vollständig in `base.html`.** Die Vorlage ist in rund 35 eigenen minijinja-Umgebungen eingebettet
  (`include_str!`); ein `{% include %}` darin müsste in allen registriert werden. Neue Teile des Bands also direkt in
  `base.html`, Seiten liefern nur Blöcke (`band_tab`, `page_actions`, `band_einfuegen`, `band_pruefen`, `band_buch`,
  `band_ansicht`).
- **Wörtliche Formen:** Die Schicht `consent::inject` → `band::apply` ersetzt im fertigen HTML Zeichenketten (Reiter,
  Feld, `… data-band-group open>`, Laptop-Schalter, Seitenleisten). Reihenfolge der Attribute nie ändern; weitere
  Attribute einer Gruppe nur zwischen `class="band-group"` und `data-band-group`.
- **Neue Seite:** `{% block band_tab %}` setzen und die Aktionen als Gruppen in der wörtlichen Form in den Block des
  eigenen Reiters. `round33_seiten::every_page_names_its_default_tab_and_keeps_its_actions_in_groups` hat die Liste.
- **Modi nur über Attribute an `<html>`** (`data-laptop`, `data-compact`, `data-narrow`, `data-rail-*`, `data-band`):
  keine Media-Query auf die Fensterbreite, sonst gibt es zwei Wahrheiten, sobald der Mensch „An“/„Aus“ wählt.
  `band.css` gehört das Band in beiden Zuständen, `laptop.css` alles außerhalb.
- **Werk im Band:** Rangfolge `topbar` → `work.id` (nur wenn `work` eine Abbildung ist) → Cookie `baruch_work`. Seiten,
  die `work` als Zeichenkette reichen (z. B. `/einfaelle`), zeigen Prüfen/Buch nur mit dem Cookie – Tests rufen solche
  Seiten wie der Browser mit `baruch_work=W` ab.
- **Testhelfer:** `band(html)`, `panel(head, id)`, `tabs(head)` (round33_menueband.rs, round9_menu.rs); der Kopf heißt
  `header.band`, nicht mehr `header.topbar`.

### Integration

- **Tests in fremden Dateien:** Jede der drei Aufgaben hinterließ rote Tests in Dateien einer anderen Aufgabe (alte
  Beschriftung „Satz“, entfernte `.topbar`-Regeln, früheres Band der Satz-Seite). Das ist bei striktem Dateibesitz
  erwartbar: Die Entwickler liefern den Textvorschlag (Datei, Stelle, alt → neu), der Integrator trägt ihn ein. Ein
  Test über „alle Gruppen der Seite“ muss nach dem Zusammenführen die Gruppen aus `base.html` kennen (Gruppe
  „Darstellung“ mit dem Thema-Knopf).
- **Reihenfolge:** Sichtprüfung vor dem abschließenden vollen Lauf (aber nie gleichzeitig mit einem Lauf): Sie fand
  drei Dinge, die CSS änderten (gekappte Gruppen-Beschriftung, Umbruch der Zeile 1 bei 1024 px, „Einklappen ›“ am
  Rahmen). Nach dem vollen Lauf wären das ein zweiter Lauf gewesen.
- **Firefox headless ohne Klick:** `--screenshot` kann nichts anklicken und keine Cookies setzen. Hilfe: ein kleiner
  Vermittler (Python, `http.server` auf 127.0.0.1:0) vor dem Wegwerf-Server, der je Anfrage Cookies ergänzt
  (`Cookie` an den Server **und** `Set-Cookie` an den Browser – das Thema-Widget liest `document.cookie` und stellt
  sonst zurück), `<details … open>` setzt (ohne JavaScript) oder ein `<script>` einfügt, das nach `load` ein
  `summary` anklickt (mit JavaScript; band.ts klappt serverseitig geöffnete Gruppen im kompakten Band wieder zu).
  Ohne JavaScript: Profil mit `user_pref("javascript.enabled", false);`. Schublade ohne JavaScript: Adresse mit
  `#pane-tree`.
- **podman langsam:** `timeout 20 podman info` antwortete, die pgvector-Tests brauchten trotzdem über zehn Minuten
  (Start und `podman rm`). Nicht abbrechen – der Lauf endet; nur bei hängendem `podman info` auslassen.
- **Parallele Runden:** Runde 33 lief neben Runde 32 auf einem eigenen Integrationszweig (`runde33`, Hauptordner
  `.claude/worktrees/r33`). Der Name weicht von der Regel `team/<id>` ab; die Stellen für das spätere Zusammenführen
  sind in `base.html` mit `{#- R32-PLATZ … -#}` markiert (import-word, beenden, export-word).

## Runde 32 – Word und Beenden

### Word (`crates/baruch-docx`, `docx_import`, `docx_export`)

- **Zwei Seiten, ein Modell:** Leser und Schreiber entstanden getrennt gegen `model.rs` und `ooxml.rs`. Was beide
  wirklich verbindet, zeigt nur die Rundreise: `crates/baruch-docx/tests/roundtrip.rs` (Modell → Datei → Modell) und
  `round32_abnahme` (Werk → Datei → Werk). Wer an `write/**` oder `read/**` etwas ändert, lässt beide laufen.
- **Testdateien:** `crates/baruch-docx/tests/data/import/mod.rs` baut eine frei erfundene .docx als ZIP aus
  XML-Zeichenketten (`sample()`, `with_body(…)`, Helfer `p`, `r`, `ins`, `del`, `list_item`, `drawing`, `cell`); die
  Server-Tests binden sie einmal ein (`round32_docx_import.rs`: `#[path = "…/data/import/mod.rs"] pub(crate) mod docx;`,
  andere Dateien mit `use crate::round32_docx_import::docx;` – zweimal `#[path]` lehnt clippy als `duplicate_mod` ab).
  Kein echtes Word.
- **Listen in Typst:** `baruch-wysiwyg` liest „3.“ gefolgt von „+“ als zwei Listen (Typst selbst als eine). Wer Typst
  erzeugt, das der Editor und der Word-Export als eine Liste sehen sollen, schreibt bei einem Beginn ≠ 1 jede Nummer
  aus („3.“, „4.“ …).
- **Bildpfad:** Der Schreiber legt den Pfad im Werk in `wp:docPr name` ab; der Leser nimmt ihn nur, wenn er wie ein
  Pfad aussieht (`read/body.rs`, `work_path`). Word nennt Zeichnungen „Grafik 1“ – dann gilt `bilder/<Dateiname>`.
- **Datei ansehen ohne Word:** `soffice -env:UserInstallation=file://<eigener Ordner> --headless --convert-to pdf
  --outdir <Ordner> datei.docx`, dann `pdftoppm -r 70 -png`. Das ist kein Firefox und kein Fenster; es zeigt, ob
  LibreOffice die Datei öffnet und wie Änderungen, Fußnoten, Listen und Tabellen ankommen. Ersetzt nicht Word.

### Beenden (`shutdown.rs`)

- **Signale früh einrichten:** `tokio::signal` richtet einen Empfänger erst beim Aufruf von `signal(…)` ein, eine
  `async fn` sogar erst beim ersten Warten. `system_signal()` richtet darum beim Aufruf ein, und `main` ruft es vor
  der Zeile „Baruch läuft“. Ein Test, der gleich nach dieser Zeile `SIGTERM` schickt
  (`sigterm_ends_the_throwaway_server_with_exit_0`), fand die Lücke; mit einer Schreibanfrage davor blieb sie
  verborgen.
- **Echter Prozess im Test:** `Process` in `round32_beenden.rs` und `round32_abnahme.rs` – Datenordner im Testprozess
  anlegen, Verbindungen `eliezer`/`multigpt` auf `http://127.0.0.1:9`, `pool().close()`, dann
  `CARGO_BIN_EXE_baruch-server serve --listen 127.0.0.1:0 --secrets memory --data <Temp>`; Adresse aus „Baruch läuft: “
  lesen; beenden nur über die eigene PID (`kill -TERM <pid>`), `Drop` räumt auf.
- **`Host` im Test:** `reqwest` schickt einen eigenen `Host`-Kopf (`.header("host", NAME)`) – so prüft man einen
  erlaubten Proxy-Namen am echten Prozess.
- **Wächter `round33_seiten`:** Jede Vorlage, die `base.html` erweitert, braucht eine Zeile in `TEMPLATES`. Wer eine
  Seite baut, liefert die Zeile im Bericht mit (die Datei gehört dem Integrator).

### Integration

- **Sichtprüfung:** Eine Seite, die nur als Antwort auf `POST` entsteht und nichts nachlädt (Abschlussseite), per
  `curl -X POST -H "Origin: <Adresse>"` holen und die Datei fotografieren. Seiten mit Stylesheets lassen sich nicht
  von `file://` aus fotografieren (der Schutz gegen fremde Herkunft lehnt die Stylesheets ab) – dafür den Vermittler
  aus Runde 33 nehmen (hier: Menü „Baruch“ mit `open`).
- **`.work-form` hat eine Grundhöhe von 320 px** (`works.css`): Eine Seite mit nur einem kleinen Formular setzt
  `flex: 0 0 auto` (wie `assets.css`, `docx_export.css`), sonst bleibt unter dem Knopf ein leerer Kasten.

## Runde 34 – Mindmap

- **Wo was liegt:** Daten `crates/baruch-store/src/ideas.rs` (Migration 0052: `title`, `parent_id`, `position`,
  `main_topic`), Baum/Wolke/Import `src/idea_map/tree.rs`, Bild `src/idea_map/bild/{mod,mindmap,tree_layout}.rs`,
  Ein-/Ausgabe und Gliederungsansicht `src/idea_map/files/`, Widget `web/src/mindmap.ts`, Vorlagen
  `karte.html`, `karte_gliederung.html`, `karte_einlesen.html`.
- **Vier Plätze eines Einfalls:** Elternknoten → Unterknoten; sonst Bezug → am Kapitel; sonst `main_topic` →
  Hauptthema am Werk; sonst Wolke. `idea.create` ohne Platz legt in die Wolke; „aufs Werk“ braucht `cloud: false`.
- **Seite, Bild, Ausgaben lesen `tree::Scene`** (`tree::scene`), nie `tree::tree` allein: Sonst fehlt die Wolke.
  `mindmap::render` bekommt die Wolke über `RenderOptions::of(scene, …)`. Wer einen Baum von Hand baut
  (`round34_widget.rs`, `round34_austausch.rs`), nennt die Kennungen der Wolke selbst.
- **Annahmen in parallelen Zweigen prüft erst die Naht:** Zwei Zweige hatten gegen die Schnittstelle dieselbe
  Annahme über die Wolke getroffen, der dritte hat sie anders gebaut; alle drei Läufe waren für sich grün. Beim
  Zusammenführen deshalb zuerst die Berichte nach „Annahme“ durchsuchen und einen Ende-zu-Ende-Test über die Naht
  schreiben (`round34_abnahme.rs`), bevor fremde Tests angepasst werden.
- **Widget ohne Browsertest:** `mindmap.ts` läuft in keinem Test. Geprüft wird, welche RPC es mit welchen Feldern
  ruft (z. B. `idea.delete` mit `confirm: true`), und das Bild danach. Wer am Widget ändert, gleicht die Parameter
  mit `ideas.rs` ab und macht eine Sichtprüfung mit **Probe**: Der Vorschalt-Server (siehe „Fotos nach einem Klick“)
  fügt ein Skript ein, das Ereignisse schickt – `click` auf `.mm-box`, `keydown` **an den gewählten Knoten**
  (`.mm-node.mm-selected`; auf `document` hört die Mindmap nur für Strg+Z, Alt+E, Alt+L), Ziehen als `pointerdown`
  auf dem Knoten und `pointermove`/`pointerup` auf `window` (Ziel vorher mit `scrollIntoView` ins Fenster holen,
  sonst findet `elementFromPoint` nichts) – und das Ergebnis per `fetch` zurückmeldet; den Stand danach über RPC lesen.
- **Fotos der Mindmap:** `/werke/{id}/karte` (mit `zu=<Kennung>` für einen eingeklappten Ast), Gliederungsansicht
  `/werke/{id}/karte/gliederung?wahl=i:<id>`; die Vorschau des Einlesens ist eine POST-Antwort (per `curl -F file=@…`
  speichern, `<base href>` einfügen, Datei fotografieren).
- **Austauschformat:** Unterversion 9; ein Werk ohne geordneten Baum besteht weiter das eingefrorene u8-Schema.

## Runde 35 – Konten und Rollen

- **Wo was liegt:** Konten/Sitzungen/Sperre `crates/baruch-store/src/accounts.rs` (0053), Freigaben `members.rs`
  (0054); Anmeldung `src/auth/` (`gate` = Schicht, `password`, `session`, `throttle`, `routes`, `rpc`, `cli`);
  Rechteprüfung `src/roles/` (`mod.rs` mit `LEKTOR_ROUTES`, `LEKTOR_RPC`, `check_request`, `check_rpc`; `rpc.rs`,
  `lektor.rs`, `routes.rs`); Seiten und Menüeintrag `src/konten_ui/`; Vorlagen `anmelden.html`, `einrichten.html`,
  `konto.html`, `mitwirkende.html`, `settings/konten.html`, `fragments/lektor_*.html`; Stil `static/konten.css`.
- **Neue Route oder RPC-Methode:** Für Lektoren ist sie gesperrt, bis sie in `roles::LEKTOR_ROUTES` bzw.
  `LEKTOR_RPC` steht – das ist gewollt. Aufnehmen nur, wenn ein Lektor sie braucht: rein lesend oder einer der
  erlaubten Schreibwege, nie etwas, das ein Modell, eine Verbindung oder das Dateisystem anfasst, immer mit
  Werk-Bindung (`WorkFrom`) und Begründung in `docs/Architektur.md`. `round35_schreibwege` und `round35_abnahme`
  zählen Routen und Methoden aus dem Quelltext (`common::registered_routes`, `common::rpc_method_candidates`) – wer
  eine Route so anmeldet, dass die Hilfe sie nicht findet (kein `.route("…", get(/post(`), umgeht den Wächter.
- **Formulare der Module gehen nicht durch `rpc::call`** (`/beitraege/act`, `/werke/befunde/act` …) und sind darum
  für Lektoren zu. Ein Formular für Lektoren schickt an `POST /lektorat/act` (Feld `method` aus `LEKTOR_RPC`).
- **Wer fragt?** `auth::current()` (task-lokal; außerhalb einer Anfrage `Who::Open`). Nie in `tokio::spawn` oder
  `spawn_blocking` auswerten – vorher holen und mitgeben. Herkunft beim Schreiben: `roles::author_name(&who)`.
  `AppState::default_work()` ist für Lektoren das kleinste freigegebene Werk; `store.default_work()` bleibt die
  kleinste id (nur für Systemzwecke nehmen).
- **Vorlagen:** `nur-autor` ist immer die **letzte** Klasse (`class="btn nur-autor"`), Gruppen des Menübands tragen
  `data-nur-autor`; Stücke nur für Lektoren stehen zwischen `<!--NUR-LEKTOR-->` … `<!--/NUR-LEKTOR-->`, Stücke nur
  mit Konten zwischen `<!--NUR-KONTEN-->` … `<!--/NUR-KONTEN-->`. `konten_ui::apply` räumt ohne Konten alles weg –
  die Seite muss dann bis auf „Konten einrichten…“ wie vorher sein (`round35_oberflaeche`, `round35_abnahme`).
  Verbergen per CSS ist nur Bequemlichkeit; die Sicherheit liegt allein in `roles`.
- **Tests:** Identität ohne Anmeldung über `common::with_who(request, who)` (ganzer Router) oder `auth::scope(who,
  …)` (ein Aufruf); Konten für Fremdschlüssel per SQL mit Hash `$argon2id$test`. Den echten Weg (Einrichten →
  Cookie) nur dort, wo er geprüft wird (`round35_konten`, `round35_abnahme`): `ConnectInfo` mit Loopback **und**
  `Host: 127.0.0.1:…` = eigener Rechner; `Host` = Proxy-Name (`state.allowed_hosts = AllowedHosts::parse(…)`) = über
  den Proxy. Ohne `ConnectInfo` gilt eine Anfrage nie als eigener Rechner. Nie ein Passwort oder Token in einer
  Zusicherungsmeldung ausgeben. Sperre im Test ablaufen lassen: `UPDATE login_attempt SET blocked_until = …`.
- **Annahmen an der Naht:** Die drei Zweige bauten gegen Stubs der anderen (Lektor durch den ganzen Router war in
  keinem Zweig prüfbar). Fehler der Naht fand erst die Sichtprüfung mit echten Konten: „Zurück zum Werk“ auf Seiten
  ohne eigenes Werk zeigte auf das Vorgabewerk. Bei Rollen also jede Seite auch **ohne** `work` und mit einem
  **alten Cookie** `baruch_work` ansehen.
- **Sichtprüfung mit Konten:** Wegwerf-Server wie oben; Konten per `curl` direkt am Server anlegen (`/einrichten`
  von 127.0.0.1, Cookie in eine Datei, danach löschen). Der Vermittler ergänzt je Anfrage das Cookie
  `baruch_session`, reicht `Host` und `Origin` mit der Adresse des Servers weiter (sonst 403 von
  `guard::same_origin`) und schreibt `/rpc` mit Methode und Fehlercode mit – so sieht man, ob ein Widget für Lektoren
  eine gesperrte Methode ruft. Dunkel für die Seiten ohne Menüband (Anmelden, Einrichten): Profil mit
  `user_pref("ui.systemUsesDarkTheme", 1);`. Menü „Baruch ▾“ offen: `<details class="menu" data-menu="baruch" open>`.

## Runde 36 – Reste (Typst-Kommandozeile, sqlx 0.9)

- **Typst-Kommandozeile:** Argumente `crates/baruch-server/src/typst_cli.rs` (`dispatch` läuft in `main` vor der
  Tokio-Laufzeit, `run` schreibt nur in `out`/`err` und ist darum ohne Prozess prüfbar), Satz
  `crates/baruch-render/src/cli.rs`. Ausprobieren: `target/debug/baruch-server typst compile x.typ x.pdf` oder ein
  Symlink `baruch-typst` auf die Binärdatei. Exit 0/1/2 fest (Erfolg / Satz- oder Dateifehler / falscher Aufruf oder
  Nicht-Unterstütztes). Kein Paket legt eine Datei `typst` an (`round36_typst`). CI: `scripts/ci-typst.sh
  <baruch-server>`.
- **sqlx 0.9:** `query`, `query_as`, `query_scalar`, `begin_with` nehmen `&'static str`. Zusammengesetzter Text nur
  über `sqlx::AssertSqlSafe(format!(…))` und nur aus festen Bausteinen des Quelltexts (Regel im Modulkommentar von
  `baruch-store/src/lib.rs`); Werte immer binden. In Tests gilt dasselbe (`sqlx::query(AssertSqlSafe(sql))` für
  einen `String`). `Migrator { … }` als Struktur-Ausdruck braucht `..Migrator::DEFAULT`. Wächter:
  `round36_sqlx` (genau eine sqlx-Version im Lock, keine Ausnahme RUSTSEC-2026-0459 in `deny.toml`),
  `sqlx09_migrations` in Store und Bibliothek (Prüfsummen wie unter 0.8). Vor dem Commit jetzt
  `cargo deny check` mit allen vier Prüfungen (advisories braucht Netz).
- **Zusammenführen eines Umbaus quer durch den Workspace** (sqlx09 zuletzt): erst `development` in den Zweig holen
  und dort `cargo check --workspace --all-targets` (der Compiler meldet jede neue Stelle), dann im Hauptordner
  `git merge --no-ff --no-commit`, voller Lauf, erst danach der Commit – so steht nie ein halber Umbau auf
  `development`.
- **„Dieselbe PDF“ prüfen:** typst-pdf ohne Zeitstempel ist deterministisch; `export --pdf` und
  `baruch-typst compile` auf der Datei aus `export --out` sind Byte für Byte gleich, solange das Werk keinen
  Umschlag und keine Titelei hat (`round36_abnahme`).
- **Lektoren:** Anzeigenamen kommen über `konten_ui::names` in den Text der Herkunfts-Elemente; ohne Konten bleibt
  die Ausgabe Byte für Byte gleich. `settings::resolve_work` lässt das Cookie `baruch_work` für Lektoren nur bei
  freigegebenem Werk gelten.
- **Einmaliger Vermerk im Prozess** (`settings/data.rs`, „Sicherung angelegt“): gilt je Prozess und Datenordner,
  nicht je Sitzung; Tests mit eigenem Datenordner stören sich nicht.

## Runde 37 – Hilfe auf der Webseite

### hilfe_text (`site/content/de/hilfe/`, `docs/Bedienung.md`)

- **Bedienung = Hilfeseite.** Wer Oberfläche ändert, ändert im selben Commit die Seite unter
  `site/content/de/hilfe/` und – für Routen, Cookies, Parameter, Commits – die „Technischen Anmerkungen“ in
  `docs/Bedienung.md`. Regeln stehen dort unter „Regel für Entwickler“; `round37_hilfe` setzt sie durch.
- **Hilfeseite schreiben:** jede Überschrift mit `{#anker}`; jede `##` mit `{{< hilfebild … >}}` oder Eintrag in
  `ohne_bild`; kein rohes HTML und kein HTML-Kommentar (Hugo bricht mit `--panicOnWarning` ab); spitze Klammern nur
  in Code (`` `<name>` ``); Verweise nur ab `/` und nur auf vorhandene Anker; Tasten als „Strg+S (Mac: ⌘S)“ in
  derselben oder der folgenden Zeile. Verboten sind u. a. „Runde “, „Revision“, „Akkordeon“, „Cookie“, „Commit“,
  „RPC“, „RAG“, „docs/“, `` `/ `` (Code, der mit einem Schrägstrich beginnt) und die Routen der Oberfläche.
- **Tests, die Doku lesen:** sichtbare Texte der Oberfläche in der Hilfeseite suchen, Technisches in
  `docs/Bedienung.md`. Überschriften der Hilfe enden mit dem Anker – `"\n## Menüband {#menueband}\n"`, nicht
  `"\n## Menüband\n"`.
- **Mac-Tasten belegen:** `grep -rn 'metaKey\|ctrlKey\|altKey' crates/baruch-server/web/src`. `ctrlKey || metaKey`
  → ⌘; Alt-Kürzel gehen am Mac nur, wenn der Code `event.code` auswertet (⌥ erzeugt sonst ein anderes Zeichen).
- **Worktree-Schutz:** Befehle mit `git` in Pipes oder mit Hier-Dokument lehnt der Schutz ab („too complex“);
  längere Prüfungen als Skript ins Scratchpad legen und einzeln aufrufen.

### webseite (`site/content`, Theme, `scripts/site-check.py`)

- **Feste Anker statt erzeugter:** Hugo bildet Anker aus der Überschrift (`#macos-nicht-signiert` gab es nie,
  weil die Überschrift „macOS“ hieß). Jede Überschrift bekommt `{#anker}`; `scripts/site-check.py` findet tote
  Anker nach dem Bau.
- **Parallel zugesagte Anker prüfen:** Verweist ein Zweig auf Anker, die ein anderer erst liefert, eine Kopie
  von `site/` im Scratchpad mit erfundenen Abschnitten samt diesen Ankern bauen und dort prüfen – so ist belegt,
  dass auf dem eigenen Stand nur das Zugesagte fehlt.
- **Hugo, Abschnittsvorlagen:** `layouts/hilfe/page.html` gilt nur für Seiten unter `content/de/hilfe/`;
  `layouts/section.html` für die Startseite des Abschnitts. Das Inhaltsverzeichnis nur zeigen, wenn es Einträge
  hat: Bedingung `in .TableOfContents "<li"` (auf den Platzhalterseiten ohne Überschrift erscheint so kein Kasten).
- **Menüeintrag auf Unterseiten markieren:** `IsMenuCurrent` trifft nur die Seite selbst; für die Unterseiten
  eines Abschnitts `$seite.IsDescendant .Page` am Menüeintrag (nur wenn `.Page.IsSection`).
- **Die Sandbox lehnt Heredocs mit bestimmtem Inhalt ab** („too complex to verify“): Dateien mit dem
  Schreibwerkzeug anlegen, längere Ersetzungen als Python-Skript im Scratchpad ablegen und ausführen.
- **`ci-selftest.sh` baut Probe-Seiten ohne Abschnitt Hilfe:** Prüfungen, die die echte Seite voraussetzen,
  brauchen einen Schalter (`site-check.py --ohne-hilfe`, von `ci-site.sh` gesetzt, wenn `content/de/hilfe` fehlt) –
  und eine Gegenprobe, dass die Prüfung mit dem Abschnitt wirklich greift.

### bilder (`site/tools/bildschirmfotos.py`, `site/data/hilfebilder.toml`)

- **Befunde ohne Modell gibt es nur über ein Lektor-Konto:** `finding.add` und `revision.propose` sind den Lektoren
  vorbehalten (-32000 für alle anderen). Für Fotos also: Konten zuletzt anlegen, als Lektor anmelden, Befunde
  schreiben; die Bilder mit Befunden zeigen dann die angemeldete Autorin (Sitzung über den Vermittler).
- **Beiträge und Vorschläge einer Assistenten-Rolle ohne Modell:** MCP am eigenen Server von 127.0.0.1 ohne Token,
  `add_post` bzw. `propose_revision` mit `persona: "lektor"` → Autor `@lektor`. `thread.post` und MCP `chat` rufen
  kein Modell, solange die Nachricht keinen Assistenten erwähnt (`answers` und `errors` bleiben leer).
- **Wegwerf-Server und Bibelmodule:** Ohne gespeicherte Liste liest Baruch `$SWORD_PATH`, `~/.sword` und
  `/usr/share/sword` – also die Module des Nutzers. Für Fotos gleich nach dem Start `bible.set_dirs` auf einen
  leeren Ordner (mit `mods.d`) setzen.
- **Vorschauen des Einlesens sind teils Seiten, teils Antworten:** `POST /werke/import/word/vorschau` antwortet mit
  303 auf die Seite des Entwurfs, `POST /austausch/vorschau` (Dateifeld `datei`, nicht `file`) und
  `POST /werke/import/quelle/vorschau` mit 200. Der Bericht nach `POST /werke/import` erscheint nur, wenn es etwas
  zu berichten gibt (etwa ein fehlendes Bild in einer Markdown-Datei), sonst 303 auf das Werk.
- **Vermittler für Fotos als Teil des Werkzeugs** (`site/tools/bildschirmfotos.py`, Klasse `Relay`): Rezept je
  Aufnahme über `_bild=<Aufnahme>` in der Adresse; Sitzung nur zum Server hin; `Set-Cookie` für die übrigen Cookies
  auch an den Browser; Host/Origin/Referer umschreiben (Gegenprobe: dieselbe Anfrage direkt → 403). Ohne Firefox
  prüfbar ist alles außer der Wirkung von Klick und Messung.
- **Dunkel im HTML prüfbar:** Mit Cookie `baruch_theme=dark` trägt `<html>` `data-theme="dark"`.
- **Worktree-Schutz und Shell:** Zusammengesetzte Befehle mit Hier-Dokument lehnt der Schutz als „zu komplex“ ab;
  Hilfsskripte mit dem Schreib-Werkzeug ins Scratchpad legen (Name mit Aufgaben-ID) und einzeln aufrufen.
- **Attrappe statt Firefox:** Wer `--fotos` ohne Firefox proben will, prüft vorher mit `command -v firefox`, dass
  die Attrappe im Pfad vorn steht **und ausführbar ist** – `shutil.which` überspringt nicht ausführbare Dateien und
  findet dann den echten Browser.
- **Vorlagen maskieren Schrägstriche:** minijinja schreibt eingesetzte Adressen als `https:&#x2f;&#x2f;…`; Tests,
  die einen Link mit eingesetzter Adresse suchen, ersetzen vorher `&#x2f;` durch `/`.
- **`| head` an einem Python-Werkzeug:** `BrokenPipeError` abfangen, sonst endet `--liste | head` mit einer
  Fehlermeldung.

### Integration

- **Zusammenführen:** Nach der Schnittstelle S1 gab es keine gemeinsame Datei; drei `--no-ff` ohne Konflikt. Gab es
  einen Zweignamen schon (`team/bilder` aus Runde 21c, `team/webseite` aus Runde 24), vorher nachsehen, welcher
  Zweig der Runde gehört (`git worktree list`, `git merge-base`).
- **Fotos zeigen Pfade:** Einstellungen → Daten, → Quellen und „Über Baruch“ nennen Datenordner, Modulordner und
  die Vorgabe unter `$HOME`. Für Bilder, die veröffentlicht werden: `--temp /var/tmp/baruch-hilfe` (Platte, neutral),
  Bibelmodule dorthin kopieren; das Werkzeug bricht `--fotos` sonst ab und gibt dem Wegwerf-Server ein eigenes
  `HOME`. Nach den Fotos jedes Bild ansehen (Kontaktbögen mit `montage`) und die PNG-Dateien mit `grep -a` nach dem
  eigenen Namen durchsuchen.
- **`target/release/baruch-server` kann alt sein:** Das Werkzeug nahm ihn zuerst und fotografierte eine alte
  Oberfläche (hier: 404 beim Word-Export). Seit dem PM-Abschluss der Runde 37 nimmt es ohne `--server` die zuletzt
  gebaute Datei und nennt sie; ob sie zum Quelltext passt, prüft es nicht – nach Änderungen erst bauen.
- **`firefox --headless --screenshot`:** nimmt den Seitenanfang auf, ein Anker in der Adresse ändert nichts (hohes
  Fenster plus gemessener Ausschnitt nehmen); Bilder mit `loading="lazy"` fehlen im Foto, obwohl sie im Browser
  kommen – für die Sichtprüfung der Webseite eine Kopie von `site/public` ohne `loading=lazy` fotografieren.
  Dunkel: `ui.systemUsesDarkTheme = 1` in `user.js` des Wegwerf-Profils.
- **alt-Texte nach den Fotos gegenlesen:** Sie entstehen vor den Bildern. Abgewichen waren Leerstände (Abgleich,
  Querverweise, MCP-Zugang ohne Token), der Zeitwächter (Buch statt Predigt) und alles, was ein Ausschnitt nicht
  zeigt.
- **minijinja:** `{{ wert | default('') }}` ersetzt nur fehlende Werte; ein `Option::None` wird als „None“
  ausgegeben. Für Formularfelder `default('', true)`.
- **Zitat und Stelle:** Die Wortlautprüfung ordnet ein Zitat nur zu, wenn zwischen Zitat und Stelle bloß Leerraum,
  Klammern und Satzzeichen stehen – `„…“ (#bibelstelle[Psalm 119,105])` wird nicht zugeordnet, `„…“ (Psalm 119,105)`
  schon.
- **Testziel:** `cargo test -p baruch-server --test server …` (nicht `--test main`; `Cargo.toml`: `name = "server"`,
  `path = "tests/main.rs"`).


## Round 38 – English, comment translations, work language

New sections of this file are written in English; the sections above are still German.

- **Comment-only changes:** `python3 scripts/check-comments-only.py BASE HEAD [PATH…]` compares every changed file
  without its comments (Rust, TypeScript, templates, Python, shell) and exits 0 only if nothing but comments
  changed; doc-test code blocks and intra-doc link targets must stay equal. `--umlaute PATH…` lists German letters
  left in comments; exceptions go into `scripts/comment-umlaut-allow.txt` (glob, substring, reason, separated by
  TAB; the integrator maintains it). `--selftest` runs in `scripts/ci-selftest.sh`.
- **Merging a comment branch:** first merge `development` into the branch. In a conflict take code and strings from
  `development` and the English comment from the branch, then run the check between `development` and the branch
  (exit 0) before merging with `--no-ff`. A doc line that starts with an intra-doc link and a colon
  (`` [`NAME`]: text ``) is not a link definition; the check knows that since the integration of Round 38.
- **List continuation in doc comments:** keep the list structure (`- `, `1. `) of a doc comment when translating;
  clippy `doc_lazy_continuation` fails otherwise.
- **Work language in tests:** set it with `store.set_work_language(work, WorkLanguage::En)`; the model is
  `assist::mock::ScriptedModel`. Assert the prompt through `lang::language_rule(language)` and the markers through
  `lang::markers(language)`, never through copied wording. Tests with German model answers must keep their answer
  texts: for `de` every behaviour is as before Round 38.
- **Exchange schema:** tests that validate current output use `docs/schema/baruch-werk-1-u10.xsd`; the older
  schemas are frozen (`crates/baruch-exchange/tests/xsd.rs`).
