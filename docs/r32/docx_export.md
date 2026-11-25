# Runde 32 – Baustein docx_export (Word-Export)

Fertige Abschnitte für den Integrator (Vertrag §5 g). Diese Datei wird im Abschluss eingesetzt und gelöscht.

**Ehrlich vorweg:** Die Datei ist mit Tests auf Aufbau, Wohlgeformtheit, Vorlagen, Beziehungen und Inhalt geprüft.
Zusätzlich hat der Entwickler eine erfundene Probedatei (Beispieldokument plus ein Abschnitt mit Tabelle, Liste,
Zitat, Bild, Inseln und Änderungsverfolgung) mit dem auf dem Rechner vorhandenen LibreOffice **ohne Fenster** nach
PDF umgewandelt (`soffice --headless --convert-to pdf`, eigenes Profil) und das Ergebnis angesehen: LibreOffice öffnet
die Datei, zeigt Titelei, Überschriften, Fußnoten am Seitenende, Liste mit Zählung, Tabelle, grau hinterlegte Inseln
und die Änderungen farbig. **In Word hat die Datei niemand geöffnet**, und auch in LibreOffice hat niemand am
Bildschirm damit gearbeitet (Navigator, Annehmen/Ablehnen, Vorlagennamen) – das prüft nur der Nutzer.

## Bedienung

### Word-Export für den Verlag

**Buch → Export → „Word …“** (oder `/werke/<id>/word`) öffnet eine Seite mit Erklärung und einem Formular. Der Knopf
„Word-Datei herunterladen“ liefert `<ordnername>.docx`. Das geht ohne JavaScript (gewöhnliches Formular).

In der Datei steht:

- die **Titelei** aus den Werkangaben: Titel, Untertitel, Autor; mit eingeschalteter Titelei (Buchangaben) auch die
  Zeilen des Impressums. Dieselben Angaben stehen in den Dateieigenschaften.
- danach alle Abschnitte in Lesereihenfolge mit ihrer **aktuellen Fassung**:
  - Überschriften mit den Formatvorlagen „Überschrift 1–3“ (tiefere Überschriften im Text „Überschrift 4–6“), Teile
    mit „Baruch Teil“, Überschriften ohne Nummer mit „Baruch ohne Nummer 1–3“. Die Nummern der Gliederung selbst
    stehen nicht in der Datei – die Zählung richtet der Verlag an den Vorlagen ein.
  - fett und kursiv, erzwungene Zeilenumbrüche, Verweise (`#link`) als anklickbare Verweise;
  - **Fußnoten als echte Fußnoten**;
  - Blockzitate (Vorlage „Zitat“) mit Quelle („Baruch Zitatquelle“), Zitate im Text (Zeichenvorlage „Baruch Zitat“,
    ohne Anführungszeichen – siehe Grenzen);
  - Listen und Aufzählungen (auch geschachtelt; jede Aufzählung zählt für sich und beginnt bei ihrer ersten Nummer);
  - einfache Tabellen (`#table(columns: …, […], …)`, auch mit `table.header`) als Word-Tabellen;
  - **Bilder eingebettet** (PNG, JPEG, GIF) mit Alternativtext und Bildunterschrift.
- Was Word nicht abbilden kann – eigene Typst-Funktionen, `#let`/`#set`/`#show`, Formeln, Begriffslisten, Szenen,
  Verweise `@name`, Tabellen mit verbundenen Zellen, Bilder in anderen Formaten (SVG, WebP) –, steht als
  **Typst-Insel** in der Datei: ein grau hinterlegtes Feld mit dem sichtbaren Quelltext bzw. dem lesbaren Inhalt;
  der wörtliche Quelltext liegt verborgen dahinter. Nichts geht still verloren. Über RPC und Kommandozeile nennt
  der Export solche Stellen als Warnung (`docx_typst_insel`, `docx_bild_fehlt`, `docx_tabelle_vereinfacht`); die
  Seite im Browser liefert die Datei ohne Bericht.

**Offene Vorschläge als Änderungsverfolgung:** Mit dem Häkchen auf der Seite steht je Abschnitt der **jüngste offene
Vorschlag** als Einfügung und Löschung in der Datei („Überarbeiten“ in Word), mit dem Handle der Persona als
Bearbeiter (z. B. `@lektor`) und dem Zeitpunkt, zu dem der Vorschlag angelegt wurde. Ausgangstext bleibt die aktuelle
Fassung: „Alle ablehnen“ ergibt sie wieder, „Alle annehmen“ den Vorschlag. Hat ein Abschnitt mehrere offene
Vorschläge, steht nur der jüngste in der Datei (Warnung `docx_weitere_vorschlaege`). Ausgeblendete Fassungen zählen
nicht. Die Seite nennt vorab, wie viele Vorschläge offen sind.

