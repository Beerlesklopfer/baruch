# MCP-Server von Baruch

`POST /mcp`, JSON-RPC 2.0 nach Model Context Protocol **2025-06-18** (Streamable HTTP, nur JSON-Antworten,
kein SSE). Quelle: `crates/baruch-server/src/mcp.rs` (Protokoll, Zugang), `src/mcp/tools.rs` (Werkzeuge),
`src/mcp/ideas.rs` (Einfälle), `src/mcp/session.rs` (Sitzungen), `src/mcp/scripture.rs` (`scripture_check`),
`src/mcp/scripture_map.rs` (`scripture_map`, Runde 17a),
`src/mcp/bibles.rs` (`bible_*`, SWORD-Bibeln), `src/mcp/exchange.rs` (`exchange_export`, Runde 26),
`src/mcp/cover.rs` (`cover_layout`, `cover_layout_set`, Runde 27),
`src/mcp/tokens.rs` (RPC `mcp_token.*`). Laut Plan der gemeinsame Eingang für alle Personas und Werkzeuge wie
Claude Code („Eigener Endpunkt je Persona“, eingehend).

**Stand:** Lesen (Werke, Gliederung, Abschnitte, Suche, Personas, Bilder) und Schreiben als Vorschlag bzw.
Beitrag sind umgesetzt, dazu Einfälle mit vollem CRUD (ändern keinen Buchtext, siehe „Einfälle“) und die
Gliederung (anlegen sofort, alles andere als Antrag, siehe „Gliederung“). Seit Runde 10 (B2) antworten Personas
wirklich (`ask_persona`, `chat` über Eliezer), `scripture_check` prüft Bibelstellen (seit 2026-11-06 mit gewählter
SWORD-Bibel auch den Wortlaut, `bible_*` lesen die Bibel), und andere Rechner kommen mit
einem Token herein (siehe „Zugang“). Offene Stubs im Code:
`grep -rn "STUB:" crates`.

**Nur der Mensch:** Übernehmen, Ändern, Löschen und Umschalten von Revisionen gehen nur über die Oberfläche
bzw. RPC (`docs/RPC.md`). **Ändern, Löschen und Umschalten gibt es über MCP nicht**, ebenso keinen Import:
Ein Import legt aktuelle Revisionen unter dem Namen des Menschen an, und das darf ein Assistent nicht
(Entscheidung 2026-11-05). Typst-Bücher kommen über `baruch-server import` ins Programm
(`docs/Bedienung.md`) oder über das Hochladen in der Oberfläche. Ein Aufruf von `import_typst` ergibt heute
-32602 „unbekanntes Werkzeug“. Ebenso gibt es **kein Einlesen einer Austauschdatei** über MCP (Runde 26): Ausgeben
geht lesend mit `exchange_export`, Einlesen nur durch den Menschen (Seite „Austausch: Einlesen…“, RPC
`work.exchange_preview`/`work.exchange_import`, `baruch-server import DATEI.baruch`). Ebenso gibt es kein Werkzeug, das Werke anlegt, ändert (Titel, Metadaten,
Werkvorlage) oder löscht: Das ist die Werkverwaltung des Menschen (`/werke`, RPC `work.create/update/delete`,
`docs/RPC.md` „Werke“). `list_works` liest nur.

## Zugang

1. **Fremde Webseiten:** `/mcp` liegt wie `/rpc` hinter `guard::same_origin`. Passt `Origin` nicht zu `Host`
   oder ist `Sec-Fetch-Site` weder `same-origin` noch `none`, antwortet der Server **403** – noch vor jeder
   weiteren Prüfung. Clients ohne `Origin` (Claude Code, curl, Skripte) gehen durch.
2. **Ohne Token nur vom eigenen Rechner:** Ohne `Authorization` nimmt der Endpunkt nur Verbindungen von
   `127.0.0.1`, `::1` (auch `::ffff:127.0.0.1`) an. Jede andere Gegenstelle – und eine Anfrage, deren
   Gegenstelle der Server nicht kennt – bekommt **401** mit `WWW-Authenticate: Bearer realm="baruch"` und dem
   Text „MCP ohne Token nur vom eigenen Rechner (127.0.0.1 oder ::1). Von anderen Rechnern mit
   „Authorization: Bearer <Token>“; Tokens erzeugt die Einstellungsseite „MCP-Zugang“.“
   **Ausnahme hinter einem Reverse-Proxy (Runde 24):** Ist Baruch mit `--allow-host NAME` bzw.
   `BARUCH_ALLOWED_HOSTS` gestartet und trägt die Anfrage in `Host` einen dieser zusätzlichen Namen (mit oder
   ohne Port), gilt sie **nie als lokal** – auch wenn die Gegenstelle `127.0.0.1` oder `::1` ist, denn das ist
   dann der Proxy und nicht der Mensch am eigenen Rechner. `/mcp` geht unter einem solchen Namen
   **nur mit Token**; ohne `Authorization` kommt dieselbe **401**. Sind zusätzliche Namen erlaubt, gilt auch eine Anfrage
   ganz ohne lesbare `Host`-Kopfzeile nicht als lokal (`guard::via_proxy`). Unter `127.0.0.1`, `::1` oder
   `localhost` bleibt es auf demselben Server bei der Regel oben. Grenze: Schreibt der Proxy `Host` auf
   `127.0.0.1:<Port>` um, kann Baruch die Anfrage nicht von einer lokalen unterscheiden – der Proxy muss den
   Namen durchreichen (`docs/Betrieb.md`, mit Probe nach dem Einrichten). `X-Forwarded-*` wertet Baruch nicht aus.
   **Mit Token (Runde 10):** `Authorization: Bearer <Token>` (erzeugt unter Einstellungen → MCP-Zugang bzw.
   RPC `mcp_token.create`, gespeichert nur als SHA-256-Hash) öffnet `/mcp` von jedem Rechner. Der Client heißt
   dann wie der Token (`@mcp:<tokenname>`, auch ohne oder mit fremder Sitzung; aus `clientInfo` zählt nur die
   Version) und darf nie `persona` als Autor oder Herkunft nennen (siehe „Beiträge verwalten“). Ein falscher oder widerrufener Token oder ein anderes Verfahren als Bearer → **401** mit
   `error="invalid_token"`, auch über Loopback. Von anderen Rechnern ist **nur** `/mcp` erreichbar
   (`guard::remote_only_mcp`, sonst 403). Dafür muss Baruch auf einer Netzadresse lauschen (`--listen`), der
   Client spricht es über die IP-Adresse an (siehe 4.); Baruch spricht nur HTTP ohne TLS – Token und Texte
   gehen im Klartext durchs Netz, nur im eigenen Netz oder über einen Tunnel verwenden.
3. **Inhaltstyp:** `Content-Type: application/json`, sonst 415.
4. **Kein fremder Name:** `Host` muss eine IP-Adresse, `localhost` oder ein beim Start mit `--allow-host`
   erlaubter Rechnername sein (Runde 24, genau dieser Name, keine Unterdomänen), sonst **421** (gilt für alle
   Routen, `guard::local_host`). So kann eine Webseite per DNS-Rebinding (fremder Name zeigt auf
   127.0.0.1) weder Werkzeuge aufrufen noch Texte lesen. Ohne `--allow-host` ist kein Name erlaubt.

Der Server muss die Gegenstelle kennen (`into_make_service_with_connect_info::<SocketAddr>()` in `main.rs`);
sonst gilt jede Anfrage als fremd.

Einrichten in Claude Code (Server lokal auf dem Vorgabe-Port 8765, nur für dieses Projekt):

