<!-- translation-of: docs/Architektur.md source-commit: 4d3148c part: 3 -->

## Befunde (Runde 28) {#findings-round-28}

Schnittstelle R28-S1 (Integrator): Migration 0046 (`finding`, `finding_status`, `post_version`, Spalten `post.edited`
und `chat_thread_post.edited`, Grabstein-Arten `finding`, `finding_status`, `post_version` mit Kaskaden-Triggern wie
0042/0045), Store `src/findings.rs` (Typen, lesende Abfragen und `finding_insert` vollständig, Statuswege als Stub) und
`src/post_edit.rs` (lesende Abfragen vollständig, Bearbeiten als Stub), Server `src/findings/{mod,parse,rpc,page,view}.rs`,
`mcp/findings.rs`, `mcp/posts.rs`, `PostAction::Edit`, `mirror::describe_finding`/`record_finding`. Offene Stubs:
`grep -rn "STUB:" crates`.

### Datenmodell, Zerlegung, Ähnlichkeit, Spiegel, Austausch u4 (befunde) {#data-model-splitting-similarity-mirror-exchange-u4-befunde}

- **Datenmodell (Migration 0046):** `finding` (Werk, Abschnitt `node_id` – NULL für Fäden und gelöschte Abschnitte –,
  Herkunft `post_id` oder `thread_post_id`, Ort `position` eindeutig je Herkunft, Persona, Art, Text, Zitat,
  Prüfvermerk, Status, Begründung, Fassung/Ziel als UUID ohne Fremdschlüssel, `similar_to`, `summary_of` als
  JSON-Liste von UUIDs), `finding_status` (jede Änderung; Mensch `proposed = 0`, Assistent `proposed = 1` mit
  `decision`), `post_version` (alte Fassungen; das Bearbeiten selbst: chat-auftrag). Die Statusregeln gibt es doppelt:
  als CHECKs in der Datenbank und als Prüfung in `baruch_store::findings::resolve` (verständliche Meldung,
  `StoreError::Finding` → -32602). Der Store löst Fassung und Ziel beim Lesen über die UUID auf (`adopted_rev`,
  `deferred`); fehlt die Fassung, bleibt die UUID stehen und `adopted_rev` ist `None`.
- **Schreibwege (Store):** `finding_set_status` (nur Mensch, eine Transaktion: Befund + Verlaufseintrag; gleicher
  Zustand → abgelehnt), `finding_propose_status` (nur Persona/MCP, nur adopted/deferred/done, kein offenes Duplikat;
  der Befund ändert sich nicht), `finding_decide` (confirmed/refused; veraltet, wenn nach dem Vorschlag ein Eintrag des
  Menschen oder ein bestätigter Vorschlag kam, die Fassung oder das Ziel fehlt oder der Befund den Status schon hat).
  Server `findings/rpc.rs`: `set_status`, `propose`, `decide` halten `write_lock` vom Schreiben bis zum Commit, genau
  ein Commit über `mirror::record_finding` (`describe_finding`: Betreff „Befund <id>: …“, Trailer `Baruch-Finding`,
  `Baruch-Action: finding-status|finding-propose|finding-decide`, `Baruch-Node`, bei Vorschlägen `Persona`, bei
  Entscheidungen `Baruch-Proposer`).
- **Zerlegung (`findings/parse.rs`):** zeilenweise; die Gliederung „Was trägt/fehlt/stört“ legt die Art der folgenden
  Punkte fest, Marker haben Vorrang, Listenpunkte und (unter einer Gliederung) Absätze sind je ein Befund, eingerückte
  Folgezeilen und Unterpunkte gehören dazu, eine Leerzeile schließt ihn ab. „Kritisch:“ nach einem kurzen
  Stellenbezug erkennt `assist::prompts::critical_finding` (dieselbe Regel wie die Prüfung). Prüfvermerke
  (`UNBACKED_NOTE`, `ref_claims::IN_TEXT`/`INVALID`) wandern nach `quote_check`, Vermerke von Baruch (`CUT_NOTE`,
  `TRUNCATED_NOTE`, `CRITICAL_MISSING_NOTE`, „Komprimiert: …“) fallen weg. `ingest` nimmt nie die Schreibsperre (die
  Aufrufer in `assist::save` und `threads::answer` halten sie), schreibt nur über `Store::finding_insert` und
  überspringt Orte, die für diesen Beitrag schon existieren (wiederholbar). Kein Commit.
- **Ähnlichkeit:** nur Text (`parse::similar`): Inhaltswörter, kleingeschrieben, als Mengen – Wörter ab drei Zeichen
  ohne `STOPWORDS` (Artikel, Präpositionen einschließlich „zur“/„zum“, Pronomen, Hilfsverben, Partikeln), dazu jede
  Zahl –, Dice-Maß ab `SIMILARITY_THRESHOLD` = 0,8 (oder gleicher Text): bei drei oder vier Inhaltswörtern trennt schon
  ein abweichendes (0,67 oder 0,75), erst ab fünf darf eines abweichen. Vorher (0,6, mit Füllwörtern) galten „Der
  Übergang zur Werft fehlt“ und „… zum Hafen fehlt“ als gleich, und der neue Befund verschwand als angehängt.
  Kandidaten: Befunde desselben Abschnitts aus anderen Beiträgen, die nicht angehängt sind – entschiedene vor offenen
  derselben Persona. Keine Einbettungen (R19a). Grenze: Umformulierungen mit anderen Wörtern erkennt das nicht; der
  Mensch kann dann verwerfen, oder die Persona bekommt die Entscheidungen gerahmt (chat-auftrag).
- **Nachtrag (`findings::backfill`):** beim Start von `serve` nach Migration und Spiegelabgleich, einmal je
  Datenbank (`store_meta.findings_backfill`), nur Beiträge, deren Autor ein Handle aus `persona` ist (nicht „Autor“,
  nicht `@mcp:…`); bricht bei einem Fehler ab, ohne die Marke zu setzen (der nächste Start macht weiter, vorhandene
  Orte werden übersprungen).
- **Spiegel:** `befunde.md` (`findings::mirror_files`, in `snapshot::files` nach den Einfällen) nur, wenn es Befunde
  gibt; Gruppen in Baumreihenfolge (`snapshot::ordered_currents`), dann Fäden nach Id, dann Befunde ohne Bezug; je
  Befund eine Überschrift `[B<id>] Art · Status · Persona`, Text als Zitatblock, Zitat mit Prüfvermerk, Begründung,
  Fassung oder Ziel, Anhang, Sammlung, offener Vorschlag, Herkunft und Verlauf. Nur Daten aus der Datenbank
  (deterministisch).
- **Austausch u4:** `baruch-exchange` (Modell `ExchangeFinding`, `ExchangeFindingEntry`, `ExchangePostVersion`, Wörter
  `ExchangeFindingKind`, `ExchangeFindingStatus`, `ExchangeDecision`, Grabstein-Arten `befund`, `befund-status`,
  `beitrag-fassung` seit 4), Schreiber (`<befunde>` am Ende, `geaendert` und `<alte-fassung>` an Beiträgen), Leser (erst
  ab `version="4"`, davor unbekannt; Grabsteine ab 4), Prüfung (`check_findings`, Regeln wie die CHECKs), XSD
  `baruch-werk-1-u4.xsd` (u3 eingefroren). Store `exchange_dump` (Verweise über UUIDs) und
  `exchange_apply::apply_findings` (neu: alles mit Kennungen; Abgleich: unbekannte Befunde ergänzt, bekannte nie
  überschrieben, belegter Ort → Hinweis, `gleicht` nach allen Befunden), `insert_versions` für alte Fassungen.
  Einzelheiten in `docs/Austauschformat.md`.

### Übersicht und Darstellung (befund-uebersicht) {#overview-and-display-befund-uebersicht}

- **Grenze der Listen:** `Store::finding_list` liefert höchstens `limit` (Standard 500, höchstens 2000), und zwar die
  **neuesten** (`ORDER BY created DESC, id DESC`, umgedreht: weiter älteste zuerst) – Matrix, Entscheidungen im Prompt
  und Ähnlichkeitskandidaten verlieren die ältesten oberhalb der Grenze, nicht die jüngsten. `Store::finding_count`
  zählt mit demselben Filter ohne Grenze: `finding.list`/MCP `findings` melden `total` (alle) und `truncated`, die
  Matrix `truncated` (Zahl aller, sonst `null`) und auf der Seite „Gekappt: …“. `finding.get` lädt angehängte Befunde
  und Herkünfte gezielt (`finding_attached_to`, `findings_by_uuid`).
- **Zählregel an einer Stelle:** offen = `Store::finding_open_counts` (Vertrag §1.5). `findings::view::subtree_open`
  summiert diese Zahlen über die Baumreihenfolge (Eltern vor Kind) – daraus zeichnen sich die Gliederung (`view.rs`,
  `TreeItem.findings`), der Editorkopf (`node.open_findings`), `/gliederung` (`outline.rs`, `entries_with`) und die
  Einfallskarte; `finding.matrix` nennt `open_badge` je Zeile aus denselben Zahlen, und ohne Filter gilt
  `total.open == open_badge` (Test `round28_uebersicht.rs`). `menu.badges.open_findings` ist ihre Summe.
- **`findings::view`:** `load` holt je Seite einmal die offenen Zahlen, alle Befunde des Werks (einschließlich Lob und
  angehängter, höchstens `VIEW_LIMIT` = 2000) und je Abschnitt mit übernommenen Befunden `findings_adopted_in` (für
  den Fassungsbaum: `adopted_in`, offene Vorschläge: `proposed_for`). `card` baut das gemeinsame Bild eines Befunds
  für Seite und Akkordeon (`finding_json` plus Zeichen, Abschnitt, Zeit, angehängte Befunde, Herkünfte eines
  Sammelvorschlags über `summary_of`-UUIDs, offener Vorschlag in Worten, Fassungswahl). Statuszeichen: ○ ✓ ✗ → =.
- **`findings::page`:** `build_matrix` ist rein (testbar); Zeilen = Kapiteleinheiten (nächster Vorfahr der Ebene ≤ 1),
  Gruppen = Teile, „ohne Abschnitt“ am Ende. Die Seite hat eine eigene Template-Umgebung (`base.html`,
  `befunde.html`, `fragments/findings_panel.html`); die Karten- und Formular-Makros stehen in
  `fragments/findings_panel.html` und werden von `befunde.html` importiert (ohne `findings_panel` im Kontext zeichnet
  das Template selbst nichts). Formulare laufen über `findings::rpc::{parse_change, set_status, decide}` (Sperre und
  Commit dort); die Seite selbst nimmt keine Sperre.
- **Akkordeon:** `view.rs::findings_panel` filtert `FindingsView.findings` auf den gezeigten Abschnitt (offen oder mit
  offenem Vorschlag, entschieden, Lob); `ViewExtras.findings` füllt `view::extras` mit `unwrap_or_default` (ein Fehler
  beim Laden lässt die Werkseite ohne Befunde stehen). Das Cookie `baruch_accordion=befunde` hält es offen.
- **Einfallskarte:** `MapFilter.findings` (Seite `befunde=1`, RPC `findings`), `MapNode.open_findings` und
  `MapSection.open_findings` werden in `idea_map::build` wie die Einfälle über Anker und Vorfahren aufsummiert;
  `svg.rs` setzt `im-many-findings` aus `findings::page::MANY_OPEN` (3) und schreibt „k offen“ als
  `<tspan class="im-findings">` (DOM-Vertrag erweitert, nicht geändert).
- **Widget `findings`** (`web/src/findings.ts`): Statusformulare über RPC (danach `baruch:navigate` oder ein Neuladen
  des Inhalts), auf der Seite Filter ohne Neuladen (Abruf derselben Adresse, Austausch von `[data-findings-main]`,
  `history.pushState`), Pfeiltasten in der Matrix. Ohne JavaScript bleibt alles bei Links, GET-Filtern und
  Formularen.
- **MCP** (`mcp/findings.rs`): `findings` über `Store::finding_list`, `finding_propose_status` prüft Status und Felder
  selbst und ruft `findings::rpc::propose` mit `tools::origin_of` (Persona nur ohne Token).

### Chat-Aufträge, Rahmen, Beitragsfassungen (chat-auftrag) {#chat-jobs-frame-post-revisions-chat-auftrag}

- **Lektor als Begleiter** (`assist/prompts.rs`): `CRITICAL_HANDLES = ["nathan"]`; die Nachforderung (`answer_call`)
  und `CRITICAL_MISSING_NOTE` jetzt nur noch für Nathan. Die Voreinstellung des Lektors besteht aus `LECTOR_RULE`
  (6: zuerst „Was trägt:“ mit Zitat, höchstens zwei „Vorschlag:“, kritische Befunde erlaubt; mit derselben Ausnahme
  für neue Fassungen wie `EVIDENCE_RULE`/`CRITICAL_RULE`, weil der Systemprompt nicht vom Modus abhängt),
  `BACKING_RULE` (7: Belegpflicht für Behauptungen, für jede Rolle mit kritischen Befunden) und `COMPANION_RULE`
  (8–9: Chat, Entscheidungen, RFC); der Exeget hat `EVIDENCE_RULE` + `BACKING_RULE`, Nathan `CRITICAL_RULE` +
  `BACKING_RULE` (wörtlich wie zuvor). Die Regeln sind kurz gehalten: Jedes Zeichen des Rollenprompts fehlt dem Kapitel
  im Kontext (`round18_lektor.rs` und `round15_lektor.rs` messen die Grenzen). `mark_unbacked_criticals` gilt
  unverändert für alle.
- **Rahmen** (`assist/context.rs`): `decision_lines` liest über `Store::finding_list` die Befunde des Abschnitts oder
  der Kapiteleinheiten mit Status adopted, discarded, deferred, done (ohne Lob, ohne angehängte), verworfene zuerst,
  höchstens `MAX_DECISIONS` = 30 (darüber eine ehrliche Schlusszeile), je Zeile `- [B<id>] rejected by the author
  (@x on Titel): <<<entscheidung "…" (reason: …) >>> – do not report again`, entschärft wie `framed_line`. Dazu
  `DECISIONS_RULE` (und `OTHERS_RULE`) im Systemteil. Bei einem RFC rahmt `rfc_lines` die Antworten der anderen
  (`<<<antwort … >>>`, Kopf `@x [B1] [B2]`) unter `RFC_TITLE`, der Auftrag bekommt `RFC_TASK`; die Beiträge dieser
  Antworten fallen aus dem Chatverlauf (nicht doppelt). Der Chatverlauf (`RECENT_TITLE`) steht im Chat und beim
  Schreibauftrag (`frame.as_order`, Auftrag mit `AS_ORDER_TASK`). Beim Kürzen fallen Antworten und Entscheidungen
  zuletzt und nie stillschweigend (`truncated`). `FOREIGN_KINDS` enthält `entscheidung` und `antwort`: Sie belegen
  keinen „Kritisch:“-Befund (Korpus der Zitatprüfung ohne sie).
- **Chat-Aufträge** (`assist/chat.rs`): `is_rfc` (erstes Wort „RFC“ oder „@alle“), `is_write_order` (das erste Wort
  nach Erwähnungen und „bitte“ ist ein Imperativ aus `WRITE_VERBS`), `rfc_personas` (aktive Personas in ihrer
  Reihenfolge, der Lektor durch `personas::rows_and_lector` herausgenommen). RFC: je Persona `run` im Modus chat, die
  Antworten als `FramedAnswer` (Text, Beitrag, Befund-Ids aus `Outcome::findings`), zuletzt der Lektor mit
  `OrderFrame { rfc_answers, rfc_summary: true }` (seine Befunde werden über `findings::ingest` zu Sammelbefunden).
  Schreibauftrag: `run` im Modus `Propose` mit `frame.as_order`, dann unter der Schreibsperre der kurze Beitrag
  `order_note(rev)` (kein Befundmarker am Anfang, ergibt beim Zerlegen keinen Befund) und für jedes `[B<id>]` aus
  Nachricht oder Vorschlag `findings::rpc::propose(… Adopted { rev } …)` (eigene Sperre und Commit; Fehler in
  `notes`) – nur wenn `finding.key == order.key`: `rev` ist eine Nummer im Abschnitt des Auftrags, `resolve` sucht sie
  aber im Abschnitt des Befunds; ein Befund aus einem anderen Abschnitt bekommt darum nur einen Hinweis in `notes`.
  Fäden (`threads.rs`) nutzen `is_rfc`/`rfc_personas`; `thread_call` nimmt die gerahmten Antworten, `answer` zerlegt die
  Antwort des Lektors mit `summary = true`. Die Relaisstufe von MCP entfällt bei einem RFC.
- **Beitragsfassungen**: Store `post_edit.rs` (`post_edit`, `thread_post_edit`: eine Transaktion `BEGIN IMMEDIATE`,
  alte Fassung als `post_version` mit `valid_from` = Anlage oder vorherige Änderung, `edited` = `replaced`; leer, mehr
  als 4000 Zeichen oder unverändert → `StoreError::PostEdit`). Server: `posts::edit` (RPC `post.edit`, Formular, MCP
  `post_edit`) prüft unter der Schreibsperre, dass der Handelnde der Autor ist (Mensch „Autor“, MCP `@mcp:<name>`), und
  schreibt genau einen Commit über `mirror::describe_post(…, PostAction::Edit, Some(&before), …)`;
  `threads::edit_post` ohne Commit. Volltextindex: Trigger `post_fts_edit` (Bibliothek 0107, `AFTER UPDATE OF body ON
  post`).
- **Oberfläche**: `posts::visible_range` zeigt die neuesten `CHAT_PAGE_POSTS` = 50 (`view.rs` liefert
  `post_older`); RPC `post.page` zum Nachladen. Seiten `/beitraege` und `/beitrag` (eigene Template-Umgebung in
  `posts.rs`, Templates `posts.html`, `post.html`). CSS: `app.css` begrenzt `.chat` (`max-height`,
  `overflow-y: auto`, `user-select: text`) und stellt `.acc[data-section="chat"] .acc-body` von `overflow: hidden`
  auf `auto` um – das war die Sperre für Kopieren und Scrollen (ohne begrenzte Höhe schnitt der Bereich ab; wo
  `::details-content` nicht greift, gab die Flex-Kette die Höhe nicht weiter). Bausteine in `chat.css`, Fäden in
  `threads.css` (`.thread-scroll`). Widget `chat` (`web/src/chat.ts`): `enableCopy` (Clipboard-API, Rückfall
  `execCommand`), Zitieren (`quoteLine`, einzeilig), „Neue Beiträge ↓“, „Ältere Beiträge“ über `post.page`; Widget
  `chat-page` für die beiden Seiten, `threads.ts` nutzt `enableCopy`. Schaltflächen, die nur mit JavaScript
  funktionieren, stehen mit `hidden` im HTML.
- Grenze (ehrlich): Ein Schreibauftrag, der einen **neuen** Abschnitt verlangt („Erstelle ein Vorwort“), wird zum
  Vorschlag für den gezeigten Abschnitt; neue Abschnitte aus dem Chat anzulegen ist nicht umgesetzt (nur über die
  Gliederung oder MCP `outline_add`) – ein Rest für den Backlog. Ob das Kopieren im echten Browser früher an etwas
  anderem als dem abgeschnittenen Bereich scheiterte, ließ sich ohne Browser nicht nachstellen (Teamregel: kein
  Firefox); HTML, CSS und Skript sind geprüft.

## Schnittstelle von Runde 30 (R30-S1) {#interface-of-round-30-r30-s1}

Gemeinsame Grundlage der drei Aufgaben von Runde 30 (Vertrag §1), danach füllt jede nur ihre eigenen Dateien:

- Migration `0047_r30.sql` (reines Schema, eingefroren): Metadaten an `post` und `chat_thread_post` (`basis_rev`,
  `basis_label`, `reply_to`, `repeat_of`, `relay_from`, `request`; Verweise mit ON DELETE SET NULL),
  `finding.solutions`, `revision.variant_*` (Gruppe und Beschriftung beide oder keine, im Code geprüft), Tabelle
  `replace_batch` (lokales Protokoll, keine uuid). Reserviert: Store 0048–0051, Bibliothek 0108/0109.
- Store, vollständig umgesetzt: `Store::add_post_meta`/`post_meta*`/`thread_post_meta*` (`post_meta.rs`),
  `Store::add_proposal_on`/`variant_group` (`variants.rs`, `write::add_proposal_on`), `Store::finding_set_solutions`,
  `Store::replace_batch`; `replace_apply`/`replace_undo` sind Stubs (`StoreError::Replace` → RPC `-32000`).
- Server-Haken: `assist::save` speichert Persona-Beiträge mit Metadaten (geprüfte Fassung, Auftrag, Verweise aus
  `Order::links`); `assist::chat` wählt eine Grundlage für alle Personas (`basis::choose`), ruft
  `variants::is_variants_order`/`run` und nach dem Lektor bei einem RFC `solutions::rfc_proposal`; `view::posts`
  hängt an jeden Beitrag `extra` (`chat_render::post_extra`) und `solutions`, an den Fassungsbaum `variant`
  (`variants::revision_variant`); `mirror::record_request` spiegelt fertige Commit-Nachrichten der neuen Module.
- Weiterleitung von `search.*`, `xref.*`, `solution.*` in `rpc.rs`, Routen in `lib.rs` (schreibende hinter
  `guard::same_origin`), MCP `text_search` und `solution_variants` (Stubs, `solution_variants` zählt zum Kontingent),
  Menüeinträge unter Werkzeuge, leere Template-Makros (`search_bar`, `post_item`, `finding_block`,
  `post_solutions`, `basis_select`) und TypeScript-Module (`search`, `search_keys`, `jump`, `xref`, `variants`).

## Suchen und Ersetzen (Runde 30a) {#search-and-replace-round-30a}

- **Server `src/search/`:**
  - `mod.rs`: Typen (`SearchQuery`, `SearchHit`, `SearchResult`, `TextKind`, `SearchRange`, `RevChoice`), Bereiche
    (`sections_in`: Abschnitt, Kapitel = nächster Kapitelvorfahr einschließlich Unterpunkten, Teil, ab Abschnitt,
    Auswahl, Werk, alle Werke), Text je Abschnitt (aktuelle oder gezeigte Fassung, ungespeicherter `text`, Auswahl in
    UTF-16), `find`/`find_within` (Zeitgrenze als Parameter für Tests) und RPC-Weiterleitung.
  - `scan.rs`: Textarten je Byte (`classify`): Überschriftentitel, Fußnoteninhalt, Zitate über
    `crate::quotes::quote_spans`, Code (`#name`, Argumente, Schlüsselwortzeilen, Roh, Formeln, Kommentare, Marken,
    `@verweis`), sonst Text. Ein Treffer über mehrere Arten zählt als die strengste (Code > Zitat > Fußnote >
    Überschrift > Text). Dazu das Muster (`pattern`: Crate `regex`, `size_limit(1<<20)`, `dfa_size_limit(1<<22)`; im
    Wortlautmodus findet jedes Sonderzeichen auch `\\x`), Treffer mit Ersetzung (`find_in`; Regex:
    `Captures::expand`, Wortlaut: `escape_literal`) und UTF-16-Umrechnung.
  - `replace.rs`: `preview` (Diff je Abschnitt als HTML-Zeilen mit `<del>`/`<ins>`, alles maskiert, `body_after`),
    `replace` und `undo` unter der Schreibsperre; je ein Commit über `mirror::record_request` mit
    `Baruch-Action: replace` oder `replace-undo` und `Baruch-Replace: <id>`.
  - `page.rs`: `GET /suche`, `POST /suche/act` (eigene Template-Umgebung, `templates/search.html`).
- **Ablauf einer Ersetzung:**
  - Der Server sucht noch einmal auf der aktuellen Fassung. Die Reihenfolge der Treffer ist dieselbe wie in der
    Vorschau, `skip` zählt darin.
  - Er berechnet die neuen Texte und gibt sie mit der Grundlage aus der Vorschau an `Store::replace_apply`.
  - Der Store prüft in **einer** Transaktion je Abschnitt unter einem eigenen `SAVEPOINT`: Ist die aktuelle Fassung
    noch die Grundlage, entsteht eine neue Fassung des Menschen (`write::save_human`), danach
    `outline::check_current_heading`. Andere Ebene, weitere Überschriften oder keine → nur dieser Abschnitt wird
    zurückgerollt und mit Grund übersprungen.
  - Am Ende schreibt er eine Zeile `replace_batch` (Optionen als JSON, `items` = `[{key, prev_rev, new_rev}]`).
- **Rückgängig:** `Store::replace_undo` schaltet je Eintrag mit `write::set_current` + `outline::follow_switched`
  zurück, nur wenn `new_rev` noch aktuell ist, und setzt `undone`. `Store::replace_latest` liefert die jüngste offene
  Ersetzung für `/suche`. `replace_batch` ist lokal: keine uuid, nicht im Austauschformat.
- **Widgets:**
  - `web/src/search_keys.ts`: reine Funktion `keyAction(e, ctx)`, unter Node geprüft.
  - `web/src/search.ts`: Suchleiste `search-bar` aus `fragments/search_bar.html`, Werk-Seitenleiste, Listener am
    Dokument in der Capture-Phase. Treffer in der Satzansicht über die CSS Custom Highlight API (`baruch-search`,
    `baruch-search-current`). Ersetzen im Abschnitt läuft über `search.preview` mit `text=` und ändert nur das
    Textfeld.
  - Stil `static/search.css`.
- **MCP** `text_search` (`src/mcp/search.rs`): `crate::search::find` mit Bereich `section|chapter|part|from|work`,
  Standard 100 Treffer, kein Schreibweg.

## Querverweise (Runde 30a) {#cross-references-round-30a}

- **`baruch_typst::xref`** (rein):
  - `label_heading`/`label_section` projizieren die Sprungmarke: ` <kN>` am Ende einer `=`-Zeile vor einem Kommentar;
    bei einer eigenen Marke (Teil, „ohne Nummer“, Marke am Zeilenende oder allein darunter) die Ankerzeile
    `#metadata("baruch-key") <kN>` darunter.
  - `strip_key_label`/`strip_section`/`strip_key_labels` entfernen sie wieder.
  - `find_xrefs` findet `#link(<kN>)[…]` einschließlich des Seitenzahl-Zusatzes, `remap_keys` schreibt Schlüssel um,
    `xref_snippet` baut die Form.
  - `part_anchors_for_typesetting` dient nur dem Satz mit Teilseiten, siehe unten.