Der Export **ändert nichts am Werk**: keine Fassung, kein Vorschlag, kein Eintrag im Spiegel.

Grenzen (ehrlich):

- Das Öffnen in Word und das Arbeiten mit der Datei in LibreOffice prüft nur der Nutzer (siehe oben).
- Zitate im Text tragen in Word keine Anführungszeichen (Typst setzt sie beim Satz selbst); der Verlag sieht sie an
  der Zeichenvorlage „Baruch Zitat“.
- Änderungsverfolgung: Geänderte Wörter stehen wortweise im Absatz. Eine ganz neue oder gestrichene Abbildung
  erscheint als Absatz mit dem Bild als Änderung; bei einer ganz neuen oder gestrichenen Tabelle ist der Inhalt jeder
  Zelle verfolgt, das leere Gitter bleibt nach „Annehmen“ stehen. Seitenumbrüche werden nicht verfolgt (es gelten die
  des Vorschlags). Wechselt eine Überschrift die Ebene, steht sie als gestrichen und neu da.
- Der Word-Export kennt keine Kommentare aus Baruch (Beiträge gehen nicht mit).
- Über MCP gibt es den Word-Export nicht.

### Kommandozeile

```
baruch-server export ID|SLUG --docx DATEI [--vorschlaege] [--data ORDNER]
```

Schreibt das Werk als Word-Datei nach `DATEI`; `--vorschlaege` gibt die offenen Vorschläge als Änderungsverfolgung
mit. Die Ausgabe nennt Datei, Größe, die Zahl der Vorschläge und alle Warnungen. Eine vorhandene Datei wird nie
überschrieben (Exit 1).

## RPC

### Runde 32 – Word: `work.docx_export`

Nur lesend; kein MCP-Werkzeug.

`work.docx_export {work?: number|string, proposals?: bool = false}`

- `work`: Werk-id (Zahl), Ordnername des Werks (Text) oder – ohne Angabe – das Vorgabewerk.
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
  `docx_bild_fehlt` (Bildpfad ohne einbettbares Bild: fehlt im Werk oder ist SVG/WebP), `docx_tabelle_vereinfacht`
  (Gestaltung einer Tabelle entfiel), `docx_weitere_vorschlaege` (mehr als ein offener Vorschlag im Abschnitt).
- Fehler: unbekanntes Werk oder unbekannter Parameter → `-32602`; Schreibfehler → `-32603`.
- Gleicher Stand des Werks ergibt dieselben Bytes (der Zeitpunkt in der Datei ist die jüngste Änderung des Werks,
  nicht die Uhrzeit des Exports).

Seiten dazu (kein RPC): `GET /werke/{id}/word` (Erklärung und Formular), `GET /werke/{id}/word.docx[?vorschlaege=1]`
(Datei, `Content-Type: application/vnd.openxmlformats-officedocument.wordprocessingml.document`,
`Content-Disposition: attachment; filename="<slug>.docx"`; unbekanntes Werk 404, anderer Wert für `vorschlaege` 400,
Anfrage einer fremden Seite 403).

## Architektur

### Word-Export (`baruch-docx` Schreiber, `baruch-server/src/docx_export`)

Der Weg vom Werk zur Datei hat drei reine Stufen im Crate `baruch-docx` und eine lesende im Server:

1. **`from_typst(body, image_paths)`** (`from_typst.rs`, `from_typst/args.rs`): liest einen Abschnitt mit
   `baruch_wysiwyg::parse` und bildet die Blöcke auf das Modell ab. Abbildungen (`#figure(image(…))`, `#image(…)`)
   werden `Block::Figure` mit wörtlichem `raw`, einfache Tabellen `Block::Table`, `#pagebreak()` `Block::PageBreak`.
   Alles andere wird `Block::Typst`/`Inline::Typst` mit wörtlichem Quelltext und sichtbarem Ersatz (Warnung
   `docx_typst_insel`); Unsichtbares (Label, Registermarke, Kommentar) geht ohne Warnung als Insel ohne Sichtbares
   mit, Querverweise mit ihrem Text.
2. **`track_changes(current, proposed, change)`** (`diff.rs`): richtet die Blöcke über die längste gemeinsame
   Teilfolge aus, ordnet in jeder Lücke ähnliche Blöcke gleicher Art einander zu (Anteil gemeinsamer Wörter) und
   vergleicht sie wortweise; übrige Blöcke sind ganz neu oder entfallen ganz (`mark` an der Absatzmarke, Inhalt in
   `Inline::Tracked`). Zusage: Einfügungen behalten und Löschungen entfernt = Vorschlag, umgekehrt = aktuelle
   Fassung (Ausnahmen siehe Kopf von `diff.rs`: leeres Tabellengitter, Seitenumbrüche).
