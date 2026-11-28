<!-- translation-of: docs/MCP.md source-commit: bffe2bc -->

# MCP-Server von Baruch {#mcp-server-of-baruch}

> Dies ist die deutsche Übersetzung von `docs/MCP.md` (englisches Original, maßgeblich), Stand: Commit `bffe2bc`.

`POST /mcp`, JSON-RPC 2.0 nach dem Model Context Protocol **2025-06-18** (Streamable HTTP, nur JSON-Antworten,
kein SSE). Quelle: `crates/baruch-server/src/mcp.rs` (Protokoll, Zugang), `src/mcp/tools.rs` (Werkzeuge),
`src/mcp/ideas.rs` (Einfälle), `src/mcp/session.rs` (Sitzungen), `src/mcp/scripture.rs` (`scripture_check`),
`src/mcp/scripture_map.rs` (`scripture_map`, Runde 17a),
`src/mcp/bibles.rs` (`bible_*`, SWORD-Bibeln), `src/mcp/exchange.rs` (`exchange_export`, Runde 26),
`src/mcp/cover.rs` (`cover_layout`, `cover_layout_set`, Runde 27),
`src/mcp/tokens.rs` (RPC `mcp_token.*`). Laut Plan der gemeinsame Eingang für alle Personas und Werkzeuge wie
Claude Code („Eigener Endpunkt je Persona“, eingehend).

**Stand:** Lesen (Werke, Gliederung, Abschnitte, Suche, Personas, Bilder) und Schreiben als Vorschlag oder
Beitrag sind umgesetzt, dazu Einfälle mit vollem CRUD (sie ändern keinen Buchtext, siehe „Einfälle“) und die
Gliederung (sofort anlegen, alles andere als Antrag, siehe „Gliederung“). Seit Runde 10 (B2) antworten Personas
wirklich (`ask_persona`, `chat` über Eliezer), `scripture_check` prüft Bibelstellen (seit 2026-11-06 mit einer
gewählten SWORD-Bibel, auch den Wortlaut; `bible_*` lesen die Bibel), und andere Rechner kommen mit
einem Token hinein (siehe „Zugang“). Offene Stubs im Code:
`grep -rn "STUB:" crates`.

**Nur der Mensch:** Übernehmen, Ändern, Löschen und Umschalten von Fassungen geht nur über die Benutzeroberfläche
oder RPC (`docs/RPC.md`). **Ändern, Löschen und Umschalten gibt es über MCP nicht**, ebenso wenig den Import:
Ein Import legt aktuelle Fassungen unter dem Namen des Menschen an, und das darf ein Assistent nicht
(Entscheidung 2026-11-05). Typst-Bücher kommen über `baruch-server import` ins Programm
(`docs/Bedienung.md`) oder durch Hochladen in der Benutzeroberfläche. Ein Aufruf von `import_typst` ergibt heute
-32602 „unbekanntes Werkzeug“. Ebenso gibt es über MCP **kein Einlesen einer Austauschdatei** (Runde 26): Das Ausgeben
geht nur lesend mit `exchange_export`, das Einlesen nur durch den Menschen (Seite „Austausch: Einlesen…“, RPC
`work.exchange_preview`/`work.exchange_import`, `baruch-server import DATEI.baruch`). Seit Runde 32 gilt dasselbe für Word: **Word-Import und Word-Export gibt es über MCP nicht** (Seiten
`/werke/import/word` und `/werke/{id}/word`, RPC `docx_import.*` und `work.docx_export`, `baruch-server import DATEI.docx`
und `export … --docx`); ein Aufruf von `docx_import` oder `docx_export` ergibt -32602 „unbekanntes Werkzeug“.
**Das Beenden von Baruch gibt es weder über RPC noch über MCP:** Es geht nur über die Seite `/beenden` am eigenen Rechner
oder durch ein Signal an den Prozess. Ebenso gibt es kein Werkzeug, das Werke anlegt, ändert (Titel, Metadaten,
Werkvorlage) oder löscht: Das ist die Werkverwaltung des Menschen (`/werke`, RPC `work.create/update/delete`,
`docs/RPC.md`, Abschnitt „Works“). `list_works` liest nur.

## Zugang {#access}

1. **Fremde Webseiten:** `/mcp` liegt wie `/rpc` hinter `guard::same_origin`. Passt `Origin` nicht zu `Host`
   oder ist `Sec-Fetch-Site` weder `same-origin` noch `none`, antwortet der Server mit **403** – vor jeder
   weiteren Prüfung. Clients ohne `Origin` (Claude Code, curl, Skripte) kommen durch.
2. **Ohne Token nur vom eigenen Rechner:** Ohne `Authorization` nimmt der Endpunkt nur Verbindungen von
   `127.0.0.1`, `::1` (auch `::ffff:127.0.0.1`) an. Jede andere Gegenstelle – und eine Anfrage, deren
   Gegenstelle der Server nicht kennt – erhält **401** mit `WWW-Authenticate: Bearer realm="baruch"` und dem
   Text `MCP ohne Token nur vom eigenen Rechner (127.0.0.1 oder ::1). Von anderen Rechnern mit
   „Authorization: Bearer <Token>“; Tokens erzeugt die Einstellungsseite „MCP-Zugang“.`
   **Ausnahme hinter einem Reverse-Proxy (Runde 24):** Wurde Baruch mit `--allow-host NAME` oder
   `BARUCH_ALLOWED_HOSTS` gestartet und trägt die Anfrage einen dieser zusätzlichen Namen in `Host` (mit oder
   ohne Port), gilt sie **nie als lokal** – auch wenn die Gegenstelle `127.0.0.1` oder `::1` ist, denn das ist
   dann der Proxy und nicht der Mensch am eigenen Rechner. Unter einem solchen Namen funktioniert `/mcp`
   **nur mit Token**; ohne `Authorization` kommt dasselbe **401**. Sind zusätzliche Namen erlaubt, gilt eine Anfrage
   ganz ohne lesbaren `Host`-Header ebenfalls nicht als lokal (`guard::via_proxy`). Unter `127.0.0.1`, `::1` oder
   `localhost` bleibt auf demselben Server die obige Regel in Kraft. Grenze: Schreibt der Proxy `Host` auf
   `127.0.0.1:<Port>` um, kann Baruch die Anfrage nicht von einer lokalen unterscheiden – der Proxy muss den
   Namen durchreichen (`docs/Betrieb.md`, mit einer Probe nach dem Einrichten). Baruch wertet `X-Forwarded-*` nicht aus.
   **Mit Token (Runde 10):** `Authorization: Bearer <Token>` (erzeugt unter „Einstellungen → MCP-Zugang“ oder
   RPC `mcp_token.create`, nur als SHA-256-Hash gespeichert) öffnet `/mcp` von jedem Rechner. Der Client heißt dann
   wie das Token (`@mcp:<tokenname>`, auch ohne Sitzung oder mit einer fremden; von `clientInfo` zählt nur die
   Version) und darf sich nie als `persona` als Autor oder Herkunft nennen (siehe „Beiträge verwalten“). Ein falsches oder widerrufenes Token oder ein anderes Schema als Bearer → **401** mit
   `error="invalid_token"`, auch über Loopback. Von anderen Rechnern ist **nur** `/mcp` erreichbar
   (`guard::remote_only_mcp`, sonst 403). Dafür muss Baruch auf einer Netzadresse lauschen (`--listen`), der
   Client spricht es über seine IP-Adresse an (siehe 4.); Baruch spricht nur HTTP ohne TLS – Token und Texte
   laufen im Klartext durch das Netz, nur im eigenen Netz oder durch einen Tunnel verwenden.
3. **Inhaltstyp:** `Content-Type: application/json`, sonst 415.
4. **Kein fremder Name:** `Host` muss eine IP-Adresse, `localhost` oder ein beim Start mit `--allow-host` erlaubter Hostname sein
   (Runde 24, genau dieser Name, keine Subdomains), sonst **421** (gilt für alle
   Routen, `guard::local_host`). So kann eine Webseite DNS-Rebinding (ein fremder Name zeigt auf
   127.0.0.1) nicht nutzen, um Werkzeuge aufzurufen oder Texte zu lesen. Ohne `--allow-host` ist kein Name erlaubt.

Der Server muss die Gegenstelle kennen (`into_make_service_with_connect_info::<SocketAddr>()` in `main.rs`);
sonst gilt jede Anfrage als fremd.

Einrichten in Claude Code (Server lokal auf dem Standardport 8765, nur für dieses Projekt):

