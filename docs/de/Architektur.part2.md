<!-- translation-of: docs/Architektur.md source-commit: 4d3148c part: 2 -->

## Personas für echt (Runde 10) {#personas-for-real-round-10}

### Port und Modell-Backend {#port-and-model-backend}

- **Port** `assist/port.rs`: `ModelPort { chat(state, ModelCall, deltas: UnboundedSender<String>), models(state),
  alias(name), context(state, persona) → ContextBudget {tokens, reported, warning} }` (seit Runde 15 mit einer
  Standardimplementierung = Einstellung der Persona) mit `ModelCall {model, messages, max_tokens, temperature, label,
  persona}` (Handle der Persona, leer = keine; wählt Verbindung und Freigabe), `ModelAnswer {text, model,
  finish_reason, prompt_tokens, completion_tokens}`, `ModelFailure` (`NotConfigured`, `Unreachable`, `Denied`,
  `Timeout`, `UnknownModel`, `Busy`, `Cancelled`, `Failed`; `Display` auf Deutsch, nie ein Geheimnis), `ModelList`.
  `NoModel` meldet ehrlich „Kein Modell-Backend angebunden …“. **Abbruch** = das Future fallen lassen (Client
  trennt → Strom fällt → Task fällt → HTTP-Anfrage an Eliezer bricht ab, Platz in der Warteschlange frei).
- **Adapter** `assist/eliezer.rs` (`EliezerPort`): bildet `ModelCall`/`ModelAnswer`/`ModelFailure` 1:1 auf
  `baruch_llm::ChatRequest`/`ChatReply`/`LlmError` ab und ruft `crate::models`.
- **`models.rs`**: `chat(state, ChatRequest, label, deltas)` – Adresse aus der Verbindung `eliezer`, Schlüssel aus
  dem Schlüsselbund `baruch/eliezer` nur mit `connections::secret_bound` (sonst geht gar nichts hinaus), Alias vor
  dem Senden, prozessweite `Queue` (`static`, eine Anfrage zugleich, höchstens 8 wartend); `list(state)` →
  `Listing {models, live, fetched}` (60 s je Adresse zwischengespeichert, nicht über die Warteschlange);
  `alias`, `status`, `probe` (kurzer Test), Obergrenze `BARUCH_MAX_TOKENS` (Umgebung, optional; begrenzt
  `max_tokens` jedes Aufrufs, etwa für manuelle Prüfungen mit kurzen Antworten), `persona_states`/`persona_summary` (Abgleich der Personamodelle),
  `remember` (Cache aus „Verbindung prüfen“); RPC `model.*` (`handles`/`call`).
- **`baruch-llm`**: Grenzen (`Limits`): Verbindung 10 s, erstes Stück 180 s (Modell laden), Pause 60 s,
  gesamt 600 s, Modellliste 10 s, Antwort höchstens 1 MiB, `max_tokens` auf 1–4096 begrenzt, keine
  Weiterleitungen. Nur `choices[0].delta.content` zählt (`reasoning_content` übersprungen); `[DONE]` beendet das
  Lesen sofort. Fehler: Verbindung → `Unreachable`; 401/403 → `Denied` (Hinweis `baruch/eliezer`); 404 oder
  „model not found“ → `UnknownModel`; 409/429/503 → `Busy`; sonst `Failed` mit höchstens 300 Zeichen der
  Gegenseite, Schlüssel daraus entfernt (`scrub`); Zeitgrenzen → `Timeout`. Baruch fragt **nie** eine
  Kontextlänge an und ruft nie `/api/v1/models/load|unload|estimate` auf.
- `connections::run_check`: ein `tokio::time::timeout` (10 s) um die ganze Prüfung; bei Eliezer danach der Abgleich
  der Personas (`details.personas`) und `models::remember`. Der Nutzungshinweis der Verbindung Eliezer ist kein
  STUB mehr (`connections::usage_is_stub`).

### Ablauf eines Auftrags (`assist/mod.rs`, `assist::run`) {#course-of-a-task-assistmodrs-assistrun}

Persona prüfen (aktiv, sonst -32602) → Kontext **ohne** Schreibsperre lesen (`assist::context::prepare`) →
Modell **ohne** die Sperre aufrufen (mit Gemma ~25 s) → Nachbearbeitung (`assist::prompts`, Haken
`hooks::mark_answer`) → erst dann `write_lock`, `key` neu auflösen, schreiben: Beiträge über `Store::add_post`
(Autor = Persona-Handle), Vorschläge über `Store::add_proposal` (immer `proposed`, `model` = tatsächliches
Modell, Notiz „Vorschlag von @x“, Überschriftzeile der gezeigten Revision erzwungen, `assist::force_heading`).
Fehler oder Abbruch speichern nichts. Chat (`assist::chat`): Beitrag des Absenders sofort, dann je erwähnter
Persona `run` nacheinander (höchstens drei, kein Relay). `assist/rpc.rs` (RPC `persona.ask/review/propose/chat`),
`assist/routes.rs` (`POST /assistenten/act`, `POST /assistenten/stream` mit einem eigenen `Stream`, der den
Auftrag antreibt; neue Abhängigkeit `tokio-stream`).

- **Kontext** (`assist/context.rs`): System = Rollenprompt (+ Haken `scripture_facts`); Nutzer = Werk (Titel,
  Art, Zielgruppe), Kapitelumgebung (Kapitel, Nachbarabschnitte mit je ≤ 300 Zeichen Anfang), beim Lektor das
  Gedächtnis (Einfälle mit `idea.node_id = key`, nicht gelöscht, dazu ≤ 10 offene ohne Bezug), im Chat die
  letzten 8 Beiträge, dann der Abschnitt der gezeigten Revision (`rev`, sonst die aktuelle) zwischen `<<<`/`>>>`,
  zuletzt die Aufgabe. Tokens ≈ ⌈Zeichen/3⌉, Budget = Kontextlänge − `max_tokens` − 256; zuerst gekürzt wird
  die Kapitelumgebung, dann Gedächtnis oder Chatverlauf, zuletzt der Abschnitt vom Ende her mit
  „[… gekürzt: n Zeichen]“ → `truncated: true`. `max_tokens`: ask 700, review 900, chat 500, propose
  min(4096, Abschnittstokens·1,5 + 200); `temperature` 0,4; `label` „@x <art> key K“ (ohne Text).
- **Rollenprompts** (`assist/prompts.rs`): Vorgaben für `@lektor`, `@exeget`, `@schrift`, `@belege`, `@nathan`,
  `@hoerer`, sonst allgemein aus der Rolle; ein gespeicherter Prompt (`persona.prompt`, Migration 0015) ersetzt
  die Vorgabe vollständig. Harte Regeln mit Gegenbeispielen („Falsch: … / Richtig: …“) – siehe `docs/Bedienung.md`
  „Assistenten fragen“.
- **Nachbearbeitung**: beginnt die Antwort (nach Leerraum, Anführungs- und Markdown-Zeichen, ohne Rücksicht auf
  Groß-/Kleinschreibung, nur ganze Wörter) mit „Als Assistent“, „Als KI“, „Als Sprachmodell“, „Gerne“ oder
  „Natürlich“, fragt Baruch **einmal** neu an (Zusatzanweisung `prompts::RETRY_NOTE` am Ende des
  Systemprompts); beginnt auch die zweite so, wird ihr erster Satz verworfen (bis `.`/`!`/`?`/`:` mit Leerraum
  dahinter oder bis zum Zeilenende). Beim Streamen hält Baruch die ersten 24 Zeichen zurück, bis klar ist, ob die
  Antwort so beginnt. Vorschläge: Codeblock ``` entfernt.
- **Haken** (`assist/hooks.rs`): `scripture_facts` → `crate::scripture::facts_for`, `mark_answer` →
  `crate::scripture::mark_answer`.
- **Lektor**: `baruch_core::LectorChoice`, gespeichert in `store_meta` Schlüssel `lector` (`auto` oder ein
  Handle; keine Zeile = fest `@lektor`, Versuch seit 2026-11-06); Store `lector_choice`/`set_lector_choice`;
  `personas::lector_of(rows, &choice)`. Ist die feste Persona inaktiv, gilt „automatisch“.
- **Migration 0015** `persona.prompt TEXT CHECK (NULL oder 1–8000 Zeichen)`; Store `persona_prompt`,
  `set_persona_prompt`, `PersonaRow.prompt`, `MAX_PROMPT_CHARS`. Keine Datenänderung.
- **Benutzeroberfläche**: `fragments/assistants.html` (Formular `assist-form` mit Prüfen/Vorschlag, Chat-Eingabe an
  `persona.chat`), Widget `web/src/assist.ts` (`fetch` + `AbortController`, SSE-Parser `parseEvents`,
  vorläufiger Beitrag in der Personafarbe, danach `baruch:navigate {url}`), Stil `static/assist.css`; Einstellungen
  `settings/assistants.rs` + `settings/assistenten.html` (Lektorformular, `<datalist id="baruch-models">` aus
  `state.model.models`, Warnungen über `state.model.alias`, Rollenprompt).
- **Testmodell** `assist::mock::ScriptedModel` (öffentlich, nur für Tests): Skript je Aufruf (`Step::words`,
  `Fail`, `Hang`, `Wait` + `release()`), `calls()`, `cancelled()`.

### Lektor mit dem ganzen Kapitel (Runde 15, „lektor“) {#lector-with-the-whole-chapter-round-15-lektor}

Das löst die Punkte „Kontext“ (Kapitelumgebung mit 300 Zeichen je Nachbar, Kürzen des Abschnitts) und
„Nachbearbeitung“ (verbotene Einleitung auch beim Vorschlag) oben ab.

- **Schnittstelle S2:** Chat in `assist/chat.rs` (`mentions`, `chat`, `ChatOrder`, `run_chat`, `chat_json`);
  `Progress` erweitert um `Posted(SavedPost)` (Beitrag des Absenders gespeichert), `Retry {persona, reason}` (Neuanfrage:
  „verbotene Einleitung“ oder „fehlendes „Kritisch:““) und `Compressed(Compression)`; `Compression {message,
  summarized, chunks}`, `Scope {Section (Vorgabe), Chapter}`, `Order.scope`, `Outcome.compressed`,
  `Outcome.proposals` (Vorschlag für das Kapitel). SSE-Weiterleitung in `assist/routes.rs`.
- **Kontext** (`assist/context.rs`, `prepare_inputs` mit `RagInput`/`BibleInput`): `chapter_units` liefert die
  Teile des Kapitels (nächstes `NodeKind::Chapter` über dem Abschnitt oder er selbst, mit allen Knoten darunter;
  bei `WorkKind::Sermon` das ganze Werk; ohne Kapitel der Knoten mit seinen Unterpunkten), je Teil die aktuelle Revision,
  beim eigenen die gezeigte. Die Nutzernachricht baut `Frame::call`: Vorspann (Werk, Befunde der anderen
  Personas – Beiträge, deren Autor das Handle einer anderen Persona ist, je Teil die letzten drei, im Chat ohne
  den eigenen Abschnitt –, Gedächtnis, Chatverlauf, Faktenblöcke), dann "Chapter surroundings:" mit jedem Teil als
  `--- Titel ---` und `<<< … >>>` (Wortlaut, "summary only" oder "not included"; der Prüfgegenstand
  als Verweis), dann der Prüfgegenstand mit einem Kopf, zuletzt die Aufgabe. `Scope::Chapter`: alle Teile sind
  Prüfgegenstand (`max_tokens` ask 900, review 1500).
- **Budget:** `state.model.context(state, &persona).await.tokens` (Schnittstelle S1, `ModelPort::context`) −
  `max_tokens` − 256, in Zeichen ·3. Zugaben (`Extras`: Web, SWORD, RAG, Befunde, Gedächtnis von hinten, Chat von
  vorn) werden gekürzt, solange etwas zu viel ist und sie mehr als ein Drittel des Budgets belegen; reicht der Platz
  danach nicht für den größten Teil des Prüfgegenstands plus 600 Zeichen Umgebung, auch darunter (`truncated`).
  Kapitel und Prüfgegenstand werden nie gekürzt; `Prepared::fits` sagt, ob sie in ihrem Wortlaut passen. Passt nicht
  einmal das Minimum (weniger als 600 Zeichen Platz), lehnt `prepare` ehrlich ab (-32000).
- **Zerlegen** (`assist/chunk.rs`, rein; Ablauf in `assist/mod.rs`, `compressed_run`): Prüfgegenstand in
  Stücken von höchstens ⅔ des Platzes (passt damit nicht einmal die untere Grenze unten, der Reihe nach ½, ⅓, ¼ –
  `PIECE_SHARES`: ein kleinerer Kern lässt der Umgebung mehr Platz) (`chunk::split`, Schnitt an einem Absatz, sonst Satzende, sonst Leerraum,
  Überlappung ⅛, höchstens 600 Zeichen; ein Abschnitt bis ¾ des Platzes bleibt ganz; Kapitel: `chunk::pack`). Je
  Stück plant `chunk::plan_piece` die Umgebung: alles im Wortlaut; solange es nicht passt, der vom Kern am weitesten entfernte Teil
  als Zusammenfassung, danach – nur wenn es Platz spart – als bloßer Titel. Die nötigen Zusammenfassungen holt
  `plan_and_summarize` **vor** dem ersten Prüfaufruf und **nacheinander** (eigener Aufruf, `SUMMARY_SYSTEM`,
  `max_tokens` 80–300, Label „… Zusammenfassung“, `persona` gesetzt – die Freigabe „extern“ gilt auch hier);
  danach plant es mit den echten Längen neu (höchstens drei Runden). Der Platz je Stück ist `frame.room` abzüglich
  `piece_extra` (je Kernteil die Umgebungszeile "under review, given verbatim below", dazu was der Kopf "piece k of n (…)" und
  der Zusatz in der Aufgabe, oder die eigene Aufgabe beim Vorschlag, länger sind als in `prepare` gerechnet). Vor dem ersten
  Modellaufruf prüft `plan_and_summarize` eine Untergrenze je Stück (Kern im Wortlaut, jeder andere Teil so kurz
  wie möglich); reicht nicht einmal die, eine ehrliche Ablehnung (`too_many_parts`, -32000, nichts gespeichert). Danach
  werden alle Aufrufe gebaut und **gemessen** (`check_calls`: Zeichen aller Nachrichten ≤ (Kontextlänge −
  `max_tokens` − 256) · 3), bevor der erste hinausgeht; auch der ungeteilte Aufruf wird gemessen und zerlegt, wenn
  er überschreitet. Dann `Progress::Compressed`, dann je Stück
  ein Aufruf (Strom mit einer Leerzeile zwischen den Stücken), Befunde mit `chunk::merge` ohne Doppelte (gleich oder einer
  ganz im anderen, ab 24 Zeichen; der ausführlichere bleibt). Vermerk „Komprimiert: …“ (`compression`) als letzter
  Absatz des Beitrags oder in der Notiz des Vorschlags. Ein Vorschlag für den Abschnitt wird nie zerlegt (der
  Abschnitt muss ganz passen, sonst `too_long_for_proposal`).
- **Vorschlag für das Kapitel** (`propose_chapter`): je Teil mit Text ein Aufruf nacheinander (Teil im Wortlaut,
  Umgebung wie oben geplant, Aufgabe `propose_part_task`), danach `proposal_text` (Codeblock, „Kritisch:“-Zeilen entfernt,
  Überschrift erzwungen); unveränderte und abgeschnittene Teile entfallen; gespeichert wird erst am Ende unter
  **einer** Schreibsperre, genau ein `proposed` je Teil.
- **„Kritisch:“** (`prompts::needs_critical`, `has_critical`): `answer_call` fragt bei Lektor, Exeget und Nathan
  höchstens einmal je Auftrag neu an (`CRITICAL_RETRY_NOTE`, `Progress::Retry`); scheitert die Neuanfrage, bleibt die erste
  Antwort. Fehlt es im gespeicherten Text, hängt `post_body` `CRITICAL_MISSING_NOTE` an. Beim Vorschlag
  prüft `call_model` keine Einleitung (`check_opening = false`), `strip_critical_lines` entfernt Befundzeilen.
- **Faktenblock aus SWORD** (`assist/bible_facts.rs`, Haken `hooks::bible_research` → `crate::bible_research::research`,
  Daten und Schalter mit dem Zweig „anbieter“): Block "Evidence from SWORD" mit `[Sn] <Art> <Modul> · <Stelle> · Lizenz:
  …` und Text zwischen `<<<beleg`/`>>>`, entschärft mit `rag_facts::defuse`; Hinweise der Recherche darunter;
  Bibelwortlaut, der schon im Faktenblock der Stellenprüfung steht, entfällt; Regel `SYSTEM_RULE`; Budget geteilt
  mit RAG/Web (`BIBLE_MAX_CHARS` = min(4000, Budget/5) an `research`); `[Sn]` ohne Eintrag → „(unbekannte Quelle)“
  (`rag_facts::mark_unknown`). Bis zur Integration ist `research()` ein Stub-Rumpf (`None`) – Tests über
  `BibleInput::Given`.
- **Benutzeroberfläche:** `templates/fragments/toast.html` (Hinweisbox `div.toast.toast-<level>[data-widget=toast]`,
  ohne JavaScript sichtbar), Widget `web/src/toast.ts` (verschiebt Boxen in `.toast-stack`, „Schließen“, hört auf
  `baruch:toast {message, level, summarized?}`; „warn“ ohne Ablauf, „info“ 8 s), Stil `static/toast.css`;
  `templates/fragments/assist_scope.html` (Radioknöpfe `scope`), vom Zweig „auftraege“ eingebunden.

### Bibelstellen (`baruch-scripture`, `scripture.rs`) {#bible-references-baruch-scripture-scripturers}

- `data/versification.tsv`: je Zeile `OSIS<TAB>Kapitel<TAB>Verse`, 66 Bücher, 1189 Kapitel, 31172 Verse; nur
  Zahlen, **deutsche Verszählung**. Herkunft: Verszählung „German“ des SWORD-Projekts (CrossWire Bible Society,
  `include/canon_german.h`, SHA-256 im Kopf der Tabelle, abgerufen 2026-11-06); das SWORD-Modul der Schlachter 1951
  (`GerSch`) ist in dieser Verszählung angelegt (`Versification=German`). AT: Bücher, Kapitel, Verse aus
  `otbooks_german`/`vm_german`; NT: Kapitel wie in der SWORD-Standardliste, Verse aus `vm_german`. Nur
  Zahlen werden übernommen. Nicht mehr verwendet: eBible.org `deu1951` (VPL und USFM) ist in die englische Verszählung (KJV, 31102 Verse) umgesetzt
  – sie stand hier bis 2026-11-06 und meldete zum Beispiel „Ps 51,21“ als ungültig. Der Test
  `versification.rs` prüft die Summe 31172 und Joel 4/Maleachi 3, `references.rs` Stellen, die nur in der deutschen
  Verszählung gültig sind. Nie von Hand ändern. Lizenz: `canon_german.h` steht unter GPLv2; nur die Zahlen werden als
  Fakten übernommen, kein Code (Entscheidung des Nutzers 2026-11-06).
- Parser: ein Komma mit Leerraum davor vor einem nummerierten Buch („Joh 3, 1. Joh 4“) trennt Stellen und leitet keinen
  Vers ein (`verse_mark`). Seit Runde 17b: Verslisten mit Kommas (`comma_verse`), Zahlwörter (`COUNT_WORDS`,
  `count_word_follows`), Uhrzeiten nach einem Wochentag (`weekday_clock`, nur `scan`), „… Vers 23“ im selben Satz
  (`verse_later`) – siehe „Runde 17b – stellen“.
- `src/books.rs`: 66 Bücher (OSIS, Name, Abkürzung, weitere Schreibweisen); Vergleich ohne Punkte, Leerraum,
  Groß-/Kleinschreibung; jede Schreibweise gehört zu genau einem Buch. Parser: rekursiver Abstieg über Byte-Positionen, nie
  eine Panik.
- Server `src/scripture.rs`: RPC `scripture.check` (`check(text)`, gemeinsam mit MCP `scripture_check` in
  `src/mcp/scripture.rs`), `facts_for(handle, section)` (Faktenblock nur für `@schrift`/`@belege`),
  `mark_answer(handle, section, body)` (Zeile „Wortlaut: ungeprüft …“ oder ein Hinweis an @schrift). Nur lesend.
  Seit 2026-11-06 mit einer Bibel: `facts_for_work`/`mark_answer_for_work` (Haken in `assist/hooks.rs`, async mit
  `state` und Werk), `check_with(choice, text)`; `mark_with` fügt auseinandergerissene Ziffern einer Stelle des
  Abschnitts in der Antwort wieder zusammen (`wording::rejoin_split_refs`, Befund „Mk 1 0,42–45“: Parser und
  Anzeige schreiben die Zahl richtig, die Lücke stand im Text des Modells).

### SWORD-Bibeln (2026-11-06, `baruch-sword`, `bibles.rs`, Migration 0030) {#sword-bibles-2026-11-06-baruch-sword-biblesrs-migration-0030}

- **Entscheidung zum Leser (Format):** die Crate **`sword-modules` 0.1.0** (Logoxide, **CC0-1.0**, reines Rust, kein
  FFI) liest das Dateiformat; im Quelltext geprüft (`~/.cargo/registry/src/…/sword-modules-0.1.0`): Treiber zText,
  zText4, RawText, RawText4 (dazu Kommentare/Lexika, die Baruch nicht nutzt), conf nach `docs/spec/conf.md`
  (Fortsetzungszeilen, RTF `About`), `DataPath` relativ zum Modulordner (führendes `./` oder `/` entfernt), alle 18
  SWORD-Verszählungen als Fakten aus der Laufzeit-API, Index je Testament (Position 0 Modulkopf, 1 Testamentskopf,
  je Buch Einleitung, je Kapitel Einleitung, dann Verse), Blockcache, Normalisierung der Einträge. Sie
  dokumentiert einen Clean-Room-Ersatz für Teile, die früher aus libsword portiert waren (`CLEANROOM.md`); Abhängigkeit
  `topos-bible` 0.6 (CC0-1.0, Stellenangaben, von Baruch nicht genutzt) und dessen regex, smallvec, itertools und andere
  (MIT/Apache). Eingebunden mit `default-features = false, features = ["zip"]`: kein Installer, kein Netz (ureq,
  zip, tar), kein bzip2/xz (seit Runde 17c `features = ["zip", "bzip2", "xz"]`, weiterhin ohne `install`/`https`). **Nicht genutzt:** libsword (GPL-2.0) und `rsword_chirho` (GPL-2.0-or-later), kein
  Code daraus. Baruchs eigener Code: Modulordner und Auswahl, Prüfung der Unterstützung, `DataPath`-Wächter
  (`..` → abgelehnt), OSIS/ThML/GBF → Klartext, Abgleich der Verszählung, Wortlautvergleich, Suche, Testmodule.
- **Crate `baruch-sword`** (rein bis auf Dateizugriff): `scan(dirs)` → `BibleInfo` (Name, Beschreibung, About als
  Text, Versification, Lang, Version, ModDrv, CompressType, BlockType, SourceType, Encoding, DistributionLicense,
  Copyright, `notice` = Lizenzvermerk, `supported`/`problem`), das erste Vorkommen eines Namens gewinnt; nur
  Bibelmodule. `open(info)` → `Bible` (`verse`, `intro`, `slot_text`, `passage` höchstens 400 Verse, `search` linear
  mit Treffer- und Zeitgrenze). Unterstützt: zText/zText4 mit ZIP oder LZSS, RawText/RawText4; Verszählung nur
  „German“ und „KJV“ (`baruch_scripture::versification`), andere → „nicht unterstützt“ (STUB in
  `versification.rs`); verschlüsselte Module → STUB. (Stand vor Runde 17; seit R17 jede Verszählung von
  `baruch_scripture::versification()`, BZIP2/XZ und `CipherKey`, siehe „Runde 17c – pruefanzeige“.) `text::to_plain`: `note`, `rdg`, `title` (außer
  `canonical="true"`), `catchWord`, `figure` (ThML auch `scripRef`, `h1`–`h6`; GBF `<RF>…<Rf>`, `<TS>…<Ts>`)
  entfallen; `divineName`, `w`, `q`, `transChange` bleiben als Text; Entitäten, Leerraum zusammengezogen.
  `fixture`: schreibt Testmodule (zText BOOK/CHAPTER/VERSE mit zlib, RawText) mit **eigener** Positionsarithmetik aus
  den Zahlen von `baruch-scripture`; Tests `tests/modules.rs` lesen sie über `sword-modules` zurück (jede Art von Position,
  beide Verszählungen, alle Blockarten) und vergleichen die Tabellen von `sword-modules` Vers für Vers mit unseren.
  Beleg am echten Modul GerSch: `ot.bzv` 241850 Bytes = 24185 Positionen, `nt.bzv` 8246 Positionen – genau die
  Zahl der Positionen der Verszählung German.
- **Verszählung KJV:** `baruch-scripture/data/versification-kjv.tsv` (31102 Verse; die frühere eBible-Tabelle aus
  der Historie vor `6e1e268`), gleich der KJV-Tabelle von `sword-modules`; `versification::{german, versification}`.
- **Wortlaut** (`baruch_scripture::wording`, rein): `inline_quotes` („…“, “…”, »…«, «…», "…", `#quote[…]`; eine
  leere Zeile beendet die Suche), Blockzitate über `baruch_wysiwyg::parse` (`BlockKind::Quote`, Bereich des Inhalts
  durch Klammerzählung, `bibles::block_quotes`), `quote_for` (Zitat direkt vor oder nach der Stelle, dazwischen nur
  Leerraum/Satzzeichen/andere Stellen derselben Liste, höchstens 24 Bytes; Blockzitat mit der Stelle in der Quellenangabe
  oder am Ende), `words` (Typst-Markup entfernt, Bindestriche/Apostrophe/weiches Trennzeichen im Wort entfernt,
  Silbentrennung „Hei- land“ zusammengefügt außer vor „und/oder“, Kleinbuchstaben), `compare` (lückenlose Wortfolge je Teil zwischen
  Auslassungen → Treffer, sonst ein Wortdiff über die längste gemeinsame Teilfolge, auf den zitierten
  Bereich begrenzt). Andere Verszählung im Modul: nur geprüft, wenn jedes betroffene Kapitel im Deutschen und im Modul gleich viele Verse hat,
  sonst `versification_mismatch` (keine Umrechnung zwischen Verszählungen).