3. **`write(document, options)`** (`write.rs`, `write/body.rs`, `write/styles.rs`, `write/image.rs`): schreibt das
   Archiv in fester Reihenfolge mit festen ZIP-Zeiten (deterministisch): `[Content_Types].xml`, `_rels/.rels`,
   `docProps/app.xml`, `docProps/core.xml`, `word/document.xml` samt Beziehungen, `styles.xml`, `settings.xml`,
   `numbering.xml`, `footnotes.xml` (bei Verweisen in Fußnoten mit eigenen Beziehungen), `comments.xml` (nur mit
   Kommentaren), Bilder unter `word/media/imageN.<endung>`. XML-verbotene Zeichen werden durch U+FFFD ersetzt.
4. **`docx_export::build`** (Server): liest Werkangaben, alle Fassungen (`Store::load_work`, Zeitpunkte aus
   `Store::revision_tree`), Bilder (`Store::export_assets`, nur PNG/JPEG/GIF) und – mit Vorschlägen – die Marken
   „bis hierhin“ (`Store::work_revision_folds`). Offen ist ein Vorschlag mit Status „vorgeschlagen“, der nicht nach
   `baruch_core::revision_fold::is_hidden` ausgeblendet ist. Es gibt nur lesende Zugriffe; das Packen läuft in
   `spawn_blocking`.

Festlegungen des Schreibers über `ooxml.rs` hinaus (für den Leser und die Rundreise):

- Der Pfad eines Bildes im Werk steht in `wp:docPr name`, der Alternativtext in `descr`.
- Eine Abbildung mit `raw` steht in einem Inhaltssteuerelement `baruch-typst` (verborgener Quelltext, dann Bild und
  Unterschrift) – für den Leser also eine Insel mit `raw`, was beim Zurücklesen den wörtlichen Quelltext ergibt.
- Geordnete Listen: die erste mit Beginn 1 benutzt `NUM_DECIMAL`, jede weitere eine eigene `w:num` (ab 3) auf
  derselben abstrakten Nummerierung mit `w:startOverride` – sonst zählte Word über alle Listen des Buchs weiter.
- Verfolgte Änderungen stehen um jeden Lauf einzeln (`w:ins`/`w:del`), eine verfolgte Insel als
  `w:ins`/`w:del` um das `w:sdt`. Die Absatzmarke der Zitatquelle und der Bildunterschrift gilt als verfolgt, wenn
  ihr ganzer Inhalt eine Änderung derselben Art ist.
- Fußnotentext beginnt wie in Word mit einem Leerzeichen hinter dem Fußnotenzeichen.
- Untertitel steht zusätzlich in `dc:subject`, das Impressum in `dc:description`.

## CHANGELOG

- **Word-Export für den Verlag:** Buch → Export → „Word …“ (`/werke/<id>/word`), RPC `work.docx_export` und
  `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]` schreiben das Werk als .docx: Titelei aus den
  Werkangaben, Formatvorlagen „Überschrift 1–3“, „Baruch Teil“ und „Baruch ohne Nummer“, echte Fußnoten, eingebettete
  Bilder mit Alternativtext, Listen, Blockzitate und einfache Tabellen. Was Word nicht abbilden kann, steht als
  Typst-Insel mit wörtlichem Quelltext in der Datei und wird als Warnung genannt. Auf Wunsch liegt je Abschnitt der
  jüngste offene Vorschlag als Änderungsverfolgung über der aktuellen Fassung, mit der Persona als Bearbeiter. Der
  Export ändert nichts am Werk. Das Öffnen in Word prüft nur der Nutzer; mit LibreOffice ist nur eine Umwandlung
  ohne Fenster geprüft.

## README

- **Word (.docx) für den Verlag:** Export des ganzen Werks mit Formatvorlagen, echten Fußnoten und eingebetteten
  Bildern; offene Vorschläge der Assistenten auf Wunsch als Änderungsverfolgung (`/werke/<id>/word`,
  `baruch-server export ID --docx DATEI [--vorschlaege]`).

## Webseite

Satz für `site/content/funktionen.md` (setzt der Integrator ein):

> **Word-Export für den Verlag:** Baruch schreibt das Werk als Word-Datei (.docx) mit Formatvorlagen für
> Überschriften, echten Fußnoten und eingebetteten Bildern – auf Wunsch mit den offenen Vorschlägen der Assistenten
> als Änderungsverfolgung, die der Verlag in Word annimmt oder ablehnt.

## Für den Integrator

Bitten und Annahmen aus dieser Aufgabe (nichts davon habe ich in fremden Dateien geändert):