- **Store:**
  - `snapshot::export_parts` und `snapshot::files` fügen `label_section` nach der Projektion „Nummer an/aus“ ein.
    Export, Exportkarte (byte-identisch), Spiegel und Satz tragen also die Sprungmarken, SQLite nicht.
  - `import.rs` entfernt sie je Überschrift (`strip_section`), legt zuerst alle Knoten an und schreibt dann die Texte
    mit `remap_keys` (alt → neu `node.id`). Die Prüfsumme gegen einen doppelten Import bleibt die der Datei.
- **Anker eines Teils im Satz (Messung `baruch-render/tests/xref.rs`):**
  - Die Teilregel (`part_page_rule`) bricht die Seite nach dem Teil um. Eine Ankerzeile nach der Teilzeile läge
    deshalb auf der Folgeseite, und `counter(page).at(<kN>)` nennte die falsche Seite.
  - `typeset::part_pages` fügt darum beim Satz `part_anchors_for_typesetting` ein. Die Ankerzeile rückt vor die
    Teilzeile, mit eigenem `#pagebreak(weak: true, to: "odd")`. Der Umbruch der Regel entfällt dann, weil die Seite nur
    den Anker trägt (gleiche Seitenzahl gemessen).
  - Export und Spiegel behalten die Ankerzeile darunter (ohne die Teilregel liegt sie auf der Seite der Überschrift).
    `typeset::rewrite_sections` führt Zeilenzahl und Byte-Bereiche der Karte nach.
- **Kaputte Verweise:**
  - `xref::broken_in` meldet Verweise auf Schlüssel ohne aktuelle Fassung.
  - `typeset::xref_safety` macht sie beim Satz zu einfachem Linktext; sonst scheiterte der ganze Satz mit "label does
    not exist".
  - Die Druckprüfung (`typeset.html`, `li.typeset-xref`) nennt sie.
- **baruch-wysiwyg:**
  - `Inline::Xref {key, c, page}`; `parse` erkennt `#link(<kN>)[…]` einschließlich eines Zusatzes mit demselben
    Schlüssel.
  - `emit` schreibt `xref_snippet`, `render_html` ein `span.ws-xref` mit `data-key`/`data-page` und ein nicht
    editierbares `span.ws-xref-page`.
  - `wysiwyg.ts` liest den Block zurück (`t: "xref"`), fügt ihn über den Befehl `xref` ein (`ws_command.ts`) und
    markiert `baruch:find` mit der Hervorhebung `baruch-find`.
- **Server `src/xref.rs`:**
  - `jump_url`, `check_marker` (Schlüssel, Zitat mit normalisiertem Leerraum, `at` = Byte des Zitats);
    `outline_mentions` (zählende Erwähnungen über ein Muster mit Ordnungszahlen eins bis zwölf, römisch bis XXXIX,
    arabisch und Zahlen wie 2.3; Titel ab vier Zeichen als ganze Wörter mit Artikel, kürzere nur in Anführungszeichen;
    eindeutig oder gar nicht, nur Punkte mit Nummer zählen).
  - `candidates_in` nimmt nur Fließtext (`search::scan::classify`), nicht innerhalb vorhandener Verweise, nicht den
    eigenen Abschnitt.
  - `convert` legt über `Store::save_human_on` eine Fassung auf `rev` an und spiegelt sie mit `mirror::record`.
  - RPC `xref.*`, Seiten `/verweise` und `/verweise/act` (Template `templates/xrefs.html`).
- **Widgets:**
  - `jump.ts` (ohne Importe; reine Funktionen `pushJump`/`popJump`/`forwardJump`, `byteToUtf16`, `locate`, `findOf`
    unter Node geprüft): `a[data-jump]`, Verlauf in `sessionStorage` `baruch_jumps`, Sprungleiste, `baruch:find {text,
    at}` nach dem Laden und nach `baruch:rendered`.
  - `workspace.ts` meldet nach jedem Neuzeichnen `baruch:rendered`; Alt+← läuft über `jumpBack()`, wenn es einen
    Verlauf gibt.
  - `xref.ts`: Dialog über `baruch:xref-open` aus `ribbon.ts`, `localStorage` `baruch_xref_page`, Strg+Klick und
    „Öffnen | Bearbeiten“ (Menü fest im Dokument, nicht in der Satzansicht).
  - Das Band `ribbon::formats` hat `cmd = "xref"`.

## Lösungsvorschläge und Varianten (Runde 30b) {#solution-proposals-and-variants-round-30b}

- **Lösungsform** (`assist/solutions.rs`): `parse_solutions` liest `statt: „…“` + `besser: „…“`, `Schritt: …` und die
  Kurzform `statt „…“ besser „…“` (auch mitten in einer Zeile, auch nummeriert); `is_solution_line` erkennt eine
  Lösungszeile. `findings::parse::split_answer` hängt Lösungszeilen an den Befund darüber (dasselbe wie eingerückt,
  auch nach einer Leerzeile – nur eine andere Zeile schließt den Befund ab), nimmt sie aus dem Befundtext und liest
  den Schlussblock „Mein Vorschlag:“ nicht als Befunde; `finding_spans` liefert die Bytes je Befund einschließlich
  Lösungszeilen (für die Blöcke im Chat, Runde 30c). `findings::ingest` speichert die Lösungen je Befund
  (`Store::finding_set_solutions`, Spalte `finding.solutions` ab 0047); Fehler dabei nur auf stderr, der Befund bleibt.
- **Pflicht zu Lösungsvorschlägen** (`assist/prompts.rs`, `assist/mod.rs`): `SOLUTION_RULE` steht in jeder
  Voreinstellung. `answer_call` fragt je Auftrag nach der Pflichtfeststellung (Nathan) einmal nach, wenn
  `missing_solutions` > 0 (`Retries { critical, solution }`, Grund `SOLUTION_RETRY_REASON`); die zweite Antwort zählt
  nur, wenn ihr nicht mehr Lösungen fehlen und sie die Pflichtfeststellung behält. `mark_missing_solutions` setzt dann
  „(ohne Lösungsvorschlag)“ ans Ende der ersten Zeile jedes kritischen Punkts ohne Lösung (vom Modell geschriebene
  Vermerke werden entfernt und neu geprüft; idempotent). Abgeschnittene Antworten: weder Nachforderung noch Vermerk.
  Der Vermerk wandert beim Zerlegen nach `quote_check`.
- **Übernehmbar und Einfügen**: `locate` sucht das Zitat wörtlich, sonst mit beliebigem Leerraum (Regex nur aus
  maskierten Wörtern, `size_limit`); `apply_solutions` fügt der Reihe nach je das erste Vorkommen ein und meldet ein
  fehlendes Zitat. `post_solutions(body, shown)` ist der Haken in `view::posts` (`post.solutions`).
- **RPC** `solution.*` (`assist/solutions.rs::call`, von `rpc.rs` weitergeleitet): `adopt` legt über
  `Store::add_proposal_on` (Grundlage = gezeigte Fassung oder die Grundlage des Beitrags `PostMeta::basis_rev`, bei
  einem Befund die aktuelle) einen Vorschlag mit der Herkunft der Persona an und schlägt über `findings::rpc::propose`
  „übernommen in Fassung n“ vor (nur Befunde dieses Abschnitts). `reject_siblings` verwirft unter der Schreibsperre die
  anderen offenen Vorschläge der Gruppe (`Store::reject`, kein Spiegel-Commit, wie `revision.reject`). `compare`
  liefert je Variante den Zeilendiff zur Grundlage und zu den anderen als maskiertes HTML (`diff_html`,
  `baruch_core::line_diff`). Seiten: `GET /varianten` (eigene Umgebung mit `base.html`, `variants.html`; ohne `key`
  bestimmt `Store::variant_group_key` den Abschnitt) und `POST /loesungen/act` (Paare aus dem Formular,
  `picks`/`findings` mehrfach, 303; Fehler → 200 mit `role="alert"`), hinter `guard::same_origin`.
- **RFC** (`solutions::rfc_proposal`, Haken in `chat.rs`): `proposal_block` liest die Lösungen nach „Mein
  Vorschlag:“; höchstens drei übernehmbare werden in die Grundlage eingefügt, ein Vorschlag (Vermerk „Mein Vorschlag
  von @lektor (Beitrag n)“); `[B<id>]` im Block ergeben Statusvorschläge.
- **Grundlage** (`assist/basis.rs`): `choose_in` wählt shown/current/n; "shown" auf einer aktuellen Fassung mit nur
  einer Überschrift (`heading_only`) nimmt den neuesten Vorschlag mit Text, ein ausdrückliches "current" bleibt mit
  einem Hinweis in der Kopfzeile. `assist/rpc.rs::order` liest `basis` für ask/review (nicht propose), `run_inputs`
  wählt sie für Aufträge ohne Grundlage (MCP `ask_persona`, Fäden, Einfälle) – die Adresse bleibt ohne `rev`, solange
  die aktuelle Fassung geprüft wird. `Outcome.basis` (JSON `basis` für ask/review) und `PostMeta::basis_label` tragen
  sie. Bei einer Fassung mit nur einer Überschrift sagt der Auftrag `HEADING_ONLY_TASK`.
- **Kontext** (`assist/context.rs`): `outline_block` mit `[key:N]`; bei „Chat nur Abschnitt“ steht er dort als Umfeld,
  sonst wird er (zusammen mit `JUMP_RULE` im Systemteil) nur angefügt, wenn danach noch `OUTLINE_SLACK` Zeichen frei
  sind – er verdrängt keinen Text und löst keine Zerlegung in Stücke aus. Befunde anderer: der Lektor unter
  `FINDINGS_TITLE`, alle anderen unter `KNOWN_TITLE` mit `KNOWN_RULE` (nicht kommentieren). `OrderFrame.relay` steht
  gerahmt und entschärft (`relay_line`, Art `antwort`, zählt nicht zum Korpus) mit `RELAY_TASK`. `RFC_TASK` verlangt
  „Mein Vorschlag:“.
- **Varianten** (`assist/variants.rs`): Lektor nach Einstellung, Befunde benannt (nur dieses Abschnitts) oder die
  offenen des Abschnitts (ohne Lob, ohne angehängte), höchstens `MAX_VARIANT_FINDINGS`; ohne Befunde eine ehrliche
  Ablehnung. Je Variante ein eigener `ModelCall` (System: Lektor-Prompt, `OTHERS_RULE`, `VARIANT_RULE`; Befunde mit
  Lösungen gerahmt; Richtung `VARIANT_STYLES[k]` und die bereits geschriebenen Zeilen), nacheinander über `ask_model`.
  Passt der Abschnitt samt Antwort nicht in die Kontextlänge, gehen nur die betroffenen Absätze (Zitat eines Befunds
  oder einer Lösung, nie die Überschrift) mit Markern `[[Absatz n]]` hinaus (`PARTIAL_RULE`); `splice` fügt die
  Antwort ein. Gespeichert wird erst am Ende unter einer Sperre (`add_proposal_on` mit `VariantInfo`, Gruppe
  `new_uuid`). Chat-Auftrag: `is_variants_order` (Wort „Varianten“, keine Frage). Fassungsbaum: `revision_variant`
  (JSON mit `title`), Beschriftungen in `revtree.html`, „übrige verwerfen“ in `revisions.html`.
- **Templates**: `fragments/finding_block.html` (Befund als Block: Art mit Zeichen und Wort, Status mit Schaltflächen
  an `/werke/befunde/act`, Lösungen, „Als Vorschlag übernehmen“), `fragments/post_solutions.html` (Lösungen eines
  Beitrags, „Lösungen umsetzen“), `fragments/basis_select.html` (in `assist_scope.html`). Dafür tragen die Karten
  (`findings::view::card`) `body_html` (über `chat_render::markdown_html`), `solution_views`, `adoptable`, `work`,
  `back`; die Matrix zählt `solved`/`unsolved`.
- **Austausch-Nebenversion 5**: siehe `docs/Austauschformat.md` §6; nach dem Lesen setzt `exchange_apply::apply_u5`
  Varianten, Metadaten und Lösungen auf den neu angelegten Zeilen (Ids oberhalb des Stands vor dem Lauf) und ruft
  `baruch_typst::xref::remap_keys` mit der Zuordnung `schluessel` der Datei → Kennung → hiesiger Schlüssel. Beim
  Abgleich (`apply_merge`, Schritt (c)/(d)) schreibt Baruch die Schlüssel im Text einer Fassung um, **bevor** er sie mit
  der lokalen vergleicht (dieselbe Zuordnung über alle Punkte des Werks): Eine Fassung, die sich nur in den Schlüsseln
  der Querverweise unterscheidet, ist nicht abweichend; eine wirklich abweichende Fassung (abgeleitete Kennung) wird
  mit den lokalen Schlüsseln angelegt, weil `apply_u5` sie über die Kennung der Originalfassung nicht erreicht.

## Chat: Wiederholen, Löschen, Anrede, Live, strukturierte Beiträge, Sprungmarken (Runde 30c) {#chat-retry-delete-address-live-structured-posts-jump-marks-round-30c}

**Daten.** Je Beitrag im Abschnittschat (`post`) und im Faden (`chat_thread_post`) Metadaten aus Migration 0047
(`baruch-store/src/post_meta.rs`): `basis_rev`/`basis_label` (geprüfte Fassung und Kopfzeile), `reply_to`,
`repeat_of`, `relay_from` und `request` (`PostRequest {persona, mode, scope, text, rev}`, lokal, nicht im
Austauschformat). `assist::save` schreibt Persona-Beiträge im Abschnitt über `Store::add_post_meta` (aus
`Order::links`/`Order::basis`), `threads::answer` im Faden über `Store::thread_add_post_meta` (eine Transaktion,
`scope = "thread"`, Auftragstext = Nachricht des Absenders). `Store::thread_post_delete` löscht einen Fadenbeitrag;
Verweise darauf werden durch die Fremdschlüssel NULL (`ON DELETE SET NULL`), Verlauf und Befundverweise folgen 0046.

**Anrede und Relais** (`assist/chat.rs`): `addressed(message, personas)` = `@handle` irgendwo plus Name/Handle ohne
"@" am Satzanfang (Anfang des Beitrags, nach einem Zeilenumbruch, nach "."/"!"/"?" mit Leerraum) mit ","/":";
`mentions` bleibt für das MCP-Relais und vorhandenen Code. `line_addresses(answer, personas)` = Anreden am Zeilenanfang
einer Antwort (auch nach Listenmarkern und `**`, nicht in "> "-Zitaten). `chat()` sammelt die Antworten der ersten
Runde und ruft dann je Anrede eine Relais-Antwort: `Order { mode: Chat, text: relay_text(..), asker:
anredende Persona, frame.relay: deren Antwort gerahmt, links: {reply_to, relay_from} }`. Grenzen `MAX_RELAY_ANSWERS`
(3), jede Persona einmal (`called` deckt alle gerufenen Personas einschließlich gescheiterter), eine Stufe, nicht bei
einem RFC oder Schreibauftrag. Fäden (`threads::chat`) nutzen dieselben Funktionen; das MCP-Relais aus Runde 15
(jede Erwähnung, Gesamtgrenze 3) bleibt daneben bestehen (seit dem Nachtrag nach R30 in derselben Stufe und gleich
gerahmt, siehe „Relais“ unter Runde 15), `mcp/tools.rs` `relay_section` rief danach im Abschnitt nur Personas, die
noch nicht dran waren (seit Runde 23 entfernt: ein Relaisweg `assist::chat::relay_targets`).

**Wiederholen** (`posts::repeat`, `threads::repeat_post`): Auftrag aus `PostMeta::request` (Rückfall:
`chat_render::repeat_fallback` oder `threads::thread_fallback` – vorangehender Beitrag eines Nicht-Persona-Autors),
Grundlage über `assist::basis::choose`, Ausführung über `assist::run` oder `threads::answer` mit `links.repeat_of`.
Danach `posts::propose_done`: offene, nicht lobende Befunde des alten Beitrags, für die `findings::parse::split_answer(neue
Antwort)` kein `similar` liefert, bekommen den Vorschlag `Done` über `findings::rpc::propose` (Herkunft die Persona,
je ein Commit). Verglichen wird der Text der neuen Antwort, nicht die neu angelegten Befunde – `findings::ingest`
hängt Wiederholungen eines Befunds an den alten (`similar_to`).

**Löschen:** Abschnitt wie zuvor `posts::act(Op::Delete)` (ein Commit `post-delete`); Faden `threads::delete_post` unter
der Schreibsperre mit `Actor::only_own` (MCP: `delete_post_as`), Commit über `mirror::record_request` mit `allow_empty`
(Protokoll, der Baum ändert sich nicht).

**Live** (`chat_live.rs`): prozessweite Tabellen im Speicher je (Werk, Abschnitt oder Faden): `typing()` /
`typing_thread()` melden an und liefern einen `TypingGuard`, dessen `Drop` abmeldet (Ende, Fehler oder Abbruch der
Future); `assist::run_inputs` und `threads::answer` halten ihn während des Auftrags. `record_failure()` merkt sich
gescheiterte Aufrufe (aus `chat::fail`, `routes::remember_failure` für Einzelaufträge, `threads::answer`,
`posts::repeat`), höchstens fünf je Ort, zehn Minuten. `post.since`/`thread.since` rendern neue Beiträge mit denselben
Makros wie die Seite (`fragments/post_item.html`: `post_entry` oder `thread_entry`, über `Environment::render_str`),
dazu `typing_now` und `failures_now`. `web/src/chat.ts` (`startLive`) fragt alle 3 s ab, solange das Akkordeon offen
und die Seite sichtbar ist, und sofort bei `baruch:chat-refresh` (assist.ts am Ende eines SSE-Stroms); threads.ts
nutzt dieselbe Funktion.

**Darstellung** (`chat_render.rs`): `markdown_html` (Signatur fest, auch für das Befund-Akkordeon und die Matrix)
zerlegt in Blöcke (Absatz, Liste mit Unterpunkten, Zitat, Überschrift) und einen Fuß (Schlussvermerke von Baruch:
`scripture::MARK_*`, `CUT_NOTE`, `TRUNCATED_NOTE`, „Komprimiert:“, `CRITICAL_MISSING_NOTE`); Inline nur `**`/`*`,
alles über `bibles::html_escape`. `render_post` legt Links auf Byte-Bereiche der Zeile (Auftragsmarker →
Einfall/Befund → Bibelstellen → Zitate → `xref::outline_mentions`, keine Überlappung) und maskiert den Rest;
Sprungmarken prüft `xref::check_marker` (die Fassungsnummer nur für den eigenen Abschnitt, sonst die aktuelle),
Sprünge baut `xref::jump_url` (`data-jump data-jump-origin="chat"`), Bibelstellen `baruch_scripture::scan` (nur
`Valid`, Ziel `/literatur/bibel?work&ref`, ohne `module` wählt die Seite die Bibel des Werks). Befunde:
`findings::parse::finding_spans` liefert Byte-Bereiche je Ort; ein Bereich wird zu `Segment::Finding`, wenn ein
gespeicherter Befund dieses Ortes (`Finding::position`) zum Beitrag gehört, seine Karte (`findings::view::card`) steht
in `PostExtra::finding_cards` für das Makro `finding_block`. Der Chat ruft es als `finding_block(card, seg.html)`
(Integrator Runde 30): Der Befundtext im Block ist dann das Segment mit den geprüften Links aus `post_html`, nicht
noch einmal `card.body_html` – so steht der Text genau einmal dort. `PostExtra::plain` = Klartext für „Kopieren“.
`post_extra` (Haken aus `view::posts`) nimmt Befunde und Metadaten aus `ViewExtras`; die Seiten `/beitraege`,
`/beitrag`, `/faden` und `post.since` laden beides selbst (`posts::page_extras`, `threads::entry_values`).

**Templates:** `fragments/post_item.html` mit den Makros `post_item(post)` (Kopf und Text, ohne Seitenkontext),
`post_entry(post, at)` (ganzer Beitrag des Abschnittschats einschließlich ↻/✕/Verwalten, `at = {work, key, node, rev,
view, lq, targets}`) und `thread_entry(e, at)` (`at = {work, thread}`); `assistants.html`, `thread.html`, `posts.html`,
`post.html` binden sie ein. Die Makros `finding_block` und `post_solutions` (chat-loesungen) werden aus
`post_item.html` heraus importiert.

## Runde 29: Satz in den Einstellungen {#round-29-typesetting-in-the-settings}

Schnittstelle R29-S1: Datenmodell `baruch_typst::typeset` (`model.rs` eingefroren: `TypesetSettings`, Grenzen,
`validate`/`normalized`/`changed_fields`/`to_json`/`from_json`, `FIELDS`), Regel-, Vorlagen-, Vorschau- und
EPUB-Funktionen als Stubs (`rules` liefert ""), Store-API `baruch_store::typeset` (Lesen ohne Zeile → `None`, Schreiben →
`StoreError::Typeset` „noch nicht umgesetzt“), Haken in `snapshot::head` (Regelblock nach Werkvorlage, Schema und
Rückfallregeln; ohne Einstellungen byte-identisch), Servermodule `typeset_settings` (RPC `work.typeset_*`),
`mcp::typeset` (Werkzeug `typeset_settings`, nur lesend), `settings::work_typeset` (Block „Satz“, Routen,
`work.typeset_preview`) und `typeset::cached_page`. Reserviert: Store 0048 satz-daten, 0049 satz-typst, 0050
satz-oberflaeche, 0051 Integrator; Bibliothek 0108 satz-daten, 0109 Integrator.

### Daten (satz-daten) {#data-satz-daten}

- **Migration 0048** (`crates/baruch-store/migrations/0048_work_typeset.sql`, die einzige neue Store-Migration der
  Runde; 0049–0051 und Bibliothek 0108/0109 bleiben ungenutzt): Tabelle `work_typeset (work_id PRIMARY KEY → work ON
  DELETE CASCADE, uuid, settings, updated, updated_by)` – höchstens eine Zeile je Werk, keine Zeile = keine
  Satz-Einstellungen. `settings` ist das kanonische JSON von `TypesetSettings::to_json` (CHECK: JSON-Objekt, höchstens
  16384 Zeichen; die genaue Prüfung macht `TypesetSettings::validate` im Code). `uuid` wie 0045/0046 (eindeutiger Index,
  Füll-Trigger `work_typeset_uuid_fill`, Grabstein-Trigger `work_typeset_uuid_tombstone`, Kaskaden-Trigger
  `work_typeset_cascade_uuid_tombstone` vor dem Löschen eines Werks, damit der Grabstein die Werk-uuid kennt).
  `uuid_tombstone` bekommt die Art `work_typeset` durch den Umbau wie 0046 (sechzehn Grabstein-Trigger löschen,
  temporäre Kopie, Tabelle mit erweitertem CHECK, Trigger wörtlich neu); kein `FOREIGN_KEYS_OFF`, weil kein Fremdschlüssel
  auf `uuid_tombstone` zeigt (Beleg in `tests/migration_0048.rs`).
- **Store** `baruch_store::typeset` (Vertrag §4): `work_typeset` (lesen; unbekanntes Werk → `UnknownWork`),
  `set_work_typeset` (normalisieren, prüfen – ungültig → `StoreError::Typeset` mit allen Meldungen, nichts geändert;
  gleiche Werte → kein Schreiben; leere Einstellungen = Zurücksetzen; Ändern per Upsert, die uuid bleibt),
  `reset_work_typeset` (Zeile löschen → Grabstein), `adopt_typeset_rule` (Einstellungen und eine neue Fassung der
  Werkvorlage einschließlich `work.template`/`work.updated` in einer Transaktion `BEGIN IMMEDIATE`; Vorlage mit
  Überschrift oder unverändert → `Refused`) und `typeset_in` für den Haken in `snapshot::head`. Dort steht der
  Regelblock nach Werkvorlage, Schema und Rückfallregeln – und so gleichermaßen in Export, Exportkarte, Spiegel
  (`main.typ`), Satzvorschau, PDF und Umschlag; ohne Einstellungen ist der Export byte-identisch mit dem Stand R29-S1
  (Golden `crates/baruch-store/tests/data/satz_golden_*.typ`).
- **Server** `crate::typeset_settings`: `load`/`view_json` (gemeinsam für RPC, MCP und den Block „Satz“; Hinweis auf
  eine unbekannte Grundschrift über `baruch_render::font_families`), `save`/`reset`/`adopt_template_rule` unter der
  Schreibsperre mit genau einem Spiegel-Commit je Änderung (`mirror::describe_typeset`/`record_typeset`, Trailer
  `Baruch-Action: typeset-set|typeset-reset|typeset-adopt`, immer `allow_empty`); RPC `work.typeset_get|set|reset|
  adopt_rule` (`docs/RPC.md`), MCP `typeset_settings` nur lesend (`crate::mcp::typeset`, `docs/MCP.md`).
- **Austausch** Nebenversion 6 (`docs/Austauschformat.md`, Abschnitt "Typesetting settings"): `ExchangeTypeset` im
  Modell (mit `TypesetSettings` aus `baruch-typst`, weshalb `baruch-exchange` nun von `baruch-typst` abhängt), Wörter je
  Feld in `baruch_exchange::typeset`, Grabstein-Art `satz` ↔ `work_typeset`, XSD `docs/schema/baruch-werk-1-u6.xsd`.
  `exchange_dump` schreibt die Zeile mit Kennung und letzter Änderung; `exchange_apply` legt sie bei einem neuen Import
  an, beim Abgleich nur, wenn sie hier fehlt und kein Grabstein existiert – sonst ein Hinweis mit beiden Ständen, nie
  überschreiben. Die Austauschseite zählt sie (`counts.typeset`, `typeset_fields`), die Vorschau des Imports nennt
  „Satz-Einstellungen“ in der Liste der hinzukommenden Stücke.

### Typst-Regeln (satz-typst) {#typst-rules-satz-typst}

