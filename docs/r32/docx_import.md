# Runde 32 – Bausteine der Aufgabe docx_import (Word-Import)

Fertige Abschnitte für den Integrator; er setzt sie in `docs/Bedienung.md`, `docs/RPC.md`, `docs/Architektur.md`,
`CHANGELOG.md` und `README.md` ein und löscht diese Datei.

## Bedienung

### Word-Datei importieren

**Baruch ▾ → Word (.docx) importieren…** (Adresse `/werke/import/word`) liest eine Word-Datei und legt daraus ein neues
Werk an – in zwei Schritten, ganz ohne JavaScript bedienbar:

1. **Lesen.** Datei wählen (`.docx`, höchstens 64 MiB), auf Wunsch einen Titel eintragen, „Lesen und Vorschau zeigen“.
   Angelegt wird dabei nichts außer einem Entwurf; die Datei bleibt auf diesem Rechner.
2. **Prüfen und übernehmen.** Die Vorschau zeigt
   - die **Gliederung** (Teile und Überschriften der Ebenen 1 bis 3, je mit Länge und Textprobe),
   - die **Bearbeiter** aus der Änderungsverfolgung: Name in Word, Name in Baruch (`@word:<name>`), Zahl der
     Einfügungen und Löschungen und in wie vielen Abschnitten daraus ein Vorschlag wird,
   - die Zahl der **Kommentare**, Fußnoten und Bilder,
   - alle **Hinweise**: was nicht oder nur vereinfacht übernommen wird.

   „Als Werk übernehmen“ legt das Werk an (Titel änderbar; zwei Häkchen schalten Änderungen bzw. Kommentare ab),
   „Verwerfen“ löscht den Entwurf. Entwürfe verfallen nach 24 Stunden.

**Änderungen und Kommentare werden nie zur aktuellen Fassung.** Es gilt:

- **Fassung 1** jedes Abschnitts ist der Stand **vor** allen verfolgten Änderungen (Gelöschtes steht noch da,
  Eingefügtes noch nicht) und ist die aktuelle Fassung.
- Die Änderungen **jedes Bearbeiters** werden je geändertem Abschnitt **ein offener Vorschlag** unter
  `@word:<name>` (Modell „word“) mit der Notiz „Änderungen aus Word von <Name>, <Datum>“. Haben zwei Bearbeiter
  denselben Abschnitt geändert, gibt es zwei Vorschläge, jeder gegen Fassung 1. Übernehmen, verwerfen oder mischen
  kannst nur du – wie bei jedem Vorschlag.
- **Überschriften** ändern nie die Gliederung: Eine in Word geänderte Überschrift wird ein Beitrag am Abschnitt
  („Überschrift in Word geändert: „vorher“ → „nachher““). Eine vom Bearbeiter eingefügte Überschrift legt keinen
  Abschnitt an (ihr Text steht fett im Vorschlag zum vorangehenden Abschnitt), eine gelöschte bleibt bestehen – beides
  steht als Hinweis da.
- **Kommentare** werden Beiträge am Abschnitt der kommentierten Stelle: erste Zeile Name und Datum, dann die Stelle
  als Zitat (höchstens 200 Zeichen), dann der Kommentar; Antworten stehen direkt dahinter („Antwort auf <Name>“). Ein
  Kommentar ohne auffindbare Stelle steht am ersten Abschnitt.

Der Name in Baruch entsteht aus dem Namen in Word: klein, ä→ae, ö→oe, ü→ue, ß→ss, alles andere als Bindestrich,
höchstens 40 Zeichen („Jörg Müller“ → `@word:joerg-mueller`); ohne Namen `@word:unbekannt`.