- **Server `src/bibles.rs`:** RPC `bible.*`, `Choice` (keine, fehlt, unlesbar, bereit), Cache geöffneter
  Module je conf-Datei und Änderungszeit (höchstens 8), Lesen in `spawn_blocking` für Scan, Öffnen und
  Suche; `check_wording` je Stelle. Seite Einstellungen → Quellen (`connections/pages.rs`, `settings/quellen.html`,
  Block `#bibeln`). MCP `src/mcp/bibles.rs`. Nur der laufende Dienst liest die echten Modulordner; Tests setzen immer
  eigene Ordner.
- **Store (Migration 0030):** `bible_dir (position, path UNIQUE)` und `store_meta.bible_dirs` (gesetzt = eigene
  Liste, auch leer; fehlend = Vorgaben), `work_bible (work_id → work ON DELETE CASCADE, module)`; `bible_dirs`,
  `set_bible_dirs`, `work_bible`, `set_work_bible` (`src/bibles.rs`), kein Commit im Spiegel (kein Buchtext). Test
  `tests/migration_0030.rs` (verlustfrei, ein zweites Öffnen gleich, das Löschen des Werks nimmt die Auswahl mit).

### Verbindung je Persona (Runde 15b, `models.rs`, `baruch-llm/src/client.rs`, Migration 0036) {#connection-per-persona-round-15b-modelsrs-baruch-llmsrcclientrs-migration-0036}

- **`baruch_llm::Client`** (Vertrag §3): `BackendKind {Eliezer, Openai, Ollama}`, `ModelInfo {id, context_length}`,
  `Client::new(kind, Endpoint, Limits)` mit **einem** `reqwest::Client` je Endpunkt (Klone teilen ihn; Verbindungen
  bleiben im Pool von reqwest), `models()` (`GET /v1/models`, bei Eliezer zusätzlich die Kontextlängen aus der
  Verwaltungsliste), `context_length(model)`, `chat_stream`. Die freien Funktionen `models`, `models_with`,
  `chat_stream`, `chat_stream_with` bleiben als Hüllen (eigener HTTP-Client je Aufruf, wie bisher). Der Test
  `tests/backends.rs` zählt die TCP-Verbindungen am Nachbau (Keep-Alive): vier Anfragen über einen Client = eine
  Verbindung, zwei freie Anfragen = zwei.
- **Gemeldete Kontextlänge – Annahmen:**
  - *Eliezer:* `GET /api/v1/models` (Verwaltungsliste in der Form von LM Studio 0.4) mit `key`,
    `max_context_length` und `loaded_instances[].config.context_length`. Quelle: `docs/wiki/API.md` und
    `crates/eliezer-api/src/lib.rs` im Projekt Eliezer (gelesen, nicht gegen den laufenden Dienst geprüft).
    Gemeldet wird die Länge der geladenen Instanz, sonst `max_context_length`. Nur lesend; nie
    `/api/v1/models/load|unload|estimate`. Ob Eliezer ein noch nicht geladenes Modell mit genau dieser Länge lädt,
    sieht Baruch weiterhin nicht.
  - *Ollama:* gemeldet wird die **wirksame** Länge, nie das Maximum: der Chat läuft über
    `/v1/chat/completions` ohne `num_ctx` (die OpenAI-Schnittstelle von Ollama nimmt keines), Ollama nimmt dann
    `num_ctx` aus dem Modelfile, sonst seine Vorgabe, und schneidet längere Eingaben vorn still ab. Darum
    `ollama_context`: `num_ctx` aus `parameters` von `POST /api/show {"model"}`; sonst `context_length` des
    geladenen Modells aus `GET /api/ps` (`ollama_loaded_context`, Annahme: neuere Versionen melden es; fehlt
    `/api/ps`, gilt das nicht als Fehler); sonst `OLLAMA_DEFAULT_CONTEXT` = 4096 (Annahme nach den Ollama-Dokumenten;
    eine größere Vorgabe über `OLLAMA_CONTEXT_LENGTH` sieht Baruch nur bei einem geladenen Modell) – jeweils höchstens
    `<arch>.context_length` aus `model_info`. Alles gegen ein echtes Ollama ungeprüft, nur gegen den Nachbau
    (`MockEliezer` mit `Flavor::Ollama`: `/api/show`, `/api/ps` ohne geladenes Modell). Modellliste über
    `/v1/models`.
  - *OpenAI-kompatibel:* keine Meldung; die Einstellung der Persona gilt.
- **Server `models.rs`:** Verbindungen aus dem Store (`model_connection`), `resolve` (Adresse, Schlüssel aus dem
  Konto der Verbindung, nur wenn an das aktuelle Ziel gebunden), `client_for` (Pool je Art, Adresse und
  Fingerabdruck des Schlüssels), Warteschlange **je Endpunkt** (`status()` fasst zusammen), `route(state, persona)`
  mit der Freigabe „extern“ (externe Verbindung ohne `persona_backend.external_allowed` → `NotConfigured`, keine
  Anfrage), `chat_on`, `list_on` (60-s-Cache, bei Fehler die abgelaufene Liste mit Hinweis),
  `context` (min(Einstellung, Meldung), 60 s gemerkt, höchstens 10 s Wartezeit, scheitert nie),
  `context_cached` (ohne Netz, für `persona.list` und die nicht gezeigten Reiter), `probe_on` (Ergebnis in
  `model_connection.probe`), `check_connection`, RPC `model.*`. `EliezerPort::chat` ruft `route` mit
  `ModelCall::persona`, `EliezerPort::context` ruft `models::context` – so gilt die Freigabe für jeden Modellaufruf
  der Personas (auch Zusammenfassungen des Lektors), und der Lektor bekommt sein Budget über `state.model.context`.
  Einbettungen (`rag/embed.rs`) nutzen weiter die Verbindung `eliezer` und die freien Funktionen; ist die
  Modellverbindung Eliezer (ID 1) als extern markiert, lehnt `rag::embed::endpoint` jede Einbettung ab
  (`EXTERNAL_REFUSED`, gemeldet als „kein Einbettungsmodell“) – Einbetten hat keine Persona, also keine Freigabe.
- **Ort nach Adresse** (`models::looks_local`, ohne DNS): lokal sind Loopback-, private, CGNAT- und
  Link-Local-Adressen, ULA, Rechnernamen ohne Punkt und `.localhost`/`.local`/`.lan`/`.internal`/`.home.arpa`.
  `model.connection_set` ohne `external`: beim Anlegen `!looks_local(url)`, beim Ändern bleibt external, und eine neue
  nicht erkennbar lokale Adresse macht sie extern. `describe` setzt `locality_warning`, wenn eine als lokal geführte Verbindung
  auf eine nicht erkennbar lokale Adresse zeigt (Anzeige auf der Seite, keine Sperre: ein Intranet-Name
  darf bewusst lokal sein).
- **Store (Migration 0036):** `model_connection (id, name UNIQUE NOCASE, kind, url NULL nur bei id 1, external,
  account UNIQUE, secret_target, probe, checked, position, …)` ohne AUTOINCREMENT (sonst änderte die Migration
  `sqlite_sequence`, das die älteren Migrationstests vergleichen); neue IDs vergibt der Store über
  `store_meta.model_connection_last` nie doppelt (ein neues Konto `baruch/model-<id>` erbt nie den Schlüssel einer
  gelöschten Verbindung). Die Verbindung Eliezer (ID 1) stammt aus der Migration, ihre Adresse und Bindung bleiben
  in `connection` (nicht doppelt gehalten), ein Trigger verhindert ihr Löschen. `persona_backend (persona_id,
  connection_id ON DELETE CASCADE, external_allowed)`; keine Zeile = Eliezer, nicht freigegeben. `PersonaRow` bleibt
  unverändert. Test `tests/migration_0036.rs`.
- **Seiten:** Einstellungen → Verbindungen, Abschnitt `#modelle` (`connections/pages.rs`, Formular
  `POST /einstellungen/modelle/act` im Router von `connections::writes`, hinter `guard::same_origin`);
  Einstellungen → Assistenten, je Reiter `action=backend` und `action=bible` (`personas.rs`, `settings/assistants.rs`).

### Kommentare und Lexika (Runde 15, Punkte 5a–5c, Migration 0037) {#commentaries-and-lexicons-round-15-items-5a5c-migration-0037}

- **`baruch-sword/src/reference.rs`:** `Commentary` (RawCom, RawCom4, zCom, zCom4; Verszählung German oder KJV;
  `entries(book, start, end)` fasst gleiche Einträge benachbarter Verse zu einem Bereich zusammen; lineare Suche)
  und `Lexicon` (RawLD, RawLD4, zLD; `lookup` mit `@LINK`, `keys(prefix, limit)`, `search` in Stichwort und Text).
  Klartext über `text::to_plain`, je Eintrag höchstens `MAX_ENTRY_CHARS` (20 000). Seit R15 füllt `scan` auch
  `Scan::commentaries`/`lexicons`, `BibleInfo.kind` (`SourceKind`); `Scan::find` bleibt bei Bibeln. HREFCom,
  RawFiles, verschlüsselte Module und fremde Verszählungen: `supported: false` mit Begründung. Testmodule:
  `fixture::Layout::{RawCom, ZCom}` und `fixture::{LexiconFixture, LexLayout, write_lexicon}` (Index, `.dat`,
  zLD-Blöcke mit ZIP), Tests `tests/reference.rs`.
- **Server `bibles.rs`:** Cache je Art (höchstens 8 offene Module), `commentary_by_name`,
  `lexicon_by_name` (`Source::Ready`/`Problem`), RPC `commentary.*`, `lexicon.*`, `bible.work_sources`/
  `set_work_sources`, `bible.modules` mit `kind`. HTML-Bausteine für die Literatur: `commentary_panel_html`
  (`<details class="commentaries" data-widget="commentaries">`, je Modulname, Einträge, Lizenz; alles maskiert)
  und `commentary_hits_html` (Gruppen „Kommentar“ mit Links `/literatur/bibel?work=…&ref=…&kom=1<tail>`, insgesamt
  3 s, 10 Treffer je Gruppe). Stil `static/commentaries.css`. MCP `mcp/bibles.rs`: `commentary_lookup`,
  `lexicon_lookup`, `lexicon_search` (nur lesend).
- **`bible_research.rs` (Daten des Faktenblocks):** `research(state, work, persona, text, max_chars)` – Schalter
  und Erlaubnis (`personas::bible_state`: @belege Vorgabe an, Lektor Vorgabe aus, sonst nie), Stellen nur aus
  `baruch_scripture::scan` (ungültige → Hinweis, kein Text), je Stelle der Wortlaut der Bibel des Werks, dann je gewähltem
  Kommentar seine Einträge; zuletzt Lexikonartikel zu Stichwörtern (seit Runde 17b: Strong-Nummern der Verse bei
  einer Bibel mit Markup, dann Strong-Nummern und großgeschriebene Wörter des Textes mit einer Lemma-Probe;
  höchstens 5 je Lexikon). Gesamtlänge ≤ `max_chars` (je Beleg ≤ 1500), höchstens 12
  Stellen; Lesen in `spawn_blocking`. `None`, wenn kein Block dorthin gehört (Schalter aus, nicht erlaubt, weder Bibel noch
  Kommentar noch Lexikon gewählt). Die Darstellung im Kontext ist Sache des Lektors (`assist/bible_facts.rs`).
- **Store (Migration 0037):** `work_sword_source (work_id ON DELETE CASCADE, kind commentary|lexicon, module,
  position)`, `persona_bible (persona_id, enabled)`; `src/sword_sources.rs`. Test `tests/migration_0037.rs`.
- **Seite:** Einstellungen → Werk, Block `#kommentare` (`settings/work.rs`, Formular `POST /einstellungen/werk/bibeln`,
  Behandler `connections::pages::work_bibles_act`, weil die Router von `settings/` nicht zu diesem Zweig gehören).

### MCP-Token (Migration 0016) {#mcp-tokens-migration-0016}

- Tabelle `mcp_token (id, name, hash UNIQUE, prefix, created, last_used, revoked)`, Teilindex
  `mcp_token_active_name` (Name unter gültigen eindeutig). Store `src/mcp_tokens.rs`: `MCP_TOKEN_PREFIX =
  "baruch_"`, `McpToken` (ohne Hash/Klartext), `mcp_token_create` (32 Bytes `getrandom`, Base64url ohne Padding,
  SHA-256 hex, `prefix` = erste 12 Zeichen), `mcp_token_list`, `mcp_token_revoke` (idempotent),
  `mcp_token_verify` (nur nicht widerrufene; `last_used` höchstens einmal je Minute).
- Server `src/mcp/tokens.rs` (RPC `mcp_token.*`, nur Mensch), `src/settings/access.rs` (Seite und
  `POST /einstellungen/mcp/act`; der frische Klartext geht über ein eigenes `tokio::task_local` nur in diese eine
  Antwort, `Cache-Control: no-store`), Vorlage `templates/settings/mcp.html`.

### Aufträge aus der Benutzeroberfläche, Fäden und MCP-Grenzen (Runde 15, „auftraege“) {#tasks-from-the-user-interface-threads-and-mcp-limits-round-15-auftraege}

- **Strom:** `assist/routes.rs` leitet die Ereignisse aus `Progress` weiter (`posted`, `retry`, `compressed`,
  Schnittstelle S2) und gibt `done.compressed` aus; `assist.ts` legt den eigenen Beitrag sofort in den Chat,
  verwirft bei `retry`, was die Persona bisher gezeigt hat, und meldet `compressed` als `baruch:toast` (Empfänger `toast.ts` des
  Lektors). Abbrechen: im Chat sind Beitrag und fertige Antworten schon gespeichert (neu zeichnen), sonst nichts.
- **Hinweisseite:** `assist::routes::notice_page[_with]` rendert `assist_notice.html` (Status 200) in
  `view::environment()` + eigener Vorlage; ist `fragments/toast.html` (Lektor) dort registriert, bindet die Seite sie
  ein, sonst eine gleichwertige Box. Genutzt von `/assistenten/act`, „an Persona geben“ und der Bibelseite.
  Rücksprünge nur auf eigene Pfade (`safe_back`).
- **Fäden ohne Abschnitt (Migration 0038):** Tabellen `chat_thread (id, work_id → work ON DELETE CASCADE, title,
  created_by, created, updated)` und `chat_thread_post (thread_id → chat_thread ON DELETE CASCADE, position UNIQUE je
  Faden, author, body, sources JSON, created)`; Store `src/threads.rs` (`thread_list/get/create/add_post`, je eine
  Transaktion, unbekannter oder fremder Faden → `Refused`). Server `src/threads.rs`: RPC `thread.*`, Seiten
  `/faeden`, `/faden`, `POST /faden/act`, und `threads::chat` – ein eigener schlanker Kontext (Rollenprompt, Werk,
  letzte 12 Beiträge, Aufgabe) mit eigenem Modellaufruf über `state.model` (verbotene Einleitung einmal neu,
  `hooks::mark_answer`, `CUT_NOTE`); `assist::run` bleibt an Abschnitte gebunden. Test `migration_0038.rs`.
- **Relay:** eine Stufe – nur Antworten auf die ursprüngliche Nachricht rufen weiter, jede Persona höchstens einmal,
  alle Antworten zusammen `MAX_CHAT_PERSONAS`. Im Faden in `threads::chat` (`relay: true` nur für MCP), im
  Abschnittschat über MCP bis Runde 22 in `mcp/tools.rs` (`relay_section` nach `assist::chat`; seit Runde 23 ein Weg
  `assist::chat::relay_targets` für Benutzeroberfläche und MCP: nur Erwähnungen aus Antworten,
  die nicht selbst aus dem Relay von Runde 30c stammen – `ChatOutcome::relayed` –, gerahmt über
  `OrderFrame::relay` mit `relay_text` als Aufgabe). Der Chat der Benutzeroberfläche (`persona.chat`) reicht nur bei
  direkter Ansprache weiter (Runde 30c).
- **Warteschlange je MCP-Client:** `mcp/quota.rs` (über `#[path]` in `mcp/tools.rs` eingebunden, `mcp.rs`
  unverändert): Zähler je (Zustand des Servers, Client-Handle), `Slot` gibt beim Fallenlassen frei; höchstens 2 laufende
  oder wartende `chat`/`ask_persona` je Client, sonst `isError` vor jeder Modellanfrage. Seit Runde 23
  teilen sich alle Clients ohne Token zusätzlich vier Plätze (siehe „Runde 23 – Härtung“, mcp-namen).
- **Einfälle:** `ideas::idea_source` nennt die Herkunft („Einfall n von @x“) als Quelle des Beitrags; die Seite
  des Einfalls bietet die Personawahl (`persona` im Formular).
- **Marke „neu“ (`chat.ts`):** Beiträge des Menschen tragen `data-post-own` (die Vorlage vergleicht mit „Autor“ =
  `mirror::HUMAN_NAME`); gemerkt werden nur Zeiten des Servers oder „leer“, dazu je Werk der letzte Stand für den
  ersten Besuch eines Abschnitts.

### Integration Runde 15 (Integrator, 2026-11-08) {#integration-round-15-integrator-2026-11-08}

- `team/anbieter` → `team/lektor` → `team/auftraege` gemergt (`--no-ff`); die Schnittstellen S1/S2 lagen in den anderen Zweigen
  per Cherry-Pick, bei Konflikten galt die Revision des Besitzers (§9), dazu die Verdrahtungszeilen
  aus §10 vereinigt (`lib.rs` STYLE, `tests/main.rs`, `web/src/main.ts`, `docs/RPC.md`).
- **`.toast` doppelt benutzt:** seit Runde 1 hat `app.css` `.toast` für die kurzen roten Fehlermeldungen
  (`literature.ts`, `outline.ts`, `workspace.ts`, `ideas.ts`, `wysiwyg.ts`, fest unten in der Mitte). Der Hinweis des
  Lektors nutzt dieselbe Klasse; ohne Anpassung klebte er ohne JavaScript unten, und die Fehlermeldungen verloren
  ihr Rot. Darum gilt `static/toast.css` nur für `.toast[data-widget="toast"]` (Test
  `round15::the_hint_toast_does_not_restyle_the_error_toasts_of_other_widgets`).
- **Quellen für [Sn]:** `Prepared.bible_labels` (Kurzstelle je Beleg) und `bible_facts::cited_sources`: jedes in der
  Antwort zitierte `[Sn]` wird eine Quelle des Beitrags („SWORD [Sn] <Art> <Modul>, <Stelle>“); ohne das blieb die Nummer
  für den Leser unauflösbar.
- **Kontext mit OpenAI-kompatibel:** `models::context` und `context_cached` geben für `kind = openai` nie eine
  gemeldete Länge zurück (Vertrag §3), auch wenn eine Eliezer-Verbindung mit derselben Adresse die Caches gefüllt hat
  (je Adresse und Modell).
- **Prüfung MultiGPT:** die Grenze je Anfrage liegt 1 s über der gemeinsamen Grenze (`check::multigpt`); bei gleicher
  Länge lief unter Last die Grenze von `list_collections` zuerst ab, und die Prüfung meldete „steht“ (Test
  `round13_quellen::the_check_has_one_timeout_for_all_requests` scheiterte im vollen Lauf einmal).
- MCP `initialize` nennt die neuen Werkzeuge (`commentary_lookup`, `lexicon_*`), `scope`, den Faden ohne Abschnitt und
  die Warteschlange je Client. `/faeden` und `/literatur/bibel` stehen in `round9_menu::all_pages`.
- Ende-zu-Ende `tests/round15.rs`: Faktenblock mit dem echten `bible_research::research` (Bibel, Kommentar mit
  eingeschleuster „Anweisung“, Lexikon) für `@belege` und den Lektor mit dem Schalter; Kommentarfeld und Gruppe
  „Kommentar“; „an Persona geben“ von der Bibelseite; komprimierter Kapitelbefund über das Formular ohne JavaScript →
  Hinweisseite mit dem echten Toast; Persona an einem Ollama-Nachbau mit gemeldeter kleinerer Länge (komprimiert,
  nacheinander); externe Verbindung ohne Freigabe auch für Zusammenfassungen, ohne Anfrage; OpenAI ohne Länge; MCP
  `commentary_lookup` über den echten Eingang.

## Suche und Spiegel im Einzelnen {#search-and-mirror-in-detail}

- **baruch-library:** `fts_query` macht aus NUL und Steuerzeichen Worttrenner; bleibt nichts übrig,
  `EmptyQuery`. FTS5 `snippet()` arbeitet mit den Marken U+E000/U+E001; `search.rs::snippet_html`
  maskiert den Text (`& < > " '`) und übersetzt die Marken, ausgewogen, in `<mark>…</mark>`.
  Markierungen stehen als `kind = 'highlight'` im Index (Trigger aus `0103_highlights.sql`, auch beim
  Umbenennen und Löschen des Titels); die Abfrage verbindet sie mit `library_highlight`, lässt
  `rejected` aus und füllt `quote_id`/`highlight_id` im Treffer. `build.rs` baut bei neuen Migrationen neu.
- **Beiträge im Index (0105, `0105_post_index.sql`):** `post_fts_delete` entfernt die Indexzeile eines
  gelöschten Beitrags, `post_fts_move` trägt Titel und Werk des neuen Abschnitts ein, wenn sich `post.node_id` ändert
  (Löschen von Werken, Zusammenführen von Gliederungspunkten); die Migration räumt verwaiste `post`-Zeilen.
  Die Abfrage lässt Werktreffer ohne lebenden Abschnitt ohnehin aus.
- **baruch-gitmirror:** vor jedem Commit werden alle Zwischenordner der alten und neuen Pfade mit
  `symlink_metadata` geprüft, beim Schreiben fehlende Ordner einzeln angelegt (`walk_parents`).
  Ein Symlink ergibt `MirrorError::InvalidPath`, bevor im Arbeitsordner etwas geändert wird. Eine Datei,
  die selbst ein Symlink ist, wird durch eine echte Datei ersetzt, ihr Ziel bleibt unberührt. Ohne
  Änderung am Baum entsteht kein Commit, es sei denn `CommitRequest::allow_empty` ist gesetzt (Log-Commit
  mit demselben Baum wie sein Vorgänger).
- **Nachholen:** scheitert ein Commit, holt der nächste Schnappschuss den vollen DB-Stand nach (Test
  `a_mirror_catches_up_after_an_error`).

## Ablauf einer Änderung {#course-of-a-change}

1. `revision.accept`, `revision.set_current`, `revision.save`, `revision.update` oder `revision.delete`
   nimmt die **Schreibsperre** (`AppState::write_lock`).
2. Die Änderung wird in einer Transaktion nach SQLite geschrieben.
3. Weiter unter der Sperre: Schnappschuss der aktuellen Revisionen, Commit im Spiegel mit Autor, Rumpf und
   Trailern dieser Änderung (`Persona`, `Model`, `Baruch-Node`, `Baruch-Revision`, `Baruch-Action`; siehe
   „Revisionsbaum“).
4. Sperre frei. Die Antwort trägt `commit` oder, bei einem Spiegelfehler, `mirror`.

Die Sperre deckt die Schritte 2 und 3: liefen zwei Änderungen gleichzeitig, enthielte der erste
Schnappschuss sonst beide, und der zweite Commit fiele weg. Verwerfen und Beiträge ändern den Spiegel nicht
und laufen ohne die Sperre.

## Schutz der Schnittstellen {#protection-of-the-interfaces}

`/rpc`, `/act` und `/mcp` liegen hinter `guard::same_origin`: `Sec-Fetch-Site` muss `same-origin` oder
`none` sein, `Origin` (falls vorhanden) muss zu `Host` passen; sonst 403. `/rpc` und `/mcp` verlangen
`Content-Type: application/json` (sonst 415), damit ein fremdes `fetch` einen Preflight bräuchte.
Lesende GET-Seiten, die mit Zugangsdaten teure Wege auslösen würden – die semantische Suche in
`GET /literatur?…&lmode=rag` und `GET /einfaelle?…&mode=rag` – prüfen dieselbe Regel (`guard::check`); für
eine fremde Seite läuft keine Suche, die Seite nennt `literature::FOREIGN_PAGE`.

`/mcp` prüft dann die Gegenseite: ohne Token nur Loopback (`127.0.0.1`, `::1`, `::ffff:127.0.0.1`),
sonst 401 mit `WWW-Authenticate: Bearer realm="baruch"`. Dafür startet `main.rs` mit
`into_make_service_with_connect_info::<SocketAddr>()`; fehlt `ConnectInfo`, gilt die Anfrage als fremd.
Vor allen Routen, auch den lesenden, steht `guard::local_host`: `Host` muss eine IP-Adresse sein (v4 oder `[v6]`,
mit oder ohne Port), `localhost` oder ein beim Start zusätzlich erlaubter Rechnername
(`AppState::allowed_hosts`, siehe „Host-Regel hinter einem Reverse-Proxy“ unten), sonst 421. So
funktioniert DNS-Rebinding nicht (ein fremder Name, der auf 127.0.0.1 zeigt, schickt seinen Namen als `Host`). Ohne `Host`
(nur ohne Browser) geht es durch.
Mit `Authorization: Bearer <Token>` (Runde 10) prüft `mcp::endpoint` das Token über
`Store::mcp_token_verify` (nur der SHA-256-Hash gespeichert): gültig → Zugriff von jedem Rechner, Clientname =
Tokenname; ungültig oder widerrufen → 401 mit `error="invalid_token"`, auch über Loopback.
`guard::remote_only_mcp` (im Router um alles, innerhalb von `local_host`) lässt von anderen Seiten außerhalb von
Loopback nur `/mcp` durch, sonst 403; ohne `ConnectInfo` (Tests) geht die Anfrage durch.
**Clickjacking:** `guard::no_framing` (äußerste Schicht, also auch bei 403/421) setzt auf jede Antwort
`X-Frame-Options: DENY` und `Content-Security-Policy: frame-ancestors 'none'` (an eine vorhandene CSP wie die
`sandbox`-CSP der Bilder angehängt). Sonst könnte eine fremde Seite den Zustimmungsdialog der Gliederung, die
Freigabe „extern“ oder die Verbindungen unsichtbar einrahmen; das eingerahmte Formular ginge als same-origin ab, und
`same_origin` griffe nicht. Baruch rahmt sich nirgends selbst ein (Test `round15_nachbesserung`).