```
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

Von einem anderen Rechner (Token aus „Einstellungen → MCP-Zugang“):

```
claude mcp add -s local --transport http baruch http://<IP>:8765/mcp --header "Authorization: Bearer <Token>"
```

Hinter einem Reverse-Proxy mit TLS (Name mit `--allow-host` erlaubt, immer mit Token):

```
claude mcp add -s local --transport http baruch https://<Name>/mcp --header "Authorization: Bearer <Token>"
```

Der systemd-Benutzerdienst (`packaging/systemd/`) nutzt denselben Port. Ein älterer Eintrag mit einem
anderen Port wird mit `claude mcp remove baruch -s local` entfernt und wie oben neu gesetzt.

## Sitzungen {#sessions}

`initialize` merkt sich `clientInfo` (`name`, `version`) unter einer neuen Sitzung und schickt deren Kennung im
Antwortkopf **`Mcp-Session-Id`** (32 Hexzeichen). Spätere Anfragen mit diesem Kopf
werden dem Client zugeordnet; ohne Kennung oder mit einer unbekannten gilt der Client als „unbekannt“. Die Tabelle
lebt im Prozess (nach einem Neustart leer) und hält höchstens 256 Sitzungen; die älteste fällt zuerst heraus.
Die Kennung ist kein Geheimnis und ersetzt kein Token.

Aus `clientInfo` stammen:
- **Client-Kürzel** `@mcp:<name>`: Name in Kleinbuchstaben, Zeichen außerhalb von `[a-z0-9._-]` → `-`, höchstens 32 Zeichen,
  leer → `unbekannt`. Beispiel: "Claude Code" → `@mcp:claude-code`.
- **Modell** (wenn der Aufruf keines nennt): "<name> <version>", ohne Version nur der Name, ohne Client
  „unbekannt“.

## Nachrichten {#messages}

| Methode | Antwort |
|---|---|
| `initialize` | `protocolVersion` (die des Clients, sonst `2025-06-18`), `capabilities.tools`, `serverInfo`, `instructions`; Kopf `Mcp-Session-Id` |
| `ping` | `{}` |
| `tools/list` | `{tools: [{name, description, inputSchema}]}` in der Reihenfolge der Tabelle unten |
| `tools/call {name, arguments}` | siehe „Antworten“ |
| Benachrichtigung (ohne `id`) | HTTP 202 ohne Rumpf |

Eine Liste von Nachrichten wird der Reihe nach beantwortet; nur Benachrichtigungen → 202.

### Antworten von `tools/call` {#answers-of-toolscall}

- **Erfolg:** `{content: [{type: "text", text: <JSON as text>}], structuredContent: <the same JSON>, isError: false}`.
  `structuredContent` ist immer ein Objekt (das verlangt MCP); Listen stehen darin unter einem Namen
  (`works`, `nodes`, `hits`, `personas`).
- **Fachlicher Fehler** (unbekanntes Werk, unbekannter oder nicht mehr vorhandener Abschnitt, unbekannte
  Fassung oder Persona, leerer Text, Text gleich der aktuellen Fassung, Überschriften passen nicht zum
  Abschnitt, ungültiger Verweis, Stub): `{content: [{type: "text", text}], isError: true}`
  mit einem deutschen Text.
- **Protokollfehler:** unbekanntes Werkzeug (auch das entfernte `import_typst`), unlesbare `arguments` oder ein
  schreibendes Werkzeug mit `node` oder ohne `key` (siehe unten) → JSON-RPC-Fehler **-32602** mit einem deutschen
  Hinweis; Datenbankfehler → **-32603**.

## Werkzeuge {#tools}

`work` ist überall die Kennung (Zahl) oder der Slug (Text) aus `list_works`; `node` ist die **Stelle** des
Abschnitts im Werk (`node` aus `outline`, ab 0), `key` sein **fester Schlüssel** (`key` aus `outline` oder
`read_node`); `rev` ist die **Nummer** einer Fassung (`number` in `read_node`, ab 1).

**Achtung, Stellen ändern sich:** Verschiebt der Mensch Punkte, ändert deren Ebene, legt sie an, löscht, teilt oder führt sie zusammen,
vergibt Baruch die Stellen `node` neu. Ein Assistent, der vorher `outline` gelesen hat, träfe mit
der alten Stelle einen anderen Abschnitt. Darum gilt (Entscheidung nach Runde 4, umgesetzt in Runde 5):

- **Schreiben nur über `key`:** `propose_revision` und `add_post` – auch jeder Verweis in `refs` – nehmen den Abschnitt
  **nur** über `key` (Pflicht). Steht `node` in den Argumenten (auch zusätzlich zu `key`) oder fehlt `key`,
  antwortet Baruch mit JSON-RPC **-32602** und einem deutschen Hinweis, z. B. „arguments: node (Stelle)
  wird nicht angenommen; key mitgeben …“ oder „refs[1]: key fehlt …“; nichts wird geschrieben. Das
  `inputSchema` beider Werkzeuge kennt kein `node` und verlangt `work`, `key` und `body`; jeder Eintrag in
  `refs` verlangt `key`, `rev` und `quote`.
- **Lesen wie bisher:** `read_node` nimmt `node` oder `key` (ist `key` vorhanden, gewinnt es); `outline` und `search`
  bleiben unverändert. Die Antworten nennen `node` (Stelle jetzt) und `key`.
- Existiert der Abschnitt eines Schlüssels nicht mehr, kommt „Diesen Gliederungspunkt gibt es nicht mehr …“
  (`isError`). Der Schlüssel wird unter der Schreibsperre aufgelöst, damit zwischendurch niemand umordnet.

**Vorschläge ändern nie die Gliederung:** Der Text von `propose_revision` beginnt mit genau einer Überschrift
der Ebene des Abschnitts (`=` Kapitel, `==` Abschnitt, `===` Unterabschnitt; ein neuer Titel ist erlaubt und
benennt bei der Übernahme um). Eine andere Ebene, weitere Überschriften, keine Überschrift oder Text davor → Fehler
mit Begründung, kein Vorschlag. Ebene, Teilen und Zusammenführen bleiben beim Menschen (Editor, Gliederung).

Nummern werden nie neu vergeben; nachdem der Mensch eine Fassung gelöscht hat, hat die Folge Lücken
(z. B. 1, 2, 4). Baruch sucht Fassungen deshalb immer über die Nummer, nie über ihre Stelle in der Liste.

| Werkzeug | Parameter | Ergebnis (`structuredContent`) | Stand |
|---|---|---|---|
| `list_works` | – | `{works: [{id, slug, title, language}]}`; `language` = Werksprache `de` \| `en` (Runde 38) | umgesetzt |
| `outline` | `{work}` | `{work: {id, slug, title}, language, nodes: [{node, key, parent, number, title, kind, current_rev, proposals, url}]}`; `current_rev` ist die Nummer der aktuellen Fassung | umgesetzt |
| `work_timing` | `{work}` | Sprechdauer des Werks (Zeitwächter), **nur lesend**, genau wie RPC `work.timing` (`docs/RPC.md`, „Time watch“): `{work, kind, words_per_minute, target_seconds, words, seconds, status: "none" \| "ok" \| "over", over_seconds, shown, clock, target_clock, over_clock, sections: [{key, seq, depth, number, title, words, seconds, start_seconds, subtree_seconds, clock, start_clock, subtree_clock}]}`; je Punkt wird die aktuelle Fassung ohne Typst-Auszeichnung gezählt (Regeln in `docs/Bedienung.md`, „Zeitwächter“). Unbekanntes Werk → `isError`. Wörter je Minute und Zielzeit setzt nur der Mensch („Einstellungen → Werk → Predigtzeit“, RPC `work.timing_set`); dafür gibt es kein Werkzeug | umgesetzt (Runde 16b) |
| `read_node` | `{work, node \| key, rev?}` | `{node, key, number, title, rev, text, revisions: [{number, status, author: {kind, name, model?}, note}], posts: [{id, author, body, created}], url}`; `text` ist Fassung `rev`, sonst die aktuelle (ohne aktuelle die mit der höchsten Nummer); `posts` nennt die Kennungen, damit ein Client seine eigenen Beiträge findet | umgesetzt |
| `search` | `{query, kinds?, work?, all_revisions?, limit?}` | `{hits: [SearchHit]}` wie RPC `library.search` (`docs/RPC.md`), je Treffer zusätzlich `node_key` und `url` (`/?work=W&key=K[&rev=R]`, ohne Abschnitt `null`); ohne `work` über alle Werke, Bibliothek immer | umgesetzt |
| `typst_check` | `{work?, key?}` | `{ok, stand, pages, errors, warnings, diagnostics, print, missing, url}` – setzt das Werk mit Typst (wie RPC `preview.typst`), **nur lesend**, ändert nichts, kein Commit; `ok` = keine Fehler, `diagnostics` wie bei `preview.typst`, `url` = `/werke/W/satz?key=K` oder `/werke/W/satz`; unbekannter `key` oder Zeitüberschreitung (60 s) → `isError`; seit Runde 18a lässt sich der Lauf über RPC `preview.cancel` oder „Satz abbrechen“ der Satzseite abbrechen → `isError` „Satz abgebrochen.“; `@preview`-Pakete werden nicht geladen | umgesetzt (Runde 8) |
| `exchange_export` | `{work, format?: "xml" (default) \| "baruch"}` | `{work, uuid, format, file_name, media_type, bytes, sha256, xml?, data?}` – das ganze Werk als Austauschdatei (`docs/Austauschformat.md`), **nur lesend**, ändert nichts, kein Commit; wie RPC `work.exchange_export`: bei `xml` die Datei `<slug>.baruch.xml` als Text in `xml` (Bilder als Base64), bei `baruch` der ZIP-Container `<slug>.baruch` als Base64 in `data`; `sha256` über die gelieferten Bytes. Format Unterversion 7: einschließlich der Einfallskarte (`<karte>` mit Verbindungen zwischen Einfällen und Lagen, seit Runde 25), Umschlag-Layout, Anschriften im Impressum und Satzbild-Einstellungen (seit Runde 27), Befunde und alte Beitragsfassungen (Unterversion 4, Runde 28), Lösungen, Varianten und Schlüssel der Punkte (Unterversion 5, Runde 30), Satzeinstellungen `<satz>` (Unterversion 6, Runde 29), seit Runde 31 Unterversion 7 mit eigenen Registern, Glossar
(auch Vorschläge) und den Register-/Glossarfeldern von `<satz>`. Ohne Schlüssel, Verbindungen zu Diensten, Persona-Modelle und Anträge. **Obergrenze 4 MiB** Datei (`MCP_EXPORT_LIMIT`, die Antwort trägt den Inhalt doppelt): darüber `isError` mit einem Hinweis auf die Seite `/werke/{id}/austausch` und `baruch-server export SLUG --baruch DATEI`; unbekanntes Werk → `isError`, unbekanntes `format` → -32602. Das Einlesen gibt es über MCP nicht | umgesetzt (Runde 26) |
| `typeset_settings` | `{work?}` | `{work, set, uuid, updated, updated_by, settings, rules, conflicts, template_rule, warnings}` wie RPC `work.typeset_get` ohne `fields` – Satzeinstellungen des Werks, die erzeugten Typst-Regeln „// Baruch: Satz“ und Hinweise, wo die Werkvorlage dasselbe selbst setzt; **nur lesend**, nur der Mensch ändert sie („Einstellungen → Werk → Satz“); kein Commit; unbekanntes Werk → `isError` | umgesetzt (Runde 29a, nur lesend) |
| `epub_export` | `{work, footnotes?: "popup" \| "endnotes", include_bytes? = false}` | `{work, file_name, url, bytes, sha256, footnotes, meta, files, toc, warnings, epub_base64?}` – das Werk als EPUB, **nur lesend**, kein Commit; Bytes nur mit `include_bytes`, höchstens 8 MiB (Einzelheiten unter „EPUB (Runde 20)“) | umgesetzt (Runde 20a) |
| `epub_check` | `{work, readers?: [slug], footnotes?}` | `{findings, xml_tool, epubcheck, summary, readers, work, url}` wie RPC `epub.check` – eigene Prüfung und Reader-Warnungen, **nur lesend**, startet nie EPUBCheck (Einzelheiten unter „EPUB (Runde 20)“) | umgesetzt (Runde 20c) |
| `glossary_list` | `{work?, status?: "accepted" \| "proposed" \| "all", word?}` | `{work, terms: [Term], url: "/werke/W/glossar"}` (mit `word` zusätzlich `word`) – Glossar des Werks, **nur lesend**, kein Commit; Term wie RPC `glossary.list`; mit `word` wie RPC `glossary.lookup` (nur angenommene Begriffe, Wort oder Variante, Groß-/Kleinschreibung egal); unbekannter Status → `isError` | umgesetzt (Runde 31a) |
| `glossary_propose` | `{work?, term, short?, long?, variants?, source?, bible_refs?, reason?, persona?}` | `{work, id, status: "proposed", origin, term, url: "/werke/W/glossar#begriff-ID", reason?, commit?\|mirror?}` – **nur Vorschlag**, Herkunft Persona (`persona`, nur ohne Token vom eigenen Rechner) oder `@mcp:<name>`; nur der Mensch nimmt an, ändert, löscht; ein schon angenommener oder schon vorgeschlagener Begriff (gleicher Schlüssel), eine unlesbare Bibelstelle oder eine überschrittene Grenze → `isError`; `reason` steht nicht im Glossar, nur in der Antwort; ein Spiegel-Commit `Baruch-Action: glossary-propose` mit dem Client oder der Persona als Autor | umgesetzt (Runde 31a) |
| `index_propose` | `{work?, key, word, occurrence?, register?, term?, sub?, see?, also?, reason?, persona?}` (kein `node`) | `{work, id, status: "open", proposal, url: "/werke/W/register#vorschlag-ID", commit?\|mirror?}` – **nur Registervorschlag**, nie eine Textänderung; der Mensch setzt die Marke bei der Übernahme (RPC `index.decide`, neue Fassung des Menschen); `node` statt `key`, fehlender `key`, Wort nicht in der aktuellen Fassung, unbekanntes Register oder ungültige Marke → `isError`; ein Spiegel-Commit `Baruch-Action: index-propose` | umgesetzt (Runde 31a) |
| `propose_revision` | `{work, key, body, note?, persona?, model?}` (kein `node`) | `{work, node, key, rev, status: "proposed", author, model}` | umgesetzt |
| `add_post` | `{work, key, body, refs?: [{key, rev, quote}], sources?: [string], persona?}` (kein `node`, auch nicht in `refs`) | `{work, node, key, post, id, author, url}` (`post` = Stelle im Abschnittschat ab 0, `id` = Beitrag) | umgesetzt |
| `post_move` | `{work, id, key, persona?}` (kein `node`) | `{work, id, node, key, from_node, from_key, url, commit?}` – **nur eigene Beiträge**, ans Ende des Chats von `key` | umgesetzt (Runde 8) |
| `post_delete` | `{work, id, thread?, persona?}` | `{work, id, node, key, deleted: {author, body}, url, commit?}` – **nur eigene Beiträge**, ohne Rückfrage (der Commit hält den Beitrag fest); mit `thread` ein eigener Faden-Beitrag (`{work, thread, id, deleted, url, commit?}`) | umgesetzt (Runde 8, `thread` Runde 30c) |
| `post_to_idea` | `{work, id, words, persona?}` | `{work, id, idea, node, key, url, commit?}` – **nur eigene Beiträge**; Einfall mit Bezug = Abschnitt, Gedanke = Text, danach ist der Beitrag weg (ein Commit) | umgesetzt (Runde 8) |
| `outline_add` | `{work, key?, place? = "after", title, level? \| items: [{title, level?}], note?, persona?, model?}` (kein `node`) | `{work, action: "added", detail, points: [{node, key, level, title, url}], node, key, url, author, adopted: [], commit?, mirror?}` – siehe „Gliederung“; Teile (`level: 0`) seit Runde 14 auch mit einer Werkvorlage. Würde ein neuer Teil vorhandene Kapitel übernehmen (Nutzerfrage 12): nichts angelegt, stattdessen `{work, action: "requested", status: "open", request: OutlineRequest, adopted: [key], message, url}` – ein Antrag, den der Mensch aktiv bestätigen muss | umgesetzt |
| `outline_rename` | `{work, key, title, note?, persona?}` | Antrag: `{work, request: OutlineRequest, status: "open", message, url}` (`url` = `/gliederung?work=W#antraege`) | umgesetzt |
| `outline_move` | `{work, key, target_key, place, note?, persona?}` (kein `node`, kein `target`) | Antrag wie oben; die Ebene bleibt wie bei RPC `outline.move` | umgesetzt |
| `outline_set_level` | `{work, key, level: 1–3, note?, persona?}` | Antrag wie oben; `level: 0` (zu einem Teil) und Ändern der Ebene von Teilen → `isError` | umgesetzt |
| `outline_set_numbered` | `{work, key, numbered: bool, note?, persona?}` | Antrag wie oben (mit/ohne Nummer zählen; Teil ohne Nummer → `isError`) | umgesetzt |
| `outline_delete` | `{work, key, note?, persona?}` | Antrag wie oben (nur ohne Unterpunkte, nicht das letzte Kapitel) | umgesetzt |
| `outline_requests` | `{work, open? = true, mine? = false, preview? = false, persona?}` | `{work, requests: [OutlineRequest], url}` – `OutlineRequest` wie RPC `outline.requests` (`docs/RPC.md`); mit `preview: true` je offenem Antrag `preview: [{key, level, number, title, mark, kind_word, before_number, before_level, label, before_label, changed_label}]` oder `preview_error` (Runde 14, Art und früherer Zustand seit Runde 16c) | umgesetzt |
| `asset_add` | `{work?, file_name, content, encoding: "base64" \| "svg", name?, path?, caption?, alt?, note?, role?: "cover", persona?}` | `{work, name, rev, mime, bytes, url, author, commit? \| mirror?, request?, message?, request_error?}` – legt ein Bild **sofort** an (Fassung 1, Autor Persona oder `@mcp:<client>`, ein Commit mit `Baruch-Origin`); höchstens 8 MiB dekodiert; `role: "cover"` stellt zusätzlich einen Antrag `set_cover` (scheitert nur der Antrag, bleibt das Bild, `request_error` nennt den Grund); Kontingent siehe „Kontingent für Bilder (21c)“ | umgesetzt (Runde 11, Kontingent 21c) |
| `asset_replace` | `{work?, name, content, encoding, caption?, alt?, note?, rename?: bool = false, persona?}` | Antrag: `{work, request: AssetRequest, status: "open", message, url}` – nicht unverändert; ändert sofort nichts. Ein anderes Format als das des Bildes nur mit `rename: true` (Runde 21c; sonst `isError` mit einem Hinweis): **ein** Antrag, bei der Bestätigung erhält das Bild die neue Endung (`haus.svg` → `haus.png`), alle Pfade bleiben; `request.rename` = true. Kontingent siehe unten | umgesetzt (Runde 11, `rename` und Kontingent 21c) |
| `asset_set_cover` | `{work?, name: string \| null, note?, persona?}` | Antrag wie oben (`null` = Umschlag entfernen) | umgesetzt (Runde 11) |
| `asset_delete` | `{work?, name, rev?, note?, persona?}` | Antrag wie oben (ohne `rev` das ganze Bild, mit `rev` eine nicht aktuelle Fassung) | umgesetzt (Runde 11) |
| `asset_requests` | `{work?, open? = true, mine?, persona?}` | `{work, requests: [AssetRequest], url}` – lesen; nie Inhalte, nur `bytes`/`content_url`; `AssetRequest` wie RPC `asset.request_list` (`docs/RPC.md`, seit 21c mit `rename` und `purged`); entfernt zuerst den Inhalt entschiedener Ersetzungsanträge, die vor mehr als 30 Tagen entschieden wurden | umgesetzt (Runde 11, Aufräumen 21c) |
| `cover_layout` | `{work}` | wie RPC `cover.layout` (`docs/RPC.md`, „Cover layout and imprint“): alle 15 Blöcke (gespeichert oder Voreinstellung) mit Lage und Breite in mm vom Beschnittrand ihres eigenen Bereichs, Maße, Druckprüfung, Standardschrift; ohne Seitenformat (Satzfehler im Buch) `{…, blocks: only the stored ones, geometry: null, geometry_error}`; nur lesend | umgesetzt (Runde 27) |
| `cover_layout_set` | `{work, blocks: [CoverBlock], note?, persona?}` | Antrag (Bildanträge, Operation `cover_layout`, Herkunft Persona oder `@mcp:<client>`) → `{work, request, status: "open", message, url}`; jeder genannte Block ersetzt seine Art vollständig, fehlende Felder eines Blocks kommen aus dem wirksamen Block (z. B. `{kind: "front_title", y_mm: 40}`); der Antrag merkt sich den Fingerabdruck des gespeicherten Layouts (`expected_layout`); ein gleicher offener Antrag wird nicht doppelt angelegt; ungültige Werte (Grenzen wie bei RPC `cover.layout_set`) oder ein Antrag, der nichts änderte → `isError`; ändert sofort nichts, der Mensch bestätigt auf der Seite „Bilder → Anträge“ (veraltet, wenn er das Layout inzwischen geändert hat) | umgesetzt (Runde 27) |
| `findings` | `{work, key?, status?, kind?, persona?, include_praise?, limit?}` | `{work, total, truncated, findings: [Finding], url}` – nur lesend; `total` = alle passenden (ohne Grenze), `truncated`, wenn nur die neuesten `limit` mitkommen; Finding wie `finding_json` (alle Felder von `Finding`, dazu `kind_label`, `status_label`, `url`); `key` nur dieser Abschnitt; `status`/`kind` Datenbankwörter durch Kommas getrennt; ohne angehängte, Lob nur mit `include_praise`; unbekanntes Wort oder unbekannter Abschnitt → `isError` | umgesetzt (Runde 28) |
| `finding_propose_status` | `{work, id, status: adopted\|deferred\|done, rev?, target_key?, target_idea?, note?, persona?}` | nur ein Vorschlag für den Status eines Befunds (Herkunft `persona` oder `@mcp:<client>`) über `findings::rpc::propose` (Sperre, ein Commit), den der Mensch bestätigt; `adopted` braucht `rev`, `deferred` genau eines von `target_key`/`target_idea`; `rejected`/`open` → `isError` „Nur der Mensch kann …“ | umgesetzt (Runde 28; Statusweg selbst: befunde) |
| `post_edit` | `{work, id, body, thread?}` | bearbeitet einen eigenen Beitrag (nur Autor `@mcp:<client>`); die alte Fassung bleibt; im Abschnittschat ein Commit, im Faden keiner (siehe „Beiträge verwalten“) | umgesetzt (Runde 28) |
| `text_search` | `{work, q, case?, word?, re?, range?, key?, kinds?, limit?}` | Suche im Werk wie RPC `search.find` (Wortlaut, ganzes Wort, regulärer Ausdruck; `range` `section`/`chapter`/`part`/`from`/`work`, Voreinstellung `work`; `kinds` Voreinstellung `text`, `heading`, `footnote`, `quote`) – nur lesend; Ersetzen gibt es über MCP nicht, Vorschläge je Abschnitt über `propose_revision`; `search` (Volltext) bleibt | umgesetzt (Runde 30a) |
| `solution_variants` | `{work, key, rev?, findings?, count?: 2\|3}` | der Lektor (nach der Einstellung) schreibt 2 oder 3 Varianten A/B/C als Geschwistervorschläge mit derselben Basis; legt nur Vorschläge an, zählt zur Grenze von zwei Modellaufträgen je Client | umgesetzt (Runde 30b) |
| `list_personas` | – | `{personas: [{handle, name, role, model, context_length, active, lector}], lector, lector_mode}`; `lector` folgt der Wahl (fest, Voreinstellung `@lektor`, oder automatisch nach Kontext) | umgesetzt |
| `list_assets` | `{work?}` | `{work, assets: [AssetSummary]}` wie RPC `asset.list`; ohne `work` das Standardwerk; nur lesend | umgesetzt |
| `read_asset` | `{work?, name, rev?, content? = false}` | `{work, name, revision, revisions, encoding}`, mit `content: true` auch der Inhalt (Base64 oder SVG-Text); unbekanntes Bild oder unbekannte Fassung → `isError`; nur lesend | umgesetzt |
| `sources_list` | – | `{sources: [{id, title, url, note, position}]}` – die bevorzugten Quellen für `@belege` („Einstellungen → Quellen“) in fester Reihenfolge; nur lesend, nur der Mensch kann sie ändern | umgesetzt (Runde 9) |
| `idea_create` | `{words?, title?, parent?, before?, cloud?, work?, key?, note?, quote?, source?, bible?, persona?}` (seit Runde 34 genügt `title` statt `words`; ohne Platz landet der Einfall in der Wolke) | `{idea, commit?}` wie RPC `idea.create`; jeder Einfall in den `idea_*`-Antworten trägt `url` (mit Bezug `/?work=W&key=K`, nur Werk `/?work=W`, sonst `null`) | umgesetzt |
| `add_idea` | wie `idea_create` | wie `idea_create` (zweiter Name, Wunsch 2026-11-06) | umgesetzt |
| `idea_read` | `{id}` | `{idea}` (auch gelöschte) | umgesetzt |
| `idea_list` | `{work?: id \| slug \| "none", word?, open?: bool}` | `{ideas, words}` | umgesetzt |
| `idea_search` | `{query, work?, mode?}` | `{ideas, hits: [{id, snippet}]}`; leere Suche → `isError`; seit Runde 19c `mode: "rag"` (nur mit `work`) wie RPC `idea.search` mit semantischer Suche (`hits` mit `score`, `weak`), Ausfall als `isError` mit Grund | umgesetzt |
| `idea_update` | `{id, title?, words?, note?, quote?, source?, bible?, open?: bool, work?: id \| slug \| null, key?: int \| null, persona?}` | `{idea, changed, commit?, unlinked?}`; beim Verschieben in ein anderes Werk `unlinked` = Kennungen der dabei weich entfernten Verbindungen (nur innerhalb eines Werks), Lagen auf Karten entfallen | umgesetzt |
| `idea_delete` | `{id, persona?}` | `{id, deleted: true, deleted_ids, idea, commit?}` (ohne `confirm`, wiederherstellbar); seit Runde 34 werden die Unterknoten mitgelöscht | umgesetzt |
| `idea_move` | `{id, parent?: id \| null, key?: key \| null, before?: id \| null, cloud?: bool, persona?}` | `{idea, changed, commit?}` wie RPC `idea.move`: hängt einen Einfall in der Mindmap um, ordnet ihn, legt ihn in die Wolke (`cloud: true`) oder macht ihn zu einem Hauptthema am Werk (`cloud: false`); eine verletzte Regel (Zyklus, anderes Werk, Tiefe über 32) → `isError` | umgesetzt (Runde 34) |
| `idea_tree` | `{work, status?, word?, origin?, q?}` | `{map, cloud}` wie RPC `idea.tree`, **nur lesend**: die Mindmap als Baum (Werk, Teile, Kapitel, Hauptthemen, Unterknoten) und daneben die Wolke mit einem Zähler und Vorschlägen „passt zu Ast …“ | umgesetzt (Runde 34) |
| `ask_persona` | `{work, key, persona, question?, mode?: "ask" \| "review" \| "propose", rev?, scope?: "section" \| "chapter"}` | fragt eine Persona mit ihrer Rollenanweisung über Eliezer. `ask` (Voreinstellung): Frage (Pflicht) als Beitrag des Clients (`@mcp:<name>`), Antwort als Beitrag der Persona; `review`: Befunde als Beitrag – bei erneuter Prüfung sieht die Persona ihre eigenen offenen Befunde und meldet umgesetzte mit „Erledigt: Befund n“; daraus entsteht ein Statusvorschlag „erledigt“ (Rückfrage, nur der Mensch entscheidet; Runde 22); `propose`: neue Fassung nur als Vorschlag (`proposed`). `scope` (Runde 15): `section` (Voreinstellung) oder `chapter` (ganzes Kapitel oder ganze Predigt; unbekannt → -32602). Ergebnis `{work, key, persona, mode, scope, model, text, truncated, cut, compressed, post_id? (+ question_post_id), rev?/status?, url, basis?}`; `basis` (Runde 30b) = geprüfte Fassung `{rev, label, proposal}`; `compressed`: `null` oder `{message, summarized, chunks}`, wenn der Kontext in Stücke geteilt und zusammengefasst wurde; `cut`: Das Modell hielt an der Längengrenze an (Beitrag mit Hinweis; ein abgeschnittener Vorschlag wird nicht angelegt → `isError`). `key` Pflicht, `node` → -32602; Fehler des Backends, unbekannte oder inaktive Persona → `isError` mit einer ehrlichen Meldung. Keine Websuche: @belege sucht über MCP nie im Web (nur Aufträge aus der Benutzeroberfläche). Zählt zur Warteschlange je Client (siehe unten) | umgesetzt (Runde 10, `scope`/`compressed` Runde 15) |
| `revision_amends` | `{work, key? \| node?, rev}` | Verlauf der Fortschreibungen einer Fassung, **nur lesend**: `{work, node, key, rev, amend_seq, steps: [{uuid, seq, kind, undo_of?, author, created, body_before, body_after}], note}` (Einzelheiten unter „Round 22“); rückgängig macht nur der Mensch | umgesetzt (Runde 22) |
| `chat` | `{work, key?, thread?, message, persona?}` | Mit `key`: Beitrag des Clients im Abschnittschat. Ohne `key` (Runde 15): Beitrag in einem **Faden ohne Abschnitt** – neu oder mit `thread` fortgesetzt (der Mensch liest und antwortet unter `/faeden`; `thread` zusammen mit `key` → -32602, unbekannter Faden → `isError`). Dann antworten jede erwähnte aktive Persona (`@handle`, seit Runde 30c auch Name/Kürzel ohne „@“ am Satzanfang, gefolgt von „,“ oder „:“) und `persona` nacheinander mit je einem Beitrag. **Relay (eine Stufe):** Erwähnt eine Antwort auf die Nachricht weitere aktive Personas, antworten diese einmal auf diese Antwort (die erwähnende Antwort als Daten gerahmt); Relay-Antworten rufen niemanden weiter auf (auch nicht die aus dem Relay bei direkter Ansprache, Runde 30c), `relayed` nennt alle, die über das Relay geantwortet haben, jede Persona antwortet auf jede Nachricht höchstens einmal, alle Antworten zusammen höchstens drei – Schleifen sind ausgeschlossen. Antwort mit `key`: `{posts, errors, url, truncated, cut, compressed, relayed, work, key}`; ohne `key`: `{posts, answers: [{persona, model, post_id, text, cut, truncated, relay, asked_by}], errors, url, thread, title, work, truncated, cut}`. Keine Websuche für @belege über MCP | umgesetzt (Runde 10; Faden ohne Abschnitt und Relay Runde 15) |
| `scripture_check` | `{text?, work?, key?}` | findet Bibelstellen in `text` oder in der aktuellen Fassung des Abschnitts `key` (dann wie RPC `bible.check`) und prüft Buch, Kapitel, Vers gegen die Verszählung der Schlachter 1951; Antwort wie RPC `scripture.check` (`refs` je mit `input, span, normalized, book, status, message, wording, wording_code, quote, bible_text, partial, diff, diff_line`, dazu `source`, `bible`). Wortlaut nur mit der gewählten SWORD-Bibel des Werks (`match`, `differs` mit einem Wortdiff, `no_quote`, `missing_verse`, `versification_mismatch`, `invalid_ref`, `module_error`; `bible` nennt Modul, Verszählung, Lizenzvermerk, seit R17c auch `cipher`), sonst „ungeprüft“ (`unchecked`). Seit R17c additiv je Stelle `quote_span` (Byte-Bereich des Zitatinhalts im geprüften Text oder `null`) und `module_ref` (Stelle in der Verszählung des Moduls gelesen, z. B. „Mal 4,1–6 (KJV)“, nur wenn sie abweicht); Module in einer anderen Verszählung (alle Verszählungen von `sword-modules`) liest Baruch über die geprüfte Abbildung, was sich nicht abbilden lässt, bleibt `versification_mismatch` mit einer Begründung. Weder `text` noch `key` → -32602; nur lesend | umgesetzt (Runde 10, Wortlaut 2026-11-06, Abbildung R17c) |
| `scripture_map` | `{ref, from?, to?}` | Runde 17a: eine Bibelstelle zwischen Verszählungen umrechnen, nur lesend; Antwort wie RPC `scripture.map` (`{from, to, refs: [{input, normalized_from, status: same\|moved\|unmapped\|invalid, target, verses, rules, message}], versifications}`). `ref` (1–500 Zeichen) wird in `from` (Voreinstellung KJV) gelesen und geprüft, Ziel `to` (Voreinstellung deutsch, die Verszählung der Stellenprüfung). Alle 18 Verszählungen von SWORD (Groß-/Kleinschreibung egal); Abbildung nur über die geprüfte Regeltabelle (Psalmüberschriften, Kapitelgrenzen wie Joel 2/3, Mal 3/4 …); was keine Regel abdeckt und in der Kapitellänge abweicht, ist `unmapped` mit „Versifikation passt nicht: …“ – nie geraten. Unbekannte Verszählung → `isError` mit der Liste | umgesetzt |
| `bible_modules` | `{work?}` | wie RPC `bible.modules`: Modulordner, je Bibelmodul `module, description, versification, license`/`notice` (Lizenzvermerk der conf), `supported`/`problem`, seit R17c `cipher` (`none`/`locked`/`unlocked`) und `key_stored` – nie der Schlüssel; dazu `work_bible`; nur lesend (nur der Mensch wählt und speichert Schlüssel: RPC `bible.cipher_key_set/_delete` haben **kein** MCP-Werkzeug) | umgesetzt (2026-11-06, R17c) |
| `bible_lookup` | `{ref, module?, work?}` | wie RPC `bible.lookup`: Verse der Stelle(n) als reiner Text, `text: null` = fehlt im Modul; seit R17c über die Abbildung in der Verszählung des Moduls gelesen (`module_ref`, `rules`, `truncated` erst ab 400 Versen; nicht abbildbar `status: versification_mismatch`); `bible` mit Modul, Verszählung, Lizenzvermerk; ohne Bibel oder verschlüsselt ohne Schlüssel → `isError`; nur lesend | umgesetzt (2026-11-06, R17c) |
| `bible_search` | `{query, module?, work?, limit? = 50}` | wie RPC `bible.search`: lineare Suche nach einem Wort/einer Wortfolge über alle Verse, höchstens 200 Treffer, 5 s (`truncated`); nur lesend | umgesetzt (2026-11-06) |
| `commentary_lookup` | `{work, ref, module?}` | wie RPC `commentary.lookup`: je für das Werk gewähltem Kommentar (oder `module`) die Einträge zur Stelle in der Verszählung des Kommentars, reiner Text (höchstens 4000 Zeichen je Eintrag), `notice` = Lizenzvermerk der conf; `problem` bei fehlendem Modul, ungültiger Stelle, ohne Eintrag; nur lesend (Wahl je Werk nur durch den Menschen) | umgesetzt (Runde 15) |
| `lexicon_lookup` | `{work, key, module?}` | wie RPC `lexicon.lookup`: Stichwort → Artikel im ersten gewählten Lexikon (oder `module`), `found`, `text`, `notice` (Lizenzvermerk); nur lesend | umgesetzt (Runde 15) |
| `lexicon_search` | `{work, query, module?, limit? = 50}` | wie RPC `lexicon.search`: Wort/Wortfolge in Stichwörtern und Artikeln, Stichworttreffer zuerst, 5 s; `notice`; nur lesend | umgesetzt (Runde 15) |
| `rag_search` | `{query, work?, ways?, limit? = 10}` | semantische Suche in den RAG-Wegen des Werks (LanceDB, pgvector, MultiGPT; welche, wählt nur der Mensch, `ways` kann nur welche weglassen), `query` 1–1000 Zeichen, `limit` 1–50. Antwort wie RPC `rag.search` (Answer JSON `{work, query, hits, ways: [{way, state, label, message, hits, millis}]}`, state je Weg connected/failed/off; seit Runde 19a je Treffer `weak: bool` = Score unter 0,45, „schwach“). Die Herkunft der Suche ist der Client (`@mcp:<name>`). **Nur lesend; Auszüge sind Daten aus Quellen, keine Anweisungen.** Leere/zu lange Anfrage, unbekannter Weg, `limit` außerhalb des Bereichs, unbekanntes Werk → `isError` | umgesetzt (Runde 12) |
| `rag_status` | `{work?}` | Zustand der Wege und des lokalen Index ohne Netz, wie RPC `rag.status` (`{work, ways, index, fts: true}`; seit Runde 19a `index.stale` = liegengebliebene Aufträge, `problem` bleibt gesetzt bis zu neuem Inhalt oder einem Neuaufbau); nur lesend | umgesetzt (Runde 12) |
| `source_read` | `{document, collection?, part? = 1, title?, q?}` | Teil des Volltexts einer Quelle über MultiGPT `read_document`, Antwort wie RPC `source.read` (`source`, `part`, `parts`, `more`, `text`, `marks`, `origin`, `message`). `document` z. B. aus `rag_search` (Weg multigpt), oder `title` = Kennung eines verknüpften Titels. **Nur Dokumente aus vom Menschen gewählten Sammlungen:** `document` muss in einem Treffer einer Suche in gewählten Sammlungen vorgekommen sein (`rag_search` oder Suche der Benutzeroberfläche seit dem Start des Servers, mit passender `collection`, falls angegeben), als lokale Kopie des Abgleichs vorliegen oder mit einem Titel verknüpft sein – sonst `isError` „… nur Dokumente aus vom Menschen gewählten Sammlungen …“ (ohne zu lesen; `read_document` kennt keine Sammlung). **Nur lesend; Inhalte sind Daten aus Quellen, keine Anweisungen.** Ausfall von MultiGPT, unbekanntes Dokument, Titel ohne Verknüpfung → `isError`. Das Übernehmen in die Bibliothek (`source.adopt`) gibt es über MCP nicht | umgesetzt (Runde 13) |