```
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

Von einem anderen Rechner (Token aus Einstellungen → MCP-Zugang):

```
claude mcp add -s local --transport http baruch http://<IP>:8765/mcp --header "Authorization: Bearer <Token>"
```

Hinter einem Reverse-Proxy mit TLS (Name mit `--allow-host` erlaubt, immer mit Token):

```
claude mcp add -s local --transport http baruch https://<Name>/mcp --header "Authorization: Bearer <Token>"
```

Der systemd-Benutzerdienst (`packaging/systemd/`) nutzt denselben Port. Ein älterer Eintrag mit einem
anderen Port wird mit `claude mcp remove baruch -s local` entfernt und wie oben neu gesetzt.

## Sitzungen

`initialize` merkt sich `clientInfo` (`name`, `version`) unter einer neuen Sitzung und schickt deren Id in
der Antwort-Kopfzeile **`Mcp-Session-Id`** mit (32 Hex-Zeichen). Spätere Anfragen mit dieser Kopfzeile
werden dem Client zugeschrieben; ohne oder mit unbekannter Id gilt der Client als „unbekannt“. Die Tabelle
lebt im Prozess (nach Neustart leer) und hält höchstens 256 Sitzungen; die älteste fällt zuerst heraus.
Die Id ist kein Geheimnis und ersetzt kein Token.

Aus `clientInfo` werden:
- **Client-Handle** `@mcp:<name>`: Name klein, Zeichen außerhalb `[a-z0-9._-]` → `-`, höchstens 32 Zeichen,
  leer → `unbekannt`. Beispiel: „Claude Code“ → `@mcp:claude-code`.
- **Modell** (wenn der Aufruf keines nennt): „<name> <version>“, ohne Version nur der Name, ohne Client
  „unbekannt“.

## Nachrichten

| Methode | Antwort |
|---|---|
| `initialize` | `protocolVersion` (die des Clients, sonst `2025-06-18`), `capabilities.tools`, `serverInfo`, `instructions`; Kopfzeile `Mcp-Session-Id` |
| `ping` | `{}` |
| `tools/list` | `{tools: [{name, description, inputSchema}]}` in der Reihenfolge der Tabelle unten |
| `tools/call {name, arguments}` | siehe „Antworten“ |
| Benachrichtigung (ohne `id`) | HTTP 202 ohne Rumpf |

Eine Liste von Nachrichten wird der Reihe nach beantwortet; nur Benachrichtigungen → 202.

### Antworten von `tools/call`

- **Erfolg:** `{content: [{type: "text", text: <JSON als Text>}], structuredContent: <dasselbe JSON>, isError: false}`.
  `structuredContent` ist immer ein Objekt (MCP verlangt das); Listen stehen darin unter einem Namen
  (`works`, `nodes`, `hits`, `personas`).
- **Fachlicher Fehler** (unbekanntes Werk, unbekannter oder nicht mehr vorhandener Abschnitt, unbekannte
  Revision oder Persona, leerer Text, Text gleich der aktuellen Revision, Überschriften passen nicht zum
  Abschnitt, ungültiger Verweis, Stub): `{content: [{type: "text", text}], isError: true}`
  mit deutschem Text.
- **Protokollfehler:** unbekanntes Werkzeug (auch das entfernte `import_typst`), unlesbare `arguments` oder ein
  schreibendes Werkzeug mit `node` bzw. ohne `key` (siehe unten) → JSON-RPC-Fehler **-32602** mit deutschem
  Hinweis; Datenbankfehler → **-32603**.

## Werkzeuge

`work` ist überall die id (Zahl) oder der slug (Text) aus `list_works`; `node` ist die **Stelle** des
Abschnitts im Werk (`node` aus `outline`, ab 0), `key` sein **fester Schlüssel** (`key` aus `outline` bzw.
`read_node`); `rev` ist die **Nummer** einer Revision (`number` in `read_node`, ab 1).

**Achtung, Stellen ändern sich:** Verschiebt, stuft, legt an, löscht, teilt oder führt der Mensch Punkte
zusammen, vergibt Baruch die Stellen `node` neu. Ein Assistent, der `outline` vorher gelesen hat, träfe mit
der alten Stelle einen anderen Abschnitt. Darum gilt (Entscheidung nach Runde 4, umgesetzt in Runde 5):

- **Schreiben nur über `key`:** `propose_revision` und `add_post` – auch jeder Verweis in `refs` – nehmen den
  Abschnitt **nur** über `key` (Pflicht). Steht `node` in den Argumenten (auch zusätzlich zu `key`) oder fehlt
  `key`, antwortet Baruch mit JSON-RPC **-32602** und einem deutschen Hinweis, z. B. „arguments: node (Stelle)
  wird nicht angenommen; key mitgeben …“ bzw. „refs[1]: key fehlt …“; es wird nichts geschrieben. Das
  `inputSchema` beider Werkzeuge kennt kein `node` und verlangt `work`, `key` und `body`; jeder Eintrag in
  `refs` verlangt `key`, `rev` und `quote`.
- **Lesen wie bisher:** `read_node` nimmt `node` oder `key` (ist `key` da, gilt er); `outline` und `search`
  bleiben unverändert. Die Antworten nennen `node` (Stelle jetzt) und `key`.
- Gibt es den Abschnitt eines Schlüssels nicht mehr, kommt „Diesen Gliederungspunkt gibt es nicht mehr …“
  (`isError`). Der Schlüssel wird unter der Schreibsperre aufgelöst, damit niemand dazwischen umstellt.

**Vorschläge ändern die Gliederung nie:** Der Text von `propose_revision` beginnt mit genau einer Überschrift
der Ebene des Abschnitts (`=` Kapitel, `==` Abschnitt, `===` Unterabschnitt; ein neuer Titel ist erlaubt und
benennt beim Übernehmen um). Andere Ebene, weitere Überschriften, keine Überschrift oder Text davor → Fehler
mit Grund, kein Vorschlag. Ebene, Teilen und Zusammenführen bleiben dem Menschen (Editor, Gliederung).

Nummern werden nie neu vergeben; nach dem Löschen einer Fassung durch den Menschen hat die Folge Lücken
(z. B. 1, 2, 4). Baruch sucht Revisionen deshalb immer über die Nummer, nie über ihre Stelle in der Liste.

| Werkzeug | Parameter | Ergebnis (`structuredContent`) | Stand |
|---|---|---|---|
| `list_works` | – | `{works: [{id, slug, title}]}` | umgesetzt |
| `outline` | `{work}` | `{work: {id, slug, title}, nodes: [{node, key, parent, number, title, kind, current_rev, proposals, url}]}`; `current_rev` ist die Nummer der aktuellen Fassung | umgesetzt |
| `work_timing` | `{work}` | Sprechdauer des Werks (Zeitwächter), **nur lesend**, genau wie RPC `work.timing` (`docs/RPC.md`, „Zeitwächter“): `{work, kind, words_per_minute, target_seconds, words, seconds, status: "none" \| "ok" \| "over", over_seconds, shown, clock, target_clock, over_clock, sections: [{key, seq, depth, number, title, words, seconds, start_seconds, subtree_seconds, clock, start_clock, subtree_clock}]}`; gezählt wird je Punkt die aktuelle Fassung ohne Typst-Markup (Regeln in `docs/Bedienung.md`, „Zeitwächter“). Unbekanntes Werk → `isError`. Wörter je Minute und Zielzeit stellt nur der Mensch ein (Einstellungen → Werk → Predigtzeit, RPC `work.timing_set`); ein Werkzeug dafür gibt es nicht | umgesetzt (Runde 16b) |
| `read_node` | `{work, node \| key, rev?}` | `{node, key, number, title, rev, text, revisions: [{number, status, author: {kind, name, model?}, note}], posts: [{id, author, body, created}], url}`; `text` ist Revision `rev`, sonst die aktuelle (ohne aktuelle die mit der höchsten Nummer); `posts` nennt die ids, damit ein Client seine eigenen Beiträge findet | umgesetzt |
| `search` | `{query, kinds?, work?, all_revisions?, limit?}` | `{hits: [SearchHit]}` wie RPC `library.search` (`docs/RPC.md`), je Treffer zusätzlich `node_key` und `url` (`/?work=W&key=K[&rev=R]`, ohne Abschnitt `null`); ohne `work` über alle Werke, Bibliothek immer | umgesetzt |
| `typst_check` | `{work?, key?}` | `{ok, stand, pages, errors, warnings, diagnostics, print, missing, url}` – setzt das Werk mit Typst (wie RPC `preview.typst`), **nur lesend**, ändert nichts, kein Commit; `ok` = keine Fehler, `diagnostics` wie bei `preview.typst`, `url` = `/werke/W/satz?key=K` bzw. `/werke/W/satz`; unbekannter `key` oder Zeitüberschreitung (60 s) → `isError`; seit Runde 18a lässt sich der Lauf über RPC `preview.cancel` bzw. „Satz abbrechen“ der Satz-Seite abbrechen → `isError` „Satz abgebrochen.“; `@preview`-Pakete werden nicht geladen | umgesetzt (Runde 8) |
| `exchange_export` | `{work, format?: "xml" (Vorgabe) \| "baruch"}` | `{work, uuid, format, file_name, media_type, bytes, sha256, xml?, data?}` – das ganze Werk als Austauschdatei (`docs/Austauschformat.md`), **nur lesend**, ändert nichts, kein Commit; wie RPC `work.exchange_export`: bei `xml` die Datei `<slug>.baruch.xml` als Text in `xml` (Bilder als Base64), bei `baruch` der ZIP-Container `<slug>.baruch` als Base64 in `data`; `sha256` über die gelieferten Bytes. Format-Unterversion 7: samt Gedankenkarte (`<karte>` mit Verbindungen zwischen Einfällen und Lagen, seit Runde 25), Umschlag-Anordnung, Anschriften im Impressum und Satzbild-Einstellungen (seit Runde 27), Befunde und alte Beitragsfassungen (Unterversion 4, Runde 28), Lösungen, Varianten und Schlüssel der Punkte (Unterversion 5, Runde 30) Satz-Einstellungen `<satz>` (Unterversion 6, Runde 29), seit Runde 31 Unterversion 7 mit eigenen Registern, Glossar
(auch Vorschlägen) und den Register/Glossar-Feldern von `<satz>`. Ohne Schlüssel, Verbindungen zu Diensten, Persona-Modelle und Anträge. **Obergrenze 4 MiB** Datei (`MCP_EXPORT_LIMIT`, die Antwort trägt den Inhalt zweimal): darüber `isError` mit Hinweis auf die Seite `/werke/{id}/austausch` und `baruch-server export SLUG --baruch DATEI`; unbekanntes Werk → `isError`, unbekanntes `format` → -32602. Ein Einlesen gibt es über MCP nicht | umgesetzt (Runde 26) |
| `typeset_settings` | `{work?}` | `{work, set, uuid, updated, updated_by, settings, rules, conflicts, template_rule, warnings}` wie RPC `work.typeset_get` ohne `fields` – Satz-Einstellungen des Werks, die erzeugten Typst-Regeln „// Baruch: Satz“ und Hinweise, wo die Werkvorlage dieselbe Sache selbst setzt; **nur lesend**, ändern nur der Mensch (Einstellungen → Werk → Satz); kein Commit; unbekanntes Werk → `isError` | umgesetzt (Runde 29a, nur lesend) |
| `epub_export` | `{work, footnotes?: "popup" \| "endnotes", include_bytes? = false}` | `{work, file_name, url, bytes, sha256, footnotes, meta, files, toc, warnings, epub_base64?}` – das Werk als EPUB, **nur lesend**, kein Commit; Bytes nur mit `include_bytes`, höchstens 8 MiB (Einzelheiten unter „EPUB (Runde 20)“) | umgesetzt (Runde 20a) |
| `epub_check` | `{work, readers?: [slug], footnotes?}` | `{findings, xml_tool, epubcheck, summary, readers, work, url}` wie RPC `epub.check` – eigene Prüfung und Reader-Warnungen, **nur lesend**, startet nie EPUBCheck (Einzelheiten unter „EPUB (Runde 20)“) | umgesetzt (Runde 20c) |
| `glossary_list` | `{work?, status?: "accepted" \| "proposed" \| "all", word?}` | `{work, terms: [Term], url: "/werke/W/glossar"}` (mit `word` zusätzlich `word`) – Glossar des Werks, **nur lesend**, kein Commit; Term wie RPC `glossary.list`; mit `word` wie RPC `glossary.lookup` (nur angenommene Begriffe, Wort oder Variante, Groß-/Kleinschreibung egal); unbekannter Status → `isError` | umgesetzt (Runde 31a) |
| `glossary_propose` | `{work?, term, short?, long?, variants?, source?, bible_refs?, reason?, persona?}` | `{work, id, status: "proposed", origin, term, url: "/werke/W/glossar#begriff-ID", reason?, commit?\|mirror?}` – **nur Vorschlag**, Herkunft Persona (`persona`, nur ohne Token vom eigenen Rechner) bzw. `@mcp:<name>`; übernehmen, ändern, löschen nur der Mensch; schon angenommener oder schon vorgeschlagener Begriff (gleicher Schlüssel), unlesbare Bibelstelle oder Grenze überschritten → `isError`; `reason` steht nicht im Glossar, nur in der Antwort; ein Spiegel-Commit `Baruch-Action: glossary-propose` mit dem Client bzw. der Persona als Autor | umgesetzt (Runde 31a) |
| `index_propose` | `{work?, key, word, occurrence?, register?, term?, sub?, see?, also?, reason?, persona?}` (kein `node`) | `{work, id, status: "open", proposal, url: "/werke/W/register#vorschlag-ID", commit?\|mirror?}` – **nur Registervorschlag**, nie eine Textänderung; die Marke setzt der Mensch mit der Übernahme (RPC `index.decide`, neue Fassung des Menschen); `node` statt `key`, fehlender `key`, Wort nicht in der aktuellen Fassung, unbekanntes Register oder ungültige Marke → `isError`; ein Spiegel-Commit `Baruch-Action: index-propose` | umgesetzt (Runde 31a) |
| `propose_revision` | `{work, key, body, note?, persona?, model?}` (kein `node`) | `{work, node, key, rev, status: "proposed", author, model}` | umgesetzt |
| `add_post` | `{work, key, body, refs?: [{key, rev, quote}], sources?: [string], persona?}` (kein `node`, auch nicht in `refs`) | `{work, node, key, post, id, author, url}` (`post` = Stelle im Abschnitts-Chat ab 0, `id` = Beitrag) | umgesetzt |
| `post_move` | `{work, id, key, persona?}` (kein `node`) | `{work, id, node, key, from_node, from_key, url, commit?}` – **nur eigene Beiträge**, ans Ende des Chats von `key` | umgesetzt (Runde 8) |
| `post_delete` | `{work, id, thread?, persona?}` | `{work, id, node, key, deleted: {author, body}, url, commit?}` – **nur eigene Beiträge**, ohne Rückfrage (der Commit hält den Beitrag fest); mit `thread` ein eigener Fadenbeitrag (`{work, thread, id, deleted, url, commit?}`) | umgesetzt (Runde 8, `thread` Runde 30c) |
| `post_to_idea` | `{work, id, words, persona?}` | `{work, id, idea, node, key, url, commit?}` – **nur eigene Beiträge**; Einfall mit Bezug = Abschnitt, Gedanke = Text, danach ist der Beitrag fort (ein Commit) | umgesetzt (Runde 8) |
| `outline_add` | `{work, key?, place? = "after", title, level? \| items: [{title, level?}], note?, persona?, model?}` (kein `node`) | `{work, action: "added", detail, points: [{node, key, level, title, url}], node, key, url, author, adopted: [], commit?, mirror?}` – siehe „Gliederung“; Teile (`level: 0`) seit Runde 14 auch mit Werkvorlage. Übernähme ein neuer Teil vorhandene Kapitel (Nutzerfrage 12): nichts angelegt, sondern `{work, action: "requested", status: "open", request: OutlineRequest, adopted: [key], message, url}` – Antrag, dem der Mensch aktiv zustimmen muss | umgesetzt |
| `outline_rename` | `{work, key, title, note?, persona?}` | Antrag: `{work, request: OutlineRequest, status: "open", message, url}` (`url` = `/gliederung?work=W#antraege`) | umgesetzt |
| `outline_move` | `{work, key, target_key, place, note?, persona?}` (kein `node`, kein `target`) | Antrag wie oben; die Ebene bleibt wie bei RPC `outline.move` | umgesetzt |
| `outline_set_level` | `{work, key, level: 1–3, note?, persona?}` | Antrag wie oben; `level: 0` (zum Teil) und Teile umstufen → `isError` | umgesetzt |
| `outline_set_numbered` | `{work, key, numbered: bool, note?, persona?}` | Antrag wie oben (mit/ohne Nummer zählen; Teil ohne Nummer → `isError`) | umgesetzt |
| `outline_delete` | `{work, key, note?, persona?}` | Antrag wie oben (nur ohne Unterpunkte, nicht das letzte Kapitel) | umgesetzt |
| `outline_requests` | `{work, open? = true, mine? = false, preview? = false, persona?}` | `{work, requests: [OutlineRequest], url}` – `OutlineRequest` wie RPC `outline.requests` (`docs/RPC.md`); mit `preview: true` je offenem Antrag `preview: [{key, level, number, title, mark, kind_word, before_number, before_level, label, before_label, changed_label}]` bzw. `preview_error` (Runde 14, Art und Stand davor seit Runde 16c) | umgesetzt |
| `asset_add` | `{work?, file_name, content, encoding: "base64" \| "svg", name?, path?, caption?, alt?, note?, role?: "cover", persona?}` | `{work, name, rev, mime, bytes, url, author, commit? \| mirror?, request?, message?, request_error?}` – legt **sofort** ein Bild an (Fassung 1, Autor Persona bzw. `@mcp:<client>`, ein Commit mit `Baruch-Origin`); höchstens 8 MiB entschlüsselt; `role: "cover"` stellt zusätzlich einen Antrag `set_cover` (scheitert nur der Antrag, bleibt das Bild, `request_error` sagt warum); Kontingent siehe „Kontingent für Bilder (21c)“ | umgesetzt (Runde 11, Kontingent 21c) |
| `asset_replace` | `{work?, name, content, encoding, caption?, alt?, note?, rename?: bool = false, persona?}` | Antrag: `{work, request: AssetRequest, status: "open", message, url}` – nicht unverändert; ändert nichts sofort. Ein anderes Format als das Bild nur mit `rename: true` (Runde 21c; sonst `isError` mit Hinweis): **ein** Antrag, beim Bestätigen bekommt das Bild die neue Endung (`haus.svg` → `haus.png`), alle Pfade bleiben; `request.rename` = true. Kontingent siehe unten | umgesetzt (Runde 11, `rename` und Kontingent 21c) |
| `asset_set_cover` | `{work?, name: string \| null, note?, persona?}` | Antrag wie oben (`null` = Umschlag entfernen) | umgesetzt (Runde 11) |
| `asset_delete` | `{work?, name, rev?, note?, persona?}` | Antrag wie oben (ohne `rev` das ganze Bild, mit `rev` eine nicht aktuelle Fassung) | umgesetzt (Runde 11) |
| `asset_requests` | `{work?, open? = true, mine?, persona?}` | `{work, requests: [AssetRequest], url}` – lesen; Inhalte nie, nur `bytes`/`content_url`; `AssetRequest` wie RPC `asset.request_list` (`docs/RPC.md`, seit 21c mit `rename` und `purged`); entfernt vorher den Inhalt entschiedener Anträge zum Ersetzen, die älter als 30 Tage entschieden sind | umgesetzt (Runde 11, Aufräumen 21c) |
| `cover_layout` | `{work}` | wie RPC `cover.layout` (`docs/RPC.md`, „Umschlag-Anordnung und Impressum“): alle 15 Blöcke (gespeichert oder Vorgabe) mit Lage und Breite in mm ab der Schnittkante der eigenen Fläche, Maße, Druckprüfung, Vorgabeschrift; ohne Seitenmaß (Satzfehler im Buch) `{…, blocks: nur die gespeicherten, geometry: null, geometry_error}`; nur lesend | umgesetzt (Runde 27) |
| `cover_layout_set` | `{work, blocks: [CoverBlock], note?, persona?}` | Antrag (Bildanträge, op `cover_layout`, Herkunft Persona bzw. `@mcp:<client>`) → `{work, request, status: "open", message, url}`; jeder genannte Block ersetzt seine Art ganz, fehlende Felder eines Blocks kommen aus dem wirksamen Block (z. B. `{kind: "front_title", y_mm: 40}`); der Antrag merkt sich den Fingerabdruck der gespeicherten Anordnung (`expected_layout`); ein gleicher offener Antrag wird nicht doppelt angelegt; ungültige Werte (Grenzen wie RPC `cover.layout_set`) oder ein Antrag, der nichts ändern würde → `isError`; ändert nichts sofort, der Mensch bestätigt auf der Seite Bilder → Anträge (veraltet, wenn er die Anordnung inzwischen geändert hat) | umgesetzt (Runde 27) |
| `findings` | `{work, key?, status?, kind?, persona?, include_praise?, limit?}` | `{work, total, truncated, findings: [Befund], url}` – nur lesend; `total` = alle passenden (ohne Grenze), `truncated`, wenn nur die neuesten `limit` mitkommen; Befund wie `finding_json` (alle Felder von `Finding`, dazu `kind_label`, `status_label`, `url`); `key` nur dieser Abschnitt; `status`/`kind` Datenbankwörter mit Komma; ohne angehängte, Lob nur mit `include_praise`; unbekanntes Wort oder Abschnitt → `isError` | umgesetzt (Runde 28) |
| `finding_propose_status` | `{work, id, status: adopted\|deferred\|done, rev?, target_key?, target_idea?, note?, persona?}` | nur ein Vorschlag zum Status eines Befunds (Herkunft `persona` bzw. `@mcp:<client>`) über `findings::rpc::propose` (Sperre, ein Commit), den der Mensch bestätigt; `adopted` braucht `rev`, `deferred` genau eins von `target_key`/`target_idea`; `rejected`/`open` → `isError` „Nur der Mensch kann …“ | umgesetzt (Runde 28; Statusweg selbst: befunde) |
| `post_edit` | `{work, id, body, thread?}` | eigenen Beitrag bearbeiten (nur Autor `@mcp:<client>`); die alte Fassung bleibt; im Abschnitts-Chat ein Commit, im Faden keiner (siehe „Beiträge verwalten“) | umgesetzt (Runde 28) |
| `text_search` | `{work, q, case?, word?, re?, range?, key?, kinds?, limit?}` | im Werk suchen wie RPC `search.find` (Wortlaut, ganzes Wort, regulärer Ausdruck; `range` `section`/`chapter`/`part`/`from`/`work`, Vorgabe `work`; `kinds` Vorgabe `text`, `heading`, `footnote`, `quote`) – nur lesend; Ersetzen gibt es über MCP nicht, Vorschläge je Abschnitt über `propose_revision`; `search` (Volltext) bleibt | umgesetzt (Runde 30a) |
| `solution_variants` | `{work, key, rev?, findings?, count?: 2\|3}` | der Lektor (nach Einstellung) schreibt 2 oder 3 Varianten A/B/C als Geschwister-Vorschläge mit gleicher Basis; legt nur Vorschläge an, zählt zur Grenze von zwei Modellaufträgen je Client | umgesetzt (Runde 30b) |
| `list_personas` | – | `{personas: [{handle, name, role, model, context_length, active, lector}], lector, lector_mode}`; `lector` folgt der Wahl (fest, Vorgabe `@lektor`, oder automatisch nach Kontext) | umgesetzt |
| `list_assets` | `{work?}` | `{work, assets: [AssetSummary]}` wie RPC `asset.list`; ohne `work` das Vorgabewerk; nur lesend | umgesetzt |
| `read_asset` | `{work?, name, rev?, content? = false}` | `{work, name, revision, revisions, encoding}`, mit `content: true` auch der Inhalt (Base64 bzw. SVG-Text); unbekanntes Bild oder Fassung → `isError`; nur lesend | umgesetzt |
| `sources_list` | – | `{sources: [{id, title, url, note, position}]}` – die bevorzugten Quellen für `@belege` (Einstellungen → Quellen) in fester Reihenfolge; nur lesend, ändern kann sie nur der Mensch | umgesetzt (Runde 9) |
| `idea_create` | `{words, work?, key?, note?, quote?, source?, bible?, persona?}` | `{idea, commit?}` wie RPC `idea.create`; jeder Einfall in den `idea_*`-Antworten trägt `url` (mit Bezug `/?work=W&key=K`, nur Werk `/?work=W`, sonst `null`) | umgesetzt |
| `add_idea` | wie `idea_create` | wie `idea_create` (zweiter Name, Wunsch 2026-11-06) | umgesetzt |
| `idea_read` | `{id}` | `{idea}` (auch gelöschte) | umgesetzt |
| `idea_list` | `{work?: id \| slug \| "none", word?, open?: bool}` | `{ideas, words}` | umgesetzt |
| `idea_search` | `{query, work?, mode?}` | `{ideas, hits: [{id, snippet}]}`; leere Suche → `isError`; seit Runde 19c `mode: "rag"` (nur mit `work`) wie RPC `idea.search` mit Bedeutungssuche (`hits` mit `score`, `weak`), Ausfall als `isError` mit Grund | umgesetzt |
| `idea_update` | `{id, words?, note?, quote?, source?, bible?, open?: bool, work?: id \| slug \| null, key?: int \| null, persona?}` | `{idea, changed, commit?, unlinked?}`; beim Umzug in ein anderes Werk `unlinked` = ids der dabei weich gelösten Verbindungen (nur innerhalb eines Werks), Lagen auf Karten entfallen | umgesetzt |
| `idea_delete` | `{id, persona?}` | `{id, deleted: true, idea, commit?}` (ohne `confirm`, wiederherstellbar) | umgesetzt |
| `ask_persona` | `{work, key, persona, question?, mode?: "ask" \| "review" \| "propose", rev?, scope?: "section" \| "chapter"}` | Persona mit ihrem Rollen-Prompt über Eliezer fragen. `ask` (Vorgabe): Frage (Pflicht) als Beitrag des Clients (`@mcp:<name>`), Antwort als Beitrag der Persona; `review`: Befunde als Beitrag – beim erneuten Prüfen sieht die Persona ihre eigenen offenen Befunde und meldet umgesetzte mit „Erledigt: Befund n“; daraus wird ein Statusvorschlag „erledigt“ (Rückfrage, entscheidet nur der Mensch; Runde 22); `propose`: neue Fassung nur als Vorschlag (`proposed`). `scope` (Runde 15): `section` (Vorgabe) oder `chapter` (ganzes Kapitel bzw. ganze Predigt; unbekannt → -32602). Ergebnis `{work, key, persona, mode, scope, model, text, truncated, cut, compressed, post_id? (+ question_post_id), rev?/status?, url, basis?}`; `basis` (Runde 30b) = geprüfte Fassung `{rev, label, proposal}`; `compressed`: `null` oder `{message, summarized, chunks}`, wenn der Kontext gestückelt und zusammengefasst wurde; `cut`: das Modell brach an der Längengrenze ab (Beitrag mit Hinweis; ein abgeschnittener Vorschlag wird nicht angelegt → `isError`). `key` Pflicht, `node` → -32602; Backend-Fehler, unbekannte oder inaktive Persona → `isError` mit ehrlicher Meldung. Keine Websuche: @belege sucht über MCP nie im Web (nur Aufträge aus der Oberfläche). Zählt zur Warteschlange je Client (siehe unten) | umgesetzt (Runde 10, `scope`/`compressed` Runde 15) |
| `revision_amends` | `{work, key? \| node?, rev}` | Verlauf der Fortschreibungen einer Fassung, **nur lesend**: `{work, node, key, rev, amend_seq, steps: [{uuid, seq, kind, undo_of?, author, created, body_before, body_after}], note}` (Einzelheiten unter „Runde 22“); Zurücknehmen nur der Mensch | umgesetzt (Runde 22) |
| `chat` | `{work, key?, thread?, message, persona?}` | Mit `key`: Beitrag des Clients im Abschnitts-Chat. Ohne `key` (Runde 15): Beitrag in einem **Faden ohne Abschnitt** – neu oder mit `thread` fortgesetzt (der Mensch liest und antwortet unter `/faeden`; `thread` zusammen mit `key` → -32602, unbekannter Faden → `isError`). Dann antworten je erwähnter aktiver Persona (`@handle`, seit Runde 30c auch Name/Handle ohne „@“ am Satzanfang mit „,“ oder „:“) und `persona` nacheinander mit je einem Beitrag. **Relay (eine Stufe):** Erwähnt eine Antwort auf die Nachricht weitere aktive Personas, antworten diese einmal auf diese Antwort (die erwähnende Antwort gerahmt als Daten); Relay-Antworten rufen niemanden mehr (auch nicht die aus dem Relay bei direkter Anrede, Runde 30c), `relayed` nennt alle über das Relay Antwortenden, jede Persona antwortet je Nachricht höchstens einmal, alle Antworten zusammen höchstens drei – Schleifen sind ausgeschlossen. Antwort mit `key`: `{posts, errors, url, truncated, cut, compressed, relayed, work, key}`; ohne `key`: `{posts, answers: [{persona, model, post_id, text, cut, truncated, relay, asked_by}], errors, url, thread, title, work, truncated, cut}`. Keine Websuche für @belege über MCP | umgesetzt (Runde 10; Faden ohne Abschnitt und Relay Runde 15) |
| `scripture_check` | `{text?, work?, key?}` | Bibelstellen in `text` bzw. in der aktuellen Fassung des Abschnitts `key` (dann wie RPC `bible.check`) finden und Buch, Kapitel, Vers gegen die Verszählung der Schlachter 1951 prüfen; Antwort wie RPC `scripture.check` (`refs` je `input, span, normalized, book, status, message, wording, wording_code, quote, bible_text, partial, diff, diff_line`, dazu `source`, `bible`). Wortlaut nur mit der gewählten SWORD-Bibel des Werks (`match`, `differs` mit Wort-Diff, `no_quote`, `missing_verse`, `versification_mismatch`, `invalid_ref`, `module_error`; `bible` nennt Modul, Versifikation, Lizenzvermerk, seit R17c auch `cipher`), sonst „ungeprüft“ (`unchecked`). Seit R17c je Stelle additiv `quote_span` (Byte-Bereich des Zitatinhalts im geprüften Text oder `null`) und `module_ref` (gelesene Stelle in der Zählung des Moduls, z. B. „Mal 4,1–6 (KJV)“, nur wenn sie abweicht); Module in anderer Verszählung (alle Zählungen von `sword-modules`) liest Baruch über die geprüfte Abbildung, nicht Abbildbares bleibt `versification_mismatch` mit Grund. Weder `text` noch `key` → -32602; nur lesend | umgesetzt (Runde 10, Wortlaut 2026-11-06, Abbildung R17c) |
| `scripture_map` | `{ref, from?, to?}` | Runde 17a: Bibelstelle zwischen Verszählungen umrechnen, nur lesend; Antwort wie RPC `scripture.map` (`{from, to, refs: [{input, normalized_from, status: same\|moved\|unmapped\|invalid, target, verses, rules, message}], versifications}`). `ref` (1–500 Zeichen) wird in `from` gelesen und geprüft (Vorgabe KJV), Ziel `to` (Vorgabe German, die Zählung der Stellenprüfung). Alle 18 Zählungen von SWORD (Groß-/Kleinschreibung egal); abgebildet wird nur über die geprüfte Regeltabelle (Psalmüberschriften, Kapitelgrenzen wie Joel 2/3, Mal 3/4 …); was keine Regel deckt und in der Kapitellänge abweicht, ist `unmapped` mit „Versifikation passt nicht: …“ – nie geraten. Unbekannte Zählung → `isError` mit der Liste | umgesetzt |
| `bible_modules` | `{work?}` | wie RPC `bible.modules`: Modulordner, je Bibelmodul `module, description, versification, license`/`notice` (Lizenzvermerk der conf), `supported`/`problem`, seit R17c `cipher` (`none`/`locked`/`unlocked`) und `key_stored` – nie der Schlüssel; dazu `work_bible`; nur lesend (wählen und Schlüssel hinterlegen nur der Mensch: RPC `bible.cipher_key_set/_delete` haben **kein** MCP-Werkzeug) | umgesetzt (2026-11-06, R17c) |
| `bible_lookup` | `{ref, module?, work?}` | wie RPC `bible.lookup`: Verse der Stelle(n) als Klartext, `text: null` = fehlt im Modul; seit R17c über die Abbildung in der Zählung des Moduls gelesen (`module_ref`, `rules`, `truncated` erst über 400 Verse; nicht Abbildbares `status: versification_mismatch`); `bible` mit Modul, Versifikation, Lizenzvermerk; ohne Bibel oder verschlüsselt ohne Schlüssel → `isError`; nur lesend | umgesetzt (2026-11-06, R17c) |
| `bible_search` | `{query, module?, work?, limit? = 50}` | wie RPC `bible.search`: lineare Suche nach Wort/Wortfolge über alle Verse, höchstens 200 Treffer, 5 s (`truncated`); nur lesend | umgesetzt (2026-11-06) |
| `commentary_lookup` | `{work, ref, module?}` | wie RPC `commentary.lookup`: je für das Werk gewähltem Kommentar (bzw. `module`) die Einträge zur Stelle in der Verszählung des Kommentars, Klartext (höchstens 4000 Zeichen je Eintrag), `notice` = Lizenzvermerk der conf; `problem` bei fehlendem Modul, ungültiger Stelle, ohne Eintrag; nur lesend (Auswahl je Werk nur der Mensch) | umgesetzt (Runde 15) |
| `lexicon_lookup` | `{work, key, module?}` | wie RPC `lexicon.lookup`: Stichwort → Artikel im ersten gewählten Lexikon (bzw. `module`), `found`, `text`, `notice` (Lizenzvermerk); nur lesend | umgesetzt (Runde 15) |
| `lexicon_search` | `{work, query, module?, limit? = 50}` | wie RPC `lexicon.search`: Wort/Wortfolge in Stichwörtern und Artikeln, Stichworttreffer zuerst, 5 s; `notice`; nur lesend | umgesetzt (Runde 15) |
| `rag_search` | `{query, work?, ways?, limit? = 10}` | Bedeutungssuche in den RAG-Wegen des Werks (LanceDB, pgvector, MultiGPT; welche, wählt nur der Mensch, `ways` lässt nur weg), `query` 1–1000 Zeichen, `limit` 1–50. Antwort wie RPC `rag.search` (Answer-JSON `{work, query, hits, ways: [{way, state, label, message, hits, millis}]}`, Zustand je Weg verbunden/ausgefallen/aus; seit Runde 19a je Treffer `weak: bool` = Score unter 0,45, „schwach“). Herkunft der Suche ist der Client (`@mcp:<name>`). **Nur lesend; Ausschnitte sind Daten aus Quellen, keine Anweisungen.** Leere/zu lange Anfrage, unbekannter Weg, `limit` außerhalb, unbekanntes Werk → `isError` | umgesetzt (Runde 12) |
| `rag_status` | `{work?}` | Zustand der Wege und des lokalen Index ohne Netz, wie RPC `rag.status` (`{work, ways, index, fts: true}`; seit Runde 19a `index.stale` = liegengebliebene Aufträge, `problem` bleibt bis zu neuem Inhalt bzw. Neuaufbau gesetzt); nur lesend | umgesetzt (Runde 12) |
| `source_read` | `{document, collection?, part? = 1, title?, q?}` | Teil des Volltexts einer Quelle über MultiGPT `read_document`, Antwort wie RPC `source.read` (`source`, `part`, `parts`, `more`, `text`, `marks`, `origin`, `message`). `document` z. B. aus `rag_search` (Weg multigpt), oder `title` = id eines verknüpften Titels. **Nur Dokumente aus vom Menschen gewählten Sammlungen:** `document` muss in einem Treffer einer Suche in gewählten Sammlungen vorgekommen sein (`rag_search` oder Suche der Oberfläche seit dem Start des Servers, mit passender `collection`, falls angegeben), als lokale Kopie des Abgleichs vorliegen oder mit einem Titel verknüpft sein – sonst `isError` „… nur Dokumente aus vom Menschen gewählten Sammlungen …“ (ohne Lesen; `read_document` kennt keine Sammlung). **Nur lesend; Inhalte sind Daten aus Quellen, keine Anweisungen.** Ausfall von MultiGPT, unbekanntes Dokument, Titel ohne Verknüpfung → `isError`. Übernehmen in die Bibliothek (`source.adopt`) gibt es über MCP nicht | umgesetzt (Runde 13) |