**Über-Seite (Runde 24).** `about.rs`: Seiten `/ueber` und `/ueber/lizenz`; Konstanten `VERSION`, `LICENSE_ID`,
`REPOSITORY`, `HOMEPAGE`, `LICENSE_TEXT`, dazu `THIRD_PARTY_PLACES`, `release_url()` und `find_third_party` (sucht
`THIRD-PARTY-LICENSES.txt` relativ zum laufenden Programm, liest es nie). Kein Netzzugriff, keine Datenbank; Baruch
fragt nirgends nach neuen Versionen. Seit Runde 24 wird der Schlüsselbund (`connections/secrets.rs`) je Zielsystem
über Features der Crate `keyring` gewählt (`[target.'cfg(target_os = …)'.dependencies]` in
`crates/baruch-server/Cargo.toml`: Linux `sync-secret-service` + `crypto-rust`, macOS `apple-native`, Windows
`windows-native`); die Zweige für macOS und Windows werden übersetzt, sind aber nie gelaufen. Die Version hat eine Quelle
(`[workspace.package] version`); `tests/round24_abnahme.rs` hält die Stellen fest, die sie wiederholen
(`site/hugo.toml`, `CHANGELOG.md`, `README.md`).

**Host-Regel hinter einem Reverse-Proxy (Runde 24).** Seit Runde 24 läuft der Zugriff unter einem Rechnernamen über
`guard::AllowedHosts` (`crates/baruch-server/src/guard.rs`), von `main.rs` aus `--allow-host NAME`
(wiederholbar, nur `serve`) und `BARUCH_ALLOWED_HOSTS` (Komma oder Leerraum) gefüllt und vor `app(state)` in
`AppState::allowed_hosts` gesetzt; `AppState::open_with` lässt die Liste leer, alle Tests und `import`/`export`
laufen also wie vor Runde 24.

- `AllowedHosts::parse` nimmt nur ganze Namen: 1–253 Zeichen, Labels von 1–63 Zeichen aus `a–z0–9-` (kein `-` am
  Rand), in Kleinbuchstaben gespeichert, ohne Port, ohne Wildcard, keine IP-Adresse, nicht `localhost`, letztes Label
  keine Zahl (`127.1`, `0x7f.1` – Browser lesen das als IPv4). Ein ungültiger Name ist ein Fehler mit dem Namen
  in der Meldung; `main.rs` beendet `serve` dann mit Exit 2, bevor der Datenordner geöffnet wird.
- `AllowedHosts::allows(host)` = `allowed_host(host)` oder `is_extra(host)`; `is_extra` vergleicht den Namen
  ohne Port (Port wie bei `allowed_host`: 1–5 Ziffern) und ohne Beachtung der Groß-/Kleinschreibung mit der Liste. `local_host` lässt
  genau das durch; alles andere bleibt 421 (anderer Name, Unter- oder Obername, Leerzeichen, Wildcard).
- **Nie lokal:** `guard::via_proxy(hosts, headers)` ist wahr, wenn `Host` ein zusätzlicher Name ist – und, sobald
  überhaupt Namen erlaubt sind, auch wenn `Host` fehlt oder unlesbar ist (dann steht ein Proxy davor, und
  „lokal“ würde nicht angezeigt). `mcp::endpoint` nimmt eine Anfrage ohne Token nur an, wenn die Gegenseite
  Loopback ist **und** `via_proxy` falsch ist: unter einem zusätzlichen Namen verlangt `/mcp` immer ein Token,
  obwohl die Gegenseite (der Proxy auf demselben Rechner) Loopback ist. Ohne erlaubte Namen ist `via_proxy`
  immer falsch.
- **Origin-Prüfung unverändert:** `guard::check` vergleicht `Origin` (ohne Schema) wörtlich mit `Host`. Der
  Proxy muss `Host` also unverändert durchreichen (einschließlich Port); sonst 403. Das Schema spielt keine Rolle, darum
  geht `Origin: https://name` gegen `Host: name` durch, obwohl Baruch selbst nur HTTP spricht.
- **Bewusst nicht ausgewertet:** `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host`, `Forwarded`. Sie
  kämen von jedem, der Baruch direkt erreicht; sie auszuwerten wäre nur mit einer Liste vertrauenswürdiger Proxys sicher.
  Folge: `guard::remote_only_mcp` sieht für jede Anfrage über den Proxy Loopback und lässt alle Routen durch –
  ohne Konten liegt der Zugriffsschutz ganz beim Proxy (ohne Konten hat Baruch keine Anmeldung, `docs/Betrieb.md`;
  mit Konten verlangt die Schicht `auth::gate` seit Runde 35 eine Sitzung, siehe „Runde 35 – Konten und Rollen“). Steht der Proxy auf
  einem anderen Rechner, ist die Gegenseite nicht Loopback, und es bleibt bei „nur `/mcp`“.
- **Bekannte Grenze:** schreibt der Proxy `Host` auf `127.0.0.1:<Port>` um, ist die Anfrage von einer lokalen nicht zu
  unterscheiden; `/mcp` ginge dann ohne Token. Baruch kann das nicht erkennen; darum verlangt `docs/Betrieb.md`
  nach dem Einrichten eine Probe.
- Die Seite „MCP-Zugang“ (`settings/access.rs`) zeigt die Adresse unter einem zusätzlichen Namen als
  `https://<Host>/mcp` (Annahme: TLS im Proxy; Baruch kennt das Schema der Anfrage nicht) und den Befehl mit
  `--header "Authorization: Bearer <Token>"`.
- Tests: Unit-Tests in `guard.rs` (`parse`, `allows`, `is_extra`, `via_proxy`), `tests/round24_betrieb.rs`
  (Router mit gesetzten Namen, Programm mit ungültigem Wert).

Offen: kein eigenes TLS. Seit Runde 35: Anmeldung mit Konten (ohne Konten weiterhin keine); das Sitzungscookie
trägt hinter dem Proxy `Secure`, die übrigen Cookies (Darstellung, zuletzt geöffnetes Werk) noch nicht.

## Literatur {#literature}

Die Literatur ist ein Popup der Kopfzeile und hat eigene Seiten; die Werkseite bleibt dreispaltig.

- **Daten** (`baruch-library`, Migration `0103_highlights.sql`): Tabelle `library_highlight` mit Titel
  (`ON DELETE CASCADE`), optionalem Zitat (`ON DELETE SET NULL`), Fundstelle (`locator`), Zeichenbereich
  `start_char`/`end_char` (beide NULL oder `0 ≤ start ≤ end`, Unicode-Zeichen; im Zitat
  prüft `add_highlight` zusätzlich, dass das Ende im Zitat liegt und dass der Text, abgesehen von Leerraum, der Ausschnitt ist –
  `normalize_space`: Folgen von Unicode-Leerraum werden ein Leerzeichen, Ränder entfallen –, und speichert dann
  den genauen Ausschnitt des Zitats), Text (nicht leer), Art
  (`quote`, `core`, `contradiction`, `check`), Notiz, Urheber (`human` oder `assistant` mit Modell) und
  Status (`active`, `proposed`, `rejected`). Eine CHECK-Bedingung erlaubt Personas nur `proposed`/`rejected`.
  `src/highlights.rs`: `add_highlight` (Mensch → `active`, Persona → immer `proposed`; ohne Fundstelle die
  Fundstelle des Zitats), `list_highlights`, `get_highlight`, `update_highlight`, `delete_highlight`,
  `cite_typst`. Fehler über eigenen `HighlightError` (`Library`, `Unknown`, `Empty`, `InvalidRange`, `TextMismatch`),
  `LibraryError` bleibt unverändert.
- **Server** (`src/literature.rs`): eigene Vorlagenumgebung (`OnceLock`, Vorlagen `literature.html`,
  `source.html`, `fragments/library.html`, `fragments/highlights.html`); die der Werkseite bleibt, wie
  sie ist. `pages()` liefert `GET /literatur` und `GET /quelle`, `writes()` `POST /literatur/act` (im Router der
  schreibenden Routen, also hinter `guard::same_origin`). `handles`/`call` bedienen `literature.render` und
  `highlight.*` aus `rpc::call` heraus. Das Modul baut Adressen selbst (`query_escape`) und reicht sie als
  sichere Werte an die Vorlagen.
- **Abschnitt der Literaturseiten:** `/literatur` und `/quelle` tragen den festen Schlüssel selbst: aus
  `?key=` (hat Vorrang vor `?node=`), sonst beim Laden aus der Position bestimmt. Er steht in den Links zur
  Dokumentansicht, in den versteckten Feldern aller Formulare (`place_fields`) und als `data-key` am Widget
  `source`.
- **Ein Zitat übernehmen:** `highlight.cite {highlight, key}` übersetzt `key` unter der Schreibsperre in die
  Position, liest die aktuelle Revision, hängt `cite_typst` an und legt über das unveränderte `Store::add_proposal` einen Vorschlag an (Persona
  `@literatur`, Modell `baruch-zitat`, Notiz „Zitat aus @schlüssel“). Die aktuelle Revision bleibt; der Git-Spiegel
  ändert sich erst beim Übernehmen. `cite_typst` maskiert Backslash, Backtick und
  `# [ ] * _ $ < > @ ~ = - + /` sowie eine Aufzählung „1.“ am Zeilenanfang mit einem Backslash; eine bloße Seitenzahl wird zu `S. …`.
- **Widgets** (`web/src/literature.ts`): `literature` (der `<dialog>`, Alt+L, Inhalt über
  `literature.render`, Suche in `lq` der Adresse, Werktreffer über einen versteckten `data-nav`-Link im
  Werkbereich), `source` (Kontextmenü der Dokumentansicht; Zeichenbereich in Unicode-Zeichen wie in der
  Datenbank, Anfang und Ende aus DOM-Bereichen ab dem Anfang des Zitats, Text wörtlich über `Range.toString()` statt
  des zusammengefallenen `selection.toString()`) und die Markierung der Fundstelle im Textfeld mit
  `#fundstelle`. `cite` über `/literatur/act` verlangt das Feld `key` (fehlt → 422 „Abschnitt fehlt.“, existiert
  der Punkt nicht mehr → 409); das Kontextmenü zitiert nur mit `data-key`.
- **Volltext und Prüfung (Runde 13, `src/literature/fulltext.rs`, Migration `0106_source_remote.sql`):**
  `library_title_remote` verknüpft einen Titel mit einem Dokument (`way = 'multigpt'`, Sammlung, Dokument;
  `baruch_library::{RemoteRef, set_title_remote, title_remote, title_for_remote}`), `library_highlight_check` merkt sich
  das Ergebnis je Markierung (`FulltextCheck`: `found`, `not_found`, `unverifiable`, mit Meldung und
  Zeit; `set_highlight_check`, `highlight_checks`, `highlight_check(_entries)`). `check_range(volltext, start,
  end, text)` vergleicht wie `check_excerpt` (Leerraum zusammengezogen, Unicode-Zeichen) und gibt nie
  `unverifiable` zurück. `fulltext::read` (RPC `source.read`, MCP `source_read`, Seiten) liest über
  `MultigptWay::from_settings`; bei Misserfolg die lokale Kopie (`local_copy` → `sync::local_document`; ohne Sammlung
  die einzige Kopie mit passender Dokument-ID) oder ein ehrlicher Fehler. `fulltext::adopt` (nur RPC/Formular) legt Titel und Verknüpfung an
  unter der Schreibsperre an und ruft `rag::notify(state, None)` auf. `fulltext::check_highlight` prüft nach jedem
  `highlight.add`/Formular „Hervorheben“ ohne Zitat, aber mit Bereich; die Markierung ist vorher gespeichert.
  Vorlage `source_multigpt.html` (eigene Umgebung der Literatur); `/quelle` zeigt den Volltext im Abschnitt
  `#volltext`. GET-Seiten laden den Volltext nur, wenn `guard::check` die Anfrage als eigene erkennt.
- **An eine Persona geben (Runde 15):** `literature::to_persona` (Formular `action=persona`, RPC
  `literature.to_persona`, Bibelseite) baut aus Frage, markiertem Text und Quellenangabe eine Frage (höchstens
  `assist::MAX_QUESTION_CHARS`, sonst sichtbar gekürzt) und ruft `assist::run` mit `Mode::Ask`, `post_question`
  und der Quelle als `sources`; ohne `key` die Hinweisseite „Abschnitt wählen“ (`assist::routes::notice_page`). Der
  Stub ist weg. Kontextmenü auch im Volltext (`literature.ts`: Bereich aus `data-offset` des Teils).
- **Eine Markierung übernehmen (Runde 15):** `literature::own_highlight` kopiert eine Markierung einer Persona über das
  unveränderte `baruch_library::add_highlight` mit dem Menschen als Urheber (also `active`), Notiz „übernommen von @x
  (Hervorhebung n)“ – daran erkennt Baruch eine schon übernommene; das Original bleibt `proposed`. Keine Migration
  (Runde 15), kein MCP-Werkzeug.
- **Bibeln in der Literatur (Runde 15, `literature/bibles.rs`):** Feld und Treffergruppen über die vorhandene
  Server-API `bibles::{modules_value, by_name, lookup, search}` (unverändert) und die HTML-Bausteine
  `bibles::commentary_panel_html`/`commentary_hits_html` des Zweigs anbieter (im eigenen Zweig leer). Die Modulordner werden
  nur mit konfigurierten Ordnern oder einer gewählten Bibel des Werks gelesen (`scan_allowed`), damit eine
  Seite ohne Einstellung – und kein Test – die Standardordner des Rechners liest. Suche im gemeinsamen Feld: höchstens
  drei Bibeln, je 10 Treffer, zusammen 3 s (`tokio::time::timeout`). Seite `GET /literatur/bibel` und Formular
  `POST /literatur/bibel/act` (Routerzeilen in `lib.rs`); das Zitat entsteht wie `highlight.cite` über
  `Store::add_proposal` unter der Schreibsperre (Autor `@literatur`, Modell `baruch-zitat`), der Text kommt aus dem
  Modul (`ref`), nie aus dem Formular. Die Bibliothek trägt keine Markierungen auf Bibeltexten (Markierung =
  Titel + Zitat/Bereich); die Seite sagt das ehrlich. Vorlagen `literature_bible.html`,
  `fragments/literature_bibles.html` (auch in `view::environment`, `library.html` bindet sie mit `ignore missing` ein,
  weil `baruch-library` das Fragment allein rendert), Widget `bibles.ts`, Stil `static/literature_bibles.css`.

### Runde 16b – zeitwaechter {#round-16b--zeitwaechter}

Sprechzeit je Gliederungspunkt und insgesamt, Zielzeit je Werk, Anzeige in Gliederung und Editorkopf, RPC und MCP;
dazu lokal/extern im Akkordeon „Assistenten“.

- **Schnittstelle R16-S1 (erster Commit der Runde):** `baruch_typst::timing` (rein, keine Abhängigkeit):
  `spoken_words` (Zählregeln 1–8, siehe `docs/Bedienung.md` „Zeitwächter“; ein Zeichenscanner mit Zeilenanfangs-
  Regeln für Überschriften, `#set/#show/#let/#import/#include/#heading` und Listenmarken; Klammergruppen werden
  im Code- oder Markupmodus übersprungen, gesprochene Inhaltsblöcke über einen Stapel offener `[`), `speaking_seconds`
  (ceil(Wörter·60/wpm), wpm auf 60–250 begrenzt), `clock`, Konstanten `DEFAULT_WORDS_PER_MINUTE` 110,
  `MIN_/MAX_WORDS_PER_MINUTE` 60/250, `MAX_TARGET_MINUTES` 240. Store: Migration `0039_work_timing.sql`
  (`work_timing`, eine Zeile je Werk, CHECK-Grenzen, `ON DELETE CASCADE` – `Store::delete_work` kennt die Tabelle nicht),
  `baruch_store::TimingSettings` (Vorgabe 110/keine Zielzeit ohne Zeile), `Store::timing_settings` und
  `set_timing_settings` (Grenzen vor dem Schreiben → `StoreError::Refused`, nichts geändert; `BEGIN IMMEDIATE`).
  Server: `crate::timing` mit `TimingStatus`, `SectionTiming`, `WorkTiming` (`shown()` = Predigt oder Zielzeit),
  `timing_of` (rein: je Punkt die aktuelle Revision; Teilbaum rückwärts aufsummiert, weil Nachkommen in
  Baumreihenfolge hinter dem Elternteil stehen), `work_timing`, `timing_json`.
- **Nur lesend, nicht gespiegelt:** `work.timing`, MCP `work_timing` und die Anzeige nehmen keine Schreibsperre
  und schreiben keinen Commit. Die Einstellungen stehen nur in SQLite (wie die Farben der Personas).
- **RPC** (`timing::handles/call`, eine Zeile im werkbezogenen `match` von `rpc::call`): `work.timing`,
  `work.timing_set` (nur Mensch; `/act` kennt die Methode nicht, MCP hat kein Werkzeug dafür), `timing.count`
  (höchstens 1 000 000 Zeichen) – so zählt das Widget mit derselben Regel wie der Server.
- **Anzeige:** für die Werkseite lädt `view::extras` die Zeiten (`timing::work_timing`) und die Freigaben der
  Personas (nur über `personas::call("persona.list")`: `locality`, `external_allowed`, `connection`) in
  `ViewExtras`; `rpc::library_view` hängt sie als Feld `extras` an `LibraryView` (die Zeichenfunktionen von
  `view` sind synchron). Abweichung vom Vertrag: dafür bekam `rpc.rs` neben der Verteilzeile das Feld und einen
  Aufruf in `library_view` (in Runde 16 gehört die Datei ohnehin nur R16b). `attach_timing` hängt Dauer und
  Zeitmarke an die Einträge der Gliederung (Punkte mit Unterpunkten: Teilbaum), `timing_context` liefert den
  Kontext `timing` für Baum und Editorkopf (`none`, wenn nicht `shown`). Die Abschnittszeit im Editorkopf ist die
  der gezeigten Revision, `data-rest` = Gesamtzeit ohne die aktuelle Revision des Abschnitts.
- **Widget** `web/src/timing.ts` (`data-widget="timing"`): hört auf `input` im Editor (`[data-widget=wysiwyg]`,
  Textfeld und Satzansicht), entprellt 700 ms, sendet den Quelltext an `timing.count` und rechnet `rest + seconds`;
  Fehler leise. Keine zweite Zählregel in TypeScript. Seit Runde 17b (L6) bekommt es den Quelltext in der Satzansicht über
  das Ereignis `baruch:serialize-text` am Editorformular (`detail.wait`, Antwort von `wysiwyg.ts`, Vertrag §2.6);
  ohne Antwort zählt es `innerText` in der offenen Satzansicht, sonst das Textfeld.
- **Einstellungen → Werk:** Block „Predigtzeit“ (`id="zeit"`, `settings/werk.html`), Formular
  `POST /einstellungen/werk/zeit` (`settings::work::timing_act`, Route in `settings::writes()` hinter
  `same_origin`), 303 mit `ok=zeit#zeit` oder `zeitfehler=1#zeit`; `settings.get page=werk` nennt `timing`.
- **Personas lokal/extern:** `fragments/assistants.html` zeigt `locality` je Persona; extern ohne Freigabe
  (`PersonaAccess::blocked`) wird markiert („Gesperrt“, Hinweis auf Einstellungen → Assistenten) und fällt aus
  `active_personas` heraus (keine Wahl für Prüfen/Vorschlag/Chat). Die eigentliche Sperre bei einem Auftrag bleibt im
  Persona-Backend (Runde 15b).
- **Tests:** `baruch-typst/tests/timing.rs` (Pflichtbeispiele, Regeln), `baruch-store/tests/timing.rs`,
  `baruch-store/tests/migration_0039.rs` (nachgebaute Datenbank bis 0038 → 0039 verlustfrei, CASCADE),
  `baruch-server/tests/round16_zeit.rs`; Werkzeuglisten in `mcp.rs` und `round4.rs` (`work_timing` direkt nach `outline`; am Ende hätte es die
  Folgeprüfungen in `round12_multigpt.rs`/`mcp_ideas.rs` gebrochen, darum bleibt `mcp_ideas.rs` unverändert).

### Runde 16a – predigt-vorlage {#round-16a--predigt-vorlage}

- **Werkvorlage Predigt** (`baruch-store/src/works.rs`): `Preset::Sermon` legt in einer Transaktion
  `numbering_scheme = 'sermon'` und die Punkte aus `SERMON_OUTLINE` an – „Predigttext“ (ohne Nummer), „Hinführung“,
  „Beobachtung am Text“, „Deutung“, „Zuspruch“, „Anwendung“ (ohne Nummer), „Ausblick“. Die Nummern berechnet
  `preset_numbers` mit demselben `Numbering::of_scheme(Scheme::Sermon)` wie die Gliederung, nur über
  die gezählten Punkte gezählt; `node.numbered` wird ausdrücklich gesetzt (nicht über den Trigger 0006). Keine neue
  Art: „ist Predigt“ heißt `work.kind == WorkKind::Sermon`.
- **Entscheidung zur Titelwahl:** das Schema `sermon` macht den ersten gezählten Punkt zu „Einleitung“, den
  letzten zu „Schluss“ und die übrigen zu „I.“, „II.“, „III.“. Hießen die Punkte „Einleitung“ oder „Erster Hauptpunkt“,
  stünde die Nummer in der Gliederung doppelt („Einleitung Einleitung“, „I. Erster Hauptpunkt“). Darum
  tragen die gezählten Punkte Titel, die ihre Aufgabe nennen, ohne die Nummer zu wiederholen („Hinführung“, „Beobachtung am Text“,
  „Deutung“, „Zuspruch“, „Ausblick“); „Predigttext“ und „Anwendung“ zählen nicht und heißen, was sie sind. So bleibt der
  letzte gezählte Punkt immer „Schluss“, auch wenn „Anwendung“ dahinter oder davor steht. Export → Import behält
  Gliederung, Nummern und „Nummer aus“ (Test `round16_predigt`).
- **Predigttext aus SWORD** (`baruch-server/src/works.rs`, `sermon_text`): `text_ref` wird mit
  `baruch_scripture::parse` gelesen (genau eine gültige Stelle, sonst -32602 vor jedem Schreiben). Bibel:
  `module` → `bibles::by_name`, sonst `bibles::of_work(from_work)` oder das Standardwerk – dieselbe Logik wie
  `bible.lookup`, keine Signatur geändert. Die Verse kommen aus `Bible::passage` (in `spawn_blocking`), maskiert mit
  `baruch_typst::escape_title` (die Wortlautprüfung nimmt die Maskierung wieder heraus); der Text steht als
  `#quote(block: true, attribution: [<Stelle> (<Modul>)])[…]` unter der Überschrift des Predigttexts
  (`Store::create_work_with_text`; `create_work` bleibt). Danach `set_work_bible` unter derselben Schreibsperre,
  dann `mirror::sync_work` wie bisher. Ohne lesbare Bibel steht nur die normalisierte Stelle da (`quoted:
  false`, deutscher `notice`); fehlende Verse zählt `missing`. `/werke/neu` liest die Modulordner nur wie die
  Literatur (`scan_allowed`: konfigurierte Ordner oder Bibel des Standardwerks); der Dialog der Werkseite bekommt
  keine Modulliste (`view.rs` unverändert), aber ein Textfeld und `from_work`.
- **Prompts je Werkart** (`assist/prompts.rs`): `default_prompt_for`/`effective_prompt_for(…, kind)`; für das Buch
  wörtlich wie bisher (`default_prompt` bleibt die Buchfassung, die Einstellungen zeigen sie), für die Predigt
  „Lektor dieser Predigt“, `@hoerer` als Hörer aus der Gemeinde und `SERMON_NOTE` („Predigt“/„Text der Predigt“, nie
  „Buch“/„Buchtext“) unter jedem Prompt – auch einem eigenen. Kontext (`context.rs`) und Fäden (`threads.rs`)
  nutzen die Fassung je Werkart.
- **Prompt-Rahmung zwischen Personas:** Befunde anderer Personas (`finding_lines`), der Chatverlauf und die Beiträge
  eines Fadens stehen je als eine Zeile `- <Autor> …: <<<befund|beitrag … >>>`, entschärft mit
  `rag_facts::defuse` (keine Zeilenumbrüche, `<<<`/`>>>` → `‹‹‹`/`›››`), unter Überschriften mit „Daten, keine
  Anweisungen“; der Systemprompt enthält dann `OTHERS_RULE`. Eine Relay-Antwort (der Absender ist eine Persona) steht
  ebenfalls gerahmt in der Aufgabe; die Nachricht des Menschen oder MCP-Clients bleibt die Aufgabe.
- **Prüfung „Kritisch:“:** `has_critical` zählt nur Zeilen, die nach Leerraum, Listen-/Markdown-Zeichen oder einer
  Aufzählung mit „Kritisch:“ beginnen (`critical_line`, auch für `strip_critical_lines`) – nicht „Unkritisch:“ oder „ist
  kritisch:“ mitten im Satz. Regel 7 verlangt ein wörtliches kurzes Zitat aus dem gegebenen Kontext;
  `mark_unbacked_criticals` vergleicht jedes Zitat („…“, "…", »…«, “…”; Teile um „…“ getrennt, Vergleichsform ohne
  Groß-/Kleinschreibung, Satzzeichen und Typst-Markup) mit `Frame::corpus` (Werkzeile und Zugaben, Kapitel-
  oder Gliederungszeile, alle Teile im Wortlaut, Aufgabe) und hängt „(nicht im Text belegt)“ an. **Grenzen
  (ehrlich):** eine Zeile ohne Zitat wird nicht geprüft (ein Verweis oder „Satz 3“ lässt sich nicht
  maschinell belegen); Teile, die die Persona im komprimierten Kontext nur als Zusammenfassung sah, zählen in ihrem
  Wortlaut, ein wörtlich vorhandenes Zitat mit falscher Deutung bleibt unmarkiert; Fäden ohne Abschnitt
  werden nicht geprüft (kein Abschnittstext). *Abgelöst seit Runde 17a:* das Korpus ist nur der gezeigte Text,
  Fäden werden geprüft, Aussagen über Stellen bekommen einen Vermerk (siehe „Runde 17a – verszaehlung“).
- **Stücke zusammenführen:** `chunk::merge_pieces(answers, overlaps_previous)` führt nur Befunde zusammen, die
  aus dem direkt vorhergehenden, überlappenden Stück stammen (`Layout::continued`, von `overlapping` für das
  zweite und jedes weitere Teilstück eines Abschnitts gesetzt); gleich lautende Befunde zu verschiedenen Abschnitten oder
  innerhalb einer Antwort bleiben. `merge` = alle Stücke überlappend (wie bisher für Teilstücke).