- **Regelblock** `baruch_typst::typeset::rules(&s)` (`rules.rs`): "" ohne Einstellungen (auch wenn nur
  `xref_with_page` gesetzt ist – der Schalter ändert keine Datei), sonst die Zeile `// Baruch: Satz`, dann je Feld eine
  Zeile in der Reihenfolge von `FIELDS`, endend mit `\n`. Der Store hängt ihn in `snapshot::head` nach Werkvorlage,
  Schema und Rückfallregeln an; Export, Exportkarte, Spiegel, Satzvorschau, Buch-PDF und Umschlag sehen dadurch
  dieselben Regeln. `chapter_start` → Show-Regel `#show heading.where(level: 1): it => { pagebreak(weak: true[, to:
  "odd"]); it }`, das ist genau die übernehmbare Vorlagenregel; sie gilt auch für Teilüberschriften
  (`<baruch-part>`, Ebene 1 im Export), damit „Vorlagenregel übernehmen“ den Umbruch vor Teilen im Export nicht
  verliert (Befund R29, Test `adopting_the_template_rule_keeps_every_heading_page_in_the_export_form`). Im Satz wirkt
  die Teilregel davor; `continuous` ist nur eine Kommentarzeile. `page_format`/`margins` → `#set page(width/height)`
  oder `margin: (inside: innen + Bundsteg, outside, top, bottom)`; Schrift/Größe/Zeilenabstand/Blocksatz/Silbentrennung/
  Sprache/Schusterjungen → `#set text`/`#set par`; Einzug → `first-line-indent` und Absatzabstand = Zeilenabstand
  (Typst-Voreinstellung 0,65 em, sonst `leading_em`); Fußnoten → `#set footnote(numbering: "1"|"*"|"a")` und je Kapitel
  `counter(footnote).update(0)` in einer Show-Regel.
- **Seitenzahlen und Kolumnentitel** sind `context`-Kopf- und Fußzeilen mit den Hilfsfunktionen `baruch-satz-chapters`,
  `baruch-satz-opening(p)` und `baruch-satz-quiet(p)` (Teil- und Leerseiten). „Ab dem ersten Kapitel“ setzt
  `counter(page).update(1)` in den **Seitenhintergrund** der ersten Kapitelseite: Typst setzt alles, was eine
  Show-Regel liefert, zwischen Start- und Ende-Marke der Überschrift; ein Zurücksetzen dort käme zu spät für
  `counter(page).at(<kN>)` (Querverweis „S. n“). Der Hintergrund steht im Seitenrahmen vor Kopfzeile und Inhalt
  (gemessen in `baruch-render/tests/typeset_measure.rs`). `pagebreak(to: "odd")` und `inside`/`outside` zählen
  physische Seiten.
- **Leerseiten erkennen:** Eine Show-Regel setzt die Marke `<baruch-satz-end>` vor jede Überschrift der Ebene 1 – vor
  den Umbruch der Einstellung oder der Vorlage. Typst lässt geschlossene Marken vor dem Umbruch, wo sie sind, und
  verschiebt nur nicht geschlossene Startmarken (`migrate_unterminated_tags`); Seiten zwischen Marke und Überschrift
  sind leer. Steht nur die Marke vor dem Umbruch, setzt Typst sie an den Anfang der Folgeseite (Position 0 pt) – das
  zählt ebenfalls als leer. Die leere Rückseite einer Teilseite trägt `<baruch-satz-blank>`. Die Teilregel des Satzes
  wirkt vor dieser Show-Regel und gibt `it` nicht zurück; sie setzt die Marke deshalb selbst, sobald es einen
  Regelblock gibt (`part_page_rule_for`, auch ohne die Einstellung `part_page` – Befund R29: sonst trug die Leerseite
  vor einem Teil Seitenzahl und Kolumnentitel; Test `the_blank_page_before_a_second_part_stays_empty`). Ohne
  Satz-Einstellungen bleibt die Regel von Runde 14 byte-identisch. Messende Tests erkennen Leerseiten an der leeren
  Satzfläche, nicht an einer völlig leeren Seite (sonst ginge eine Leerseite mit falscher Nummer als Textseite durch).
  Ohne Kapitelregel gibt es keinen Umbruch: Die Marke steht dann an derselben Stelle wie ihre Überschrift und folgt
  ihr in der Abfrage; eine solche Marke zählt nicht (Integrator R29 – vorher zählten alle Seiten bis zur nächsten
  Überschrift als leer, ohne Seitenzahl und Kolumnentitel; Test
  `without_a_chapter_rule_every_text_page_keeps_number_and_head`).
- **Teilseiten:** `typeset::prepare` liest `state.store.work_typeset(work)` und setzt `part_page_rule_for(labels,
  &settings)`: mit `part_page` = `part_page_rule_with(labels, Some(part))`, mit einem Regelblock ohne `part_page` =
  rechte Seite ohne leere Rückseite (wie in Runde 14) einschließlich der Marke `<baruch-satz-end>`, ohne
  Satz-Einstellungen byte-identisch zu `marker::part_page_rule` (Stand und Seitenzahl wie vor Runde 29, Golden-Stand in
  `tests/round29_typst.rs`).
- **Werkvorlage** (`template.rs`): Gelesen wird eine Kopie ohne Kommentare (gleiche Byte-Positionen).
  `template_conflicts` meldet je gesetztem Feld die Argumente von `set page|text|par|footnote(…)` (auch im Code von
  Vorlagenfunktionen) mit Zeile und Text, Wirkung `overrides`; Show-Regeln auf Überschriften der Ebene 1 mit
  `pagebreak` gegen `chapter_start` sind `ineffective`, es sei denn, die Vorlage will dasselbe oder nur eine neue Seite
  mit „rechte Seite“ (Typst wendet die zuerst definierte Show-Regel zuletzt an, ihr Umbruch kommt also hinzu).
  `detect_chapter_break` erkennt nur Regeln, die ausschließlich aus Umbruch und Überschrift bestehen (Leerraum,
  mehrzeilig, Code- oder Inhaltsblock, mit/ohne `weak`, `to: "odd"` → rechte Seite, ohne `to` → neue Seite, `to:
  "even"` → nicht übernehmbar), damit `remove_rule` nichts anderes entfernt.
- **Satzseite:** `typeset::template_hints` liefert Konflikte und die erkannte Regel für `#druckpruefung` (Link
  `/einstellungen/werk?work=W#satz`).
- **Umschlag:** `cover::build_wrap` misst die erste Seite des Buchsatzes (das ist das Satzformat) und zählt alle Seiten
  einschließlich Leerseiten für den Rücken. Für die Umschlagseiten im Buch misst `cover::frame` die Werkvorlage
  einschließlich Regelblock (`cover::page_template`); Blöcke ohne Schrift nehmen `cover::book_font` (Einstellung,
  sonst Werkvorlage), ebenso `cover_layout::default_font` und die Druckprüfung des Umschlags.
- **Vorschau** `preview_source(s, template, title)`: Vorlage, Regelblock, Teilregel (`part_page_rule_for`) und ein
  erfundener Mustertext (ein Teil, drei Kapitel mit Abschnitten und je einer Fußnote, ≥ 6 Seiten A5).
- **EPUB** `epub_style(&s)` (`epub.rs`): CSS für `section.baruch-chapter`/`section.baruch-part` (Umbrüche), `p`
  (Einzug oder Abstand, Blocksatz, `hyphens`, `widows`/`orphans`), `body` (Schrift, Größe relativ zu 11 pt,
  Zeilenhöhe), `ol.baruch-footnotes` (Nummerierung); `split_chapters` immer `true`, `lang`. Noch nirgends
  eingebunden (R20a).
- **Messhilfe** `baruch_render::page_texts(&Source) -> Option<Vec<Vec<PlacedText>>>`: je Seite die Textstücke (Text,
  Grundlinie links in pt von links oben, Breite, Größe, Schrift) mit den Transformationen der Gruppen. Typst setzt
  einen Trennstrich als eigenes Stück mit dem Text U+00AD; Satzzeichen hängen ≈ 1,9 pt in den rechten Rand.

### Oberfläche (satz-oberflaeche) {#user-interface-satz-oberflaeche}

- `src/settings/work_typeset.rs`: Block „Satz“ (`block` → Template `templates/settings/werk_satz.html`, Kontext aus
  `block_context`, das nur `typeset_settings::TypesetView`, die Werkvorlage und das abgelehnte Formular braucht – so
  lässt er sich in einem Unit-Test ohne Server zeichnen), Formulare `act`/`adopt_act`, Vorschaubilder `preview_svg`,
  RPC `work.typeset_preview` (`call`, mit der Formularvorschau über `preview` geteilt).
- Formularfelder ↔ Einstellungen: `settings_from_form` (Vertrag §6.3) und die Umkehrung `form_from_settings`
  (Rundreisetest mit dem Vertragsbeispiel). Keine eigene Grenzprüfung: Zahlen, die keine Zahlen sind, werden zu `NaN`,
  damit `TypesetSettings::validate` sie mit derselben Meldung ablehnt wie über RPC; nur „unbekannte Wahl“ und
  unvollständige Ränder kommen hinzu. Meldungen sind `(Feld-Id oder Unterfeld, Text)`; das Template zeigt sie am Feld
  (Kopf vor dem Punkt), und so auch Konflikte aus `template_conflicts` (`data-typeset-error`, `data-typeset-hint`).
- `Retry` (in `WorkRetry::Typeset`) trägt Eingabe, Meldungen und – nach `action=preview` – die `Preview`. Abgelehnte
  Formulare und die Vorschau ohne JavaScript nutzen denselben Weg: `settings::work_retry_page` zeichnet die ganze Seite
  „Werk“ (Status 422 oder 200), der Block zeigt dann die ungespeicherten Werte, ihre Regeln und Konflikte.
- Vorschau: `preview_source` + `export_assets` → `typeset::compile` (Sperre `RUNS`, `COMPILE_TIMEOUT`, legt den Stand
  in den Cache); `preview_svg` liest nur `typeset::cached_page` und startet nie einen Lauf (Schutz vor Satzläufen beim
  Aufruf anderer Seiten: `guard::cross_site_get` → 403).
- Widget `web/src/typeset_settings.ts` („typeset-settings“ am Formular): `formToSettings` liest das Formular wie
  `settings_from_form` (ein Test unter Node vergleicht beide), `debounce` (Timer nachbildbar) und `Newest`
  (AbortController; späte Antworten werden verworfen). Die Schaltfläche „Vorschau“ setzt sofort, ohne abzusenden;
  „Speichern“/„Zurücksetzen“ bleiben Formulare (im Popup sendet `settings.ts` sie ab).
- `web/src/xref.ts`: `initialPage(vorhanden, Werk-Einstellung, Browser)`; der Dialog ruft `work.typeset_get` ab und
  nimmt `settings.xref_with_page`, solange das Kästchen unberührt ist; `localStorage` (`baruch_xref_page`, try/catch)
  nur, wenn die Werkeinstellung nicht gesetzt ist.
- Stil `static/typeset_settings.css` (in `STYLE`). Rückkehrhinweise `ok=satz|satz_zurueck|satz_vorlage` in
  `settings/work.rs` (`notice`).

### Integration (Runde 29) {#integration-round-29}

- Zusammenführen `team/satz-daten` → `team/satz-typst` → `team/satz-oberflaeche` mit `--no-ff`, ohne Konflikte.
- Abnahme `crates/baruch-server/tests/round29_abnahme.rs` (erfundenes „Fährhausbuch“ mit Vorspann, zwei Teilen, vier
  Kapiteln, Fußnoten und einem Querverweis mit Seitenzahl): Formular → `work_typeset` → Haken `snapshot::head` →
  Satz und Buch-PDF aus derselben Quelle (gleicher Stand), gemessen mit `baruch_render::page_texts` und
  `pdfinfo`/`pdftotext` (A5, Kapitel rechts, Seitenzahlen außen ab dem ersten Kapitel auf jeder Textseite,
  Kolumnentitel, Bundsteg, „S. n“), Umschlagseitengröße und Rücken, jede Einstellung einzeln durch den Server,
  Übernahme der Vorlagenregel (Kapitelseiten vorher = nachher), Konflikt `#set page(paper: "a4")`, MCP nur lesend,
  Austausch u6 mit Grabstein über drei Instanzen.
- `typeset_settings::load` meldet zusätzlich `NO_DOCUMENT_TITLE`, wenn der Kolumnentitel „Werktitel“ gewählt ist, die
  Werkvorlage aber kein `#set document(title: …)` hat (der Kolumnentitel bliebe leer).
- Die Austauschseite (`templates/exchange.html`) nennt die Satz-Einstellungen (`counts.typeset_fields`).

## Runde 19 – RAG-Pflege, Rest der Verweise {#round-19--rag-maintenance-rest-of-the-references}

### R19a Index {#r19a-index}

Ohne Migration (Präfix im JSON der Verbindung, Sperrvermerk und Marke in `sync_state`). Einzelheiten unter
„Semantische Suche (Runde 12, Migrationen 0031 und 0032)“ (Dienst) und im Abschnitt zum Abgleich (`sync/`):

- `rag/index.rs`: `Runtime` mit `enqueue` (laut, holt Liegengebliebenes zurück) und `enqueue_quiet` (Suche), `stale()`,
  `wish_reset`/`reset_wished`/`apply_reset`, `unbound`; `process` → `Outcome {embedded, follow}`; `sync` mit Bereich
  `replace`, Präfixen und Abbruch; `PREFIX_CHANGED`.
- `rag/mod.rs`: `catch_up` ohne Einbettungsdienst für einen eigenen Datenordner ohne gespeicherte Eliezer-Verbindung
  (`NO_EMBEDDER_OWN_DATA`), `index_info` mit `stale` und `stale_text`, `reindex_report` → `Reindexed {pending,
  deferred}`, `answer_json` mit `weak`.
- `baruch-lancedb`: `Scope::contains`, `Index::replace(scope, …)`, `IndexMeta.document_prefix`.
- `sync/`: `Link::answered` (über `multigpt::Session::broken`), `Writer::blocks`/`chapter`/`restore`, Marke
  `UPLOADING`, `Filler::number` liefert den gesetzten Wert, Vollständigkeit der Liste in `download::list`.
- `connections/check.rs`: `multigpt` beendet die Sitzung mit `McpClient::delete` (nicht, wenn die gemeinsame Zeitgrenze
  von 10 s die Prüfung abbricht).
- Offen: ältere Fassungen und Befunde im Index (Nutzerfrage 1), Kommentare/Befunde an MultiGPT (Nutzerfrage 11); ob das
  echte MultiGPT eine Dokument-Id nennt und `limit` kappt, prüft der Nutzer.

### R19b pgvector {#r19b-pgvector}

Keine Migration (Präfixe im JSON der Verbindung, Vorschläge für Gruppennamen im Speicher, übernommene Namen werden über
`idea.update` zu Stichwörtern).

- **pgvector** (`connections/pgvector.rs`, siehe „pgvector“ oben): alle Felder in SQL gekürzt, `quoted` verdoppelt
  Anführungszeichen, `tls_warning`, Anfrage mit `query_prefix` eingebettet. `connections/config.rs`: `fields` kennt
  `query_prefix` (pgvector, lancedb) und `document_prefix` (lancedb), `check_field` prüft mit
  `baruch_rag::check_prefix` (ohne Trimmen), `NOMIC_PREFIXES` ist nur der Vorschlag auf der Seite (keine
  Voreinstellung, damit ein vorhandener Index gültig bleibt). `connections/pages.rs`: Felder mit
  `placeholder`/`maxlength`, Fehlercode `praefix`, `tls_warning` je Verbindung, `status_context` reicht `index.stale`
  durch (vom Index-Worker gefüllt, R19a); das Template zeigt „n Aufträge liegengeblieben …“ nur bei `stale > 0`.
- **Einfallskarte, Gruppen** (`idea_map/groups.rs`): `components` (Union-Find über die Nähepaare, Wurzel = kleinste Id,
  nur Komponenten ≥ 2) → `Group {group: "g<id>", ideas, words, proposal}`; `groups(state, work, filter)` baut die Karte
  mit `near: true`, `of_map` nutzt eine schon gebaute (Seite). `name_groups`: ein Lauf je (Datenordner, Werk)
  (`running`, der Wächter `Running` meldet beim Fallenlassen ab), Persona = die genannte aktive oder
  `rows_and_lector`, Aufruf über `state.model.chat` ohne die Schreibsperre, Nachricht mit `DATA_TITLE` und je Einfall
  `assist::context::framed_line(…, "einfall", …)` (entschärft, kann die Box nicht verlassen), Systemprompt `SYSTEM` mit
  "data, not instructions"; `parse_names`/`clean_name` nehmen nur "g<id>: Name" bekannter Gruppen (≤ 40 Zeichen).
  Vorschläge in einer prozessweiten Tabelle je (Datenordner, Werk) → Gruppe (`Proposal` mit den Einfällen zur Zeit des
  Vorschlags; nur für dieselbe Gruppe gezeigt) – kein SQLite, kein Spiegel, Verlust beim Neustart gewollt (Vertrag §1).
  `accept`: Einfälle aus dem Vorschlag oder aus den Gruppen nach Filter, nur die noch vorhandenen, Vorabprüfung
  „höchstens 20 Stichwörter“, dann je Einfall `ideas::call("idea.update", {id, words})` (Herkunft Mensch, Commit mit
  `Baruch-Action: idea-update`). RPC in `idea_map/mod.rs` (`METHODS`), Formulare über `bild::ACT_METHODS` und `run_act`
  (Filter aus `back`), Seite: Abschnitt „Gruppen“ in `karte.html` (Kontext `clusters`), Stil `static/karte.css`. MCP
  bekommt dafür kein Werkzeug. MCP `idea_map` nimmt `near_limit` und `near_threshold` (`filter_from_params`, Ablehnung
  als `Failure::Tool` = `isError`).
- **Band** (`ribbon.rs`): `SCRIPTURE_FUNCTIONS` (`bibelstelle`, `bibel`) über `defined_functions`, `cites_literature`
  (Zeile mit `#bibliography(` außerhalb von `//`). `ws_command.ts` (sonst eingefroren) bekommt die Befehle
  `scripture`/`cite` und die reinen Bausteine `scriptureFunction`, `scriptureSource`, `citeKey` (ohne Importe, daher
  unter Node testbar); `ribbon.ts` sendet `arg = {fn}` für `scripture`, `wysiwyg.ts` fügt eine Insel
  (`span.ws-island[data-raw]`) oder einen Verweis (`button.ws-ref[data-target]`) ein, die `wysiwyg.serialize` wörtlich
  oder als `@schlüssel` speichert (`docs/WYSIWYG.md`).
- **NFC**: `baruch_library::check_range` vergleicht Auszug und Zitat in NFC (Bereich in Zeichen des gespeicherten
  Volltexts, alternativ in Zeichen seiner NFC-Form); `websearch::verify::normalize` beginnt mit `nfc()` (Seite und
  Zitat laufen beide hindurch). Crate `unicode-normalization` 0.1 (S1).
- **Annahmen:** Die Funktion `#bibelstelle`/`#bibel` der Vorlage nimmt die Stelle als Inhalt `[…]`; das Literaturverzeichnis
  steht als `#bibliography(` in der Vorlage (nicht in einer eingebundenen Datei). Beides wird ohne Typst erkannt
  (`ribbon::code_lines`); seit Runde 23 zählen Blockkommentare und die Klammertiefe (nur oberste Ebene), und Funktionen,
  die per `#import "datei.typ": name` eingeführt werden, zählen auch.

### R19c Treffer {#r19c-treffer}

- **Darstellung der Treffer:** Die Schwelle nur in `baruch_rag` (`WEAK_SCORE`, `is_weak`, `WEAK_LABEL`, Schnittstelle S1).
  `literature::rag_view` sortiert stabil nach `weak` und liefert `weak_count`; `fragments/library.html` zeichnet
  schwache Treffer über ein Makro in `<details class="weak-hits">`. `ideas::rag_ideas` sortiert schwache nach hinten;
  `ideas.html` dasselbe Muster. `assist::rag_facts::drop_own_section` und `block_entries` setzen schwache Treffer ans
  Ende (vor die Grenze `LIMIT`) und schreiben `rag_facts::similarity` („Ähnlichkeit (schwach, n %)“).
- **`idea.search` (rag):** LanceDB nicht `Connected` oder ein anderer Fehler als `RagError::Invalid` →
  `rpc::RAG_UNAVAILABLE` (-32001). MCP `idea_search {mode: "rag"}` ruft denselben RPC und macht daraus `isError`.
- **baruch-scripture:** Nach dem Wortvergleich vergleicht `wording::compare` zusätzlich ohne Leerraum
  (`Comparison::spacing`): die Schlüssel der Bibelwörter aneinandergehängt, jeder Teil des Zitats muss von einer
  Wortgrenze des Moduls bis zu einer Wortgrenze passen (kein Treffer mitten im Wort). `plain_words` normalisiert auf NFC
  (`unicode-normalization`), sonst zerfiel ein zerlegtes „ü“ in zwei Wörter. `wording::fragment_survey` schätzt
  Silbentrennungsreste: Paar „A b“ im selben Vers, A ohne Satzzeichen am Ende, b kleingeschrieben, höchstens 5
  Zeichen, kein häufiges Kurzwort, und „Ab“ kommt andernorts im gelesenen Text vor; auffällig ab 3 Fällen und 1 auf 200
  Wortpaare (`FragmentSurvey::fragmented`). `quote_for` erlaubt nun bis zu drei Wörter aus `INTRO_WORDS` mit einem
  Doppelpunkt vor einem Zitat (höchstens 48 Bytes). `count_word_follows`: „Mal“ mit folgender Kapitel-und-Vers-Angabe ist
  keine Zahl.
- **baruch-sword:** `Bible::sample_texts(max, deadline)` (Stichprobe in Modulreihenfolge), `reference::lemma_candidates`,
  `compound_tails`, `strong_like`, `Lexicon::lookup_lemma` (Stichwort, dann `get_strongs` für Strong-Nummern unabhängig
  von `StrongsPadding`, dann Endungen/Umlaut, zuletzt Zusammensetzung; Antwort mit der versuchten Form).
  `fixture::hyphen_remnants` baut Silbentrennungsreste in erfundenen Text (Tests; kein Test liest Fremdmodule).
- **Server:** `bibles::WordingResult.module_spacing`, `SPACING_LABEL`; `bibles::module_note` (Stichprobe von 4000 Versen,
  2 s, Cache je conf-Datei und Änderungszeit, blockierend) und `choice_note`; `scripture::check_with`, `bible.marks`,
  `facts_with` (Zeile „Hinweis: …“) und `bible_line` tragen den Hinweis. `scripture::mark_with`: andere Personas mit
  einer Bibel bekommen `mark_others_bible(Modul)` (beginnt mit `MARK_BIBLE` und zählt dadurch in `chat_render` als
  Schlussvermerk), ohne Bibel `MARK_OTHERS`; `mark_answer_for_work` öffnet die Bibel nur, wenn eine Stelle vorkommt.
  `bibles::german_of` bildet einen Vers des Moduls über `mapping::map_verse` in die deutsche Verszählung zurück;
  `bible.lookup` liefert `german_ref`/`german_problem`, die Bibelseite (`literature/bibles.rs`,
  `literature_bible.html`) setzt sie in `data-ref`/`data-verse-ref` und als Link. `literature::bibles::workspace_shelf`
  (über `view::extras`, Feld `ViewExtras.bibles`) und `Shelf::context_at` bringen den Kasten „Bibeln“ serverseitig auf
  die Werkseite.
- **Annahmen:** Die Schwelle 0.45 gilt für alle Wege gleich, obwohl MultiGPT seine Skala nicht meldet; die
  Fragmentschätzung ist eine Faustregel (sie kann sich bei sehr kurzen Stichproben irren und meldet dann eher nichts).

### Integration {#integration}

- Ohne Konflikte zusammengeführt (Index → pgvector → Treffer, je `--no-ff`). Die Teile greifen über die Schnittstelle
  R19-S1 ineinander: Der Index füllt `IndexInfo.stale`, die Seite „Verbindungen“ (pgvector) zeigt es;
  `config::embed_prefixes` liest LanceDB (Index) und die pgvector-Suche; `baruch_rag::is_weak` gilt gleich in
  `rag.search` (Index) und in Literatur, Einfällen und Belegen (Treffer).
- Eine `EmbedderFactory` für alle Wege: `rag::set_embedders` oder Eliezer liefert die Einbettungsdienste für LanceDB
  und für die Anfrage an pgvector (`rag::plan_way` reicht dieselbe Fabrik an `hooks::open_pgvector`); jeder Weg wählt das
  Präfix selbst aus seiner Verbindung.
- MCP `idea_search`: die Kurzbeschreibung nennt `mode` (Integrator, `mcp/tools.rs`).
- Abnahme `tests/round19_abnahme.rs` (5 Tests, Werk „Die Laternen am Kai“, alles frei erfunden, nur Attrappen).

## Runde 20 – EPUB-Vorschau {#round-20--epub-preview}

### Schnittstelle R20-S1 {#interface-r20-s1}

Neues Crate `crates/baruch-epub` (rein: keine Datenbank, kein Netz, kein Typst-Satz). Eingefroren in `lib.rs`:
`EpubMeta`, `EpubImage`, `EpubSection`, `FootnoteMode` (`popup` Standard, `endnotes`), `EpubInput`, `ItemKind`,
`EpubItem`, `TocEntry`, `EpubWarning`, `EpubBook`, `EpubError`; Konstanten `MIMETYPE`, `CONTENT_DIR` (`OEBPS`),
`OPF_PATH`, `CONTAINER_PATH`. Module: `build` (minimaler Aufbau S1: Dateiaufteilung je Teil/Kapitel
`text/k{key}.xhtml`, Vorspann `text/front.xhtml`, `<p>` mit maskiertem Rohtext, `nav.xhtml`, `style/book.css` aus
`baruch_typst::typeset::epub_style`, minimale OPF), `package` (echt: `mimetype` zuerst, STORED ohne Extra-Feld, feste
Zeitstempel, DEFLATE), `xhtml::escape_text`, `sample::input` (frei erfundenes Werk „Das Gartenhaus am Bach“, PNGs im
Code erzeugt), `readers` (Typen und Slugs eingefroren, Platzhalterwerte), `check` (Typen eingefroren, Rumpf STUB).
Namensregeln der Dateien und Klassen: Vertrag §1.4 (Kopf von `lib.rs`).
Store: `Store::epubcheck_jar`/`set_epubcheck_jar` (`store_meta`, Schlüssel `epubcheck_jar`, keine Migration).
Server: `epub.rs` (RPC `work.epub`, `GET /werke/{id}/epub`, `epub_input`, `book_for`, `epub_bytes`,
`write_epub_file`; STUB `EPUB_COVER` aus `cover.rs` hierher verschoben, `cover::book_info` ist `pub(crate)`),
`epub_preview.rs` (`epub.readers` echt, `epub.preview` und Seiten STUB), `epub_check.rs` (`epub.check`,
`epub.checker_get`/`epub.checker_set`, Seiten, Formulare, `preview_panel` – alles STUB), MCP `mcp/epub.rs`
(`epub_export`) und `mcp/epub_check.rs` (`epub_check`), beide STUB und nur lesend. Stile `static/epub.css`,
`static/epub_check.css`, `static/readers/<slug>.css`; Widget `web/src/epub_preview.ts` (leer).