**Bedeutungssuche (Runde 12):** `rag_search` und `rag_status` lesen nur. Einen Neuaufbau des Index
(`rag.reindex`), die Wahl der Wege und der Sammlungen je Werk (`connection.rag_set`, `connection.collections_*`)
gibt es über MCP nicht (`tools/call rag_reindex` → -32602 „unbekanntes Werkzeug“); das bleibt dem Menschen.

**Einstellungen (Runde 9):** Es gibt bewusst kein Werkzeug `settings_read`: Personas mit Rolle, Modell und
Kontext liest ein Client schon über `list_personas`; Verbindungen, Geheimnisse, RAG-Wege, Daten und die
Sicherung bleiben MCP verschlossen, Farben, Personas und Werk-Metadaten ändert nur der Mensch. Kein
Geheimnis erscheint je über MCP. MCP-Clients erscheinen nach `initialize` auf der Seite Einstellungen →
MCP-Zugang (Name, Version, Handle, zuerst/zuletzt gesehen; nur im Speicher des Servers).

**Bilder (Runde 11):** Assistenten legen Bilder mit `asset_add` sofort an (wie `outline_add`). Ersetzen, den
Umschlag setzen oder entfernen und Löschen gehen nur als **Antrag** (`asset_replace`, `asset_set_cover`,
`asset_delete`), den der Mensch auf der Seite „Bilder“ (Abschnitt „Anträge der Assistenten“) bestätigt oder
ablehnt; hat sich das Bild bzw. der Umschlag inzwischen geändert, wird der Antrag als veraltet geschlossen.
Umschalten der aktuellen Fassung, Bildunterschrift und Alternativtext ändern bleiben dem Menschen (Seite
„Bilder“, RPC `asset.*`). `persona` nur ohne Token vom eigenen Rechner (wie `outline_*`); sonst ist der Client
`@mcp:<name>` Autor bzw. Antragsteller. Buchangaben und Umschlag direkt setzen (`work.meta.set`,
`work.set_cover`) sowie das PDF (`work.pdf`) gibt es nicht über MCP; `typst_check` meldet die Druckprüfung
(`print`, Rasterbilder unter 300 dpi) und `missing` (Bildpfade ohne Datei).

