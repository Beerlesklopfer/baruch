# Baruch – freie Schreibwerkstatt für Bücher und Predigten

## Context

Die Bücher des Autors werden heute über MultiGPT-Chats überarbeitet (Chat 59): PDF anhängen,
Kritik holen, Kapitel einzeln nacharbeiten. Dabei geht Struktur verloren, Kontexte laufen über
(Eliezer HTTP 400, behoben in `958b796`/`e37814d`), und es gibt keine Revisionen je Abschnitt.

Entscheidungen des Nutzers (2026-11-05):
- **Neues Repo „Baruch“** (privat `Beerlesklopfer/baruch`, später frei), Redesign, nicht in
  MultiGPT integriert; zunächst undokumentiert.
- ~~Rust, native Desktop-Oberfläche (Iced)~~ → **Rust-Server mit Browser-Oberfläche wie David**
  (2026-11-05): Der Kern ist ein RPC-Server, der Gerüst und Vorlagen ausliefert; Widgets in TypeScript.
  Loco (Rails-artig, axum + SeaORM + Tera, Worker, JWT) geprüft und vorerst verworfen: RPC fehlt dort
  auch, der Ballast passt nicht zum Einzelplatz; für die Bibelschule (Anmeldung, Worker) später neu prüfen.
- **Import über MCP zu MultiGPT** (2026-11-05): Baruch liest Quellen genauso wie MultiGPT (gleiche
  Aufbereitung, wichtig für die Recherche); keine eigenen Import-Parser. PDF, EPUB, MOBI usw. müssen in
  Baruch darstellbar sein.
- **Literaturverwaltung wie Citavi** (2026-11-05): Quellen aus RAG und Web als Titel mit Zitaten;
  Belege im Text über Typst-`@cite` (Kandidat: `hayagriva`, das Literaturformat von Typst, mit CSL-Stilen).
- **SQLite bleibt die Quelle** (2026-11-05, bestätigt): Werke, Revisionen, Chat, Personas und die ganze
  **Autorenbibliothek** (Titel, Zitate, Belege) liegen dort; das LLM sucht darin (Werkzeug für die
  Personas, Volltext über FTS5, Bedeutung über LanceDB). LanceDB bleibt reiner Suchindex.
  *Umgesetzt (B1):* `<data>/baruch.sqlite` mit Kernschema (`baruch-store`) und Autorenbibliothek samt
  FTS5-Suche (`baruch-library`); seit Runde 12 LanceDB als Bedeutungssuche (Index im Datenordner).
- **Git-Revisionssystem: entschieden und umgesetzt** (2026-11-05, B1): SQLite bleibt die Quelle, Git
  spiegelt. Jede Übernahme und jede eigene Speicherung wird ein Commit im Typst-Ordner des Werks
  `<data>/works/<slug>/` (`baruch-gitmirror`, reines Rust über gix): Autor ist der Mensch bzw. die
  Persona, Committer der Mensch, Persona und Modell stehen als Trailer im Commit. Kein Commit ohne
  Änderung; scheitert der Spiegel, bleibt die Änderung gültig und der nächste Commit holt den vollen
  Stand nach. Der Ordner lässt sich klonen, pushen (Forgejo/GitHub, nur durch den Nutzer) und mit
  `typst compile main.typ` setzen. Offen bleibt nur das Einlesen eines bestehenden Repos mit Historie.
- **Frei (AGPL-3.0)** – bewusst als Gegenentwurf zu bezahlten Werkzeugen wie Logos.
- Bildschirm: **links Kapitelbaum, Mitte Editor mit Revisionen je Abschnitt („‹ ›“), rechts
  Assistenten.**
- Assistenten schreiben **nur Vorschlag-Revisionen**; aktuell wird eine Revision erst durch Klick.
  **Revisionsbaum (Runde 3, umgesetzt):** je Abschnitt genau eine aktuelle Fassung als Markierung, jederzeit
  umschaltbar; Ändern und Löschen nur durch den Menschen, protokolliert im Git-Spiegel. Import nur über die
  Kommandozeile (`baruch-server import`), nicht über MCP (Entscheidung 2026-11-05).
  **Ausnahme Einfälle (IDEA_CRUD, Runde 7 umgesetzt):** Einfälle ändern keinen Buchtext; Assistenten und
  MCP dürfen sie anlegen, ändern und löschen, jede Aktion mit Herkunft als Commit im Spiegel.
  **Ausnahme Beiträge (Runde 8):** Beiträge sind ebenfalls kein Buchtext; der Mensch verschiebt, löscht und
  wandelt alle um, Assistenten und MCP nur ihre eigenen (Autor = Persona bzw. `@mcp:<name>`), je ein Commit. Offene
  Vorschläge stehen im Lesefluss in der Farbe ihrer Persona (Runde 7). Titel von Gliederungspunkten sind
  nie leer, einzeilig und höchstens 200 Zeichen lang (Runde 7).
  **Ausnahme Gliederung (Wunsch 2026-11-06, umgesetzt):** „Ein LLM hat den besseren Überblick durch seinen
  Context“ – Assistenten dürfen Gliederungspunkte **anlegen** (MCP `outline_add`, auch eine ganze Gliederung in
  einem Aufruf und einem Commit; Revision 1 und Commit tragen die Persona bzw. den Client). **Umbenennen,
  Verschieben, Umstufen, Nummer an/aus und Löschen nur nach Bestätigung durch den Menschen** („UD nach
  Bestätigung“): Assistenten stellen einen Antrag (Migration 0012), der Mensch bestätigt oder lehnt ab
  (Gliederungsseite, RPC `outline.request_decide`); veraltete Anträge werden mit Grund abgeschlossen.
- **Alle Personas sind miteinander verbunden**; die Persona mit dem **größten Kontext ist
  automatisch Lektor**.
- Satz und **PDF-Vorschau mit Typst** wie im bestehenden Build der Bücher.
- Import: **Typst-/Markdown-Kapitel, PDF, leer neu**.
- **Bibeltexte und Literatur über MultiGPT-RAG** (keine mitgelieferten Bibeln, keine Lizenzfragen).
- Zweiter Anwendungsfall gleichrangig: **Predigten**.

Name: Baruch, der Schreiber Jeremias, der die verbrannte Rolle neu und erweitert niederschrieb
(Jer 36,32) – Revisionen sind der Kern.

## Vorbilder (Ideen, kein Code)

**Elisa** (`Beerlesklopfer/elisa`): Personas als bearbeitbare Datensätze mit Modell je Persona
(`LLMRoleDescription`, `context_for()`); Regel „LLM verfasst keinen verbindlichen Inhalt“ mit
Herkunft je Maschinenbeitrag (`generated_by_llm`, `llm_model`); Revisionslog nur anhängend
(`app/history.py`); stabile Gliederungsnummern (`app/outline.py`); Chat je Abschnitt für
Begründungen; Auditor Nathan (Befunde, nie Freigabe); Lektor-JSON-Vertrag (`write-time-feedback`);
Zitierpflicht (`app/grounding.py`); Personas sprechen über einen Relay miteinander
(`list_personas`, lokale fragen externe um zweite Meinung).

**MultiGPT:** MCP-Werkzeuge `ask` (Modell, Kosten, Budget), `search_documents`, `upload_document`,
`read_document`, `list_models`; Kontextlänge je Modell (`context_budget.context_length`);
Versionswechsler (`chat/templates/chat/_message_actions.html`) als Bedienmuster.

## Architektur

- **Server mit Browser-Oberfläche** (wie David): `baruch-server` auf axum 0.8, Seiten und Fragmente als
  minijinja-Vorlagen, Kern über JSON-RPC 2.0 (`POST /rpc`, u. a. `ui.render`, `revision.accept`);
  Widgets in TypeScript (`data-widget`, von `build.rs` mit `tsc` übersetzt und eingebettet); ohne
  JavaScript bleibt alles über Links und Formulare (`POST /act`) bedienbar. Typst-Vorschau später als
  Seitenbilder (`typst-render`) bzw. SVG. Kern als Bibliothek (`baruch-core`). Lokal auf dem Laptop
  oder auf dem Schulrechner betrieben.
- **Daten lokal in SQLite** (sqlx, Migrationen), eine Datei je Datenordner (`<data>/baruch.sqlite`),
  einschließlich der Autorenbibliothek (Literatur, Zitate, Belege); für Personas als Suchwerkzeug
  freigegeben (RPC `library.search`). Regeln sichert die Datenbank selbst: genau eine `current` je
  Abschnitt (Teilindex); Elternteil (`based_on`) nur auf eine frühere Fassung desselben Abschnitts,
  „aktuell“ umschaltbar aus `proposed`/`rejected`/`superseded`, Herkunft unveränderlich, Löschen nie an
  der aktuellen Fassung, Nummern nie wiederverwendet (Trigger, Migration 0004; ersetzt „nur anhängen“ aus
  0001 und die Statusregel aus 0003). Ändern und Löschen nur durch den Menschen (Server, nicht über MCP).
- **Git-Spiegel je Werk** (`<data>/works/<slug>/`, Git-Repo mit `main.typ` und `abschnitte/NNN-slug.typ`):
  Schnappschuss der aktuellen Revisionen nach jeder Änderung; Pfade und Zwischenordner werden geprüft
  (kein `..`, kein `.git`, kein Symlink), damit nichts außerhalb des Spiegels geschrieben oder gelöscht wird.