### R20a epub {#r20a-epub}

- **`baruch-epub::build`** ersetzt den minimalen Aufbau von S1: Aufteilung nach §0.2 (`split`), Text über das
  Blockmodell (`xhtml::Renderer`, Zuordnung in `docs/WYSIWYG.md` "EPUB mapping"), Fußnoten (`FootnoteMode::Popup` →
  `noteref`/`aside` in `ol.baruch-footnotes` am Dateiende; `Endnotes` → `text/notes.xhtml`; Nummern fortlaufend wie bei
  Typst, je Kapitel neu beginnend nur mit der Satzeinstellung `restart = per_chapter` im Popup-Modus, Zeichen nach
  `FootnoteNumbering`), Bilder (nur die vom Text genannten, `images/<bereinigter Pfad>`, Ids `img-n`), Umschlag
  (`images/cover.<ext>` + `text/cover.xhtml`), Vorspann (`title`, `imprint`, `dedication`), `nav.xhtml` (Inhaltsverzeichnis
  + Landmarks, `hidden`), `toc.ncx` (verschachtelte navPoints, `dtb:depth`), OPF (Metadaten nur über `escape_text`,
  `urn:isbn:` erst nach `isbn13_check`, Bildnachweise als zweites `dc:rights`, schema.org-Barrierefreiheit,
  `alternativeText` nur, wenn jedes Bild alt hat, `<guide>` für EPUB 2/Kindle), CSS = `build::BASE_CSS` +
  `epub_style(&typeset)`. Marken werden vorher gesammelt (`collect_labels`), damit `@name` über Dateien hinweg zeigt.
- **`baruch-epub::template`**: `pagebreak_rules` (eigener Scanner: Kommentare entfernt, jede Show-Regel bis zum
  Zeilenende bei Klammertiefe 0) → `TEMPLATE_PAGEBREAK_MAPPED` für `heading.where(level: 1)`/`<baruch-part>`, sonst
  `TEMPLATE_PAGEBREAK_UNMAPPED`; `template_chapter_start` liest `detect_chapter_break` und setzt `chapter_start`, wenn die
  Satzeinstellung fehlt; `template_lang` (`#set text(lang: …)`), `is_language_tag`.
- **`baruch_typst::typeset::epub_style`**: Umbruchregeln für `section.baruch-chapter`/`section.baruch-part` immer
  (Standard neue Seite, fortlaufend → `auto`, rechte Seite → `right`), weil Kapitel immer eigene Dateien sind.
- **Server `epub.rs`**: `epub_input` sammelt `export_map` (Werkvorlage = Text vor dem ersten Abschnitt), Nummern aus
  `numbering_preview` mit dem eingestellten Schema, `export_assets` + Alternativtexte aus `asset_list`, Umschlag
  (`cover::cover_name`, `asset_data`), `book_meta` + `cover::book_info` → `imprint_lines`, `copyright_line`, Sprache
  (Satzeinstellung → Vorlage → "de"), `dcterms:modified` (jüngster Zeitpunkt aus Werk, Buchangaben, Satzeinstellungen,
  Bildern, Fassungen). Ein Abschnitt ohne lesbare Überschrift bleibt als Ebene 2 erhalten. RPC `work.epub`, Route
  `GET /werke/{id}/epub`, `write_epub_file` (CLI, `create_new`). STUB `EPUB_COVER` entfernt.
- **MCP `mcp/epub.rs`**: `epub_export` nur lesend, `MCP_EPUB_LIMIT` = 8 MiB für `include_bytes`.
- **CLI** `export --epub` (`main.rs`).
- Tests: `baruch-epub/tests/{build,xhtml,package}.rs` (Aufteilung, Fußnoten, Querverweise, Bilder/alt, Inseln,
  Vorlagenregeln, Metadaten/Maskierung, OCF-Reihenfolge, Manifest/Spine, xmllint), `baruch-typst/tests/typeset_epub.rs`,
  Server `tests/round20_epub.rs` (RPC, Download, 403/404/422, Umschlag/ISBN/Bilder, MCP mit der 8-MiB-Grenze, CLI).

### R20b reader {#r20b-reader}

- **Profile** `baruch_epub::readers` (`PROFILES`, `profile(slug)`, neu `required()`): die neun eingefrorenen Slugs in
  Vertragsreihenfolge mit Gerät, Maßen (CSS-Pixel im Hochformat), `required` (die fünf Pflichtprofile),
  `via_conversion` (nur Kindle/Kindle-App), `ReaderSupport` und Hinweis. **Annahmen** (ohne echtes Gerät und ohne Netz
  aus dem Wissen der Entwickler zusammengestellt): Apple Books „Original“ mit Georgia als Rückfall, eigene Schriften
  nur mit `com.apple.ibooks.display-options.xml` verlässlich; Google Play Books „Original“ mit Literata; Kindle/Kindle-App
  nach Send to Kindle mit Bookerly statt der Verlagsschrift, ohne Hintergrundfarben, SVG unsicher; tolino und Kobo
  (gewöhnliches EPUB, nicht KEPUB) zeigen Fußnoten als Sprung statt als Popup; E-Ink in Graustufen; Thorium (Readium)
  und der calibre-Betrachter unterstützen EPUB 3 vollständig. **Hersteller-Reader auf Android** (Samsung, Huawei …)
  sind ohne Gerät nicht prüfbar – kein erfundenes Profil, Nutzerfrage.
- **Reader-CSS** `static/readers/<slug>.css` (eine Datei je Profil, im Server per `include_str!`,
  `epub_preview::reader_css`): Standardschrift, Größe, Ränder (`padding` am `body`), Zeilenabstand, Ausrichtung,
  Silbentrennung (`hyphens`), Fußnoten (Popup-Reader: Kasten „Popup-Fußnote“, sonst Notiztext mit Linie), E-Ink
  (`filter: grayscale(1)`, keine Hintergründe), eigene Standardschrift (`!important`) und nach der Marke
  `/* @night */` der Nachtmodus. Kein `<`/`&` (das CSS steht in XHTML), keine Adressen.
- **Reader-Dateien** `GET /werke/{id}/epub/leser/{slug}/{*file}` (`epub_preview::reader_file`): Das Buch wird je
  Anfrage neu gebaut (`epub::book_for`, kein Cache); XHTML bekommt `<style id="baruch-reader">` vor `</head>` (nach
  `style/book.css`), alles andere byte-identisch mit dem Typ aus dem Manifest. Antwort mit CSP `sandbox
  allow-same-origin; frame-ancestors 'self'` (keine Skripte, keine Formulare, nur in Baruch einrahmbar;
  `allow-same-origin`, damit Bilder und Stile der Lesedatei als Anfragen desselben Ursprungs laden – sonst lehnte der
  Schutz vor anderen Seiten sie mit 403 ab), `nosniff`, `no-cache`. Pfade mit `..`, `.`, leeren Teilen, `\` oder
  Steuerzeichen und alles außerhalb des Manifests → 404; `text/notes.xhtml` ohne `fussnoten` baut mit Endnoten (Links aus
  Kapiteln verlieren die Abfrage).
- **Einrahmen und Ursprung:** Bei einer Antwort mit eigener `frame-ancestors`-Direktive setzt `guard::no_framing` kein
  `DENY` und hängt nichts an die CSP an: mit `frame-ancestors 'self'` `X-Frame-Options: SAMEORIGIN`, sonst ohne
  `X-Frame-Options`. Alle anderen Antworten behalten `DENY` und `frame-ancestors 'none'` (Test). Wegen `sandbox` läuft
  das eingerahmte Dokument mit einem opaken Ursprung; seine Unteranfragen (Bilder, `book.css`, Links zwischen Dateien)
  kommen unter Fetch Metadata als `Sec-Fetch-Site: cross-site` an. Sie werden angenommen, wenn `Referer` auf denselben
  `Host` zeigt (`epub_preview::from_own_frame`), sonst 403. **In der Sichtprüfung des Integrators (Firefox) geprüft:** Der
  Browser kürzt diesen `Referer` auf den Ursprung (`http://host/`), selbst mit `Referrer-Policy:
  no-referrer-when-downgrade` im Rahmen; deshalb zählt der gekürzte `Referer` desselben Hosts (oder, ungekürzt, eine
  Reader-Datei desselben Werks). Eine fremde Seite kann den `Referer` nicht auf unseren Host setzen.
- **Vorschauseite** `GET /werke/{id}/epub/vorschau` (`templates/epub_preview.html`, eigene Umgebung im Modul): Formular
  GET (`leser` mehrfach, `datei`, `fussnoten`, `nacht`), Liste der Kapitelgrenzen aus den Spine-Dateien mit Text (Titel
  aus `EpubBook.items`, Teil/Kapitel als Grenze markiert, Links mit der Profilwahl), Hinweise aus dem Bauen
  (`EpubBook.warnings`), je Profil eine `<figure>` mit einem `<iframe>` in Gerätegröße, der Hinweis „Send to Kindle nimmt
  EPUB an“, darunter der Platz „Prüfung“ mit dem unveränderten Fragment `epub_check::preview_panel` (leer → Link auf
  `/werke/{id}/epub/pruefung`). Unbekannter Slug/Datei → Hinweis mit 404, ungültiges `fussnoten` → 400, Werk ohne
  Abschnitt → Hinweis mit 422, fremde Seite → 403. Das Widget `epub-preview` (`web/src/epub_preview.ts`) sendet das
  Formular bei jeder Änderung ab (nicht, wenn kein Reader mehr gewählt ist).
- **RPC** `epub.readers`, `epub.preview` (siehe RPC.md). Menü: base.html (siehe den Abschnitt zu Kopf und Menüs, `menu.rs`).
- Tests: `baruch-epub/tests/readers.rs` (Slugs, Pflicht, Kindle, CSS-Datei je Slug), `tests/round20_reader.rs`
  (Seite, Formular, 404-Hinweise, Reader-Datei mit CSP/`SAMEORIGIN`/`nosniff`, xmllint, Bilder über `reader_file`,
  `no_framing`, RPC, Prüfplatz, Reader-CSS), `tests/round9_menu.rs` (Menüeinträge, Seite in `all_pages`).

### R20c epub-pruefung {#r20c-epub-pruefung}

- **`baruch_epub::check`** (rein, ohne Netz): `check_structure(epub, xmllint)` liest die ZIP-Bytes. Es prüft den ersten
  lokalen Dateikopf direkt in den Bytes (`mimetype` als erster Eintrag, Methode STORED, kein Extra-Feld, Inhalt
  `application/epub+zip`), den Rest über `zip::ZipArchive` (nur XML/CSS werden entpackt, höchstens je 16 MiB; Pfade mit
  `..`, `/` am Anfang oder `\` sind ein Fehler). `container.xml` → `rootfile/@full-path` (ohne Container der Rückfall
  `OEBPS/content.opf`, damit die anderen Prüfungen noch laufen), OPF → Metadaten (Kennung einschließlich
  `unique-identifier`, Titel, Sprache, `dcterms:modified` für EPUB 3, `schema:access*` als Hinweis), Manifest (doppelte
  Ids, fehlende Dateien, Dateien ohne Eintrag außer `mimetype`, `META-INF/*` und OPF), Spine (bekannte Ids, nur XHTML
  oder mit `fallback`, `toc` → NCX, ohne `toc` der Hinweis `NCX_MISSING`), `<meta name="cover">`, Navigation
  (`properties="nav"` und `<nav epub:type="toc">`), Lesereihenfolge (Dateien der Nav-Links müssen in Spine-Reihenfolge
  stehen). Je XHTML: Sprache, `<title>`, `alt` (fehlt = Fehler, leer = Hinweis), doppelte Ids, Verweise von `img`, `link`,
  `a`, SVG `image` auf fehlende Dateien oder Sprungmarken. Codes fest in `check::codes`.
- **Wohlgeformtheit:** mit `xmllint --noout --nonet -` je Datei über die Standardeingabe (wenn `find_xmllint` es im
  übergebenen PATH-Inhalt findet), sonst und als Rückfall, falls xmllint nicht startet, mit einem kleinen eigenen
  XML-Baum über quick-xml (`check/xml.rs`: genau eine Wurzel, geschlossene Elemente auch am Dateiende, gültige Attribute
  ohne Duplikate, nur die fünf vordefinierten Entitäten und Zeichenverweise – XHTML im EPUB hat keine DTD). Der Inhalt
  wird immer durch diesen Baum geprüft; `CheckReport.xml_tool` nennt das Werkzeug der Wohlgeformtheitsprüfung.
- **Reader-Warnungen** `reader_warnings(book, profile)` aus `ReaderSupport` und dem gebauten Buch (CSS und XHTML der
  Items): Popup-Fußnoten (`epub:type="noteref"`), Seitenumbruch-CSS (für Kapitel-/Teildateien), eingebettete Schriften
  (`font/*`, `@font-face`), Verlagsschrift (`font-family` mit `overrides_publisher_font`), CSS-Hintergründe und
  Nachtmodus, SVG, Silbentrennung (`hyphens: auto`), Umwandlung über Send to Kindle (`via_conversion`). Jede Warnung
  trägt `reader = slug` und beginnt mit dem Namen des Readers.
- **EPUBCheck:** `find_epubcheck(path_env, jar)` – `epubcheck` (ausführbar) im übergebenen PATH-Inhalt vor einer
  vorhandenen `.jar`; `run_epubcheck` startet das Programm oder `java -jar` mit dem EPUB in einer eigenen
  Prozessgruppe, liest stdout/stderr in eigenen Threads (höchstens je 4 MiB) und beendet bei Ablauf der Zeitgrenze die
  ganze Gruppe (`kill -KILL -- -pgid`, dann `Child::kill`), ohne auf die Leser zu warten. Zeilen `ERROR(…)`, `FATAL(…)`,
  `WARNING(…)`, `INFO(…)`/`USAGE(…)` werden zu Befunden `EPUBCHECK_<Code>` mit dem Pfad im EPUB; „bestanden“ heißt Exit 0
  ohne Fehlerzeile. Ein eben geschriebenes Skript, das kurz „Text file busy“ meldet, wird bis zu 20-mal neu gestartet.
  Nie ein Download; der PATH des Prozesses wird nur gelesen, nie gesetzt.
- **Server `epub_check.rs`:** `run_check(state, work, readers, footnotes, epubcheck, path_env)` baut das EPUB über
  `crate::epub::book_for` und `baruch_epub::package` (wie der Download, kein Commit), prüft in `spawn_blocking`, hängt
  die Warnungen des Bauens an (`…_MAPPED` als Hinweis, sonst Warnung) und je Reader `reader_warnings`. EPUBCheck nur mit
  `epubcheck = true`: Pfad aus `Store::epubcheck_jar`, höchstens ein Lauf zugleich (statische Semaphore), EPUB in einem
  eigenen Temp-Ordner (`baruch-epubcheck-<pid>-<n>`, beim Fallenlassen entfernt), Zeitgrenze 180 s. RPC `epub.check`,
  ohne Werk `epub.checker_get`/`epub.checker_set` (Pfadprüfung in `validate_jar`: absolut, `.jar`, reguläre Datei, keine
  Steuerzeichen, ≤ 4096 Bytes). Seite `GET /werke/{id}/epub/pruefung` (Template `epub_check.html` mit dem Fragment
  `fragments/epub_check_panel.html`, eigene minijinja-Umgebung), Formulare `POST /werke/epub/pruefen` (200 mit dem Bericht
  einschließlich EPUBCheck) und `POST /einstellungen/daten/epubcheck` (303 oder 422 über `settings::error_page`); der Block
  „EPUBCheck“ unter Einstellungen → Daten kommt aus `epub_check::settings_context`. `preview_panel` liefert den Bericht
  ohne EPUBCheck als fertiges, maskiertes Fragment für die Vorschau. MCP `epub_check` ruft `run_check` mit festem
  `epubcheck = false`.
- **Reste der Buchangaben:** `baruch_typst::cover::isbn13_check` (Signatur unverändert) schneidet das Präfix
  „ISBN“/„ISBN-13“/„ISBN 13“ in jeder Schreibweise ab und kennt die neue Fehlerart `IsbnError::Prefix` (nicht 978/979);
  `Store::set_book_meta` vergleicht die ISBN als Text (eine nur anders geschriebene ist eine Änderung). Papierstärke:
  `paper_mm` je Blatt bleibt gespeichert (keine Migration, Austauschformat unverändert); `work.meta.get` rechnet
  `paper_sheet_mm` = 2 × `paper_mm`, `work.meta.set` und das Formular nehmen `paper_sheet_mm` (halbiert gespeichert;
  zusammen mit `paper_mm` -32602). Löschen des Umschlagbilds: Unter derselben Schreibsperre entfernt
  `cover::release_cover_before_delete` zuerst den Umschlag mit einem eigenen Commit „Umschlag entfernt“ (`asset.delete`
  ohne `rev`, Formular, bestätigter Löschantrag), erst dann löscht der Store das Bild. Ein bestätigter Umschlagantrag setzt
  den Umschlag über `Store::set_work_cover` unter der Sperre und schreibt **einen** Commit (`mirror::describe_asset_request`
  plus „Umschlag vorher/nachher“ über `cover::commit_request`) statt zuvor zwei.
- **Druckerumschlag mit Textregeln (Abweichung vom Backlog: von R20a hierher verschoben, Vertrag §0.6):**
  `baruch_typst::cover::template_text_rules` liest die `#set text(…)`/`#set par(…)`-Regeln am Zeilenanfang der Werkvorlage
  einschließlich des Regelblocks der Satzeinstellungen (`cover::page_template`), behält nur `lang`, `region`, `script`,
  `dir`, `hyphenate`, `tracking`, `spacing`, `costs`, `kerning`, `ligatures` oder `justify`, `leading`, `spacing`,
  `first-line-indent`, `hanging-indent`, `linebreaks` mit einfachen Werten (Zahlen mit Einheit, Zeichenketten,
  `true`/`false`/`auto`/`none`, Wörterbücher daraus) und lässt bedingte Regeln, Variablen und ein ohnehin geltendes
  `lang: "de"` weg. `wrap_source_with_rules` und `block_measure_source_with_rules` setzen sie direkt nach
  `#set text(lang: "de")`; ohne Regeln sind sie byte-identisch mit `wrap_source`/`block_measure_source` (Golden-Stände
  R26/R27 weiter grün, `round27_satz.rs`, `baruch-typst/tests/cover_layout.rs`). Der Umschlag übernimmt weiterhin keine
  Show-Regeln der Vorlage (sie könnten Überschriften oder Seiten betreffen, die es auf dem Umschlag nicht gibt).

### Integration {#integration-1}

- Zusammenführung ohne Konflikte (`--no-ff`, Reihenfolge S1 → epub → reader → epub-pruefung).
- Integrator: Die `instructions` in `mcp.rs` nennen `epub_export` und `epub_check`; die S1-Ausnahme für STUB-Beschreibungen
  in `tests/mcp.rs` ist weg; die Prüfseite `/werke/{id}/epub/pruefung` steht in `round9_menu` `all_pages` (42 Seiten); der
  Modulkopf von `cover.rs` verweist nicht mehr auf das entfernte `EPUB_COVER`; S1-Standsätze in Bedienung/RPC/MCP ersetzt.
  `grep -rn "STUB:" crates` zeigt keine R20-Vermerke mehr.
- Tests nach der Zusammenführung angepasst (Verhalten von R20a, das reader/epub-pruefung in ihrem eigenen Worktree nicht
  kannten): Titel der Dateien tragen die Nummer aus dem Schema („2 Das Holz“, `round20_reader.rs`), das Testwerk bringt
  eigene Bilder (eigener Bildpfad im Reader-Test; `IMG_ALT_EMPTY` in `check.rs` zählte gegen den Ausgangspunkt).
- Sichtprüfung (Firefox): Bilder und `book.css` luden im Rahmen nicht (403), weil Firefox den `Referer` der Unteranfragen
  aus dem `sandbox`-Dokument auf den Ursprung kürzt; `from_own_frame` akzeptiert jetzt den gekürzten `Referer` desselben
  Hosts (Test in `round20_reader.rs` und Unit-Test). E-Ink-Profile (kindle, kobo, tolino): `filter: grayscale(1)` am `body`
  statt am `html`, sonst füllt der Papierton den Rahmen nicht ganz.
- Abnahme `tests/round20_abnahme.rs` (7 Tests, Werk „Das Feuer am Kap“, frei erfunden): Download über Menü/Route, RPC und
  CLI mit gleichen Bytes; Umschlag + Vorspann + zwei Teile + drei Kapitel als eigene Dateien, nav/ncx, zwei Popup-Fußnoten
  mit Ziel, Querverweis `kC.xhtml#kN`, Metadaten mit `&`, `<`, `#[` wörtlich, Satzeinstellungen im CSS, xmllint auf jeder
  XML-Datei, eigene Prüfung ohne Fehler; Vorschau (fünf Pflichtprofile, Wahl per GET, Reader-Datei mit CSP/SAMEORIGIN,
  Hinweis `TEMPLATE_PAGEBREAK_MAPPED`, `..._UNMAPPED` für eine Abbildungsregel); absichtlich kaputte EPUBs (fehlende Datei,
  falsches oder komprimiertes mimetype, kaputtes XHTML); EPUBCheck nicht gefunden oder als Skript im übergebenen
  PATH-Inhalt; MCP beide Werkzeuge ohne Commit, `epub_check` ohne EPUBCheck.

## Runde 31 – Register und Glossar {#round-31--index-and-glossary}

### Schnittstelle R31-S1 {#interface-r31-s1}

Vertrag R31 (PM, 2026-11-09). Eingefrorene Typen und Signaturen, Rümpfe teils als ehrliche Stubs (`grep -rn "STUB:"
crates`, Vermerk „(R31 …)“):

- `baruch-typst::index` (neu, Besitz von register-satz): Registermarke `#metadata((register: "…", term: "…"))<baruch-idx>`
  (`IndexMark`, `mark_source`, `parse_mark` echt), `Occurrence` (`first`/`all`/n), eingebaute Register `stichwort`,
  `personen`, `bibel` (`builtin_registers`, `valid_register_slug`), `term_key` (NFC, Leerraum, Kleinschreibung; er wird in
  der Datenbank gespeichert, ist also unveränderlich), `glossary_anchor` (`baruch-gl-UUID`), `BackMatter` mit
  `is_active`. Stubs: `sort_key` (= `term_key`), `find_marks`, `mark_word`, `insert_mark_at`, `remove_mark`
  (`IndexError::NotYet`), `Projector::project` (Identität), `back_matter` (""), `overview` (leer). Neue Abhängigkeiten
  `baruch-scripture` und `unicode-normalization`.
- `TypesetSettings` (eingefroren) hat drei neue Felder am Ende: `glossary_page` (`GlossaryPosition`: vor/nach den
  Registern), `index_pages` (`IndexPages {registers, columns}`, 1–3 Spalten, höchstens 16 Register) und
  `glossary_first_mention`; `FIELDS` hat 19 Einträge. Der Regelblock "// Baruch: Satz" erzeugt dafür keine Zeile.
- `baruch-wysiwyg`: `Inline::IndexMark {mark}` (JSON `{"t":"index_mark","mark":{…}}`); `parse` erkennt die kanonische
  Marke vor jedem anderen `#`-Ausdruck, `emit` schreibt `mark_source`, `html` zeigt sie wie eine Inline-Insel
  (`ws-island ws-index-mark`, `data-raw` = `mark_source`), damit das heutige `wysiwyg.ts` sie verlustfrei zurückschreibt.
- `baruch-epub`: `EpubInput::back_matter` (Standard leer), `ItemKind::Glossary`/`Index`, Namensregeln
  `text/glossary.xhtml` (Id `glossary`), `text/index-{slug}.xhtml` (Id `index-{slug}`); `build` ignoriert das Feld noch.
- `baruch-store::glossary` (Besitz von glossar-daten): Typen `GlossaryTerm`, `GlossaryEdit`, `GlossaryChange`,
  `IndexRegisterRow`, `NewIndexProposal`, `IndexProposal`, `IndexAccepted` und die `Store`-Methoden; S1 liest leer und
  lehnt das Schreiben mit `StoreError::Glossary("noch nicht umgesetzt (R31 glossar-daten)")` ab. Neue
  `StoreError`-Varianten `Glossary`, `UnknownGlossaryTerm`, `UnknownIndexProposal` (RPC: INVALID_PARAMS). Keine
  Migration in S1 (0049 kommt mit glossar-daten).
- Store-Haken (Besitz von register-satz): `snapshot::back_matter_in` (Satzeinstellungen + eingebaute und eigene
  Register + angenommene Begriffe), `Store::back_matter`; `export_parts` liefert den Anhang als dritten Wert, Export und
  Exportkarte hängen ihn über `snapshot::append_tail` an (Karte byte-identisch zum Export), `main.typ` nach den
  `#include`-Zeilen; je Abschnitt läuft nach `label_section` ein `Projector::project`; der Spiegel bekommt
  `glossary::mirror_files` (`glossar.md`). Ohne R31-Einstellungen ist alles byte-identisch.
- Server: Module `glossary` (RPC `glossary.*`, `index.registers/register_*/proposals/decide`), `index_marks`
  (`index.entries/mark/unmark/overview`), `glossary_page` und `index_page` (Seiten), `mcp::glossary`,
  `assist::glossary`; Stubs antworten REFUSED „noch nicht umgesetzt (R31 …)“ (`glossary::registers` liefert die
  eingebauten schon). `epub.rs` setzt `back_matter`, `mirror::typeset_value` kennt die neuen Felder. Widgets
  `context-menu` und `glossary` (leer).

### 31a glossar-daten {#31a-glossar-daten}

- **Migration 0049** (`0049_glossary.sql`, die einzige neue Migration der Aufgabe): `glossary_term` (Begriff, `term_key`,
  kurze/ausführliche Erklärung, Quelle, Bibelstellen als JSON-Liste, Status `accepted`/`proposed`, Herkunft, Zeiten;
  eindeutiger Teilindex `(work_id, term_key) WHERE status = 'accepted'`), `glossary_variant` (Varianten je Begriff in
  Reihenfolge, `form_key`, ohne uuid), `index_register` (eigene Register, eingebaute Kennungen per CHECK gesperrt,
  `UNIQUE (work_id, slug)`), `index_proposal` (Registervorschläge, ohne uuid – Arbeitsablauf der Instanz). uuid,
  Füll-Trigger, Grabstein- und Kaskaden-Trigger für `glossary_term` und `index_register` wie 0048; `uuid_tombstone`
  bekommt die Arten `glossary_term` und `index_register` durch Umbau (die siebzehn Grabstein-Trigger wörtlich neu). Kein
  `FOREIGN_KEYS_OFF` (kein Fremdschlüssel zeigt auf `uuid_tombstone`); Beleg
  `crates/baruch-store/tests/migration_0049.rs`.
- **Store** `baruch_store::glossary` (Vertrag §2.5): Lesen sortiert in Rust nach `baruch_typst::index::sort_key`;
  `glossary_lookup` über `term_key` oder `form_key` (nur angenommene); Schreiben prüft die Grenzen `MAX_GLOSSARY_*` mit
  deutschen Meldungen (`StoreError::Glossary`), räumt auf (Begriff und Varianten in einer Zeile, Duplikate entfernt) und
  lehnt Duplikate unter den angenommenen ab; `glossary_propose` lehnt angenommene und schon vorgeschlagene Begriffe ab;
  `glossary_decide(false)` löscht den Vorschlag (Grabstein). Register: eingebaute und ungültige Kennungen abgelehnt,
  höchstens 13. `index_propose` prüft die Marke (`IndexMark::validate`), das Register (eingebaut oder eigen), den festen
  Schlüssel (`GoneNode`) und ob das Wort in der aktuellen Fassung steht; `index_proposal_accept` wendet `mark_word` auf
  die aktuelle Fassung in **einer** Transaktion an und speichert eine neue Fassung des Menschen (`write::save_human`,
  dann `check_current_heading`); steht das Wort nicht mehr da, wird der Vorschlag `stale` (gespeichert) und der Text
  bleibt. Solange `mark_word` (register-satz) ehrlich `NotYet` meldet, lehnt die Übernahme mit dieser Meldung ab, nichts
  geändert. `accepted_entries_in`/`registers_in` speisen `Store::back_matter`; `mirror_files` liefert `glossar.md`
  (angenommene Begriffe alphabetisch mit Varianten, Erklärungen, Bibelstellen, Quelle; dann „Vorschläge (noch nicht
  angenommen)“ mit Herkunft; dann eigene Register) – ohne Begriffe und Register keine Datei.
- **Server** `crate::glossary` (§3.1): alle RPC-Methoden; schreibende Funktionen nehmen die Schreibsperre, bevor sie in
  die Datenbank schreiben, genau ein Spiegel-Commit je Änderung über `mirror::record_glossary` (`describe_glossary`,
  `describe_register`, `describe_index_proposal`, `describe_index_accept`; Trailer `Baruch-Action`,
  `Baruch-Glossary`/`Baruch-Register`/`Baruch-Index-Proposal`, bei Vorschlägen `Persona` und als Autor die Persona oder
  der Client, beim Entscheiden `Baruch-Proposer`). `glossary::normalize_bible_refs` prüft und normalisiert Bibelstellen
  mit `baruch_scripture::parse`. `glossary.suggest` (`assist::glossary`): Persona nach der Lektor-Regel oder `persona`,
  ein Modellaufruf über den `ModelPort` (Antwortform "Term | short explanation", seit Runde 38c; davor „Begriff | kurze Erklärung“),
  Ergebnisse nur über `glossary::propose`; ein Fehler des Modells → REFUSED, nichts angelegt.
- **MCP** `mcp::glossary`: `glossary_list` (nur lesend), `glossary_propose` und `index_propose` (nur Vorschläge,
  Herkunft durch `acting_persona`, `node` → `isError`).
- **Austausch-Nebenversion 7** (`baruch-exchange`, XSD `docs/schema/baruch-werk-1-u7.xsd`, u6 eingefroren): Modell
  `ExchangeRegister`, `ExchangeGlossaryTerm`, `ExchangeGlossaryStatus`; Grabstein-Arten `glossar-begriff`
  (`glossary_term`) und `register` (`index_register`) ab 7; `<satz>`-Attribute `glossarseite`, `registerseiten`,
  `registerseiten-spalten`, `glossar-erste-erwaehnung` (`typeset::R31_ATTRIBUTES`, gelesen erst ab 7 über
  `typeset::r31_from_attrs`). `exchange_dump` schreibt Register und alle Begriffe, `exchange_apply` übernimmt sie neu mit
  Kennungen oder gleicht ab (eine bekannte Kennung wird nie überschrieben, derselbe `slug`/`term_key` unter fremder
  Kennung → Hinweis statt Duplikat, Grabstein → „hier gelöscht“). Registervorschläge stehen nicht im Format.

### 31b register-satz {#31b-register-satz}

**baruch-typst::index** (Rümpfe; private Untermodule `index/scan.rs`, `index/project.rs`, `index/back.rs`):

- **Lesen** (`scan.rs`): je Byte Fließtext oder ein gesperrter Ort mit eigener Nummer. Gesperrt sind Überschriftenzeilen
  (`=`-Zeilen, Teil- und „ohne Nummer“-Marker), Escapes, Roh, Kommentare, Formeln, Marken, Verweise (`@name`), Adressen,
  Code nach `#` einschließlich Zeichenketten und Klammern, Anweisungen (`#let`, `#set`, `#show`, `#if` … bis zum
  Zeilenende einschließlich aller Klammern) und Registermarken. Inhaltsblöcke von Aufrufen (`#emph[…]`, `#footnote[…]`,
  `caption: […]`) sind wieder Fließtext. Eine Einfügestelle ist erlaubt, wenn sie nicht zwischen zwei Bytes desselben
  gesperrten Ortes liegt, nicht am Ende einer Kommentar- oder Überschriftenzeile, nicht am Anfang einer
  Überschriftenzeile und nicht vor einer Marke (sie hängte sich sonst an die Registermarke).