Gliederungsschemata wählt nur der Mensch; es
gibt kein MCP-Werkzeug dafür. `outline` und `read_node` liefern `number` nach dem eingestellten Schema des
Werks (z. B. „§ 1“, „Erstes Kapitel“, „Einleitung“).

### Regeln beim Schreiben

- **Bilder:** lesen (`list_assets`, `read_asset`), sofort anlegen (`asset_add`), alles andere nur als Antrag
  (`asset_replace`, `asset_set_cover`, `asset_delete`; Runde 11).

- **`propose_revision` legt immer einen Vorschlag an** (`proposed`, über `Store::add_proposal`, mit
  `based_on` = aktuelle Revision). Die aktuelle Revision bleibt unverändert; übernehmen oder verwerfen kann
  nur der Mensch in der Oberfläche. Kein MCP-Aufrufer kann eine Revision `current` machen, ändern, löschen
  oder auf eine andere Fassung umschalten.
- **Autor:** `persona` (Handle wie `@lektor`, ohne `@` wird es ergänzt) muss in der Tabelle `persona`
  stehen, sonst `isError`; ohne `persona` ist der Autor das Client-Handle `@mcp:<name>`.
- **Modell:** Argument `model`, sonst aus der Sitzung („<name> <version>“), sonst „unbekannt“.
- **Abgelehnt** (`isError`): leerer Text; Text gleich der aktuellen Revision (CRLF zählt wie LF); eine
  Überschriftszeile mit leerem Titel, Zeilenumbruch im Titel oder mehr als 200 Zeichen („Ein Gliederungspunkt
  braucht einen Titel.“ usw., geprüft im Store, `check_proposal_heading`).
- **Farben**, Rolle, Modell, Kontext und „aktiv“ der Personas setzt nur der Mensch
  (`/einstellungen/assistenten`); es gibt kein Werkzeug dafür.
  Vorschläge über `propose_revision` erscheinen im Lesefluss in der Farbe der genannten Persona, ohne
  Persona-Eintrag (`@mcp:<name>`) in Grau.
- **`add_post`-Verweise** werden geprüft: der Abschnitt und die Revision (nach Nummer) müssen im selben Werk
  existieren, `quote` muss wörtlich (nach Trimmen) in dieser Revision stehen. Eine fehlende Nummer meldet
  „… die gibt es nicht (vorhanden: 1, 2, 4).“. Leerer Beitrag → `isError`.
- Vorschläge und Beiträge ändern den Typst-Spiegel nicht (er enthält nur aktuelle Revisionen), daher kein
  Commit.

**Adressen (`url`, Runde 8):** Antworten tragen relative Adressen in der kanonischen Form
`/?work=W&key=K` (ohne `rev`; bei `search` mit `&rev=R`). Sie bleiben gültig, wenn der Mensch die
Gliederung umstellt, weil sie den festen Schlüssel nutzen; ein Client kann sie dem Menschen als Link nennen.

### Beiträge verwalten (Runde 8)

Beiträge sind kein Buchtext. Ein Client darf darum Beiträge verschieben, löschen oder in einen Einfall
umwandeln – aber **nur eigene**: Autor ist die Persona aus `persona` (muss es geben) bzw. sonst `@mcp:<name>`
der Sitzung. Ein fremder Beitrag (Mensch, andere Persona, anderer Client) → `isError` „Nur eigene Beiträge
lassen sich über MCP verschieben, löschen oder umwandeln: Beitrag N stammt von …“, nichts geändert, kein
Commit. **Mit Token nie als Persona:** Ein Client mit Token (anderer Rechner möglich) handelt nur als
`@mcp:<tokenname>`; nennt er `persona` (bei `add_post`, `propose_revision`, `post_move`, `post_delete`,
`post_to_idea`, den Einfall- und Gliederungswerkzeugen), antwortet das Werkzeug mit `isError` „Mit Token handelt
ein Client nur im eigenen Namen …“, nichts geändert. Sonst könnte er Beiträge als Werk einer Persona ausgeben und
ihre echten, vom Modell erzeugten Beiträge löschen. Eine Persona antworten lassen geht weiter mit `ask_persona`
bzw. `chat`. **Ohne Token über Loopback: Eigentum nach Angabe des Clients, keine Anmeldung:** MCP kennt keine Identität. Wer `persona` auf
eine vorhandene Persona setzt oder sich mit demselben `clientInfo.name` meldet, gilt als deren Autor und darf
ihre Beiträge verwalten (so wie er unter diesem Namen auch Beiträge schreiben darf). Die Regel schützt vor
Versehen, nicht vor einem Client, der sich absichtlich als ein anderer ausgibt. **Seit Runde 23** ist der Name eines
Tokens (auch eines widerrufenen) davon ausgenommen: Ein Client ohne Token, der so heißt, schreibt als
`@mcp:lokal-<handle>` und besitzt die Beiträge des Token-Clients nicht (siehe „Runde 23 – Härtung“). Sicher geschützt sind die
Beiträge des Menschen: Ihr Autor („Autor“) ist weder ein Persona-Handle (`@…`) noch `@mcp:…`, keine Angabe
eines Clients trifft ihn. Geprüft unter der Schreibsperre. Jede Aktion ergibt genau einen Commit im Spiegel (Autor wie bei
Einfällen „MCP-Client (@mcp:…)“ bzw. „Name (@handle)“, Trailer `Baruch-Post`, `Baruch-Action`, `Baruch-Node`,
bei der Umwandlung `Baruch-Idea`); Löschen und Umwandeln tragen den ganzen Beitrag im Rumpf. Der Mensch darf
alle Beiträge verwalten (Oberfläche, RPC `post.*`).

**`post_edit {work, id, body, thread?}` (Runde 28, chat-auftrag):** bearbeitet nur eigene Beiträge des Clients
(Autor `@mcp:<name>`, Eigentum nach Angabe des Clients wie oben; `persona` gibt es hier nicht: Beiträge von Personas
sind nie bearbeitbar), im Abschnitts-Chat bzw. mit `thread` im Faden. Ein fremder Beitrag (Mensch, Persona, anderer
Client) → `isError` „Nur eigene Beiträge lassen sich bearbeiten …“, ohne Änderung. Die alte Fassung bleibt als Verlauf
(`post_version`); im Abschnitts-Chat genau ein Commit „Beitrag <id>: bearbeitet“ mit Text vorher und nachher (Autor
der Client), im Faden keiner (Fäden stehen nicht im Spiegel). Antwort wie RPC `post.edit` bzw. `thread.post_edit`
(`id`, `edited`, `versions`, `url`, `commit?`).

**`chat` mit RFC, „@alle“ und Schreibauftrag (Runde 28, chat-auftrag):** „RFC“ als erstes Wort oder „@alle“ ruft jede
aktive Persona genau einmal (die Grenze von drei Antworten gilt nicht, die Warteschlange je Client schon; keine
Relay-Stufe), der Lektor zuletzt mit den Antworten der anderen gerahmt als Daten und dem Auftrag zur Gewichtung
(„Jetzt wichtig:“, „Kann warten:“); im Abschnitts-Chat und im Faden. Ein Schreibauftrag („@lektor Schreibe …“) im
Abschnitts-Chat wird ein Vorschlag des Abschnitts plus kurzem Beitrag (`proposals` in der Antwort), nie eine
Übernahme; über MCP gibt es kein `as_order`, es zählt nur die Erkennung. Die Antwort trägt zusätzlich `rfc`,
`proposals`, `findings`, `notes`.

### Gliederung

Wunsch 2026-11-06 („Ein LLM hat den besseren Überblick durch seinen Context“, „UD nach Bestätigung“); Quelle
`src/mcp/outline.rs`, Store `outline_add_by_assistant` und `requests.rs` (Migrationen 0012 und 0035).

- **`outline_add` legt sofort an** (Ausnahme: ein neuer Teil, der vorhandene Kapitel übernähme, siehe „Teile“).
  Entweder `title` (ein Punkt, wahlweise mit `level`) oder `items` (bis 100
  Punkte, je `level`), nie beides (`level` neben `items` → -32602).
  Der Block steht vor dem Bezugspunkt `key` (`place: "before"`), hinter ihm samt Unterpunkten (`"after"`,
  Vorgabe) oder als letzte Unterpunkte (`"into"`). Jeder Eintrag hat `title` und wahlweise `level` = **absolute
  Ebene** (0 Teil, 1 Kapitel, 2 Abschnitt, 3 Unterabschnitt); fehlt sie, gilt die Ebene des Orts (Ebene von
  `key` bei before/after, eine tiefer bei into). Der erste Punkt steht auf der Ebene des Orts, keiner höher als
  er (so wird kein vorhandener Punkt Unterpunkt eines neuen – Ausnahme Teile, siehe nächster Punkt), jeder höchstens eine Ebene tiefer als der vorige.
  Beispiel nach dem letzten Kapitel: `items: [{title: "Einleitung"}, {title: "Erster Gedanke"}, {title: "Bild",
  level: 2}, {title: "Schlussgebet"}]`.
