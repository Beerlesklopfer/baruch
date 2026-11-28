<!-- translation-of: docs/Plan.md source-commit: 8f54ca2 -->

# Baruch – freie Schreibwerkstatt für Bücher und Predigten {#baruch--free-writing-workshop-for-books-and-sermons}

> Dies ist die deutsche Übersetzung von `docs/Plan.md` (englisches Original, maßgeblich), Stand: Commit `8f54ca2`.

## Ausgangslage {#context}

Die Bücher des Autors werden heute über MultiGPT-Chats überarbeitet (Chat 59): ein PDF anhängen,
Kritik holen, Kapitel eins nach dem anderen umarbeiten. Dabei geht die Struktur verloren, Kontexte laufen über
(Eliezer HTTP 400, behoben in `958b796`/`e37814d`), und es gibt keine Revisionen je Abschnitt.

Entscheidungen des Nutzers (2026-11-05):
- **Neues Repo „Baruch“** (privat `Beerlesklopfer/baruch`, später frei), ein Neuentwurf, nicht in
  MultiGPT integriert; zunächst undokumentiert.
- ~~Rust, native Desktop-Oberfläche (Iced)~~ → **Rust-Server mit Browser-Oberfläche wie David**
  (2026-11-05): Der Kern ist ein RPC-Server, der das Skelett und die Vorlagen liefert; Widgets in TypeScript.
  Loco (Rails-artig, axum + SeaORM + Tera, Worker, JWT) geprüft und vorerst verworfen: Auch dort fehlt RPC,
  der Ballast passt nicht zu einem Einzelnutzer-Betrieb; für die Bibelschule (Anmeldung, Worker) später erneut prüfen.
- **Import über MCP zu MultiGPT** (2026-11-05): Baruch liest Quellen genau wie MultiGPT (dieselbe
  Aufbereitung, wichtig für die Recherche); keine eigenen Import-Parser. PDF, EPUB, MOBI usw. müssen in
  Baruch anzeigbar sein.
- **Literaturverwaltung wie Citavi** (2026-11-05): Quellen aus RAG und Web als Titel mit Zitaten;
  Belege im Text über Typst `@cite` (Kandidat: `hayagriva`, das Literaturformat von Typst, mit CSL-Stilen).
- **SQLite bleibt die Quelle** (2026-11-05, bestätigt): Werke, Revisionen, Chat, Personas und die gesamte
  **Autorenbibliothek** (Titel, Zitate, Belege) liegen dort; das LLM sucht darin (ein Werkzeug für die
  Personas, Volltext über FTS5, Bedeutung über LanceDB). LanceDB bleibt ein reiner Suchindex.
  *Umgesetzt (B1):* `<data>/baruch.sqlite` mit dem Kernschema (`baruch-store`) und der Autorenbibliothek einschließlich
  FTS5-Suche (`baruch-library`); seit Runde 12 LanceDB als semantische Suche (Index im Datenordner).
- **Git-Revisionssystem: beschlossen und umgesetzt** (2026-11-05, B1): SQLite bleibt die Quelle, Git
  spiegelt. Jede Übernahme und jedes eigene Speichern wird ein Commit im Typst-Ordner des Werks
  `<data>/works/<slug>/` (`baruch-gitmirror`, reines Rust über gix): Autor ist der Mensch oder die
  Persona, Committer der Mensch, Persona und Modell stehen als Trailer im Commit. Kein Commit ohne
  Änderung; scheitert die Spiegelung, bleibt die Änderung gültig, und der nächste Commit holt den vollen
  Stand nach. Der Ordner lässt sich klonen, pushen (Forgejo/GitHub, nur durch den Nutzer) und mit
  `typst compile main.typ` setzen. Offen bleibt nur das Einlesen eines vorhandenen Repos mit Verlauf.
- **Frei (AGPL-3.0)** – bewusst als Gegenentwurf zu bezahlten Werkzeugen wie Logos.
- Bildschirm: **Kapitelbaum links, Editor mit Revisionen je Abschnitt („‹ ›“) in der Mitte, Assistenten
  rechts.**
- Assistenten schreiben **nur Vorschlagsrevisionen**; eine Revision wird nur per Klick aktuell.
  **Revisionsbaum (Runde 3, umgesetzt):** je Abschnitt genau eine aktuelle Revision als Marke, jederzeit umschaltbar;
  Ändern und Löschen nur durch den Menschen, protokolliert im Git-Spiegel. Import nur über die
  Kommandozeile (`baruch-server import`), nicht über MCP (Entscheidung 2026-11-05).
  **Ausnahme Einfälle (IDEA_CRUD, umgesetzt in Runde 7):** Einfälle ändern keinen Buchtext; Assistenten und
  MCP dürfen sie anlegen, ändern und löschen, jede Aktion mit Herkunft als Commit im Spiegel.
  **Ausnahme Beiträge (Runde 8):** Beiträge sind ebenfalls kein Buchtext; der Mensch verschiebt, löscht und
  wandelt alle um, Assistenten und MCP nur ihre eigenen (Autor = Persona oder `@mcp:<name>`), je ein Commit. Offene
  Vorschläge stehen im Lesefluss in der Farbe ihrer Persona (Runde 7). Titel von Gliederungspunkten sind
  nie leer, einzeilig und höchstens 200 Zeichen lang (Runde 7).
  **Ausnahme Gliederung (Wunsch 2026-11-06, umgesetzt):** „Ein LLM hat den besseren Überblick durch seinen
  Context“ – Assistenten dürfen Gliederungspunkte **anlegen** (MCP `outline_add`, auch eine ganze Gliederung in
  einem Aufruf und einem Commit; Revision 1 und der Commit tragen die Persona oder den Client). **Umbenennen,
  Verschieben, Ebene ändern, Nummer an/aus und Löschen nur nach Bestätigung durch den Menschen** („UD nach
  Bestätigung“): Assistenten stellen einen Antrag (Migration 0012), der Mensch bestätigt oder lehnt ab
  (Gliederungsseite, RPC `outline.request_decide`); veraltete Anträge werden mit Begründung geschlossen.
- **Alle Personas sind miteinander verbunden**; die Persona mit dem **größten Kontext ist
  automatisch der Lektor**.
- Satz und **PDF-Vorschau mit Typst** wie im bestehenden Bau der Bücher.
- Import: **Typst-/Markdown-Kapitel, PDF, leer neu**.
- **Bibeltexte und Literatur über MultiGPT-RAG** (keine mitgelieferten Bibeln, keine Lizenzfragen).
- Ein zweiter Anwendungsfall gleichen Ranges: **Predigten**.

Name: Baruch, der Schreiber Jeremias, der die verbrannte Schriftrolle neu schrieb und erweiterte
(Jer 36,32) – Revisionen sind der Kern.

## Vorbilder (Ideen, kein Code) {#models-ideas-no-code}

**Elisa** (`Beerlesklopfer/elisa`): Personas als bearbeitbare Datensätze mit einem Modell je Persona
(`LLMRoleDescription`, `context_for()`); die Regel „ein LLM schreibt keinen verbindlichen Inhalt“ mit
Herkunft je maschinellem Beitrag (`generated_by_llm`, `llm_model`); ein nur anhängendes Revisionsprotokoll
(`app/history.py`); stabile Gliederungsnummern (`app/outline.py`); ein Chat je Abschnitt für
Begründungen; Prüfer Nathan (Befunde, nie Freigabe); Lektor-JSON-Vertrag (`write-time-feedback`);
Belegpflicht (`app/grounding.py`); Personas sprechen über ein Relay miteinander
(`list_personas`, lokale fragen externe um eine zweite Meinung).

**MultiGPT:** MCP-Werkzeuge `ask` (Modell, Kosten, Budget), `search_documents`, `upload_document`,
`read_document`, `list_models`; Kontextlänge je Modell (`context_budget.context_length`);
Versionsumschalter (`chat/templates/chat/_message_actions.html`) als Bedienmuster.

## Architektur {#architecture}

- **Server mit Browser-Oberfläche** (wie David): `baruch-server` auf axum 0.8, Seiten und Fragmente als
  minijinja-Vorlagen, der Kern über JSON-RPC 2.0 (`POST /rpc`, u. a. `ui.render`, `revision.accept`);
  Widgets in TypeScript (`data-widget`, von `build.rs` mit `tsc` kompiliert und eingebettet); ohne
  JavaScript bleibt alles über Links und Formulare benutzbar (`POST /act`). Typst-Vorschau später als
  Seitenbilder (`typst-render`) oder SVG. Der Kern als Bibliothek (`baruch-core`). Lokal auf dem Laptop
  oder auf dem Rechner der Schule betreiben.
- **Daten lokal in SQLite** (sqlx, Migrationen), eine Datei je Datenordner (`<data>/baruch.sqlite`),
  einschließlich der Autorenbibliothek (Literatur, Zitate, Belege); den Personas als Suchwerkzeug freigegeben
  (RPC `library.search`). Die Datenbank selbst sichert die Regeln: genau eine `current` je
  Abschnitt (partieller Index); ein Elternteil (`based_on`) nur auf eine frühere Revision desselben Abschnitts,
  „aktuell“ umschaltbar von `proposed`/`rejected`/`superseded`, Herkunft unveränderlich, Löschen nie an
  der aktuellen Revision, Nummern nie wiederverwendet (Trigger, Migration 0004; ersetzt „nur anhängend“ aus
  0001 und die Statusregel aus 0003). Ändern und Löschen nur durch den Menschen (Server, nicht über MCP).
- **Git-Spiegel je Werk** (`<data>/works/<slug>/`, ein Git-Repo mit `main.typ` und `abschnitte/NNN-slug.typ`):
  ein Abbild der aktuellen Revisionen nach jeder Änderung; Pfade und Zwischenordner werden geprüft
  (kein `..`, kein `.git`, kein Symlink), damit nichts außerhalb des Spiegels geschrieben oder gelöscht wird.
- **Vektorsuche eingebettet mit LanceDB** (Rust, ohne Server): Texte, Revisionen und Kommentare
  für die Suche der Personas; MultiGPT-RAG zusätzlich für Bibeln und Literatur.
