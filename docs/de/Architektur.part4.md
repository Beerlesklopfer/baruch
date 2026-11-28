<!-- translation-of: docs/Architektur.md source-commit: 4d3148c part: 4 -->

## Runde 32 – Word und Beenden {#round-32--word-and-quitting}

Neues Crate `baruch-docx` (rein, ohne Datenbank und Netz; `zip` 9 nur mit `deflate`, `quick-xml` 0.42) mit dem
gemeinsamen Modell (`model.rs`), Fehlern, Grenzen (`limits.rs`) und den festen Namen von OOXML (`ooxml.rs`). Leser
(`read`, `resolve`, `to_typst`) und Schreiber (`from_typst`, `diff`, `write`) stehen in getrennten Dateien; beide kennen nur
das Modell. Im Server stehen `docx_import/`, `docx_export/` und `shutdown.rs`. Keine Migration.

### Runde 32 – Word-Import (docx_import) {#round-32--word-import-docx_import}

**Crate `baruch-docx`, Leseseite** (rein, ohne Datenbank und Netz):

- `read::read(bytes, &Limits) -> ReadDocx { document, warnings }` – `read/pack.rs` öffnet das ZIP (Größe der Datei, Zahl der
  Einträge, angegebene Größen vor dem Entpacken; jeder Eintrag über `take(Grenze + 1)`, auch die Summe des wirklich
  Entpackten wird gezählt; Namen mit `..`, führendem `/`, Backslash oder Laufwerk → `Unsafe`; OLE-Anfang → `NotDocx`
  „altes .doc oder verschlüsselt“) und löst Beziehungen auf (ein Ziel außerhalb des Archivs ohne `TargetMode="External"` →
  `Unsafe`; externe Ziele werden nie abgerufen, ein nur verlinktes Bild ergibt `docx_bild_extern`). `read/xml.rs` baut je Teil
  einen kleinen Baum mit quick-xml: DTD → `Unsafe`, nur die fünf festen Entitäten und Zeichenverweise werden aufgelöst,
  Tiefe und Zahl der Elemente begrenzt. Elemente heißen wie in der Datei (`w:p`) – Annahme: die üblichen Präfixe.
  `read/body.rs` bildet den Textkörper auf das Modell ab: Rolle der Absatzvorlage (ID `Heading N`, Name "heading
  N"/„Überschrift N“, `basedOn`, `w:outlineLvl` in Vorlage oder Absatz, die Baruch-Vorlagen aus `ooxml.rs`), Läufe mit
  Zeichenformaten, `w:ins`/`w:del`/`w:moveFrom`/`w:moveTo` als `Inline::Tracked`, Änderung der Absatzmarke als `mark`,
  Fuß- und Endnoten inline, Kommentarbereiche, Links, Bilder (DrawingML und VML; Medientyp aus den ersten Bytes), Listen
  über `numbering.xml`, Tabellen, Typst-Inseln (`w:sdt` mit `w:tag="baruch-typst"`: der mit `w:vanish` verborgene Text ist
  die Quelle, Absätze sind Zeilen). Warnungen einmal je Code mit der Zahl der Stellen.
- `resolve::resolve(document, View)` löst die Änderungsverfolgung auf (`Original`, `Final`, `FinalBy(name)`); eine eingefügte
  Absatzmarke, die nicht angewendet ist, führt den Absatz mit dem folgenden zusammen, eine angewendete gelöschte lässt ihn
  den folgenden aufnehmen. `change_authors` nennt die Bearbeiter in der Reihenfolge ihres ersten Auftretens.
  `proposal_view(document, name)` ist `FinalBy` mit den Überschriften des Originals und liefert `HeadingChange` (`before`
  leer = eingefügt, `after` leer = gelöscht; `section` zählt wie `to_typst` am Original, Vorspann vor der ersten Überschrift
  ist Abschnitt 0).
- `to_typst::to_typst(document) -> TypstOut { sections, warnings }` schreibt je Abschnitt Typst in den Formen des
  Markdown-Imports (Text maskiert, Auszeichnung als Funktionen, `#quote(block: true, attribution: […])[…]`, Listen mit
  `-`/`+`, `#table(…)`, `#figure(image("…", alt: "…"), caption: […])`) und sammelt je Abschnitt die Kommentaranker
  (kommentierter Wortlaut). Text vor der ersten Überschrift wird ein Abschnitt mit leerem Titel.

**Server `docx_import`** (`src/docx_import/mod.rs`, `draft.rs`, `pages.rs`, Template `docx_import.html`):

- `prepare(bytes) -> Prepared` (rein, in `spawn_blocking`): liest, schreibt das Original als Abschnitte, legt den
  Vorschlagsstand jedes Bearbeiters auf die Abschnitte des Originals (nur abweichende Abschnitte zählen), ordnet Kommentare
  ihren Abschnitten zu (zuerst am Original, sonst am Stand eines Bearbeiters) und sammelt die Warnungen.
- `preview` speichert den Entwurf (`draft.rs`, aufbewahrt wie `pdf_import/draft.rs`: 32 Hex, 20 Stück, 24 Stunden); `apply`
  liest die gespeicherte Datei neu und ruft `apply_prepared`:
  1. Werk über `import::import_generated` (ein Commit „Import aus <datei>“, Bilder als `ImportAsset`),
  2. unter der Schreibsperre je Bearbeiter und Abschnitt `Store::add_proposal(…, "@word:<name>", "word", Notiz)` – der Text
     ist die aktuelle Fassung mit ersetztem Abschnittstext, die Überschriftzeile bleibt wörtlich –, danach
     `mirror::side_refs_after_proposal` (Seitenzweige `refs/revisions/<key>/<n>` mit Persona und Modell),
  3. Beiträge über `Store::add_post` (geänderte Überschriften, Kommentare mit `TextRef` auf Fassung 1, Antworten direkt nach
     ihrem Kommentar).
  Nie `set_current`: Fassung 1 bleibt die aktuelle. Ein Fehler in 2 oder 3 wird als Warnung gemeldet, das Werk bleibt.
- `cli_import` teilt `prepare`/`apply_prepared`; ohne `--uebernehmen` wird nichts geschrieben.

**Annahmen (ohne echtes Word geprüft):** Die Tests bauen ihre Word-Dateien selbst nach ECMA-376 und `ooxml.rs`; eine von
Microsoft Word oder LibreOffice geschriebene Datei wurde nicht gelesen. Angenommen insbesondere: Präfixe wie oben; Antworten
auf Kommentare über `commentsExtended.xml` (`w15:paraIdParent` ↔ `w14:paraId` des Kommentarabsatzes); die Träger der
Typst-Insel wie in `ooxml.rs` beschrieben (mehrzeilige Quelle als mehrere verborgene Absätze oder mit `w:br`).

### Word-Export (`baruch-docx`-Schreiber, `baruch-server/src/docx_export`) {#word-export-baruch-docx-writer-baruch-serversrcdocx_export}

Der Weg vom Werk zur Datei hat drei reine Stufen im Crate `baruch-docx` und eine lesende Stufe im Server:

1. **`from_typst(body, image_paths)`** (`from_typst.rs`, `from_typst/args.rs`): liest einen Abschnitt mit
   `baruch_wysiwyg::parse` und bildet die Blöcke auf das Modell ab. Abbildungen (`#figure(image(…))`, `#image(…)`) werden zu
   `Block::Figure` mit wörtlichem `raw`, einfache Tabellen zu `Block::Table`, `#pagebreak()` zu `Block::PageBreak`. Alles
   andere wird `Block::Typst`/`Inline::Typst` mit wörtlicher Quelle und sichtbarem Ersatz (Warnung `docx_typst_insel`);
   Unsichtbares (Marke, Registermarke, Kommentar) geht ohne Warnung als Insel ohne etwas Sichtbares mit, Querverweise mit
   ihrem Text.
2. **`track_changes(current, proposed, change)`** (`diff.rs`): gleicht die Blöcke über die längste gemeinsame Teilfolge ab,
   paart in jeder Lücke ähnliche Blöcke gleicher Art (Anteil gemeinsamer Wörter) und vergleicht sie Wort für Wort; die
   übrigen Blöcke sind ganz neu oder ganz entfallen (`mark` an der Absatzmarke, Inhalt in `Inline::Tracked`). Zusage:
   Einfügungen behalten und Löschungen entfernt = Vorschlag, umgekehrt = aktuelle Fassung (Ausnahmen siehe Kopf von
   `diff.rs`: leeres Tabellenraster, Seitenumbrüche).
3. **`write(document, options)`** (`write.rs`, `write/body.rs`, `write/styles.rs`, `write/image.rs`): schreibt das Archiv in
   fester Reihenfolge mit festen ZIP-Zeiten (deterministisch): `[Content_Types].xml`, `_rels/.rels`, `docProps/app.xml`,
   `docProps/core.xml`, `word/document.xml` einschließlich Beziehungen, `styles.xml`, `settings.xml`, `numbering.xml`,
   `footnotes.xml` (mit Verweisen in Fußnoten mit eigenen Beziehungen), `comments.xml` (nur mit Kommentaren), Bilder unter
   `word/media/imageN.<endung>`. In XML verbotene Zeichen werden durch U+FFFD ersetzt.
4. **`docx_export::build`** (Server): liest Werkangaben, alle Fassungen (`Store::load_work`, Zeitpunkte aus
   `Store::revision_tree`), Bilder (`Store::export_assets`, nur PNG/JPEG/GIF) und – bei Vorschlägen – die Marken „bis
   hierhin“ (`Store::work_revision_folds`). Offen ist ein Vorschlag mit Status „vorgeschlagen“, der nicht von
   `baruch_core::revision_fold::is_hidden` ausgeblendet ist. Es gibt nur lesende Zugriffe; das Packen läuft in
   `spawn_blocking`.

Entscheidungen des Schreibers über `ooxml.rs` hinaus (für den Leser und die Rundreise):

- Der Pfad eines Bildes im Werk steht in `wp:docPr name`, der Alternativtext in `descr`.
- Eine Abbildung mit `raw` steht in einem Inhaltssteuerelement `baruch-typst` (verborgene Quelle, dann Bild und
  Bildunterschrift) – für den Leser also eine Insel mit `raw`, die beim Zurücklesen die wörtliche Quelle ergibt.
- Geordnete Listen: Die erste, die bei 1 beginnt, nutzt `NUM_DECIMAL`, jede weitere ein eigenes `w:num` (ab 3) auf derselben
  abstrakten Nummerierung mit `w:startOverride` – sonst zählte Word über alle Listen des Buchs weiter.
- Verfolgte Änderungen stehen um jeden Lauf einzeln (`w:ins`/`w:del`), eine verfolgte Insel als `w:ins`/`w:del` um das
  `w:sdt`. Die Absatzmarke der Zitatquelle und der Bildunterschrift gilt als verfolgt, wenn ihr ganzer Inhalt eine Änderung
  derselben Art ist.
- Fußnotentext beginnt wie in Word mit einem Leerzeichen nach der Fußnotenmarke.
- Der Untertitel steht zusätzlich in `dc:subject`, das Impressum in `dc:description`.

### Beenden (`shutdown.rs`) {#quitting-shutdownrs}

- `crates/baruch-server/src/shutdown.rs` – Beenden (Runde 32).
  - `Shutdown` (in `AppState::shutdown`): Auslöser auf einem `watch`-Kanal; einmal angefordert, bleibt es angefordert.
  - `own_machine(hosts, peer, headers)`: nur wahr, wenn der gegenüberliegende Rechner (`ConnectInfo`) bekannt und Loopback ist
    (`to_canonical`, damit `::ffff:127.0.0.1` zählt), `guard::via_proxy` nein sagt und `Host` fehlt oder
    `guard::allowed_host` erfüllt. `guard.rs` blieb unberührt.
  - `serve` → `serve_until(listener, state, grace, signal)`: `axum::serve(…)` mit
    `into_make_service_with_connect_info::<SocketAddr>()` und `with_graceful_shutdown` auf das Erste von Signal
    (`shutdown/signals.rs`) und `Shutdown::requested()`. Danach nimmt der Server keine Verbindungen mehr an, ruhende
    Verbindungen schließen, laufende Anfragen werden beantwortet. Eine zweite Future wartet ab dem Auslöser die Karenzzeit
    (`GRACE` = 10 s) ab; ist dann noch eine Verbindung offen, wartet sie nicht länger (Meldung auf stderr). Zuletzt
    `store.pool().close()` (höchstens `CLOSE_WAIT` = 5 s): SQLite schreibt das Protokoll (`-wal`) zurück. `serve` schreibt
    dann „Baruch beendet.“; `main` kehrt zurück, Status 0.
  - `shutdown/signals.rs` (nur `tokio`, allein übersetzbar): Strg+C überall; Unix `SIGTERM`; Windows `ctrl_break`,
    `ctrl_close`, `ctrl_shutdown` – die letzten beiden mit der Karenzzeit `WINDOWS_CLOSE_GRACE` = 3 s, weil Windows den
    Prozess kurz nach dem Ereignis selbst beendet. Kein `SIGHUP` (ein Empfänger bräche `nohup` ab).
  - Seiten: eine eigene Template-Umgebung (`base.html`, `shutdown.html`, `shutdown_done.html`). Die Schlussseite ist
    in sich geschlossen (Stil inline, das einzige Skript `window.close()`), weil der Server danach nichts mehr ausliefert.
    `POST /beenden` antwortet zuerst und löst `REPLY_DELAY` = 500 ms später aus.
  - Kein Widget, keine `.ts`-Datei: Das Inline-Skript der Schlussseite genügt.
- Tests: `src/shutdown.rs` (Auslöser, Tabelle für `own_machine`), `tests/round32_beenden.rs` (Seiten auf dem echten Server im
  Testprozess, echter Prozess mit Schaltfläche/SIGTERM/Strg+C, Starter mit Attrappen in einem geschlossenen `PATH`,
  Einheiten, Abläufe).
- CI: `pages.yml` und `release.yml` trennen `pages-build` und `pages-deploy`; Regel 11 in `scripts/ci-check-workflows.sh`,
  Gegenproben in `scripts/ci-selftest.sh`.

Bekannte Grenze: Läuft beim Beenden noch ein blockierender Auftrag (`spawn_blocking`, etwa ein Typst-Satz), wartet die
Tokio-Laufzeit beim Verlassen von `main` darauf – das Ende kann sich dann über die Karenzzeit hinaus verzögern. Die
Datenbank ist zu diesem Zeitpunkt schon geschlossen. Nicht durch einen Test belegt.

### Integration (Runde 32) {#integration-round-32}

- **Naht Leser/Schreiber:** Beide Seiten entstanden getrennt gegen `model.rs` und `ooxml.rs`. Die Rundreise
  (`crates/baruch-docx/tests/roundtrip.rs` am Modell, `tests/round32_abnahme.rs` durch den Server) fand drei Lücken, alle im
  Leser oder in `to_typst` behoben:
  - `w:lvlOverride`/`w:startOverride` an `w:num` zählt als erste Nummer der Liste (`read.rs`, `Numbering::overrides`) – so
    schreibt der Schreiber jede weitere nummerierte Liste, und so startet auch Word eine Liste neu.
  - Eine nummerierte Liste, die nicht bei 1 beginnt, trägt ihre Nummer in `to_typst` an jedem Punkt („3.“, „4.“ …,
    verschachtelt ab 1). Typst versteht "3." gefolgt von "+", aber `baruch-wysiwyg` läse es als zwei Listen.
  - Der Pfad eines Bildes im Werk kommt aus `wp:docPr name` zurück, wenn der Name wie ein Pfad aussieht (`read/body.rs`,
    `work_path`: relativ, ohne `..`, Endung passend zum Inhalt); Bilder im sichtbaren Teil einer Typst-Insel werden auch
    gelesen (`island_images`), weil die Quelle der Insel sie nennt.
- **Signale vor „Baruch läuft“:** `signals::system_signal()` richtet die Empfänger (Unix: `SIGINT`, `SIGTERM`) beim Aufruf
  ein, nicht erst beim ersten Warten; `main` ruft es vor der Zeile „Baruch läuft“ auf und gibt die Future an
  `shutdown::serve_with(listener, state, signal)` (`serve` = `serve_with` mit `system_signal()`). Vorher beendete ein
  `SIGTERM` unmittelbar nach dieser Zeile den Prozess hart (Test `sigterm_ends_the_throwaway_server_with_exit_0`).
- **`main.rs`:** Nach einem erfolgreichen `shutdown::serve_with` endet `main` mit `std::process::exit(0)`, damit ein noch
  laufender blockierender Auftrag das Ende nicht über die Karenzzeit hinaus verzögert (die Datenbank ist dann schon
  geschlossen, die Schlusszeile gedruckt).
- **Wächter `round33_seiten`:** Die drei neuen Templates (`docx_import.html`, `docx_export.html`, `shutdown.html`) stehen in
  `TEMPLATES`.
- **Offen:** Der Schreiber schreibt keine `commentsExtended.xml` (Antworten stehen als eigene Kommentare, der Server schreibt
  ohnehin keine); die privaten Konstanten des Schreibers (`NS_APP`, `PART_FOOTNOTES_RELS`, `STYLE_HYPERLINK`,
  `STYLE_COMMENT_TEXT`, `STYLE_COMMENT_REFERENCE`) sind nicht nach `ooxml.rs` verschoben; Zitate im Text stehen in Word ohne
  Anführungszeichen; SVG und WebP werden nicht eingebettet.

## Runde 34 – Mindmap {#round-34--mind-map}

Die Einfallskarte (Runde 25) ist zu einer Mindmap wie in XMind geworden. Drei Zweige (`team/mindmap_daten`,
`team/mindmap_widget`, `team/mindmap_austausch`) auf der Schnittstelle R34-S1, mit `--no-ff` ohne Textkonflikte
zusammengeführt; die Naht im Inhalt war die **Wolke** (siehe „Integration“).

### Wolke und Mindmap: eine Regel mit vier Plätzen {#cloud-and-mind-map-one-rule-with-four-places}

Der Vertrag der Runde ließ Einfälle ohne Elternknoten und ohne Bezug direkt am Werk als „freie Hauptthemen“ hängen; die Wolke
„Ohne Kapitel“ wäre entfallen. Nach dem Zusatz des Nutzers vom 2026-11-25 („Es macht Sinn eine Verbindung zwischen der Wolke
und der Mindmap zu machen“) gilt **eine Regel mit vier Plätzen**:

1. `parent` gesetzt → Kindknoten dieses Einfalls (gleiches Werk, kein Zyklus, Kette höchstens 32 tief). Der Bezug (`key`)
   bleibt daneben erlaubt und ist dann nur die Verbindung zum Abschnitt.
2. sonst `key` gesetzt → Kind des Ankers (nächster Vorfahr der Art Kapitel, sonst der Punkt selbst).
3. sonst `main_topic` → **freies Hauptthema** direkt am Werk.
4. sonst → **in der Wolke**: nicht im Baum, sondern in der Ablage neben der Mindmap.

Dafür trägt `idea` eine vierte neue Spalte `main_topic` (0/1, Standard 0); der Vertrag sah drei vor. Ohne sie ließe sich ein
Einfall, den man auf das Werk zieht, nicht von einem Einfall ohne Platz unterscheiden. Folgen:

- **Vorhandene Einfälle** ohne Bezug (heute die Wolke „Ohne Kapitel“) liegen nach der Migration in der Wolke; Einfälle mit
  Bezug hängen wie zuvor an ihrem Kapitel. Nichts wird zum Hauptast, ohne dass es jemand dorthin zieht.
- **Neue Einfälle ohne Platz** (Schnellerfassung Alt+E, Formular, RPC `idea.create` und MCP `idea_create` ohne
  `parent`/`key`) landen in der Wolke.
- **Aus der Wolke in die Mindmap:** `idea.move` mit `parent` (auf einen Einfall), mit `parent: null, key` (auf ein Kapitel)
  oder mit `parent: null, key: null` oder `cloud: false` (auf das Werk, wird ein Hauptthema).
- **Aus der Mindmap in die Wolke:** `idea.move` mit `cloud: true`. Der Einfall verliert Elternknoten und Bezug; **seine
  Kindknoten bleiben bei ihm** – der Teilbaum zieht als Ganzes in die Wolke. Grund: Kein Einfall verliert still seinen
  Zusammenhang, die Aktion lässt sich mit einem `idea.move` zurück rückgängig machen (Strg+Z des Widgets), und „nur er“
  hieße, seine Kinder ungefragt an den Großelternknoten zu hängen. Wer nur den einen Einfall lösen will, zieht die
  Kindknoten vorher woandershin.
- **Zähler:** `cloud.count` („n in der Wolke“) zählt die obersten Einfälle der Wolke ohne Filter, `cloud.count_with_children`
  einschließlich ihrer Kindknoten.
- **Nähe als Hilfe:** `cloud.hints` nennt je Einfall der Wolke höchstens einen Vorschlag „passt zu Ast …“ – der nächste
  Einfall in der Mindmap (gespeicherte Einbettungen und gleiche Stichwörter wie auf der Karte, nie eine neue Einbettung) und
  der Hauptast, in dem er hängt. Die Vorschläge werden auch berechnet, wenn der Filter die Nähelinien im Bild abschaltet.
- **Wiederherstellen** eines Einfalls, dessen Elternknoten noch gelöscht ist: Er bleibt an seinem Bezug oder liegt in der
  Wolke (`detached: true`), nicht als Hauptthema am Werk.
- **Baumimport** (`tree::import` ohne `parent`): Die obersten Knoten werden freie Hauptthemen, nicht Einfälle der Wolke – wer
  eine Mindmap einliest, will sie in der Mindmap sehen.

### Daten (Migration 0052, Store) {#data-migration-0052-store}

Migration `0052_idea_tree.sql` (nur `ADD COLUMN`, nicht in `FOREIGN_KEYS_OFF`):

```sql
ALTER TABLE idea ADD COLUMN title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200);
ALTER TABLE idea ADD COLUMN parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL;
ALTER TABLE idea ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
ALTER TABLE idea ADD COLUMN main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1));
CREATE INDEX idea_parent ON idea(parent_id, position);
```

Regeln im Store (`crates/baruch-store/src/ideas.rs`):

- **Titel:** höchstens 200 Zeichen, eine Zeile, darf leer sein. Ein Einfall braucht einen Titel **oder** mindestens ein
  Stichwort. Angezeigter Titel (`Idea::display_title`): der Titel, sonst die Stichwörter "a, b", sonst der Anfang des
  Gedankens, sonst „Einfall <id>“ – nie leer. Der Titel steht im Volltextindex (`idea_fts`, Spalte `words`).
- **Elternknoten:** ein nicht gelöschter Einfall desselben Werks; kein Zyklus; Kette höchstens 32 Einfälle
  (`MAX_IDEA_DEPTH`). Einfälle ohne Werk haben keinen Elternknoten.
- **Reihenfolge:** `position`, dann `id`, unter Geschwistern (gleicher Elternknoten oder alle Einfälle des Werks ohne
  Elternknoten). Neue kommen ans Ende; „vor einem Geschwister“ (`before`) nummeriert die Geschwister lückenlos ab 1 neu, ohne
  ihr `updated` anzufassen. Vorhandene Einfälle tragen 0 und ordnen sich nach `id`; solange unter einem Elternknoten nichts
  geordnet wurde, behalten neue ebenfalls die 0 (im Austausch dann ohne `platz` – ein Werk ohne geordneten Baum schreibt
  dieselbe Datei wie vor Runde 34 und besteht auch das eingefrorene u8-Schema).
- **Löschen** nimmt die nicht gelöschten Nachfahren mit **demselben Zeitstempel** mit; **Wiederherstellen** bringt den
  Einfall und genau die Nachfahren mit diesem Zeitstempel zurück (was früher einzeln gelöscht wurde, bleibt gelöscht).
  Verknüpfungen (`idea_link`) und Positionen (`idea_map_pos`) bleiben währenddessen gespeichert.
- **Werkwechsel** (`idea.update` mit `work`): Der Einfall löst sich von seinem Elternknoten und liegt im neuen Werk in der
  Wolke; seine Kindknoten bleiben im alten Werk und verlieren ihren Elternknoten (kein Baum über zwei Werke).
- **Positionen** (`idea_map_pos`) bleiben gespeichert und im Austausch, haben aber in der Mindmap keine Wirkung (der Server
  berechnet das Layout aus dem Baum).

Spiegel `einfaelle.md`: Hat ein Werk mindestens einen Einfall mit Titel, Elternknoten oder Hauptthema, steht der Abschnitt
`## Baum` vor den Einträgen – die Einfälle nach Elternknoten eingerückt, Geschwister nach `position`, dann `id`, an oberster
Stelle mit dem Platz („(am Abschnitt key 12)“, „(Hauptthema)“, „(Wolke)“). Jeder Eintrag nennt zusätzlich `- Titel: …`,
`- Unterknoten von: Einfall <id>` oder `- Platz: Hauptthema am Werk`. Ohne Baum bleibt die Datei byte-identisch wie vor Runde
34.

Austauschformat: Nebenversion 9, `<einfall>` mit `titel`, `eltern`, `platz`, `hauptthema` – siehe
`docs/Austauschformat.md` ("Tree of the mind map").

### Baum, Wolke und Import im Server (`idea_map/tree.rs`) {#tree-cloud-and-import-in-the-server-idea_maptreers}

Die Signaturen aus S1.2 blieben wörtlich gleich (`MindMap`, `TreeNode`, `TreeKind`, `tree`, `ImportTree`, `import`,
Konstanten). Neu daneben:

```rust
pub struct CloudHint { pub idea: IdeaId, pub node: String, pub node_title: String,
                       pub branch: String, pub branch_title: String, pub score: f32, pub shared_words: Vec<String> }
pub struct Cloud { pub work: WorkId, pub ideas: Vec<TreeNode>, pub count: u32, pub count_with_children: u32,
                   pub hints: Vec<CloudHint>, pub links: Vec<IdeaLink> }
pub async fn cloud(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<Cloud, RpcError>;
pub async fn tree_and_cloud(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<(MindMap, Cloud), RpcError>;
```

- `tree` liefert das Werk (`w`), Teile und Kapitel (`n:<key>`) in Gliederungsreihenfolge, dann die freien Hauptthemen nach
  `position, id`; unter jedem Knoten zuerst die Gliederungspunkte, dann seine Einfälle nach `position, id`. Einfälle der
  Wolke stehen **nicht** im Baum.
- Filter `status`, `word`, `origin`: Ein Einfall, der nicht passt, bleibt als Geist (`ghost: true`, `hit: false`) stehen,
  solange ein Nachfahre passt. `q` blendet nichts aus, es setzt `hit` (Volltext, auch über den Titel).
- `MindMap::links` und `MindMap::near.pairs` nennen nur Einfälle, die im Baum stehen; `Cloud::links` die Verknüpfungen mit
  mindestens einem Ende in der Wolke. `total_ideas` zählt die gezeigten Einfälle im Baum (ohne Geister, ohne die Wolke);
  `descendants` zählt je Knoten die Einfälle, die im Baum darunter stehen.
- `Cloud::ideas` sind `TreeNode` der Art `idea` mit ihren Kindknoten in `children`; derselbe Filter gilt.
- `import` legt den Baum in einer Transaktion an (Store `idea_import_tree`) und schreibt **einen** Commit „Mindmap eingelesen:
  n Einfälle“ (`Baruch-Action: idea-import`) mit der übergebenen Herkunft. Abgelehnt (-32000, nichts angelegt, kein Commit):
  kein Knoten, mehr als `MAX_IMPORT_NODES` (2000) Knoten, tiefer als `MAX_TREE_DEPTH` (32, auch zusammen mit der Tiefe von
  `parent`), `parent` gelöscht oder in einem anderen Werk, Titel über 200 oder Notiz über 4000 Zeichen, Stichwörter über den
  Grenzen der Einfälle (höchstens 20, je 60 Zeichen), eine Verknüpfung mit einem Pfad ohne Knoten. Ein Knoten ohne Titel und
  Stichwort heißt „(ohne Titel)“; eine Verknüpfung eines Knotens mit sich selbst wird übersprungen. `ImportTree::title`
  steht im Rumpf des Commits, `warnings` werden nicht gespeichert. Wer `import` aufruft, kürzt Titel und Notizen vorher
  (`files/import.rs`); der Import kürzt nicht, er lehnt ab.

- Knoten der Mindmap sind Zeilen der Tabelle `idea` – keine zweite Struktur. Der Store kennt die Regeln des Baums
  (`idea_create_at`, `idea_move`, `idea_delete_tree`, `idea_restore_tree`, `idea_import_tree`, `idea_tree_rows`), der Server
  verbindet ihn mit der Gliederung (`idea_map::tree`), der Spiegel schreibt ihn in `einfaelle.md`.
- Jede schreibende Aktion läuft unter der Schreibsperre und ergibt genau einen Commit mit Herkunft (`idea-create`,
  `idea-update`, `idea-move`, `idea-delete`, `idea-restore`, `idea-import`).
- Die flache Karte (`idea_map::build`, RPC `idea.map`) bleibt aus Rückwärtsverträglichkeit; der Baum baut darauf auf
  (Gliederung, Filter, Verknüpfungen, Nähe, Befunde) und liest zusätzlich die Einfälle ungefiltert, um Geister und die Wolke
  zu bestimmen.

### Bild und Seite (`idea_map/bild/`) {#picture-and-page-idea_mapbild}

`bild/mod.rs` (Seite `/werke/{id}/karte`, Bild `/werke/{id}/karte/bild`, Filter), `bild/mindmap.rs` (Zeichnen),
`bild/tree_layout.rs` (Layout, rein rechnerisch und deterministisch). Ein Layout, in Rust: Bildschirm und Ausgabe (`.svg`,
`.pdf`) kommen aus derselben Funktion `mindmap::render(map, options)`; das Widget legt selbst nichts aus. `bild/layout.rs` und
`bild/svg.rs` der alten Einfallskarte sind gelöscht.

- Bühne: `<div class="mm-stage" data-widget="mindmap" data-work data-bild data-back data-zu data-wahl data-gliederung>`, darin
  `[data-mm-toolbar]`, `[data-mm-canvas]` mit `<svg class="mm-svg" data-cloud-count data-zu>` und `<g class="mm-viewport">`,
  darunter `[data-mm-status]` (Live-Region). Zähler: `[data-mm-cloud-count]`.
- Reihenfolge im Viewport: Äste, Nähe, Beziehungen, Wolke, Knoten, zuletzt die Beschriftungen der Beziehungen
  (`g.mm-link-labels`; sie liegen über den Knoten, damit nahe Knoten sie nicht verdecken – Integration).
- Knoten: `<g class="mm-node mm-<work|part|chapter|idea>" data-node data-parent data-depth data-side="right|left"
  data-branch="0..7" data-editable="1|0" data-title data-own tabindex="0">`; Einfälle `data-idea`, mit Bezug `data-key`; mit
  Nachfahren `data-children="n"`; mit offenen Befunden `data-open-findings="k"`. Zusätzliche Klassen: `mm-b<n>` (Astfarbe),
  `mm-main` (Hauptast), `mm-done`, `mm-hit`, `mm-dim`, `mm-ghost`, `mm-closed`, `mm-many-findings`. Das Werk hat weder
  `data-side` noch `data-branch`. Kinder: `<title>`, `rect.mm-box`, `text.mm-label` (in `a.mm-label-link`), `g.mm-note`,
  `a.mm-section`, `a.mm-findings[data-open-findings]`, `a.mm-toggle[data-count]`.
- Äste: `<path class="mm-edge mm-b<n>" data-from data-to>` (kubische Kurve, `stroke-width` 3.6 / 2.4 / 1.8 / 1.4).
- Beziehungen: `<g class="mm-rel">` mit `<path class="mm-link mm-link-<kind>" data-link data-from="i:<a>" data-to="i:<b>"
  data-end-from data-end-to>`; die Beschriftung ist `<text class="mm-link-label" data-link>` in `g.mm-link-labels`.
  Widerspruch zusätzlich `mm-contradiction` an Gruppe, Pfad und Text. `data-end-*` nennt den Knoten, an dem die Linie sichtbar
  endet.
- Nähe: `<path class="mm-near" data-from data-to>` (nur mit `near`, nur zwischen Knoten des Baums).
- Wolke: `<g class="mm-cloud" data-count>` mit `rect.mm-cloud-area` und `text.mm-cloud-head`; Karten sind
  `g.mm-node.mm-idea.mm-cloud-item[data-cloud="1"][data-parent="w"]` (welche Kinder der Wurzel in der Wolke liegen, sagt
  `RenderOptions::cloud`; alle anderen sind Hauptäste), mit einem Vorschlag `data-fits="<Hauptast>"` und
  `text.mm-cloud-hint.mm-b<n>`, mit Kindknoten `text.mm-cloud-below` ("+ n").
- Eingeklappte Knoten zeichnen keine Nachfahren; `a.mm-toggle` zeigt "+ n", sein `href` ist `?<Filter>&zu=<neue Liste>`. Die
  Zahl zählt alle verborgenen Knoten (jeder Art), nicht nur Einfälle.
- **Fragment** (`standalone: false`): kein `xmlns`, kein `<style>`, keine Farbattribute – Farben über `--mm-b0`…`--mm-b7` in
  `karte.css` (hell, dunkel nach System, dunkel nach Schalter). **Eigenständig** (`standalone: true`, für `.svg`/`.pdf`):
  `xmlns`, eigenes `<style>`, Farben als Attribute (`mindmap::BRANCH_COLORS`), weißer Hintergrund, keine Links (statt `<a>` ein
  `<g>` derselben Klasse).
- Das Layout ist deterministisch; Textbreiten werden geschätzt (der Server kennt die Schrift des Browsers nicht), Titel über
  48 Zeichen werden im Bild gekürzt (voller Titel in `data-title` und im Tooltip).

### Widget `web/src/mindmap.ts` {#widget-websrcmindmapts}

Ersetzt `idea_map.ts` (gelöscht). Es legt selbst nichts aus: Nach jeder Aktion holt es `data-bild` neu mit Filter und `zu=`,
setzt das SVG ein und behält Auswahl und Viewport (`transform` an `.mm-viewport`). RPC (nur diese, über `rpc.ts`):
`idea.create`, `idea.update`, `idea.move`, `idea.delete`, `idea.restore`, `idea.link`, `idea.unlink`, `idea.link_restore`.
Rückgängig ist der umkehrende RPC der letzten Aktion (Anlegen ↔ `idea.delete`, Löschen ↔ `idea.restore`, Umbenennen ↔
`idea.update` mit dem alten gespeicherten Titel aus `data-own`, Umhängen ↔ `idea.move` zurück an den alten Platz – `parent`,
`key`, `before`, für eine Karte der Wolke `cloud: true`, für ein Hauptthema `parent: null, key: null, cloud: false` –,
Verknüpfen ↔ `idea.unlink`, Lösen ↔ `idea.link_restore`). Schaltflächen im Band tragen `data-mm-action` (`child`, `sibling`,
`rename`, `delete`, `link`, `cloud`, `undo`), Links mit `data-mm-zu` setzen die eingeklappten Knoten aus ihrem `href`.

Das Widget sendet `idea.delete` mit `confirm: true` (es stellt die Frage selbst). Auf das Werk gezogen oder Tab auf dem Werk
ergibt ein freies Hauptthema (`cloud: false`), in die Wolke gezogen `cloud: true`.

### Ein- und Ausgabe, Gliederungsansicht (`idea_map/files/`) {#input-and-output-outline-view-idea_mapfiles}

`crates/baruch-server/src/idea_map/files/` (Ein- und Ausgabe der Mindmap, Benutzung ohne JavaScript):

- `mod.rs` – Routen (`pages()`, `writes()`), die Ausgaben, `POST /werke/{id}/karte/act`.
- `export.rs` – Wolke und Hauptäste aus der Ansicht `tree::Scene` (`cloud`, `placed`), das Blatt (`Sheet`) als Vorstufe von
  `.mm`/`.xmind`, Markdown.
- `freemind.rs`, `xmind.rs` – je ein Schreiber und Leser. `import.rs` – Grenzen, Bereinigung, `Raw` → `ImportTree`
  (`finish`, `check`). `upload.rs` – Formular, Vorschau, Bestätigung. `outline.rs` – Gliederungsansicht.
- Templates `karte_gliederung.html`, `karte_einlesen.html`; Stil `static/karte_gliederung.css` (nur Variablen aus `app.css`).

Sicherheit beim Einlesen: Upload höchstens 10 MiB (`DefaultBodyLimit` an der Route). Aus dem ZIP wird nur der Eintrag
`content.json` gelesen (Namen als Bytes verglichen, nie als Pfad benutzt, nichts auf die Platte entpackt); beim Entpacken
werden die Bytes gezählt, und bei 20 MiB bricht es ab; Archive mit mehr als 50 Einträgen oder mit Namen, die aus dem Archiv
hinausführen würden (`..`, absoluter Pfad, Laufwerk, `\`), werden ganz abgelehnt. XML: Ein DOCTYPE wird abgelehnt, Entitäten
werden nicht aufgelöst (nur die fünf von XML, `nbsp`, Zeichenverweise); der Leser arbeitet mit eigenem Stapel und begrenzt
Knotenzahl und Tiefe beim Lesen. Zwischen Vorschau und Bestätigung steht der Baum als JSON in einem verborgenen Formularfeld
(keine Datei im Datenordner) und wird bei der Bestätigung noch einmal geprüft (`import::check`). Beide POST-Routen liegen
hinter `guard::same_origin`.

### Abweichungen vom Vertrag der Runde {#deviations-from-the-contract-of-the-round}

- **`idea.delete`:** Der Vertrag sagt „Antwort zusätzlich `deleted: [ids]`“. `deleted` ist seit Runde 7 `true` (RPC und MCP,
  unter anderem in `round23_geheimnisse.rs` geprüft); die Liste heißt darum **`deleted_ids`**, `deleted` bleibt `true`.
- **Ablehnungen von `idea.move` und `tree::import`** tragen den Code **-32000** (`rpc::REFUSED`, "refused"). Der Vertrag
  schreibt -32001; das ist seit Runde 19c `RAG_UNAVAILABLE`.
- **`MapIdea`** (Typ der flachen Karte) bekommt die Felder `title`, `parent`, `position` nicht am Typ, sondern in der Antwort
  von `idea.map` und `idea_map` (dazu `main_topic`, `cloud`): Die eingefrorene Datei `bild/layout.rs` baut `MapIdea` in ihren
  Tests von Hand und bräche sonst.
- **`MindMap` unverändert:** Die Wolke ist kein Feld von `MindMap` (von Hand gebaute Werte in anderen Tests brächen), sondern
  eine zweite Funktion `tree::cloud`, oder beides zusammen `tree::tree_and_cloud` (siehe 3).
- **XSD:** Das Schema der Nebenversion 9 liegt als neue Datei `docs/schema/baruch-werk-1-u9.xsd` neben den eingefrorenen;
  `baruch-werk-1-u8.xsd` ist unverändert (SHA-256 im Test).
- Dateinamen der Ausgaben heißen `<slug>-mindmap.*` (vorher `-karte.*`); die Adressen bleiben `/werke/{id}/karte.*`.
- Ein Einfall eines anderen Werks im Formular der Mindmap ergibt 409 (vorher 422).
- Das alte XMind-8-Format (nur `content.xml`) wird mit einer klaren Meldung abgelehnt, nicht gelesen.

### Integration (Runde 34) {#integration-round-34}

- **Die Naht „Wolke“.** mindmap_daten liefert die Wolke getrennt vom Baum (`tree::cloud`, `tree::tree_and_cloud`);
  mindmap_widget und mindmap_austausch hatten gegen S1 angenommen, dass „Einfälle direkt an der Wurzel“ die Wolke seien. Freie
  Hauptthemen wären dann als Karten der Wolke gezeichnet worden und die echte Wolke gar nicht. Der Integrator führte die Ansicht
  **`tree::Scene`** (`tree::scene`) ein: Sie hängt die Einfälle der Wolke nach den Hauptästen an die Wurzel (damit
  `walk`/`find` sie kennen), nennt ihre Kennungen (`Scene::cloud`), den Zähler ohne Filter (`cloud_count`) und die Vorschläge
  (`hints`) und fügt die Verknüpfungen mit einem Ende in der Wolke hinzu. Seite, Bild, Gliederungsansicht und alle Ausgaben
  lesen nur noch diese Ansicht; `RenderOptions` trägt dafür `cloud`, `cloud_count` und `hints`
  (`RenderOptions::of(scene, standalone, closed)`), `tree_layout::layout_with` unterscheidet Hauptäste und Karten an diesen
  Kennungen. RPC `idea.tree` und MCP `idea_tree` bleiben bei `{map, cloud}`.
- **Hauptthema und Wolke im Gebrauch.** Widget: Tab auf dem Werk und „auf das Werk ziehen“ senden `cloud: false` (freies
  Hauptthema), „in die Wolke“ `cloud: true`; die Umkehrung für Strg+Z kennt beide Plätze. Formular `POST
  /werke/{id}/karte/act`: `ziel=wolke` → `cloud: true`, `ziel=w` → Hauptthema, `ort=werk` beim Anlegen; Gliederungsansicht mit
  „Neues Hauptthema“ und „Als Hauptthema ans Werk“. Einlesen ohne Zielknoten legt Hauptthemen an (Texte des Templates
  angepasst).
- **Beim Zusammenführen gefunden:** Das Widget rief `idea.delete` ohne `confirm: true` auf – der Server hätte jedes Löschen
  (und das Rückgängig eines Anlegens) abgelehnt. Behoben, Test in `round34_abnahme`.
- Der Vorschlag „passt zu Ast …“ in der Gliederungsansicht nennt den Hauptast mit seiner Nummer (`export::node_title`).
- Gelöscht: `bild/layout.rs`, `bild/svg.rs`, `bild::load`, `export::tree_order`, `web/src/idea_map.ts`, der S1-Text
  `tree::NOT_YET`. `PdfKind::Map` heißt „Mindmap“. Alte Bildregeln `.im-*` in `static/karte.css` stehen noch (Filter,
  Gruppen, Widersprüche und Legende der Seite nutzen einen Teil davon); sie auszuräumen ist offen (Backlog).
- Tests: `round34_abnahme.rs` (Bild und Aktionen des Widgets über RPC, Gliederungsansicht ohne JavaScript mit allen Aktionen,
  MCP mit Herkunft, Rundreise `.mm`/`.xmind` in ein zweites Werk, alte Daten unverändert). Angepasste Tests anderer: elf
  Rundreisen auf `version="9"`, `round25_abnahme`, `round25_bild`, `round18_menueband`, `round28_abnahme`,
  `round28_uebersicht`, `round9_menu`, `round33_seiten` (zwei neue Templates).

## Runde 35 – Konten und Rollen {#round-35--accounts-and-roles}

Konten, Passwort-Hash, Sitzungen und die Anmeldeschicht (Modul `auth`), Rollen je Werk und der einzige Ort der
Berechtigungsprüfung (Modul `roles`), Seiten und Menü (Modul `konten_ui`). Bedienung: `docs/Bedienung.md` (Runde 35, Konten
und Rollen), Betrieb: `docs/Betrieb.md` (Runde 35, Anmeldung und Konten), Methoden und Routen: `docs/RPC.md` (Runde 35).

- **Ohne Konten** (Tabelle `account` leer) bleibt Baruch offen wie vor Runde 35: `Who::Open`, kein Cookie, keine Weiterleitung,
  kein zusätzlicher Kopf.
- **Mit Konten** verlangen Seiten, Formulare und `/rpc` eine Sitzung – auch am eigenen Rechner (Standard). `/mcp` bleibt bei
  Tokens; die Schicht lässt `/mcp` unberührt.
- Das erste Konto ist der Autor und wird nur am eigenen Rechner angelegt (`POST /einrichten`). Jedes weitere Konto ist ein
  Lektor. Konten werden nie gelöscht (das Handle `@lektorat:<slug>` bleibt vergeben), nur gesperrt.

**Entscheidungen des PM** (der Nutzer kann sie kippen): Mit Konten gilt die Anmeldung auch am eigenen Rechner; die Ausnahme „Am
eigenen Rechner ohne Anmeldung“ (`store_meta` `auth.local_open`) schaltet der Autor ein. Direkter Zugriff von anderen Rechnern
bleibt gesperrt (`guard::remote_only_mcp` unverändert) – ein Lektor kommt über den Reverse-Proxy auf demselben Rechner herein.
Kontoart global (`author` | `lektor`), Rolle je Werk. Für Lektoren ist alles verboten, was nicht ausdrücklich erlaubt ist.
Konten und Freigaben sind Instanzdaten (nicht im Austauschformat, kein Commit im Spiegel). Kein neues MCP-Werkzeug.

### Konten und Anmeldung (`auth`) {#accounts-and-login-auth}

| Datei | Inhalt |
|---|---|
| `baruch-store/src/accounts.rs` | Tabellen `account`, `account_session`, `login_attempt` (Migration 0053) und der Schalter `store_meta` `auth.local_open`. Der Store hasht nie und sieht nie ein Klartext-Passwort; `Account` trägt keinen Hash. |
| `baruch-server/src/auth/mod.rs` | `Who`, `Identity`, `current()`/`scope()` (task-lokal), `AuthState` (gemerkt „es gibt Konten“), `accounts_exist`, `identity_of`, `check_name`, `message` |
| `auth/gate.rs` | Schicht `gate` (im Router unmittelbar außerhalb von `consent::inject`), `resolve`, `own_machine`, `peer` |
| `auth/password.rs` | Argon2id: `hash`, `hash_with`, `verify`, `policy`, `hash_blocking`, `verify_blocking`, `DUMMY_PHC` |
| `auth/session.rs` | Token, Cookie, `current_hash()` (Sitzung der laufenden Anfrage, task-lokal) |
| `auth/throttle.rs` | Sperre gegen Durchprobieren: `wait_seconds`, Bereiche, `reserve`/`succeeded` |
| `auth/routes.rs` | Formulare (hinter `guard::same_origin`) und die gemeinsamen Bausteine `create_account`, `change_password`, `check_current`, `set_active` |
| `auth/rpc.rs` | `account.*` |
| `auth/cli.rs`, `main.rs` | Notweg `baruch-server passwort <name>` |
| `guard.rs` | `forwarded(headers)`, `FORWARDED_HEADERS` |

**Ablauf einer Anfrage** (`auth::gate`): (1) `/mcp` geht als `Who::Open` durch. (2) `Who` aus den Extensions der Anfrage (nur
innerhalb des Prozesses setzbar: Tests), sonst `resolve`. (3) `Who::Anonymous`: `PUBLIC_PATHS` und `/static/js/…` gehen durch;
GET/HEAD → 303 auf `/anmelden?next=<Pfad+Query, prozentkodiert>`; `POST /rpc` → 401 mit JSON-RPC-Fehler `-32002`; sonst 401
Text „Anmeldung erforderlich.“ (4) `roles::check_request`. (5) Die Anfrage läuft in `auth::scope(who, …)` und
`session::scope(<Token-Hash>, …)`.

**`resolve`:** keine Konten → `Open`. Sonst: eine gültige Sitzung eines aktiven Kontos → `Account`; sonst der Schalter „Am
eigenen Rechner ohne Anmeldung“ an **und** `gate::own_machine` → `LocalAuthor`; sonst `Anonymous`. Ein Datenbankfehler führt nie
zu mehr Rechten (er geht nach stderr, die Anfrage gilt als nicht angemeldet; lässt sich nicht einmal die Zahl der Konten lesen,
gilt „es gibt Konten“).

**Eigener Rechner** (`gate::own_machine`, Entscheidung 0.2): `shutdown::own_machine` (Gegenstelle bekannt und Loopback, kein
Proxy-Name, `Host` eine IP-Adresse oder `localhost`) **und** keiner der Köpfe `Forwarded`, `X-Forwarded-For`,
`X-Forwarded-Host`, `X-Forwarded-Proto`, `X-Real-IP`, `Via` (`guard::forwarded`; schon ihr Vorhandensein zählt, der Inhalt wird
nicht gelesen). Ohne `ConnectInfo`: nein. Sind Proxy-Namen erlaubt (`--allow-host`), gilt auch eine Anfrage ohne `Host` nicht
als eigener Rechner.

**Antworten:** Mit Konten bekommt jede Antwort (angemeldet oder abgelehnt) `Cache-Control: no-store`, wenn sie nicht selbst
eine Anweisung trägt (`/static/…` behält `no-cache`). Ohne Konten hängt die Schicht nichts an.

**Regel:** `auth::current()` und `auth::session::current_hash()` nie in gestarteten Tasks (`tokio::spawn`, `spawn_blocking`)
auswerten – vorher holen und mitgeben.

#### Passwort {#password}

- Argon2id, **m = 19456 KiB, t = 2, p = 1**, Salz 16 Bytes (getrandom), Ausgabe 32 Bytes, gespeichert als PHC-Text
  (`$argon2id$v=19$m=19456,t=2,p=1$<Salz>$<Hash>`), Crate `argon2` 0.5. Hashen und Prüfen laufen in `spawn_blocking`.
- `verify` liest die Parameter aus dem PHC-Text: Wächst die Voreinstellung später, bleiben alte Hashes prüfbar. Nur Argon2id
  zählt; ein unlesbarer Text passt auf kein Passwort.
- Regel: 10 bis 200 Zeichen (Unicode-Zeichen, nicht Bytes), keine weitere Regel.
- Unbekannter Name: Prüfung gegen den festen Dummy-Hash `password::DUMMY_PHC` (gleiche Parameter, gleiche Dauer), dieselbe
  Meldung „Name oder Passwort stimmt nicht.“ – auch bei einem gesperrten Konto und einem überlangen Passwort.
- Kein Klartext in Datenbank (CHECK `password LIKE '$argon2id$%'`), Protokollen, Fehlermeldungen oder Antworten; kein Typ mit
  einem Passwort leitet `Debug` ab; Meldungen über falsche RPC-Parameter zitieren keinen Wert.

#### Sitzung {#session}

- Token: 32 Zufallsbytes (getrandom) als Base64url (43 Zeichen). In der Datenbank steht nur das SHA-256 (hex, 64 Zeichen,
  CHECK an der Spalte). Wer nur die Datenbank kennt, kann sich damit nicht anmelden.
- Cookie: `baruch_session=<token>; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000`, zusätzlich `; Secure`, wenn
  `guard::via_proxy` wahr ist (Proxy-Name im `Host`, oder Proxy-Namen erlaubt und kein `Host`).
- 30 Tage ohne Benutzung gültig, höchstens 90 Tage seit der Anmeldung (beides prüft der Server in SQL); `last_seen` wird
  höchstens einmal je Minute geschrieben. Die nächste Anmeldung räumt abgelaufene Sitzungen aller Konten weg.
- **Erneuern (Zusatz zum Vertrag):** Liegt die vorige Benutzung mehr als eine Stunde zurück, setzt die Antwort dasselbe Cookie
  erneut (frische 30 Tage im Browser) – sonst endete die Sitzung im Browser 30 Tage nach der Anmeldung, obwohl sie benutzt
  wird. Die Obergrenze von 90 Tagen bleibt.
- Abmelden löscht die Sitzung und das Cookie (`Max-Age=0`). Eine Passwortänderung löscht alle **anderen** Sitzungen des Kontos
  (die ändernde bleibt; ein Zurücksetzen durch den Autor oder die Kommandozeile löscht alle). Sperren (`active = 0`) löscht
  alle. Beides wirkt bei der nächsten Anfrage.

#### Sperre gegen Durchprobieren (`login_attempt`) {#lock-against-brute-force-login_attempt}

- Bereiche: `konto:<Namensschlüssel>` (immer, auch für unbekannte Namen; Namensschlüssel = ohne umgebenden Leerraum, einfacher
  Leerraum, Kleinschreibung, höchstens 40 Zeichen) und `adresse:<IP>` (nur mit bekannter Gegenstelle und nicht über den
  Proxy).
- Bis zum dritten Fehlversuch keine Wartezeit; ab dem vierten 2^(n−3) Sekunden (2, 4, 8 …), höchstens 900.
- Der Fehlversuch, der die Sperre auslöst, und jeder Versuch während der Sperre antworten mit **429** und `Retry-After:
  <Sekunden>`; während der Sperre wird kein Passwort geprüft und nichts gezählt.
- Jeder Versuch wird **vor** der Passwortprüfung reserviert (`Store::login_reserve`, `throttle::reserve`): Sperre prüfen und
  als Fehlversuch zählen sind eine Transaktion (`BEGIN IMMEDIATE`). Von beliebig vielen gleichzeitigen Versuchen kommen
  deshalb nur so viele zur Prüfung (Argon2id), wie es nacheinander wären – bei leerem Zähler vier, die übrigen bekommen 429.
  Ein Versuch, dessen Verbindung während der Prüfung abbricht, bleibt ebenfalls gezählt. (Bis zur Codeprüfung nach Runde 35
  wurde erst nach der Prüfung gezählt; eine Salve kam an der Sperre vorbei.)
- Erfolg nimmt die Reservierung zurück und setzt die Bereiche des Versuchs zurück. Einträge älter als 24 Stunden fallen beim
  nächsten Versuch weg.
- Höchstens 1000 Zeilen: Ist die Tabelle voll (999 Bereiche), bekommt ein neuer Bereich keine Zeile; stattdessen wartet der
  Bereich `alle` fest 2 Sekunden – in dieser Zeit wartet jeder Anmeldeversuch. Weil vor der Prüfung gezählt wird, löst bei
  voller Tabelle sogar eine erfolgreiche Anmeldung aus einem neuen Bereich diese 2 Sekunden aus (Erfolg setzt `alle` nicht
  zurück, sonst könnte ein Konto die gemeinsame Bremse für andere lösen).
- Das bisherige Passwort beim Ändern (`/konto/passwort`, `account.set_password`) zählt ebenfalls in `konto:<…>`: Wer eine
  Sitzung hat, kann das Passwort nicht beliebig oft raten.

#### Kommandozeile {#command-line}

`baruch-server passwort <name> [--data <Ordner>]` – Notweg für ein vergessenes Passwort. Liest das neue Passwort als **eine
Zeile von der Standardeingabe** (nie von der Kommandozeile: Dort stünde es in der Prozessliste), setzt es für das vorhandene
Konto und beendet alle Sitzungen des Kontos. Exit 0 mit „Passwort für „…“ gesetzt …“; unbekanntes Konto (nennt die
vorhandenen), ungültige Länge, Ordner ohne Datenbank oder noch kein Konto → Exit 1 mit dem Grund auf stderr; ohne Namen Exit
2. Legt kein Konto an und entsperrt keines. Läuft auch neben dem laufenden Server.

Beim Start gibt `serve` eine Zeile „Anmeldung: aus (keine Konten …)“ oder „Anmeldung: an (n Konten)“ aus.

#### Store (nur von `auth` aufgerufen) {#store-called-only-by-auth}

`account_count`, `account_list`, `account_get`, `account_create` (Lektor nur, wenn es ein Autor-Konto gibt),
`account_create_first` (Prüfung „Tabelle leer“ und Einfügen in einer Anweisung: Von zwei gleichzeitigen Aufrufen gewinnt genau
einer), `account_for_login`, `account_password`, `account_set_password(id, phc, keep)`, `account_set_active` (der letzte aktive
Autor bleibt – Prüfung und Änderung in einer Anweisung), `session_create`, `session_account`, `session_delete`,
`session_delete_all`, `login_reserve` (Sperre prüfen und den Versuch zählen in einer Transaktion; Ergebnis `LoginAttempt`),
`login_wait`, `login_failure`, `login_reset`, `auth_local_open`, `set_auth_local_open`; Helfer `Store::account_name_key`,
`Store::session_token_hash`. `lib.rs` exportiert aus dem Modul nur `Account`, `NewAccount` und `LoginAttempt`; Meldungen und
Grenzen hängen deshalb als assoziierte Konstanten an `Store` (`ACCOUNT_TAKEN`, `ACCOUNT_NOT_FIRST`, `ACCOUNT_NEEDS_AUTHOR`,
`ACCOUNT_LAST_AUTHOR`, `LOGIN_ATTEMPT_ROWS`, `LOGIN_FULL_WAIT`).

### Rollen (`roles`) {#roles-roles}

- **Kontoart global, Rolle je Werk.** `roles::role_in(state, who, work)`: volle Rechte (`Who::Open`, `Who::LocalAuthor`,
  Autor-Konto) → `Role::Author` in jedem Werk; Lektor-Konto → `Role::Lektor` nur mit einer Zeile in `work_member` (Migration
  0054); sonst `None` (Werk unsichtbar).
- **Ein Ort der Berechtigungsprüfung:** das Modul `roles` (`crates/baruch-server/src/roles/mod.rs`).
  - `roles::check_request` – jede Route, aufgerufen aus `auth::gate`. Lektor: nur Treffer in `LEKTOR_ROUTES` (Methode +
    Muster), sonst 403 „Abgelehnt: …“ mit dem Kopf `x-baruch-abgelehnt: rolle`. HEAD zählt als GET; andere Verben als GET,
    HEAD, POST sind für Lektoren gesperrt.
  - `roles::check_rpc` – jede RPC-Methode, aufgerufen aus `rpc::call` vor jedem Dispatch (auch für unbekannte Namen), also für
    `/rpc` (einzeln und als Stapel), `/act` und `/lektorat/act`. Lektor: nur Methoden aus `LEKTOR_RPC`, sonst `-32003` „Das
    darf nur der Autor: <methode>“.
  - Alles, was nicht in den Tabellen steht, ist für Lektoren gesperrt – auch Routen und Methoden künftiger Runden. Die
    Formulare der Module (`/beitraege/act`, `/werke/befunde/act`, `/faden/act` …) rufen ihr Modul direkt auf und stehen darum
    nicht in `LEKTOR_ROUTES`.
- **Werkbindung.** Das Werk steht im Pfad (`{id}`), in der Adresse (`?work=`) oder im RPC-Parameter `work`.
  - Ein nicht freigegebenes Werk wird mit derselben Meldung abgelehnt wie ein unbekanntes („unbekanntes Werk N“; 403 oder
    `-32003`) – ein Lektor erfährt nicht, welche Werke es gibt.
  - Fehlt das Werk, gilt das Standardwerk der Instanz **nie**: RPC setzt das kleinste freigegebene Werk ein; GET-Seiten mit
    `WorkFrom::Query` leiten mit 303 auf dieselbe Adresse mit `work=<Werk>` um – das zuletzt geöffnete (Cookie `baruch_work`),
    wenn es freigegeben ist, sonst das kleinste freigegebene (`roles::check_request_preferring`, so ruft es `auth::gate`
    auf); ohne freigegebenes Werk 303 auf `/werke` (RPC: `-32003` „Noch kein Werk freigegeben.“). Das Cookie gewährt nie ein
    Recht. Für Seiten ohne eigenes Werk (`/werke`, `/konto`, `/ueber`) siehe „Integration“.
  - Ein `work` in der Adresse oder in den Parametern wird für **jede** erlaubte Route und Methode geprüft, auch wenn sie selbst
    kein Werk braucht (Werkwahl im Band). Die Adresse wird so gelesen, wie die Seiten sie lesen (prozentdekodiert; ein
    kodierter Name wie `wor%6B` zählt auch, mehrere `work` müssen alle freigegeben sein).
  - Der Store sucht Ids (Beitrag, Befund, Faden, Bild, fester Schlüssel eines Abschnitts) immer im genannten Werk; eine Id
    eines fremden Werks trifft unter dem freigegebenen nichts (Test
    `ids_of_a_foreign_work_yield_nothing_under_the_granted_work`).
- **Nichts wird gemerkt.** Jede Prüfung liest die Freigaben neu (`Store::member_works`, liefert nur für aktive Lektor-Konten
  etwas). Das Entziehen der Freigabe und das Sperren des Kontos wirken bei der nächsten Anfrage.
- **Sichtbarkeit.** `roles::visible_works` liefert dem Lektor nur freigegebene Werke (`work.list`, `/werke`, Werkwahl);
  `rpc::library_view` liefert Lektoren eine leere Literaturliste (keine Titel, keine Suche, keine Treffer, kein Bildkasten) –
  für `ui.render` und die Seite `/`.
- **Herkunft.** Ein Lektor schreibt unter `@lektorat:<slug>` (`roles::author_name`), alle anderen weiter als „Autor“.
  - Vorschlag: `Store::add_proposal` mit Persona = Handle, Modell leer; Seitenzweig `refs/revisions/<key>/<n>`
    (`mirror::side_refs_after_proposal`) wie bei MCP-Vorschlägen, Trailer `Persona: @lektorat:<slug>`.
  - Beitrag: `post.author` = Handle; wie beim Autor kein Commit beim Anlegen. Eigene Beiträge bearbeiten/löschen: ein Commit
    (`mirror::describe_post`), Autor des Commits ist das Handle, Committer „Autor“.
  - Befund: `finding.persona` = Handle; ein Commit „Befund <id>: angelegt von @lektorat:<slug>“
    (`mirror::describe_finding_added`, neu), Autor „<Anzeigename> (<Handle>)“ `<lektorat-slug>@lektorat.baruch.invalid`,
    Trailer `Baruch-Finding`, `Baruch-Action: finding-add`, `Baruch-Node` (falls ein Abschnitt), `Baruch-Origin: lektor`,
    `Baruch-Lektor`.
- **Freigaben sind Instanzdaten:** Tabelle `work_member` (0054), nicht im Austauschformat, kein Commit im Spiegel; der
  Zeitpunkt steht in `granted`. Das Löschen eines Werks oder Kontos entfernt seine Freigaben (ON DELETE CASCADE).
- **Store (`baruch-store/src/members.rs`):** `member_list(work)`, `member_grant(work, account, role)` (nur Lektor-Konten, nur
  vorhandene Werke, idempotent – der Zeitpunkt der ersten Freigabe bleibt), `member_revoke(work, account)`,
  `member_works(account)` (aufsteigend; leer für gesperrte, unbekannte und Autor-Konten).

#### Erlaubte Routen für Lektoren (`roles::LEKTOR_ROUTES`) {#allowed-routes-for-lektors-roleslektor_routes}

| Methode | Muster | Werk aus | Grund |
|---|---|---|---|
| GET | `/` | Adresse | Werkseite: einen Abschnitt lesen, Vorschlag, Chat |
| GET | `/werke` | – | Werkliste, über `roles::visible_works` gefiltert |
| GET | `/beitraege` | Adresse | ganzer Chatverlauf eines Abschnitts (ohne JavaScript) |
| GET | `/beitrag` | Adresse | ein Beitrag: kopieren, zitieren, einen eigenen bearbeiten |
| GET | `/faeden` | Adresse | Fäden lesen |
| GET | `/faden` | Adresse | einen Faden lesen (nicht schreiben: `/faden/act` gesperrt) |
| GET | `/suche` | Adresse | Suche im Werk (nicht ersetzen) |
| GET | `/gliederung` | Adresse | die Gliederung lesen (nicht ändern: `/gliederung/act` gesperrt) |
| GET | `/werke/{id}/befunde` | Pfad | Befundseite, dort auch „Befund schreiben“ |
| GET | `/werke/{id}/fassungen/verlauf` | Pfad | den Verlauf der Fortschreibungen lesen |
| GET | `/werke/{id}/glossar` | Pfad | das Glossar lesen |
| GET | `/werke/{id}/register` | Pfad | das Register lesen |
| GET | `/werke/{id}/bilder/{name}/datei` | Pfad | Bilder im Text der Werkseite |
| GET | `/werke/{id}/bilder/{name}/vorschau` | Pfad | Vorschaubilder im Text |
| GET | `/ueber`, `/ueber/lizenz` | – | über Baruch, Lizenz (AGPL: Hinweis auf den Quelltext) |
| GET | `/konto` | – | eigenes Konto (Passwort ändern, abmelden) |
| GET | `/anmelden` | – | Anmeldeseite (nach dem Abmelden) |
| GET | `/einstellungen/darstellung` | Adresse | Darstellung im eigenen Browser (Cookie); alle anderen Einstellungsseiten gesperrt. **Abweichung:** mit dem Werk aus der Adresse statt „ohne Werk“ – ohne `work` zeigte das Band der Seite das Standardwerk der Instanz (Titel eines fremden Werks) |
| GET | `/static/app.css`, `/static/favicon.svg`, `/static/js/{name}` | – | Stil, Symbol, Widgets |
| POST | `/rpc` | – | JSON-RPC; jede Methode prüft `roles::check_rpc` |
| POST | `/lektorat/act` | – | Formularweg ohne JavaScript; ruft `rpc::call`, also dieselbe Prüfung |
| POST | `/abmelden` | – | abmelden |
| POST | `/konto/passwort` | – | eigenes Passwort ändern |
| POST | `/einstellungen/darstellung/act` | – | die Darstellung speichern (nur Cookies im eigenen Browser) |

Nicht erlaubt und darum 403: jeder Upload, `/act`, jedes Formular eines Moduls, jeder Export (PDF, EPUB, Word, Austausch,
Mindmap-Datei), Import, alle anderen Einstellungen, Beenden, Literatur, Einfälle, Bildseiten außer Datei und Vorschau. `/mcp`
bleibt bei Tokens und läuft nicht durch die Rollenprüfung.

#### Erlaubte RPC-Methoden für Lektoren (`roles::LEKTOR_RPC`) {#allowed-rpc-methods-for-lektors-roleslektor_rpc}

"Werk" = `work` aus den Parametern; fehlt es, wird das kleinste freigegebene eingesetzt.

| Methode | Werk | Art | Grund |
|---|---|---|---|
| `work.get`, `node.get` | Werk | lesend | Werk und Abschnitt lesen |
| `revision.tree`, `revision.amends`, `revision.fold_get` | Werk | lesend | Fassungen, ihr Verlauf, Ausblenden |
| `ui.render` | Werk | lesend | Werkseite ohne Neuladen (Literatur für Lektoren leer) |
| `menu.badges` | Werk | lesend | Zähler im Band |
| `post.page`, `post.since`, `post.versions` | Werk | lesend | Chat des Abschnitts |
| `finding.list`, `finding.get`, `finding.matrix` | Werk | lesend | Befunde |
| `thread.list`, `thread.get`, `thread.since`, `thread.post_versions` | Werk | lesend | Fäden lesen |
| `glossary.list`, `glossary.lookup`, `index.entries`, `index.overview` | Werk | lesend | Glossar und Register |
| `wysiwyg.parse`, `wysiwyg.serialize`, `wysiwyg.diff`, `wysiwyg.style` | Werk | lesend | Editor für den Vorschlag |
| `ribbon.formats` | Werk | lesend | Formate der Vorlage im Band |
| `work.sheet` | Werk | lesend | **ergänzt:** Einstellungen der Satzansicht des Werks für den Editor (nur lesen; `work.sheet_set` bleibt gesperrt) |
| `search.find` | Werk | lesend | **ergänzt:** Suche im Werk (Seite `/suche` und Suchleiste); `search.preview/replace/undo` bleiben gesperrt |
| `work.list` | – | lesend | auf freigegebene Werke gefiltert |
| `account.me`, `account.set_password`, `account.logout_all` | – | eigenes Konto | Konto des Lektors (Baustein „konten“) |
| `revision.propose` | Werk | schreibend | Vorschlag einer Fassung |
| `post.add`, `post.edit`, `post.delete` | Werk | schreibend | Beitrag; bearbeiten und löschen nur eigene |
| `finding.add` | Werk | schreibend | Befund oder „Anmerkung zum Buch“ |

Abweichungen vom Anfang des Vertrags (§2.2), gemeldet (dazu die Route `/einstellungen/darstellung` in §2):

1. `wysiwyg.*` und `ribbon.formats` stehen mit Werkbindung statt „ohne Werk“. Grund: Ohne `work` gibt `rpc::dispatch` ihnen das
   Standardwerk der Instanz (`wysiwyg.parse` liest dessen Bildtexte, `wysiwyg.style` dessen Satzansichts-Einstellungen,
   `ribbon.formats` die Formate seiner Vorlage) – für einen Lektor wäre das ein fremdes Werk. Folge: Ein Lektor ohne
   freigegebenes Werk bekommt dafür `-32003`.
2. Nur die beiden rein lesenden Methoden `work.sheet` und `search.find` wurden ergänzt (Grund in der Tabelle). Keine davon
   berührt ein Modell, eine Verbindung oder das Dateisystem. Nicht ergänzt (offen für den Integrator oder „oberflaeche_konten“,
   falls die Werkseite sie für Lektoren aufruft): `bible.marks` und `scripture.check` (lesen SWORD-Module aus dem Dateisystem),
   `timing.count`, `xref.targets`, `outline.check`, `index.registers`, `work.typeset_get`.

### Oberfläche (`konten_ui`, `view.rs`, Templates) {#user-interface-konten_ui-viewrs-templates}

| Datei | Inhalt |
|---|---|
| `src/konten_ui/mod.rs` | `login_page`/`login_html`, `setup_page`/`setup_html`, `safe_next`, `MESSAGES` mit `ok_message`/`error_message`, `apply`, Marken `KONTO_MARK`, `LEKTOR_OPEN/CLOSE`, `KONTEN_OPEN/CLOSE`, eine eigene minijinja-Umgebung (die 38.) |
| `src/konten_ui/pages.rs` | Routen `GET /anmelden`, `/einrichten`, `/konto`, `/werke/{id}/mitwirkende`; Inhalt der Einstellungsseite „Konten“ (`render_settings`); reine Zeichenfunktionen `accounts_html`, `members_html`, `account_html` über Ansichtsdaten; `direct_from_own_machine` (dieselbe Regel wie `auth::gate::own_machine`, über `guard::forwarded`) |
| `src/view.rs` | `values` liest `auth::current()`; für Lektoren eigene Templates für die drei Bereiche (`pane_templates`), `post.mine`, Beiträge ohne Befundkarten, Herkunft `@lektorat:…` ohne leeres Modell |
| `src/consent.rs` | Hinweisleiste und Dialog nur mit vollen Rechten |
| `src/settings/mod.rs` | achte Seite `konten` in `PAGES`; Navigation des Rahmens für Lektoren nur „Darstellung“ |
| `templates/anmelden.html`, `einrichten.html` | eigenständige Seiten ohne Band und ohne Skript |
| `templates/konto.html`, `mitwirkende.html` | erweitern `base.html` (Reiter „Start“ oder „Buch“) |
| `templates/settings/konten.html` | Inhalt der Einstellungsseite |
| `templates/fragments/lektor_tree.html`, `lektor_editor.html`, `lektor_assistants.html` | Werkseite eines Lektors |
| `static/konten.css` | Regeln der Rollen und Gestaltung der neuen Seiten (nur Variablen aus `app.css`) |
| `web/src/settings.ts`, `menu.ts` | Lektor: „Einstellungen“ bleibt ein Link (kein `settings.render`), Pfeiltasten überspringen verborgene Einträge |

#### `konten_ui::apply(page, who, accounts_exist)` {#konten_uiapplypage-who-accounts_exist}

Läuft in `consent::inject` nach `band::apply` über jede Seite mit der Marke `<!--BARUCH-KONTO-->` (sie steht in `base.html` im
Menü „Baruch ▾“ nach „Baruch beenden…“). Seiten ohne die Marke bleiben Byte für Byte gleich.

| Zustand | Menüeintrag | `<body>` | Marken |
|---|---|---|---|
| ohne Konten | „Konten einrichten…“ (`/einrichten`) | unverändert | ` nur-autor"` → `"`, ` data-nur-autor` entfernt, Stücke `NUR-LEKTOR` und `NUR-KONTEN` herausgeschnitten |
| Autor-Konto | „Angemeldet: <Anzeigename> (Autor)“, „Mein Konto“, Formular „Abmelden“ (`POST /abmelden`) | `data-rolle="autor"` | `NUR-LEKTOR` herausgeschnitten, `NUR-KONTEN` bleibt (ohne Klammern) |
| Lektor-Konto | wie oben mit „(Lektor)“ | `data-rolle="lektor"` | `NUR-LEKTOR` und `NUR-KONTEN` bleiben (ohne Klammern) |
| `LocalAuthor` | „Autor (eigener Rechner, ohne Anmeldung)“, „Anmelden“ | `data-rolle="autor"` | wie das Autor-Konto |
| `Anonymous` | „Anmelden“ | unverändert | wie das Autor-Konto |

Eine Seite ohne Konten ist also bis auf den einen Menüeintrag dasselbe wie vor Runde 35. Geprüft mit einem Byte-Vergleich von
23 Seiten und `ui.render` vor und nach der Änderung (Ziffern genullt): Unterschiede nur der Menüeintrag, der achte Eintrag
„Konten“ in der Navigation der Einstellungen und die Anzeige einer Herkunft `@lektorat:…` (vorher „Persona @lektorat:erika
()“, jetzt „@lektorat:erika“ – ohne Konten kommt sie nicht vor).

Regel für Templates: `nur-autor` ist immer die **letzte** Klasse eines Elements (`class="btn nur-autor"`), sonst findet
`apply` sie ohne Konten nicht. `apply` nimmt die Marken nur aus **Tags**, nie aus Text zwischen den Tags – Buchtext im
Textfeld oder in der Satzansicht bleibt unberührt, auch wenn er zufällig dieselben Zeichen enthält. Gruppen des Bands tragen
das Attribut `data-nur-autor` statt der Klasse (ihr Tag muss wörtlich `<details class="band-group"` … ` data-band-group
open>` bleiben, `band::apply`).

#### Was ein Lektor sieht {#what-a-lektor-sees}

- **Werkseite (Serverseite):** `view::page`/`view::panes` nehmen für Lektoren `fragments/lektor_*.html`. Darin: das
  Formular `form#editor-form` an `POST /lektorat/act` mit `method=revision.propose`, `work`, `key`, `body`, `note` (mit
  JavaScript dieselbe Methode über `/rpc`, `data-rpc` in `workspace.ts`), Schaltfläche „Vorschlagen“; rechts „Befund
  schreiben“ (`finding.add`: `work`, `key`, `kind`, `body`, `quote`), Beitrag (`post.add`: `work`, `key`, `body`), eigene
  Beiträge ändern/entfernen (`post.edit`: `id`, `body`; `post.delete`: `id`, `confirm`). Im HTML fehlen: Speichern, als neue
  Fassung speichern, übernehmen, verwerfen, umschalten („aktuell machen“, „Diese Fassung verwenden“), löschen, ausblenden,
  Personas und ihre Formulare, Lösungen, Beiträge verschieben/umwandeln, ein Bild einfügen, markieren, Zitate prüfen, die
  Dialoge Literatur/Werk anlegen/Einfall, Reiter und Feld „Einfügen“, das Widget "outline" (Gliederung umordnen) und das
  Widget "chat" (siehe Annahmen). Befundblöcke in Beiträgen der Personas liest der Lektor als Text des Beitrags, ohne die Karte
  mit den Statusschaltflächen.
- **Andere Seiten (CSS, ohne JavaScript):** Für `body[data-rolle="lektor"]` blendet `konten.css` `.nur-autor`,
  `[data-nur-autor]` und **jedes POST-Formular** aus, dessen `action` nicht `/lektorat/act`, `/abmelden`, `/konto/passwort`
  oder `/einstellungen/darstellung/act` ist (dieselbe Regel wie im Server: alles verboten, was nicht erlaubt ist – neue
  Formulare künftiger Runden sind für Lektoren von selbst verborgen). `.nur-lektor` ist nur dort sichtbar. Markiert in
  `base.html`: neues Werk/Import (3 Einträge), Austausch einlesen, Beenden, die Gruppen Einfälle, Werkzeuge, Quellen, Satz,
  Ausgabe, Umschlag und die Schaltflächen Zitate prüfen, Querverweise, Werk bearbeiten, Anträge, Gliederungsschema, Bilder,
  Mitwirkende; in `works.html` die Gruppe „Werke“ und ändern, exportieren, PDF, löschen; auf der Befundseite die Formulare für
  Status, Entscheidung und Lösungen (Makro `card(…, na)`).
- **Befundseite:** Das Formular „Befund schreiben: Anmerkung zum Buch“ (`finding.add` ohne `key`, `back=befunde`) als
  `nur-lektor`, nur für Lektoren im HTML.
- **Einstellungen:** In der Navigation steht für Lektoren nur „Darstellung“.
- **Einwilligung:** Hinweisleiste und Dialog (`consent`) nur mit vollen Rechten.

#### Seiten {#pages}

| Route | Verhalten |
|---|---|
| `GET /anmelden[?next=…][&ok=abgemeldet]` | ohne Konten 303 `/`; sonst Formular `POST /anmelden` (`name`, `password`, `next`), Link „Weiter“ nach `safe_next(next)`; mit einer Sitzung „Angemeldet als …“ |
| `GET /einrichten` | mit Konten 303 `/einstellungen/konten`; nicht vom eigenen Rechner 403 mit Erklärung (ohne Formular); sonst Formular `POST /einrichten` (`name`, `display`, `password`, `password2`) |
| `GET /konto[?ok=…][&fehler=…]` | Name, Rolle, Herkunft, Werke (`roles::visible_works`), `POST /konto/passwort` (`current`, `password`, `password2`), `POST /abmelden`; ohne Konten eine Erklärung; `LocalAuthor` ein Hinweis |
| `GET /werke/{id}/mitwirkende[?ok=…][&fehler=…]` | nur volle Rechte (sonst 403), unbekanntes Werk 404; Daten aus `rpc::call("member.list", {work})`; Formulare `POST /werke/{id}/mitwirkende/act` (`action=grant|revoke`, `account`) |
| `GET /einstellungen/konten[?ok=…][&fehler=…]` | Liste aus `Store::account_list`, Schalter aus `account.me` (`local_open`); Formulare `POST /einstellungen/konten/act` mit `action=create` (`name`, `display`, `password`, `password2`), `deactivate`/`activate` (`id`), `reset_password` (`id`, `password`, `password2`), `local_open` (`open=0|1`); ohne Konten eine Erklärung und „Einrichten“ |

`?ok=`/`?fehler=` zeigen nur den Satz aus `MESSAGES` (ein Erfolgscode zählt nicht als Fehler und umgekehrt, unbekannte Codes
zeigen nichts). Alle Kontoseiten antworten mit `Cache-Control: no-store`.

### Annahmen und Abweichungen vom Vertrag {#assumptions-and-deviations-from-the-contract}

Konten:

1. **„Es gibt Konten“ wird nur als Ja gemerkt** (Vertrag §1.4 sagt: Ergebnis zwischenspeichern und nach dem Anlegen
   ungültig machen). Konten werden nie gelöscht, ein Ja bleibt wahr; das Nein wird je Anfrage nachgeschlagen (ein `SELECT
   count(*)` auf der leeren Tabelle). Grund: Ein gemerktes Nein ließe Anfragen ohne Anmeldung durch, wenn ein Konto am Store
   vorbei entsteht (Tests der Kollegen legen Konten per SQL an; ein zweiter Prozess).
2. **Erneuern des Cookies** nach einer Pause von mehr als einer Stunde (siehe Sitzung) – im Vertrag nicht genannt.
3. **Schon der vierte Fehlversuch selbst antwortet mit 429** und `Retry-After: 2` (Abnahme: „ab dem vierten Fehlversuch 429“),
   nicht erst der fünfte Versuch.
4. **`alle`** sperrt jeden Anmeldeversuch (auch bekannte Bereiche) höchstens 2 Sekunden, solange es wartet.
5. **Das bisherige Passwort beim Ändern** zählt in der Sperre des Kontos; während der Sperre ist der Code `passwort_falsch`
   (die Meldungstabelle hat keinen eigenen Code für „gesperrt“).
6. **`POST /einrichten` bei vorhandenem Konto** zeigt die Seite mit 403 und dem Satz „Es gibt schon ein Konto.“ (kein Code
   der Tabelle §3.4); für nicht angemeldete Nutzer endet die Anfrage schon in der Schicht mit 401.
7. **`/einstellungen/konten/act` `create` ohne jedes Konto** leitet auf `/einrichten` um (303): Ein Lektor ohne Autor wäre ein
   Zustand, in dem niemand volle Rechte hat; der Store lehnt es ebenfalls ab.
8. **Das eigene Konto sperren** ist für einen Autor erlaubt, solange ein anderer aktiver Autor bleibt (in dieser Runde gibt es
   über die Oberfläche nur einen Autor – in der Praxis gilt immer `letzter_autor`).
9. **`resolve` nimmt `&http::request::Parts`** statt `&Request` (in S1 schon so festgelegt und begründet).

Rollen:

- **Kein Zähler für offene Vorschläge:** MCP begrenzt offene Vorschläge je Client; für Lektoren (Menschen) gibt es keine solche
  Grenze, nur die Größen (1 MiB Text, 2000 Zeichen Notiz).
- **Lektoren und der Fadenchat:** lesen ja, schreiben nein (kein Modellauftrag, Vertrag §0.7).
- **`/lektorat/act` für den Autor:** nimmt nur Methoden aus `LEKTOR_RPC`; der Server lehnt `revision.propose`/`finding.add`
  für ihn mit einem Hinweis ab. Der Autor nutzt `/act` und die Formulare der Module wie zuvor.

Oberfläche:

2. **`account.me`** liefert `local_open` (Vertrag §1.6); schlägt der Aufruf fehl, zeigt die Seite "switched off".
3. **`member.list`** in der Form des Vertrags (§2.3); eine unlesbare Antwort zeigt die Seite als Fehler.
4. **`/einstellungen/konten/act`** bekommt genau die Felder des Vertrags – kein `work`. Nach der Aktion fehlt das Werk in der
   Adresse (es kommt dann aus dem Cookie `baruch_work`).
5. **`/lektorat/act`** leitet nach `revision.propose`, `post.*` und `finding.add` zurück auf den Abschnitt (`view::node_url`)
   oder, mit `back=befunde`, auf die Befundseite. Die Lektor-Formulare der Werkseite senden kein `back`.
6. **`revision.propose`** über `/rpc` antwortet mit `{key, rev, …}`; `workspace.ts` zeigt dann diese Fassung.
7. **Live-Chat:** Für Lektoren fehlt das Widget "chat" (neue Beiträge erscheinen nach dem Neuladen): `post.since` liefert
   Beiträge mit den Werkzeugen des Autors (`post_entry`), die ein Lektor nicht nutzen kann.
8. **Seiten `/beitrag` und `/beitraege`:** Ein Lektor liest dort; eigene Beiträge ändert und entfernt er nur auf der
   Werkseite. Die Formulare des Autors sind dort durch die CSS-Regel für POST-Formulare verborgen.
9. **Anzeigename bei der Herkunft:** Fassungen, Beiträge und Befunde zeigen `@lektorat:<slug>` (wie `@word:<name>`). Der
   Anzeigename dafür steht auf „Mein Konto“, „Mitwirkende“ und in den Einstellungen – nicht auf der Werkseite (die
   Ansichtsdaten `LibraryView` kennen keine Konten; Besitzer `rpc.rs`).
10. **Links auf gesperrte Seiten** (z. B. Umschlagbild in der Werkliste, „Zitate prüfen“ in älteren Seiten) können für Lektoren
    stehen bleiben, wo sie nicht in `base.html` liegen; sie enden mit 403 „Abgelehnt: …“.

### Angepasste ältere Tests {#adjusted-older-tests}

| Datei | Warum |
|---|---|
| `tests/round9_settings.rs`, `tests/round9_integration.rs`, Unit-Test in `settings/mod.rs` | acht statt sieben Einstellungsseiten (Vertrag §3.1) |
| `tests/round24_pakete.rs` | nach „Baruch beenden…“ folgt der Eintrag für das Konto (Vertrag §3.2) |
| `tests/round32_abnahme.rs` | die Einträge „Word (.docx) importieren…“ und „Baruch beenden…“ tragen in `base.html` die Klasse `nur-autor`; nach der Marke `R32-PLATZ beenden` folgt die Marke für den Eintrag für das Konto |
| `tests/round33_laptop.rs` | `konten.css` steht in `STYLE` nach `laptop.css` (S1); der Test verlangt jetzt, dass `laptop.css` nach dem Grundrahmen steht und nur `konten.css` darauf folgt (die die Zustände des Laptop-Modus nicht berührt – in `round35_oberflaeche` geprüft) |
| `tests/round33_seiten.rs` | `konto.html` und `mitwirkende.html` in `TEMPLATES` |

### Integration (Runde 35) {#integration-round-35}

- **Zusammengeführt** `team/konten`, `team/rollen`, `team/oberflaeche_konten` ohne Konflikt. Die drei Zweige hatten je gegen die
  Stubs der anderen gebaut; die Naht prüft `tests/round35_abnahme.rs` mit echten Sitzungen (kein `common::with_who`):
  Einrichten nur vom eigenen Rechner (nicht über den Proxy-Namen, nicht ohne Gegenstelle, nicht mit `X-Forwarded-For`),
  danach Seiten 303 auf `/anmelden?next=…`, `/rpc` 401 mit −32002, Formulare 401, `/mcp` unverändert; vier falsche Passwörter
  → 429 mit `Retry-After`, auch für das richtige; Abmelden beendet genau eine Sitzung; kein Passwort und kein Token im
  Datenordner (alle Dateien durchsucht). Der Lektor (Name mit Umlaut → `@lektorat:baerbel`) sieht nur das freigegebene Werk,
  schreibt Vorschlag, Beitrag und Befund über `/lektorat/act` (ohne JavaScript) mit Herkunft im Spiegel, darf nicht
  übernehmen (403 über `/act`, `/lektorat/act`, −32003 über `/rpc`); der Autor übernimmt über `/act`; nach dem Entziehen der
  Freigabe 403. Dazu mit einer echten Sitzung jede Route aus `common::registered_routes()` und jeder Methodenname aus
  `common::rpc_method_candidates()` außerhalb der beiden Tabellen: 403 oder −32003, Datenbank (Zeilenzahlen) und Spiegel
  (HEAD) danach unverändert.
- **Ohne Konten:** Die Startseite ist bis auf den Eintrag „Konten einrichten…“ (direkt nach „Baruch beenden…“) die von
  vorher: Der Test schneidet den Eintrag heraus und findet danach nichts aus Runde 35. Ein Byte-für-Byte-Vergleich gegen ein
  Programm am Stand vor S1 steht im Bericht von oberflaeche_konten (23 Seiten und `ui.render`); der Integrator hat ihn nicht
  wiederholt.
- **Zuletzt geöffnetes Werk (Vertrag §2.1, offen bei rollen):** `roles::check_request_preferring(…, preferred)` nimmt das Werk
  aus dem Cookie `baruch_work`, wenn es freigegeben ist; `check_request` (Signatur aus S1) bleibt und ruft es ohne Vorliebe auf.
- **Seiten ohne eigenes Werk (Sichtprüfung):** Für Lektoren verlinkte `/werke` „Zurück zum Werk“ auf das Standardwerk der
  Instanz (403), und mit einem alten Cookie `baruch_work` zeigte das ganze Band auf ein fremdes Werk. Zwei Stellen, beide
  zentral: `AppState::default_work` liefert einem Lektor das kleinste **freigegebene** Werk (ohne Freigabe `UnknownWork(0)`),
  und `auth::gate` richtet das Cookie `baruch_work` **der Anfrage** für Lektoren auf ein freigegebenes Werk (`pin_last_work`;
  das Cookie im Browser bleibt, ohne Freigabe fehlt es). Titel eines fremden Werks standen auch vorher nicht in der Seite.
- **Kopf eines Vorschlags:** „@lektorat:baerbel (@lektorat:baerbel) · Fassung 2“ nannte das Handle doppelt (ein Konto hat
  keinen Persona-Eintrag); jetzt „@lektorat:baerbel · Fassung 2“ (`view.rs`). Der Anzeigename steht dort weiterhin nicht
  (Backlog).
- **Befundseite:** Das Formular „Anmerkung zum Buch“ stand wegen `.fd-form` (befunde.css) am rechten Rand; `konten.css`
  richtet es als Spalte aus.
- `konten_ui::pages::direct_from_own_machine` nutzt `guard::forwarded` (eine Liste der sechs Köpfe statt zwei); der S1-Text
  `accounts::NOT_YET` und die Ausnahmeliste `pending` in `round35_schreibwege` sind entfernt.
- **Widgets der Werkseite eines Lektors** (Sache von rollen: Aufrufe gesperrter Methoden): Die Seite trägt nur `workspace`,
  `editor`, `wysiwyg`, `tree`, `accordion`, `splitter`, `band`, `band-dock`, `menubar`, `settings`, `theme`, `work-select`.
  Beim Laden geschieht kein RPC-Aufruf; „Vorschlagen“ mit JavaScript ruft `wysiwyg.serialize`, `revision.propose`,
  `ui.render`, `menu.badges` – alle erlaubt (Sichtprüfung mit einer Sonde, Protokoll beim Vermittler). `timing`,
  `scripture-check`, `context-menu` stehen für Lektoren nicht in der Seite; `bible.marks`, `timing.count`, `index.registers`
  usw. bleiben deshalb zu Recht gesperrt.
- **Bestätigt (PM-Frage von rollen):** `wysiwyg.*`, `ribbon.formats` und `GET /einstellungen/darstellung` mit Werkbindung;
  `work.sheet` und `search.find` als rein lesende Ergänzungen. Folge: Ein Lektor ohne freigegebenes Werk erreicht die
  Darstellungsseite nicht (Weiterleitung auf `/werke`).
- **Bausteine eingearbeitet:** Die Dateien `docs/r35/*.md` der Entwickler gingen in diesen Abschnitt, in `docs/RPC.md` und
  `docs/MCP.md` ein und sind gelöscht.
- **Fotos der Sichtprüfung:** `.claude/sicht/r35/` (Einrichten, Anmelden, Konten, Mitwirkende, Menü und Werkseite als Autor und
  als Lektor, Befundseite, mein Konto, Ablehnung; hell und dunkel, mit und ohne JavaScript, 1024 px).

### Tests {#tests-1}

- `crates/baruch-store/tests/migration_0053.rs`, `migration_0054.rs`: Migrationen auf einer auf Stand 0052 neu aufgebauten
  Datenbank (alle Zeilen byte-identisch), Store-Regeln (erstes Konto nur einmal, letzter Autor, Sitzungen, Sperre,
  Freigaben).
- `crates/baruch-server/tests/round35_konten.rs` (Anmeldung, Sitzung, Sperre, Verwaltung, Befehl `passwort`),
  `round35_rollen.rs` (Werkbindung, Herkunft, eigene Beiträge, Befunde, Freigaben), `round35_schreibwege.rs` (jede Route und
  Methode außerhalb der Tabellen), `round35_oberflaeche.rs` (Seiten, Menü, Werkseite, Stylesheet), `round35_abnahme.rs` (der
  echte Weg mit Sitzungen, siehe „Integration“).
- Testhelfer in `tests/common/mod.rs`: `registered_routes()`, `rpc_method_candidates()`, `with_who(request, who)`.

### Handprüfung für den Nutzer (echter Browser, Proxy mit TLS) {#manual-check-for-the-user-real-browser-proxy-with-tls}

Zuerst eine Sicherung machen oder mit einem eigenen Datenordner (`--data`) prüfen.

**Ohne Konten**

1. Baruch öffnen: alles wie zuvor? Im Menü „Baruch ▾“ gibt es einen neuen Eintrag „Konten einrichten…“.
2. Einstellungen → Konten: Erklärung und Schaltfläche „Einrichten“.

**Einrichten (am Rechner selbst, `http://127.0.0.1:8765`)**

3. „Konten einrichten…“: Das Formular erscheint. Zwei verschiedene Passwörter → Meldung, der Name bleibt.
4. Das Konto anlegen → Einstellungen → Konten mit „Das Konto ist angelegt.“; das Menü zeigt „Angemeldet: … (Autor)“.
5. Über den Proxy (`https://NAME/einrichten`) oder in einem privaten Fenster: Weiterleitung auf die Konten oder die Anmeldung.
6. Passwortmanager des Browsers: Bietet er beim Anlegen das Speichern und bei der Anmeldung das Ausfüllen an?

**Autor**

7. Abmelden → Anmeldeseite mit „Abgemeldet.“; vier Mal ein falsches Passwort → die Wartezeit wird genannt.
8. Einen Lektor anlegen (Einstellungen → Konten), ein Werk freigeben (Reiter „Buch“ → „Mitwirkende“), wieder entziehen.
9. Sperren und entsperren; das Passwort zurücksetzen (ausklappbares Formular in der Zeile).
10. Den Schalter „Am eigenen Rechner ohne Anmeldung“ an- und ausschalten: Das Menü zeigt „Autor (eigener Rechner, ohne
    Anmeldung)“. **Über den Proxy muss weiterhin die Anmeldeseite kommen.**
11. Dunkles Thema und Laptop-Modus: Anmeldeseite, „Mein Konto“, Konten, Mitwirkende lesbar, nichts läuft aus dem Fenster.

**Lektor (zweiter Browser oder privates Fenster, über den Proxy)**

12. Anmelden: nur das freigegebene Werk in der Werkwahl und unter „Alle Werke“; ohne Freigabe „Noch kein Werk freigegeben.“.
13. Werkseite: kein „Speichern“, kein Reiter „Einfügen“, keine Assistenten rechts; im Menü „Baruch ▾“ nur „Alle Werke“,
    „Einstellungen“, „Über Baruch“, „Mein Konto“, „Abmelden“.
14. Text ändern → „Vorschlagen“ (mit Notiz): Der Vorschlag erscheint als neue Fassung „Vorschlag · @lektorat:…“. Dasselbe mit
    abgeschaltetem JavaScript.
15. Einen Beitrag senden; einen eigenen Beitrag unter „Verwalten“ ändern und entfernen; am Beitrag des Autors fehlt „Verwalten“.
16. „Befund schreiben“ rechts; auf der Seite Befunde „Anmerkung zum Buch schreiben“; dort keine Statusschaltflächen.
17. Adresse von Hand: `/werke/<id>/pdf`, `/einstellungen/werk`, `/literatur` → „Abgelehnt: …“ (403).
18. „Einstellungen“ führt zu „Darstellung“ (kein Dialog); das Umschalten des Themas funktioniert.
19. Als Autor: den Vorschlag des Lektors übernehmen oder verwerfen; die Herkunft steht im Git-Spiegel.
20. Einen Link auf eine Baruch-Seite aus einer E-Mail anklicken, während man angemeldet ist: Anmeldeseite mit „Schon
    angemeldet? Weiter“ → der Klick führt ohne neue Anmeldung zum Ziel.

**Notweg**

21. `echo 'neues-langes-passwort' | baruch-server passwort <Name> --data <Ordner>`; danach anmelden.
22. `curl -i -X POST <Adresse>/rpc -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"work.list"}'` ohne
    Cookie → 401 mit Code −32002; `/mcp` verhält sich wie vor Runde 35 (Token oder eigener Rechner).
23. `sqlite3 'file:<Ordner>/baruch.sqlite?mode=ro' 'select password from account; select token_hash from account_session'` → nur
    `$argon2id$v=19$m=19456,t=2,p=1$…` und 64 Hex-Zeichen.
24. Als Lektor mit zwei freigegebenen Werken: im zweiten Werk arbeiten, dann „Baruch ▾ → Alle Werke“ und „Zurück zum Werk“ →
    das zuletzt geöffnete Werk, nie ein fremdes.

## Runde 36 – Reste {#round-36--leftovers}

Kleine, klar abgegrenzte Punkte aus den Runden 33 bis 35, aus der Gegenprobe von 0.9.0 und aus der Liste „offen nach R24“
(Aufgabe `reste`; Bedienung in `docs/Bedienung.md`, Runde 36, Reste). Keine neue Route, keine neue RPC-Methode, kein neues
MCP-Werkzeug, keine Migration (die reservierte Nummer 0055 bleibt ungenutzt), keine neue Abhängigkeit. Neues SQL steht nur in
`crates/baruch-store/src/reste36.rs`, als Zeichenkettenliterale mit gebundenen Werten.

### Konten und Rollen {#accounts-and-roles}

- **Notweg `passwort`** (`auth/cli.rs`): Nach `account_set_password` ruft `reset_password`
  `Store::login_reset(&throttle::account_scope(name))` auf. Der Bereich hängt am Namensschlüssel des Kontos; der Bereich der
  Adresse und die Bereiche anderer Namen bleiben. Ein abgelehnter Aufruf (Passwort zu kurz, unbekanntes Konto) ändert nichts.
- **Werkbindung gelockert** (`roles/mod.rs`): `GET /einstellungen/darstellung` in `LEKTOR_ROUTES` sowie `wysiwyg.parse`,
  `wysiwyg.serialize`, `wysiwyg.diff`, `wysiwyg.style` und `ribbon.formats` in `LEKTOR_RPC` stehen auf `WorkFrom::None`. Das
  gilt, weil alle Wege zu einem Werk ohne ausdrückliches `work` einem Lektor nur freigegebene Werke liefern:
  `AppState::default_work` gibt ihm das kleinste freigegebene Werk und ohne Freigabe keines; `settings::resolve_work` nimmt
  das Cookie `baruch_work` für einen Lektor nur an, wenn `roles::role_in` das Werk als freigegeben kennt (vorher hätte die
  Seite ohne `work` das zuletzt geöffnete Werk aus dem Cookie genommen). Ein in der Adresse oder im Parameter genanntes `work`
  prüft `roles` wie zuvor. `roles` bleibt der einzige Ort der Berechtigungsprüfung; die Änderung in `resolve_work` wählt nur,
  was gezeigt wird. Beleg: `round36_lektor` (Standardwerk nie fremd, Seite ohne Freigabe und ohne fremden Titel erreichbar,
  Methoden ohne `work`).
- **Anzeigename bei der Herkunft** (`konten_ui/names.rs`): ein Nachbearbeitungsschritt über das fertige HTML, an zwei Stellen
  eingehängt – `consent::inject` (ganze Seiten) und `ui.render` (die drei Stücke zum Neuladen). Er liest
  `Store::account_list`, baut je Konto `@lektorat:<slug>` → Anzeigename und ersetzt nur den **Text** der Herkunftselemente
  durch "Anzeigename (@lektorat:slug)"; Attribute (`data-post-author` …) und alles Gespeicherte bleiben das Handle. Ohne
  Konten und ohne Treffer gibt er die Seite unverändert zurück (Byte für Byte, Test). Die Nachbearbeitung wurde gewählt, weil
  `LibraryView` und die Templates keine Konten kennen und der Store in dieser Runde nicht umgebaut wird.
- **Beschriftungen und Links für Lektoren:** `base.html` trägt die Suchschaltfläche doppelt (`nur-autor` „Im Werk suchen und
  ersetzen“, `nur-lektor` „Im Werk suchen“); wie bei den anderen Paaren lässt `konten_ui::apply` nur die passende übrig.
  `works.html` zeichnet das Umschlagbild für Lektoren ohne Link (`lektor` im Kontext von `works::list_page`). Beides ist
  Bequemlichkeit; gesperrt wird weiter nur in `roles`.
- **Thema auf `/anmelden` und `/einrichten`** (`konten_ui::themed`, in `consent::inject` eingehängt): Die beiden eigenständigen
  Seiten (ohne Band, erkannt am Kopf des Templates und an `body.konto-seite`) bekommen `data-theme` aus dem Cookie
  `baruch_theme` an `<html>`, wie `base.html`. Ohne Cookie oder mit einem unbekannten Wert bleibt die Seite unverändert
  (Farbschema des Systems). Kein JavaScript.

### Mindmap {#mind-map}

- **Rundreise mit Wolke:** `files/import.rs` erkennt den eigenen Ast an der Kennung `export::CLOUD_ID` (nie am Titel), löst
  ihn auf und gibt seinen Kindern `ImportNode::cloud = true` (`tree.rs`, `#[serde(default)]`, damit eine ältere Vorschau im
  Formular gültig bleibt). Beim Anlegen ohne Zielknoten legt der Store die obersten Knoten wie zuvor als Hauptthemen an
  (`idea_import_tree`, eine Store-Datei von sqlx09 in dieser Runde); danach verschiebt `tree.rs` die markierten mit
  `Store::idea_move` in die Wolke – unter derselben Schreibsperre und vor dem einen Spiegel-Commit. Mit einem Zielknoten
  zählt die Markierung nicht, tiefer im Baum ebenfalls nicht.
- **Beschriftungen der Verknüpfungen** (`bild/mindmap.rs`, `free_label_spot`): Jede Beschriftung ist ein Rechteck (geschätzte
  Textbreite plus Rand). Gesucht wird ein Platz, der kein Knotenrechteck und keine schon gesetzte Beschriftung überlappt (3 px
  Abstand) und im Bild liegt: zuerst an sieben Stellen der Kurve mit wachsendem Abstand darüber und darunter, dann in Ringen um
  die Mitte der Linie. Wird nichts gefunden, bleibt sie in der Mitte und trägt `data-crowded`. Dazu ein deckender Hintergrund
  (`rect.mm-link-label-bg`, in der Dateiausgabe weiß), damit kreuzende Linien sie nicht durchstreichen. Grenze: Die Textbreite
  ist geschätzt, der Server kennt die Schrift des Browsers nicht.
- **Heller Streifen im dunklen Modus** (`static/karte.css`): `.karte` ist der Scroll-Container der Seite und war auf 1400 px
  begrenzt und zentriert; in breiten Fenstern stand seine Bildlaufleiste deshalb neben der Bühne statt am Fensterrand, in den
  Farben des Systems. Jetzt spannt `.karte` die volle Breite und rückt den Inhalt über `padding-inline` in die Mitte; `.karte`
  und `.mm-canvas` setzen `scrollbar-color` aus den Variablen der Seite. Die Ursache ist aus dem Stylesheet abgeleitet und dort
  zugesichert; im Browser angesehen wurde es nicht (Handprüfung durch den Nutzer).

### Fassungen und Assistenten {#revisions-and-assistants}

- **Zähler im Band** (`fragments/edit_toolbar.html`): derselbe Schalter wie im Editorkopf aus `rev.fold` („(+ n ausgeblendet)
  ▸“ oder „n ältere verbergen ▾“, gleiche Adressen); ‹ und › nehmen `rev.fold.q` mit.
- **Eigene offene Befunde nur beim Prüfen** (`assist/chat.rs`, `is_check_order`): `OrderFrame::recheck` einer direkt
  angesprochenen Persona ist `rfc || is_check_order(message)`. Eine Prüfanfrage ist eine Nachricht, in der ein Wort (außerhalb
  von Erwähnungen und Zitatzeilen) mit `prüf`, `überprüf`, `nachprüf`, `gegenprüf`, `kontrollier`, `check` oder `review`
  beginnt oder die einen Befund "[B<n>]" nennt. „Prüfen“, ↻ und RFC bleiben erneute Prüfungen. Die Erkennung ist eine
  Wortliste, kein Verstehen: „geprüft“ zählt nicht, „Checkliste“ zählt.
- **`propose_chapter` auf der richtigen Grundlage** (`assist/mod.rs`, `assist/basis.rs`): Der Vorschlag für den Teil des
  Auftrags entsteht über `Store::add_proposal_on` auf `prepared.shown_rev` (gewählt oder gezeigt), die anderen Teile wie zuvor
  auf ihrer aktuellen Fassung. `Outcome::basis` kommt aus `basis::of_proposal` (Kopfzeile „Grundlage: …“) und steht in der
  Antwort von `persona.propose` – auch beim Vorschlag für einen Abschnitt. Ein Text gleich der gezeigten Fassung gilt als
  unverändert.
- **`baruch_vw` beim ersten Aufruf** (`web/src/vw.ts`, `base.html`): ein klassisches Skript im `<head>` vor dem Modul
  `main.js`. Es läuft vor dem ersten Zeichnen, vergleicht die Breitenklasse des Fensters mit dem Cookie, schreibt das Cookie
  bei einem Unterschied neu und setzt die Attribute an `<html>` nach derselben Formel wie `laptop.rs` und `laptop.ts`
  (Schwellen 1280/1600). Stimmt das Cookie, tut es nichts. Der Server kann die Breite bei der ersten Anfrage nicht kennen; ohne
  JavaScript bleibt es „breit“. `vw.js` wird über die vorhandene Route `/static/js/{name}` ausgeliefert; `build.rs` übersetzt
  es wie die anderen Dateien unter `web/src`.

### Einstellungen, Editor, Spiegel {#settings-editor-mirror}

- **„Sicherung angelegt“** (`settings/data.rs`): ein prozessweiter Hinweis je Datenordner (`static BACKUP_NOTES:
  Mutex<BTreeMap<PathBuf, String>>`, kein Feld in `AppState`). Das Formular legt ihn nach einer erfolgreichen Sicherung ab, die
  Seite mit `?ok=sicherung` holt ihn genau einmal ab. Nach einem Neustart ist er weg; `settings.backup` über RPC antwortet wie
  zuvor direkt.
- **Rechte des Datenordners** (`settings::open_permissions`, `permissions_warning`): nur lesen
  (`metadata().permissions().mode() & 0o777`, gemeldet, wenn `& 0o077 != 0`), nur unter Unix. Baruch ändert die Rechte eines
  vorhandenen Ordners nie.
- **Tokenliste** (`settings/mcp.html`, `settings.css`): Klasse `token-former` mit `white-space: nowrap`.
- **Schaltfläche „Zitat“** (`ribbon::QUOTE_SNIPPET`): `#quote(block: true)[]\n` ohne die folgende Zeile "– ".
- **`outline_mentions`** (`xref.rs`): Ordnungszahlen bis `MAX_ORDINAL` = 100 aus Stämmen gebaut und am ganzen Stamm verglichen
  („achtzehnten“ ist 18, nicht 8); römische Zahlen bis 3999, nur in der üblichen Schreibweise. Die Regel „nur wenn eindeutig“
  ist unverändert.
- **store-5** (`baruch-store/src/reste36.rs`, `outline.rs`, `mirror.rs`): `Store::outline_delete_losses` liest alle Fassungen
  außer der aktuellen (`proposal` bei `status = 'proposed'`) und alle Beiträge des Punkts. Der Server ruft es unter der
  Schreibsperre unmittelbar vor `outline_delete` auf – wenn der Mensch löscht und wenn ein Antrag „löschen“ bestätigt wird – und
  gibt das Ergebnis an `mirror::record_outline_losses`: derselbe eine Commit, im Rumpf zusätzlich jede mitgelöschte Fassung und
  jeder mitgelöschte Beitrag im Wortlaut, dazu die Trailer `Baruch-Lost-Revisions` und `Baruch-Lost-Posts`. Ohne Verluste ist
  der Commit wie zuvor. Ein Antrag allein löscht nichts; nur der Mensch kann löschen.

### Grenzen und Ehrlichkeit {#limits-and-honesty}

- Kein Browser in dieser Runde: Tokenliste, heller Streifen, Lage der Beschriftungen in der echten Schrift und das frühe Skript
  `vw.js` sind über Auszeichnung, Stylesheet, erzeugtes SVG und das Node-Gerüst geprüft, nicht gesehen.
- Der Hinweis „Sicherung angelegt“ gilt je Prozess und Datenordner, nicht je Sitzung: Bei mehreren Menschen an einer Instanz
  sieht ihn, wer die Seite mit `?ok=sicherung` zuerst lädt.
- Nicht bearbeitete Punkte stehen mit Begründung in `docs/Backlog.md` („R36: nicht bearbeitet, …“).

### Tests {#tests-2}

- `crates/baruch-store/tests/reste36.rs`: `outline_delete_losses` (verbleibende Fassungen, Beiträge, nur lesen, unbekanntes
  Werk und unbekannter Punkt).
- `crates/baruch-server/tests/round36_lektor.rs` (Werkbindung, Anzeigename, „Im Werk suchen“, Umschlagbild, Thema),
  `round36_mindmap.rs` (Rundreise `.mm`/`.xmind`, fremder Ast „Wolke“, Beschriftungen im SVG, Stylesheet), `round36_reste.rs`
  (Zähler im Band, `vw.js` auf dem Server und im Node-Gerüst, Sicherung, Rechte, Tokenliste, Zitat, Erwähnungen, Spiegel-Commit
  beim direkten Löschen und über einen bestätigten Antrag).
- In vorhandenen Dateien: `round35_konten.rs` (der Notweg hebt die Sperre auf), `kritik_zyklus.rs` (Vorschlag für das Kapitel
  auf der gezeigten oder gewählten Fassung), `round22_befunde.rs` (einfache Frage ohne eigene Befunde, Prüfanfrage mit).

### Typst-Kommandozeile (Aufgabe `typst_alias`) {#typst-command-line-task-typst_alias}

`main` entscheidet vor dem Start der Tokio-Laufzeit über `typst_cli::dispatch(args)`: Heißt das Programm `typst` oder
`baruch-typst` (Dateiname von argv[0], ohne `.exe`, ohne Beachtung der Schreibweise) oder ist das erste Argument `typst`, läuft
`typst_cli::run`, und der Prozess endet mit dessen Exit-Code (0 Erfolg, 1 Satz- oder Dateifehler, 2 falscher Aufruf oder
Nichtunterstütztes) – ohne Datenordner, Datenbank, Server oder Netz. `baruch-server/src/typst_cli.rs` liest nur die Argumente
und schreibt die Dateien (erst nach erfolgreichem Satz); der Satz steht in `baruch-render/src/cli.rs` (`compile_file`,
`font_names`) als Kindmodul von `baruch-render`, das Schriften, Bibliothek und die deutschen Meldungen des Servers teilt. Die
Welt `Disk` liest Dateien nur unter dem Wurzelordner (aufgelöst, nach außen zeigende Symlinks gesperrt) und lehnt Pakete mit
`OFFLINE` ab. `TYPST_VERSION` folgt dem Crate `typst` in `Cargo.lock` (Tests und `scripts/ci-typst.sh` halten das fest). Die
Pakete liefern nur den Namen `baruch-typst` (Symlink oder `.cmd`). Bedienung und Grenzen: `docs/Verteilung.md`, "Typst command
line".

### Datenbankzugriff mit sqlx 0.9 (Aufgabe `sqlx09`) {#database-access-with-sqlx-09-task-sqlx09}

Store, Bibliothek und Server nutzen einen gemeinsamen sqlx-Bau 0.9 (SQLite gebündelt über libsqlite3-sys 0.40; der Server
zusätzlich PostgreSQL mit rustls/ring und Systemzertifikaten für pgvector). `query`, `query_as`, `query_scalar` und
`begin_with` nehmen nur `&'static str` oder einen mit `sqlx::AssertSqlSafe` bestätigten Text. Regel (Modulkommentar von
`baruch-store/src/lib.rs`): SQL wo möglich als Literal; `AssertSqlSafe` nur für Text aus festen Bausteinen des Quelltexts
(Spaltenlisten, Platzhalterketten, feste Bedingungen, Tabellennamen aus Literalen, für pgvector geprüfte Bezeichner über
`quoted()`); Werte werden immer gebunden. Die Migrationen und ihre Prüfsummen in `_sqlx_migrations` sind gegenüber 0.8
unverändert (Tests `sqlx09_migrations` in Store und Bibliothek, `round36_sqlx` im Server). Neu gegenüber 0.8: sqlx-sqlite 0.9
beachtet `-- no-transaction` am Kopf einer Migration; keine Migration von Baruch trägt die Marke, 0040 braucht weiter
`baruch_store::migrate` mit `FOREIGN_KEYS_OFF`.

### Abnahme der Integration {#acceptance-of-the-integration}

`crates/baruch-server/tests/round36_abnahme.rs` (Integrator) prüft über das gebaute Programm die Naht zwischen Export und
Typst-Kommandozeile: Import einer erfundenen Typst-Datei mit einem Bild, `export --out` und `export --pdf`, dann `baruch-typst
compile` (Symlink) und `baruch-server typst compile` auf der exportierten Datei – alle drei PDFs sind Byte für Byte gleich
(Werk ohne Umschlag und Vorspann; mit einem Umschlag setzt der Export seinen Rahmen um die Quelle, die Kommandozeile setzt nur
die Datei). `watch` endet mit Exit 2 und schreibt nichts.

Aus der Sichtprüfung der Integration: Die Kopfzeile eines Vorschlags im Lesefluss (`pv-label`) war die einzige Stelle der
Werkseite, die nur das Handle nannte; `konten_ui::names::ORIGIN_CLASSES` enthält die Klasse nun (`round36_lektor`). Im
Editorkopf wird der Chip mit Anzeigename und Handle mit "…" gekürzt, wenn die mittlere Spalte schmal ist (`.editor-head >
.rev-meta`, wie jede lange Herkunft zuvor) – im Backlog.

## Hilfe auf der Website (Runde 37) {#help-on-the-website-round-37}

Die Benutzerhilfe steht nicht mehr in `docs/`, sondern auf der Website (Hugo, `site/`):

- **Quelle:** `site/content/de/hilfe/*.md` (Nutzer, 14 Seiten, Einstieg `_index.md` mit `aliases: ["/bedienung/"]`),
  `site/content/de/betrieb.md` (Betreiber), `installation.md` und `mcp.md` (erste Einrichtung). `docs/Bedienung.md` und
  `docs/Betrieb.md` tragen nur „wo steht was“ und technische Hinweise.
- **Theme:** `site/themes/baruch/layouts/section.html` (Startseite der Hilfe), `layouts/hilfe/page.html` (Navigation des
  Abschnitts, Inhaltsverzeichnis, zurück/weiter), Shortcodes `hilfebild` (Bild aus `static/bilder/hilfe/`, der Name muss in
  `site/data/hilfebilder.toml` stehen, dunkle Fassung über `<picture>`, Link auf die volle Größe), `taste` und das ältere
  `bild`. Kein JavaScript, keine fremden Adressen.
- **Bilder:** `site/tools/bildschirmfotos.py` baut auf einem Wegwerf-Server (`--listen 127.0.0.1:0`, eigener Datenordner,
  `--secrets memory`, eigenes `HOME`) das erfundene Beispielwerk `site/tools/beispielwerk.typ` und fotografiert jede Zeile der
  Bildliste mit Firefox über einen Vermittler im Skript (Cookies, Sitzung, Öffnen von `<details>`, Klick, Messen eines
  Ausschnitts). `--pruefen` belegt jede Adresse ohne Firefox, `--abgleich` hält Liste, Seiten und Dateien miteinander im
  Gleichlauf. Einzelheiten: `site/tools/README.md`.
- **Prüfung:** `scripts/site-check.py` (aufgerufen von `scripts/ci-site.sh` nach dem Hugo-Bau): tote interne Links und Anker,
  Bilder ohne Alternativtext, JavaScript, fremde Adressen, Weiterleitung `/bedienung/` → `/hilfe/`. Tests
  `tests/round37_hilfe.rs` (Seiten, Anker, Bildnamen, verbotene Wörter, Schlüssel) und `tests/round37_bilder.rs` (Bildliste,
  Hilfelink).
- **Oberfläche:** `about::HELP_URL`; Eintrag „Hilfe“ im Menü „Baruch ▾“ (`templates/base.html`) und auf „Über Baruch“ – ein
  gewöhnlicher Link nach außen, Baruch selbst ruft nichts ab.

## Runde 38 – Englisch als Primärsprache, Werksprache {#round-38--english-as-the-primary-language-work-language}

> Ab Runde 38 wurden neue Abschnitte dieses Dokuments auf Englisch geschrieben; Runde 38c hat die älteren Abschnitte als Ganzes
> übersetzt. Deutsch bleibt nur dort, wo es zitiert wird (Texte der Benutzeroberfläche, deutsche Wörter der Modellausgabe) oder
> Code ist.

**Sprachregel (`AGENTS.md`).** Die Primärsprache ist Englisch für Code, Kommentare, Commits, Entwicklerdokumente, MCP und RPC.
Die Benutzeroberfläche bleibt bis Runde 39 deutsch (danach Deutsch und Englisch über Kataloge). Dateinamen, Routen, RPC- und
MCP-Namen, Bezeichner, Cookies und Fehlercodes haben sich in dieser Runde nicht geändert. `scripts/check-comments-only.py`
belegt, dass eine Kommentarübersetzung nichts außer Kommentaren geändert hat (Vergleich des Texts ohne Kommentare je Datei,
Doctest-Codeblöcke und Ziele von Intra-Doc-Links unverändert; `--umlaute` listet in Kommentaren übrig gebliebene Umlaute,
Ausnahmen in `scripts/comment-umlaut-allow.txt`; `--selftest` läuft in `scripts/ci-selftest.sh`).

### Werksprache {#work-language}

Jedes Werk hat eine **Werksprache**, `de` oder `en`. Personas antworten in dieser Sprache; die Buchausgabe folgt ihr ab Runde
39.

- **Die Werksprache ist nicht die Sprache der Benutzeroberfläche.** Sie zu ändern ändert weder Menüs noch Meldungen. Sie ist
  auch unabhängig von `TypesetSettings.lang` (`<satz sprache>` im Austauschformat, Silbentrennung); die Kopplung kommt in
  Runde 39.
- **Migration `0055_work_language.sql`** (Store): `ALTER TABLE work ADD COLUMN language TEXT NOT NULL DEFAULT 'de' CHECK
  (language IN ('de', 'en'))`. Nur `ADD COLUMN`, kein Tabellenumbau, daher nicht in `FOREIGN_KEYS_OFF`. Vorhandene Werke werden
  `de`. 0056 war für den Integrator der Runde reserviert; die Bibliothek bekam keine Migration (0109 bleibt frei).
- **Nur Datenbank:** Die Werksprache lebt nur in der Datenbank; sie zu ändern ist kein Commit im Spiegel (wie `work_timing`).

Module:

| Modul | Inhalt |
|---|---|
| `crates/baruch-core/src/language.rs` | `WorkLanguage { De, En }` (Standard `De`, serde kleingeschrieben), `ALL`, `as_str()` (`"de"`/`"en"`), `parse()` (genau `"de"` oder `"en"`, kein Trimmen, Groß-/Kleinschreibung beachtet), `english_name()` (`"German"`/`"English"`) |
| `crates/baruch-store/src/language.rs` | `Store::work_language(work)` (`UnknownWork` für ein fehlendes Werk), `Store::set_work_language(work, language) -> bool` (`true` = geändert; eine Transaktion `BEGIN IMMEDIATE`; eine Änderung setzt `work.updated` wie `update_work`); `WorkSummary` und `WorkMeta` tragen `language` |
| `crates/baruch-server/src/work_language.rs` | RPC `work.language`, `work.language_set` (`handles`, `call`, gleiche Form wie `crate::timing`); `of(state, work)` liest die Werksprache für einen Modellaufruf, `fallback(work, read)` entscheidet, was eine unlesbare Werksprache bedeutet (siehe unten) |
| `crates/baruch-server/src/assist/lang.rs` | Markersätze `DE` und `EN` (`Markers`), `markers(language)`, `language_rule(language)`; weitere Marker `MORE_DE`/`MORE_EN` (`MoreMarkers`: `besser:`/`better:`, `Schritt:`/`Step:`, `befund`/`finding`, `Stelle`/`place`, `was fehlt`/`what is missing`, `was stört`/`what disturbs`); `localized(text, language)` tauscht die in einem Prompttext genannten deutschen Marker gegen die der Werksprache; `of_prompt(system)` liest die Werksprache eines Aufrufs aus seiner Sprachregel zurück (für die Nachforderungsvermerke genutzt) |
| `crates/baruch-server/src/assist/prompts.rs` | `default_prompt_in(handle, role, kind, language)`, `effective_prompt_in(handle, role, stored, kind, language)`; `default_prompt_for` und `effective_prompt_for` sind Hüllen mit `WorkLanguage::De`. `effective_prompt(handle, role, stored)` (Einstellungsseite, `persona.prompt_get`) trägt bewusst **keine** Sprachregel: Ein von der Einstellungsseite gespeicherter Prompt darf die Regel eines Werks nicht in ein anderes tragen |

**RPC, MCP, Formular.**

- `work.language {work}` → `{"work": id, "language": "de"}`. `work.language_set {work, language}` → `{"work": id, "language":
  "en", "changed": true}`: nur der Mensch mit vollen Rechten, nicht über `/act`, nicht über MCP (Muster `work.timing_set`); ein
  ungültiger Wert ist Fehler -32602 mit `language: allowed values are "de" and "en"`. Keine der Methoden steht in `LEKTOR_RPC`.
- `work.get` (oberste Ebene, neben `work`), `work.meta` (`work.language`) und `work.list` (je Werk) tragen zusätzlich
  `language`. MCP hat kein neues Werkzeug; `list_works` (je Werk) und `outline` (oberste Ebene, nicht innerhalb von `work`)
  tragen `"language"`.
- Formular `POST /einstellungen/werk/sprache` (Felder `work`, `language`; in `settings::writes()` hinter
  `guard::same_origin`, Handler `settings::work::language_act`); es antwortet 303 zurück auf die Seite mit
  `ok=sprache#sprache` oder `sprachfehler=1#sprache` (Muster `timing_act`) und steht nicht in `LEKTOR_ROUTES`. Der Block
  „Werksprache“ in `templates/settings/werk.html` (`id="sprache"`, unmittelbar vor `id="zeit"`) ist ein einfaches Formular mit
  einem `<select name="language">` („Deutsch“ = `de`, „Englisch“ = `en`) und funktioniert ohne JavaScript.
- Ein unbekanntes oder fehlendes Werk bei `POST /einstellungen/werk/sprache` (auch wenn sich kein Standardwerk bestimmen
  lässt) ändert nichts und antwortet 303 auf `/einstellungen/werk?sprachfehler=werk#sprache`; bei `sprachfehler=werk` zeigt
  der Block „Werksprache“ die Meldung „Die Werksprache ließ sich nicht speichern: dieses Werk gibt es nicht; nichts
  geändert.“ und bei jedem anderen Wert von `sprachfehler` die bisherige Meldung.

**Unlesbare Werksprache.** `work_language::of(state, work)` lässt einen Modellaufruf nie scheitern: Kann der Store die
Werksprache nicht lesen, antwortet die reine Funktion `work_language::fallback(work, read)` mit `de` samt einer Meldung, und
`of` gibt sie auf stderr aus als `work language of work <id> unreadable, answering in German: <error>`. Die Persona antwortet
dann wie vor Runde 38 auf Deutsch. Der Austauschexport (`exchange_dump`) liest die Spalte weiter direkt; der CHECK der
Migration 0055 schließt andere Werte als `de` und `en` aus.

**Welche Texte englisch sind.** Englisch seit Runde 38: die MCP-`instructions`, alle Werkzeug- und Parameterbeschreibungen,
Stub-Antworten und Fehlertexte, die nur in `src/mcp/**` entstehen; für RPC die Protokollfehler (Parse-Fehler, ungültige
Anfrage, unbekannte Methode, Parameterprüfung, Anmeldung/Rolle auf `/rpc`). Fehlercodes sind unverändert. Deutsch bis Runde
39: jede Meldung, die auch eine Seite, ein Formular oder eine Hinweisleiste zeigt (z. B. die Texte von
`StoreError::Refused`, Fortschrittsgründe wie `CRITICAL_RETRY_REASON`). `docs/MCP.md` und `docs/RPC.md` nennen diese Grenze.

**Persona-Prompts und zweisprachige Marker.** Die Standard-Prompts (`assist/prompts.rs`) sind englisch; die Aufgabentexte, die
Baruch um den Text eines Abschnitts legt (`assist/context.rs`, `solutions.rs`, `variants.rs`, `glossary.rs`, `threads.rs`, die
Faktenblöcke, `idea_map/groups.rs`, `models.rs` `PROBE_PROMPT`), sind seit Runde 38c englisch – die Sprachregel allein legt
die Antwortsprache fest; Zusammenfassungen und Gruppennamen werden in der Werksprache verlangt. Vom Nutzer gespeicherte
Prompts bleiben unberührt. Die Prompt-Konstanten nennen die deutschen Marker; für ein englisches Werk tauscht
`lang::localized` sie aus. Unter jeden wirksamen Prompt eines Modellaufrufs (auch gespeicherte) hängt Baruch
`lang::language_rule(language)`: einen festen englischen Satz, der das Modell anweist, immer in der Werksprache zu antworten,
gleich in welcher Sprache Prompt oder Frage sind, und die Marker dieser Sprache wörtlich zu verwenden (die Liste enthält die
Lösungsmarker `besser:`/`better:` und `Schritt:`/`Step:`). Der Prompt schreibt die Marker der Werksprache vor; **jede
Erkennung akzeptiert beide Sprachen**, unabhängig von der Werksprache (`critical_line`, `has_critical`, die Pflicht zu einem
Lösungsvorschlag, `companion_line`, `all_done`, `findings/parse.rs`, `chat.rs`, verbotene Einleitungen, die Wortliste für
Prüfanfragen). Für `de` ist das Verhalten wörtlich dasselbe wie vor Runde 38. Vermerke, die Baruch selbst in Beiträge schreibt
(nicht belegt, keine Lösung, Pflichtfeststellung fehlt), folgen der Werksprache. Die Wörter, die Baruch aus @schrift und
@belege zurückliest (`Wortlaut ungeprüft`, `nicht belegt`, `Angabe prüfen`), werden nur auf Deutsch erkannt; die vollständige
Tabelle der Marker und wo sie gelesen werden steht in `docs/RPC.md`, "Work language".

| Feld von `Markers` | `DE` | `EN` |
|---|---|---|
| `critical` | „Kritisch:“ | `Critical:` |
| `suggestion` | „Vorschlag:“ | `Suggestion:` |
| `note` | „Hinweis:“ | `Note:` |
| `praise` | „Lob:“ | `Praise:` |
| `what_works` | „Was trägt:“ | `What works:` |
| `important_now` | „Jetzt wichtig:“ | `Important now:` |
| `can_wait` | „Kann warten:“ | `Can wait:` |
| `my_proposal` | „Mein Vorschlag:“ | `My proposal:` |
| `done` | `DONE_MARK` | `done:` |
| `instead_of` | „statt:“ | `instead of:` |
| `all_done_sentence` | `ALL_DONE_SENTENCE` | `All my findings on this section seem to be resolved.` |
| `unbacked_note` | `UNBACKED_NOTE` | `(not backed by the text)` |
| `no_solution_note` | `NO_SOLUTION_NOTE` | `(no solution proposed)` |

**Austauschformat.** Nebenversion 10: Attribut `sprache` (`de`|`en`) an `<angaben>`, immer geschrieben; eine Datei ohne es
(jede Datei bis Nebenversion 9) bedeutet `de`. Modell: `ExchangeLanguage { De, En }`, Feld `ExchangeWork::language`. Ein
unbekannter Wert ist ein Fehler des Lesers (`…/@sprache: unbekannter Wert „…“`, wie jedes Aufzählungsattribut). Ein neues Werk
übernimmt die Sprache; beim Abgleich mit einem vorhandenen Werk wird die Sprache wie das Feld `bible` behandelt: Der lokale
Wert bleibt und der Bericht hat die Zeile „Werkangabe „Sprache“ weicht ab (bleibt)“. Einzelheiten:
`docs/Austauschformat.md`, "Work language (minor version 10, Round 38)".

**Tests.** `crates/baruch-store/tests/migration_0055.rs` (Migration auf einer auf dem Stand von 0054 neu aufgebauten
Datenbank, verlustfrei, alle Werke `de`, der CHECK lehnt `fr` ab), `work_language.rs`, `exchange_u10.rs`;
`crates/baruch-exchange/tests/xsd.rs` (u10-Schema, ältere Schemas eingefroren); `crates/baruch-server/tests/round38_sprache.rs`
(RPC, MCP, Formular auch ohne JavaScript, Lektoren gesperrt, Prompts und zweisprachige Erkennung, MCP `initialize` und
`tools/list` ohne Umlaute), `round38_doku.rs` (die übersetzten Entwicklerdokumente, der Kopfabsatz der Verlaufsdateien, die
Regeln in `AGENTS.md`), `round38_abnahme.rs` (Abnahme der Runde).

## Runde 39a – Benutzeroberfläche Deutsch und Englisch {#round-39a--user-interface-german-and-english}

Die Benutzeroberfläche bekommt Kataloge auf Englisch (Basis) und Deutsch; Templates, Widgets und die für Nutzer sichtbaren
Meldungen des Servers nehmen ihre Texte von dort. Die Werksprache aus Runde 38 bleibt eine eigene Einstellung: Buchausgaben
(PDF, EPUB, Word-Export, Austauschdateien, Spiegel-Commits) werden in dieser Runde nicht angefasst (Runde 39b).

### Schnittstelle (S1): Gerüst der Lokalisierung {#interface-s1-localisation-scaffold}

- **Kataloge:** `crates/baruch-server/locales/en/<stem>.ftl` (Basiskatalog) und `locales/de/<stem>.ftl` (der deutsche Wortlaut
  wie er war), immer beide, mit denselben Schlüsseln in derselben Reihenfolge. Fluent-Syntax (`fluent-bundle` 0.15,
  `unic-langid` 0.9), beschränkt auf Variablen `{ $name }`, Zahlenauswahlen und mehrzeilige Werte; keine Attribute, Terme,
  Nachrichtenverweise oder Funktionen. `build.rs` listet jede `locales/<lang>/*.ftl` als `$OUT_DIR/locales.rs` (`(lang, stem,
  include_str!(…))`, sortiert), eingebettet als `i18n::CATALOGS`; eine neue Bereichsdatei braucht keine Codeänderung. Stems und
  Besitzer je Aufgabe stehen im Vertrag der Runde; `common.ftl` gehört zu S1 (`i18n::COMMON_KEYS`: `common.lang.*`,
  `common.save`, `common.cancel`, `common.delete`, `common.close`, `common.back`, `common.yes`, `common.no`).
- **Schlüsselregeln:** `<stem>.<part>.<name>…` (Regex `i18n::KEY_PATTERN`), erstes Segment = Dateistem, stabile Namen und nie
  deutscher Text; mit Punkten in Spalte 0 geschrieben, der Lader bildet `.` auf `-` für die Fluent-Id ab. Nur Schlüssel, die auf
  `_html` enden, dürfen Auszeichnung tragen, genau `<code> <strong> <em> <kbd> <br>` ohne Attribute (`i18n::MARKUP_TAGS`);
  `t()` maskiert alles andere und die Argumente. `widget.*`-Texte nur mit einfachem `{ $name }`. Ein Schlüssel ist an seiner
  Aufrufstelle immer ein vollständiges Zeichenkettenliteral (die Prüfungen finden ihn dort).
- **Zählungen (Nachtrag zu Runde 39a):** Ein Text mit einer Zahl braucht seinen Singular. Server- und Template-Texte nutzen eine
  Fluent-Auswahl auf die Zahl (`{ $count -> [one] … *[other] … }`, Zahlen als Zahlen übergeben; ein Template übergibt eine
  Zahl als Zahl), im Englischen und – wo sich das deutsche Wort ändert – auch im Deutschen; ein Codepfad, der schon einen
  eigenen Singular-Schlüssel wählt (`view_msg.menu.one_section`), behält ihn. Widget-Texte haben keine Auswahlen, sie sind also
  ein Paar `…_one`/`…_other`, und das Widget wählt eines mit `=== 1`, beide Schlüssel als Literale (`widget.search.total_*`,
  `widget.typeset_settings.preview_*`, `widget.mindmap.*_children_*`). Wächter: `tests/round39a_plurals.rs`
  (`every_counted_english_text_has_a_singular`) durchsucht jeden englischen Wert nach einem gezählten Plural-Nomen ohne
  Auswahl; die Ausnahmen (feste Grenzen, Kontextlängen, Zahlen, die immer über 1 liegen) sind mit Gründen aufgelistet.
- **Nachschlagen (`src/i18n.rs`):** ein nebenläufiges Bundle je Sprache, einmal ohne Isolationsmarken gebaut (kein
  U+2068/U+2069 in HTML); `t(lang, key, args)` → Text der Sprache, sonst der englische Text, sonst der Schlüssel; `tc(key,
  args)` = `t(current(), …)`; `exists(key)`; `t_html` für Auszeichnungsschlüssel. Argumente sind `i18n::Arg` (Text oder Zahl;
  Zahlen wählen Pluralformen, Dezimalzahlen werden vorformatiert übergeben).
- **Auflösung und Schichten (`src/lib.rs`):** `i18n::layer` sitzt außerhalb von `auth::gate`: gespeicherte Instanzeinstellung
  (nur solange es keine Konten gibt) → Cookie `baruch_lang` → `Accept-Language` (q-Werte, `i18n::negotiate`) → Englisch;
  `/mcp` ist immer englisch. `i18n::account_layer` sitzt innerhalb des Gates (zwischen `auth::gate` und `consent::inject`): Ein
  Konto mit eigener Einstellung oder der Autor am eigenen Rechner mit der Instanzeinstellung überstimmt sie. Ablehnungen des
  Gates nutzen die äußere Sprache; Seiten, Fragmente, `/rpc`-Antworten und die Einwilligungsleiste nutzen `i18n::current()`.
  Speichern der Einstellung: `Store::ui_language`/`set_ui_language` (`crates/baruch-store/src/ui_language.rs`, `store_meta`-
  Schlüssel `ui.language` und `ui.language.account.<id>`, keine Migration).
- **Task-Locals:** Die Sprache lebt in einem task-lokalen Platz wie `auth::current`. Gestartete Arbeit erbt sie nicht:
  `i18n::scope`/`sync_scope`, `i18n::carry(closure)` für `spawn_blocking` (S1 hat die Closures des Servers umhüllt, die in
  Anfragen laufen), `i18n::scoped_stream` für Antwortrümpfe, die nach dem Handler abgefragt werden (SSE von `assist/routes.rs`,
  der Strom von `connections/multigpt.rs`).
- **Store-Fehler:** `src/store_text.rs` `store_error_text(lang, &StoreError)` ist der eine Ort, an dem Store-Meldungen zu
  Nutzertext werden: `/rpc` in der Sprache der Anfrage (`impl From<StoreError> for RpcError`), MCP englisch (`impl
  From<StoreError> for Failure`). S1 reicht den deutschen Text durch.
- **Umschalter (`src/i18n_switch.rs`):** `POST /einstellungen/sprache` (`lang=en|de|auto`, optional `back`) hinter
  `guard::same_origin`, öffentlich (`auth::PUBLIC_PATHS`) und für Lektoren erlaubt (`roles::LEKTOR_ROUTES`). Cookie für ein
  Jahr (`auto` löscht es), Einstellung je Konto oder Instanz (anonym: nur Cookie), 303 auf ein lokales `back`, sonst auf den
  Referer desselben Hosts, sonst `/`; eine ungültige Sprache → 422 `common.lang.invalid`. Formulare: Menü „Baruch ▾“ in
  `base.html` (Block `R39A-S1 lang-switch`) und `settings/darstellung.html` (Block `R39A-S1 lang-form`, Radiobuttons mit „Wie
  der Browser“); `settings.get {page: "darstellung"}` liefert `lang: en|de|auto`.
- **Templates:** `i18n::install(&mut env)` folgt jedem `Environment::new()` und registriert `t(key, **args)`, `ui_lang()`,
  `widget_catalog()` und die Filter `ui_minutes`, `ui_day`, `ui_number(decimals=0)`. Eine Template-Variable `lang`
  (`en`/`de`) wählt die Sprache, sonst `current()`. `<html lang="{{ ui_lang() }}">` in `base.html`, `anmelden.html`,
  `einrichten.html`, `shutdown_done.html`; `konten_ui::themed` akzeptiert im eigenständigen Kopf jeden Code aus zwei
  Buchstaben.
- **Widget-Katalog:** `<script type="application/json" id="baruch-i18n">{{ widget_catalog() }}</script>` vor dem Modulskript in
  `base.html` (jede Seite, die `main.js` lädt; die eigenständigen Seiten `anmelden.html` und `einrichten.html` laden kein
  Skript und bleiben ohne): alle `widget.*`-Texte der Seitensprache mit ihren `{ $name }`-Platzhaltern, `<`, `>`, `&` und die
  Klammern in Texten als JSON-Escapes (kein `{{`/`}}` in einer Seite). `web/src/i18n.ts` liest ihn einmal: `t`, `uiLang`,
  `fmtNumber`, `fmtMinutes`, `fmtDay` (gleiche Ausgabe wie der Server); ohne das Element liefert `t` den Schlüssel.
- **Formatierung:** `i18n::number(lang, value, decimals)` (de `1.234,5`, en `1,234.5`), `localtime::ui_minutes` (de `07.11.2026
  18:53`, en `2026-11-07 18:53`) und `localtime::ui_day`; `german_minutes`/`german_day` bleiben für Buchausgaben.
- **Prüfungen (`tests/round39a_i18n.rs`):** gleiche Dateien, Schlüssel und Platzhalter in en und de, Fluent-Beschränkungen,
  Auszeichnung nur in `_html`-Schlüsseln, jeder benutzte Schlüssel existiert, kein verwaister Schlüssel, der Literalfinder über
  Templates (Textknoten, gezeigte Attribute, Literale in Ausdrücken) und Widgets (Zeichenketten- und Template-Literale) gegen
  `scripts/i18n-literal-allow.txt` (jedes deutsche Literal aufgelistet, kein veralteter Eintrag), Auflösung und Umschalter,
  Widget-Katalog, Formatierung. Das Testgerüst legt die älteren Tests auf Deutsch fest: `common::router` fügt `Accept-Language:
  de` hinzu, wenn eine Anfrage weder einen Kopf noch das Cookie nennt; `common::with_lang` fragt eine andere Sprache an.

#### templates_1 {#templates_1}

Die 41 Templates des Layouts (`base.html`, `workspace.html`, Toast, Kontextmenü, Einfallserfassung, neues Werk,
Persona-Hinweisseite), der Werkfläche (Editor, Band-Leiste, Baum, Fassungen, Fassungsbaum, Hervorhebungen, Vorschläge,
Prüfgrundlage und -bereich, Assistenten, die drei Lektor-Fragmente, Notizkarten, Markierung, Varianten, Fassungsverlauf), der
Gliederungsseiten (`outline*.html`), Beiträge und Fäden, Suche (Leiste und Seite) und der Verweisseiten (Querverweise,
Zitatprüfung, Glossar, Register) auf `t()` umgestellt. Stems und Dateien: `layout`, `editor`, `outline`, `posts`, `search`,
`reference` in `locales/{en,de}/`, jede Datei nach Template gegliedert mit einem Kommentar `# templates/…` (1 227
Schlüssel); gemeinsame Wörter kommen aus `common.*` (Speichern, Löschen, Schließen).

- Plurale, die die Templates innerhalb eines Wortes bauten (`Beitr{% if %}ag{% else %}äge{% endif %}`, `Fassung{% if %}en`,
  `offene{% if %}r Befund{% if %}e`), wurden zu je zwei Schlüsseln (`…_one`, `…_other`), gewählt über das unveränderte `{% if
  %}`; die deutschen Texte sind dieselben Wörter wie zuvor. Trennbare Verben um eine Schleife (`setzt [B1], [B2] um`) behalten
  zwei Schlüssel; der englische Schlussteil ist die leere Zeichenkette `{ "" }`.
- Zeichenketten, die mit `~` gebaut wurden (`ws_hint`, `save_hint`, `foot_hint` im Editor und im Lektor-Editor), sind ein
  Schlüssel mit Variablen; der Hilfetext des Typst-/Markdown-Imports mit `<code>` ist der einzige `_html`-Schlüssel
  (`layout.work_new.import_help_html`), der Verweissatz der Zitatprüfung ein weiterer (`reference.scripture.quote_rule_html`).
- Schleifenvariablen namens `t` überdeckten die Funktion `t()` und wurden umbenannt (`gterm` im Glossar, `iterm` im Register,
  `th` in der Fädenliste, `tm` in der Gliederung, `target` im Beitragselement); die Ausgabe ist unverändert.
- Das Zeichen am Reiter „Prüfen“ bleibt „<Zahl> <Wort>“ (`layout.band.findings_open`), damit `menu::fill_badges` die Null
  weiterhin direkt nach dem Tag in beiden Sprachen ersetzt.
- Maskierung: Ein einfacher Schlüssel wird als Ganzes automatisch maskiert, und minijinja schreibt `/` als `&#x2f;`. Die 16
  Texte mit einem Schrägstrich (`Neues Werk / Importieren…`, `/ Ziel …`, `Groß/klein`, …) sind darum `_html`-Schlüssel ohne
  Auszeichnung: `t_html` maskiert nur `& < > " '`, so behalten die Seiten ihren wörtlichen `/` (Tests prüfen es, z. B.
  `assets.rs`).
- Aufrufe nutzen einfache Anführungszeichen (`{{ t('…') }}`) in Text und Attributen gleichermaßen, damit eine Gruppenbeschriftung
  in `<summary>` und ihr `aria-label` wörtlich gleich bleiben (`round33_seiten`).
- In der Erlaubnisliste mit Grund belassen: Bezeichner, die der Finder als deutsch sieht (`data-band-tab="buch"`, die
  `band_tab`-Blockwerte, Formularwerte `gliederung`/`keine`, `data-section="befunde"`, der Handle-Name `alle`,
  `data-settings-page="werk"`). Nicht angefasst: Eigennamen (`Baruch`), Rust-Werte (`{{ notice }}`, Daten aus Rust) und Texte,
  die die Widgets zur Laufzeit setzen.
- Test: `tests/round39a_templates_1.rs`.

#### templates_2 {#templates_2}

- **Umgestellte Templates (29):** Werke (`works.html`, `works_archive.html`, `work_edit.html`, `work_delete.html`,
  `work_numbering.html`, `import_report.html`), Einfälle und Mindmap (`ideas.html`, `idea.html`, `karte.html`,
  `karte_einlesen.html`, `karte_gliederung.html`), Befunde (`befunde.html`, `fragments/findings_panel.html`,
  `fragments/finding_block.html`), Umschlag (`cover.html`, `fragments/cover_panel.html`), Bilder (`assets.html`, `asset.html`,
  `fragments/asset_requests.html`) und Export/Import (`docx_export.html`, `docx_import.html`, `epub_check.html`,
  `fragments/epub_check_panel.html`, `epub_preview.html`, `exchange.html`, `exchange_done.html`, `exchange_import.html`,
  `exchange_preview.html`, `typeset.html`). Jeder sichtbare deutsche Text, Titel, aria-label, Platzhalter und jedes
  Zeichenkettenliteral eines Ausdrucks (z. B. die Schaltflächenbeschriftungen von `asset_requests.html`, der Rückfall „(ohne
  Titel)“) läuft über `t()`; Auszeichnung, Attributreihenfolge und die S1-Blöcke sind unverändert.
- **Stems:** `works` (auch der Importbericht), `ideas`, `findings`, `cover`, `assets`, `export`; bereichsweite Wörter stehen am
  Anfang jeder Datei (`works.back_to_work`, `findings.open_count`, …), Seitentexte unter `<stem>.<page>.*`. Sätze um Links sind
  in Schlüssel `…_before`/`…_link`/`…_after` geteilt; Inline-Texte mit `<code>`/`<strong>` sind `_html`-Schlüssel mit
  maskierten Argumenten. Frühere Template-Bedingungen für Singular/Plural (`Abschnitt{% if n != 1 %}e{% endif %}`, `{% if n ==
  1 %}Seite{% else %}Seiten{% endif %}`) sind Fluent-Auswahlen auf `$count`; Zahlen werden als Zahlen übergeben.
- **Buchausgaben unberührt:** Umschlag-, EPUB-, Word- und Satzseiten übersetzen nur ihre Oberfläche. Werte, die aus Rust
  kommen (`{{ notice }}`, Artbeschriftungen der Austauschzählungen, Diagnosen, Daten), bleiben Variablen für die
  Meldungs-Aufgaben; Word-Vorlagennamen („Überschrift 1–3“, „Baruch Teil“) und der Spiegel-Commit-Text „Werk gelöscht“ sind im
  englischen Text unverändert zitiert, weil sie benennen, was die Datei oder der Spiegel wirklich enthält.
- **Schleifenvariablen:** `karte_gliederung.html` und `fragments/findings_panel.html` nannten eine Schleifenvariable `t`, die
  die Funktion `t()` verdeckte; die Schleifen nutzen jetzt `target`.
- **Erlaubnisliste:** Die 639 Zeilen dieser Templates sind weg; übrig sind nur die Reiterkennungen `{% block band_tab
  %}buch{% endblock %}` (kein angezeigter Text; der Finder liest das bloße Wort als Stoppwort), mit Grund aufgelistet.
- **Tests:** `tests/round39a_templates_2.rs` ruft 21 Seiten in en und de ab (`<html lang>`, kein roher Schlüssel der sechs Stems,
  eine deutsche Probe nur in de und ihr englischer Text nur in en), das Befund-Akkordeon der Werkseite, die Plural-Auswahlen
  und deutsche Stichproben. Unit-Tests, die diese Templates außerhalb einer Anfrage rendern, legen Deutsch mit
  `i18n::sync_scope(Lang::De, …)` fest.

#### templates_3 {#templates_3}

- **Umgestellt (28 Templates):** die Einstellungsseiten (`settings.html`, `settings/frame.html`, `settings/darstellung.html`
  außerhalb des S1-Blocks `lang-form`, `assistenten`, `daten`, `konten`, `mcp`, `quellen`, `verbindungen`, `werk`,
  `werk_satz`), die Kontoseiten (`anmelden.html`, `einrichten.html`, `konto.html`, `mitwirkende.html`), die Systemseiten
  (`about.html`, `shutdown.html`, `shutdown_done.html`, `sync.html`, `consent.html`) und Quellen, Literatur und Web-Belege
  (`source.html`, `source_import.html`, `source_multigpt.html`, `literature.html`, `literature_bible.html`,
  `fragments/literature_bibles.html`, `fragments/library.html`, `websearch_evidence.html`). Ihre Zeilen in
  `scripts/i18n-literal-allow.txt` sind weg (keine übrig, keine `# reason`-Zeile nötig).
- **Kataloge:** `settings` (Schlüssel `settings.<page>.…`: `page`, `frame`, `display`, `assistants`, `data`, `accounts`, `mcp`,
  `sources`, `connections`, `work`, `typeset`), `accounts` (`sign_in`, `setup`, `mine`, `contributors`, `cmd_name`), `system`
  (`about`, `shutdown`, `shutdown_done`, `sync`, `consent`) und `sources` (`source`, `import`, `multigpt`, `literature`,
  `bible`, `bibles`, `library`, `evidence`); rund 860 Schlüssel, en und de in derselben Reihenfolge. Der Kommentar `#
  templates/<file>` in den Katalogen nennt das Template, zu dem ein Schlüsselblock gehört.
- **Muster:** Wörter, die sich nur mit einer Zahl änderten (`Sekunde/Sekunden`, `Auftrag/Aufträge`, `Dokument/Dokumente`,
  `Seite/Seiten`, `schwacher/schwache Treffer`, „hat 1 Abschnitt / haben n Abschnitte“), wurden ein Schlüssel mit einer
  Fluent-Zahlenauswahl (`count=…`). Platzhalter innerhalb von Code (`<Name>`, `<Token>`, `<IP>`, `@mcp:<name>`,
  `baruch/model-<id>`, `sword-cipher:<modul>`, „Baruch: <Werktitel>“) werden im Template mit `{% set %}` gebaut und dem Text als
  Argument übergeben, damit kein Katalogtext ein `<` außerhalb der erlaubten Tags trägt; die Befehlszeilen in
  `settings/mcp.html` bleiben wörtlicher Code außerhalb von `t()` nach dem Satz, der sie einleitet. Makroargumente
  (`choice(…)`, `num(…)`, `sub(…)`), Standardtexte (`a.last_login or …`, `| default(…)`) und die Beschriftungen des Dicts
  `kind_labels` sind `t()`-Aufrufe; Vergleichswerte (`"fixed"`, `"Lektor"`, `"bekannt"`, …) und `data-*`-Zustände bleiben wie
  sie waren.
- **Schleifenvariablen namens `t`:** Eine Schleifenvariable `t` verdeckt die Funktion `t()` innerhalb ihrer Schleife;
  `settings/mcp.html` (Tokens, jetzt `tok`) und `fragments/library.html` (Titel, jetzt `lit`) wurden umbenannt. Andere
  Templates nutzen `t` noch als Schleifenvariable (`threads.html`, `glossary.html`, `karte_gliederung.html`,
  `index_page.html`, `outline.html`, `fragments/findings_panel.html`, `fragments/post_item.html`); ihre Besitzer müssen sie
  umbenennen, bevor sie in diesen Schleifen `t()` aufrufen.
- **Maskierung:** Ein Text aus `t()` wird wie jeder Wert automatisch maskiert, `/`, `"` und `'` mitten in einem Satz erscheinen
  also als `&#x2f;`, `&quot;`, `&#x27;` im HTML (der Browser zeigt denselben Text); Zusicherungen auf rohem HTML mit diesen
  Zeichen vergleichen die maskierte Form.
- **Ältere Tests:** Tests, die diese Templates außerhalb einer Anfrage rendern (direkte Aufrufe von `render_connections`,
  `render_sources`, `accounts_html`, `members_html`, `account_html`, `login_page`, `setup_page` und zwei Unit-Tests in
  `connections/pages.rs`, `settings/work_typeset.rs`), rendern nun innerhalb von `i18n::scope`/`sync_scope(Lang::De, …)`;
  `round32_beenden` fragt seinen echten Prozess mit `Accept-Language: de`; `round36_reste` prüft den Schlüsselaufruf und den
  deutschen Katalogtext statt des Template-Quelltexts; der Test von `baruch-library`, der `fragments/library.html` in einer
  nackten Umgebung rendert, registriert ein kleines `t()`, das `locales/de/sources.ftl` liest.
- **Offen:** die Seitennamen der Einstellungsnavigation, der Speichervermerk („Gespeichert: …“), aus Rust übergebene Hinweise und
  Fehler (`settings/*.rs`, `konten_ui`, `sync`, `websearch`) bleiben deutsch bis messages_2/messages_1;
  `tests/round39a_templates_3.rs` rendert jede Seite der Aufgabe in en und de (mit und ohne Konten, eigener Rechner und
  entfernt für Einrichten und Beenden, der Einwilligungsdialog).

#### Widgets {#widgets}

- **Umfang:** Jede für Nutzer sichtbare Zeichenkette der TypeScript-Widgets (`web/src/*.ts` außer `i18n.ts`, 45 von 47 Dateien
  hatten Texte) kommt aus dem Seitenkatalog über `t("widget.<module>.<name>", { name: … })`, importiert aus `./i18n.js`; Stem
  `widget` (`locales/{en,de}/widget.ftl`, 353 Schlüssel, nach Modul in Dateireihenfolge gruppiert). Deutsch ist der alte
  Wortlaut Byte für Byte; Englisch ist der Basiskatalog. Texte, die die Widgets aus `data-*`-Attributen, dem HTML des Servers
  oder RPC-Antworten nehmen (Beschriftungen von Blöcken, `decision_label`, `range_label`, Servermeldungen), bleiben wie sie
  sind.
- **Angewandte Regeln:** Das zweite Schlüsselsegment ist das Modul, das den Schlüssel nutzt (`widget.mindmap.*` nur in
  `mindmap.ts`); kein `t()` auf Modulebene (der Katalog wird beim ersten Aufruf träge gelesen, damit Node-Tests nach den
  Importen ein falsches `#baruch-i18n`-Element einsetzen können): Tabellen behalten Schlüssel und rufen `t()` dort auf, wo der
  Text gezeigt wird (`outline.ts` Ebenennamen, Ablagehinweise und Anfüge-Beschriftungen, `mindmap.ts` Verknüpfungsarten,
  `scripture_check.ts` Zustände, `typeset_settings.ts` Feldbeschriftungen über `labelOf`, `panes.ts` Themennamen);
  `ws_command.ts` `BUILTIN_REGISTERS` gibt seine Titel über Getter. Plurale und optionale Teile sind eigene Schlüssel
  (`cover_editor.saved_one/_other`, `print_check_one/_other`, `mindmap.delete_question`/`_children`, `created`/`created_cloud`,
  `now_before`/`now_after`, `chat.typing_one`/`typing_many` mit `chat.and`); führende und folgende Leerzeichen bleiben
  außerhalb von `t()`. Text mit `<…>` (Typst-Marken) geht als Argument hinein, weil nur `_html`-Schlüssel `<` enthalten
  dürfen. Die beiden als Auszeichnungszeichenketten gebauten Dialoge (`search.ts` Seitenleiste, `xref.ts` Querverweisdialog)
  maskieren die Katalogtexte mit einem lokalen `esc()`. Zahlen: `cover_editor.ts` `mm()` nutzt `fmtNumber(value, 1)` (war ein
  festes Dezimalkomma); kein Widget formatiert selbst Daten (die Zeiten kommen formatiert vom Server). Kein Widget schaut auf
  `navigator.language`.
- **Belassen (kein angezeigter Text, Erlaubnisliste mit `# reason`):** Routenpfade und Anker (`/werke/…`, `/einstellungen…`,
  `/quelle?…`, `#beitrag-`), das Handle `@alle`, die CSS-Klasse `nur-autor`, Formular-/Abfragewerte (`keine`, `alle`,
  `fassungen`, `werk`) und die Anführungszeichensätze von `quote_find.ts`. Buchinhalt bleibt für Runde 39b in der Werksprache:
  das Standard-Entwurfswasserzeichen `ENTWURF` (`typeset_settings.ts`), das Seitensuffix `, S. …` von Querverweisen in der
  Satzansicht und die Typst-Schnipsel (`xref.ts`), der Verweisabsatz `– ` nach einem neuen Zitatblock (`wysiwyg.ts`).
  Platzhalter, die in eine neue Fußnote, ein Zitat oder einen Begriff eingefügt werden (`Fußnote`, `Zitat`, `Begriff`), folgen
  der Oberflächensprache (`widget.wysiwyg.placeholder_*`) wie die Prompts darum. Protokollzeilen (`console.error`) bleiben
  englisch.
- **Tests:** `tests/round39a_widgets.rs`: Der Seitenkatalog trägt genau die `widget.*`-Schlüssel in der Seitensprache (en und
  de); jedes `"widget.…"`-Literal in `web/src` existiert und gehört zu seinem Modul, keine Waisen; Englisch ohne Deutsch,
  deutsche Stichproben gegen den alten Wortlaut; keine Spracherkennung in den Widgets; unter Node zeigen `chat.js`, `rpc.js`,
  `context_menu.js`, `ws_command.js` und `i18n.js` mit dem englischen Katalog Englisch und mit dem deutschen das alte Deutsch,
  und den Schlüssel ohne Katalog. Ältere Node-Tests, die Deutsch zusichern, bekommen den deutschen Katalog
  (`i18n::widget_catalog_json(Lang::De)`) als falsches `#baruch-i18n`-Element, bevor die Probe läuft.

#### messages_1 {#messages_1}

Servermeldungen der Module Inhalt, Werk, Ausgabe und Literatur (Werke, Gliederung, Nummerierung, Fassungen, Beiträge, Fäden,
Chat-Darstellung, Einfälle und Einfallskarte, Bilder und Bildanträge, Umschlag, Befunde, Glossar, Register, Querverweise, Suchen
und Ersetzen, Bibelstellenprüfung und -zuordnung, Ansicht, Band, Menü, Einwilligungsleiste, Sprechzeit, Personas,
Assistentenoberfläche, Austausch, EPUB, PDF, Satz, Word-Export, Literatur und Bibeln) kommen aus zwölf Bereichsdateien
`locales/{en,de}/{works,outline,posts,ideas,assets,cover,findings,reference,view,assist,export,sources}_msg.ftl` (rund 1 100
Schlüssel; Deutsch wörtlich zu den früheren Rust-Zeichenketten, Englisch als Basiskatalog).

- **Aufrufstellen:** `crate::i18n::tc("…", &[…])` mit dem vollständigen Schlüsselliteral; Argumente nach Namen (`{ $name }`).
  Hinweis-Tabellen (`?ok=`, `?hinweis=`) liefern `String` statt `&'static str`. Frühere deutsche `pub const`-Texte, die Tests oder
  andere Module nutzten, wurden Funktionen mit gleicher Bedeutung: `works::use_word_import`, `literature::{pick_section,
  not_linked, foreign_fulltext, foreign_page, not_checkable}`, `literature::bibles::{no_bible, no_highlight}`,
  `personas::{bible_refused, rag_refused}`, `cover_print::draft_warning`, `typeset::{cancelled_text, timeout_text,
  tail_label}`, `pdf::busy_text`, `findings::view::stale_summary_words`, `typeset_settings::{no_rule, no_document_title}`,
  `exchange::import::title_hint`, `idea_map::near::off`.
- **Vergleiche über Sprachen hinweg:** Statuszuordnungen, die früher deutsche Texte verglichen, prüfen nun beide Sprachen:
  `pdf::is_busy`, `typeset::is_timeout`, `ideas::is_unknown_work` (Store-Text von `UnknownWork` über `store_text`),
  `works::is_duplicate_title` (Store-Text von `DuplicateTitle`).
- **Plural:** Gezählte Wörter der Austauschvorschau nutzen Fluent-Zahlenauswahlen (`export_msg.exchange.add.*`); andere
  Singular-/Plural-Paare sind zwei Schlüssel. Dezimalzahlen behalten das Komma im Deutschen und den Punkt im Englischen
  (Umschlagführungen und -grenzen, Bildgrößen, Typst-Zeitgrenzen); das Kontrastverhältnis nutzt `i18n::number`.
- **`<` in Texten:** Schlüssel ohne `_html` dürfen kein `<` enthalten (Katalogprüfung 4); Platzhalter wie `block:<Nummer>` werden
  `{ $lt }Nummer{ $gt }` mit den Argumenten `lt = "<"`, `gt = ">"` geschrieben.
- **Kommandozeile bleibt deutsch:** Die CLI-Einstiegspunkte in eigenen Modulen (`exchange::import::cli_import`,
  `pdf::write_pdf_file`, `pdf::notes::write_notes_file`, `epub::write_epub_file`, `docx_export::cli_export`) laufen in
  `i18n::scope(Lang::De, …)`, damit gemeinsame Meldungen die frühere deutsche Ausgabe behalten.
- **Nicht übersetzt (laut Vertrag oder weil der Text Daten ist):** Spiegel-Commit-Texte, Modell-Aufgabentexte und -Fakten
  (`assist::prompts`, `context`, `ref_claims`, `rag_facts`, Probleme von `bible_research`, Chunk-Beschriftungen),
  Markererkennung (`chat_render`, `xref`-Muster, `findings::parse`, Auftragsmarker von `assist::chat`, `basis`-Beschriftungen),
  gespeicherte Daten (Vorschlagsnotizen, Beitragstexte, die Baruch schreibt, wie `assist::chat::order_note`,
  Bibliotheksnotizen, Begründungen von Statusvorschlägen, `TRUNCATED_NOTE`/`CUT_NOTE`, mit Beiträgen gespeicherte
  Komprimierungsmeldungen), Buchausgaben (Typst, EPUB-/Word-Inhalt, Umschlag-Druckquelle, Inhalt der Austauschdatei,
  FreeMind-/XMind- und Markdown-Exporte der Mindmap einschließlich der Titel ihres Baums), Protokollzeilen, Panik-Meldungen.
- **Offen (Deutsch noch sichtbar):** Beschriftungen aus anderen Crates (`CoverBlockKind::label`, `CoverFace::label`,
  `FindingStatus::label`, `RevisionReason::label`, Bibelstellenprüfungs-Meldungen von `baruch_scripture`,
  docx-Leserwarnungen); die Wortlautprüfungs-Beschriftungen von `bibles` (`Wortlaut stimmt …`, `state_label`,
  `MISMATCH_PREFIX`, `module_note`) und `bibles::CIPHER_KEY_WRONG` (von `connections/pages.rs` verglichen), weil sie Modellfakten
  oder Vergleiche außerhalb dieser Aufgabe speisen; die Feldbeschriftungen des Umschlaglayouts in Antragszeilen
  (`cover_layout::block_line`, mit Commit-Texten geteilt); die Tooltips im Mindmap-SVG (`idea_map/bild/mindmap.rs`, auch der
  Bildexport); die Beschriftungen von `band::TABS` (vom Zeichnen nicht benutzt).
- **Tests:** `tests/round39a_messages_1.rs` (Hinweis, Formularfehler, RPC-Ablehnung und Fehlerseite in en und de, direkte
  Aufrufe in einem Sprachbereich, Plural-Auswahlen, eigene Kataloge vollständig ohne deutsche Buchstaben im Englischen). Ältere
  Tests, die eigene Funktionen außerhalb einer Anfrage aufrufen, legen jetzt Deutsch mit `i18n::sync_scope`/`scope` fest;
  MCP-Tests, die deutsche Texte eigener Module zusicherten, sichern den englischen Text zu (MCP ist immer englisch).

#### messages_2 {#messages_2}

Servermeldungen von Einstellungen, Verbindungen, Konten/Rollen, System, Abgleich, Websuche, Import und die Meldungen des
Stores. Acht Bereichsdateien, je in `locales/en` (Basis, Englisch) und `locales/de` (der Wortlaut von vor Runde 39a, Byte für
Byte): `settings_msg`, `connections_msg`, `accounts_msg`, `system_msg`, `sync_msg`, `websearch_msg`, `import_msg`, `store_msg`.

- **Umgestellt** (`i18n::tc` in der Sprache der Anfrage): `settings/**` (Seitennamen und Speichervermerke des Rahmens,
  `?ok=`-Hinweise, Formularfehler, `settings.*`-Ablehnungen, Sicherungsmeldungen), `auth/**` außer der Kommandozeile
  `auth/cli.rs` (Ablehnungen des Gates, Anmeldeseite, `account.*`-Sätze), `konten_ui/**` (Meldungscodes `?ok=`/`?fehler=`: die
  Tabelle `MESSAGES` bildet nun einen Code auf einen Schlüssel ab; Kontoeintrag des Menüs „Baruch ▾“), `roles/**`
  (Ablehnungen des Gates und von `check_rpc`, Lektor-Vorschläge und -Befunde), `guard.rs` (die vier „Abgelehnt“-Rümpfe;
  `check` liefert den Schlüssel seines Grundes), `lib.rs` (`internal()`, unbekanntes Werk; die Spiegelordnerprüfungen bleiben
  deutsch, es sind Systemmeldungen), `rpc.rs`, `instance.rs`, `about.rs` (die zwei übersetzbaren Paketorte), `shutdown.rs`
  (Schlussseite), `models.rs` (Verbindungsfehler, Freigabesatz, Notizen der Modellliste, `model.*`-Ablehnungen), `sync/**` außer
  `sync/mock.rs` (Seitenhinweise, `sync.*`-Fehler, jede Notiz eines Laufs), `websearch/**` außer `verify.rs` und `mock.rs`
  (Belegseite, Übernehmen, Notizen und abgelehnte Adressen eines Suchlaufs), `connections/{mod,check,config,collections,
  secrets,sources,pages}.rs` (Prüfungen, Feldprüfungen, Schlüsselbund, die Seite „Verbindungen“ mit ihren Meldungscodes,
  Prüfdetails und Formularfelder), `pdf_import/pages.rs`, `docx_import/pages.rs` und die für den Browser bestimmten Fehler und
  Warnungen von `docx_import/mod.rs`.
- **Wächter außerhalb von `i18n::layer`** (`own_reads`, `remote_only_mcp`, `local_host`) nehmen die Sprache aus Cookie und
  `Accept-Language` der Anfrage selbst (`i18n::resolve(headers, None)`); `same_origin` und das Gate laufen innerhalb und
  nutzen `i18n::tc`.
- **Store-Meldungen** (`src/store_text.rs`): `store_error_text` ordnet jede `StoreError`-Variante erschöpfend zu. Varianten mit
  festem Wortlaut werden aus ihren Feldern formatiert (`store_msg.error.*`). Varianten, die einen Text tragen (`Refused`,
  `Heading`, `Asset`, `Title`, `Color`, `Idea`, `Finding`, `PostEdit`, `Replace`, `CoverBlock`, `Stale`; das Detail von
  `Import`, `Exchange`, `Numbering`, `Glossary`, `Typeset`), werden gegen die deutschen Texte von `PATTERN_KEYS` in
  `locales/de/store_msg.ftl` abgeglichen: Jedes `{ $name }` passt auf jeden Text (der längste Literalteil wird zuerst
  versucht), die erfassten Teile werden die Argumente, und ein Argument namens `detail` wird noch einmal übersetzt
  (verschachtelte Meldungen wie „Veraltet: …“, „Vorschlag abgelehnt: …“). Die deutsche Ausgabe ist deshalb immer der eigene
  Wortlaut des Stores. `/rpc` nutzt die Sprache der Anfrage, `/mcp` Englisch (Verdrahtung von S1). Stellen, die einen
  Store-Text weiterreichen (`connections`-Fehler, `sync.set_collections`, `roles::unknown_work`, die Anmeldeseite), übersetzen
  ihn genauso.
- **Absichtlich deutsch belassen** (Tests und Dokumentation nennen sie): die Konstanten `auth::routes::LOGIN_FAILED` und
  `LOGIN_BLOCKED`, `roles::NO_WORK`, `shutdown::DONE_TEXT`, `about::THIRD_PARTY_PLACES` und `connections::usage_stub` halten den
  deutschen Wortlaut; die Seiten zeigen den Katalogschlüssel desselben Satzes. `docx_import::cli_import` läuft auf Deutsch
  (`i18n::scope(Lang::De, …)`), wie jeder Text der Kommandozeile.
- **Vergleiche vor dem Übersetzen**: `auth::routes` vergleicht `StoreError::Refused` mit `Store::ACCOUNT_*`, bevor ein Text
  gemacht wird; `connections::pages::model_fault` liest die RPC-Meldung für einen Code und kennt nun den deutschen und den
  englischen Wortlaut (`Schlüsselbund`/`Keyring`, `Adresse`/`Address`, `Art`/`Kind`, `gibt es nicht`/`does not exist`).
- **Tests**: `tests/round39a_messages_2.rs` löst einen Einstellungshinweis, einen Formularfehler, eine gescheiterte Anmeldung,
  einen Importfehler, eine Store-Ablehnung (`Refused` und `UnknownIdea`) über `/rpc` in en und de aus, denselben Store-Fehler
  über `/mcp` (Englisch für einen deutschen Browser), die Wächter-Ablehnung und englische Protokollfehler; er durchsucht
  `crates/baruch-store/src` nach jedem `Refused("…")`-Literal und jeder an `Refused(…)` übergebenen Konstante und sichert ein
  `store_msg`-Muster mit dem wörtlichen deutschen Text zu und prüft, dass jeder in `src` zitierte `*_msg`-Schlüssel existiert.
- **Offen** (deutscher Nutzertext bleibt; Englisch zeigt dort Deutsch): `connections/multigpt.rs` (MCP-Client-Fehler, Such- und
  Volltextmeldungen), `connections/pgvector.rs` (Prüfdetails, Suchablehnungen), `rag/**` (Status- und Indexmeldungen),
  `import.rs` (Importnotizen und Bildablehnungen, auch vom Browser-Import von Typst/Markdown genutzt), `archive.rs`
  (ZIP-Ablehnungen), `markdown_import/**`, `pdf_import/{mod,pdf,epub,remote,typst_out,draft}.rs`, `docx_import/draft.rs`; die
  Anzeigewerte `models::locality` („lokal“/„extern“, auch ein `data-locality`-Code) und `models::kind_title`
  („OpenAI-kompatibel“). Store-Texte ohne Muster bleiben deutsch: die dynamischen Nutzlasten von `ideas.rs`, `idea_links.rs`,
  `glossary.rs`, `outline.rs` (Überschriftsprobleme von `Heading`), `cover_blocks.rs`, `exchange_apply.rs`, die Satzprüfungen
  von `baruch-typst` innerhalb von `Typeset`, die ISBN-Prüfung von `book.rs` und die Feldbeschriftungen in
  `store_msg.book.field_*`.
- **Absichtlich nicht umgestellt** (Vertrag §5): Kommandozeile (`auth/cli.rs`, `--allow-host`-Auswertung in `guard.rs`,
  `connections::secrets::describe`/`--secrets`), Attrappen, Protokollzeilen, `expect()`-Texte, in das Werk geschriebene
  Word-Kommentar- und Vorschlagstexte (`docx_import::apply`), Meldungscodes in Adressen (`?ok=`, `?fehler=`).

### Integration (Runde 39a) {#integration-round-39a}

- **Zusammenführung:** Die sechs Zweige `team/r39a-templates_1`, `-templates_2`, `-templates_3`, `-widgets`, `-messages_1`,
  `-messages_2` (je auf S1 `3828d0c` aufgebaut) wurden mit `--no-ff` in Aufgabenreihenfolge zusammengeführt. Konflikte nur in
  `tests/main.rs` (eine `mod`-Zeile je Aufgabe, alle behalten), `scripts/i18n-literal-allow.txt` (Dreiwege-Mengenzusammenführung:
  Eine Basiszeile bleibt nur, wenn kein Zweig sie entfernt hat, jede hinzugefügte Begründungszeile bleibt) und einem Helfer von
  `tests/round35_oberflaeche.rs` (templates legte das Rendern der Seite auf Deutsch fest, messages_2 den Aufruf von
  `konten_ui::apply`; beides behalten). Die Erlaubnisliste hat nach der Zusammenführung 55 Einträge (mit Kommentaren 90
  Zeilen), alle mit Grund: Bezeichner, Routenwerte und Anführungszeichen; kein unübersetzter Text eines Templates oder Widgets
  ist übrig.
- **Korrektur nach der Zusammenführung:** `tests/round39a_templates_2.rs` suchte rohe Schlüssel in der ganzen Seite; da der
  Widget-Katalog im Kopf jeder Seite steht, enthält `widget.ideas.source` den Schlüssel `ideas.source`. Die Prüfung entfernt
  jetzt zuerst das Katalogelement.
- **Seitenlauf (`tests/round39a_pages.rs`):** jede GET-Route aus `common::registered_routes` (die acht Einstellungsseiten für
  `/einstellungen/{seite}`), gefüllt mit einem reinen ASCII-Werk, einem Bild und einem Einfall, abgerufen mit
  `Accept-Language: en` und `de`. Kein Serverfehler; kein roher Katalogschlüssel im sichtbaren Text oder in den Textattributen
  irgendeiner Seite; auf einer englischen Seite kein `ä ö ü ß „ ‚` außerhalb von `KNOWN_GERMAN`. `NOT_PAGES` listet die Routen,
  die keine eigene Seite sind (Downloads, Satzausgaben, Reader-Rahmen, Routen, die eine offene Anfrage brauchen), jede mit
  Grund. `KNOWN_GERMAN` listet, was in dieser Runde deutsch bleibt, jedes mit seiner Quelle: Nutzerdaten eines neuen
  Datenordners (Beispielwerk, gespeicherte Personas, Beispielbibeln), absichtlich zitierte Namen von Buchausgaben (`Werk
  gelöscht`, `Überschrift 1–3`), Beschriftungen aus anderen Crates (`baruch-typst`-Nummerierungsschemata, Satzfelder und
  Umschlagbeschriftungen, `baruch-store` `FindingStatus::label`, `baruch-epub`-Readerprofile), `src/rag/**` (von messages_2
  nicht umgestellt) und das Mindmap-SVG (mit dem Bildexport geteilt). Einen Eintrag zu entfernen ist der Test für seine
  Behebung. Deutsch ohne Umlaute oder deutsche Anführungszeichen (Schemanamen wie „Aus der Werkvorlage“) sieht diese Prüfung
  nicht.

## Runde 39b – Buchausgabe, englische Website, Rest der Literale {#round-39b--book-output-english-website-rest-of-the-literals}

Die Schnittstelle R39b-S1 legte den Grund: der Buchkatalog `baruch-core/locales/{en,de}/book.ftl` mit `baruch_core::book_text`
(`book_t(work_language, key, args)`, Rückfall Werksprache → Englisch → Schlüssel; die deutschen Werte sind wörtlich die
Literale, die die Ausgaben vorher schrieben, die deutsche Ausgabe bleibt also byte-gleich), `baruch-core` als Abhängigkeit von
`baruch-typst`, `baruch-epub`, `baruch-docx` und `baruch-render`, die Theme-i18n-Dateien `site/themes/baruch/i18n/{de,en}.toml`
(`hilfebild_gross`), das Partial `_partials/bildpfad.html` (deutsche Bilder unter `bilder/<datei>`, jede andere Sprache unter
`bilder/<lang>/<datei>`) und die Prüfung `tests/round39b_hilfe_en.rs` für englische Seiten (gleicher Pfad, gleiches Front
Matter, gleiche Anker und Bilder wie Deutsch, keine deutschen Buchstaben in der Prosa, Links unter `/en/`). Die Buchausgabe
folgt der WERKSPRACHE (`WorkLanguage`), nie der Oberflächensprache (`i18n::Lang`).

### Buchausgabe in der Werksprache (ausgaben) {#book-output-in-the-work-language-ausgaben}

Geschrieben von der in der Überschrift genannten Aufgabe (Runde 39b).

**Katalog.** Jedes Wort, das Baruch in eine Buchausgabe erzeugt, kommt aus `crates/baruch-core/locales/{en,de}/book.ftl` über
`baruch_core::book_text::book_t(work_language, key, args)`; die deutschen Werte sind wörtlich die Literale von vorher, die
englischen der Basiskatalog. ausgaben fügte Schlüssel hinzu für Vorspann und Navigation des EPUB, Glossar- und Registerwörter,
Entwurf, Impressumszeilen, Notizzettel und die zurückgelesenen Wörter der Stellenprüfung, Behauptungen und Webprüfung.
`tests/round39b_ausgaben.rs` belegt, dass jeder in `crates/*/src` benutzte Schlüssel existiert und jeder Schlüssel benutzt wird.

**Wo die Werksprache eingeht.** `Numbering::of_scheme_in(scheme, custom, template, lang)` (`of_scheme` = Deutsch); der Store
übergibt `work.language` in `load::work_numbering` (Laden, Gliederung, Snapshot, Austauschimport), sodass Knotennummern, die
exportierte Überschriftenregel und die Teilbeschriftungen ihr folgen. `Numbering::from_template` erkennt die Regeln beider
Sprachen. `BackMatter.lang` (von `Store::back_matter` gefüllt) wählt Überschriften, `No entries.` und die Wörter der Helfer.
`EpubInput.work_language` steuert die EPUB-Beschriftungen und mit `baruch_epub::template::book_language` das Sprachkennzeichen
(§2.1); `epub_input` des Servers und der Word-Export nutzen dieselbe Funktion. `NotesDoc.lang` (aus `Notes.lang`) legt den
Notizzettel fest. `snapshot::rule_settings` fügt den Satzeinstellungen eines englischen Werks ohne eigenes `lang` `lang: "en"`
hinzu, sodass `#set text(lang: "en")` im Regelblock von Export, Exportkarte, Spiegel und Satz steht; deutsche Werke behalten
ihre Einstellungen unberührt.

**Funktionen mit `_in`.** Die alten Funktionen bleiben und sind der deutsche Fall: `xref_snippet_in` (`p.`), `draft_rule_in`,
`draft_line_in`, `draft_text_in` (das gespeicherte Standardwort `ENTWURF` wird das Katalogwort), `imprint_lines_in`,
`frame_parts_in`, `wrap_source_with_rules_in`, `front_source_with_rules_in`, `book_dict_in` (die erzeugten deutschen Literale
des Rahmens durch die Katalogwörter ersetzt), `footnote_list_in`/`endnote_list_in`, `epub_draft_in`,
`scripture::check_with_in`/`facts_with_in`/`mark_with_in`, `websearch::verify::check_in`. Die Erkennung liest beide Sprachen:
`find_xrefs` (`, S.~` und `, p.~`), `strip_back_matter`, der Chat-Fuß, `ref_claims`, die Webzusätze.

**Editor-Widgets.** `fragments/editor.html` und `fragments/lektor_editor.html` tragen `data-work-lang` (aus `view::values`,
`work_lang`); `xref.ts` (`workLangOf`, `pageAbbr`, `snippet(…, lang)`) und `wysiwyg.ts` (`workLang`) schreiben für englische
Werke `p.` und den Geviertstrich `— ` vor die Quelle eines neuen Zitats, sonst wie zuvor.

**Satzansicht (Nachtrag zu Runde 39b).** `baruch-wysiwyg` hat `emit_in`, `serialize_in`, `render_html_in`, `diff_html_in` und
`hunk_html_in` mit einer `WorkLanguage` (die alten Funktionen sind der deutsche Fall). `emit` trägt die Sprache in seinem
Kontext, sodass ein aus der Satzansicht geschriebener Querverweis mit Seitenzahl `xref_snippet_in(…, lang)` bekommt (`, p.~` in
einem englischen Werk), auch in Fußnoten, Listenpunkten, Zitaten und Überschriften; der HTML-Zusatz `span.ws-xref-page` zeigt
das Katalogwort `book.xref.page_abbr`. `parse` erkennt den Zusatz in jeder Werksprache (wie `find_xrefs`), sodass ein englisches
`, p.~` aus `xref.ts` ein Seitenzahl-Zusatz ist und kein Linktext. Der Server gibt die Werksprache in `wysiwyg.parse`,
`wysiwyg.serialize`, `wysiwyg.diff` und in der Editorseite weiter (`ProposalContext.lang`: fertige Satzansicht und
Vorschlagsstücke). Tests: `baruch-wysiwyg/tests/xref.rs` (englischer Teil) und `round39b_ausgaben.rs`
(`the_typeset_view_of_an_english_work_…`).

**Golden-Fixtures.** `crates/{baruch-typst,baruch-epub,baruch-render,baruch-docx,baruch-server}/tests/data/round39b/` wurden mit
dem unveränderten S1-Code erzeugt; `round39b_golden.rs` (Crates) und `round39b_ausgaben.rs` (Server) vergleichen byte-gleich.
`BARUCH_BLESS_R39B` gibt es nur für diesen ersten Commit.

**Außerhalb des Umfangs, in jedem Werk weiter deutsch.** Spiegel-Commit-Texte, die Mindmap-Exporte (`idea_map/files/**`), die
als Nutzerdaten gespeicherten Standard-Registertitel, die Namen des Austauschformats, die Kommentarzeilen des erzeugten Typst
(`// Baruch: Gliederungsschema „…“`, `// Stichwortzettel – …`) und CSS-Kommentare des EPUB-Stylesheets.

### Website auf Englisch (hilfe_en_1, hilfe_en_2) {#website-in-english-hilfe_en_1-hilfe_en_2}

Geschrieben von der in der Überschrift genannten Aufgabe (Runde 39b): hilfe_en_1 (Hugo-Konfiguration, Theme, die festen Seiten
und vier Hilfeseiten), hilfe_en_2 (die übrigen zehn Hilfeseiten). Bau- und Schreibregeln: `site/README.md`, Abschnitt
"Languages".

- **Konfiguration** (`site/hugo.toml`): `[languages.de]` (`contentDir = "content/de"`, `locale = "de"`, `weight = 1`, `label =
  "Deutsch"`) und `[languages.en]` (`contentDir = "content/en"`, `locale = "en"`, `weight = 2`, `label = "English"`);
  `defaultContentLanguage = "de"` mit `defaultContentLanguageInSubdir = false` behält jede deutsche Adresse der Runden 24 bis 39a,
  Englisch lebt unter `/en/`. Menüs zogen nach `[languages.<lang>.menus]` um (gleiches `pageRef` in beiden Sprachen; Hugo löst
  es innerhalb der Sprache auf), die englische Seitenbeschreibung nach `[languages.en.params]`. Das oberste `[params]` behält
  `version`, `repo`, `aptFingerprint` und `description` als einfache Zeilen `key = "value"`, weil `scripts/ci-site.sh` (awk)
  und `tests/round24_abnahme.rs` sie dort lesen. Hugo 0.168 meldet `languageName` als veraltet seit 0.158 (ein Abbruch mit
  `--panicOnWarning`); der Sprachname heißt deshalb `label`, entgegen Vertrag §2.2, der `languageName` nannte. Ebenso ist
  `.Site.Sites` seit 0.156 veraltet; `baseof.html` nutzt `hugo.Sites.Default`.
- **Paarung:** Hugo paart eine deutsche und eine englische Seite über ihren Pfad relativ zum Inhaltsordner, also wiederholt
  `content/en/` die deutschen Dateinamen (`hilfe/erste-schritte.md`, kein übersetzter Slug). Anker bleiben die deutschen
  `{#…}`-Ids, sodass ein Link `/en/hilfe/buchsatz/#pdf` `/hilfe/buchsatz/#pdf` spiegelt. Das Testmodul `round39b_hilfe_en.rs`
  prüft je Seite Pfad, Front Matter, Anker, Bildaufrufe, deutsche Buchstaben in der Prosa und `/en/`-Links.
- **Theme-Texte:** Jedes Literal der Layouts läuft über `{{ T "key" }}` mit `themes/baruch/i18n/de.toml` (der frühere Wortlaut,
  wörtlich) und `en.toml`; die Schlüssel sind deutsch wie die anderen Bezeichner des Themes. Der deutsche Bau unterscheidet
  sich von Runde 39a nur durch die `hreflang`-Alternativen und den Sprachumschalter; Text und Adressen sind unverändert.
  Interne Theme-Links nutzen `relLangURL` (Start, Installation, Funktionen, Lizenz), Assets (`favicon.svg`, `css/baruch.css`)
  bleiben bei `relURL`.
- **Sprachumschalter** (`_partials/kopf.html`): der letzte Eintrag des Hauptmenüs, ein einfaches `<a hreflang lang>` auf den
  ersten Eintrag von `.Translations`; ohne Übersetzung verlinkt er auf die Startseite der anderen Sprache
  (`site.Home.AllTranslations`) mit einem Titel, der das sagt. Kein JavaScript, kein Cookie, keine Erkennung der
  Browsersprache. `baseof.html` schreibt `<link rel="alternate" hreflang="<locale>">` für jede Übersetzung einer übersetzten
  Seite und `x-default` für die Standardsprache.
- **Bilder:** `home.html` nimmt den Pfad seines Bildes aus dem Partial `bildpfad.html` (Schnittstelle S1) und lässt das Bild
  weg, solange `static/bilder/en/arbeitsplatz.png` nicht existiert; die Shortcodes gehören fotos_tool.
- **Offen nach hilfe_en_1:** Theme-Texte in den Shortcodes (`fingerabdruck`: deutscher Hinweis; `release`: deutscher
  Standardtext, englische Seiten geben `"release page"` mit; `taste`: deutsche Tastennamen nur, wo die Seite sie übergibt)
  gehören fotos_tool; `scripts/site-check.py` prüft die GitHub-Link-Regel nur für `hilfe/`, nicht für `en/hilfe/`. Die
  Predigtvorlage und das erste Kapitel eines neuen Buchs werden in jeder Werksprache mit deutschen Titeln angelegt
  (`baruch-store/src/works.rs`); die englische Hilfe sagt das. Die Namen der Nummerierungsschemata
  (`baruch-typst/src/scheme.rs`) sind noch deutsch; die englische Hilfe nennt sie auf Englisch.

### Bildschirmfotos in zwei Sprachen (fotos_tool) {#screenshots-in-two-languages-fotos_tool}

Geschrieben von der in der Überschrift genannten Aufgabe (Runde 39b).

- **Werkzeug:** `site/tools/bildschirmfotos.py --lang de|en` (Standard `de`). Die Sprache bestimmt den Zielordner
  (`target_dir`: deutsch `site/static/bilder`, englisch `site/static/bilder/en`, Hilfebilder darunter in `hilfe/`), die
  Oberflächensprache (Cookie `baruch_lang` in jedem Rezept des Vermittlers und bei jeder Bauanfrage, `Accept-Language` zum
  Server, `intl.accept_languages` in jedem Firefox-Profil), das Beispielwerk (`beispielwerk.typ` oder `beispielwerk.en.typ`, "A
  Year on Gull Holm") mit seiner über RPC `work.language_set` gesetzten Werksprache und die erfundenen Daten (`SAMPLES[lang]`:
  jeder Text, den der Bau schreibt; die deutschen Texte sind byte-gleich zu Runde 37). `--liste`, `--abgleich` und `--pruefen`
  funktionieren für beide Sprachen ohne Firefox; `--pruefen` verlangt zusätzlich `<html lang>` = Sprache der Bilder und weder
  Home-Ordner noch Anmeldenamen in einer Seite. `--abgleich --lang L` liest `site/content/L`, endet auch bei einem fehlenden
  hellen Bild, das eine Seite nutzt, mit Exit 1, und zählt die 20-MB-Grenze je Sprache.
- **Liste:** `site/data/hilfebilder.toml` behält jedes Feld; additiv je Bild: `sprachen` (Standard beide), `adresse_en`,
  `ausschnitt_en`, `schritt_en` (fehlend = deutscher Wert). Genutzt: `adresse_en` für die Such- und Markierungsbilder. Die
  Texte von `erwartet` übersetzt die Tabelle `EXPECTED_EN` im Werkzeug (kein Feld `erwartet_en`, Vertrag §2.5); jeder Eintrag
  wird bei jedem Aufruf in beiden Sprachen geprüft.
- **Shortcodes:** `hilfebild` und `bild` nehmen ihren Pfad aus dem Partial `bildpfad.html`, stoppen den Bau mit `errorf` bei
  einem fehlenden hellen Bild in der Seitensprache (schließt den niedrigen Befund von Runde 37), halten die dunkle Fassung
  optional und nehmen den Titel des Links auf die volle Größe aus i18n `hilfebild_gross`. `bild` stoppt jetzt auch bei einem
  fehlenden `datei` oder `alt`.
- **Seitenprüfungen:** `scripts/site-check.py` (Sprachen: Wurzel `de`, `SPRACHEN_UNTER = ("en",)`) fügt die Regeln 7-10 hinzu:
  `<html lang>` je Sprache, `/bilder/en/` genau auf englischen Seiten, jedes `<a hreflang>` löst zu einer Seite auf, deren
  `<html lang>` sein `hreflang` ist (und nie die eigene Sprache; auf jeder Seite verlangt, sobald `en/index.html` existiert),
  `en/hilfe/index.html` vorhanden und kein `en/bedienung/`. Sein Selbsttest hat einen zweisprachigen Normalbaum und ein
  Gegenbeispiel je Regel. `scripts/ci-site.sh` verlangt `public/en/index.html`, wenn `content/en` existiert;
  `scripts/ci-selftest.sh` baut eine erfundene Website mit den echten Shortcodes (fehlendes deutsches Bild, fehlendes
  englisches Bild, `content/en` ohne `[languages.en]`, fehlender Sprachumschalter).
- **Tests:** `crates/baruch-server/tests/round39b_bilder.rs` liest nur Dateien (Listenfelder für beide Sprachen, `errorf` und
  Partial in den Shortcodes, `--lang` mit beiden Werten und das englische Ziel im Werkzeug).
- **Offen:** die englischen Bilder selbst (Integrator, `--fotos --lang en`); das eingebaute Beispielwerk von `baruch-core`
  bleibt in den englischen Bildern `start-werkliste` und `installation-erster-start` deutsch.

### Rest der Oberflächenliterale (rest_literals) {#rest-of-the-interface-literals-rest_literals}

Geschrieben von der in der Überschrift genannten Aufgabe (Runde 39b).

**Beschriftungen anderer Crates, vom Server übersetzt.** `baruch-typst`, `baruch-store` und `baruch-epub` behalten ihre
deutschen Beschriftungen (der Spiegel zitiert sie in Commit-Texten, ältere Tests vergleichen dagegen, der Vertrag verbietet
Änderungen dort). Der Server bildet jede Kennung mit einem erschöpfenden `match`, der den Schlüssel als Literal nennt (damit
die Waisenprüfung von `round39a_i18n` ihn sieht), auf einen Katalogschlüssel ab und übersetzt mit `i18n::t`/`tc`:

- Gliederungsschemata: `numbering::scheme_label`/`scheme_example` (Schlüssel `outline.scheme.*`), das fehlende eigene Muster
  `numbering::scheme_message`; die deutschen Texte von `NumberingInfo` (`scheme_label`, `label`) laufen durch den
  Template-Filter `scheme_ui` (`i18n::install` → `numbering::scheme_text`), der einen bekannten Namen, ein Beispiel oder „keine“
  übersetzt und ein Muster wie `I|1` in Ruhe lässt. Andere Ablehnungen der Mustersprache bleiben deutsch (offen);
- Satzeinstellungen: `typeset::ui_labels::field_label`/`choice_label` (Schlüssel `settings.typeset.field.*`,
  `settings.typeset.choice.<list>.<id>`), genutzt vom Einstellungsblock, `work.typeset_preview` und den Hinweisen der Satzseite;
  das Standard-Wasserzeichen des Formulars ist `book_t(work language, "book.draft.watermark")` (`BlockInput::language`), nicht
  die Oberflächensprache;
- Umschlag: `cover_layout::face_label_in`, `block_name_in`, `kind_label_in` (Schlüssel `cover.face.*`, `cover.block.*`);
  `cover_layout::block_line` (Antragsliste) ist für Deutsch wörtlich deutsch und für Englisch aus `cover.block_line.*` mit
  `i18n::number` gebaut, während die Commit-Texte `kind_label`/`fields` behalten; auf der Umschlagseite laufen die Angaben
  (Blätter, Millimeter) durch `i18n::number` (`cover::ui_decimal`, `format_mm`: „12,5“ / „12.5“), die Diagnosebeschriftungen
  durch `cover.page.diagnostic_*`;
- Befunde: `findings::status_label`, `kind_label` (Schlüssel `findings.status_label.*`, `findings.kind_label.*`) auf der Seite,
  im Panel, in der Matrix und in `finding_json` (RPC in der Sprache der Anfrage, MCP englisch; außerhalb einer Anfrage ist
  `i18n::request_lang` `None`, der Wortlaut des Stores bleibt deutsch);
- EPUB-Readerprofile: neue Bereichsdatei `readers.ftl` (Name, Gerät, Hinweis der neun Profile),
  `epub_preview::reader_name`/`reader_device`/`reader_note`; `src/epub_check.rs` (nicht besessen) zeigt auf der EPUB-Prüfseite
  noch die Namen aus dem Crate;
- Modellverbindungen: `models::locality` bleibt der Code `lokal|extern` (RPC, `data-locality`), die Seiten zeigen die Wörter
  `settings.models.locality_*` (`models::locality_label` für die Assistentenseite; RPC behält nur den Code);
  `models::kind_title` ist übersetzt (RPC in der Sprache der Anfrage, wie jeder Text seit Runde 39a).

Ein Unit-Test je Gruppe hält fest, dass der deutsche Katalogwert gleich der Beschriftung des Crates ist, damit deutsche Seiten
ihren Wortlaut behalten; `tests/round39a_pages.rs` verlor die zugehörigen `KNOWN_GERMAN`-Einträge.

**Sprachumschalter.** `i18n::layer` hält den Pfad einer GET-Anfrage nur mit den Ortsparametern (`i18n::BACK_PARAMS`: `work`,
`key`, `rev`, `tab`, `seite`, einfache Werte; nie ein Suchtext, ein Ziel oder ein Meldungscode aus der Adresse) in einem
Task-Local (`i18n::page_path`, Template-Funktion `page_path()`); das Menüformular von `base.html` sendet ihn als `back`, sodass
der Umschalter nicht mehr vom Referer abhängt (Befund von Runde 39a). `anmelden.html` (back = `/anmelden?next=` mit dem Ziel, das
`safe_next` bestanden hat, nie die rohe Adresse) und `einrichten.html` (nur mit dem Einrichtungsformular; die 403-Seite bleibt
ohne Formular) tragen dasselbe einfache Formular.

**Hilfelink.** `about::HELP_URL_EN` und `about::help_url(lang)`; die Template-Funktion `help_url()` gibt dem Menüeintrag „Help“
die Hilfe in der Oberflächensprache, die Über-Seite nutzt sie als `help`.

**Erlaubnisliste.** `scripts/i18n-literal-allow.txt` behält nur Bezeichner (Bandreiternamen und -ids, CSS-Klassen, Abschnitts-
und Seiten-Ids, Formular- und Abfragewerte, Routenpfade, das Handle `@alle`, die Anführungszeichen des Finders), jede Gruppe
unter einem Kommentar `# reason:` (`tests/round39b_literals.rs`). Vollständigkeit der Kataloge je Bereich und der englische
Katalog ohne Deutsch (außer aufgelisteten Ausnahmen) werden dort ebenfalls geprüft.

### Integration (Runde 39b) {#integration-round-39b}

Geschrieben vom Integrator. Aufrufstellen des Servers, die keine Aufgabe besaß, geben nun die Werksprache weiter:
`typeset::draft_rule_for_in` (Buch-PDF über `prepare_with`, Umschlag-PDF, Vorschau der Satzeinstellungen; `draft_rule_for`
bleibt der deutsche Fall), `cover::frame` ruft `frame_parts_in`, der Druckerei-Umschlag und die EPUB-Umschlagfront rufen
`wrap_source_with_rules_in` und `front_source_with_rules_in`, und `cover::cover_settings` fügt die Sprachregel von
`snapshot::rule_settings` (`lang: "en"` für ein englisches Werk ohne eigene Einstellung) den Umschlag-Textregeln hinzu;
deutsche Werke behalten byte-gleiche Quellen. `assist::run` ruft `websearch::verify::check_in` mit der Werksprache auf.
`epub_check.rs` zeigt die Readerprofile über `epub_preview::reader_name` (Oberflächensprache). `tests/round39b_hilfe_en.rs`
bekam die Vollständigkeitsprüfung (jede deutsche Seite hat eine englische Seite). Noch deutsch: das Datum der Entwurfszeile
(`28.11.2026`, `typeset::draft_today`) und neue Querverweise, die die WYSIWYG-Serialisierung schreibt (`baruch-wysiwyg` `emit.rs`
nutzt `xref_snippet`, `Inline::Xref` trägt keine Sprache).

## Runde 39c – reste_1 {#round-39c--reste_1}

Geschrieben von der in der Überschrift genannten Aufgabe (Runde 39c): Die noch deutschen Servermodule der Priorität 4 (Runde
39b) laufen über die Oberflächenkataloge, und das Entwurfsdatum folgt der Werksprache.

**Kataloge.** Neue Bereichsdateien `locales/{en,de}/rag_msg.ftl` (`src/rag/**`: Zustände der Wege, Einbettungsdienst, lokaler
Index, Prüfung von LanceDB, Nähe der Einfälle, RPC `rag.*`), `archive_msg.ftl` (`src/archive.rs`: die vier Ablehnungen von
`ArchiveError` und ihre Gründe) und `markdown_msg.ftl` (`src/import.rs` und `src/markdown_import/**`: Ablehnungen, übersprungene
Bilder, Notizen des Berichts, Export in einen Ordner, der Bericht der Kommandozeile). Neue Abschnitte am Ende von
`connections_msg.ftl` (`connections_msg.mcp.*` für `src/connections/multigpt.rs`, `connections_msg.pg.*` für
`src/connections/pgvector.rs`; die Zeitüberschreitungen und Anmeldefehler nutzen `connections_msg.check.*` wieder) und von
`ideas_msg.ftl` (`ideas_msg.mindmap.*` für `src/idea_map/bild/mindmap.rs`). Der deutsche Katalog behält den Wortlaut von vorher
wörtlich, mit einer Ausnahme: Die Ablehnung eines Indexordners innerhalb des Git-Spiegels nennt den echten Spiegelpfad statt
`<Datenordner>/works` (Katalogwerte tragen außerhalb von `_html`-Schlüsseln kein `<`).

**Sätze, die Konstanten waren.** `rag::BUSY`, `NOT_CHOSEN`, `NOT_ASKED`, `NO_COLLECTION`, `NEVER_CHECKED`,
`NO_EMBEDDER_OWN_DATA`, `rag::embed::NO_EMBEDDER`/`EXTERNAL_REFUSED`, `rag::neighbors::NO_EMBEDDER`/`NOT_INDEXED` und
`rag::index::PREFIX_CHANGED` sind jetzt Funktionen (`rag::too_busy()`, `not_chosen()`, `not_asked()`, `no_collection()`,
`never_checked()`, `no_embedder_own_data()`, `no_embedder()`, `embed::external_refused()`,
`neighbors::no_embedder()`/`not_indexed()`, `index::prefix_changed()`), die den Text jeweils in der Sprache des Aufrufers
liefern. Der Start des Dienstes (`rag::catch_up`) läuft außerhalb einer Anfrage; er merkt sich nur den Katalogschlüssel
(`NO_EMBEDDER_OWN_DATA_KEY`, `Runtime::set_unbound(&'static str)`), und `Runtime::unbound()` übersetzt ihn, wenn der Status
gelesen wird. Der Status erkennt seinen eigenen Satz über liegengebliebene Aufträge an `stale_text` (kein deutsches Wort mehr).
`import::TITLE_HINT` bleibt die deutsche Konstante (die Seite des PDF-/EPUB-Imports, `pdf_import/pages.rs`, erkennt ihn in einer
Meldung); `import::title_hint()` liefert ihn in der Oberflächensprache und ist das, was die Ablehnung eines vergebenen Titels
anhängt.

**Geltungsbereich (welche Sprache gilt).** Innerhalb einer Anfrage kommt jede Meldung aus `i18n::tc` und damit aus der Sprache
der Anfrage. Die Module starten keine eigene blockierende Aufgabe mit einer Meldung; die vorhandenen `spawn_blocking`-Aufrufe
von `import.rs` und `markdown_import` nutzen schon `i18n::carry`. Wege außerhalb einer Anfrage nutzen `i18n::current()`, das
dort Englisch ist:

- Der Hintergrund-Worker des lokalen Index (`rag::index::worker`, auf der Laufzeit von `baruch-lancedb`) führt jeden Auftrag und
  jedes aufgeschobene Zurücksetzen in `i18n::scope(Runtime::lang())` aus: die Oberflächensprache der letzten Anfrage, die einen
  Auftrag eingereiht oder ein Zurücksetzen gewünscht hat (`Runtime::note_lang`, über `i18n::request_lang`); vor jeder Anfrage
  (der Start des Dienstes, `catch_up`) ist das Englisch. Seine Probleme (`Runtime::problem`) werden wie geschrieben gezeigt;
- der Hintergrundabgleich mit MultiGPT (`crate::sync`) und ein gespeichertes Prüfergebnis (`connection_record_check`) behalten
  die Sprache, in der sie geschrieben wurden (eine von einer Seite gestartete Prüfung: die der Anfrage);
- direkte Aufrufe ohne Anfrage (Unit-Tests, Hilfscode, MCP ist ohnehin englisch).

Die Kommandozeilen-Importe sind die Ausnahme, wie seit Runde 39a: `import::import_file_as` (Typst) und, neu,
`markdown_import::cli_import` und `import::export_to_dir` (`export --out`) laufen in `i18n::scope(Lang::De, …)`, weil `main.rs`
seine eigenen Zeilen auf Deutsch ausgibt und der Bericht die Sprachen nicht mischen darf (`docx_import` und `exchange::import`
tun dasselbe). Damit ändert sich keine zitierte CLI-Ausgabe in `site/content/{de,en}/betrieb.md`.

**Was absichtlich deutsch bleibt.** Indizierter Inhalt des lokalen Index (`rag::index::idea_text` Beschriftungen „Stichworte: “,
„Zitat: “, „Quelle: “, „Bibel: “, die Ersatztitel „Einfall N“, „Quelle N“ und „Notiz:“): Er geht in die Zeilen- und
Inhaltshashes ein, ein Sprachwechsel würde jeden Index neu einbetten. Der feste Probetext `rag::PROBE_TEXT` (nur an das
Einbettungsmodell gesendet). Buchinhalt, den die Markdown-Umwandlung schreibt (`markdown_import::convert`: der Kapiteltitel „Ohne
Titel“ und die Beschriftungen „Bild: …“ nicht übernommener Bilder; ein neues Werk ist standardmäßig deutsch) und der
Spiegelvermerk „Import aus <datei>“. Die Antworten des MultiGPT-Ersatzes `connections::multigpt::mock` (sie bilden die Gegenseite
nach). Protokollzeilen auf stderr (`eprintln!`) von `src/rag/**` sind jetzt englisch.

**Mindmap.** `idea_map::bild::mindmap::render` zeichnet den Kopf der Wolke („Cloud – empty …“ / „Wolke – leer …“), den Hinweis
„fits branch …“ und das `aria-label` „Mind map: <title>“ in der Sprache der Anfrage, für das Seitenfragment und den
`.svg`/`.pdf`-Export gleichermaßen. `tests/round39a_pages.rs` listet für `src/rag/**` und die Mindmap keine deutsche Ausnahme
mehr (KNOWN_GERMAN 21 → 17 Einträge).

**Entwurfsdatum.** `typeset::draft_today_in(lang: baruch_core::WorkLanguage) -> String`: deutsch `%-d.%-m.%Y` („10.11.2026“,
byte-gleich zum früheren `draft_today`), englisch ISO 8601 `%Y-%m-%d` („2026-11-10“, keine Wörter, also kein Schlüssel in
`book.ftl`). `draft_today()` bleibt und liefert die deutsche Form. `draft_rule_for_in` (Buch-PDF und Vorschau über
`prepare_with`, Umschlag-PDF, Vorschau der Satzeinstellungen), `epub::epub_draft_in` (EPUB) und die Stichwortzettel
(`pdf::notes`) geben die Werksprache weiter.

**Tests.** `tests/round39c_reste_1.rs`: eine Meldung je umgestelltem Modul in beiden Oberflächensprachen (Cookie `baruch_lang`),
der deutsche CLI-Bericht des Markdown-Imports, die Wolke der Mindmap, das Entwurfsdatum eines englischen und eines deutschen
Werks (Regel des Wasserzeichens, EPUB-Vermerk). Vorhandene Tests, die deutsche Sätze über Konstanten verglichen
(`round12_lancedb.rs`, `round19_index.rs`), lesen sie über `i18n::sync_scope(Lang::De, …)` auf Deutsch; die Unit-Tests von
`archive.rs` und die TLS-Warnung von `pgvector.rs` laufen im deutschen Bereich.

## Runde 39c – reste_2 {#round-39c--reste_2}

Meldungen der Crates, Voreinstellungen in der Werksprache, Wörter der Shortcodes und Katalogwortlaut. Alles Deutsche bleibt
Byte für Byte; Englisch ist neu.

- **Voreinstellungen in der Werksprache.** `baruch_store::NewWork` hat `pub language: WorkLanguage` (als `crate::WorkLanguage`
  wieder exportiert). `create_work`, `create_work_with_text` und `create_work_with_bible` schreiben sie nach `work.language`
  (Spalte der Migration 0055, keine neue Migration) und nehmen die Titel der Voreinstellung aus dem Buchkatalog: Schlüssel
  `book.preset.*` am Ende von `crates/baruch-core/locales/{en,de}/book.ftl` (Deutsch = die alten Wörter).
  `sermon_outline_in(lang)` und `sermon_text_title_in(lang)` liefern die Punkte; `SERMON_OUTLINE` und `SERMON_TEXT_TITLE` bleiben
  (deutsch) aus Verträglichkeit. Der Predigttext landet im Punkt, dessen Titel gleich `sermon_text_title_in(lang)` ist (dem
  ersten). RPC `work.create` nimmt das optionale `language` (`de`|`en`, Standard `de`; alles andere -32602,
  `works_msg.rpc.language_invalid`); das Formular `/werke/neu` hat das Auswahlfeld `language`, vorbelegt mit `i18n::current()`,
  ein fehlendes Feld bedeutet `de`. Die Zahlwörter des Gliederungsschemas (`Scheme::Sermon`) gehören nicht zur Voreinstellung
  (die beim Anlegen gespeicherte vorläufige Nummer wird vom Schema neu berechnet).
- **Store-Texte.** `store_text::PATTERN_KEYS` wuchs um 66 Schlüssel: Persona-Prüfungen, Antragsablehnungen, Einfälle,
  Einfallsverbindungen und die Validierung der Satzeinstellungen von `baruch-typst`. Zuerst die gemessene Liste (Skript über
  `baruch-store/src` und `baruch-typst/src/typeset`: wörtliche Bruchstücke des deutschen Katalogtexts, die im Quelltext
  gefunden werden; der deutsche Text eines Schlüssels wird beim Schreiben gegen den Quelltext geprüft). Noch deutsch (gemeldet,
  aus einem Formular nicht erreichbar oder nur über seltene Wege): Berichtszeilen des Imports in `exchange_apply.rs` (rund 70
  Texte), Ablehnungen von `glossary.rs` und Register (rund 20), Überschriftenprüfungen in `outline.rs` mit Ebenenwörtern (`Die
  Fassung bringt Ebene … mit`, `level_word` bleibt deutsch), Überspringgründe in `replace.rs` (`ReplaceSkip.reason`),
  Feldfehler in `cover_blocks.rs` (`Umschlag-Block „…“, Feld …`), `sheet.rs` (`Satzbild: …`),
  Integritätsmeldungen in `load.rs`/`seed.rs`/`typeset.rs` (technisch, keine Nutzereingabe) und Spiegelnotizen in
  `amend.rs`/`write.rs`.
- **Bibelnotizen.** Die Wortbruchstück-Notiz von `bibles::module_note` wird als Daten (`FragmentFacts`) zwischengespeichert,
  nicht als Text, sodass der Cache von einer Sprache unabhängig ist. `module_note` schreibt die Oberflächensprache
  (`reference_msg.bibles.*`, von `scripture.check`-JSON genutzt), `module_note_in(bible, WorkLanguage)` die Werksprache
  (`book.scripture.fragment_*`, vom Faktenblock und vom Schlussvermerk einer Persona-Antwort genutzt). Ablehnungen von
  `scripture.check` nutzen `reference_msg.scripture.*`. `bibles::CIPHER_KEY_WRONG` bleibt deutsch, weil `connections/pages.rs` es
  nach Wert vergleicht (Datei gehört nicht dieser Aufgabe).
- **Satzeinstellungen und Nummerierung.** `typeset_settings::field_label` (Oberflächensprache, Unterfelder nennen ihr Feld) und
  `conflict_message` ersetzen die deutsche Beschriftung und Meldung des Crates in `work.typeset_get`/MCP `typeset_settings`.
  `numbering::scheme_message` erkennt die vier Musterablehnungen von `baruch_typst::scheme::Numbering::parse_spec` per Regex und
  bildet sie auf `settings_msg.numbering.err_*` ab; das Crate behält seine deutschen Meldungen (Meldungscodes waren nicht
  nötig). `cover_print::name` nutzt `cover_layout::kind_label_in(i18n::current(), kind)`.
- **Tooltip eines Querverweises.** `baruch_wysiwyg::render_html_titled(doc, images, lang, xref_title)` nimmt die Funktion, die
  den ganzen `title` schreibt; `render_html_in` (Signatur unverändert) nutzt den deutschen Text. Der Server übergibt
  `wysiwyg::xref_title` (`reference_msg.xref.tooltip`) an seinen zwei Aufrufstellen (`wysiwyg.rs`, `view.rs`). Die Diff-Funktionen
  behalten den deutschen Tooltip.
- **Shortcodes.** `fingerabdruck.html`, `release.html` und `taste.html` nehmen ihre Wörter aus `shortcode_*`-Schlüsseln von
  `site/themes/baruch/i18n/{en,de}.toml`; die deutschen Werte sind die alten Wörter (geprüft: die `hugo`-Ausgabe der deutschen
  Website ist identisch zum Bau davor). Der Platzhalter `FINGERABDRUCK-TRÄGT-DER-NUTZER-EIN` ist eine Marke, die
  `scripts/ci-fingerprint.sh` sucht, und wird nicht übersetzt.
- **Katalogwortlaut (Englisch).** Persona und Rolle heißen in den Oberflächentexten „Lector“; `Exchange: Write out`/`Exchange:
  Read in` angeglichen (Schaltfläche, Überschrift, Link); „revisions“ statt „versions“ in `settings.work.fold_*`; die
  Schaltflächen für fett und kursiv im Band lauten im Englischen `B` und `I` (`editor.toolbar.*_glyph`, deutsch `F`, `K`).