- `find_marks` = alle kanonischen Marken in Textreihenfolge (auch in Fußnoten und Inhaltsblöcken, nie in Kommentar, Roh
  oder Zeichenkette). `mark_word` sucht das Wort wörtlich als ganzes Wort im Fließtext (Leerraum im Wort passt auf
  Leerraum ohne Leerzeile; ein Wort direkt vor einer Marke → Registermarke nach der Marke), setzt `mark_source` direkt
  dahinter (erstes, n-tes oder alle Vorkommen); `insert_mark_at` prüft die Stelle; `remove_mark` nimmt nur einen
  Bereich, den `find_marks` liefert. `sort_key` = DIN 5007-1 (NFD ohne Akzente, `ß` = `ss`, Kleinschreibung,
  Satzzeichen am Anfang übersprungen).
- **Projektion** (`project.rs`, `Projector`): (a) mit dem Register `bibel` in `index_pages.registers` nach jeder
  gültigen Stelle im Fließtext (`baruch_scripture::scan` auf dem Text mit Leerzeichen statt gesperrter Bytes) eine Marke
  `register: "bibel"`, `term` = Buchname („Johannes“), `sub` = Kapitel und Verse („3,16“), `sort` =
  `Passage::sort_key` (`43.003.016-003.016`, biblische Reihenfolge); (b) mit `glossary_first_mention == Some(true)` je
  Kapitel (eine Überschrift der Ebene 1, Teil oder „ohne Nummer“ Ebene 1 setzt zurück) das erste Vorkommen eines Begriffs
  oder einer Variante als ganze Wörter (ohne Beachtung der Groß-/Kleinschreibung, Wortgruppen mit Leerraum, `-`, `'`, `’`,
  `.`). Form: mit Glossarseite `#link(<baruch-gl-UUID>)[#emph[Wort]]`, ohne `#emph[Wort]` – so gibt es nie einen Verweis
  auf eine fehlende Marke. Folgt `(`, `[`, `;` oder `.Wort`, steht danach ein `;` (beendet den eingebetteten Ausdruck,
  Typst setzt es nicht). Projektion nur im Export; SQLite bleibt unverändert.
- **Anhang** (`back.rs`, `back_matter`): "" ohne Registerseiten und Glossarseite; sonst aus
  `// Baruch: Register und Glossar` Hilfsfunktionen (`baruch-idx-ranges`, `baruch-idx-pages`, `baruch-idx-group`,
  `baruch-idx-entry`, `baruch-gl-entry`), dann die Glossarseite (vor oder nach den Registern) und je Register aus
  `index_pages.registers` (unbekannte Kennungen entfallen) eine Seite `#heading(level: 1, numbering: none)[…]`.
  Sortieren und Gruppieren in Rust (Buchstabengruppen nach `sort_key`, Bibel nach Sortierschlüssel), Seitenzahlen in
  Typst: `query(<baruch-idx>)`, je Marke `counter(page).at(m.location())` (dieselbe Zählung wie Fußzeile und „S. n“, auch
  ab dem ersten Kapitel), folgende Seiten "12–14", Marken mit `see` ohne Seite. Spalten über `columns(n, gutter: 1.5em,
  …)`. Glossar: Begriff fett, Varianten in Klammern, kurze Erklärung, Absätze der ausführlichen, „Bibelstellen:“; Anker
  `#metadata("baruch-glossar")<baruch-gl-UUID>` (nur für eine UUID aus Buchstaben, Ziffern, `-`, `_`). Alle Daten als
  maskierte Zeichenkettenliterale. Gleiche Eingabe → gleiche Bytes.
- `overview` (Übersicht je Register, gespeicherte Texte): alle Register aus `back.registers`, Einträge sortiert wie im
  Anhang, `keys` = Abschnitte mit Marken, die eine Seite tragen; automatische Bibelstellen nur bei eingeschaltetem
  Bibelstellenregister.

**baruch-scripture** (additiv): `book_order` (1–66), `Passage::sort_key`, `Passage::chapter_verse`.

**Store** (`snapshot.rs`, `export_map.rs`): Die Haken aus S1 bleiben unverändert; geprüft in
`crates/baruch-store/tests/index_export.rs` (Goldens R29-S1 byte-identisch, Export = projizierte Abschnitte + Anhang,
Karte = Export, `main.typ` = vorher + Anhang, SQLite ohne Projektion).

**EPUB** (`build.rs`, `xhtml.rs`): `Inline::IndexMark` → `<span id="baruch-idx-{key}-{n}" class="baruch-idx">`
(`Renderer::index_marks`, `PlacedMark`); die erste Erwähnung (Insel `#link(<baruch-gl-…>)[#emph[…]]`) →
`<a epub:type="glossref" href="glossary.xhtml#baruch-gl-UUID"><em>…</em></a>` (ohne Glossarseite `<em>`); ein `;` direkt
nach einem eingebetteten Ausdruck vor `(`, `[`, `;`, `.Wort` entfällt wie in Typst. Mit aktiven Einstellungen:
`text/glossary.xhtml` (`epub:type="glossary"`, `dt` glossterm mit `id="baruch-gl-UUID"`, `dd` glossdef) und
`text/index-{slug}.xhtml` (`epub:type="index"`, `index-entry`/`index-term`, je Abschnitt ein `index-locator` zum ersten
Anker dort, beschriftet mit Nummer und Titel des Abschnitts), am Ende von Spine und Inhaltsverzeichnis in der Reihenfolge
des Satzes; `book.css` bekommt `INDEX_CSS` nur dann (sonst byte-identisch).

**Server** `crate::index_marks`: `index.mark`/`index.unmark` nehmen die Schreibsperre, setzen oder entfernen die Marke in
Fassung `rev` und speichern eine neue Fassung des Menschen mit `save_human_on`; `mirror::record` schreibt genau einen
Commit. Register müssen in `glossary::registers` stehen. `index.entries` liest die Marken der aktuellen Fassungen und die
automatischen Bibelstellen (Projektion je Abschnitt); `index.overview` ruft `baruch_typst::index::overview`.

Bekannte Grenzen: Ein importierter Export mit Anhang nähme den Anhang in den letzten Abschnitt auf (der Import kennt
`// Baruch: Register und Glossar` noch nicht); die Karte ordnet Typst-Meldungen aus dem Anhang dem letzten Abschnitt zu.
Stichwörter gelten als gleich, wenn sie wörtlich gleich sind.

### 31c kontextmenue {#31c-kontextmenue}

- **Satzansicht:** `baruch-wysiwyg` zeigt `Inline::IndexMark` als leeres `span.ws-index-mark` (Felder in `data-*`,
  Beschreibung aus `index_mark_label` in `title`/`aria-label`, Zeichen nur im Stil); `wysiwyg.ts` liest es als
  `{t: "index_mark", mark}` zurück und führt den Befehl `index_mark` aus (`add|edit|remove`, `ws_command.ts`). Einzelheiten
  in `docs/WYSIWYG.md` ("Index marks and context menu").
- **Kontextmenü:** `web/src/context_menu.ts` (Widget `context-menu` auf `span.ribbon-index` im Band,
  `fragments/edit_toolbar.html`) hängt sich an Satzansicht und Textfeld desselben Editors; Befehle an die Satzansicht nur
  über `baruch:ws-command`, kein Import von `wysiwyg.ts`. RPC: `glossary.add`, `glossary.lookup`, `index.registers`.
  Reine Bausteine (`menuItems`, `opensMenu`, `menuKey`, `markFromFields`, `glossaryParams`, `entryLine`) und in
  `ws_command.ts` die Marken-Bausteine (`markSource`, `parseMarkAt`, `findMarks`, `validateMark`, `markLabel`,
  `markInSource`) – sie folgen `baruch_typst::index` und werden unter Node gegen die Rust-Funktionen geprüft.
- **Seiten ohne JavaScript:** `src/glossary_page.rs` (`/werke/{id}/glossar`, `POST …/glossar/act`,
  `/werke/{id}/markierung`, Templates `glossary.html`, `marking.html`, `fragments/context_menu.html`) und
  `src/index_page.rs` (`/werke/{id}/register`, `POST …/register/act`, Template `index_page.html`). Keine eigene
  Datenhaltung: Lesen und Schreiben nur über `crate::glossary` (`call("glossary.list"|"glossary.lookup"|
  "index.proposals")`, `add`, `update`, `delete`, `decide`, `registers`, `register_*`, `index_decide`) und
  `crate::index_marks` (`entries`, `overview`, `mark`, `unmark`); ihre Meldungen erscheinen auf der Seite oder als 422 mit
  den Eingaben. `glossar/act` mit `action=suggest` ruft `crate::assist::glossary::suggest` (Nachtrag R31). Erfolg → 303
  nach `back` (nur Pfade dieses Servers, `glossary_page::safe_back`), sonst die Seite, mit `ok=<action>` vor dem Anker
  (`#begriff-{id}`, `#register-{slug}`, `#vorschlag-{id}`). Stile `static/glossary.css`, `static/context_menu.css`;
  Widget `web/src/glossary.ts` (Filter der Listen).
- **Satzeinstellungen:** `settings/work_typeset.rs` liest und schreibt die drei R31-Felder (`glossary_page`, je Register
  ein Kästchen `index_register` in Satzreihenfolge + `index_columns` (leer = 2), `glossary_first_mention`);
  `BlockInput::registers` kommt aus `crate::glossary::registers`. `formToSettings` (`typeset_settings.ts`) liest dieselben
  Felder (Vergleichstest unter Node in `tests/round31_kontextmenue.rs`).

### Integration {#integration-2}

- Zusammenführung ohne Konflikte (`--no-ff`, Reihenfolge S1 → glossar-daten → register-satz → kontextmenue).
- Integrator: Die `instructions` in `mcp.rs` nennen `glossary_list`, `glossary_propose` und `index_propose` (nur
  Vorschläge); die S1-Ausnahmen in `tests/mcp.rs` (STUB-Beschreibungen) und `tests/round29_oberflaeche.rs` (drei
  R31-Felder im Satzblock) sind weg; S1-Standsätze in Bedienung/RPC/MCP/SKILLS ersetzt. `grep -rn "STUB:" crates` zeigt
  außerhalb der Tests nichts.
- **Import eines Exports mit Anhang** (offener Punkt von register-satz): `baruch_typst::index::strip_back_matter` schneidet
  alles ab der Zeile `// Baruch: Register und Glossar` ab, `strip_projection` nimmt die Zutaten der Projektion heraus
  (automatische Bibelstellenmarken = Register `bibel` mit `sort`, verlinkte erste Erwähnung
  `#link(<baruch-gl-…>)[#emph[…]]` einschließlich des Schutz-`;`). `import::import_source` wendet beides an,
  `strip_projection` nur, wenn ein Anhang gefunden wurde (das gibt es nur bei Exporten von Baruch). Sonst wanderte der
  Anhang in den letzten Abschnitt, und der Text verwiese auf Glossarmarken, die es im neuen Werk nicht gibt (Typst-Fehler).
  Die Auszeichnung ohne Glossarseite (`#emph[…]`) ist von von Hand geschriebener Hervorhebung nicht zu unterscheiden und
  bleibt.
- Abnahme `tests/round31_abnahme.rs` (5 Tests, Werk „Die Wärterin vom Kap“, frei erfunden, zwei Teile, drei Kapitel,
  Bibelstellen, zwei Personen): Glossar über die Seite „Markierung“ und `/glossar/act` (ohne JavaScript) und über
  `glossary.add` (wie der Dialog) → Glossarseite, Glossarseite im PDF (pdftotext: Begriff, Variante, kurze und
  ausführliche Erklärung, Bibelstellen), EPUB `glossary.xhtml` mit `glossterm`/`glossdef` und `glossref` im Kapitel;
  Marken über `/register/act` (alle 70 Vorkommen) und `index.mark` (Untereintrag, „siehe auch“, Personen) →
  Registerseiten mit gemessenen Seiten („Nebel 3–6“), Bibelstellenregister in biblischer Reihenfolge („1. Mose“,
  „Psalmen“, „Johannes“, „Römer“; eine Stelle in einem Kommentar fehlt), erste Erwähnung je Kapitel verlinkt; der
  Typst-Export setzt dieselben Register eigenständig; Persona (`ScriptedModel`) und MCP schlagen nur vor, der Mensch
  übernimmt per Formular (Commits mit `Baruch-Proposer`); Rundreise u7 zwischen zwei Wegwerfinstanzen (Export
  einschließlich Anhang byte-identisch, Grabstein, kein Duplikat beim Abgleich); ohne R31-Einstellungen bleiben Export
  und EPUB byte-identisch, auch mit Glossar und eigenem Register; Import eines Exports mit Anhang; Tastatur
  (Kontextmenütaste, Umschalt+F10) und ARIA im ausgelieferten Kontextmenü, Link „Markierung …“ ohne JavaScript.
- Nach der Zusammenführung: Die Probe in `round31_kontextmenue.rs`
  (`register_forms_check_their_inputs_and_keep_them_on_422`) rief `index.mark` auf und legte dadurch selbst eine neue
  Fassung an, die die des Formulars verdeckte – der Datenweg war vor der Zusammenführung ungeprüft. Die Probe liest
  jetzt nur (`index.entries`).
- Sichtprüfung (Wegwerf-Server mit `--secrets memory`, Verbindungen nach 127.0.0.1:9, keine Anfrage an Eliezer oder ans
  Netz; Bilder `.claude/sicht/r31/`): Kontextmenü in der Satzansicht mit „In Glossar aufnehmen“, Seite „An der
  Markierung“, Glossarseite mit einem Vorschlag aus MCP, Registerseite, PDF-Seiten (Stichwortregister „Nebel 3–4“,
  Bibelstellenregister in biblischer Reihenfolge, Glossarseite, erste Erwähnung kursiv), EPUB-Vorschau mit Glossar.
  **Behoben:** Die Registerseite nannte Fundstellen und Vorschläge mit dem internen Schlüssel („k13“) und die Fundstelle
  englisch („first“) – jetzt Nummer und Titel des Abschnitts („1 Die Ankunft“) und „erste Fundstelle“; die Glossarseite
  schrieb „Vorgeschlagen von Autor“ bei Begriffen des Menschen – „Vorgeschlagen von“ steht nur noch bei Herkunft `@…`.
- `typst` (CLI) liegt auf diesem Rechner nicht im PATH: Die Zusicherungen mit `typst compile` melden „ÜBERSPRUNGEN“, der
  Export wird stattdessen mit dem eingebetteten Compiler als eigenständige Datei gesetzt (gleiche Typst-Version, keine
  weiteren Dateien).

### Nachtrag nach der Prüfung (R31) {#follow-up-after-review-r31}

- **Keine doppelten Marken:** `baruch_typst::index::mark_word` prüft für jedes gewählte Vorkommen die Kette der Marken
  direkt nach dem Wort oder der Marke (`carries_mark` über `Scan::marks`); steht dieselbe `IndexMark` dort schon, bleibt
  das Vorkommen unberührt. Bleibt keines übrig → `IndexError::AlreadyMarked` (ersetzt das unbenutzte `NotYet`), der
  Aufrufer legt keine Fassung an; `index_proposal_accept` setzt den Vorschlag dann auf `stale`. "first"/"n-th" zählen
  weiter alle Vorkommen im Text (die Wahl in der Oberfläche bleibt eindeutig). `markInSource` (TS) prüft dasselbe im
  Quelltext, `sameMarkBehind` (`wysiwyg.ts`) in der Satzansicht. Vorher erlaubte `Scan::insertable` den Ort vor einer
  vorhandenen Marke, und jede Wiederholung war eine neue Fassung einschließlich Spiegel-Commit.
- **„Begriffe vorschlagen lassen“ benutzbar:** Formulare auf der Glossarseite und auf „An der Markierung“
  (`glossar/act`, `action=suggest`, optional `key`); Weiterleitung mit `neu`/`uebersprungen` für den Hinweis.
- **Tests ohne S1-Zweige:** Wie in R27 sind die Zweige entfernt, die „noch nicht umgesetzt“ als richtig akzeptierten
  (`round31_kontextmenue.rs` mit `implemented`, `round31_glossar.rs`, `baruch-store/tests/glossary.rs`); die 422-Fälle mit
  behaltenen Eingaben prüfen echte Fehler (unbekannte Bibelstelle, eingebaute Registerkennung, Marke schon gesetzt).

## Runde 21 – Import und Bilder {#round-21--import-and-images}

### Schnittstelle R21-S1 {#interface-r21-s1}

- **Abhängigkeiten** (in S1 festgelegt, die Entwickler ändern keine Cargo.toml/Cargo.lock): baruch-server `zip` 2.4 (nur
  DEFLATE; aus den Dev-Abhängigkeiten in die Abhängigkeiten verschoben, für `src/archive.rs`), `quick-xml` 0.38 (EPUB,
  21a; schon im Lock), `pulldown-cmark` 0.13 ohne Standardfunktionen (Markdown, 21b; aufgelöst zu 0.13.6); baruch-render
  `typst-render` 0.15.1 (Rasterbild einer Seite für den EPUB-Umschlag, 21c; es ließ sich laden, der Ersatz resvg/image
  wurde nicht gebraucht). `cargo tree -d` zeigt keine neuen doppelten Versionen.
- **`src/archive.rs`** (in S1 vollständig, danach markdown-import): `read_zip(bytes, &ArchiveLimits)` →
  `Vec<ArchiveEntry {path, bytes}>` in Archivreihenfolge, nur reguläre Dateien (Ordner übersprungen). Abgelehnt mit
  `ArchiveError::Unsafe {path, reason}`: absolute Pfade, `..`, `\`, Laufwerk, Steuerzeichen, leere oder nicht-UTF-8-Namen,
  Links, Spezialdateien, Verschlüsselung und doppelte Namen (auch solche, die sich nur in der Groß-/Kleinschreibung
  unterscheiden; das zentrale Verzeichnis zählt der Code selbst vom Endesatz aus, weil die ZIP-Bibliothek Duplikate still
  zusammenführt). `TooLarge`: mehr Einträge, größere Einträge oder eine größere Summe als `ArchiveLimits` – beim Entpacken
  mit `take` gezählt, nicht aus dem Kopf –, Verhältnis entpackt/gepackt über `max_ratio` (Einträge bis
  `RATIO_FREE_BYTES` = 64 KiB sind ausgenommen). `Corrupt`: unlesbar, falsche Prüfsumme, eine andere Methode als
  STORED/DEFLATE. `NotZip`: Anfang ist nicht `PK`. Grenzen für Importe `IMPORT_LIMITS`: 2000 Einträge, je 16 MiB, zusammen
  80 MiB, Verhältnis 200.
- **`src/import.rs`**: `ImportWarning {code, message, line}` (Hinweis statt stillem Verwerfen), Feld
  `ImportReport.warnings` (nach `skipped`), `import_source(…, warnings)`; neu `import_generated(state, source, file_name,
  title, assets, skipped, warnings)` für fertigen erzeugten Typst-Text (PDF/EPUB nach Bestätigung): eine dünne Hülle um
  `import_source`, ein schon vergebener Titel → `REFUSED` mit `TITLE_HINT`, keine Überschrift → `INVALID_PARAMS`.
- **CLI-Weiche** (`main.rs`, `import DATEI`): `exchange::is_exchange_name` → Austausch; `pdf_import::is_source_name`
  (.pdf/.epub) → `pdf_import::cli_import` mit `CliSourceImport {title, apply: --uebernehmen, library: --bibliothek}`;
  `markdown_import::is_markdown_name`/`is_archive_name` (.md/.markdown/.zip) → `markdown_import::cli_import`; sonst Typst.
  `--uebernehmen`/`--bibliothek` bei einer anderen Dateiart → Exit 2. Der Typst-Import gibt je Warnung `Hinweis (<code>):
  <message>` auf stderr aus.
- **Routen und RPC**: `pdf_import::pages()` (`GET /werke/import/quelle`) mit den Seiten, `pdf_import::writes()`
  (`POST /werke/import/quelle/vorschau` mit `DefaultBodyLimit::max(UPLOAD_LIMIT)` = 64 MiB + 64 KiB, `…/uebernehmen`,
  `…/verwerfen`) hinter `guard::same_origin`; RPC `source_import.*` über `pdf_import::handles`/`call` nach
  `works::handles`. Markdown-Routen bleiben in `works.rs`. Stil `static/source_import.css` im gemeinsamen `STYLE`. Menü
  „Baruch ▾ → PDF oder EPUB importieren…“ in `base.html`, ein Link unter dem Typst-Formular in `fragments/work_new.html`.
  In S1 antworteten Seite (200), Formulare (501), RPC (-32000) und CLI (Exit 1) ehrlich „noch nicht umgesetzt“
  (`// STUB: … (R21a/R21b)`).