- **Modelle:** jede OpenAI-kompatible Adresse (Ollama, LM Studio, Eliezer, kleine 3B-Modelle
  lokal) oder ChatGPT/Claude mit eigenem Schlüssel; MultiGPT optional.
- **Typst eingebettet** (Crate `typst`): Live-Vorschau als PDF/Seitenbilder neben dem Editor, Export
  PDF; Vorlagen „Buch“ und „Predigt“ mitgeliefert, Schriften mitgeliefert (offline).
- **Austauschbare Modellanbieter** (`trait Backend`): MultiGPT (MCP, Schlüssel je Konto; Kosten/Budget dort),
  OpenAI-kompatibel (Eliezer, Ollama, LM Studio), Anthropic/OpenAI direkt mit eigenem Schlüssel. Jedes
  Backend meldet Modelle und Kontextlänge.
- **Austauschbare Wissensquelle** (`trait Knowledge`): zuerst MultiGPT-RAG (`search_documents`, Bibeln
  und Literatur als Sammlungen); ohne MultiGPT bleibt die Bibelsuche aus und wird als solche angezeigt.
- Schlüssel im Schlüsselbund des Systems (Crate `keyring`), nie im Klartext in der Datenbank.
- Oberfläche deutsch (Englisch vorbereitet, i18n), Bezeichner englisch, kein Inline-JS.

## RAG-Abgleich mit MultiGPT {#rag-reconciliation-with-multigpt}

Baruch arbeitet offline mit dem eingebetteten LanceDB und gleicht mit MultiGPT ab, sobald dieses
erreichbar ist (MCP, Schlüssel je Bibliothek).

- **Was abgeglichen wird:** Texte, nicht Vektoren. Die MCP-Schnittstelle liefert keine Vektoren,
  und die Embedding-Modelle können sich unterscheiden; jede Seite bettet selbst ein (Standard
  `nomic-embed-text`, 768 Dimensionen wie MultiGPT – dann sind Suchergebnisse vergleichbar).
- **Baruch → MultiGPT:** aktuelle Revisionen und Kommentare/Befunde je Werk als Dokumente in eine
  eigene Sammlung („Baruch: <Werk>“) über `upload_document`; ein Dokument je Kapitel, bei einer
  Änderung ersetzt (`delete_document` + neu). So finden auch MultiGPT-Chats den Stand.
- **MultiGPT → Baruch:** gewählte Sammlungen (Bibeln, Literatur) über `list_documents` /
  `read_document` holen und lokal einbetten; danach funktioniert die Bibelsuche auch offline.
- **Kennung und Änderungen:** je Dokument eine stabile Id (Werk/Kapitel oder MultiGPT-Dokument-Id)
  und ein Inhaltshash; abgeglichen wird nur, was sich seit dem letzten Lauf geändert hat
  (`sync_state`: Seite, Id, Hash, Zeit). Sind beide Seiten gleich, gewinnt die Quelle:
  eigene Werke gehören Baruch, Sammlungen gehören MultiGPT – kein Zusammenführen.
- **Bedienung:** Knopf „Abgleichen“ und Anzeige „zuletzt abgeglichen …“; Fehler (offline, Budget)
  nur als Hinweis, die Arbeit geht lokal weiter.
- **MultiGPT:** braucht dafür nichts Neues außer einem API-Schlüssel mit `docs.read`/`docs.write`; prüfen,
  ob `list_documents` eine Änderungszeit oder einen Hash liefert, sonst als kleine Erweiterung vermerken.

## Anwendungsfall Bibelschule {#use-case-bible-school}

- **Schule:** ein großer Rechner mit MultiGPT und Eliezer (große Modelle, gemeinsame Sammlungen:
  Bibeln, Kommentare, Skripte der Lehrer, Bibliothek).
- **Studierende:** Baruch auf dem eigenen Laptop mit einem lokalen LanceDB; gleicht die Sammlungen ihres
  Kurses mit MultiGPT ab und arbeitet offline mit kleinen lokalen Modellen (3B) weiter. Im
  Schulnetz laufen die Personas auf dem Rechner der Schule (großer Kontext, dort der Lektor).
- **Berechtigungen über MultiGPT:** ein Konto und API-Schlüssel je Studierendem; welche Sammlungen er lesen darf, regelt
  MultiGPT (Rollen, Freigaben). Baruch legt den Schlüssel im Schlüsselbund ab.
- **Abgabe und Betreuung:** Ein Studierender kann ein Werk (Predigtentwurf, Hausarbeit) in eine
  Kurssammlung abgeben; Lehrer sehen den Stand und die Befunde, ihre Kommentare kommen beim nächsten
  Abgleich als Kommentare zurück (nie als Änderung am Text).
- **Kosten:** große Modelle der Schule über deren Budget in MultiGPT; ChatGPT nur, wenn der Studierende
  einen eigenen Schlüssel einträgt.

## Datenmodell (SQLite) {#data-model-sqlite}

- `work` (kind = book|sermon, title, subtitle, audience, purpose, bible_version, markup =
  typst|markdown, template, knowledge_collections[])
- `node` (work, parent, position, number „6.2“, title, kind = part|chapter|section|sermon_part)
- `revision` (node, number, body, author_kind = human|assistant, persona, model, based_on,
  status = proposed|current|rejected|superseded, note, edited, created) – ein Baum je Abschnitt (`based_on` =
  Elternteil), genau eine `current` je Abschnitt als umschaltbare Marke; die aktuelle Revision kann nicht
  gelöscht werden (Migration 0004); seit Runde 22 ergänzt der Mensch seine aktuelle Revision beim Speichern im
  Editor (`amend_seq`, Verlauf `revision_amend` mit Text davor/danach, Rückgängig als neuer Schritt), solange
  keine Regel eine neue Revision verlangt (Einstellung je Werk `work_revision_policy`, Migration 0050)
- `thread` / `post` (node, persona|human, reply_to, body, sources[], created) – der gemeinsame
  Abschnitts-Chat aller Personas (ihre Verbindung untereinander)
- `persona` (handle „@exeget“, name, role, prompt, backend, model, context_length, active)
- `job` (node, persona, kind = review|propose|audit|ask_persona, status, error)

## Verbundenes Persona-Team {#connected-persona-team}

- **Lektor automatisch:** die aktive Persona mit dem größten gemeldeten Kontext; sie bekommt das
  ganze Kapitel oder die ganze Predigt, moderiert, sammelt die Befunde der anderen und schreibt die
  Vorschlagsrevision. Wechselt das Modell, wechselt der Lektor; anzeigen, wer es gerade ist.
- **Verbindung:** Personas schreiben in den Abschnitts-Chat und rufen einander per `@handle` auf
  (Auftrag `ask_persona`); der Lektor sieht alle Beiträge. Grenze gegen Endlosschleifen: Runden je
  Auftrag und eine Kostengrenze.
- **Startteam** (Prompts bearbeitbar): Exeget (Text, Kontext, Kernaussage, mit RAG),
  Schrift (Bibelstellen und Wortlaut belegen, sonst „nicht belegt“), Belege (Quellen, Fußnoten,
  „Angabe prüfen“), Nathan/Auditor (theologisch verletzliche Stellen, Regeln zu Gewalt/Schutz,
  Notrufnummern; nur Befunde), „Hörer“ (liest als Zielgruppe), für Predigten zusätzlich Zeitwächter
  (Länge in Minuten) und Anwendung (Bezug zum Alltag).
- **Belegpflicht der Persona Belege** (Erkenntnis aus MultiGPT-Chat 59): Recherche durch die Modelle
  liefert fremde, oft nicht überprüfbare Quellen. Deshalb braucht jede Tatsachenbehauptung einen Beleg als
  `@schlüssel` aus der Literatur (Autorenbibliothek, Typst `@cite`) oder den Vermerk „nicht belegt“.
  Neue Quellen kommen zuerst als Titel mit Zitat in die Literatur, dann in den Text.
- **Eigener Endpunkt je Persona, auch als Server** (2026-11-05):
  - *Ausgehend:* Jede Persona hat einen eigenen Endpunkt (`endpoint`: Adresse, Art `openai`|`mcp`|`multigpt`,
    Modell; Schlüssel je Persona im Schlüsselbund). So laufen Personas auf verschiedenen Rechnern (Laptop,
    Schulrechner, Eliezer); der Endpunkt meldet die Kontextlänge, daraus folgt der Lektor.
  - *Eingehend:* **ein gemeinsamer MCP-Server** in `baruch-server` (`POST /mcp`, mit Token) für alle
    Personas: Werkzeuge u. a. `list_personas`, `ask_persona {handle, …}`, Suche im Werk und in der
    Autorenbibliothek (nur lesend). Damit rufen andere Werkzeuge, andere Baruch-Instanzen oder die
    Personas untereinander (`@handle`, Relay wie in Elisa) eine Persona auf – mit ihrem Prompt und
    ihrem Endpunkt. Ergebnisse bleiben Vorschläge oder Beiträge.
    **Chatten über MCP:** Werkzeug `chat {handle?, node?, message, thread?}` – ein Werkzeug (z. B. Claude
    Code, MultiGPT, ein Skript) chattet mit einer Persona oder dem ganzen Team; mit `node` landet die
    Unterhaltung im Abschnitts-Chat (Autor = Name des Werkzeugs oder Tokens), sonst in einem eigenen Faden.
    Der Lektor sieht sie ebenfalls; `@handle` im Text ruft weitere Personas auf (Rundengrenze und Kostengrenze gelten).
- **Das Gedächtnis ist der Lektor** (2026-11-05): Er kennt das ganze Werk und die Autorenbibliothek und
  meldet „du hast schon in … geschrieben, dass …“ – Wiederholungen, Widersprüche, Querbezüge. Er sucht in SQLite
  (FTS5) und LanceDB; jeder Beitrag trägt Verweise auf die Stelle (`TextRef`: Abschnitt, Revision,
  Wortlaut), in der Oberfläche als Link zum Hinspringen. Dafür gibt es keine eigene Persona.

## Bildschirm {#screen}

- **Links:** Kapitelbaum oder Predigtgliederung, Umordnen durch Ziehen, Marken für offene Vorschläge
  und Befunde.