- **Wirksame Kontextlänge:** bei „automatisch nach Kontext“ (und bei
  einer festen, inaktiven Wahl) vergleicht `personas::effective_lector_of` `state.model.context(…).tokens`; eine feste, aktive Wahl fragt nichts.
  `rows_and_lector` nutzt es (gleiche Signatur), `lector_of` bleibt die Regel nach der Einstellung. Der Lektor im
  Kontext (`is_lector`, Gedächtnis) kommt seitdem aus `rows_and_lector` statt aus `Work::lector`. Fäden rechnen
  mit derselben wirksamen Länge. Testnachbau: `ScriptedModel::with_reported_context_for(model, n)`.
- **Schlanker Fadenkontext:** Gliederung als Datenblock (`context::outline_block`, nur Nummern und Titel), Belege
  aus SWORD zu den Stellen der Nachricht für berechtigte Personas (`hooks::bible_research`, dieselben Schalter wie im
  Abschnitt); gekürzt wird in der Reihenfolge Belege, Gliederung, älteste Beiträge. Semantische und Websuche gibt es im
  Faden weiterhin nicht.
- **Nur Chatabschnitt:** `store_meta.chat_scope` (`chapter` | `section`, `Store::chat_scope`/`set_chat_scope`,
  keine Migration, nicht im Spiegel), RPC `persona.chat_scope_get/set` nur für den Menschen (nicht `/act`, nicht MCP),
  Formular unter Einstellungen → Assistenten. Im Chat mit `section` ist der einzige Teil der Abschnitt selbst, statt
  der Kapitelzeile steht dort `outline_block`.

### Runde 16c – stichwortzettel {#round-16c--stichwortzettel}

- **Satz (`baruch-render::notes`, rein):** `notes_source(&NotesDoc, NotesFormat, font_pt) -> String` baut aus
  fertigen Karten (`NotesCard`: Nummer, Titel, Tiefe, Zeitmarke, Dauer, Kernpunkte, Bibelstellen) eine Typst-Quelle;
  `compile_pdf` setzt sie unverändert (`compile`/`compile_pdf` werden nicht angefasst). Formate `A5` (A5 hoch, 16 pt,
  fließend) und `Cards` (A6 quer, 14 pt, `#pagebreak()` vor jedem Abschnitt, ein langer läuft über), Größe auf
  14–28 pt begrenzt. **Maskierung:** jeder Text steht nur als Typst-String (`typst_string`: `\` und `"`
  maskiert, Steuerzeichen als `\u{…}`) im Aufruf `#card((…))`; die Funktion `card` setzt Strings als Text –
  Markup in Nutzertexten wird nie gelesen. Jeder Titel und Kernpunkt steht zusätzlich als `metadata` mit dem Label
  `<baruch-note>`; `tests/notes.rs` fragt Typst über `query` und `assert`, ob ein Text mit `#`, `[`, `]`, `*`, `$`,
  `\`, `<`, `@` und `"` wörtlich ankommt (und prüft die Maße A5/A6 über die `viewBox` der SVG-Seiten).
- **Server (`src/pdf/notes.rs`, Untermodul von `pdf.rs`):** `points_of` (Regelmarker → fett → erster Satz,
  höchstens 6 Punkte zu 160 Zeichen; Lesetext über einen kleinen Zeichenscanner: Kommentare, stumme Zeilen,
  Code-Argumente, `#footnote`/`#arbeit`, Labels, Verweise, Mathematik, Raw entfallen) und `refs_of`
  (`crate::scripture::refs`, nur `RefStatus::Valid`, `normalized`, ohne Doppelte). `notes_of` lädt das Werk einmal und
  nimmt die Zeiten aus `crate::timing::timing_of` mit `Store::timing_settings` – dieselbe Berechnung wie
  `crate::timing::work_timing` (S1), keine eigene Zählung; der Test `the_clocks_of_the_notes_are_those_of_the_timer`
  vergleicht Karte für Karte mit `timing_json`. PDF über die vorhandene private Erzeugung `pdf::generate`
  (`PdfKind::Notes`, „Kein PDF (Stichwortzettel)…“): `pdf::claim` je Werk (409), `typeset::RUNS`, Zeitgrenze 120 s
  (503), Satzfehler 422. Für Tests gibt es `NotesOptions::{limit, append}` (`#[doc(hidden)]`): die Karten selbst
  können keinen Satzfehler auslösen; `append` hängt Typst an die erzeugte Quelle an, damit der Pfad 422 oder „Exit 1
  ohne Datei“ geprüft werden kann; `notes_response` ist dafür öffentlich. Verdrahtung ohne `rpc.rs`/`lib.rs`:
  `pdf::handles`/`call` reichen `notes.*` weiter, `pdf::pages()` trägt die beiden GET-Routen. Eigene
  Vorlagenumgebung (`base.html`, `notes.html`). CLI: `main.rs` liest nur `--stichwort/--format/--schrift`
  (Prüfung über `NotesOptions::parse`); `pdf::notes::write_notes_file` schreibt über `pdf::write_new_file`
  (`create_new`, eine halbe Datei wird entfernt; gemeinsam mit `write_pdf_file`).
- **Anträge (Nutzerentscheidung 2026-11-08):** `outline.request_decide` ruft `mirror::record_request_decision` für
  jede Entscheidung ohne `change` (abgelehnt, veraltet), nicht mehr nur bei `op.needs_consent()`; so gehen RPC,
  `/gliederung/act` (ohne JavaScript) und der Zustimmungsdialog (Formular oder `consent.ts` über RPC) denselben
  Weg. `mirror.rs`, `consent.rs` und `consent.ts` blieben dafür unverändert. Tests je Antragsart in
  `tests/round16_antraege.rs`; `tests/outline_mcp.rs` erwartet jetzt den Log-Commit.
- **Vorschau „neuer Teil“:** `PreviewPoint` (Store) trägt zusätzlich `kind_word`, `before_number`, `before_level`
  (aus dem Stand vor dem Probelauf); `outline::add_preview` ergänzt `label`/`before_label` mit
  `view::level_name(work.kind, level)` (Predigt: „Predigtteil“) und `changed_label`. `outline.html` und
  `consent.html` zeigen `label` statt der bloßen Nummer und „(vorher … → nachher …)“.
- **Zeiten auf `/gliederung`:** `outline::manager` holt `crate::timing::work_timing` + `timing_json`; bei
  `timing.shown` (Predigt oder Zielzeit) zeigt die Vorlage die Spalte „Dauer“ (`sections[entry.id]`, Zeitmarke „ab …“) und
  oben Gesamt-/Zielzeit mit einer Warnung als Text. Nur lesend.

### Integration Runde 16 {#integration-round-16}

Mit `--no-ff` gemergt in der Reihenfolge zeitwaechter → predigt-vorlage → stichwortzettel. Konflikte nur in
`tests/main.rs`, `docs/Architektur.md` (Abschnitte nebeneinander) und den S1-Dateien `baruch-typst/src/timing.rs`,
`baruch-server/src/timing.rs` (add/add: der Zweig stichwortzettel trug den Cherry-Pick des S1-Commits; maßgeblich ist
die korrigierte Fassung aus zeitwaechter, stichwortzettel hatte sie nicht geändert).

- **Ende-zu-Ende `tests/round16.rs`:** Predigt über das Formular „Neues Werk“ mit einem Predigttext aus einer Testbibel
  (`baruch_sword::fixture`), Text in allen Punkten, Formular „Predigtzeit“ → Zeiten und Warnung in `work.timing`, MCP
  `work_timing`, `timing.count`, Baum, Editorkopf und `/gliederung`; ohne Bibel nur die Stelle; Stichwortzettel
  als Seite, `notes.get`, `notes.pdf` (A5 und Karten), Download und `write_notes_file`; jede Antragsart an der
  Predigt ohne JavaScript abgelehnt → je ein Log-Commit; Lektor der Predigt mit einem gerahmten fremden Befund,
  belegter und erfundener Kritik, „Unkritisch:“; Prüfen der ganzen Predigt mit „Kritisch:“ hinter dem Teillabel.
- **„Kritisch:“ hinter dem Teillabel** (`assist/prompts.rs`, bei der manuellen Prüfung gefunden): die Aufgabe „Prüfe das
  ganze Kapitel … jeden mit … dem Teil“ führt bei Gemma zu Zeilen „- I. Beobachtung am Text: Kritisch: …“. Seit 16a
  zählte nur „Kritisch:“ am Zeilenanfang – Baruch fragte deshalb einen vorhandenen Befund neu an (eine zweite
  Modellanfrage) und schrieb zu Unrecht „nennt keinen mit „Kritisch:“ markierten Befund“ unter den Beitrag. Neu:
  `critical_finding`: Zeilenanfang wie `critical_line` oder „Kritisch:“ direkt nach einem Label, das mit „:“, „–“
  oder „—“ endet (höchstens 80 Zeichen, ohne Satzende – Zahlen wie „I.“ gelten nicht als Satzende –, Trenner nicht
  in einer offenen Klammer; Nachtrag siehe unten).
  `has_critical` und `mark_unbacked_criticals` nehmen `critical_finding`; `strip_critical_lines` (Vorschlag =
  Buchtext) bleibt beim strengen `critical_line`, damit ein Satz wie „Die kritische Lage: Kritisch: …“ im
  Buchtext bleibt. Neben „…“/„...“ behandelt die Zitatprüfung auch Einschübe in `[…]` als Lücke
  („das Glas ... gesprungen [ist]“ ist belegt).
- **Predigt in der Aufgabe:** Kopf und Aufgabe sagten auch bei einer Predigt „das ganze Kapitel“; `context.rs`
  (`sermon_wording`) schreibt dort „die ganze Predigt“ oder „zu dieser Predigt“.
- **Stichwortzettel:** der Kopf jeder Karte ist `block(sticky: true)` – bei der Sichtprüfung stand „III. Zuspruch“
  allein am Fuß von Seite 1 des A5-Blatts.
- **Stil:** `static/predigt.css` (in `STYLE`) für den Editorkopf (Zeit in eigener Zeile, sonst wurde der Titel
  abgeschnitten), Baum- und Gliederungszeiten, „Predigtzeit“, Stichwortzettel-Seite (Karten in einem Raster), lokal/extern
  im Akkordeon „Assistenten“ und das Kästchen „Nur bei einer Predigt“ in „Neues Werk“.
- **Manuelle Prüfung gegen Eliezer** (Integrator, Torwächter wie in Runde 15, höchstens zwei Anfragen an
  `google/gemma-4-26b-a4b`, eine erfundene „Laternenpredigt“ mit dem Predigttext Lk 15,4–6 aus einer erfundenen Testbibel,
  598 gesprochene Wörter ≈ 5:30): eine Anfrage „Prüfen, ganze Predigt“ (11,8 s, 1 928 Tokens Prompt, ganze Predigt im
  Wortlaut, Systemprompt „Lektor dieser Predigt“ mit dem Predigtvermerk) – die Antwort trug zwei „Kritisch:“-Befunde
  hinter dem Teillabel; Baruch erkannte sie nicht und fragte neu an (zweite Anfrage, 3,1 s, wieder zwei Befunde,
  wieder nicht erkannt). Beide Fehler oben behoben und mit den echten Antwortformen getestet; eine dritte
  Anfrage gab es nicht. Alle Zitate in den „Kritisch:“-Befunden stehen im Text (in Zeilen ohne „Kritisch:“ setzt Gemma auch
  Umschreibungen in Anführungszeichen, etwa „gesprungenem Glas“ für „Das Glas ist gesprungen“); ein Befund ist in der Sache falsch („Lk 15,5“ sei im Predigttext angeblich nicht belegt,
  obwohl der Vers in Lk 15,4–6 steht) – das sieht die Zitatprüfung nicht (→ Backlog).

### Nachbesserung Runde 16 (Review) {#follow-up-round-16-review}

- **Stichwortzettel, Fußnoten** (`pdf/notes.rs`): `strong_spans` übersprang `#footnote[…]` und `#arbeit[…]` nicht;
  eine Hervorhebung darin wurde ein Kernpunkt (Regel 2), die ersten Sätze fielen weg, ein Arbeitsauftrag konnte auf die
  Kanzelkarte geraten. Jetzt überspringt `strong_spans` stumme Aufrufe samt Argumenten und Inhaltsblöcken
  (`silent_call_end`, dieselbe Namensregel wie `plain`).
- **Stichwortzettel, Verschachtelung** (`pdf/notes.rs`): `plain_into` öffnete jeden Inhaltsblock `#name[…]` rekursiv
  ohne Grenze; 60 000 Ebenen (z. B. aus einer importierten `.typ`-Datei) liefen über den Stapel eines tokio-Arbeiters,
  und der ganze Server brach ab (`GET /werke/{id}/stichwortzettel` liegt nicht hinter `same_origin`).
  Jetzt höchstens `MAX_NESTING` = 32 Ebenen, tiefer Verschachteltes entfällt; damit läuft auch `group_end` höchstens
  32-mal je Ebene über den Text (linear statt quadratisch). `at_line_start` liest nur noch bis zum letzten
  Nicht-Leerraum zurück statt über die ganze Zeile. Tests: Unit-Test mit 256 KiB Stapel, Servertest `notes.get`
  und Seite mit 60 000 Ebenen.
- **„Kritisch:“ hinter dem Teillabel** (`assist/prompts.rs`): `critical_finding` erkennt nun auch ein fettes
  Label, dessen `**` hinter dem Trenner schließt („- **III. Zuspruch:** Kritisch: …“), einen
  geschlossenen Zusatz in Klammern („III. Zuspruch (Schluss): Kritisch: …“; nur ein Trenner in einer *offenen* Klammer
  zählt nicht) und eine römische Zahl als Aufzählung („II. Kritisch: …“, `strip_list_prefix` kennt „II.“/„II)“
  wie „2.“/„2)“ – das gilt damit auch für `critical_line` und `strip_critical_lines`).

### Runde 17a – verszaehlung {#round-17a--verszaehlung}

- **Alle Verszählungen** (`baruch-scripture/src/versification.rs`): `versification(name)` kennt jede Verszählung von
  `sword-modules` 0.1.0 (Calvin, Catholic, Catholic2, DarbyFr, German, KJV, KJVA, LXX, Leningrad, Luther, MT, NRSV,
  NRSVA, Orthodox, Segond, Synodal, SynodalProt, Vulg; Groß-/Kleinschreibung egal, Schreibweise von sword-modules).
  German und KJV kommen weiter aus `data/versification*.tsv` (maßgeblich); die anderen baut Baruch beim ersten
  Gebrauch aus den Daten von sword-modules (CC0, `default-features = false`: nur Daten, kein Lesen, kein Netz). Jede
  Verszählung enthält nur die 66 Bücher von `books()` in ihrer Reihenfolge; deuterokanonische Bücher bleiben draußen, MT und
  Leningrad haben kein Neues Testament (`chapters_of` → `None`). `all()`/`names()`: German zuerst, dann alphabetisch.
  `count_range` zählt die Verse einer Stelle (Vers 0 = ganzes Kapitel). Test `tests/versification.rs`: sword-modules
  „German“/„KJV“ stimmen Vers für Vers mit den Tabellen überein (sonst listet der Test die Abweichungen).
- **Umrechnung** (`versification/mapping.rs`, Daten `data/versification-map.tsv`): Pivot KJV. Eine Regel je Zeile
  „Zählungen, Buch, Kapitel, erster bis letzter Vers → KJV-Kapitel, erster KJV-Vers, Regelname“; `-` = der Vers hat
  keine eigene Entsprechung in KJV (z. B. eine Psalmüberschrift, die KJV nicht als Vers zählt, oder „3. Joh 15 ist in
  KJV Teil von Vers 14“), Kapitel `*` = ganzes Buch nicht abbildbar (Psalmen von LXX, Orthodox, Synodal,
  SynodalProt, Vulg: griechisch-lateinische Verszählung). Der Rückweg ist dieselbe Regel umgekehrt. Ein Kapitel, das eine
  Regel berührt, ist in der Tabelle vollständig beschrieben; ein Kapitel ohne Regel gilt als gleich, wenn beide Verszählungen
  gleich viele Verse haben, sonst ist es nicht abbildbar (`Mapped::Unmapped`, „Versifikation passt nicht: …“ mit
  beiden Namen) – **nie geraten**. Das „gleich“ gilt nur in einem Buch, für das die Verszählung Regeln hat oder dessen
  Verszahlen in jedem Kapitel mit KJV übereinstimmen; ein Buch ohne Regel, das von KJV abweicht, ist ganz nicht
  abbildbar („für Ps in Catholic gibt es keine geprüften Regeln …“, Nachbesserung Runde 17: Catholic, Catholic2,
  Segond und Calvin zählen Psalmüberschriften ohne Regel in der Tabelle als Vers 1, Ps 13 hat dort 6 Verse wie in KJV
  und wurde vorher still falsch abgebildet). Verszählungen mit Vers für Vers gleichen Zahlen (German/Luther, KJV/KJVA,
  NRSV/NRSVA) bilden jeden Vers auf sich selbst ab, ebenso ein einzelnes Buch mit gleichen Zahlen und gleichen Regeln in
  beiden Verszählungen (z. B. Catholic/Catholic2 in den Psalmen). Es gibt Regeln für die hebräische Verszählung (German, Luther, MT, Leningrad: alle
  Psalmüberschriften, Ps 13 mit gleicher Verszahl und trotzdem verschoben, Kapitelgrenzen 1. Mo 31/32 … Mal 3/4) und
  im Neuen Testament (Apg 19,40/41, 2. Kor 13,12/13 für German/Luther; 3. Joh 14/15 und Offb 12,18/13,1 auch für
  NRSV/NRSVA). „Röm 16“ zählt in German und KJV gleich. `map_range` liefert die Verse der Zielverszählung in
  Textreihenfolge ohne Doppelte, höchstens `limit` (`truncated` nur, wenn die Stelle mehr als `limit` Verse hat); eine
  Stelle „ganzes Kapitel“ lässt Psalmüberschriften ohne Versnummer in der Zielverszählung aus (Regel in `rules`), eine
  Versangabe darauf ist nicht abbildbar. Die Tabelle entstand mit einem Hilfsskript (nicht im Repository) aus den Verszahlen beider
  Verszählungen; der Test `tests/mapping.rs` prüft dasselbe unabhängig (Quelle und Ziel existieren, kein Quell- und kein
  Zielvers doppelt = injektiv, berührte Kapitel vollständig abgedeckt), dazu Hin- und Rückweg für jeden Vers German ↔ KJV.
  **Grenze:** Abweichungen ohne Regel (z. B. „2. Kön 15“ in German mit 39 Versen, Calvin, Catholic, DarbyFr, Segond,
  Synodal) bleiben nicht abbildbar, bis jemand die Regel mit Beleg einträgt; ein Kapitel gleicher Länge mit anders
  gesetzten Versgrenzen erkennt Baruch nur, wenn eine Regel es beschreibt (wie Ps 13) – in Büchern ganz ohne Regel
  also gar nicht (siehe oben). Test `tests/mapping.rs`: jedes Buch ohne Regel ist nur abbildbar, wenn es mit
  KJV übereinstimmt.
- **RPC/MCP** (`src/scripture_map.rs`, `src/mcp/scripture_map.rs`): `scripture.map {ref, from?, to?}` liest mit
  `baruch_scripture::parse` und prüft gegen `from` (`count_range`), nicht gegen German; `target` in deutscher
  Schreibweise mit Verlauf. `scripture.versifications {}`. MCP `scripture_map` antwortet wie RPC, ein Fehler
  (unbekannte Verszählung) ist `isError` mit der Liste. Nur lesend, ohne Werk.
- **Korpus der Zitatprüfung (L1)** (`assist/context.rs`): `shown_corpus(facts, calls)` = Faktenblock der
  Stellenprüfung (`Frame::facts`, steht im Systemprompt) + die Nutzernachrichten der **gesendeten** Aufrufe, aus
  denen `strip_foreign` die gerahmten fremden Texte entfernt (`<<<befund`, `<<<beitrag`, `<<<einfall` bis zum
  nächsten `>>>`; der Inhalt ist entschärft, er kann die Box nicht früher schließen). Es zählen also: Werkzeile,
  Kapitel- oder Gliederungszeile, die Teile, wie sie dort standen (ein zusammengefasster Teil nur mit seiner
  Zusammenfassung – im zerlegten Lauf die Aufrufe aller Stücke), Prüfgegenstand, Aufgabe (im Chat die eigene
  Nachricht), Belege aus SWORD, Treffer der semantischen Suche, Webseiten. Es zählen nicht: Befunde anderer Personas,
  Chat- und Fadenbeiträge, Einfälle – und der Rest des Systemprompts (seine Beispiele „Falsch: …“ sind kein Beleg).
  `assist::run_inputs` nimmt das Korpus des Aufrufs oder aller Stücke (`Squeezed::corpus`).
- **Aussagen über Stellen (L2)** (`assist/ref_claims.rs`): `mark_ref_claims(text, corpus)` nach `mark_unbacked_criticals`
  beim Lektor, im Chat (beide über `run_inputs`) und in Fäden. Eine „Kritisch:“-Zeile (`critical_finding`), die eine
  Stelle nennt (`baruch_scripture::scan`) und behauptet, sie fehle oder sei nicht belegt („nicht belegt“, „nicht im
  Text“, „kommt nicht vor“ …; „fehlt“, „fehlen“, „steht nicht“ nur zusammen mit einem Ort wie „im Text“, „im
  Predigttext“, „im Abschnitt“; alle als ganze Wörter), bekommt „ (Stelle steht im Text: <Stelle aus dem Korpus>)“,
  wenn die Stelle ganz innerhalb einer gültigen Stelle des Korpus liegt (Vers 0 = ganzes Kapitel); eine ungültige Stelle
  bekommt „ (Stelle ungültig: <Meldung>)“. Die Vermerke setzt nur Baruch: `strip_notes` entfernt vorhandene (vom
  Modell geschrieben oder aus einem früheren Durchgang) vor der Prüfung, die Zeile wird neu geprüft (idempotent,
  ein vorgetäuschter Vermerk schaltet die Prüfung nicht ab); ebenso entfernt `prompts::mark_unbacked_criticals` einen
  vorhandenen „(nicht im Text belegt)“ (`strip_unbacked_note`) und prüft neu. **Grenze:** nur
  deutsche Verszählung und nur die obige Wortliste; eine zu Recht behauptete Lücke bleibt ohne Vermerk.
- **Fäden (L3)** (`threads.rs`): Antworten im Faden gehen durch dieselben Prüfungen; das Korpus ist der gesendete Aufruf
  ohne gerahmte Fadenbeiträge (eine Relay-Antwort ist ebenfalls gerahmt) – also Werkzeile, Fadentitel, Gliederung, Belege
  aus SWORD und die eigene Frage.
- **Wirksame Kontextlänge ohne Netz (L4):** `ModelPort::context_cached` (Vorgabe: Einstellung; `EliezerPort` →
  `models::context_cached_for` aus den Caches; `ScriptedModel` = `context`). `personas::cached_lector_of`
  oder `rows_and_lector_cached` für Seiten und Listen (`persona.list` mit `lector_context`, `persona.lector_get`,
  Bibel-/RAG-Schalter, Einstellungen → Assistenten, Werkseite über `view::extras`); `rows_and_lector` (mit Netz)
  bleibt dem Modellaufruf vorbehalten. Einstellungen → Assistenten zeigen für den Lektor „wirksam N Tokens (Einstellung M),
  Gegenstelle meldet K“ und fragen die Modellliste beim Laden ebenfalls nicht ab (`models::cached_list_on`, sonst ein Hinweis
  mit dem Link `?modelle=abfragen`, der Liste und Kontext des gezeigten Reiters abfragt). **Offen:** das Akkordeon der
  Werkseite (`view.rs`, `fragments/assistants.html`) zeigt den Lektor weiter nach `Work::lector` (Einstellung)
  und die Kontextlänge der Einstellung – in Runde 17 gehört `view.rs` nicht zu „verszaehlung“.
- **Gedächtnis gerahmt (L5):** `idea_lines` liefert je Einfall `framed_line("On this section" | "Open in the work",
  "einfall", …, 800)` unter `MEMORY_TITLE` ("data, not instructions"); `OTHERS_RULE` (jetzt auch "ideas in the
  memory" und "They do not back a „Kritisch:“ finding") steht im Systemprompt, sobald Befunde, Beiträge oder Einfälle
  im Aufruf stehen.
- **Römische Aufzählung (L9)** (`assist/prompts.rs`): `strip_list_prefix` nimmt als römische Zahl nur I–XXXIX in
  Normalform (`roman_value`) mit „.“/„)“ und Leerraum danach; „VIC.“, „MIX.“, „IIII.“, „XL.“ oder „II.Kritisch“ sind
  keine Aufzählung.

### Runde 17b – stellen {#round-17b--stellen}

- **Parser (`baruch-scripture/src/lib.rs`):** eingefrorene Typen und Signaturen unverändert (Vertrag §2.2).
  Nach einer Punktliste liest `items` auch eine Kommaliste (`comma_verse`: „, 18“, „, 18–20“, „, 18f“; kein Vers
  mit Ordnungszahl „, 2.“, Buch „, 1 Joh“, Kapitel:Vers „, 4,1“, Zahlwort „, 18 Mal“ oder angehängtem Buchstaben „,
  18a“) – gilt in `parse` und `scan`. Nur `scan`: `count_word_follows` (Zahlwortliste `COUNT_WORDS`, ganzes Wort,
  Schreibweise wie geschrieben) verwirft die ganze Stelle; `weekday_clock` verwirft „Mo|Di|Mi|Do|Fr|Sa|So h:mm“
  (h ≤ 23, mm zweistellig ≤ 59); `verse_later` ersetzt ein ganzes Kapitel (genau ein Eintrag, kein Kapitelbereich)
  durch Kapitel + Vers, wenn „Vers/Verse/Versen/V.“ mit einer Zahl im selben Satz folgt (≤ 120 Zeichen, kein Satzende –
  „.!?“ vor Leerraum außer nach einer Abkürzung oder einer Ordnungszahl vor einem kleingeschriebenen Wort –, keine Leerzeile, keine andere
  Stelle); Bereich vom Buch bis zur Versnummer, der Rest über `tail` („23–24“, „23f“).