**Was übernommen wird:** Überschriften (Vorlagen „Überschrift 1–3“/„Heading 1–3“, auch abgeleitete Vorlagen und die
Gliederungsebene des Absatzes), Titel (Vorlage „Titel“), fett, kursiv, unterstrichen, durchgestrichen, hoch-/tiefgestellt,
Kapitälchen, Links, Fußnoten (`#footnote[…]`), Blockzitate (Vorlage „Zitat“), Listen (geordnet, ungeordnet,
verschachtelt), Tabellen (als Typst-Tabelle, im WYSIWYG-Editor eine Insel), eingebettete Bilder (PNG, JPEG, GIF, WebP,
SVG) mit Alt-Text und Bildunterschrift, Seitenumbrüche. Text vor der ersten Überschrift bekommt ein eigenes erstes
Kapitel mit dem Titel des Werks; ein Dokument ganz ohne Überschrift wird ein Kapitel.

**Hinweise (feste Codes) – nichts wird still verworfen:**

| Code | Bedeutung |
|---|---|
| `docx_ueberschrift_tief` | Überschrift 4–6 gehört nicht zur Gliederung; sie steht fett im Abschnitt |
| `docx_text_vor_ueberschrift` | Text vor der ersten Überschrift steht in einem eigenen ersten Kapitel |
| `docx_ueberschrift_eingefuegt`, `docx_ueberschrift_geloescht` | vom Bearbeiter eingefügte bzw. gelöschte Überschrift (Gliederung bleibt) |
| `docx_vorspann_eingefuegt` | vom Bearbeiter eingefügter Text vor der ersten Überschrift – nicht übernommen |
| `docx_formataenderung` | verfolgte Formatänderungen: es gilt die Formatierung des Endstands |
| `docx_verschoben` | verschobener Text steht als Löschung an der alten und Einfügung an der neuen Stelle |
| `docx_textfeld`, `docx_objekt` | Textfeld, Zeichnung, eingebettetes Objekt – nicht übernommen (Textfeld mit Textanfang) |
| `docx_formel` | Formel nur als einfacher Text |
| `docx_tabelle_vereinfacht` | verbundene Zellen, ungleich lange Zeilen, eingefügte/gelöschte Tabellenzeilen |
| `docx_bild_format`, `docx_bild_extern`, `docx_bild_uebersprungen` | Bildformat nicht unterstützt (z. B. EMF/WMF), nur verknüpftes Bild (wird nie abgerufen), Bild zu groß |
| `docx_endnote`, `docx_fussnote_absaetze` | Endnoten als Fußnoten; mehrere Absätze einer Fußnote als Zeilen |
| `docx_verborgener_text`, `docx_symbol`, `docx_unbekannt` | verborgener Text, Zeichen aus Symbolschriften, unbekanntes Element |
| `docx_kommentar_ohne_stelle` | Kommentar ohne auffindbare Stelle steht am ersten Abschnitt |
| `docx_gliederung_abweichend`, `docx_vorschlag_abgelehnt`, `docx_beitrag_abgelehnt` | ein Vorschlag oder Beitrag ließ sich nicht anlegen (das Werk besteht trotzdem) |

Nicht gelesen werden das alte Format `.doc` und mit Kennwort verschlüsselte Dateien (Meldung im Klartext). Über MCP
gibt es keinen Import.

**Kommandozeile:**

```
baruch-server import DATEI.docx [--uebernehmen] [--title "TITEL"] [--data ORDNER]
```

Ohne `--uebernehmen` nur die Vorschau (Gliederung, Bearbeiter, Zahlen, Hinweise) – es wird nichts geschrieben, auch
kein Entwurf. Mit `--uebernehmen` entsteht das Werk samt Vorschlägen und Beiträgen; die Ausgabe nennt die Zahlen.
Eine unlesbare Datei endet mit Exit 1 und dem Grund.

## RPC

### docx_import.preview, docx_import.get, docx_import.apply, docx_import.discard (Runde 32 – Word)

Global (ohne Werk; Weiche in `rpc::call` über `docx_import::handles`, Modul `docx_import`). Kein MCP-Gegenstück: Ein
Import legt aktuelle Fassungen unter dem Namen des Menschen an.