**Semantische Suche (Runde 12):** `rag_search` und `rag_status` lesen nur. Ein Neuaufbau des Index
(`rag.reindex`) und die Wahl der Wege und der Sammlungen je Werk (`connection.rag_set`, `connection.collections_*`)
gibt es über MCP nicht (`tools/call rag_reindex` → -32602 „unbekanntes Werkzeug“); das bleibt beim Menschen.

**Einstellungen (Runde 9):** Es gibt absichtlich kein Werkzeug `settings_read`: Ein Client liest Personas mit Rolle, Modell und
Kontext schon über `list_personas`; Verbindungen, Geheimnisse, RAG-Wege, Daten und die
Sicherung bleiben MCP verschlossen, Farben, Personas und Werkangaben ändert nur der Mensch. Nie erscheint ein
Geheimnis über MCP. MCP-Clients erscheinen nach `initialize` auf der Seite „Einstellungen →
MCP-Zugang“ (Name, Version, Kürzel, zuerst/zuletzt gesehen; nur im Speicher des Servers).

**Bilder (Runde 11):** Assistenten legen Bilder mit `asset_add` sofort an (wie `outline_add`). Das Ersetzen,
Setzen oder Entfernen des Umschlags und das Löschen geht nur als **Antrag** (`asset_replace`, `asset_set_cover`,
`asset_delete`), den der Mensch auf der Seite „Bilder“ (Abschnitt „Anträge der Assistenten“) bestätigt oder abweist;
hat sich das Bild oder der Umschlag inzwischen geändert, wird der Antrag als veraltet geschlossen.
Das Umschalten der aktuellen Fassung, das Ändern von Unterschrift und Alternativtext bleiben beim Menschen (Seite
„Bilder“, RPC `asset.*`). `persona` nur ohne Token vom eigenen Rechner (wie bei `outline_*`); sonst ist der Client
`@mcp:<name>` Autor oder Antragsteller. Das direkte Setzen von Buchangaben und Umschlag (`work.meta.set`,
`work.set_cover`) sowie das PDF (`work.pdf`) gibt es über MCP nicht; `typst_check` meldet die Druckprüfung
(`print`, Rasterbilder unter 300 dpi) und `missing` (Bildpfade ohne Datei).