- **Migrationen**: S1 legt keine an. Reserviert, nur bei Bedarf: Store `0050_r21_assets.sql` (bilder),
  `0051_r21_import.sql` (markdown-import); Bibliothek `0108_title_text.sql` (pdf-import, Tabelle `library_title_text`).

### 21a pdf-import {#21a-pdf-import}

Modul `src/pdf_import/` (global, ohne Werk), Bibliothek `0108_title_text.sql`, Volltext in `literature/fulltext.rs`.

| Datei | Inhalt |
|---|---|
| `mod.rs` | RPC `source_import.*`, `preview`/`preview_with` (Weg wählen, lesen, Entwurf), `apply`/`apply_draft` (Werk über `import::import_generated`, Quelle über `baruch_library::add_title` + `set_title_remote` oder `set_title_text`), `discard`, CLI `cli_import`/`cli_import_with` (eigene `PdfTools` für Tests), `local_pdf_reads` (Zähler je Datenordner: Tests prüfen "no pdftotext on the MultiGPT way") |
| `pdf.rs` | lokal: `pdfinfo -enc UTF-8` (Titel, Autor, Seiten) und `pdftotext -enc UTF-8 -bbox-layout DATEI -` über `tokio::process` mit `TOOL_TIMEOUT` (120 s), `kill_on_drop`, Standardeingabe `null`, nur Seiten 1–`MAX_PAGES` (2000, wie über MultiGPT; mehr → Warnung `import-pdf-pages-cut`), Ausgabe ≤ 128 MiB, höchstens `MAX_PARALLEL_READS` (2) Lesevorgänge zugleich (weitere warten); fehlt das Programm → ehrliche Meldung. Gliederung aus den Wortkästen (`structure`) |
| `remote.rs` | über MultiGPT: `upload_document` (genau einmal), Warten (`run_status`, sonst `read_document` mit Wiederholung, ≤ `INDEX_WAIT` 120 s, Pausen 0,1–2 s), `document_info` (Titel, Autor, Seiten, `toc`), Text mit `read_document {page_from, page_to}` in Bereichen ≤ 10 Seiten (gekürzt → Seite für Seite), Schnitt am Inhaltsverzeichnis (`split_by_toc`), ohne Inhaltsverzeichnis nummerierte Zeilen (`split_by_heuristic`) |
| `epub.rs` | EPUB: `archive::read_zip` mit `IMPORT_LIMITS`, `container.xml` → OPF (Titel, `dc:creator`, Sprache, Manifest, Spine), Navigation aus `nav.xhtml` (`epub:type="toc"`) oder `toc.ncx`, XHTML → Textfluss mit Sprungzielen; Verweise über `resolve` (Prozentkodierung, `..` nur innerhalb des Archivs) |
| `typst_out.rs` | `escape_text` (Typst-Zeichen `\ # * _ $ @ < > [ ] ~` und der Backtick, `/` vor `/`/`*`, `-` vor `-`/`?`, am Absatzanfang `= - + /` und `12.`), `Inline` → Auszeichnung (`_…_`/`*…*` nur an Wortgrenzen, sonst `#emph[…]`/`#strong[…]`; `#footnote[…]`, `#super`, `#sub`, `#link`, `#raw`, `#linebreak()`), `assign`/`build_typst` (bestätigte Gliederung → Typst; Teil über `baruch_typst::heading_line(0, …)`) |
| `xml.rs` | kleiner nachsichtiger XML-Baum über quick-xml 0.38 (`check_end_names` aus, HTML-Entitäten, ≤ 256 Ebenen, ≤ 2 Millionen Knoten) |
| `draft.rs` | Entwürfe unter `<data>/import-drafts/<32 hex>/` (`draft.json`, `asset-<n>.bin`; nach einem Fehler von MultiGPT `pending.json` + `source.bin` für „lokal lesen“); `clean` vor jedem `preview`: älter als 24 h entfernt, dann die ältesten, bis < 20 |
| `pages.rs` | `GET /werke/import/quelle`, `POST …/vorschau` (multipart, `UPLOAD_LIMIT`), `…/uebernehmen`, `…/verwerfen`; Template `templates/source_import.html`, Stil `static/source_import.css` |

- **Gemeinsames Modell:** `Extracted {title, author, lang, pages, sections: [Section {level, title, part, body (Typst),
  plain}], assets, skipped, warnings, plain_text}`. Die Vorschau zeigt `sections` als Gliederung; `apply` bildet die
  bestätigte Liste (`Confirmed {level, title, part, source}`) auf die Abschnitte ab. Text vor dem ersten Punkt wird zum
  Punkt „Vorspann“. Werkvorlage des neuen Werks: `#set document(title, author)` und – bei EPUB mit `dc:language` –
  `#set text(lang)`.
- **PDF-Heuristik (lokal):** Zeilenhöhe = Median der Wortkästen; Fließtext = die häufigste Höhenklasse (halbe Punkte,
  nach Zeichen gewichtet); Überschrift = Block mit ≤ 3 Zeilen, ≤ 160 Zeichen, Höhe ≥ 1,12 × Fließtext; Ebenen nach
  absteigender Höhe (≤ 3); einzeilige Blöcke in Fließtextgröße mit einer Nummer `1.2.3 Titel` ohne Satzzeichen am Ende
  zählen als Überschrift (Ebene = Zahl der Nummernteile, Nummer entfernt). Die größte Klasse genau einmal auf Seite 1 vor
  jeder anderen Überschrift = Titel des Dokuments. Reine Ziffernzeilen am Seitenrand = Seitenzahlen (entfernt). Zeilen
  eines Blocks werden mit Leerzeichen verbunden, `Wort-` + Kleinbuchstabe ohne den Bindestrich.
- **EPUB-Umwandlung:** Blöcke (`p`, Überschriften, Listen als `-`/`+` mit Einrückung, `blockquote` → `#quote(block:
  true)[…]`, `figure`/`img` → `#figure(image("pfad", alt: …), caption: […])` oder `#image(…)`, `pre` → `#raw(block:
  true, …)`, `hr` → `#line(length: 100%)`, Tabellen als Zeilen "a | b" mit einer Warnung). Fußnoten: Elemente mit
  `epub:type` `footnote`/`endnote`/`rearnote`/`note` oder `role="doc-footnote|doc-endnote"`, auf die ein Verweis zeigt
  (`noteref`/`doc-noteref` oder `href` auf eine solche Notiz), fehlen im Fluss und stehen als `#footnote[…]` an der Marke
  (Rückverweise entfernt; eine führende Nummer nur, wenn sie der Fußnotenmarke gleicht und kein Rückverweis die Nummer
  trug – „1 Kor 13,4“ bleibt –, mit Warnung `import-epub-footnote-number`; Bilder in der Fußnote bleiben darin als
  `#image(…)`). Nach jedem eingebetteten Ausdruck (`#linebreak()`, `#emph[…]`, `#raw(…)`, `#footnote[…]` …) steht ein
  Schutz-`;`, wenn `(`, `[`, `;` oder `.Name` folgt (`typst_out::needs_guard`, wie im Markdown-Import), sonst läse Typst
  einen Aufruf oder Feldzugriff. Bilder: Pfad im Werk relativ zum Ordner der OPF-Datei, mit `check_asset_path` geprüft,
  ≤ 8 MiB je Bild, ≤ 64 MiB zusammen, nur PNG/JPEG/GIF/WebP/SVG (`detect_mime`), sonst mit Grund übersprungen.
  Navigation: Ziel `datei#id` → Sprungziel im Fluss; die Überschrift, die dem Navigationstitel gleicht, fällt aus dem
  Text; andere Überschriften bleiben fetter Text (Warnung). Tiefer als 3 → Ebene 3 (Warnung).
- **Bibliothek:** `library_title_text (title_id → library_title ON DELETE CASCADE, source_name, mime, sha256, pages,
  text CHECK typeof = 'text', created)` – nur eine neue Tabelle, Migrationstests `baruch-library/tests/migration_0108.rs`
  (Kern-Fixture) und `baruch-server/tests/round21_pdf_import.rs` (echtes Kernschema von development). Funktionen
  `set_title_text`, `title_text`, `title_text_created`, `title_for_text_sha256` (Duplikate erkennen).
- **Volltext (`literature/fulltext.rs`):** `read(Target::Title)` liest den lokalen Volltext ohne Verknüpfung
  (`origin: "import"`, wie eine lokale Kopie über `local_part` geteilt), `check_highlight` prüft dagegen,
  `has_fulltext(state, title)` = Verknüpfung oder lokaler Volltext. Seit der Integration fragt die Dokumentansicht
  `/quelle` (`literature.rs`, `source_page`) `fulltext::has_fulltext` statt nur `title_remote` und zeigt so auch den
  lokalen Volltext in Teilen einschließlich der Felder Anfang/Ende (Abnahme `round21_abnahme.rs`).
- **Regeln:** Gliederung nie still (immer eine Vorschau; die CLI ohne Schalter schreibt nichts), Teil nur auf
  Bestätigung, kein stiller Wechsel des Weges, Warnungen statt Verwerfen, kein Import über MCP, Inhalte der Datei sind
  Daten (nur maskiert in Seiten und Typst).

### 21b markdown-import {#21b-markdown-import}

- **Module** (`crates/baruch-server/src/markdown_import/`): `convert.rs` (Markdown → Typst, rein, ohne Zustand),
  `mod.rs` (Namen, `Bundle` für ZIP und Ordner, Hauptdatei, Bilder aus dem Bündel, `import_text`, `import_bundle`,
  `import_archive`, `cli_import`, `report_text`). Das Anlegen läuft immer über `import::import_generated` (eine
  Transaktion für Werk und Bilder, Titelregeln wie im Typst-Import). Formulare und RPC in `works.rs`, Bericht
  `templates/import_report.html`. Keine Migration (0051 blieb frei).
- **Umwandlung** (`convert::to_typst`, `pulldown-cmark` 0.13 mit Fußnoten, Tabellen, Durchstreichung, Definitionslisten,
  Überschriftattributen und YAML-Vorspann; Ereignisse → kleiner Baum → Ausgabe):

| Markdown | Typst | Hinweiscode, wenn nicht abgebildet |
|---|---|---|
| `#`/`##`/`###` | `heading_line(1…3)` (`escape_title`) | `md-heading-level` (`####`–`######` → `===`) |
| Auszeichnung in einer Überschrift | Titel als Text | `md-heading-format`; eine Fußnote darin `md-heading-footnote`; `{#id}` `md-heading-attributes` |
| Überschrift in einem Zitat/einer Liste | `#strong[…]` | `md-heading-nested` |
| Text vor der ersten Überschrift | `= <Titel>` davor | `md-preamble-heading` |
| YAML-Vorspann `title`/`author` | `#set document(title: …, author: …)` (Werkvorlage) | `md-front-matter` für andere Schlüssel |
| Absatz, weicher Umbruch, harter Umbruch | Absatz, Zeilenende, `\` + Zeilenende | – |
| `*…*`, `**…**`, `~~…~~` | `#emph[…]`, `#strong[…]`, `#strike[…]` | – |
| `` `Code` `` | `#raw("…")` | – |
| Codeblock | "```sprache" (Zaun länger als jede Backtick-Folge darin) | `md-code-lang` (Sprache mit anderen Zeichen) |
| Liste, nummerierte Liste (Startnummer), verschachtelt | `- `, `n. `, Einrückung = Breite des Markers | – |
| `> Zitat` | `#quote(block: true)[…]` | `md-quote-kind` (`> [!NOTE]`) |
| `[Text](https:…)`, `<https:…>`, E-Mail | `#link("…")[Text]` oder `#link("…")` | `md-link-local` (andere Datei, `#Sprung`, fremdes Schema: nur der Text) |
| Fußnote | `#footnote[…]` am Verweis | `md-footnote-repeated`, `md-footnote-unused`, `md-footnote-nested`, `md-footnote-missing` |
| Bild allein in einem Absatz | `#image("p", alt: "…")`, mit Titel `#figure(image(…), caption: […])` | `md-image-remote` (Netz → `#link`), `md-image-data` (`data:`), `md-image-path` (kaputte %-Folge) |
| Bild im Fließtext | `#box(image(…))` | `md-image-title` (Titel entfällt) |
| Tabelle | `#table(columns: n, align: (…), table.header(…), …)` | – |
| Definitionsliste | `/ Begriff: Erklärung` (`:` im Begriff maskiert) | `md-definition-multiple` (ein Eintrag je Erklärung) |
| Horizontale Linie | `#line(length: 100%)` | – |
| `<br>` | `\ ` | – |
| HTML im Text / HTML-Block / HTML-Kommentar | entfällt (Text bleibt) / Kommentar `// …` / entfällt | `md-html` / `md-html-block` / `md-html-comment` |
| Unbekannte Form | Text als Absatz | `md-unknown` |

  Maskierung (`convert::escape`, `escape_line_starts`): `\ # * _ $ @ < [ ] ~` und der Backtick, `/` vor `/`/`*`, `-` vor
  `-`/`?`; am Zeilenanfang (nach Einrückung) `= + - /` und `Ziffern.` – idempotent. Nach `#…[…]`/`#…(…)` steht ein `;`
  vor `(`, `[`, `;` und `.Buchstabe` (wie `needs_guard` der Projektion). Höchstens 200 Hinweise je Datei, dann
  `md-warnings-truncated`. Zeilen zählen in der Markdown-Datei ab 1 (`into_offset_iter`).
- **Bündel** (`Bundle::from_archive` über `archive::read_zip` mit `IMPORT_LIMITS`; `Bundle::from_files` für Ordner mit
  denselben Pfadregeln, Duplikaten ohne Beachtung der Schreibweise und Grenzen): Hauptdatei = die einzige
  `.typ`/`.md`/`.markdown`, sonst die flachste `index.md`/`main.typ`/`README.md`, sonst ein Fehler mit Kandidaten
  (-32602/422). Bilder: Pfade aus `image_paths` des (umgewandelten) Typst-Textes, `check_asset_path`, Normalform, relativ
  zum Ordner der Hauptdatei (führendes `/` = dieser Ordner), Prüfung `import::check_image` (8 MiB, 64 MiB zusammen,
  Bildtyp) – Gründe wie im Typst-Import. Unbenutzte Dateien: ein Hinweis `archive-unused` (bis zu zehn Namen). `file_name`
  des Werks = Hauptdatei, bei `index.md`/`main.typ`/`README.md` der Name des Archivs oder Ordners mit ihrer Endung
  (Titelrückfall).
- **Wege:** `POST /werke/import` (multipart `file` | `files` + `title`, `IMPORT_UPLOAD_LIMIT` = 80 MiB + 64 KiB nur auf
  dieser Route; `TooLarge` → 413, andere Archivfehler → 422; ohne Hinweise und übersprungene Bilder 303, sonst 200 mit
  Bericht), RPC `work.import` (`.md` als Text, Bilder in `skipped`), `work.import_archive` (Base64-ZIP), CLI
  `markdown_import::cli_import` (Markdown: Bilder von der Platte über `import::read_assets_in` wie im Typst-Import; ZIP:
  aus dem Archiv; Bericht mit `Hinweis (<code>, Zeile n)` auf stdout).
- **`export --out`** (`import::check_targets` vor der ersten Datei): `out` ist kein Nicht-Ordner; keine Zieldatei
  existiert; kein Pfad ist Vorordner eines anderen (`a.png` gegen `a.png/b.svg`, Komponenten nach Normalform); kein
  vorhandener Zwischenordner ist ein Link oder eine Datei. Beim Schreiben zusätzlich `canonicalize(parent)` unter
  `canonicalize(out)` (gegen Änderungen zwischen Prüfung und Schreiben).
- **Satzansicht** (`baruch-wysiwyg/src/html.rs`): `Soft` direkt nach `Linebreak` als leeres
  `<span class="ws-soft" data-after-break="1">` (kein Einzug; das Widget liest es weiter als `soft`); eine Bildinsel mit
  mehreren Bildern nennt ohne `caption:` jedes Bild in der Bildunterschrift und alle Pfade in
  `<span class="ws-alt ws-paths">Bilder: …</span>`. Definitionslisten bleiben ohne Verschachtelung (Grenze in
  `docs/WYSIWYG.md`).

### 21c bilder {#21c-bilder}

Keine Migration (0050 nicht gebraucht): Kontingent und Aufräumen arbeiten auf vorhandenen Zeilen.

- **Typwechsel:** RPC `asset.replace {rename}`, Formularfeld `rename` („Endung anpassen“), MCP `asset_replace {rename}` →
  `Store::asset_request_with(…, rename)` (Feld `rename` in `params` von `replace`, in alten Zeilen fehlend = false;
  `AssetRequest.rename`); das Bestätigen ruft `replace_asset_renaming`. Satz unter dem alten Pfad: `baruch_render`
  `with_image_formats` (siehe „Bilder (Migration 0009)“), im Test mit `pdfimages -list` gemessen.
- **Alter Bestand:** `asset.path_conflicts`, `asset.path_resolve`, Hinweis „Pfade mit zwei Bildern“ oben auf der Seite
  „Bilder“ mit dem Formular `POST /werke/bilder/act` (`action=resolve_path`).
- **Kontingent** (`mcp/assets.rs`, `check_quota` unter `write_lock` vor `add_asset` oder `asset_request_with`):
  `Store::asset_mcp_usage(work, origin_name)` → `AssetUsage {writes, bytes, open_replace}` – `asset_revision` mit
  `author_name` = Anfragender und `created` ≥ jetzt − 1 Tag (Anzahl, Summe von `bytes`) plus `asset_request` `op =
  'replace'` desselben `origin_name` im Zeitraum (Anzahl, Größe aus `params.bytes` oder dem Inhalt), plus offene
  Ersetzungsanträge. Grenzen `MAX_MCP_ASSET_WRITES_PER_DAY` 30, `MAX_MCP_ASSET_BYTES_PER_DAY` 64 MiB,
  `MAX_MCP_OPEN_REPLACE_REQUESTS` 10.
- **Aufräumen:** `Store::purge_decided_asset_requests(work, ASSET_REQUEST_PURGE_DAYS = 30)` setzt in `params` entschiedener
  `replace`-Anträge mit `decided` < jetzt − 30 Tage `content = ""`, `purged = true`, `bytes` = Größe; aufgerufen von
  `asset_requests::purge` vor der Liste, einer Entscheidung, dem Abschnitt „Anträge“, MCP `asset_requests` und der
  Antragsdatei (dann 410 Gone).
- **ImageView:** `rpc::LibraryView.images` (`wysiwyg::image_view` liefert `image_alts` und `ImageView` in einem Durchgang);
  der reservierte Schlüssel `IMAGE_VIEW_KEY` in `image_alts` und `ImageView::from_alts` entfallen.
- **Literatur:** `literature::image_hits` (alle Wörter in Name/Bildunterschrift/Alternativtext der aktuellen Fassung,
  höchstens `MAX_IMAGE_HITS` = 24), ein kleiner eigener Template-Text `IMAGE_HITS_TEMPLATE` (Autoescape HTML); angehängt
  an `literature.render` (Popup) und als `images_html` in `literature.html` (`/literatur`). `fragments/library.html`
  bleibt unberührt; die erste Werkseite mit `?lq=` zeigt den Kasten nur, wenn das Popup über `literature.render` lädt
  (beim Öffnen tut es das immer).
- **Bild einfügen:** `assets::insert_figure_at` mit `Offset` rückt innerhalb eines Absatzes an offenen Klammern vorbei
  (`open_brackets`: `[ ( {`, `\`-Maskierung, Rohtext, Zeichenketten im Code) bis zur nächsten Zeilengrenze mit
  geschlossenen Klammern, sonst hinter den Absatz.
- **EPUB-Umschlag:** siehe „Bilder, Umschlag und PDF (Runde 11, Migrationen 0017 und 0018)“; `epub::epub_cover` →
  `(Option<EpubImage>, Vec<EpubWarning>)`, Warnung `COVER_RASTER_FAILED` (vorn in `EpubBook.warnings`, `book_for`),
  Rückfall die rohe Bilddatei. `EpubInput` bleibt unverändert (Typen seit R20-S1 eingefroren).
- **`pdfinfo`:** Alle Typst-PDFs (Buch, Stichwortzettel, Karte, Umschlag) tragen `/MarkInfo << /Marked true /Suspects false
  >>` (krilla 0.8.2, `chunk_container.rs`: "We always set suspects to false because it's required by PDF/UA"). poppler 25.03
  meldet jeden Wert außer `true` als "Syntax Error: Suspects object is wrong type (boolean)". Im Test gezeigt: dieselbe
  Datei mit `/Xuspects false` (gleiche Länge, Querverweise korrekt) meldet nichts. Nicht behoben (das PDF ist gültig;
  PDF/UA verlangt den Wert).

## Runde 22 – Fassungen und Spiegel {#round-22--revisions-and-mirror}

Migration 0050, Fortschreiben (`baruch_core::revision_policy`, `baruch-store/src/amend.rs`), Seitenzweige, erneute Prüfung
erledigt gemeldeter Befunde, Archiv, Entwurf als Wasserzeichen, Austausch-Nebenversion 8.

### fassungen {#fassungen}

#### Migration 0050 (Runde 22) {#migration-0050-round-22}

`0050_revision_amend.sql`: `revision.amend_seq` (Stand, 0 = nie fortgeschrieben) und `revision.based_on_amend` (Stand der
Grundlage eines Vorschlags); Tabelle `revision_amend` (je Schritt: `revision_id`, `seq` ab 1, `kind` `amend`|`undo`,
`undo_of` genau für `undo`, `body_before`, `body_after`, `author_name`, `created`, `uuid` mit Füll-Trigger),
Grabstein-Trigger (Art `revision_amend`, Werk-uuid über revision→node→work) und Kaskade
`revision_amend_before_revision_delete` (BEFORE DELETE ON revision, damit der Grabstein das Werk noch kennt);
`work_revision_policy` (je Werk: `amend`, `max_change_percent` 1–100, `new_after_minutes` 0–1440, ohne uuid, nicht im
Austausch, geht mit dem Werk); `finding.adopted_amend`, `finding_status.adopted_amend`; `uuid_tombstone` mit der
zwanzigsten Art `revision_amend` (Umbau wie 0049, ohne `FOREIGN_KEYS_OFF`). Auf Verlustfreiheit geprüft in
`crates/baruch-store/tests/migration_0050.rs` (eine auf Stand 0049 aufgebaute Datenbank; vor 0050 listet der CHECK
neunzehn Arten – der Vertrag sagte 21).

#### Fortschreiben (Runde 22) {#amending-round-22}

- `baruch_core::revision_policy`: `RevisionPolicy` (Standard an/20 %/30 min, `validate`), `SaveMode` (`from_param`: fehlt →
  `New`), `NewReason` (`id`, `label`), `SaveFacts`, `decide` (Reihenfolge: Auftrag verlangt → no_current → policy_off →
  not_own → not_on_current → heading_changed → large_change → time_elapsed; `None` = fortschreiben), `change_percent`,
  `word_diff` (Wortunterschied für den Verlauf).
- `baruch-store/src/amend.rs`: `verdict` (Fakten aus der Datenbank: aktuelle Fassung, Autor, Grundlage, Minuten seit
  max(Anlage, letztes Fortschreiben), Einstellung), `write_step` (Text ersetzen, `amend_seq` + 1, Zeile in
  `revision_amend`, `edited` bleibt), `Store::amend_history`, `amend_undo`, `revision_policy`, `set_revision_policy`,
  `save_human_mode`/`save_human_with_reason` (RPC `revision.save`), `outline_save_mode` (→ `outline::save_auto`:
  Überschrift/Ebene/Teilen/Zusammenführen → `heading_changed`, sonst die Regeln). `write.rs`:
  `add_proposal`/`add_proposal_on` setzen `based_on_amend`. `tree.rs`: `RevisionInfo.amend_seq`,
  `RevisionInfo.based_on_amend`, `Store::side_revisions` (Seitenzweige). Die alten Funktionen (`save_human`,
  `save_human_on`, `outline_save`) bleiben „neue Fassung“.
- Server: `src/revisions.rs` (RPC `revision.amends|amend_undo|policy_get|policy_set`, Seite „Verlauf“, Formulare),
  `rpc::save_mode`/`rpc::add_saved` (Antwortfelder `saved`, `amend_seq`, `reason`, `reason_label`), `outline.rs`
  (`mode`, `as_new`, Rückmeldung `saved=` in der Weiterleitung), `mirror.rs` (`describe` für Amended/AmendUndone,
  `describe_revision_policy`, Seitenzweige), `view.rs` (Stand, Verlaufslinks, „weitere Fassung(en)“, Zeichen "+n",
  Rückmeldung), `settings/work.rs` (Block „Fassungen“), `mcp/revisions.rs` (`revision_amends`). Oberfläche:
  `templates/fragments/editor.html` (zwei Schaltflächen, `mode=auto`), `templates/revision_history.html`,
  `static/revisions.css`; `workspace.ts` übersetzt `as_new` in `mode: "new"` und zeigt die Rückmeldung,
  `outline.ts`/`wysiwyg.ts` geben beim erneuten Absenden die auslösende Schaltfläche weiter.

#### Seitenzweige im Spiegel (Runde 22) {#side-branches-in-the-mirror-round-22}

`baruch_gitmirror::Mirror::sync_side_refs(prefix, base, refs)` schreibt je `SideRef` (Name, Pfad, Inhalt, Autor,
Committer, Nachricht) einen Commit aus dem Snapshot `base` mit ersetzter Datei, Elternteil HEAD, und setzt die Referenz
(ohne Arbeitsordner, Index, HEAD); unveränderte (gleicher Inhalt an der Stelle, gleiche Nachricht) bleiben, fehlende werden
entfernt; Pfad- und Namensprüfung vor jedem Schreiben. `side_refs`, `read_file`, `parents` lesen nur. Der Server
(`mirror::commit_revisions`, `mirror::side_refs`) aktualisiert `refs/revisions/<key>/<n>` aus `Store::side_revisions` nach
jedem Fassungs-Commit, beim Start (`sync_work`) und nach `revision.reject` (`mirror::sync_side_refs`). Fehler der
Seitenzweige gehen nur nach stderr; der Commit zählt.

### befund-rueckfrage {#befund-rueckfrage}

#### Runde 22 – befund-rueckfrage {#round-22--befund-rueckfrage}

- **Erneute Prüfung erledigt gemeldeter Befunde:** `assist::context::is_recheck` (Prüfung `Mode::Review`, ↻ Wiederholen
  `links.repeat_of`, RFC `OrderFrame::recheck`, nie ein Vorschlag) → `own_finding_lines`: die eigenen offenen Befunde der
  Persona am Abschnitt (Bereich `chapter`: am Kapitel), offen, nicht angehängt, kein Lob, die letzten 20, gerahmt
  (`<<<befund … >>>`, also nicht im Korpus der Zitatprüfung) unter `prompts::OWN_FINDINGS_TITLE`; im Systemteil
  `prompts::RECHECK_RULE`. Beim Kürzen fallen sie zuletzt weg (älteste zuerst). `Prepared::own_findings` nennt die Ids.
- **Zerlegung:** `findings::parse` behandelt Zeilen „Erledigt: …“ (`done_line`, nach Listen- und Markdown-Zeichen, auch
  fett) und den Satz `prompts::ALL_DONE_SENTENCE` wie Vermerke – kein Befund, Orte der anderen Befunde ohne Lücke (also
  `finding_spans` unverändert). `done_claims` liest die Nummer („Befund n“, „Befund Nr. n“, „[Bn]“, „Bn“) und den Ort
  (erstes Zitat).
- **Statusvorschlag:** Nach `findings::ingest` ruft `assist::save` unter der schon gehaltenen Schreibsperre
  `assist::propose_done` → `findings::rpc::propose_held` (neu, wie `propose` ohne eigene Sperre) →
  `Store::finding_propose_status(…, StatusChange::Done {reason}, IdeaOrigin::Persona, note)`; ein Commit je Vorschlag.
  Übersprungene (andere Persona, unbekannt, entschieden, eine Rückfrage schon offen) bekommen einen Vermerk in
  `Outcome::done_notes`.
- **Pflichtfeststellung:** `prompts::critical_waived(text, had_own)` – nur wenn die Persona ihre eigenen offenen Befunde
  gesehen hat (`Retries::had_own`) und die Antwort den Satz sagt; dann keine Nachforderung und kein
  `CRITICAL_MISSING_NOTE`.
- **Stand an Befunden:** `Store` (`findings.rs`): `Target::adopted_amend` aus `revision.amend_seq` beim Auflösen von
  `Adopted`; `write_status` schreibt `finding.adopted_amend`, `finding_set_status`/`finding_propose_status` schreiben
  `finding_status.adopted_amend`, `finding_decide` beim Bestätigen den Stand zu diesem Zeitpunkt. `findings::adopted_words`
  („Fassung n · Stand k“) für den Spiegel und `findings::view::status_words_amend`.
- **Oberfläche:** `findings::view::entry_json` setzt `question` (offener Vorschlag `done`); das Makro `decide_forms`
  (`fragments/findings_panel.html`) und `finding_block.html` zeigen dann „Rückfrage“ mit „Ja, erledigt“ / „Nein, noch
  offen“ (Formulare an `/werke/befunde/act`, `data-finding-form`; in der Befundkarte des Chats trägt der Kasten
  `data-widget="findings"`).
- **Archiv:** `Store::restore_retired` (eine Transaktion: `work_retired` prüfen, Titel frei, Zeile löschen,
  `import::import_typst` mit dem Titel aus `work_retired` und dem Vermerk `works::RESTORED_NOTE`, den reservierten Slug
  setzen). Server `works.rs`: `archive_folder`, `read_archive` (Pfadschutz, Zusammensetzen wie der Export,
  `strip_back_matter`, `strip_projection`/`strip_first_mentions` wie `import.rs`), `archives`, `restore` (Sperre → lesen →
  Store → `rename` nach `works/<slug>` → Commit „Werk wiederhergestellt“ mit `allow_empty`), Seite `works_archive.html`.
- **Spiegel der Einfälle ohne Werk:** `mirror_ideas::sync_ideas_without_work` (Commit des Menschen, `allow_empty: false`,
  Trailer `Baruch-Action: ideas-synced`; ohne Einfälle ohne Werk und ohne Ordner nichts) aus `sync_all` (Start) und
  `works::delete` (nach dem Protokoll-Commit; Antwort `ideas_commit`).

### spiegel-entwurf {#spiegel-entwurf}

#### Entwurf als Wasserzeichen (Runde 22) {#draft-as-a-watermark-round-22}

- **Modell:** `baruch_typst::typeset::Draft` (S1) als Feld `TypesetSettings.draft`; `validate`/`normalized` prüfen und
  stutzen (spiegel-entwurf). `rules` setzt dafür keine Zeile (der Entwurf wird wie `xref_with_page` herausgenommen),
  `settings_from_rules` kennt ihn nicht – Export, Exportkarte, Spiegel und Regelblock bleiben byte-identisch.
- **Regel:** `baruch_typst::typeset::draft_rule(&Draft, Option<&str>)` (`typeset/draft.rs`) liefert `// Baruch: Entwurf (nur
  in den Ausgaben)` + `#set page(foreground: …)`: Text, Zusatz und Datum nur als maskierte Typst-Zeichenketten
  (geschützte Leerzeichen gegen Umbrüche), Deckkraft als Alpha von `rgb(…)`, Lage diagonal (−45°, Größe aus der Zeichenzahl,
  höchstens 80 pt) oder Kopf-/Fußzeile (7 mm vom Rand). `foreground`, weil Hintergrund, Kopf- und Fußzeile schon der Vorlage
  und den Satzregeln gehören (die Seitenzählung ab dem ersten Kapitel steht im Hintergrund). Das Datum bestimmt nur der
  Server (`typeset::draft_today`, Ortszeit) und gibt es hinein.