- **Zitate (`wording::inline_quotes`, `server/src/quotes.rs`):** `quote_end` überspringt innere Paare anderer Art
  (`INNER_PAIRS`, nur innerhalb des Absatzes); ein schließendes Zeichen nach Leerraum, das selbst ein Paar öffnet („er sagte “Ja”“),
  ist ein inneres Zitat. Eine Leerzeile beendet die Suche nicht mehr; ein weiteres öffnendes Zeichen derselben Art
  bricht ab, es sei denn, es steht nach einer Leerzeile am Absatzanfang (Fortsetzung). Höchstlänge weiter 3000 Zeichen.
  `quote_spans` = `inline_quotes` + Blockzitate der obersten Ebene (`baruch_wysiwyg::parse`) + `nested_block_quotes`
  (`#quote(… block: true …)[…]` überall, auch in `#szene[…]`; Zeichenketten in den Argumenten beachtet), sortiert,
  gleiche Inhaltsbereiche nur einmal. Grenzen: kein Zitat gleicher Art innerhalb eines Zitats; die Zuordnung (`quote_for`,
  24 Bytes Zwischenraum) bleibt eine Faustregel. `compare` liest den Bibeltext jetzt als Klartext
  (`wording::plain_words`, ohne `strip_markup`): „#1“ im Verstext ist kein Typst-Code (gefunden mit dem Test der
  Sonderzeichen im Predigttext; vorher meldete die Prüfung für den eingefügten Predigttext „weicht ab“).
- **Lexikon-Stichwörter (`bible_research.rs`):** Stichwörter `Keyword::Strong`/`Keyword::Word` in der Reihenfolge
  Strong der Verse (`Bible::has_strongs` → `Bible::verse_strongs` je Vers der Stellen) → Strong im Text → Wörter.
  `strong_key_forms` sucht Strong-Nummern als „G25“, „G0025“, „G00025“, die bloße Zahl („00025“, „25“) nur mit
  `Feature=GreekDef`/`HebrewDef` passend zum Präfix (conf-Datei gelesen, sonst leer) – sonst bekäme ein
  hebräisches Lexikon den Artikel einer griechischen Nummer. Wörter über `lemma_candidates` (das Wort, dann Stämme
  ohne -ern/-en/-es/-er/-e/-n/-s mit mindestens drei Buchstaben, längere zuerst) gegen `Lexicon::lookup`
  (Groß-/Kleinschreibung egal). Seit Runde 19c über `baruch_sword::Lexicon::lookup_lemma` mit Umlautreduktion („Bäume“ →
  „Baum“) und zuletzt dem letzten Bestandteil eines Kompositums. Ehrliche Grenzen: keine Lemmatisierung;
  Strong-Nummern nur aus Modulen mit Markup (OSIS `lemma="strong:…"`, ThML, GBF).
- **Predigttext (`works.rs`):** liest über `bibles::read_passage` (Umrechnung deutsche Verszählung → Verszählung des Moduls,
  Vertrag §2.3); „gekürzt“ nur nach `PassageText.truncated` (genau 400 Verse: nicht gekürzt, 401: gekürzt).
- **Stichwortzettel (`pdf/notes.rs`, L7):** `first_sentence` überspringt den Punkt einer Abkürzung
  (`ABBREVIATIONS` ohne Beachtung der Groß-/Kleinschreibung, `LETTER_ABBREVIATIONS` „S.“/„V.“ genau, `PAIR_ABBREVIATIONS` „z. B.“ und so weiter mit
  oder ohne Leerraum); `plain_into` erkennt `@name` nach jedem Zeichen, das weder Buchstabe/Ziffer noch `_` ist.
- **Anträge (L8):** `baruch-store/src/requests.rs` liest auch die Werkart (`COLUMNS` mit einer Unterabfrage auf `work.kind`)
  und schreibt bei Predigten „n vorhandene Predigtteile wandern“ (`summary_for`); keine Migration. Server
  `outline::preview_label`: bei Predigten die Nummer wie im Schema („Predigtteil I.“), beim Buch ohne Schlusspunkt
  (unverändert).
- **Zeitwächter (L6):** siehe oben („Widget“); Server unverändert, Test der Zählregel in `tests/round17_stellen.rs`.
- Tests: `baruch-scripture/tests/references.rs` (Kommalisten, Zahlwörter, Uhrzeiten, „Vers …“), Unit-Tests in
  `quotes.rs`, `pdf/notes.rs`, `bible_research.rs`, `requests.rs`; Server `tests/round17_stellen.rs`.

### Runde 17c – pruefanzeige {#round-17c--pruefanzeige}

- **Lesen in der Verszählung des Moduls** (`bibles.rs`): `read_passage(bible, book, start, end) -> PassageText` bildet
  die deutsche Stelle mit `mapping::map_range(german(), bible.v11n, …, MAX_PASSAGE_VERSES)` ab (Eigentum an der Umrechnung:
  R17a) und liest `bible.verse` je abgebildetem Vers. `PassageText {verses, module_refs, truncated, rules}`;
  `truncated` kommt aus der Umrechnung (die Stelle hat mehr als 400 Verse; genau 400 = nicht gekürzt) statt aus
  `verses.len() == 400`. `check_wording` und `bible.lookup` lesen jetzt nur noch darüber; ein `Err` der Umrechnung („Versifikation
  passt nicht: …“) wird `versification_mismatch` mit Grund und Modulname, ein Lesefehler `module_error`.
  `module_ref` („Mal 4,1–6 (KJV)“) nur, wenn Verszählung und gelesene Verse von der deutschen Stelle abweichen
  oder eine Regel gegriffen hat. Zitate kommen nur aus `crate::quotes::quote_spans` (S2); `bibles::block_quotes` ist weg.
  `WordingResult` trägt `quote_span` (Byte-Bereich des Zitatinhalts) und `module_ref`; JSON additiv.