Gliederungsschemata wählt nur der Mensch; dafür
gibt es kein MCP-Werkzeug. `outline` und `read_node` liefern `number` nach dem eingestellten Schema des
Werks (z. B. „§ 1“, „Erstes Kapitel“, „Einleitung“).

### Regeln beim Schreiben {#rules-when-writing}

- **Bilder:** lesen (`list_assets`, `read_asset`), sofort anlegen (`asset_add`), alles andere nur als Antrag
  (`asset_replace`, `asset_set_cover`, `asset_delete`; Runde 11).

- **`propose_revision` legt immer einen Vorschlag an** (`proposed`, über `Store::add_proposal`, mit
  `based_on` = aktuelle Fassung). Die aktuelle Fassung bleibt unverändert; nur der Mensch kann ihn in der
  Benutzeroberfläche übernehmen oder verwerfen. Kein MCP-Aufrufer kann eine Fassung `current` machen, ändern, löschen
  oder auf eine andere Fassung umschalten.
- **Autor:** `persona` (Kürzel wie `@lektor`, ohne `@` wird es ergänzt) muss in der Tabelle `persona` stehen,
  sonst `isError`; ohne `persona` ist der Autor das Client-Kürzel `@mcp:<name>`.
- **Modell:** Argument `model`, sonst aus der Sitzung ("<name> <version>"), sonst „unbekannt“.
- **Abgewiesen** (`isError`): leerer Text; Text gleich der aktuellen Fassung (CRLF zählt wie LF); eine
  Überschriftzeile mit leerem Titel, einem Zeilenumbruch im Titel oder mehr als 200 Zeichen („Ein Gliederungspunkt
  braucht einen Titel.“ usw., im Speicher geprüft, `check_proposal_heading`).
- **Farben**, Rolle, Modell, Kontext und „aktiv“ der Personas setzt nur der Mensch
  (`/einstellungen/assistenten`); dafür gibt es kein Werkzeug.
  Vorschläge über `propose_revision` erscheinen im Lesefluss in der Farbe der genannten Persona, ohne
  Persona-Eintrag (`@mcp:<name>`) in Grau.
- **Verweise bei `add_post`** werden geprüft: Der Abschnitt und die Fassung (über die Nummer) müssen im selben Werk existieren,
  `quote` muss wörtlich (nach dem Kürzen) in dieser Fassung stehen. Eine fehlende Nummer meldet
  „… die gibt es nicht (vorhanden: 1, 2, 4).“. Leerer Beitrag → `isError`.
- Vorschläge und Beiträge ändern den Typst-Spiegel nicht (er enthält nur aktuelle Fassungen), daher kein
  Commit.

**Adressen (`url`, Runde 8):** Antworten tragen relative Adressen in der kanonischen Form
`/?work=W&key=K` (ohne `rev`; bei `search` mit `&rev=R`). Sie bleiben gültig, wenn der Mensch die
Gliederung umordnet, weil sie den festen Schlüssel nutzen; ein Client kann sie dem Menschen als Link nennen.

### Beiträge verwalten (Runde 8) {#managing-posts-round-8}

Beiträge sind kein Buchtext. Ein Client darf Beiträge daher verschieben, löschen oder in einen Einfall umwandeln
– aber **nur seine eigenen**: Autor ist die Persona aus `persona` (sie muss existieren) oder sonst `@mcp:<name>`
der Sitzung. Ein fremder Beitrag (Mensch, andere Persona, anderer Client) → `isError` „Nur eigene Beiträge
lassen sich über MCP verschieben, löschen oder umwandeln: Beitrag N stammt von …“, nichts geändert, kein
Commit. **Mit Token nie als Persona:** Ein Client mit Token (möglicherweise ein anderer Rechner) handelt nur als
`@mcp:<tokenname>`; nennt er `persona` (bei `add_post`, `propose_revision`, `post_move`, `post_delete`,
`post_to_idea`, den Einfalls- und Gliederungswerkzeugen), antwortet das Werkzeug mit `isError` „Mit Token handelt
ein Client nur im eigenen Namen …“, nichts geändert. Sonst könnte er Beiträge als Werk einer Persona ausgeben und
deren echte, vom Modell erzeugte Beiträge löschen. Eine Persona antworten lassen geht weiterhin mit `ask_persona`
oder `chat`. **Ohne Token über Loopback: Besitz nach dem, was der Client angibt, keine Anmeldung:** MCP kennt keine Identität. Wer `persona` auf
eine vorhandene Persona setzt oder sich mit demselben `clientInfo.name` meldet, gilt als deren Autor und darf
ihre Beiträge verwalten (so wie er auch Beiträge unter diesem Namen schreiben darf). Die Regel schützt vor
Versehen, nicht vor einem Client, der sich absichtlich als ein anderer ausgibt. **Seit Runde 23** ist der Name eines
Tokens (auch eines widerrufenen) davon ausgenommen: Ein Client ohne Token mit diesem Namen schreibt als
`@mcp:lokal-<handle>` und besitzt die Beiträge des Token-Clients nicht (siehe „Round 23 – Hardening“). Sicher geschützt sind die
Beiträge des Menschen: Ihr Autor („Autor“) ist weder ein Persona-Kürzel (`@…`) noch `@mcp:…`, keine Angabe
eines Clients trifft sie. Unter der Schreibsperre geprüft. Jede Handlung ergibt genau einen Commit im Spiegel (Autor wie bei
Einfällen „MCP-Client (@mcp:…)“ oder „Name (@handle)“, Trailer `Baruch-Post`, `Baruch-Action`, `Baruch-Node`,
bei Umwandlung `Baruch-Idea`); Löschen und Umwandeln tragen den ganzen Beitrag im Rumpf. Der Mensch darf
alle Beiträge verwalten (Benutzeroberfläche, RPC `post.*`).

**`post_edit {work, id, body, thread?}` (Runde 28, chat-auftrag):** bearbeitet nur eigene Beiträge des Clients
(Autor `@mcp:<name>`, Besitz nach dem, was der Client angibt, wie oben; `persona` gibt es hier nicht: Beiträge von Personas
sind nie bearbeitbar), im Abschnittschat oder mit `thread` im Faden. Ein fremder Beitrag (Mensch, Persona, anderer
Client) → `isError` „Nur eigene Beiträge lassen sich bearbeiten …“, ohne Änderung. Die alte Fassung bleibt als Verlauf
(`post_version`); im Abschnittschat genau ein Commit „Beitrag <id>: bearbeitet“ mit dem Text davor und danach (Autor
der Client), im Faden keiner (Fäden stehen nicht im Spiegel). Antwort wie RPC `post.edit` oder `thread.post_edit`
(`id`, `edited`, `versions`, `url`, `commit?`).

**`chat` mit RFC, „@alle“ und einem Schreibauftrag (Runde 28, chat-auftrag):** „RFC“ als erstes Wort oder „@alle“ ruft jede
aktive Persona genau einmal auf (die Grenze von drei Antworten gilt nicht, die Warteschlange je Client schon; keine
Relay-Stufe), den Lektor zuletzt mit den Antworten der anderen als Daten gerahmt und der Aufgabe, sie zu gewichten
(„Jetzt wichtig:“, „Kann warten:“); im Abschnittschat und im Faden. Ein Schreibauftrag („@lektor Schreibe …“) im
Abschnittschat wird ein Vorschlag für den Abschnitt plus ein kurzer Beitrag (`proposals` in der Antwort), nie eine
Übernahme; über MCP gibt es kein `as_order`, nur die Erkennung zählt. Die Antwort trägt zusätzlich `rfc`,
`proposals`, `findings`, `notes`.

### Gliederung {#outline}

Wunsch 2026-11-06 („Ein LLM hat den besseren Überblick durch seinen Context“, „UD nach Bestätigung“); Quelle
`src/mcp/outline.rs`, Speicher `outline_add_by_assistant` und `requests.rs` (Migrationen 0012 und 0035).

- **`outline_add` legt sofort an** (Ausnahme: ein neuer Teil, der vorhandene Kapitel übernähme, siehe „Teile“).
  Entweder `title` (ein Punkt, optional mit `level`) oder `items` (bis zu 100
  Punkte, jeder mit `level`), nie beides (`level` neben `items` → -32602).
  Der Block steht vor dem Bezugspunkt `key` (`place: "before"`), hinter ihm samt seinen Unterpunkten (`"after"`,
  Voreinstellung) oder als letzte Unterpunkte (`"into"`). Jeder Eintrag hat `title` und optional `level` = **absolute
  Ebene** (0 Teil, 1 Kapitel, 2 Abschnitt, 3 Unterabschnitt); fehlt sie, gilt die Ebene der Stelle (Ebene von
  `key` bei before/after, eine tiefer bei into). Der erste Punkt steht auf der Ebene der Stelle, keiner höher als
  sie (so wird kein vorhandener Punkt zum Unterpunkt eines neuen – Ausnahme Teile, siehe nächster Punkt), jeder höchstens eine Ebene tiefer als der vorige.
  Beispiel nach dem letzten Kapitel: `items: [{title: "Einleitung"}, {title: "Erster Gedanke"}, {title: "Bild",
  level: 2}, {title: "Schlussgebet"}]`.
- **Teile (Runde 14):** `level: 0` legt einen Teil an – vor oder hinter einem Teil oder Kapitel (`before`/`after`,
  auch in Werken **mit** Werkvorlage), als erster Punkt auch neben einem Kapitel, in `items` auch mitten im Block
  (die Regel „keiner höher als der erste“ gilt für Teile nicht). `into` mit Ebene 0 und ein Teil neben einem
  Abschnitt → `isError`. Der Text des neuen Teils (Fassung 1) ist die Teilmarke
  `#heading(level: 1, numbering: none)[Titel] <baruch-part>` mit einer Leerzeile (`docs/WYSIWYG.md`, Schnittstelle A).
  Stehen dahinter vorhandene Kapitel (bis zum nächsten Teil), würden sie unter den neuen Teil wandern. **Das ist
  nicht bloßes Anlegen** (Nutzerfrage 12, Entscheidung des Nutzers 2026-11-07: „es braucht eine aktive Zustimmung
  (Popup)“): Baruch legt dann **nichts** an, sondern stellt einen Antrag (Art `add`, Migration 0035) und antwortet
  `{work, action: "requested", status: "open", request, adopted, message, url}` – `message` = „Antrag N gestellt,
  wartet auf Zustimmung: …“, `adopted` = feste Schlüssel der Kapitel, die wandern würden, `note` = Begründung für
  den Menschen. Der Mensch bestätigt oder weist im Dialog (auf jeder Seite) oder auf `/gliederung#antraege` ab;
  `outline_requests` liest den Stand. Nach der Bestätigung entsteht der Teil wie beim direkten Anlegen (Fassung 1
  vom Assistenten, Kapitel mit ihren Texten und Nummern unverändert darunter); würde er dann andere Kapitel übernehmen als
  zur Zeit des Antrags (z. B. weil der Mensch inzwischen selbst einen Teil angelegt hat), wird der Antrag als veraltet geschlossen
  und nichts geändert. Übernimmt der neue Teil **keine** vorhandenen Kapitel (am Ende des Werks, vor einem anderen
  Teil, in einem Werk ohne Punkte), bleibt es nach AGENTS.md Anlegen: sofort, `action: "added"`,
  `adopted: []`. Eine Liste (`items`) mit einem solchen Teil wird als Ganzes ein Antrag.
  Umbenennen, Verschieben und Löschen eines Teils gehen wie bei jedem Punkt nur über einen Antrag; das Ändern der Ebene zu einem Teil
  oder eines Teils gibt es nicht (`outline_set_level` mit 0 → `isError`).
- **Atomar:** eine Transaktion, ein Commit; ein Fehler (Titelregeln mit „Punkt N: …“, Ebenen, Teil an der falschen
  Stelle) → `isError`, nichts angelegt. Titelregeln wie überall (nicht leer, einzeilig, ≤ 200 Zeichen).
- **Ohne `key`** nur in einem Werk ohne Gliederungspunkt (erster Punkt, Kapitel oder Teil);
  sonst `isError`. Ein solches Werk entsteht heute nicht über die Benutzeroberfläche (ein neues Werk bringt ein Kapitel
  mit, das letzte lässt sich nicht löschen).
- **Herkunft:** Fassung 1 jedes neuen Punkts (Überschriftzeile + Leerzeile) gehört `persona` oder dem Client
  als `@mcp:<name>` (Modell `model` oder aus der Sitzung), nie dem Menschen. Im Commit ist es der Autor
  („MCP-Client (@mcp:…)“ oder „<Name> (@persona)“), Committer der Mensch, Trailer `Baruch-Origin: mcp|persona`
  und `Persona`.
- **Alles andere ist ein Antrag:** `outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`,
  `outline_delete` ändern nichts sofort. Baruch probiert die Änderung in einem Sicherungspunkt (zurückgerollt): Ginge sie jetzt nicht
  (Titelregeln, Löschen mit Unterpunkten, keine Wirkung, ungültige Ebene), kommt `isError`, und
  es entsteht kein Antrag; ein gleicher offener Antrag → `isError`. Der Mensch bestätigt oder weist ab
  (`docs/Bedienung.md` „Anträge der Assistenten“, RPC `outline.request_decide`); veraltete Anträge werden mit
  einer Begründung geschlossen (`decision: "stale"`). `outline_requests` liest den Stand (`mine: true` = Antragsteller wie
  `persona` oder `@mcp:<name>`).