- **Mitte:** der Abschnitt im Editor (Satzbild nach der Werkvorlage, umschaltbar auf Typst oder Markdown), Kopf „‹ 3/5 ›“ für
  Revisionen, Vorschläge im Lesefluss in der Farbe der Persona mit Diff und „Übernehmen / Verwerfen“; umschaltbar auf die
  Typst-Vorschau (Seite) des Kapitels.
- **Rechts:** Personas (Lektor markiert), „Prüfen“ / „Vorschlag“ für Abschnitt oder Kapitel,
  darunter der gemeinsame Abschnitts-Chat mit Quellen.

## Etappen {#stages}

Gesamtbild in der Zeit (fertige Runden aus Git, geplante geschätzt): `docs/Fahrplan.md`; Projekte in
`docs/Backlog.md`.

1. **B1 Skelett:** Repo, Workspace (`baruch-core`, `baruch-server`), SQLite, Werke, Kapitelbaum,
   Revisionen von Hand mit „‹ ›“, Import Typst/Markdown nach Überschriften (`=`/`#`), Export als
   Typst-Ordner, Typst-Vorschau und PDF.
2. **B2 Assistenten:** Backends (zuerst MultiGPT-MCP, OpenAI-kompatibel), Personas, automatischer
   Lektor nach Kontext, Abschnitts-Chat mit `@`-Aufrufen, Vorschlagsrevisionen mit Diff. Umgesetzt in Runde 10
   mit Eliezer (OpenAI-kompatibel) als erstem Backend; in der Erprobung seit 2026-11-06 ist der Lektor fest `@lektor`
   (umschaltbar auf „automatisch nach Kontext“).
3. **B3 PDF-Import:** über MultiGPT `upload_document`/`read_document` (oder lokal, wenn ohne
   MultiGPT), Gliederung mit Rückfrage. Umgesetzt in Runde 21 (integriert 2026-11-09): PDF über MultiGPT oder lokal
   (`pdftotext`), EPUB immer lokal (laut seiner Werkzeugbeschreibung nimmt MultiGPT kein EPUB an), Vorschau mit bearbeitbarer
   Gliederung, angelegt erst nach der Bestätigung des Menschen (Werk, Quelle der Bibliothek oder beides); dazu Markdown,
   ZIP und Ordner mit Bildern. Die MultiGPT-Annahmen sind ungeprüft (manuelle Prüfung durch den Nutzer).
4. **B4 Predigt:** Werkart und Typst-Vorlage (Text, Einleitung, Teile, Anwendung, Schluss),
   Zeitwächter, Stichwortzettel als Export. Umgesetzt in Runde 16 (integriert 2026-11-08): Vorlage mit einem Predigttext
   aus SWORD, der Lektor kennt die Predigt, Sprechzeit mit Zielzeit (Baum, Editorkopf, Gliederung, RPC/MCP),
   Stichwortzettel als PDF (A5, Karten; Seite, RPC, CLI).
5. **B5 Verteilung:** Server-Pakete (.dmg, .msi, .deb/.AppImage), Signieren offen, AGPL-Hinweise,
   Aktualisieren.

## Heute (Stand 2026-11-05) {#today-state-2026-11-05}

- **B1-Ausschnitt erledigt:** `baruch-server` (axum, minijinja, TypeScript-Widgets) mit einer dreispaltigen
  Oberfläche, hellem/dunklem Theme, Alt+←/→, verschiebbaren Spalten; Daten in SQLite statt fest im
  Code (Beispielwerk beim ersten Start), Übernehmen/Verwerfen/Speichern je eine Transaktion, gefolgt von einem
  Commit im Git-Spiegel. Literatur und Volltextsuche über Werk und Bibliothek als Popup der oberen Leiste
  (seit Runde 3; Alt+L, ohne JavaScript die Seiten `/literatur` und `/quelle`, `?lq=…`; Treffer als
  `<mark>`, Trefferlinks behalten Werk und Suche, Titel mit Treffern aufgeklappt).
- **Härtung:** NUL- und Steuerzeichen in der Suche ergeben eine leere Suche statt eines Serverfehlers;
  Statusübergänge per Trigger; Symlinks im Spiegel werden abgelehnt; der Spiegel holt nach einem Fehler
  mit dem nächsten Commit auf (Tests).
- **Runde B1.5 erledigt:**
  - MCP-Werkzeuge über `POST /mcp` (`list_works`, `outline`, `read_node`, `search`, `propose_revision`,
    `add_post`, `list_personas`, bis Runde 3 auch `import_typst`; ohne Token nur vom eigenen Rechner). `ask_persona` und
    `chat` bleiben Stubs (B2).
  - Typst-Import eigener Bücher: CLI `baruch-server import/export` und (bis Runde 3) MCP `import_typst`, die Werkvorlage
    bis zur ersten Überschrift in `work.template`, `=`/`==`/`===` als Gliederung, Rundreise Byte für
    Byte, Doppelschutz über `source_sha256`.
  - Mehrere Werke mit Werkwahl in der oberen Leiste, ein Git-Spiegel je Werk.
  - Offen: `#let`-Definitionen der Vorlage gelten nicht in den per `#include` eingebundenen Abschnittsdateien des Spiegels
    (Entscheidung beim Nutzer); `#heading(...)` vor der ersten
    `=`-Überschrift bleibt Teil der Vorlage; Markdown-Import.
- **Runde 3 erledigt (revtree · litpopup · small):**
  - Revisionsbaum: ein Elternteil je Revision, „aktuell“ umschaltbar (`revision.set_current`), Ändern und Löschen
    nur durch den Menschen (`revision.update`/`revision.delete`), je ein Commit im Spiegel mit
    `Baruch-Action`, Baumansicht und Vorschau mit `pick`. Nummern bleiben eindeutig, mit Lücken.
  - Literatur als Popup (obere Leiste, Alt+L, ohne JavaScript `/literatur`), Treffer öffnen die Fundstelle,
    Dokumentansicht `/quelle`, Markierungen (Zitat, Kernaussage, Widerspruch, zu prüfen), Zitat als
    Vorschlagsrevision.
  - Kleinigkeiten: Port 8765, Datenordner `dirs::data_local_dir()`, `import_typst` nicht mehr in MCP, Vorlage
    als `vorlage.typ` im Spiegel, Slug an Wortgrenzen, Beispielwerk umbenannt (nur neue Datenbanken),
    Parsergrenzen festgelegt.
- **Runde 4 erledigt (works · outline · fixes):**
  - Werke verwalten: eindeutige Titel (Migration 0005, `title_key`), die Werkwahl öffnet sofort, Cookie
    `baruch_work`, Seite `/werke`, Anlegen (Buch/Predigt), Ändern (Metadaten, Werkvorlage mit Revisionen),
    Löschen mit eingetipptem Titel (Spiegel archiviert unter `works/.geloescht/`), Upload-Import und Export als
    Download, CLI `import --title`. Nur der Mensch, nicht über MCP.
  - Gliederungsverwaltung: Ebenen (Teil, Kapitel, Abschnitt, Unterabschnitt), Verschieben, Ebene ändern, Anlegen,
    Umbenennen, Löschen (RPC `outline.*`, Formulare ohne JavaScript, Drag and Drop), Nummerierung nach der
    Werkvorlage oder konfigurierbar (Migration 0006), aufgelöste Typst-Escapes in Titeln, Überschriften im
    Editor passen die Gliederung an; ein Commit im Spiegel je Änderung.
  - Nacharbeit: Löschen braucht `confirm`, die Meldung nennt vorhandene Revisionen, Zitieren ohne Abschnitt
    abgelehnt, Markierungen robust gegen Leerraum, der Volltextindex folgt gelöschten/verschobenen Beiträgen (0105),
    Tests zur Nebenläufigkeit.
- **Runde 5 erledigt (tree · wysiwyg · r4):**
  - Eine Gliederung: der Baum links mit Drag and Drop, Kontextmenü, Umbenennen an Ort und Stelle (F2,
    Doppelklick), Anlegen, Ebene ändern, Löschen und Nummerierung im Kopf; `/gliederung…` jetzt nur noch ohne JavaScript.
    Verschieben behält die Ebene. Umschalten und Übernehmen lassen die Gliederung der Revision folgen, auch in der
    Ebene (Unterpunkte werden mit umgestuft, ein Commit); die Revision selbst bleibt unverändert (Nutzerentscheidung
    2026-11-05). Römische Kapitel auch aus einer Nummerierungsfunktion der Vorlage.
  - Satzbild-Editor, erster Schritt: Crate `baruch-wysiwyg` (Blöcke, Typst-Inseln, Rundreise byteidentisch), RPC
    `wysiwyg.*`, Widget mit dem Schalter „Satzbild | Quelltext“, Stil aus der Werkvorlage, Satzbild-Diff der
    Vorschläge; ohne JavaScript nur der Quelltext. Beschreibung: `docs/WYSIWYG.md`.
  - Nacharbeit: Werk-Ids nie wiederverwendet (Migration 0007, `AUTOINCREMENT`), Slugs gelöschter Werke
    reserviert, keine erneute Befüllung nach dem Löschen aller Werke, MCP und Zitieren nur mit dem festen Schlüssel `key`,
    `work.import` über `/rpc` bis 16 MiB Text.
- **Runde 6 erledigt (numbering · satzbild · images):**
  - Gliederungsschemata je Werk (Migration 0008, zwölf Schemata von „aus der Werkvorlage“ bis „Predigt“ und
    „eigenes Muster“ in einer Mustersprache mit `P|k` und `P1;P2;…`), Seite „Gliederungsschema“ mit Vorschau,
    ohne JavaScript; Gliederung, Editorkopf, Export und Spiegel folgen, „aus der Werkvorlage“ byteidentisch.
    `heading_numbering` liest nur die letzte Regel der obersten Ebene und erkennt Tiefenbedingungen.
  - Satzbild: `#szene`/`#arbeit` als Blöcke mit bearbeitbaren Absätzen, Labels an der Überschrift, Stil nur aus
    Regeln der obersten Ebene, kein leerer Bereich im Quelltext, ein Klick öffnet das Satzbild, Einfügen mehrerer
    Absätze, Bilder in Inseln mit Alternativtext.
  - Bilder als Text (Migration 0009: Base64 oder SVG, nie BLOB) mit Revisionen wie Abschnitte, Seite „Bilder“,
    RPC `asset.*`, MCP nur lesend, Import der Bilder über die CLI und `export --out` byteidentisch.