0. **Nötig, damit der volle Lauf grün ist – `tests/round33_seiten.rs`** (Besitz Integrator): Der Wächter
   `every_template_puts_its_actions_as_literal_groups_into_the_block_of_its_tab` verlangt jede Vorlage, die
   `base.html` erweitert, in seiner Liste `TEMPLATES`. Für die neue Seite fehlt dort die Zeile (hinter `cover.html`):
   ```rust
       ("docx_export.html", "buch", &["href=\"/?work={{ work.id }}\""]),
   ```
   Ohne sie scheitert genau dieser eine Test; mit ihr (lokal geprüft, nicht committet) ist der volle Lauf grün.

1. **Name der Warnung:** Der Kopf der Stub-Datei `from_typst.rs` nannte die Warnung `docx_insel`; nach Vertrag §3.4
   heißt sie `docx_typst_insel` (so umgesetzt, Konstante `from_typst::WARN_ISLAND`).
2. **`ooxml.rs` – fehlende Festlegungen**, die ich nicht als öffentliche Konstanten erfunden, sondern privat im
   Schreiber gehalten habe; Vorschlag wörtlich:
   ```rust
   /// Namensraum von `docProps/app.xml`.
   pub const NS_APP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";
   /// Beziehungen der Fußnoten (Verweise und Bilder in Fußnoten).
   pub const PART_FOOTNOTES_RELS: &str = "word/_rels/footnotes.xml.rels";
   pub const STYLE_HYPERLINK: &str = "Hyperlink";
   pub const STYLE_COMMENT_TEXT: &str = "CommentText";
   pub const STYLE_COMMENT_REFERENCE: &str = "CommentReference";
   ```
   `commentsExtended.xml` schreibt der Schreiber nicht (Inhaltstyp und Beziehungsart fehlen in `ooxml.rs`, dort „nur
   der Leser“): Antworten stehen als eigene Kommentare an der Stelle ihres Kommentars; die Beziehung `reply_to` geht
   in der Datei verloren. Der Server-Export schreibt ohnehin keine Kommentare.
3. **Rundreise (`round32_abnahme.rs`) – worauf der Leser treffen wird:** die Punkte unter „Festlegungen des
   Schreibers“ oben (Bildpfad in `wp:docPr name`, Abbildung mit `raw` als Insel, eigene `w:num` je weiterer
   Aufzählung, `w:ins`/`w:del` um `w:sdt`, Leerzeichen am Anfang des Fußnotentexts, Titelei-Absätze vor dem ersten
   Block). Zitate im Text schreibt der Schreiber **ohne** Anführungszeichen; sollen sie in Word sichtbar sein,
   braucht es eine Festlegung für beide Seiten (Vorschlag: Schreiber setzt „…“ als eigene Läufe mit der
   Zeichenvorlage `BaruchZitat`, Leser entfernt genau diese beiden Zeichen am Rand).
4. **`resolve.rs`:** Nach dem Auflösen können leere Hüllen übrig bleiben (`Block::Quote` ohne Blöcke und Quelle,
   `Block::List` ohne Punkte, ein Absatz mit `mark`, dessen Inhalt ganz entfällt). Meine Probe im Test entfernt sie;
   der Auflöser sollte es ebenso halten.
5. **`work.docx_export {work}`** nimmt neben der Zahl auch den Ordnernamen (Vertrag „number|slug“);
   `rpc::work_param` kennt nur Zahlen, die Auflösung des Ordnernamens steht darum in `docx_export::work_of`.
6. **Bilder SVG/WebP** werden nicht eingebettet (Word zeigt SVG nur mit Ersatzbild, WebP gar nicht verlässlich):
   Die Abbildung bleibt Insel mit `docx_bild_fehlt`. Ein Rastern nach PNG (wie beim EPUB-Umschlag) wäre eine spätere
   Ausbaustufe.

## Handprüfung durch den Nutzer

1. Ein Werk mit Fußnoten, einem PNG/JPEG-Bild, einer Liste, einem Blockzitat und einer Tabelle exportieren
   (`/werke/<id>/word`) und in **Word** und **LibreOffice** öffnen: Öffnet die Datei ohne Reparaturmeldung?
2. Navigationsbereich: Stehen die Überschriften 1–3 und die Teile in der Gliederung? Heißen die Vorlagen in der
   deutschen Oberfläche „Überschrift 1“, „Titel“, „Zitat“, „Fußnotentext“?
3. Fußnoten: am Seitenende mit Trennlinie, Zeichen hochgestellt?
4. Bild: sichtbar, in der Breite des Satzspiegels, Alternativtext unter „Alternativtext bearbeiten“?
5. Mit Häkchen exportieren: Zeigt „Überarbeiten“ die Änderungen mit `@lektor` (bzw. der Persona) als Bearbeiter?
   Ergibt „Alle ablehnen“ die aktuelle Fassung, „Alle annehmen“ den Vorschlag?
6. Typst-Inseln: als grau hinterlegte Felder erkennbar, der verborgene Quelltext nur mit „Ausgeblendeten Text
   anzeigen“ sichtbar?