- **Vorschau (Runde 14):** `outline_requests` mit `preview: true` gibt je **offenem** Antrag die Gliederung so,
  wie sie nach der Bestätigung wäre: `preview: [{key, level, number, title, mark}]` in Baumreihenfolge, `mark` =
  `same`, `renamed`, `moved` (der verschobene Punkt samt Unterpunkten), `level` (auch mitwandernde Unterpunkte),
  `numbered`, `deleted` (gelöschte Punkte an ihrer alten Stelle mit alter Ebene und Nummer), bei einem Antrag `add`
  zusätzlich `added` (neuer Punkt) und `adopted` (Kapitel, das unter den neuen Teil wandert). In einem
  Sicherungspunkt berechnet und zurückgerollt – nichts ändert sich. Ein veralteter oder nicht mehr ausführbarer Antrag
  erhält statt der Vorschau `preview_error` mit der Begründung; entschiedene Anträge (`open: false`) ebenso
  („schon entschieden“).
  Seit Runde 16c trägt jeder Punkt zusätzlich `kind_word` (Art nach der Bestätigung: „Teil“, „Kapitel“, „Abschnitt“,
  „Unterabschnitt“), `before_number`/`before_level` (früherer Zustand, `null` bei neuen Punkten), `label` (Art und Nummer,
  z. B. „Teil I“, „Kapitel 2“, bei Predigten „Predigtteil …“), `before_label` (ebenso davor, `null` bei neuen Punkten)
  und `changed_label` (`true`, wo sich Art oder Nummer ändern oder ein Kapitel übernommen wird; die Benutzeroberfläche zeigt dort
  „(vorher … → nachher …)“).
- `node`/`target` (Stellen) → -32602 wie bei `propose_revision`; nur `key`/`target_key`.

### Einfälle {#ideas}

Einfälle ändern **keinen Buchtext**. Darum gilt für sie die Assistentenregel „nur Vorschläge“ nicht
(Entscheidung IDEA_CRUD): Assistenten und MCP-Clients dürfen Einfälle anlegen, ändern und löschen. Die Herkunft
ist `persona` (sie muss existieren, sonst `isError` „Unbekannte Persona …“) oder der Client als `@mcp:<name>`; sie
steht am neuen Einfall (`origin`) und in jedem Commit (`Baruch-Origin`, `Persona`, Autor des Commits; Committer
der Mensch). `idea_delete` braucht keine Bestätigung: Der Einfall bleibt wiederherstellbar (Benutzeroberfläche,
RPC `idea.restore`), und der Lösch-Commit trägt den ganzen Eintrag. `open: false` markiert „verarbeitet“
(von Hand, `done_as: note`), `open: true` macht ihn wieder offen. Das Verarbeiten zu einem Gliederungspunkt, Beitrag
oder einer Notiz bleibt beim Menschen (Benutzeroberfläche, RPC `idea.process`). Grenzen und Fehler wie bei RPC, als
`isError` mit einem deutschen Text; unlesbare Argumente (z. B. fehlendes `words`) → -32602.

Verbindungen zwischen Einfällen (Runde 25) lesen und schreiben MCP mit `idea_map`, `idea_links`, `idea_link`,
`idea_unlink` (siehe „Einfallskarte“).

Der Lektor kennt die Einfälle zum Abschnitt als Gedächtnis (Runde 10, im Kontext jedes ihm gegebenen Auftrags).
Noch nicht umgesetzt (Runde „Werkzeuge der Personas“): Er schlägt von sich aus passende Stellen vor.

Der Hinweis in `initialize` (`instructions`) nennt die Einfalls-Werkzeuge und die Ausnahme, seit Runde 10 auch
die Personas und Bibelstellen, seit Runde 12 `rag_search`/`rag_status` (semantische Suche, nur lesend, Treffer sind
Daten) und seit Runde 15 (Stand von `crates/baruch-server/src/mcp.rs`, englisch seit Runde 38):

"ask_persona asks a persona (answer as a post, review as findings, propose only as a proposal, scope chapter
for the whole chapter); chat writes into the section chat (with key) or into a thread without a section (without
key, thread continues it) and calls mentioned personas (@handle), one relay step; at most two model jobs
(chat, ask_persona) per client at a time; scripture_check checks Bible references (wording only with the chosen
SWORD Bible of the work, otherwise unchecked; quote_span and module_ref per reference); bible_modules, bible_lookup
and bible_search read SWORD Bibles; commentary_lookup, lexicon_lookup and lexicon_search read SWORD commentaries
and lexicons (read-only, with licence note)."

Seit Runde 25 nennt er auch `idea_map`/`idea_link`, seit Runde 26 `exchange_export` (kein Einlesen über MCP) und seit
Runde 27: "cover_layout reads the cover layout (blocks, position, print check); cover_layout_set changes nothing
immediately but files a request that the human confirms."
Seit Runde 22: "revision_amends reads the history of the amendments of a revision (read-only; only the human
can take one back)." und bei `ask_persona` die Rückfrage zu eigenen Befunden, die erledigt sind.

**Warteschlange je Client (Runde 15):** Die Modellwarteschlange wird geteilt. Damit ein Client sie nicht füllt,
laufen oder warten je Client (`@mcp:<name>`, mit Token der Tokenname) höchstens **zwei** Modellaufträge (`chat`, `ask_persona`)
zugleich; der dritte erhält sofort `isError` („… hat schon 2 Modellaufträge … nichts gespeichert“),
ohne Anfrage an das Modell. Andere Clients sind nicht betroffen; ein abgebrochener Auftrag gibt seinen Platz frei.

### Einfallskarte (Runde 25) {#idea-map-round-25}

Verbindungen zwischen Einfällen ändern keinen Buchtext; darum darf jeder sie anlegen und entfernen (wie Einfälle,
IDEA_CRUD), auch Verbindungen anderer. Herkunft `persona` (sie muss existieren, sonst `isError` „Unbekannte Persona …“)
oder der Client als `@mcp:<name>`; sie steht an einer neuen Verbindung (`origin`) und in jedem Commit
(`Baruch-Origin`, `Persona`, Autor des Commits; Committer der Mensch). Jede Änderung an einer Verbindung ist genau
ein Commit (Betreff und Trailer wie RPC, `docs/RPC.md` „Idea map“). Wiederherstellen über ein weiteres `idea_link`
(Trailer `idea-link-restore`). MCP setzt keine Lagen (reine Anzeige im Widget).

| Werkzeug | Argumente | Ergebnis (`structuredContent`) |
|---|---|---|
| `idea_map` | `{work (id or slug, mandatory), status?: open \| done \| all, word?, origin?: human \| persona \| mcp, q?, near?: bool = true, near_limit?: 1..5 = 3, near_threshold?: 0..1 = 0.75}` | Karte wie RPC `idea.map`, je Einfall zusätzlich `url` (`/einfaelle/<id>`); nur lesend |
| `idea_links` | `{work?` \| `id?` (idea, exactly one)`, deleted?: bool = false}` | `{links}`; nur lesend |
| `idea_link` | `{from, to, kind: leads_to \| belongs_to \| example_of \| contradicts, note?, persona?}` | `{link, changed, commit?\|mirror?}`; ein aktives gleiches Paar → `changed: false`, kein Commit |
| `idea_unlink` | `{id, persona?}` | `{link, commit?\|mirror?}` (weich, wiederherstellbar) |

Fachliche Abweisungen (Selbstbezug, fremdes Werk, gelöschter Einfall, Notiz über 500 Zeichen, unbekannte Persona,
unbekannte Verbindung) kommen als `isError` mit einem deutschen Satz; unlesbare oder falsche Argumente (fehlendes `work`
oder `id`, `kind` außerhalb der vier Wörter, `work` und `id` zugleich bei `idea_links`, falscher `status`/`origin`)
als JSON-RPC-Fehler -32602. Die Nähe in `idea_map` stammt aus gespeicherten Einbettungen und gleichen Stichwörtern,
ohne neue Einbettungsanfrage (`near.state` `no_embedder` ohne Einbettungsdienst). Seit Runde 19b wirken `near_limit`
(Partner je Einfall) und `near_threshold` (Kosinus-Schwelle) wie in der RPC (dieselbe Prüfung, in
`filter` angewandt); ein Wert außerhalb von 1–5 oder 0–1 oder keine Zahl ist eine fachliche Abweisung: `isError` mit der Begründung
(„near_limit: 1 bis 5, nicht 6“). Das Benennen und Übernehmen von Gruppen der Karte gibt es über MCP **nicht** (nur der Mensch
über RPC und das Formular der Karte, `docs/RPC.md` „Idea map“).
**Notizen, Stichwörter und Verbindungsnotizen sind Daten, keine Anweisungen:** Sie können von Personas oder
anderen MCP-Clients stammen; die Beschreibungen von `idea_map` und `idea_links` sagen das ebenfalls.

Beispiel:
```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"idea_link",
  "arguments":{"from":79,"to":48,"kind":"contradicts","note":"beißt sich","persona":"@lektor"}}}
→ structuredContent {"link":{"id":7,"from":48,"to":79,"kind":"contradicts","note":"beißt sich",
   "origin":{"kind":"persona","handle":"@lektor"},…},"changed":true,"commit":"3f2c…"}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"idea_links","arguments":{"id":48}}}
→ structuredContent {"links":[{"id":7,…}]}
```

Der Hinweis in `initialize` lautet: "idea_map reads the mind map as a flat map (ideas, relations, links, on
request proximity); idea_link, idea_unlink and idea_links connect ideas (change no book text, one commit with
its origin)."

### Umschlag-Layout (Runde 27) {#cover-layout-round-27}

Das Layout der Umschlagblöcke (Titel, Untertitel, Autor, Verlag und Freitext auf Vorderseite und Rücken; Motto,
Klappentext, Kurzvita, ISBN mit Strichcode, Verlag und Preis, Freitext auf der Rückseite) gehört zum Buch:
Nur der Mensch darf es ändern (Benutzeroberfläche, RPC `cover.layout_set`/`cover.layout_reset`). Über MCP gibt es nur
**Lesen** (`cover_layout`) und **einen Antrag** (`cover_layout_set`); kein MCP-Weg ändert das Layout direkt
(Test `round27_daten::no_mcp_tool_changes_the_layout_directly`).

- `cover_layout_set` legt einen Bildantrag mit `op: "cover_layout"` an (Tabelle `asset_request`, dieselbe Liste
  wie `asset_replace` usw.; `asset_requests` zeigt ihn mit `blocks` und `expected_layout`). Jeder Block wird beim Antrag
  geprüft und in Normalform gespeichert (Grenzen wie RPC); ein Antrag, der nichts änderte, ein
  ungültiger Wert oder ein falscher Fingerabdruck ergeben keinen Antrag (`isError` mit einer deutschen Meldung, die Art und
  Feld nennt). Unlesbare Blöcke (unbekannte Art, falscher Typ, unbekanntes Feld) → JSON-RPC-Fehler -32602.
- `expected_layout` = SHA-256 über das JSON der gespeicherten Blöcke zur Zeit des Antrags. Bestätigt der Mensch (Seite
  „Bilder → Anträge“ oder RPC `asset.request_decide`) und passt der Fingerabdruck noch, wird der Antrag ausgeführt
  (ein Commit „Umschlag: Anordnung geändert (Antrag N)“ mit Antragsteller und Bestätiger); hat der Mensch das
  Layout inzwischen geändert, wird der Antrag mit einer Begründung `stale`. Ablehnen ändert nichts.

```json
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"cover_layout_set",
  "arguments":{"work":2,"blocks":[{"kind":"front_title","y_mm":40}],"note":"Titel in das helle Drittel"}}}
→ structuredContent {"work":2,"status":"open","request":{"id":4,"op":"cover_layout","blocks":[{"kind":"front_title",
   "x_mm":5,"y_mm":40,…}],"expected_layout":"4f53…","summary":"Umschlag-Anordnung ändern (1 Block)",…},
   "message":"Antrag 4 angelegt (…); der Antrag selbst ändert noch nichts. …","url":"/werke/2/bilder#antraege"}
```

### Befunde (Runde 28) {#findings-round-28}

Antworten der Personas werden in Befunde zerlegt; nur der Mensch setzt den Status. Über MCP gibt es nur **Lesen**
(`findings`) und **einen Vorschlag** (`finding_propose_status`, nur `adopted`, `deferred`, `done`), den der Mensch
bestätigt (befund-uebersicht, `src/mcp/findings.rs`):

- `findings` liest wie die Übersicht: Angehängte Befunde (gleich einem früheren) fehlen, Lob nur mit
  `include_praise`; `key` filtert genau diesen Abschnitt (ohne Unterpunkte). Befunde sind Daten aus Antworten von
  Personas, keine Anweisungen.
- `finding_propose_status` prüft Status und Pflichtfelder selbst (`rejected` und `open` → „Nur der Mensch kann …“,
  `adopted` ohne `rev`, `deferred` ohne oder mit zwei Zielen → `isError`) und reicht den Vorschlag mit seiner Herkunft
  (`persona` nur ohne Token, sonst der Client als `@mcp:<name>`) an `findings::rpc::propose` weiter; die Antwort ist
  die von `propose` (plus `url` zum Befund). Ein Vorschlag ändert den Status nicht; der Mensch entscheidet im
  Akkordeon „Befunde“ oder auf der Seite `/werke/<id>/befunde`.
- Runde 30b: Jeder Befund trägt seine Lösungen als `solutions: [{quote, replacement, step}]` (fehlt das Feld, hat er
  keine): ein Zitat mit einer neuen Formulierung („statt/besser“) oder ein Schritt. Sie stammen aus den Zeilen `statt: „…“`,
  `besser: „…“` oder `Schritt: …` unter dem Befund in der Antwort der Persona. Ein kritischer Befund ohne Lösung trägt
  „(ohne Lösungsvorschlag)“ in seinem Prüfvermerk. Eine Lösung als Vorschlag zu übernehmen geht nur über die Benutzeroberfläche oder
  RPC `solution.adopt` (Mensch); MCP schreibt Vorschläge über `propose_revision` oder `solution_variants`.

```json
{"name":"findings","arguments":{"work":2,"key":13,"status":"open"}}
→ {"work":2,"total":2,"truncated":false,"findings":[{"id":1,"persona":"@lektor","kind":"hint","kind_label":"Hinweis","status":"open",
   "status_label":"offen","key":13,"body":"Der Übergang zum Nebel fehlt.","url":"/werke/2/befunde?befund=1",…}],
   "url":"/werke/2/befunde"}
```

**Wie Befunde entstehen** (befunde, umgesetzt R28a): Antworten aus `ask_persona` (ask, review) und `chat` (Abschnitt
und Faden) werden gleich nach dem Speichern des Beitrags in Befunde zerlegt (`findings::ingest`): ein Befund je Aufzählungspunkt,
je Zeile mit Marker („Kritisch:“, „Vorschlag:“, „Hinweis:“, „Lob:“) und je Punkt unter „Was trägt“ (Lob),
„Was fehlt“, „Was stört“ (Hinweis); ein wörtliches Zitat „…“ wird `quote`, der Prüfvermerk („(nicht im
Text belegt)“ und andere) `quote_check`, „[B<id>]“ ein Verweis. Vorschläge (`propose`) und Notizen von Baruch werden nie
Befunde, eine Antwort ohne Liste und ohne Marker ergibt keine. Der Beitrag bleibt Byte-identisch, und das Anlegen ist
kein Commit (`befunde.md` steht im nächsten Schnappschuss). Gleicht ein neuer Befund einem schon entschiedenen des Abschnitts
(oder einem offenen derselben Persona dort), wird er angehängt und zählt nicht als offen. Beiträge eines
MCP-Clients (`add_post`, Autor `@mcp:<name>`) werden nicht zerlegt; der Mensch kann jeden Beitrag selbst umwandeln
(`post.to_findings`, chat-auftrag). Den Status setzt nur der Mensch (RPC `finding.set_status`, `docs/RPC.md`); MCP
kann ihn nur vorschlagen (`finding_propose_status`: nur `adopted`, `deferred`, `done`; `rejected`/`open` →
`isError` „Nur der Mensch kann …“; ein gleicher offener Vorschlag nicht doppelt; ein Commit je Vorschlag
`Baruch-Action: finding-propose`).

### `text_search` (Runde 30a) {#text_search-round-30a}