- **Runde 8 erledigt (inline · posts · typst):**
  - Vorschläge an ihrem Platz im Lesefluss (ausgerichtet an der gezeigten Revision, `baruch_wysiwyg::align`),
    das Satzbild schon im Server-HTML als Standard, Linien mit Kontrast 3 : 1.
  - Abschnitte über den festen Schlüssel adressiert (`/?work=W&key=K`, MCP-Feld `url`); Beiträge löschen,
    verschieben, zu Einfällen machen (der Mensch alle, MCP nur die eigenen; eine Ausnahme wie bei Einfällen, Beiträge sind
    kein Buchtext); Chat-Marken.
  - Typst-Vorschau mit der Werkvorlage: Crate `baruch-render` (Typst 0.15.1 eingebettet, offline, SVG), Seite
    `/werke/{id}/satz`, RPC `preview.typst`, MCP `typst_check` (nur lesend).
- **Runde 10 erledigt (B2: backend · personas · scripture):**
  - Modell-Backend Eliezer: Crate `baruch-llm` (gestreamte Chat-Antworten, eine Anfrage zur Zeit, Grenzen,
    Aliase), `models.rs` (RPC `model.*`, kurzer Test), Vergleich der Persona-Modelle beim Prüfen der Verbindung.
  - Personas im Ernst: Prüfen, Vorschlag und „@persona fragen“ im Abschnitts-Chat, die Antwort Wort für Wort
    (`/assistenten/stream`, ohne JavaScript `/assistenten/act`), Rollen-Prompts mit harten Regeln (Migration
    0015), der Lektor fest @lektor (Erprobung) oder automatisch, der Lektor kennt die Einfälle, MCP `ask_persona`/`chat`.
  - Bibelstellen: Crate `baruch-scripture` (deutsche Verszählung der Schlachter 1951 nach SWORD „German“), RPC
    `scripture.check`, MCP `scripture_check`; der Wortlaut bleibt „ungeprüft“. MCP-Token für andere Rechner
    (Migration 0016, nur Hash).
- **Runde 11 erledigt (Bilder und Umschlag: mcp-assets · cover · images):**
  - Buch als PDF (`typst-pdf`, dieselbe Seitenzahl wie die gesetzte Seite, RPC `work.pdf`, `/werke/{id}/pdf`, CLI
    `export --pdf`), Druckprüfung (fehlende Bilder, Rasterbilder unter 300 dpi).
  - Umschlag je Werk mit Vorder-/Rückseite im Satz, Vorspann mit Impressum aus den Buchdaten (Migration 0017),
    Umschlag-PDF für die Druckerei mit Rücken und Beschnitt, EAN-13 aus der ISBN.
  - Bilder über MCP: `asset_add` sofort, Ersetzen/Umschlag/Löschen nur als Antrag (Migration 0018), den der
    Mensch auf der Seite „Bilder“ bestätigt. Bildersuche, Vorschaubilder, Bild einfügen, Kapitelbild im Editor.
  - „Einstellungen → Assistenten“ mit einem Reiter je Persona und Textfeldern für Rolle und Rollen-Prompt;
    Vervollständigung „@…“ im Abschnitts-Chat.
- **Runde 12 erledigt (Verbindungen nutzen, RAG: lancedb · pgvector · multigpt):**
  - Semantische Suche je Werk umschaltbar: LanceDB lokal im Datenordner (Haupt-RAG; Abschnitte in der aktuellen
    Revision, Einfälle, Markierungen, Autorenbibliothek; Embeddings über Eliezer, eine Anfrage zur Zeit),
    pgvector direkt (nur lesend) und MultiGPT-MCP; Treffer zusammengeführt (Reciprocal Rank Fusion) ohne Duplikate,
    ein gescheiterter Weg blockiert nie, die Wortlautsuche (FTS5) bleibt immer.
  - Sammlungen je Werk (Migration 0031), Status der Wege, „Index neu aufbauen“; Umschalter Wortlaut/Bedeutung in
    Literatur und Einfällen; MCP `rag_search`/`rag_status` (nur lesend); @belege und der Lektor bekommen Treffer
    als Belege `[Rn]` (Migration 0032), Treffer sind Daten, nie Anweisungen.
- **Runde 13 erledigt (Quelldokumente und Websuche: bau · quellen · websuche · abgleich):**
  - Server-Integrationstests als eine Binärdatei (`target/` 38 → 9 GB).
  - Volltext einer Quelle aus MultiGPT in der Dokumentansicht (seitenweise Teile, Fundstelle hervorgehoben, Übernahme in
    die Bibliothek), Prüfen von Markierungen ohne Zitat gegen den Volltext (Migration 0106), MCP `source_read`
    (nur Dokumente aus gewählten Sammlungen); eine gemeinsame MCP-Sitzung mit einem Pool.
  - @belege recherchiert auf Wunsch im Web über MultiGPT (`web_search`, `fetch_url`; bevorzugte Quellen zuerst);
    Baruch prüft jedes Zitat gegen die geholte Seite und setzt die Quellenzeile selbst (Migration 0034); nicht
    über MCP, nicht für einen Vorschlag.
  - Abgleich Baruch ↔ MultiGPT (Migration 0033): aktuelle Revisionen je Kapitel nach „Baruch: <Werk>“ nur nach
    Bestätigung, gewählte Sammlungen als lokale Kopie im LanceDB-Index (offline durchsuchbar), Seite „Abgleichen“.
    Abweichung: Kommentare/Befunde gehen noch nicht hinaus (Nutzerfrage im Backlog).
- **Runde 14 erledigt (Teile und Gliederung: teile-typst · teile-gliederung · nummerierung):**
  - Teile im Typst-Text als Marker `#heading(level: 1, numbering: none)[T] <baruch-part>`, „ohne Nummer“ als
    `… <baruch-unnumbered>`; Import, Export (byteidentisch), Spiegel, Satzbild und Satz (ein Teil allein auf einer rechten
    Seite, Kapitel zählen über Teile hinweg weiter). Vorhandene Texte werden nie umgewandelt.
  - Teile anlegen in jedem Werk, auch mit Werkvorlage (Baum, Formular, RPC `outline.add` mit `level`, MCP
    `outline_add` mit `level: 0` – Assistenten dürfen anlegen); Umbenennen schreibt die Markerzeile, Umschalten
    gleicht ab; Anträge der Assistenten mit Vorschau (auch ohne JavaScript); `idea.process` atomar als Gliederungspunkt.
  - Nummerierung je Ebene aus einer Quelle (`Numbering`): Funktionen der Werkvorlage je Ebene ausgewertet, Schemata
    mit Wörtern (Juristisch, Predigt, …) kommen beim erneuten Import als dasselbe Schema zurück; Kapitel ohne Nummer stehen
    in Export, Spiegel und Satz als Marker (die DB bleibt), Typst zählt wie Baruch.
- **Runde 15 erledigt (Lektor und Personas: lektor · anbieter · auftraege; 2026-11-08, Review nachgearbeitet):**
  - Der Lektor nach Plan: das ganze Kapitel (bei Predigten das Werk) im Wortlaut, Befunde der anderen Personas, Prüfen,
    Fragen und Vorschlag für einen Abschnitt oder ein ganzes Kapitel (ein Vorschlag je Teil, nur `proposed`). Passt der Kontext nicht,
    kürzt Baruch nie stillschweigend: entferntere Teile zusammengefasst, zu lange Abschnitte in überlappenden Stücken geprüft,
    der Hinweis „Komprimiert: …“; jeder Aufruf wird vor dem Senden gemessen, und was auch komprimiert nicht
    passt, wird ehrlich abgelehnt. „Kritisch:“ wird geprüft und noch einmal angefragt, im Vorschlag entfernt.
  - Modell-Backends frei je Persona: Eliezer, beliebige OpenAI-kompatible Dienste und Ollama (Migration 0036), „lokal“
    oder „extern“ (beim Anlegen nach der Adresse, Warnung bei einer als lokal markierten öffentlichen Adresse), Buchtext an externe
    Verbindungen nur nach Freigabe je Persona – auch keine Embeddings über ein externes Eliezer; Kontextlänge
    mit der Gegenstelle verglichen (Ollama: die wirksame Länge, nie das Maximum des Modells).
  - Aufträge für Personas: Chat-Strom mit dem eigenen Beitrag sofort, Hinweisseite ohne JavaScript, ein Einfall an eine gewählte
    Persona, Literatur „an Persona geben“, Fäden ohne Abschnitt (Migration 0038), MCP `chat` mit einer Warteschlange je
    Client.
  - Nutzerwünsche 2026-11-07: **Bibeln in der Literatur** (SWORD-Bibeln im Literatur-Popup und unter
    `/literatur/bibel`, Treffergruppen „Bibel“ und „Kommentar“, Zitat als Vorschlagsrevision) und **SWORD als
    Recherchewerkzeug** (Kommentare und Lexika je Werk, Migration 0037; ein Faktenblock aus Bibeltext, Kommentaren und
    Lexika für @belege und den Lektor, nummerierte Belege; MCP `commentary_lookup`/`lexicon_*`).
  - Schutz: Keine Seite kann von fremden Seiten eingerahmt werden (`X-Frame-Options`, `frame-ancestors`).
- **Runde 17 integriert (Bibelstellen, Rest von SWORD, Rest des Lektors: verszaehlung · stellen · pruefanzeige; 2026-11-08):**
  - Alle 18 Verszählungen von SWORD und eine geprüfte Abbildung zwischen ihnen (Psalmüberschriften, Kapitelgrenzen
    wie Joel 2/3, Mal 3/4 …; nie geraten), RPC/MCP `scripture.map`; Bibeln in einer anderen Verszählung werden über die
    Abbildung gelesen (Wortlautprüfung, Predigttext, Nachschlagen).
  - Stellen-Parser: Kommalisten, Tageszeiten und Zählungen sind keine Stellen, „Psalm 139, … Vers 23“, Zitate über
    Absätze hinweg und verschachtelt, Blockzitate in Szenen; 4. und 5. Mose im Fließtext.
  - Prüfanzeige: „Zitate prüfen“ im Editor (Hervorhebung im Satzbild, Liste; ohne JavaScript eine eigene Seite), RPC
    `bible.marks`; verschlüsselte (Schlüssel nur im Schlüsselbund) und BZIP2/XZ-gepackte SWORD-Module, Lizenzhinweis
    überall, Kommentar-Widget.
  - Lektor: Zitatprüfung nur gegen den gezeigten Text, falsche Stellenbehauptungen in „Kritisch:“-Befunden markiert,
    Einfälle gerahmt, wirksame Kontextlänge ohne Netzabfrage beim Laden der Seite.
