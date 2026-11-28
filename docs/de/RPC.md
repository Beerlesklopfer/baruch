<!-- translation-of: docs/RPC.md source-commit: 00772cb -->

# JSON-RPC von Baruch {#json-rpc-of-baruch}

> Dies ist die deutsche Übersetzung von `docs/RPC.md` (englisches Original, maßgeblich), Stand: Commit `00772cb`.

`POST /rpc`, JSON-RPC 2.0, einzeln oder als Batch. Formulare ohne JavaScript senden dieselben Methoden an
`POST /act` (erlaubt: `revision.accept`, `revision.reject`, `revision.save`, `revision.set_current`,
`revision.update`, `revision.delete`, `post.add`; Felder wie unten (`node`, `key`, `rev`, `body`, `note`,
`based_on`), dazu `work`, `view`, `lq` und beim Löschen `confirm`) und werden dann auf die Seite umgeleitet
(`/?work=W&key=K&rev=R[&view=preview][&lq=…]`, `rev` = Nummer aus der Antwort). Formulare zur Verwaltung von Beiträgen
gehen an `POST /beitraege/act` (siehe „Managing posts“). Quelltext:
`crates/baruch-server/src/rpc.rs`. `rev` ist überall die **Nummer** einer Fassung, nicht ihre Position: Nach
einem Löschen hat die Nummernfolge Lücken; eine Nummer wird nie neu vergeben.

**Konten und Rollen (Runde 35):** Ohne Konten gilt hier alles wie beschrieben. Gibt es Konten, verlangt `/rpc`
eine Sitzung (Cookie `baruch_session`; sonst HTTP 401 mit Fehler -32002), und für Lektor-Konten sind nur die Methoden aus
`roles::LEKTOR_RPC` erlaubt (sonst -32003) – siehe „Round 35 – Accounts and roles“ am Ende.

**Abschnitte: Position und fester Schlüssel.** `node` ist die Position des Abschnitts in der Baumreihenfolge (`seq`,
ab 0); jede Änderung der Gliederung vergibt sie neu. `work.get` nennt zusätzlich für jeden Knoten `key`, den festen
Schlüssel (Datenbank-ID), der gleich bleibt, wenn andere Punkte verschoben, umgestuft, angelegt, gelöscht, geteilt oder
zusammengeführt werden. **Jede Methode mit `node` nimmt auch `key`** (eine Zahl oder Ziffern als Text), die
Gliederungsmethoden mit `target` nehmen auch `target_key`. Ist der Schlüssel vorhanden, **gilt er anstelle der Position**: Ein
veralteter Tab schreibt also in den Abschnitt, den er angezeigt hat, nicht in den, der jetzt an der alten Position steht,
und löscht nie den Punkt eines anderen. Gibt es den Punkt nicht mehr → -32000 „Diesen Gliederungspunkt
gibt es nicht mehr (gelöscht oder zusammengeführt); bitte neu laden.“ Alle Formulare und der Editor senden
den Schlüssel mit. Übersetzen und Schreiben laufen unter derselben Schreibsperre.

**Nur nach Schlüssel zitieren (Runde 5):** `highlight.cite` und `/literatur/act` mit `action=cite` nehmen
den Abschnitt nur über `key`. `highlight.cite` mit `node` → -32602 „node (Stelle) wird nicht angenommen; key
mitgeben“, ohne `key` → -32602; das Formular ohne `key` → 422 „Abschnitt fehlt.“ (auch wenn `node`
vorhanden ist), mit einem Schlüssel, den es nicht mehr gibt → 409. Ebenso verlangen die MCP-Werkzeuge
`propose_revision` und `add_post` (einschließlich `refs`) `key` (`docs/MCP.md`). Links `/?node=` dienen nur der Anzeige.

**Adressen (Runde 8):** Die kanonische Adresse eines Abschnitts ist `/?work=W&key=K` (+ optional `&rev=R`,
`&view=preview`, `&pick=…`, `&lq=…`, in dieser Reihenfolge). Sie bleibt gültig, wenn die Gliederung umgestellt wird.
`node=` wird nur beim Lesen als Rückfall akzeptiert; erzeugte Links und Umleitungen (`/act`,
`/gliederung/act`, `/literatur/act`, `/einfaelle/act`, `/beitraege/act`) tragen es nicht mehr. Auflösung: Gibt es
den Schlüssel, gilt sein Abschnitt (vor `node`); sonst `node`, sonst der Standardabschnitt, und der
Editorkopf zeigt „Abschnitt mit Schlüssel K gibt es nicht (mehr).“. Ebenso nehmen `/gliederung`,
`/gliederung/bearbeiten`, `/literatur` und `/quelle` den Parameter `key`. `pick` bleibt `<Position>:<Nummer>`.
Die Gliederungsmethoden antworten zusätzlich mit `key` (fester Schlüssel des Punkts `node`, sonst `null`).

**Überschriften und Gliederung:** Wird die aktuelle Fassung durch `revision.save` oder `revision.update` (der
aktuellen Fassung) gesetzt, muss ihr Text mit genau einer Überschrift der Ebene des Punkts beginnen (Teile:
nur eine Fassung mit Teilmarke muss mit genau dieser einen Marke beginnen, Runde 14). Sonst -32000 mit Begründung (andere Ebene, weitere Überschriften, keine Überschrift, Text davor), und
nichts ändert sich. Ein neuer Titel derselben Ebene benennt den Punkt um.
**Umschalten und Übernehmen** (`revision.set_current`, `revision.accept`) ändern nie den Text der Fassung;
stattdessen **folgt die Gliederung ihrer Überschriftzeile** (Nutzerentscheidung 2026-11-05): Bei gleicher Ebene nur
der Titel, bei anderer Ebene Ebene und Titel, und alle Unterpunkte verschieben sich um dieselbe Differenz (würde einer tiefer als
3 landen, wird abgelehnt), jeweils mit einer neuen Fassung des Menschen, die sich nur in der Überschriftzeile unterscheidet; die
Antwort trägt dann `outline`. Abgelehnt (-32000, nichts geändert) werden nur Fassungen ohne Überschrift, mit
weiteren Überschriften, mit Text vor der Überschrift (Hinweis auf „Speichern“ im Editor) und mit einer Ebene, die an dieser Stelle
ungültig ist (z. B. der erste Punkt als Abschnitt) oder einer Teilmarke auf einem Kapitel;
Teile: Eine Fassung mit Teilmarke gleicht den Titel an (Runde 14), eine ohne schaltet um, ohne anzugleichen.
So ergeben Export und erneuter Import immer dieselbe Gliederung; Teilen und Zusammenführen laufen nur über
`outline.*`.

**Löschbestätigung:** `/act` mit `revision.delete` verlangt das angehakte Kontrollkästchen `confirm` (Wert `1` oder
`on`); fehlt es, antwortet der Server mit 422 „Löschen bitte bestätigen.“ und löscht nichts (`required` im HTML
allein genügt nicht). Über `/rpc` verlangt `revision.delete` `confirm: true`. Fehler von `/act` kommen als
422 (ungültig, auch eine unbekannte Fassung) mit dem Text der Meldung.

Anfragen an `/rpc` brauchen `Content-Type: application/json` (sonst 415). Anfragen von fremden Webseiten
(`Origin` passt nicht zu `Host`, oder `Sec-Fetch-Site` ist weder `same-origin` noch `none`) lehnen
`/rpc` und `/act` mit 403 ab. Ist `Host` weder eine IP-Adresse noch `localhost` noch ein beim Start mit
`--allow-host NAME` oder `BARUCH_ALLOWED_HOSTS` erlaubter Hostname (Runde 24, Betrieb hinter einem
Reverse-Proxy, `docs/Betrieb.md`), antworten alle Routen (auch die Seite) mit 421 (Schutz gegen DNS-Rebinding,
`guard::local_host`). Unter einem erlaubten Namen ist `/rpc` wie die Benutzeroberfläche ohne eigene Anmeldung erreichbar –
den Zugangsschutz liefert der Proxy; der Vergleich von `Origin` und `Host` besteht nur, wenn der Proxy `Host`
unverändert durchreicht (schreibt er ihn um, antwortet `/rpc` mit 403). `/mcp` verlangt unter einem solchen
Namen immer ein Token (`docs/MCP.md`, „Access“). Der MCP-Server unter `POST /mcp` ist in `docs/MCP.md` beschrieben.

**Werk:** Jede Methode nimmt den optionalen Parameter `work` (Werk-ID, Zahl). Fehlt er, gilt das
Standardwerk (das mit der kleinsten ID **jetzt**, nach dem Löschen eines Werks also das nächste;
`work.update` und `work.delete` verlangen `work` ausdrücklich); ein unbekanntes Werk oder ein `work`, das keine Zahl ist,
ergibt -32602. Die Daten liegen in `<data>/baruch.sqlite`; jede Änderung ist eine Transaktion.

## Methoden {#methods}

| Methode | Parameter | Antwort |
|---|---|---|
| `work.list` | – | `{works: [WorkSummary]}` – alle Werke nach ID: `{id, slug, title, author, nodes, kind, subtitle, sections, proposals, created, updated}`; `nodes` = Knoten, `sections` = Knoten ohne Teile, `proposals` = offene Vorschläge, `updated` = letzte Änderung (Metadaten, Vorlage, Fassungen); auch Werke ohne jede Arbeit (siehe „Works“) |
| `work.get` | `{work?}` | `{work, lector, cover}` – Werk mit Gliederung, Fassungen, Beiträgen, Personas; Handle des Lektors; `cover` = `CoverInfo \| null` (Runde 11, siehe „Book as PDF and cover“) |
| `work.export` | `{work?}` | `{typst}` – Werkvorlage und aktuelle Texte als eine Typst-Datei (siehe „Import and export“) |
| `node.get` | `{node}` | Abschnitt mit Fassungen und Chat |
| `revision.accept` | `{node, rev}` | `{node, rev, outline?, commit?, mirror?, markdown?}` – der Vorschlag wird die einzige aktuelle Fassung (Umschalten; nur Vorschläge, sonst -32000); die Gliederung folgt ihrer Überschriftzeile, `outline` = Gliederungsantwort (ohne `commit`) bei anderer Ebene. Enthält der Vorschlag Markdown-Überschriften (`# Titel`, Nutzer 2026-11-07, `docs/WYSIWYG.md`), folgt eine Fassung des Menschen mit den umgewandelten Überschriften (wie `outline.save` mit `split`); `node`/`rev`/`outline` gelten dann für sie, `markdown` = Hinweis („umgewandelt“ oder warum nicht) |
| `revision.reject` | `{node, rev}` | `{node, rev}` – Vorschlag verworfen, `rev` ist die aktuelle Fassung; kein Commit |
| `revision.set_current` | `{node, rev}` | `{node, rev, outline?, commit?, mirror?}` – die Marke „aktuell“ auf eine beliebige nicht aktuelle Fassung (Vorschlag, verworfen, ersetzt); der Text der Fassung ändert sich nie, die Gliederung folgt ihrer Überschriftzeile (`outline` bei anderer Ebene, siehe oben); bereits aktuell → -32000 „Diese Fassung ist schon aktuell.“ |
| `revision.save` | `{node, body, based_on?, based_on_amend?, mode?}` | `{node, rev, saved, amend_seq?, reason?, reason_label?, commit?, mirror?}` – wie bisher; Runde 22: `mode` `"auto"` ergänzt die eigene aktuelle Fassung des Autors, wenn keine Regel eine neue Fassung verlangt (siehe „Amending“), `"new"` legt immer eine neue Fassung an; **fehlt `mode`, gilt `"new"`** (alle alten Clients unverändert); unbekannter Wert → -32602. `saved`: `"amended"` (mit `amend_seq` = neuer Stand), `"new"` (im Modus `"auto"` mit `reason` und `reason_label`) oder `"unchanged"` |
| `revision.update` | `{node, rev, body?, note?, based_on_amend?}` | `{node, rev, changed, amend_seq?, commit?, mirror?}` – Runde 23: Schritt im Verlauf und Zustandsprüfung, siehe „Round 23 – Hardening“; ändert Text und/oder Notiz der Fassung `rev` (nur der Mensch), setzt `edited`; `changed: false` ohne Änderung; leerer Text → -32000 „Eine Fassung braucht Text.“; Runde 18b: Markdown-Überschriften (`# Titel` …) im Text werden wie bei `revision.save` zu Typst-Überschriften (`baruch_typst::markdown`); eine weitere Überschrift lehnt die Methode dann wie eine getippte `=`-Zeile ab |
| `revision.delete` | `{node, rev, confirm: true}` | `{node, rev, deleted, reparented, commit?, mirror?}` – Fassung `deleted` gelöscht (nur der Mensch), `rev` = aktuelle Fassung, `reparented` = Kinder, die nun am Elternteil hängen; ohne `confirm: true` → -32602 „Löschen bitte bestätigen (confirm).“; aktuelle Fassung → -32000 |
| `revision.tree` | `{node?, chapter?}` | mit `node`: `{revisions: [RevisionInfo]}`; mit `chapter: true` oder ohne `node`: `{sections: [{node, number, title, current, revisions}]}` für das Kapitel des Abschnitts oder das ganze Werk. `RevisionInfo` = `{number, based_on, status, author, note, created, edited, current}` (ohne Text) |
| `post.add` | `{node \| key, body}` | `{node, key, id}` – Beitrag im Abschnitts-Chat (getrimmt); `id` = Beitrag |
| `post.delete` | `{id, confirm: true}` | `{id, node, key, deleted: {author, body}, commit?, mirror?}` – siehe „Managing posts“ |
| `post.move` | `{id, target_key \| target}` | `{id, node, key, from_node, from_key, commit?, mirror?}`; Runde 23: Die Befunde des Beitrags wandern mit (außer angenommenen) |
| `post.to_idea` | `{id, words}` | `{id, idea, node, key, commit?, mirror?}` |
| `preview.typst` | `{key?, page?}` | `{stand, pages, page, chapter, diagnostics, print, cached, millis, svg?}` – siehe „Typst preview“ |
| `library.list` | – | `{titles: [TitleEntry]}` – Titel der Bibliothek des Autors mit Zitaten |
| `library.search` | `{query, kinds?, work?, all_revisions?, limit?}` | `{hits: [SearchHit]}` – Volltextsuche, Werkzeug des Lektors |
| `ui.render` | `{work?, key?, node?, rev?, view?, pick?, lq?, fassungen?}` | `{tree, editor, assistants, url, title}` – HTML der drei Bereiche; `key` geht vor `node` (unbekannt → Rückfall mit Hinweis); `url` = `/?work=W&key=K&rev=R[&view=preview][&pick=…][&lq=…][&fassungen=alle]` (0.9.0: `fassungen: "alle"` öffnet den Schalter „ältere Fassungen“, siehe „Hiding older revisions“). `pick` = `"<node>:<rev>[,…]"` fügt Fassungen probeweise in die Vorschau ein (ungültige Einträge werden übersprungen, die Marke bleibt) |

Alle Methoden außer `work.list` nehmen zusätzlich `work?` (in der Tabelle oben nur bei `work.*` und
`ui.render` ausgeschrieben). Assistenten legen nur Vorschläge an; `revision.save`, `revision.set_current`,
`revision.update` und `revision.delete` sind dem Menschen vorbehalten und gibt es über MCP nicht.
`wysiwyg.*` (siehe „Typeset view“) liest nur und liegt nicht unter `/act`. `numbering.*` (siehe „Numbering schemes“) und `asset.*`
(siehe „Images“) liegen weder unter `/act` noch über MCP; sie haben eigene Formulare. Ebenso `persona.*`
(siehe „Personas“) und `idea.*` (siehe „Ideas“); beide arbeiten ohne Werk. Ebenso (Runde 9) `menu.*`,
`settings.*`, `connection.*` und `sources.*` (siehe „Menu“, „Settings“, „Connections and sources“) und (Runde 10)
`model.*`, `scripture.check`, `scripture.map`, `scripture.versifications` (Runde 17a) und `mcp_token.*` (siehe „Personas“), dazu (2026-11-06) `bible.*` (siehe „Bibles“). `ui.render` liefert in `editor`
die offenen Vorschläge des Abschnitts (`details.pv`, siehe `docs/Bedienung.md`, Vorschläge im Lesefluss) und
liest dafür das Cookie `baruch_folded` der Anfrage; seit Runde 8 stehen sie an ihrem Platz im Lesefluss
(`section.pv-flow`, je Stück `details.pv[data-pv-at][data-pv-kind]`, Formulare `pv-accept-…`/`pv-reject-…`),
und das Editor-HTML folgt dem Cookie `baruch_editor` (Schalter; bei „satz“ eine fertig gesetzte Ansicht im HTML).

### Beiträge verwalten (Runde 8) {#managing-posts-round-8}

`post.delete`, `post.move` und `post.to_idea` laufen unter der Schreibsperre und ergeben genau einen Commit im
Spiegel des Werks (`allow_empty`, Beiträge stehen nicht im Spiegel). Über RPC handelt immer der Mensch; er darf
alle Beiträge verwalten (MCP nur die eigenen, `docs/MCP.md`).

- `post.delete {id, confirm: true}`: ohne `confirm: true` → -32602 „Löschen bitte bestätigen (confirm).“;
  unbekannte ID → -32602 „Beitrag N gibt es in Werk W nicht.“
- `post.move {id, target_key | target}`: ans Ende des Abschnitts-Chats des Ziels (`target_key` geht vor
  `target`), nur innerhalb des Werks; gleiches Ziel → -32000; kein Ziel → -32602.
- `post.to_idea {id, words}`: Einfall mit Werk, Bezug = Abschnitt, Gedanke = Text, Quelle = Quellen, verbunden mit „; “
  (höchstens 200 Zeichen), Herkunft vom Autor (`@mcp:` → MCP, Persona-Handle → Persona, sonst Mensch);
  danach wird der Beitrag gelöscht – eine Transaktion. `words` wie bei `idea.create` (Liste oder Text). Schlägt
  der Einfall fehl (z. B. kein Stichwort), bleibt der Beitrag.
- Commit: Betreff „Beitrag <id>: gelöscht“ | „…: verschoben nach <Nummer Titel>“ | „…: in Einfall <n>
  umgewandelt“; Trailer `Baruch-Post`, `Baruch-Action` (`post-delete` | `post-move` | `post-to-idea`),
  `Baruch-Node` (fester Schlüssel), bei der Umwandlung `Baruch-Idea`. Löschen und Umwandeln tragen den ganzen
  Beitrag (Autor, Zeit, Abschnitt, Quellen, Verweise, Text) im Rumpf; die Umwandlung ist **ein** Commit, dessen
  Schnappschuss bereits `einfaelle.md` mit dem neuen Einfall enthält.
- Formular ohne JavaScript: `POST /beitraege/act` (Felder `method`, `work`, `key` = angezeigter Abschnitt, `id`,
  `rev?`, `view?`, `lq?`, `confirm`, `target_key`, `words`) → 303 auf `/?work=W&key=K[&rev][&view=preview][&lq]`,
  Fehler 422 mit Text, fremder Origin 403.

#### post.edit, post.versions, post.page, post.to_findings (Runde 28, chat-auftrag) {#postedit-postversions-postpage-postto_findings-round-28-chat-auftrag}

Beantwortet von `src/posts.rs` (Speicher `post_edit.rs`, Migration 0046; Volltextindex Bibliothek 0107). Handelnder ist
der Mensch.

- `post.edit {work?, id, body}` → `{id, work, node, key, body, edited, versions: [PostVersion], url, commit?|mirror?}`.
  Nur Beiträge des Menschen (Autor „Autor“); ein Beitrag einer Persona oder eines MCP-Clients → -32000 „Nur eigene
  Beiträge lassen sich bearbeiten …“ (nichts geändert, kein Commit). Text getrimmt, höchstens 4000 Zeichen; leer
  oder unverändert → -32602. Die alte Fassung bleibt als `post_version` erhalten, `edited` = Zeitpunkt der Änderung, genau ein
  Commit „Beitrag <id>: bearbeitet“ mit Rumpf `Vorher:\n…\n\nNachher:\n…` (Trailer `Baruch-Action: post-edit`,
  `Baruch-Post`, `Baruch-Node`). Die Volltextsuche (`library.search`) findet dann den neuen Text (Trigger
  `post_fts_edit`).
- `post.versions {work?, id}` → `{id, work, versions}`; `PostVersion` = `{id, uuid, body, valid_from, replaced,
  edited_by}`, älteste zuerst, leer bei nie bearbeiteten Beiträgen.
- `post.page {work?, key, before?, limit?}` → `{work, key, posts, older, history_url}`: die bis zu `limit` (1–200,
  Standard `CHAT_PAGE_POSTS` = 50) Beiträge vor dem Beitrag `before` (ohne `before`: die neuesten), älteste zuerst;
  je Beitrag `{id, author, body, created, time, edited, edited_time, own, style, sources, url}`; `older` = wie viele
  noch davor stehen. Ein `before` aus einem anderen Abschnitt → -32602.
- `post.to_findings {work?, id}` → `{id, work, key, findings: [finding_json], attached: [finding_json]}`: zerlegt
  den Beitrag durch `findings::ingest` mit `manual = true` (ohne Marker ein „Hinweis“ mit dem ganzen Text); der
  Beitrag bleibt byte-identisch, kein Commit (Befunde anzulegen ist keiner).
- Das Formular `POST /beitraege/act` nimmt zusätzlich `method=post.edit` (Feld `body`) und `method=post.to_findings`;
  mit `back=post` geht es dann auf die Beitragsseite `/beitrag?work&id` statt zum Abschnitt.
- Seiten ohne JavaScript: `GET /beitraege?work&key` (ganzer Chatverlauf, Anker `#post-<id>` und `#chat-neueste`,
  Chatformular) und `GET /beitrag?work&id` (Text in einem schreibgeschützten, markierbaren Feld, „Zitieren“ als
  Chatformular mit „> …“, Bearbeiten der eigenen Beiträge, Verlauf, „In Befunde umwandeln“).

### Typst-Vorschau (Runde 8) {#typst-preview-round-8}

`preview.typst {work?, key?, page?}` – nur lesend, kein Commit, beantwortet von `src/typeset.rs`. Das Werk wird
mit eingebettetem Typst 0.15.1 gesetzt (Werkvorlage, Nummerierungsschema, Bilder des Werks; ohne Netz, ohne
Pakete). Höchstens ein Lauf zugleich, ein Cache für 8 Stände und zusammen höchstens 192 MiB SVG (Runde
18a; der neueste Stand bleibt immer).

- `key`: fester Schlüssel eines Abschnitts; unbekannt → -32602. `page`: Seite ab 1; dann trägt die Antwort
  `svg` (die Seite als SVG-Text); außerhalb von `1..=pages` → -32602.
- Antwort: `{stand /*SHA-256 hex über Text und Bilder*/, pages /*Anzahl*/, page /*Seite des Abschnitts key, ohne
  key 1, mit page diese*/, chapter: {first, last}, diagnostics: [{severity: "error"|"warning", message, hints,
  line, column, file?, key, section /*„2.1 Titel“ | „Werkvorlage“ | null*/, section_line}], print: [PrintWarning],
  cached, millis, svg?}`. `PrintWarning` (Runde 11) = `{path, page, width_px, height_px, width_mm, height_mm,
  dpi, needed_width_px, needed_height_px, message}` für jedes Rasterbild unter 300 dpi in seiner gesetzten Größe
  (nicht in `diagnostics`). Der Satz schließt den Rahmen (Umschlag, Vorspann) ein, wenn er eingeschaltet ist.
- Teilseiten (Runde 14): Stehen Teilmarken (`… <baruch-part>`) im Werk, wird nach Werkvorlage und Rahmen eine Zeile `#show <baruch-part>: …`
  ergänzt (`baruch_typst::part_page_rule` mit den Nummern der Teile):
  jeder Teil allein auf einer eigenen rechten Seite, der Typst-Überschriftenzähler bleibt unberührt. Zeilen in
  `diagnostics` (`section_line`) beziehen sich weiterhin auf den Abschnitt. Ohne Teilmarke ist der
  gesetzte Quelltext byte-identisch mit `work.export` (oder mit Rahmen wie bisher).
- Bei einem Fehler: `pages = 0`, `diagnostics` mit `severity: "error"`. Zeitlimit 60 s → -32000 „Typst brauchte
  länger als 60 s“. Abgebrochen durch `preview.cancel` oder das Formular der Satzseite (Runde 18a) → -32000
  „Satz abgebrochen.“ (sofort, auch beim Warten auf die Sperre).
- `chapter` (Runde 18a): Eine Seite, die sich zwei Kapitel teilen, zählt für beide – `last` eines Kapitels ist die
  Seite, auf der das nächste beginnt, wenn dort noch Text aus seinen Zeilen steht (Überschriftzeilen zählen nicht,
  weil die Vorlage sie als lebende Kolumnentitel zitieren kann), sonst die Seite davor.
- Seiten ohne RPC: `GET /werke/{id}/satz?key=&page=` (HTML) und `GET /werke/{id}/satz/{stand}/{n}.svg`
  (`image/svg+xml`, `nosniff`, `Content-Security-Policy: sandbox; default-src 'none'; img-src data:;
  style-src 'unsafe-inline'`; fremder, alter oder ungültiger Stand, Seite 0 oder hinter dem Ende → 404).
  Runde 18a: Beide (und `GET /werke/{id}/pdf`; seit dem Nachtrag zu Runde 18 auch alle anderen GETs, die
  Typst starten: `/werke/{id}/umschlag`, `/werke/{id}/umschlag.svg`, `/werke/{id}/umschlag.pdf`,
  `/werke/{id}/stichwortzettel.pdf` und `/werke/{id}/karte.pdf`) antworten bei `Sec-Fetch-Site: cross-site` oder `same-site` mit 403
  „Abgelehnt: Der Satz startet nicht auf Anfrage einer fremden Seite.“, ohne Typst zu starten; ohne den Header (curl,
  ältere Browser) oder mit `same-origin`/`none` wie bisher. Die Satzseite lädt kein `main.js`; ihre Leiste hat
  das Formular `POST /werke/{id}/satz/abbrechen` (Feld `key?`, hinter dem Origin-Schutz: fremder Origin → 403) → 303
  auf `/werke/{id}/satz?abgebrochen=1[&key=K]` (lief keiner: `abgebrochen=0`). Mit `abgebrochen` setzt die Seite
  nicht neu, sondern zeigt nur einen bereits gesetzten Stand oder die Meldung mit „Neu setzen“.

### preview.cancel {#previewcancel}

`preview.cancel {work?}` → `{cancelled: bool}` (Runde 18a, `src/typeset.rs`). Bricht alle Typst-Läufe des Werks ab,
die durch `preview.typst`, die Satzseite, ihre Seitenbilder oder MCP `typst_check` gestartet wurden – auch die, die noch
auf die Sperre warten oder nach dem Zeitlimit im Hintergrund weiterrechnen. `cancelled` = mindestens einer lief.
Die wartenden Anfragen kehren sofort mit -32000 „Satz abgebrochen.“ zurück; ein abgebrochener Lauf legt nichts in den
Cache. Typst selbst hält erst nach dem Umbruch an (vor dem Schreiben der SVG-Seiten) und hält die
Sperre bis dahin; eine lange Schleife in der Vorlage läuft bis zum Ende (siehe `docs/Architektur.md`, „Round 18a – Ribbon“). Die PDF-Erzeugung
(`work.pdf`, `/werke/{id}/pdf`) wird nicht abgebrochen. Unbekanntes Werk → -32602. Kein Commit.

### ribbon.formats {#ribbonformats}

`ribbon.formats {work?}` → `{formats: [RibbonFormat]}` (Runde 18a, `src/ribbon.rs`), nur lesend. Die Schaltflächen der
Leiste: zuerst die Formate der Werkvorlage – `scene` nur, wenn sie `#let szene(…)` oder `#let szene =` definiert,
`work` nur mit `#let arbeit` –, dann immer `footnote`, `quote`, `list`, `enum`, `terms`, `break`, `link`.
`RibbonFormat = {cmd /*Befehl an die Satzansicht, baruch:ws-command*/, label /*„Szene“*/, title /*Tooltip*/, snippet
/*Typst-Text für den Quelltext, z. B. "#szene[\n\n]\n", "#footnote[]", "- \n", "\\\n"*/}`. Dieselbe Liste steht
in der Leiste des Editors (`data-ribbon-cmd`, `data-ribbon-snippet`). Unbekanntes Werk → -32602.
Seit Runde 30a kommt `xref` („Verweis“) hinzu; seit Runde 19b, nach `scene`/`work` aus der Vorlage: `scripture`
(„Bibelstelle“) nur, wenn die Vorlage `#let bibelstelle(…)` oder `#let bibel(…)` definiert (Snippet `#bibelstelle[]`
oder `#bibel[]` mit dem gefundenen Namen; Annahme: Die Funktion nimmt die Stelle als Inhalt), und `cite`
(„Literaturverweis“, Snippet `@`) nur, wenn die Vorlage `#bibliography(` enthält (nicht in einem Kommentar).
Die Erkennung arbeitet wie bei `szene`, ohne Typst (`ribbon::defined_functions` über `code_lines`), seit Runde 23 aber nicht mehr
nur zeilenweise: Zeilen- und (auch verschachtelte) Blockkommentare zählen nicht, ein `#let` im Rumpf einer anderen
Funktion (Klammertiefe > 0) zählt nicht, und mit `#import "datei.typ": name` eingeführte Namen zählen (`*` nicht;
Umbenennung `a as b` → `b`).

### Buch als PDF und Umschlag (Runde 11) {#book-as-pdf-and-cover-round-11}

Nur lesend oder nur für den Menschen; nicht über `/act`. Alle nehmen `work?`. Fehler: -32602 Eingabe,
-32000 Fachlogik (REFUSED).

| Methode | Parameter | Ergebnis |
|---|---|---|
| `work.pdf` | `work, inline?: bool = false` | `{file_name: "<slug>.pdf", pages, bytes, sha256, url: "/werke/{id}/pdf", print: [PrintWarning], stand, millis, pdf_base64?}` – das Buch wie die Satzansicht (gleiche Seitenzahl, auch mit den Teilseiten aus Runde 14); `pdf_base64` nur mit `inline: true`. Satzfehler → -32000 „Kein PDF (Buch): Der Satz scheitert mit n Fehler(n). Erster: …“; läuft bereits → -32000 „Für dieses Werk entsteht gerade ein PDF …“; Zeit → -32000 „Typst brauchte länger als 120 s“. Kein Commit |
| `work.set_cover` | `work, name: string\|null` | `{work, cover: CoverInfo\|null, commit?, mirror?}` (Commit „Umschlag: <name> gesetzt“ oder „Umschlag entfernt“ nur bei einer Änderung) |
| `work.meta.get` | `work` | `{work, meta: BookMeta + title/subtitle/author/bible + paper_sheet_mm (Runde 20c: Dicke je Blatt = 2 × paper_mm), cover: CoverInfo\|null}` |
| `work.meta.set` | `work, subtitle?, author?, bible?, publisher?, place?, year?, edition?, isbn?, price?, series?, printer?, bible_notice?, image_credit?, dedication?, blurb?, bio?, publisher_address?, printer_address?, author_photo?: name\|null, title_area?: "top"\|"middle"\|"bottom", front_matter?, cover_pages?, paper_mm?, paper_sheet_mm? (Runde 20c, je Blatt, über 0 bis 2 mm; nicht zusammen mit paper_mm → -32602), bleed_mm?` | `{work, changed: [...], commit?, mirror?}` (Commit „Buchangaben geändert: <Felder>“; `subtitle`/`author`/`bible` wie `work.update`) |
| `work.cover_pdf` | `work, inline?: bool` | `{file_name: "<slug>-umschlag.pdf", pages, bytes, sha256, url: "/werke/{id}/umschlag.pdf", spine_mm, checks: [{code, block, message}], pdf_base64?}` – Umschlag für die Druckerei (eine Seite); `checks` = Druckprüfung des Umschlags (Runde 27) |
| `notes.get` | `work` | Runde 16, nur lesend: `{work, title, kind: "book"\|"sermon", format_choices: ["a5", "karten"], cards: [{key, number, title, depth, start_clock, clock, points: [String], refs: [String], rule: "marker"\|"strong"\|"first_sentence"}], total_clock, target_clock\|null}` – Stichwortzettel: je Gliederungspunkt Zeitmarke und Dauer aus der Zeitwache (wie `work.timing`), Stichpunkte nach der Regel in `docs/Bedienung.md` (Stichwortzettel) (höchstens 6 zu je höchstens 160 Zeichen), gültige Bibelstellen normalisiert und ohne Duplikate |
| `notes.pdf` | `work, format?: "a5"\|"karten" = "a5", font_pt?: 14–28 (default 16 or 14), inline?: bool` | `{file_name: "<slug>-stichwortzettel.pdf", format, font_pt, pages, bytes, sha256, url: "/werke/{id}/stichwortzettel.pdf?format=…&schrift=…", print, millis, pdf_base64?}` – wie `work.pdf` (eine Erzeugung je Werk, gleiche Fehler mit „Kein PDF (Stichwortzettel): …“); falsches Format oder falsche Größe → -32602. Kein Commit |

`CoverInfo = {name, caption, alt, url: "/werke/{id}/bilder/{name}/datei", thumb: "/werke/{id}/bilder/{name}/vorschau?b=320"}`.

Seiten des Stichwortzettels (Runde 16, nur GET): `/werke/{id}/stichwortzettel?format=a5|karten&schrift=14–28`
(Formatwahl, Vorschau der Karten, Link zum PDF; falsche Eingabe → 422 mit Seite) und
`/werke/{id}/stichwortzettel.pdf?format=&schrift=` (`application/pdf` als Anhang `<slug>-stichwortzettel.pdf`;
409 läuft bereits, 422 Satzfehler oder falsche Eingabe, 503 Zeit abgelaufen – als Seite).
`BookMeta = {publisher, place, year, edition, isbn, price, series, printer, bible_notice, image_credit,
dedication, blurb, bio, author_photo, cover, title_area, front_matter, cover_pages, paper_mm, bleed_mm, updated}`.
Grenzen: einzeilige Felder 200 Zeichen, `blurb` 4000, `bio` 2000, `dedication`/`bible_notice`/`image_credit`
1000; `year` leer oder vier Ziffern; `isbn` leer oder ISBN-13 mit Prüfziffer; `paper_mm` (0, 1], `bleed_mm`
[0, 10] (Zahl oder Text mit Dezimalkomma); unbekanntes Bild, ungültiges `title_area` → -32602.

Seiten und Formulare (ohne JavaScript): `GET /werke/{id}/pdf` (200 `application/pdf`, `Content-Disposition:
attachment; filename="<slug>.pdf"`; 404 unbekanntes Werk, 409 läuft bereits, 422 HTML mit Meldungen und Links,
503 Zeit), `GET /werke/{id}/umschlag` (Seite), `…/umschlag.svg` (Vorschau, CSP `sandbox`), `…/umschlag.pdf`
(Anhang `<slug>-umschlag.pdf`, Fehlercodes wie bei `/pdf`), `POST /werke/umschlag/act` (`work, action=set|remove|area,
name, title_area, back`), `POST /werke/buchangaben/act` (`work`, alle Felder, Kontrollkästchen `front_matter`,
`cover_pages`, `back`). `back` nur relativ und sicher (Standard `/einstellungen/werk?work=W`); Ziel in den
Einstellungen → `&ok=umschlag|buchangaben`, sonst `?hinweis=umschlag_gesetzt|umschlag_entfernt|buchangaben`;
ungültige Eingabe → 422 mit erhaltener Eingabe.

Kommandozeile: `baruch-server export <id|slug> --pdf DATEI [--data ORDNER]` (überschreibt nie; Satzfehler →
Exit 1 ohne Datei; mit `--out` → Exit 2).

### Werke {#works}
Methoden aus `crates/baruch-server/src/works.rs`, nur für den Menschen (Oberfläche, RPC), **nicht über MCP**.
Sie laufen vor der Wahl des Standardwerks und funktionieren daher auch, wenn es kein Werk gibt. Titel sind eindeutig:
zwei Werke dürfen nicht denselben Titel haben, wobei Groß-/Kleinschreibung und Leerraum nicht zählen
(„ Das Buch “ = „das  buch“).

| Methode | Parameter | Antwort |
|---|---|---|
| `work.meta` | `{work?}` | `{work: WorkMeta, template_revisions: [{number, author, note, created}]}`; `WorkMeta` = `{id, slug, kind, title, subtitle, author, audience, bible, template, template_rev, source_name, created, updated}` (`audience` = Zielgruppe, `bible` = Bibelübersetzung, `template_rev` = Nummer der aktuellen Fassung der Werkvorlage) |
| `work.create` | `{title, author?, kind?, preset?, subtitle?, audience?, bible?, language?, text_ref?, module?, from_work?}` | `{work, slug, title, nodes, sermon_text?, commit?, mirror?}` – neues Werk; `kind` und `preset` aus `book`, `sermon` (fehlt eines, folgt es dem anderen, sonst Buch). Buch: Werkvorlage `#set document(title, author)` + `#set heading(numbering: "1.1")`, Kapitel „Erstes Kapitel“; Predigt (Runde 16a): `#set document`, Nummerierungsschema `sermon`, Punkte „Predigttext“ (ohne Nummer), „Hinführung“ (Einleitung), „Beobachtung am Text“, „Deutung“, „Zuspruch“ (I.–III.), „Anwendung“ (ohne Nummer), „Ausblick“ (Schluss). Je Gliederungspunkt Fassung 1 vom Menschen (`= <Titel>`), Vorlage als Fassung 1. Runde 39c: `language` (`de` oder `en`, fehlend oder leer = `de`, damit bestehende Aufrufer die deutschen Titel oben behalten) ist die Werksprache und wird in `work.language` geschrieben; die Titel der Voreinstellungspunkte stammen in dieser Sprache aus dem Buchkatalog (`book.preset.*`) (Englisch: „First chapter“, „Sermon text“, „Introduction“, „Observation on the text“, „Interpretation“, „Assurance“, „Application“, „Outlook“); jeder andere Wert → -32602 (`works_msg.rpc.language_invalid`), nichts angelegt. Runde 16a: `text_ref` (nur für eine Predigt, höchstens 500 Zeichen, genau eine gültige Bibelstelle, sonst -32602 und nichts angelegt) setzt den Predigttext: mit einer lesbaren SWORD-Bibel (`module`, leer: die Bibel von `from_work`, sonst des Standardwerks) als `#quote(block: true, attribution: [<Stelle> (<Modul>)])[<Wortlaut>]`, ohne Bibel nur die Stelle; die gewählte Bibel wird die SWORD-Bibel des neuen Werks. `sermon_text: {ref, normalized\|null, module\|null, quoted, verses, missing, notice\|null}` (`verses` = eingefügte Verse, `missing` = im Modul fehlende). Ohne `text_ref` bleibt `{kind: "sermon"}` gültig. Formular `POST /werke/act` (`action=create`, dazu `language`, `text_ref`, `module`, `from_work`; ein fehlendes Feld `language` bedeutet `de`) → 303 `/?work=N`, Fehler → Formularseite mit Meldung und Eingabe (422 oder 409) |
| `work.import` | `{file_name, source, title?}` | `ImportReport` = `{work, slug, title, nodes, commit?, mirror?}` – Typst-Text als neues Werk wie `baruch-server import`; nur Dateinamen, die auf `.typ` enden; `source` höchstens 16 MiB (UTF-8-Bytes), darüber -32602 „<datei> ist größer als 16 MiB“; `title` (leer = aus `#set document`, sonst der Dateiname). Die ganze Anfrage an `POST /rpc` darf höchstens `works::RPC_BODY_LIMIT` = 2 × 16 MiB + 64 KiB groß sein (JSON-Maskierung kann den Text verdoppeln); der Server lehnt eine größere Anfrage vor dem Lesen mit **413** ab |
| `work.update` | `{work, title?, subtitle?, author?, kind?, audience?, bible?, template?, note?}` | `{work, changed, template_rev?, commit?, mirror?}` – `changed` = Felder mit anderem Wert (`title`, `subtitle`, `author`, `kind`, `audience`, `bible`, `template`); eine andere Werkvorlage (CRLF → LF) wird die neue Fassung `template_rev` mit Notiz `note`, die alten bleiben lesbar; eine Überschrift in der Vorlage → -32000. Slug und Spiegelordner bleiben beim Umbenennen |
| `work.delete` | `{work, confirm_title}` | `{deleted, title, slug, archive, next, commit?, mirror?}` – löscht das Werk mit Gliederung, Fassungen, Beiträgen und Vorlagenfassungen; `confirm_title` muss der Titel sein (Groß-/Kleinschreibung und Leerraum egal), sonst -32000 „Der eingegebene Titel passt nicht; nichts gelöscht.“; `archive` = `works/.geloescht/<slug>-<JJJJ-MM-TT>` (Spiegelordner danach), `next` = neues Standardwerk oder `null` |

`work.update` und `work.delete` verlangen `work` ausdrücklich (fehlt → -32602). Fehler: Titel vergeben →
-32000 „Es gibt schon ein Werk „…“ (slug).“, leerer Titel → -32602 „Ein Werk braucht einen Titel.“, unbekanntes
Werk → -32602. Spiegel: Anlegen → Commit „Werk angelegt: <Titel>“; Ändern → „Werk geändert: <Felder>“ (z. B.
„Werk geändert: Titel, Werkvorlage Fassung 3“), nur wenn sich eine Datei ändert (Titel ohne Werkvorlage, Vorlage);
Löschen → leerer Log-Commit „Werk gelöscht: <Titel>“ mit Trailer `Baruch-Action: work-deleted`, danach
wandert der Ordner nach `<data>/works/.geloescht/<slug>-<JJJJ-MM-TT>/` (am selben Tag `-2`, `-3` …). Ein
Fehler im Spiegel stoppt das Löschen in der Datenbank nicht; die Antwort trägt dann `mirror`.
ID, Slug und Titel des gelöschten Werks stehen danach in `work_retired`: Die Datenbank vergibt die ID nie
wieder (`AUTOINCREMENT`, Migration 0007), der Slug bleibt für neue Werke gesperrt (ein neues Werk mit demselben
Titel heißt `<slug>-2`). Sind alle Werke gelöscht, legt der nächste Start kein Beispielwerk an (`store_meta.seeded`).

Ohne JavaScript: `POST /werke/act` (hinter demselben Schutz wie `/act`, fremder `Origin` → 403) mit `action` =
`create`, `update`, `delete` und den Feldern wie oben (`work`, `title`, `author`, `kind`, `preset`, `subtitle`,
`audience`, `bible`, `template`, `note`, `confirm_title`); danach 303 auf `/?work=<id>` (create, update) oder
`/werke?geloescht=<Titel>` (delete). Fehler als HTML-Seite mit dem Formular: 422 (ungültig), 409 (abgelehnt).
`POST /werke/import` (multipart, Felder `file` und `title`, Anfrage höchstens 16 MiB + 64 KiB): Erfolg → 303
`/?work=<id>`, nicht `.typ` oder kein UTF-8 → 422, größer als 16 MiB → 413, bereits importiert oder Titel vergeben
→ 409 mit Hinweis. Seiten: `GET /werke`, `/werke/neu`, `/werke/{id}/bearbeiten[?vorlage=n]`,
`/werke/{id}/loeschen`, `/werke/{id}/export` (`Content-Disposition: attachment; filename="<slug>.typ"`, Text
von `work.export`).

### Gliederung {#outline}
Methoden aus `crates/baruch-server/src/outline.rs`; alle nehmen `work?`, nur der Mensch (nicht über MCP; dort
gibt es `outline_add` und Anträge, siehe „Outline requests“ unten und `docs/MCP.md`).
Zahlen dürfen als Text kommen, Wahrheitswerte als `true`, `"1"` oder `"on"` (Formulare). Ebenen: Teil 0
(seit Runde 14 mit der Teilmarke `#heading(level: 1, numbering: none)[…] <baruch-part>` als erster Zeile),
Kapitel 1 (`= `), Abschnitt 2 (`== `), Unterabschnitt 3 (`=== `; jeweils auch als Marke „ohne Nummer“); die
Gliederung ist die Folge der Ebenen in Baumreihenfolge (Elternteil = nächster Vorgänger mit kleinerer Ebene).
Ungültig (Kapitel unter einem Abschnitt, Ebene 4, in den eigenen Teilbaum, Teil unter einem Punkt oder neben
einem Abschnitt, Ebenenwechsel zu oder von Ebene 0, keine Wirkung) → -32000, nichts geändert. Die Einschränkung „Teile nur
in Werken ohne Werkvorlage“ gibt es seit Runde 14 nicht mehr.

| Methode | Parameter | Antwort |
|---|---|---|
| `outline.move` | `{node, target, place}` | Gliederungsantwort – `place` = `before`/`after` (vor das Ziel oder hinter seine Unterpunkte) oder `into` (letzter Unterpunkt); **die Ebene bleibt**: `before`/`after` nur bei gleicher Ebene wie das Ziel, `into` nur bei Ebene des Ziels = Ebene − 1, sonst -32000 „Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).“; Unterpunkte wandern mit |
| `outline.set_level` | `{node, level}` | Gliederungsantwort – Ebene 1–3, Unterpunkte um dieselbe Differenz; nie Teile, nichts wird ein Teil (0 → -32000); eine Marke „ohne Nummer“ bleibt eine Marke (nur `level:` ändert sich) |
| `outline.rename` | `{node, title}` | Gliederungsantwort – neuer Titel, neue Überschriftzeile (maskiert; die Marke „ohne Nummer“ bleibt eine Marke). Teil (Runde 14): Mit Teilmarke wird nur ihre Zeile erneuert (Kommentar bleibt), ein leerer Teiltext bekommt eine; ein Teiltext ohne Marke (alt „= Teil …“) bleibt unverändert, nur der Titel ändert sich (`detail` sagt „Text ohne Teil-Markierung unverändert“) |
| `outline.add` | `{target, place, title, level?}` | Gliederungsantwort + `adopted: [key]` – neuer Punkt, Fassung 1 vom Menschen = Überschriftzeile + Leerzeile. `level` (Runde 14) = absolute Ebene 0–3; fehlt sie, die Ebene des Ortes. `level: 0` legt einen **Teil** an (Text = Teilmarke + Leerzeile): nur mit `place` `before`/`after` an einem Teil oder Kapitel, auch mit Werkvorlage; `into` mit 0 oder ein Abschnitt als Ziel → -32000. `adopted` = feste Schlüssel der vorhandenen Kapitel, die nun unter dem neuen Teil stehen (Reihenfolge, Texte und Kapitelnummern unverändert; sonst `[]`) |
| `outline.delete` | `{node, confirm: true}` | Gliederungsantwort – ohne `confirm` -32602 „Löschen bitte bestätigen (confirm).“; mit Unterpunkten oder als letztes Kapitel -32000; `node` = der nun angezeigte Punkt (der vorherige) |
| `outline.check` | `{node, body}` | `{title, level, split, merge, valid, message}` – was das Speichern ändern würde (schreibt nichts; Markdown-Überschriften zählen wie bei `outline.save`): `split` = Titel neuer Überschriften, `merge` = keine Überschrift |
| `outline.save` | `{node, body, based_on?, based_on_amend?, split?, merge?, mode?}` | `{node, rev, saved, amend_seq?, reason?, reason_label?, outline?, commit?, mirror?}` – wie bisher; `mode` wie bei `revision.save`. Teilen, Zusammenführen, eine neue Überschrift oder Ebene ergeben immer eine neue Fassung (`reason: "heading_changed"`) |
| `outline.set_numbered` | `{node, numbered}` | Gliederungsantwort (`action: "numbered"`) – `numbered: false` zählt den Punkt ohne Nummer (Vorwort, Einleitung), `true` wieder mit; die übrigen Nummern rücken nach, der Text bleibt; bereits so → -32000; Teil mit `numbered: false` → -32000 (Teile zählen immer: Die Teilmarke kennt kein „ohne Nummer“, Export → Import würde ihn wieder zählen); fehlendes `numbered` → -32602 |
| `outline.requests` | `{open? = true, preview? = false}` | `{work, requests: [OutlineRequest]}` – Anträge der Assistenten (nur lesend), mit `open: false` auch entschiedene; mit `preview: true` (Runde 14) je Antrag `preview: [PreviewPoint]` oder `preview_error` (siehe „Outline requests“) |
| `outline.request_decide` | `{id, decision: "confirm" \| "reject"}` | `{request, node?, key?, outline?, commit?, mirror?}` – siehe „Outline requests“; seit Runde 16 (Nutzerentscheidung 2026-11-08) trägt jede Entscheidung ohne Gliederungsänderung (abgelehnt oder veraltet, jede Antragsart) ebenfalls `commit` (leerer Log-Commit) oder `mirror` |
| `outline.set_numbering` | `{numbering}` | Gliederungsantwort – `null` = Schema „aus der Werkvorlage“, `""` = keine Nummern (`none`), sonst ein Muster der Mustersprache (Schema `custom`; `1.1`, `I|1`, `I.;A.;1.;a);aa)`); ungültig → -32602 „Nummerierung: …“, unverändert → -32000. Das Formular steht im Kopf des Baums (mit und ohne JavaScript) |

Titel (`outline.rename`, `outline.add`, Überschriftzeilen in `outline.save`, `revision.save`,
`revision.update` der aktuellen Fassung, `revision.set_current`/`revision.accept`, Vorschläge über MCP und
`highlight.cite`): nicht leer, ohne Zeilenumbruch, höchstens 200 Zeichen; sonst -32602 „Ein Gliederungspunkt
braucht einen Titel.“ / „Der Titel darf keinen Zeilenumbruch enthalten.“ / „Der Titel ist zu lang (höchstens
200 Zeichen, hier N).“, nichts geändert. `outline.check` meldet dasselbe als `valid: false` mit `message`.
(Bis Runde 6 war ein leerer Titel -32000.) Vorhandene leere Titel aus einem Import bleiben, bis jemand umbenennt.

Gliederungsantwort: `{node, action, title, detail, revisions: [{node, rev}], renumbered: [[alt, neu], …],
commit?, mirror?}` – `node` ist danach die seq, `action` eines von `moved`, `level`, `renamed`, `added`,
`deleted`, `split`, `merged`, `numbering`, `numbered`, `revisions` die neuen Fassungen (nur Überschriftzeilen),
`renumbered` die geänderten seq. Jede Änderung vergibt seq in Baumreihenfolge neu und schreibt Verweise in
Beiträgen um (Verweise auf gelöschte Punkte entfallen); der Schlüssel `key` bleibt. `revision.save` ändert nie
die Gliederung (kein Teilen, kein Zusammenführen, kein Ebenenwechsel: Es lehnt solche Texte ab, siehe
„Headings and outline“ oben); der Editor speichert über `outline.save`. Wie nach jeder Änderung der
aktuellen Fassung (`revision.save`, `revision.accept`, `revision.set_current`, `revision.update` der aktuellen
Fassung) übernimmt der Gliederungspunkt den Titel aus der Überschriftzeile derselben Ebene (`sync_heading`);
`revision.accept` und `revision.set_current` übernehmen auch eine andere Ebene (siehe oben).

Ohne JavaScript: `POST /gliederung/act` (hinter demselben Schutz wie `/act`, fremder `Origin` → 403) mit
`method` = `outline.*` (außer `outline.check`) und den Feldern `work`, `node`, `key`, `target`, `target_key`, `place`, `level`,
`title`, `body`, `based_on`, `based_on_amend`, `split`, `merge`, `confirm`, `numbered` (`1`/`0`), `id`, `decision`, `numbering` (`vorlage` = aus der Vorlage, `keine` =
keine Nummern, sonst ein Muster), `view`, `lq`; danach 303 auf `/?work&node[&rev][&view=preview][&lq]` oder mit
`view=gliederung` auf `/gliederung?work&node[&lq]`. Fehler 422 (ungültig) oder 409 (abgelehnt). Würde
`outline.save` teilen oder zusammenführen, antwortet es mit 200 und einer Bestätigungsseite (Text in einem versteckten
Feld, Schaltflächen „Teilen und speichern“ oder „Zusammenführen und speichern“ mit `split=1`/`merge=1`, „Zurück“).
Seiten: `GET /gliederung?work&node&lq` (Gliederungsverwaltung), `GET /gliederung/bearbeiten?work&node&lq` – der
Rückfall ohne JavaScript; mit JavaScript verlinkt die Oberfläche sie nicht (Baum mit Kontextmenü, siehe
`docs/Bedienung.md`).

#### Gliederungsanträge {#outline-requests}

Migration 0012 (`outline_request`), Wunsch vom 2026-11-06 „UD nach Bestätigung“. Assistenten legen Anträge über MCP an
(`outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`, `outline_delete`);
der Mensch entscheidet mit `outline.request_decide`. Seit Nutzerfrage 12 (Entscheidung 2026-11-07, Migration 0035)
wird auch MCP `outline_add` zu einem Antrag (Art `add`), wenn ein neuer Teil vorhandene Kapitel übernähme; solche
Anträge brauchen die **aktive Zustimmung** des Menschen (Dialog auf jeder Seite, siehe `docs/Bedienung.md`).

`OutlineRequest` = `{id, work, op: {op: "rename"|"move"|"set_level"|"set_numbered"|"delete"|"add", key, title? |
target_key, place? | level? | numbered? | place, points: [{title, level?}], model}, expected: {title, level,
target_title?, adopted?}, origin: {kind: "persona"|"mcp",
handle}, origin_label, note, created, decided, decision: null|"confirmed"|"rejected"|"stale", decision_label,
decided_by, message, summary}`. `expected` ist der Zustand zur Zeit des Antrags, `summary` der deutsche Satz (`„„Sommer“ in
„Hochsommer“ umbenennen“`).

- `decision: "reject"` → `decision: "rejected"`, die Gliederung bleibt; bei **jeder** Antragsart (Nutzerentscheidung
  2026-11-08, Runde 16; vorher nur `add`) ein leerer Log-Commit „Gliederung: Antrag N abgelehnt“ mit Trailern
  `Baruch-Action: outline-request`, `Baruch-Decision: rejected`, `Baruch-Origin`, `Baruch-Request`,
  `Baruch-Requested-By`, `Baruch-Decided-By`; die Antwort trägt `commit` (oder `mirror` bei einem Spiegelfehler).
  Ebenso „… veraltet“ mit `Baruch-Decision: stale`, wenn ein Antrag bei der Bestätigung veraltet ist.
- `decision: "confirm"` → Ist der Antrag veraltet (Punkt oder Ziel weg, Titel oder Ebene anders als
  `expected`) oder lehnt die Gliederungsfunktion jetzt ab, wird er mit `decision: "stale"` und `message` abgeschlossen
  (Gliederung unverändert, leerer Log-Commit „… veraltet“, siehe oben). Ein Antrag `add` ist auch
  veraltet, wenn der neue Teil nun andere Kapitel übernähme als `expected.adopted`. Sonst läuft er wie `outline.rename`/`move`/`set_level`/
  `set_numbered`/`delete` im Namen des Menschen (`add`: die neuen Punkte wie `outline.add`, Fassung 1 vom
  Antragsteller): `decision: "confirmed"`, `outline` = Gliederungsantwort,
  genau ein Commit „Gliederung: … (Antrag N)“ mit Trailern `Baruch-Origin`, `Baruch-Request`,
  `Baruch-Requested-By`, `Baruch-Confirmed-By`.
- Bereits entschieden → -32000 „Über diesen Antrag ist schon entschieden.“; unbekannte ID → -32602 „Antrag N gibt
  es in Werk W nicht.“; fehlendes oder anderes `decision` → -32602.

**Vorschau (Runde 14):** `outline.requests` mit `preview: true` berechnet für jeden offenen Antrag die Gliederung nach
der Bestätigung (Speicher `outline_request_preview`: in einem Savepoint ausgeführt und zurückgerollt, die Datenbank
bleibt): `preview` = `[PreviewPoint]` in Baumreihenfolge, `PreviewPoint` = `{key, level, number, title, mark,
kind_word, before_number, before_level, label, before_label, changed_label}` (Runde 16: `kind_word` „Teil“ |
„Kapitel“ | „Abschnitt“ | „Unterabschnitt“; `label` Art und Nummer nach dem Schema, z. B. „Teil I“, „Kapitel 2“, für
Predigten „Predigtteil II“, ohne Nummer „Kapitel (ohne Nummer)“; `before_*` der Zustand vor der Bestätigung, `null` für
neue Punkte; `changed_label` = Art- oder Nummernwechsel oder das Kapitel wandert unter einen neuen Teil – dann
zeigen Seite und Dialog „(vorher … → nachher …)“),
`mark` = `same` | `renamed` | `moved` (der verschobene Punkt mit seinen Unterpunkten) | `level` (auch mitwandernde
Unterpunkte) | `numbered` | `deleted` (gelöschte Punkte an ihrer alten Position, mit alter Ebene und Nummer) |
`added` (neuer Punkt eines Antrags `add`) | `adopted` (Kapitel, das unter den neuen Teil wandert).
Titel, Ebenen, Nummern und Reihenfolge (ohne `deleted`) sind die der Gliederung nach `outline.request_decide`
mit `confirm`. Statt `preview` gibt es `preview_error` (Text), wenn der Antrag veraltet ist („Veraltet: …“ mit
Begründung), die Gliederungsfunktion jetzt ablehnt („Nicht mehr ausführbar: …“) oder er bereits entschieden ist. Die Seite
`/gliederung#antraege` zeigt die Vorschau jedes offenen Antrags ohne JavaScript (`<details>` „Vorschau“).

Ohne JavaScript: Formular an `POST /gliederung/act` mit `method=outline.request_decide`, `work`, `id`, `decision`
(`confirm`/`reject`); danach 303 auf `/gliederung?work=…#antraege`. `outline.requests` liegt nicht unter `/act`.

**Zustimmung auf jeder Seite (Nutzerfrage 12):** Solange ein Antrag `add` offen ist, setzt die Schicht
`crate::consent` in jede HTML-Seite mit der Marke `<!--baruch-consent-->` (aus `base.html`) eine Hinweisleiste
mit dem Link `/gliederung?work=W#antrag-N` und einen `<dialog id="consent" data-widget="consent">` mit Vorschau und
den Formularen „Zustimmen“/„Ablehnen“ (dieselben Felder wie oben, wie alle Formulare durch
`guard::same_origin` geschützt). Das Widget `web/src/consent.ts` öffnet den Dialog modal und sendet die Entscheidung über
`outline.request_decide`; ohne Klick geschieht nichts. Andere Antworten (JSON, PDF, Fragmente) bleiben
unverändert. Neue RPC-Methoden gibt es dafür nicht.

#### Nummerierungsschemata {#numbering-schemes}

Methoden aus `crates/baruch-server/src/numbering.rs`; alle nehmen `work?`, nur der Mensch (nicht über MCP).
`scheme` ist eine ID: `template`, `decimal`, `decimal_dot`, `alphanumeric`, `chapter_roman`,
`chapter_arabic`, `parts_chapters`, `chapter_words`, `legal`, `sermon`, `book`, `none`, `custom`; `custom` ist das
eigene Muster (nur mit `custom`; fehlt es, gilt das gespeicherte). Unbekanntes Schema, fehlendes oder
ungültiges Muster → -32602.

| Methode | Parameter | Antwort |
|---|---|---|
| `numbering.schemes` | `{}` | `{schemes: [{id, label, example, preview: [NumberPreview], more, error?}], current: NumberingInfo}` – `preview` = die ersten 12 Punkte der eigenen Gliederung des Werks, `more` = weitere; `error` statt einer Vorschau (z. B. eigenes Muster fehlt) |
| `numbering.preview` | `{scheme, custom?}` | `{numbers: [NumberPreview], rule}` – alle Punkte; `rule` = die Zeile `#set heading(numbering: …)`, die der Export hinter die Vorlage setzt, `null` für `template`. Schreibt nichts |
| `numbering.set` | `{scheme, custom?}` | Gliederungsantwort (`action: "numbering"`, `detail` z. B. „Gliederungsschema „Juristisch““, „Nummerierung „I|1““, „keine Nummern“, „Nummerierung aus der Vorlage („1.1“)“), `commit?`/`mirror?`; unverändert → -32000 |

`NumberPreview` = `{node, key, level, number, title}` (Position, fester Schlüssel, Ebene 0–3, Nummer nach dem
Schema, Titel), in Baumreihenfolge. `NumberingInfo` = `{numbering, template, effective, scheme,
scheme_label, custom}`: `numbering` = Mustersprache (`null` = aus der Vorlage oder ein Schema mit Wörtern, `""` =
keine), `template` = Muster der Werkvorlage, `effective` = Anzeigetext (Muster, `""` = keine, bei Schemata
mit Wörtern ihr Beispiel), `custom` = gespeichertes eigenes Muster (auch wenn gerade ein anderes Schema
gilt).

Runde 14 (Namen und Antwortformen unverändert): Bei `template` stammen die Nummern in `preview`/`numbers`
und im Baum aus `Numbering::from_template` – ein String in der Vorlage ist ein wörtliches Typst-Muster (`;`, `|`
sind Text), Baruchs eigene Regel eines Schemas mit Wörtern (aus einem früheren Export) zählt wieder wie
dieses Schema, eine Funktion wird je Ebene genau ausgewertet. `NumberingInfo.effective`/`template` zeigen für
eine solche Funktion weiterhin die Näherung (erstes `numbering`-Muster); für Baruchs Regel eines Schemas mit
Wörtern ist `numbering` `null` und `effective` das Beispiel von „Aus der Werkvorlage“. Kapitel mit „Nummer
aus“ standen im Export (`work.export`), in der Exportkarte, im Spiegel und im Satz als
`#heading(level: N, numbering: none)[…] <baruch-unnumbered>`; die gespeicherten Texte bleiben unverändert.

Ohne JavaScript: Seite `GET /werke/{id}/nummerierung[?schema=ID&muster=SPEC]` (auch `scheme`/`custom`, wie
die Schaltfläche „Vorschau“ es per GET sendet), Formular `POST /werke/nummerierung/act` (hinter demselben Schutz
wie `/act`, fremder `Origin` → 403) mit `work`, `scheme`, `custom`; danach 303 auf
`/werke/{id}/nummerierung?ok=1` (Hinweis „Das Gliederungsschema ist übernommen.“), Fehler 422 (ungültig)
oder 409 (unverändert) mit der Seite und der Meldung. Spiegel: genau ein Commit „Gliederung: <detail>“,
Trailer `Baruch-Action: outline-numbering`, auch ohne Dateiänderung.

Git-Spiegel für die Gliederung: Jede Gliederungsänderung ergibt genau einen Commit (auch ohne Dateiänderung, z. B.
bei der Nummerierung), Autor und Committer der Mensch, Betreff `„Gliederung: <detail>“` (z. B. `„Gliederung:
„Sommer“ vor „Frühling“ verschoben“`, `„…: „Aussaat“ Ebene 2 → 1“`, `„…: „Alt“ → „Neu“ umbenannt“`, `„…: „Tiefe“
gelöscht“`), Rumpf mit den neuen Fassungen, der Umnummerierung und dem vollen Text gelöschter Punkte,
Trailer `Baruch-Action: outline-move|outline-level|outline-rename|outline-add|outline-delete|outline-split|outline-merge|outline-numbering`
und `Baruch-Node`. Abschnittsdateien heißen `{seq:03}-<slug(aufgelöster Titel)>.typ`; nach einer Umstellung
werden sie umbenannt.

### Import und Export {#import-and-export}
Ein Werk entsteht aus einer Typst-Datei durch `baruch-server import <datei> [--title "…"]`
(`docs/Bedienung.md`), durch Hochladen auf `/werke/neu` oder im Dialog „Importieren…“ (`POST /werke/import`)
oder RPC `work.import`; ein MCP-Werkzeug dafür gibt es nicht (ein Import legt aktuelle Fassungen im Namen des
Menschen an). Der Titel muss frei sein (sonst -32000, in der CLI mit dem Hinweis auf `--title`); dieselbe Datei
ein zweites Mal bleibt „schon importiert“, auch mit anderem Titel. `work.export` liefert das Werk wieder als eine Datei: Werkvorlage +
aktuelle Texte in Gliederungsreihenfolge, ohne die Zeilen `#import "../vorlage.typ": *`, die nur im
Git-Spiegel vor jeder Abschnittsdatei stehen. Direkt nach dem Import ist das Byte für Byte die Eingabe (nach
CRLF → LF). Endet ein gespeicherter Text ohne Zeilenende, fügt der Export eines hinzu, damit die nächste
Überschrift am Zeilenanfang steht. Bei einem anderen Nummerierungsschema als „aus der Werkvorlage“ folgt auf die
Vorlage die Regel des Schemas (siehe „Numbering schemes“). Bilder kommen über die CLI mit
(`import` liest `image("…")`-Pfade relativ zum Ordner der Datei, `export --out ORDNER` schreibt Text und
Bilder, `docs/Bedienung.md`) und seit Runde 21b über ein ZIP oder einen Ordner (`work.import_archive`, Upload);
`work.import` und das Hochladen einer einzelnen Datei bringen nur den Text.

Seit Runde 21b nimmt `work.import {file_name, source, title?}` neben `.typ` auch Markdown (`.md`, `.markdown`, nur Text,
höchstens 16 MiB): Der Text wird nach Typst umgewandelt (Tabelle in `docs/Architektur.md`, „21b
markdown-import“), jedes Bild, das er nennt, steht mit dem Grund „nicht mitgeschickt …“ in `skipped`, und was nicht
abgebildet werden konnte, in `warnings` (Codes `md-…`, mit der Zeile der Markdown-Datei). Ein `.zip` über `work.import` →
-32602 mit dem Hinweis auf `work.import_archive`; jede andere Endung → -32602.

Rückweg eines Exports (Runde 21b, alle Importwege): Regelblock „// Baruch: Satz“ → Satzeinstellungen des neuen Werks
(`warnings` `import-typeset-adopted`, sonst `import-typeset-kept`), Show-Regeln „// Baruch: Überschriften ohne Nummer“
→ `numbered = 0`, erzeugter Anhang abgeschnitten (`import-back-matter-cut` mit Zeile; eine von Hand geschriebene Kennzeile bleibt,
`import-back-matter-kept`), Zusätze der Projektion entfernt (`import-projection-removed`). Einzelheiten in
`docs/Bedienung.md` („Export → import, hint codes“).

Teile und „ohne Nummer“ (Runde 14): Die Zeilen `#heading(level: 1, numbering: none)[T] <baruch-part>` und
`#heading(level: N, numbering: none)[T] <baruch-unnumbered>` (kanonisch, `docs/Architektur.md`, Teile im
Typst-Text) werden beim Import zu einem Teil (oberste Ebene, die folgenden Kapitel darunter) oder einem Punkt mit
`numbered = 0`; der Text bleibt wörtlich, `work.export` liefert die Datei byte-identisch zurück (auch mit einer
Werkvorlage). Jede andere `#heading(...)`-Zeile bleibt wie bisher Text oder Werkvorlage.

Fehler beim Import (Codes wie bei RPC, in der CLI als Meldung auf stderr und Exit 1): Datei bereits importiert
(gleiches SHA-256 nach CRLF → LF) → -32000 mit „Diese Datei ist schon importiert: Werk „…“ (slug).“; Titel
vergeben → -32000 „Es gibt schon ein Werk „…“ (slug).“ (CLI: „… Mit --title "…" einen anderen Titel wählen.“); keine
Überschrift, kein UTF-8, größer als 16 MiB, keine reguläre Datei (FIFO, Gerät, Ordner: „… ist keine
Datei“) → -32602.

### source_import.preview, source_import.get, source_import.apply, source_import.discard (21a) {#source_importpreview-source_importget-source_importapply-source_importdiscard-21a}

Global (ohne Werk; Weiche in `rpc::call` nach `works::handles`, Modul `pdf_import`). Kein MCP-Gegenstück: Ein Import
legt aktuelle Fassungen im Namen des Menschen an (Entscheidung 2026-11-05), bei PDF/EPUB ebenso wenig.

- **`source_import.preview {file_name, content, way?, collection?, title?}`** – `content` = Datei als Base64 (Standard,
  mit `=`); `file_name` endet auf `.pdf` oder `.epub`; `way` = `"auto"` (Standard) | `"multigpt"` | `"local"`. Liest die
  Datei und legt nur einen **Entwurf** an (kein Werk, kein Titel). Antwort:
  `{draft, file_name, kind: "pdf"|"epub", way: "multigpt"|"local", way_reason, title, author, outline: [{index, level
  1..3, title, chars, part, sample}], warnings: [{code, message, line}], skipped: [{path, reason}], assets: [pfad],
  pages?, document?: {collection, id}}`. `title` = Titel, unter dem sie übernommen würde (eigenes `title`, sonst aus der
  Datei, sonst der Dateiname); `part` = „möglicher Teil“ (nur ein Hinweis; ein Teil entsteht nur mit `part: true` in
  `apply`); `sample` = Textprobe (höchstens 240 Zeichen); `index` = Nummer des Abschnitts für `apply`.
  - Weg: EPUB immer lokal (Grund in `way_reason`). PDF mit `"auto"`: MultiGPT, wenn eine Verbindung eingerichtet ist
    und `collection` angegeben wird, sonst lokal (Grund). `"multigpt"` ohne Verbindung oder ohne `collection` → -32602.
  - Scheitert MultiGPT (Hochladen, Indexieren, Lesen) → -32000 mit dem Grund, „Weg multigpt“ und dem Angebot „lokal
    lesen“ (dieselbe Datei mit `way: "local"`); **kein** automatischer Wechsel.
  - Lokal ohne Poppler → -32000 „pdftotext … ist nicht installiert …“.
  - Ungültig (Name, Base64, leer, größer als 64 MiB oder über `RPC_BODY_LIMIT`, EPUB unsicher oder zu groß –
    Pfadflucht, Link, Grenzen von `archive::IMPORT_LIMITS` –, kein OPF) → -32602; nichts wird angelegt.
  - Warncodes (Auswahl): `import-pdf-plain`, `import-pdf-page-numbers`, `import-pdf-numbering`,
    `import-pdf-pages-cut` (lokal mehr als 2000 Seiten: nur die ersten gelesen), `import-epub-footnote-number`
    (Fußnotennummer am Anfang einer Fußnote entfernt),
    `import-multigpt-plain`, `import-multigpt-truncated`, `import-heading-not-found`, `import-outline-heuristic`,
    `import-epub-table`, `import-epub-unsupported`, `import-epub-heading-as-text`, `import-epub-footnote-missing`,
    `import-epub-nav-target-missing`, `import-epub-deep-nav`, `import-epub-no-nav`, `import-front-matter`,
    `import-no-outline`; beim Übernehmen `import-outline-merged`.
- **`source_import.get {draft}`** → wie `preview`. Unbekannt, verworfen, übernommen oder älter als 24 h → -32602.
- **`source_import.apply {draft, target?, title?, author?, outline?}`** – `target` = `"work"` (Standard) | `"library"` |
  `"both"`. `outline` = die vom Menschen bestätigte Gliederung `[{level 1..3, title, part?, source?}]`:
  - mit `source` (= `index` aus der Vorschau): Jeder Punkt mit `source` erhält den Text dieses Abschnitts; `source`
    steigt (Reihenfolge wie in der Datei, Umordnen → -32602); Abschnitte ohne Punkt werden **gestrichen**, ihr Text geht
    an den vorherigen Punkt (am Anfang an den nächsten), Warnung `import-outline-merged`; Punkte ohne `source` sind neue,
    leere Punkte (etwa ein Teil);
  - ganz ohne `source`: so viele Punkte wie vorgeschlagen, Zuordnung nach Position;
  - `part: true` (nur Ebene 1) legt einen Teil an (AGENTS.md, Ergänzung 2026-11-07: nur auf Bestätigung des Menschen);
  - ohne `outline`: der Vorschlag, Teile als Kapitel.

  Werk durch `import::import_generated` (Notiz und Commit „Import aus <datei>“, Bilder des EPUB als Bilder des Werks,
  Warnungen im Bericht). Quelle durch `baruch_library::add_title` (Art `book`, Schlüssel `q-…`, Titel/Autor, Notiz mit
  Datei und Weg) und – lokal gelesen – Volltext in `library_title_text` (Migration 0108) oder – durch MultiGPT – die
  Verknüpfung `library_title_remote` (Sammlung, Dokument). Antwort `{report?: ImportReport, library_title?: id}`; danach
  ist der Entwurf weg. Fehler: Titel vergeben → -32000 mit `TITLE_HINT` (Entwurf bleibt); Datei bereits Quelle in der
  Bibliothek (gleiches SHA-256 oder gleiches Dokument) → -32000, dann wird auch kein Werk angelegt; Gliederung ungültig (leer,
  Ebene, Titel fehlt, Teil nicht oben, `source` falsch) → -32602; zweimal zugleich → -32000.
- **`source_import.discard {draft}`** → `{discarded: bool}` (`false`, wenn es ihn nicht gab); kein gültiger Name
  (32 Hex-Zeichen in Kleinbuchstaben) → -32602.

Der lokale Volltext einer so angelegten Quelle ist über `source.read {title}` lesbar (`origin: "import"`, Notiz mit
Datei und Datum), und `highlight.add`/`highlight.check` prüfen Hervorhebungen ohne Zitat dagegen.

### work.import_archive (21b) {#workimport_archive-21b}

`work.import_archive {file_name, content, title?}` → `ImportReport` (wie `work.import`, mit `assets`, `skipped`,
`warnings`). `file_name` muss auf `.zip` enden (sonst -32602), `content` ist das Archiv als Base64 (kein gültiges
Base64 → -32602); die ganze Anfrage liegt innerhalb von `RPC_BODY_LIMIT` (sonst 413 vor dem Aufruf, also ein Archiv bis
etwa 24 MiB – größere über das Formular `POST /werke/import`, bis 80 MiB).

Das Archiv liest `archive::read_zip` mit `IMPORT_LIMITS` (2000 Einträge, je 16 MiB, zusammen 80 MiB, Verhältnis 200):
Links, Spezialdateien, `..`, absolute Pfade, `\`, doppelte Namen, Zip-Bomben und kaputte Archive → -32602 mit einem
deutschen Grund, nichts wird angelegt. Es braucht genau eine Hauptdatei (`.typ`, `.md`, `.markdown`; bei mehreren
die flachste `index.md`, `main.typ` oder `README.md`), sonst -32602 mit den Kandidaten. Bilder (`image("…")` oder
`![…](…)`) liegen relativ zur Hauptdatei; fehlende, zu große, unsichere und Nicht-Bilder stehen mit Grund in
`skipped`, Dateien ohne Verweis als Hinweis `archive-unused`. Titel vergeben → -32000 mit einem Hinweis auf einen anderen
Titel (`title`), dieselbe Hauptdatei ein zweites Mal → -32000 „schon importiert“. Kein MCP-Gegenstück.

```json
{"jsonrpc": "2.0", "id": 1, "method": "work.import_archive",
 "params": {"file_name": "heft.zip", "content": "UEsDBBQAAAAIA…", "title": "Das Heft"}}
```

Seit R21-S1 trägt der Bericht eines Imports (`ImportReport`, Antwort von `work.import`) das Feld `warnings`: eine Liste von
`{code, message, line}` (`line` kann `null` sein) für alles, was nicht wörtlich übernommen wurde; beim Typst-Import
bisher leer.

### Git-Spiegel {#git-mirror}
Nach `revision.accept`, `revision.set_current`, `revision.save`, `revision.update` und `revision.delete` (jeweils mit einer
Änderung) wird der Typst-Ordner `<data>/works/<slug>/` des betroffenen Werks als Commit gespiegelt (jedes Werk
hat seinen eigenen Spiegel). `commit` ist dann die Commit-ID. Schlägt der Spiegel fehl, gilt die Änderung trotzdem; die
Antwort trägt `mirror` mit dem Fehlertext, der nächste Commit holt den Stand nach. Das Schreiben in die DB und der
Commit laufen unter einer gemeinsamen Sperre: Auch gleichzeitige Änderungen erhalten je einen eigenen
Commit, der genau ihre Datei ändert. `revision.reject` und `post.add` committen nicht.

Commit: Betreff `„<Nummer> <Titel>: Vorschlag <rev> übernommen“`, `„…: Revision <rev> gespeichert“`, `„…: auf
Fassung <rev> umgeschaltet“`, `„…: Fassung <rev> geändert“` oder `„…: Fassung <rev> gelöscht“`. Bringt die
umgeschaltete oder übernommene Fassung eine andere Ebene mit, bleibt es **ein** Commit: Betreff mit dem Zusatz
`„ · Ebene a → b“`, Rumpf mit den mitgestuften Punkten und ihren neuen Fassungen („Mitgestuft …: 002 Tiefe:
Fassung 3“) und der Umnummerierung, Trailer `Baruch-Action: switched` (auch beim Übernehmen).
Bei Vorschlägen (und beim Umschalten auf eine Fassung einer Persona) ist der Autor die Persona
(`Name (@handle) <handle@persona.baruch.invalid>`; Zeichen außerhalb von
`[A-Za-z0-9._-]` im Handle werden in der Adresse zu `-`, `@mcp:claude-code` → `mcp-claude-code@…`), Committer der
Mensch (`Autor <autor@baruch.invalid>`), Trailer `Persona`, `Model`. Ändern und Löschen: Autor und Committer
der Mensch, Rumpf mit einem Zeilendiff und der Notiz vorher/nachher oder dem vollen gelöschten Text mit Autor,
Status, Elternteil, Notiz und „Umgehängt: …“; der Commit entsteht auch ohne Änderung an den Dateien (leerer
Log-Commit). Immer: `Baruch-Node`, `Baruch-Revision`; beim Umschalten, Ändern und Löschen
`Baruch-Action: switched|updated|deleted`.

Ein Teil mit Teilmarke (Runde 14) hat wie jeder Abschnitt mit einer aktuellen Fassung eine eigene Datei
`abschnitte/NNN-<titel>.typ` (mit Werkvorlage hinter der `#import`-Zeile); `main.typ` bindet sie in
Baumreihenfolge ein.

### Literatur und Hervorhebungen {#literature-and-highlights}
Methoden aus `crates/baruch-server/src/literature.rs`; alle nehmen `work?`. Hervorhebungen über RPC stammen immer
vom Menschen (Status `active`).

| Methode | Parameter | Antwort |
|---|---|---|
| `literature.render` | `{work?, node?, key?, rev?, view?, lq?, lmode?}` (der Schlüssel geht vor der Position; `lmode` = `fts` \| `rag`, unbekannt = `fts`, Runde 12) | `{html, lq, lmode, url}` – Inhalt des Literatur-Popups (Suchformular, Treffer, Titel mit Zitaten und Hervorhebungen); `url` = `/literatur?…` |
| `highlight.list` | `{title?}` | `{highlights: [Highlight]}` – alle oder die eines Titels; seit Runde 13 trägt jede Hervorhebung `check` (Prüfung gegen den Volltext, siehe unten) oder `null` |
| `highlight.add` | `{title, quote?, locator?, start?, end?, text, kind, note?}` | `{highlight, check}` – `kind` aus `quote`, `core`, `contradiction`, `check`; `start`/`end` beide oder keines, ab 0, in Unicode-Zeichen; mit `quote` endet der Bereich innerhalb des Zitats, und `text` ist der Ausschnitt `start..end`, wobei Leerraum nicht zählt (Folgen von Unicode-Leerraum wie Zeilenumbruch, Tabulator, geschütztes Leerzeichen zählen als ein Leerzeichen, Ränder entfallen); gespeichert wird dann der genaue Ausschnitt des Zitats; inhaltlich abweichender Text → -32602; ohne `locator` die Stelle des Zitats. **Runde 13:** Ohne `quote`, mit `start`/`end` prüft Baruch den Bereich gegen den Volltext der Quelle (nur bei einem mit MultiGPT verknüpften Titel): `check.result` = `found` („im Volltext gefunden“, der Text steht genau in `start..end`, Leerraum zählt nicht), `not_found` („im Volltext nicht gefunden“) oder `unverifiable` („nicht prüfbar“: keine Verknüpfung, MultiGPT nicht erreichbar – `message` nennt den Grund). Die Hervorhebung wird in jedem Fall gespeichert; ohne Bereich oder mit einem Zitat ist `check` `null` |
| `highlight.check` | `{highlight}` | `{highlight, check: {result, label, message, checked}}` – prüft erneut gegen den Volltext (Runde 13); nur Hervorhebungen ohne Zitat mit einem Bereich, sonst -32602 |
| `highlight.update` | `{highlight, kind?, note?, status?}` | `{highlight}` – `status` aus `active`, `proposed`, `rejected`; Hervorhebungen von Personas werden nicht `active` (-32000) |
| `highlight.delete` | `{highlight}` | `{deleted}` |
| `highlight.cite` | `{highlight, key}` | `{node, key, rev}` – Vorschlagsfassung: aktueller Text + `#quote(block: true, attribution: [@key[Stelle]])[Text]` (maskiert), Autor `@literatur`, Modell `baruch-zitat`, Notiz „Zitat aus @key“; die aktuelle Fassung bleibt. `key` ist der feste Schlüssel des Abschnitts (Pflicht, unter der Schreibsperre aufgelöst); `node` → -32602 |
| `highlight.own` | `{highlight}` | Runde 15: `{highlight, original}` – übernimmt eine Hervorhebung einer Persona als **eigene**: eine neue Hervorhebung des Menschen (`active`) mit Text, Bereich, Zitat, Stelle und Art des Originals, Notiz „übernommen von @x (Hervorhebung n)“; das Original bleibt ein Vorschlag. Eigene Hervorhebung oder bereits übernommen → -32000. Nicht über MCP |
| `literature.to_persona` | `{work?, key, persona?, text, source?, title?, highlight?, question?}` | Runde 15 („an Persona geben“): wie `persona.ask` – die Frage (Standard „Was sagt dieser Text zu meinem Abschnitt?“) mit dem markierten Text und der Quelle wird ein Beitrag des Menschen, die Antwort der Persona (Standard der Lektor) ein Beitrag mit der Quelle `source` (mit `highlight` dazu „Hervorhebung n“); Antwort wie `persona.ask` (`post`, `question_post`, `model`, `compressed`, `url`, …). `text` 1–4000 Zeichen, `source` eine Zeile ≤ 300 Zeichen (leer = „Literatur“); ohne `key` → -32602 „Abschnitt wählen …“, `node` → -32602. Nichts im Buchtext |
| `source.read` | `{work?, title? \| document, collection?, part? = 1, q?}` (Runde 23: `q` höchstens 500 Zeichen) | Runde 13: Teil des Volltexts durch MultiGPT `read_document`: `{source: {title, url, document, collection, title_id}, part, parts, more, text, marks: [[start, end]], origin: "multigpt" \| "lokal", message?, offset, paged_by_remote}`. `title` = Titel mit Verknüpfung (ohne → -32602), sonst `document` (Dokument-ID, 1–500 Zeichen). Kennt das Schema von `read_document` einen Seitenparameter (`page` oder ähnlich) und nennt die Antwort Seiten, liest Baruch seitenweise (`parts` kann `null` sein = Ende unbekannt); sonst teilt Baruch den Volltext an Absatzgrenzen in Teile von höchstens 20 000 Zeichen (`offset` = erstes Zeichen des Teils im Volltext). `marks`: Vorkommen von `q` im Teil (ohne Beachtung der Groß-/Kleinschreibung; kommt `q` nicht vor, seine Wörter ab drei Zeichen). Antworten über 4 MiB, Zeitlimit 20 s. Misserfolg → -32000 „Der Volltext ist gerade nicht abrufbar: …“; mit einer lokalen Kopie aus dem Abgleich stattdessen `origin: "lokal"` und `message` „… lokale Kopie vom …“. Inhalte sind Daten |
| `source.adopt` | `{document, collection?}` | `{title: TitleEntry, remote: {way, collection, document}}` – Runde 13: übernimmt ein Dokument aus MultiGPT in die Bibliothek des Autors (Titel und Adresse aus `document_info`, sonst die Dokument-ID; Zitierschlüssel `mg-…`, Art `other`, Notiz „Aus MultiGPT übernommen …“) und verknüpft es (`library_title_remote`); bereits übernommen → derselbe Titel. Danach holt der Index den gemeinsamen Bereich nach. Nur der Mensch (kein MCP-Werkzeug) |

`Highlight`: `{id, title_id, quote_id, locator, start, end, text, kind, note, author, status, created}`,
`author` = `{kind: "human", name}` oder `{kind: "persona", handle, model}`. Fehler: leerer Text, ungültiger
Bereich, unbekannte Art, unbekannter Titel, unbekannte Hervorhebung oder ein Zitat eines anderen Titels → -32602.

Ohne JavaScript: `POST /literatur/act` (hinter demselben Schutz wie `/act`, fremder `Origin` → 403) mit
`action` = `highlight`, `cite`, `persona`, `own`, `similar`, `update`, `delete`, seit Runde 13 `adopt` (Felder `document`,
`collection`; 303 auf `/quelle?title=…&notice=adopted`) und `check` (Feld `highlight`; 303 mit `notice=checked`), und
den Feldern `work`, `node`, `key`,
`rev`, `lq`, `title`, `quote`, `highlight`, `locator`, `start`, `end`, `text`, `kind`, `note`, `status`,
`confirm` (beim Löschen erforderlich); danach 303 auf `/quelle?…`, `/literatur?…` oder, beim Zitieren, auf den
Vorschlag `/?work&node&rev`. `cite` braucht den festen Schlüssel `key` des Abschnitts;
fehlt er, antwortet der Server mit 422 „Abschnitt fehlt.“, gibt es ihn nicht mehr, mit 409 – in beiden Fällen ohne
Hervorhebung und ohne Vorschlag. `persona` (Runde 15, Felder `persona`, `question`, `source?`, sonst die Quelle aus
Titel und Stelle `@key, S. 3`) wie `literature.to_persona`, danach 303 zum Abschnitt; ohne `key` oder bei
einem Fehler die Hinweisseite (Status 200, `assist_notice.html`) mit Meldung und einem Link zurück. `own` (Runde 15,
Feld `highlight`) wie `highlight.own`, 303 auf `/quelle?…&notice=owned`. Fehler: 422
(ungültig), 409 (abgelehnt). Seiten: `GET /literatur?work&node&key&rev&lq`,
`GET /quelle?work&node&key&title&quote?&highlight?&lq&teil?`; beide tragen den Schlüssel selbst in ihren Links und
Formularen (aus `key`, sonst beim Laden aus `node` ermittelt), `key` geht vor `node`. `/quelle` zeigt für
einen mit MultiGPT verknüpften Titel den Volltext (Teil `teil`, Anker `#volltext`). Runde 13:
`GET /quelle/multigpt?work&key&rev&lq&lmode&collection&document&section&teil&q` – Volltext eines Dokuments aus
MultiGPT, Teil für Teil mit Links „vorheriger/nächster Teil“, Vorkommen `q` als `<mark>`, Schaltfläche „In die Bibliothek
übernehmen“; ohne JavaScript. Von einer fremden Seite (`Sec-Fetch-Site`/`Origin`) lädt keine der beiden
Seiten den Volltext. Treffer von MultiGPT in der Bedeutungssuche (`literature::rag_view`) tragen `fulltext_link`
(`/quelle/multigpt?collection=…&document=…[&section=…]&q=…`).

**Bibeln in der Literatur (Runde 15):** keine eigenen RPC-Methoden; `literature.render` und `/literatur` tragen den
Kasten „Bibeln“ (Bibelmodule aus `bible.modules`, Einträge ohne `kind` zählen als Bibel, die Bibel des Werks markiert
und zuerst) und im Wortlautmodus mit einer Suche die Treffergruppen „Bibel“ (je Modul, höchstens 10 Treffer, höchstens
3 Module, Zeitlimit 3 s, durch `bibles::search`) und „Kommentar“ (`bibles::commentary_hits_html`). Die
Modulordner werden nur gelesen, wenn sie konfiguriert sind oder das Werk eine Bibel hat. Seiten ohne JavaScript:
`GET /literatur/bibel?work&key&rev&lq&lmode&module&ref&book&chapter&q&kom` (Bücher, Blättern durch Kapitel, Öffnen der Stelle
`ref`, Wortlaut `q`, neben der Stelle das Kommentarfeld `bibles::commentary_panel_html`, `kom=1`
klappt es auf; ohne Bibel ein Hinweis mit einem Link auf `/einstellungen/quellen`) und `POST /literatur/bibel/act` (hinter
`same_origin`) mit `action=cite` (Bibelzitat als Vorschlagsfassung: aktueller Text + `#quote(block: true,
attribution: [Ps 139,23–24 (Modul)])[…]`, Autor `@literatur`, Modell `baruch-zitat`, Notiz „Bibelzitat …“; 303 auf den
Vorschlag) oder `action=persona` (Verse an `persona` mit `question`, Quelle „Bibel MODUL, Stelle“, 303 zum
Abschnitt) und den Feldern `work`, `key` (erforderlich, sonst Hinweisseite „Abschnitt wählen“), `module`, `ref`. Der
Bibeltext stammt immer aus dem Modul, nie aus dem Formular. Ein `action=highlight` gibt es nicht (Hinweisseite: Hervorheben
nur für Titel der Bibliothek).

### Satzansicht {#typeset-view}

Nur lesend, nur über `POST /rpc` (nicht `/act`, nicht MCP), ohne die Schreibsperre; beantwortet von
`src/wysiwyg.rs`. Blöcke, Anweisungen und HTML: `docs/WYSIWYG.md`.

| Methode | Parameter | Antwort |
|---|---|---|
| `wysiwyg.parse` | `{work?, body}` | `{blocks, html}` – Blöcke `{kind, …, raw}` (Verkettung der `raw` = `body`, byte-identisch) und HTML der Satzansicht (`div.ws-block[data-b][data-kind]`, Inseln `contenteditable="false"`); Überschrift mit `label` (Label in der Zeile danach), Szene/Arbeit mit `args` (Argumente wörtlich) und `body`; Runde 14: Teil `{kind: "part", title}` (`div.ws-block.ws-part[data-kind=part][data-level=0]`) und „ohne Nummer“ `{kind: "unnumbered", level, title}` (`div.ws-block.ws-unnumbered[data-kind=unnumbered][data-level]`), Titel als reiner Text; Inseln mit einem Bildpfad erscheinen als Bild: `div.ws-block.ws-figure[data-kind=island][data-image=1]` mit `<img class="ws-image" src="/werke/{work}/bilder/pfad?p=…" alt="…" loading="eager">` (Alternativtext der aktuellen Fassung aus „Images“, sonst leer) und `span.ws-caption`, inline `span.ws-island.ws-inline-image`; Rohtext nur in `data-raw`; Pfade in Normalform (`./` und seit Runde 18b auch ein führendes `/` entfallen: `p=bilder%2Fa.png`); Runde 18b: alle Bilder einer Insel (`image_paths`), neue Arten `enum` (`{start, items, depths?}`, `<ol start>`) und `terms` (`{items: [{term, desc}]}`, `<dl>`), Listen mit `depths` (verschachtelt), Inlines `linebreak`, `link` (`{url, c?}`, `a.ws-link`) und `ref` (`{target, supplement?}`, `button.ws-ref`, nicht editierbar); Formen in `docs/WYSIWYG.md`; Runde 39b: Ein Querverweis mit Seitenzahl (`, S.~…` oder `, p.~…`, beide Sprachen gelesen) zeigt die Seitenabkürzung der Werksprache in `span.ws-xref-page` |
| `wysiwyg.serialize` | `{work?, body, edits}` | `{body}` – neuer Text aus dem ursprünglichen `body` und den Anweisungen `edits` (`{keep: i}` wörtlich, `{replaces: i\|null, block}` neu erzeugt, `{replaces: i, body: [Anweisungen]}` nur im Rumpf einer Szene/Arbeit, Runde 18b `{replaces: i, raw: "<Typst>"}` Quelltext des Blocks `i` wörtlich – Insel an Ort und Stelle, leer = gelöscht); unbekannter oder doppelter Index, neue Insel (auch `raw` ohne `replaces`) oder eine als Block bearbeitete Insel, Rumpfanweisung für einen Block ohne Rumpf, neuer oder umgewandelter Teil (Runde 18b: „Einen Teil legt man in der Gliederung an, nicht im Satzbild.“), falsche Form → -32602 mit einer deutschen Meldung. Eine Überschrift endet am ersten Zeilenumbruch (`{"t": "soft"}` oder ein Zeilenende im Text); der Rest wird danach ein eigener Absatz. Ein geänderter Teil wird mit `part_line` neu aufgebaut, „ohne Nummer“ mit `unnumbered_line` (ersetzt es eine solche Marke, bleibt ihr Kommentar `// …`). Runde 39b: Neu erzeugte Blöcke werden in der Werksprache geschrieben – ein Querverweis `{t: "xref", page: true}` erhält in einem englischen Werk `, p.~…`, in einem deutschen `, S.~…` |
| `wysiwyg.style` | `{work?}` | `{css, style}` – Erscheinung aus der Werkvorlage: `style` = `{font, size, fill, emph_fill, strong_fill, heading_font, heading_fill, indent, leading, justify, width, scene, work}` (fehlend = `null`), `css` = Variablen unter `.ws-sheet`; nur Regeln der obersten Ebene (nicht in Show-Regeln, `#show: …`, Funktionsrümpfen), mehrere → die letzte |
| `wysiwyg.diff` | `{work?, key\|node, rev, base?}` | `{html, base}` – Fassung `rev` gegen `base` in der Satzansicht (`div.ws-same`, `div.ws-del`, `div.ws-ins`); fehlt `base`, gilt ihr Elternteil (`based_on`), sonst die aktuelle Fassung; unbekannte Fassung → -32602; Bilder in Inseln wie bei `wysiwyg.parse` (seit Runde 18b alle Bilder einer Insel, Adresse ohne führendes `/`). Der Editor nutzt die Methode seit Runde 7 nicht mehr (der Satzansicht-Diff der Vorschläge kommt fertig im HTML); sie bleibt für andere Aufrufer |

### Bilder {#images}

Alle Methoden nehmen `work?`. Schreibende laufen unter der Schreibsperre und tragen `commit` oder
`mirror`; es gibt sie weder über `/act` noch über MCP.

| Methode | Parameter | Ergebnis |
|---|---|---|
| `asset.list` | – | `{assets: [AssetSummary]}` (`name, mime, encoding, bytes, current, revisions, caption, alt, paths, updated`) |
| `asset.get` | `name, rev?, content? = true` | `{asset, revision: AssetRevisionInfo, encoding, content?}` |
| `asset.revisions` | `name` | `{revisions: [AssetRevisionInfo]}` (`number, based_on, current, mime, encoding, bytes, sha256, caption, alt, author, note, created, edited`) |
| `asset.add` | `file_name, content, encoding: "base64"\|"svg", name?, path?, caption?, alt?, note?` | `{name, rev}` |
| `asset.replace` | `name, file_name, content, encoding, note?, rename?: bool = false` | `{name, rev, renamed_from?}` (neue Fassung, sofort aktuell). Runde 21c: Ein anderes Format als die Endung des Namens nur mit `rename: true` (sonst -32602 mit dem Hinweis „Endung anpassen“); dann erhält das Bild die Standardendung des neuen Formats (`haus.svg` → `haus.png`, vergeben → `haus-2.png`), `name` ist der neue Name, `renamed_from` der alte; alle Pfade bleiben beim Bild, offene Anträge nehmen den Namen mit; ein Commit |
| `asset.set_current` | `name, rev` | `{name, rev}` |
| `asset.update` | `name, rev, caption?, alt?, note?, paths?` | `{name, rev, changed}` (Felder und Pfade zusammen ein Commit) |
| `asset.delete` | `name, rev?, confirm: true` | `{name, rev?, deleted}` (ohne `rev` das ganze Bild) |
| `asset.path_conflicts` | – | `{conflicts: [{normal, paths: [{path, asset}]}]}` (Runde 21c) – Altbestand: Pfade, die in Normalform gleich sind (`/x.png`, `x.png`, `./x.png`), aber zu verschiedenen Bildern gehören; nach Normalform, Schreibweisen nach Pfad |
| `asset.path_resolve` | `normal, asset` | `{name, rev, commit?}` (Runde 21c) – nur der Mensch: Alle Schreibweisen von `normal` gehören danach zum Bild `asset`, die anderen Bilder verlieren nur diesen Pfad; ein Commit „Bild <asset>: …“ mit „Pfad-Kollision … aufgelöst“. Keine Kollision → -32000; `asset` hat keine der Schreibweisen → -32602 |

| `asset.search` | `query` | `{assets: [AssetSummary]}` (Runde 11) – Name, Beschriftung, Alternativtext der aktuellen Fassung, ohne Beachtung der Groß-/Kleinschreibung, `%`/`_` wörtlich, leer = alle; fehlendes `query` → -32602 |
| `asset.request_list` | `open?: bool = true` | `{work, requests: [AssetRequest]}` (Runde 11) – Anträge der Assistenten |
| `asset.request_decide` | `id, decision: "confirm"\|"reject"` | `{request, commit?, mirror?, cover?}` (Runde 11) – nur der Mensch |

Fehler: ohne `confirm: true` → -32602; unbekanntes Bild, unbekannte Fassung, zu groß (8 MiB), kein
unterstütztes Bild, kaputtes Base64, ungültiger Name oder Pfad → -32602; doppelter Name, unverändert ersetzt,
bereits aktuell, Löschen der aktuellen Fassung → -32000.

**Anträge für Bilder (Runde 11, Migration 0018):** `AssetRequest` = `{id, work, op: "replace"|"set_cover"|
"delete", name, …, origin {kind, handle}, origin_label, note, created, decided, decision: "confirmed"|"rejected"|
"stale", decision_label, decided_by, message, summary, url, content_url?}`; bei `replace` `mime, encoding, bytes,
caption, alt, expected_sha256` (ohne Inhalt; der steht unter `content_url` =
`/werke/{id}/bilder/antraege/{nr}/datei`), bei `delete` `rev, expected_sha256`, bei `set_cover`
`expected_cover`. `asset.request_decide` führt bei Bestätigung aus (Ersetzen/Löschen mit Commit „… (Antrag n)“,
Trailer `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`; Umschlag durch `work.set_cover` plus
Log-Commit „Bildantrag n bestätigt“, `cover` = dessen Antwort). Passt `expected_sha256` oder
`expected_cover` nicht mehr → `decision: "stale"` mit `message`, kein Fehler. Bereits entschieden → -32000.
Formular `POST /werke/bilder/antraege/act` (`work, id, decision=confirm|reject`) → 303
`/werke/{w}/bilder?hinweis=antrag_bestaetigt|antrag_abgelehnt|antrag_veraltet#antraege`.

**Runde 21c:** `AssetRequest` hat zusätzlich `rename` (Antrag, mit Typwechsel zu ersetzen; `summary` endet dann auf „– mit
neuer Endung“, das Bestätigen benennt wie `asset.replace` mit `rename` um) und `purged`. Vor jedem `asset.request_list`,
`asset.request_decide`, dem Abschnitt „Anträge“ der Seite „Bilder“ und der Antragsdatei entfernt Baruch den Inhalt
entschiedener Ersetzungsanträge, deren Entscheidung mehr als 30 Tage zurückliegt (`baruch_store::ASSET_REQUEST_PURGE_DAYS`)
(`purged: true`, `bytes` bleibt die Größe davor, Entscheidung und Herkunft bleiben).
`GET /werke/{id}/bilder/antraege/{nr}/datei` liefert den Inhalt mit MIME, `X-Content-Type-Options: nosniff` und
`Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox`; nach der Bereinigung 410 Gone mit
einer Meldung (Text). Formulare: Ersetzen (`POST /werke/bilder/hochladen`) mit Feld `rename=1` („Endung anpassen“; Erfolg
mit neuem Namen → 303 `/werke/{w}/bilder/<neu>?hinweis=endung_angepasst`); `POST /werke/bilder/act` mit
`action=resolve_path`, `normal`, `name` (das Bild, das den Pfad behält) → 303 `/werke/{w}/bilder?hinweis=pfad_zugeordnet`,
Fehler → Seite „Bilder“ mit einer Meldung (422/409).

Seiten (Runde 11): `GET /werke/{id}/bilder?q=<Suche>`; `GET /werke/{id}/bilder/{name}/vorschau?b=<px>[&fassung=n]`
(Raster verkleinert, längste Seite ≤ b, JPEG bleibt JPEG, sonst PNG; `b` 64–1024, Standard 320, sonst 400;
`ETag`, `If-None-Match` → 304, `nosniff`, CSP `sandbox`; SVG unverändert wie `/datei`); `POST
/werke/bilder/einfuegen` (`work, key, rev` = Basis, `name, caption?, place` (seit Runde 18c auch `start`, `block:<n>`,
`offset:<n>` und `body`, siehe „work.sheet_set“) → neue Fassung des Menschen mit
`#figure(image("<Pfad>"), caption: [...])`, fehlender Pfad `bilder/<name>`, ein Commit; 303 `/?work=W&key=K`,
422 Eingabe, 409 abgelehnt). `wysiwyg.parse`/`wysiwyg.diff` zeigen Bilder ohne Eintrag als „Bild fehlt: <Pfad>“
(`ws-missing`).

`/werke/{id}/bilder/pfad?p=` findet ein Bild unter jeder Schreibweise seines Pfads (`/bilder/a.png`,
`bilder/a.png`, `./bilder/a.png`), nie mit `..`.

### Personas {#personas}

Personas gelten für alle Werke; die Verwaltungsmethoden nehmen kein `work`. Nur über `/rpc` (nicht `/act`,
nicht MCP); die Aufgaben (`ask`/`review`/`propose`/`chat`) laufen auch über `POST /assistenten/act` und
`/assistenten/stream` (siehe „Tasks for personas“).

| Methode | Parameter | Ergebnis |
|---|---|---|
| `persona.list` | – | `{personas: [{handle, name, role, model, context_length, active, lector, color, chosen, prompt_custom, tints: {light, dark, line, line_dark}, connection: {id, name, kind}, locality: "lokal"\|"extern", external_allowed, context: {tokens, reported, warning}}], palette: ["#rrggbb", …], lector, lector_mode, chat_scope}`; vollständig auch ohne jedes Werk. Runde 15b: `connection` = Modellverbindung der Persona (Standard Eliezer), `context` ohne Netzabfrage aus den Zwischenspeichern (ohne Meldung ist `tokens` = Einstellung). Runde 16a: zusätzlich `chat_scope` (`"chapter"`\|`"section"`, siehe `persona.chat_scope_get`); bei „automatisch nach Kontext“ vergleicht `lector` die wirksame Kontextlänge (`ModelPort::context`), alle anderen Felder unverändert. Seit Runde 17a auch `lector_context` (wirksame Kontextlänge des Lektors `{tokens, reported, warning}` oder `null`); Lektor und `context` ohne Netzabfrage (Zwischenspeicher, sonst die Einstellung) – nur ein Modellaufruf fragt die Gegenseite |
| `persona.chat_scope_get` | `{}` | `{scope: "chapter"\|"section"}` (Runde 16a) – was der Abschnitts-Chat mitgibt: `chapter` (Standard) das ganze Kapitel oder die ganze Predigt, `section` nur den Abschnitt, dazu Titel und Gliederung des Werks. Gespeichert in `store_meta` (`chat_scope`), nicht im Spiegel |
| `persona.chat_scope_set` | `{scope}` | dito; anderer Wert → -32602. Nur der Mensch: nicht über `POST /act`, nicht über MCP. Formular `POST /einstellungen/personas/act` mit `method=persona.chat_scope_set`, `scope=chapter\|section`, `work?` → 303 `/einstellungen/assistenten?ok=chat[&work=W]#chat` |
| `persona.backend_get` | `{persona}` | `{persona, connection: {id, name, kind, kind_title, external}, locality, external_allowed, explicit, usable, warning}` (Runde 15b) – `explicit: false` = keine Zeile in `persona_backend` (Standard: Eliezer, nicht erlaubt); `usable: false` = externe Verbindung ohne Freigabe (dann `warning` als Satz) |
| `persona.backend_set` | `{persona, connection: id, external_allowed?: bool = false}` | wie `persona.backend_get`; unbekannte Persona oder Verbindung → -32602. Nur der Mensch. Formular `POST /einstellungen/personas/act` mit `action=backend`, `handle`, `connection`, `external_allowed=1` (fehlt = aus) → 303 `…?persona=<h>&ok=verbindung#persona-<h>` |
| `persona.bible_get` | `{persona}` | `{persona, bible_context, allowed, default}` (Runde 15) – Bibelbelege aus SWORD im Kontext (`crate::bible_research`); `allowed` = @belege oder der Lektor je nach Wahl; ohne Zeile gilt `default` (@belege an, Lektor aus) |
| `persona.bible_set` | `{persona, on: bool}` | `{persona, bible_context}`; nur für @belege und den Lektor, sonst -32602 „Bibel-Belege aus SWORD gibt es nur für @belege und den Lektor.“ Tabelle `persona_bible` (Migration 0037). Formular `action=bible`, `handle`, `on=1` → 303 `…&ok=bibel` |
| `persona.set_color` | `handle` (mit oder ohne `@`), `color`: `"#rrggbb"`/`"#rgb"` oder `null` (Standard) | `{handle, color, chosen}` |
| `persona.update` | `handle`, `name?` (Runde 11: getrimmt 1–80 Zeichen, eine Zeile), `role?`, `model?`, `context_length?`, `active?` | `PersonaRow {handle, name, role, model, context_length, active, color, chosen, prompt}`; Rolle/Modell getrimmt 1–200 Zeichen, Kontext 1–10 000 000, `active` ein Wahrheitswert, unbekannte Persona → -32602 |
| `persona.prompt_get` | `{handle}` | `{handle, prompt, default, custom}` – wirksamer Rollen-Prompt, Standard aus dem Code, eigener ja/nein |
| `persona.set_prompt` | `{handle, prompt: Text \| null}` | dito; `null` = Standard; getrimmt, leer oder über 8000 Zeichen → -32000 (Migration 0015, Spalte `persona.prompt`) |
| `persona.lector_get` | `{}` | `{mode: "fixed"\|"auto", handle, lector}` |
| `persona.set_lector` | `{mode, handle?}` | dito; unbekannte Persona, falscher Modus oder `fixed` ohne `handle` → -32602; gespeichert in `store_meta` (`lector`), ohne Zeile gilt fest `@lektor` |
| `persona.rag_get` | `{persona}` | `{persona, rag_context, allowed}` (Runde 12) – `allowed` = @belege oder der Lektor je nach gespeicherter Wahl (fest oder automatisch); unbekannte Persona → -32602 |
| `persona.rag_set` | `{persona, on: bool}` | `{persona, rag_context}`; nur für @belege und den Lektor, sonst -32602 „Bedeutungs-Treffer gibt es nur für @belege und den Lektor.“; `on` kein Wahrheitswert → -32602. Tabelle `persona_rag` (Migration 0032), kein Commit im Spiegel. Formular `POST /einstellungen/personas/act` mit `action=rag`, `handle`, `on=1` (fehlt = aus), `work?` → 303 `/einstellungen/assistenten?persona=<h>&ok=rag[&work=W]#persona-<h>`, abgelehnt → 422 |

- `color` ist die wirksame Farbe (`chosen: false` = Standard aus `palette` in der Reihenfolge der Personas).
- `tints.light`/`tints.dark`: Hintergründe, auf denen Text, abgeschwächter Text und Linktext des hellen
  (`#1d2125`, `#5f6b76`, `#3b5bdb`) beziehungsweise dunklen (`#e3e6ea`, `#9aa5b1`, `#7d97ff`) Themas mindestens
  4,5 : 1 erreichen; `tints.line` = Linie im hellen Thema mit mindestens 3 : 1 gegen `#ffffff`,
  `tints.line_dark` = Linie im dunklen Thema mit mindestens 3 : 1 gegen `#1f2328` (WCAG 1.4.11; die Farbe
  selbst, bei Bedarf abgedunkelt oder aufgehellt).
- `role`, `model`, `context_length`, `active`, `lector` stammen seit Runde 9 aus der Tabelle `persona` (nicht
  mehr aus dem Standardwerk), vollständig auch ohne Werke; `lector` folgt der Wahl (`LectorChoice`): fest eine
  Persona (Standard `@lektor`, Versuch seit 2026-11-06); ist sie inaktiv, oder bei „automatisch“, die aktive
  Persona mit dem größten Kontext (bei Gleichstand die frühere; seit Runde 16a die wirksame Länge, seit Runde 17a auf
  Seiten und in Listen ohne Netzabfrage: was der Zwischenspeicher weiß, sonst die Einstellung – nur ein Modellaufruf
  fragt die Gegenseite). `work.get` trägt `work.lector_choice`
  (`{mode, handle}`). Seit Runde 10 geht das Modell an Eliezer; die Kontextlänge dient nur zum Kürzen in Baruch.

#### Aufgaben für Personas (Runde 10) {#tasks-for-personas-round-10}

Adressierung immer über `key` (`node` → -32602), fehlendes `work` → Standardwerk; Text (Frage, Hinweis, Anweisung,
Nachricht) höchstens 4000 Zeichen; inaktive oder unbekannte Persona → -32602; Fehler des Modells → -32000 mit einer
ehrlichen Meldung, dann wird nichts gespeichert. Das Modell wird **ohne** die Schreibsperre aufgerufen; das Schreiben
geschieht danach unter `write_lock` (Beitrag durch `Store::add_post`, Vorschlag durch `Store::add_proposal`).

| Methode | Parameter | Ergebnis |
|---|---|---|
| `persona.ask` | `{work?, key, persona, question, rev?, scope?}` | `{post: {id, author, body}, question_post: {id, author, body}, model, truncated, cut, compressed, url, key, work, persona}`; die Frage wird zuerst ein Beitrag des Menschen |
| `persona.review` | `{work?, key, persona?, rev?, question?, scope?}` | wie `ask` ohne Fragebeitrag (`question_post: null`); fehlendes `persona` → der Lektor |
| `persona.propose` | `{work?, key, persona?, instruction?, rev?, scope?}` | `{rev, status: "proposed", author, model, truncated, compressed, proposals, url, key, work, basis}` (`basis: {rev, label, proposal}` seit Runde 36: die Fassung, auf der der Vorschlag beruht, Kopfzeile „Grundlage: …“); Überschriftzeile der gezeigten Fassung erzwungen; gleich der aktuellen → -32000 |
| `persona.chat` | `{work?, key, message, rev?}` | `{posts: [{id, author, body}], errors: [{persona, message}], url, truncated, cut, compressed}` (Runde 23: dazu `done_proposals?`, `done_notes?`, `all_done?`; Zitatzeilen „> …“ rufen niemanden): Beitrag des Menschen, dann für jede erwähnte aktive Persona (`@handle`, höchstens drei, ohne Duplikate, nacheinander) ihre Antwort; `compressed` = die erste Komprimierung einer Antwort oder `null` (Runde 15) |

`model` ist das Modell, das tatsächlich geantwortet hat; `truncated: true`, wenn der Kontext gekürzt wurde (der Beitrag
endet dann mit „(Kontext gekürzt)“). `cut: true`, wenn das Modell an der Längengrenze anhielt (`finish_reason`
„length“): Der Beitrag endet dann mit „… (Antwort abgeschnitten: Längengrenze erreicht)“; ein so abgeschnittener
**Vorschlag** entsteht nicht (-32000, nichts gespeichert). `max_tokens`: ask 700, review 900, chat 500, propose
min(4096, Abschnitts-Tokens·1,5 + 200); `temperature` 0,4.

**Umfang, ganzes Kapitel und Komprimierung (Runde 15, „lektor“):**
- Kontext jeder Aufgabe: das ganze Kapitel oder, bei einer Predigt, die ganze Predigt im Wortlaut („Chapter surroundings“,
  der Abschnitt der Aufgabe steht darin in der gezeigten Fassung), die Befunde der anderen Personas zu Teilen
  des Kapitels (je Teil die letzten drei) und – wie bisher – Gedächtnis, Chatverlauf und Faktenblöcke.
- `scope` (optional, bei `persona.ask|review|propose`): `"section"` (Standard; fehlend oder leer) prüft den Abschnitt,
  `"chapter"` das ganze Kapitel (`max_tokens` ask 900, review 1500). Bei einem Vorschlag mit `"chapter"` schreibt die
  Persona für jeden Teil mit Text nacheinander eine neue Fassung; jeder geänderte Teil erhält genau ein `proposed`
  (Notiz „Vorschlag von @x (ganzes Kapitel)“), unveränderte und an der Längengrenze abgeschnittene entfallen.
  `proposals: [{key, rev, label}]` nennt alle, `rev`/`url` zeigen auf den ersten; ohne einen einzigen → -32000.
  Andere Werte → -32602. `persona.chat` kennt kein `scope` (immer der Abschnitt).
- Budget: wirksame Kontextlänge nach dem Modellport (Einstellung der Persona, mit der Gegenseite abgeglichen) −
  `max_tokens` − 256, Zeichen ≈ Tokens·3. Passt nicht alles, werden zuerst Zusätze gekürzt (Faktenblöcke, Befunde,
  Gedächtnis, Chat; nur bis auf ein Drittel des Budgets, `truncated: true`, Beitrag mit „(Kontext gekürzt)“).
  Kapitel und Prüfgegenstand werden nie gekürzt: Passen sie nicht, wird der Prüfgegenstand in überlappende Stücke zerlegt
  (jedes in seinem Wortlaut geprüft), das Modell fasst die übrigen Teile zuerst nacheinander zusammen (Bezeichnung des
  Aufrufs „… Zusammenfassung“), was auch dann nicht passt, steht nur mit seinem Titel. Die Befunde der Stücke werden
  ohne Duplikate zusammengeführt.
- `compressed`: `null` oder `{message, summarized, chunks}` – `message` beginnt mit „Komprimiert:“, `summarized`
  nennt die Teile, die die Persona nur als Zusammenfassung sah („1.1 Ausflug“, „Zeichen 1–4000 von Abschnitt …“;
  ganz ausgelassene mit „(ausgelassen)“), `chunks` die Zahl der Prüfstücke (1 = nicht zerlegt). Der Beitrag endet dann
  mit dem Absatz `message`; bei einem Vorschlag steht er in der Notiz (nie im Buchtext). Ein Vorschlag braucht den
  Abschnitt ganz im Wortlaut; passt er nicht → -32000 („zu lang …; Nichts angelegt“).
- „Kritisch:“: Fehlt es bei `@lektor`, `@exeget` oder `@nathan` (Frage, Review, Chat), wird es je Aufgabe einmal nachgefordert
  (Stream: Ereignis `retry {persona, reason: "fehlendes „Kritisch:“"}`); fehlt es weiterhin, endet der
  Beitrag mit `„(Hinweis von Baruch: Diese Antwort nennt keinen mit „Kritisch:“ markierten Befund, auch nicht nach
  einer Nachforderung.)“`. In einem Vorschlag entfallen Zeilen, die mit „Kritisch:“ beginnen; verbotene Einleitungen
  werden bei einem Vorschlag nicht geprüft (der erste Satz ist Buchtext).
- Belege aus SWORD: Erhält die Persona Belege (Entscheidung in `bible_research`, Einstellung je Persona),
  steht der Block „Evidence from SWORD“ mit `[S1]…[Sn]` (Art, Modul, Stelle oder Stichwort, Lizenzhinweis) vor dem Kapitel;
  jedes `[Sn]` der Antwort ohne Eintrag erhält „(unbekannte Quelle)“.

Formulare und Stream (hinter `guard::same_origin`):
- `POST /assistenten/act` (Formular, ohne JavaScript): `method` ∈ `persona.ask|review|propose|chat`, `work`,
  `key`, `persona`, `question`/`message`/`instruction`, `rev`, `view`, `lq`, seit Runde 15 `scope`
  (`section`|`chapter`, Feld aus `fragments/assist_scope.html`, geht unverändert an die RPC) → nach der fertigen
  Antwort 303 auf die Schlüsseladresse (`/?work=W&key=K[&rev=R]…`, bei einem Vorschlag mit seinem `rev`). **Runde 15:** Fehler
  zeigen statt 422 die Hinweisseite `assist_notice.html` (Status 200, `base.html`, Meldung und ein Link zurück zum
  Abschnitt); scheitert im Chat eine Persona, nennt die Seite eine Zeile je Persona („Dein Beitrag und die fertigen
  Antworten sind gespeichert …“). Trägt das Ergebnis `compressed`, leitet das Formular nicht stillschweigend um, sondern
  zeigt dieselbe Seite mit der Toast-Komponente (`fragments/toast.html`) und dem Link „Weiter zum Abschnitt“.
- `POST /assistenten/stream` (`Content-Type: application/json`, Rumpf `{method, …}` wie RPC) →
  `text/event-stream` (`Cache-Control: no-store`) mit `start {persona, style}`, `delta {text}`, seit Runde 15
  `posted {post: {id, author, body}}` (Chat: Der Beitrag des Absenders wird gespeichert, kommt vor dem ersten `delta`),
  `retry {persona, reason}` (die Antwort wird erneut angefordert, z. B. „verbotene Einleitung“; verwirf, was von dieser Persona gezeigt wurde
  – seit Runde 18b bei zerlegten Aufgaben nur das des laufenden Stücks), `compressed {message,
  summarized, chunks}` (Kontext zerlegt), seit Runde 18b `piece {persona, index, count}` (das nächste Stück einer
  zerlegten Aufgabe beginnt), dann
  `done {kind: "post"|"proposal", post?|rev?, posts?, url, truncated, compressed: null|{…}}` oder `error {message}`.
  Trennt der Client die Verbindung, wird der Modellaufruf abgebrochen: Bei einer einzelnen Aufgabe wird nichts gespeichert, im Chat
  bleiben der Beitrag des Absenders und bereits fertige Antworten.
- `POST /einstellungen/personas/act` neu: `method=persona.set_prompt` (`handle`, `prompt`, `reset=1` = Standard;
  → `?ok=prompt|vorgabe#persona-<handle>`) und `method=persona.set_lector` (`mode`, `handle`; → `?ok=lektor#lektor`).

#### persona.chat: RFC, @alle, als Auftrag (Runde 28, chat-auftrag) {#personachat-rfc-alle-as-a-task-round-28-chat-auftrag}

`persona.chat {work?, key, message, rev?, as_order?}` (`as_order`: `true` oder aus Formularen `"1"`/`"on"`, Standard
`false`) → die bisherigen Felder und `{rfc, proposals: [{persona, rev, url}], findings: [id], notes: [Text]}`
(`src/assist/chat.rs`):

- **RFC:** „RFC“ als erstes Wort (Groß-/Kleinschreibung egal, auch „RFC:“) oder die Erwähnung „@alle“ → `rfc: true`;
  jede aktive Persona antwortet genau einmal (die Grenze von drei Antworten gilt nicht, die Warteschlange der Modelle
  schon), der Lektor je nach Einstellung zuletzt. Er erhält die Antworten der anderen als Daten gerahmt (`<<<antwort …
  >>>`, jede mit ihren Befunden `[B<id>]`) und die Aufgabe, sie zu „Jetzt wichtig:“ und „Kann warten:“ zu gewichten,
  ohne Befunde anderer ungeprüft zu übernehmen; seine Befunde werden Sammelbefunde (`summary_of`). Inaktive Personas
  antworten nicht.
- **Schreibauftrag:** Beginnt die Nachricht nach Erwähnungen und „bitte“ mit einem Schreibverb im Imperativ
  („Schreibe“, „Formuliere“, „Erstelle“, „Überarbeite“, „Kürze“, „Ergänze“ …) oder ist `as_order` gesetzt,
  schreibt jede erwähnte Persona (bei `as_order` ohne Erwähnung der Lektor) einen **Vorschlag** für den Abschnitt (`proposals`,
  nie übernommen), und ein kurzer Beitrag der Persona steht im Chat („Fassung n dieses Abschnitts liegt als Vorschlag
  bereit …“; Baruch schreibt den Text, kein zweiter Modellaufruf). Löschen und neue Abschnitte bleiben beim Menschen
  (neue Abschnitte nur über die Gliederung oder MCP `outline_add`). Nennen Nachricht oder Vorschlag Befunde mit
  „[B<id>]“, macht Baruch für jeden einen Statusvorschlag „übernommen in Fassung n“ (`findings::rpc::propose`,
  Herkunft die Persona); scheitert das, steht der Grund in `notes`.
- Sonst wie bisher: eine Antwort je erwähnter aktiver Persona (höchstens drei).
- Formular `POST /assistenten/act` mit dem Kontrollkästchen `as_order`; nach einer Chatnachricht geht die Umleitung auf
  `…#chat-neueste` (neuester Beitrag).

#### Modell-Backend (Runde 10) {#model-backend-round-10}

Nur der Mensch: über `/rpc`, nicht über `POST /act` und nicht über MCP (`crate::models`, vor der Wahl des Werks).

| Methode | Parameter | Ergebnis |
|---|---|---|
| `model.list` | `{persona? \| connection?}` | `{models: [Name…], live, fetched: ISO, message, contexts: [{id, context_length}], aliases: {alt: neu}, personas: [{handle, model, active, state: "bekannt"\|"alias"\|"unbekannt", target?}], connection}`; Runde 15b: Liste der Verbindung der Persona oder der Verbindung `connection` (ID), ohne beides Eliezer; `personas` nur die Personas dieser Verbindung; je Adresse 60 s zwischengespeichert (`live: false`). Antwortet die Gegenseite nicht und gibt es eine (abgelaufene) Liste: diese mit `live: false` und `message` („… antwortet nicht (…); Liste aus dem abgelaufenen Zwischenspeicher (Stand …)“); sonst → -32000 mit einem deutschen Satz |
| `model.status` | `{}` | `{running: label \| null, waiting: number}` – Warteschlangen je Endpunkt (eine Anfrage zugleich je Adresse, höchstens acht wartend), zusammengefasst; Bezeichnung z. B. `"@lektor ask key 17"`, nie Text |
| `model.probe` | `{model, persona? \| connection?}` | `{ok: true, model, requested, reply (höchstens 200 Zeichen), ms, finish_reason, completion_tokens, at, connection}` oder `{ok: false, model, requested, message, ms, at, connection}`; feste Aufgabe "Answer only with: OK", `max_tokens` 16, durch die Warteschlange der Verbindung (ohne eine Eliezer); kein Buchtext, daher auch über eine externe Verbindung ohne Freigabe. Das Ergebnis steht in der Datenbank (`model_connection.probe`, überlebt einen Neustart); fehlendes, mehrzeiliges oder längeres `model` als 200 Zeichen → -32602 |
| `model.context` | `{persona}` | `ContextBudget {tokens, reported, warning}` + `{persona, setting}` (Runde 15b): `tokens` = min(Einstellung, gemeldete Länge), ohne Meldung die Einstellung; Ollama meldet die wirksame Länge (`num_ctx` des Modelfile, sonst das des geladenen Modells aus `/api/ps`, sonst 4096 – nie das Maximum des Modells); `warning` z. B. „Einstellung 65536, Eliezer meldet 32768; wirksam sind 32768 Tokens.“; wartet höchstens so lange wie die Modellliste (10 s), Ergebnis 60 s zwischengespeichert; scheitert nie (nur eine unbekannte Persona → -32602) |
| `model.connections` | `{}` | `{connections: [{id, name, kind, kind_title, url, external, locality, locality_warning?, account, secret: "hinterlegt"\|"fehlt"\|null, secret_for?, secret_withheld?, probe, check, position, eliezer, personas}], kinds: [{kind, title}]}` – nie ein Schlüssel |
| `model.connection_set` | `{id?, name, kind: "eliezer"\|"openai"\|"ollama", url, external?: bool}` | wie ein Eintrag von `model.connections`; fehlendes `external` → beim Anlegen nach der Adresse (nicht erkennbar lokal → `true`, siehe `models::looks_local`), beim Ändern bleibt `true` bestehen, und eine neue nicht erkennbar lokale `url` setzt `true`; `locality_warning` (Satz), wenn eine als lokal geführte Verbindung auf eine nicht erkennbar lokale Adresse zeigt; ohne `id` neu (Konto `baruch/model-<id>`, IDs nie wiederverwendet), mit `id` ändern (fehlende Felder bleiben). Name 1–80 Zeichen, eindeutig; Adresse wie `connection.set` (http/https, ohne Benutzer und Schlüssel). Für Eliezer (ID 1) geht `url` an die Verbindung `eliezer` (mit ihrer Bindung), die Art bleibt `eliezer`. Zeigt die Adresse auf ein anderes Ziel (Origin), bleibt ein gespeicherter Schlüssel an das alte gebunden und wird nicht gesendet. Nur der Mensch |
| `model.connection_delete` | `{id, confirm: true}` (Runde 23: ohne `confirm: true` -32602) | `{id, deleted: true, personas: [Handle]}`; ihr Schlüssel wird entfernt, ihre Personas nutzen wieder Eliezer (nicht erlaubt); Eliezer selbst → -32000. Nur der Mensch |
| `model.connection_set_secret` | `{id, secret}` | `{id, secret: "hinterlegt"}`; Schlüssel nur im Schlüsselbund, gilt danach für die aktuelle Adresse (Eliezer: Konto `eliezer`). Leer, über 4096 Zeichen oder mehrzeilig → -32602. Nur der Mensch |
| `model.connection_delete_secret` | `{id, confirm: true}` (Runde 23: ohne `confirm: true` -32602) | `{id, secret: "fehlt"}`. Nur der Mensch |
| `model.connection_check` | `{id}` | `{ok, message, details: {models, contexts}, at, connection}`, gespeichert in `model_connection.checked`; ein Fehlschlag ist `ok: false`; Schlüssel für ein anderes Ziel → `ok: false` „… gilt für ein anderes Ziel; nicht gesendet“, die Gegenseite wird nicht gefragt. Eliezer: wie `connection.check {kind: eliezer}` |

Alte Ollama-Namen werden vor dem Senden aufgelöst (`llama3.2:3b` → `llama-3.2-3b-instruct-q4-k-m`, `qwen3:4b` →
`qwen3-4b-q4-k-m`; nur bei Verbindungen der Art Eliezer). Baruch fordert nie eine Kontextlänge an und ruft nie
`/api/v1/models/load|unload|estimate` auf.

**Verbindung je Persona (Runde 15b):** Eine Aufgabe einer Persona (`persona.*`, `/assistenten/*`, MCP `ask_persona`/
`chat`) geht über ihre Verbindung. Ist die Verbindung extern und die Persona nicht freigegeben, lehnt Baruch mit
-32000 `„Die Verbindung „…“ ist extern; für @… ist sie nicht freigegeben (Einstellungen → Assistenten). Es wurde
nichts gesendet.“` ab – geprüft in `EliezerPort::chat`, also bei jedem Modellaufruf der Personas. Formulare ohne
JavaScript: `POST /einstellungen/modelle/act` mit `action=save` (Anlegen ohne `id`: `name`, `kind`, `url`,
`external=1|0|auto` – `auto`, der Standard beim Anlegen, nach der Adresse; das Kontrollkästchen beim Ändern: nicht angehakt
lokal), `secret` (`id`, `secret`), `delete_secret` und `delete` (`id`, `confirm=1`), `check` und `probe`
(`id`, `model`) → 303 `/einstellungen/verbindungen?ok=modell_…#modell-<id>` oder `?modellfehler=<code>#modelle`.

#### Bibelstellen (Runde 10) {#bible-references-round-10}

| Methode | Parameter | Ergebnis |
|---|---|---|
| `scripture.check` | `{text, work?, module?}` | `{refs: [{input, span: [a, b], normalized, book, book_id, status, message, wording, wording_code, quote, bible_text, partial, diff, diff_line}], truncated, source, bible}`; `status` ∈ `valid\|unknown_book\|no_such_chapter\|no_such_verse\|malformed`, `message` deutsch mit dem Maximum; ist der ganze einzeilige Text eine Stelle, wird er streng gelesen, sonst im Fließtext gesucht; höchstens 200 000 Zeichen (sonst -32602), höchstens 200 Stellen (`truncated`); ohne `text` → -32602. **Wortlaut:** mit der Bibel des Werks (`work`, fehlend → Standardwerk) oder dem Modul `module` je Stelle `wording_code` ∈ `match` („Wortlaut stimmt“, `partial` bei einem Teilzitat), `differs` („Wortlaut weicht ab“, `diff` = `[{op: same\|bible\|quote, text}]`, `diff_line` mit `[−…]`/`[+…]`), `no_quote` („kein Zitat gefunden“), `missing_verse`, `versification_mismatch`, `invalid_ref`, `module_error`; `bible_text` = Text der Bibel (höchstens 4000 Zeichen), `quote` = Zitat an der Stelle. Ohne Bibel `wording` = `"ungeprüft"`, `wording_code` = `unchecked`, `bible` = `null`. Nur lesend |

**Versifikationen umrechnen (Runde 17a).** Nur lesend, ohne Werk, auch über `POST /act`:

| Methode | Parameter | Ergebnis |
|---|---|---|
| `scripture.map` | `{ref, from? = "KJV", to? = "German"}` | `{from, to, refs: [{input, normalized_from, status, target, verses: [{book, chapter, verse}], rules, message}], versifications: [Namen]}`; `ref` 1–500 Zeichen, gelesen mit `baruch_scripture::parse` (mehrere mit „;“, Buchnamen deutsch oder OSIS) und **gegen `from`** geprüft (nicht gegen die deutsche Verszählung): „Joel 3,21“ ist in KJV gültig, in German nicht. `status` ∈ `same` (gleiche Verse), `moved` (andere Verse nach einer Regel, `rules` nennt sie), `unmapped` (keine Regel deckt den Unterschied ab, `message` beginnt mit „Versifikation passt nicht: “ – nie geraten), `invalid` (Buch unbekannt oder die Stelle gibt es in `from` nicht; `message` nennt den Grund). `target` = Verse in `to` in deutscher Schreibweise („Joel 3,1–5“, Folgen mit „; “), sonst `null`; `verses` einzeln (Buch als OSIS-Abkürzung), höchstens 1000 je Stelle (dann „– gekürzt“ in `message`). Eine Stelle „ganzes Kapitel“ lässt Psalmüberschriften aus, die die Ziel-Versifikation nicht als Vers zählt (Regel in `rules`). Versifikationen: German, Calvin, Catholic, Catholic2, DarbyFr, KJV, KJVA, Leningrad, Luther, LXX, MT, NRSV, NRSVA, Orthodox, Segond, Synodal, SynodalProt, Vulg (Groß-/Kleinschreibung egal); unbekannte Versifikation → -32602 mit dieser Liste; fehlendes oder zu langes `ref` → -32602. Regeltabelle `crates/baruch-scripture/data/versification-map.tsv` (Herkunft in ihrem Kopf), siehe `docs/Architektur.md`, „Round 17a“ |
| `scripture.versifications` | `{}` | `{versifications: [{name, books, chapters, verses}]}` – German zuerst, dann alphabetisch; nur die 66 Bücher der Stellenprüfung (MT und Leningrad: 39, ohne das Neue Testament) |

#### Bibeln (SWORD, 2026-11-06) {#bibles-sword-2026-11-06}

Quelltext `src/bibles.rs`, Leser-Crate `baruch-sword`. `work?` wie überall (fehlend → Standardwerk); `module?` ist ein
Modulname aus `bible.modules` (fehlend → die Bibel des Werks). Jede Antwort nennt in `bible` Modul, Versifikation
und Lizenzhinweis: `{module, description, versification, license (DistributionLicense; Copyright oder „keine
Lizenzangabe in der conf“), distribution_license, copyright, lang, version, driver, dir, supported, problem, cipher,
notice}`. Seit Runde 17c: `cipher` ∈ `none` (nicht verschlüsselt) | `locked` (verschlüsselt, Schlüssel fehlt) |
`unlocked` (Schlüssel im Schlüsselbund oder schon in der conf), `notice` = `license` (Lizenzhinweis unter dem
Namen, den auch Kommentare und Lexika tragen). Der Schlüssel selbst steht nie in einer Antwort.

**Versifikation des Moduls (R17c):** Stellen werden immer deutsch gezählt (Schlachter 1951). Liest Baruch ein Modul in
einer anderen Versifikation (KJV, Luther, Vulgata, Synodal …, alle Versifikationen von `sword-modules`),
bildet `bibles::read_passage` die Stelle durch `baruch_scripture::versification::mapping::map_range` auf die Versifikation des
Moduls ab und liest dort (höchstens 400 Verse; `truncated` = die Stelle hat mehr). Was sich nicht abbilden lässt,
bleibt `versification_mismatch` mit einem Grund („Versifikation passt nicht: …“ mit beiden Versifikationen); nie geraten.
`scripture.check`, `bible.check` und MCP `scripture_check` tragen je Stelle **additiv** `quote_span: [s, e] | null`
(Byte-Bereich des Zitatinhalts ohne Anführungszeichen im geprüften Text) und `module_ref: string | null` (Stelle gelesen
in der Versifikation des Moduls, z. B. „Mal 4,1–6 (KJV)“, nur wenn sie von der deutschen abweicht); alle
bisherigen Felder bleiben. Zitate kommen nur aus `crate::quotes::quote_spans` (Anführungszeichen, verschachtelt,
`#quote[…]`, Blockzitate).

| Methode | Parameter | Ergebnis |
|---|---|---|
| `bible.modules` | `{work?}` | `{dirs, default_dirs, modules: [bible …], problems, work, work_bible, work_sources: {commentaries, lexicons}}`; `default_dirs`: keine eigene Liste gespeichert (dann `$SWORD_PATH`, `~/.sword`, `/usr/share/sword`); `problems`: fehlende Ordner, unlesbare confs, doppelte Namen. Seit Runde 15 je Modul `kind`: `bible` (zText/zText4/RawText/RawText4), `commentary` (RawCom/RawCom4/zCom/zCom4) oder `lexicon` (RawLD/RawLD4/zLD); fehlt `kind`, gilt `bible`. Unlesbare (HREFCom, RawFiles, unbekannte Kompression, unbekannte Versifikation `„Verszählung „X“ kennt Baruch nicht“`) stehen mit `supported: false` und einem Grund. Seit Runde 17c je Modul `cipher` und `key_stored` (Schlüssel im Schlüsselbund gespeichert ja/nein); verschlüsselte Module sind `supported: true` und ohne Schlüssel `cipher: locked`; Kompression ZIP, LZSS, BZIP2 und XZ |
| `bible.set_dirs` | `{dirs: [Pfad] \| null}` | wie `bible.modules`; jeder Eintrag ein absoluter Pfad oder `~/…`, getrimmt, leere entfallen, Duplikate einmal, höchstens 20 zu je 1000 Zeichen, sonst -32602; `null` = zurück auf die Standardwerte. Nur der Mensch (nicht über MCP) |
| `bible.set_work` | `{work?, module: Name \| null}` | wie `bible.modules`; unbekanntes oder unlesbares Modul → -32602; `null`/leer = keine Bibel. Nur der Mensch (nicht über MCP) |
| `bible.lookup` | `{ref, work?, module?}` | `{bible, refs: [{input, normalized, book, book_id, status, message, verses: [{chapter, verse, text \| null}], missing, truncated, module_ref, rules}]}`; `ref` streng gelesen wie `scripture.check` (höchstens 500 Zeichen, 20 Stellen), seit R17c durch `read_passage` gelesen: `verses` in der Versifikation des Moduls (höchstens 400; `truncated` aus `PassageText`, genau 400 Verse = nicht gekürzt), `module_ref` wie oben, `rules` = angewandte Regeln der Abbildung, nicht abbildbar → `status: versification_mismatch` mit einem Grund; `text: null` = Vers fehlt im Modul; reiner Text ohne Fußnoten, Querverweise, Überschriften (kanonische bleiben). Ohne Bibel → -32000 „Keine Bibel gewählt …“; verschlüsselt ohne Schlüssel → -32000 mit „… ist verschlüsselt; der Schlüssel fehlt …“ |
| `bible.search` | `{query, work?, module?, limit? = 50}` | `{bible, query, hits: [{ref, book_id, chapter, verse, text}], truncated, searched_verses, ms}`; lineare Suche nach der Wortfolge (normalisiert wie der Wortlautvergleich), `limit` 1–200, Zeitlimit 5 s (`truncated`); `query` 1–200 Zeichen, sonst -32602 |
| `bible.check` | `{work?, key}` | wie `scripture.check` für die aktuelle Fassung des Abschnitts `key`, dazu `work`, `key`, `title`. Nur lesend |
| `bible.marks` | `{work?, key, rev?}` | Prüfanzeige (R17c, nur lesend, kein MCP-Werkzeug – MCP nimmt `scripture_check`): `{work, key, rev, title, bible, notice, marks: [{ref, ref_span: [s, e], quote_span: [s, e] \| null, code, state, label, quote, bible_text, module_ref, diff_line}], counts: {stimmt, weicht_ab, andere}}`. Geprüft wird die gezeigte Fassung `rev` (Nummer der Fassung, Standard die aktuelle); Spannen sind Byte-Bereiche in **ihrem** Text. `state` ∈ `stimmt` (`match`), `weicht_ab` (`differs`), `ohne_zitat` (`no_quote`), `ungeprueft` (`unchecked`), `fehler` (andere Codes); `notice` = Lizenzhinweis der conf. Unbekannte Fassung → -32602 |
| `bible.cipher_key_set` | `{module, key}` | `{module, cipher: "unlocked"}` (R17c). Schlüssel eines **verschlüsselten** Moduls (Bibel, Kommentar oder Lexikon) vom Verlag: 1–255 Zeichen in einer Zeile (sonst -32602), unbekanntes oder unverschlüsseltes Modul → -32602. Baruch liest zuerst den ersten vorhandenen Eintrag als Probe (entschlüsseln, entpacken, gültiges UTF-8); passt er nicht: -32000 „Schlüssel passt nicht zum Modul; nichts gespeichert.“ Gespeichert nur im Schlüsselbund (Dienst `baruch`, Konto `sword-cipher:<modulname klein>`), nie in der Datenbank, conf, in Antworten, Meldungen oder Logs. Nur der Mensch (nicht `/act`, nicht MCP) |
| `bible.cipher_key_delete` | `{module, confirm: true}` | `{module, cipher}` (`locked`, oder `unlocked`, wenn die conf selbst einen Schlüssel trägt); ohne `confirm: true` → -32602. Nur der Mensch (nicht `/act`, nicht MCP) |
| `bible.work_sources` | `{work}` | `{work, bible, commentaries: [Name], lexicons: [Name]}` (Runde 15) – Bibel, Kommentare und Lexika des Werks |
| `bible.set_work_sources` | `{work, commentaries: [Name], lexicons: [Name]}` | wie `bible.work_sources`; nur gefundene, lesbare Module der richtigen Art (Schreibweise wie in der conf), Duplikate und leere entfallen, höchstens 20 je Art; sonst -32602. Tabelle `work_sword_source` (Migration 0037). Nur der Mensch (nicht `/act`, nicht MCP). Formular `POST /einstellungen/werk/bibeln` mit `method=bible.set_work_sources`, `work`, wiederholten Feldern `commentary` und `lexicon` → 303 `/einstellungen/werk?work=W&ok=kommentare#kommentare` oder `&kommentarfehler=1` |
| `commentary.lookup` | `{ref, work?, module?}` (Runde 23: Stelle in deutscher Versifikation, abgebildet in die Versifikation des Kommentars) | `{ref, work, commentaries: [{module, description, versification, notice, entries: [{ref, text, truncated}], problem?}]}` (Runde 15) – für jeden gewählten Kommentar (oder `module`) die Einträge zur Stelle in der **Versifikation des Kommentars**; ein Eintrag für mehrere Verse erscheint einmal mit einem Bereich („Ps 139,23–24“). Reiner Text ohne Auszeichnung, höchstens 4000 Zeichen je Eintrag (`truncated`); `problem`: Modul fehlt/unlesbar, Stelle ungültig, kein Eintrag. `ref` 1–500 Zeichen |
| `commentary.search` | `{query, work?, module?, limit? = 50}` | `{query, work, commentaries: [{module, …, hits: [{ref, text, truncated}], truncated, problem?}]}`; lineare Suche (wie `bible.search`), Zeitlimit insgesamt 5 s |
| `lexicon.lookup` | `{key, work?, module?}` | `{module, key, found, text, notice, truncated}` – erstes gewähltes (oder genanntes) lesbares Lexikon; Schreibweise nach der conf, `@LINK` gefolgt, Strong-Nummern wie `G25`; `found: false` = kein solches Stichwort. Ohne Lexikon → -32000 |
| `lexicon.keys` | `{work?, module?, prefix?, limit? = 100}` | `{module, prefix, keys, truncated, total, notice}` – Stichwörter wie im Modul, sortiert, mit Präfix, höchstens 500 |
| `lexicon.search` | `{query, work?, module?, limit? = 50}` | `{module, query, hits: [{key, excerpt, in_key}], truncated, notice}` – Stichworttreffer zuerst, dann Treffer im Artikeltext; Zeitlimit 5 s |

Formulare ohne JavaScript unter Einstellungen → Quellen: `POST /einstellungen/quellen/act` mit `method=bible.set_dirs`
(`dirs` ein Ordner je Zeile, `reset=1` = Standard; Codes `ok=bibel_ordner|bibel_vorgabe`, `fehler=bibel_ordner`)
oder `method=bible.set_work` (`work`, `module`, leer = keine; `ok=bibel_gewaehlt|bibel_keine`, `fehler=bibel_modul`),
seit R17c `method=bible.cipher_key_set` (`module`, `key` als Passwortfeld; `ok=bibel_schluessel`,
`fehler=bibel_schluessel_falsch|bibel_schluessel|schluesselbund`) und `method=bible.cipher_key_delete` (`module`,
erforderliches Kontrollkästchen `confirm=1`; `ok=bibel_schluessel_entfernt`, `fehler=bestaetigen`), 303 zurück auf
`/einstellungen/quellen?…#bibeln`. Seite ohne JavaScript `GET /werke/{id}/pruefung?key=K[&rev=N]`: dieselben Daten
wie `bible.marks` als Liste (`src/scripture_check.rs`).

#### MCP-Token (Runde 10) {#mcp-tokens-round-10}

Nur der Mensch: über `/rpc` und das Formular der Seite „MCP-Zugang“, nicht über `POST /act` und nicht über MCP.

| Methode | Parameter | Ergebnis |
|---|---|---|
| `mcp_token.list` | `{}` | `{tokens: [McpToken]}`; `McpToken = {id, name, prefix, created, last_used, revoked}` (nie Hash oder Klartext) |
| `mcp_token.create` | `{name}` | `{token: McpToken, secret}` – Runde 23: auch -32000, wenn das Handle (`@mcp:<handle>`) vergeben ist, das eines widerrufenen Tokens ist oder mit `lokal-` reserviert ist (siehe „Round 23 – Hardening“); `secret` ist der Klartext (`baruch_…`), **nur in dieser Antwort**; Name leer, über 64 Zeichen, mit Steuerzeichen oder doppelt unter gültigen → -32000 |
| `mcp_token.revoke` | `{id, confirm: true}` | `{token}`; ohne `confirm: true` → -32602; unbekannt → -32000; idempotent |

Formular `POST /einstellungen/mcp/act`: Das Anlegen (`name`) antwortet direkt mit 200 und der Seite und dem Klartext
genau einmal (`Cache-Control: no-store`, nie in einer Adresse, Umleitung, einem Cookie oder einer Meldung); Widerrufen (`id`,
`confirm=1`, der Server prüft das Kontrollkästchen) → 303 `?ok=widerrufen`.
- Fehler: unbekannte Persona (`„Persona „@x“ gibt es nicht.“`), ungültige Farbe („Farbe als #rrggbb
  angeben.“), fehlendes `handle` oder `color` → -32602. Kein Commit im Spiegel.

### Websuche für @belege (Runde 13) {#web-search-for-belege-round-13}

Nur über `POST /rpc` (Mensch), nicht über `POST /act`, **kein MCP-Werkzeug** (Begründung in Architektur.md).
Vor der Wahl des Werks in `rpc::call` (`crate::websearch`).

| Methode | Parameter | Ergebnis |
|---|---|---|
| `websearch.get` | `{}` | `{on, configured, preferred}` – `on` = Schalter (`store_meta` `websearch_belege` = `'1'`, fehlt = aus), `configured` = für MultiGPT ist eine Adresse eingetragen, `preferred` = Zahl bevorzugter Quellen |
| `websearch.set` | `{on: bool}` | `{on}`; kein Wahrheitswert → -32602 |
| `websearch.evidence` | `{post}` | `{post, evidence: [{id, label, url, title, preferred, accessed, quote, verdict, library_title, post, work, key, persona, created}]}` – Belege eines Beitrags in der Reihenfolge der Entstehung; `label` „W1“ …, `accessed` UTC-Zeit, `verdict` ∈ `belegt` \| `nicht_belegt` \| `nicht_zitiert`, `library_title` = übernommener Titel oder `null`; ohne Belege `[]` |
| `websearch.adopt` | `{evidence}` | `{title: TitleEntry, quote: Quote, highlight: Highlight, evidence}` – nur mit `verdict = 'belegt'`; sonst, bei unbekanntem oder bereits übernommenem Beleg -32602 (nichts angelegt). Legt einen Titel `kind: "web"` mit `url`, `publisher` = Host und Notiz „Abgerufen am TT.MM.JJJJ (Websuche @belege)“ an (gibt es schon einen Titel `web` mit derselben Adresse, wird dieser genommen), das Zitat als `library_quote` und eine Hervorhebung `kind: quote` des Menschen; setzt `library_title_id`, dann `rag::notify(state, None)` |

`settings.get {page: "assistenten"}` enthält zusätzlich `websearch: {on}`.

Ablauf bei `persona.ask`/`persona.review`/`persona.chat` (nur `@belege`, nur bei eingeschaltetem Schalter, nicht bei
`persona.propose`): zuerst `web_search` mit „site:<Rechner> <Anfrage>“ für die ersten drei bevorzugten Quellen, dann
ein allgemeiner Aufruf; höchstens 8 Treffer, 4 Abrufe durch `fetch_url`, je 10 s, insgesamt 30 s, Seite ≤ 1 MiB. Der
Code prüft die Antwort (`post.body`): „(nicht belegt)“, „(unbekannte Quelle)“, „(nicht abgerufen)“, „Angabe
prüfen: nicht belegt“, Zeile „Quellen: [Wn] Titel – URL (abgerufen am TT.MM.JJJJ)“, bei einem Ausfall „Hinweis: Websuche
ausgefallen (<Grund>)“. Mit Belegen trägt der Beitrag die Quelle „Web: n Beleg(e)“ (`post.sources`).

Formular `POST /websuche/act` (hinter `guard::same_origin`, fremder `Origin` → 403), immer 303:
- `action=switch` (`on=1`, fehlt = aus; `work`) → `/einstellungen/assistenten?persona=belege&ok=websuche[&work=W]#persona-belege`.
- `action=adopt` (`evidence`, optional `work`, `key`, `post`) → `/websuche/belege?work=W&key=K&post=P&ok=uebernommen#beleg-<id>`
  oder `&fehler=nicht_belegt|schon|unbekannt|intern`; unbekannte `action` → `&fehler=aktion`.

Seite: `GET /websuche/belege?work&key&post` – Belege eines Beitrags mit der Schaltfläche „In die Bibliothek“ (nur bei
`belegt`), ohne JavaScript.

### Menü {#menu}

- `menu.badges {work?}` → `{work, outline_requests, open_proposals}`: Zahlen der Plaketten der Kopfzeile –
  offene (unentschiedene) Gliederungsanträge und offene Vorschläge über alle Abschnitte des Werks;
  die Werkseite zeichnet dieselben Zahlen auf dem Server. Fehlt `work`, das Standardwerk. Parameter, die kein
  Objekt sind, `work` keine Zahl oder ein unbekanntes Werk → `-32602`. Nur lesend; beantwortet vor der Wahl
  des Standardwerks in `rpc::call` (`crate::menu`).
- Runde 28 (befund-uebersicht): `menu.badges` liefert zusätzlich `open_findings` – offene Befunde im ganzen Werk
  nach der Zählregel (Vertrag R28 §1.5: `status = 'open'`, nicht angehängt, kein Lob, mit einem Abschnitt; Summe aus
  `Store::finding_open_counts`). Plakette „k offen“ am Reiter **„Prüfen“** und am Eintrag „Befunde“.

### Einstellungen {#settings}

Ohne erforderliches Werk, vor der Wahl des Werks in `rpc::call`; nur über `/rpc` (nicht `/act`, nicht MCP).

| Methode | Parameter | Ergebnis |
|---|---|---|
| `settings.pages` | – | `{pages: [{slug, title, storage, url}]}` (sieben, feste Reihenfolge) |
| `settings.render` | `page`, `work?`, `query?: {…}` (Text, Zahl, Wahrheitswert) | `{page, title, storage, html}` – `html` wie `GET /einstellungen/<page>?fragment=1`; ohne `Host` zeigt „MCP-Zugang“ nur den Pfad `/mcp` |
| `settings.get` | `page`, `work?` | `darstellung`: `{theme: system\|light\|dark, editor, pv, accordion, widths: {left, right}, lang: en\|de\|auto}` aus den Cookies der Anfrage (`lang` seit Runde 39a: die gewählte Oberflächensprache, eigene Einstellung oder Cookie, `auto` = wie der Browser); `werk`: `{work, title, subtitle, author, kind, audience, bible, template_rev, numbering: {scheme, custom}, timing: {words_per_minute, target_seconds}}` (`timing` seit Runde 16b, `target_seconds` `null` = keine Zielzeit); `assistenten`: `{personas: [PersonaRow], lector, lector_mode, lector_handle}`; `mcp`: `{path: "/mcp", loopback_without_token: true, clients: [SeenClient], tokens: [McpToken]}` (`loopback_without_token` ist fest `true` und beschreibt den direkten Zugriff von der eigenen Maschine; unter einem mit `--allow-host` erlaubten Hostnamen braucht `/mcp` trotzdem ein Token, `docs/MCP.md`, „Access“); `daten`: `{dir, size_bytes, mirrors: [{work, slug, dir, head}], version, instance: {uuid, name}}` (`instance` seit Runde 26); `verbindungen`/`quellen` → -32602 („über connection.list bzw. sources.list“) |
| `settings.set` | `page`, `work?`, `values: {…}` | `werk`: wie `work.update` (`{work, changed, …}`); `assistenten`: `values {handle, role?, model?, context_length?, active?, color?}` → `{persona: PersonaRow, lector}` (erst alles prüfen, dann schreiben); `darstellung` → -32602 („Cookies setzt das Formular POST /einstellungen/darstellung/act“); sonst -32602 |
| `settings.backup` | `confirm: true` | `{dir, size_bytes, skipped}`; ohne `confirm` -32602, Ziel existiert, schon 10 Sicherungen `<Name>.sicherung-*` neben dem Datenordner (Baruch löscht selbst keine) oder zu wenig Platz (nach der Sicherung müssen 512 MiB frei bleiben, Unix `statvfs`) und anderes → -32603 mit einer deutschen Meldung |

`persona.update` siehe „Personas“.

Formulare (hinter `guard::same_origin`, fremder `Origin` → 403):
- `POST /einstellungen/darstellung/act`: `theme=system|light|dark`, `editor=satz|quelltext`, `pv=open|closed`,
  `accordion=personas|revisions|chat`, `reset_widths=1` (jeweils optional), `work` → Set-Cookie
  `baruch_<n>=<v>; Path=/; Max-Age=31536000; SameSite=Lax` (Löschen mit `Max-Age=0`; `system` löscht
  `baruch_theme`, `open` löscht `baruch_pv`, `reset_widths` löscht `baruch_left`/`baruch_right`), 303
  `/einstellungen/darstellung?ok=1[&work=W]`; ungültig → 422 mit der Seite, kein Cookie.
- `POST /einstellungen/sprache` (Runde 39a): `lang=en|de|auto`, optional `back` → Set-Cookie
  `baruch_lang=<code>; Path=/; Max-Age=31536000; SameSite=Lax` (`auto` mit `Max-Age=0`) und die gespeicherte Einstellung
  (Konto, oder die Instanz ohne Konten und für den Autor an der eigenen Maschine; anonym: nur Cookie), 303 auf
  `back`, wenn es ein lokaler Pfad ist, sonst auf den Referer desselben Hosts, sonst `/`; eine ungültige Sprache → 422 mit der
  Meldung, nichts gesetzt. Öffentlich und für Lektoren erlaubt; keine RPC-Methode.
- `POST /einstellungen/personas/act`: `method=persona.set_color` (wie bisher) oder `method=persona.update`
  (`handle, role, model, context_length, active=1|leer`) → 303
  `/einstellungen/assistenten?ok=farbe|persona[&work=W]#persona-<handle ohne @>`; Fehler → 422 mit der Seite.
  Runde 11 (ein Reiter je Persona, `GET /einstellungen/assistenten?persona=<handle ohne @>[&fragment=1]`):
  `action=save` (`handle, name, color, color_text, role, model, context_length, active, prompt`; alles in einer
  Transaktion, ein unveränderter Standard-Prompt bleibt der Standard) oder `action=reset_prompt` (`handle`, `confirm=1`;
  ohne `confirm` → Rückfrage) oder `action=reset_color` (`handle`; Farbe zurück auf die Palette)
  → 303 `/einstellungen/assistenten?persona=<h>&ok=persona|vorgabe|farbe` oder
  `&rueckfrage=vorgabe` (`[&work=W]#persona-<h>`); abgelehnt → 422 mit dem Reiter und der Eingabe.
- `POST /einstellungen/werk/zeit` (Runde 16b, Block „Predigtzeit“): `method=work.timing_set`, `work`,
  `words_per_minute` (60–250), `target_minutes` (1–240, leer = keine Zielzeit) → 303
  `/einstellungen/werk?work=W&ok=zeit#zeit`; ungültig → 303 `…&zeitfehler=1#zeit`, nichts geändert.
- `POST /einstellungen/daten/sicherung`: `confirm=1` erforderlich (sonst 422) → 303
  `/einstellungen/daten?ok=sicherung[&work=W]` (seit Runde 23 ohne `ziel`: ein fester Meldungscode; die Seite liest den Ordnernamen der
  neuesten Sicherung aus dem Dateisystem).
- `POST /werke/act` (update) und `POST /werke/nummerierung/act`: neues Feld `back` (siehe Aufbau). Mit einem
  gültigen `back` antwortet ein abgelehntes Formular (422 oder 409) mit der ganzen Einstellungsseite „Werk“
  (`settings.html` mit genau einem `[data-settings-frame]`), Meldung und Eingabe im jeweiligen Formular; so
  übernimmt das Popup den Rahmen, ohne JavaScript steht dort die Seite.

Seiten: `GET /einstellungen` → 303 `/einstellungen/darstellung[?work=W]`; `GET /einstellungen/<seite>[?work=W]
[&fragment=1]` (unbekannt → 404); `GET /einstellungen/personas` → 303 `/einstellungen/assistenten[?work=W]`.

### Verbindungen und Quellen {#connections-and-sources}

Nur der Mensch: über `POST /rpc` und die eigenen Formulare, nicht über `POST /act` und nicht über MCP. Fehler:
-32602 ungültig (Meldung ohne das eingegebene Geheimnis), -32603 intern oder „Schlüsselbund: …“.

| Methode | Parameter | Antwort |
|---|---|---|
| `connection.list` | – | `{connections: [{kind, title, config, updated, secret, secret_error?, secret_for?, check}]}`; `secret` = `"hinterlegt"` \| `"fehlt"` \| `null` (LanceDB oder Schlüsselbund nicht lesbar, dann `secret_error`); `secret_for` = Ziel, für das das gespeicherte Geheimnis gilt, nur wenn die Einstellungen inzwischen auf ein anderes zeigen (siehe „Binding“); `check` = `{at, ok, message, details}` \| `null` |
| `connection.get` | `{kind}` | eine Verbindung wie oben |
| `connection.set` | `{kind, config}` | eine Verbindung; angegebene Felder ersetzen die gespeicherten. Ändert sich das Ziel, bleibt ein gespeichertes Geheimnis an das alte gebunden (`secret_for`). -32602 bei einem unbekannten Feld, einem Geheimnisfeld (`password`, `token`, `key`, `secret` …), einer Adresse mit Benutzerinfo, Schema ≠ http/https, einem Schlüssel in der Query, Port ∉ 1–65535, unbekanntem TLS-Modus, Host mit Leerzeichen |
| `connection.set_secret` | `{kind, secret}` | `{kind, secret: "hinterlegt"}`, gilt danach für das aktuelle Ziel; leer, mehrzeilig, über 4096 Zeichen oder `lancedb` → -32602 |
| `connection.delete_secret` | `{kind, confirm: true}` | `{kind, secret: "fehlt"}`; ohne `confirm: true` → -32602 |
| `connection.check` | `{kind}` (`eliezer` \| `multigpt` \| `pgvector` \| `lancedb`) | `{kind, ok, message, details, at}`, gespeichert in `connection`; ein Fehlschlag ist `ok: false`, kein RPC-Fehler; ist das Geheimnis an ein anderes Ziel gebunden: `ok: false`, „Nicht geprüft: …“, `details: {secret_withheld: true, secret_target}`, die Gegenseite wird nicht gefragt; bei `eliezer` zusätzlich `details.personas` (wie `model.list`) und der Vergleich in `message`; die ganze Prüfung höchstens 10 s. `details` (Runde 12): `multigpt` `{server: {name, version}, tools, pages, collections: [{name, count}]}` (`tools/list` über `nextCursor`, höchstens 10 Seiten; `list_collections`, wenn angeboten); `pgvector` `{server_version, vector, table, columns: {rolle: spalte}, collections: [{name, count}], chunks, dimension, models}` (`ok` = verbunden ∧ `vector` ∧ Tabelle/Spalten passen ∧ Dimension bekannt; ohne `vector` `{server_version, vector: null}`); `lancedb` `{dir, exists, writable, tables, entries, model, dimension, configured_model, embedder: "ok" \| Grund, probe}` (`ok` = Ordner nutzbar ∧ Probe-Embedding ∧ Modell/Dimension passen zum Index) |
| `connection.rag_get` | `{work?}` | `{work, ways, default}` (ohne Zeile `["lancedb"]`, `default: true`) |
| `connection.rag_set` | `{work?, ways}` | `{work, ways, default: false}`; ways ⊆ `lancedb`, `pgvector`, `multigpt` (leer = nur Volltext), feste Reihenfolge |
| `connection.collections_get` | `{work?, way}` | `{work, way, collections: [..], available: [{name, count}], available_at}` (Runde 12); `available` aus `details.collections` der letzten Prüfung (leere Namen entfallen), `available_at` ihre Zeit oder `null`; `way` nur `pgvector` \| `multigpt`, sonst -32602 |
| `connection.collections_set` | `{work?, way, collections: [..]}` | `{work, way, collections}` – getrimmt, ohne Duplikate (das erste gewinnt), Reihenfolge wie angegeben; kein Text, leer, mehrzeilig, über 200 Zeichen, über 100 Einträge, `lancedb` oder unbekanntes Werk → -32602, nichts geändert. Tabelle `work_rag_collection` (Migration 0031) |
| `sources.list` | – | `{sources: [{id, title, url, note, position, created, updated}]}` |
| `sources.create` | `{title, url, note?}` | die neue Quelle (am Ende) |
| `sources.update` | `{id, title?, url?, note?}` | die geänderte Quelle |
| `sources.move` | `{id, position}` | `{sources}` (Position außerhalb 1..n → an den Rand) |
| `sources.delete` | `{id, confirm: true}` | `{deleted}` |

Seit Runde 13 nutzt die Websuche von `@belege` die Liste der Quellen (siehe „Web search for @belege“): Für die ersten
drei Hosts je ein Aufruf `web_search` mit „site:<Rechner>“, Kennzeichnung „bevorzugte Quelle“ nach dem Host
der Adresse. Die Seite Quellen zeigt das als `data-usage` (kein STUB mehr).

**Bindung des Geheimnisses an das Ziel** (Migration 0014, Spalte `connection.secret_target`): Ein Geheimnis
gilt nur für das Ziel, für das es gespeichert wurde – bei Eliezer und MultiGPT der Origin der Adresse
(Schema, Host, Port; ein anderer Pfad auf demselben Server ist dasselbe Ziel), bei pgvector Host, Port und
TLS-Modus. `connection.set` mit einem anderen Ziel merkt sich das vorherige Ziel, wenn ein Geheimnis gespeichert ist; danach
sendet `connection.check` das Geheimnis nicht an das neue (weder als Bearer noch als Passwort). Die Bestätigung
ist das erneute Eingeben: Nur `connection.set_secret` (oder „Speichern“ mit einem eingegebenen Geheimnis) bindet es an
das aktuelle Ziel. Zurück beim alten Ziel gilt es wieder; `connection.delete_secret` löst die Bindung. So
leert `connection.set` mit einer fremden Adresse plus `connection.check` keinen gespeicherten Schlüssel.

`connection.set` für `pgvector` kennt seit Runde 12 zusätzlich `table` (`tabelle` oder `schema.tabelle`),
`text_column`, `vector_column`, `collection_column`, `document_column` (erforderlich), `section_column`, `title_column`,
`url_column`, `model_column` (leer = existiert nicht) und `embedding_model`; Bezeichner
`^[A-Za-z_][A-Za-z0-9_]{0,62}$`, sonst -32602.

Seit Runde 19b die **Aufgabenpräfixe** des Embeddings: `lancedb` `query_prefix` und `document_prefix`, `pgvector`
nur `query_prefix` (seine Daten sind schon eingebettet; `document_prefix` dort → -32602 „unbekanntes Feld“). Geprüft
mit `baruch_rag::check_prefix`: höchstens 64 Zeichen, keine Steuerzeichen (auch kein Zeilenumbruch), sonst -32602
(Formularcode `praefix`); **nicht getrimmt** – das Leerzeichen am Ende gehört dazu (nomic-embed-text:
`"search_query: "` / `"search_document: "`). Fehlt das Feld, gilt „kein Präfix“ (Verhalten wie bisher, kein
Neuaufbau). Die pgvector-Suche bettet die Anfrage als `query_prefix + query` ein; LanceDB nutzt beide (R19a).
`connection.check` für `pgvector` (Runde 19b): alle Felder schon in SQL gekürzt (Sammlungen der Liste auf 200
Zeichen, nach vollem Namen gezählt, höchstens 1000; Modelle auf 200); bei einem gespeicherten Passwort und TLS `prefer`,
`disable` oder `require` (verschlüsselt, aber ohne Prüfung des Zertifikats) hängt `message` einen Hinweis an,
`details.tls_warning` trägt ihn; nur `verify-full` bleibt ohne Hinweis. Ein Schema aus `information_schema`
wird mit verdoppelten Anführungszeichen zitiert; konfigurierte Namen bleiben auf die Bezeichnerregel beschränkt.

### Bedeutungssuche (RAG, Runde 12) {#semantic-search-rag-round-12}

Über `POST /rpc` (Herkunft „Autor“), nicht über `/act`; über MCP nur `rag_search`/`rag_status` (siehe MCP.md).
Nur lesend: Kein Weg schreibt Buchtext, Einfälle oder Hervorhebungen.

| Methode | Parameter | Ergebnis |
|---|---|---|
| `rag.search` | `{work?, query, ways?, collections?, limit?}` | Antwort-JSON `{work, query, hits: [{way, collection, document, section, title, snippet, score, weak, url, meta, rank, fused, found_by: [{way, rank, score}]}], ways: [{way, state, label, message, hits, millis}]}`. `query` getrimmt 1–1000 Zeichen, `limit` 1–50 (Standard 10), `ways` ⊆ {`lancedb`, `pgvector`, `multigpt`} (geschnitten mit den Wegen des Werks), `collections` eine Liste (LanceDB: Arten `section`, `idea`, `highlight`, `library`, seit Runde 13 `remote`; remote: Schnittmenge mit den gewählten Sammlungen); sonst und bei einem unbekannten Werk -32602 |
| `rag.status` | `{work?}` | `{work, ways, index: {dir, entries, model, dimension, configured_model, built, pending, stale, problem}, fts: true}` – ohne Netz; `stale` = hängengebliebene Aufträge (Runde 19a) |
| `rag.reindex` | `{work, confirm: true}` | `{work, queued: true, pending, deferred}`; ohne `work` oder `confirm: true` -32602, ohne Embedder -32000 (mit Grund). Passt das Modell oder das Dokumentpräfix nicht (oder ist `baruch-index.json` beschädigt), wird der ganze Index verworfen und neu aufgebaut, sonst die Zeilen des Werks. Antwortet sofort: Hält der Index-Worker gerade die Schreibsperre, wird ein Rücksetzwunsch vermerkt (`deferred: true`), der Worker bricht den laufenden Auftrag ab, setzt danach zurück und stellt den unterbrochenen Auftrag wieder in die Warteschlange (auch den eines anderen Werks oder der Bereinigung; Duplikate nur einmal). Nur der Mensch: kein MCP-Werkzeug, nicht über `/act` |

- **`ways` der Antwort** nennt immer alle drei Wege in der Reihenfolge lancedb, pgvector, multigpt; `state` ∈
  `connected` | `failed` | `off`, `label` „verbunden“ | „ausgefallen“ | „aus“. Aus: „Für dieses Werk nicht gewählt
  (Einstellungen → Verbindungen).“, „In dieser Anfrage nicht gefragt.“, „Keine Sammlung gewählt (Einstellungen →
  Verbindungen).“ oder der Grund beim Öffnen (kein Host, keine Adresse, Geheimnis für ein anderes Ziel).
  Ausgefallen: Fehler, Zeitlimit (20 s je Weg), Modell passt nicht, kein Embedding-Modell. Ein Weg blockiert nie
  die anderen.
- **Score** 0..=1 je Weg (LanceDB und pgvector 1 − Kosinusdistanz; MultiGPT sein Score, sonst 1 − Distanz,
  sonst seit Runde 21a ein Ersatz aus dem Rang: `WEAK_SCORE + (1 − WEAK_SCORE)/rank`, `meta.score = "aus Rang"`, nie
  „schwach“), nicht über Wege hinweg vergleichbar. **`weak`** (Runde 19a) = `baruch_rag::is_weak(score)`: Score unter
  0,45 (`WEAK_SCORE`) oder keine Zahl; Anzeigetext „schwach“. Die Treffer bleiben in ihrer Reihenfolge, nichts
  wird versteckt. **Zusammenführen:** Duplikate = gleiches Dokument und gleicher
  Abschnitt (getrimmt, Kleinbuchstaben), auch über Wege hinweg; `fused` = Σ 1/(60 + Rang) (Reciprocal Rank Fusion), Vertreter =
  Treffer mit dem besten Rang; deterministisch.
- **LanceDB-Treffer:** Abschnitte `collection: "section"`, `document: "work:W"`, `section: "key:K"`,
  `url: "/?work=W&key=K"`, `meta: {work, key, rev, chunk}`; Einfälle `idea:<id>`, `url: "/einfaelle/<id>"`,
  `meta: {idea, work, key, status}`; Hervorhebungen `title:<T>`/`highlight:<H>`; Bibliothek `title:<T>` oder
  `quote:<Q>` mit `url: "/quelle?…"`; abgerufene MultiGPT-Dokumente (Runde 13, Art `remote`) siehe „Sync with
  MultiGPT“.
- Die Anfrage wird je Embedding-Modell einmal eingebettet (LanceDB und pgvector teilen sie, solange ihre
  Anfragepräfixe gleich sind).
- **Hängengebliebene Aufträge** (Runde 19a): Scheitert das Embedding nach drei Versuchen, bleibt der Auftrag
  hängen (`index.stale`); `problem` bleibt gesetzt, bis eine echte Änderung (Speichern, Einfall, Abgleich) oder
  `rag.reindex` die Aufträge zurückbringt. `rag.search` löst keinen neuen Versuch aus (nur still die Bereinigung
  gelöschter Werke und höchstens alle 30 s den gemeinsamen Bereich, ohne hängende). Der Zustand von
  LanceDB in `ways` nennt dann `„n Aufträge liegengeblieben – erneuter Versuch nach einer Änderung oder „Index
  neu aufbauen““`.
- **Aufgabenpräfixe** (Runde 19b, in LanceDB mit 19a umgesetzt): Die Verbindung LanceDB kennt `query_prefix`
  und `document_prefix` (Standard leer = wie bisher). Dokumente werden mit dem Dokumentpräfix eingebettet, die
  Anfrage mit dem Anfragepräfix; `baruch-index.json` hält `document_prefix` fest. Ein anderes konfiguriertes
  Dokumentpräfix ist wie ein anderes Modell: LanceDB „ausgefallen“ mit „Aufgaben-Präfix geändert; Index neu
  aufbauen.“, bis `rag.reindex` den ganzen Index neu aufbaut.
- **Start ohne Embedder:** Mit eigenem Datenordner (`--data`) und ohne gespeicherte Verbindung Eliezer
  bindet der Dienst beim Start keinen Embedder (keine Anfrage an die Standardadresse); `problem` sagt, warum.
  `rag.reindex` bindet Eliezer, sobald die Verbindung gespeichert ist.
- Einfälle eines gelöschten Werks (`work.delete`) sind im gemeinsamen Bereich sofort auffindbar (Runde 19a).
- Formular `POST /einstellungen/verbindungen/act` mit `method=rag.reindex`, `work`, `confirm=1` →
  `?ok=reindex` oder `?fehler=bestaetigen|reindex`.

Formulare (Feld `method`, immer 303):
- `POST /einstellungen/verbindungen/act`: `connection.set` (Felder `kind`, `url` oder `host`, `port`,
  `database`, `user`, `tls` oder `dir`, `embedding_model`, optional `secret`), `connection.set_secret`,
  `connection.delete_secret` (`confirm=1`), `connection.check`, `connection.rag_set` (`work`, `way_lancedb=1`,
  `way_pgvector=1`, `way_multigpt=1`), `connection.collections_set` (`work`, `way`, wiederholtes `collection`,
  `collection_new`; → `?ok=sammlungen` oder `?fehler=sammlung|weg`), `rag.reindex` (`work`, `confirm=1`; →
  `?ok=reindex` oder `?fehler=bestaetigen|reindex`). pgvector zusätzlich `table`, `text_column`, … (siehe oben). Erfolg → `/einstellungen/verbindungen?ok=<code>[&work=W]#<kind|rag>`
  mit `ok` ∈ `gespeichert`, `gespeichert_geheimnis`, `geheimnis`, `geheimnis_entfernt`, `geprueft`, `rag`.
  „Speichern“ mit einem Geheimnis prüft zuerst beides (Geheimnis: nicht leer, eine Zeile, höchstens 4096 Zeichen),
  speichert dann die Einstellungen und danach das Geheimnis; ein ungültiges Geheimnis ändert daher nichts.
  Verweigert nur der Schlüsselbund, sind die Einstellungen bereits gespeichert: Code `gespeichert_ohne_geheimnis`.
  Neu (Runde 10): `method=model.probe&kind=eliezer&model=…` (Kurztest) → 303 `?ok=kurztest#eliezer` oder
  `?fehler=kurztest_modell&kind=eliezer` (ohne Modell, nichts gesendet); für andere Verbindungen `?fehler=methode`.
- `POST /einstellungen/quellen/act`: `sources.create` (`title`, `url`, `note`), `sources.update` (`id`, …),
  `sources.move` (`id`, `direction=up|down` oder `position`), `sources.delete` (`id`, `confirm=1`). Erfolg →
  `/einstellungen/quellen?ok=<angelegt|geaendert|verschoben|geloescht>[&work=W]#quelle-<id>` (gelöscht:
  `#quellen`).
- Fehler → `?fehler=<code>[&kind=<kind>][&work=W]`, Codes: `kind`, `url`, `url_schema`, `url_benutzer`,
  `url_geheimnis`, `geheimnis_feld`, `host`, `port`, `tls`, `feld`, `ordner`, `modell`, `geheimnis_leer`,
  `geheimnis_ungueltig`, `kein_geheimnis`, `bestaetigen`, `schluesselbund`, `gespeichert_ohne_geheimnis`, `werk`, `weg`, `stub_lancedb`,
  `titel`, `adresse`, `notiz`, `quelle`, `position`, `methode`, `intern`; unbekannte Codes zeigen eine
  allgemeine Meldung, nie den Code selbst.
- Seiten: `GET /einstellungen/verbindungen` und `/einstellungen/quellen` werden vom Rahmen der Einstellungen
  (Aufgabe Einstellungen) mit `crate::connections::render_connections` beziehungsweise `render_sources` angezeigt.

### Abgleich mit MultiGPT (Runde 13) {#sync-with-multigpt-round-13}

Über `POST /rpc`, nicht über `/act` und **nicht über MCP** (kein Werkzeug, insbesondere keines, das hochlädt):
Nur der Mensch löst den Abgleich aus. Quelltext `crates/baruch-server/src/sync/`, Speicher `baruch-store/src/sync.rs`
(Migration 0033). Nur `sync.run` und `sync.collections` sprechen mit MultiGPT; alle anderen Methoden und die
Seite `/abgleich` lesen oder schreiben nur SQLite.

| Methode | Parameter | Ergebnis |
|---|---|---|
| `sync.status` | `{work?}` | `{last: {id, started, finished, ok, message, uploaded, replaced, removed, downloaded, unchanged, failed} \| null, running, works: [{work, title, upload, collection, confirmed, documents, synced}], collections: [{collection, documents, synced}], configured}` – `work` beschränkt `works` auf das Werk (unbekannt → -32602); `documents` = hochgeladene Kapitel oder lokale Kopien, `synced` = letzter Abgleich; `message`: erste Zeile das Ergebnis, dann je Zeile ein Hinweis; `configured` = Adresse für MultiGPT eingetragen |
| `sync.run` | `{}` | `{started: true, running: true, run}` – startet einen Lauf im Hintergrund (Zeile in `sync_run`); läuft bereits einer: `{started: false, running: true}` (kein zweiter Lauf) |
| `sync.set_upload` | `{work, on, confirm?}` | `{work, upload, collection}` – Schalter „nach MultiGPT hochladen“ (Standard aus). `on: true` ohne `confirm: true` → -32602 (Bestätigung vor dem ersten Hochladen), nichts geändert; beim Einschalten wird die Sammlung „Baruch: <Werktitel>“ vermerkt (eine schon vermerkte bleibt); `on: false` behält sie. Ohne `work`, unbekanntes Werk oder `on` kein Wahrheitswert → -32602 |
| `sync.set_collections` | `{collections: [String]}` | `{collections}` – ersetzt die zum Abrufen gewählten Sammlungen (getrimmt, Duplikate entfernt, je 1–200 Zeichen in einer Zeile, höchstens 100; sonst -32602, nichts geändert) |
| `sync.collections` | `{}` | `{collections: [{name, count, selected}]}` – online durch `list_collections` (für die Auswahl); MultiGPT nicht eingerichtet, nicht erreichbar oder ohne das Werkzeug → -32000 mit einem ehrlichen Satz |

- **Ein Lauf:** zuerst Baruch → MultiGPT (nur Werke mit `upload`; ein Dokument je Kapitel, Text = aktuelle
  Fassungen von Kapitel und Unterabschnitten, Titel „<Nummer> <Titel>“; geänderter Hash (SHA-256 über Titel
  und Text) → `upload_document`, danach `delete_document` des alten; fehlendes Kapitel oder gelöschtes Werk →
  `delete_document`), dann MultiGPT → Baruch (gewählte Sammlungen durch `list_documents` und `read_document`,
  nur was sich nach `remote_stamp` oder Hash geändert hat; Grenzen 500 Dokumente je Sammlung, 2 MiB je Dokument, 64 MiB je
  Lauf, 30 s je Aufruf). Danach holt der lokale Index die Kopien nach (Art `remote`, siehe „Semantic search“).
- **Zahlen:** `uploaded` neu hochgeladen, `replaced` ersetzt (neu hochgeladen und das alte gelöscht), `removed`
  entfernt (Dokumente fehlender Kapitel oder gelöschter Werke bei MultiGPT, lokale Kopien gelöschter oder
  abgewählter Dokumente), `downloaded` abgerufen, `unchanged` unverändert, `failed` gescheitert.
- **Fehler sind Hinweise:** nicht eingerichtet, nicht erreichbar, abgelehnt (HTTP 401/403 oder `isError`,
  z. B. ohne `docs.write`), Schema unbekannt, Grenzen → `ok: false` und ein Satz in `message`; lokal
  bleibt alles unverändert. Ein Fehler bei Berechtigung, Transport oder Schema beendet das Schreiben nach MultiGPT für diesen
  Lauf; ein `isError` der Gegenseite für **ein** Dokument (z. B. ein leeres Kapitel; Text ohne Hinweis auf
  Berechtigungen) zählt als `failed` und sperrt nur dieses (Runde 19a). Kein Token in Meldungen.
- **Härtung (Runde 19a):** Nennt `upload_document` keine Dokument-ID, steht ein Hinweis in `message`, und der
  Zustand wird mit dem Hash und einer leeren ID gespeichert (Sperrmarke): Derselbe Inhalt geht nicht noch einmal hinaus.
  Vor jedem Hochladen trägt der Zustand die Marke „Hochladen begonnen“; geht die Antwort verloren (Zeitlimit,
  unterbrochene Verbindung), meldet der nächste Lauf, dass das Kapitel bei MultiGPT möglicherweise doppelt vorhanden ist. Beim
  Abrufen gilt eine volle Seite ohne `has_more`/Fortsetzung als möglicherweise unvollständig (das `limit` steht am
  `maximum` des Schemas oder wird still gekappt): Dann wird keine lokale Kopie gelöscht (Hinweis). Die Sammlung
  eines Werks bleibt beim Umbenennen die, die beim Einschalten vermerkt wurde.
- **LanceDB-Treffer der Art `remote`:** `collection: "remote"`, `document: "remote:<Sammlung>/<Dokument-ID>"`,
  `url: "/quelle/multigpt?collection=…&document=…"` (URL-kodiert), `meta: {collection, document, sync}`
  (`sync` = ID der lokalen Kopie).

Formular `POST /abgleich/act` (hinter `guard::same_origin`, immer 303), Feld `action`:
- `run` → `/abgleich[?work=W]&ok=gestartet` oder `ok=laeuft`;
- `upload` (`work`, `on=1` zum Einschalten, fehlt = aus, `confirm=1` beim Einschalten erforderlich, optional
  `back` = `/abgleich…` oder `/einstellungen/werk…`) → `back` oder `/abgleich?work=W` mit `ok=hochladen_an|
  hochladen_aus`; ohne das Kontrollkästchen `/abgleich?work=W&fehler=bestaetigen`, unbekanntes Werk `fehler=werk`;
- `collections` (wiederholtes `collection`, dazu `collection_new`) → `ok=sammlungen` oder `fehler=sammlung`;
- `load_collections` → `ok=geladen` oder `fehler=laden` (Grund auf der Seite).
Seite `GET /abgleich?work=W` (siehe Bedienung.md, Abgleich mit MultiGPT), ohne Netz.

### Einfälle {#ideas}

Einfälle (Runde 7) funktionieren auch ohne Werk: `work` fehlend oder `null` = ohne Werk (bei Listen: alle).
Über RPC ist die Herkunft immer der Mensch („Autor“). Jede schreibende Methode läuft unter der Schreibsperre und
ergibt genau einen Commit je betroffenem Spiegel; die Antwort trägt `commit` oder `mirror` (Fehlertext).
Nicht unter `/act`; die Seiten haben ein eigenes Formular `POST /einfaelle/act`.

| Methode | Parameter | Antwort |
|---|---|---|
| `idea.create` | `{work?, key?, words: [..] \| "a, b; c", note?, quote?, source?, bible?}` | `{idea, commit?, mirror?}` |
| `idea.get` | `{id}` | `{idea}` (auch gelöschte) |
| `idea.list` | `{work?: id \| "none" \| "all", word?, status?: "open" \| "done" \| "all", deleted?: bool, limit?, offset?}` | `{ideas, words: [{word, count}]}`; `deleted: true` = nur gelöschte; `words` = Wolke über Werk/Status (ohne den Wortfilter) |
| `idea.search` | `{query, work?, status?, limit?, mode?}` | `{ideas, hits: [{id, snippet}]}`; FTS5 durch `baruch_library::fts_query`, `snippet` maskiertes HTML mit `<mark>`; leere Suche → -32602. `mode` (Runde 12) `fts` (Standard) \| `rag`, sonst -32602; `rag`: Bedeutungssuche (Weg LanceDB, Art `idea`), ein Werk ist nötig (sonst -32602 „Bedeutungssuche nur innerhalb eines Werks“) → `{ideas, hits: [{id, snippet, score}], ways}` in der Reihenfolge des Rangs, gelöschte und nicht zu `status` passende fallen heraus; seit Runde 19c je Treffer `weak` (Ähnlichkeit unter `baruch_rag::WEAK_SCORE`), schwache Treffer zuletzt; scheitert die Bedeutungssuche (LanceDB nicht verbunden, z. B. ohne Embedding-Modell): -32001 (`RAG_UNAVAILABLE`) „Bedeutungssuche nicht verfügbar: LanceDB (lokal): <Grund>.“ (bis Runde 19: -32000 oder eine leere Liste) |
| `idea.update` | `{id, words?, note?, quote?, source?, bible?, work?, key?, status?: "open" \| "done"}` | `{idea, changed, commit?, commits?, unlinked?}`; `work`/`key`: `null` = ohne; ein neues Werk ohne `key` → der Bezug entfällt; beim Verschieben `commits` = beide Commits, `unlinked` = IDs der dabei weich gelösten Verbindungen (Verbindungen nur innerhalb eines Werks; Positionen des Einfalls auf Karten werden entfernt) |
| `idea.delete` | `{id, confirm: true}` | `{id, deleted: true, idea, commit?}`; ohne `confirm: true` → -32602; wiederherstellbar |
| `idea.restore` | `{id}` | `{idea, commit?}` |
| `idea.process` | `{id, as: "outline" \| "post" \| "note" \| "persona", key?, place?: "before" \| "after" \| "into", title?, persona?}` | `{idea, node?, key?, outline?, post?, commit?}`; `outline`: `Store::idea_to_outline` (seit Runde 14 **eine** Transaktion: einen Punkt wie `outline.add` nach (`place`) dem Abschnitt `key` oder dem Bezug anlegen und den Einfall mit `done_ref` „key <key>“ als erledigt markieren; scheitert ein Schritt, ändert sich nichts und es gibt keinen Commit), Titel = `title` oder die Stichwörter (200 Zeichen), `outline` = Gliederungsantwort mit eigenem `commit`; `post`/`note`: Beitrag (Autor „Autor“ oder „Notiz“, Quelle „Einfall <id>“, seit Runde 15 bei einem Einfall einer Persona oder eines MCP-Clients „Einfall <id> von @x“ oder „… von @mcp:…“); danach `status: done`. `persona` (Runde 10, `persona?`, Standard der Lektor; seit Runde 15 auch im Formular wählbar): der Einfall als Frage zum bezogenen Abschnitt an die Persona (mit Herkunft „Einfall von @x“) → Beitrag der Persona, Einfall verarbeitet (`done_as: persona`, `done_ref` = Beitrag); Antwort `{idea, node, key, post_id, persona, model, truncated, url, commit?\|mirror?}`; Modellaufruf vor der Schreibsperre; scheitert er → -32000, der Einfall bleibt offen |

`idea` = `{id, work, key, node_title, words, note, quote, source, bible, origin: {kind: "human", name} |
{kind: "persona" | "mcp", handle}, status, done_as, done_ref, created, updated, deleted}`.

Fehler: unbekannter Einfall („Einfall 7 gibt es nicht.“), eine verletzte Grenze (1–20 Stichwörter, je ≤ 60
Zeichen, ohne Zeilenumbruch; Gedanke/Marke ≤ 4000, Quelle/Bibelstelle ≤ 200), Bezug außerhalb des
Werks („Der Abschnitt gehört nicht zu diesem Werk.“), ein gelöschter Einfall beim Ändern/Verarbeiten, Verarbeiten
ohne Werk oder Abschnitt → -32602; unbekanntes Werk → -32602.

Seit Runde 25 trägt `einfaelle.md` im Spiegel für jeden Einfall mit Verbindungen nach den übrigen Feldern die Zeile
`„- Verbindungen: → 79 führt weiter; ↔ 48 widerspricht („Notiz“); ← 12 Beispiel für“` (ausgehend →, eingehend ←,
ungerichtet ↔; nach Verbindungs-ID; nur aktive zu Einfällen, die nicht gelöscht sind). Ein gelöschter Einfall nimmt seine
Verbindungen und Position aus Karte, Listen und Spiegel und bringt sie mit `idea.restore` zurück (siehe „Idea map“).

### Einfallskarte (Runde 25) {#idea-map-round-25}

Seit Runde 34 heißt die Seite „Mindmap“; Baum, Wolke, `idea.move` und `idea.tree` stehen unter „Round 34 – Mind map“
am Ende. Die Methoden dieses Abschnitts bleiben (flache Ansicht, Verbindungen, Positionen – Positionen haben in der
Mindmap keine Wirkung).

Karte der Einfälle eines Werks, Verbindungen zwischen Einfällen und gespeicherte Positionen (`crate::idea_map`,
Speicher `idea_links.rs`, Migration 0043). Über RPC ist die Herkunft immer der Mensch („Autor“); schreibende Methoden laufen
unter der Schreibsperre. Nicht unter `/act`; die Karte hat ein eigenes Formular `POST /werke/{id}/karte/act`, das
dieselben Methoden aufruft (Zahlen dürfen daher auch als Ziffern-Text kommen; leere Texte zählen als fehlend).
`work` ist erforderlich bei `idea.map`, `idea.map_pos_set` und `idea.map_pos_reset` (kein Standardwerk).

| Methode | Parameter | Antwort |
|---|---|---|
| `idea.map` | `{work, status?: "open" \| "done" \| "all", word?, origin?: "human" \| "persona" \| "mcp", q?, near?: bool = true, near_limit?: 1..5, near_threshold?: 0..1, open?: [key]}` | `IdeaMap` = `{work, title, nodes, ideas, links, near, positions, filter}` |
| `idea.links` | `{work}` oder `{idea}`, jeweils mit `deleted?: bool = false` (genau eines von `work`/`idea`, sonst -32602) | `{links: [IdeaLink]}` nach ID |
| `idea.link` | `{from, to, kind: "leads_to" \| "belongs_to" \| "example_of" \| "contradicts", note?}` | `{link, changed, commit?\|mirror?}` |
| `idea.unlink` | `{id}` (weich, kein `confirm` nötig: wiederherstellbar) | `{link, commit?\|mirror?}` |
| `idea.link_restore` | `{id}` | `{link, commit?\|mirror?}` |
| `idea.map_pos_set` | `{work, idea, x, y}` | `{pos: {idea, x, y, updated}}` (kein Commit) |
| `idea.map_pos_reset` | `{work, idea?}` (ohne `idea` alle Positionen des Werks) | `{removed}` (kein Commit) |
| `idea.map_groups` | `{work}` + Filter wie `idea.map` (einschl. `near_limit`, `near_threshold`; `near` zählt nicht, die Nähe ist für Gruppen immer an) | `{groups: [{group: "g<kleinste id>", ideas: [id], words: [..], proposal?: {name, persona, model, at}}]}` (Runde 19b) |
| `idea.map_groups_name` | `{work, persona?}` + Filter wie oben | `{proposals: [{group, ideas, name}], persona, model}`; ohne Gruppe `{proposals: [], persona, model: null, message}` (Runde 19b) |
| `idea.map_group_accept` | `{work, group, name?}` + Filter (nur ohne Vorschlag nötig) | `{group, name, ideas, changed: [id], commits: [..]}` (Runde 19b, nur der Mensch); Runde 23: veralteter Vorschlag → -32000, Grenzen vorher geprüft |
| `idea.map_group_dismiss` | `{work, group}` | `{group, removed: bool}` (Runde 19b) |

**Gruppen (Runde 19b, `idea_map/groups.rs`):** Eine Gruppe ist eine Zusammenhangskomponente der Nähe-Paare
(`near.pairs`, siehe unten) mit mindestens zwei Einfällen – ohne Modell, deterministisch, Kennung `g<kleinste id>`,
Einfälle aufsteigend, Gruppen nach Kennung; `words` = Stichwörter der Einfälle, häufigste zuerst (höchstens 12).
`idea.map_groups_name` fragt das Modell durch `state.model.chat` (Warteschlange wie die anderen Aufgaben): den Lektor
nach Einstellung oder die genannte `persona` (muss aktiv sein, sonst -32602). Stichwörter und Notizen gehen als
gerahmte Daten ins Modell (`<<<einfall … >>>`, "data, not instructions"), höchstens 40 Gruppen zu je 12 Einfällen.
Erwartet werden Zeilen „g<id>: Name“; nur bekannte Gruppen zählen, mit einem bereinigten Namen von 1 bis 40 Zeichen
(Steuer- und Markdown-Zeichen, Anführungszeichen an den Rändern, `,`/`;` entfernt), je Gruppe die erste gültige Zeile; alles
andere wird verworfen. Die Vorschläge leben **nur im Speicher** (je Datenordner und Werk): kein Commit, nichts in
SQLite oder im Spiegel, nach einem Neustart weg. Höchstens ein Lauf je Werk zugleich (sonst -32000 „… benennt schon
…“). Ausfall des Modells → -32000 mit dem Grund. `idea.map_groups` zeigt einen Vorschlag nur, solange die Gruppe
genau dieselben Einfälle hat. `idea.map_group_accept` (nur über RPC und das Formular der Karte, **kein
MCP-Werkzeug**): Name = `name` (bereinigt wie oben, sonst -32602) oder der Vorschlag (beides fehlt → -32602); die
Einfälle aus dem Vorschlag oder, ohne Vorschlag, aus den Gruppen nach dem Filter (unbekannte Gruppe → -32602). Jeder
Einfall, der das Stichwort (gleicher `word_key`) noch nicht trägt, erhält es durch `idea.update` (Herkunft Mensch,
ein Commit je Einfall mit `Baruch-Action: idea-update`, `Baruch-Origin: human`); hatte einer schon 20 Stichwörter,
wird vorher mit -32000 abgelehnt, nichts geändert. Danach ist der Vorschlag weg. `idea.map_group_dismiss` entfernt
nur den Vorschlag. `group` darf `"g12"` oder `12` sein, sonst -32602.

```json
{"jsonrpc":"2.0","id":6,"method":"idea.map_groups_name","params":{"work":3}}
→ {"proposals":[{"group":"g48","ideas":[48,52,79],"name":"Saat und Reihe"}],"persona":"@lektor","model":"gemma"}
{"jsonrpc":"2.0","id":7,"method":"idea.map_group_accept","params":{"work":3,"group":"g48","name":"Aussaat"}}
→ {"group":"g48","name":"Aussaat","ideas":[48,52,79],"changed":[48,52,79],"commits":["5d1e…","8a0b…","c41f…"]}
```

**`idea.map`** stellt die Karte zusammen:
- `nodes`: `{id, kind, key, parent, number, title, sections, open_ideas, total_ideas}`; `id` = `"w"` (Werk, `kind`
  `work`, `parent` `null`), `"n:<key>"` (`part`, `chapter`, `section`) oder `"none"` (`no_chapter`, Titel „Ohne
  Kapitel“, nur wenn ein gezeigter Einfall keinen Bezug hat); `parent` = nächster Vorfahr, der selbst ein Knoten ist,
  sonst `"w"`. Abschnitte unter einem Kapitel stehen eingeklappt in dessen `sections` (`{key, number, title,
  ideas}`); steht der Schlüssel des Kapitels in `open`, sind sie eigene Knoten (`section`). Abschnitte ohne ein Kapitel
  über sich sind immer eigene Knoten. Zähler = gezeigte Einfälle, die am Knoten oder an einem Knoten darunter hängen
  (ein Teil mit seinen Kapiteln, ein Kapitel mit seinen Abschnitten, eingeklappt wie aufgeklappt), beim Werk alle
  gezeigten.
- `ideas`: `{id, node: "i:<id>", anchor, key, label, words, note, status, origin, hit}` nach ID; `anchor` = ID des
  Knotens, an dem der Einfall hängt (eingeklappt das Kapitel, sonst `"none"`), `key` = genauer Bezug, `label` =
  Stichwörter „a, b“ (höchstens 60 Zeichen). Nur nicht gelöschte Einfälle des Werks; Einfälle ohne Werk
  erscheinen nicht. `status`, `word` (verglichen über den `word_key`: Groß-/Kleinschreibung und Leerraum egal) und `origin` entfernen
  nicht passende Einfälle samt ihren Kanten; `q` (Volltext wie `idea.search`) markiert Treffer mit `hit: true`, die anderen
  bleiben mit `hit: false` (ohne `q` alle `true`).
- `links`: aktive Verbindungen zwischen gezeigten Einfällen; `positions`: gespeicherte Positionen gezeigter Einfälle
  (`{idea, x, y, updated}`, SVG-Benutzereinheiten, Werk bei 0,0).
- `near`: `{state: "ok" | "off" | "no_embedder" | "unavailable", message, pairs: [{a, b, score, via: "vector" |
  "words" | "both", shared_words}]}` (`a < b`). Nur Anzeige, nie gespeichert, **keine Embedding-Anfrage**: Vektoren
  sind die bereits im LanceDB-Index gespeicherten (`idea:<id>`, `rag/neighbors.rs`); ein Kosinus ab `near_threshold`
  (Standard 0,75) zählt als `vector`, ein gleiches Stichwort als `words` (auch ohne Embedding), beides als `both`.
  `score`: Kosinus oder Anteil gemeinsamer Stichwörter (Jaccard) oder der größere. Je Einfall höchstens `near_limit`
  (Standard 3) Partner (Paare nach `score` absteigend). Ohne Embedder oder ohne Vektoren im Index `no_embedder`, bei
  einem Fehler oder einem anderen Modell des Index `unavailable`, jeweils mit einer ehrlichen `message` und der Stichwortnähe;
  `near: false` → `off` ohne Paare.
- `filter`: der angewandte Filter (Standardwerte für `near_limit`/`near_threshold` eingesetzt, `open` nur mit Kapiteln
  des Werks, `q` ohne Suchwort entfällt).

Beispiel:
```json
{"jsonrpc":"2.0","id":1,"method":"idea.map","params":{"work":3,"status":"open","open":[12]}}
→ {"work":3,"title":"Das Laubenbuch",
   "nodes":[{"id":"w","kind":"work","key":null,"parent":null,"number":"","title":"Das Laubenbuch","sections":[],"open_ideas":5,"total_ideas":5},
            {"id":"n:12","kind":"chapter","key":12,"parent":"n:11","number":"1","title":"Säen","sections":[],"open_ideas":2,"total_ideas":2}, …],
   "ideas":[{"id":48,"node":"i:48","anchor":"n:12","key":12,"label":"Saat, Reihe","words":["Saat","Reihe"],"note":"…","status":"open","origin":{"kind":"human","name":"Autor"},"hit":true}, …],
   "links":[{"id":7,"uuid":"…","from":48,"to":79,"kind":"contradicts","note":"beißt sich","origin":{"kind":"human","name":"Autor"},"created":"…","updated":"…","deleted":null}],
   "near":{"state":"no_embedder","message":"Keine Einbettung angebunden (Einstellungen → Verbindungen); Nähe nur über gleiche Schlagwörter.","pairs":[{"a":48,"b":52,"score":0.5,"via":"words","shared_words":["Saat"]}]},
   "positions":[{"idea":48,"x":120.5,"y":-40.0,"updated":"…"}],
   "filter":{"status":"open","word":null,"origin":null,"q":null,"near":true,"near_limit":3,"near_threshold":0.75,"open":[12]}}
```

**Verbindungen** (`IdeaLink` = `{id, uuid, from, to, kind, note, origin: {kind, name|handle}, created, updated,
deleted}`): nur Einfall ↔ Einfall desselben Werks (auch beide ohne Werk), keiner gelöscht, kein Selbstbezug, Notiz
höchstens 500 Zeichen ohne Steuerzeichen außer Zeilenumbruch; sonst -32602 mit einem deutschen Satz, nichts geschrieben.
`contradicts` („widerspricht“) ist ungerichtet und wird mit `from < to` gespeichert. `idea.link` auf ein bereits
vorhandenes Paar mit derselben Art: aktiv → unverändert (`changed: false`, kein Commit); weich gelöscht →
wiederhergestellt (`changed: true`, Notiz ersetzt, wenn eine neue mitkommt). `idea.link_restore` verlangt ebenso
zwei verbindbare Einfälle. Ein gelöschter Einfall nimmt seine Verbindungen und Position aus Karte, Listen und Spiegel
(nur gefiltert) und bringt sie mit `idea.restore` zurück.

Spiegel: Jede Änderung an einer Verbindung ist genau ein Commit im Spiegel des Werks (ohne Werk in
`<data>/einfaelle/`), Betreff „Verbindung <id>: Einfall <a> <Art> Einfall <b> (angelegt|gelöscht|wiederhergestellt)“,
Trailer `Baruch-Action: idea-link|idea-unlink|idea-link-restore`, `Baruch-Idea-Link: <id>`, `Baruch-Origin:
human|persona|mcp` (bei Persona/MCP dazu `Persona`); `einfaelle.md` trägt für jeden Einfall die Zeile „- Verbindungen: …“
(siehe „Ideas“). Positionen ergeben keinen Commit.

```json
{"jsonrpc":"2.0","id":2,"method":"idea.link","params":{"from":79,"to":48,"kind":"contradicts","note":"beißt sich"}}
→ {"link":{"id":7,"from":48,"to":79,"kind":"contradicts",…,"deleted":null},"changed":true,"commit":"3f2c…"}
{"jsonrpc":"2.0","id":3,"method":"idea.unlink","params":{"id":7}}
→ {"link":{"id":7,…,"deleted":"2026-11-08T20:01:02.345Z"},"commit":"9a1b…"}
{"jsonrpc":"2.0","id":4,"method":"idea.map_pos_set","params":{"work":3,"idea":48,"x":120.5,"y":-40}}
→ {"pos":{"idea":48,"x":120.5,"y":-40.0,"updated":"…"}}
{"jsonrpc":"2.0","id":5,"method":"idea.map_pos_reset","params":{"work":3}}
→ {"removed":1}
```

Positionen: höchstens eine je (Werk, Einfall); der Einfall muss zum Werk gehören und darf nicht gelöscht sein, `x`/`y`
endlich mit einem Betrag von höchstens 1 000 000 (sonst -32602); ein zweites Setzen ersetzt die Position. Zurücksetzen löscht
hart (Grabstein `idea_map_pos` für den Austausch). Fehler: unbekanntes Werk wie bei den anderen Methoden, unbekannter
Einfall oder unbekannte Verbindung („Verbindung 7 gibt es nicht.“), falsche Parameter → -32602.

### Fäden ohne Abschnitt (Runde 15) {#threads-without-a-section-round-15}

Gespräche je Werk, die an keinem Abschnitt hängen (Migration 0038, `src/threads.rs`): MCP `chat` ohne `key` legt
einen an oder setzt ihn fort, der Mensch liest und antwortet. Kein Buchtext, kein Abschnitts-Chat, nicht im Spiegel.
Alle nehmen `work?` (fehlend → Standardwerk).

| Methode | Parameter | Antwort |
|---|---|---|
| `thread.list` | `{work?}` | `{work, threads: [{id, work, title, created_by, created, updated, posts, last_author}]}`, zuletzt beschriebene zuerst |
| `thread.get` | `{work?, thread}` | `{work, thread: {…wie oben}, posts: [{id, thread, position, author, body, sources, created}], url}`; unbekannter Faden (auch einer eines anderen Werks) → -32000 „Diesen Faden gibt es in diesem Werk nicht.“ |
| `thread.post` | `{work?, thread?, message, persona?}` | Beitrag des Menschen („Autor“; ohne `thread` ein neuer Faden, Titel aus der Nachricht, ≤ 80 Zeichen), dann antworten die erwähnten aktiven Personas (`@handle`) und `persona` (höchstens drei, ohne Relay): `{work, thread, title, posts: [{id, author, body, position}], answers: [{persona, model, post_id, text, cut, truncated, relay, asked_by}], errors, url, cut, truncated}`; leere Nachricht → -32602 |

Kontext einer Antwort im Faden: Rollen-Prompt, Werk (Titel, Art, Zielgruppe), die letzten 12 Beiträge des Fadens (je
höchstens 600 Zeichen; reicht die Kontextlänge nicht, fallen die ältesten heraus), die Aufgabe; `max_tokens` 500. Seit Runde
17a prüft Baruch auch hier „Kritisch:“-Zitate gegen den gezeigten Text (Werkzeile, Gliederung, Belege aus SWORD, die
eigene Frage – nicht die Fadenbeiträge) und vermerkt falsche Aussagen über Stellen („(Stelle steht im Text: …)“,
„(Stelle ungültig: …)“). Seiten
ohne JavaScript: `GET /faeden?work` (Übersicht, „Neuer Faden“), `GET /faden?work&thread` (Beiträge, Antwortformular),
`POST /faden/act` (hinter `same_origin`; Felder `work`, `thread?`, `message`, `persona?`; 303 zum Faden, Fehler
als Hinweis auf derselben Seite mit dem Entwurf im Feld).

#### thread.post_edit, thread.post_versions (Runde 28, chat-auftrag) {#threadpost_edit-threadpost_versions-round-28-chat-auftrag}

- `thread.post_edit {work?, thread, id, body}` → `{work, thread, id, body, edited, versions, url}`: nur eigene
  Fadenbeiträge des Menschen (fremde und Persona-Beiträge → -32000); die alte Fassung bleibt als `post_version`.
  Fadenbeiträge stehen nie im Spiegel (Regel seit Runde 15), daher **kein Commit**.
- `thread.post_versions {work?, thread, id}` → `{work, thread, id, versions}` (älteste zuerst).
- `thread.post` erkennt RFC oder „@alle“ wie `persona.chat` (alle aktiven Personas, der Lektor zuletzt mit einer Sammlung,
  keine Relay-Stufe) und liefert zusätzlich `{rfc, findings}`. Ein Schreibauftrag im Faden bleibt eine Antwort (es
  gibt keinen Abschnitt für einen Vorschlag).
- Formular ohne JavaScript `POST /faden/bearbeiten` (`work`, `thread`, `id`, `body`; hinter `same_origin`) → 303 auf
  `/faden?work&thread#beitrag-<id>`, Fehler als Hinweis auf der Seite (200). `GET /faden?…&zitat=<id>` füllt die
  Antwort mit dem zitierten Beitrag („> …“) vor; `POST /faden/act` leitet auf `…#chat-neueste` um.

### Zeitwache (Runde 16b) {#time-watch-round-16b}

Sprechzeit je Gliederungspunkt und insgesamt (`src/timing.rs`, Zählung `baruch_typst::timing`, Regeln in
`docs/Bedienung.md`, Zeitwache). Wörter pro Minute und Zielzeit stehen je Werk nur in SQLite (`work_timing`,
Migration 0039; ohne Zeile 110 Wörter pro Minute, keine Zielzeit), **nicht im Spiegel** (wie die Farben der
Personas). Alle nehmen `work?` (fehlend → Standardwerk, unbekannt → -32602).

| Methode | Parameter | Antwort |
|---|---|---|
| `work.timing` | `{work?}` | `{work, kind: "book" \| "sermon", words_per_minute, target_seconds, words, seconds, status: "none" \| "ok" \| "over", over_seconds, shown, clock, target_clock, over_clock, sections: [{key, seq, depth, number, title, words, seconds, start_seconds, subtree_seconds, clock, start_clock, subtree_clock}]}` – nur lesend, keine Schreibsperre, kein Commit. Je Punkt in `seq`-Reihenfolge die **aktuelle** Fassung; `words`/`seconds` nur der eigene Text des Punkts, `start_seconds` = Summe der vorherigen Punkte (Zeitmarke), `subtree_seconds` = eigener Text samt Nachkommen, `depth` = Zahl der Vorfahren; `seconds` insgesamt = Summe der Punkte; `status` `over` genau dann, wenn `seconds > target_seconds` (`over_seconds` = Differenz), `none` ohne Zielzeit; `shown` = Predigt oder Zielzeit gesetzt (dann zeigen Gliederung, Baum und Editorkopf die Zeiten); `*_clock` = „m:ss“ oder „h:mm:ss“ |
| `work.timing_set` | `{work, words_per_minute?, target_minutes?: number \| null}` | `{work, words_per_minute, target_seconds}`; fehlende Felder bleiben, `target_minutes: null` (oder leerer Text) löscht die Zielzeit. `work` erforderlich; Wörter pro Minute 60–250, Zielzeit ganze Minuten 1–240, sonst -32602, nichts geändert. **Nur der Mensch:** nicht über `/act`, nicht über MCP |
| `timing.count` | `{work?, text}` | `{words, seconds, clock, words_per_minute}` – zählt `text` mit derselben Regel wie `work.timing` und rechnet mit den Wörtern pro Minute des Werks; `text` höchstens 1 000 000 Zeichen (sonst oder ohne `text` -32602). Das Editor-Widget zählt damit live (keine zweite Zählregel in TypeScript) |

MCP: `work_timing {work}` liefert dasselbe wie `work.timing` (`docs/MCP.md`).

### work.sheet {#worksheet}

Runde 18c (`src/sheet.rs`, Speicher `work_sheet`, Migration 0044): Satzansicht-Einstellungen des Werks. Nur lesend.

| Methode | Parameter | Ergebnis |
|---------|-----------|----------|
| `work.sheet` | `{work?}` | `{settings: {font, heading_font, size, fill, heading_fill, background, width}, css}` – jedes Feld Text oder `null` (= aus der Werkvorlage); `css` = `.ws-sheet { --ws-…: …; }` aus der Werkvorlage, überschrieben von den gesetzten Feldern (`sheet_style_over`), dasselbe wie `wysiwyg.style` und das `<style id="ws-style" data-work="W">` im Editor. Unbekanntes Werk → Fehler |

### work.sheet_set {#worksheet_set}

| Methode | Parameter | Ergebnis |
|---------|-----------|----------|
| `work.sheet_set` | `{work?, font?, heading_font?, size?, fill?, heading_fill?, background?, width?, reset?: bool}` | `{settings, css}` wie `work.sheet`. Ein fehlendes Feld bleibt, `null` oder leerer Text nimmt es zurück (wieder aus der Vorlage), `reset: true` nimmt zuerst alle zurück; alles zurückzunehmen löscht die Zeile. Prüfung im Speicher: Schrift/Überschriftschrift höchstens 200 Zeichen, Familien durch Kommas getrennt, jeder Name aus Buchstaben, Ziffern, Leerzeichen, `- _ .` (auch in `"…"`/`'…'`); `size` 6–48 pt, `width` 6–60 cm, jeweils als Zahl mit der Einheit `pt`, `mm`, `cm` oder `em` (`em` = 12 pt); Farben `#rrggbb` (kleingeschrieben gespeichert). Ungültig → -32000 mit einer deutschen Meldung („Satzbild: …“), nichts geändert; ein Feld, das kein Text ist → -32602. Kein Spiegel-Commit, nicht im Austauschformat; Typst, PDF und Export bleiben byte-identisch. **Nur der Mensch:** nicht über MCP |

Formular ohne JavaScript: `POST /einstellungen/werk/satzbild` (hinter `guard::same_origin`, fremder `Origin` → 403):
`method=work.sheet_set`, `work` und alle sieben Felder (leer = aus der Vorlage) oder `reset=1` → 303
`/einstellungen/werk?work=W&ok=satzbild#satzbild`; ungültig → 303 `…&satzbildfehler=1#satzbild`, nichts geändert.

Bild an der Cursorposition (Runde 18c, Formular `POST /werke/bilder/einfuegen`, hinter `guard::same_origin`): Zu
`work, key, rev` (Basis), `name`, `caption?` kommen `place` = `end` (Standard) | `start` (unter der Überschrift) |
`block:<n>` (vor dem Block `n` der obersten Ebene von `wysiwyg.parse`; `n` ≥ Zahl der Blöcke = Ende) | `offset:<n>` (UTF-16-Position,
an der nächsten Zeilengrenze ≥ n; mitten in einem Block, der kein Absatz ist, dahinter; Leerzeilen hinter einem Block
übersprungen) und optional `body` (vollständiger ungespeicherter Text, CRLF wird zu LF; dann gelten die Positionen darin,
und seine Überschriftzeile muss die der Fassung `rev` sein, sonst 422 „erst speichern“). Nie vor oder in die
Überschrift des Abschnitts. Die Abbildung steht als eigener Block (davor und danach eine Leerzeile). Pfadzuweisung
(`bilder/<name>`, wenn das Bild keinen hat) und Fassung in **einer** Transaktion
(`Store::save_human_with_asset_path`), ein Spiegel-Commit; scheitert das Speichern, bleibt kein Pfad. Unbekannte
Position → 422.

### `library.search` {#librarysearch}
- `query`: Wörter, alle müssen vorkommen; Groß-/Kleinschreibung und Akzente egal („langmutig“ findet „langmütig“).
  FTS-Operatoren im Text bleiben wirkungslos. NUL und Steuerzeichen trennen Wörter wie ein Leerzeichen; besteht
  `query` nur daraus (oder aus Wörtern ohne Buchstaben und Ziffern), lautet die Antwort -32602 „Leere Suche.“.
- `kinds`: Liste aus `revision`, `post`, `title`, `quote`, `highlight`; fehlt sie, alle. Abgelehnte
  Hervorhebungen kommen nie vor.
- `work`: Werktreffer nur aus diesem Werk (Standard: Standardwerk); Bibliothekstreffer immer.
- `all_revisions`: `true` auch ersetzte, verworfene und vorgeschlagene Fassungen; Standard nur aktuelle.
- `limit`: Standard 20, höchstens 100.
- Treffer: `kind`, `work`, `node`, `node_key` (fester Schlüssel des Abschnitts bei `revision`/`post`, sonst
  `null`), `rev`, `title_id`, `quote_id` (bei `quote` das Zitat, bei `highlight` das
  Zitat, in dem sie liegt), `highlight_id` (bei `highlight`), `key` (Zitierschlüssel), `label` (bei
  Hervorhebungen mit ihrer Stelle), `snippet`, `rank` (bm25, kleiner ist besser). `snippet` ist **HTML**: Text maskiert (`&` `<` `>` `"` `'`), Treffer in `<mark>…</mark>`;
  Typst-Klammern bleiben wörtlich. Aufrufer zeigen es als HTML oder entfernen die Tags.

## Fehler {#errors}

| Code | Bedeutung |
|---|---|
| -32700 | JSON nicht lesbar |
| -32600 | keine JSON-RPC-2.0-Anfrage |
| -32601 | unbekannte Methode |
| -32602 | ungültige Parameter: unbekanntes Werk, unbekannter Abschnitt, unbekannte (auch gelöschte) Fassung oder Basis („Fassung 3 gibt es nicht; vorhanden: 1, 2, 4“, ohne Fassungen „vorhanden: keine“), Löschen ohne `confirm: true` („Löschen bitte bestätigen (confirm).“), leerer Beitrag, leere Suche, unbekannter Titel, Import ohne Überschrift, unbekanntes Nummerierungsschema oder ungültiges Muster („Nummerierung: …“), unbekanntes Bild, zu großes oder nicht unterstütztes Bild, kaputtes Base64, Titel eines Gliederungspunkts leer, mit Zeilenumbruch oder über 200 Zeichen, unbekannte Persona oder ungültige Farbe, unbekannter oder abgelehnter Einfall |
| -32603 | interner Fehler (Datenbank, Vorlage) |
| -32000 | aus fachlichen Gründen abgelehnt, z. B. „Nur Vorschläge lassen sich übernehmen.“, „Diese Fassung ist schon aktuell.“, „Eine Fassung braucht Text.“, „Die aktuelle Fassung lässt sich nicht löschen; erst auf eine andere umschalten.“, „Diese Datei ist schon importiert …“ |
| -32001 | Bedeutungssuche fehlgeschlagen (idea.search mit mode rag, Runde 19c) |

Die Datenbank sichert die Regeln des Fassungsbaums zusätzlich selbst (Migration 0004, siehe
`docs/Architektur.md`): Statuswechsel nur → aktuell (aus Vorschlag, verworfen, ersetzt), aktuell → ersetzt
und Vorschlag → verworfen („Unzulässiger Statuswechsel der Revision“), Herkunft unveränderlich, aktuelle Fassung
nicht löschbar, Elternteil vorhanden und kleiner, Nummern nie wiederverwendet. Der Speicher prüft vorher und
meldet fachliche Fälle als -32000; eine Verletzung, die je außerhalb des Speichers aufträte, käme als -32603.

HTTP: 421 bei einem fremden `Host`, 403 bei Anfragen von fremden Webseiten, 415 ohne `Content-Type: application/json`. Die Seite `/?work=<id>`
mit einem unbekannten Werk antwortet mit 404 „unbekanntes Werk“.

## Austausch (Runde 26) {#exchange-round-26}

Austausch zwischen Baruch-Instanzen (`crates/baruch-server/src/exchange/`, `src/instance.rs`; Format in
`docs/Austauschformat.md`). Die Methoden werden verteilt, bevor das Standardwerk aufgelöst wird; `work.exchange_export` nimmt ohne `work`
das Standardwerk, das Einlesen braucht kein Werk (es legt eines an oder findet es über die Werk-UUID). Das Einlesen gibt es nur über RPC,
Formular und Kommandozeile, nicht über MCP.

### work.exchange_export {#workexchange_export}


Ganzes Werk als Austauschdatei (`src/exchange/export.rs`), nur lesend.

- Parameter: `work?` (Werk-ID; fehlend → Standardwerk), `format?`: `"baruch"` (Standard, ZIP-Container) | `"xml"`
  (nur XML, Bilder als Base64).
- Antwort: `{work, uuid, format, file_name, media_type, bytes, sha256, data?, xml?}` – `uuid` des Werks,
  `file_name` `<slug>.baruch` oder `<slug>.baruch.xml`, `media_type` `application/vnd.baruch.werk+zip` oder
  `application/vnd.baruch.werk+xml`, `bytes` und `sha256` (Hex in Kleinbuchstaben) der gelieferten Datei; bei
  `baruch` die Datei als Base64 in `data`, bei `xml` als Text in `xml`.
- Fehler: unbekanntes Werk, `work` keine Zahl, unbekanntes `format` → -32602; ein in XML verbotenes Zeichen in
  einem Attribut (z. B. Autorname) → -32603 mit dem Namen des Feldes.
- Format-Unterversion 2 (Runde 25): mit `<karte>` (Verbindungen zwischen Einfällen, auch weich gelöschte, und
  gemerkte Positionen, je mit einer UUID) und den Grabsteinarten `verbindung`/`lage`.
- Kein Geheimnis, keine Verbindung zu Diensten, keine Persona-Modelle, keine Anträge in der Datei
  (`docs/Austauschformat.md` §1). Die Antwort ist nicht begrenzt; bei sehr großen Werken sind der Download (`GET /werke/{id}/austausch.baruch`)
  und die Kommandozeile handlicher.

```json
{"jsonrpc":"2.0","id":1,"method":"work.exchange_export","params":{"work":2,"format":"xml"}}
→ {"result":{"work":2,"uuid":"…","format":"xml","file_name":"die-muehle.baruch.xml",
   "media_type":"application/vnd.baruch.werk+xml","bytes":18234,"sha256":"…","xml":"<?xml …"}}
```

### work.exchange_preview {#workexchange_preview}


Vorschau des Einlesens; schreibt nichts (in einer zurückgerollten Transaktion geplant).

- Parameter: `data` (Base64 der Datei, `.baruch` oder `.baruch.xml`), `file_name?`, `title?` (für die Vorschau
  eines neuen Werks unter eigenem Titel).
- Antwort: `{mode: "new"|"merge", work_uuid, title, known_work: id|null, instance: {uuid, name}, exported,
  version, sha256, add: {nodes, revisions, template_revisions, ideas, posts, threads, thread_posts, assets,
  asset_revisions, links, positions}, items: [{kind, uuid, label}] (höchstens 500), truncated, deviations: [{kind, uuid, key?, rev?,
  label, here, there, action: "fassung"|"antrag"|"hinweis"}], requests: [{op, key, label}], deleted_here: [{kind,
  uuid, label}], unknown: [String], refusals: [String]}`. `kind` sind die XML-Wörter (`punkt`, `fassung`,
  `vorlage`, `einfall`, `beitrag`, `faden`, `faden-beitrag`, `bild`, `bildfassung`, seit Runde 25 `verbindung`,
  `lage`; bei Hinweisen auch `werk`, `buch`, `sprechdauer`, `bibelmodul`, `quellen`, `version`). `links`/`positions`
  zählen Verbindungen und Positionen der Einfallskarte (Abgleich: kein Duplikat für dasselbe Paar und dieselbe Art, eine
  Position nur ohne eine lokale; `docs/Austauschformat.md` §4 (h)). `key`/`rev` führen zu `/?work=W&key=K&rev=R`.
- Fehler: unsichere, kaputte oder ungültige Datei, falscher Namensraum, Grenze → -32602 mit einem Grund (§5 in
  `docs/Austauschformat.md`). Ein vergebener Titel ist kein Fehler, sondern steht in `refusals`.
- Größe: begrenzt durch `RPC_BODY_LIMIT` (32 MiB + 64 KiB, Base64); größere Dateien über das Formular oder die
  Kommandozeile.

```json
{"jsonrpc":"2.0","id":1,"method":"work.exchange_preview","params":{"data":"PD94bWwg…","file_name":"muehle.baruch.xml"}}
```


### work.exchange_import {#workexchange_import}


Übernehmen (eine Transaktion unter der Schreibsperre, plant neu, ein Commit im Spiegel).

- Parameter: `data` (Base64), `sha256` (Prüfsumme der vorher gezeigten Datei), `mode: "new"|"merge"`, `title?`
  (nur „neu“).
- Antwort: `{work, slug, title, mode, added: {…wie add}, requests: [id], deviations: [...], deleted_here,
  refusals, unknown, commit?, mirror?}` (`mirror` statt `commit`, wenn der Spiegel scheiterte; ohne beides,
  wenn ein Zusammenführen nichts änderte).
- Fehler: `sha256` passt nicht → -32602; Modus passt nicht zur Datei (neu bei einem bekannten Werk, Zusammenführen bei einem
  unbekannten) → -32000; vergebener Titel → -32000 mit einem Hinweis auf einen eigenen Titel; Datei wie bei der Vorschau
  → -32602. Bei jedem Fehler wird nichts geschrieben.
- Das Einlesen gibt es nicht über MCP (nur der Mensch liest ein).

```json
{"jsonrpc":"2.0","id":2,"method":"work.exchange_import","params":{"data":"PD94bWwg…","sha256":"0b2e…","mode":"merge"}}
```


### instance.get und instance.set_name {#instanceget-and-instanceset_name}


Kennung und Name dieser Baruch-Instanz (`crates/baruch-server/src/instance.rs`, Speicher `uuids.rs`). Der Name geht
im Austauschformat als Herkunft mit (`<ausgabe instanzname>`, Herkunft der Anträge „Austausch: <Name>“).

| Methode | Parameter | Ergebnis |
| --- | --- | --- |
| `instance.get` | – | `{uuid, name}` – `uuid` aus `store_meta.instance_uuid` (Migration 0041), `name` leer, solange keiner gesetzt ist |
| `instance.set_name` | `name` (Text; fehlend oder `null` = leer) | `{uuid, name}` nach dem Speichern |

- `name` wird getrimmt; höchstens 80 Zeichen (Zeichen, nicht Bytes), keine Steuerzeichen (Zeilenumbruch, Tabulator,
  DEL, C1). Sonst -32602 mit einer deutschen Meldung („Der Name der Instanz darf höchstens 80 Zeichen haben …“ oder
  „… keine Steuerzeichen …“), nichts gespeichert. `<` und `>` sind erlaubt; ein Name aus einer eingelesenen Datei
  wird bereinigt (`<` → `‹`, `>` → `›`, Steuerzeichen → Leerzeichen, höchstens 80 Zeichen; `docs/Austauschformat.md`
  §4), weil Git sie im Autor des Austausch-Commits ablehnt. Kein Text (z. B. eine Zahl) → -32602.
- Leer (auch nur Leerraum) setzt den Namen zurück; die Kennung bleibt immer gleich.
- Ein MCP-Werkzeug dafür gibt es nicht (eine Einstellung des Menschen). Formular ohne JavaScript:
  `POST /einstellungen/daten/instanz` (siehe `docs/Bedienung.md`). `settings.get {page: "daten"}` liefert zusätzlich
  `instance: {uuid, name}`.
- Die Werk-UUID steht in `work.meta` → `work.uuid` (nur lesend; `work.update` kennt kein Feld dafür).

```json
→ {"jsonrpc": "2.0", "id": 1, "method": "instance.set_name", "params": {"name": "  Arbeitszimmer "}}
← {"jsonrpc": "2.0", "id": 1, "result": {"uuid": "3f2b8c1e-9a47-4d2e-b5c3-0e1f2a3b4c5d", "name": "Arbeitszimmer"}}
```

## Umschlaganordnung und Impressum (Runde 27) {#cover-layout-and-imprint-round-27}

Vertrag Runde 27, Abschnitt 4. Blöcke (`CoverBlock`): `{kind, x_mm, y_mm, width_mm, align, size_pt, leading_em,
color, font, style, backdrop, visible, direction, text}`; Koordinaten in mm ab der Beschnittkante der jeweiligen Seite.

### Anordnung (umschlag-daten) {#layout-umschlag-daten}

- `cover.layout {work}` → `{work, stored, title_area, blocks: [CoverBlock + stored + height_mm], geometry:
  {page_width_mm, page_height_mm, spine_mm, bleed_mm, safety_mm}, warnings: [{code, block, message}], default_font}`.
  `blocks` hat immer alle 15 Arten in fester Reihenfolge (`front_title` … `back_free`): den gespeicherten Block
  (`stored: true`) oder den Standardblock (`stored: false`). `stored` oben = das Werk hat mindestens eine Zeile (eigene
  Anordnung). Satzfehler (keine Seitengröße) → `REFUSED`.
- `cover.layout_set {work, blocks}` → `{work, layout, commit?, mirror?}`. Jeder genannte Block ersetzt seine Art
  vollständig, nicht genannte Arten bleiben. Ein Block braucht alle Felder außer `font`, `direction`, `text` (fehlend = `null`);
  die Felder `stored` und `height_mm` aus `cover.layout` dürfen mitkommen, damit ein gelesener Block geändert
  zurückgeschickt werden kann. Grenzen (gleich den CHECKs von Migration 0045): `x_mm`, `y_mm` −50 bis 1000, `width_mm` 1 bis
  1000, `size_pt` 4 bis 200, `leading_em` 0 bis 3, `align` `left|center|right|justify`, `color` `light|dark|#rrggbb`
  (kleingeschrieben gespeichert), `style` `normal|bold|italic`, `backdrop` `none|shadow|gradient`, `font` höchstens 200 Zeichen
  aus Buchstaben, Ziffern, Leerzeichen, `- _ .` (kein `; { } < > \`, keine Steuerzeichen; leer = Schrift der
  Werkvorlage), `direction` `down|up` nur auf dem Rücken (fehlt es dort: `down`), `text` (höchstens 1000 Zeichen)
  nur bei `front_free`, `spine_free`, `back_free`, `back_motto`; dieselbe Art zweimal in einer Liste ist nicht möglich.
  Ungültig → `-32602` mit einer deutschen Meldung, die Art und Feld nennt (z. B. `„Umschlag-Block „Titel (Vorderseite)“
  (front_title), Feld x_mm: erlaubt -50 bis 1000 mm, nicht 2000.“`), nichts geändert. Ändert sich etwas: genau ein
  Commit „Umschlag: Anordnung geändert“ (`Baruch-Action: cover-layout`, leer erlaubt), Rumpf je Block
  „Titel (Vorderseite): x 12,0 → 15,0 mm, Schnitt normal → fett“; ein erstmals gespeicherter Block wird mit seinem
  Standardblock verglichen („… (vorher Vorgabe)“). Unverändert → kein Commit. Lässt sich die Ansicht danach nicht aufbauen
  (Satzfehler), stehen `layout: null` und `layout_error` in der Antwort; gespeichert wird trotzdem.
- `cover.layout_reset {work, faces?: ["front"|"spine"|"back"], area?: "top"|"middle"|"bottom"}` → wie
  `cover.layout_set`. Löscht die Zeilen der Seiten (fehlt `faces`: alle; je Zeile ein Grabstein `cover_block`) und
  setzt mit `area` den Titelbereich, beides in einer Transaktion mit einem Commit: „Umschlag: Schnellwahl
  oben|Mitte|unten“ (mit `faces: ["front"]` bei `area`) oder „Umschlag: Anordnung zurückgesetzt“
  (`Baruch-Action: cover-layout-reset`), Rumpf mit den gelöschten Blöcken und „Titelbereich: oben → unten“. Ändert sich
  nichts, kein Commit.
- `cover.fonts {work?}` → `{families: [{name, styles, embedded}], default}` (Schriften des Satzes, `default` =
  Schrift der Werkvorlage oder „Libertinus Serif“).
- Anträge (MCP `cover_layout_set`, `docs/MCP.md`): `asset.request_list` zeigt einen Antrag mit `op: "cover_layout"`,
  `blocks` (in Normalform) und `expected_layout` (SHA-256 über das JSON der gespeicherten Blöcke zur Zeit des Antrags).
  `asset.request_decide {decision: "confirm"}` führt ihn wie `cover.layout_set` aus, mit dem Commit „Umschlag: Anordnung
  geändert (Antrag N)“, Antragsteller und Bestätiger im Rumpf und in den Trailern (`Baruch-Request`,
  `Baruch-Requested-By`, `Baruch-Confirmed-By`). Hat der Mensch die Anordnung inzwischen geändert (der Fingerabdruck
  passt nicht mehr), wird der Antrag mit einem Grund `stale`; Ablehnen ändert nichts.

```json
→ {"jsonrpc": "2.0", "id": 1, "method": "cover.layout_set", "params": {"work": 2, "blocks": [
    {"kind": "front_title", "x_mm": 15, "y_mm": 40, "width_mm": 118, "align": "center", "size_pt": 28,
     "leading_em": 0.65, "color": "light", "font": null, "style": "bold", "backdrop": "shadow", "visible": true}]}}
← {"jsonrpc": "2.0", "id": 1, "result": {"work": 2, "layout": {"work": 2, "stored": true, …}, "commit": "3f0c…"}}
→ {"jsonrpc": "2.0", "id": 2, "method": "cover.layout_reset", "params": {"work": 2, "faces": ["front"], "area": "bottom"}}
```

### Impressum-Vorlage und Anschriften (umschlag-widget) {#imprint-template-and-addresses-umschlag-widget}

- `cover.imprint_adopt {work}` → `{work, template_rev, commit?, mirror?}`: hängt die Vorspannfunktionen der
  Impressum-Vorlage, die der Werkvorlage fehlen (`baruch_typst::cover::imprint_template_missing`), an die Vorlage an – als
  neue Vorlagenfassung des Menschen durch `work.update` (Commit wie dort). Hat die Vorlage schon alle vier
  (`schmutztitel`, `haupttitel`, `impressum`, `widmung`): `REFUSED` („Impressum-Vorlage nicht übernommen: …“), nichts
  geändert. Formular ohne JavaScript: `POST /werke/umschlag/impressum` (`work`, `back`) → 303 auf `back` mit
  `ok=impressum`, abgelehnt 422 mit einer Meldung.
- `work.meta.get` liefert zusätzlich `publisher_address` und `printer_address` in `meta`; `work.meta.set` nimmt beide
  (Text, mehrzeilig, höchstens 500 Zeichen, Grenzen im Speicher). Im Commit-Rumpf heißen sie „Anschrift des Verlags bzw.
  Herausgebers“ und „Anschrift der Druckerei“ (je eine Zeile, Zeilenumbrüche als Leerzeichen). Formular: Felder
  `publisher_address`, `printer_address` in `POST /werke/buchangaben/act`.
- `work.cover_pdf` antwortet zusätzlich mit `checks: [{code, block, message}]` (`cover_print::cover_checks` der
  gespeicherten Anordnung, Codes wie bei `cover.layout`).
- Editor ohne JavaScript (Routen in `cover::pages()`/`writes()`): `GET /werke/{id}/umschlag` und
  `GET /werke/{id}/umschlag.svg` nehmen Query-Felder `b.<kind>.<feld>` (`feld` = `x_mm`, `y_mm`, `width_mm`, `align`,
  `size_pt`, `leading_em`, `color`, `font`, `style`, `backdrop`, `visible` (1/0), `direction`, `text`; Zahlen mit Punkt
  oder Komma) als Vorschau ohne Speichern; ungültig → 422 mit einer Meldung, die Block und Feld nennt.
  `POST /werke/umschlag/anordnung` mit `work`, `back`, `action=save` (alle `b.*`-Felder → `cover.layout_set`),
  `action=quick` mit `area=top|middle|bottom` (→ `cover.layout_reset {faces: ["front"], area}`) oder `action=reset`
  mit wiederholtem `faces` (keines = alle) → 303 auf `back` mit `ok=anordnung`; ungültig → 422 mit der Eingabe. GET mit
  Satz von fremden Seiten (`Sec-Fetch-Site: cross-site|same-site`) → 403 ohne Typst-Lauf; die POST-Formulare stehen
  hinter `guard::same_origin`.

## Befunde (Runde 28) {#findings-round-28}

Antworten der Personas werden in Befunde zerlegt (Art Lob/Hinweis/kritisch; Status offen, übernommen, verworfen,
verschoben, erledigt ohne Änderung). Nur der Mensch setzt einen Status; Assistenten machen nur Vorschläge. Daten in
Migration 0046 (`finding`, `finding_status`, `post_version`). Fehler: unbekannter Befund, Grenzen und Statusregeln →
-32602.

### finding.list, finding.get, finding.set_status, finding.decide (befunde) {#findinglist-findingget-findingset_status-findingdecide-befunde}

Beantwortet von `src/findings/rpc.rs`. Alle nehmen `work` (Zahl oder Ziffern; fehlend → Standardwerk). Zahlen dürfen
auch als Ziffern-Text kommen (Formulare). Ein Befund als JSON (`Finding`): `{id, uuid, work, key, node, post,
thread, thread_post, position, persona, kind, body, quote, quote_check, status, reason, adopted_rev,
adopted_revision, deferred: {kind: "node", key} | {kind: "idea", id} | null, deferred_uuid, similar_to, summary_of:
[uuid], pending: FindingStatusEntry | null, created, updated, kind_label, status_label, url}`; `kind` = `praise`,
`hint`, `critical` („Lob“, „Hinweis“, „kritisch“), `status` = `open`, `adopted`, `rejected`, `deferred`, `done`
(„offen“, „übernommen“, „verworfen“, „verschoben“, „erledigt ohne Änderung“); `url` = `/werke/<work>/befunde?befund=<id>`.
Ein Verlaufseintrag (`FindingStatusEntry`): `{id, uuid, finding, status, reason, adopted_rev, adopted_revision,
deferred, deferred_uuid, actor: {kind: "human"|"persona"|"mcp", …}, proposed, note, decision:
"confirmed"|"rejected"|"stale"|null, decided, decided_by, created}`.

- `finding.list {work, key?, keys?, subtree?, post?, status?, kind?, persona?, since?, until?, include_praise? =
  false, include_attached? = false, limit?}` → `{findings: [Finding], total, truncated}` – älteste zuerst; passen
  mehr als `limit`, die neuesten `limit`. `keys`, `status` und
  `kind` als Liste oder Text „a,b“; `subtree: true` fügt zu `key`/`keys` alle Unterpunkte hinzu; `since`/`until`
  vergleichen `created` (ISO, `until` exklusiv); `limit` 1 bis 2000 (Standard 500). Ohne `include_praise` kein
  Lob (außer `kind` nennt es), ohne `include_attached` keine angehängten Befunde (`similar_to`). `total` = Zahl aller
  passenden Befunde (ohne das Limit), `truncated` = `total` ist größer als die gelieferte Liste. Nur lesend.
- `finding.get {work, id}` → `{finding, history: [FindingStatusEntry] (älteste zuerst), attached: [Finding] (an
  diesen angehängte), summary_of: [Finding] (die, die ein Sammelvorschlag zusammenfasst)}`. Unbekannter Befund oder
  Befund eines anderen Werks → -32602.
- `finding.set_status {work, id, status, reason?, rev?, target_key?, target_idea?}` (nur der Mensch, auch über
  `POST /act`; über MCP gibt es keinen Weg) → `{finding, before, commit?|mirror?}`. `adopted` braucht `rev` (eine
  Fassung, die im Abschnitt des Befunds existiert), `rejected` eine Begründung `reason` (getrimmt nicht leer, höchstens
  2000 Zeichen), `deferred` genau eines von `target_key` (anderer Abschnitt desselben Werks) und `target_idea`
  (Einfall desselben Werks), `done` und `open` nehmen `reason` optional. Fehlt etwas, ist es zu lang, existiert
  die Fassung oder das Ziel nicht oder hat der Befund schon genau diesen Zustand → -32602, und nichts ändert sich. Sonst
  schreibt der Speicher den Befund und einen Verlaufseintrag (`actor_kind = human`, `proposed = 0`) in einer Transaktion,
  und der Spiegel erhält **genau einen Commit**: Betreff „Befund <id>: <Status>“ (z. B. „verworfen“, „übernommen in
  Fassung 3“, „verschoben nach 1.2 Titel“), Rumpf mit Art, Persona, Ort, Text und Begründung, Trailer
  `Baruch-Finding`, `Baruch-Action: finding-status`, `Baruch-Node` (fester Schlüssel, wenn der Befund einen
  Abschnitt hat); Autor ist der Mensch.
- `finding.decide {work, entry, confirm: bool}` (nur der Mensch) → `{finding, entry, commit?|mirror?}`. Bestätigt
  (`confirm: true`), erhält der Befund den vorgeschlagenen Status (`decision = confirmed`); abgelehnt, bleibt er
  (`rejected`). Ein **veralteter** Vorschlag – der Befund hat inzwischen einen Status vom Menschen oder aus einem
  bestätigten Vorschlag erhalten, die vorgeschlagene Fassung oder das Ziel fehlt, oder der Befund hat den Status schon – wird
  `stale` und ändert nichts. Unbekannter oder bereits entschiedener Vorschlag → -32602. Genau ein Commit (Trailer
  `Baruch-Action: finding-decide`, `Baruch-Proposer: <Herkunft des Vorschlags>`; Autor ist der Mensch).

Vorschläge von Assistenten gehen nicht über `/rpc`, sondern über `findings::rpc::propose` (MCP
`finding_propose_status`, befund-uebersicht): nur `adopted`, `deferred`, `done` – der Speicher lehnt `rejected` und `open` ab
(„Nur der Mensch kann einen Befund verwerfen oder wieder öffnen …“, -32602); ein gleicher offener Vorschlag
(gleicher Status, gleiche Fassung oder gleiches Ziel) wird nicht zweimal angelegt (-32602). Genau ein Commit
(`Baruch-Action: finding-propose`, Trailer `Persona: <handle>`, Autor die Persona oder der MCP-Client). Das Anlegen von
Befunden beim Zerlegen ist kein Commit; `befunde.md` steht im nächsten Schnappschuss.

Austausch: Seit Unterversion 4 tragen `work.exchange_export`/`work.exchange_import` Befunde mit ihrem Verlauf und alten
Beitragsfassungen; die Vorschau zählt sie in `add.findings`, `add.finding_entries`, `add.post_versions`
(`docs/Austauschformat.md`).

### finding.matrix und Seite /werke/<id>/befunde (befund-uebersicht) {#findingmatrix-and-page-werkebefunde-befund-uebersicht}

`finding.matrix {work?, status?, kind?, persona?, since?, until?, include_praise? = false, older_than_days?}` – nur
lesend (`src/findings/page.rs`). Fehlt `work`, das Standardwerk. `status` oder `kind` als Liste oder Text mit Kommas
(Datenbankwörter `open|adopted|rejected|deferred|done` oder `praise|hint|critical`); `since`/`until` als Zeit wie
`2026-11-01` (`created >= since`, `created < until`); `older_than_days` 0–3650 (nur Befunde, die älter sind). Falsche
Werte → -32602. Antwort:

```json
{"work": 2, "title": "Wanderbuch", "personas": ["@lektor", "@nathan"],
 "groups": [{"kind": "part", "key": 12, "number": "I", "title": "Aufbruch",
             "rows": [{"key": 13, "number": "1", "title": "Morgen", "level": 1, "part": false, "keys": [13, 14, 15],
                       "cells": [{"open": 2, "adopted": 0, "rejected": 1, "deferred": 0, "done": 0, "praise": 0, "total": 3}, …],
                       "total": {…}, "open_badge": 3}, …],
             "totals": [{…}, …], "total": {…}, "open_badge": 3}, …],
 "totals": [{…}], "total": {…}, "open_by_key": {"13": 2, "14": 1}, "open_findings": 7,
 "history": [{"key": 13, "label": "1 Morgen", "total": {…},
              "revisions": [{"key": 13, "label": "1 Morgen", "rev": 1, "findings": [4]}]}],
 "filter": {"status": [], "kind": [], "persona": null, "since": null, "until": null, "include_praise": false,
            "older_than_days": null},
 "truncated": null}
```

- **Zeilen** sind Kapitel mit ihren Unterpunkten (`keys`); Abschnitte ohne Kapitel stehen für sich, ein Teil mit eigenen
  Befunden (am Teil oder an Abschnitten direkt darunter) erhält die Zeile „… (Teil selbst)“ (`part: true`). Jedes
  Kapitel hat eine Zeile, auch ohne Befunde.
- **Gruppen**: `kind = "part"` je Teil, `"none"` für Kapitel ohne einen Teil davor, `"unplaced"` für Befunde ohne Abschnitt
  (Fäden, gelöschte Abschnitte) am Ende; je Gruppe Summen je Persona (`totals`) und insgesamt (`total`).
- **Spalten** (`personas`): Personas mit Befunden im Filter, in der Reihenfolge der Personas, dann alphabetisch.
- **Zählen**: angehängte Befunde (`similar_to`) nie; Lob nur mit `include_praise` und dann nur in `praise` (nie als
  offen). `open_badge` = Summe von `open_by_key` über `keys` (Zählregel §1.5, ohne Filter): Ohne Filter gilt je Zeile
  `total.open == open_badge` – dieselbe Zahl wie die Plaketten in Gliederung, Editorkopf und `/gliederung`.
- **Gedeckelt** (`truncated`): Passen mehr als 2000 Befunde zum Filter, zählen die Zellen nur die neuesten 2000, und
  `truncated` nennt die Zahl aller (sonst `null`); die Plaketten (`open_by_key`, `open_findings`) zählen immer alle.
- **Verlauf** (`history`): Zeilen mit entschiedenen Befunden; `revisions` je Abschnitt und Fassung die übernommenen
  Befunde (`Store::findings_adopted_in`, nur Befunde im Filter).

Seite und Formulare (ohne JavaScript nutzbar):

- `GET /werke/<id>/befunde` mit denselben Filtern als GET-Parameter (`status` und `kind` wiederholbar,
  `include_praise=1`); unbekanntes Werk → 404, ungültige Werte entfallen. Dazu `key` (+ `persona`): Befunde der Zelle
  (Kapitel mit Unterpunkten; angehängte unter ihrem Original), `befund=<id>`: ein Befund mit Verlauf, Original und
  Quellen eines Sammelvorschlags; ohne beides die Liste „offen, älter als n Tagen“ (`older_than_days`, Standard 14),
  die Sammelvorschläge und der Verlauf je Kapitel. `ok=status|entscheidung` zeigt die Rückmeldung.
- `POST /werke/befunde/act` (hinter `guard::same_origin`): `method=finding.set_status` (`work`, `id`, `status`,
  `reason?`, `rev?`, `target_key?`, `target_idea?`; leere Felder entfallen) durch `findings::rpc::parse_change` und
  `findings::rpc::set_status`; `method=finding.decide` (`work`, `entry`, `confirm=1|0`, `id` nur für die Fehlerseite)
  durch `findings::rpc::decide`. Danach 303 auf `back` (nur eigene Adressen) oder `/werke/<id>/befunde?befund=<id>&ok=…`;
  Fehler 200 mit einem Hinweis (`role="alert"`) und dem Entwurf im Formular; unbekannte Aktion 400, unbekanntes Werk 404,
  fremde Seite 403.
- Einfallskarte: `idea.map` nimmt zusätzlich `findings?: bool` (Ebene „Befunde“, Standard `false`); dann trägt jeder
  Knoten `open_findings` (am Knoten einschließlich allem darunter) und jeder zusammengefasste Abschnitt seine eigenen. Seite
  `/werke/<id>/karte?befunde=1`.

## Suchen und Ersetzen (Runde 30a) {#search-and-replace-round-30a}

Alle mit `work?`; die Felder der Suche liegen flach: `q` (1–500 Zeichen), `case`, `word`, `re` (Standard `false`),
`range` = `selection|section|chapter|part|from|picked|work|all` (Standard `section`), `key` (für selection, section,
chapter, part, from), `sel: [key]` (picked), `start`/`end` (UTF-16, nur selection), `rev` = `current|shown` (shown nur
mit section/selection, mit `shown_rev`), `kinds: [text|heading|footnote|quote|code]` (Standard beim Suchen
`text, heading, footnote, quote`, beim Ersetzen `text, heading, footnote`; Baruch ersetzt nie Code), `text`
(ungespeicherter Editortext von `key`), `limit` (Standard 500, höchstens 2000).

- `search.find {…}` → `{hits: [{work, key, number, title, rev, kind, start, end, u16_start, u16_end, line, before,
  matched, after, url}], total, truncated, timed_out, range_label}`. `start`/`end` sind Bytes, `u16_*` UTF-16-Einheiten
  im Text der Fassung; `url` ist die Sprungadresse (`/?work=W&key=K[&rev=R]&find=…&at=Byte`). Zeitlimit 2 s
  (`timed_out: true`, die Liste ist dann unvollständig). Ungültiger oder zu großer regulärer Ausdruck → -32602.
- `search.preview {…, replacement}` → `{sections: [{key, number, title, base_rev, hits, replacements, diff_html,
  body_after}], total, truncated, timed_out, range_label}`. `replacement` höchstens 2000 Zeichen, Regex mit `$1`/
  `${name}`, im Wortlautmodus für Typst maskiert. `range=all` → -32602. Die eingefügten Ersetzungstexte aller Treffer
  zusammen dürfen höchstens 8 MiB groß sein (jeder Treffer zählt mindestens ein Byte), sonst -32000 „Zu viele
  Ersetzungen auf einmal …“ (gilt ebenso für `search.replace`, dann wird nichts ersetzt). `sections` enthält nur
  ganze Abschnitte, zusammen höchstens 500 Abschnitte und 2000 Stellen (sonst `truncated: true`).
- `search.replace {…, replacement, sections?: [{key, base_rev, skip: [Stelle]}], confirm: true}` → `{batch: {id, work,
  created, query, replacement, items: [{key, prev_rev, new_rev}], undone}, changed: [{key, rev, prev}], skipped:
  [{key, reason}], commit?, mirror?}`. Nur der Mensch (nicht über MCP, nicht über `/act`). `skip` zählt in der
  Trefferliste der Vorschau; ohne `sections` alle Abschnitte mit Treffern auf ihrer aktuellen Fassung. Übersprungen
  mit einem Grund: Basis nicht mehr aktuell, Ebene oder Überschriften würden sich ändern, kein Treffer mehr. Höchstens 500
  Abschnitte; ohne `confirm` → -32602; nach Ablauf der Zeit → -32000, nichts ersetzt. Genau ein Commit `„Ersetzt „x“ → „y“ in
  n Abschnitten“`.
- `search.undo {batch, confirm: true}` → `{batch, restored: [{key, rev}], skipped: [{key, reason}], commit?}`; ein
  Commit `„Ersetzen rückgängig: „x“ → „y“ (n Abschnitte)“`; bereits rückgängig gemacht → -32000.

## Querverweise (Runde 30a) {#cross-references-round-30a}

Alle mit `work?`.

- `xref.targets {q?, limit?}` → `{targets: [{key, label, number, title, level, snippet}]}`. Filtert nach Titel
  (enthält), Nummer (beginnt mit) oder Label `kN`; `level` 0 Teil … 3 Unterabschnitt; `snippet` =
  `#link(<kN>)[Titel]`; `limit` Standard 50, höchstens 500.
- `xref.resolve {label}` → `{key, url, number, title}` (`label` als `kN` oder `<kN>`; unbekannt → -32602).
- `xref.candidates {key?}` → `{candidates: [{key, rev, start, end, text, target_key, replacement}]}`: mögliche
  Querverweise (Erwähnungen im Fließtext der aktuellen Fassung, siehe `xref::outline_mentions`), nur ein Vorschlag.
- `xref.convert {key, rev, picks: [Stelle], confirm: true}` → `{node, key, rev, converted, commit?}`: Die gewählten
  Kandidaten der Fassung `rev` (Positionen in der Liste von `xref.candidates` für diesen Abschnitt) werden eingefügt,
  eine neue Fassung des Menschen auf `rev`, ein Commit. Ohne `confirm` → -32602.
- `xref.broken {}` → `{broken: [{key, rev, label, start, text, target}]}`: Verweise auf Schlüssel ohne aktuelle
  Fassung.

## Lösungsvorschläge und Varianten (Runde 30b) {#solution-proposals-and-variants-round-30b}

Alle Methoden mit `work?` (fehlend → Standardwerk) und `key` (fester Schlüssel des Abschnitts).

- `solution.list {key, post?, rev?}` → `{solutions: [{index, quote, replacement, step, adoptable, finding}], rev}`:
  mit `post` die Lösungen des Beitrags (`index` = Position für `picks`), sonst die Lösungen aller Befunde des Abschnitts
  (`index` je Befund, `finding` gesetzt). `adoptable`: Zitat und neuer Wortlaut, das Zitat steht in der Fassung `rev`
  (ohne: die Prüfgrundlage des Beitrags oder die aktuelle; Leerraum darf abweichen).
- `solution.adopt {key, post | finding, rev?, picks: [usize]}` → `{rev, base, persona, note, url,
  proposed_findings: [id], notes: [..]}`: fügt die gewählten Lösungen nacheinander in die Basis ein (`rev`, sonst
  die Prüfgrundlage des Beitrags, bei einem Befund die aktuelle Fassung) und legt **einen Vorschlag** an (Herkunft die
  Persona des Beitrags, Notiz „Lösung aus Beitrag n“ oder „… (Befund [B m])“); die Befunde mit diesen Lösungen
  erhalten den Statusvorschlag „übernommen in Fassung n“. Eine nicht übernehmbare Lösung, eine unbekannte Position, ein Beitrag aus
  einem anderen Abschnitt → `-32602`; Text unverändert → `-32000`. `picks` auch als Zahl oder „1,2“.
- `solution.variants {key, rev?, findings?: [id], count?: 2|3, post?}` → `{group, base, persona, variants: [{rev,
  label, summary, findings, url}], notes}`: Der Lektor (nach Einstellung) schreibt die Varianten als Vorschläge auf
  der Basis `rev` (Standard: aktuelle Fassung) aus den genannten oder offenen Befunden des Abschnitts; ein `count`
  außer 2 oder 3 → `-32602`; ohne Befunde → `-32000`.
- `solution.reject_siblings {key, rev, confirm: true}` → `{rejected: [rev], group, kept}`: verwirft die anderen offenen
  Vorschläge der Variantengruppe von `rev` (nur der Mensch; ohne `confirm` oder ohne Variante → `-32602`).
- `solution.compare {key, group}` → `{base, group, key, work, title, variants: [{rev, label, summary, findings,
  status, diff_html, against: [{rev, label, diff_html}], url}]}`; `diff_html` ist maskiertes HTML (`<ins>`/`<del>`).
- Prüfgrundlage `basis` = `shown` (Standard) | `current` | Zahl oder Ziffern bei `persona.ask` und `persona.review`
  (Antwort `basis: {rev, label, proposal}`); `persona.propose` beruht auf der gezeigten Fassung `rev` oder der
  ausdrücklich gewählten (`basis` = `current` | Zahl), auch auf einem offenen Vorschlag (Fehler 0.9.0, Kritikzyklus;
  vorher immer auf der aktuellen). Seit Runde 36 gilt das auch für `scope: "chapter"`: Der Vorschlag für den Teil
  der Aufgabe (`key`) beruht auf der gezeigten oder gewählten Fassung, die anderen Teile auf ihrer aktuellen; die
  Antwort nennt sie in `basis` („Grundlage: Fassung 2 (Vorschlag von @lektor)“), und ein Vorschlag, der der gezeigten
  Fassung gleicht, zählt als unverändert. Im Chat sieht eine direkt angesprochene Persona ihre eigenen offenen Befunde
  des Abschnitts (erneutes Prüfen wie beim RFC) – seit Runde 36 nur bei einem Prüfauftrag
  (`assist::chat::is_check_order`: Ein Wort beginnt mit `prüf`, `überprüf`, `nachprüf`, `gegenprüf`, `kontrollier`, `check` oder
  `review`, oder die Nachricht nennt „[B<n>]“), nicht bei einer einfachen Frage. Für den Chat
  liest `persona.chat` den Parameter mit Runde 30c (chat-bedienung). Unbekanntes Wort → `-32602`.
- Formular `POST /loesungen/act` (hinter `same_origin`): `method` = `solution.adopt` | `solution.variants` |
  `solution.reject_siblings`, dazu `work`, `key`, `rev`, `post`, `finding`, `picks` (wiederholbar), `findings` (wiederholbar),
  `count`, `confirm`, `back` (eigene Adresse); Erfolg → 303 (Vorschlag oder `/varianten` oder `back`), Fehler → 200
  mit einem Hinweis. Seite `GET /varianten?work=W&key=K&group=G` (ohne `key`: Abschnitt aus der Gruppe).

## Chat: Wiederholen, Löschen, Ansprechen, live, strukturierte Beiträge, Labels (Runde 30c) {#chat-repeating-deleting-addressing-live-structured-posts-labels-round-30c}

Alle Methoden nehmen `work?` (sonst das Standardwerk); Handelnder ist der Mensch.

- `post.repeat {id, rev?, basis?}` → `{work, key, post: {id, author, body}, url, repeat_of, basis: {rev, label, proposal},
  proposed_done: [finding id], notes: [Text]}`. Wiederholt die Anfrage eines Persona-Beitrags: Persona, Modus (`ask`,
  `review`, `chat`), Umfang und Text aus der gespeicherten Aufgabe (`post.request`, Migration 0047), gegen die gezeigte
  Fassung `rev` oder nach `basis` (`shown` Standard | `current` | `<n>`, `assist::basis::choose`). Der neue Beitrag trägt
  `repeat_of` (der alte Beitrag bleibt), `reply_to` wie der alte. Offene Befunde (ohne Lob) der alten Antwort, die in der neuen
  nicht wiederkommen (`findings::parse::similar` gegen die Befunde der neuen Antwort), schlägt die Persona als
  „erledigt“ vor (`finding_propose_status`, je ein Commit `finding-propose`; der Mensch entscheidet). Rückfall bei
  Beiträgen ohne gespeicherte Aufgabe (vor Runde 30): die vorangehende Nachricht des Menschen oder eines MCP-Clients im
  selben Abschnitt als Chat; gibt es keine oder stammt der Beitrag nicht von einer Persona (oder ist der kurze Beitrag eines
  Schreibauftrags) → `-32000`. Scheitert das Modell → `-32000`, nichts gespeichert, Fehler in der Live-Anzeige.
- `post.since {key, after, rev?}` → `{work, key, posts: [{id, author, html}], truncated, typing: [{persona, since}],
  failures: [{persona, message, at}]}`. Beiträge des Abschnitts nach dem Beitrag `after` (steht er dort nicht mehr: alle
  mit größerer ID), höchstens 50, als fertiges HTML (Makro `post_entry` aus `fragments/post_item.html`, wie im
  Abschnitts-Chat gegen die Fassung `rev`); `typing` = Personas mit einer laufenden Aufgabe in diesem Abschnitt
  (`chat_live::typing_now`), `failures` = gescheiterte Modellaufrufe der letzten zehn Minuten (höchstens fünf, älteste
  zuerst; `at` ISO-Zeit, der Browser zeigt jeden einmal).
- `thread.post_delete {thread, id, confirm: true}` → `{work, thread, id, deleted: {author, body}, url, commit?|mirror?}`.
  Ohne `confirm: true` → `-32602`. Der Mensch löscht jeden Fadenbeitrag; genau ein Commit „Fadenbeitrag n: gelöscht“
  durch `mirror::record_request` (Trailer `Baruch-Thread`, `Baruch-Thread-Post`, `Baruch-Action: thread-post-delete`,
  Rumpf mit dem ganzen Text; der Baum des Spiegels bleibt, Fäden stehen nicht im Spiegel). Verweise anderer Beiträge
  (`reply_to`, `repeat_of`) werden NULL.
- `thread.repeat {thread, id}` → `{work, thread, post: {id, author, body}, url, repeat_of, proposed_done, notes}`: wie
  `post.repeat` im Faden (Aufgabe = Nachricht des Absenders, `request.scope = "thread"`; Rückfall die vorangehende
  Nachricht des Menschen oder eines MCP-Clients); ohne neues Relay.
- `thread.since {thread, after}` → `{work, thread, posts: [{id, author, html}], truncated, typing, failures}` (Makro
  `thread_entry`).

Geändertes Verhalten bestehender Methoden:
- `persona.chat` und `thread.post`: Das Ansprechen mit Namen am Satzanfang („Lektor, …“, `assist::chat::addressed`) ruft wie
  `@handle`; eine Persona-Antwort mit `@handle:` oder einer Anrede am Zeilenanfang ruft die angesprochene Persona einmal
  (Relay: eine Stufe, höchstens drei Relay-Antworten, jede Persona einmal, nicht bei RFC/Schreibauftrag). `persona.chat`
  nimmt `basis` (`shown` | `current` | `<n>`); unbekannter Wert → `-32602`.
- `post.page` bleibt schlicht (Text ohne Struktur); `post.since` liefert das gesetzte HTML.

Formulare (ohne JavaScript, hinter `guard::same_origin`):
- `POST /beitraege/act` nimmt `method=post.repeat` (Felder `id`, `rev`, `basis`) → 303 zum Abschnitt mit
  `#post-<neu>`; Fehler 422 mit Text.
- `POST /faden/act` nimmt `method=thread.post_delete` (Kontrollkästchen `confirm=1`, sonst ein Hinweis auf der Seite, Status 200)
  oder `method=thread.repeat` (Feld `id`) → 303 zum Faden; ohne `method` eine Nachricht wie bisher.
- `POST /assistenten/act` reicht `basis` bei allen vier Methoden an die RPC weiter.

## Satz-Einstellungen (Runde 29) {#typesetting-settings-round-29}

Satz-Einstellungen eines Werks (Kapitelanfang, Seitenformat, Ränder, Schrift, Seitenzahlen …) als Typst-Regeln
„// Baruch: Satz“ hinter der Werkvorlage; Datenmodell `baruch_typst::typeset::TypesetSettings` (JSON: nur gesetzte
Felder, unbekannte Felder werden abgelehnt). Geändert nur vom Menschen (Oberfläche); MCP liest nur.

### work.typeset_get, work.typeset_set, work.typeset_reset, work.typeset_adopt_rule (satz-daten) {#worktypeset_get-worktypeset_set-worktypeset_reset-worktypeset_adopt_rule-satz-daten}

Modul `crate::typeset_settings` (Runde 29a). Nur aus der Oberfläche (= Mensch, `HUMAN_NAME`); fehlendes `work` → das
Standardwerk, unbekanntes Werk → `-32602`. Die schreibenden Methoden nehmen die Schreibsperre und erzeugen **genau einen**
Spiegel-Commit je Änderung; keinen ohne Änderung.

JSON der Einstellungen (Beispiel mit allen sechzehn Feldern; jedes fehlende Feld = „aus der Werkvorlage“):

```json
{"chapter_start":"right_page","part_page":{"side":"right","blank_back":true},"page_format":{"kind":"a5"},
 "margins":{"inside_mm":20.0,"outside_mm":15.0,"top_mm":18.0,"bottom_mm":22.0,"binding_mm":5.0},
 "font":"Libertinus Serif","font_size_pt":10.5,"leading_em":0.65,"paragraph":{"kind":"indent","em":1.0},
 "justify":true,"hyphenate":true,"lang":"de","page_numbers":{"position":"bottom_outside","start":"first_chapter"},
 "running_head":"chapter","footnotes":{"numbering":"arabic","restart":"per_chapter"},"widows_orphans":true,
 "xref_with_page":true}
```

Werte: `chapter_start` `continuous|new_page|right_page`; `part_page.side` `right|any`; `page_format.kind`
`a5|120x190|135x215|a4|custom` (bei `custom` mit `width_mm`, `height_mm`); `paragraph.kind` `indent|spacing`;
`page_numbers.position` `bottom_center|bottom_outside|top_outside|hidden`, `.start` `first_page|first_chapter`;
`running_head` `off|work_title|chapter`; `footnotes.numbering` `arabic|symbols|letters`, `.restart`
`continuous|per_chapter`. Grenzen und Rundung wie `TypesetSettings::validate`/`normalized` (mm und pt auf 0,1, em auf
0,01; Schrift und Sprache getrimmt).

- `work.typeset_get {work?}` → `{work, set, uuid|null, updated|null, updated_by|null, settings, rules, conflicts
  [{field, label, line, text, effect, message}], template_rule {line, text, chapter_start}|null, warnings [String],
  fields [{id, label}]}`. `rules` ist der Block „// Baruch: Satz“ aus `baruch_typst::typeset::rules` ("" ohne
  Einstellungen); `warnings` nennt z. B. eine Grundschrift, die Typst nicht kennt (`„Schrift „X“ fehlt; Typst setzt mit
  der eingebetteten Schrift“`) oder den Kolumnentitel „Werktitel“, obwohl die Werkvorlage kein `#set document(title: …)` hat
  (der Kolumnentitel bliebe leer); `fields` sind die sechzehn Feld-IDs mit einer Bezeichnung in fester Reihenfolge.
- `work.typeset_set {work?, settings}` → `{work, settings, changed [ids], commit?, mirror?}`. Ersetzt die Einstellungen
  vollständig (normalisiert). Ungültig → `-32602` mit **allen** Meldungen („Satz-Einstellungen: Rand innen: 5 bis 80 mm;
  Schriftgröße: 6 bis 24 pt; …“), nichts geändert; unbekannte Felder oder fehlendes `settings` → `-32602`. Gleiche
  Werte → `changed: []`, kein Commit. Leere Einstellungen `{}` setzen zurück (Commit wie `work.typeset_reset`).
  Die UUID der Zeile bleibt bei Änderungen gleich.
- `work.typeset_reset {work?}` → `{work, changed, commit?, mirror?}`. Löscht die Zeile (Grabstein `work_typeset`,
  im Austausch `satz`); ohne Einstellungen `changed: []`, kein Commit.
- `work.typeset_adopt_rule {work?, confirm: true}` (Runde 23: ohne `confirm: true` -32602) → `{work, chapter_start, template_rev, changed, commit?, mirror?}`. Erkennt die
  Kapitelanfangsregel der Werkvorlage (`detect_chapter_break`), setzt `chapter_start` auf ihren Wert (andere Felder
  bleiben), entfernt die Zeile(n) aus der Vorlage (`remove_rule`) und legt in **einer** Transaktion eine neue
  Vorlagenfassung an (Notiz „Kapitelanfang in die Satz-Einstellungen übernommen (Runde 29)“, Autor der Mensch) – ein
  Commit für beides. Keine erkannte Regel → `-32000` „Die Werkvorlage hat keine erkennbare Regel für den
  Kapitelanfang; nichts geändert.“
  Die Methode fragt nicht nach einer Bestätigung (wie `work.typeset_reset`): `/rpc` ist der Weg des Menschen hinter der
  Origin-Prüfung, der Aufrufer holt die Bestätigung ein – das Formular verlangt das Kontrollkästchen (`confirm=1`, sonst 422);
  heute ruft kein Widget die Methode auf. Die alte Vorlagenfassung bleibt im Fassungsverlauf.

Spiegel-Commit (`mirror::describe_typeset`): Autor und Committer der Mensch; Betreff „Satz: <Bezeichnungen> geändert“,
„Satz: Einstellungen zurückgesetzt“ oder „Satz: Kapitelanfang aus der Werkvorlage übernommen (Vorlagenfassung n)“;
Rumpf je geändertem Feld „Bezeichnung: vorher → nachher“ (ungesetzt = „aus der Werkvorlage“, z. B. „Schriftgröße:
10,5 pt → 11 pt“); Trailer `Baruch-Action: typeset-set|typeset-reset|typeset-adopt`. Der Commit entsteht auch ohne
Dateiänderung (`allow_empty`, z. B. bei „Verweis mit Seitenzahl“, das keine Typst-Regel erzeugt). `main.typ` trägt den
Regelblock automatisch (`snapshot::head`); eine eigene Spiegeldatei gibt es nicht. Scheitert der Spiegel, gilt die
Änderung trotzdem; die Antwort trägt `mirror` statt `commit`.

### work.typeset_preview (satz-oberflaeche) {#worktypeset_preview-satz-oberflaeche}

`work.typeset_preview {work?, settings}` → `{work, stand, pages, urls, errors, warnings, diagnostics, conflicts}` – setzt
die **ungespeicherten** Einstellungen `settings` (JSON wie bei `work.typeset_set`) probeweise; nur lesend, kein Commit.
Quelltext: `baruch_typst::typeset::preview_source(settings, Werkvorlage, Werktitel)` (Werkvorlage, Regelblock
„// Baruch: Satz“, erfundener Mustertext mit einem Teil und drei Kapiteln) und die Bilder des Werks; gesetzt durch
`typeset::compile` (eine Sperre mit der Satzvorschau und dem PDF, Zeitlimit 60 s).

- `stand`: Hash des gesetzten Zustands; `pages`: Seitenzahl; `urls`: je Seite
  `/einstellungen/werk/satz/vorschau/<stand>/<n>.svg` (n ab 1).
- `errors`, `warnings`: **Anzahl** der Fehler beziehungsweise Warnungen von Typst; `diagnostics`: `[{severity, message}]`.
- `conflicts`: `[{field, label, line, text, effect, message}]` wie bei `work.typeset_get`, aber für die übergebenen
  Einstellungen.
- Fehler: `settings` fehlt, unbekannte Felder oder falsche Typen → `-32602`; Werte außerhalb der Grenzen → `-32602` mit
  allen Meldungen von `TypesetSettings::validate`, getrennt durch „; “ (jede beginnt mit der Bezeichnung, z. B.
  „Schriftgröße: 6 bis 24 pt“); unbekanntes Werk → `-32602`; Zeitlimit oder Abbruch → `-32000`.

Formulare und Seiten (Block „Satz“ unter Einstellungen → Werk, ohne JavaScript; POST hinter `guard::same_origin`, fremder
Origin → 403):

- `POST /einstellungen/werk/satz` mit `work` und `action`:
  - `save` (Standard) → `typeset_settings::save` → 303 `/einstellungen/werk?work=W&ok=satz#satz`;
  - `preview` → 200, ganze Einstellungsseite mit den ungespeicherten Werten und `<img>` auf die Vorschaubilder;
  - `reset` (erforderlich `confirm=1`) → `typeset_settings::reset` → 303 `…&ok=satz_zurueck#satz`.
  Felder (wörtlich, "" = aus der Werkvorlage, Dezimalkomma erlaubt): `chapter_start`, `part_side`, `part_blank_back=1`,
  `page_format`, `page_width_mm`, `page_height_mm` (nur `custom`), `margin_inside_mm`, `margin_outside_mm`,
  `margin_top_mm`, `margin_bottom_mm`, `binding_mm` (alle vier Ränder oder keiner; Bundzuschlag leer = 0), `font`,
  `font_size_pt`, `leading_em`, `paragraph`, `paragraph_em` (leer = 1 oder 0,5), `justify`, `hyphenate`,
  `widows_orphans`, `xref_with_page` (jeweils `""|on|off`), `lang`, `page_number_position`, `page_number_start` (leer =
  `first_page`), `running_head`, `footnote_numbering`, `footnote_restart` (leer = `continuous`). Unterfelder ohne
  Hauptwahl (z. B. `part_blank_back` ohne `part_side`) zählen nicht. Gelesen von
  `settings::work_typeset::settings_from_form` (Meldungen wie `TypesetSettings::validate`, dazu „unbekannte Wahl“ und
  unvollständige Ränder).
- `POST /einstellungen/werk/satz/vorlage` mit `work`, erforderlich `confirm=1` → `typeset_settings::adopt_template_rule` → 303
  `…&ok=satz_vorlage#satz`.
- Ungültig oder abgelehnt (auch ein fehlendes `confirm`) → 422, ganze Einstellungsseite mit der Eingabe und der Meldung am
  Feld (`settings::work_retry_page`, `WorkRetry::Typeset`); interner Fehler → 500. Unbekanntes Werk → 404, unbekannte
  `action` → 400.
- `GET /einstellungen/werk/satz/vorschau/{stand}/{n}.svg`: nur Zustände aus dem Satz-Cache
  (`typeset::cached_page`, startet nie einen Satzlauf), sonst 404; `Sec-Fetch-Site: cross-site|same-site` → 403. Header wie
  die Seiten der Satzansicht: `image/svg+xml`, `X-Content-Type-Options: nosniff`, `Content-Security-Policy` =
  `typeset::SVG_CSP`, `Cache-Control: no-cache`.

## Runde 19 – RAG-Pflege, Rest der Verweise {#round-19--rag-maintenance-rest-of-the-references}

### R19a Index {#r19a-index}

Keine neue Methode, keine neue Route; Einzelheiten in „Semantic search (RAG, Round 12)“ und „Sync with
MultiGPT (Round 13)“.

- `rag.search`: jeder Treffer mit `weak: bool` (Score unter 0,45 oder keine Zahl).
- `rag.status`: `index.stale` (hängengebliebene Aufträge); `problem` bleibt gesetzt, solange etwas hängt, auch
  wenn der letzte (billige) Auftrag gelang; Suchen lösen keinen neuen Versuch aus.
- `rag.reindex`: antwortet sofort, zusätzlich `deferred: bool` (Rücksetzwunsch, weil der Worker die
  Schreibsperre hielt; er bricht zwischen zwei Stapeln ab, ein hängendes Embedding sofort).
- LanceDB: `query_prefix`/`document_prefix` der Verbindung (Änderung des Dokumentpräfixes → ausgefallen, bis
  der Neuaufbau erfolgt); `Index::replace` löscht nur in seinem Bereich (ein Werk oder gemeinsam).
- Start mit eigenem `--data` ohne gespeicherte Verbindung Eliezer: kein Embedder, `problem` nennt den Grund.
- `sync.run`: Hochladen ohne ID mit einem Hinweis und einer Sperrmarke, `isError` für ein Dokument sperrt nur dieses,
  unvollständige Listen löschen keine lokale Kopie, eine Marke gegen einen stillen Abbruch zwischen Hochladen und Zustand.
- `connection.check` für MultiGPT beendet seine MCP-Sitzung zuletzt mit HTTP `DELETE`.

### R19b pgvector {#r19b-pgvector}

- `connection.set`/`connection.get`: `query_prefix` (lancedb, pgvector) und `document_prefix` (lancedb), siehe
  „Connections and sources“; `connection.check` (pgvector) mit gekürzten Feldern und einem TLS-Hinweis.
- pgvector-Suche (`rag.search` über den Weg pgvector): alle Felder in SQL gekürzt – Dokument 500, Abschnitt 200, Titel
  300, Adresse 2000, Sammlung 200, Text 2000 Zeichen; eine Sammlung mit längerem Namen lässt sich über den gekürzten Namen wählen;
  die Anfrage wird mit `query_prefix` eingebettet.
- Einfallskarte: `idea.map_groups`, `idea.map_groups_name`, `idea.map_group_accept`, `idea.map_group_dismiss` (siehe
  „Idea map (Round 25)“); Formular `POST /werke/{id}/karte/act` mit denselben drei schreibenden Methoden.
- `ribbon.formats`: `scripture` und `cite` aus der Werkvorlage (siehe „ribbon.formats“).

### R19c Treffer {#r19c-treffer}

Schwache Treffer, Wortlaut mit Silbentrennungsresten und der Rest der Verweise (Abschnitte „Ideas“, „Bible references“,
„Bibles“, „Literature and highlights“). Alles additiv, keine neuen Methoden.

- **Schwach** heißt: Ähnlichkeit `score` < `baruch_rag::WEAK_SCORE` (0,45, eine Annahme für alle Wege) oder keine Zahl
  (`baruch_rag::is_weak`). Nichts wird endgültig versteckt.
- `idea.search {mode: "rag"}`: je Treffer `weak: bool`, schwache Treffer nach den anderen (sonst Rang). Ausfall der
  Bedeutungssuche (LanceDB nicht verbunden oder ein Fehler der Suche außer ungültiger Eingabe) → **-32001**
  (`rpc::RAG_UNAVAILABLE`) mit der Meldung „Bedeutungssuche nicht verfügbar: LanceDB (lokal): <Grund>.“; ungültige
  Eingabe bleibt -32602.
- `literature.render` (Modus `rag`) und die Bedeutungssuche der Literatur (`literature::rag_view`): je Treffer
  `weak`, Treffer nach `weak` geordnet (stabil), dazu `weak_count` und `weak_label` („schwach“).
- `scripture.check`: je Stelle neu `module_spacing: bool` – der Wortlaut stimmt nur ohne Leerraum (Silbentrennungsreste im
  Modul wie „die ses“, „Men schen“); `wording_code` bleibt `match`, `wording` „Wortlaut stimmt (Leerraum im Modul)“
  (Teilzitat: „Wortlaut stimmt (Teilzitat, Leerraum im Modul)“). Neu je Antwort und je Stelle `module_note:
  string|null`: ein Hinweis, wenn das Modul auffallend viele Wortfragmente hat (Schätzung an den ersten 4000 Versen,
  höchstens 2 s, je conf-Datei im Speicher; sonst `null`, ohne Bibel immer `null`).
- `bible.marks`: je Marke `module_spacing`, je Antwort `module_note` (wie oben).
- `bible.lookup`: je Vers neu `german_ref` (deutsche Stelle, zurückgebildet aus der Versifikation des Moduls, z. B. KJV
  „Mal 4,2“ → „Mal 3,20“) oder `german_problem` (Grund, warum der Vers nicht abgebildet werden kann; dann
  `german_ref: null`).
- `lexicon.lookup`: Lemma-Probe, wenn das Stichwort fehlt – Strong-Schreibweisen („G26“ ↔ „G0026“, auch mit
  `StrongsPadding=false`), einfache Endungen, Umlautwiederherstellung („Bäume“ → „Baum“), zuletzt der letzte Bestandteil
  eines Kompositums („Feigenbaum“ → „Baum“). Neu `probe` (die versuchte Form oder `null`, wenn das Stichwort selbst
  traf) und `asked` (das erfragte Stichwort).
- Stellen im Fließtext: „Mal“ mit folgendem Kapitel und Vers ist Maleachi, kein Zahlwort („Er las Joh 3,16
  Mal 3,20 vor.“ ergibt beide Stellen). Zitatzuordnung: Zwischen einer Stelle und dem folgenden Zitat dürfen bis zu drei
  kurze einleitende Wörter mit Doppelpunkt stehen („wie Lk 15,6 erzählt: „…““, „heißt es:“, „sagt:“; Liste
  `wording::INTRO_WORDS`). Wortlautvergleich in der Unicode-Normalform NFC.

### Integration {#integration}

Keine RPC-Änderung durch die Integration. Der Abnahmetest `tests/round19_abnahme.rs` geht durch die hier beschriebenen Methoden:
`connection.set` mit `query_prefix`/`document_prefix`, `rag.status` (`index.stale`), `rag.search`
(`weak`, pgvector durch `connection.rag_set`/`connection.collections_set`), `rag.reindex` (`deferred`), `work.delete`,
`idea.search` (`mode: "rag"`, `weak`), `persona.review` (Belegblock), `idea.map_groups`, die Formulare an
`/werke/{id}/karte/act` (`idea.map_groups_name`, `idea.map_group_accept`) und `scripture.check` (`module_spacing`,
`module_note`, „Mal“).

## EPUB (Runde 20) {#epub-round-20}

Seit Runde 20 alles umgesetzt (kein STUB): `work.epub` (R20a), `epub.readers` und `epub.preview` (R20b),
`epub.check`, `epub.checker_get` und `epub.checker_set` (R20c).

### work.epub (R20a) {#workepub-r20a}

Umgesetzt seit R20a (kein STUB mehr).

`work.epub {work, inline?: bool=false, footnotes?: "popup"|"endnotes"="popup"}` → `{file_name, bytes, sha256, url,
footnotes, files:[{href, media_type, kind, title, key}], warnings:[{code, message, key}], epub_base64?}`.

- Baut das EPUB des aktuellen Zustands (aktuelle Fassungen, Werkvorlage, Bilder, Umschlag, Buchangaben,
  Satz-Einstellungen) und liefert Angaben; **kein Commit**, nichts wird gespeichert.
- `file_name` = `<slug>.epub`, `bytes` = Größe der ZIP-Datei, `sha256` = Prüfsumme, `url` = `/werke/{id}/epub`.
- `files`: Dateien in Lesereihenfolge (Spine), relativ zu `OEBPS/` – `kind` ∈ `cover`, `front` (Vorspann, Vorwort),
  `part`, `chapter`, `notes`; `title` = angezeigte Überschrift, `key` = Schlüssel des ersten Abschnitts der Datei.
- `warnings`: `{code, message, key}` (`key` = Abschnitt oder `null` für Werkvorlage/Metadaten). Codes:
  `ISLAND_OMITTED`, `ARGS_OMITTED`, `IMAGE_NO_ALT`, `IMAGE_MISSING`, `IMAGE_PATH_INVALID`, `IMAGE_TYPE_FOREIGN`,
  `XREF_UNKNOWN`, `TEMPLATE_PAGEBREAK_MAPPED` (Info), `TEMPLATE_PAGEBREAK_UNMAPPED`, `ISBN_INVALID`,
  `META_YEAR_IGNORED`, `LANGUAGE_INVALID`.
- `inline: true` → zusätzlich `epub_base64` (die Datei als Base64).
- Fehler: unbekannte Art der Fußnoten → -32602; Werk ohne Abschnitt → -32000 („Das Werk hat keinen Abschnitt …“).
- Gleicher Zustand → gleiche Bytes: feste ZIP-Zeitstempel und Reihenfolge, `dcterms:modified` = spätester Zeitpunkt aus
  Werk, Buchangaben, Satz-Einstellungen, Bildern und Fassungen. Annahme: Das bloße Umschalten der aktuellen Fassung
  ohne eine neue Fassung ändert diesen Zeitpunkt nicht (der Inhalt und damit die Prüfsumme ändern sich trotzdem).

### epub.readers und epub.preview (R20b) {#epubreaders-and-epubpreview-r20b}

Beide umgesetzt (R20b). Nur lesend, kein Commit.

- `epub.readers {}` → `{readers:[ReaderProfile…]}` in Anzeigereihenfolge: je `{slug, name, device, width_px,
  height_px, required, via_conversion, supports:{popup_footnotes, embedded_fonts, css_backgrounds, page_break_css,
  hyphenation, svg_images, night_mode, overrides_publisher_font}, note}`. Slugs (eingefroren): `apple-books-iphone`,
  `apple-books-ipad`, `google-play-books`, `kindle` („Kindle (über EPUB)“), `kindle-app` („Kindle-App (über
  EPUB)“) mit `required: true`; `tolino`, `kobo`, `thorium`, `calibre`. `via_conversion: true` nur für Kindle und
  Kindle-App (Send to Kindle wandelt das EPUB um). Die Werte sind Annahmen ohne Gerät (siehe Architektur, R20b).
- `epub.preview {work, readers?: [slug], file?: href, footnotes?: "popup"|"endnotes"="popup"}` →
  `{readers:[{slug, name, url}], files:[{href, title, kind, key}], file, footnotes, warnings:[{code, message,
  key}]}`. Ohne `readers` (oder leer) die erforderlichen Profile; Reihenfolge immer wie `epub.readers`. Ohne `file` die
  erste Teil- oder Kapiteldatei, sonst die erste Datei mit Text. `files` = Dateien mit Text in Lesereihenfolge
  (Spine ohne `nav.xhtml`/`toc.ncx`; `kind` wie `ItemKind`, z. B. `part`, `chapter`, `front`, `cover`, `notes`).
  `url` = `/werke/{work}/epub/leser/{slug}/{file}` (bei Endnoten `?fussnoten=endnoten`). Unbekannter Slug, unbekannte
  oder unsichere Datei (`..`, `/` am Anfang) → -32602; Werk ohne Abschnitt → -32000.

### epub.check, epub.checker_get, epub.checker_set (R20c) {#epubcheck-epubchecker_get-epubchecker_set-r20c}

- `epub.check {work, readers?: [slug], epubcheck?: bool = false, footnotes?: "popup"|"endnotes" = "popup"}` →
  `{findings: [Finding], xml_tool: "xmllint"|"quick_xml", epubcheck: EpubcheckStatus, summary: {errors, warnings,
  infos}, readers: [slug]}`. Baut das EPUB wie `work.epub` (kein Commit) und prüft es: eigene Struktur- und
  Barrierefreiheitsprüfung (`baruch_epub::check::check_structure`, xmllint aus PATH mit `--noout --nonet`, sonst
  quick-xml), Warnungen des Baus (z. B. `ISLAND_OMITTED`, `TEMPLATE_PAGEBREAK_MAPPED` als Hinweis) und je Lesegerät
  (`reader_warnings`). Ohne `readers` die erforderlichen Profile; unbekannter Slug → -32602; ohne Abschnitte -32000.
  - `Finding = {code, severity: "error"|"warning"|"info", message, path: string|null, reader: slug|null}`. Codes der
    eigenen Prüfung sind u. a. `MIMETYPE_NOT_FIRST`, `MIMETYPE_COMPRESSED`, `CONTAINER_MISSING`, `MANIFEST_FILE_MISSING`,
    `FILE_NOT_IN_MANIFEST`, `SPINE_IDREF_UNKNOWN`, `XHTML_NOT_WELLFORMED`, `NAV_MISSING`, `LANGUAGE_MISSING`,
    `IMG_ALT_MISSING` (Fehler), `IMG_ALT_EMPTY`, `NCX_MISSING`, `A11Y_METADATA_MISSING` (Hinweise),
    `SPINE_NAV_ORDER`, `XHTML_TITLE_MISSING` (Warnungen); vollständig in `baruch_epub::check::codes`. Lesegerät:
    `READER_POPUP_FOOTNOTES`, `READER_PAGE_BREAKS`, `READER_EMBEDDED_FONTS`, `READER_CSS_BACKGROUNDS`, `READER_SVG`,
    `READER_HYPHENATION`, `READER_PUBLISHER_FONT` (nur wenn das Werk eine Schrift setzt; die Grundschrift
    zählt nicht), `READER_NIGHT_MODE`, `READER_CONVERSION`. EPUBCheck:
    `EPUBCHECK_<Code>` (z. B. `EPUBCHECK_RSC-005`), Pfad im EPUB.
  - `EpubcheckStatus = {status: "not_requested"} | {status: "not_found"} | {status: "ran", tool: {kind:
    "binary"|"jar", path}, version: string|null, ok: bool} | {status: "failed", message}`. Mit `epubcheck: true`
    sucht Baruch `epubcheck` in PATH, sonst die konfigurierte epubcheck.jar (`java -jar`); höchstens ein Lauf zugleich,
    Zeitlimit 180 s (dann wird das Programm samt seiner Prozessgruppe beendet, `failed`). Nie ein Download.
- `epub.checker_get {}` → `{jar: string|null, found: {kind: "binary"|"jar", path}|null, java: bool}` (ohne Werk).
- `epub.checker_set {jar: string|null}` → wie `checker_get`. `jar`: absoluter Pfad, Endung `.jar`, eine vorhandene
  reguläre Datei, keine Steuerzeichen, höchstens 4096 Bytes – sonst -32602, nichts gespeichert; `null` (oder leer)
  entfernt die Einstellung (`store_meta.epubcheck_jar`). Kein Commit (kein Werk). Nur RPC und das Formular
  `POST /einstellungen/daten/epubcheck`, kein MCP.
- **Reste der Buchangaben (R20c):** `work.meta.set` lehnt eine ISBN-13 ohne das Präfix 978/979 mit -32602 ab („Eine ISBN-13
  beginnt mit 978 oder 979 …“); das Präfix „ISBN“, „ISBN-13“, „isbn-13“, „ISBN 13“ in beliebiger Schreibweise darf davor stehen;
  eine nur anders geschriebene ISBN (Bindestriche, Präfix) ist eine Änderung und wird wie eingegeben gespeichert
  (Commit „Buchangaben geändert: ISBN“). `work.meta.get` liefert `paper_sheet_mm` (Dicke je Blatt = 2 Seiten);
  `work.meta.set` nimmt `paper_sheet_mm` (über 0 bis 2 mm, gespeichert als `paper_mm` = die Hälfte, keine Migration;
  zusammen mit `paper_mm` → -32602). Bei der Papierdicke nennt der Commit-Rumpf beide („Papierdicke: 0.1 → 0.075 mm je
  Seite (je Blatt 0.2 → 0.15 mm)“). `asset.delete` ohne `rev` auf dem Umschlagbild entfernt zuerst den Umschlag mit
  eigenem Commit „Umschlag entfernt“ und meldet `cover_removed: true` (dazu `cover_commit` oder `cover_mirror`), dann
  das Bild; ebenso das Formular `POST /werke/bilder/act` (`action=remove`) und ein bestätigter Löschantrag.
  `asset.request_decide` mit `confirm` auf einen Umschlagantrag schreibt genau **einen** Commit „Bildantrag <n>
  bestätigt: …“ (Rumpf mit „Umschlag vorher/nachher“); `cover` in der Antwort ist `{work, cover, commit?}`.

### Integration {#integration-1}

Keine RPC-Änderung durch die Integration. Der Abnahmetest `tests/round20_abnahme.rs` nutzt `work.epub` (`inline`),
`epub.preview` (alle Profile), `asset.add`, `work.set_cover`, `work.meta.set`, `work.typeset_set` und
`revision.accept` (Querverweis); `epub.check` durch `epub_check::run_check` mit eigenem PATH-Inhalt.

## Runde 31 – Register und Glossar {#round-31--index-and-glossary}

### Schnittstelle R31-S1 {#interface-r31-s1}

Neue Methoden (Vertrag §3.1/§3.2); in der Schnittstelle bekannt, aber mit REFUSED „noch nicht umgesetzt (R31 …)“ abgelehnt,
seit 31a/31b alle umgesetzt:

- glossar-daten: `glossary.list`, `glossary.get`, `glossary.lookup`, `glossary.add`, `glossary.update`,
  `glossary.delete`, `glossary.decide`, `glossary.suggest`, `index.registers`, `index.register_add`,
  `index.register_rename`, `index.register_delete`, `index.proposals`, `index.decide`.
- register-satz: `index.entries`, `index.mark`, `index.unmark`, `index.overview`.

Die neuen `StoreError`-Varianten (`Glossary`, `UnknownGlossaryTerm`, `UnknownIndexProposal`) kommen als INVALID_PARAMS.
`work.typeset_get`/`work.typeset_set` tragen die neuen Felder `glossary_page`, `index_pages {registers, columns}` und
`glossary_first_mention` durch das JSON der Satz-Einstellungen (Prüfung: mindestens ein Register, gültige und
eindeutige Kennungen, höchstens 16, 1–3 Spalten).

### 31a glossar-daten {#31a-glossar-daten}

Umgesetzt (`src/glossary.rs`, `src/assist/glossary.rs`). Werk durch `work` (fehlend → Standardwerk). Begriff =
`{id, uuid, term, short, long, variants, source, bible_refs, status: "accepted"|"proposed", origin, created, updated,
updated_by, anchor}` (`anchor` = `baruch-gl-<uuid>`). Schreibende Methoden: genau ein Spiegel-Commit
(`commit` in der Antwort; scheitert der Spiegel, `mirror` mit der Meldung, die Änderung gilt trotzdem).

| Methode | Parameter | Ergebnis | Commit (`Baruch-Action`) |
|---|---|---|---|
| `glossary.list` | `{work?, status?: "accepted"\|"proposed"\|"all"}` (Standard `all`) | `{work, terms: [Term]}` nach `sort_key` | – |
| `glossary.get` | `{work?, id}` | `{work, term}` | – |
| `glossary.lookup` | `{work?, word}` | `{work, word, terms}` – nur angenommene, nach Begriff oder Variante | – |
| `glossary.add` | `{work?, term, short?, long?, variants?, source?, bible_refs?}` | `{work, term, commit?}` | `glossary-add` (Autor Mensch) |
| `glossary.update` | `{work?, id, term, …}` wie add | `{work, term, commit?}` | `glossary-update` (Rumpf vorher/nachher) |
| `glossary.delete` | `{work?, id, confirm: true}` | `{work, deleted: id, commit?}` | `glossary-delete` (Rumpf mit dem Begriff) |
| `glossary.decide` | `{work?, id, accept}` | `{work, term}` oder `{work, deleted}` | `glossary-accept`/`glossary-reject` (`Baruch-Proposer`) |
| `glossary.suggest` | `{work?, key?, persona?}` | `{work, persona, model, proposed: [Term], skipped: [{term, reason}], notes}` | je Vorschlag `glossary-propose` (Autor die Persona) |
| `index.registers` | `{work?}` | `{work, registers: [{slug, title, kind, builtin}]}` | – |
| `index.register_add` | `{work?, slug, title}` | `{work, register: {slug, title, uuid, position, created}, commit?}` | `register-add` |
| `index.register_rename` | `{work?, slug, title}` | wie add | `register-rename` |
| `index.register_delete` | `{work?, slug, confirm: true}` | wie add, dazu `deleted: slug` | `register-delete` |
| `index.proposals` | `{work?, status?: "open"\|"all"}` (Standard `open`) | `{work, proposals: [{id, key, based_on, word, occurrence, mark, reason, origin, status, created, decided, decided_by, url}]}` | – |
| `index.decide` | `{work?, id, accept}` | accept: `{work, proposal, key, rev, marks, commit?}`; reject: `{work, proposal, commit?}` | `index-accept` (neue Fassung des Menschen, `Baruch-Node`, `Baruch-Revision`, `Baruch-Proposer`) oder `index-reject` |

- `variants` und `bible_refs` als Liste von Texten (ein einzelner Text zählt als Liste mit einer Zeile je Eintrag).
  Bibelstellen werden mit `baruch_scripture::parse` geprüft und normalisiert (mehrere mit `;` werden getrennte Einträge);
  unlesbare → INVALID_PARAMS mit einer Meldung, nichts gespeichert.
- Fehler: Grenzen, Duplikate (`term_key` unter den angenommenen), eingebaute/ungültige Registerkennung, unbekannter Begriff
  oder Vorschlag, bereits entschiedener Vorschlag, Wort steht nicht mehr im Text (Vorschlag wird `stale`) → INVALID_PARAMS;
  `glossary.delete`/`index.register_delete` ohne `confirm: true` → INVALID_PARAMS; Fehler des Modells bei
  `glossary.suggest` (kein Backend, keine Antwort in der Form "Term | explanation") → REFUSED.
- `index.decide` mit `accept: true` setzt die Marke mit `baruch_typst::index::mark_word`; steht das Wort nicht mehr im
  Text oder trägt die gewählte Fundstelle schon dieselbe Marke → INVALID_PARAMS mit einer Meldung, der Vorschlag wird
  `stale`, keine neue Fassung.
- `work.exchange_preview`/`work.exchange_import`: `add` beziehungsweise `added` zählen zusätzlich `registers` und
  `glossary_terms`; Abweichungen der Arten `register` und `glossar-begriff` stehen in `deviations`.

### 31b register-satz {#31b-register-satz}

Fehler sind INVALID_PARAMS mit einer deutschen Meldung (Abschnitt oder Fassung unbekannt, Wort fehlt oder steht nicht im
Abschnitt außerhalb von Überschriften, Code, Kommentaren und Marken, ungültige Marke, unbekanntes Register, kein
Markenbereich).

- `index.entries {work?, key?}` → `{work, entries:[{key, register, term, sub, see, also, auto, start, end}]}`: die
  Marken der aktuellen Fassungen (`start`/`end` = Byte-Bereich der Marke im gespeicherten Text, für `index.unmark`);
  ist das Register `bibel` in den Registerseiten eingeschaltet, auch die automatischen Bibelstellen der Projektion
  (`auto: true`, `term` = Buch, `sub` = Kapitel/Vers, `start`/`end` = null). Unbekannter `key` → INVALID_PARAMS.
- `index.mark {work?, key, rev, word, occurrence?, register?, term?, sub?, see?, also?}` → `{work, key, node, rev,
  marks, commit?|mirror?}`: setzt die Marke direkt hinter das Wort (`occurrence` = `"first"` (Standard), `"all"` oder
  eine Zahl ab 1; `register` Standard `stichwort`, muss im Werk existieren; `term` Standard = `word`) und speichert
  eine neue Fassung des Menschen auf `rev` (ein Spiegel-Commit). `marks` = Zahl der gesetzten Marken. Dieselbe Marke
  wird nie zweimal gesetzt: Fundstellen, hinter denen sie schon steht (auch in einer Kette mehrerer Marken), bleiben;
  ist die gewählte Fundstelle schon markiert oder ist bei `"all"` keine mehr offen → INVALID_PARAMS „… trägt an der
  gewählten Fundstelle schon diese Registermarke; nichts geändert …“, keine neue Fassung.
- `index.unmark {work?, key, rev, start, end}` → `{work, key, node, rev, commit?|mirror?}`: neue Fassung ohne die Marke
  genau in diesem Bereich.
- `index.overview {work?}` → `{work, registers:[{slug, title, kind, terms:[{term, sub, keys, see, also, auto}]}]}`:
  alle Register des Werks, Einträge sortiert wie auf den Registerseiten (DIN 5007-1, Bibel in biblischer Reihenfolge),
  `keys` = Abschnitte mit Fundstellen.

`work.export` trägt bei Registerseiten oder einer Glossarseite den Anhang ab der Zeile `// Baruch: Register und Glossar`;
`work.epub` die Dateien `text/glossary.xhtml` und `text/index-{slug}.xhtml`.

### 31c Kontextmenü {#31c-kontextmenue}

Keine neue RPC-Methode. Das Kontextmenü (`web/src/context_menu.ts`) ruft aus 31a `glossary.add` („In Glossar aufnehmen“),
`glossary.lookup` („Im Glossar nachschlagen“) und `index.registers` (Wahl des Registers) auf; es setzt Registermarken nicht
über RPC, sondern im Editor (Satzansicht oder Textfeld); das Speichern geht wie immer durch `outline.save`.

- `wysiwyg.parse`: Eine Registermarke erscheint in `html` als `span.ws-index-mark` (Felder `data-register`, `data-term`,
  `data-sub`, `data-see`, `data-also`, `data-sort`, Beschreibung in `title`/`aria-label`, `data-raw` = kanonische Form),
  in `blocks` als Inline `{"t": "index_mark", "mark": {…}}`.
- `wysiwyg.serialize`: nimmt `{"t": "index_mark", "mark": {"register", "term", "sub"?, "see"?, "also"?, "sort"?}}` in
  neu erzeugten Blöcken und schreibt `mark_source`; unbekannte Felder der Marke → `INVALID_PARAMS`.

Formulare ohne JavaScript (keine RPC, `docs/Bedienung.md`): `POST /werke/{id}/glossar/act` (`action` =
add|update|delete|accept|reject|suggest; `suggest` mit optionalem `key` ruft `glossary.suggest` auf und leitet auf
`/werke/{id}/glossar?ok=suggest&neu=N&uebersprungen=M#vorschlaege` um, Fehler des Modells 422 mit einer Meldung) und `POST /werke/{id}/register/act` (`action` =
mark|unmark|accept|reject|register_add|register_rename|register_delete) → 303 mit `ok=<action>`, Fehler 422 mit der
Eingabe; Satz-Einstellungen durch `POST /einstellungen/werk/satz` mit den neuen Feldern `glossary_page`
(`before_indexes|after_indexes`, leer = keine), `index_register` (wiederholbar, Satzreihenfolge), `index_columns` (1–3, leer
= 2) und `glossary_first_mention` (`on|off`, leer = nicht angegeben).

### Integration {#integration-2}

- Keine neue Methode. `work.import` und die Kommandozeile `import` nehmen den Anhang (ab
  `// Baruch: Register und Glossar`) und die Zusätze der Projektion (automatische Bibelstellenmarken, verlinkte
  Ersterwähnung) aus einem Export mit Anhang heraus, bevor die Abschnitte angelegt werden (`baruch_typst::index::strip_back_matter`,
  `strip_projection`).
- MCP `initialize` → `instructions` nennen die drei Glossar-Werkzeuge (nur Vorschläge).

## Runde 22 – Fassungen und Spiegel {#round-22--revisions-and-mirror}

Fortschreiben statt neuer Fassung, Verlauf und Rückgängig, Einstellung „Fassungen“, Rückfragen zu erledigten Befunden,
Archiv, Entwurf als Wasserzeichen, Austausch-Unterversion 8. Migration 0050 (`0050_revision_amend.sql`).

### Fassungen {#fassungen}

#### Methoden (`revision.save` und `outline.save` stehen auch in der Tabelle oben) {#methods-revisionsave-and-outlinesave-also-stand-in-the-table-above}

| Methode | Parameter | Ergebnis |
|---|---|---|
| `revision.save` | `{node, body, based_on?, based_on_amend?, mode?}` | `{node, rev, saved, amend_seq?, reason?, reason_label?, commit?, mirror?}` – wie bisher; Runde 22: `mode` `"auto"` ergänzt die eigene aktuelle Fassung des Autors, wenn keine Regel eine neue Fassung verlangt (siehe „Amending“), `"new"` legt immer eine neue Fassung an; **fehlt `mode`, gilt `"new"`** (alle alten Clients unverändert); unbekannter Wert → -32602. `saved`: `"amended"` (mit `amend_seq` = neuer Stand), `"new"` (im Modus `"auto"` mit `reason` und `reason_label`) oder `"unchanged"` |
| `outline.save` | `{node, body, based_on?, based_on_amend?, split?, merge?, mode?}` | `{node, rev, saved, amend_seq?, reason?, reason_label?, outline?, commit?, mirror?}` – wie bisher; `mode` wie bei `revision.save`. Teilen, Zusammenführen, eine neue Überschrift oder Ebene ergeben immer eine neue Fassung (`reason: "heading_changed"`) |
| `revision.amends` | `{node\|key, rev}` | `{node, key, rev, amend_seq, steps: [{uuid, seq, kind, undo_of?, author, created, body_before, body_after}]}` – Verlauf der Fortschreibungen der Fassung `rev` nach `seq` (nur lesend); `kind` `"amend"` (Fortschreibung durch den Menschen) oder `"undo"` (Zurücknehmen durch den Verlauf, `undo_of` = Schritt, dessen früherer Zustand wiederhergestellt wurde); unbekannte Fassung → -32602 wie bei den anderen Fassungsmethoden |
| `revision.amend_undo` | `{node\|key, rev, seq}` | `{node, key, rev, amend_seq, commit?, mirror?}` – setzt den Text der Fassung auf den Zustand **vor** dem Schritt `seq` (`body_before`) – als **neuer** Schritt (`kind: "undo"`, `undo_of: seq`), nie durch Löschen von Verlaufszeilen; nur auf der aktuellen Fassung des Menschen, sonst -32000 mit einer deutschen Meldung („… nur auf der aktuellen Fassung …“ oder „… nur auf einer Fassung des Menschen …“); unbekannter Schritt oder gleicher Zustand → -32000. Nur der Mensch: nicht über `/act`, nicht über MCP (Formular `POST /werke/fassungen/act`) |
| `revision.policy_get` | `{work?}` | `{work, amend, max_change_percent, new_after_minutes, defaults: {amend, max_change_percent, new_after_minutes}, limits: {max_change_percent: [1, 100], new_after_minutes: [0, 1440]}}` – Einstellung „Fassungen“ des Werks (fehlt sie, der Standard: an, 100 %, 0 min – seit 2026-11-27 ohne Größen- und Zeitgrenze, Speichern schreibt immer fort) |
| `revision.policy_set` | `{work?, amend?, max_change_percent?, new_after_minutes?}` | `{work, changed: [Feld…], commit?, mirror?}` – setzt die Einstellung (fehlende Felder bleiben; Wahrheitswerte auch `"1"`/`"0"`, Zahlen auch als Text); Grenzen 1–100 % oder 0–1440 min (0 = keine Zeitgrenze) → sonst -32602 mit einer Meldung; nur bei einer Änderung ein Commit „Fassungen: <Felder> geändert“ (`Baruch-Action: revision-policy-set`). Nur der Mensch: nicht über `/act`, nicht über MCP (Formular `POST /einstellungen/werk/fassungen`) |

`revision.tree` und `node.get`: Jede Fassung (`RevisionInfo`) trägt `amend_seq` (Stand, 0 = nie fortgeschrieben)
und bei Vorschlägen, die auf einer fortgeschriebenen Basis entstanden sind, `based_on_amend` (Stand der Basis).

#### Fortschreiben {#amending}

Speichert der Mensch im Editor (Modus `"auto"`) auf seiner aktuellen Fassung, wird sie **fortgeschrieben**: Text
ersetzt, `amend_seq` + 1, eine Zeile in `revision_amend` (Text vorher und nachher, Mensch, Zeit, UUID). Nummer,
Herkunft, Status, `based_on` und `edited` bleiben (`edited` setzt nur `revision.update`). Stattdessen entsteht eine **neue Fassung**
aus dem ersten zutreffenden Grund (`reason`, feste Reihenfolge, `baruch_core::revision_policy::decide`):

| `reason` | `reason_label` | wann |
|---|---|---|
| `requested` | „als neue Fassung gewünscht“ | Modus `"new"` (dann ohne `reason` in der Antwort) |
| `no_current` | „keine aktuelle Fassung“ | Abschnitt ohne aktuelle Fassung |
| `policy_off` | „Fortschreiben ausgeschaltet“ | Einstellung „Fassungen“: aus |
| `not_own` | „aktuelle Fassung stammt nicht vom Menschen“ | Fassung einer Persona/eines MCP-Clients, auch jeder übernommene Vorschlag |
| `not_on_current` | „nicht auf der aktuellen Fassung bearbeitet“ | `based_on` ≠ aktuelle Fassung, oder `based_on_amend` ≠ Stand der aktuellen Fassung (inzwischen anderswo fortgeschrieben, z. B. in einem zweiten Tab) |
| `heading_changed` | „Überschrift oder Gliederung geändert“ | Titel, Ebene, Teilen, Zusammenführen, Titel eines Teils |
| `large_change` | „große Änderung“ | `change_percent` > `max_change_percent` |
| `time_elapsed` | „Zeitgrenze überschritten“ | `new_after_minutes` > 0 und seit dem letzten Schreiben (Maximum aus dem Anlegen der Fassung und der letzten Fortschreibung) mindestens so viele Minuten |

`change_percent(alt, neu) = (1 − similar::TextDiff::from_words(alt, neu).ratio()) · 100`. Hinweis: Die Wortgrenzen
(Leerraum) zählen als gleiche Teile; ein ganz neuer Satz mit gleich vielen Wörtern liegt daher bei etwa
50 %, eine Grenze von 20 % (der Standard bis 2026-11-26) entspricht grob einem Drittel bis der Hälfte geänderter Wörter.

Nur das Speichern aus dem Editor schreibt fort (`outline.save`/`revision.save` mit `mode: "auto"`). Das Einfügen eines Bildes,
Registermarken, das Umwandeln eines Verweises, Ersetzen und Gliederungsaktionen erzeugen weiterhin neue Fassungen. Personas und
MCP legen immer Vorschläge an. `add_proposal`/`add_proposal_on` merken den Stand der Basis (`based_on_amend`).

#### Git-Spiegel und Nebenzweige {#git-mirror-and-side-branches}

Jede Fortschreibung und jedes Rückgängigmachen ist genau ein Commit (auch bei gleichem Baum): Betreff
„<Nummer> <Titel>: Fassung n fortgeschrieben (Stand k)“ oder „…: Fortschreibung von Fassung n zurückgenommen
(Stand k)“, Trailer `Baruch-Action: amended` oder `amend-undone` und `Baruch-Amend: k`. Die Einstellung „Fassungen“
ist ein leerer Log-Commit „Fassungen: <Felder> geändert“ mit vorher → nachher im Rumpf
(`Baruch-Action: revision-policy-set`).

**Nebenzweige (Runde 22):** Jede nicht aktuelle Fassung (ersetzt, verworfen, offener Vorschlag mit seinen Varianten)
steht als Referenz `refs/revisions/<key>/<n>` im Spiegel (key = fester Schlüssel des Abschnitts, n =
Fassungsnummer): ein Commit mit dem Schnappschuss des Werks, in dem der Abschnitt diese Fassung trägt (Abschnittsdatei
mit der Importzeile der Werkvorlage, Label `<kN>` und Bildpfade von der Wurzel wie in HEAD, ohne den Schalter „Nummer an/aus“ und ohne
die Registerprojektion), Elternteil = HEAD zum Zeitpunkt des Anlegens; Autor wie bei den Fassungs-Commits, Trailer `Baruch-Node`,
`Baruch-Key`, `Baruch-Revision`, `Baruch-Side` (`superseded`|`rejected`|`proposed`) und bei Personas `Persona`/`Model`.
Aktualisiert nach jedem Fassungs-Commit (Speichern, Fortschreiben, Umschalten, Ändern, Löschen, Gliederung, Austausch),
nach `revision.reject` und beim Start; eine unveränderte Fassung behält ihren Commit, eine gelöschte verliert ihre
Referenz. Arbeitsordner, Index und HEAD bleiben unberührt. Lesen: `git log refs/revisions/<key>/<n>` im
Spiegelordner. Ein neuer Vorschlag selbst ist kein Commit; der Server legt seine Referenz gleich danach an
(`mirror::side_refs_after_proposal`, Integrator).

#### Routen (Formulare ohne JavaScript) {#routes-forms-without-javascript}

- `GET /werke/{id}/fassungen/verlauf?key=K&rev=R` – Verlauf einer Fassung, neueste Schritte zuerst, je Schritt der
  Wortunterschied (vorher → nachher) und, auf der aktuellen Fassung des Menschen, „Bis hier zurück“; `ok=1` zeigt den
  Hinweis nach einem Rückgängigmachen.
- `POST /werke/fassungen/act` – `method=revision.amend_undo`, `work`, `key`, `rev`, `seq` → 303 auf den Verlauf mit
  `ok=1`; abgelehnt: Verlauf mit einer Meldung (409/422).
- `POST /einstellungen/werk/fassungen` – `work`, `amend` (Kontrollkästchen, fehlt = aus), `max_change_percent`,
  `new_after_minutes` → 303 auf `/einstellungen/werk?work=W&ok=fassungen#fassungen`, abgelehnt
  `…&fassungenfehler=1#fassungen` (nichts geändert).
- `POST /gliederung/act` mit `method=outline.save`: nimmt `mode` (der Editor sendet `auto`) und `as_new=1` (Schaltfläche
  „Als neue Fassung speichern“ → `mode` `"new"`); die Umleitung trägt `saved=amended-<k>` oder
  `saved=new-<n>[-<grund>]`, woraus der Editorkopf die Rückmeldung zeigt.
- `/act` nimmt weiterhin `revision.save` mit `mode`; es lehnt `revision.amend_undo` und `revision.policy_set` ab.

#### Ältere Fassungen ausblenden (0.9.0, Nutzer 2026-11-24) {#hiding-older-revisions-090-user-2026-11-24}

Je Abschnitt höchstens eine **Marke** „bis hierhin“ (Nummer einer Fassung, Tabelle `node_revision_fold`, Migration
0051). Ausgeblendet werden **ersetzte und verworfene** Fassungen mit kleinerer Nummer; **nie** die aktuelle Fassung und
**nie** offene Vorschläge (ältere auch nicht; sie warten auf eine Entscheidung). Liegt die Marke über der aktuellen
Fassung (weil später auf eine ältere umgeschaltet wurde), gilt sie nur bis unterhalb der aktuellen (`cutoff`); sie lässt sich
nicht über der aktuellen setzen (-32000). Nichts wird gelöscht: Fassungen, Verlauf und Spiegel bleiben
unverändert, kein Commit im Spiegel (kein Buchtext); protokolliert in der Datenbank (`updated`, `updated_by`) wie die
Einstellung „Fassungen“; nicht im Austauschformat. Nur der Mensch setzt und entfernt sie – über RPC und Formular, **nicht
über MCP** (kein Werkzeug). Regel in `baruch_core::revision_fold`, Methoden in `src/revision_fold.rs`.

| Methode | Parameter | Ergebnis |
|---|---|---|
| `revision.fold_get` | `{node\|key}` | `{node, key, below, cutoff, hidden, updated?, updated_by?}` – `below` = gespeicherte Marke (`null` = keine), `cutoff` = wirksame Grenze (höchstens die aktuelle Fassung), `hidden` = Zahl der ausgeblendeten Fassungen (nur lesend) |
| `revision.fold_set` | `{node\|key, rev}` | `{node, key, rev, below, cutoff, hidden, updated, updated_by}` – setzt die Marke an der Fassung `rev` (ersetzt eine vorhandene); unbekannte Fassung → -32602 wie bei den anderen Fassungsmethoden, über der aktuellen Fassung → -32000 („Über der aktuellen Fassung lässt sich nicht ausblenden …“). Auch über `/act` |
| `revision.fold_clear` | `{node\|key, rev?}` | `{node, key, cleared, below: null, cutoff: null, hidden: 0, rev?}` – entfernt die Marke („Wieder alle zeigen“); `cleared` = es gab eine; `rev` wird nur für den Rücksprung von `/act` durchgereicht. Auch über `/act` |
| `revision.fold_before` | `{work?, date}` | `{work, date, sections: [{key, below}]}` – werkweit: für jeden Abschnitt die Marke auf der letzten Fassung, die vor `date` entstand (Tag `JJJJ-MM-TT`, Mitternacht Ortszeit), höchstens auf der aktuellen; Abschnitte ohne eine solche Fassung bleiben unverändert, vorhandene Marken werden ersetzt; ein Schritt. Ungültiger Tag → -32602 |
| `revision.fold_clear_all` | `{work?}` | `{work, cleared}` – entfernt alle Marken des Werks (Zahl) |

Ansicht: Auf der Werkseite überspringt das Blättern ‹ › ausgeblendete Fassungen, der Zähler zeigt „2/3 (+ 24
ausgeblendet)“. Der Schalter steht in der Adresse: `&fassungen=alle` (nach `lq`) zeigt alle, auch beim Blättern;
`ui.render` nimmt `fassungen` wie die anderen Felder der Adresse. Eine ausgeblendete Fassung, die direkt über `?rev=` aufgerufen wird,
öffnet den Schalter von selbst.

Routen (ohne JavaScript):

- `/act` nimmt `revision.fold_set` und `revision.fold_clear` (`work`, `key`, `rev`) → 303 auf die Fassung.
- `POST /einstellungen/werk/fassungen/ausblenden` – `work`, `date` und `do=fold` (Standard) oder `do=clear` (alle
  Marken entfernen) → 303 auf `/einstellungen/werk?work=W&ok=ausgeblendet#fassungen` oder `ok=eingeblendet`;
  abgelehnt `…&ausblendenfehler=1#fassungen` (nichts geändert).

### Befund-Rückfrage {#befund-rueckfrage}

#### Werke: Löschen, Archiv, Wiederherstellen {#works-deleting-archive-restoring}

| Methode | Parameter | Antwort |
|---|---|---|
| `work.delete` | `{work, confirm_title}` | wie bisher, seit Runde 22 zusätzlich `ideas_commit?` oder `ideas_mirror?`: Danach aktualisiert der Server den Spiegel der Einfälle ohne Werk (`<data>/einfaelle/`) – die Einfälle des gelöschten Werks sind dort sofort; ohne Änderung kein Commit |
| `work.archives` | `{}` | `{archives: [{slug, title, deleted, folder?, restorable, reason?}]}` – gelöschte Werke aus `work_retired`, neueste zuerst (nur lesend). `folder` = Archivordner `works/.geloescht/<slug>-<JJJJ-MM-TT>[-n]` relativ zum Datenordner (bei mehreren der jüngste; ein Werk, dessen Slug nur mit `<slug>-` beginnt, passt nicht); `restorable: false` mit einem deutschen `reason`, wenn der Ordner oder `main.typ` fehlt oder ein lebendes Werk denselben Titel trägt |
| `work.restore` | `{slug}` | `{work, slug, title, nodes, folder, skipped_images, commit?\|mirror?}` – stellt ein archiviertes Werk wieder her (nur der Mensch; **nicht** über `/act` und MCP). Unter der Schreibsperre liest der Server den Archivordner (keine symbolischen Links – weder Ordner noch `abschnitte/` noch Dateien –, nur `main.typ`, `vorlage.typ` und die eingebundenen `abschnitte/<name>.typ`, zusammen höchstens 16 MiB, höchstens 10 000 Abschnitte), setzt den Typst-Text wie bei einem Export zusammen (jede Zeile `#include "abschnitte/…"` durch den Text der Datei ersetzt, ohne die Zeile `#import "../vorlage.typ": *`, ohne Werkvorlage ohne die Kopfzeile `// Baruch: <Titel>`; Anhang und Projektion wie beim Import entfernt) und legt das Werk mit dem reservierten Slug an (`Store::restore_retired`, eine Transaktion): Titel aus `work_retired`, Werkvorlage als Fassung 1, jede Überschrift ein Gliederungspunkt mit Fassung 1 vom Menschen (aktuell, Notiz „aus dem Archiv wiederhergestellt“), ein Regelblock „// Baruch: Satz“ wird wieder zu Satz-Einstellungen, die Zeile in `work_retired` entfällt; das Werk erhält eine neue ID und die Art „Buch“ (`work_retired` kennt die Art nicht). Danach wandert der Ordner zurück nach `works/<slug>` und erhält **einen** Commit „Werk wiederhergestellt: <Titel>“ (`Baruch-Action: work-restored`, auch bei gleichem Stand). `skipped_images` = Bilder unter `bilder/` des Archivs, die **nicht** mitkommen. Fehler: fehlendes `slug` → -32602; unbekannter Slug („steht nicht im Archiv“), kein Archivordner, symbolischer Link, fremdes `#include`, zu groß, `works/<slug>` existiert schon, Titel vergeben, dieselbe Quelle bereits als Werk importiert → -32000 mit einer deutschen Meldung, nichts geändert; keine Überschrift → -32602. Ein Fehler beim Verschieben oder im Spiegel stoppt das Werk in der Datenbank nicht (`mirror`) |

Ohne JavaScript: Seite `GET /werke/archiv` (Liste mit Archivordner und einem Formular je Werk) und
`POST /werke/archiv/wiederherstellen` (Felder `slug`, `confirm` = `1`; hinter demselben Schutz wie `/werke/act`):
Erfolg → 303 `/?work=<id>`; ohne Bestätigung → 422, Fehler → 409/422, jeweils die Archivseite mit einer Meldung
(`role="alert"`).

#### Befunde: Zustand und Rückfrage {#findings-state-and-follow-up-question}

- `Finding` und `FindingStatusEntry` tragen seit Runde 22 `adopted_amend: number | null` (Migration 0050): den
  Fortschreibungsstand (`revision.amend_seq`) der übernommenen Fassung. `finding.set_status` mit `adopted`
  merkt sich den Stand der Fassung in diesem Moment (Befund und Verlaufseintrag); ein Vorschlag „übernommen“ eines
  Assistenten merkt im Eintrag den Stand zum Zeitpunkt des Vorschlags, `finding.decide` mit `confirm: true` setzt Befund **und**
  Eintrag auf den Stand bei der Bestätigung. Jeder andere Status setzt `finding.adopted_amend` auf `null` (der Verlauf
  behält seine Werte). Anzeige „Fassung n · Stand k“ nur bei k > 0 (Karte, Liste, Verlauf je Kapitel der Matrix,
  `befunde.md` im Spiegel: „- Übernommen in Fassung 1 · Stand 3“); Stand 0 oder `null` zeigt wie bisher nur „Fassung n“.
- `finding.matrix`: `history[].revisions[]` (`AdoptedIn`) hat zusätzlich `stands: [number]` – je Befund in `findings`
  (gleiche Reihenfolge) der gemerkte Stand, 0 = keiner.
- Rückfrage einer Persona („Befund n scheint erledigt?“, siehe Bedienung): ein offener Vorschlag `done` eines
  Assistenten mit `note` `„Befund n scheint erledigt (Stelle „…“) – als erledigt markieren?“` und `reason` `„laut
  @persona umgesetzt (Stelle „…“)“`. Er steht in `finding.list`/`finding.get` als `pending`; entschieden wird er mit
  `finding.decide {entry, confirm: true|false}` („Ja, erledigt“ / „Nein, noch offen“). Keine neuen RPC-Namen.

#### Personas: erneutes Prüfen (`persona.review`, `persona.chat`, `post.repeat`) {#personas-re-checking-personareview-personachat-postrepeat}

Das erneute Prüfen (`persona.review`, `post.repeat`, RFC durch `persona.chat`, MCP `ask_persona` mit `mode: "review"`)
gibt der Persona ihre eigenen offenen Befunde des Abschnitts mit (siehe Architektur). Meldet sie „Erledigt: Befund
n“, entsteht statt eines neuen Befunds ein Statusvorschlag `done` (genau ein Commit `Baruch-Action:
finding-propose`, Trailer `Persona`). Das Ergebnis von `assist::run` (`Outcome`) trägt zusätzlich `done_proposals` (IDs mit
einem neuen Vorschlag), `done_notes` (übersprungene Meldungen mit einem Grund) und `all_done`; `persona.ask`/`persona.review`
geben die drei Felder in ihrer Antwort aus, sobald es eine Meldung gab (sonst fehlen sie wie bisher).
`persona.chat` und MCP `ask_persona` geben sie nicht gesondert aus (offen nach Runde 22). `post.repeat` schlägt einen
Befund mit einer offenen Rückfrage nicht noch einmal als „erledigt“ vor (Integrator); ein offener Vorschlag anderer Art
hält es nicht auf. Meldungen zu Befunden, die nicht in der Aufgabe waren (anderer Abschnitt, kein erneutes Prüfen),
überspringt Baruch mit einem Hinweis in `done_notes` (PM-Abschluss R22).

### Spiegel-Entwurf {#spiegel-entwurf}

#### Entwurf (Wasserzeichen) auf den Ausgaben – Runde 22 {#draft-watermark-on-the-outputs--round-22}

Parameter `draft?: bool` (dreiwertig) bei `work.pdf`, `notes.pdf`, `work.epub`, `work.cover_pdf` und `preview.typst`:

| `draft` | Wirkung |
| --- | --- |
| fehlt / `null` | nach der Satz-Einstellung `draft` (Umschlag-PDF nur mit `draft.cover`) |
| `true` | mit Wasserzeichen: gespeicherte Werte, sonst Standard („ENTWURF“, 12 %, diagonal) |
| `false` | ohne Wasserzeichen (Quelltext und PDF byte-identisch zum Werk ohne die Einstellung) |

Kein Weg ändert die gespeicherte Einstellung. Ein anderer Wert als `true`/`false`/`null` → -32602 „draft: true (mit
Wasserzeichen), false (ohne) oder weglassen (nach Einstellung)“.

- `work.pdf {work, inline?, draft?}` – Antwort wie bisher; mit `draft` trägt `url` den Schalter
  (`/werke/{id}/pdf?entwurf=1|0`).
- `notes.pdf {work, format?, font_pt?, inline?, draft?}` – `url` mit `&entwurf=1|0`.
- `work.epub {work, inline?, footnotes?, draft?}` – mit Entwurf: Titelseite mit einem Hinweis („Entwurf: <Text> · <Zusatz>“;
  ohne Vorspann wird dafür eine Titelseite angelegt), `dc:title` und `docTitle` „<Titel> (Entwurf)“; `url` mit `?entwurf=…`.
- `work.cover_pdf {work, inline?, draft?}` – zusätzlich `draft: bool` (das PDF trägt ein Wasserzeichen); `checks`
  enthält dann die Warnung `{code: "draft", block: null, message: "Das Umschlag-PDF trägt das Wasserzeichen „Entwurf“ –
  so nicht an die Druckerei geben …"}`.
- `preview.typst {work?, key?, page?, draft?}` – zusätzlich `draft: bool` in der Antwort (der Satzquelltext trägt das
  Wasserzeichen); `stand` unterscheidet sich je Schalter.
- `work.typeset_get/set/reset/preview` tragen das Feld `settings.draft` (`{text, note?, date, opacity_percent,
  placement: "diagonal"|"header"|"footer", cover}`), geprüft in `TypesetSettings::validate` (Text getrimmt 1–40 Zeichen
  ohne Steuerzeichen, Zusatz getrimmt ≤ 120 ohne Steuerzeichen, Deckkraft 5–50 %; Meldungen „Entwurf: …“, Felder
  `draft.text`, `draft.note`, `draft.opacity_percent`). `rules` bleibt ohne Zeile für den Entwurf. Spiegel-Commit
  „Satz: Entwurf geändert“ mit `„Entwurf: aus → an: „<Text>“, <Lage>“` im Rumpf.

Routen (GET, Parameter `?entwurf=1|0`, fehlt = nach der Einstellung; ein anderer Wert → 400): `/werke/{id}/pdf`,
`/werke/{id}/stichwortzettel.pdf`, `/werke/{id}/epub`, `/werke/{id}/umschlag.pdf`, `/werke/{id}/satz` (dort auch auf den
Links der Seite und `…/satz/{stand}/{n}.svg?entwurf=…`; ein unlesbarer Wert zählt auf der Seite als „fehlt“).

#### Austausch-Unterversion 8 – Runde 22 {#exchange-sub-version-8--round-22}

`work.exchange_export` schreibt die Unterversion 8 (`docs/Austauschformat.md`, Abschnitt zu Fortschreibungen, Entwurf und
Stand). `work.exchange_preview`/`work.exchange_import`: `added.amends` (Fortschreibungen, auch beim Vorspulen),
Abweichung „Fassung n: vorgespult von Stand k auf Stand m“ (Hinweis); ein neuer Teil, der Kapitel übernähme, wird ein
Antrag `add` mit Kennung und Fassungen (Abweichung „… Antrag mit Kennung und n Fassung(en)“).
`outline.requests`/`outline.request_decide`: Die Zusammenfassung eines solchen Antrags lautet `„Teil „…“ (mit Kennung und
n Fassungen) vor „…“ anlegen; … wandern unter den neuen Teil“`; das Zustimmen legt den Teil mit dieser Kennung und diesen
Fassungen an (ein Spiegel-Commit wie bisher).

### Integration {#integration-3}

- `node.get`: Jede Fassung trägt wie in `revision.tree` `amend_seq` und bei Vorschlägen auf einer fortgeschriebenen Basis
  `based_on_amend`.
- Jeder neue Vorschlag (Personas, MCP `propose_revision`, Zitate aus Literatur und Bibel, Varianten, Lösungen) legt
  sofort seinen Nebenzweig `refs/revisions/<key>/<n>` an, ohne eigenen Commit.
- `post.repeat`: `proposed_done`/`notes` überspringen Befunde, zu denen die Persona in derselben Antwort schon
  „Erledigt: Befund n“ gemeldet hat (offene Rückfrage).
- Erneuter Import eines Exports mit dem Block „Überschriften ohne Nummer über die Sprungmarke“: Der Block verschwindet aus
  der Vorlage, die genannten Abschnitte kommen „ohne Nummer“ zurück.

## Runde 23 – Härtung {#round-23--hardening}

Keine neuen Methoden; geänderte Parameter, Antworten und Fehler. Die Tabellen oben nennen die Änderung jeweils kurz
und verweisen hierher.

### Web {#web}

**`revision.update {node|key, rev, body?, note?, based_on_amend?}` → `{node, rev, changed, amend_seq?, mirror?}`**
(geändert, Runde 23a). Ändert sich der **Text** der aktuellen Fassung oder einer bereits fortgeschriebenen (Stand > 0),
erzeugt das Ändern durch den Baum einen Schritt im Verlauf (`revision_amend`, Art `amend`, Text vorher und nachher) und
erhöht den Stand; die Antwort nennt ihn in `amend_seq`. Eine nie fortgeschriebene, nicht aktuelle Fassung (z. B. ein
Vorschlag) erhält wie bisher keinen Verlauf, ebenso wenig eine bloße Änderung der Notiz. Neu ist der optionale Parameter
`based_on_amend` (Stand, den der Client geladen hatte): Steht die Fassung inzwischen auf einem anderen Stand,
wird nichts geändert, und die Methode meldet -32000 „Veraltet: Fassung n steht inzwischen auf Stand k (geladen war Stand j); bitte
neu laden und die Änderung dort eintragen.“ (`StoreError::Stale`). Ohne den Parameter wird nichts geprüft (alte Clients).
Das Formular „Fassung ändern“ sendet ihn mit.

**`revision.save` / `outline.save`** (geändert): `based_on_amend` zählt auch ohne `based_on` – der Stand bezieht sich dann
auf die aktuelle Fassung. Vorher wurde er in diesem Fall übersprungen.

**`ui.render {work?, …}`** (geändert): Fehlt `work`, zeichnet die Methode das Werk aus dem Cookie `baruch_work` (wie
die Seite `/`), nur ohne Cookie oder mit einem unbekannten Werk das Standardwerk. Vorher zeigte „Zurück“ auf die Adresse `/`
das Standardwerk ohne einen Hinweis.

**`work.typeset_adopt_rule {work?, confirm: true}`** (geändert): verlangt `confirm: true`, sonst -32602 „confirm: true
erwartet“. Die Methode schreibt die Werkvorlage um; das Formular verlangt dafür schon sein Kontrollkästchen. Kein Widget ruft sie auf
(`grep -rn typeset_adopt_rule crates/baruch-server/web/src` ohne Treffer). `work.typeset_reset` bleibt unverändert.

**`idea.map_group_accept {work, group, name?}`** (geändert): Maßgeblich ist die Gruppe, wie die Karte sie jetzt zeigt.
Ein gespeicherter Vorschlag, dessen Gruppe nun andere Einfälle hat, wird ohne eigenen `name` mit -32000
`„Veraltet: Die Gruppe gN hat inzwischen andere Einfälle als beim Vorschlag „…“; nichts geändert. …“` abgelehnt und
entfernt; kein Einfall ändert sich. Die Grenzen (Name, höchstens 20 Stichwörter je Einfall) werden geprüft, bevor der erste
Einfall geändert wird.

**`idea.map`** (geändert): `near.message` nennt eine Obergrenze, wenn die Karte mehr als 1500 Einfälle zeigt („… die Nähe
ist nur für die ersten 1500 (nach Nummer) berechnet.“).

**`cover.layout_set`, `asset.request_decide`** (Wortlaut unverändert): Ungültige Blöcke melden weiterhin -32602 mit
`„Umschlag-Block „…“ (art), Feld x: …“`; im Speicher ist es nun die eigene Fehlerart `StoreError::CoverBlock`.
`cover.layout` liefert Standardblöcke auf 0,01 mm gerundet.

**`commentary.lookup {ref, module?, work?}`** (geändert): Die Stelle wird in deutscher Versifikation gelesen und in die
Versifikation des Kommentars **abgebildet**; `entries[].ref` nennt die Stelle des Kommentars, bei abweichender Versifikation mit
ihrem Namen („Joel 2,28 (KJV)“). Vorher wurden die deutschen Zahlen unverändert im Kommentar nachgeschlagen.

**`source.read {…, q?}`** (geändert): `q` höchstens 500 Zeichen, sonst -32602 „q: höchstens 500 Zeichen (die Anfrage
dient nur dem Markieren der Fundstellen).“

**`finding.get`, `finding.list`** (ergänzt): Ein Befund trägt `stale_summary: true`, wenn er ein offener Sammelbefund ist,
dessen Quellen alle entschieden sind (sonst fehlt das Feld). `finding.matrix`: Ein offener Sammelbefund zählt nicht unter
`open` (weder in den Zellen noch in `open_findings`).

**`menu.badges`** (unverändert): `open_findings` zählt Sammelbefunde nicht mehr (Zählregel unten).

### MCP-Namen {#mcp-namen}

**`mcp_token.create`:** Seit Runde 23 prüft die Methode zusätzlich den Autor, der sich aus dem Namen ergibt (`@mcp:<handle>`, Regel `mcp::session::handle_name`: getrimmt, Kleinbuchstaben, `ä` → `ae`, `ö` → `oe`, `ü` → `ue`, `ß` → `ss`, Akzente entfernt, sonst alles außerhalb von `[a-z0-9._-]` als „-“, höchstens 32 Zeichen). Mit **-32000** abgelehnt, und nichts wird angelegt: ein Name, dessen Handle oder altes Handle (Regel bis Runde 22: Umlaut → „-“) dem Handle oder alten Handle eines **vorhandenen oder widerrufenen** Tokens gleicht, und ein Name, dessen Handle mit `lokal-` beginnt (reserviert für Clients ohne Token). Ein widerrufener Name ist also nicht wieder frei. Prüfung und Anlegen laufen in einer Transaktion: Von zwei gleichzeitigen Anfragen mit demselben Handle entsteht genau ein Token. Das Formular `POST /einstellungen/mcp/act` geht denselben Weg (422 mit derselben Meldung). Vorhandene Tokens behalten ihre Namen und Beiträge; kein Autor wird in der Datenbank umgeschrieben.

**`persona.chat`, Antwort:** dazu `done_proposals` (Liste der Befundnummern, für die eine Meldung „Erledigt: Befund n“ einen Statusvorschlag „erledigt“ erzeugte, ohne Duplikate), `done_notes` (Liste der übersprungenen Meldungen mit einem Grund) und `all_done` (`true`, wenn mindestens eine antwortende Persona sagt, dass alle ihre Befunde zum Abschnitt erledigt scheinen). Namen wie bei `persona.ask`; jedes Feld steht in der Antwort nur, wenn es nicht leer oder `true` ist. Im Chat gibt es solche Meldungen nur beim RFC (dort prüft jede Persona erneut).

**`persona.chat`, Erkennung der Nachricht:** Zeilen, die – nach Leerraum – mit „>“ beginnen, sind Zitate und zählen weder für Erwähnungen und Anreden noch für RFC („@alle“, „RFC“), Schreibauftrag oder Variantenauftrag. Bei einem Schreibauftrag werden Statusvorschläge „übernommen in Fassung n“ nur für `[B<id>]` aus der **Nachricht** angelegt, nicht mehr aus dem Text des Vorschlags.

**`post.move`:** Die Befunde des Beitrags (`finding.post_id`) wandern in derselben Transaktion mit in den Zielabschnitt, ein Commit. Ausnahme: Übernommene Befunde bleiben beim alten Abschnitt, weil ihr Status eine Fassung jenes Abschnitts nennt.

**`post.repeat` und `thread.repeat`:** Die gespeicherte Aufgabe (`PostRequest`, JSON in `post.request`) trägt seit Runde 23 zusätzlich `web` (bool), `asker` (Text) und `relay` (JSON `{persona, post, text, findings}` der ansprechenden Antwort). Die Wiederholung übernimmt alle drei: Ein Relay-Beitrag wird mit dem Relay-Rahmen erneut gefragt und bleibt ein Relay-Beitrag (`relay_from`); eine Aufgabe eines MCP-Clients behält ihre Herkunft und löst weiterhin keine Websuche aus. Zeilen ohne diese Felder (vor Runde 23) wiederholen wie bisher: Websuche erlaubt, Fragender der Mensch, kein Rahmen. Keine Migration; die Felder sind `#[serde(default)]`.

**`outline.requests {preview: true}`:** `preview_error` eines veralteten Antrags lautet weiterhin „Veraltet: <Grund>“; im Speicher ist es nun `StoreError::Stale` (vorher `Heading`), über RPC weiterhin -32000. „Nicht mehr ausführbar: …“ bleibt `Heading`.

### Geheimnisse {#geheimnisse}

- `model.connection_delete`: Parameter `{id, confirm: true}`. Ohne `confirm: true` (fehlt, `false`, Text) →
  -32602 „confirm: true erwartet“, und nichts wird gelöscht. Sonst wie bisher `{id, deleted: true, personas: [Handle]}`.
- `model.connection_delete_secret`: Parameter `{id, confirm: true}`; ohne `confirm: true` → -32602 „confirm: true
  erwartet“, der Schlüssel bleibt. Sonst `{id, secret: "fehlt"}`.
- Die Formulare `POST /einstellungen/modelle/act` (`action=delete`, `action=delete_secret`)
  verlangen weiterhin das Pflicht-Kontrollkästchen `confirm=1` und rufen dann dieselben Funktionen auf (`models::delete_connection`,
  `models::delete_secret`) – ohne JavaScript nutzbar. Kein Widget ruft die beiden Methoden auf
  (`grep -rn "connection_delete" crates/baruch-server/web/src crates/baruch-server/static crates/baruch-server/templates`
  findet nichts).
- `model.context`: Ist die Verbindung der Persona extern und die Persona nicht freigegeben, geht **keine** Anfrage
  hinaus; Antwort `{tokens: <Einstellung>, reported: null, warning: null, …}`.
- `model.list`: Der Zwischenspeicher (60 s) gilt je (Art, Adresse). Zwei Verbindungen verschiedener Art auf derselben
  Adresse teilen weder Liste noch Kontextlänge.
- `model.connection_check`, `connection.check`: Antwortet die Gegenseite mit einer Weiterleitung (3xx), ist das
  `ok: false` mit „… leitet weiter (HTTP 30x); Baruch folgt Weiterleitungen nicht – bitte die endgültige Adresse
  eintragen.“ oder „HTTP 30x von …: Weiterleitung. Baruch folgt ihr nicht …“. Das Ziel (`Location`) wird nicht genannt.
- `work.create`: `module` höchstens 100 Zeichen in einer Zeile, sonst -32602 „module: Modulname mit höchstens 100
  Zeichen in einer Zeile erwartet“. Werk und SWORD-Bibel werden in einer Transaktion angelegt.
- `work.exchange_import` (und Vorschau): -32602 „Austausch: Das Glossar hätte N Begriffe; höchstens 5000 je Werk
  (Vorschläge mitgezählt). Nichts eingelesen.“ oder „… Register <kennung>: Titel hat N Zeichen (höchstens 80) …“.

### Integration {#integration-4}

- **`POST /act`** (Formulare ohne JavaScript) reicht `based_on_amend` an `revision.update` weiter: Das Formular
  „Fassung ändern“ wird auch ohne JavaScript mit einem veralteten Stand abgelehnt („Veraltet: …“).
- **`POST /mcp`:** eigene Größengrenze `MCP_BODY_LIMIT` = 12 MiB (ein Bild von 8 MiB als Base64 mit seinem Rahmen), darüber
  413; `/rpc` behält `works::RPC_BODY_LIMIT` (Code-Review ki-2).
- Re-Exporte in `baruch_store`: `MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN` (200), `MAX_OPEN_INDEX_PROPOSALS_PER_WORK` (2000),
  `MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN` (50).

## Runde 32 – Word und Beenden {#round-32--word-and-quitting}

### docx_import.preview, docx_import.get, docx_import.apply, docx_import.discard (Runde 32 – Word) {#docx_importpreview-docx_importget-docx_importapply-docx_importdiscard-round-32--word}

Global (ohne Werk; Weiche in `rpc::call` durch `docx_import::handles`, Modul `docx_import`). Kein MCP-Gegenstück: Ein
Import legt aktuelle Fassungen im Namen des Menschen an.

- **`docx_import.preview {file_name, data_base64, title?}`** – `data_base64` = Datei als Base64 (Standard, mit `=`);
  `file_name` endet auf `.docx`. Liest die Datei und legt nur einen **Entwurf** an
  (`<data>/docx-drafts/<draft>/` mit `draft.json` und `source.docx`; kein Werk). Antwort:
  `{draft, file_name, title, sections: [{level 0..3, numbered, title, chars, sample}], editors: [{name, handle,
  insertions, deletions, sections}], comments, footnotes, images, warnings: [{code, message}]}`.
  `title` = Titel, unter dem sie übernommen würde (eigenes `title`, sonst Titel der Datei, sonst Dateiname ohne die Endung);
  `level` 0 = Teil; `sample` = Anfang des Abschnitts als Typst (höchstens 160 Zeichen); `editors[].sections` = Zahl
  der Abschnitte, für die ein Vorschlag angelegt würde; `handle` = `@word:<name>`.
  Fehler: kein `.docx`-Name, kaputtes Base64, größer als 64 MiB, Fehler der Datei (kein ZIP, altes `.doc` oder
  verschlüsselt, unsicherer Pfad oder Beziehungsziel, DTD, Grenze überschritten, kaputtes XML, kein Text) → -32602 mit
  dem Grund; nichts wird angelegt. Über `POST /rpc` liegt die ganze Anfrage zusätzlich innerhalb von `RPC_BODY_LIMIT`
  (sonst 413 vor dem Aufruf, also eine Datei bis etwa 24 MiB – größere über das Formular
  `POST /werke/import/word/vorschau`, bis 64 MiB).
- **`docx_import.get {draft}`** → dasselbe Objekt wie `preview`. Unbekannt, verworfen, übernommen oder älter als 24 h
  → -32602.
- **`docx_import.apply {draft, title?, changes?: bool = true, comments?: bool = true}`** – legt das Werk an (Fassung 1
  je Abschnitt = Zustand vor allen Änderungen, aktuell), mit `changes` je Bearbeiter und geändertem Abschnitt ein
  offener Vorschlag (Persona `@word:<name>`, Modell `word`) und je geänderter Überschrift ein Beitrag, mit `comments`
  je Kommentar ein Beitrag. Antwort: `{work, slug, sections, proposals, posts, warnings: [{code, message}]}`. Der
  Entwurf wird danach gelöscht. Fehler: unbekannter Entwurf → -32602; vergebener Titel → -32000 (wie
  `source_import.apply`, der Entwurf bleibt); derselbe Entwurf wird gerade übernommen → -32000. Konnte ein
  einzelner Vorschlag oder Beitrag nicht angelegt werden, bleibt das Werk, und `warnings` nennt ihn
  (`docx_vorschlag_abgelehnt`, `docx_beitrag_abgelehnt`, `docx_gliederung_abweichend`).
- **`docx_import.discard {draft}`** → `{discarded: true}` (auch wenn der Entwurf schon weg ist). `draft` ist kein
  32-stelliger Hex-Name → -32602.

Höchstens 20 Entwürfe, jeder höchstens 24 Stunden alt; ältere werden beim nächsten `preview` verworfen.
`work.import` und `POST /werke/import` lehnen `.docx` mit einem Hinweis auf `/werke/import/word` ab (S1).

Seiten (ohne JavaScript): `GET /werke/import/word` (Formular; mit `?draft=<id>` die Vorschau, unbekannter Entwurf 422),
`POST /werke/import/word/vorschau` (multipart `file`, `title` → 303 `/werke/import/word?draft=<id>`; Fehler 422, zu
groß 413, jeweils mit dem Formular), `POST /werke/import/word/uebernehmen` (`draft`, `title`, Kontrollkästchen `changes`,
`comments` → 303 `/?work=<id>`; sind beim Übernehmen neue Hinweise entstanden, 200 mit einem Bericht und einem Link zum Werk; Fehler
422 mit der Vorschau), `POST /werke/import/word/verwerfen` (`draft` → 303 `/werke/import/word`). Alle POSTs hinter
`guard::same_origin` (fremder Origin 403).

### Runde 32 – Word: `work.docx_export` {#round-32--word-workdocx_export}

Nur lesend; kein MCP-Werkzeug.

`work.docx_export {work?: number|string, proposals?: bool = false}`

- `work`: Werk-ID (Zahl), Ordnername des Werks (Text) oder – ohne sie – das Standardwerk.
- `proposals`: `true` legt je Abschnitt den jüngsten offenen Vorschlag als Änderungsverfolgung über die aktuelle
  Fassung.

Antwort:

```json
{
  "file_name": "der-steg-am-weiher.docx",
  "data_base64": "UEsDB…",
  "bytes": 5321,
  "proposals": 1,
  "warnings": [
    { "code": "docx_typst_insel", "message": "Abschnitt „Der Abend“: Typst-Insel „#let rahmen(inhalt) = …“: Word kann sie nicht abbilden – der Quelltext steht sichtbar und wörtlich in der Datei." },
    { "code": "docx_weitere_vorschlaege", "message": "Abschnitt „Der Weiher“: 1 weiterer offener Vorschlag steht nicht in der Datei – als Änderungsverfolgung steht nur der jüngste (Fassung 3 von @kritiker)." }
  ]
}
```

- `proposals`: Zahl der Abschnitte, deren Vorschlag als Änderungsverfolgung in der Datei steht (ohne `proposals: true`
  immer 0; ein Vorschlag, der der aktuellen Fassung gleicht, zählt nicht).
- `warnings`, feste Codes: `docx_typst_insel` (Insel mit sichtbarem Ersatz; ab der sechsten nur noch als Zahl),
  `docx_bild_fehlt` (Bildpfad ohne einbettbares Bild: im Werk fehlend oder SVG/WebP), `docx_tabelle_vereinfacht`
  (Formatierung einer Tabelle weggelassen), `docx_weitere_vorschlaege` (mehr als ein offener Vorschlag im Abschnitt).
- Fehler: unbekanntes Werk oder unbekannter Parameter → `-32602`; Schreibfehler → `-32603`.
- Derselbe Zustand des Werks ergibt dieselben Bytes (der Zeitpunkt in der Datei ist die letzte Änderung des Werks,
  nicht der Zeitpunkt des Exports).

Seiten dazu (keine RPC): `GET /werke/{id}/word` (Erklärung und Formular), `GET /werke/{id}/word.docx[?vorschlaege=1]`
(Datei, `Content-Type: application/vnd.openxmlformats-officedocument.wordprocessingml.document`,
`Content-Disposition: attachment; filename="<slug>.docx"`; unbekanntes Werk 404, anderer Wert für `vorschlaege` 400,
Anfrage von einer fremden Seite 403).

### Beenden {#quitting}

Keine Methode. Das Beenden ist bewusst weder eine RPC-Methode noch ein MCP-Werkzeug (`rpc::call` kennt die Gegenseite nicht;
Test `there_is_no_rpc_method_and_no_mcp_tool_to_quit`).

Baruch zu beenden gibt es weder über RPC noch über MCP: Es ist nur über die Seite `/beenden` an der eigenen Maschine
oder durch ein Signal an den Prozess möglich. Routen: `GET /beenden` (Rückfrage; nicht von der eigenen Maschine 403) und
`POST /beenden` (hinter `guard::same_origin`, nur von der eigenen Maschine, Antwort 200 mit der Abschlussseite, 500 ms
später fährt Baruch herunter).

## Runde 34 – Mindmap {#round-34--mind-map}

Die Einfallskarte ist eine Mindmap geworden: Einfälle tragen einen Titel, einen Elternknoten und eine Reihenfolge (Migration 0052),
Einfälle ohne Platz liegen in der **Wolke** neben der Mindmap. Die Abschnitte „Ideas“ und „Idea map (Round
25)“ weiter oben gelten weiter; hier steht, was hinzukam. Abweichungen vom Vertrag der Runde: Die Liste der
mitgelöschten Einfälle heißt `deleted_ids` (`deleted` bleibt `true`), Ablehnungen tragen -32000 (nicht -32001, das
seit Runde 19c `RAG_UNAVAILABLE` ist).

### Methoden {#methods-1}

Über RPC ist die Herkunft immer der Mensch. Jede Antwort mit `idea` trägt zusätzlich `title`, `parent` (ID oder
`null`), `position`, `main_topic` und `cloud` (abgeleitet: liegt in der Wolke).

| Methode | Parameter | Antwort |
|---|---|---|
| `idea.create` | wie bisher, dazu `title?` (≤ 200 Zeichen), `parent?` (Einfall-ID, gleiches Werk; setzt `work` daraus, wenn es fehlt), `before?` (ID eines Geschwisters; ohne = am Ende), `cloud?` (`false` = ohne `parent` und `key` ein freies Hauptthema; Standard: in die Wolke). `words` darf fehlen, wenn `title` nicht leer ist; ohne beides -32602. | `{idea, commit?}` |
| `idea.update` | wie bisher, dazu `title?` (Umbenennen; leer nur, solange ein Stichwort bleibt) | `{idea, changed, commit?}` |
| `idea.move` | `{id, parent?: id\|null, key?: key\|null, before?: id\|null, cloud?: bool}` – fehlende Felder bleiben. `parent: null` löst vom Elternknoten; bleibt auch kein Bezug, wird der Einfall ein freies Hauptthema am Werk. `key` setzt oder löscht den Bezug. `before` ordnet vor einem Geschwister ein, `null` ans Ende. `cloud: true` legt in die Wolke (löst Elternknoten und Bezug, Kindknoten bleiben am Einfall); `cloud: false` macht einen Einfall ohne Elternknoten und Bezug zu einem Hauptthema. | `{idea, changed, commit?}`; unverändert → `changed: false` ohne Commit |
| `idea.delete` | `{id, confirm: true}` | `{id, deleted: true, deleted_ids: [ids], idea, commit?}` – zuerst der Einfall, dann die mitgelöschten Kindknoten |
| `idea.restore` | `{id}` | `{idea, restored: [ids], detached?: true, commit?}` |
| `idea.tree` | `{work, status?, word?, origin?, q?, near?, near_limit?, near_threshold?, findings?}` (Filter wie `idea.map`) | `{map: MindMap, cloud: Cloud}` |
| `idea.map` | unverändert | wie bisher; je Einfall zusätzlich `title`, `parent`, `position`, `main_topic`, `cloud`; `label` ist der Titel, sonst die Stichwörter |

`idea.move` lehnt mit -32000 ab (nichts geändert, kein Commit): ein Elternknoten in einem anderen Werk, der Einfall
selbst oder einer seiner eigenen Nachkommen als Elternknoten (Zyklus), Tiefe über 32, ein gelöschter Einfall oder Elternknoten,
`before` kein Geschwister, `cloud: true` zusammen mit `parent` oder `key`. Unbekannter Einfall oder falsche Typen:
-32602. Jede Änderung ist ein Commit „Einfall <id>: umgehängt“ (`Baruch-Action: idea-move`) mit dem Ort vorher und
nachher im Rumpf. Kapitel und Teile lassen sich nicht durch `idea.*` ändern – ihre Kennung `n:<key>` ist keine
Einfall-ID; dafür bleiben `outline.*` und die Antragsregel.

`idea.map_pos_set` und `idea.map_pos_reset` bleiben: gespeichert, ohne Wirkung in der Mindmap.

Umkehrung für „Rückgängig“ im Widget: Anlegen ↔ `idea.delete`, Löschen ↔ `idea.restore` (bringt die Kindknoten mit),
Umbenennen ↔ `idea.update` mit dem alten `title`, Verschieben ↔ `idea.move` mit `parent`/`key`/`before` oder `cloud`
aus dem Einfall vor der Aktion (`cloud: true`, wenn er in der Wolke lag; `parent: null, key: null, cloud: false`,
wenn er ein Hauptthema war).

Formulare ohne JavaScript: `POST /einfaelle/act` nimmt zusätzlich `method=idea.move` mit `parent` („in die Mindmap
unter Einfall Nr. …“), `cloud=1` („in die Wolke“) oder `parent=none&key=none&cloud=0` („als Hauptthema ans Werk“);
die Seite `/einfaelle/{id}` zeigt den Ort und diese drei Formulare, `idea.create`/`idea.update` haben ein Feld
`title`.

### Routen der Seite und des Bildes {#routes-of-the-page-and-the-image}

| Route | Inhalt |
| --- | --- |
| `GET /werke/{id}/karte` | Seite „Mindmap“ (`idea_map/bild/mod.rs`, Vorlage `karte.html`) |
| `GET /werke/{id}/karte/bild` | nur das SVG (`image/svg+xml`, `Cache-Control: no-store`), gleiche Query; hier erhält das Fragment `xmlns` |

Query: Filter wie bisher (`status`, `word`, `origin`, `q`, `near`, `near_limit`, `near_threshold`, `befunde`),
dazu `zu=<Kennungen, Komma>` (eingeklappte Knoten) und `wahl=<Kennung>` (gewählter Knoten, im Bild
`aria-current="true"`). Kennungen sind `w`, `n:<key>`, `i:<id>`; alles andere entfällt (`bild::node_id`,
höchstens 2000). Beide stehen in `bild::Extra` (`closed`, `wahl`), das `parse_filter` füllt; `filter_query` trägt
sie nicht (Ausgaben klappen nie ein).

Die Seite baut zwei Ansichten: Baum und Wolke (`tree::scene`) für das Bild und – für den Abschnitt „Gruppen“ – die flache
Karte (`idea_map::build`). Das ist ein zweiter Durchlauf mit demselben Filter; scheitert er, fehlen nur die Gruppen.

### Routen für Ausgabe, Einlesen und Formulare (keine RPC) {#routes-for-output-reading-in-and-forms-no-rpc}

| Route | Zweck |
|---|---|
| `GET /werke/{id}/karte.svg` \| `.pdf` \| `.md` \| `.mm` \| `.xmind` | Ausgaben; Query wie die Seite (`status`, `word`, `origin`, `q`, `near`, `befunde`), `.svg`/`.pdf` zusätzlich `zu=<Kennungen>` |
| `GET /werke/{id}/karte/gliederung` | Gliederungsansicht; dazu `wahl=<Kennung>` (gewählter Knoten), `zu=<Kennungen>`, nach Formularen `ok`, `undo`, `wieder` |
| `POST /werke/{id}/karte/act` | Formulare (urlencoded), siehe unten |
| `GET /werke/{id}/karte/einlesen` | Formular; `unter=i:<id>` wählt den Zielknoten vor |
| `POST /werke/{id}/karte/einlesen` | multipart `file`, optional `parent`; zeigt die Vorschau, legt nichts an (413 über 10 MiB, 422 nicht lesbar, 409 Zielknoten aus einem anderen Werk) |
| `POST /werke/{id}/karte/einlesen/anlegen` | urlencoded `tree` (JSON aus der Vorschau), `parent`; legt durch `tree::import` an, 303 auf `/werke/{id}/karte?ok=eingelesen` |

`POST /werke/{id}/karte/act`: Feld `method` ∈ `idea.create`, `idea.update`, `idea.move`, `idea.delete`,
`idea.restore`, `idea.link`, `idea.unlink`, `idea.link_restore`, `idea.map_pos_reset`, `idea.map_groups_name`,
`idea.map_group_accept`, `idea.map_group_dismiss`. Dazu die Felder der Aktion:

- `idea.create`: `title`, `words`, `note`, `parent`, `key`, `before`, `ort` (`ort=werk` → freies Hauptthema; ohne
  `parent`, `key` und `ort` → Wolke).
- `idea.update`: `id`, optional `title`, `key`, `words`, `note` – nur Felder, die das Formular sendet, ändern sich.
- `idea.move`: `id`, entweder `ziel` ∈ `wolke` (RPC `cloud: true`) \| `w` (Hauptthema am Werk) \| `i:<Einfall>` \|
  `n:<Kapitel-key>` oder die Felder der RPC
  `parent`, `key`, `before` (vorhanden und leer = `null`).
- `idea.delete`, `idea.restore`: `id`. `idea.link`: `from`, `to`, `kind`, `note`. `idea.unlink`,
  `idea.link_restore`: `id` (Verbindung).
- `back` (Query-Teil des Filters), `seite` ∈ `karte` \| `gliederung` (Ziel der Umleitung, Standard `karte`),
  `wahl` (gewählter Knoten, nur mit `seite=gliederung`).

Antwort: 303 auf `/werke/{id}/karte[/gliederung]?<back>&ok=<wort>` (`angelegt`, `umbenannt`, `bezug`, `umgehaengt`,
`in-wolke`, `ort-unveraendert`, `geloescht` mit `wieder=<id>`, `einfall-wieder`, `verbunden`, `unveraendert`,
`geloest` mit `undo=<id>`, `wiederhergestellt`, …). Fehler als Seite mit einer Meldung: 400 unbekannte Aktion, 409 abgelehnt
(auch: ein genannter Einfall gehört nicht zum Werk der Adresse – vorher 422), 422 ungültige Eingabe, 404 unbekanntes
Werk, 403 fremder Origin.

## Runde 35 – Konten und Rollen {#round-35--accounts-and-roles}

Aufbau und Berechtigungsprüfung: `docs/Architektur.md` („Round 35 – Accounts and roles“, dort auch die Tabellen
`LEKTOR_ROUTES` und `LEKTOR_RPC`). **Ohne Konten** ändert sich nichts an `/rpc`, `/act` und allen Formularen.

**Neue Fehlercodes:** `-32002` (`UNAUTHENTICATED`: Es gibt Konten, und die Anfrage trägt keine gültige Sitzung; HTTP
401 bei `/rpc`, schon in der Schicht `auth::gate`) und `-32003` (`FORBIDDEN`: Die Rolle darf die Methode oder das Werk nicht nutzen;
HTTP 200 bei `/rpc`, 403 bei `/lektorat/act`). `rpc::call` prüft **jede** Methode durch `roles::check_rpc`, bevor
sie ausgeführt wird – für `/rpc` (einzeln und Batch), `/act` und `/lektorat/act`; für Lektor-Konten ist jede Methode
außerhalb von `LEKTOR_RPC` gesperrt, auch ein unbekannter Name („Das darf nur der Autor: <methode>“).

### `account.*` {#account}

Anmelden, Abmelden und Einrichten gibt es nur als Formular (Cookie). Passwörter, Hashes und Token stehen nie in
einer Antwort. Fehler: `-32602` falsche Parameter (die Meldung zitiert keinen Wert), `-32000` aus fachlichen Gründen abgelehnt,
`-32002` nicht angemeldet (HTTP 401, schon in der Schicht), `-32003` nur der Autor.

| Methode | Parameter | Antwort |
|---|---|---|
| `account.me` | `{}` | `{accounts_exist, signed_in, local_open, id?, name?, display?, kind?, handle?}` – die Felder des Kontos nur mit einer Sitzung (`kind`: `author` \| `lektor`). Jeder, auch ohne Konten. |
| `account.list` | `{}` | `{accounts: [{id, name, display, kind, handle, active, created, last_login}]}` nach ID. Nur volle Rechte. |
| `account.create` | `{name, display?, password}` | das Konto (wie in `account.list`), immer `kind: "lektor"`. Nur volle Rechte. Ohne jedes Konto `-32000` mit einem Hinweis auf `/einrichten`; Name ungültig oder vergeben, Passwort zu kurz oder zu lang → `-32000`. |
| `account.set_active` | `{id, active}` | das Konto. Nur volle Rechte. Letztes aktives Autorenkonto oder unbekanntes Konto → `-32000`. Das Sperren beendet alle Sitzungen des Kontos. |
| `account.set_local_open` | `{open}` | `{local_open}`. Nur volle Rechte. |
| `account.set_password` | `{current?, password, id?}` | `{id, changed: true}`. Eigenes Konto (ohne `id` oder mit der eigenen): `current` ist erforderlich (`-32602` ohne, `-32000` wenn falsch); die Sitzung der Anfrage bleibt, alle anderen enden. Ein anderes Konto (`id`): nur volle Rechte, alle Sitzungen des Kontos enden. |
| `account.logout_all` | `{}` | `{sessions}` – beendet alle Sitzungen des eigenen Kontos, auch die der Anfrage. Ohne Sitzung `-32000`. |

`roles::check_rpc` entscheidet vor der Verteilung, was ein Lektor aufrufen darf (`account.me`,
`account.set_password`, `account.logout_all`); die Methoden „nur volle Rechte“ prüfen hier `who.full_rights()` ein
zweites Mal.

### `revision.propose`, `finding.add`, `member.*` {#revisionpropose-findingadd-member}

| Methode | Parameter | Antwort / Fehler |

|---|---|---|
| `revision.propose` | `{work?, key \| node, body, note?}` | `{work, node, key, rev, status: "proposal", author}`; nur Lektor (andere: -32000 mit einem Hinweis auf `revision.save`). Vorschlag unter `@lektorat:<slug>`, nie die aktuelle Fassung; Markdown-Überschriften wie bei `revision.save`. Leerer Text, über 1 MiB, `note` über 2000 Zeichen → -32602; Text gleich der aktuellen Fassung → -32000; unbekannter Abschnitt → -32602 oder -32000 (Schlüssel weg). Kein Commit auf dem Hauptzweig, Nebenzweig `refs/revisions/<key>/<rev>`. |
| `finding.add` | `{work?, key?, kind: "hint" \| "critical" \| "praise", body, quote?}` | `{finding, commit? \| mirror?}` (`finding` wie in `finding.get`, `persona` = Handle, `status: "open"`, ohne Beitrag); ohne `key` „Anmerkung zum Buch“ (Befund ohne Abschnitt). Nur Lektor (andere: -32000). Leerer Text, unbekannte Art, Abschnitt nicht im Werk → -32602. Genau ein Commit im Spiegel. |
| `member.list` | `{work}` | `{work, members: [{account, name, display, handle, role, granted, active}], candidates: [{account, name, display, handle}]}`; `candidates` = aktive Lektor-Konten ohne Zugang. Nur volle Rechte (Lektor: -32003). `work` ist erforderlich (kein Standardwerk). |
| `member.grant` | `{work, account, role?: "lektor"}` | `{work, account, role, granted}`; idempotent. Unbekanntes Werk oder Konto, andere Rolle → -32602; Autorenkonto → -32000. Nur volle Rechte. Kein Commit. |
| `member.revoke` | `{work, account}` | `{work, account, revoked}` (`false`, wenn es keinen Zugang gab). Nur volle Rechte. Kein Commit. |

Geändert:

- `post.add`: Autor des Beitrags ist `roles::author_name` – bei Lektor-Konten das Handle, sonst wie bisher „Autor“.
- `post.edit`, `post.delete`: Lektor-Konten nur ihre eigenen Beiträge (Autor des Beitrags = Handle); fremde und die des Autors
  → -32000 mit der Meldung „Nur eigene Beiträge lassen sich bearbeiten: …“. Der Autor verwaltet weiterhin jeden Beitrag.
- `post.page`, Seiten `/beitraege` und `/beitrag`: Bei Lektor-Konten bedeutet `own` „unter dem eigenen Handle“.
- `work.list`: bei Lektor-Konten nur die mit ihnen geteilten Werke.
- `ui.render`: bei Lektor-Konten ohne Literatur.
- Jede Methode außerhalb von `LEKTOR_RPC` (auch unbekannte Namen): bei Lektor-Konten -32003.

### Routen (Formulare ohne JavaScript, alle hinter `guard::same_origin`) {#routes-forms-without-javascript-all-behind-guardsame_origin}

| Route | Felder | Antwort |
|---|---|---|
| `POST /anmelden` | `name`, `password`, `next` | 303 auf `konten_ui::safe_next(next)` mit einem Cookie; sonst die Anmeldeseite mit 401, bei Sperre 429 mit `Retry-After`. Ohne Konten 303 `/` ohne Cookie. |
| `POST /abmelden` | – | 303 `/anmelden?ok=abgemeldet`, Cookie gelöscht. Ohne Konten 303 `/`. |
| `POST /einrichten` | `name`, `display`, `password`, `password2` | Nur ohne Konten und von der eigenen Maschine: Autorenkonto, Sitzung, 303 `/einstellungen/konten?ok=konto_angelegt`. Konto vorhanden oder nicht die eigene Maschine → Seite mit 403; ungültige Eingabe → 422. |
| `POST /konto/passwort` | `current`, `password`, `password2` | 303 `/konto?ok=passwort_geaendert` oder `?fehler=passwort_falsch\|passwort_kurz\|passwort_lang\|passwort_ungleich\|unbekanntes_konto` (ohne eigenes Konto) |
| `POST /einstellungen/konten/act` | `action`, `id`, … | 303 `/einstellungen/konten?ok=…\|fehler=…`; nur volle Rechte (sonst 403 mit `x-baruch-abgelehnt: rolle`); unbekannte Aktion 422 |

Aktionen von `/einstellungen/konten/act`:

| `action` | Felder | ok | Fehler |
|---|---|---|---|
| `create` | `name`, `display`, `password`, `password2` – immer `kind = lektor` | `konto_angelegt` | `name_ungueltig`, `name_vergeben`, `passwort_kurz`, `passwort_lang`, `passwort_ungleich`; ohne jedes Konto 303 `/einrichten` |
| `deactivate` | `id` | `konto_gesperrt` | `letzter_autor`, `unbekanntes_konto` |
| `activate` | `id` | `konto_entsperrt` | `unbekanntes_konto` |
| `reset_password` | `id`, `password`, `password2` | `passwort_geaendert` | wie `create` (Passwort), `unbekanntes_konto` |
| `local_open` | `open` = `0` \| `1` | `lokal_offen` / `lokal_anmelden` | – |

Namen: 1 bis 40 Zeichen ohne Steuerzeichen; der Slug (Regel wie `docx_import::word_author`) darf nicht `unbekannt` sein
– ein Name braucht daher mindestens einen Buchstaben a–z (auch `ä`, `ö`, `ü`, `ß`) oder eine Ziffer. Name
(unabhängig von Groß-/Kleinschreibung und Leerraum) und Slug sind eindeutig, auch gegenüber gesperrten Konten.
Anzeigename höchstens 80 Zeichen.

- `POST /lektorat/act` (hinter `guard::same_origin`): Felder `method`, `work`, `key`, `node`, `id`, `body`, `note`,
  `kind`, `quote`, `confirm` (`1` | `on` | `true`), `back`. Ruft `rpc::call` auf – dieselbe Berechtigungsprüfung wie `/rpc`.
  Angenommen werden nur Methoden aus `LEKTOR_RPC` (auch vom Autor: sonst 403). Fehlt `work`, gilt bei Lektor-Konten das
  kleinste mit ihnen geteilte Werk. Danach 303: nach `revision.propose` auf `/?work=W&key=K&rev=N`, sonst zum Abschnitt
  `/?work=W&key=K`; mit `back=befunde` (und nach `finding.add` ohne `key`) auf `/werke/W/befunde`; mit `back=post`
  auf `/beitrag?work=W&id=N` (nach `post.delete` zum Abschnitt). Fehler: 403 „Abgelehnt: …“ mit
  `x-baruch-abgelehnt: rolle` (`-32003`), 401 (`-32002`), 500 (intern), sonst 422 „Fehler: …“.
- `POST /werke/{id}/mitwirkende/act` (nur volle Rechte): `action` = `grant` | `revoke`, `account`. 303 auf
  `/werke/{id}/mitwirkende?ok=freigegeben` oder `?ok=entzogen`; `?fehler=unbekanntes_konto` (Konto fehlt, unbekannt
  oder keine Zahl) oder `?fehler=kein_lektor` (Zugang für ein Autorenkonto). Unbekannte Aktion 400, unbekanntes Werk
  422.

Seiten (`konten_ui::pages`):

| Route | Verhalten |
|---|---|
| `GET /anmelden[?next=…][&ok=abgemeldet]` | ohne Konten 303 `/`; sonst das Formular `POST /anmelden` (`name`, `password`, `next`), Link „Weiter“ auf `safe_next(next)`; mit einer Sitzung „Angemeldet als …“ |
| `GET /einrichten` | mit Konten 303 `/einstellungen/konten`; nicht von der eigenen Maschine 403 mit einer Erklärung (ohne Formular); sonst das Formular `POST /einrichten` (`name`, `display`, `password`, `password2`) |
| `GET /konto[?ok=…][&fehler=…]` | Name, Rolle, Herkunft, Werke (`roles::visible_works`), `POST /konto/passwort` (`current`, `password`, `password2`), `POST /abmelden`; ohne Konten eine Erklärung; `LocalAuthor` ein Hinweis |
| `GET /werke/{id}/mitwirkende[?ok=…][&fehler=…]` | nur volle Rechte (sonst 403), unbekanntes Werk 404; Daten aus `rpc::call("member.list", {work})`; Formulare `POST /werke/{id}/mitwirkende/act` (`action=grant|revoke`, `account`) |
| `GET /einstellungen/konten[?ok=…][&fehler=…]` | Liste aus `Store::account_list`, Schalter aus `account.me` (`local_open`); Formulare `POST /einstellungen/konten/act` mit `action=create` (`name`, `display`, `password`, `password2`), `deactivate`/`activate` (`id`), `reset_password` (`id`, `password`, `password2`), `local_open` (`open=0|1`); ohne Konten eine Erklärung und „Einrichten“ |

`?ok=`/`?fehler=` zeigen nur den Satz aus `MESSAGES` (ein Erfolgscode zählt nicht als Fehler und umgekehrt; unbekannte Codes
zeigen nichts). Alle Kontenseiten antworten mit `Cache-Control: no-store`.

## Runde 36 – Reste {#round-36--leftovers}

Keine neue Methode, keine neue Route. Geändert:

- **Rollen (`roles::LEKTOR_RPC`, `LEKTOR_ROUTES`):** `wysiwyg.parse`, `wysiwyg.serialize`, `wysiwyg.diff`,
  `wysiwyg.style` und `ribbon.formats` stehen für Lektor-Konten wieder „ohne Werk“ (Runde 35: Bindung an das Werk durch den
  Parameter). Fehlt `work`, gilt wie für alle `AppState::default_work` – für einen Lektor das kleinste
  mit ihm geteilte Werk, ohne Zugang ein Fehler (`-32602`, „unbekanntes Werk 0“; vorher `-32003` „Noch kein Werk freigegeben.“);
  ein genanntes `work` wird weiterhin geprüft (`-32003`). `GET /einstellungen/darstellung` braucht für Lektor-Konten kein
  `work` mehr (200 statt 303 auf `?work=`; ohne Zugang erreichbar statt 303 auf `/werke`); das Cookie
  `baruch_work` zählt für einen Lektor nur, wenn das Werk mit ihm geteilt ist.
- **`persona.propose`:** Antwort mit `basis` (siehe die Prüfgrundlage unter „Solution proposals and variants“); `scope: "chapter"` beruht für den eigenen
  Teil auf der gezeigten oder gewählten Fassung.
- **`persona.chat`:** eigene offene Befunde nur bei einem Prüfauftrag oder RFC.
- **`ui.render`:** Mit Konten tragen die HTML-Stücke den Anzeigenamen der Herkunft („Erika Beispiel
  (@lektorat:erika)“), nur als Text der Herkunftselemente; Attribute (`data-post-author`) bleiben das Handle.
- **`outline.delete` und `outline.request_decide` (bestätigtes Löschen):** unveränderte Antwort; der Spiegel-Commit
  nennt zusätzlich die mitgelöschten Fassungen und Beiträge (Trailer `Baruch-Lost-Revisions`, `Baruch-Lost-Posts`).
- **`ribbon.formats`:** Das Format `quote` trägt den Text `#quote(block: true)[]\n` ohne die folgende Zeile „– “.
- **`idea_map`-Einlesen (`POST /werke/{id}/karte/einlesen/anlegen`):** Knoten des Baums im Feld `tree` dürfen
  `cloud: true` tragen (oberste Ebene, ohne `parent`): Sie kommen in die Wolke. Fehlt das Feld, gilt `false`.
- **`GET /einstellungen/daten?ok=sicherung`:** zeigt die Meldung nur einmal nach einer über das Formular erzeugten Sicherung
  (ein Hinweis im Prozess); `settings.backup` (RPC) antwortet wie bisher direkt.
- **`GET /static/js/vw.js`:** neu ausgeliefertes Skript unter der bestehenden Route `/static/js/{name}` (klassisch,
  kein Modul; korrigiert das Cookie `baruch_vw` und die Attribute an `<html>` vor dem ersten Zeichnen).

## Werksprache {#work-language}

Runde 38 (`src/work_language.rs`, Speicher `work.language`, Migration `0055_work_language.sql`): Jedes Werk hat eine Werk-
sprache, `"de"` (Standard, jedes Werk von vor Runde 38) oder `"en"`. Personas antworten in ihr und schreiben ihre Marker
in ihr; sie ändert nicht die Sprache der Oberfläche und ist unabhängig von der Silbentrennungssprache von
den Satz-Einstellungen (`<satz sprache>`, in Runde 39 gekoppelt). Die Sprache ist kein Buchtext: Sie zu setzen legt
keinen Eintrag im Spiegel an (wie `work.timing_set`); sie reist mit dem Werk im Austauschformat (Unterversion 10,
Attribut `sprache` an `<angaben>`).

| Methode | Parameter | Antwort |
|---|---|---|
| `work.language` | `{work?}` | `{work, language: "de" \| "en"}` – nur lesend. `work` fehlt → Standardwerk, unbekannt → -32602 |
| `work.language_set` | `{work, language: "de" \| "en"}` | `{work, language, changed}`; `changed: false`, wenn das Werk diese Sprache schon hatte (nichts geschrieben). `work` ist erforderlich. Jeder andere Wert (auch `"EN"`, `" en"`, eine Zahl) → -32602 mit `language: allowed values are "de" and "en"`, nichts geändert. **Nur der Mensch mit vollen Rechten:** nicht durch `/act`, nicht durch MCP, nicht in `LEKTOR_RPC` |

`work.get` trägt `language` auf der obersten Ebene neben `work`, `lector` und `cover`; `work.meta` trägt sie als
`work.language`, `work.list` als `language` jedes Werks.

Formular (ohne JavaScript): `POST /einstellungen/werk/sprache` mit `work` und `language`, hinter
`guard::same_origin`, nicht in `LEKTOR_ROUTES`. Antwort 303 auf `/einstellungen/werk?work=W&ok=sprache#sprache` oder bei
einem ungültigen Wert auf `…&sprachfehler=1#sprache` (nichts geändert). Ein unbekanntes oder fehlendes Werk
(auch wenn sich kein Standardwerk bestimmen lässt) → 303 auf `/einstellungen/werk?sprachfehler=werk#sprache`; der Block
`id="sprache"` zeigt dann „Die Werksprache ließ sich nicht speichern: dieses Werk gibt es nicht; nichts geändert.“,
und nichts ändert sich. Der Block `id="sprache"` („Werksprache“) steht auf der Seite „Einstellungen → Werk“ direkt vor
„Predigtzeit“.

MCP: Kein Werkzeug setzt die Sprache. `list_works` (je Werk) und `outline` (oberste Ebene) tragen `language` (`docs/MCP.md`).

### Sprache der Schnittstellen (Runde 38) {#language-of-the-interfaces-round-38}

Englisch sind seit Runde 38: die MCP-`instructions`, jede Werkzeug- und Parameterbeschreibung, die Antworten von Stubs und
jeder Fehlertext, der nur in `src/mcp/**` entsteht; bei RPC die Protokollfehler – Parse-Fehler, ungültige Anfrage,
`unknown method …`, die Parameterprüfungen von `rpc.rs` (`work: work id (number) expected`, `key: stable key (number)
expected`, `mode: "auto" or "new" expected`), `Login required.` (-32002 bei `/rpc` ohne Sitzung) und `Only the
author may do this: <method>` (Rolle). **Die Fehlercodes sind unverändert.**

**Meldungen der Oberfläche bleiben bis zu den Katalogen von Runde 39a deutsch, auch wenn sie durch RPC oder
MCP laufen:** jede Meldung, die auch eine Seite, ein Formular oder eine Hinweisleiste zeigt – die Texte von `StoreError` (`Refused`,
unbekanntes Werk, unbekannter Abschnitt oder unbekannte Fassung), die Ablehnungen der Module (`REFUSED`, -32000) und ihre Parameterprüfungen außerhalb
von `rpc.rs`, Fortschrittsgründe wie `fehlendes „Kritisch:“`, die Hinweise `CUT_NOTE` und `TRUNCATED_NOTE` unter einem Beitrag,
der Hinweis eines Umschlagantrags, den `asset_add` anlegt, und Bezeichnungen von Modellaufrufen. Deshalb zitiert dieses Dokument solche
Meldungen deutsch („…“). Bis Runde 38 galt das auch für die Aufgabentexte, die Baruch um den Text eines
Abschnitts legt, wenn es ein Modell aufruft (`assist/context.rs`, `solutions.rs`, `variants.rs`, `glossary.rs`, `threads.rs`; siehe
„Open“ unten). Seit Runde 38c ist dieses ganze Dokument englisch; deutsch bleibt nur als Zitat (Texte der Benutzer-
oberfläche, Meldungen, Commit-Betreffe, Modellmarker) und im Code.

### Prompts und Marker {#prompts-and-markers}

Die Standard-Prompts der Personas (`assist/prompts.rs`) sind seit Runde 38 englisch. Ein vom Nutzer gespeicherter Prompt
bleibt unangetastet. Unter **jedem** wirksamen Prompt eines Modellaufrufs – Standard oder gespeichert – steht die Sprachregel
`assist::lang::language_rule(language)`: "Always answer in {German|English} – the language of this work – whatever
language this prompt or the question uses. Use these markers literally: …" mit den Markern der Werksprache.
Der zum Bearbeiten gezeigte Prompt (`persona.prompt_get`, „Einstellungen → Assistenten“) trägt keine Sprachregel, damit ein
gespeicherter Prompt nicht die Regel eines Werks in ein anderes mitnimmt.

Der Prompt schreibt die Marker der Werksprache vor; **jede Erkennung akzeptiert beide Sprachen**, welche Sprache das
Werk auch hat. Für ein deutsches Werk ist jedes Verhalten wörtlich wie vor Runde 38.

| Bedeutung | Deutsch (`lang::DE`) | Englisch (`lang::EN`) | Gelesen von |
|---|---|---|---|
| verpflichtender oder kritischer Befund | `Kritisch:` | `Critical:` | `prompts::critical_line`, `has_critical`, Befund-Parser |
| Hinweis | `Vorschlag:`, `Hinweis:` | `Suggestion:`, `Note:` | Befund-Parser, `strip_critical_lines` (`Vorschlag:`/`Suggestion:`) |
| Lob | `Lob:` | `Praise:` | Befund-Parser |
| Gliederung einer Prüfung | `Was trägt:`, `Was fehlt:`, `Was stört:` | `What works:`, `What is missing:`, `What disturbs:` | Befund-Parser, `strip_critical_lines` (erster) |
| RFC-Zusammenfassung des Lektors | `Jetzt wichtig:`, `Kann warten:` | `Important now:`, `Can wait:` | `strip_critical_lines` |
| Lösungen, die die Persona anwenden würde | `Mein Vorschlag:` | `My proposal:` | `findings::parse::proposal_heading`, `assist::solutions::proposal_block` |
| Lösungszeilen | `statt:`, `besser:`, `Schritt:` | `instead of:`, `better:`, `Step:` | `assist/solutions.rs`, `prompts::missing_solutions` |
| Rückmeldung zu einem eigenen Befund | `Erledigt: Befund n (Stelle "…")` | `Done: finding n (place "…")` | `findings::parse::done_claims` |
| alle eigenen Befunde erledigt | `Alle meine Befunde zu diesem Abschnitt scheinen erledigt.` | `All my findings on this section seem to be resolved.` | `prompts::all_done` |
| Hinweis von Baruch: Zitat nicht im Text | `(nicht im Text belegt)` | `(not backed by the text)` | in der Werksprache geschrieben, in beiden entfernt und gelesen |
| Hinweis von Baruch: keine Lösung | `(ohne Lösungsvorschlag)` | `(no solution proposed)` | in der Werksprache geschrieben, in beiden entfernt und gelesen |
| Hinweis von Baruch: verpflichtender Befund fehlt | `(Hinweis von Baruch: …)` (`CRITICAL_MISSING_NOTE`) | `(Note from Baruch: …)` (`CRITICAL_MISSING_NOTE_EN`) | in der Werksprache geschrieben, Fußzeile in beiden |
| Schlusszeile der Stellenprüfung (Runde 39b) | `Wortlaut: ungeprüft – …`, `Hinweis: Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft.`, `Wortlaut geprüft von Baruch (nicht vom Modell) …` | `Wording: unchecked – …`, `Note: Bible references and wording are checked by @schrift; unchecked here.`, `Wording checked by Baruch (not by the model) …` | `scripture::mark_with_in`, in der Werksprache geschrieben, Fußzeile in beiden (`chat_render`) |
| Wörter, die @schrift und @belege schreiben (Runde 39b) | `Wortlaut ungeprüft`, `nicht belegt`, `Angabe prüfen` | `wording unchecked`, `not supported`, `check reference` | Standard-Prompts (`prompts::read_back_words`); gespeicherte Prompts bleiben wie geschrieben |
| Hinweis zu einer Stellenbehauptung (Runde 39b) | `(Stelle steht im Text: …)`, `(Stelle ungültig: …)` | `(reference is in the text: …)`, `(reference invalid: …)` | `ref_claims`: Sprache der Befundzeile (`Kritisch:`/`Critical:`), in beiden entfernt und gelesen; Behauptungswörter beider Sprachen |
| Zusätze der Webprüfung (Runde 39b) | `(nicht belegt)`, `(unbekannte Quelle)`, `(nicht abgerufen)`, `Angabe prüfen: nicht belegt` | `(not supported)`, `(unknown source)`, `(not fetched)`, `check reference: not supported` | `websearch::verify::check_in`; in beiden erkannt (der Aufrufer in `assist/mod.rs` übergibt noch keine Sprache, siehe `HANDOFF.md`) |

**Buchausgabe in der Werksprache (Runde 39b).** Alle Wörter, die Baruch in ein Buch erzeugt, folgen der Werksprache
durch den Buchkatalog `crates/baruch-core/locales/{en,de}/book.ftl` (`baruch_core::book_text::book_t`): EPUB
(`epub.export`, `epub.check`, `epub.preview`), die Exportkarte und der Satzquelltext (Nummerierungswörter der
Schemata mit Wörtern, `p.` bei Querverweisen mit Seite, Überschriften von Register und Glossar, `No entries.`, `see`/`see also`),
der Word-Export (`docx.export`: `w:lang`, Impressumzeilen), der Stichwortzettel (`notes.pdf`) und die Stellenprüfung
(`scripture.check`, `bible.check`: Feld `wording`). Das Sprachkürzel eines Buchs: die Satz-Einstellung `lang`, wenn gesetzt
und gültig; sonst für ein deutsches Werk das `#set text(lang: …)` der Vorlage oder `de` (unverändert); für ein englisches Werk
`en` – dann steht `#set text(lang: "en")` im Regelblock hinter der Werkvorlage. Deutsche Werke erzeugen byte-gleiche
Ausgaben (Golden Fixtures `tests/data/round39b/`). Die Nummerierungswörter eines englischen Werks: `Part I`, `Chapter 1`
(Teile und Kapitel), `Part One`, `Chapter One` … `Chapter Ninety-Nine`, ab 100 Ziffern (ausgeschrieben und Buch),
`para.`, `no.` (juristisch), `Introduction`, `Conclusion` (Predigt, Buch).

Weitere in Runde 38 über den Vertrag hinaus gefundene Marker (`lang::MoreMarkers`): `besser:`/`better:`,
`Schritt:`/`Step:`, die Wörter der „Erledigt“-Zeile (`Befund`/`finding`, `Stelle`/`place`) und die zwei weiteren Gliederungs-
überschriften. Verbotene Einleitungen werden in beiden Sprachen erkannt (`Als Assistent`, `Als KI`, `Als Sprachmodell`,
`Gerne`, `Natürlich`; `As an assistant`, `As an AI`, `As a language model`, `Sure`, `Certainly`), und die Wortliste
eines Prüfauftrags im Chat kennt auch englische Stämme (`check`, `review`, `verif…`, `examin…`, `proofread`).

**Seit Runde 38c** sind auch die Aufgabentexte um den Abschnitt (`Work: …`, `Task: …`, Rahmen von Befunden und Chatverlauf)
englisch; Texte, die Marker nennen (`OTHERS_RULE`, `RFC_TASK`, `PROPOSE_OUTPUT`), laufen durch
`lang::localized`, sodass ein deutsches Werk seine deutschen Marker wörtlich behält und ein englisches Werk die englischen erhält.
Die Sprachregel allein legt die Antwortsprache fest. Absichtlich noch deutsch: die Rahmen-Tags (`<<<befund`,
`<<<einfall` …), die Antwortform der Varianten (`Zeile:`/`Befunde:`/`Text:`, `[[Absatz n]]`, vom Parser zurückgelesen)
und Bezeichnungen, die auch in der Oberfläche erscheinen. **Offen (ehrlich):** Die wörtlichen Wörter
von `@schrift` und `@belege`, die Baruch zurückliest (`Wortlaut ungeprüft`, `nicht belegt`, `Angabe prüfen`), werden
nur deutsch erkannt; der englische Prompt nennt daher für diese noch die deutschen Wörter.