- **`docx_import.preview {file_name, data_base64, title?}`** – `data_base64` = Datei als Base64 (Standard, mit `=`);
  `file_name` endet auf `.docx`. Liest die Datei und legt nur einen **Entwurf** an
  (`<data>/docx-drafts/<draft>/` mit `draft.json` und `source.docx`; kein Werk). Antwort:
  `{draft, file_name, title, sections: [{level 0..3, numbered, title, chars, sample}], editors: [{name, handle,
  insertions, deletions, sections}], comments, footnotes, images, warnings: [{code, message}]}`.
  `title` = Titel, unter dem übernommen würde (eigener `title`, sonst Titel der Datei, sonst Dateiname ohne Endung);
  `level` 0 = Teil; `sample` = Anfang des Abschnitts als Typst (höchstens 160 Zeichen); `editors[].sections` = Zahl
  der Abschnitte, zu denen ein Vorschlag entstünde; `handle` = `@word:<name>`.
  Fehler: kein `.docx`-Name, kaputtes Base64, größer als 64 MiB, Fehler der Datei (kein ZIP, altes `.doc` oder
  verschlüsselt, unsicherer Pfad oder Beziehungsziel, DTD, Grenze überschritten, kaputtes XML, kein Text) → -32602 mit
  dem Grund; es entsteht nichts.
- **`docx_import.get {draft}`** → dasselbe Objekt wie `preview`. Unbekannt, verworfen, übernommen oder älter als 24 h
  → -32602.
- **`docx_import.apply {draft, title?, changes?: bool = true, comments?: bool = true}`** – legt das Werk an (Fassung 1
  je Abschnitt = Stand vor allen Änderungen, aktuell), mit `changes` je Bearbeiter und geändertem Abschnitt einen
  offenen Vorschlag (Persona `@word:<name>`, Modell `word`) und je geänderter Überschrift einen Beitrag, mit `comments`
  je Kommentar einen Beitrag. Antwort: `{work, slug, sections, proposals, posts, warnings: [{code, message}]}`. Der
  Entwurf ist danach gelöscht. Fehler: unbekannter Entwurf → -32602; vergebener Titel → -32000 (wie
  `source_import.apply`, der Entwurf bleibt); derselbe Entwurf wird gerade übernommen → -32000. Ließ sich ein
  einzelner Vorschlag oder Beitrag nicht anlegen, bleibt das Werk bestehen und `warnings` nennt es
  (`docx_vorschlag_abgelehnt`, `docx_beitrag_abgelehnt`, `docx_gliederung_abweichend`).
- **`docx_import.discard {draft}`** → `{discarded: true}` (auch wenn der Entwurf schon weg ist). `draft` ist kein
  32-stelliger Hex-Name → -32602.

Höchstens 20 Entwürfe, je höchstens 24 Stunden alt; beim nächsten `preview` fallen ältere weg.
`work.import` und `POST /werke/import` weisen `.docx` mit Verweis auf `/werke/import/word` ab (S1).

Seiten (ohne JavaScript): `GET /werke/import/word` (Formular; mit `?draft=<id>` die Vorschau, unbekannter Entwurf 422),
`POST /werke/import/word/vorschau` (multipart `file`, `title` → 303 `/werke/import/word?draft=<id>`; Fehler 422, zu
groß 413, jeweils mit Formular), `POST /werke/import/word/uebernehmen` (`draft`, `title`, Häkchen `changes`,
`comments` → 303 `/?work=<id>`; entstanden beim Übernehmen neue Hinweise, 200 mit Bericht und Link zum Werk; Fehler
422 mit der Vorschau), `POST /werke/import/word/verwerfen` (`draft` → 303 `/werke/import/word`). Alle POST hinter
`guard::same_origin` (fremde Herkunft 403).

## Architektur

### Runde 32 – Word-Import (docx_import)

**Crate `baruch-docx`, Leserseite** (rein, ohne Datenbank und Netz):