- **Vektorsuche eingebettet mit LanceDB** (Rust, ohne Server): Texte, Revisionen und Kommentare
  für die Suche der Personas; MultiGPT-RAG zusätzlich für Bibeln und Literatur.
- **Modelle:** jede OpenAI-kompatible Adresse (Ollama, LM Studio, Eliezer, kleine 3B-Modelle
  lokal) oder ChatGPT/Claude mit eigenem Key; MultiGPT optional.
- **Typst eingebettet** (`typst`-Crate): Live-Vorschau als PDF/Seitenbilder neben dem Editor, Export
  PDF; Vorlagen „Buch“ und „Predigt“ mitgeliefert, Schriften gebündelt (offline).
- **Modell-Anbieter steckbar** (`trait Backend`): MultiGPT (MCP, Key je Konto; Kosten/Budget dort),
  OpenAI-kompatibel (Eliezer, Ollama, LM Studio), Anthropic/OpenAI direkt mit eigenem Key. Jedes
  Backend meldet Modelle und Kontextlänge.
- **Wissensquelle steckbar** (`trait Knowledge`): zuerst MultiGPT-RAG (`search_documents`, Bibeln
  und Literatur als Sammlungen); ohne MultiGPT bleibt die Bibelsuche aus und wird so angezeigt.
- Keys im Schlüsselbund des Systems (`keyring`-Crate), nie in der Datenbank im Klartext.
- Oberfläche Deutsch (Englisch vorbereitet, i18n), Bezeichner Englisch, kein Inline-JS.

## RAG-Abgleich mit MultiGPT

Baruch arbeitet offline mit der eingebetteten LanceDB und gleicht sich mit MultiGPT ab, sobald es
erreichbar ist (MCP, Key je Bibliothek).

- **Was abgeglichen wird:** Texte, nicht Vektoren. Die MCP-Schnittstelle liefert keine Vektoren,
  und die Einbettungsmodelle können sich unterscheiden; jede Seite bettet selbst ein (Vorgabe
  `nomic-embed-text`, 768 Dimensionen wie MultiGPT – dann sind Suchergebnisse vergleichbar).
- **Baruch → MultiGPT:** aktuelle Revisionen und Kommentare/Befunde je Werk als Dokumente in eine
  eigene Sammlung („Baruch: <Werk>“) über `upload_document`; ein Dokument je Kapitel, ersetzt bei
  Änderung (`delete_document` + neu). So finden auch MultiGPT-Chats den Stand.
- **MultiGPT → Baruch:** gewählte Sammlungen (Bibeln, Literatur) über `list_documents` /
  `read_document` holen und lokal einbetten; danach geht die Bibelsuche auch offline.
- **Kennung und Änderungen:** je Dokument eine stabile ID (Werk/Kapitel bzw. MultiGPT-Dokument-ID)
  und ein Inhalts-Hash; abgeglichen wird nur, was sich seit dem letzten Lauf geändert hat
  (`sync_state`: Seite, ID, Hash, Zeitpunkt). Bei Gleichstand beider Seiten gewinnt die Quelle:
  eigene Werke gehören Baruch, Sammlungen gehören MultiGPT – kein Zusammenführen.
- **Bedienung:** Knopf „Abgleichen“ und Anzeige „zuletzt abgeglichen …“; Fehler (offline, Budget)
  nur als Hinweis, die Arbeit geht lokal weiter.
- **MultiGPT:** braucht dafür nichts Neues außer einem API-Key mit `docs.read`/`docs.write`; prüfen,
  ob `list_documents` Änderungszeit oder Hash liefert, sonst als kleine Erweiterung vormerken.

## Einsatzbild Bibelschule

- **Schule:** ein großer Rechner mit MultiGPT und Eliezer (große Modelle, gemeinsame Sammlungen:
  Bibeln, Kommentare, Skripte der Lehrer, Bibliothek).
- **Schüler:** Baruch auf dem eigenen Laptop mit lokaler LanceDB; gleicht die Sammlungen ihres
  Kurses aus MultiGPT ab und arbeitet offline mit kleinen lokalen Modellen (3B) weiter. Im
  Schulnetz laufen die Personas auf dem Schulrechner (großer Kontext, Lektor dort).
- **Rechte über MultiGPT:** Konto und API-Key je Schüler; welche Sammlungen er lesen darf, regelt
  MultiGPT (Rollen, Freigaben). Baruch speichert den Key im Schlüsselbund.
- **Abgabe und Begleitung:** ein Schüler kann ein Werk (Predigtentwurf, Hausarbeit) in eine
  Kurs-Sammlung abgeben; Lehrer sehen den Stand und Befunde, ihre Kommentare kommen beim nächsten
  Abgleich als Kommentare zurück (nie als Änderung am Text).
- **Kosten:** große Modelle der Schule über deren Budget in MultiGPT; ChatGPT nur, wenn der Schüler
  einen eigenen Key einträgt.

## Datenmodell (SQLite)

- `work` (kind = book|sermon, title, subtitle, audience, purpose, bible_version, markup =
  typst|markdown, template, knowledge_collections[])
- `node` (work, parent, position, number „6.2“, title, kind = part|chapter|section|sermon_part)
- `revision` (node, number, body, author_kind = human|assistant, persona, model, based_on,
  status = proposed|current|rejected|superseded, note, edited, created) – Baum je Abschnitt (`based_on` =
  Elternteil), genau eine `current` je Abschnitt als umschaltbare Markierung; die aktuelle Fassung lässt
  sich nicht löschen (Migration 0004); seit Runde 22 schreibt der Mensch seine aktuelle Fassung beim Speichern im
  Editor fort (`amend_seq`, Verlauf `revision_amend` mit Text vorher/nachher, Rückgängig als neuer Schritt), solange
  keine Regel eine neue Fassung verlangt (Einstellung je Werk `work_revision_policy`, Migration 0050)
- `thread` / `post` (node, persona|human, reply_to, body, sources[], created) – gemeinsamer
  Abschnitts-Chat aller Personas (die Verbindung untereinander)
- `persona` (handle „@exeget“, name, role, prompt, backend, model, context_length, active)
- `job` (node, persona, kind = review|propose|audit|ask_persona, status, error)

## Verbundenes Persona-Team

- **Lektor automatisch:** die aktive Persona mit dem größten gemeldeten Kontext; sie bekommt das
  ganze Kapitel bzw. die ganze Predigt, moderiert, sammelt die Befunde der anderen und schreibt die
  Vorschlag-Revision. Ändert sich das Modell, wechselt der Lektor; anzeigen, wer es gerade ist.
- **Verbindung:** Personas posten in den Abschnitts-Chat und rufen einander per `@handle` an
  (Job `ask_persona`); der Lektor sieht alle Posts. Grenze gegen Endlosschleifen: Runden je
  Auftrag und Kostenlimit.
- **Start-Team** (Prompts bearbeitbar): Exeget (Text, Zusammenhang, Hauptaussage, mit RAG),
  Schrift (Bibelstellen und Wortlaut belegen, sonst „nicht belegt“), Belege (Quellen, Fußnoten,
  „Angabe prüfen“), Nathan/Auditor (theologisch angreifbare Stellen, Gewalt-/Schutzregeln,
  Notrufnummern; nur Befunde), Hörer (liest als Zielgruppe), für Predigten zusätzlich Zeitwächter
  (Länge in Minuten) und Anwendung (Alltagsbezug).
- **Zitierpflicht der Belege-Persona** (Erkenntnis aus MultiGPT-Chat 59): Recherchen der Modelle
  liefern fremde, oft nicht nachprüfbare Quellen. Darum braucht jede Sachaussage einen Beleg als
  `@schlüssel` aus der Literatur (Autorenbibliothek, Typst-`@cite`) oder den Vermerk „nicht belegt“.
  Neue Quellen kommen erst als Titel mit Zitat in die Literatur, dann in den Text.
- **Eigener Endpunkt je Persona, auch als Server** (2026-11-05):
  - *Ausgehend:* jede Persona hat ihren eigenen Endpunkt (`endpoint`: Adresse, Art `openai`|`mcp`|`multigpt`,
    Modell; Key je Persona im Schlüsselbund). So laufen Personas auf verschiedenen Rechnern (Laptop,
    Schulrechner, Eliezer); die Kontextlänge meldet der Endpunkt, daraus folgt der Lektor.
  - *Eingehend:* **ein gemeinsamer MCP-Server** in `baruch-server` (`POST /mcp`, mit Token) für alle
    Personas: Werkzeuge u. a. `list_personas`, `ask_persona {handle, …}`, Suche in Werk und
    Autorenbibliothek (nur lesend). Damit rufen andere Werkzeuge, andere Baruch-Instanzen oder die
    Personas untereinander (`@handle`, Relay wie in Elisa) eine Persona auf – mit ihrem Prompt und
    ihrem Endpunkt. Ergebnisse bleiben Vorschläge bzw. Beiträge.
    **Chatten über MCP:** Werkzeug `chat {handle?, node?, message, thread?}` – ein Werkzeug (z. B. Claude
    Code, MultiGPT, ein Skript) chattet mit einer Persona oder dem ganzen Team; mit `node` landet das
    Gespräch im Abschnitts-Chat (Autor = Name des Werkzeugs bzw. Tokens), sonst in einem eigenen Faden.
    Der Lektor sieht es mit; `@handle` im Text ruft weitere Personas (Rundengrenze und Kostenlimit gelten).