- **Runde 26 integriert (Austauschformat zwischen Instanzen, XML: ids · export · import; 2026-11-08, vor
  R25 und R18 vorgezogen):**
  - Jedes Stück eines Werks hat eine global eindeutige Kennung (UUID, Migrationen 0041/0042), gelöschte hinterlassen
    Grabsteine; jede Instanz hat eine Kennung und einen Namen.
  - `.baruch` (ZIP mit `werk.xml`, Bildern und einem Manifest mit SHA-256) oder `.baruch.xml`, Namensraum
    `urn:baruch:werk:1` mit XSD; alle Revisionen, Vorlage, Einfälle, Beiträge, Fäden, Bilder, Buchdaten. Nicht im
    Format: Schlüssel, Verbindungen, Persona-Modelle, offene Anträge, fremde Texte (`docs/Austauschformat.md`).
  - Einlesen mit Vorschau: neu oder abgleichen (nur hinzufügen, nie löschen oder überschreiben, keine eingelesene Revision
    wird aktuell, Gliederungsänderungen als Anträge); sicher gegen DTD, Bomben und Pfadtricks. Wege: „Werk ▾“,
    RPC, Kommandozeile; MCP nur Ausgabe.
- **Runde 25 integriert (Ideenkarte: Mindmap der Einfälle; karte-daten · karte-bild · karte-widget; 2026-11-08,
  nach R26 und vor R18):**
  - Seite `/werke/{id}/karte` („Werkzeuge ▾ → Gedankenkarte“): das Werk in der Mitte, Teile und Kapitel im Ring, Abschnitte
    ins Kapitel gesammelt (aufklappbar), Einfälle an ihrem Bezug, „Ohne Kapitel“ als Wolke; Filter nach Status,
    Stichwort, Herkunft und Volltext per GET; Gliederungsliste und die Liste „Widersprüche“; Ausgaben SVG, PDF, Markdown
    und FreeMind (`.mm`). Vollständig über Formulare ohne JavaScript benutzbar.
  - Verbindungen zwischen Einfällen (führt weiter, gehört zu, Beispiel für, widerspricht; Migration 0043) für den Menschen,
    Persona und MCP (`idea_link`, `idea_unlink`, `idea_links`, `idea_map`), jede Änderung ein Spiegel-Commit, Zeile
    „Verbindungen“ in `einfaelle.md`; Positionen auf der Karte (nur Anzeige, ohne Commit); Nähe aus den gespeicherten
    Embeddings und gleichen Stichwörtern (nur angezeigt, keine Embedding-Anfrage beim Zeichnen).
  - Widget: Zoom, Verschieben, Ziehen eines Einfalls (auf ein Kapitel = Bezug, auf einen Einfall = Verbindung, sonst Position), Tastatur
    (Pfeile, Enter, V, N, Alt+E), Seitenleiste.
  - Austauschformat Nebenversion 2: Verbindungen und Positionen in `<karte>` (XSD `baruch-werk-1-u2.xsd`).
  - Gruppen benennen durch einen Assistenten: seit Runde 19b (ein Vorschlag, der Mensch übernimmt ihn als Stichwort).
  - Noch nicht: eine Karte über Werke hinweg (Nutzerfrage).
- **Runde 18 integriert (Editor und Menüband: menueband · konstrukte · satzbild-werk; 2026-11-08):**
  - Menüband zum Bearbeiten: Formate der Werkvorlage als Knöpfe (`#szene`, `#arbeit` nur wenn die Vorlage sie
    definiert; Fußnote, Zitat, Liste, Aufzählung, Begriff, Umbruch, Link immer), „Bild“ öffnet „Bild einfügen …“; im
    Satzbild führt das Widget den Befehl aus, im Quelltext wird der Typst-Text an der Cursorposition eingefügt.
  - Satzseite ohne JavaScript (‹ › mit `aria-label`, PDF, „Satz abbrechen“ als Formular), Typst-Lauf abbrechbar
    (`preview.cancel`; Auswertung und Layout laufen in Typst bis zum Ende, gespart wird das Schreiben der Seiten), fremde
    Seiten lösen keinen Satz und kein PDF aus (`Sec-Fetch-Site`); Schriftsuche beim Start; die obere Leiste bricht in schmalen
    Fenstern um; Ideenkarte: Kanten mit Beschriftungen, ein Pfeil für gerichtete Arten und ein eigener Strichstil.
  - Satzbild: Aufzählungen, Begriffslisten, verschachtelte Listen, Umbruch `\`, `#link`, Verweise `@…` als Blöcke statt
    Inseln; Inseln an Ort und Stelle bearbeitbar; mehrere Bilder je Block; Markdown-Überschriften auch beim Wechsel
    einer Revision; Lektor: ausgelassene Teile in einer Zeile, Cache der Zusammenfassungen.
  - Satzbild-Einstellungen je Werk („Einstellungen → Werk → Satzbild“: Schrift, Größe, Farben, Blattfarbe, Textbreite;
    Migration 0044, nur HTML, nie Typst/PDF), Stil schon im HTML; Bild an der Cursorposition (Pfad mit der Revision in
    einer Transaktion); Literaturtreffer im Satzbild hervorgehoben; „Alle einklappen“ auch im Satzbild.
- **Runde 27 integriert (Umschlag-Editor und Impressum in der Werkvorlage: umschlag-daten · umschlag-satz ·
  umschlag-widget; 2026-11-09, nach R18 und vor R19):**
  - Umschlagseite als Editor: Titel, Untertitel, Autor, Verlag, Rückentexte, Motto, Klappentext, Kurzbiografie,
    Strichcode, Preis und je einen freien Text je Seite frei als Blöcke platzieren (Position, Breite, Größe, Zeilenabstand, Farbe,
    Schrift, Stärke, Ausrichtung, Schatten/Verlauf, Laufrichtung des Rückens); Hilfslinien, Einrasten, Tastatur,
    Rückgängig; Vorschau per GET und Speichern über Formulare auch ohne JavaScript; Schnellwahl oben/Mitte/unten und
    Zurücksetzen (Migration 0045).
  - Dieselbe Position in Vorschau, Umschlag-PDF und Buch-PDF (Millimeter ab der Beschnittkante); ohne eigene Anordnung
    bleibt der Satz byteidentisch. Druckprüfung: Beschnitt, Sicherheitsabstand, Strichcode, Rücken, Kontrast im Umschlagbild,
    fehlende Schrift (Rückfall mit Hinweis).
  - Vorspann aus der Werkvorlage (`schmutztitel`, `haupttitel`, `impressum`, `widmung` mit dem Wörterbuch
    `baruch_buch`), neue Bücher bekommen die Impressumsvorlage, ein Knopf übernimmt sie in bestehende Werke; Adressen
    von Verlag und Druckerei; Prüfung der Pflichtangaben (nach den Landespressegesetzen üblich, keine Rechtsberatung).
  - MCP liest die Anordnung (`cover_layout`) und stellt höchstens einen Antrag (`cover_layout_set`); Austauschformat
    Nebenversion 3 trägt Anordnung, Adressen und Satzbild-Einstellungen.
  - Noch nicht: eigene Schriftdateien (Nutzerfrage); Umschlag im EPUB seit Runde 20.
- **Runde 28 integriert (Befunde: Befundliste, Überblick über das Werk, Aufträge im Chat: befunde · befund-uebersicht ·
  chat-auftrag; 2026-11-09, nach R27 und vor R19):**
  - Befund als Datensatz (Migration 0046: `finding`, `finding_status`, `post_version`): Antworten der Personas werden
    nach dem Speichern ausgewertet (Art Lob/Hinweis/Kritisch, Zitat, Prüfvermerk, `[B<id>]`-Verweise); der Beitrag bleibt
    unverändert. Den Status setzt nur der Mensch (offen, übernommen mit einer Revision, verworfen mit Begründung, zurückgestellt, erledigt),
    je Änderung ein Commit; Assistenten stellen Vorschläge, der Mensch entscheidet. Ähnliche Wiederholungen
    werden angehängt (nur Text, Embeddings R19a). Spiegel `befunde.md`, Austauschformat Nebenversion 4.
  - Überblick: Seite **„Werk ▾ → Befunde“** (Matrix Kapitel × Persona mit Symbol und Zahl, Filter, Listen),
    Marke „k offen“ in der Gliederung, im Editorkopf, auf `/gliederung` und am Menü, Akkordeon „Befunde“ mit
    Statusformularen, Ebene „Befunde“ in der Ideenkarte, „setzt Befund x um“ im Revisionsbaum; MCP `findings`
    (lesen) und `finding_propose_status` (Vorschlag).
  - Chat: „RFC“/`@alle` ruft alle aktiven Personas auf, der Lektor zuletzt mit einer Sammlung und Gewichtung (gemeinsame Vorschläge
    mit Verweisen auf die Ursprünge); eine Bitte um Text wird zu einem Vorschlag; Entscheidungen des Autors gehen gerahmt in jeden
    Aufruf. Die Pflicht zu „Kritisch“ gilt nur noch für @nathan, der Lektor beginnt mit dem, was trägt. Der Chat scrollt in begrenzter
    Höhe, Kopieren/Zitieren, eigene Beiträge mit Verlauf bearbeiten (`post.edit`, MCP `post_edit`), ganzer Verlauf
    ohne JavaScript (`/beitraege`, `/beitrag`).
  - Noch nicht: Ähnlichkeit über Embeddings und Befunde im RAG-Index (R19a), ein neuer Abschnitt durch einen Schreibauftrag.