Suche im Werk, nur lesend (`src/mcp/search.rs`). Argumente: `work`, `q` (höchstens 500 Zeichen), `case`, `word`,
`re` (Syntax der Rust-Crate `regex`), `range` = `section|chapter|part|from|work` (Voreinstellung `work`), `key`, `kinds`
(Voreinstellung `text, heading, footnote, quote`), `limit` (1–2000, Voreinstellung 100). Antwort `{work, range, total, truncated,
timed_out, hits}` mit denselben Feldern wie RPC `search.find` (Abschnitt `key`, Nummer, Titel, Fassung, Textart, Byte-
und UTF-16-Positionen, Zeile, Kontext, `url`). Zeitgrenze 2 s. `range=all` gibt es über MCP nicht. Ersetzen gibt es über
MCP nicht: Vorschläge je Abschnitt gehen weiter über `propose_revision`. Das Werkzeug `search` (Volltext) bleibt
unverändert.

### `solution_variants` (Runde 30b) {#solution_variants-round-30b}

Varianten eines Abschnitts als Vorschläge (`src/mcp/solutions.rs` → `assist::variants::run`); zählt wie `chat` und
`ask_persona` zum Kontingent je Client (`mcp/tools.rs` nimmt es vor dem Aufruf). Der Lektor (nach der Einstellung) setzt die
genannten Befunde (`findings`, Kennungen aus `[B<id>]`, nur dieses Abschnitts; ohne Angabe die offenen des Abschnitts) mit
ihren Lösungen in 2 oder 3 Varianten um (`count`, Voreinstellung 2): Geschwistervorschläge auf derselben Basis `rev`
(Voreinstellung: aktuelle Fassung), Bezeichnung A/B/C, eine Zeile, was sie anders macht, und die umgesetzten Befunde. Lange
Abschnitte erhalten Teilvorschläge (nur die betroffenen Absätze). Es entstehen nur Vorschläge – nur der Mensch kann einen übernehmen und die
übrigen verwerfen (`solution.reject_siblings`). `count` außer 2/3 → `isError`; unbekannter
Abschnitt, kein aktiver Lektor oder keine Befunde → `isError` mit einer ehrlichen Meldung.

```json
{"name":"solution_variants","arguments":{"work":2,"key":13,"findings":[4,7],"count":2}}
→ {"group":"5b0d…","base":3,"persona":"@lektor","status":"proposed","work":2,"key":13,
   "variants":[{"rev":5,"label":"A","summary":"knapp: nur [B4]","findings":[4],"url":"/?work=2&key=13&rev=5"},
               {"rev":6,"label":"B","summary":"alle Lösungen, Absatz umgestellt","findings":[4,7],"url":"…"}],
   "notes":[],"url":"/varianten?work=2&key=13&group=5b0d…"}
```

### `ask_persona`: Grundlage der Prüfung (Runde 30b) {#ask_persona-basis-of-the-review-round-30b}

`ask_persona` (ask, review) prüft die gezeigte Fassung `rev`; ohne `rev` die aktuelle – zeigt diese nur die Überschrift
und gibt es einen Vorschlag mit Text, den Vorschlag (`assist::basis::choose`, gewählt in `assist::run`). Der Beitrag
trägt die Kopfzeile „geprüft: …“ (`PostMeta::basis_label`, aus dem Aufruf, nie vom Modell). Das Ergebnis des Auftrags
kennt sie als `Outcome.basis = {rev, label, proposal}`; in der MCP-Antwort steht sie als `basis`. Jede Antwort folgt der Pflicht, Lösungen zu geben (siehe
„Befunde“): Ein kritischer Punkt ohne Lösung wird einmal nachgefordert, sonst „(ohne Lösungsvorschlag)“.

Kritikkreislauf (Fehler 0.9.0): `ask_persona` review mit `rev` = Nummer eines Vorschlags prüft diesen Vorschlag seinerseits
(eigene offene Befunde im Auftrag, „Erledigt: Befund n“ wird ein Statusvorschlag, Zitate aus dem Vorschlag zählen);
`ask_persona` propose mit `rev` legt den neuen Vorschlag **auf** der Fassung `rev` an (`based_on` = `rev`, vorher immer die
aktuelle Fassung).

### `chat` und `post_delete` (Runde 30c) {#chat-and-post_delete-round-30c}

**`chat`** (Abschnittschat und Faden, `src/assist/chat.rs`, `src/threads.rs`):
- **Ansprache:** Neben `@handle` (überall) ruft auch der Name oder das Kürzel ohne „@“ **am Anfang eines Satzes oder einer Nachricht**,
  gefolgt von „,“ oder „:“, eine aktive Persona auf („Lektor, was meinst du?“, „Exeget: …“, Groß-/Kleinschreibung egal); im Fließtext
  nicht („der Lektor meint“ ruft niemanden auf). Höchstens drei Personas je Nachricht wie bisher.
- **Relay bei direkter Ansprache:** Beginnt eine Zeile einer Persona-Antwort mit `@handle:` oder einer Anrede, antwortet die angesprochene
  Persona einmal (eine Stufe, höchstens drei Relay-Antworten je Nachricht, jede Persona höchstens
  einmal, nicht bei RFC und nicht beim Schreibauftrag). Ihr Beitrag trägt intern `relay_from` und `reply_to` (sichtbar als
  „antwortet auf Beitrag n von @…“). Das Relay aus Runde 15 (jede Erwähnung in einer Antwort, alle Antworten zusammen
  höchstens drei) gilt über MCP weiter; beide zusammen rufen keine Persona zweimal auf und bilden **eine** Stufe: Eine
  Relay-Antwort ruft niemanden weiter auf, auch nicht durch eine Erwähnung. In beiden Fällen steht die erwähnende Antwort
  als Daten gerahmt im Auftrag; Baruch baut den Auftragstext (nie den rohen Modelltext als Anweisung).
- Antworten sind Beiträge mit Metadaten (geprüfte Fassung, Antwort auf, Auftrag für „wiederholen“); die Antwort von
  `chat` ändert sich nicht. Wiederholen (`post.repeat`, `thread.repeat`) gibt es nur für den Menschen (RPC, Formular).
- Fehler einer Persona stehen weiter in `errors` und zusätzlich in der Live-Anzeige des Abschnitts oder Fadens.

**`post_delete`:** unverändert, nur eigene Beiträge im Abschnittschat (ein Commit). Faden-Beiträge löscht der Mensch über
RPC `thread.post_delete` oder ✕ auf `/faden` (ein Commit „Fadenbeitrag n: gelöscht“ mit dem ganzen Text; Fäden selbst
stehen weiterhin nicht im Spiegel). Über MCP löscht `post_delete {thread, id}` nur eigene Faden-Beiträge
(`threads::delete_post_as`, `only_own`; fremde → `isError` „Nur eigene Beiträge …“, ohne Commit).

### typeset_settings (Runde 29a) {#typeset_settings-round-29a}

Eigner: satz-daten. `typeset_settings {work?}` liest die Satzeinstellungen des Werks über dieselbe Funktion wie RPC
`work.typeset_get` (`typeset_settings::load`/`view_json`) und antwortet in `structuredContent` mit genau deren Form
ohne `fields`:

- `work`, `set` (gibt es Satzeinstellungen?), `uuid`, `updated`, `updated_by` (ohne Einstellungen `null`);
- `settings`: nur die gesetzten Felder im JSON des Datenmodells (`baruch_typst::typeset::TypesetSettings`, Beispiel in
  `docs/RPC.md`); ein fehlendes Feld heißt „aus der Werkvorlage“;
- `rules`: der erzeugte Typst-Block „// Baruch: Satz“ ("" ohne Einstellungen), so wie Export, Spiegel und Satz
  ihn hinter die Werkvorlage setzen;
- `conflicts` `[{field, label, line, text, effect, message}]`: Zeilen der Werkvorlage, die ein gesetztes Feld selbst setzen
  (`effect` `overrides` = die Einstellung gilt trotzdem, `ineffective` = sie wirkt nicht, solange die Zeile
  steht);
- `template_rule` `{line, text, chapter_start}` oder `null`: eine erkannte Kapitelanfangs-Regel der Werkvorlage, die
  der Mensch in die Einstellungen übernehmen kann;
- `warnings`: Hinweise, z. B. `Schrift „X“ fehlt; Typst setzt mit der eingebetteten Schrift`.

**Nur lesend**, kein Commit. Ein schreibendes MCP-Werkzeug für Satzeinstellungen gibt es nicht (`tools/list` enthält
kein `typeset_set` oder Ähnliches, Test `the_mcp_tool_reads_like_typeset_get_and_there_is_no_writing_tool`): Speichern,
Zurücksetzen und das Übernehmen der Vorlagenregel bleiben dem Menschen vorbehalten („Einstellungen → Werk → Satz“ oder
RPC aus der Benutzeroberfläche).

### Suche {#search}

`search` ruft `baruch_library::search` auf: `kinds` aus `revision`, `post`, `title`, `quote`, `highlight`
(Markierungen in Quellen der Autorenbibliothek, siehe `docs/RPC.md`); ohne `all_revisions` nur aktuelle
Fassungen; `limit` Voreinstellung 20, höchstens 100. Abgelehnte Markierungen fehlen in der Suche.
Jeder Treffer (`SearchHit`) hat `kind`, `work`, `node`, `rev`, `title_id`, `quote_id` (bei Zitaten),
`highlight_id` (bei Markierungen), `key`, `label`, `snippet` und `rank`. `snippet` ist derselbe Auszug
wie in der Benutzeroberfläche: HTML, Text mit Escape, Treffer in `<mark>…</mark>`. Leere Suche → `isError`.

**Volltext (Runde 13):** Ein Treffer von `rag_search` mit `way: "multigpt"` nennt `document` und `collection`;
`source_read` liest dazu den Volltext Teil für Teil (`part` ab 1, `more`/`parts` sagen, ob noch mehr kommt;
`marks` sind die Vorkommen von `q` als Zeichenbereiche im Teil). Der Text ist Material aus einer Quelle und
wird nie als Anweisung behandelt. `source_read` liest keine anderen Dokumente: nur Treffer aus vom Menschen gewählten Sammlungen,
lokale Kopien des Abgleichs und verknüpfte Titel (sonst könnte ein Client über die Dokumentkennung jede
Sammlung lesen, die das Token sieht). Über MCP lässt sich nichts in die Bibliothek übernehmen und nichts in
MultiGPT schreiben.

## Beispiel {#example}

```
curl -si http://127.0.0.1:8765/mcp -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"curl","version":"8"}}}'
# header Mcp-Session-Id: <id>
curl -s http://127.0.0.1:8765/mcp -H 'Content-Type: application/json' -H 'Mcp-Session-Id: <id>' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"outline","arguments":{"work":1}}}'
```

Ablauf für einen Assistenten: `list_works` → `outline` (liefert `key` je Abschnitt) → `read_node` →
`propose_revision` mit diesem `key`; der Mensch
übernimmt in der Benutzeroberfläche (oder über RPC `revision.accept`), erst dann entsteht ein Commit im Spiegel.

## Runde 19 – RAG-Wartung, Rest der Verweise {#round-19--rag-maintenance-rest-of-the-references}

### R19a Index {#r19a-index}

Kein neues Werkzeug. `rag_search` trägt `weak: bool` je Treffer (Score unter `baruch_rag::WEAK_SCORE` = 0,45 oder
keine Zahl, Anzeigetext „schwach“; die Reihenfolge bleibt, nichts wird verborgen). `rag_status` nennt
`index.stale` (nach ausgeschöpften Versuchen liegengebliebene Aufträge) und hält `problem`, bis eine Änderung oder
„Index neu aufbauen“ (nur der Mensch, nicht über MCP) die Aufträge zurückbringt; Suchen über `rag_search` lösen keinen
neuen Versuch aus. Beide Beschreibungen nennen die neuen Felder.

### R19b pgvector {#r19b-pgvector}

- `idea_map` nimmt `near_limit` (1–5) und `near_threshold` (0–1) wie RPC `idea.map`; unzulässige Werte → `isError`
  mit Begründung (siehe „Einfallskarte (Runde 25)“). Kein neues Werkzeug: Das Benennen von Gruppen (`idea.map_groups_name`) und
  deren Übernehmen (`idea.map_group_accept`) bleiben RPC/Formular des Menschen.
- `rag_search` über den Weg pgvector liefert alle Felder in SQL gekürzt und bettet die Anfrage mit dem eingestellten
  `query_prefix` ein (Einstellung nur über RPC/Seite, nicht über MCP).

### R19c Treffer {#r19c-treffer}

- `idea_search`: neuer Parameter `mode` (`fts` Voreinstellung, `rag`). `rag` braucht `work` (sonst `isError`), liefert wie
  RPC `idea.search` Treffer mit `score` und `weak` (schwach: Ähnlichkeit unter `baruch_rag::WEAK_SCORE`, zuletzt);
  scheitert die semantische Suche, ist die Antwort `isError` mit „Bedeutungssuche nicht verfügbar: LanceDB (lokal): <Grund>.“
  Die Kurzbeschreibung des Werkzeugs nennt beide Modi (Integration).
- `scripture_check`: je Stelle `module_spacing` (der Wortlaut stimmt nur ohne Leerraum, der Code bleibt `match`, Text
  „Wortlaut stimmt (Leerraum im Modul)“) und `module_note` (je Antwort und Stelle; Modul mit auffällig vielen
  Wortbruchstücken, sonst `null`).
- `bible_lookup`: je Vers `german_ref` oder `german_problem` (Abbildung zurück in die deutsche Verszählung).
- `lexicon_lookup`: mit einer Lemma-Probe (Strong-Schreibweisen, Endungen, Umlaut, Zusammensetzung); die Antwort hat zusätzlich
  `probe` und `asked`.
- Kein neues Werkzeug.

### Integration {#integration}

- Die Kurzbeschreibung von `idea_search` (`mcp/tools.rs`) nennt `mode` "fts"/"rag", `score`/`weak` und den Ausfall als
  `isError`. Sonst keine Änderung; in Runde 19 kein neues Werkzeug (Liste wie nach Runde 29).
- Der Abnahmetest `tests/round19_abnahme.rs` prüft über MCP: `idea_map` mit `near_limit`/`near_threshold` (und `isError` bei
  `near_limit` 9), kein Werkzeug zum Übernehmen von Gruppennamen, `scripture_check` mit `module_note`.

## EPUB (Runde 20) {#epub-round-20}

Beide Werkzeuge stehen in der Liste (direkt nach `typeset_settings`) und sind seit Runde 20 umgesetzt (kein STUB);
die `instructions` beim Verbinden nennen beide.

### epub_export {#epub_export}

`{work, footnotes?: "popup"|"endnotes", include_bytes?: bool=false}` – das Werk als EPUB, **nur lesend** (kein
Commit, umgesetzt seit R20a). Antwort: `work`, `file_name` (`<slug>.epub`), `url` (`/werke/{id}/epub`), `bytes`,
`sha256`, `footnotes`, `meta` (`identifier`, `title`, `subtitle`, `author`, `language`, `isbn`, `publisher`, `year`,
`modified`), `files` (Lesereihenfolge wie bei `work.epub`), `toc` (`title`, `href`, `level`, `key`), `warnings`
(`code`, `message`, `key`; Codes wie bei `work.epub` in `docs/RPC.md`). Mit `include_bytes: true` zusätzlich
`epub_base64`, höchstens 8 MiB (`mcp::epub::MCP_EPUB_LIMIT`); größer → `isError` mit einem Hinweis auf die Adresse und auf
`baruch-server export <slug> --epub DATEI`.

### epub_check {#epub_check}