- **Gedächtnis ist der Lektor** (2026-11-05): Er kennt das ganze Werk und die Autorenbibliothek und
  meldet „du hast in … schon geschrieben …“ – Wiederholungen, Widersprüche, Querbezüge. Sucht in SQLite
  (FTS5) und LanceDB; jeder Beitrag trägt Verweise auf die Stelle (`TextRef`: Abschnitt, Revision,
  Wortlaut), in der Oberfläche als Link zum Springen. Keine eigene Persona dafür.

## Bildschirm

- **Links:** Kapitelbaum bzw. Predigtgliederung, Umordnen per Ziehen, Marken für offene Vorschläge
  und Befunde.
- **Mitte:** Abschnitt im Editor (Satzbild nach der Werkvorlage, umschaltbar auf Typst bzw. Markdown), Kopf „‹ 3/5 ›“ für
  Revisionen, Vorschläge im Lesefluss in der Farbe der Persona mit Diff und „Übernehmen / Verwerfen“; umschaltbar auf
  Typst-Vorschau (Seite) des Kapitels.
- **Rechts:** Personas (Lektor markiert), „Prüfen“ / „Vorschlag“ für Abschnitt oder Kapitel,
  darunter der gemeinsame Abschnitts-Chat mit Quellen.

## Stufen

Zeitliche Gesamtsicht (erledigte Runden aus Git, geplante geschätzt): `docs/Fahrplan.md`; Vorhaben in
`docs/Backlog.md`.

1. **B1 Gerüst:** Repo, Workspace (`baruch-core`, `baruch-server`), SQLite, Werke, Kapitelbaum,
   Revisionen von Hand mit „‹ ›“, Import Typst/Markdown nach Überschriften (`=`/`#`), Export als
   Typst-Ordner, Typst-Vorschau und PDF.
2. **B2 Assistenten:** Backends (MultiGPT-MCP zuerst, OpenAI-kompatibel), Personas, automatischer
   Lektor nach Kontext, Abschnitts-Chat mit `@`-Aufrufen, Vorschlag-Revisionen mit Diff. Umgesetzt in Runde 10
   mit Eliezer (OpenAI-kompatibel) als erstem Backend; der Lektor ist im Versuch seit 2026-11-06 fest `@lektor`
   (umschaltbar auf „automatisch nach Kontext“).
3. **B3 PDF-Import:** über MultiGPT `upload_document`/`read_document` (oder lokal, wenn ohne
   MultiGPT), Gliederung mit Rückfrage. Umgesetzt in Runde 21 (integriert 2026-11-09): PDF über MultiGPT oder lokal
   (`pdftotext`), EPUB immer lokal (MultiGPT nimmt laut Werkzeugbeschreibung kein EPUB an), Vorschau mit bearbeitbarer
   Gliederung, angelegt erst nach Bestätigung des Menschen (Werk, Quelle der Bibliothek oder beides); dazu Markdown,
   ZIP und Ordner mit Bildern. Die MultiGPT-Annahmen sind ungeprüft (Handprüfung beim Nutzer).
4. **B4 Predigt:** Werkart und Typst-Vorlage (Text, Einleitung, Teile, Anwendung, Schluss),
   Zeitwächter, Stichwortzettel als Export. Umgesetzt in Runde 16 (integriert 2026-11-08): Vorlage mit Predigttext
   aus SWORD, Lektor kennt die Predigt, Sprechdauer mit Zielzeit (Baum, Editorkopf, Gliederung, RPC/MCP),
   Stichwortzettel als PDF (A5, Karten; Seite, RPC, CLI).
5. **B5 Verteilung:** Server-Pakete (.dmg, .msi, .deb/.AppImage), Signierung offen, AGPL-Hinweise,
   Aktualisierung.

## Heute (Stand 2026-11-05)

- **B1-Ausschnitt erledigt:** `baruch-server` (axum, minijinja, TypeScript-Widgets) mit dreispaltiger
  Oberfläche, hellem/dunklem Thema, Alt+←/→, verschiebbaren Spalten; Daten in SQLite statt fest im
  Code (Beispielwerk beim ersten Start), Übernehmen/Verwerfen/Speichern je eine Transaktion, danach ein
  Commit im Git-Spiegel. Literatur und Volltextsuche über Werk und Bibliothek als Popup der Kopfzeile
  (seit Runde 3; Alt+L, ohne JavaScript die Seiten `/literatur` und `/quelle`, `?lq=…`; Treffer als
  `<mark>`, Treffer-Links behalten Werk und Suche, Titel mit Treffern aufgeklappt).
- **Härtung:** NUL und Steuerzeichen in der Suche ergeben eine leere Suche statt eines Serverfehlers;
  Statusübergänge per Trigger; Symlinks im Spiegel werden abgelehnt; Spiegel holt nach einem Fehler
  mit dem nächsten Commit nach (Tests).
- **Runde B1.5 erledigt:**
  - MCP-Werkzeuge über `POST /mcp` (`list_works`, `outline`, `read_node`, `search`, `propose_revision`,
    `add_post`, `list_personas`, bis Runde 3 auch `import_typst`; ohne Token nur vom eigenen Rechner). `ask_persona` und
    `chat` bleiben Stubs (B2).
  - Typst-Import eigener Bücher: CLI `baruch-server import/export` und (bis Runde 3) MCP `import_typst`, Werkvorlage
    bis zur ersten Überschrift in `work.template`, `=`/`==`/`===` als Gliederung, Rundreise Byte für
    Byte, Duplikatschutz über `source_sha256`.
  - Mehrere Werke mit Werkwahl in der Kopfzeile, ein Git-Spiegel je Werk.
  - Offen: `#let`-Definitionen der Vorlage gelten nicht in den per `#include` eingebundenen
    Abschnittsdateien des Spiegels (Entscheidung beim Nutzer); `#heading(...)` vor der ersten
    `=`-Überschrift bleibt Teil der Vorlage; Markdown-Import.
- **Runde 3 erledigt (revtree · litpopup · small):**
  - Revisionsbaum: Elternteil je Fassung, „aktuell“ umschaltbar (`revision.set_current`), Ändern und Löschen
    nur durch den Menschen (`revision.update`/`revision.delete`), je ein Commit im Spiegel mit
    `Baruch-Action`, Baumansicht und Vorschau mit `pick`. Nummern bleiben eindeutig, mit Lücken.
  - Literatur als Popup (Kopfzeile, Alt+L, ohne JavaScript `/literatur`), Treffer öffnen die Fundstelle,
    Dokumentansicht `/quelle`, Hervorhebungen (Zitat, Kernaussage, Widerspruch, zu prüfen), Zitat als
    Vorschlag-Revision.
  - Kleinigkeiten: Port 8765, Datenordner `dirs::data_local_dir()`, `import_typst` nicht mehr im MCP, Vorlage
    als `vorlage.typ` im Spiegel, slug an Wortgrenzen, Beispielwerk umbenannt (nur neue Datenbanken),
    Parser-Grenzen behoben.
- **Runde 4 erledigt (works · outline · fixes):**
  - Werke verwalten: eindeutige Titel (Migration 0005, `title_key`), Werkwahl öffnet sofort, Cookie
    `baruch_work`, Seite `/werke`, Anlegen (Buch/Predigt), Ändern (Metadaten, Werkvorlage mit Fassungen),
    Löschen mit eingetipptem Titel (Spiegel archiviert unter `works/.geloescht/`), Upload-Import und Export als
    Download, CLI `import --title`. Nur der Mensch, nicht über MCP.
  - Gliederungsmanager: Ebenen (Teil, Kapitel, Abschnitt, Unterabschnitt), Verschieben, Umstufen, Anlegen,
    Umbenennen, Löschen (RPC `outline.*`, Formulare ohne JavaScript, Ziehen und Ablegen), Nummerierung nach der
    Werkvorlage oder einstellbar (Migration 0006), aufgelöste Typst-Maskierungen in Titeln, Überschriften im
    Editor gleichen die Gliederung an; je Änderung ein Commit im Spiegel.
  - Nacharbeit: Löschen braucht `confirm`, Meldung nennt vorhandene Fassungen, Zitieren ohne Abschnitt
    abgelehnt, Hervorhebungen leerraum-robust, Volltextindex folgt gelöschten/verschobenen Beiträgen (0105),
    Tests zur Gleichzeitigkeit.
- **Runde 5 erledigt (tree · wysiwyg · r4):**
  - Eine Gliederung: der Baum links mit Ziehen und Ablegen, Kontextmenü, Umbenennen an Ort und Stelle (F2,
    Doppelklick), Anlegen, Umstufen, Löschen und Nummerierung im Kopf; `/gliederung…` nur noch ohne JavaScript.
    Verschieben behält die Ebene. Umschalten und Übernehmen lassen die Gliederung der Fassung folgen, auch in der
    Ebene (Unterpunkte mitgestuft, ein Commit); die Fassung selbst bleibt unverändert (Nutzerentscheidung
    2026-11-05). Römische Kapitel auch aus einer Nummerierungsfunktion der Vorlage.
  - Satzbild-Editor, erster Schritt: Crate `baruch-wysiwyg` (Blöcke, Typst-Inseln, Rundreise byte-gleich), RPC
    `wysiwyg.*`, Widget mit Umschalter „Satzbild | Quelltext“, Stil aus der Werkvorlage, Satzbild-Diff der
    Vorschläge; ohne JavaScript nur der Quelltext. Beschreibung: `docs/WYSIWYG.md`.
  - Nacharbeit: Werk-ids nie wiederverwendet (Migration 0007, `AUTOINCREMENT`), slugs gelöschter Werke
    reserviert, keine neue Saat nach dem Löschen aller Werke, MCP und Zitieren nur mit festem Schlüssel `key`,
    `work.import` über `/rpc` bis 16 MiB Text.