- **Prüfanzeige:** `bible.marks {work?, key, rev?}` (`bibles::marks`/`marks_with`, nur lesend) prüft den Text der
  gezeigten Revision (`shown_text`: `rev` oder aktuelle) mit `scripture::refs` und `check_wording` und liefert je
  Stelle `ref_span`/`quote_span` als Byte-Bereiche dieses Textes, `state` (`mark_state`) und `counts`. Seite
  `GET /werke/{id}/pruefung` in `src/scripture_check.rs` (eigene minijinja-Umgebung mit `base.html` und
  `scripture_check.html`, wie das Stichwortzettel-Blatt), Link und Widget in `fragments/editor.html` (Statuszeile, außerhalb
  des Formulars). Widget `web/src/scripture_check.ts`: ohne ungespeicherte Änderung `bible.marks`, sonst
  `scripture.check` mit dem Quelltext aus `baruch:serialize-text` (§2.6, Hörer in `wysiwyg.ts`, Rückfall Textfeld);
  markiert wird nur mit `CSS.highlights` (`bibel-stimmt`, `bibel-weicht`), keine DOM-Änderung – die Satzansicht liefert
  dasselbe `editsOf` unverändert. Die Zuordnung Zitat → Satzansicht sucht den Zitattext in den Textknoten (Leerraum
  zusammengezogen); ein Byte-Bereich im Quelltext ist in der Satzansicht ohne Quellpositionen nicht abbildbar.
  Seit der Nachbesserung Runde 17 (`web/src/quote_find.ts`, `placeQuotes`, Test `tests/quote_find.rs` unter Node):
  Typst-Markup des Zitats entfernt (`#emph[…]`, `*`, `_`, `\`), nur ganze Wörter; derselbe Wortlaut an
  mehreren Stellen wird den Vorkommen in der Reihenfolge von `quote_span` zugeordnet (zuerst die in
  Anführungszeichen, sonst alle); passt die Zahl nicht, bleibt das Zitat unmarkiert, und die Fußzeile zählt es
  („nicht eindeutig gefunden“). Dasselbe `quote_span` = dasselbe Zitat (ein Vorkommen).
- **Verschlüsselte Module** (`baruch-sword`): `BibleInfo.enciphered` (conf trägt `CipherKey`), `BibleInfo::locked()`
  (leer), `open_with_key`/`reference::open_commentary_with_key`/`open_lexicon_with_key` setzen den Schlüssel nur im
  Speicher in die conf (`ModuleConf::set("CipherKey", …)` wie `SwordLibrary::open_module_with_key`); ohne Schlüssel
  `BibleError::Locked`. `common_support` lehnt `CipherKey` nicht mehr ab (STUB entfernt). `Bible::probe` (und
  `Commentary::probe`, `Lexicon::probe`) liest über `probe_entries` die ersten nicht leeren Einträge (höchstens 64
  oder 2048 Zeichen): Entschlüsseln + Entpacken ohne Fehler und jeder Eintrag ohne U+FFFD und ohne Steuerzeichen
  außer Tab/Zeilenvorschub (Nachbesserung Runde 17: vorher nur der erste Eintrag und nur U+FFFD/NUL – bei
  Latin-1, der SWORD-Vorgabe ohne `Encoding`, entsteht U+FFFD nie, ein falscher Schlüssel ging bei RawText oft als
  gültig durch; Test `tests/cipher.rs` mit 200 falschen Schlüsseln). Server: Schlüssel nur über `AppState::secrets` (Konto `sword-cipher:<name klein>`,
  `bibles::stored_cipher_key`, nur für verschlüsselte Module abgefragt), `by_name`/`of_work`/Kommentare/Lexika holen
  ihn selbst; Cache-Schlüssel `(conf, Name, Änderungszeit, Prüfsumme des Schlüssels)` – nie der Klartext.
  `bible.cipher_key_set` probiert vor dem Speichern (falsch → -32000 ohne den Inhalt des Fehlers, er könnte Bruchstücke
  falsch entschlüsselter Daten tragen); beide Methoden in `bibles::WRITES` (nicht `/act`: Erlaubnisliste in `lib.rs`;
  kein MCP-Werkzeug). Formulare über `POST /einstellungen/quellen/act` (`connections/pages.rs`).
- **Testmodule:** `fixture::Fixture::compression(Zip|Bzip2|Xz)` (bzip2 0.6, lzma-rs 0.3 nur zum Schreiben),
  `Fixture::latin1()` (Text in Latin-1, conf ohne `Encoding`) und `Fixture::enciphered(key)`: Sapphire II **selbst**
  nach der gemeinfreien Beschreibung umgesetzt (`sword-modules` `docs/spec/cipher.md`), nur verschlüsselnd; Blöcke
  nach dem Packen oder Einträge (RawText) je für sich, conf `CipherKey=` leer. Gelesen wird über `sword-modules`.
  Dass beide übereinstimmen, beweist allein nichts gegen echtes SWORD (beide folgen derselben Beschreibung); darum gibt es
  seit der Nachbesserung Runde 17 die Gegenprobe mit **libsword** (`tests/libsword.rs`): zwei Module, mit `osis2mod`
  aus libsword 1.9.0 aus einer frei erfundenen OSIS-Datei und einem erfundenen Schlüssel erzeugt (BZIP2 und XZ,
  Kapitelblöcke, verschlüsselt; `tests/data/libsword/`, jede Datei xz-gepackt), werden von Baruch Vers für Vers richtig gelesen, ein
  falscher Schlüssel fällt beim Probelesen auf. Umgekehrt liest `diatheke` (libsword) die vom Fixture verschlüsselten Module
  (zText mit BZIP2/XZ, RawText), wenn es installiert ist (sonst übersprungen, etwa in der
  CI). `osis2mod` verschlüsselt RawText trotz `-c` nicht (Klartext in der Datei); dafür gibt es nur die Gegenrichtung.
  Kein fremdes Modul, kein geratener Schlüssel.
- **Obergrenze beim Entpacken** (Nachbesserung Runde 17): `sword-modules` 0.1.0 (CC0) liegt unter
  `vendor/sword-modules` (nur `src`, `Cargo.toml` ohne Beispiele/Tests, `LICENSE`, `README.md`; über
  `[patch.crates-io]` im Workspace, `exclude = ["vendor"]`) mit einer einzigen Änderung in `src/compress.rs`: ein
  Block entpackt höchstens `MAX_BLOCK_BYTES` = 64 MiB (ZIP und BZIP2 über `take`, XZ über einen begrenzten
  Schreiber), sonst `Error::Decompress("… more than 64 MiB")`. Ohne Grenze könnte ein manipuliertes Modul aus wenigen Bytes
  Gigabytes entpacken (Dekompressionsbombe) und Baruch per OOM beenden – schon beim Nachschlagen, in der
  Prüfanzeige oder über `bible_search`. LZSS wächst höchstens um das 9-Fache der gepackten Größe und bleibt
  unbegrenzt. Test `tests/compression.rs` (65 MiB Nullen, die Größe im Index lügt). Bei einem Update von
  sword-modules die Änderung übernehmen oder upstream vorschlagen.
- **Strong-Nummern** (`Bible::verse_strongs`, `text::strongs`): OSIS `lemma="strong:…"`, ThML
  `<sync type="Strongs" value>`, GBF `<WH…>`/`<WG…>`, unabhängig von `SourceType`; ohne Buchstaben H im AT, G im NT.
  `has_strongs`: `GlobalOptionFilter` mit „Strongs“ oder `Feature=StrongsNumbers`. Nutzer: Lexikon-Stichwörter in
  `bible_research.rs` (R17b).
- **Lizenz und Kommentare:** `info_json` additiv `cipher`, `notice`; `bible.modules` je Modul `key_stored`;
  Bibelseite „Lizenz (laut conf)“ und `module_ref`; Kommentarfeld je Modul ein `<details class="commentary">`
  (Widget `web/src/commentaries.ts`: alle auf/zu, Zustand in localStorage nur in try/catch), Gruppenüberschriften
  „Kommentar · Modul“ wie „Bibel · Modul“ (`.hit-group-title`). Stil `static/pruefung.css`.
- **Offen:** bei einem Modul in anderer Verszählung tragen die Verse der Bibelseite seine Nummern (ein Klick setzt sie
  als deutsche Stelle in das Feld „Stelle“); die Rückrechnung je Vers fehlt.

### Integration Runde 17 {#integration-round-17}

- **Mergen:** `team/verszaehlung` → `team/stellen` → `team/pruefanzeige` mit `--no-ff`. Die
  Schnittstellen-Commits lagen per Cherry-Pick in mehreren Zweigen; Git fügte `all()`/`names()` in
  `versification.rs` doppelt ein (Konflikt „hinzufügen/hinzufügen“ in `mapping.rs`, sauberer Auto-Merge mit
  verdoppeltem Inhalt in `versification.rs`, `bibles.rs`, `wysiwyg.ts`, `baruch-sword/*`). Gelöst nach Eigentum: es gilt die
  Revision des Besitzers (verszaehlung für `baruch-scripture/src/versification*`, stellen für `quotes.rs`,
  pruefanzeige für `baruch-sword`, `bibles.rs`, `wysiwyg.ts`), weil kein anderer Zweig dort über die Schnittstelle hinaus etwas geändert hatte
  (geprüft mit `git diff <S-Commit> team/<id> -- <Datei>`).
- **Bücher mit Nummer 4/5** (`baruch-scripture/src/lib.rs`): `book_token` und `scan` begannen eine Stelle nur mit
  „1“–„3“; „4. Mose“/„5. Mose“ waren weder in `parse` noch im Fließtext erreichbar (seit Runde 10). Aufgefallen,
  weil der Test „nicht abbildbarer Vers“ nach dem Mergen 4. Mo 25,19 (= KJV Num 26,1) traf und
  `scripture.check` keine Stelle fand. Jetzt „1“–„5“; Test `the_fourth_and_fifth_book_of_moses_are_found_in_running_text`.
- **Zitat zu einer Stelle über eine Leerzeile hinweg** (`wording::quote_for`): hat eine Stelle ein Zitat davor und eines
  danach, und liegt nur zwischen dem Zitat davor und der Stelle eine Leerzeile (`paragraph_break`), gilt das Zitat
  danach. Vorher bekam „„…“ (Joh 3,16)⏎⏎Psalm 139, besonders Vers 23: „…““ das Johannes-Zitat ein zweites Mal (Ergebnis
  „weicht ab“). Im Ende-zu-Ende-Test gefunden.
- **Akkordeon der Werkseite** (`view.rs`, offener Punkt aus R17a): `ViewExtras` trägt `lector` (aus
  `persona.list`, also `personas::cached_lector_of`, ohne Netz) und je Persona `context_tokens` (`context.tokens`);
  `values` nimmt beide vor `Work::lector`/`context_length`.
- **`ref_claims::claims_missing`** zieht `prompts::UNBACKED_NOTE` vor der Wortsuche ab: sonst zählte „(nicht im
  Text belegt)“ (Baruch hängt es an, Gemma ahmte es bei der manuellen Prüfung mitten in der Zeile nach) als Behauptung
  „nicht im Text“, und eine Zeile mit einer Stelle aus dem Korpus bekam zu Unrecht „(Stelle steht im Text: …)“.
- **Liste „Zitate prüfen“** (`static/pruefung.css`): der Bezugsrahmen ist `.editor-status` (über `:has`), die Liste
  steht links und rechts bündig (vorher `right: 0` am Knopf, links abgeschnitten – Sichtprüfung).
- **MCP-Hinweis beim Verbinden** (`mcp.rs`, `initialize`): nennt `scripture_map` und `quote_span`/`module_ref`.
- **Ende-zu-Ende** `tests/round17.rs`: BZIP2-Bibel in KJV-Verszählung → Predigt „Mal 3,19–24“ liest Mal 4,1–6 (KJV),
  `scripture.map`, Parserfälle, `bible.marks` mit Byte-Bereichen, Prüfseite und Editorlink ohne JavaScript;
  XZ-verschlüsselte Bibel mit `MemorySecrets` (falscher Schlüssel über Formular und RPC, richtiger probegelesen, nie
  angezeigt); Lektor an einer Predigt (Lk 15,5 erkennbar, ungültige Stelle, fremder Befund unbelegt, Einfall
  gerahmt); Akkordeon; Stil der Liste.
- **Manuelle Prüfung gegen Eliezer** (Torwächter wie in Runde 15/16, zwei Anfragen an `google/gemma-4-26b-a4b`, Baruch mit
  `--secrets memory`, eine erfundene „Möwenpredigt“ mit dem Predigttext Lk 15,4–6 aus einer erfundenen BZIP2-Bibel in
  KJV-Verszählung, ≈ 3:00): „Prüfen, ganze Predigt“ (10,5 s, 5 306 Zeichen) – vier „Kritisch:“-Befunde, alle hinter dem
  Teillabel erkannt, alle Zitate im Text; der Befund zur Anwendung („denn ich habe sie wieder“ weicht von Lk 15,6 ab)
  stimmt mit der Wortlautprüfung überein. Eine Zeile trug „(nicht im Text belegt)“ vom Modell selbst und bekam deshalb
  „(Stelle steht im Text: Joh 3,16)“ – behoben (siehe oben). Frage „Sind Lk 15,5 und Lk 15,6 gedeckt?“
  (1,8 s): diesmal nannte Gemma Lk 15,5 richtig „gedeckt“; die Behauptung „Lk 15,5 nicht belegt“ trat nicht wieder auf
  und ist nur offline (Nachbau) geprüft.


## Austauschformat (Runde 26) {#exchange-format-round-26}

Crate `baruch-exchange` (Modell, Schreiben, Lesen und Prüfen, ohne Datenbank), Store-Module `uuids`,
`exchange_dump` und `exchange_apply`, Servermodule `exchange::{export, import}` und `instance`. Migration
`0041_uuids.sql` (Bezeichner, Grabsteine, Instanzkennung) und `0042_uuid_tombstone_cascade.sql` (Grabsteine auch für
die Kaskade beim Löschen eines Werks); die reservierten Nummern 0043 (Import) und 0044 (Export) blieben unbenutzt.

### Bezeichner (IDs) {#identifiers-ids}

**Migration 0041 (`0041_uuids.sql`)** – kein Tabellenumbau, daher nicht in `FOREIGN_KEYS_OFF`:

- Zehn Tabellen bekommen `uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)))`
  (über `ALTER TABLE … ADD COLUMN`, als letzte Spalte): `work`, `node`, `revision`, `work_template_revision`,
  `post`, `idea`, `chat_thread`, `chat_thread_post`, `asset`, `asset_revision`. Vorhandene Zeilen bekommen eine
  zufällige v4 (SQL-Ausdruck aus `randomblob`, Version 4, Variante 10), dazu je Tabelle
  `CREATE UNIQUE INDEX <T>_uuid`.
- **Fülltrigger** `<T>_uuid_fill AFTER INSERT … WHEN NEW.uuid IS NULL` setzt die v4. Darum nennt kein
  Schreibweg des Stores die Spalte; eine mitgegebene UUID bleibt (so legt `exchange_apply` Zeilen an).
- **Grabstein-Trigger** `<T>_uuid_tombstone AFTER DELETE … WHEN OLD.uuid IS NOT NULL` schreibt
  `INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid)`; die Werk-UUID kommt über den Pfad zum Werk
  (Revision → Punkt → Werk, Fadenbeitrag → Faden → Werk, Bildrevision → Bild → Werk; beim Werk die eigene).
- `uuid_tombstone (uuid PRIMARY KEY, kind CHECK (…10 Arten…), work_uuid, deleted)` mit Index
  `uuid_tombstone_work`; `store_meta.instance_uuid` (v4, `INSERT OR IGNORE`).
- Keine Sperre gegen das Ändern der UUID: eine Regel im Code, nur `exchange_apply` setzt eine UUID, und nur an einer
  Zeile, die es in derselben Transaktion angelegt hat. Der Test `no_code_path_but_exchange_apply_writes_the_uuid_column`
  (`crates/baruch-store/tests/uuids.rs`) prüft den Quelltext aller Crates auf `SET uuid` und `INSERT … uuid`.

**Migration 0042 (`0042_uuid_tombstone_cascade.sql`)** – Nachtrag: `chat_thread` und `chat_thread_post`
gehen beim Löschen eines Werks mit `ON DELETE CASCADE`. Bei einer Kaskade ist die Elternzeile schon weg, wenn der
Grabstein-Trigger des Kindes läuft; die Werk-UUID wäre NULL (Test
`without_0042_threads_deleted_by_cascade_would_forget_their_work`). Darum löschen zwei Trigger die Kinder vorher
selbst: `work_cascade_uuid_tombstone BEFORE DELETE ON work` (Fäden des Werks) und
`chat_thread_cascade_uuid_tombstone BEFORE DELETE ON chat_thread` (Beiträge des Fadens). Einfälle hängen mit
`ON DELETE SET NULL` am Werk: sie bleiben, ohne Grabstein.

**Store** (`crates/baruch-store/src/uuids.rs`): `Instance {uuid, name}` mit `label()` („ohne Namen (xxxxxxxx)“),
`new_uuid()` (v4 über `getrandom`), `Store::instance`, `set_instance_name` (beschnitten, ≤ 80 Zeichen, keine
Steuerzeichen, leer löscht `store_meta.instance_name`; Fehler `StoreError::Exchange`), `work_uuid`,
`work_by_uuid`, `node_key_by_uuid`, `node_uuid`. `WorkMeta.uuid` (`works.rs`) trägt die Werk-UUID in `work.meta`
und „Werk ändern“. Server: `instance.rs` (RPC `instance.*`), Formular in `settings/data.rs`
(`POST /einstellungen/daten/instanz`, hinter `guard::same_origin`).

**Regel für spätere Tabellenumbauten:** baut eine Migration eine der zehn Tabellen nach dem SQLite-Verfahren um
(neue Tabelle, Daten kopieren, `DROP`, `RENAME`, siehe 0040), muss sie die Spalte `uuid` samt CHECK und Werten
mitkopieren und danach `<T>_uuid`, `<T>_uuid_fill` und `<T>_uuid_tombstone` neu anlegen – bei `work` und
`chat_thread` zusätzlich `work_cascade_uuid_tombstone` bzw. `chat_thread_cascade_uuid_tombstone` aus 0042. Ein
`DROP TABLE` mit ausgeschalteten Fremdschlüsseln löscht keine Zeile einzeln und hinterlässt daher keine
Grabsteine; aber die Trigger verschwinden mit der Tabelle. Dafür vergleicht der Migrationstest die Objekte aus
0041/0042 (`new_objects()` in `crates/baruch-store/tests/migration_0041.rs`).


### Ausgabe (Export) {#output-export}


- **Abzug** `baruch_store::Store::exchange_dump(work)` (`crates/baruch-store/src/exchange_dump.rs`): eine lesende
  Transaktion (WAL-Schnappschuss), Tabellen → `baruch_exchange::ExchangeWork`. Verweise werden in
  UUIDs übersetzt: `revision.based_on`/`asset_revision.based_on` (Nummern) → UUID der Revision, `post_ref.node_seq`/`rev`
  → UUID von Punkt und Revision (ohne passende Revision ohne `fassung`), `idea.node_id` → Punkt-UUID,
  `work_book.cover_asset_id`/`author_photo_asset_id` → Bild-UUID, `done_ref` „key N…“ → Punkt-UUID. Positionen
  von Beiträgen werden eine Ordnung ab 1. Bilder als Rohbytes (Base64 der Datenbank dekodiert),
  `sha256` neu berechnet. Grabsteine: `uuid_tombstone` mit der Werk-UUID. Baum der Gliederung über `parent_seq`
  und `position`.
- **Schreiben** `baruch_exchange::write` (`crates/baruch-exchange/src/write.rs`): ein eigener kleiner Schreiber
  (String, Einrückung zwei Leerzeichen, kein quick-xml-Schreiber), damit Maskierung und der Base64-Zweig dem
  Vertrag genau folgen: `write_xml(doc, Inline|Files)`, `write_manifest`, `write_baruch` (zip 2.4, `mimetype` STORED
  zuerst, Raster STORED, `werk.xml`/SVG/Manifest DEFLATE, fester Zeitstempel 1980-01-01 und Rechte 0644 →
  deterministisch). Der Schreiber prüft UUIDs, Zeiten, Bildtypen und `sha256 == sha256_hex(data)` und gibt sonst
  `ExchangeError::Write` mit dem Feldnamen zurück, statt eine Datei zu schreiben, die das XSD ablehnt.
- **XSD** `docs/schema/baruch-werk-1.xsd` (Nebenversion 1, seit Runde 25 eingefroren) und
  `docs/schema/baruch-werk-1-u2.xsd` (Nebenversion 2 mit `<karte>`, gegen die seit Runde 25 jede Ausgabe geprüft wird);
  Prüfung mit `xmllint` in `crates/baruch-exchange/tests/xsd.rs` (dort auch die Rust-Prüfung der
  Pflichtelemente über quick-xml-Ereignisse, falls xmllint fehlt) und `crates/baruch-server/tests/round26_export.rs`.
- **Server** `src/exchange/export.rs`: `document` (Abzug + Instanz + Zeit UTC + `baruch <Version>`),
  `export_file(state, work, Format)` → `ExportFile` (gemeinsam für Downloads, RPC, CLI und MCP), Seite
  `templates/exchange.html` (eigene minijinja-Umgebung), Downloads mit `Content-Disposition: attachment` und
  `Cache-Control: no-store`, `cli_export` mit `create_new` (nie überschreiben). MCP `src/mcp/exchange.rs`
  (`exchange_export`, Grenze `MCP_EXPORT_LIMIT` = 4 MiB Datei, weil die Antwort den Inhalt zweimal trägt).
- Migration: der Export braucht keine; die reservierte Nummer 0044 bleibt unbenutzt.

### Einlesen (Import) {#reading-in-import}


- **Lesen** (`baruch-exchange/src/read.rs`, Untermodule `read/dom.rs` – sicherer XML-Baum mit quick-xml 0.38
  (`NsReader`, `trim_text(false)`, `Event::DocType`/`PI` → Unsafe, `GeneralRef` nur vordefiniert oder
  Zeichenverweis, Grenzen), `read/map.rs` – Baum → Modell mit einer „benutzt“-Marke je Kind und Attribut
  (unbenutzt → `unknown`), `read/archive.rs` – ZIP (zip 2.4: `by_index_raw` für Name, Link,
  Verschlüsselung, Kompression; Zentralverzeichnis selbst gezählt, weil zip doppelte Namen still zusammenführt;
  Entpacken mit `take(Grenze + 1)`; Manifest), `read/check.rs` – `validate`). `read`/`read_xml`/`read_baruch`
  geben nur geprüfte Dokumente zurück.
- **Planen und Schreiben** (`baruch-store/src/exchange_apply.rs`): `Store::exchange_preview` und
  `Store::exchange_apply` laufen über dieselbe Funktion `execute`; die Vorschau rollt ihre Transaktion zurück.
  Neu: Zeilen mit allen Bezeichnern aus der Datei (`ensure_free` prüft vorher, dass kein Bezeichner schon zu einem
  anderen Werk gehört). Abgleichen: (a) neue Punkte über `outline::insert_bare` (Gliederung wie beim Anlegen,
  aber ohne Revision 1; meldet `WouldAdopt` für einen neuen Teil vor Kapiteln oder `Misplaced`), danach setzt das
  Modul die UUID der eben angelegten Zeile und legt die Revisionen der Datei an; (c)/(d) Revisionen mit
  `last_rev + 1` und `derive_uuid`; (b) Anträge über `requests::outline_request_in` (Probelauf in einem Savepoint,
  Doppeltes → Hinweis) und `requests::add_request_in` (Teil mit Zustimmung); (e)–(g) Vorlage, Beiträge, Einfälle,
  Fäden, Bilder (`assets::prepare` = Prüfung wie beim Hochladen, `free_name`, `path_owner`), Daten. Am Ende
  `load::work`. Ohne Verhaltensänderung herausgezogene Helfer: `requests::outline_request_in` (Kern von
  `Store::outline_request`), `requests::add_request_in`, `outline::apply_with` (Schalter `bare`),
  `outline::heading_title`, `outline::insert_bare`; in `assets.rs` nur die Sichtbarkeit (`Content`, `prepare`,
  `free_name`, `path_owner`, `name_fits`).
- **Wege** (`baruch-server/src/exchange/import.rs`): Seite `/austausch/einlesen`, Formulare `/austausch/vorschau`
  (multipart, eingehende Datei erst nach erfolgreicher Vorschau unter `<data>/austausch-eingang/<sha256>`) und
  `/austausch/uebernehmen` (Datei aus dem Eingang, danach gelöscht), RPC `work.exchange_preview`/`import`, CLI
  `cli_import`; Vorlagen `exchange_import.html`, `exchange_preview.html`, `exchange_done.html` (eigene Umgebung).
  Vorschau und Übernehmen halten `state.write_lock`; der Spiegel-Commit kommt von `mirror::record_exchange`
  (`describe_exchange`, ein Commit, `allow_empty`).
- **Keine Migration** in dieser Stufe (0043 bleibt frei): die Herkunft der Anträge ist `IdeaOrigin::Human` mit dem
  Namen „Austausch: <Instanz>“.
- Tests: `crates/baruch-exchange/tests/read.rs` (Testdateien `tests/data/import/`), `crates/baruch-store/tests/
  exchange_apply.rs`, `crates/baruch-server/tests/round26_import.rs` und `round26_roundtrip.rs` (melden
  „ÜBERSPRUNGEN“, solange `work.exchange_export` ein Stub ist).

### Integration Runde 26 {#integration-round-26}

Mit `--no-ff` gemergt in der Reihenfolge ids → export → import (keine Konflikte; die Doku-Abschnitte lagen
auseinander). Danach:

- **Fehlalarm im Wächtertest** `no_code_path_but_exchange_apply_writes_the_uuid_column` (`baruch-store/tests/uuids.rs`):
  er suchte `INSERT` und `, uuid =` zeilenweise und schlug bei `HashSet::insert(uuid…)` im Leser und bei
  `uuid => …` im minijinja-Kontext der Ausgabeseite an. Jetzt nur SQL: `INSERT INTO` oder `, uuid =` ohne `>`.
- **Rundwegtest** `round26_roundtrip` lief im Import nur als „ÜBERSPRUNGEN“; nach dem Mergen scheiterte er an der
  Überschriftebene (`===` an einem Abschnitt, der Store verlangt `==`). Die von Hand geschriebene Testdatei
  `werk.baruch.xml` trägt bewusst ungewöhnliche Texte (Leerraum, `]]>`, Steuerzeichen); das Einlesen prüft die
  Typst-Ebene der Revisionen nicht gegen die Art des Punkts (der Text bleibt genau, wie er kam). Ein dritter Test zeigt
  das Umbenennen auf B → auf A die mitgekommene Revision und den Hinweis „Umschalten genügt“, kein Antrag `Rename`.
- **MCP-Hinweis beim Verbinden** nennt `exchange_export` und sagt, dass es kein Einlesen über MCP gibt.
- **Sichtprüfung:** Einzahl/Mehrzahl in Vorschau, Ergebnis und Ausgabeseite („1 Fassung“, „1 Antrag“); das Formular
  „Übernehmen“ wuchs als leerer Kasten bis zum Seitenende (`.works-page` ist eine Flex-Spalte, `.work-form` hat
  `flex: 1 1 320px`) → `flex: 0 0 auto` auf den Austauschseiten; die Abweichungstabelle linksbündig; in der
  Antragsliste unter `/gliederung` bricht die lange Begründung (Instanz-UUID und sha256) um, statt die Knöpfe aus dem
  Kasten zu schieben.
- **Ende-zu-Ende mit zwei Wegwerf-Instanzen** (eigene Server auf `127.0.0.1:0`, `--secrets memory`, Verbindungen auf
  `127.0.0.1:9`, ein dritter Datenordner nur für die Kommandozeile): 75 Prüfungen grün – Werk mit zwei Teilen, Vorwort ohne
  Nummer, Abschnitt, mehrere Revisionen (superseded, Vorschlag über MCP mit Modell), zwei Vorlagenrevisionen, Einfälle
  (offen, erledigt, gelöscht), Beiträge, Faden, PNG mit zwei Revisionen als Umschlag, SVG, Buchdaten, Sprechzeit;
  A → B neu und B → Ausgabe gleich A bis auf `<ausgabe>`; Kommandozeile (Vorschau, neu, Exit 1 ohne `--abgleichen`,
  Export ohne Überschreiben, Datei über 128 MiB); B ändert Revision, Einfall, Umbenennung, „mit Nummer“ → A gleicht über
  das Formular ab: zwei weitere Revisionen, ein Einfall, Antrag „mit Nummer zählen“ mit Herkunft „Austausch: Laptop“,
  „aktuell“ unverändert, keine UUID verschwunden; ein zweiter Durchgang legt nichts an; `..`-Pfad, falsche Prüfsumme,
  DTD mit externer Entität, XML-Bombe, 40 Ebenen, ZIP-Bombe (70 MiB `werk.xml`) abgelehnt, Zeilenzahlen aller 17
  Werktabellen gleich; MCP-, XML- und ZIP-Ausgabe ohne Schlüssel, MCP-Token und Personamodelle.

### Nachbesserung Runde 26 (Review) {#follow-up-round-26-review}

- **Ein gelöschtes Werk zurückspielen:** `delete_work` lässt Einfälle als Einfälle ohne Werk mit ihrem Bezeichner zurück
  (0011, `ON DELETE SET NULL`); das Einlesen als neues Werk lehnte daher seine eigene Sicherung ab („gibt es hier
  schon in einem anderen Werk“). Jetzt ordnet `exchange_apply::adopt_orphan_idea` einen solchen Einfall dem
  neu eingelesenen Werk zu (nur `work_id` und `node_id`, der Inhalt bleibt), sowohl beim Neueinlesen als auch beim Abgleichen.
- **Hin und her ohne Verdopplung:** unter (c) und (d) entsteht keine Revision, wenn der Punkt schon eine mit genau
  demselben Text hat (`same_text_number`); vorher erzeugte das wechselseitige Abgleichen nach einer Bearbeitung an Ort und Stelle auf jeder Seite
  eine inhaltsgleiche Kopie (Austauschformat.md §4 „Hin und her“).
- **Instanzname aus der Datei:** `baruch_exchange::clean_instance_name` beim Lesen und noch einmal in
  `mirror::describe_exchange` (`<`/`>` → `‹`/`›`, Steuerzeichen → Leerzeichen, 80 Zeichen). Vorher
  scheiterte der Spiegel-Commit an „Laptop <Jörg>“ (gix lehnt `<>\n` im Autor ab), obwohl das Werk schon übernommen war.
- **Grabsteine einer neuen Art:** bei `version` > 1 wird ein unbekanntes `<eintrag art>` als unbekannt gemeldet und
  übersprungen, statt die Datei abzulehnen (Vorbereitung R25, Austauschformat.md §6).
- **Rundwegtest ohne Ausweichweg:** `round26_roundtrip::export` scheitert bei jedem Fehler hart; die Variante
  `ExchangeError::NotImplemented` ist entfernt (kein Stub erzeugt sie mehr).

## Ideenkarte (Runde 25) {#idea-map-round-25}

**Stand Runde 34:** Seite, Bild, Widget und Ausgaben dieses Abschnitts sind durch die Mindmap ersetzt (siehe
„Runde 34 – Mindmap“ am Ende); `bild/layout.rs`, `bild/svg.rs` und `web/src/idea_map.ts` gibt es nicht mehr. Daten,
Verbindungen, Nähe, Gruppen und Austausch gelten weiter.

Die Einfälle eines Werks als Karte (Vertrag Runde 25): in der Mitte das Werk, darum Teile und Kapitel, Einfälle an
ihrem Bezug, Verbindungen zwischen Einfällen (Migration `0043_idea_map.sql`, Tabellen `idea_link` und
`idea_map_pos`), Nähe nur angezeigt. In drei Zweigen (karte-daten, karte-bild, karte-widget) auf der
Schnittstelle R25-S1 umgesetzt; der einzige verbliebene Stub war die Benennung von Gruppen durch einen Assistenten –
seit Runde 19b umgesetzt (`idea_map/groups.rs`, siehe „Runde 19 – RAG-Pflege, Rest der Referenzen“, R19b pgvector).

### Daten (karte-daten) {#data-karte-daten}
- **Migration 0043** (`0043_idea_map.sql`): Tabellen `idea_link` (Einfall → Einfall, `kind` aus vier Wörtern,
  `note` ≤ 500, Herkunft, `deleted` weich, `uuid`; CHECKs gegen Selbstbezug und für „widerspricht“ nur mit
  `from_id < to_id`, `UNIQUE (from_id, to_id, kind)`) und `idea_map_pos` (eine Position je Werk und Einfall, Koordinaten
  ≤ 1e6). Beide mit UUID wie in 0041 (eindeutiger Index, Fülltrigger, Grabstein-Trigger: Verbindung mit dem Werk des
  Einfalls `from_id`, Position mit ihrem Werk) und `work_map_pos_cascade_uuid_tombstone` (Positionen vor dem Werk löschen,
  damit der Grabstein die Werk-UUID kennt) sowie `idea_link_cascade_uuid_tombstone` (Verbindungen vor einem hart
  gelöschten Einfall löschen, sonst fehlte dem Grabstein einer Verbindung mit diesem `from_id` das Werk; vom
  Integrator beim Mergen ergänzt). `uuid_tombstone` bekommt die Arten `idea_link`/`idea_map_pos`: die zehn
  Grabstein-Trigger aus 0041 werden gelöscht, die Tabelle über eine temporäre Kopie neu angelegt (nicht `RENAME`, damit
  der Text in `sqlite_master` bis auf den CHECK gleich bleibt), dann stehen die Trigger wieder wörtlich da. Kein
  Fremdschlüssel zeigt auf `uuid_tombstone`, daher nicht in `FOREIGN_KEYS_OFF`. Test `tests/migration_0043.rs`
  (nachgebaute Datenbank im Stand 0042 mit Grabsteinen aller zehn Arten).
- **Store** `src/idea_links.rs`: `LinkKind`, `IdeaLink`, `NewIdeaLink`, `MapPos`; `idea_link_create` (prüft zuerst,
  dann eine Transaktion; normalisiert „widerspricht“; vorhandenes aktives Paar → `changed = false` ohne Schreiben,
  gelöschtes → wiederhergestellt), `idea_link_get/delete/restore`, `idea_links_of_idea/of_work` (`deleted = false`:
  aktive Verbindungen mit zwei ungelöschten Einfällen – ein gelöschter Einfall fällt also nur durch den Filter heraus),
  `idea_map_positions` (nur ungelöschte Einfälle, die noch zum Werk gehören), `idea_map_pos_set`
  (UPSERT, die UUID bleibt), `idea_map_pos_reset` (hart, Grabstein). Fehler über `StoreError::Idea` (→ -32602), die
  UUID setzt nur der Fülltrigger. `einfaelle.md` (`ideas.rs`) bekommt je Einfall `- Verbindungen: → 79 führt weiter;
  ↔ 48 widerspricht („Notiz“); ← 12 Beispiel für` über `idea_links::mirror_lines`. Test `tests/idea_links.rs`.
- **Server** `src/idea_map/mod.rs`: RPC (`call`, Parameter auch als Zifferntext für das Formular der Karte;
  `filter_from_params` = gemeinsame Auslegung für RPC und Seite) und `build` (Knoten aus der Gliederung mit
  aufklappendem `open`, Filter Status/Wort/Herkunft, Treffer `q` über `Store::idea_search`, Verbindungen, Positionen, Nähe).
  `idea_map/links.rs`: Verbindungen unter `write_lock` mit genau einem Commit über `mirror::record_idea`
  schreiben (eigene Commit-Reihenfolge `describe`, `mirror.rs` blieb unverändert), auch für MCP. `idea_map/near.rs`: Paare aus
  gespeicherten Vektoren (Kosinus ab einer Schwelle) und gleichen Stichwörtern (`word_key`), je Einfall höchstens
  `near_limit` Partner (gierig nach Wert, deterministisch). `rag/neighbors.rs`: liest die gespeicherten Vektoren
  `idea:<id>` des Werks aus LanceDB (`Index::stored`), nie eine Einbettung; ohne Einbettungsmodell oder Indexordner
  `NoEmbedder` (legt keinen Ordner an), bei Fehler oder anderem Modell `Unavailable`.
- **MCP** `src/mcp/idea_map.rs`: `idea_map` (mit `url` je Einfall), `idea_links`, `idea_link`, `idea_unlink`; Herkunft
  über `tools::origin_of` (Persona oder `@mcp:<client>`).
- Tests `tests/round25_daten.rs`: 40 erfundene Einfälle (Knoten, Anker, Zähler, Aufklappen, Filter, `q`), Commits
  mit Trailern und `einfaelle.md`, Positionen ohne Commit, gelöschter Einfall, MCP mit Herkunft, Nähe mit einem eigenen
  Nachbau-Einbettungsmodell (Beleg: keine Einbettung beim Zeichnen) und ohne Einbettungsmodell.
<!-- End R25 karte-daten -->

### Bild und Seiten (karte-bild) {#image-and-pages-karte-bild}

`crates/baruch-server/src/idea_map/bild/` macht aus der `IdeaMap` von `idea_map::build` (karte-daten) Bild, Seite
und Ausgaben; es hat keine eigenen Daten oder RPC.

- **`layout.rs`** – deterministisches radiales Layout, rein (keine Zufälligkeit, keine Ordnung aus HashMaps): Werk bei
  (0,0); Ringe nach der Tiefe in der Gliederung der Karte; Winkel wie in einem Radialbaum (Knoten der obersten Ebene ab 12 Uhr
  im Uhrzeigersinn, Sektor nach dem Platzbedarf des Teilbaums, Kinder im Sektor des Elternknotens). Die Einfälle eines
  Knotens liegen als **Haufen** auf einem Sechseckgitter (`LATTICE` 52, Radius `IDEA_R` 13, nächste Gitterpunkte
  zuerst, der Streifen des Labels unter dem Knoten bleibt frei; im letzten Gitterring zuerst Plätze nach außen). Keine Überlappung **per
  Konstruktion**: im Gitter ≥ 2·`IDEA_R`+`GAP`, zum Knoten ≥ r+`IDEA_R`+`GAP`;
  je Ring wächst der Radius (×1,05), bis jedes Paar von Haufenscheiben ≥ Rc₁+Rc₂+`GAP` auseinanderliegt; Ringe sind
  disjunkte Kreisringe. Gespeicherte Positionen (`positions`) überschreiben (`pinned`, kein Gitterplatz). Messung: 300 Einfälle
  ≈ 1 ms im Debug-Build (Test `round25_bild::three_hundred_ideas_…`, Abnahme < 200 ms). Ergebnis `Layout` (Kreise in der
  Reihenfolge Knoten, Einfälle; `get(id)`; viewBox mit Rand für Labels).
- **`svg.rs`** – Zeichnen nach dem DOM-Vertrag §4.4: `<svg class="im-svg" … data-im-svg>`, `<g class="im-viewport">`,
  Kanten als `<line>` von Mitte zu Mitte (Baum `im-tree`, Bezug `im-ref`, Verbindungen
  `im-link im-kind-<art>` mit `data-link`/`data-kind` und `<title>`, Nähe `im-near` mit `data-score`, nur mit `near`;
  seit Runde 18a Pfeilspitzen, Strichart als Attribut und ein sichtbares Label `im-edge-label`, siehe „Runde 18a – Menüband“),
  Knoten mit `data-node`/`data-x`/`data-y`/`data-r` und `transform="translate(x y)"` (Werk und „Ohne Kapitel“ `<g>`,
  Teil/Kapitel/Abschnitt `<a>` in Schlüsselform, Einfälle `<a href="/einfaelle/<id>">` mit `data-idea`/`data-key`/
  `data-anchor`, `im-open|im-done`, `im-pinned`, mit `q` `im-hit|im-dim`). Zahlen auf eine Dezimalstelle (`num`),
  alle Nutzertexte über `esc`. `render(map, layout, standalone)`: im HTML ohne eigenen Stil (`static/karte.css`, Farben aus
  `app.css`, also hell/dunkel ohne Doppelung), eigenständig mit XML-Kopf, Größe, `<title>` und `STANDALONE_STYLE`
  (feste helle Farben, nur einfache Klassenselektoren – der SVG-Leser im Typst-Satz kennt keine Variablen); Attribute
  ohne Wert dann als `=""` (wohlgeformtes XML, von `xmllint` geprüft).
- **`export.rs`** – Markdown (Überschrift je Knoten, Ebene = Tiefe + 1; Einfälle mit Verbindungen in der Schreibweise
  des Spiegels; „Widersprüche“, „Nähe“; Markdown-Zeichen maskiert) und FreeMind 1.0.1 (Knoten verschachtelt wie auf
  der Karte, IDs `ID_<id ohne Sonderzeichen>`, Gedanke als `richcontent TYPE="NOTE"`, Verbindungen als `arrowlink`
  – „widerspricht“ mit einem Pfeil an beiden Enden – und als Attribut „Verbindung“). `tree_order` und `links_of` nutzt auch
  die Seite.
- **`mod.rs`** – Routen und Seite. Filter aus der Adresse (`parse_filter`, unbekannte Werte entfallen; bei `near` gewinnt der letzte
  Wert, weil das Formular vor das Ankreuzfeld ein verstecktes `near=0` setzt; `open` wiederholbar) und kanonischer
  Query-Teil (`filter_query`) für alle Links, Ausgaben und den Rücksprung. Eigene Vorlagenumgebung (`base.html` +
  `templates/karte.html`, kein `fragments/idea_capture.html`). Ausgaben: `.svg` (`image/svg+xml`), `.md`
  (`text/markdown; charset=utf-8`), `.mm` (`application/x-freemind`, Anhang), `.pdf` über `pdf::source_to_pdf(work,
  PdfKind::Map, …)` (`pdf_source`: A4 quer, Titel und Filter nur als Typst-Strings, das eigenständige SVG als
  Datei `karte.svg` eingepasst; 409 läuft schon, 503 Zeit, 422 Satzfehler – als Karte mit einer Meldung).
  `POST /werke/{id}/karte/act` (hinter `guard::same_origin`): `idea.update` über `crate::ideas::call` (nur `id`, `key`),
  `idea.link`/`idea.unlink`/`idea.link_restore`/`idea.map_pos_reset` über `crate::idea_map::call`; vorher eine Prüfung, dass
  der Einfall oder die Verbindung zu diesem Werk gehört (sonst 422). `back` wird nicht übernommen, sondern als Filter
  gelesen und neu aufgebaut (`back_filter`) – die Weiterleitung `303 /werke/{id}/karte?<filter>&ok=<wort>[&undo=<id>]`
  enthält nichts Ungeprüftes. Fehler: die Karte mit einer Meldung (422 ungültig, 409 abgelehnt, 400 unbekannte Aktion, 404
  unbekanntes Werk).
- Daneben: `PdfKind::Map` („Gedankenkarte“) in `pdf.rs`; Menüeintrag in `base.html`; `ideas.rs` reicht den Seiten
  `map_href` (Liste) oder `links`/`map_href`/`map_act` (Einfall, über `Store::idea_links_of_idea`) durch; Stil in
  `static/karte.css` (Strichart je Art, `im-empty`, `im-dim`, `im-hide-near`, Druck, dunkel) und `static/ideas.css`.
- Gruppen benennen: seit Runde 19b kein Stub mehr, sondern der Knopf „Gruppen benennen (Assistent)“ mit dem
  Abschnitt „Gruppen“ (`idea_map/groups.rs`, siehe „Runde 19 – RAG-Pflege, Rest der Referenzen“, R19b pgvector).
<!-- End R25 karte-bild -->

### Widget und Austausch (karte-widget) {#widget-and-exchange-karte-widget}

**Widget** `crates/baruch-server/web/src/idea_map.ts` (`register("idea-map", …)` auf `.im-stage`, von `build.rs` übersetzt,
über `main.ts` geladen), Stil `static/karte_widget.css`. Es liest nur den DOM-Vertrag §4.4 (Klassen
`im-*`, `data-node/x/y/r/key/idea/anchor/work/near`, `data-from/to` der Kanten, `[data-im-viewport]`,
`[data-im-toolbar]`, `[data-im-side]`, `form.im-filter`) und ruft nur `idea.get`, `idea.create`, `idea.update`,
`idea.link`, `idea.map_pos_set`, `idea.map_pos_reset` auf (der Test `round25_widget::the_widget_reads_only_the_dom_contract_and_calls_only_the_allowed_methods`
prüft das am ausgelieferten Skript). widgets.ts, ideas.ts, zoom.ts und main.ts bleiben unverändert.

- **Ausschnitt:** Zustand `{s, tx, ty}` → `transform="translate(tx ty) scale(s)"` nur auf `[data-im-viewport]`.
  Zoomen um einen Punkt der viewBox (`svg.getScreenCTM()`), Verschieben über die Differenz zweier viewBox-Punkte;
  `sessionStorage` `baruch_im_view_<werk>` (try/catch) hält den Ausschnitt über das Neuladen. Das Mausrad mit Strg
  wird nicht angefasst (zoom.ts).
- **Ziehen:** Pointer-Ereignisse mit `setPointerCapture`; ab 4 Bildschirmpixeln wird aus einem Klick ein Ziehen.
  Knotenkoordinaten über `viewport.getScreenCTM().inverse()` (also in den Einheiten von `data-x/data-y`);
  `place()` setzt `data-x/y`, `transform` und die Endpunkte jeder `.im-edge` mit `data-from`/`data-to` des Knotens.
  Ablageziel (`targetAt`): Knoten mit Abstand ≤ `data-r`, Einfälle vor Gliederungsknoten, nie das Werk. Einfall →
  Dialog (`<dialog class="im-dialog">`, vier Arten, Notiz ≤ 500) → `idea.link`; Teil/Kapitel/Abschnitt →
  `idea.update {id, key}`; „Ohne Kapitel“ → `idea.update {id, key: null}`; sonst `idea.map_pos_set` (Koordinaten auf
  ±1e6 begrenzt) und `im-pinned`. Nach einem Bezug und einer Verbindung lädt die Seite neu (der Server legt aus).
- **Tastatur:** wandernder `tabindex` (genau ein Knoten 0, zuerst das Werk), `<g>`-Knoten bekommen `role="button"`
  und ein `aria-label` aus ihrem `<title>`. Pfeile: nächster Knoten nach Abstand in der Richtung plus doppeltem
  seitlichem Versatz; der Verbindungsmodus (`V`, Klassen `im-connecting`/`im-source`/`im-target`) bewegt sich nur über Einfälle.
  `N` schaltet `im-hide-near` und `near` in der Adresse um (`history.replaceState`); wurde die Seite mit `near=0` gezeichnet
  (keine Nähekanten im DOM), lädt sie mit `near=1` neu. Alt+E: Dialog „Neuer Einfall“ →
  `idea.create {work, key, words, note}` mit dem Bezug des fokussierten Knotens. Meldungen über `.im-status`
  (`role="status"`), Hilfe `.im-help` über `aria-describedby` an der Karte. Texte nur über `textContent`.

**Austausch, Nebenversion 2** (Vertrag §4.6):

- `baruch-exchange/src/model.rs`: `FORMAT_VERSION = 2`; `ExchangeLinkKind` (`fuehrt-weiter`, `gehoert-zu`,
  `beispiel-fuer`, `widerspricht`), `ExchangeLink`, `ExchangePosition`, `ExchangeMap` (`ExchangeWork.map`),
  Grabsteinarten `Link = "verbindung"` (`idea_link`) und `Position = "lage"` (`idea_map_pos`) mit
  `since_version()` (2, sonst 1); dazu `MAP_COORD_LIMIT`, `MAX_LINK_NOTE_CHARS` und `ExchangeMap::is_empty`.
- `write.rs`: `<karte>` am Ende von `<werk>` nur mit Inhalt, zuerst `<verbindung>` (mit `<notiz>`), dann `<lage/>`;
  x/y über `f64::to_string` (kürzeste exakte Darstellung), außerhalb ±1e6 → `ExchangeError::Write`.
- `read/map.rs`: `<karte>` nur ab `version >= 2` (sonst unbekannt, `werk/karte`); eine Grabsteinart mit
  `since_version() > version` ist ein XML-Fehler; unbekannte Arten bei `version > FORMAT_VERSION` werden gemeldet und
  übersprungen. `read/check.rs` (`check_map`): Verweise auf Einfälle des Dokuments, kein Selbstbezug, kein Paar
  mit gleicher Art zweimal (`widerspricht` normalisiert), Notiz ≤ 500 ohne Steuerzeichen außer `\n`, je Einfall
  eine Position, |x|, |y| ≤ 1e6; die Bezeichner zählen zur Eindeutigkeit im ganzen Dokument (auch gegen Grabsteine).
- `docs/schema/baruch-werk-1-u2.xsd`: Kopie des v1-XSD mit einer definierten `Karte` (`Verbindung`, `Lage`,
  `VerbindungArt`, `Koordinate` als `xs:double` in ±1e6) und einer erweiterten `GrabsteinArt`; das v1-XSD bleibt
  unverändert.
- Store `exchange_dump.rs`: `map()` liest `idea_link` (beide Einfälle im Werk, auch weich gelöschte, nach ID) und
  `idea_map_pos` (Einfall noch im Werk, nach ID); Grabsteine wie bisher über `work_uuid`.
- Store `exchange_apply.rs`: `new_map` (nach Einfällen und Bildern; `ensure_free`, `widerspricht` mit from < to
  nach lokalen IDs) und `merge_map` (nach `merge_ideas`, damit eben eingelesene Einfälle zählen): unbekannter
  Bezeichner + beide Einfälle im Werk → anlegen, sonst ein Hinweis; dasselbe Paar+Art (in der normalisierten Richtung)
  → Hinweis „gibt es hier schon unter anderer Kennung“; bekannter Bezeichner → nie überschreiben (Hinweis bei Notiz/
  gelöscht); Grabstein → „hier gelöscht“; Position nur, wenn es für (Werk, Einfall) keine gibt. `AddCounts.links`,
  `AddCounts.positions` (in `total()`), Wörter `verbindung`/`lage` in `items` und `deleted_here`. Nur hier wird
  `uuid` in `idea_link`/`idea_map_pos` geschrieben (Wächtertest `uuids.rs`).
- Server: `exchange/import.rs` (`add_list`: „Verbindungen der Gedankenkarte“, „Lagen auf der Gedankenkarte“ in
  Vorschau, Ergebnis und CLI), `exchange/export.rs` (`counts`: `links`, `deleted_links`, `positions` für
  `templates/exchange.html`), Beschreibung von MCP `exchange_export`. Keine Migration (0044 bleibt unbenutzt).
- Tests: `baruch-exchange/tests/{read,write,xsd}.rs`, `baruch-store/tests/{exchange_map,exchange_apply,uuids}.rs`,
  Server `tests/round25_widget.rs` (Rundweg A → B → A mit Karte, Vorschauseite, Seite „Austausch“) und
  `round26_*` gegen das u2-XSD.
- Gefunden (an karte-daten gemeldet, Migration 0043): wird ein Einfall, der `from_id` einer
  Verbindung ist, hart gelöscht, schreibt der Grabstein-Trigger `idea_link_uuid_tombstone` `work_uuid` NULL (die Einfallszeile ist
  bei der Kaskade schon weg, wie bei Fäden vor 0042); Abhilfe wäre ein `BEFORE DELETE ON idea`, das die
  Verbindungen vorher löscht. In der Praxis selten (Einfälle werden nur weich gelöscht). In der Integration behoben
  (Trigger `idea_link_cascade_uuid_tombstone` in 0043, `uuids.rs` prüft jetzt beide Richtungen).
<!-- End R25 karte-widget -->

### Integration Runde 25 (Integrator, 2026-11-08) {#integration-round-25-integrator-2026-11-08}
- `team/karte-daten` → `team/karte-bild` → `team/karte-widget` gemergt (`--no-ff`, ohne Konflikte; alle drei
  zweigten von S1 `c0e5e8a` ab).
- **Migration 0043 korrigiert** (noch auf keiner Instanz gelaufen): `BEFORE DELETE ON idea` löscht die Verbindungen,
  solange die Einfallszeile noch steht (`idea_link_cascade_uuid_tombstone`), sonst trüge der Grabstein einer Verbindung mit
  diesem `from_id` kein Werk (Befund karte-widget). `migration_0043.rs` kennt das neue Objekt, `uuids.rs` löscht jetzt
  den mittleren Einfall (Quelle und Ziel zugleich).
- **Kanonische Adresse der Karte:** `build` legt die Vorgaben `near_limit`/`near_threshold` in den angewandten Filter;
  `bild::filter_query` schrieb sie dann in jeden Link (`?near_limit=3&near_threshold=0.75`). Erst nach dem
  Mergen sichtbar (zwei Tests in `round25_bild.rs` schlugen an); jetzt entfallen Vorgabewerte aus der Adresse.
- Die Teile von `round25_bild.rs`, die ohne R25a „ÜBERSPRUNGEN“ meldeten, laufen jetzt vollständig.
- **Abnahme** `tests/round25_abnahme.rs`: erfundenes Werk mit zwei Teilen und 40 Einfällen, Einbettungen aus dem Nachbau
  `MockEliezer` (512 Dim.); Seite ohne JavaScript (SVG, Gliederung, Filter über GET, Formulare), `im-empty`, Anker am
  Kapitel oder „Ohne Kapitel“; Nähe über Vektoren ohne Einbettungsanfrage beim Zeichnen und über Stichwörter;
  Bezug über Widget-RPC und Formular (je ein Commit); „widerspricht“ in der Karte, Liste der Widersprüche, `einfaelle.md` und
  MCP `idea_links`; Positionen ohne Commit; ein gelöschter Einfall fällt samt seiner Verbindung heraus und kommt zurück; SVG/Markdown/
  FreeMind/PDF (xmllint). Ohne Einbettungsmodell nennt die Seite den Ersatz über Stichwörter.
- **Sichtprüfung** (Firefox headless, eigener Wegwerf-Server mit einem erfundenen Werk, Einbettungen aus einem eigenen
  Nachbau auf 127.0.0.1:0): die Bühne wurde in der Flex-Spalte `.karte` auf ihre Mindesthöhe gequetscht (Karte winzig,
  mit JavaScript unten abgeschnitten) – jetzt `flex-shrink: 0` für die Kinder von `.karte` und `.im-stage` und eine
  feste Höhe `min(78vh, 900px)` für `.im-svg` (`static/karte.css`). „Einpassen“ im Widget lässt Platz für die
  Labels unter und neben den Kreisen (`LABEL_SIDE`, `LABEL_BELOW`). Die Seitenleiste nennt den Status wie
  der Rest der Benutzeroberfläche „verarbeitet“ (nicht „erledigt“). Bilder: `.claude/sicht/r25/`.
- **Nachbesserung nach dem Review:**
  - Werkwechsel eines verbundenen Einfalls: beim Verschieben (anderes Werk oder „ohne Werk“) löst `Store::idea_update`
    alle ungelösten Verbindungen des Einfalls in derselben Transaktion weich auf und löscht seine Positionen hart
    (Grabstein `idea_map_pos` im alten Werk). Vorher blieb die Verbindung aktiv und verband zwei Werke: Karte und
    Austauschformat ließen sie aus, `/einfaelle/<id>`, `idea_links {idea}` und `einfaelle.md` beider Werke zeigten
    sie, „Lösen“ von der Seite eines verschobenen Ziels scheiterte mit 422, eine alte Position tauchte beim Zurückschieben wieder auf.
    `ideas::update` nennt die aufgelösten Verbindungen im Commit beider Spiegel und als `unlinked` in der
    Antwort (RPC `idea.update`, MCP `idea_update`). Tests: Store `idea_links.rs`
    (`moving_a_linked_idea_…`), Server `round25_daten.rs` (`moving_a_linked_idea_to_another_work_unlinks_it_everywhere`).
  - Zähler der Karte rollen auf: `idea_map::build` zählt einen Einfall am Anker und an allen Vorfahren bis unter
    das Werk (`parent_slot`). Ein Teil mit gefüllten Kapiteln zählte vorher als leer (blass, ohne Zahl, „Keine
    Einfälle.“); ein aufgeklapptes Kapitel zählt jetzt auch seine Abschnitte. Liste und Markdown sagen für einen Punkt
    ohne eigene Einfälle, aber mit Einfällen darunter „Keine eigenen Einfälle; sie hängen an den Punkten darunter.“
  - „Zeigen, was fehlt“ hebt hervor statt abzutönen: `im-empty` = gestrichelter Rand in der Warnfarbe, im Kreis
    das Wort „leer“ (`svg.rs`, auch im eigenständigen SVG), in der Liste das Etikett „leer“ und ein gestrichelter
    Warnrand (`static/karte.css`, `templates/karte.html`).

## Runde 18a – Menüband {#round-18a--ribbon}

Menüband für Editor und Satzseite, abbrechbarer Typst-Satz ohne Läufe von fremden Seiten, Kapitel eines gemeinsamen
Seite, Kopfzeile in schmalen Fenstern und die Kanten der Ideenkarte.

- **Menüband** (`templates/fragments/edit_toolbar.html`): ein Fragment, zwei Fassungen. Im Editor
  (`div.ws-bar.ribbon[data-edit-toolbar][hidden]`) bleiben alle Haken der Satzansicht (`data-ws-mode`, `data-ws-tools`,
  `data-ws-format`, `data-ws-cmd`); dahinter `span.ribbon-formats[data-widget=ribbon]` mit je einem Knopf
  `data-ribbon-cmd`/`data-ribbon-snippet` aus `src/ribbon.rs` (`formats(template)`: `scene`/`work` nur mit
  `#let szene`/`#let arbeit` in der Werkvorlage, dann `footnote`, `quote`, `list`, `enum`, `terms`, `break`, `link`;
  RPC `ribbon.formats`; Kontext `ribbon` aus `view::ViewExtras`) und `data-ribbon-image`. Widget `web/src/ribbon.ts`:
  bei offener Satzansicht nur `sendWsCommand` (eingefrorenes `ws_command.ts`), `handled = false` → Hinweis über
  `baruch:toast`; im Quelltextmodus Einfügen in das Textfeld bei `selectionStart` (`execCommand("insertText")`, Rückfall
  `setRangeText` + `input`). Kein Import von `wysiwyg.ts`. Mit `satz_page` (Satzseite) `div.ribbon-satz
  [data-ribbon-satz]`: zurück, PDF, ‹ › mit `aria-label`, Formular „Satz abbrechen“ – ohne JavaScript; `typeset.html`
  leert `{% block scripts %}` (kein `main.js`). Stil `static/ribbon.css`. Nachbesserung: `typeset::render` setzt
  `scriptless = true`; `base.html` zeigt dann kein Widget in der Kopfzeile – das Thema als Link auf
  `/einstellungen/darstellung`, „⚙ Einstellungen“ ohne Dialog und `aria-keyshortcuts`, „Öffnen“ der Werkwahl
  sichtbar statt in `<noscript>`.
- **Abbrechen** (`baruch-render`): `Cancel(Arc<AtomicBool>)` mit `cancel`/`is_cancelled`, `compile_cancellable(source,
  cancel) -> Option<Rendered>` (`None` = abgebrochen, nie halbe Seiten); `compile`/`compile_pdf` unverändert.
  Prüfstellen: vor dem Lauf, nach `typst::compile` (Auswertung und Umbruch), vor jeder SVG-Seite, am Ende.
  **Gemessen** (`crates/baruch-render/tests/cancel.rs`, Debug-Build, 2026-11-08): 500 Absätze (512 Seiten) – ohne
  Abbruch 777 ms, nach 200 ms abgebrochen nach 252 ms zurück (der Umbruch war fertig, gespart wurde das Schreiben
  der SVG-Seiten); Auswertungsschleife mit 3 000 000 Runden – ohne Abbruch 353 ms, nach 100 ms abgebrochen erst
  nach 368 ms zurück (nicht unterbrochen). **Nicht möglich:** ein Abbruch über die Welt (nach dem Abbruch keine
  Dateien oder Schriften mehr liefern). Typst merkt sich die Antworten der Welt (`comemo`) und bricht mit „comemo:
  found differing return values. is there an impure tracked function?“ ab, wenn dieselbe Frage innerhalb eines Laufs
  anders beantwortet wird – so im ersten Entwurf beobachtet und darum verworfen. Auswertung und Umbruch laufen also immer
  zu Ende; ein eigener Prozess je Lauf wäre der einzige harte Abbruch (nicht gemacht, Rest).
- **Server** (`src/typeset.rs`): jeder Lauf für ein Werk (`compile_for`: Satzseite, SVG-Seite, `preview.typst`,
  `typst_check`) meldet sich mit Datenordner, Werk und `Cancel` in einer prozessweiten Liste an; die Anmeldung
  (`Arc<Registration>`) halten Anfrage und Hintergrundlauf, sie fällt mit dem letzten weg. `cancel(work)` (Vertrag, alle
  Server im Prozess) oder `cancel_in(data, work)` (RPC `preview.cancel`, `POST /werke/{id}/satz/abbrechen`) setzen
  alle `Cancel` des Werks. Die Anfrage wartet mit `tokio::select!` auf die Sperre oder den Lauf **oder** den Abbruch (alle
  20 ms abgefragt) und kehrt sofort mit -32000 `CANCELLED` zurück; der Lauf hält die Sperre `RUNS`, bis Typst an der
  nächsten Prüfstelle anhält, und speichert nichts. `compile` ohne Werk (Umschlag) wird nicht angemeldet. Die Seite
  nach dem Abbrechen (`?abgebrochen=1|0`) setzt nicht neu (`cached_typeset`: nur ein schon gesetzter Stand). Für
  Tests: `running_in`, `runs_of(stand)` (begonnene Läufe je Stand, höchstens 256 Stände), `cache_contains`,
  `pdf::pdf_runs()`.
- **Fremde Seiten:** `GET /werke/{id}/satz`, `…/{stand}/{n}.svg` und `/werke/{id}/pdf` – nach der Nachbesserung zu
  Runde 18 auch der Umschlag (`/werke/{id}/umschlag`, `.svg`, `.pdf`), `/werke/{id}/stichwortzettel.pdf` und
  `/werke/{id}/karte.pdf`, also jedes GET, das Typst startet und die Sperre `typeset::RUNS` hält – prüfen zuerst
  `guard::cross_site_get` (`Sec-Fetch-Site: cross-site|same-site`) → 403 (`typeset::foreign_get`), bevor sie das Werk vorbereiten
  oder Typst starten. Ohne den Header (curl, alte Browser) bleibt alles wie bisher.
- **Cache:** zusätzlich zur Anzahl (8) eine Bytegrenze `CACHE_BYTES` = 192 MiB über alle SVG-Seiten
  (`cache_put_within`); der neueste Stand bleibt auch über der Grenze.
- **Kapitel einer gemeinsamen Seite:** `Rendered.page_lines` (neu in `baruch-render`) nennt je Seite die Zeilen von
  `main`, von denen Text darauf steht (erster Glyph jedes Textstücks, auch in Kopf- und Fußzeilen).
  `Typeset::chapter` fügt dem Kapitel die Seite hinzu, auf der das nächste Kapitel beginnt, wenn Text aus dessen
  Zeilen dort steht – ohne Überschriftzeilen, weil die Vorlage Überschriften als Kolumnentitel oder im
  Inhaltsverzeichnis auf fremden Seiten zitiert. Grenzen: ein Kapitel, das auf der gemeinsamen Seite nur mit einem Bild
  oder einer Linie (ohne Text) endet, zählt dort nicht; zitiert die Vorlage auf fremden Seiten anderen Text als
  Überschriften, kann eine Seite einem Kapitel zu viel zugeordnet werden.
- **Schriften:** `baruch_render::warm_fonts()` lädt eingebettete und Systemschriften einmal je Prozess;
  `typeset::warm_fonts()` startet das in `spawn_blocking` und kehrt sofort zurück. Vorschlag für den Integrator:
  in `main.rs` am Anfang von `serve` aufrufen (in der Tokio-Laufzeit); Tests rufen es nicht von selbst auf.
- **Kopfzeile:** `static/app.css` – unter 1000 px Breite `@media (max-width: 999.98px) { .topbar { flex-wrap: wrap } }`;
  darüber bleibt die einzeilige Regel. Der Test (`round9_menu.rs`) liest nur die Regeln; die Breite misst die
  Sichtprüfung.
- **„neu“ als Wort:** `chat.ts` hängt `span.badge.post-new-label` („neu“) hinter den Namen neuer Beiträge.
- **Heller Streifen rechts** (Satz, Gliederungsschema): `.works-page` (`works.css`) ist zugleich ein Scroll-Container
  (`overflow: auto`) und auf 1100 px begrenzt und zentriert; auf langen Seiten stand ihre Bildlaufleiste daher in der Mitte des
  Fensters rechts vom Inhalt. `typeset.css`/`numbering.css` geben diesen beiden Seiten die volle Breite und
  rücken den Inhalt, gleich breit, über `padding-inline: max(18px, calc((100% - 1100px) / 2 + 18px))` in die Mitte.
  Ursache aus dem CSS abgeleitet, nicht im Browser gemessen (Sichtprüfung des Integrators).
- **Ideenkarte, DOM-Vertrag erweitert** (`src/idea_map/bild/svg.rs`, `web/src/idea_map.ts`, Tests
  `round25_{bild,widget}.rs`, `round18_menueband.rs`):
  - `<defs>` vor `g.im-viewport` mit `<marker id="im-arrow-<art>" class="im-arrow im-arrow-<art>">` für die
    gerichteten Arten (`leads_to`, `belongs_to`, `example_of`; `contradicts` ist ungerichtet). Die Spitze hat eine
    feste helle Füllung (für SVG-Datei und PDF); auf der Seite färbt `karte.css` sie nach dem Thema.
  - Verbindungskante `line.im-edge.im-link.im-kind-<art>` zusätzlich mit `stroke-dasharray` (außer `leads_to`), bei
    gerichteten Arten `marker-end="url(#im-arrow-<art>)"` und `data-arrow` (in der eigenständigen Datei
    `data-arrow=""`); sie endet dann am Rand des Zielkreises (`r` + 2), sonst in der Mitte. Bezug `im-ref` mit
    `stroke-dasharray="4 3"`, Nähe `im-near` mit `"1 3"`, Baum durchgezogen.
  - Label: `g.im-edge-labels` nach `g.im-edges` und `g.im-nodes` (seit der Integration über den Knoten, damit
    kurze Kanten zwischen nahen Einfällen lesbar beschriftet sind; `pointer-events: none`), darin je Verbindungs- oder Nähekante
    `<text class="im-edge-label im-label-<art>|im-label-near" data-from data-to x y text-anchor="middle">` mit der
    Art („führt weiter“ …) oder „nah“, 4 Einheiten über der Mitte.
  - Widget: beim Ziehen zeichnet `place` jede Kante über `redraw` neu (Pfeilkanten am Kreisrand) und setzt jedes
    Label mit gleichem `data-from`/`data-to` über `relabel` in die Mitte. Die Seitenleiste `.im-side` beginnt
    bei `--im-top` (Abstand des Bildes vom oberen Rand der Bühne, vom Widget gemessen, neu bei `ResizeObserver` und
    `resize`), sie verdeckt die Werkzeugleiste also nicht mehr.
  - Ausgaben: SVG/PDF wie die Seite (Stil des Labels in `STANDALONE_STYLE`); FreeMind `arrowlink` mit `COLOR`
    je Art (FreeMind kennt keine Strichart für Pfeile); Markdown nennt die Art wie bisher als Wort.

## Runde 18b – Konstrukte {#round-18b--constructs}

**Satzansicht (`crates/baruch-wysiwyg`):** Neue Blockarten `Enum` (nummerierte Liste `1.`, `start`) und `Terms` (Begriffs-
liste, `Term {term, desc}`), `List`/`Enum` mit `depths` (Tiefe je Punkt, verschachtelt; `#[serde(default)]`, ohne
Verschachtelung nicht geschrieben – altes JSON bleibt gültig); neue Inlines `Linebreak`, `Link {url, c?}`, `Ref {target, supplement?}`.
Das Zerlegen in Stücke (`parse.rs`) kennt Zeilenarten für nummerierte Punkte und Begriffe; ein Listenstück umfasst eingerückte Punkte
und Fortsetzungszeilen, `list()` bestimmt die Tiefe mit einem Stapel von Zeichenspalten und lehnt alles
Unsichere ab (Insel). `scan::skip_escape` behandelt `\` vor Leerraum als Zeilenumbruch (ein Zeichen), damit ein Zeilen-
ende danach die logische Zeile beendet. `emit.rs` schreibt kanonisch (zwei Leerzeichen je Ebene, fortlaufende Nummern, `\:` im
Begriff, `#ref(<…>)` vor Zeichen des Ziels, `\ ` vor schließenden Zeichen). `Edit::Raw {replaces, raw}`
ersetzt den Quelltext eines Blocks wörtlich (im Rumpf mit seiner Einrückung), `WysiwygError::NewPart` verbietet neue oder
umgewandelte Teile. `image_paths` liefert alle Bildpfade einer Insel; `html.rs` zeigt alle. Eingefroren waren
`render_block`/`escape` (von `align.rs` genutzt) und die `pub use`-Zeilen von `style`/`align`.

**Server:** `wysiwyg::image_ref` nimmt die Normalform ohne führenden `/`; `rpc.rs` (`revision.update`) wandelt
Markdown-Überschriften wie `revision.save` um. Der Teilblock im Kapiteltext ist fest (siehe `docs/WYSIWYG.md` „Teile
und „ohne Nummer““): die Satzansicht lehnt ab, `outline.save` nur nach Bestätigung (`split`).

**Widget (`web/src/wysiwyg.ts`):** liest und schreibt die neuen Konstrukte (verschachtelte `<ul>/<ol>`, `<dl>`,
`span.ws-linebreak`, `a.ws-link`, `button.ws-ref`), öffnet Inseln per Doppelklick an Ort und Stelle (`editIsland`,
`data-edited` → `{replaces, raw}`), führt `baruch:ws-command` aus (`runCommand`, merkt sich die letzte Auswahl in der
Satzansicht) und beantwortet `baruch:ws-caret` (`caretPlace`: zwei `wysiwyg.serialize`, Text und Präfix bis zum Block
an der Schreibmarke). `guardHeadingMerge` überspringt `.pv` (`blockNeighbour`), `applyStyle` übernimmt ein vorhandenes
`style#ws-style[data-work=W]` ohne RPC. `ws_command.ts` ist unverändert.

**Lektor-Reste (`src/assist`):**
- Ausgelassene Teile: `context::Frame::environment` schreibt aufeinanderfolgende ausgelassene Teile in **eine** Zeile
  (`--- nicht mitgegeben (…): A; B; C ---`, `context::OMITTED_HEAD`). Die Planung rechnet dafür
  `chunk::OMITTED_OVERHEAD` (100) einmal je Lauf und nur Titel + `chunk::OMITTED_ITEM` (2) für jeden weiteren
  Teil, ebenso die Untergrenze `floor_violation`. Folge: ein Kapitel mit 10 Abschnitten zu je 3000 Zeichen
  passt jetzt bei 4096 Tokens in Stücken (gemessen: kein Aufruf über dem Budget, `round18_lektor.rs`), wo Baruch vorher
  ehrlich ablehnte.
- Cache der Zusammenfassungen: `assist::summary_cache()` (prozessweit, `VecDeque`, höchstens `SUMMARY_CACHE_MAX` = 256
  Einträge, die ältesten fallen heraus, keine Migration). Schlüssel: Datenordner, Modell und SHA-256 über den ganzen Aufruf (Regel einschließlich
  Wortgrenze, Werktitel, Name des Teils, Text, `max_tokens`) – das ist „Modell + Text“ und alles andere, was
  die Antwort ändert. Leere Antworten und Fehler werden nicht gespeichert.
- „An Persona geben“ auf `/quelle/multigpt`: Formular `action=persona` an `/literatur/act` (wie die Bibelseite),
  mit dem Text des gezeigten Teils vorbelegt (höchstens 4000 Zeichen), Quelle „MultiGPT: Titel (Dokument …)“, ohne
  den Titel der Bibliothek; ohne Abschnitt nur ein Hinweis.
- Neuanfrage im Strom: neues Ereignis `piece {persona, index, count}` vor jedem weiteren Stück eines Auftrags in Stücken (nach
  dem trennenden Delta, `Progress::Piece`); `assist.ts` beginnt dort einen neuen Teil des vorläufigen Beitrags, `retry`
  leert nur diesen. Der endgültige Text kam vorher schon richtig mit `done` an.

## Runde 18c – Satzansicht je Werk {#round-18c--typeset-view-per-work}

- **Store** (`crates/baruch-store/src/sheet.rs`, Migration `0044_work_sheet.sql`): Tabelle `work_sheet` (eine Zeile je
  Werk, `ON DELETE CASCADE`, CHECK auf Längen oder `#`+6 Zeichen; eine neue Tabelle, kein Umbau, daher nicht in
  `FOREIGN_KEYS_OFF`), `SheetSettings` (`font`, `heading_font`, `size`, `fill`, `heading_fill`, `background`,
  `width`; `None` = aus der Vorlage), `Store::sheet_settings` (leer ohne Zeile) und `set_sheet_settings` (ganze
  Zeile, alles leer löscht sie). Vor dem Schreiben geprüft (`SheetSettings::normalized`): Schriftlisten höchstens
  `MAX_SHEET_FONT_CHARS` = 200 Zeichen Namen mit Buchstaben, Ziffern, Leerzeichen, `- _ .` (auch paarweise in Anführungs-
  zeichen) – nie `< > ; { } \` oder Steuerzeichen; Längen `pt|mm|cm|em` (Größe 6–48 pt, Breite 6–60 cm,
  `em` = 12 pt); Farben `#rrggbb`. Kein Spiegel, keine UUID, nicht im Austauschformat (Rest). Tests
  `tests/{sheet,migration_0044}.rs`.
- **Darstellung** (`baruch-wysiwyg/src/style.rs`): `SheetStyle.background` (neu; nie aus der Vorlage) →
  `--ws-background`; `sheet_style_over(vorlage, einstellungen)`; seit 18c erlaubt `safe` `_` und nur paarweise Anführungs-
  zeichen. Server `src/sheet.rs`: `sheet_style_for` = `sheet_style_over(sheet_style(work_template), einstellungen)` für
  `wysiwyg.style`, `view.rs` (`ViewExtras.sheet_css` → `<style id="ws-style" data-work>` in
  `fragments/editor.html`) und RPC `work.sheet`/`work.sheet_set`; `sheet_css_for` ersetzt der Sicherheit halber jedes `<`.
  `static/sheet.css` (direkt nach `wysiwyg.css`): Blattfarbe und `::highlight(literatur-fundstelle)`. Formular
  `POST /einstellungen/werk/satzbild` und Block „Satzbild“ in `settings/work.rs` / `templates/settings/werk.html`.
  Typst, PDF und Export lesen `work_sheet` nie (Test: Quelltext, Stand und PDF byte-identisch).
- **Lange Abschnitte:** `view::READY_SHEET_MAX` = 64 KiB; darüber keine vorgerenderte Satzansicht (`ready_sheet`).
- **Bild an der Schreibmarke** (`src/assets.rs`): `Place` (`end|start|block:<n>|offset:<n>`), `insert_figure_at`
  (rein: nie vor/in die Überschrift, Zeilengrenze, Inseln/Szenen/Listen als Ganzes, Leerzeilen davor und danach),
  `image_places` (Auswahl ohne JavaScript). Store `save_human_with_asset_path`: Pfad und Revision in einer Transaktion
  (`BEGIN IMMEDIATE`), ein Spiegel-Commit. Widget `web/src/sheet.ts` („sheet-tools“ auf `details[data-image-insert]`):
  Position über `baruch:ws-caret` (am Editorformular mit `bubbles`, Antwort von `wysiwyg.ts`) oder `selectionStart`;
  ohne Antwort `baruch:serialize-text` und das Ende. Es frischt auch `#ws-style` nach
  `baruch:settings-changed` über `work.sheet` auf.
- **Vorschläge:** `align.rs` `split_changed` (Wort-Dice ≥ 0,5, monotone Paarung, nur bei ungleicher Blockzahl);
  Zeilendiff ohne nachgestellten Leerraum (`view.rs`); Leiste `div.pv-bar-editor[data-pv-bar]` vor dem Lesefluss
  (`proposals.html`, `proposals.css` mit `order: -1`/`sticky`, `proposals.ts`).
- **Literatur** (`web/src/literature.ts`): die gefundene Stelle wird mit der CSS Custom Highlight API markiert, solange die
  Satzansicht offen ist (`literatur-fundstelle`), kein DOM-Eingriff; öffnet sie zuerst die Satzansicht, wird nach
  `data-ws-open` markiert.
- **Verhaltenstest** `tests/round18_satzbild.rs`: Node 20 mit eigener kleiner DOM-Attrappe (als Text im Test), die
  übersetzten `SCRIPTS` in einem Temp-Ordner, `fetch` nachgebaut; ohne `node` „ÜBERSPRUNGEN“.

## Integration von Runde 18 {#integration-of-round-18}

- Mit `--no-ff` gemergt in der Reihenfolge `team/konstrukte` → `team/satzbild-werk` → `team/menueband`, ohne Konflikte
  (Dateieigentum nach Vertrag R18).
- `main.rs` ruft `typeset::warm_fonts()` am Anfang von `serve` auf (Schriftsuche im Hintergrund, nach
  `rag::catch_up`).
- Zusicherungen anderer aktualisiert: `migration_0040.rs`/`migration_0041.rs` nehmen die neue Tabelle `work_sheet` (0044) aus
  dem Vergleich der Schemaobjekte und der Verweise auf `work` aus; `round15_nachbesserung.rs` prüft jetzt die
  ehrliche Ablehnung bei 3000 statt 4096 Tokens, weil ausgelassene Teile seit 18b eine Zeile teilen und der alte Fall passt
  (gemessen in `round18_lektor.rs`); `round18_menueband.rs` liest `details[data-image-insert]` mit dem Widget
  `sheet-tools` aus 18c.
- `docs/RPC.md`: Ereignis `piece` und Verwerfen nur des laufenden Stücks bei `retry` im Strom `/assistenten/stream`.
- Abnahme `tests/round18_abnahme.rs`: ein erfundenes Werk mit Szene, Arbeitsauftrag, Fußnote, Zitat, nummerierter
  Liste, Begriffsliste, verschachtelter Liste, Zeilenumbruch, Link, Verweis, zwei Bildern in einem Block und einer Insel. Jeder Block, den
  das Widget nach einer Bearbeitung neu sendet, bleibt bei kanonischem Text byte-identisch; die Insel wird an Ort und Stelle geändert; Formate
  der Vorlage im Menüband und ein ohne JavaScript gespeicherter Knopftext als Szene; Einstellungen der Satzansicht im
  HTML vor jedem Skript und nach einem Neustart; Bild an der Schreibmarke; andere Seiten lösen keinen Satz aus,
  „Satz abbrechen“ ohne laufenden Satz meldet `abgebrochen=0`.
- Sichtprüfung (Firefox headless, Wegwerf-Server, erfundenes Werk; Bilder unter `.claude/sicht/r18/`), behoben:
  - Unter 900 px blieben die gestapelten Bereiche mit `minmax(16em, auto)` 16em hoch (der Arbeitsbereich hat eine feste Höhe,
    es gibt keinen freien Platz zum Wachsen); der Editor (85vh) reichte unter „Assistenten“. Jetzt `grid-auto-rows: max-content`,
    Gliederung und Assistenten zwischen 16em und 85vh (`static/app.css`).
  - Labels der Ideenkarte liegen jetzt über den Knoten (`g.im-edge-labels` als letzte Gruppe, `pointer-events: none`):
    bei kurzen Kanten zwischen nahen Einfällen verschwand „Beispiel für“ unter den Kreisen.

## Runde 27 {#round-27}

Umschlageditor und Impressum in der Werkvorlage. Schnittstelle (S1): Typen in `baruch_typst::cover_layout` (`CoverBlock`,
`CoverBlockKind`, `CoverFace`, `CoverLayout`, `CoverGeometry`, Grenzen gleich den CHECKs), `TemplateFunctions`,
`BOOK_DICT` und Platzhalter in `baruch_typst::cover`, `font_families`/`has_font_family` in `baruch-render`,
Migration 0045 (`cover_block`, Grabsteinart `cover_block`, Adressen in `work_book`, Antragsart `cover_layout`),
Store `cover_blocks.rs`, Server `cover_layout.rs`, `cover_print.rs`, `mcp/cover.rs`, Widget `cover_editor.ts`.

### 27a – Daten (umschlag-daten) {#27a--data-umschlag-daten}

- **Store `cover_blocks.rs`** (Migration 0045, keine weitere Migration; 0046 blieb unbenutzt): `check_cover_block` prüft einen
  Block gegen die Grenzen aus `baruch_typst::cover_layout` (gleich den CHECKs) und bringt ihn in Normalform
  (Farbe klein, leere Schrift = `None`, Rücken ohne Richtung = `down`, Arten mit eigenem Text ohne Text =
  `""`, CRLF → LF). Schriftnamen folgen der Regel der Schriften der Satzansicht (`sheet::check_font`, hier ein einzelner Familien-
  name). Verstöße sind `StoreError::Asset(String)` (RPC -32602) mit Art und Feld – `Refused` trägt nur feste
  Texte und ergäbe `REFUSED`; eine eigene Fehlerart hätte `lib.rs` geändert (Integrator).
  `set_cover_blocks` schreibt je Art mit `INSERT … ON CONFLICT (work_id, kind) DO UPDATE`: ein geänderter Block behält Zeile
  und UUID (kein Grabstein, derselbe Block im Austausch). `reset_cover_blocks` löscht Zeile für Zeile (Trigger
  `cover_block_uuid_tombstone`) und setzt den Titelbereich über `book::read`/`book::write` in derselben Transaktion
  (`BEGIN IMMEDIATE`). `Store::cover_layout_fingerprint` = SHA-256 über das JSON der Blöcke in Artreihenfolge
  (`expected_layout` der Anträge).
- **Adressen** (`book.rs`): zwei weitere Textfelder in `TEXT_FIELDS` (mehrzeilig, 500 Zeichen), gelesen und geschrieben wie die
  anderen; die S1-Ablehnung ist weg. R27c verdrahtet das Formular und `work.meta.set`.
- **Anträge** (`asset_requests.rs`): `AssetOp::CoverLayout` mit `params` `{blocks, expected_layout}`; beim Antrag
  werden die Blöcke geprüft und normalisiert (damit der Dublettenvergleich über `params` gleiche Anträge findet),
  `expected_layout` leer = der Store trägt den Fingerabdruck ein; ein Antrag, der nichts ändern würde, wird abgelehnt.
- **Server `cover_layout.rs`**: `parse_blocks` liest Blöcke Feld für Feld aus JSON (Fehler mit Art und Feld;
  serde allein nennt bei einem falschen Typ das Feld nicht), RPC wie in `docs/RPC.md` „Layout“. Commits über einen eigenen
  `commit`-Helfer (Muster `cover::commit`, dazu der Trailer eines bestätigten Antrags wie
  `mirror::AssetBy::Confirmed`), `mirror.rs` bleibt also unangetastet. Für den Rumpf „vorher → nachher“ eines zum
  ersten Mal gespeicherten Blocks setzt `defaults` den Umschlag (`build_wrap_for`, Cache des Satzes) und nimmt `default_blocks`
  – außerhalb der Schreibsperre; ohne Seitengröße „neu (…)“. `decide_request` vergleicht den Fingerabdruck unter der Schreib-
  sperre, sonst `stale`; ein bei der Bestätigung ungültiger Block (`Asset`/`Refused`) macht den Antrag ebenfalls `stale`. `asset_requests::request_json`
  zeigt für `cover_layout` eine Zeile je Block (`block_lines`) und verweist auf die
  Umschlagseite.
- **MCP `mcp/cover.rs`**: `cover_layout` = `layout_view`, bei `REFUSED` (keine Seitengröße) die gespeicherten Blöcke mit
  `geometry: null`; `cover_layout_set` füllt fehlende Felder aus dem wirksamen Block und legt den Antrag an.
- **Austausch, Nebenversion 3** (`baruch-exchange`, `model.rs` diesmal von umschlag-daten): `FORMAT_VERSION = 3`,
  `ExchangeCoverBlock` (+ Aufzählungen mit deutschen XML-Wörtern), `ExchangeBook.publisher_address`, `printer_address`,
  `cover_blocks`, `ExchangeWork.sheet` (`ExchangeSheet`), Grabsteinart `CoverBlock` (`umschlag-block`,
  `since_version` 3). Leser: neue Teile nur ab `version >= 3` (wie `<karte>` ab 2), sonst `unknown`. Prüfung in
  `read/check.rs` (`check_cover`). XSD `docs/schema/baruch-werk-1-u3.xsd` = u2 plus `Satzbild`, Adressen (im
  Schema optional, damit u2-Dateien gültig sind), `Umschlag`/`UmschlagBlock`, `GrabsteinArt` `umschlag-block`; u2- und v1-XSD
  eingefroren (SHA-256-Test). Store: `exchange_dump` (Blöcke, Adressen, `work_sheet`; Blöcke ohne `work_book` → Buchangaben
  mit Vorgaben), `exchange_apply` (`new_cover`, `merge_cover`: lokal gewinnt je Art, Grabstein → „hier
  gelöscht“; `merge_sheet`; Satzansicht aus der Datei erst nach `SheetSettings::normalized`, sonst „Nicht
  übernommen“). Das Beispielwerk `sample.rs` trägt drei Blöcke, Adressen, Einstellungen der Satzansicht und einen Grabstein
  `umschlag-block`.
- **Tests:** Store `tests/cover_blocks.rs`, `exchange_apply.rs`, `exchange_dump.rs` (Rundweg im Store,
  Abgleich), Austausch `tests/{read,write,xsd}.rs`, Server `tests/round27_daten.rs` (RPC, Commits, MCP-Antrag bis
  Bestätigung/Veralten/Ablehnung, zwei Instanzen mit XSD-Prüfung).

### 27b – Satz (umschlag-satz) {#27b--typesetting-umschlag-satz}

- **Blöcke in Typst** (`baruch_typst::cover`): bei nicht leerem `layout` setzen `frame_parts` und `wrap_source` für
  jeden sichtbaren Block mit Inhalt `place(top + left, dx, dy, block(width, {set text(size, fill, weight|style, font);
  set par(leading, justify, first-line-indent: 0pt, …); set align(…); Inhalt}))`; `dx/dy` = Beschnittkante der Seite
  (`CoverGeometry::face_origin`, im Buch die Seite selbst mit Rand 0) + (x, y), mit Vorzeichen. Rücken:
  `rotate(±90deg, reflow: true, …)`, die linke obere Ecke des gedrehten Rahmens bei (x, y). Schatten =
  `fill: black.transparentize(50%)` mit `outset`, Verlauf = senkrechter Verlauf 0 → 35 % → 0 % Schwarz; `outset` ändert
  die Position nicht. Hintergründe (Bild, dunkle Flächen) bleiben; der alte Titelblock, die alte Rückseite und das Rücken-
  etikett entfallen. Nutzertexte und Schriftnamen nur über `typst_string`; Farben nur `white`, `DARK` oder
  `rgb("#rrggbb")` nach der Prüfung.
- **Vorgabeblöcke** (`cover_layout::default_blocks`): ahmen das alte Bild nach (Autor/Titel/Untertitel bei 10 % Rand im
  Titelbereich, Rückentext ab 6 mm mit 0,45 × Rückenbreite, höchstens 11 pt; Rückseite mit 14 mm Rand, Strichcode unten
  rechts), alle Werte innerhalb der CHECKs von 0045. `effective_blocks` mischt gespeicherte und Vorgabeblöcke. `wrap_source` ergänzt
  fehlende Arten selbst (es kennt die Maße), `frame_parts` setzt nur die Blöcke, die der Server übergibt – der
  Server vervollständigt sie vorher mit der **Seitengröße der Werkvorlage** (Vorlage plus `#hide[x]` gesetzt,
  erste Seite gemessen, je Stand zwischengespeichert), weil der Rahmen vor dem Buchsatz entsteht.
- **Byte-identisch ohne Layout**: goldene Stände aus dem Stand S1 vor der ersten Änderung – `frame_parts`/
  `wrap_source` in `baruch-typst/tests/data/cover_r26/` (zehn und sechs Fälle), Stand des Satzquelltexts und
  SHA-256 des Umschlag-PDFs für sechs Fälle in `baruch-server/tests/round27_satz.rs` (keiner, Vorspann, beides, Umschlag,
  Rücken unter/über 6 mm).
- **Höhen** (`server::cover::block_heights`): `cover::block_measure_source` setzt je Block eine Seite
  `width = Blockbreite, height: auto, margin: 0pt`; die Seitenhöhen (`baruch_render::page_sizes_pt`, aus dem SVG-
  Kopf) sind die Blockhöhen in Leserichtung. Blöcke ohne Inhalt: 0. Gemessen mit den aufgelösten Schriften.
- **Schriften** (`baruch_render::font_families`): `FontStore` merkt sich die Zahl der eingebetteten Schriften (sie kommen zuerst);
  je Familie `normal` (aufrecht < 600), `bold` (aufrecht ≥ 600), `italic` (kursiv/schräg).
  `server::cover::resolve_fonts` ersetzt eine fehlende Schrift: im Buch durch nichts (die Vorlage gilt), im Umschlag-
  PDF und beim Messen durch die Schrift der Werkvorlage (`baruch_wysiwyg::template_font`: erste Schrift aus dem letzten
  `#set text(font: …)` der obersten Ebene wie bei `sheet_style` und in Typst; Kommentare, Raw-Text, `#show …: set text(…)`,
  `#show: …`-Rümpfe und Funktionsrümpfe zählen nicht), falls vorhanden. So stehen Blöcke in beiden PDFs in derselben Schrift.
- **Server** (`cover.rs`, Bereich Satz): `build_wrap` = `build_wrap_for(None)` = gespeichertes Layout; das Umschlag-PDF und
  `cover_source` nutzen es. `frame` liest `cover_layout` und `template_functions` der Werkvorlage.
- **Vorspann aus der Werkvorlage**: `template_functions` (Erkennung wie `ribbon::defined_functions`, eine eigene Kopie,
  nur `#let name(` mit Klammer). Mit mindestens einer Funktion (und eingeschaltetem Vorspann) schreibt
  `frame_parts` einmal `book_dict` nach `FRAME_MARK` (`#let baruch_buch = (…)`, 23 Schlüssel nach Vertrag 1.6 plus
  `impressum_zeilen`, nur Zeichenkettenliterale; der Test liest das Wörterbuch zeilenweise und erlaubt nur Literale) und
  setzt `plain_page("", "name(baruch_buch)")` an die Stelle der eingebauten Seite. `imprint_template()` (vier Funktionen, die
  dasselbe setzen wie die eingebauten Seiten – Text je Seite mit `pdftotext` verglichen) und `imprint_template_missing()`
  (hängt nur Fehlendes an, sonst `IMPRINT_COMPLETE`).
- **Impressum**: `imprint_lines` nimmt die Adressen nur, wenn sie gesetzt sind (Verlag nach der ©-Zeile, Druckerei nach
  „Druck:“) – ohne Adressen byte-identisch.
- **Druckprüfung** (`server::cover_print`): `cover_checks` nur mit einem eigenen Layout (gespeichert oder Vorschau), Rechtecke
  aus Position und gemessener Höhe (Rücken gedreht), Codes `bleed`, `safety`, `barcode`, `spine`, `contrast` (mittlere
  sRGB-Farbe aus 40 × 40 Stichproben im Rasterbild, Illustration `fit: "cover"` auf der Vorderseite einschließlich Beschnitt wie in
  `wrap_source`, Abdunkelung 50 % oder 30 %, WCAG-Verhältnis < 3), `font`; dazu `imprint_checks` (nur mit Vorspann). Die
  Satzseite zeigt die Impressumswarnungen in `#druckpruefung` (`typeset::imprint_warnings`).
- **Position gemessen** (`baruch-render/tests/cover_layout.rs`): Titel und Klappentext mit `pdftotext -bbox` in Buch- und
  Umschlag-PDF, Abweichung relativ zur Beschnittkante ≤ 0,5 mm; Rücken `down`/`up` (Reihenfolge der Wörter von oben oder unten,
  Wörter im Rücken). Mit gespeichertem Layout über den Server misst es die Abnahme
  (`tests/round27_abnahme.rs`).

### 27c – Editor (umschlag-widget) {#27c--editor-umschlag-widget}

- **Seite** `GET /werke/{id}/umschlag` (`src/cover.rs`, Bereiche „Seiten“ und „Umschlageditor“): `editor_state` holt
  `cover_layout::layout_view` (gespeichert) und, mit Query-Feldern `b.<kind>.<feld>`, eine zweite Ansicht mit der Vorschau obenauf
  (`blocks_from_fields`: für jede genannte Art der wirksame Block, die Felder darauf angewandt, geprüft wie im Store
  mit `check_cover_block`); der Satz der Vorschau läuft über `build_wrap_for` mit dem gespeicherten Layout,
  je Art durch die Vorschau ersetzt (wie in `layout_view`), das SVG darunter trägt dieselbe Query. Nichts wird gespeichert.
- **Position** (`block_box`, Vertrag 1.2): Rahmen im ganzen Umschlag = Ursprung der Seite
  (`CoverGeometry::face_origin`) + (x, y); Breite × Höhe = `width_mm` × `height_mm`, auf dem Rücken gedreht
  (`height_mm` × `width_mm`). Die Vorlage bekommt alle Positionen als Prozent der Umschlaggröße, die Seite braucht ohne JavaScript also keine
  Rechnung im Browser. **Hilfslinien** (`guides`): Beschnitt (Außenkante), Beschnittkante, Sicherheitsrand
  je Seite (nur wo die Seite breiter als der doppelte Rand ist), zwei Rückenkanten, Mitte von Vorder- und Rückseite und der
  Höhe.
- **Formulare** (`cover::writes()`, hinter `guard::same_origin`): `POST /werke/umschlag/anordnung` (`layout_act`:
  `save` → `cover_layout::layout_set` mit allen Blöcken des Formulars, `quick` → `layout_reset([front], area)`,
  `reset` → `layout_reset(faces)`) und `POST /werke/umschlag/impressum` (`imprint_act` → `imprint_adopt`). Erfolg →
  303 auf `safe_back` mit `ok=…` (vor einem Anker, `with_code`); die Umschlagseite liest `hinweis` und `ok`. Ungültig →
  422 und dieselbe Seite mit der Eingabe (Zahlen mit Punkt, unlesbare Werte wie eingegeben) und der Meldung.
- **RPC** im Teil `call`: `cover.imprint_adopt` (Vorlage lesen, `imprint_template_missing`, dann
  `works::call("work.update", {template})` – Sperre, Vorlagenrevision und Commit bleiben dort); `work.cover_pdf` hängt
  `checks` aus `cover_print::cover_checks(work, None)` an; `work.meta.set` reicht `publisher_address` und
  `printer_address` an den Store durch (`field_label` für Commit und Benutzeroberfläche).
- **Widget** `web/src/cover_editor.ts` (`data-widget="cover-editor"`): liest `data-layout` (JSON von `cover.layout`),
  nimmt die vom Server gezeichneten Rahmen und Felder und hält beide gleich (`place`). Ziehen mit Pointer-Capture,
  Breite über die Griffe `start`/`end`, Einrasten (`snapDelta`, SNAP_MM 1,5 mm) an Beschnittkante, Sicherheitsrand und Mitte
  der eigenen Seite, Tastatur (STEP_MM 0,5, mit Umschalt 5; Strg+Pfeil Breite; Strg+Z, Strg+S), Rückgängig als Stapel von Block-
  zuständen, „Hell/Dunkel“ für den gewählten Block, verzögerte Vorschau über `umschlag.svg?b.…`, Speichern über `cover.layout_set`
  nur mit den geänderten Blöcken (ganze Blöcke ohne `stored`/`height_mm`), Schnellwahl und Zurücksetzen über
  `cover.layout_reset` mit Neuladen. Rahmen tragen `aria-label` (Art, Seite, Werte), die Statuszeile ist `role=status`.
  Stil in `static/cover.css` nur mit den Farbtoken aus `app.css` (hell und dunkel).
- **Test** `tests/round27_widget.rs`: Seite, Vorschau per GET (Store unverändert), Formulare (save/quick/reset, 422, 403),
  Adressen, Impressumsknopf, `checks`; das Widget läuft in einer eigenen Node-Attrappe des DOM auf dem HTML der Seite
  (Ziehen, Griff, Pfeiltasten, Einrasten, Rückgängig, hell/dunkel, Feld, Speichern). Seit dem Merge prüfen die Tests nur die
  Wirkung; ein Platzhalter „noch nicht umgesetzt“ ließe sie scheitern.

### Integration und Abnahme (Integrator) {#integration-and-acceptance-integrator}

- Mit `--no-ff` gemergt in der Reihenfolge a → b → c ohne Konflikte. Aktualisiert: die Buchvorlage in `baruch-store` (Unit-
  Test in `works.rs`, `tests/works.rs`: neue Bücher tragen `imprint_template()` und kennen alle vier Vorspann-
  Funktionen), `round27_daten.rs` (ein Block ohne Inhalt hat die gemessene Höhe 0), die Platzhalterzweige in
  `round27_widget.rs` (entfernt) und der Commit-Rumpf beim Einlesen eines Austauschs (`mirror::describe_exchange` zählt die
  Umschlagblöcke mit). Nach der Sichtprüfung: Rahmen in `static/cover.css` jetzt nur noch schwach getönt (der markierte Block
  verdeckte einen hellen Titel auf dunklem Bild), und die Liste der Druckprüfung in `templates/cover.html` nennt den
  Block nicht doppelt (die Meldungen beginnen schon damit).
- **Abnahme** `tests/round27_abnahme.rs`: ein erfundenes Werk (148 × 210 mm) mit einem PNG-Umschlag (dunkles oberes Drittel,
  im Test erzeugt) und vollständigen Buchangaben mit Typst-Sonderzeichen. Titel ins obere Drittel, Autor
  nach unten, Klappentext verschoben und schmaler, freier Text auf der Vorderseite: die Vorschau per GET speichert nichts; das Formular speichert dasselbe wie
  `cover.layout_set` an einer zweiten Instanz; Vorschau-SVG und Satzquelltext der Vorschau (`baruch_render::stand`)
  gleichen dem gespeicherten Umschlag; `pdftotext -bbox` misst Titel, freien Text und Klappentext in Umschlag- und Buch-PDF am selben
  Ort relativ zur Beschnittkante (≤ 0,5 mm). Druckprüfung (Beschnitt, Strichcode, Kontrast, fehlende Schrift mit Rückfall) auf
  der Seite ohne JavaScript und in `work.cover_pdf`. Schnellwahl oben/Mitte/unten und Zurücksetzen mit Grabsteinen;
  danach setzen Buch und Umschlag byte-identisch wie vor dem ersten Layout. Ein eigenes `impressum(buch)` in der Werk-
  vorlage auf der Rückseite des Haupttitels mit den Buchangaben wörtlich; der Knopf fügt die anderen drei Funktionen hinzu
  (eine Vorlagenrevision, ein Commit) und lehnt danach ab; eine fehlende Druckerei wird gewarnt. MCP liest und
  stellt nur einen Antrag; der Austausch (`.baruch`) trägt Layout, Adressen und Einstellungen der Satzansicht an eine zweite Instanz,
  die den Umschlag aus demselben Quelltext setzt.