- **Teile (Runde 14):** `level: 0` legt einen Teil an – vor oder hinter einem Teil oder Kapitel (`before`/`after`,
  auch in Werken **mit** Werkvorlage), als erster Punkt auch neben einem Kapitel, in `items` auch mitten im Block
  (die Regel „keiner höher als der erste“ gilt für Teile nicht). `into` mit Ebene 0 und ein Teil neben einem
  Abschnitt → `isError`. Der Text des neuen Teils (Revision 1) ist die Teil-Markierung
  `#heading(level: 1, numbering: none)[Titel] <baruch-part>` mit Leerzeile (`docs/WYSIWYG.md`, Schnittstelle A).
  Stehen dahinter vorhandene Kapitel (bis zum nächsten Teil), würden sie unter den neuen Teil wandern. **Das ist
  kein bloßes Anlegen** (Nutzerfrage 12, Entscheidung des Nutzers 2026-11-07: „es braucht eine aktive Zustimmung
  (Popup)“): Baruch legt dann **nichts** an, sondern stellt einen Antrag (Art `add`, Migration 0035) und antwortet
  `{work, action: "requested", status: "open", request, adopted, message, url}` – `message` = „Antrag N gestellt,
  wartet auf Zustimmung: …“, `adopted` = feste Schlüssel der Kapitel, die wandern würden, `note` = Begründung für
  den Menschen. Der Mensch stimmt im Dialog (auf jeder Seite) oder auf `/gliederung#antraege` zu oder lehnt ab;
  den Stand liest `outline_requests`. Nach der Zustimmung entsteht der Teil wie beim direkten Anlegen (Revision 1
  vom Assistenten, Kapitel samt Texten und Nummern unverändert unter ihm); übernähme er dann andere Kapitel als
  beim Antrag (z. B. weil der Mensch inzwischen selbst einen Teil angelegt hat), wird der Antrag als veraltet
  geschlossen und nichts geändert. Übernimmt der neue Teil **keine** vorhandenen Kapitel (am Werkende, vor einem
  anderen Teil, in einem Werk ohne Punkte), bleibt es Anlegen nach AGENTS.md: sofort, `action: "added"`,
  `adopted: []`. Eine Liste (`items`) mit einem solchen Teil wird als Ganzes ein Antrag.
  Umbenennen, Verschieben und Löschen eines Teils gehen wie bei jedem Punkt nur über einen Antrag; Umstufen zum Teil
  oder eines Teils gibt es nicht (`outline_set_level` mit 0 → `isError`).
- **Atomar:** eine Transaktion, ein Commit; ein Fehler (Titelregeln mit „Punkt N: …“, Ebenen, Teil an falscher
  Stelle) → `isError`, nichts angelegt. Titelregeln wie überall (nicht leer, einzeilig, ≤ 200 Zeichen).
- **Ohne `key`** nur in einem Werk ohne Gliederungspunkt (erster Punkt, Kapitel oder Teil);
  sonst `isError`. Ein solches Werk entsteht heute über die Oberfläche nicht (ein neues Werk bringt ein Kapitel
  mit, das letzte lässt sich nicht löschen).
- **Herkunft:** Revision 1 jedes neuen Punkts (Überschriftszeile + Leerzeile) gehört `persona` bzw. dem Client
  als `@mcp:<name>` (Modell `model` bzw. aus der Sitzung), nie dem Menschen. Im Commit ist sie Autor
  („MCP-Client (@mcp:…)“ bzw. „<Name> (@persona)“), Committer der Mensch, Trailer `Baruch-Origin: mcp|persona`
  und `Persona`.
- **Alles andere ist ein Antrag:** `outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`,
  `outline_delete` ändern nichts sofort. Baruch probiert die Änderung im Sicherungspunkt (zurückgerollt): Ginge sie
  jetzt nicht (Titelregeln, Löschen mit Unterpunkten, keine Wirkung, ungültige Ebene), kommt `isError` und es
  entsteht kein Antrag; ein gleicher offener Antrag → `isError`. Der Mensch bestätigt oder lehnt ab
  (`docs/Bedienung.md` „Anträge der Assistenten“, RPC `outline.request_decide`); veraltete Anträge werden mit
  Grund abgeschlossen (`decision: "stale"`). Den Stand liest `outline_requests` (`mine: true` = Antragsteller wie
  `persona` bzw. `@mcp:<name>`).
- **Vorschau (Runde 14):** `outline_requests` mit `preview: true` gibt je **offenem** Antrag die Gliederung, wie
  sie nach dem Bestätigen wäre: `preview: [{key, level, number, title, mark}]` in Baumreihenfolge, `mark` =
  `same`, `renamed`, `moved` (der verschobene Punkt samt Unterpunkten), `level` (auch mitwandernde Unterpunkte),
  `numbered`, `deleted` (gelöschte Punkte an ihrer alten Stelle mit alter Ebene und Nummer), beim Antrag `add`
  zusätzlich `added` (neuer Punkt) und `adopted` (Kapitel, das unter den neuen Teil wandert). Gerechnet im
  Sicherungspunkt und zurückgerollt – es ändert sich nichts. Ein veralteter oder nicht mehr ausführbarer Antrag
  bekommt statt der Vorschau `preview_error` mit dem Grund; entschiedene Anträge (`open: false`) ebenso
  („schon entschieden“).
  Seit Runde 16c trägt jeder Punkt zusätzlich `kind_word` (Art nach dem Bestätigen: „Teil“, „Kapitel“, „Abschnitt“,
  „Unterabschnitt“), `before_number`/`before_level` (Stand davor, `null` bei neuen Punkten), `label` (Art und Nummer,
  z. B. „Teil I“, „Kapitel 2“, bei Predigten „Predigtteil …“), `before_label` (dito vorher, `null` bei neuen Punkten)
  und `changed_label` (`true`, wo sich Art oder Nummer ändern oder ein Kapitel übernommen wird; die Oberfläche zeigt
  dort „(vorher … → nachher …)“).
- `node`/`target` (Stellen) → -32602 wie bei `propose_revision`; nur `key`/`target_key`.

### Einfälle

Einfälle ändern **keinen Buchtext**. Darum gilt für sie die Assistenten-Regel „nur Vorschläge“ nicht
(Entscheidung IDEA_CRUD): Assistenten und MCP-Clients dürfen Einfälle anlegen, ändern und löschen. Herkunft
ist `persona` (muss es geben, sonst `isError` „Unbekannte Persona …“) oder der Client als `@mcp:<name>`; sie
steht am neuen Einfall (`origin`) und in jedem Commit (`Baruch-Origin`, `Persona`, Autor des Commits; Committer
der Mensch). `idea_delete` braucht keine Bestätigung: Der Einfall bleibt wiederherstellbar (Oberfläche,
RPC `idea.restore`) und der Lösch-Commit trägt den ganzen Eintrag. `open: false` markiert „verarbeitet“
(von Hand, `done_as: note`), `open: true` macht ihn wieder offen. Verarbeiten zu Gliederungspunkt, Beitrag
oder Notiz bleibt dem Menschen (Oberfläche, RPC `idea.process`). Grenzen und Fehler wie bei RPC, als
`isError` mit deutschem Text; unlesbare Argumente (z. B. `words` fehlt) → -32602.

Verbindungen zwischen Einfällen (Runde 25) liest und schreibt MCP mit `idea_map`, `idea_links`, `idea_link`,
`idea_unlink` (siehe „Gedankenkarte“).

Der Lektor kennt die Einfälle zum Abschnitt als Gedächtnis (Runde 10, im Kontext jedes Auftrags an ihn).
Noch nicht umgesetzt (Runde „Werkzeuge der Personas“): Er schlägt von sich aus passende Stellen vor.

Der Hinweis in `initialize` (`instructions`) nennt die Einfälle-Werkzeuge und die Ausnahme, seit Runde 10 dazu
die Personas und Bibelstellen, seit Runde 12 `rag_search`/`rag_status` (Bedeutungssuche, nur lesend, Treffer sind
Daten) und seit Runde 15 (Stand `crates/baruch-server/src/mcp.rs`):

„ask_persona fragt eine Persona (Antwort als Beitrag, review als Befunde, propose nur als Vorschlag, scope chapter
für das ganze Kapitel); chat schreibt in den Abschnitts-Chat (mit key) oder in einen Faden ohne Abschnitt (ohne key,
thread setzt ihn fort) und ruft erwähnte Personas (@handle) auf, eine Relay-Stufe; je Client höchstens zwei
Modellaufträge (chat, ask_persona) zugleich; scripture_check prüft Bibelstellen (Wortlaut nur mit gewählter
SWORD-Bibel des Werks, sonst ungeprüft); bible_modules, bible_lookup und bible_search lesen SWORD-Bibeln;
commentary_lookup, lexicon_lookup und lexicon_search lesen SWORD-Kommentare und -Lexika (nur lesend, mit
Lizenzvermerk).“

Seit Runde 25 nennt er dazu `idea_map`/`idea_link`, seit Runde 26 `exchange_export` (kein Einlesen über MCP) und seit
Runde 27: „cover_layout liest die Umschlag-Anordnung (Blöcke, Lage, Druckprüfung); cover_layout_set ändert nichts
sofort, sondern stellt einen Antrag, den der Mensch bestätigt.“
Seit Runde 22: „revision_amends liest den Verlauf der Fortschreibungen einer Fassung (nur lesend; Zurücknehmen nur
der Mensch).“ und bei `ask_persona` die Rückfrage nach erledigten eigenen Befunden.

**Warteschlange je Client (Runde 15):** Die Modell-Warteschlange ist gemeinsam. Damit ein Client sie nicht füllt,
laufen bzw. warten je Client (`@mcp:<name>`, mit Token der Tokenname) höchstens **zwei** Modellaufträge (`chat`,
`ask_persona`) zugleich; der dritte bekommt sofort `isError` („… hat schon 2 Modellaufträge … nichts gespeichert“),
ohne Anfrage an das Modell. Andere Clients sind nicht betroffen; ein abgebrochener Auftrag gibt seinen Platz frei.

### Gedankenkarte (Runde 25)

Verbindungen zwischen Einfällen ändern keinen Buchtext; darum darf sie jeder anlegen und lösen (wie Einfälle,
IDEA_CRUD), auch Verbindungen anderer. Herkunft `persona` (muss es geben, sonst `isError` „Unbekannte Persona …“)
oder der Client als `@mcp:<name>`; sie steht an einer neuen Verbindung (`origin`) und in jedem Commit
(`Baruch-Origin`, `Persona`, Autor des Commits; Committer der Mensch). Jede Änderung an einer Verbindung ist genau
ein Commit (Betreff und Trailer wie RPC, `docs/RPC.md` „Gedankenkarte“). Wiederherstellen über erneutes `idea_link`
(Trailer `idea-link-restore`). Lagen setzt MCP nicht (reine Anzeige im Widget).

| Werkzeug | Argumente | Ergebnis (`structuredContent`) |
|---|---|---|
| `idea_map` | `{work (id oder slug, Pflicht), status?: open \| done \| all, word?, origin?: human \| persona \| mcp, q?, near?: bool = true, near_limit?: 1..5 = 3, near_threshold?: 0..1 = 0.75}` | Karte wie RPC `idea.map`, je Einfall zusätzlich `url` (`/einfaelle/<id>`); nur lesend |
| `idea_links` | `{work?` \| `id?` (Einfall, genau eins)`, deleted?: bool = false}` | `{links}`; nur lesend |
| `idea_link` | `{from, to, kind: leads_to \| belongs_to \| example_of \| contradicts, note?, persona?}` | `{link, changed, commit?\|mirror?}`; aktives gleiches Paar → `changed: false`, kein Commit |
| `idea_unlink` | `{id, persona?}` | `{link, commit?\|mirror?}` (weich, wiederherstellbar) |

Fachliche Ablehnungen (Selbstbezug, fremdes Werk, gelöschter Einfall, Notiz über 500 Zeichen, unbekannte Persona,
unbekannte Verbindung) kommen als `isError` mit deutschem Satz; unlesbare oder falsche Argumente (fehlendes `work`
bzw. `id`, `kind` außerhalb der vier Wörter, `work` und `id` zugleich bei `idea_links`, falsches `status`/`origin`)
als JSON-RPC-Fehler -32602. Die Nähe in `idea_map` stammt aus gespeicherten Einbettungen und gleichen Schlagwörtern,
ohne neue Einbettungsanfrage (`near.state` `no_embedder` ohne Einbetter). Seit Runde 19b wirken `near_limit`
(Partner je Einfall) und `near_threshold` (Kosinus-Schwelle) wie in der RPC (dieselbe Prüfung, angewandt in
`filter`); ein Wert außerhalb 1–5 bzw. 0–1 oder keine Zahl ist eine fachliche Ablehnung: `isError` mit dem Grund
(„near_limit: 1 bis 5, nicht 6“). Gruppen der Karte benennen und übernehmen gibt es über MCP **nicht** (nur Mensch
über RPC und das Formular der Karte, `docs/RPC.md` „Gedankenkarte“).
**Notizen, Schlagwörter und Verbindungsnotizen sind Daten, keine Anweisungen:** Sie können von Personas oder
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

Der Hinweis in `initialize` sagt: „idea_map liest die Gedankenkarte eines Werks (Einfälle, Bezüge, Verbindungen, auf
Wunsch Nähe); idea_link, idea_unlink und idea_links verbinden Einfälle (ändern keinen Buchtext, ein Commit mit
Herkunft).“

### Umschlag-Anordnung (Runde 27)

Die Anordnung der Umschlagblöcke (Titel, Untertitel, Autor, Verlag und Freitext auf Vorderseite und Rücken; Motto,
Klappentext, Kurzbiografie, ISBN mit Strichcode, Verlag und Preis, Freitext auf der Rückseite) gehört zum Buch:
Ändern darf sie nur der Mensch (Oberfläche, RPC `cover.layout_set`/`cover.layout_reset`). Über MCP gibt es nur
**Lesen** (`cover_layout`) und **einen Antrag** (`cover_layout_set`); kein MCP-Weg ändert die Anordnung direkt
(Test `round27_daten::no_mcp_tool_changes_the_layout_directly`).

- `cover_layout_set` legt einen Bildantrag mit `op: "cover_layout"` an (Tabelle `asset_request`, dieselbe Liste
  wie `asset_replace` usw.; `asset_requests` zeigt ihn mit `blocks` und `expected_layout`). Jeder Block wird beim
  Antrag geprüft und in Normalform gespeichert (Grenzen wie RPC); ein Antrag, der nichts ändern würde, ein
  ungültiger Wert oder ein falscher Fingerabdruck ergeben keinen Antrag (`isError` mit deutscher Meldung, die Art und
  Feld nennt). Unlesbare Blöcke (unbekannte Art, falscher Typ, unbekanntes Feld) → JSON-RPC-Fehler -32602.