- **Runde 6 erledigt (numbering · satzbild · images):**
  - Gliederungsschemata je Werk (Migration 0008, zwölf Schemata von „aus der Werkvorlage“ bis „Predigt“ und
    „eigenes Muster“ in einer Mustersprache mit `P|k` und `P1;P2;…`), Seite „Gliederungsschema“ mit Vorschau,
    ohne JavaScript; Gliederung, Editor-Kopf, Export und Spiegel folgen, „aus der Werkvorlage“ byte-gleich.
    `heading_numbering` liest nur die letzte Regel der obersten Ebene und erkennt Tiefenbedingungen.
  - Satzbild: `#szene`/`#arbeit` als Blöcke mit bearbeitbaren Absätzen, Labels an der Überschrift, Stil nur aus
    Regeln der obersten Ebene, keine leere Fläche im Quelltext, Klick öffnet das Satzbild, Einfügen mehrerer
    Absätze, Bilder in Inseln mit Alternativtext.
  - Bilder als Text (Migration 0009: Base64 bzw. SVG, nie BLOB) mit Fassungen wie Abschnitte, Seite „Bilder“,
    RPC `asset.*`, MCP nur lesend, Import der Bilder über die CLI und `export --out` byte-gleich.
- **Runde 8 erledigt (inline · posts · typst):**
  - Vorschläge an ihrer Stelle im Lesefluss (ausgerichtet an der gezeigten Fassung, `baruch_wysiwyg::align`),
    Satzbild als Vorgabe schon im Server-HTML, Linien mit 3 : 1 Kontrast.
  - Abschnitte über den festen Schlüssel adressiert (`/?work=W&key=K`, MCP-Feld `url`); Beiträge löschen,
    verschieben, in Einfälle umwandeln (Mensch alle, MCP nur eigene; Ausnahme wie bei Einfällen, Beiträge sind
    kein Buchtext); Chat-Abzeichen.
  - Typst-Vorschau mit Werkvorlage: Crate `baruch-render` (Typst 0.15.1 eingebettet, offline, SVG), Seite
    `/werke/{id}/satz`, RPC `preview.typst`, MCP `typst_check` (nur lesend).
- **Runde 10 erledigt (B2: backend · personas · scripture):**
  - Modell-Backend Eliezer: Crate `baruch-llm` (gestreamte Chat-Antworten, eine Anfrage zur Zeit, Grenzen,
    Aliasse), `models.rs` (RPC `model.*`, Kurztest), Abgleich der Persona-Modelle beim Prüfen der Verbindung.
  - Personas wirklich: Prüfen, Vorschlag und „@persona fragen“ im Abschnitts-Chat, Antwort Wort für Wort
    (`/assistenten/stream`, ohne JavaScript `/assistenten/act`), Rollen-Prompts mit harten Regeln (Migration
    0015), Lektor fest @lektor (Versuch) oder automatisch, Lektor kennt die Einfälle, MCP `ask_persona`/`chat`.
  - Bibelstellen: Crate `baruch-scripture` (deutsche Verszählung der Schlachter 1951 nach SWORD „German“), RPC
    `scripture.check`, MCP `scripture_check`; Wortlaut bleibt „ungeprüft“. MCP-Tokens für andere Rechner
    (Migration 0016, nur Hash).
- **Runde 11 erledigt (Bilder und Umschlag: mcp-assets · cover · images):**
  - Buch als PDF (`typst-pdf`, gleiche Seitenzahl wie die Satz-Ansicht, RPC `work.pdf`, `/werke/{id}/pdf`, CLI
    `export --pdf`), Druckprüfung (fehlende Bilder, Rasterbilder unter 300 dpi).
  - Umschlag je Werk mit Vorder-/Rückseite im Satz, Titelei mit Impressum aus den Buchangaben (Migration 0017),
    Umschlag-PDF für die Druckerei mit Rücken und Beschnitt, EAN-13 aus der ISBN.
  - Bilder über MCP: `asset_add` sofort, Ersetzen/Umschlag/Löschen nur als Antrag (Migration 0018), den der
    Mensch auf der Seite „Bilder“ bestätigt. Bilder-Suche, Vorschaubilder, Bild einfügen, Kapitelbild im Editor.
  - Einstellungen → Assistenten mit einem Tab je Persona und Textfeldern für Rolle und Rollen-Prompt;
    Vervollständigung „@…“ im Abschnitts-Chat.
- **Runde 12 erledigt (Verbindungen nutzen, RAG: lancedb · pgvector · multigpt):**
  - Bedeutungssuche umschaltbar je Werk: LanceDB lokal im Datenordner (Haupt-RAG; Abschnitte in der aktuellen
    Fassung, Einfälle, Hervorhebungen, Autorenbibliothek; Einbettungen über Eliezer, eine Anfrage zur Zeit),
    pgvector direkt (nur lesend) und MultiGPT-MCP; Treffer zusammengeführt (Reciprocal Rank Fusion) ohne Doppelte,
    ein ausgefallener Weg blockiert nie, die Wortlaut-Suche (FTS5) bleibt immer.
  - Sammlungen je Werk (Migration 0031), Status der Wege, „Index neu aufbauen“; Umschalter Wortlaut/Bedeutung in
    Literatur und Einfällen; MCP `rag_search`/`rag_status` (nur lesend); @belege und der Lektor bekommen Treffer
    als Belege `[Rn]` (Migration 0032), Treffer sind Daten, nie Anweisungen.
- **Runde 13 erledigt (Quellen-Dokumente und Websuche: bau · quellen · websuche · abgleich):**
  - Server-Integrationstests als eine Binärdatei (`target/` 38 → 9 GB).
  - Volltext einer Quelle aus MultiGPT in der Dokumentansicht (seitenweise Teile, Fundstelle markiert, Übernahme in
    die Bibliothek), Prüfung von Hervorhebungen ohne Zitat gegen den Volltext (Migration 0106), MCP `source_read`
    (nur Dokumente aus gewählten Sammlungen); eine gemeinsame MCP-Sitzung mit Vorrat.
  - @belege recherchiert auf Wunsch im Web über MultiGPT (`web_search`, `fetch_url`; bevorzugte Quellen zuerst);
    Baruch prüft jedes Zitat gegen die abgerufene Seite und setzt die Quellenzeile selbst (Migration 0034); nicht
    über MCP, nicht beim Vorschlag.
  - Abgleich Baruch ↔ MultiGPT (Migration 0033): aktuelle Fassungen je Kapitel nach „Baruch: <Werk>“ nur nach
    Bestätigung, gewählte Sammlungen als lokale Kopie im LanceDB-Index (offline suchbar), Seite „Abgleichen“.
    Abweichung: Kommentare/Befunde gehen noch nicht hinaus (Nutzerfrage im Backlog).
- **Runde 14 erledigt (Teile und Gliederung: teile-typst · teile-gliederung · nummerierung):**
  - Teile im Typst-Text als Markierung `#heading(level: 1, numbering: none)[T] <baruch-part>`, „ohne Nummer“ als
    `… <baruch-unnumbered>`; Import, Export (byte-gleich), Spiegel, Satzbild und Satz (Teil allein auf einer rechten
    Seite, Kapitel zählen über Teile fort). Bestehende Texte werden nie umgewandelt.
  - Teile anlegen in jedem Werk, auch mit Werkvorlage (Baum, Formular, RPC `outline.add` mit `level`, MCP
    `outline_add` mit `level: 0` – Anlegen dürfen Assistenten); Umbenennen schreibt die Markierungszeile, Umschalten
    gleicht ab; Anträge der Assistenten mit Vorschau (auch ohne JavaScript); `idea.process` als Gliederungspunkt atomar.
  - Nummerierung je Ebene aus einer Quelle (`Numbering`): Funktionen der Werkvorlage je Ebene ausgewertet, Schemata
    mit Wörtern (Juristisch, Predigt, …) kommen beim Re-Import als dasselbe Schema zurück; Kapitel ohne Nummer stehen
    in Export, Spiegel und Satz als Markierung (die DB bleibt), Typst zählt wie Baruch.
- **Runde 15 erledigt (Lektor und Personas: lektor · anbieter · auftraege; 2026-11-08, Gutachten nachgebessert):**
  - Lektor nach Plan: das ganze Kapitel (bei Predigten das Werk) im Wortlaut, Befunde der anderen Personas, Prüfen,
    Fragen und Vorschlag für Abschnitt oder ganzes Kapitel (Vorschlag je Teil, nur `proposed`). Passt der Kontext
    nicht, kürzt Baruch nie still: entferntere Teile zusammengefasst, zu lange Abschnitte in überlappenden Stücken
    geprüft, Vermerk „Komprimiert: …“; jeder Aufruf wird vor dem Absenden gemessen, und was auch komprimiert nicht
    passt, wird ehrlich abgelehnt. „Kritisch:“ wird geprüft und einmal nachgefordert, im Vorschlag entfernt.
  - Modell-Backends frei je Persona: Eliezer, beliebige OpenAI-kompatible Dienste und Ollama (Migration 0036), „lokal“
    oder „extern“ (beim Anlegen nach der Adresse, Warnung bei öffentlicher Adresse als lokal), Buchtext an externe
    Verbindungen nur nach Freigabe je Persona – auch keine Einbettungen über ein externes Eliezer; Kontextlänge
    gegen die Gegenstelle abgeglichen (Ollama: die wirksame Länge, nie der Höchstwert des Modells).
  - Aufträge an Personas: Chat-Strom mit eigenem Beitrag sofort, Hinweisseite ohne JavaScript, Einfall an eine gewählte
    Persona, Literatur „an Persona geben“, Fäden ohne Abschnitt (Migration 0038), MCP `chat` mit Warteschlange je
    Client.
  - Nutzerwünsche 2026-11-07: **Bibeln in der Literatur** (SWORD-Bibeln im Literatur-Popup und unter
    `/literatur/bibel`, Treffergruppen „Bibel“ und „Kommentar“, Zitat als Vorschlag) und **SWORD als
    Recherchewerkzeug** (Kommentare und Lexika je Werk, Migration 0037; Faktenblock aus Bibeltext, Kommentaren und
    Lexika für @belege und den Lektor, nummerierte Belege; MCP `commentary_lookup`/`lexicon_*`).
  - Schutz: keine Seite lässt sich von fremden Seiten einrahmen (`X-Frame-Options`, `frame-ancestors`).