- **Runde 30 integriert (Suchen/Ersetzen, Querverweise, Chat: Lösungsvorschläge, Varianten, Bedienung:
  suchen-verweise · chat-loesungen · chat-bedienung; 2026-11-09, vor R29 und R19):**
  - Suchen und Ersetzen im Abschnitt (Strg+F/H, Suchleiste im Editor) und im Werk (Strg+Umschalt+F/H, Seitenleiste
    oder Seite `/suche` ohne JavaScript) mit Bereichen, Groß-/Kleinschreibung, ganzem Wort und regulärem Ausdruck (Zeitgrenze);
    Ersetzen im Werk erzeugt je Abschnitt eine Revision vom Menschen, einen Commit, Rückgängig mit einem Klick;
    Bibelzitate und Zitate bleiben unberührt (Migration 0047, `replace_batch`).
  - Querverweise: eine feste Sprungmarke `<kN>` je Gliederungspunkt nur in Export, Spiegel und Satz; Verweis
    `#link(<kN>)[…]` (wahlweise mit „S. n“) über das Menüband „Verweis“; Strg+Klick springt, Alt+← zurück; eine Liste der
    möglichen und gebrochenen Verweise (`/verweise`, Druckprüfung); Einlesen und Austausch vergeben Verweise neu.
  - Lösungen: Kritische Punkte tragen `statt`/`besser` oder `Schritt` (sonst einmal nachgefragt, dann „(ohne
    Lösungsvorschlag)“); „Als Vorschlag übernehmen“; Varianten A/B/C als Geschwistervorschläge mit einem Vergleich
    (`/varianten`); die Prüfgrundlage für alle Personas eines Aufrufs gleich und im Kopf sichtbar; Austausch Nebenversion 5.
  - Chat: Anrede „Lektor, …“ ohne @, Relay bei einer direkten Anrede in einer Antwort (eine Stufe), Live-Anzeige
    („schreibt …“, neue Beiträge ohne Neuladen), ↻ wiederholen und ✕ löschen, strukturierte Beiträge mit Befundblöcken
    und geprüften Sprungmarken.
  - Noch nicht: Vorwärtssprung über den Schlüssel, Seitenzahl des Verweises als Werkeinstellung (R29); EPUB-Links seit Runde 20.
- **Runde 29 integriert (Satz in den Einstellungen: satz-daten · satz-typst · satz-oberflaeche; 2026-11-09):**
  - „Einstellungen → Werk → Satz“ (ohne JavaScript, mit Vorschau): Kapitelanfang (fortlaufend, neue Seite, neue rechte
    Seite), Teilseite, Seitenformat (A5, 12 × 19, 13,5 × 21,5, A4, eigenes), Ränder mit Bundzugabe, Grundschrift,
    Größe, Zeilenabstand, Einzug oder Abstand, Blocksatz, Silbentrennung und Sprache, Seitenzahlen (Position, ab dem ersten
    Kapitel), Kolumnentitel, Fußnoten, Schusterjungen/Hurenkinder und „Verweis mit Seitenzahl“ als Werkeinstellung.
    Jede Änderung ein Spiegel-Commit vom Menschen; MCP liest nur (`typeset_settings`).
  - Typst-Regeln „// Baruch: Satz“ hinter der Werkvorlage (Haken `snapshot::head`): dasselbe in Export, Spiegel,
    Satzvorschau, Buch-PDF und Umschlag; ohne Einstellungen byteidentisch. Hinweise, wo die Werkvorlage dasselbe
    selbst festlegt; ein Angebot, eine Kapitelanfangsregel der Vorlage als Einstellung zu übernehmen (eine neue Vorlagenrevision).
  - Migration 0048 (`work_typeset`, Grabsteinart), Austausch Nebenversion 6 (`<satz>`); `epub_style` als
    Datenmodell (noch nicht angeschlossen).
  - Seit R21b liest der Import eines Exports den Regelblock in die Satzeinstellungen zurück (nur wenn genau lesbar);
    EPUB nutzt `epub_style` seit Runde 20.
- **Runde 19 abgeschlossen (RAG-Pflege, Rest der Verweise: index · pgvector · treffer; 2026-11-09):**
  - Index: kein neuer Embedding-Versuch je Suche (Suchen stellen nur die Bereinigung in die Warteschlange), ehrlicher Status mit
    liegengebliebenen Aufträgen (`index.stale`, Seite „Verbindungen“), „Index neu aufbauen“ antwortet sofort
    (`deferred`), Einfälle gelöschter Werke sofort im gemeinsamen Bereich, Start mit eigenem Datenordner ohne
    gespeicherte Verbindung Eliezer ohne Embedding-Anfrage; Abgleich mit MultiGPT gehärtet.
  - Aufgabenpräfixe des Embeddings je Weg (LanceDB Anfrage/Dokument, pgvector Anfrage); pgvector kürzt jedes Feld in
    SQL, setzt Schemanamen in Anführungszeichen, warnt bei einem Passwort ohne TLS.
  - „schwach“ für Treffer unter 0,45 in Literatur, Einfällen, Belegen und `rag.search` (`weak`), ohne
    JavaScript eingeklappt; `idea.search` mit `mode: "rag"` meldet Fehler (-32001).
  - Ideenkarte: „Gruppen benennen (Assistent)“ als Vorschlag, Übernahme nur durch den Menschen; MCP `idea_map`
    mit `near_limit`/`near_threshold`. Menüband „Bibelstelle“/„Literaturverweis“, wenn die Vorlage sie bietet.
  - Verweise: Wortlaut gegen Module mit Trennungsresten („Wortlaut stimmt (Leerraum im Modul)“, ein Hinweis zum Modul),
    „Mal“ als Maleachi vor Kapitel und Vers, Zitatlink, Lemmaprobe der Lexika, NFC im Vergleich.
  - Noch nicht: ältere Revisionen/Befunde im RAG-Index (Nutzerfrage 1), Kommentare/Befunde an MultiGPT
    (Nutzerfrage 11).
- **Runde 20 integriert (EPUB-Vorschau: epub · reader · epub-pruefung; 2026-11-09):**
  - EPUB 3 mit EPUB-2-Rückfall (neue Crate `baruch-epub`, rein): eine XHTML-Datei je Teil und Kapitel, Text über das
    Blockmodell von baruch-wysiwyg (kein typst-html), Fußnoten als Popup oder Endnoten, Querverweise `#link(<kN>)`
    als interne Links, Bilder mit Alternativtext, Umschlag als `cover-image`, Vorspann, `nav.xhtml` + `toc.ncx`,
    Metadaten nur als maskierter Text, Satzeinstellungen als CSS (`epub_style`), deterministische Bytes. Was sich nicht
    abbilden lässt, wird gemeldet (`ISLAND_OMITTED`, Seitenumbruchregeln der Vorlage `TEMPLATE_PAGEBREAK_*` …).
    Wege: Menü „Werk → Als EPUB herunterladen“, RPC `work.epub`, CLI `export --epub`, MCP `epub_export` (lesen).
  - Vorschau je Reader (Menü „Ansicht → EPUB-Vorschau“): neun Profile (Pflicht: Apple Books iPhone/iPad, Google Play
    Books, „Kindle (über EPUB)“, „Kindle-App (über EPUB)“), iframes nebeneinander mit Reader-CSS und eigener CSP
    (`sandbox; frame-ancestors 'self'`), Kapitelgrenzen, Nachtmodus; ohne JavaScript über ein Formular-GET. Profilwerte
    sind Annahmen ohne Gerät.
  - Prüfung: eigene Struktur- und Barrierefreiheitsprüfung (xmllint, sonst quick-xml), Reader-Warnungen, EPUBCheck aus dem
    PATH oder einer konfigurierten epubcheck.jar (nie ein Download, nur der Mensch startet es; MCP `epub_check` nie). Rest der
    Buchdaten: ISBN-Präfix 978/979, `paper_sheet_mm` (Dicke je Blatt, ohne Migration), „Umschlag entfernt“ als eigener
    Commit, Druckerei-Umschlag mit den Textregeln der Werkvorlage. Keine Migration.
  - Noch nicht: eine echte Prüfung an Gerät/Reader und ein echter EPUBCheck-Lauf mit Java (manuelle Prüfung), Hersteller-Reader auf
    Android (Nutzerfrage), ein eigenes Kindle-Format (bewusst nicht, Send to Kindle nimmt EPUB an).
- **Runde 31 integriert (Register und Glossar: glossar-daten · register-satz · kontextmenue; 2026-11-09):**
  - Glossar je Werk (Migration 0049: Begriffe mit Varianten, Kurz- und Langerklärung, Quelle, Bibelstellen;
    eigene Register; Registervorschläge), RPC `glossary.*`/`index.*`, je Änderung ein Spiegel-Commit, Spiegeldatei
    `glossar.md`; Assistenten und MCP (`glossary_propose`, `index_propose`, `glossary.suggest`) nur Vorschläge, der
    Mensch übernimmt. Austausch Nebenversion 7 (Glossar, eigene Register, Satzfelder, Grabsteine).
  - Registermarke im Text `#metadata((register: …, term: …))<baruch-idx>` (Buchtext: Setzen/Entfernen = eine neue Revision des
    Menschen); Register Stichwort, Personen, Bibelstellen (automatisch aus erkannten Stellen, biblische Reihenfolge) und
    bis zu 13 eigene. Satzeinstellungen: Registerseiten (1–3 Spalten) mit Seitenzahlen aus Typst (Bereiche „12–14“,
    Untereinträge, „siehe“/„siehe auch“), Glossarseite vor/nach den Registern, die erste Erwähnung je Kapitel
    markiert; nur Projektion (Export, Spiegel, PDF, EPUB mit `glossary.xhtml`/`index-*.xhtml`), SQLite bleibt
    unverändert; ohne Einstellungen alles byteidentisch.
  - Kontextmenü im Satzbild und im Quelltext (Rechtsklick, Menütaste, Umschalt+F10; ARIA-Menü): „In Glossar
    aufnehmen“, „In Register aufnehmen“, „Im Glossar nachschlagen“, Marke bearbeiten/entfernen; Menübandgruppe
    „Register und Glossar“. Ohne JavaScript: Link „Markierung …“, Seiten „Glossar“ und „Register“ mit allen Formularen.
  - Der Import eines Exports mit einem Buchende nimmt Buchende und Projektion wieder heraus.
  - Noch nicht: Eine Marke ohne JavaScript bearbeiten nur als Entfernen und neu Setzen; Groß-/Kleinschreibung
    gleicher Stichwörter wird nicht zusammengeführt; Typst-Meldungen aus dem Buchende zeigen auf den letzten Abschnitt;
    das Kontextmenü ist in echten anderen Browsern als Firefox nicht geprüft; beim Import eines Exports ohne Glossarseite bleibt die
    Markierung der ersten Erwähnung (`#emph[…]`) als gewöhnliche Hervorhebung im Text stehen; Registervorschläge reisen nicht
    im Austauschformat mit; kein MCP-Werkzeug für „Begriffe vorschlagen lassen“ (nur der Mensch löst es aus).