- **Server:** `typeset::prepare_with(state, work, draft: Option<bool>)` setzt die Regel mit `cover::insert_parts` nach
  Werkvorlage und Regelblock **vor** den Rahmen (so tragen sie auch Umschlagseiten und Vorspann im Buch-PDF) und verschiebt
  die Karte – Diagnosen bleiben beim richtigen Abschnitt. `effective_draft` (dreiwertig), `cover_draft` (Umschlag nur mit
  `draft.cover` oder ausdrücklich), `draft_param`/`draft_query`/`draft_pair` für RPC und Routen. Stichwortzettel und
  Umschlag-PDF setzen die Regel vor die erste Zeile ihrer eigenen Quelle; das EPUB bekommt den Entwurf über
  `EpubInput.typeset.draft` (Datum vom Server in den Zusatz geschrieben, baruch-epub bleibt ohne Uhr). Druckprüfung
  `cover_print::cover_checks_with(…, draft)` mit Code `draft` (`CODES` nun 11).

#### Austausch-Nebenversion 8 (Runde 22) {#exchange-minor-version-8-round-22}

- `baruch-exchange`: `FORMAT_VERSION = 8`; `ExchangeRevision.amend_seq/based_on_amend/amends` (`ExchangeAmend`,
  `ExchangeAmendKind`), `ExchangeFinding(Entry).adopted_amend`, Grabstein-Art `Amend` („fortschreibung“ ↔
  `revision_amend`, ab 8), `typeset::draft_attrs`/`draft_from_attrs` für `<entwurf/>`; `sample::sample_document_u8` (das
  alte Muster bleibt ohne u8-Teile). XSD `docs/schema/baruch-werk-1-u8.xsd`, u7 eingefroren.
- Store: `exchange_dump` liest Stand, Verlauf und `adopted_amend`; `exchange_apply` setzt sie im Nachlauf `apply_u8` nur an
  eben angelegten Zeilen, schreibt bekannte Fassungen vor (`fast_forward`, Regel d0) und baut für einen neuen Teil mit
  übernommenen Kapiteln einen Antrag `Add` mit `requests::PartIdentity` (uuid, Fassungen einschließlich Verlauf). Bei
  Zustimmung ruft `requests::run` nach `outline::add_many` `exchange_apply::adopt_part` auf: Die erzeugte Fassung 1 entfällt
  (einschließlich ihres Grabsteins, sie hat außerhalb der Transaktion nie existiert), `node.last_rev` auf 0, dann Kennung
  und Fassungen aus dem Antrag. Nur `exchange_apply.rs` schreibt uuids (der Wächtertest bleibt grün).

#### Spiegel (Runde 22) {#mirror-round-22}

- **Show-Regel über die Sprungmarke:** Ein Knoten „ohne Nummer“, dessen Überschrift sich nicht markieren lässt (Titel mit
  Code), mit gleicher Überschrift wie ein nummerierter Knoten, bekommt statt der Regel über Ebene und Titel
  `#show <kN>: set heading(numbering: none)` unter `// Baruch: Überschriften ohne Nummer über die Sprungmarke`
  (`snapshot::label_rules`, vor dem Block des Rückfalls). Mit eigener Marke bleibt die Grenze. Rückweg beim Import:
  `snapshot::split_label_fallback` in `import.rs` (die genannten Überschriften kommen „ohne Nummer“ zurück).
- **Satz des Spiegels:** Abschnittsdateien liegen in `abschnitte/`; der Spiegel schreibt relative Bildpfade
  `image("bilder/x")` von der Wurzel aus (`image("/bilder/x")`, `snapshot::root_image_paths`), damit Typst `main.typ`
  einschließlich `#include` setzen kann (Test `the_mirror_main_typ_with_its_includes_sets_like_the_export`: dieselbe
  Seitenzahl wie der Export). Dokumentierte Abweichungen: Rasterbilder liegen im Spiegel als `bilder/<name>.b64`
  (Base64-Text) und müssen vor dem Satz zurückgewandelt werden; ein Bildpfad, der nicht `bilder/<Name des Bilds>` ist,
  steht im Spiegel nicht unter diesem Pfad; die Teilseitenregel und der Rahmen (Umschlag, Vorspann) gehören nur zum Satz,
  nicht zu Export und Spiegel.

## Runde 23 – Härtung {#round-23--hardening}

Keine Migration (Store weiter bis 0050, Bibliothek bis 0108), keine neue Nebenversion des Austauschformats, keine neuen
RPC-Methoden oder MCP-Werkzeuge.

### Web {#web}

**Lesende Anfragen fremder Seiten (`guard::own_reads`).** Die Schicht liegt über allen Routen und lehnt ein GET oder HEAD mit
403 „Abgelehnt: Anfrage von einer fremden Seite (lesend)“ ab, wenn `Sec-Fetch-Site` `cross-site` oder `same-site` ist und
die Anfrage keine Navigation des obersten Fensters ist (`Sec-Fetch-Mode: navigate` mit `Sec-Fetch-Dest: document`). Bild,
Skript, `fetch` und Rahmen einer fremden Seite erreichen damit keine lesende Route mehr (z. B. `GET
/einstellungen/assistenten?modelle=abfragen`, Stichwortzettel); ein Link von außen öffnet Baruch weiter. Ohne
`Sec-Fetch-Site` (curl, MCP-Clients, alte Browser) gilt die Anfrage als unsere eigene. Zwei Abgrenzungen: (1) Dateien der
EPUB-Vorschau (`/werke/{id}/epub/leser/…`) stehen in einem Rahmen mit `sandbox`; der Browser meldet ihre Bilder und
Sprünge als `cross-site`. Dort entscheidet weiter die Route selbst über den `Referer` (`epub_preview::from_own_frame`).
(2) Fehlen `Sec-Fetch-Mode` und `Sec-Fetch-Dest` **beide**, ist die Art nicht belegt, und die Anfrage gilt nicht als
Unterressource – kein Browser sendet `Sec-Fetch-Site` ohne `Sec-Fetch-Mode`; die Routen mit eigener Prüfung
(`cross_site_get`) lehnen eine solche Anfrage weiter selbst ab.

**Meldungscodes.** Seiten zeigen nie Text aus der Adresse als Meldung, nur feste Codes (`?ok=<code>`, `?fehler=<code>`,
`?hinweis=<code>`), die der Server auf feste Sätze abbildet. Die letzte Ausnahme (`?ok=sicherung&ziel=<Ordner>`) ist
entfernt: `settings::newest_backup` liest den Ordnernamen aus dem Dateisystem.

**Zeichen im Kopf.** Außerhalb der Werkseite markiert `base.html` die Zeichen mit `data-badge-fill` und die Menüleiste mit
`data-badges-work="<Werk>"`. Die Schicht `consent::inject` (über allen Routen, mit `AppState`) ruft nach der Hinweisleiste
`menu::fill_page` auf, die die Zahlen aus `menu::badges` in das fertige HTML einsetzt (`menu::fill_badges`) und die Marke
entfernt. So braucht keine der rund dreißig Template-Umgebungen eigene Daten.

**Fortschreiben und Ändern über den Baum.** `Store::update_revision_at` (und die alte Hülle `update_revision`) legt für die
aktuelle oder eine schon fortgeschriebene Fassung mit geändertem Text einen Schritt in `revision_amend` an (Art `amend`,
keine Migration) und erhöht `revision.amend_seq`; `Change.amend_seq` nennt den neuen Stand, die Art der Änderung bleibt
`Updated` (Spiegel-Commit wie zuvor). So hat die Kette der Schritte keine Lücken (Vorschreiben beim Abgleich, Austausch),
und ein Editor mit altem Stand bekommt `NotOnCurrent`, statt still zu überschreiben. Mit `expected_amend` lehnt der Store
einen veralteten Stand ab (`StoreError::Stale`, „Veraltet: …“).

**Satzansicht: Zitatblock** (auch `docs/WYSIWYG.md`). `baruch_wysiwyg::parse` liest eine logische Zeile, die genau ein
`#quote(block: true, …)[…]` ist, als eigenes Stück (wie `#szene[…]`/`#arbeit[…]`): Eine Zeile direkt danach ist ein eigener
Absatz, eine Zeile direkt davor beendet ihren Absatz – wie Typst setzt. `#quote[…]`, `block: false` und unlesbare Argumente
bleiben Teil ihres Absatzes. Unberührte Blöcke gehen byte-identisch zurück. Die Schaltfläche „Zitat“ fügt
`#quote(block: true)[]` mit der folgenden Zeile "– " ein, nicht `attribution: []`: Eine leere `attribution` setzt in Typst
0.15.1 eine Zeile mit dem Geviertstrich (gemessen in `crates/baruch-render/tests/quote_attribution.rs`: Mit `attribution:
[]` steht ein "—" in einer eigenen Zeile mehr als 5 pt unter dem Zitat, ohne `attribution` keiner).

**Editor: Sperre beim Speichern.** `widgets.ts` (`lockEditor`, `editorLocked`) sperrt den Bereich `#pane-editor` (`inert`,
`aria-busy`) vom Drücken auf „Speichern“ bis nach dem Neuzeichnen; `outline.ts` und `workspace.ts` zählen ihre Sperren, ein
zweites Absenden während des Laufs wird verworfen. `workspace.ts` verwirft veraltete `ui.render`-Antworten über eine
laufende Nummer (die zuletzt ausgelöste Navigation zählt).

**Einfallskarte.** Die Nähe (`idea_map::near::compute`) rechnet in `spawn_blocking` und höchstens für die ersten
`MAX_NEAR_IDEAS` = 1500 Einfälle nach Nummer (rund 1,1 Millionen Paare); darüber sagt es die Meldung der Karte.

**Umschlag.** `cover::compile_limited` setzt die Messquellen (`book_page_mm`, `block_heights`) mit der Zeitgrenze des Satzes
(`cover::MEASURE_TIMEOUT` = 60 s); danach wird der Lauf als abgebrochen markiert und das Ergebnis verworfen, `block_heights`
meldet -32000 „… Typst wurde nach 60 Sekunden nicht fertig (Zeitgrenze).“ Die Kontrastprüfung entpackt das Umschlagbild mit
Grenzen wie die Vorschaubilder (`cover_print::MAX_CONTRAST_SIDE_PX` = 20 000, 512 MiB) und merkt sich bis zu acht
entpackte Bilder (zusammen höchstens 96 MiB RGB) nach Prüfsumme. `default_blocks` rundet Lage und Breite auf 0,01 mm.

**EPUB.** Fußnoten bekommen ihren Platz in der Liste, bevor ihr Inhalt geschrieben wird (innere Fußnote nach der äußeren).
`collect_labels` zählt Abbildungen (`#figure(image(…) …)`, allein oder mit einer Marke danach) in Lesereihenfolge
(`Links::figures`); `@name` zeigt „Abbildung n“ (oder das eigene Wort des Verweises) und verlinkt auf `<figure
id="lbl-name">`. `check_mimetype` entpackt ein komprimiertes `mimetype` und prüft auch dessen Inhalt (`MIMETYPE_CONTENT`
zusätzlich zu `MIMETYPE_COMPRESSED`).

**Satzmeldungen aus dem Anhang.** `ExportMap::tail_first_line`/`in_tail`: Zeilen nach dem letzten Abschnitt (Register,
Glossar) gehören keinem Abschnitt mehr; `typeset::Typeset::diagnostics` nennt sie „Buchende (Register, Glossar)“ mit der
Zeile im Anhang.

**Befunde.** `finding_list`, `finding_attached_to` und `findings_by_uuid` holen die offenen Vorschläge in einer Abfrage je
500 Befunde (vorher eine je Befund). `Finding::stale_summary` ist abgeleitet (ein offener Sammelbefund ohne offene
Herkunft; gelöschte Herkünfte zählen als entschieden) und wird nie gespeichert. `finding_open_counts` und die Matrix zählen
offene Sammelbefunde nicht.

**Bibel und Kommentar in der Verszählung des Moduls.** Die Belege aus SWORD (`bible_research::collect`) lesen die Stelle über
`bibles::read_passage` (Zuordnung german() → Verszählung des Moduls) wie die Prüfung und `bible.lookup`; Kommentare über
`bibles::read_commentary` (dieselbe Zuordnung, mit dem Namen der Verszählung beschriftet). Die Wortlautprüfung nennt fehlende
Verse in deutscher Verszählung (zurückgebildet), sonst mit dem Namen der Verszählung des Moduls.

**Band.** `ribbon::defined_functions` wertet nur Code der obersten Ebene aus (`code_lines`: ohne Zeilenkommentare und – auch
verschachtelte – Blockkommentare, ohne Rümpfe anderer Funktionen, Zeichenketten bleiben ganz) und kennt `#import "datei.typ":
name, a as b`.

### mcp-namen {#mcp-namen}

- **Handles** (`mcp/session.rs`): `handle_name`, `legacy_handle_name`, `LOCAL_PREFIX`, `local_name` (reine Funktionen),
  `Client::handle`, `Client::owned_handles`. `mcp.rs::localize` grenzt Clients ohne Token bei jeder Anfrage gegen die
  Tokenliste ab (eine kleine Abfrage je Anfrage).
- **Tokens** (`store/mcp_tokens.rs`): `mcp_token_create_unique(name, handle, legacy, reserved_prefix)` prüft und legt in
  einer Transaktion `BEGIN IMMEDIATE` an; der Server gibt die Regeln hinein (`mcp/tokens.rs::create`, der einzige Weg von
  RPC und Formular). `mcp_token_create` (ohne die Handle-Prüfung) bleibt für Tests und vorhandenen Code.
- **Eigentum** (`posts.rs`): `Actor` trägt `also` (weitere eigene Autorennamen, das alte Handle eines Tokens) und `owns()`;
  `posts::edit`, `posts::act`, `threads::delete_post` und `threads::edit_post_as` prüfen darüber.
- **Kontingent** (`store/asset_requests.rs`): `asset_mcp_usage_without_token(work, token_handles)` zählt alle
  Bildfassungen, deren Autor mit "@" beginnt, und alle Anträge von Personas und MCP-Clients, ohne die Autoren der Tokens.
- **Obergrenzen im Store:** `glossary::MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN` (200), `…_PER_WORK` (2000),
  `requests::MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN` (50; nicht beim Lesen einer Austauschdatei und nicht für den Menschen).
- **Relais** (`assist/chat.rs`): `relay_targets` wählt die Ziele, `ChatOrder::relay` schaltet Erwähnungen ein (MCP).
  `relay_section` in `mcp/tools.rs` ist entfernt. Fäden ohne Abschnitt haben in `threads.rs` weiter eine eigene Schleife mit
  denselben Grenzen.
- **Auftrag eines Beitrags** (`store/post_meta.rs`): `PostRequest` + `web`, `asker`, `relay`;
  `assist::FramedAnswer::{to_request, from_request}`.
- **Nachtrag, vorhandene Tokens** (`mcp/session.rs`): `keeps_legacy(name, token_names)` und `token_handle` (reine
  Funktionen), `Client::keeps_legacy`. Ein Token, dessen neues Handle nur durch die Regel seit Runde 23 gleich dem Handle
  oder alten Handle eines Tokens mit anderem alten Handle würde, schreibt und besitzt weiter nur unter seinem alten Handle.
  `mcp.rs::localize` ermittelt das für Tokens bei `initialize` und bei jeder Anfrage aus derselben Tokenliste (kein
  gespeicherter Zustand, keine Migration); die Seite „MCP-Zugang“ (`settings/access.rs`) zeigt `taken`.
- **Nachtrag, Kontingente ohne Token:** `mcp/quota.rs::acquire(state, &Client)` belegt je Auftrag den Platz des Handles
  (höchstens `MAX_PER_CLIENT` = 2) und ohne Token zusätzlich einen der gemeinsamen Plätze (`WITHOUT_TOKEN`, höchstens
  `MAX_WITHOUT_TOKEN` = 4), beides unter einer Sperre; `Slot` gibt beide frei. `mcp/tools.rs::Counted` wählt für `add_post`
  und `propose_revision`, wessen Bestand zählt: `Author` (Token oder genannte Persona) oder `WithoutToken` (alle Autoren
  `@mcp:…` außer denen der Tokens, `token_handles`, auch vom Bildkontingent genutzt).
- **Kontext** (`assist/context.rs`): `kept_own` aktualisiert `Prepared::own_findings` nach dem Kürzen.
- **Varianten** (`assist/variants.rs`): `splice` ersetzt nur die Byte-Bereiche der betroffenen Absätze
  (`paragraph_spans`).
- **Befunde beim Verschieben** (`store/posts.rs::post_move`): ein `UPDATE finding` in derselben Transaktion.

### geheimnisse {#geheimnisse}

- **Geheimnisse an einer Stelle schwärzen (Runde 23).** `baruch_llm::scrub(text, secret)`
  (`crates/baruch-llm/src/secret.rs`) ersetzt durch `***`: jedes wörtliche Vorkommen und – ab einer Geheimnislänge von
  6 Bytes (`SCRUB_ENCODED_MIN`) – die Prozentkodierung (je Byte wörtlich oder `%XX` mit Groß- oder Kleinbuchstaben als
  Hexziffern, Leerzeichen auch als `+`) und Base64 (Standard und URL-sicher, mit oder ohne `=`, auch als Teil eines
  längeren Base64 wie `Basic base64(user:geheim)`: für jeden Versatz 0/1/2 werden die Zeichen gesucht, die allein vom
  Geheimnis abhängen; die gemischten Randzeichen und Füllzeichen gehen mit). Kürzere Geheimnisse nur wörtlich. Nicht
  erkannt: Hex, JSON-Escapes, doppelte Kodierung, umbrochenes Base64. Aufrufer: `Endpoint::scrub` (Modell-Client),
  `connections::check::scrub`/`scrub_value`, `connections::multigpt::scrub_text`/`scrub_error` (und damit `Session::scrub`,
  Websuche, PDF-Abruf), `PgvectorWay::scrub` (über `multigpt::scrub_error`), `sync::Link::scrub` (über
  `multigpt::scrub_text` oder die Sitzung), `models::check_connection` (über `check::scrub_value`). Neue Abhängigkeit
  `base64 0.22` in `baruch-llm` (sie war schon im Lock).
- **Keine Weiterleitungen.** Alle drei HTTP-Clients mit Schlüssel (`connections/check.rs`, `connections/multigpt.rs`,
  `baruch-llm`) haben `redirect::Policy::none()`; ein 3xx ist ein Fehler mit eigener Meldung, `Location` wird nicht gezeigt.
  Tests je Client mit zwei Attrappen (die zweite sieht keine Anfrage).
- **Filter vor `fetch_url`.** `websearch::fetch::fetch_allowed(url) -> Result<(), &'static str>` prüft nur den Wortlaut der
  Adresse: Schema http/https, keine Benutzerangabe, kein IP-Literal in Loopback, privat, Link-Local, CGNAT, unspezifiziert,
  Multicast, Broadcast, 0/8, 192.0.0.0/24, 198.18.0.0/15, 240/4; IPv6 `::1`, `::`, `ff00::/8`, ULA `fc00::/7`,
  `fe80::/10`, `fec0::/10`, dazu IPv4 in IPv6 (`::ffff:`, `::a.b.c.d`, NAT64 `64:ff9b::/96`, 6to4 `2002::/16`) und das
  ganze `64:ff9b:1::/48`; `localhost`, Namen auf `.localhost`/`.local`/`.internal`/`.home.arpa` und (über den Vertrag
  hinaus) Namen ohne Punkt. Der URL-Parser normalisiert vorher dezimale, hexadezimale, oktale und verkürzte
  IPv4-Schreibweisen. **Keine DNS-Auflösung** – Namen, die auf private Adressen zeigen, erkennt nur die abrufende Seite; ob
  MultiGPT selbst filtert, ist unbekannt (**Annahme**: nein). `websearch::search::gather` ruft abgelehnte Treffer nicht ab,
  sie verbrauchen keinen der vier Abrufe und erscheinen als Hinweis. `websearch::fetch::set_fetch_exceptions(state, hosts)`
  ist ein Ausnahmeweg je Datenordner nur für Tests (kein RPC, kein Formular, keine Einstellung führt dorthin).
- **Modell-Cache je (Art, Adresse)** (`models.rs`: `CACHE`, `CONTEXTS`); `expire_list(url)` betrifft alle Arten der
  Adresse, `remember(url, …)` gehört zur Verbindung `eliezer`. `models::context` prüft `external`/`external_allowed` vor
  jeder Anfrage.