- **Runde 17 integriert (Bibelstellen, SWORD-Rest, Reste Lektor: verszaehlung · stellen · pruefanzeige; 2026-11-08):**
  - Alle 18 Verszählungen von SWORD und eine geprüfte Abbildung zwischen ihnen (Psalmüberschriften, Kapitelgrenzen
    wie Joel 2/3, Mal 3/4 …; nie geraten), RPC/MCP `scripture.map`; Bibeln in anderer Zählung werden über die
    Abbildung gelesen (Wortlautprüfung, Predigttext, Nachschlagen).
  - Stellenparser: Komma-Listen, Uhrzeiten und Anzahlen sind keine Stellen, „Psalm 139, … Vers 23“, Zitate über
    Absätze und verschachtelt, Blockzitate in Szenen; 4. und 5. Mose im Fließtext.
  - Prüfanzeige: „Zitate prüfen“ im Editor (Markierung im Satzbild, Liste; ohne JavaScript eigene Seite), RPC
    `bible.marks`; verschlüsselte (Schlüssel nur im Schlüsselbund) und BZIP2/XZ-gepackte SWORD-Module, Lizenzvermerk
    überall, Kommentar-Widget.
  - Lektor: Zitatprüfung nur gegen den gezeigten Text, falsche Stellenaussagen in „Kritisch:“-Befunden kenntlich,
    Einfälle gerahmt, wirksame Kontextlänge ohne Netzabfrage beim Seitenladen.
- **Runde 26 integriert (Austauschformat zwischen Instanzen, XML: ids · export · import; 2026-11-08, vorgezogen vor
  R25 und R18):**
  - Jedes Stück eines Werks hat eine weltweit eindeutige Kennung (UUID, Migrationen 0041/0042), gelöschte hinterlassen
    Grabsteine; jede Instanz hat Kennung und Namen.
  - `.baruch` (ZIP mit `werk.xml`, Bildern und Manifest mit SHA-256) bzw. `.baruch.xml`, Namensraum
    `urn:baruch:werk:1` mit XSD; alle Fassungen, Vorlage, Einfälle, Beiträge, Fäden, Bilder, Buchangaben. Nicht im
    Format: Schlüssel, Verbindungen, Persona-Modelle, offene Anträge, Fremdtexte (`docs/Austauschformat.md`).
  - Einlesen mit Vorschau: neu oder abgleichen (nur hinzufügen, nie löschen oder überschreiben, keine eingelesene
    Fassung wird aktuell, Gliederungsänderungen als Anträge); sicher gegen DTD, Bomben und Pfadtricks. Wege: Werk ▾,
    RPC, Kommandozeile; MCP nur Ausgeben.
- **Runde 25 integriert (Gedankenkarte: Mindmap der Einfälle; karte-daten · karte-bild · karte-widget; 2026-11-08,
  nach R26 und vor R18):**
  - Seite `/werke/{id}/karte` (Werkzeuge ▾ → Gedankenkarte): Werk in der Mitte, Teile und Kapitel im Ring, Abschnitte
    zum Kapitel zusammengefasst (aufklappbar), Einfälle an ihrem Bezug, „Ohne Kapitel“ als Wolke; Filter nach Status,
    Schlagwort, Herkunft und Volltext per GET; Gliederungsliste und Liste „Widersprüche“; Ausgaben SVG, PDF, Markdown
    und FreeMind (`.mm`). Ohne JavaScript vollständig über Formulare bedienbar.
  - Verbindungen zwischen Einfällen (führt weiter, gehört zu, Beispiel für, widerspricht; Migration 0043) für Mensch,
    Persona und MCP (`idea_link`, `idea_unlink`, `idea_links`, `idea_map`), jede Änderung ein Spiegel-Commit, Zeile
    „Verbindungen“ in `einfaelle.md`; Lagen auf der Karte (nur Anzeige, ohne Commit); Nähe aus den gespeicherten
    Einbettungen und gleichen Schlagwörtern (nur angezeigt, keine Einbettungsanfrage beim Zeichnen).
  - Widget: Zoom, Verschieben, Einfall ziehen (auf Kapitel = Bezug, auf Einfall = Verbindung, sonst Lage), Tastatur
    (Pfeile, Enter, V, N, Alt+E), Seitenleiste.
  - Austauschformat Unterversion 2: Verbindungen und Lagen in `<karte>` (XSD `baruch-werk-1-u2.xsd`).
  - Gruppen durch einen Assistenten benennen: seit Runde 19b (Vorschlag, der Mensch übernimmt als Schlagwort).
  - Noch nicht: werkübergreifende Karte (Nutzerfrage).
- **Runde 18 integriert (Editor und Menüband: menueband · konstrukte · satzbild-werk; 2026-11-08):**
  - Menüband für das Bearbeiten: Formate der Werkvorlage als Knöpfe (`#szene`, `#arbeit` nur, wenn die Vorlage sie
    definiert; Fußnote, Zitat, Liste, Aufzählung, Begriff, Umbruch, Link immer), „Bild“ öffnet „Bild einfügen …“; im
    Satzbild führt das Widget den Befehl aus, im Quelltext wird der Typst-Text an der Cursorstelle eingefügt.
  - Satz-Seite ohne JavaScript (‹ › mit `aria-label`, PDF, „Satz abbrechen“ als Formular), Typst-Lauf abbrechbar
    (`preview.cancel`; Auswertung und Umbruch laufen in Typst zu Ende, gespart wird das Schreiben der Seiten), fremde
    Seiten lösen keinen Satz und kein PDF aus (`Sec-Fetch-Site`); Schriftsuche beim Start; Kopfzeile bricht in schmalen
    Fenstern um; Gedankenkarte: Kanten mit Beschriftung, Pfeil für gerichtete Arten und eigener Strichart.
  - Satzbild: Aufzählungen, Begriffslisten, geschachtelte Listen, Umbruch `\`, `#link`, Verweise `@…` als Blöcke statt
    Inseln; Inseln an Ort und Stelle bearbeitbar; mehrere Bilder je Block; Markdown-Überschriften auch beim Ändern
    einer Fassung; Lektor: ausgelassene Teile in einer Zeile, Zwischenspeicher der Zusammenfassungen.
  - Satzbild-Einstellungen je Werk (Einstellungen → Werk → Satzbild: Schrift, Größe, Farben, Blattfarbe, Satzbreite;
    Migration 0044, nur HTML, nie Typst/PDF), Stil schon im HTML; Bild an der Cursorstelle (Pfad mit der Fassung in
    einer Transaktion); Literatur-Treffer im Satzbild markiert; „Alle einklappen“ auch im Satzbild.
- **Runde 27 integriert (Umschlag-Editor und Impressum in der Werkvorlage: umschlag-daten · umschlag-satz ·
  umschlag-widget; 2026-11-09, nach R18 und vor R19):**
  - Umschlagseite als Editor: Titel, Untertitel, Autor, Verlag, Rückentexte, Motto, Klappentext, Kurzbiografie,
    Strichcode, Preis und je Fläche ein Freitext als Blöcke frei setzen (Lage, Breite, Größe, Zeilenabstand, Farbe,
    Schrift, Schnitt, Ausrichtung, Schatten/Verlauf, Laufrichtung des Rückens); Hilfslinien, Einrasten, Tastatur,
    Rückgängig; Vorschau per GET und Speichern über Formulare auch ohne JavaScript; Schnellwahl oben/Mitte/unten und
    Zurücksetzen (Migration 0045).
  - Dieselbe Lage in Vorschau, Umschlag-PDF und Buch-PDF (Millimeter ab der Schnittkante); ohne eigene Anordnung bleibt
    der Satz byte-gleich. Druckprüfung: Beschnitt, Sicherheitsabstand, Strichcode, Rücken, Kontrast im Umschlagbild,
    fehlende Schrift (Rückfall mit Hinweis).
  - Titelei aus der Werkvorlage (`schmutztitel`, `haupttitel`, `impressum`, `widmung` mit dem Wörterbuch
    `baruch_buch`), neue Bücher bekommen die Impressum-Vorlage, Knopf übernimmt sie in bestehende Werke; Anschriften
    von Verlag und Druckerei; Pflichtangaben-Prüfung (üblich nach den Landespressegesetzen, keine Rechtsberatung).
  - MCP liest die Anordnung (`cover_layout`) und stellt höchstens einen Antrag (`cover_layout_set`); Austauschformat
    Unterversion 3 trägt Anordnung, Anschriften und Satzbild-Einstellungen.
  - Noch nicht: eigene Schriftdateien (Nutzerfrage); Umschlag im EPUB seit Runde 20.