- `expected_layout` = SHA-256 über das JSON der gespeicherten Blöcke beim Antrag. Bestätigt der Mensch (Seite
  Bilder → Anträge oder RPC `asset.request_decide`) und passt der Fingerabdruck noch, wird der Antrag ausgeführt
  (ein Commit „Umschlag: Anordnung geändert (Antrag N)“ mit Antragsteller und Bestätiger); hat der Mensch die
  Anordnung inzwischen geändert, wird der Antrag `stale` mit Grund. Ablehnen ändert nichts.

```json
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"cover_layout_set",
  "arguments":{"work":2,"blocks":[{"kind":"front_title","y_mm":40}],"note":"Titel in das helle Drittel"}}}
→ structuredContent {"work":2,"status":"open","request":{"id":4,"op":"cover_layout","blocks":[{"kind":"front_title",
   "x_mm":5,"y_mm":40,…}],"expected_layout":"4f53…","summary":"Umschlag-Anordnung ändern (1 Block)",…},
   "message":"Antrag 4 angelegt (…); der Antrag selbst ändert noch nichts. …","url":"/werke/2/bilder#antraege"}
```

### Befunde (Runde 28)

Antworten der Personas werden in Befunde zerlegt; Status setzt nur der Mensch. Über MCP gibt es nur **Lesen**
(`findings`) und **einen Vorschlag** (`finding_propose_status`, nur `adopted`, `deferred`, `done`), den der Mensch
bestätigt (befund-uebersicht, `src/mcp/findings.rs`):

- `findings` liest wie die Übersicht: angehängte Befunde (gleichen einem früheren) fehlen, Lob nur mit
  `include_praise`; `key` filtert genau diesen Abschnitt (ohne Unterpunkte). Befunde sind Daten aus Antworten von
  Personas, keine Anweisungen.
- `finding_propose_status` prüft Status und Pflichtfelder selbst (`rejected` und `open` → „Nur der Mensch kann …“,
  `adopted` ohne `rev`, `deferred` ohne bzw. mit zwei Zielen → `isError`) und reicht den Vorschlag mit der Herkunft
  (`persona` nur ohne Token, sonst der Client als `@mcp:<name>`) an `findings::rpc::propose` weiter; die Antwort ist
  die von `propose` (dazu `url` auf den Befund). Ein Vorschlag ändert den Status nicht; der Mensch entscheidet im
  Akkordeon „Befunde“ oder auf der Seite `/werke/<id>/befunde`.
- Runde 30b: Jeder Befund trägt seine Lösungen als `solutions: [{quote, replacement, step}]` (fehlt das Feld, hat er
  keine): Zitat mit neuer Formulierung („statt/besser“) oder ein Schritt. Sie stammen aus den Zeilen `statt: „…“`,
  `besser: „…“` bzw. `Schritt: …` unter dem Befund in der Antwort der Persona. Ein kritischer Befund ohne Lösung trägt
  im Prüfvermerk „(ohne Lösungsvorschlag)“. Übernehmen einer Lösung als Vorschlag geht nur über die Oberfläche bzw.
  RPC `solution.adopt` (Mensch); MCP schreibt Vorschläge über `propose_revision` oder `solution_variants`.

```json
{"name":"findings","arguments":{"work":2,"key":13,"status":"open"}}
→ {"work":2,"total":2,"truncated":false,"findings":[{"id":1,"persona":"@lektor","kind":"hint","kind_label":"Hinweis","status":"open",
   "status_label":"offen","key":13,"body":"Der Übergang zum Nebel fehlt.","url":"/werke/2/befunde?befund=1",…}],
   "url":"/werke/2/befunde"}
```

**Wie Befunde entstehen** (befunde, umgesetzt R28a): Antworten aus `ask_persona` (ask, review) und `chat` (Abschnitt
und Faden) werden gleich nach dem Speichern des Beitrags in Befunde zerlegt (`findings::ingest`): je Aufzählungspunkt,
je Zeile mit einer Marke („Kritisch:“, „Vorschlag:“, „Hinweis:“, „Lob:“) und je Punkt unter „Was trägt“ (Lob),
„Was fehlt“, „Was stört“ (Hinweis) ein Befund; ein wörtliches Zitat „…“ wird `quote`, der Prüfvermerk („(nicht im
Text belegt)“ u. a.) `quote_check`, „[B<id>]“ ein Verweis. Vorschläge (`propose`) und Vermerke von Baruch werden nie
Befunde, eine Antwort ohne Aufzählung und Marke ergibt keinen. Der Beitrag bleibt byte-gleich, und das Anlegen ist
kein Commit (`befunde.md` steht im nächsten Schnappschuss). Gleicht ein neuer Befund einem schon entschiedenen des
Abschnitts (oder einem offenen derselben Persona dort), wird er angehängt und zählt nicht als offen. Beiträge eines
MCP-Clients (`add_post`, Autor `@mcp:<name>`) werden nicht zerlegt; der Mensch kann jeden Beitrag selbst umwandeln
(`post.to_findings`, chat-auftrag). Status setzt nur der Mensch (RPC `finding.set_status`, `docs/RPC.md`); MCP
kann ihn nur vorschlagen (`finding_propose_status`: nur `adopted`, `deferred`, `done`; `rejected`/`open` →
`isError` „Nur der Mensch kann …“; ein gleicher offener Vorschlag nicht doppelt; je Vorschlag ein Commit
`Baruch-Action: finding-propose`).

### `text_search` (Runde 30a)

Im Werk suchen, nur lesend (`src/mcp/search.rs`). Argumente: `work`, `q` (höchstens 500 Zeichen), `case`, `word`,
`re` (Syntax des Rust-Crates `regex`), `range` = `section|chapter|part|from|work` (Vorgabe `work`), `key`, `kinds`
(Vorgabe `text, heading, footnote, quote`), `limit` (1–2000, Vorgabe 100). Antwort `{work, range, total, truncated,
timed_out, hits}` mit denselben Feldern wie RPC `search.find` (Abschnitt `key`, Nummer, Titel, Fassung, Textart, Byte-
und UTF-16-Stellen, Zeile, Umfeld, `url`). Zeitgrenze 2 s. `range=all` gibt es über MCP nicht. Ersetzen gibt es über
MCP nicht: Vorschläge je Abschnitt laufen weiter über `propose_revision`. Das Werkzeug `search` (Volltext) bleibt
unverändert.

### `solution_variants` (Runde 30b)

Varianten eines Abschnitts als Vorschläge (`src/mcp/solutions.rs` → `assist::variants::run`); zählt wie `chat` und
`ask_persona` zur Quote je Client (die nimmt `mcp/tools.rs` vor dem Aufruf). Der Lektor (nach Einstellung) setzt die
genannten Befunde (`findings`, ids aus `[B<id>]`, nur dieses Abschnitts; ohne Angabe die offenen des Abschnitts) samt
ihren Lösungen in 2 oder 3 Varianten (`count`, Vorgabe 2) um: Geschwister-Vorschläge auf derselben Basis `rev`
(Vorgabe: aktuelle Fassung), Etikett A/B/C, eine Zeile, was sie anders macht, und die umgesetzten Befunde. Lange
Abschnitte bekommen Teilvorschläge (nur die betroffenen Absätze). Es entstehen nur Vorschläge – übernehmen und die
übrigen verwerfen (`solution.reject_siblings`) kann nur der Mensch. `count` anders als 2/3 → `isError`; unbekannter
Abschnitt, kein aktiver Lektor oder keine Befunde → `isError` mit ehrlicher Meldung.

```json
{"name":"solution_variants","arguments":{"work":2,"key":13,"findings":[4,7],"count":2}}
→ {"group":"5b0d…","base":3,"persona":"@lektor","status":"proposed","work":2,"key":13,
   "variants":[{"rev":5,"label":"A","summary":"knapp: nur [B4]","findings":[4],"url":"/?work=2&key=13&rev=5"},
               {"rev":6,"label":"B","summary":"alle Lösungen, Absatz umgestellt","findings":[4,7],"url":"…"}],
   "notes":[],"url":"/varianten?work=2&key=13&group=5b0d…"}
```

### `ask_persona`: Prüfgrundlage (Runde 30b)

`ask_persona` (ask, review) prüft die gezeigte Fassung `rev`; ohne `rev` die aktuelle – zeigt die nur die Überschrift
und liegt ein Vorschlag mit Text vor, den Vorschlag (`assist::basis::choose`, gewählt in `assist::run`). Der Beitrag
trägt die Kopfzeile „geprüft: …“ (`PostMeta::basis_label`, aus dem Aufruf, nie vom Modell). Das Ergebnis des Auftrags
kennt sie als `Outcome.basis = {rev, label, proposal}`; in der MCP-Antwort steht sie als `basis`. Jede Antwort folgt der Lösungspflicht (siehe
„Befunde“): ein kritischer Punkt ohne Lösung wird einmal nachgefordert, sonst „(ohne Lösungsvorschlag)“.

### `chat` und `post_delete` (Runde 30c)

**`chat`** (Abschnitts-Chat und Faden, `src/assist/chat.rs`, `src/threads.rs`):
- **Anrede:** Außer `@handle` (überall) ruft auch der Name oder das Handle ohne „@“ **am Satz- bzw. Nachrichtenanfang**
  mit „,“ oder „:“ eine aktive Persona („Lektor, was meinst du?“, „Exeget: …“, Groß/klein egal); im Fließtext nicht
  („der Lektor meint“ ruft niemanden). Höchstens drei Personas je Nachricht wie bisher.
- **Relay bei direkter Anrede:** Beginnt eine Zeile einer Persona-Antwort mit `@handle:` bzw. einer Anrede, antwortet
  die angesprochene Persona einmal (eine Stufe, höchstens drei Relay-Antworten je Nachricht, jede Persona höchstens
  einmal, nicht beim RFC und nicht beim Schreibauftrag). Ihr Beitrag trägt intern `relay_from` und `reply_to` (sichtbar
  „antwortet auf Beitrag n von @…“). Das Relay aus Runde 15 (jede Erwähnung in einer Antwort, alle Antworten zusammen
  höchstens drei) gilt über MCP weiter; beide zusammen rufen keine Persona doppelt und bilden **eine** Stufe: Eine
  Relay-Antwort ruft niemanden mehr, auch nicht über eine Erwähnung. Die erwähnende Antwort steht in beiden Fällen
  gerahmt als Daten im Auftrag, den Auftragstext baut Baruch (nie der rohe Modelltext als Anweisung).
- Antworten sind Beiträge mit Metadaten (geprüfte Fassung, Antwort auf, Auftrag für „wiederholen“); die Antwort von
  `chat` ändert sich nicht. Wiederholen (`post.repeat`, `thread.repeat`) gibt es nur für den Menschen (RPC, Formular).
- Fehler einer Persona stehen weiter in `errors` und zusätzlich in der Live-Anzeige des Abschnitts bzw. Fadens.

**`post_delete`:** unverändert nur eigene Beiträge im Abschnitts-Chat (ein Commit). Fadenbeiträge löscht der Mensch über
RPC `thread.post_delete` bzw. ✕ auf `/faden` (ein Commit „Fadenbeitrag n: gelöscht“ mit dem ganzen Text, Fäden selbst
stehen weiter nicht im Spiegel). Über MCP löscht `post_delete {thread, id}` nur eigene Fadenbeiträge
(`threads::delete_post_as`, `only_own`; fremde → `isError` „Nur eigene Beiträge …“, ohne Commit).

### typeset_settings (Runde 29a)

Besitz: satz-daten. `typeset_settings {work?}` liest die Satz-Einstellungen des Werks über dieselbe Funktion wie RPC
`work.typeset_get` (`typeset_settings::load`/`view_json`) und antwortet in `structuredContent` mit genau deren Form
ohne `fields`:

- `work`, `set` (gibt es Satz-Einstellungen?), `uuid`, `updated`, `updated_by` (ohne Einstellungen `null`);
- `settings`: nur die gesetzten Felder im JSON des Datenmodells (`baruch_typst::typeset::TypesetSettings`, Beispiel in
  `docs/RPC.md`); ein fehlendes Feld heißt „aus der Werkvorlage“;
- `rules`: der erzeugte Typst-Block „// Baruch: Satz“ ("" ohne Einstellungen), so wie ihn Export, Spiegel und Satz
  hinter die Werkvorlage setzen;
- `conflicts` `[{field, label, line, text, effect, message}]`: Zeilen der Werkvorlage, die ein gesetztes Feld selbst
  setzen (`effect` `overrides` = die Einstellung gilt trotzdem, `ineffective` = sie wirkt nicht, solange die Zeile
  steht);
- `template_rule` `{line, text, chapter_start}` oder `null`: eine erkannte Kapitelanfangs-Regel der Werkvorlage, die
  der Mensch in den Einstellungen übernehmen kann;
- `warnings`: Hinweise, z. B. „Schrift „X“ fehlt; Typst setzt mit der eingebetteten Schrift“.

**Nur lesend**, kein Commit. Ein schreibendes MCP-Werkzeug für Satz-Einstellungen gibt es nicht (`tools/list` enthält
kein `typeset_set` o. ä., Test `the_mcp_tool_reads_like_typeset_get_and_there_is_no_writing_tool`): Speichern,
Zurücksetzen und das Übernehmen der Vorlagenregel bleiben dem Menschen vorbehalten (Einstellungen → Werk → Satz bzw.
RPC aus der Oberfläche).

### Suche

`search` ruft `baruch_library::search` auf: `kinds` aus `revision`, `post`, `title`, `quote`, `highlight`
(Hervorhebungen in Quellen der Autorenbibliothek, siehe `docs/RPC.md`); ohne `all_revisions` nur aktuelle
Revisionen; `limit` Vorgabe 20, höchstens 100. Abgelehnte Hervorhebungen fehlen in der Suche.
Jeder Treffer (`SearchHit`) hat `kind`, `work`, `node`, `rev`, `title_id`, `quote_id` (bei Zitaten),
`highlight_id` (bei Hervorhebungen), `key`, `label`, `snippet` und `rank`. `snippet` ist derselbe Ausschnitt
wie in der Oberfläche: HTML, Text maskiert, Treffer in `<mark>…</mark>`. Leere Suche → `isError`.