`{work, readers?: [slug], footnotes?: "popup"|"endnotes"}` – das EPUB des Werks prüfen, **nur lesend** (R20c): eigene
Struktur- und Barrierefreiheitsprüfung (mimetype, container.xml, Manifest/Spine, Wohlgeformtheit mit xmllint oder
quick-xml, Navigation, Sprache, Alternativtexte, Lesereihenfolge), Warnungen des Baus und je Reader (ohne `readers` die
Pflichtprofile; unbekannter Slug → `isError`). Antwort wie RPC `epub.check`: `{findings, xml_tool, epubcheck,
summary, readers, work, url: "/werke/{id}/epub/pruefung"}`. **Startet nie EPUBCheck** – `epubcheck` ist immer
`{status: "not_requested"}`, auch wenn ein Argument `epubcheck` mitkommt oder eine epubcheck.jar eingestellt ist; nur der
Mensch startet den Lauf auf der Prüfseite. Die Einstellung der epubcheck.jar gibt es über MCP nicht.

### Integration {#integration-1}

- `instructions` beim Verbinden nennen `epub_export` (Bytes nur auf Wunsch) und `epub_check` (startet nie EPUBCheck).
- Die Ausnahme für STUB-Beschreibungen in `tests/mcp.rs` ist entfallen; kein Werkzeug wird mehr als STUB beschrieben.

## Runde 31 – Register und Glossar {#round-31--index-and-glossary}

### Schnittstelle R31-S1 {#interface-r31-s1}

Drei Werkzeuge stehen in der Liste direkt nach `epub_check`: `glossary_list` (nur lesend), `glossary_propose` und
`index_propose` (nur Vorschläge, nur der Mensch übernimmt). In der Schnittstelle waren alle drei ehrliche Stubs; seit
R31a sind sie umgesetzt (siehe unten), die Ausnahme in `tests/mcp.rs` ist entfallen. Code: `src/mcp/glossary.rs`.

### 31a glossar-daten {#31a-glossar-daten}

Die drei Werkzeuge sind umgesetzt (`src/mcp/glossary.rs`, Beschreibungen ohne "not implemented yet"):

- **`glossary_list`** liest nur (Statusfilter, mit `word` Nachschlagen über Begriff und Varianten der angenommenen
  Begriffe). Kein Commit.
- **`glossary_propose`** legt nur einen Vorschlag an (Status `proposed`), nie einen angenommenen Begriff. Herkunft nach
  `acting_persona`: `persona` (nur ohne Token, über Loopback) oder der Client als `@mcp:<name>`; mit Token und
  `persona` → `isError` (wie bei Einfällen und Beiträgen). Bibelstellen werden mit baruch_scripture geprüft und
  einheitlich geschrieben („ps 1,3“ → „Ps 1,3“). Ein schon angenommener oder schon vorgeschlagener Begriff (gleicher
  `term_key`) → `isError`. Nur der Mensch kann übernehmen oder verwerfen (RPC `glossary.decide`, Glossarseite);
  Verwerfen löscht den Vorschlag (Grabstein).
- **`index_propose`** legt nur einen Registervorschlag an (Status `open`, bezogen auf die aktuelle Fassung des
  Abschnitts); nie eine Textänderung. `node` statt `key` → `isError` (Vertrag Runde 5). Das Wort muss in der aktuellen
  Fassung stehen (einfacher Textvergleich; die genaue Stelle bestimmt die Übernahme). Übernimmt der Mensch
  (`index.decide`), entsteht in einer Transaktion eine neue Fassung des Menschen mit der Marke; steht das Wort nicht mehr
  im Text, wird der Vorschlag „veraltet“ (stale), der Text bleibt.
- Jeder Vorschlag ist genau ein Spiegel-Commit (`glossary-propose` oder `index-propose`, Autor die Persona oder der
  Client, Trailer `Persona`). Werkzeugnamen enthalten weder `update` noch `delete` noch `create`.
- (in der Integration erledigt) Die S1-Ausnahme in `tests/mcp.rs` für STUB-Beschreibungen der drei Werkzeuge ist
  entfernt.

### Integration {#integration-2}

- `initialize` → `instructions` nennen die drei Glossar-Werkzeuge: `glossary_list` liest, `glossary_propose` und
  `index_propose` legen nur Vorschläge an, nur der Mensch kann übernehmen (`src/mcp.rs`).
- Abnahmetest `tests/round31_abnahme.rs`: Ein MCP-Client schlägt einen Registereintrag vor, nur der Mensch übernimmt ihn über das
  Formular; der Spiegel-Commit nennt den Antragsteller (`Baruch-Proposer`).
- Seit Runde 23 angeglichen: `index_propose` meldet `node` statt `key` wie die älteren schreibenden Werkzeuge als
  JSON-RPC-Fehler -32602 (vorher `isError`).
- Kein MCP-Werkzeug für „Begriffe vorschlagen lassen“ (RPC `glossary.suggest`); nur der Mensch kann es in der
  Benutzeroberfläche auslösen (Glossarseite, „An der Markierung“). Registervorschläge wandern nicht im Austauschformat mit.

## Runde 21 – Import und Bilder {#round-21--import-and-images}

### Kein Import über MCP {#no-import-over-mcp}

Die neuen Importe der Runde 21 – PDF und EPUB (`source_import.*`, Seite „PDF oder EPUB importieren…“, CLI
`import DATEI.pdf|.epub`) sowie Markdown und ZIP-Archive (`POST /werke/import`, `work.import`, `work.import_archive`,
CLI `import DATEI.md|.zip`) – gibt es ebenfalls nicht über MCP. Ein Import legt aktuelle Fassungen unter dem Namen des Menschen an,
und das darf ein Assistent nicht (Entscheidung 2026-11-05, siehe „Nur der Mensch“ oben). Außerdem entsteht eine Gliederung aus PDF
oder EPUB nie stillschweigend, sondern erst nach einer Vorschau und ausdrücklicher Bestätigung durch den Menschen. Es gibt
daher kein Werkzeug `source_import` oder Ähnliches; ein Aufruf ergibt -32602 „unbekanntes Werkzeug“.

### Kontingent für Bilder (21c) {#quota-for-images-21c}

Schreibende Bildwerkzeuge haben ein Kontingent je Antragsteller (Persona-Kürzel oder Client `@mcp:<name>`) und Werk in
**gleitenden 24 Stunden**, gezählt aus den vorhandenen Zeilen (keine eigene Tabelle, keine Migration): Bildfassungen mit
diesem Autor (`asset_revision.author_name`, angelegt durch `asset_add`; bestätigte Ersetzungen schreibt der Mensch)
und Ersetzungsanträge (`asset_request.origin_name`, gleich wie entschieden). Konstanten in
`crates/baruch-server/src/mcp/assets.rs`:

| Grenze | Wert | Konstante |
|---|---|---|
| schreibende Aufrufe (`asset_add` + `asset_replace`) | 30 | `MAX_MCP_ASSET_WRITES_PER_DAY` |
| Bytes (dekodiert) dieser Aufrufe | 64 MiB | `MAX_MCP_ASSET_BYTES_PER_DAY` |
| offene Ersetzungsanträge (egal wie alt) | 10 | `MAX_MCP_OPEN_REPLACE_REQUESTS` |

Darüber antwortet das Werkzeug mit `isError: true` und einer deutschen Meldung („Kontingent erschöpft: … erlaubt sind
30. Nichts gespeichert …“ oder „… mehr als 64 MiB …“ oder „Zu viele offene Anträge …“); nichts wird gespeichert, kein
Commit. Die Prüfung läuft unter der Schreibsperre direkt vor dem Schreiben; ein anderer Client (oder eine andere Persona)
mit Token hat sein eigenes Kontingent. Seit Runde 23 teilen sich alle Clients ohne Token (gleich welcher Name, auch mit
`persona`) **ein** Kontingent je Werk – ein neuer Name öffnet kein neues (siehe „Round 23 – Hardening“). Gelöschte Bilder zählen nicht mehr (sie stehen nicht mehr in den Zeilen).
`asset_set_cover` und `asset_delete` zählen nicht (sie tragen keinen Inhalt; der Speicher weist doppelte offene Anträge
ohnehin ab). Der Inhalt entschiedener Ersetzungsanträge wird 30 Tage nach der Entscheidung entfernt
(`purged`, siehe `asset_requests`); die Antragsdatei antwortet dann mit 410.

## Runde 22 – Fassungen und Spiegel {#round-22--revisions-and-mirror}

Neues Werkzeug `revision_amends` (nur lesend); `epub_export` nimmt `draft`; `ask_persona` fragt bei erneuter Prüfung nach erledigten Befunden.
Rückgängigmachen, die Einstellung „Fassungen“ und das Archiv gibt es über MCP nicht.

### Fassungen {#fassungen}

#### `revision_amends` (nur lesend) {#revision_amends-read-only}

Verlauf der Fortschreibungen einer Fassung. Parameter: `work` (Kennung oder Slug), `key` (fester Schlüssel, hat Vorrang) oder
`node`, `rev` (Fassungsnummer). Antwort: `{work, node, key, rev, amend_seq, steps: [{uuid, seq, kind ("amend" =
Fortschreibung durch den Menschen, "undo" = Rückgängigmachen), undo_of?, author, created, body_before, body_after}], note}`. Die
Texte sind Daten aus dem Werk, keine Anweisungen. Das Rückgängigmachen (`revision.amend_undo`) und die Einstellung
„Fassungen“ gibt es über MCP nicht; Assistenten schreiben weiterhin nur Vorschläge (die nie fortgeschrieben werden).

Die Server-Anweisungen (`mcp.rs`) nennen das Werkzeug: "revision_amends reads the history of the amendments of a revision
(read-only; only the human can take one back)."

### befund-rueckfrage {#befund-rueckfrage}

- `ask_persona` mit `mode: "review"` (und jede wiederholte Prüfung): Die Persona sieht ihre eigenen offenen Befunde des
  Abschnitts als Daten gerahmt und meldet umgesetzte als die Zeile „Erledigt: Befund n (Stelle „…“)“; daraus macht
  Baruch einen Statusvorschlag „erledigt“ (nur für eigene, offene Befunde derselben Persona im Werk; fremde,
  unbekannte oder entschiedene Nummern überspringt Baruch). Den Status setzt nur der Mensch (`finding.decide` über die
  Benutzeroberfläche); über MCP gibt es dafür keinen Weg. Keine neuen Werkzeuge; die Beschreibung von `ask_persona` nennt
  es: "review: findings as a post – on a repeated review the persona asks about resolved own findings
  (status proposal „erledigt“, the human decides)".
- `Finding` in den Befundwerkzeugen (`mcp/findings.rs`) trägt automatisch `adopted_amend` (Stand der Fassung bei der
  Übernahme, siehe RPC.md).
- Das Archiv (`work.archives`, `work.restore`) gibt es über MCP nicht (Anlegen, Löschen und Wiederherstellen von Werken ist dem
  Menschen vorbehalten).

### spiegel-entwurf {#spiegel-entwurf}

- **`epub_export`** (nur lesend): neuer Parameter `draft` (Wahrheitswert) – `true` als Entwurf (Hinweis auf der Titelseite,
  „(Entwurf)“ in den Titel-Metadaten), `false` ohne, weggelassen = nach der Satzeinstellung; die Einstellung bleibt unverändert.
  `url` trägt dann `?entwurf=1|0`.
- **`typeset_settings`** liefert `settings.draft` automatisch mit (keine Änderung am Werkzeug).
- **`exchange_export`** schreibt Unterversion 8.

## Runde 23 – Härtung {#round-23--hardening}

Keine neuen Werkzeuge. Geändert sind Autorennamen, Besitz, Kontingente, Obergrenzen und einzelne Antworten.

### mcp-namen {#mcp-namen}

**Zugang und Namen:**
Der Autor eines MCP-Clients ist `@mcp:<handle>`. Das Kürzel entsteht aus dem Namen: gekürzt, Kleinbuchstaben, Umlaute umschrieben (`ä` → `ae`, `ö` → `oe`, `ü` → `ue`, `ß` → `ss`), Akzente entfernt, jedes übrige Zeichen außerhalb von `[a-z0-9._-]` als "-", höchstens 32 Zeichen, leer → `unbekannt`. „Laptop Küche“ schreibt als `@mcp:laptop-kueche` (bis Runde 22: `@mcp:laptop-k-che`).
- **Mit Token** heißt der Client wie das Token. Kein zweites Token kann dasselbe Kürzel erhalten, auch nicht das eines widerrufenen. Für Beiträge aus der Zeit vor Runde 23 gilt zusätzlich das alte Kürzel desselben Tokens: Das Token „Laptop Küche“ bearbeitet, verschiebt und löscht seine Beiträge weiterhin unter `@mcp:laptop-k-che`. Die Seite „MCP-Zugang“ zeigt den Autor je Token und, wo er abweicht, den früheren.
- **Ohne Token** (nur vom eigenen Rechner) gilt der Name aus `clientInfo.name`. Gehört sein Kürzel zu einem Token (auch zu einem widerrufenen, Kürzel oder altes Kürzel), erhält der Name das Präfix `lokal-`: Der Client schreibt als `@mcp:lokal-<handle>` und besitzt die Beiträge des Token-Clients nicht. Das wird bei `initialize` und bei jeder Anfrage neu bestimmt, gilt also auch für ein Token, das erst später angelegt wird.
- **Eigene Beiträge** (`post_edit`, `post_delete`, `post_move`, `post_to_idea`) sind nur die unter dem so bestimmten Autor.
- **Bewusste Entscheidung:** Clients ohne Token bleiben untereinander ununterscheidbar (wer denselben Namen nennt, ist derselbe Autor), und sie dürfen mit `persona` im Namen einer Persona handeln. Loopback ist der Mensch selbst oder ein Programm, das er auf seinem Rechner gestartet hat; eine Anmeldung brächte dort keine Sicherheit. Mit Token bleibt `persona` verboten.
- **Bestehende Daten, die erst die neue Regel zusammengeführt hat** (Nachtrag Runde 23): Zwei Tokens aus der Zeit vor Runde 23 können nach der neuen Regel dasselbe Kürzel ergeben, obwohl sie vorher getrennt schrieben – „Büro“ (bis Runde 22 `@mcp:b-ro`) und „buero“ (`@mcp:buero`) ergäben beide `@mcp:buero`. In diesem Fall **behält das Token mit dem Umlaut sein altes Kürzel**: „Büro“ schreibt weiterhin als `@mcp:b-ro` und besitzt nur diesen Autor, „buero“ bleibt bei `@mcp:buero`. Keines schreibt unter dem Autor des anderen oder bearbeitet, verschiebt, löscht oder wandelt dessen Beiträge um. Die Regel (`mcp::session::keeps_legacy`): Ein Token behält sein altes Kürzel, wenn die neue Regel sein Kürzel ändert und das neue Kürzel dem Kürzel oder alten Kürzel eines anderen Tokens (auch eines widerrufenen) gleicht, das bis Runde 22 unter einem anderen Autor schrieb. Es wird bei jeder Anfrage aus der Tokenliste bestimmt (keine Migration, kein gespeicherter Zustand) und hängt nicht von der Reihenfolge ab. Für ein solches Token zeigt die Seite „MCP-Zugang“ das alte Kürzel als Autor mit dem Hinweis „behält den Autor von vor Runde 23: `@mcp:…` gehört einem anderen Token“. Neue Tokens sind nie betroffen (`mcp_token.create` weist solche Namen ab).
- **Grenzen:** Tokens, die schon vor Runde 23 dasselbe Kürzel ergaben („Laptop A“ und „laptop-a“, ebenso „Büro“ und „BÜRO“), teilen es weiterhin; neue Paare dieser Art gibt es nicht mehr. Ein Client ohne Token, dessen Name Umlaute trägt, hat seit Runde 23 einen anderen Autor und erreicht seine älteren Beiträge über MCP nicht mehr (der Mensch verwaltet sie). Legt der Mensch ein Token an, dessen Kürzel ein Client ohne Token zuvor benutzt hat, gehören die früheren Beiträge dieses Clients dem Token.