- **Runde 28 integriert (Befunde: Befundliste, Übersicht übers Werk, Aufträge im Chat: befunde · befund-uebersicht ·
  chat-auftrag; 2026-11-09, nach R27 und vor R19):**
  - Befund als Datensatz (Migration 0046: `finding`, `finding_status`, `post_version`): Antworten der Personas werden
    nach dem Speichern zerlegt (Art Lob/Hinweis/kritisch, Zitat, Prüfvermerk, `[B<id>]`-Verweise); der Beitrag bleibt
    unverändert. Status (offen, übernommen mit Fassung, verworfen mit Begründung, verschoben, erledigt) setzt nur der
    Mensch, je Änderung ein Commit; Assistenten stellen Vorschläge, der Mensch entscheidet. Ähnliche Wiederholungen
    werden angehängt (nur Text, Einbettungen R19a). Spiegel `befunde.md`, Austauschformat Unterversion 4.
  - Übersicht: Seite **Werk ▾ → Befunde** (Matrix Kapitel × Persona mit Zeichen und Zahl, Filter, Listen),
    Abzeichen „k offen“ in Gliederung, Editorkopf, `/gliederung` und am Menü, Akkordeon „Befunde“ mit
    Statusformularen, Ebene „Befunde“ in der Gedankenkarte, „setzt Befund x um“ im Fassungsbaum; MCP `findings`
    (lesen) und `finding_propose_status` (Vorschlag).
  - Chat: „RFC“/`@alle` ruft alle aktiven Personas, der Lektor zuletzt mit Sammlung und Gewichtung (Sammelvorschläge
    mit Verweis auf die Ursprünge); Bitte um Text wird ein Vorschlag; Entscheidungen des Autors gehen gerahmt in jeden
    Aufruf. Kritisch-Pflicht nur noch für @nathan, der Lektor beginnt mit dem, was trägt. Chat rollt in begrenzter
    Höhe, Kopieren/Zitieren, eigene Beiträge bearbeiten mit Verlauf (`post.edit`, MCP `post_edit`), ganze Historie
    ohne JavaScript (`/beitraege`, `/beitrag`).
  - Noch nicht: Ähnlichkeit über Einbettungen und Befunde im RAG-Index (R19a), neuer Abschnitt per Schreibauftrag.
- **Runde 30 integriert (Suchen/Ersetzen, Querverweise, Chat: Lösungsvorschläge, Varianten, Bedienung:
  suchen-verweise · chat-loesungen · chat-bedienung; 2026-11-09, vor R29 und R19):**
  - Suchen und Ersetzen im Abschnitt (Strg+F/H, Suchleiste im Editor) und im Werk (Strg+Umschalt+F/H, Seitenleiste
    bzw. Seite `/suche` ohne JavaScript) mit Bereichen, Groß/klein, ganzem Wort und regulärem Ausdruck (Zeitgrenze);
    Ersetzen im Werk legt je Abschnitt eine Fassung vom Menschen an, ein Commit, Rückgängig mit einem Klick;
    Bibelzitate und Zitate bleiben unberührt (Migration 0047, `replace_batch`).
  - Querverweise: feste Sprungmarke `<kN>` je Gliederungspunkt nur in Export, Spiegel und Satz; Verweis
    `#link(<kN>)[…]` (wahlweise mit „S. n“) über Menüband „Verweis“; Strg+Klick springt, Alt+← zurück; Liste
    möglicher und kaputter Verweise (`/verweise`, Druckprüfung); Einlesen und Austausch schlüsseln Verweise um.
  - Lösungen: kritische Punkte tragen `statt`/`besser` bzw. `Schritt` (sonst einmal nachgefordert, dann „(ohne
    Lösungsvorschlag)“); „Als Vorschlag übernehmen“; Varianten A/B/C als Geschwister-Vorschläge mit Vergleich
    (`/varianten`); Prüfgrundlage für alle Personas eines Aufrufs gleich und im Kopf sichtbar; Austausch Unterversion 5.
  - Chat: Anrede „Lektor, …“ ohne @, Relay bei direkter Anrede in einer Antwort (eine Stufe), Live-Anzeige
    („schreibt …“, neue Beiträge ohne Neuladen), ↻ Wiederholen und ✕ Löschen, gegliederte Beiträge mit Befund-Blöcken
    und geprüften Sprungmarken.
  - Noch nicht: Vorwärts-Sprung per Taste, Verweis-Seitenzahl als Werk-Einstellung (R29); EPUB-Links seit Runde 20.
- **Runde 29 integriert (Buchsatz in den Einstellungen: satz-daten · satz-typst · satz-oberflaeche; 2026-11-09):**
  - Einstellungen → Werk → Satz (ohne JavaScript, mit Vorschau): Kapitelanfang (fortlaufend, neue Seite, neue rechte
    Seite), Teil-Seite, Seitenformat (A5, 12 × 19, 13,5 × 21,5, A4, eigenes), Ränder mit Bundsteg, Grundschrift,
    Größe, Zeilenabstand, Einzug oder Abstand, Blocksatz, Silbentrennung und Sprache, Seitenzahlen (Lage, ab erstem
    Kapitel), Kolumnentitel, Fußnoten, Schusterjungen/Hurenkinder und „Verweis mit Seitenzahl“ als Werk-Einstellung.
    Jede Änderung ein Spiegel-Commit vom Menschen; MCP liest nur (`typeset_settings`).
  - Typst-Regeln „// Baruch: Satz“ hinter der Werkvorlage (Haken `snapshot::head`): gleich in Export, Spiegel,
    Satz-Vorschau, Buch-PDF und Umschlag; ohne Einstellungen byte-gleich. Hinweise, wo die Werkvorlage dieselbe Sache
    selbst setzt; Angebot, eine Kapitelanfangs-Regel der Vorlage als Einstellung zu übernehmen (neue Vorlagenfassung).
  - Migration 0048 (`work_typeset`, Grabsteinart), Austausch Unterversion 6 (`<satz>`); `epub_style` als
    Datenmodell (noch nicht verdrahtet).
  - Seit R21b liest der Import eines Exports den Regelblock zurück in die Satz-Einstellungen (nur wenn exakt lesbar);
    EPUB nutzt `epub_style` seit Runde 20.
- **Runde 19 abgeschlossen (RAG-Pflege, Reste der Stellen: index · pgvector · treffer; 2026-11-09):**
  - Index: kein neuer Einbettungsversuch je Suche (Suchen reihen nur das Aufräumen ein), ehrlicher Status mit
    liegengebliebenen Aufträgen (`index.stale`, Seite „Verbindungen“), „Index neu aufbauen“ antwortet sofort
    (`deferred`), Einfälle gelöschter Werke sofort im gemeinsamen Bereich, Start mit eigenem Datenordner ohne
    gespeicherte Verbindung Eliezer ohne Einbettungsanfrage; Abgleich mit MultiGPT gehärtet.
  - Aufgaben-Präfixe der Einbettung je Weg (LanceDB Anfrage/Dokument, pgvector Anfrage); pgvector kürzt jedes Feld in
    SQL, quotet Schema-Namen, warnt bei Passwort ohne TLS.
  - „schwach“ für Treffer unter 0,45 in Literatur, Einfällen, Belegen und `rag.search` (`weak`), zugeklappt ohne
    JavaScript; `idea.search` mit `mode: "rag"` meldet Ausfälle (-32001).
  - Gedankenkarte: „Gruppen benennen (Assistent)“ als Vorschlag, Übernehmen nur durch den Menschen; MCP `idea_map`
    mit `near_limit`/`near_threshold`. Menüband „Bibelstelle“/„Literaturverweis“, wenn die Vorlage sie hergibt.
  - Stellen: Wortlaut gegen Module mit Silbentrennresten („Wortlaut stimmt (Leerraum im Modul)“, Hinweis zum Modul),
    „Mal“ als Maleachi vor Kapitel und Vers, Zitat-Bindeglied, Lemma-Probe der Lexika, NFC im Vergleich.
  - Noch nicht: ältere Fassungen/Befunde im RAG-Index (Nutzerfrage 1), Kommentare/Befunde nach MultiGPT
    (Nutzerfrage 11).
- **Runde 20 integriert (EPUB-Vorschau: epub · reader · epub-pruefung; 2026-11-09):**
  - EPUB 3 mit EPUB-2-Rückfall (neue Crate `baruch-epub`, rein): je Teil und Kapitel eine XHTML-Datei, Text über das
    Blockmodell von baruch-wysiwyg (kein typst-html), Fußnoten als Popup oder Endnoten, Querverweise `#link(<kN>)`
    als interne Links, Bilder mit Alternativtext, Umschlag als `cover-image`, Titelei, `nav.xhtml` + `toc.ncx`,
    Metadaten nur als maskierter Text, Satz-Einstellungen als CSS (`epub_style`), deterministische Bytes. Nicht
    Abbildbares wird gemeldet (`ISLAND_OMITTED`, Seitenumbruch-Regeln der Vorlage `TEMPLATE_PAGEBREAK_*` …).
    Wege: Menü Werk → „Als EPUB herunterladen“, RPC `work.epub`, CLI `export --epub`, MCP `epub_export` (lesend).
  - Vorschau je Reader (Menü Ansicht → „EPUB-Vorschau“): neun Profile (Pflicht: Apple Books iPhone/iPad, Google Play
    Books, „Kindle (über EPUB)“, „Kindle-App (über EPUB)“), iframes nebeneinander mit Reader-CSS und eigener CSP
    (`sandbox; frame-ancestors 'self'`), Kapitelgrenzen, Nachtmodus; ohne JavaScript per Formular-GET. Profilwerte
    sind Annahmen ohne Gerät.
  - Prüfung: eigene Struktur- und Barrierefreiheitsprüfung (xmllint, sonst quick-xml), Reader-Warnungen, EPUBCheck aus
    PATH oder eingestelltem epubcheck.jar (nie Download, nur der Mensch startet es; MCP `epub_check` nie). Reste der
    Buchangaben: ISBN-Präfix 978/979, `paper_sheet_mm` (Dicke je Blatt, ohne Migration), „Umschlag entfernt“ als
    eigener Commit, Druckerei-Umschlag mit den Textregeln der Werkvorlage. Keine Migration.
  - Noch nicht: echte Geräte-/Reader-Prüfung und echter EPUBCheck-Lauf mit Java (Handprüfung), Hersteller-Reader auf
    Android (Nutzerfrage), eigenes Kindle-Format (bewusst nicht, Send to Kindle nimmt EPUB an).