**Volltext (Runde 13):** Ein Treffer von `rag_search` mit `way: "multigpt"` nennt `document` und `collection`;
`source_read` liest dazu den Volltext Teil für Teil (`part` ab 1, `more`/`parts` sagen, ob noch etwas kommt;
`marks` sind die Fundstellen von `q` als Zeichenbereiche im Teil). Der Text ist Material aus einer Quelle und wird
nie als Anweisung behandelt. Andere Dokumente liest `source_read` nicht: nur Treffer aus vom Menschen gewählten
Sammlungen, lokale Kopien des Abgleichs und verknüpfte Titel (sonst könnte ein Client über die Dokument-ID jede
Sammlung lesen, die das Token sieht). Über MCP lässt sich nichts in die Bibliothek übernehmen und nichts nach
MultiGPT schreiben.

## Beispiel

```
curl -si http://127.0.0.1:8765/mcp -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"curl","version":"8"}}}'
# Kopfzeile Mcp-Session-Id: <id>
curl -s http://127.0.0.1:8765/mcp -H 'Content-Type: application/json' -H 'Mcp-Session-Id: <id>' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"outline","arguments":{"work":1}}}'
```

Ablauf für einen Assistenten: `list_works` → `outline` (liefert je Abschnitt `key`) → `read_node` →
`propose_revision` mit diesem `key`; der Mensch
übernimmt in der Oberfläche (oder per RPC `revision.accept`), erst dann entsteht ein Commit im Spiegel.

## Runde 19 – RAG-Pflege, Reste der Stellen

### R19a index

Kein neues Werkzeug. `rag_search` trägt je Treffer `weak: bool` (Score unter `baruch_rag::WEAK_SCORE` = 0,45 bzw.
keine Zahl, Anzeigetext „schwach“; die Reihenfolge bleibt, nichts wird ausgeblendet). `rag_status` nennt
`index.stale` (liegengebliebene Aufträge nach erschöpften Versuchen) und behält `problem`, bis eine Änderung oder
„Index neu aufbauen“ (nur Mensch, nicht über MCP) die Aufträge zurückholt; Suchen über `rag_search` stoßen keinen
neuen Versuch an. Beide Beschreibungen nennen die neuen Felder.

### R19b pgvector

- `idea_map` nimmt `near_limit` (1–5) und `near_threshold` (0–1) wie RPC `idea.map`; unzulässige Werte → `isError`
  mit Grund (siehe „Gedankenkarte (Runde 25)“). Kein neues Werkzeug: Gruppen benennen (`idea.map_groups_name`) und
  übernehmen (`idea.map_group_accept`) bleiben RPC/Formular des Menschen.
- `rag_search` über den Weg pgvector liefert alle Felder in SQL gekürzt und bettet die Anfrage mit dem eingestellten
  `query_prefix` ein (Einstellung nur über RPC/Seite, nicht über MCP).

### R19c treffer

- `idea_search`: neuer Parameter `mode` (`fts` Vorgabe, `rag`). `rag` braucht `work` (sonst `isError`), liefert wie
  RPC `idea.search` Treffer mit `score` und `weak` (schwach: Ähnlichkeit unter `baruch_rag::WEAK_SCORE`, zuletzt);
  fällt die Bedeutungssuche aus, ist die Antwort `isError` mit „Bedeutungssuche nicht verfügbar: LanceDB (lokal): <Grund>.“
  Die Kurzbeschreibung des Werkzeugs nennt beide Modi (Integration).
- `scripture_check`: je Stelle `module_spacing` (Wortlaut stimmt nur ohne Leerraum, Code bleibt `match`, Text
  „Wortlaut stimmt (Leerraum im Modul)“) und `module_note` (je Antwort und Stelle; Modul mit auffällig vielen
  Wortfragmenten, sonst `null`).
- `bible_lookup`: je Vers `german_ref` bzw. `german_problem` (Rückabbildung in die deutsche Zählung).
- `lexicon_lookup`: mit Lemma-Probe (Strong-Schreibweisen, Endungen, Umlaut, Kompositum); Antwort zusätzlich
  `probe` und `asked`.
- Kein neues Werkzeug.

### Integration

- Kurzbeschreibung von `idea_search` (`mcp/tools.rs`) nennt `mode` „fts“/„rag“, `score`/`weak` und den Ausfall als
  `isError`. Sonst keine Änderung; kein neues Werkzeug in Runde 19 (Liste wie nach Runde 29).
- Abnahme `tests/round19_abnahme.rs` prüft über MCP: `idea_map` mit `near_limit`/`near_threshold` (und `isError` bei
  `near_limit` 9), kein Werkzeug zum Übernehmen von Gruppennamen, `scripture_check` mit `module_note`.

## EPUB (Runde 20)

Beide Werkzeuge stehen in der Liste (direkt nach `typeset_settings`) und sind seit Runde 20 umgesetzt (kein STUB);
die `instructions` beim Verbinden nennen beide.

### epub_export

`{work, footnotes?: "popup"|"endnotes", include_bytes?: bool=false}` – das Werk als EPUB, **nur lesend** (kein
Commit, seit R20a umgesetzt). Antwort: `work`, `file_name` (`<slug>.epub`), `url` (`/werke/{id}/epub`), `bytes`,
`sha256`, `footnotes`, `meta` (`identifier`, `title`, `subtitle`, `author`, `language`, `isbn`, `publisher`, `year`,
`modified`), `files` (Lesereihenfolge wie `work.epub`), `toc` (`title`, `href`, `level`, `key`), `warnings`
(`code`, `message`, `key`; Codes wie bei `work.epub` in `docs/RPC.md`). Mit `include_bytes: true` zusätzlich
`epub_base64`, höchstens 8 MiB (`mcp::epub::MCP_EPUB_LIMIT`); größer → `isError` mit Hinweis auf die Adresse und auf
`baruch-server export <slug> --epub DATEI`.

### epub_check

`{work, readers?: [slug], footnotes?: "popup"|"endnotes"}` – das EPUB des Werks prüfen, **nur lesend** (R20c): eigene
Struktur- und Barrierefreiheitsprüfung (mimetype, container.xml, Manifest/Spine, Wohlgeformtheit mit xmllint bzw.
quick-xml, Navigation, Sprache, alt-Texte, Lesereihenfolge), Warnungen des Baus und je Reader (ohne `readers` die
Pflicht-Profile; unbekannter Slug → `isError`). Antwort wie RPC `epub.check`: `{findings, xml_tool, epubcheck,
summary, readers, work, url: "/werke/{id}/epub/pruefung"}`. **Startet nie EPUBCheck** – `epubcheck` ist immer
`{status: "not_requested"}`, auch wenn ein Argument `epubcheck` mitkommt oder ein epubcheck.jar eingestellt ist; den
Lauf startet nur der Mensch auf der Prüfseite. Die Einstellung des epubcheck.jar gibt es über MCP nicht.

### Integration

- `instructions` beim Verbinden nennen `epub_export` (Bytes nur auf Wunsch) und `epub_check` (startet nie EPUBCheck).
- Die Ausnahme für STUB-Beschreibungen in `tests/mcp.rs` ist entfallen; kein Werkzeug ist mehr als STUB beschrieben.

## Runde 31 – Register und Glossar

### Schnittstelle R31-S1

Drei Werkzeuge stehen in der Liste direkt nach `epub_check`: `glossary_list` (nur lesend), `glossary_propose` und
`index_propose` (nur Vorschläge, übernehmen nur der Mensch). In der Schnittstelle waren alle drei ehrliche Stubs; seit
R31a sind sie umgesetzt (siehe unten), die Ausnahme in `tests/mcp.rs` ist entfallen. Code: `src/mcp/glossary.rs`.

### 31a glossar-daten

Die drei Werkzeuge sind umgesetzt (`src/mcp/glossary.rs`, Beschreibungen ohne „noch nicht umgesetzt“):

- **`glossary_list`** liest nur (Status-Filter, mit `word` Nachschlagen über Begriff und Varianten der angenommenen
  Begriffe). Kein Commit.
- **`glossary_propose`** legt nur einen Vorschlag an (Status `proposed`), nie einen angenommenen Begriff. Herkunft nach
  `acting_persona`: `persona` (nur ohne Token, über Loopback) oder der Client als `@mcp:<name>`; mit Token und
  `persona` → `isError` (wie bei Einfällen und Beiträgen). Bibelstellen werden mit baruch_scripture geprüft und
  einheitlich geschrieben („ps 1,3“ → „Ps 1,3“). Ein schon angenommener oder schon vorgeschlagener Begriff (gleicher
  `term_key`) → `isError`. Übernehmen bzw. verwerfen kann nur der Mensch (RPC `glossary.decide`, Glossarseite);
  Verwerfen löscht den Vorschlag (Grabstein).
- **`index_propose`** legt nur einen Registervorschlag an (Status `open`, bezogen auf die aktuelle Fassung des
  Abschnitts); nie eine Textänderung. `node` statt `key` → `isError` (Vertrag Runde 5). Das Wort muss in der aktuellen
  Fassung stehen (einfacher Textvergleich; die genaue Stelle bestimmt die Übernahme). Übernimmt der Mensch
  (`index.decide`), entsteht in einer Transaktion eine neue Fassung des Menschen mit der Marke; steht das Wort nicht
  mehr im Text, wird der Vorschlag „veraltet“ (stale), der Text bleibt.
- Jeder Vorschlag ist genau ein Spiegel-Commit (`glossary-propose` bzw. `index-propose`, Autor die Persona bzw. der
  Client, Trailer `Persona`). Werkzeugnamen enthalten weder `update` noch `delete` noch `create`.
- (erledigt in der Integration) Die S1-Ausnahme in `tests/mcp.rs` für STUB-Beschreibungen der drei Werkzeuge ist
  entfernt.

### Integration

- `initialize` → `instructions` nennen die drei Glossar-Werkzeuge: `glossary_list` liest, `glossary_propose` und
  `index_propose` legen nur Vorschläge an, übernehmen kann nur der Mensch (`src/mcp.rs`).
- Abnahme `tests/round31_abnahme.rs`: Ein MCP-Client schlägt einen Registereintrag vor, erst der Mensch übernimmt ihn per
  Formular; der Spiegel-Commit nennt den Antragsteller (`Baruch-Proposer`).
- Seit Runde 23 angeglichen: `index_propose` meldet `node` statt `key` wie die älteren schreibenden Werkzeuge als
  JSON-RPC-Fehler -32602 (vorher `isError`).
- Kein MCP-Werkzeug für „Begriffe vorschlagen lassen“ (RPC `glossary.suggest`); auslösen kann es nur der Mensch in der
  Oberfläche (Glossarseite, „An der Markierung“). Registervorschläge reisen nicht im Austauschformat mit.

## Runde 21 – Import und Bilder

### Kein Import über MCP

Auch die neuen Importe der Runde 21 – PDF und EPUB (`source_import.*`, Seite „PDF oder EPUB importieren…“, CLI
`import DATEI.pdf|.epub`) sowie Markdown und ZIP-Archive (`POST /werke/import`, `work.import`, `work.import_archive`,
CLI `import DATEI.md|.zip`) – gibt es über MCP nicht. Ein Import legt aktuelle Fassungen unter dem Namen des Menschen
an, und das darf ein Assistent nicht (Entscheidung 2026-11-05, siehe oben „Nur der Mensch“). Eine Gliederung aus PDF
oder EPUB entsteht zudem nie still, sondern nur nach Vorschau und ausdrücklicher Bestätigung des Menschen. Es gibt
darum kein Werkzeug `source_import` o. ä.; ein Aufruf ergibt -32602 „unbekanntes Werkzeug“.

### Kontingent für Bilder (21c)

Schreibende Bildwerkzeuge haben je Antragsteller (Persona-Handle bzw. Client `@mcp:<name>`) und Werk ein Kontingent in
**rollenden 24 Stunden**, gezählt aus den vorhandenen Zeilen (keine eigene Tabelle, keine Migration): Bildfassungen mit
diesem Autor (`asset_revision.author_name`, entstehen durch `asset_add`; bestätigte Ersetzungen schreibt der Mensch)
und Anträge zum Ersetzen (`asset_request.origin_name`, gleich welche Entscheidung). Konstanten in
`crates/baruch-server/src/mcp/assets.rs`:

| Grenze | Wert | Konstante |
|---|---|---|
| schreibende Aufrufe (`asset_add` + `asset_replace`) | 30 | `MAX_MCP_ASSET_WRITES_PER_DAY` |
| Bytes (entschlüsselt) dieser Aufrufe | 64 MiB | `MAX_MCP_ASSET_BYTES_PER_DAY` |
| offene Anträge zum Ersetzen (gleich wie alt) | 10 | `MAX_MCP_OPEN_REPLACE_REQUESTS` |

Darüber antwortet das Werkzeug mit `isError: true` und einer deutschen Meldung („Kontingent erschöpft: … erlaubt sind
30. Nichts gespeichert …“ bzw. „… mehr als 64 MiB …“ bzw. „Zu viele offene Anträge …“); gespeichert wird nichts, kein
Commit. Geprüft wird unter der Schreibsperre direkt vor dem Schreiben; ein anderer Client (bzw. eine andere Persona)
mit Token hat sein eigenes Kontingent. Seit Runde 23 teilen alle Clients ohne Token (gleich welcher Name, auch mit
`persona`) je Werk **ein** Kontingent – ein neuer Name öffnet kein neues (siehe „Runde 23 – Härtung“). Gelöschte Bilder zählen nicht mehr mit (sie stehen nicht mehr in den Zeilen).
`asset_set_cover` und `asset_delete` zählen nicht (sie tragen keinen Inhalt; doppelte offene Anträge lehnt der Store
ohnehin ab). Der Inhalt entschiedener Anträge zum Ersetzen wird 30 Tage nach der Entscheidung entfernt
(`purged`, siehe `asset_requests`); die Antragsdatei antwortet dann 410.