- `read::read(bytes, &Limits) -> ReadDocx { document, warnings }` – `read/pack.rs` öffnet das ZIP (Größe der Datei,
  Zahl der Einträge, erklärte Größen vor dem Entpacken; jeder Eintrag über `take(Grenze + 1)`, die Summe des wirklich
  Entpackten zählt mit; Namen mit `..`, führendem `/`, Rückstrich oder Laufwerk → `Unsafe`; OLE-Anfang → `NotDocx`
  „altes .doc oder verschlüsselt“) und löst Beziehungen auf (Ziel außerhalb des Archivs ohne `TargetMode="External"`
  → `Unsafe`; externe Ziele werden nie abgerufen, ein nur verknüpftes Bild ergibt `docx_bild_extern`).
  `read/xml.rs` baut je Teil einen kleinen Baum mit quick-xml: DTD → `Unsafe`, nur die fünf festen Entitäten und
  Zeichenverweise werden aufgelöst, Tiefe und Elementzahl begrenzt. Elemente heißen wie in der Datei (`w:p`) –
  Annahme: die üblichen Präfixe. `read/body.rs` bildet den Textkörper auf das Modell ab: Rolle der Absatzvorlage
  (ID `Heading N`, Name „heading N“/„Überschrift N“, `basedOn`, `w:outlineLvl` in Vorlage oder Absatz, die
  Baruch-Vorlagen aus `ooxml.rs`), Läufe mit Zeichenformaten, `w:ins`/`w:del`/`w:moveFrom`/`w:moveTo` als
  `Inline::Tracked`, Änderung der Absatzmarke als `mark`, Fußnoten/Endnoten inline, Kommentarbereiche, Links,
  Bilder (DrawingML und VML; Medientyp aus den ersten Bytes), Listen über `numbering.xml`, Tabellen, Typst-Inseln
  (`w:sdt` mit `w:tag="baruch-typst"`: der mit `w:vanish` verborgene Text ist der Quelltext, Absätze sind Zeilen).
  Warnungen je Code einmal mit der Zahl der Stellen.
- `resolve::resolve(document, View)` löst die Änderungsverfolgung auf (`Original`, `Final`, `FinalBy(name)`); eine
  nicht angewandte eingefügte Absatzmarke lässt den Absatz im folgenden aufgehen, eine angewandte gelöschte lässt ihn
  den folgenden aufnehmen. `change_authors` nennt die Bearbeiter in der Reihenfolge des ersten Auftretens.
  `proposal_view(document, name)` ist `FinalBy` mit den Überschriften des Originals und liefert `HeadingChange`
  (`before` leer = eingefügt, `after` leer = gelöscht; `section` zählt wie `to_typst` am Original, ein Vorspann vor
  der ersten Überschrift ist Abschnitt 0).
- `to_typst::to_typst(document) -> TypstOut { sections, warnings }` schreibt je Abschnitt Typst in den Formen des
  Markdown-Imports (Text maskiert, Auszeichnung als Funktion, `#quote(block: true, attribution: […])[…]`, Listen mit
  `-`/`+`, `#table(…)`, `#figure(image("…", alt: "…"), caption: […])`) und sammelt je Abschnitt die Kommentar-Anker
  (kommentierter Wortlaut). Text vor der ersten Überschrift wird ein Abschnitt mit leerem Titel.

**Server `docx_import`** (`src/docx_import/mod.rs`, `draft.rs`, `pages.rs`, Vorlage `docx_import.html`):

- `prepare(bytes) -> Prepared` (rein, in `spawn_blocking`): liest, schreibt das Original als Abschnitte, legt je
  Bearbeiter den Vorschlagsstand auf die Abschnitte des Originals (nur abweichende Abschnitte zählen), ordnet
  Kommentare ihren Abschnitten zu (zuerst am Original, sonst am Stand eines Bearbeiters) und sammelt die Warnungen.
- `preview` legt den Entwurf ab (`draft.rs`, gehalten wie `pdf_import/draft.rs`: 32 Hex, 20 Stück, 24 Stunden);
  `apply` liest die abgelegte Datei neu und ruft `apply_prepared`:
  1. Werk über `import::import_generated` (ein Commit „Import aus <datei>“, Bilder als `ImportAsset`),
  2. unter der Schreibsperre je Bearbeiter und Abschnitt `Store::add_proposal(…, "@word:<name>", "word", Notiz)` –
     der Text ist die aktuelle Fassung mit ersetztem Abschnittstext, die Überschriftszeile bleibt wörtlich –, danach
     `mirror::side_refs_after_proposal` (Seitenzweige `refs/revisions/<key>/<n>` mit Persona und Modell),
  3. Beiträge über `Store::add_post` (geänderte Überschriften, Kommentare mit `TextRef` auf Fassung 1, Antworten
     direkt hinter ihrem Kommentar).
  Nie `set_current`: Fassung 1 bleibt die aktuelle. Ein Fehler in 2 oder 3 wird als Warnung gemeldet, das Werk
  bleibt.