- **Runde 31 integriert (Register und Glossar: glossar-daten · register-satz · kontextmenue; 2026-11-09):**
  - Glossar je Werk (Migration 0049: Begriffe mit Varianten, kurzer und ausführlicher Erklärung, Quelle, Bibelstellen;
    eigene Register; Registervorschläge), RPC `glossary.*`/`index.*`, je Änderung ein Spiegel-Commit, Spiegeldatei
    `glossar.md`; Assistenten und MCP (`glossary_propose`, `index_propose`, `glossary.suggest`) nur Vorschläge, der
    Mensch übernimmt. Austausch Unterversion 7 (Glossar, eigene Register, Satz-Felder, Grabsteine).
  - Registermarke im Text `#metadata((register: …, term: …))<baruch-idx>` (Buchtext: Setzen/Entfernen = neue Fassung des
    Menschen); Register Stichwort, Personen, Bibelstellen (automatisch aus erkannten Stellen, biblische Reihenfolge) und
    bis zu 13 eigene. Satz-Einstellungen: Registerseiten (1–3 Spalten) mit Seitenzahlen aus Typst (Bereiche „12–14“,
    Unterstichwörter, „siehe“/„siehe auch“), Glossarseite vor/nach den Registern, erste Erwähnung je Kapitel
    ausgezeichnet; nur Projektion (Export, Spiegel, PDF, EPUB mit `glossary.xhtml`/`index-*.xhtml`), SQLite bleibt
    unverändert; ohne Einstellungen alles byte-gleich.
  - Kontextmenü im Satzbild und im Quelltext (Rechtsklick, Kontextmenü-Taste, Umschalt+F10; ARIA-Menü): „In Glossar
    aufnehmen“, „In Register aufnehmen“, „Im Glossar nachschlagen“, Marke bearbeiten/entfernen; Menüband-Gruppe
    „Register und Glossar“. Ohne JavaScript: Link „Markierung …“, Seiten Glossar und Register mit allen Formularen.
  - Import eines Exports mit Buchende nimmt Buchende und Projektion wieder heraus.
  - Noch nicht: Bearbeiten einer Marke ohne JavaScript nur als Entfernen und neu Setzen; Groß-/Kleinschreibung
    gleicher Stichwörter wird nicht zusammengeführt; Typst-Meldungen aus dem Buchende zeigen auf den letzten Abschnitt;
    Kontextmenü nicht in echten Browsern außer Firefox geprüft; beim Import eines Exports ohne Glossarseite bleibt die
    Auszeichnung der ersten Erwähnung (`#emph[…]`) als gewöhnliche Hervorhebung im Text; Registervorschläge reisen nicht
    im Austauschformat mit; kein MCP-Werkzeug für „Begriffe vorschlagen lassen“ (nur der Mensch löst es aus).
- **Runde 22 integriert (Revisionen und Spiegel: fassungen · befund-rueckfrage · spiegel-entwurf; 2026-11-10):**
  - Fortschreiben: „Speichern“ im Editor schreibt die eigene aktuelle Fassung fort (Stand k, je Schritt Text
    vorher/nachher und ein Spiegel-Commit); eine neue Fassung entsteht mit Grund (auf Wunsch, Persona-Fassung, andere
    Basis, Überschrift/Gliederung, große Änderung, Zeitgrenze, ausgeschaltet). Verlauf mit Wort-Unterschied,
    Rückgängig als neuer Schritt, Einstellung „Fassungen“ je Werk, „weitere Fassung(en)“ im Lesefluss, Seitenzweige
    `refs/revisions/<key>/<n>` im Spiegel. Migration 0050.
  - Personas fragen beim erneuten Prüfen nach erledigten eigenen Befunden („Befund n scheint erledigt?“, Ja/Nein durch
    den Menschen); übernommene Befunde merken den Stand der Fassung. Archiv gelöschter Werke mit Wiederherstellen;
    Spiegel der Einfälle ohne Werk beim Start und nach dem Löschen.
  - Entwurf als Wasserzeichen (Satz-Einstellung, „Als Entwurf herunterladen“ ohne Änderung der Einstellung) in
    Satz-Vorschau, Buch-PDF, Stichwortzettel, auf Wunsch Umschlag, EPUB-Titelseite/Metadatum; ohne Entwurf byte-gleich.
    Austausch Unterversion 8 (Fortschreibungen, Entwurf, Stand an Befunden, Vorspulen, neuer Teil als Antrag mit
    Kennung), `<kN>`-Show-Regel samt Rückweg beim Import, Spiegel setzbar (`main.typ` mit `#include`).
  - Noch nicht: übrige Schreibwege (Bild, Registermarke, Verweis, Ersetzen, Gliederung) schreiben nicht fort;
    Option „Sitzung“ statt Minuten; Bilder und Werkart kommen aus dem Archiv nicht mit; `persona.chat`/MCP
    `ask_persona` geben die Rückfrage-Felder nicht eigens aus (seit Runde 23 doch: `done_proposals`, `done_notes`,
    `all_done`).
- **Runde 23 abgeschlossen (Härtung: web · mcp-namen · geheimnisse; 2026-11-24):**
  - Editor und Oberfläche: Zitat-Werkzeug (Block mit Folgezeile für die Stellenangabe, im Satzbild Zitat und eigener
    Absatz statt Code-Insel), „Fassung ändern“ mit Verlaufsschritt und Standprüfung, `guard::own_reads` gegen lesende
    Anfragen fremder Seiten, feste Meldecodes, Abzeichen ohne JavaScript, Editor-Sperre beim Speichern,
    Verhaltenstests der Widgets im Node-Harness; Reste zu Gedankenkarte, Umschlag, EPUB, Register, Befunden.
  - MCP: eindeutige Handles mit Umlaut-Umschreibung, `lokal-` für Clients ohne Token, Eigentum nur über den Handle,
    gemeinsames Bildkontingent ohne Token, Obergrenzen und Größengrenzen, ein Relay-Weg, Rückmeldefelder in
    `ask_persona`/`persona.chat`.
  - Geheimnisse: Schwärzen an einer Stelle (auch Base64 und Prozentform), keine Weiterleitungen, Filter vor
    `fetch_url`, `confirm` beim Löschen von Modell-Verbindungen, Grenzen im Austausch, privater EPUBCheck-Temp-Ordner.
  - Keine Migration, keine neue Unterversion, keine neuen Methoden oder Werkzeuge. Gutachten (14 Befunde),
    Nachbesserung (Alt-Handle für Tokens, die erst die neue Regel zusammenlegte; gemeinsame Kontingente ohne Token)
    und PM-Abschluss sind erfolgt; Reste in `docs/Backlog.md`, „Nach Runde 23 offen“.
- **Runde 24 „Verteilung (B5)“ erledigt** (2026-11-24; mit Gutachten, Nachbesserung und PM-Abschluss – die letzte
  geplante Runde): Version 0.9.0.
  - Pakete: .deb und AppImage lokal gebaut und gestartet; .dmg und .msi nur als CI-Aufträge (unsigniert, nie
    gelaufen). Jedes Paket trägt `LICENSE`, `THIRD-PARTY-LICENSES.txt` und das Quellcode-Angebot `QUELLTEXT.txt`;
    Daten bleiben im Nutzerordner, kein Paket legt dort etwas an oder löscht etwas. Schlüsselbund je Zielsystem.
  - Über-Seite `/ueber` (Version, Lizenz, Quellcode, Drittlizenzen, Datenordner) im Menü Werkzeuge und unter
    Einstellungen → Daten; **keine Aktualisierungsprüfung** – Baruch fragt nie von sich aus im Netz nach Versionen.
  - CI: `ci.yml` (Tests, clippy mit `-D warnings`, `cargo deny`, `typst compile`, Seitenbau), `release.yml` (Tag
    `v*`: Pakete, APT-Repository, Release-Entwurf, GitHub Pages), `pages.yml`. Kein Arbeitsablauf ist je gelaufen;
    kein Tag, kein Push. Alles Ausliefern geht über GitHub, nichts von Hand.
  - Webseite `site/` (Hugo, eigenes Theme, ohne JavaScript) für `https://baruch.bernau.family/`; APT unter `/apt/`.
  - Betrieb hinter einem Reverse-Proxy: `--allow-host NAME`/`BARUCH_ALLOWED_HOSTS`, unter einem solchen Namen `/mcp`
    nur mit Token; Zugangsschutz im Proxy ist Pflicht (`docs/Betrieb.md`). MultiGPT hat keine Vorgabe-Adresse mehr.
  - Keine Migration, keine neuen RPC-Methoden oder MCP-Werkzeuge. Offenes und die Schritte des Nutzers:
    `docs/Backlog.md`, „Nach Runde 24 offen“.