**Kontingent für Bilder:** Mit Token gilt das Kontingent je Token. Alle Clients ohne Token – gleich welcher Name, auch mit `persona` – teilen **ein** Kontingent je Werk (30 schreibende Aufrufe, 64 MiB in 24 Stunden, 10 offene Ersetzungsanträge). Ein neuer Name öffnet kein neues. Die Meldung nennt dann „Die Clients ohne Token (gemeinsames Kontingent)“.

**Kontingente ohne Token (Nachtrag Runde 23):** Ein Client ohne Token wählt seinen Namen bei jedem `initialize` frei; ein Kontingent je Name wäre keines. Darum wie bei den Bildern:
- **Modellaufträge** (`chat`, `ask_persona`, `solution_variants`): je Client weiterhin höchstens zwei zugleich; alle Clients ohne Token **zusammen** höchstens vier (`mcp::tools::quota::MAX_WITHOUT_TOKEN`). Der fünfte erhält sofort `isError` („Die Clients ohne Token haben zusammen schon 4 Modellaufträge …“), nichts geht an das Modell, nichts wird gespeichert. Clients mit Token sind nicht betroffen.
- **`add_post`** (300 Beiträge in 24 Stunden) und **`propose_revision`** (100 offene Vorschläge): Mit Token zählt der Autor des Tokens. Ohne Token zählen alle Autoren `@mcp:…`, die zu keinem Token gehören, zusammen je Werk; die Meldung nennt „Die Clients ohne Token (gemeinsames Kontingent)“. Handelt ein Client ohne Token mit `persona` im Namen einer Persona, zählt der Bestand dieser Persona (einschließlich ihrer vom Modell erzeugten Beiträge) – eine bewusste Grenze: Die Zahl der Personas ist fest, und `persona` ohne Token ist eine offene Nutzerfrage (Backlog).
- Nicht erfasst: Registervorschläge und Gliederungsanträge zählen weiter je Herkunft; dort begrenzt zusätzlich eine Obergrenze je Werk (2000 Registervorschläge) oder die Zustimmung des Menschen.

**`index_propose`:** `node` statt `key` und ein fehlender `key` sind JSON-RPC-Fehler **-32602** wie bei `propose_revision` und `add_post`. Je Werk gibt es höchstens 200 offene Registervorschläge je Herkunft und 2000 insgesamt; darüber `isError` mit einer deutschen Meldung, nichts angelegt, kein Commit.

**`ask_persona`:** zusätzlich `done_proposals`, `done_notes`, `all_done` wie bei RPC `persona.ask` – jeweils nur, wenn gesetzt.

**`chat`:** Zitatzeilen („> …“) rufen niemanden auf. Das Relay läuft für die Benutzeroberfläche und MCP über dieselbe Stufe (`assist::chat::relay_targets`): Ansprache am Zeilenanfang für alle (höchstens drei), über MCP zusätzlich jede Erwähnung, solange insgesamt höchstens drei Personas aufgerufen werden. `relayed` nennt wie bisher, wer über das Relay geantwortet hat. Bei einem Schreibauftrag gibt es kein Relay mehr (vorher konnte eine Erwähnung im Text des Vorschlags eines auslösen).

**Grenzen von `add_post` und `propose_revision` (neu):** `add_post`: Text höchstens 20 000 Zeichen, höchstens 20 Quellen (je 500 Zeichen) und 20 Verweise (Zitat je 2000 Zeichen), je Autor und Werk höchstens 300 Beiträge in 24 Stunden. `propose_revision`: Text höchstens 1 MiB, `note` 2000 Zeichen, `model` 200 Zeichen, je Autor und Werk höchstens 100 offene Vorschläge (ohne Token zählen alle Clients zusammen, siehe „Kontingente ohne Token“). Darüber `isError`, nichts gespeichert. Ein Stapel (JSON-RPC-Liste) trägt höchstens 50 Nachrichten, sonst -32600. `source_read`: `q` höchstens 500 Zeichen und 40 Wörter.

**`outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`, `outline_delete`, `outline_add` (als Antrag):** je Werk höchstens 50 offene Anträge je Herkunft; der 51. wird abgewiesen (`isError`), bis der Mensch über offene entschieden hat.

**`propose_revision`, Titelregel:** Ein Titel mit U+2028, U+2029 oder U+0085 (für Typst ein Zeilenende) wird über MCP abgewiesen; die Regel gab es schon, jetzt ist sie getestet.

**`source_read`, Freigaben (Entscheidung):** Die Treffer, die ein Dokument freigeben, bleiben im Speicher des Prozesses (höchstens 4096). Nicht dauerhaft gespeichert, weil die Freigabe aus einer Suche in den **jetzt** gewählten Sammlungen kommen soll: Eine gespeicherte Freigabe würde noch gelten, nachdem der Mensch eine Sammlung abgewählt hat. Nach einem Neustart genügt ein `rag_search`. Die Migration 0051 wird nicht verwendet.

**Geänderte Werkzeugbeschreibungen** (`tools/list`): `propose_revision`, `add_post` (Grenzen), `post_move`, `post_delete`, `post_to_idea`, `post_edit` (Besitz, `lokal-`), `ask_persona` (drei Felder), `chat` (Zitatzeilen), `index_propose` (-32602, Obergrenzen), `asset_add`, `asset_replace` (Kontingent je Token oder gemeinsam), `source_read` (`q`). Der Hinweistext von `initialize` nennt die Autorenregel und dass alle schreibenden Werkzeuge `node` mit -32602 abweisen.

### web (Wirkung auf MCP-Antworten) {#web-effect-on-mcp-answers}

- `commentary_lookup`: wie RPC `commentary.lookup` (Abbildung in die Verszählung des Kommentars, `ref` mit dem Namen der Verszählung).
- `source_read`: `q` über 500 Zeichen wird abgewiesen (der Fehler von `literature::source_read`).
- `findings`: Befunde tragen `stale_summary` wie in der RPC.
- `idea_map`: Kappungshinweis in `near.message`.
- `cover_layout_set`: ungültige Blöcke weiterhin mit demselben Wortlaut.
- Belege aus SWORD im Kontext der Personas (`ask_persona`, `chat`): Bibel- und Kommentartext stammen nun aus der
  abgebildeten Stelle (siehe Architektur).

### geheimnisse (Wirkung auf MCP-Antworten) {#geheimnisse-effect-on-mcp-answers}

`ask_persona`/`chat` mit `@belege` holen keine Adressen des eigenen Netzes mehr über
`fetch_url`, wenn die Websuche eingeschaltet ist (siehe `docs/Architektur.md`, „Filter before `fetch_url`“).

### Integration {#integration-3}

Eine Anfrage an `/mcp` ist höchstens 12 MiB groß (`MCP_BODY_LIMIT`; ein Bild von 8 MiB als Base64 passt), darüber 413.

## Runde 34 – Mindmap {#round-34--mind-map}

Einfälle sind kein Buchtext: MCP darf sie anlegen, ändern, umhängen und löschen; jede Handlung ist ein Commit mit
ihrer Herkunft (`persona` oder `@mcp:<client>`) im Spiegel. Kapitel und Teile der Mindmap gehören zur Gliederung und
lassen sich nicht über `idea_*` ändern.

| Werkzeug | Argumente | Anmerkung |
|---|---|---|
| `idea_create` / `add_idea` | wie bisher, dazu `title?`, `parent?`, `before?`, `cloud?` (wie RPC); `words` ist nicht mehr Pflicht, wenn `title` gesetzt ist | Ohne Platz landet der Einfall in der Wolke. Herkunft `persona` oder `@mcp:<client>`, ein Commit. |
| `idea_update` | wie bisher, dazu `title?` | ein Commit je Änderung |
| `idea_move` (neu) | `{id, parent?: id\|null, key?: key\|null, before?: id\|null, cloud?: bool, persona?}` | wie `idea.move`; eine verletzte Regel → `isError`. Herkunft `persona` oder `@mcp:<client>`, ein Commit „umgehängt“ im Spiegel. |
| `idea_tree` (neu) | `{work, status?, word?, origin?, q?}` | nur lesend: `{map, cloud}` wie `idea.tree` (Nähe-Linien aus, Vorschläge der Wolke an), jeder Einfall mit `url`. |
| `idea_delete` | unverändert | löscht Unterknoten mit (`deleted_ids`), wiederherstellbar. |
| `idea_read`, `idea_list`, `idea_search` | unverändert | Ausgabe mit `title`, `parent`, `position`, `main_topic`, `cloud`. |
| `idea_map`, `idea_links`, `idea_link`, `idea_unlink` | unverändert | Beschreibungen sagen "mind map"; `idea_map` ist die flache Ansicht mit `title`, `parent`, `position` je Einfall. |

Reihenfolge in `tools/list`: `idea_move` direkt nach `idea_delete`, `idea_tree` direkt nach `idea_map`. Kapitel und
Teile ändert weiterhin nur `outline_*` (Antrag). Das Einlesen von .mm/.xmind gibt es über MCP nicht. Kein Stub offen. Der
Hinweistext von `initialize` nennt `idea_tree`, `idea_move` und die Wolke.

## Runde 35 – Konten und Rollen {#round-35--accounts-and-roles}

Kein neues Werkzeug. `/mcp` bleibt bei Tokens und läuft als `Who::Open` an der Rollenprüfung vorbei; MCP-Clients
schreiben weiterhin als `@mcp:<name>`. Ein Lektor ist kein MCP-Client: Commits seiner Befunde tragen bewusst keinen
Trailer `Persona`, sondern `Baruch-Origin: lektor` und `Baruch-Lektor`.

Am Zugang zu `/mcp` ändert sich nichts (siehe „Zugang“): ohne Token nur vom eigenen Rechner, sonst mit Token –
auch wenn Konten eingerichtet sind. Die Anmeldeschicht (`auth::gate`) lässt `/mcp` unberührt; eine Sitzung
(Cookie) gewährt über MCP kein Recht und wird dort nicht gelesen. Wer als Lektor mitarbeitet, nutzt die Benutzeroberfläche; ein
MCP-Token bleibt ein Zugang mit den Rechten eines MCP-Clients (Vorschläge, Beiträge, Einfälle, Anträge), nicht
auf geteilte Werke beschränkt. Offene Frage an den Nutzer (Backlog): Tokens je Lektor und Werk.

## Runde 36 – Reste {#round-36--leftovers}

Kein neues Werkzeug. Verhalten, das MCP-Clients bemerken können:

- **`chat`:** Eine direkt angesprochene Persona (`@handle`) erhält ihre eigenen offenen Befunde nur bei einem
  Prüfauftrag in den Kontext (ein Wort der Nachricht beginnt mit `prüf`, `überprüf`, `nachprüf`, `gegenprüf`, `kontrollier`,
  `check` oder `review`, oder die Nachricht nennt „[B<n>]“) und bei der RFC – nicht bei einer einfachen Frage.
  `ask_persona` mit `mode: "review"` bleibt immer eine wiederholte Prüfung.
- **`ask_persona` mit `mode: "propose"`, `scope: "chapter"`:** Der Vorschlag für den Abschnitt des Auftrags beruht
  auf `rev` (die gezeigte Fassung, auch ein offener Vorschlag), die anderen Teile des Kapitels auf ihrer aktuellen
  Fassung. Es bleibt ein Vorschlag; nur der Mensch kann ihn übernehmen.
- **`outline_delete`:** weiterhin nur ein Antrag. Bestätigt ihn der Mensch, nennt der Commit im Git-Spiegel jetzt
  auch die mitgelöschten Fassungen (offene Vorschläge getrennt gezählt) und Beiträge in ihrem Wortlaut.
- **Herkunft `@lektorat:<name>`:** Über MCP bleibt der Autor einer Fassung oder eines Beitrags das Kürzel; nur die
  Werkseite zeigt den Anzeigenamen.

## Runde 38 – Englisch und Werksprache {#round-38--english-and-work-language}

Ab Runde 38 wurden neue Abschnitte dieses Dokuments auf Englisch geschrieben; seit Runde 38c ist das ganze Dokument
englisch.

**Englische Schnittstelle.** `initialize` (`instructions`), `tools/list` (jede Beschreibung und jede Parameter-
beschreibung), die Protokollfehler (`unknown tool …`, `unknown method …`, `jsonrpc 2.0 with method expected`, `object
or list expected`, `Too many messages in one batch …`), die 401-Texte des Endpunkts und jeder Fehlertext, der
nur in `src/mcp/**` entsteht (Argumentprüfungen, Grenzen und Kontingente, "Request … filed"), sind englisch. Die Fehlercodes sind
unverändert (-32602 für unlesbare Argumente und für `node` in einem schreibenden Werkzeug, -32601, -32600, -32603; `isError` für
Abweisungen). Beschreibungen von Werkzeugen, die nichts sofort ändern, beginnen mit `Request:` (Gliederung, Bilder, Umschlag-
Layout) oder `Proposal:` (Befundstatus, Glossar, Register).

**Noch deutsch (bis Runde 39)** sind Meldungen, die auch die Benutzeroberfläche zeigt und die nur durch
MCP hindurchgehen: die Texte des Speichers und der RPC-Module hinter einem Werkzeug (unbekannter Abschnitt oder unbekannte Fassung, Abweisungen von
`post_*`, `idea_*`, `chat`, `ask_persona`, `text_search` …), die Notiz des Umschlag-Antrags, den `asset_add`
mit `role: "cover"` anlegt, der Clientname `unbekannt` eines Antrags ohne Sitzung und die Antworten von Personas in einem
deutschen Werk. Die Grenze ist in `docs/RPC.md`, „Language of the interfaces“, beschrieben.

**Werksprache.** Jedes Werk hat eine Sprache, `de` oder `en` (`docs/RPC.md`, „Work language“). `list_works` trägt
`language` je Werk, `outline` auf der obersten Ebene. `ask_persona`, `chat` und `solution_variants` lassen die Persona
in der Werksprache antworten, mit den Markern dieser Sprache (`Kritisch:` oder `Critical:` …; Tabelle in
`docs/RPC.md`, „Prompts and markers“). Kein Werkzeug setzt die Werksprache: Das ist dem Menschen vorbehalten
(„Einstellungen → Werk → Werksprache“, RPC `work.language_set`). `exchange_export` schreibt die Sprache als Attribut
`sprache` an `<angaben>` (Austauschformat Unterversion 10).

**Werksprache in Ausgaben und rückgelesenen Wörtern (Runde 39b).** `epub_export`, `epub_check` und das Typst von
`typst_check` folgen der Werksprache für jedes erzeugte Buchwort (Navigation, Landmarken, Umschlag, Titelseite,
Anmerkungen, Glossar- und Registerseiten, Nummerierungswörter wie `Chapter 1`/`Part One`, Seitenabkürzung `p.`, Entwurfs-
Wasserzeichen `DRAFT`, Impressumszeilen); das Sprachkennzeichen ist die Satzeinstellung `lang`, sonst bei einem deutschen Werk die
Vorlagensprache oder `de`, bei einem englischen Werk `en` (`docs/RPC.md`, „Work language“). `scripture_check` schreibt
`wording` in der Sprache des Werks: deutsche Werke wie bisher (`ungeprüft`, `Wortlaut stimmt`, …), englische Werke
`unchecked`, `wording matches`, `wording differs`, `no quotation found`, `reference invalid – wording not checked`,
`Verse missing in the module …`; die Codes (`wording_code`, `status`) sind in jeder Sprache dieselben. Die Schlusszeilen, die Baruch an Persona-Antworten mit Bibelstellen anhängt, und die Hinweise zu Stellenbehauptungen (`(reference is in
the text: …)`) sind in einem englischen Werk englisch und in einem deutschen deutsch; jede Erkennung akzeptiert beide.

**Stubs.** Zurzeit ist kein Werkzeug ein Stub; ein künftiger Stub antwortet ehrlich auf Englisch ("not implemented yet", mit dem, was
fehlt) und trägt `// STUB: <what is missing> (<stage>)` im Code (`grep -rn "STUB:" crates`).