- `cli_import` teilt `prepare`/`apply_prepared`; ohne `--uebernehmen` wird nichts geschrieben.

**Annahmen (ohne echtes Word geprüft):** Die Tests bauen ihre Word-Dateien selbst nach ECMA-376 und `ooxml.rs`; eine
von Microsoft Word oder LibreOffice geschriebene Datei wurde nicht gelesen. Angenommen ist insbesondere: Präfixe wie
oben; Antworten auf Kommentare über `commentsExtended.xml` (`w15:paraIdParent` ↔ `w14:paraId` des Kommentarabsatzes);
die Träger der Typst-Insel wie in `ooxml.rs` beschrieben (mehrzeiliger Quelltext als mehrere verborgene Absätze oder
mit `w:br`).

## CHANGELOG

- **Word-Import:** „Baruch ▾ → Word (.docx) importieren…“ liest eine Word-Datei, zeigt Gliederung, Bearbeiter und
  Hinweise und legt nach Bestätigung ein Werk an. Fassung 1 ist der Stand vor allen verfolgten Änderungen; die
  Änderungen jedes Bearbeiters werden offene Vorschläge unter `@word:<name>`, Kommentare werden Beiträge an der
  kommentierten Stelle – nie eine aktuelle Fassung. Auch über RPC (`docx_import.preview/get/apply/discard`) und die
  Kommandozeile (`baruch-server import DATEI.docx [--uebernehmen]`); nicht über MCP.

## README

- **Word (.docx) importieren:** Gliederung, Fußnoten, Zitate, Listen, Tabellen und Bilder werden ein Werk; verfolgte
  Änderungen kommen je Bearbeiter als offene Vorschläge, Kommentare als Beiträge – entschieden wird in Baruch.

## Bitten an den Integrator

1. **`crates/baruch-server/tests/round33_seiten.rs`, Liste `TEMPLATES`:** Der Wächtertest
   `every_template_puts_its_actions_as_literal_groups_into_the_block_of_its_tab` verlangt jede Vorlage, die
   `base.html` erweitert, in der Liste (und `seen == TEMPLATES.len()`). Für die neue Vorlage fehlt die Zeile (die
   Datei gehört dem Integrator, darum nicht in diesem Commit):

   ```rust
   ("docx_import.html", "start", &["href=\"/werke\"", "href=\"{{ back_url }}\""]),
   ```

   Mit genau dieser Zeile (örtlich geprüft, wieder zurückgenommen) sind `round33_seiten` und `round32_docx_import`
   grün; ohne sie scheitert dieser eine Test. Dasselbe gilt sinngemäß für `docx_export.html` und `shutdown.html`.
2. **Festlegungen, die der Schreiber kennen sollte** (keine Änderung an eingefrorenen Dateien nötig):
   - `to_typst` gibt Text vor der ersten Überschrift als ersten Abschnitt mit **leerem Titel**; der Server setzt den
     Titel des Werks ein.
   - `HeadingChange`: `before` leer = eingefügte Überschrift, `after` leer = gelöschte; `section` zählt wie
     `to_typst(resolve(Original))` (in `resolve.rs` dokumentiert).
   - Typst-Insel beim Lesen: Quelltext = Text aller Läufe mit `w:vanish` im `w:sdt`; mehrere verborgene Absätze
     werden Zeilen, `w:br` ein Zeilenumbruch, `w:tab` ein Tabulator. Schreibt `write` es anders, bitte melden.
   - Leser erkennt Listen an `w:numPr` (nicht an `ListParagraph`); ohne `numbering.xml` gilt `NUM_DECIMAL` als
     geordnet, alles andere als Aufzählung.