- **Runde 35 „Konten und Rollen (Lektorat II)“ integriert** (2026-11-27, auf `development`; Version unverändert
  0.9.0, im CHANGELOG unter „Unveröffentlicht“):
  - Ohne Konten bleibt Baruch offen wie bisher. Mit Konten verlangen Seiten, Formulare und `/rpc` eine Anmeldung
    (Argon2id, Sitzung als Cookie, Sperre gegen Durchprobieren); `/mcp` bleibt bei Tokens. Das erste Konto (Autor)
    entsteht nur am eigenen Rechner; Notweg `baruch-server passwort <name>`.
  - Rollen je Werk: Der Autor gibt ein Werk für Lektor-Konten frei (Mitwirkende). Lektoren (`@lektorat:<name>`)
    sehen nur freigegebene Werke und schreiben nur Vorschläge, Beiträge und Befunde (auch „Anmerkung zum Buch“),
    mit Herkunft im Spiegel; übernehmen, ändern und löschen kann nur der Autor.
  - Eine Stelle der Rechteprüfung (`roles`: `LEKTOR_ROUTES`, `LEKTOR_RPC`): Für Lektoren ist alles gesperrt, was
    dort nicht steht – auch Routen und Methoden künftiger Runden.
  - Migrationen 0053 (Konten, Sitzungen, Fehlversuche) und 0054 (Freigaben); neue RPC `account.*`, `member.*`,
    `revision.propose`, `finding.add`; kein neues MCP-Werkzeug. Alles ohne JavaScript bedienbar. In einem echten
    Browser hinter einem echten Proxy ungeprüft. Offenes: `docs/Backlog.md`, „Nach Runde 35 offen“.
- **Runde 34 „Mindmap wie XMind“ integriert** (2026-11-25, auf `development`; Version unverändert 0.9.0, im
  CHANGELOG unter „Unveröffentlicht“, gedacht für 0.9.1):
  - Die Gedankenkarte ist eine Mindmap: Werk in der Mitte, Teile/Kapitel als Äste rechts und links, Einfälle als
    Unteräste beliebig tief; Anlegen (Tab/Enter), Umbenennen (F2), Umhängen (Ziehen), Einklappen, Löschen mit
    Wiederherstellen, Rückgängig, Verbindungen als beschriftete Linien. Eine Anordnung, in Rust (SVG vom Server,
    Bildschirm und Ausgabe gleich).
  - **Entscheidung (Nutzer 2026-11-25): Wolke und Mindmap sind verbunden.** Einfälle ohne Platz liegen in der Wolke
    neben dem Baum; sie wandern in die Mindmap (auf Knoten, Kapitel oder als Hauptthema ans Werk) und zurück, samt
    Unterknoten. Nichts wird ungefragt Hauptast.
  - Einfälle als Baum (Migration 0052: Titel, Elternknoten, Reihenfolge, Hauptthema), RPC `idea.move`/`idea.tree`,
    MCP `idea_move`/`idea_tree`, Austauschformat Unterversion 9.
  - Gliederungsansicht ohne JavaScript; Ausgabe `.xmind`, `.mm`, SVG, PDF, Markdown; Einlesen `.xmind`/`.mm` mit
    Vorschau. In echtem XMind und Freeplane ungeprüft. Offenes: `docs/Backlog.md`, „Nach Runde 34 offen“.
- **Runde 33 „Menüband und Laptop-Modus“ abgeschlossen** (2026-11-25, auf dem Zweig `runde33` neben Runde 32;
  Version unverändert 0.9.0; Zusammenführen mit `development` und Runde 32 samt Gegenprobe stehen aus):
  - Ein Menüband auf jeder Seite ersetzt die Menüs Werk/Ansicht/Werkzeuge und die Knopfreihe über dem Blatt: Menü
    „Baruch ▾“ (Programm), Reiter Start · Einfügen · Prüfen · Buch · Ansicht mit benannten Gruppen; die Knöpfe des
    Editors docken mit JavaScript in das Band an. Abzeichen an den Reitern. Alles ohne JavaScript erreichbar
    (Reiter als Links `?tab=`, Gruppen als `<details>`, Band auf/zu über `?band=`).
  - Laptop-Modus (Automatisch/An/Aus): unter 1600 px Band kompakt (Gruppen als Popups), unter 1280 px Gliederung und
    Assistenten als Leisten mit Schublade; Spalten einzeln einklappbar; Zoom zurücksetzen. Zustand als Attribute an
    `<html>`, vom Server aus Cookies gezeichnet, keine Media-Queries.
  - Jede Seite öffnet mit ihrem Reiter; die Satz-Seite trägt ihre Bedienung im Reiter Buch.
  - Keine Migration, keine neue Route, keine neue RPC-Methode, kein neues MCP-Werkzeug.
- **Runde 32 „Lektorat I – Word und Beenden“ abgeschlossen** (2026-11-25, auf `development` nach Runde 33; Version
  unverändert 0.9.0, im CHANGELOG unter „Unveröffentlicht“):
  - Word-Import (`/werke/import/word`, RPC `docx_import.*`, CLI `import DATEI.docx`): Vorschau mit Gliederung,
    Bearbeitern und Hinweisen; Fassung 1 = Stand vor allen verfolgten Änderungen, Änderungen je Bearbeiter als offene
    Vorschläge unter `@word:<name>`, Kommentare als Beiträge – nie eine aktuelle Fassung.
  - Word-Export (`/werke/{id}/word`, RPC `work.docx_export`, CLI `export … --docx`): Formatvorlagen, echte Fußnoten,
    eingebettete Bilder, Typst-Inseln mit wörtlichem Quelltext; auf Wunsch der jüngste offene Vorschlag je Abschnitt
    als Änderungsverfolgung. Die eigene Datei kommt beim Import mit gleicher Gliederung und gleichem Text zurück.
  - Baruch beenden (`/beenden`, nur am eigenen Rechner, nicht über RPC/MCP), sauberes Herunterfahren auch auf
    SIGTERM, Starter mit App-Fenster, Pages in zwei Jobs.
  - Neues Crate `baruch-docx`; keine Migration, kein neues MCP-Werkzeug. In Microsoft Word ist nichts geprüft.
- Prüfung: `cargo test --workspace`; Vorschau `cargo run -p baruch-server`, im Browser
  http://127.0.0.1:8765/ (Daten in `~/.local/share/baruch` bzw. `--data`).
- Noch nicht: Handprüfung der Backends OpenAI-kompatibel und Ollama gegen echte Dienste (nur Nachbauten), Kommentare/Befunde im Abgleich nach MultiGPT, Markdown-Import. Kommentare
  (Abschnitts-Chat, Befunde) werden später ebenfalls im (Graph-)RAG abgelegt, damit Personas frühere
  Befunde finden; heute liegen sie in SQLite (`post` mit Verweisen) und im FTS5-Index.

## Erste Schritte nach Freigabe

1. Nutzer legt `Beerlesklopfer/baruch` privat an (oder gibt das Anlegen frei); Lesezugriff auf den
   Typst-Buchordner (heute nur für einen Benutzer lesbar) oder Kopie nach
   `~/Documents/Project/`, damit Vorlage und Import an den echten Quellen gebaut werden.
2. Lokal `~/Documents/Project/baruch`: Cargo-Workspace, CLAUDE.md (Regeln: Deutsch UI, Bezeichner
   Englisch, keine Tokens, Version nur vom Nutzer, Suite/Firefox/Agenten nie parallel),
   `docs/Plan.md` mit diesem Plan, `LICENSE` AGPL-3.0.
3. B1 umsetzen. MultiGPT bleibt unverändert; Baruch braucht nur einen API-Key mit `ask` und
   `docs.read`.

## Verifikation

- `cargo test`: Revisions-Invarianten (eine `current`, Vorschlag ändert `current` nicht, Übernehmen
  atomar), Import/Export-Rundreise Typst und Markdown, Lektor-Wahl nach Kontext, Schleifengrenze
  bei `@`-Aufrufen.
- Backend-Tests gegen nachgebildeten MCP-/OpenAI-Server, keine echten Kosten.
- Typst-Vorschau des importierten Buchs gegen das PDF aus dem bestehenden Build vergleichen.
- Echter Lauf: ein Kapitel aus dem Buch des Nutzers über MultiGPT (gemma auf Eliezer) prüfen lassen.
- Desktop-Bau auf Linux; macOS/Windows über CI (Plattform-Runner), erst in B5.

## Offen (Nutzer)

- Lesezugriff bzw. Pfad des Typst-Buchordners.
- Login nur bei Server-Variante relevant; Desktop zunächst ohne Konten.
- ~~Zielversion~~ (0.9.0, Runde 24). Zeitpunkt der Veröffentlichung und die Schritte davor (Historie,
  Archivschlüssel, Impressum: `docs/Backlog.md`, Nutzerfrage 17), Signierung für macOS/Windows (Nutzerfrage 10).
- ~~Git im Revisionssystem~~ → entschieden und umgesetzt (siehe oben). Offen nur: ein bestehendes Repo
  (Typst-Buchordner) mit seiner Historie einlesen.