- **Runde 22 integriert (Revisionen und Spiegel: fassungen · befund-rueckfrage · spiegel-entwurf; 2026-11-10):**
  - Ergänzen: „Speichern“ im Editor ergänzt die eigene aktuelle Revision (Stand k, je Schritt Text
    davor/danach und ein Spiegel-Commit); eine neue Revision entsteht mit einem Grund (auf Wunsch, Persona-Revision, andere
    Grundlage, Überschrift/Gliederung, große Änderung, Zeitgrenze, abgeschaltet). Verlauf mit Wortdiff,
    Rückgängig als neuer Schritt, Einstellung „Fassungen“ je Werk, „weitere Fassung(en)“ im Lesefluss, Seitenzweige
    `refs/revisions/<key>/<n>` im Spiegel. Migration 0050.
  - Bei einer erneuten Prüfung fragen Personas nach eigenen erledigten Befunden („Befund n scheint erledigt?“, Ja/Nein durch
    den Menschen); übernommene Befunde merken sich den Stand der Revision. Archiv gelöschter Werke mit Wiederherstellen;
    Spiegel der Einfälle ohne Werk beim Start und nach dem Löschen.
  - Entwurf als Wasserzeichen (Satzeinstellung, „Als Entwurf herunterladen“ ohne die Einstellung zu ändern) in der
    Satzvorschau, im Buch-PDF, auf Stichwortzetteln, auf Wunsch im Umschlag, in der EPUB-Titelseite/den Metadaten; ohne Entwurf byteidentisch.
    Austausch Nebenversion 8 (Ergänzungen, Entwurf, Stand bei Befunden, Vorspulen, ein neuer Teil als Antrag mit
    einer Kennung), `<kN>`-Show-Regel einschließlich Rückweg beim Import, Spiegel einstellbar (`main.typ` mit `#include`).
  - Noch nicht: Die anderen Schreibwege (Bild, Registermarke, Verweis, Ersetzen, Gliederung) ergänzen nicht;
    Option „Sitzung“ statt Minuten; Bilder und Werkart kommen nicht aus dem Archiv mit; `persona.chat`/MCP
    `ask_persona` geben die Nacharbeitsfelder nicht getrennt aus (seit Runde 23 doch: `done_proposals`, `done_notes`,
    `all_done`).
- **Runde 23 abgeschlossen (Härtung: web · mcp-namen · geheimnisse; 2026-11-24):**
  - Editor und Oberfläche: Zitatwerkzeug (Block mit einer Folgezeile für den Beleg, im Satzbild Zitat und ein eigener Absatz
    statt einer Code-Insel), „Fassung ändern“ mit einem Verlaufsschritt und einer Zustandsprüfung, `guard::own_reads` gegen lesende
    Anfragen fremder Seiten, feste Meldungscodes, Marken ohne JavaScript, Editorsperre beim Speichern,
    Verhaltenstests der Widgets im Node-Prüfstand; Reste von Ideenkarte, Umschlag, EPUB, Register, Befunden.
  - MCP: eindeutige Handles mit Umlautumschreibung, `lokal-` für Clients ohne Token, Besitz nur über das Handle,
    eine gemeinsame Bildquote ohne Token, Ober- und Größengrenzen, ein Relay-Weg, Rückmeldefelder in
    `ask_persona`/`persona.chat`.
  - Geheimnisse: Schwärzen an einer Stelle (auch Base64- und Prozentform), keine Weiterleitungen, Filter vor
    `fetch_url`, `confirm` beim Löschen von Modellverbindungen, Grenzen im Austausch, ein privater EPUBCheck-Temp-Ordner.
  - Keine Migration, keine neue Nebenversion, keine neuen Methoden oder Werkzeuge. Review (14 Befunde),
    Nacharbeit (altes Handle für Token, die erst die neue Regel zusammenführte; gemeinsame Quoten ohne Token)
    und PM-Abschluss haben stattgefunden; Reste in `docs/Backlog.md`, „Nach Runde 23 offen“.
- **Runde 24 „Verteilung (B5)“ erledigt** (2026-11-24; mit Review, Nacharbeit und PM-Abschluss – die letzte
  geplante Runde): Version 0.9.0.
  - Pakete: .deb und AppImage lokal gebaut und gestartet; .dmg und .msi nur als CI-Jobs (unsigniert, nie
    gelaufen). Jedes Paket trägt `LICENSE`, `THIRD-PARTY-LICENSES.txt` und das Quelltextangebot `QUELLTEXT.txt`;
    Daten bleiben im Nutzerordner, kein Paket legt dort etwas an oder löscht etwas. Schlüsselbund je Zielsystem.
  - Seite „Über“ `/ueber` (Version, Lizenz, Quelltext, Fremdlizenzen, Datenordner) im Menü „Werkzeuge“ und unter
    „Einstellungen → Daten“; **keine Updateprüfung** – Baruch fragt nie von sich aus das Netz nach Versionen.
  - CI: `ci.yml` (Tests, clippy mit `-D warnings`, `cargo deny`, `typst compile`, Bau der Website), `release.yml` (Tag
    `v*`: Pakete, APT-Repository, Release-Entwurf, GitHub Pages), `pages.yml`. Kein Workflow ist je gelaufen;
    kein Tag, kein Push. Alle Auslieferung geht über GitHub, nichts von Hand.
  - Website `site/` (Hugo, eigenes Theme, ohne JavaScript) für `https://baruch.bernau.family/`; APT unter `/apt/`.
  - Betrieb hinter einem Reverse-Proxy: `--allow-host NAME`/`BARUCH_ALLOWED_HOSTS`, unter einem solchen Namen `/mcp`
    nur mit Token; Zugriffsschutz im Proxy ist Pflicht (`docs/Betrieb.md`). MultiGPT hat keine Standardadresse mehr.
  - Keine Migration, keine neuen RPC-Methoden oder MCP-Werkzeuge. Offene Punkte und die Schritte des Nutzers:
    `docs/Backlog.md`, „Nach Runde 24 offen“.
- **Runde 37 „Hilfe auf der Website“ integriert** (2026-11-27, auf `development`; Version unverändert 0.9.0, im
  CHANGELOG unter „Unveröffentlicht“):
  - **Die Website ist die Quelle der Bedienungsdoku:** `site/content/de/hilfe/` (14 Seiten, Alltagssprache, Schritte, Tasten
    für Windows/Linux und Mac) für Nutzer, `site/content/de/betrieb.md` für Betreiber; die Ersteinrichtung (Bibeln,
    Assistenten, MultiGPT, KI-Programme) steht in `installation.md` und `mcp.md`. `docs/Bedienung.md` und
    `docs/Betrieb.md` sind jetzt nur Einstiegspunkte: „Was steht wo“, Regel für Entwickler, technische Anmerkungen.
  - **Bilder:** Liste `site/data/hilfebilder.toml` (94 Namen), Shortcode `hilfebild` (fehlendes Bild = keine Ausgabe,
    unbekannter Name oder fehlender Alternativtext = Abbruch, dunkle Variante über `<picture>`, ein Link zur vollen Größe),
    Werkzeug `site/tools/bildschirmfotos.py` (Wegwerf-Server, erfundenes Beispielwerk, Zwischenspeicher; `--pruefen`,
    `--fotos`, `--abgleich`). Linkprüfung `scripts/site-check.py` in `scripts/ci-site.sh`.
  - **Oberfläche:** Menü „Baruch ▾ → Hilfe“ und „Über Baruch“ verlinken auf `https://baruch.bernau.family/hilfe/`
    (ein gewöhnlicher Link, auch für Lektoren). Keine Migration, keine neue Route, RPC-Methode oder MCP-Werkzeug.
    Offene Punkte: `docs/Backlog.md`, „Runde 37 – integriert“.
- **Runde 36 „Reste“ integriert** (2026-11-27, auf `development`; Version unverändert 0.9.0, im CHANGELOG unter
  „Unveröffentlicht“):
  - Typst-Kommandozeile im eigenen Binary: `baruch-server typst …` oder der Alias `baruch-typst` (`compile` nach
    PDF, PNG, SVG, `fonts`, `--version`; offline, ohne Pakete; Nichtunterstütztes ehrlich mit Exit 2). Die Pakete
    liefern nur `baruch-typst`, nie `typst`; die CI lädt kein Typst mehr. `pages.yml` läuft auch beim
    Veröffentlichen eines Releases (nicht durch Ausführung geprüft).
  - sqlx 0.8.6 → 0.9.1 (libsqlite3-sys 0.40, RUSTSEC-2026-0459 behoben, die Ausnahme in `deny.toml` entfallen);
    Migrationen und Prüfsummen unverändert, zusammengesetztes SQL über `AssertSqlSafe`.
  - Reste aus Runde 35 (Notweg `passwort` hebt die Sperre auf, Werkbindung für drei Einträge gelockert,
    Anzeigename der Lektoren, Theme auf der Anmeldeseite), aus Runde 34 (die Rundreise der Mindmap behält die Wolke,
    Beschriftungen frei, Bildlaufleiste), aus 0.9.0 und „offen nach R24“ (u. a. die Namen der Spiegel-Commits, wenn beim Löschen
    eines Gliederungspunkts die mitgelöschten Revisionen und Beiträge genannt werden).
  - Keine Migration, keine neue Route, RPC-Methode oder MCP-Werkzeug. Offene Punkte: `docs/Backlog.md`, „Runde 36“.