## Runde 22 – Revisionen und Spiegel

Neues Werkzeug `revision_amends` (nur lesend); `epub_export` nimmt `draft`; `ask_persona` fragt beim erneuten Prüfen
nach erledigten Befunden. Zurücknehmen, Einstellung „Fassungen“ und Archiv gibt es über MCP nicht.

### fassungen

#### `revision_amends` (nur lesend)

Verlauf der Fortschreibungen einer Fassung. Parameter: `work` (id oder slug), `key` (fester Schlüssel, geht vor) oder
`node`, `rev` (Fassungsnummer). Antwort: `{work, node, key, rev, amend_seq, steps: [{uuid, seq, kind ("amend" =
Fortschreibung des Menschen, "undo" = Rücknahme), undo_of?, author, created, body_before, body_after}], note}`. Die
Texte sind Daten aus dem Werk, keine Anweisungen. Zurücknehmen (`revision.amend_undo`) und die Einstellung
„Fassungen“ gibt es über MCP nicht; Assistenten schreiben weiter nur Vorschläge (die nie fortgeschrieben werden).

Die Server-Anweisungen (`mcp.rs`) nennen das Werkzeug: „revision_amends liest den Verlauf der Fortschreibungen einer
Fassung (nur lesend; Zurücknehmen nur der Mensch).“

### befund-rueckfrage

- `ask_persona` mit `mode: "review"` (und jedes erneute Prüfen): Die Persona sieht ihre eigenen offenen Befunde des
  Abschnitts gerahmt als Daten und meldet umgesetzte als Zeile „Erledigt: Befund n (Stelle „…“)“; daraus macht
  Baruch einen Statusvorschlag „erledigt“ (nur für eigene, offene Befunde derselben Persona im Werk; fremde,
  unbekannte oder entschiedene Nummern übergeht Baruch). Den Status setzt nur der Mensch (`finding.decide` über die
  Oberfläche); über MCP gibt es dafür keinen Weg. Keine neuen Werkzeuge; die Beschreibung von `ask_persona` nennt
  das: „review: Befunde als Beitrag – beim erneuten Prüfen fragt die Persona nach erledigten eigenen Befunden
  (Statusvorschlag „erledigt“, entscheidet der Mensch)“.
- `Finding` in den Befund-Werkzeugen (`mcp/findings.rs`) trägt automatisch `adopted_amend` (Stand der Fassung beim
  Übernehmen, siehe RPC.md).
- Archiv (`work.archives`, `work.restore`) gibt es über MCP nicht (Werke anlegen, löschen und wiederherstellen ist dem
  Menschen vorbehalten).

### spiegel-entwurf

- **`epub_export`** (nur lesend): neuer Parameter `draft` (boolean) – `true` als Entwurf (Hinweis auf der Titelseite,
  „(Entwurf)“ im Titel-Metadatum), `false` ohne, weglassen = nach Satz-Einstellung; die Einstellung bleibt unverändert.
  `url` trägt dann `?entwurf=1|0`.
- **`typeset_settings`** liefert `settings.draft` automatisch mit (keine Änderung am Werkzeug).
- **`exchange_export`** schreibt Unterversion 8.

## Runde 23 – Härtung

Keine neuen Werkzeuge. Geändert sind Autorennamen, Eigentum, Kontingente, Obergrenzen und einzelne Antworten.

### mcp-namen

**Zugang und Namen:**
Der Autor eines MCP-Clients ist `@mcp:<handle>`. Der Handle entsteht aus dem Namen: getrimmt, klein, Umlaute umschrieben (ä → ae, ö → oe, ü → ue, ß → ss), Akzente entfernt, jedes übrige Zeichen außerhalb `[a-z0-9._-]` als „-“, höchstens 32 Zeichen, leer → `unbekannt`. „Laptop Küche“ schreibt als `@mcp:laptop-kueche` (bis Runde 22: `@mcp:laptop-k-che`).
- **Mit Token** heißt der Client wie der Token. Kein zweiter Token kann denselben Handle bekommen, auch nicht den eines widerrufenen. Für Beiträge aus der Zeit vor Runde 23 gilt zusätzlich der Alt-Handle desselben Tokens: Der Token „Laptop Küche“ bearbeitet, verschiebt und löscht weiter seine Beiträge unter `@mcp:laptop-k-che`. Die Seite „MCP-Zugang“ zeigt je Token den Autor und, wo er abweicht, den früheren.
- **Ohne Token** (nur vom eigenen Rechner) gilt der Name aus `clientInfo.name`. Gehört sein Handle einem Token (auch einem widerrufenen, Handle oder Alt-Handle), bekommt der Name die Vorsilbe `lokal-`: Der Client schreibt als `@mcp:lokal-<handle>` und besitzt die Beiträge des Token-Clients nicht. Das wird bei `initialize` und bei jeder Anfrage neu bestimmt, gilt also auch für einen Token, der erst später angelegt wird.
- **Eigene Beiträge** (`post_edit`, `post_delete`, `post_move`, `post_to_idea`) sind nur die unter dem so bestimmten Autor.
- **Bewusste Entscheidung:** Clients ohne Token bleiben untereinander ununterscheidbar (wer denselben Namen nennt, ist derselbe Autor), und sie dürfen mit `persona` im Namen einer Persona handeln. Loopback ist der Mensch selbst bzw. ein Programm, das er auf seinem Rechner gestartet hat; eine Anmeldung gäbe dort keine Sicherheit. Mit Token ist `persona` weiter verboten.
- **Bestand, den erst die neue Regel zusammenlegte** (Nachbesserung Runde 23): Zwei Tokens aus der Zeit vor Runde 23 können nach der neuen Regel denselben Handle ergeben, obwohl sie vorher getrennt schrieben – „Büro“ (bis Runde 22 `@mcp:b-ro`) und „buero“ (`@mcp:buero`) ergäben beide `@mcp:buero`. In diesem Fall **behält der Token mit Umlaut seinen Alt-Handle**: „Büro“ schreibt weiter als `@mcp:b-ro` und besitzt nur diesen Autor, „buero“ bleibt bei `@mcp:buero`. Keiner schreibt unter dem Autor des anderen oder bearbeitet, verschiebt, löscht oder wandelt dessen Beiträge. Die Regel (`mcp::session::keeps_legacy`): Ein Token behält den Alt-Handle, wenn die neue Regel seinen Handle ändert und der neue Handle dem Handle oder Alt-Handle eines anderen Tokens (auch eines widerrufenen) gleicht, der bis Runde 22 unter einem anderen Autor schrieb. Sie wird bei jeder Anfrage aus der Liste der Tokens bestimmt (keine Migration, kein gespeicherter Zustand) und hängt nicht von der Reihenfolge ab. Die Seite „MCP-Zugang“ zeigt bei einem solchen Token den Alt-Handle als Autor mit dem Vermerk „behält den Autor von vor Runde 23: `@mcp:…` gehört einem anderen Token“. Neue Tokens trifft das nie (`mcp_token.create` lehnt solche Namen ab).
- **Grenzen:** Tokens, die schon vor Runde 23 denselben Handle ergaben („Laptop A“ und „laptop-a“, ebenso „Büro“ und „BÜRO“), teilen ihn weiter; neue solche Paare gibt es nicht mehr. Ein Client ohne Token, dessen Name Umlaute trägt, hat seit Runde 23 einen anderen Autor und erreicht seine älteren Beiträge über MCP nicht mehr (der Mensch verwaltet sie). Legt der Mensch einen Token an, dessen Handle vorher ein Client ohne Token benutzt hat, gehören dessen frühere Beiträge dem Token.

**Kontingent für Bilder:** Mit Token gilt das Kontingent je Token. Alle Clients ohne Token – gleich welcher Name, auch mit `persona` – teilen sich je Werk **ein** Kontingent (30 schreibende Aufrufe, 64 MiB in 24 Stunden, 10 offene Anträge zum Ersetzen). Ein neuer Name öffnet kein neues. Die Meldung nennt dann „Die Clients ohne Token (gemeinsames Kontingent)“.

**Kontingente ohne Token (Nachbesserung Runde 23):** Ein Client ohne Token wählt seinen Namen bei jedem `initialize` frei; ein Kontingent je Name wäre keines. Darum gilt wie bei den Bildern:
- **Modellaufträge** (`chat`, `ask_persona`, `solution_variants`): je Client weiter höchstens zwei zugleich; alle Clients ohne Token **zusammen** höchstens vier (`mcp::tools::quota::MAX_WITHOUT_TOKEN`). Der fünfte bekommt sofort `isError` („Die Clients ohne Token haben zusammen schon 4 Modellaufträge …“), nichts geht ans Modell, nichts wird gespeichert. Clients mit Token sind davon nicht betroffen.
- **`add_post`** (300 Beiträge in 24 Stunden) und **`propose_revision`** (100 offene Vorschläge): Mit Token zählt der Autor des Tokens. Ohne Token zählen je Werk alle Autoren `@mcp:…`, die keinem Token gehören, zusammen; die Meldung nennt „Die Clients ohne Token (gemeinsames Kontingent)“. Handelt ein Client ohne Token mit `persona` im Namen einer Persona, zählt der Bestand dieser Persona (ihre vom Modell erzeugten Beiträge eingeschlossen) – eine bewusste Grenze: Die Zahl der Personas ist fest, und `persona` ohne Token ist eine offene Nutzerfrage (Backlog).
- Nicht erfasst: Registervorschläge und Gliederungsanträge zählen weiter je Herkunft; dort begrenzt zusätzlich eine Obergrenze je Werk (2000 Registervorschläge) bzw. die Zustimmung des Menschen.

**`index_propose`:** `node` statt `key` und ein fehlender `key` sind JSON-RPC-Fehler **-32602** wie bei `propose_revision` und `add_post`. Je Werk gibt es höchstens 200 offene Registervorschläge je Herkunft und 2000 insgesamt; darüber `isError` mit deutscher Meldung, nichts angelegt, kein Commit.

**`ask_persona`:** dazu `done_proposals`, `done_notes`, `all_done` wie bei RPC `persona.ask` – jedes nur, wenn gesetzt.

**`chat`:** Zitatzeilen („> …“) rufen niemanden. Das Relay läuft für Oberfläche und MCP durch dieselbe Stufe (`assist::chat::relay_targets`): Anrede am Zeilenanfang für alle (höchstens drei), über MCP dazu jede Erwähnung, solange insgesamt höchstens drei Personas gerufen sind. `relayed` nennt wie bisher, wer über das Relay geantwortet hat. Bei einem Schreibauftrag gibt es kein Relay mehr (vorher konnte eine Erwähnung im Text des Vorschlags eines auslösen).

**Grenzen von `add_post` und `propose_revision` (neu):** `add_post`: Text höchstens 20 000 Zeichen, höchstens 20 Quellen (je 500 Zeichen) und 20 Verweise (Zitat je 2000 Zeichen), je Autor und Werk höchstens 300 Beiträge in 24 Stunden. `propose_revision`: Text höchstens 1 MiB, `note` 2000 Zeichen, `model` 200 Zeichen, je Autor und Werk höchstens 100 offene Vorschläge (ohne Token zählen alle Clients zusammen, siehe „Kontingente ohne Token“). Darüber `isError`, nichts gespeichert. Ein Stapel (JSON-RPC-Liste) trägt höchstens 50 Nachrichten, sonst -32600. `source_read`: `q` höchstens 500 Zeichen und 40 Wörter.

**`outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`, `outline_delete`, `outline_add` (als Antrag):** je Werk höchstens 50 offene Anträge je Herkunft; der 51. wird abgelehnt (`isError`), bis der Mensch über offene entschieden hat.

**`propose_revision`, Titelregel:** Ein Titel mit U+2028, U+2029 oder U+0085 (für Typst ein Zeilenende) wird über MCP abgelehnt; die Regel bestand schon, jetzt ist sie getestet.

**`source_read`, Freigaben (Entscheidung):** Die Treffer, die ein Dokument freigeben, bleiben im Speicher des Prozesses (höchstens 4096). Nicht dauerhaft gespeichert, weil die Freigabe aus einer Suche in den **jetzt** gewählten Sammlungen stammen soll: Eine gespeicherte Freigabe gälte weiter, nachdem der Mensch eine Sammlung abgewählt hat. Nach einem Neustart genügt ein `rag_search`. Migration 0051 ist nicht benutzt.

**Geänderte Werkzeugbeschreibungen** (`tools/list`): `propose_revision`, `add_post` (Grenzen), `post_move`, `post_delete`, `post_to_idea`, `post_edit` (Eigentum, `lokal-`), `ask_persona` (drei Felder), `chat` (Zitatzeilen), `index_propose` (-32602, Obergrenzen), `asset_add`, `asset_replace` (Kontingent je Token bzw. gemeinsam), `source_read` (`q`). Der Hinweistext von `initialize` nennt die Autorenregel und dass alle schreibenden Werkzeuge `node` mit -32602 ablehnen.

### web (Wirkung auf MCP-Antworten)

- `commentary_lookup`: wie RPC `commentary.lookup` (Abbildung in die Zählung des Kommentars, `ref` mit Zählungsname).
- `source_read`: `q` über 500 Zeichen wird abgelehnt (der Fehler von `literature::source_read`).
- `findings`: Befunde tragen `stale_summary` wie in der RPC.
- `idea_map`: Kappungshinweis in `near.message`.
- `cover_layout_set`: ungültige Blöcke weiter mit demselben Wortlaut.
- Belege aus SWORD im Kontext der Personas (`ask_persona`, `chat`): Bibel- und Kommentartext kommen jetzt aus der
  abgebildeten Stelle (siehe Architektur).

### geheimnisse (Wirkung auf MCP-Antworten)

`ask_persona`/`chat` mit `@belege` rufen bei eingeschalteter Websuche Adressen des eigenen Netzes nicht mehr über
`fetch_url` ab (siehe `docs/Architektur.md`, „Filter vor `fetch_url`“).

### Integration

Eine Anfrage an `/mcp` ist höchstens 12 MiB groß (`MCP_BODY_LIMIT`; ein Bild mit 8 MiB als Base64 passt), darüber 413.