- **Predigt mit Bibel:** `Store::create_work_with_bible(new, human, sermon_text, bible)` schreibt `work_bible` in der
  Transaktion der Anlage; `create_work_with_text` ruft es mit `None` auf.
- **Temp-Ordner von EPUBCheck** (`epub_check::TempDir`): `baruch-epubcheck-<32 Hex-Zeichen aus getrandom>`, unter Unix
  `DirBuilder::mode(0o700)`, ohne `recursive` – ein vorhandener Ordner, eine Datei oder ein Symlink gleichen Namens ist ein
  Fehler („Temp-Ordner für EPUBCheck: …“) und wird weder benutzt noch aufgeräumt.
- **SSE-Parser** (`baruch-llm/src/sse.rs`): Die Grenze `max_line` gilt auch für die Summe der `data:`-Zeilen eines Ereignisses
  („Ein Ereignis im Antwortstrom ist zu groß; abgebrochen.“).

### Integration {#integration-3}

- **Router:** `guard::own_reads` liegt als Schicht über allen Routen (unmittelbar vor `guard::local_host`, Schnittstelle
  R23-S1b). `/mcp` hat eine eigene Größengrenze `MCP_BODY_LIMIT` (12 MiB). `/act` kennt `based_on_amend`.
- **Datenordner:** Unter Unix legt `baruch_store::Store::open` einen neuen Datenordner mit 0700 an (`create_data_dir`,
  Codeprüfung quellen-5); ein vorhandener bleibt unverändert.
- **Abnahme:** `tests/round23_abnahme.rs` (Zitat bis zu Tokens in einem Zug; Geheimnisse, Weiterleitung, Websuche).

## 0.9.0 – ältere Fassungen ausblenden (Nutzer 2026-11-24) {#090--hiding-older-revisions-user-2026-11-24}

- **Migration 0051** (`0051_revision_fold.sql`): Tabelle `node_revision_fold` (`node_id` als Primärschlüssel mit `ON DELETE
  CASCADE`, `below` ≥ 1 = Nummer der Fassung, an der die Marke steht, `updated`, `updated_by`). Ohne uuid, nicht im
  Austauschformat (Arbeitsweise der Instanz wie `work_revision_policy`), kein Commit im Spiegel (kein Buchtext). Auf
  Verlustfreiheit geprüft in `crates/baruch-store/tests/revision_fold.rs` (Datenbank auf Stand ed9d210, Store bis 0050).
- **Regel** `baruch_core::revision_fold`: `cutoff` = Marke, höchstens die aktuelle Fassung; ausgeblendet werden ersetzte und
  verworfene Fassungen unterhalb von `cutoff` (`is_hidden`, `hidden_count`); nie die aktuelle, nie offene Vorschläge.
  `offered` = „bis hierhin ausblenden“ nur an der aktuellen Fassung und darunter, wenn darunter etwas auszublenden ist.
- **Store** `baruch-store/src/revision_fold.rs`: `revision_fold`, `work_revision_folds` (Schlüssel → Marke),
  `set_revision_fold` (oberhalb der aktuellen `Refused`), `clear_revision_fold`, `clear_work_revision_folds`,
  `fold_revisions_before` (werkweit, letzte Fassung vor einem UTC-Zeitpunkt, höchstens die aktuelle).
- **Server** `src/revision_fold.rs`: RPC `revision.fold_get|fold_set|fold_clear|fold_before|fold_clear_all` (kein
  MCP-Werkzeug), Formular `POST /einstellungen/werk/fassungen/ausblenden`; `/act` nimmt `fold_set`/`fold_clear`. `view.rs`:
  `ViewExtras.revision_folds`, `Location.fassungen` (`alle` = Schalter offen, `Resolved.show_all`, `FOLD_OPEN_Q` in
  `Resolved::url`), `FoldView` (Zähler und Nachbarn über die sichtbaren Fassungen, Links für den Schalter, Fassungen mit
  Angebot), `TreeRevision.hidden`/`fold`. Templates: `fragments/editor.html` (Zusatz am Zähler, „bis hier ausblenden“),
  `fragments/revisions.html` (Formulare `rev-fold-<n>`, „Wieder alle zeigen“, Schalter `<details class="rev-fold">`),
  `fragments/revtree.html` (Makro `folded`), `settings/werk.html`; CSS in `static/revisions.css`; `web/src/rpc.ts` reicht
  `fassungen` an `ui.render` durch.

## Runde 33 – Band {#round-33--ribbon}

Der Kopf mit „Werk ▾“ / „Ansicht ▾“ / „Werkzeuge ▾“ und die flache Schaltflächenreihe über dem Blatt werden durch ein Band
ersetzt: ein Menü **„Baruch ▾“** (Programm), Reiter **„Start · Einfügen · Prüfen · Buch · Ansicht“** mit Gruppen, und die
Schaltflächen des Editors docken im Band an. Keine neue Route, keine neue RPC-Methode, kein neues MCP-Werkzeug; `ui.render`,
`menu.badges` und `ribbon.formats` sind unverändert. Der Laptop-Modus (kompaktes Band, seitliche Leisten) gehört zu
`src/laptop.rs`/`web/src/laptop.ts` und ist dort beschrieben.

### Template (`templates/base.html`) {#template-templatesbasehtml}

Das Band steht vollständig in `base.html` (das Template ist in rund 35 eigene minijinja-Umgebungen eingebettet; ein
`{% include %}` müsste in allen registriert werden).

- `<header class="band" data-widget="band" data-tab="" data-work="W">` (die Klasse `topbar` gibt es nicht mehr), direkt
  darunter die Marke `<!--baruch-consent-->`.
- Zeile 1 (`div.band-top`): `nav.menubar[data-widget=menubar]` mit genau einem `details.menu[data-menu="baruch"]` („Alle
  Werke“, „Neues Werk / Importieren…“, „PDF oder EPUB importieren…“, „Austausch: Einlesen…“, „⚙ Einstellungen“, „Über
  Baruch“), die Reiterzeile `nav.band-tabs[role=tablist][data-band-default]`, die Werkwahl (`form.work-select`, nur
  Werkseite), `span.work-title`, der Autor und der Schalter `a[data-band-toggle]` (Band auf/zu). Die Reiterzeile steht im
  Dokument direkt nach dem Menü (nicht nach dem Titel): So bleibt sie auf jeder Seite an derselben Stelle, und die
  Tabulatorreihenfolge passt zum Bild.
- Zeile 2 (`div.band-panels`): je Reiter ein `div.band-panel[role=tabpanel]` und – nur auf der Werkseite – der Andockplatz
  `div.band-dock[data-band-dock]`.
- Reiter je Seite: Werkseite (`topbar` gesetzt) alle fünf; Seiten mit einem Werk „Start · Prüfen · Buch · Ansicht“; ohne
  Werk „Start · Ansicht“ (dort stehen „Literatur“ und „Abgleichen“ in einer Gruppe „Werkzeuge“ unter „Start“, weil sie kein
  Werk brauchen; „Werk wählen“ führt wie zuvor auf `/werke`).
- Blöcke der Seiten: `band_tab` (Standardreiter, sonst `start`), `page_actions` (Gruppen im Reiter „Start“, zwischen „Werk“
  oder „Ansicht des Textes“ und „Einfälle“), `band_einfuegen`, `band_pruefen`, `band_buch`, `band_ansicht` (je vor den festen
  Gruppen des Reiters).
- Plätze für Runde 32 als Kommentare des Templates: `{#- R32-PLATZ import-word -#}` und `{#- R32-PLATZ beenden -#}` (letzter
  Eintrag) im Menü „Baruch ▾“, `{#- R32-PLATZ export-word -#}` in der Gruppe „Ausgabe“.

**Wörtliche Formen.** Die Schicht ersetzt Zeichenketten; die Reihenfolge der Attribute gehört deshalb zur Schnittstelle (der
Unit-Test `the_template_carries_the_literal_forms_apply_relies_on` hält sie fest):

| Stück | Form |
|---|---|
| Reiter | `<a class="band-tab" role="tab" id="band-tab-ID" aria-controls="band-panel-ID" data-band-tab="ID" href="?tab=ID" aria-selected="false">` |
| Feld | `<div class="band-panel" role="tabpanel" id="band-panel-ID" aria-labelledby="band-tab-ID" data-band-panel="ID" hidden>` |
| Gruppe | `<details class="band-group" data-band-group open><summary class="band-group-label">Name</summary><div class="band-items" role="group" aria-label="Name">…</div></details>` – weitere Attribute nur zwischen `class="…"` und `data-band-group` (z. B. `data-band-after`: nach dem Andockplatz) |
| Band auf/zu | `data-band-toggle aria-expanded="true" href="?band=closed"` |
| Laptop-Schalter | `<a class="btn" data-laptop-set="auto" aria-pressed="false" href="?laptop=auto">Automatisch</a>` (ebenso `on`, `off`) |
| Seitenleisten | `<a class="btn" data-rail-toggle="left" aria-pressed="false" href="?lrail=1">Gliederung</a>`, rechts `right`/`rrail` |

### Schicht `band::apply` (`src/band.rs`) {#layer-bandapply-srcbandrs}

Seiten kennen ihre Adresse und `?tab=` nicht. Die Schicht `consent::inject` (über allen Routen, wie die Zeichen seit Runde
23a) behält vor `next.run` den Cookie-Kopf und die rohe Abfrage, baut `BandState::from_request` und
`LaptopState::from_request` und ruft nach `menu::fill_page` `band::apply(&page, &band, &laptop)` auf; gültige Schlüssel der
Adresse werden als `Set-Cookie` angehängt (`band::set_cookies`, `laptop::set_cookies`).

- `BandState { tab, closed, rest }`: `?tab=` nur für Kennungen aus `TABS` (nicht gemerkt), `?band=open|closed` vor dem
  Cookie `baruch_band` (`closed`; fehlt = offen), `rest` = rohe Abfrage ohne `tab`, `band`, `laptop`, `lrail`, `rrail` (nicht
  dekodiert). Ungültige Werte werden übersprungen; bei wiederholten Schlüsseln zählt der letzte gültige.
- `apply`, in dieser Reihenfolge; eine Seite ohne die Marke ` data-band-fill` bleibt unverändert:
  1. Marke an `<html>` → `laptop.html_attrs()` und `data-band="open|closed"`;
  2. aktiver Reiter: `?tab=`, wenn der Kopf ihn hat; sonst `data-band-default`, wenn vorhanden; sonst der erste;
  3. `data-tab=""` am Kopf → `data-tab="<aktiv>"`;
  4. Adressen der Reiter `href="?<rest>&tab=ID"`; der aktive zusätzlich `aria-selected="true"` und `&band=closed` (ein Klick
     schließt); bei geschlossenem Band tragen alle Reiter `&band=open`;
  5. `hidden` entfällt am aktiven Feld;
  6. kompakt (`laptop.compact()`): jedes ` data-band-group open>` → ` data-band-group name="band-group">` (Popups aus
     `<details>`, der gemeinsame Name hält nur eines offen);
  7. jedes `href="?K=V"` mit K ∈ {band, laptop, lrail, rrail} → `href="?<rest>&tab=<aktiv>&K=V"`;
  8. `aria-pressed` am zutreffenden Laptop-Schalter; Seitenleiste sichtbar → `aria-pressed="true"`, eingeklappt → Adresse auf
     `lrail=0` oder `rrail=0`; Band geschlossen → Schalter auf `band=open`, `aria-expanded="false"`.
- `node=` (alte Adressform) geht nicht in die erzeugten Adressen mit: Erzeugte Links tragen nie `node=` (Vertrag Runde 8
  §1). `rest` selbst bleibt die rohe Abfrage; eine Seite, die noch mit `?node=` aufgerufen wurde, verliert nur diese Stelle
  bei einem Reiterwechsel ohne JavaScript. Alle anderen Schlüssel bleiben – auch einmalige wie `?ok=gespeichert` (die
  Meldung steht dann nach einem Reiterwechsel ohne JavaScript noch einmal da).
- **Sicherheit:** `rest` stammt aus der Adresse und gelangt nur über `escape_attr` (`&`, `<`, `>`, `"`, `'`) in ein
  Attribut. Die Schritte 2 bis 6 wirken nur zwischen `<header class="band"` und `</header>`: Text im Seiteninhalt, der wie
  ein Reiter aussieht, ändert nichts. Die Schritte 7 und 8 gelten auf der ganzen Seite (auch für die schmalen Leisten der
  Werkseite); ein Wert V mit anderen Zeichen als `[A-Za-z0-9_-]` wird nicht angefasst.
- **Bekannte Grenze:** Auf einer Seite, die die Antwort auf ein POST ist (z. B. die Frage „Speichern bestätigen“), führt ein
  Reiterlink ohne JavaScript per GET auf dieselbe Adresse; mit JavaScript schaltet band.ts ohne Neuladen um.

### Zeichen {#badges}

Die Zeichen stehen an den Reitern: offene Vorschläge an **„Start“**, „N offen“ (Befunde) an **„Prüfen“**, offene
Gliederungsanträge an **„Buch“**; dazu die Zahl an den Einträgen „Befunde“ und „Anträge zur Gliederung“. Die Haken sind die
alten (`data-badge`, `data-badge-item`, `data-badge-fill`, `data-badges-work`): Die Werkseite zeichnet aus `topbar.badges`,
jede andere Seite füllt `menu::fill_badges` im fertigen HTML (ohne JavaScript), `menu.ts` frischt auf („N offen“ am Reiter,
nur die Zahl im Eintrag).

### Andocken der Editor-Schaltflächen {#docking-the-editor-buttons}

Die Editor-Widgets hängen an `div.wysiwyg`, und `ui.render` zeichnet nur die drei Spalten neu. Darum bleiben die
Schaltflächen im Fragment `fragments/edit_toolbar.html` (Editor-Zweig, `div[data-edit-toolbar][hidden]` in `div.wysiwyg`)
und werden mit dem Editor neu gezeichnet; `ui.render` ist unverändert.

- Die Leiste hat zwei Teile: `div[data-band-part="start"]` (Gruppen Text: „Satzbild“ | „Quelltext“; Absatz: Absatzformat,
  „K“, „F“; Fassung: ‹ n/m ›, „Speichern“, „Als neue Fassung“ – `form="editor-form"`; Assistenten: „Prüfen“, „Vorschlag“)
  und `div[data-band-part="einfuegen"]` (Einfügen: Formate aus `ribbon.formats`; Bild; Register und Glossar). Alle Haken
  sind die bisherigen (`data-ws-mode`, `data-ws-tools`, `data-ws-format`, `data-ws-cmd`, `data-ribbon-cmd`,
  `data-ribbon-snippet`, `data-ribbon-image`, `data-widget="ribbon"`, `data-widget="context-menu"`, `data-ctx-action`,
  `data-index-*`). Die ‹ ›-Links im Band tragen kein `data-key` (Alt+←/→ nimmt weiter die Links im Kopf des Editors).
- `fragments/editor.html` endet in beiden Ansichten mit `<span data-widget="band-dock" hidden></span>`. Das Widget
  `band-dock` (band.ts) leert beim Einhängen `div[data-band-dock]` und setzt die Leiste des gezeigten Editors hinein
  (Vorschau: nur leeren); der Kopf trägt dann `data-docked`. Weil `mount` die Widgets in Dokumentreihenfolge einhängt, haben
  sich `wysiwyg`, `ribbon` und `context-menu` vorher mit ihrem Editor verbunden.
- Welcher Teil sichtbar ist, steuert `band.css` über `header.band[data-tab]`. Die Felder zeichnen keinen Kasten
  (`display: contents`): Gruppen und Andockplatz stehen in einer Reihe, der Andockplatz zwischen „Ansicht des Textes“ und
  „Einfälle“ (`order`).
- In den Widgets nachgeführt: `widgets.ts` – `editToolbar(root)` findet die Leiste am Blatt oder im Andockplatz (wysiwyg.ts
  nutzt es für den Wechsel Satzansicht/Quelltext), `submitButton` überspringt die Schaltflächen der Leiste („Speichern“ der
  Statuszeile bleibt maßgeblich), `lockEditor` sperrt auch den Andockplatz; `workspace.ts` hört auf `a[data-nav]` am
  Dokument (Schalter und ‹ › stehen im Kopf); `sheet.ts` merkt die Cursorstellung für „Bild“ über einen Listener am
  Dokument; `ribbon.ts` lässt die Pfeiltasten nur über erreichbare Schaltflächen laufen.
- „Prüfen“/„Vorschlag“ (`[data-band-assist]`) lösen die gleichnamige Schaltfläche von `form.assist-form` rechts aus;
  „Speichern“ im Band speichert nicht, solange die Schaltfläche der Statuszeile gesperrt ist (nichts geändert).
- Ohne JavaScript bleibt die Leiste am Blatt verborgen (wie zuvor: Textfeld und Statuszeile); der Reiter „Einfügen“ zeigt
  dann einen Hinweis (`nojs-only`) mit den Links „Bild einfügen …“ (`#bild-einfuegen`) und „Markierung …“.

### Widgets `band` und `band-dock` (`web/src/band.ts`) {#widgets-band-and-band-dock-websrcbandts}

- Reiter nach dem ARIA-Muster für Reiter: Ein Klick schaltet ohne Neuladen um, ←/→ (mit Umlauf) und Pos1/Ende bewegen und
  wählen, nur der gewählte Reiter liegt in der Tabulatorreihenfolge.
- Ein Klick auf den gewählten Reiter oder auf den Schalter klappt das Band zu (`data-band` an `<html>`, Cookie
  `baruch_band`); zugeklappt zeigt ein Reiter sein Feld vorübergehend über dem Inhalt (`data-band-peek` am Kopf) – Esc,
  ein Klick daneben oder ein ausgelöster Eintrag blendet es aus.
- Gruppen: voll, alle offen, die Beschriftung ist kein Schalter; kompakt (`data-compact` an `<html>`) Popups – nur eines
  offen, Esc und ein Klick daneben schließen. band.ts misst nie Breiten; es reagiert auf das Ereignis `baruch:laptop`
  (laptop.ts) mit `syncGroups`.
- Stil `static/band.css`: nur die Attribute an `<html>` (`data-compact`, `data-band`), keine Media Queries; nur
  Farbvariablen aus `app.css`; Hilfsklassen `js-only`/`nojs-only`.

Tests: Unit-Tests in `src/band.rs`; `tests/round33_menueband.rs` (Band je Seitenart, `?tab=`, `?band=` mit Cookie,
Maskierung, Zuordnung aller alten Einträge, Zeichen, Fragment des Editors, Verhalten der Widgets im Node-Mock, Szene "band"
in `tests/round23_web/widgets.mjs`).

### Laptop-Modus (Runde 33) {#laptop-mode-round-33}

Eine Quelle der Wahrheit: **Attribute an `<html>`**. Der Server schreibt sie bei der Auslieferung, das Widget hält sie im
Browser aktuell, CSS und andere Widgets lesen sie nur. Es gibt **keine Media Query auf die Fensterbreite** für die Modi –
sonst gäbe es zwei Wahrheiten, sobald der Mensch „An“ oder „Aus“ wählt.

| Attribut | Bedeutung | gesetzt, wenn |
|---|---|---|
| `data-laptop="auto\|on\|off"` | Wahl des Menschen | immer |
| `data-compact` | Band kompakt | `on`, oder `auto` und Breite `m`/`s` |
| `data-narrow` | schmal: beide Seitenspalten sind Leisten | `on`, oder `auto` und Breite `s` |
| `data-rail-left` | Gliederung eingeklappt | schmal, oder von Hand (`lrail`) |
| `data-rail-right` | rechte Spalte eingeklappt | schmal, oder von Hand (`rrail`) |
| `data-drawer="left\|right"` | Schublade offen (nur laptop.ts, nie vom Server) | solange die Schublade offen ist |

#### Server: `src/laptop.rs` {#server-srclaptoprs}

- `LaptopState::from_request(cookie_header, query)` liest den Zustand: Schlüssel der Adresse `laptop` (`auto|on|off`),
  `lrail`, `rrail` (`0|1`) gelten **vor** den Cookies `baruch_laptop` (`on|off`, fehlt = automatisch), `baruch_lrail`,
  `baruch_rrail` (`1`, fehlt = sichtbar). Die Breitenklasse kommt nur aus dem Cookie `baruch_vw` (`w|m|s`, fehlt = breit).
  Ungültige Werte werden übersprungen; kommt ein Schlüssel mehrfach in der Adresse vor, zählt der letzte gültige (das Band
  hängt seine Schlüssel hinten an).
- `compact()`, `narrow()`, `rail_left()`, `rail_right()` sind die Formel aus der Tabelle; `html_attrs()` schreibt sie in
  fester Reihenfolge und **nur aus festen Zeichenketten** – nichts aus Adresse oder Cookie gelangt in die Seite.
- `set_cookies(query)` liefert fertige `Set-Cookie`-Werte für gültige Schlüssel der Adresse (`…; Path=/;
  Max-Age=31536000; SameSite=Lax`); die Voreinstellungen (`laptop=auto`, `lrail=0`, `rrail=0`) löschen das Cookie
  (`Max-Age=0`).
- Schwellen: `COMPACT_BELOW_PX = 1600`, `NARROW_BELOW_PX = 1280`; `Width::from_px` ordnet zu.
- Verdrahtung: `consent::inject` (Schicht über allen HTML-Antworten) baut den Zustand vor `next.run`, `band::apply` setzt
  `html_attrs()` an der Marke ` data-band-fill` ein, die Cookies werden der Antwort **angehängt** (`/` setzt schon
  `baruch_work`). Keine neue Route, keine RPC-Methode, keine Migration.

#### Browser: `web/src/laptop.ts` {#browser-websrclaptopts}

- Hängt sich beim Laden an das `document` (Seiten ohne Arbeitsfläche haben kein Element dafür); das Widget `laptop` bleibt
  registriert und löst dasselbe aus.
- Misst `window.innerWidth` beim Laden und bei `resize` (gebündelt über `requestAnimationFrame`), schreibt die Klasse in das
  Cookie `baruch_vw` und setzt die Attribute nach derselben Formel (`effective`, `htmlAttrs`). Die Schwellen stehen auch im
  Skript als `COMPACT_BELOW_PX`/`NARROW_BELOW_PX`; `tests/round33_laptop.rs` vergleicht sie und alle 36 Kombinationen der
  Formel mit der Rust-Seite.
- **Ereignis `baruch:laptop`** am `document`, `detail: {compact, narrow}`: einmal beim Laden, dann bei jeder Änderung von
  `compact` oder `narrow`. band.ts klappt daraufhin seine Gruppen. Das Ereignis beim Laden geht hinaus, bevor Widgets
  eingehängt sind – wer es zusätzlich braucht, liest beim Einhängen die Attribute an `<html>`. laptop.ts fasst die Gruppen
  des Bands nie an; band.ts misst nie Breiten.
- Klicks (ein Listener am `document`): `[data-laptop-set]` (Modus, Cookie `baruch_laptop`), `[data-rail-toggle]` (Schalter im
  Band), `[data-rail-open]` (Leiste), `[data-rail-collapse]` („Einklappen“ in der Spalte), `[data-rail-close]`
  („Schließen“, Fläche neben der Schublade). Das Widget pflegt `aria-pressed` der Schalter (Leiste sichtbar = gedrückt) und die
  Adresse des Rückfalllinks (`lrail=0|1`). Gibt es die Spalte auf der Seite nicht, bleibt der Link ein gewöhnlicher Link.
- Schublade: Im schmalen Zustand `data-drawer` an `<html>`; Fokus in die Spalte, Esc und „Schließen“ geben ihn an den Auslöser
  zurück, ein Klick daneben schließt ohne Fokuswechsel, die Wahl eines Abschnitts (`a.tree-entry`) schließt die Gliederung.
  Im nicht schmalen Zustand klappen Leiste und Schalter die Spalte dauerhaft ein (Cookies `baruch_lrail`, `baruch_rrail`).
- `web/src/zoom.ts`: `[data-zoom-reset]` setzt alle Zonen zurück und löscht `baruch_zoom`.

#### Auszeichnung und Stil {#markup-and-style}

- `templates/workspace.html`: zwischen den Spalten je eine Leiste `a.rail[data-rail-open]` (`href="#pane-tree"` oder
  `#pane-assistants`) und die Fläche `a.rail-scrim` (`href="#pane-editor"`). Die Spalten bleiben erstes und letztes Kind des
  Rasters, weil das Widget `splitter` sie so misst. `id="pane-tree"`, `id="pane-editor"`, `id="pane-assistants"`, die Teiler
  und alle `data-zoom-zone` sind unverändert.
- `templates/fragments/tree.html`, `assistants.html`: oben die Zeile `div.rail-bar` mit „Einklappen“ (`?lrail=1` oder
  `?rrail=1`) und „Schließen“ (`#pane-editor`). Sie steht in den Fragmenten, weil `ui.render` den Inhalt der Spalten ersetzt.
- `static/app.css`: Grundrahmen (Raster, Leisten im Ruhezustand verborgen). Die Regeln des alten Kopfs (`.topbar`) und die
  beiden Breiten-Media-Queries (Kopf unter 1000 px, Spalten gestapelt unter 900 px) sind weg.
- `static/laptop.css` (zuletzt in `STYLE`): alles auf `data-compact`, `data-narrow`, `data-rail-*` außerhalb des Bands. Ohne
  JavaScript öffnet `:target` die Schublade (`html:not(.js)…:target`); mit JavaScript gelten diese Regeln nicht, damit eine
  Adresse mit `#pane-tree` die Schublade nicht offen hält. Die Fläche daneben braucht ohne JavaScript `:has()`; fehlt es dem
  Browser, bleibt „Schließen“.

#### Grenzen {#limits}

- Ohne JavaScript fehlt `baruch_vw`: „Automatisch“ zeichnet breit, der Mensch stellt „An“.
- Beim allerersten Aufruf mit JavaScript kennt der Server die Breite ebenfalls noch nicht; die Seite kommt breit und wird vom
  Widget sofort umgeschaltet (ein kurzer Sprung, danach stimmt das Cookie).
- Das Cookie `baruch_vw` gilt je Browser, nicht je Fenster: Zwei Fenster unterschiedlicher Breite überschreiben es
  abwechselnd; das Widget setzt es in jedem Fenster gleich nach dem Laden richtig.