- **Runde 35 „Konten und Rollen (Lektorat II)“ integriert** (2026-11-27, auf `development`; Version unverändert
  0.9.0, im CHANGELOG unter „Unveröffentlicht“):
  - Ohne Konten bleibt Baruch offen wie bisher. Mit Konten verlangen Seiten, Formulare und `/rpc` eine Anmeldung
    (Argon2id, Sitzung als Cookie, Sperre gegen Brute Force); `/mcp` bleibt bei Token. Das erste Konto (Autor)
    entsteht nur am eigenen Rechner; Notweg `baruch-server passwort <name>`.
  - Rollen je Werk: Der Autor gibt ein Werk für Lektor-Konten frei (Mitwirkende). Lektoren (`@lektorat:<name>`)
    sehen nur freigegebene Werke und schreiben nur Vorschläge, Beiträge und Befunde (auch „Anmerkung zum Buch“),
    mit Herkunft im Spiegel; nur der Autor kann übernehmen, ändern und löschen.
  - Eine Stelle der Rechteprüfung (`roles`: `LEKTOR_ROUTES`, `LEKTOR_RPC`): Für Lektoren ist alles gesperrt, was
    dort nicht steht – auch Routen und Methoden künftiger Runden.
  - Migrationen 0053 (Konten, Sitzungen, Fehlversuche) und 0054 (Freigaben); neue RPC `account.*`, `member.*`,
    `revision.propose`, `finding.add`; kein neues MCP-Werkzeug. Alles ohne JavaScript benutzbar. Ungeprüft in einem echten
    Browser hinter einem echten Proxy. Offene Punkte: `docs/Backlog.md`, „Nach Runde 35 offen“.
- **Runde 34 „Mindmap wie XMind“ integriert** (2026-11-25, auf `development`; Version unverändert 0.9.0, im
  CHANGELOG unter „Unveröffentlicht“, gedacht für 0.9.1):
  - Die Ideenkarte ist eine Mindmap: das Werk in der Mitte, Teile/Kapitel als Äste rechts und links, Einfälle als
    Unteräste beliebig tief; Anlegen (Tab/Enter), Umbenennen (F2), Umhängen (Ziehen), Einklappen, Löschen mit
    Wiederherstellen, Rückgängig, Verbindungen als beschriftete Linien. Ein Layout, in Rust (SVG vom Server,
    Bildschirm und Ausgabe gleich).
  - **Entscheidung (Nutzer 2026-11-25): Wolke und Mindmap sind verbunden.** Einfälle ohne Platz liegen in der Wolke
    neben dem Baum; sie wandern in die Mindmap (auf Knoten, Kapitel oder als Hauptthema auf das Werk) und zurück, samt
    ihrer Unterknoten. Nichts wird ungefragt zu einem Hauptast.
  - Einfälle als Baum (Migration 0052: Titel, Elternknoten, Reihenfolge, Hauptthema), RPC `idea.move`/`idea.tree`,
    MCP `idea_move`/`idea_tree`, Austauschformat Nebenversion 9.
  - Gliederungsansicht ohne JavaScript; Ausgabe `.xmind`, `.mm`, SVG, PDF, Markdown; Einlesen von `.xmind`/`.mm` mit
    einer Vorschau. Ungeprüft in echtem XMind und Freeplane. Offene Punkte: `docs/Backlog.md`, „Nach Runde 34 offen“.
- **Runde 33 „Menüband und Laptop-Modus“ abgeschlossen** (2026-11-25, auf dem Zweig `runde33` neben Runde 32;
  Version unverändert 0.9.0; das Zusammenführen mit `development` und Runde 32 einschließlich der Gegenprobe steht noch aus):
  - Ein Menüband auf jeder Seite ersetzt die Menüs „Werk“/„Ansicht“/„Werkzeuge“ und die Knopfreihe über dem Blatt: Menü
    „Baruch ▾“ (Programm), Reiter „Start“ · „Einfügen“ · „Prüfen“ · „Buch“ · „Ansicht“ mit benannten Gruppen; die Knöpfe des
    Editors docken mit JavaScript ins Menüband an. Marken an den Reitern. Alles ohne JavaScript erreichbar
    (Reiter als Links `?tab=`, Gruppen als `<details>`, Menüband offen/zu über `?band=`).
  - Laptop-Modus (automatisch/an/aus): unter 1600 px ein kompaktes Menüband (Gruppen als Popups), unter 1280 px Gliederung und
    Assistenten als Leisten mit einer Schublade; Spalten einzeln einklappbar; Zoom zurücksetzen. Zustand als Attribute an
    `<html>`, vom Server aus Cookies gezeichnet, keine Media Queries.
  - Jede Seite öffnet mit ihrem Reiter; die Satzseite trägt ihre Bedienelemente im Reiter „Buch“.
  - Keine Migration, keine neue Route, keine neue RPC-Methode, kein neues MCP-Werkzeug.
- **Runde 32 „Lektorat I – Word und Beenden“ abgeschlossen** (2026-11-25, auf `development` nach Runde 33; Version
  unverändert 0.9.0, im CHANGELOG unter „Unveröffentlicht“):
  - Word-Import (`/werke/import/word`, RPC `docx_import.*`, CLI `import DATEI.docx`): Vorschau mit Gliederung,
    Bearbeitern und Hinweisen; Revision 1 = der Stand vor allen nachverfolgten Änderungen, Änderungen je Bearbeiter als offene
    Vorschläge unter `@word:<name>`, Kommentare als Beiträge – nie eine aktuelle Revision.
  - Word-Export (`/werke/{id}/word`, RPC `work.docx_export`, CLI `export … --docx`): Formatvorlagen, echte Fußnoten,
    eingebettete Bilder, Typst-Inseln mit wörtlichem Quelltext; auf Wunsch der jüngste offene Vorschlag je Abschnitt
    als Änderungsnachverfolgung. Die eigene Datei kommt beim Import mit derselben Gliederung und demselben Text zurück.
  - Baruch beenden (`/beenden`, nur am eigenen Rechner, nicht über RPC/MCP), sauberes Herunterfahren auch bei
    SIGTERM, Starter mit App-Fenster, Pages in zwei Jobs.
  - Neue Crate `baruch-docx`; keine Migration, kein neues MCP-Werkzeug. Nichts ist in Microsoft Word geprüft.
- Prüfen: `cargo test --workspace`; Vorschau `cargo run -p baruch-server`, im Browser
  http://127.0.0.1:8765/ (Daten in `~/.local/share/baruch` oder `--data`).
- Noch nicht: manuelle Prüfung der Backends OpenAI-kompatibel und Ollama gegen echte Dienste (nur Attrappen), Kommentare/Befunde im Abgleich mit MultiGPT, Markdown-Import. Kommentare
  (Abschnitts-Chat, Befunde) sollen später auch im (Graph-)RAG abgelegt werden, damit Personas frühere
  Befunde finden; heute liegen sie in SQLite (`post` mit Verweisen) und im FTS5-Index.

### Runde 38 – Englisch als Primärsprache, Werksprache (2026-11-27) {#round-38--english-as-the-primary-language-work-language-2026-11-27}

Entscheidung des Nutzers (2026-11-27, vor 0.9.1): Die Primärsprache ist Englisch für Code, Kommentare, Commits,
Entwicklerdoku, MCP und RPC; die Oberfläche bleibt bis Runde 39 deutsch (dann Deutsch und Englisch über
Kataloge). Jedes Werk hat eine **Werksprache** (`de`|`en`, Migration 0055): Personas antworten darin und schreiben ihre
Marker darin, die Erkennung akzeptiert beide Sprachen, die Buchausgabe folgt ihr ab Runde 39. Sie ist unabhängig von der
Sprache der Oberfläche und von der Silbentrennungssprache der Satzeinstellungen. Das Austauschformat
Nebenversion 10 trägt sie (`sprache` an `<angaben>`). Runde 38 hat das nur zum Teil geliefert; die offenen Teile stehen in
`docs/Backlog.md`, „Open after Round 38“.

## Erste Schritte nach der Freigabe {#first-steps-after-approval}

1. Der Nutzer legt `Beerlesklopfer/baruch` privat an (oder genehmigt das Anlegen); Lesezugriff auf den
   Typst-Buchordner (heute nur für einen Nutzer lesbar) oder eine Kopie nach
   `~/Documents/Project/`, damit Vorlage und Import an den echten Quellen gebaut werden.
2. Lokal `~/Documents/Project/baruch`: Cargo-Workspace, CLAUDE.md (Regeln: deutsche Oberfläche, Bezeichner
   englisch, keine Token, Version nur vom Nutzer, Suite/Firefox/Agenten nie parallel),
   `docs/Plan.md` mit diesem Plan, `LICENSE` AGPL-3.0.
3. B1 umsetzen. MultiGPT bleibt unverändert; Baruch braucht nur einen API-Schlüssel mit `ask` und
   `docs.read`.

## Prüfung {#verification}

- `cargo test`: Revisionsinvarianten (eine `current`, ein Vorschlag ändert `current` nicht, Übernehmen
  atomar), Import/Export-Rundreise Typst und Markdown, Lektorwahl nach Kontext, Schleifengrenze
  für `@`-Aufrufe.
- Backend-Tests gegen einen nachgebauten MCP-/OpenAI-Server, keine echten Kosten.
- Typst-Vorschau des importierten Buchs mit dem PDF aus dem bestehenden Bau vergleichen.
- Echter Lauf: ein Kapitel aus dem Buch des Nutzers über MultiGPT prüfen lassen (gemma auf Eliezer).
- Desktop-Bau unter Linux; macOS/Windows über CI (Plattform-Runner), erst in B5.

## Offen (Nutzer) {#open-user}

- Lesezugriff auf den Typst-Buchordner oder dessen Pfad.
- Anmeldung nur für die Server-Variante relevant; Desktop zunächst ohne Konten.
- ~~Zielversion~~ (0.9.0, Runde 24). Zeitpunkt der Veröffentlichung und die Schritte davor (Verlauf,
  Archivschlüssel, Impressum: `docs/Backlog.md`, Nutzerfrage 17), Signieren für macOS/Windows (Nutzerfrage 10).
- ~~Git im Revisionssystem~~ → beschlossen und umgesetzt (siehe oben). Offen nur: das Einlesen eines vorhandenen Repos
  (Typst-Buchordner) mit seinem Verlauf.
