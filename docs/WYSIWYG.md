# Satzbild-Editor (WYSIWYG)

Erster Schritt in Runde 5, Feinschliff in Runde 6 (Szenen und Arbeitsaufträge als eigene Blöcke, Labels an
Überschriften, Bilder in Inseln, Darstellung nur aus Regeln der obersten Ebene, Vorgabe Satzbild), Fehler aus der
Arbeit am Buch in Runde 7 (eine Überschrift ist immer eine Zeile, Titelregeln, Bilder als Bild), Vorschläge an
ihrer Stelle und Satzbild als Vorgabe schon im Server-HTML in Runde 8, mehr Konstrukte statt Inseln, Inseln an
Ort und Stelle und Befehle des Menübands in Runde 18b.

Ein Abschnitt lässt sich im **Satzbild** bearbeiten: so, wie er nach der Werkvorlage aussieht (Schrift,
Größe, Farben, Einzug, Satzbreite, Kästen von `#szene` und `#arbeit`), statt im Typst-Quelltext. Die Quelle
bleibt Typst; gespeichert wird wie bisher über das Editor-Formular (`outline.save`), jede Speicherung ist eine
neue Fassung. Ohne JavaScript gibt es nur den Quelltext.

Teile: die reine Crate `crates/baruch-wysiwyg` (Blöcke, Rundreise, HTML, Stil), das Server-Modul
`crates/baruch-server/src/wysiwyg.rs` (RPC `wysiwyg.*`), das Widget `web/src/wysiwyg.ts`, der Stil
`static/wysiwyg.css` und der Bereich im Editor (`templates/fragments/editor.html`, Werkzeugleiste
`templates/fragments/edit_toolbar.html`). Seit dem Wunsch 2026-11-06 umfasst `div.wysiwyg` die Werkzeugleiste
(fest oben), das Blatt `div.editor-body` (füllt den Editor und scrollt allein; darin das Editor-Formular mit
Satzbild und Textfeld und der Lesefluss der Vorschläge) und die Statuszeile (Hinweise, „Speichern“ und – seit
Runde 22 – „Als neue Fassung speichern“, beide mit `form="editor-form"`). Ohne JavaScript sind Werkzeugleiste,
Satzbild und sein Hinweis `hidden`.

## Blockmodell

`baruch_wysiwyg::parse(text)` zerlegt einen Abschnittstext in **Blöcke**. Jeder Block trägt seinen
Quelltext `raw` **wörtlich**, samt der Leerzeilen dahinter; die Verkettung aller `raw` ist der Text, Byte
für Byte. `parse` scheitert nie.

| Art (`kind`)  | Typst                                                         |
|---------------|---------------------------------------------------------------|
| `heading`     | `=` bis `======` am Zeilenanfang (eine Zeile), `level` 1–6; `label`: ein Label allein auf der Zeile dahinter (`= Titel⏎<name>`) |
| `paragraph`   | Fließtext bis zur Leerzeile                                   |
| `list`        | `- …` (`ordered: false`) bzw. `+ …` (`ordered: true`), Folgezeilen eingerückt; Runde 18b: geschachtelt mit `depths` (siehe unten) |
| `enum`        | Runde 18b: Aufzählung `1. …`; `start` = Nummer des ersten Punkts (Vorgabe 1), `items`, `depths` wie bei `list` |
| `terms`       | Runde 18b: Begriffsliste `/ Begriff: Erklärung`; `items: [{term, desc}]` (je Inlines), ohne Schachtelung (Grenze, siehe unten) |
| `quote`       | `#quote(block: …, attribution: […])[…]` als ganzer Absatz     |
| `scene`       | `#szene[…]` bzw. `#szene(…)[…]` als ganzer Absatz (auch mehrzeilig, direkt hinter Überschrift und Label); `args` = Argumente wörtlich, `body` = Blöcke |
| `work`        | `#arbeit[…]` bzw. `#arbeit(…)[…]`, wie `scene`                |
| `part`        | Runde 14: Teil-Markierung `#heading(level: 1, numbering: none)[T] <baruch-part>` (eine Zeile, nur oberste Ebene); `title` = aufgelöster Titel |
| `unnumbered`  | Runde 14: Markierung „ohne Nummer“ `#heading(level: N, numbering: none)[T] <baruch-unnumbered>`; `level` 1–3, `title` |
| `island`      | alles, was sich nicht sicher abbilden lässt (siehe Inseln)    |

Im Fließtext (`inlines`, Kennung `t`):

| `t`        | Typst                                                                   |
|------------|-------------------------------------------------------------------------|
| `text`     | Text, Maskierungen aufgelöst (`\#` → `#`, `\u{2014}` → —, `~` → geschütztes Leerzeichen) |
| `soft`     | einfacher Zeilenumbruch im Quelltext (im Satz ein Leerzeichen)          |
| `emph`     | `_…_` oder `#emph[…]`                                                   |
| `strong`   | `*…*` oder `#strong[…]`                                                 |
| `footnote` | `#footnote[…]`                                                          |
| `quote`    | `#quote[…]` im Satz                                                     |
| `label`    | `<name>`                                                                |
| `island`   | Typst-Insel im Satz (`raw` wörtlich)                                    |
| `linebreak`| Runde 18b: erzwungener Zeilenumbruch `\` (vor Leerraum, Zeilenende oder am Ende; Leerzeichen dahinter gehören zu ihm) |
| `link`     | Runde 18b: `#link("adresse")[Text]` → `{url, c}`; ohne Inhaltsblock `#link("adresse")` → `{url}` (angezeigt wird die Adresse) |
| `ref`      | Runde 18b: Verweis `@ziel` bzw. `@ziel[Zusatz]` (auch `#ref(<ziel>)`, `#ref(<ziel>, supplement: […])`) → `{target, supplement?}`; `supplement` ist Typst-Markup wörtlich |
| `xref`     | Runde 30a: Querverweis `#link(<kN>)[Text]` bzw. mit Seitenzahl `#link(<kN>)[Text, S.~#context counter(page).at(<kN>).first()]` → `{key, c, page}` (siehe „Querverweise“) |

`_` und `*` zwischen zwei Wortzeichen sind Text (wie in Typst: `datei_name`).

**JSON-Form der neuen Konstrukte (Runde 18b).** Alte Formen bleiben gültig: Eine Liste ohne `depths` hat alle
Punkte oben, und ohne Schachtelung schreibt der Server `depths` nicht (Antworten wie bisher); `enum` ohne `start`
beginnt bei 1. Beispiele:

```json
{"kind": "list", "ordered": false, "items": [[{"t":"text","v":"Obst"}], [{"t":"text","v":"Äpfel"}], [{"t":"text","v":"Gemüse"}]], "depths": [0, 1, 0]}
{"kind": "enum", "start": 3, "items": [[{"t":"text","v":"drei"}], [{"t":"text","v":"vier"}]]}
{"kind": "terms", "items": [{"term": [{"t":"text","v":"Krug"}], "desc": [{"t":"text","v":"Gefäß aus Ton"}]}]}
{"t": "linebreak"}   {"t": "link", "url": "https://example.org", "c": [{"t":"text","v":"Beispiel"}]}   {"t": "ref", "target": "abb-1", "supplement": "S. 3"}
```

**Listen, Aufzählungen, Begriffslisten (Runde 18b).** Jede Zeile, die nach ihrem Einzug mit dem Zeichen der Liste
beginnt (`-`, `+`, `N.` bzw. `/`), beginnt einen Punkt; die Tiefe folgt aus der Spalte des Zeichens (weiter rechts als
der Punkt davor = eine Ebene tiefer, sonst die Ebene mit derselben Spalte). Andere Zeilen sind Folgezeilen und
müssen weiter eingerückt sein als das Zeichen ihres Punkts. Eine Aufzählung zählt fortlaufend: oben ab `start`, jede
geschachtelte Gruppe ab 1. Begriff ist der Text bis zum ersten Doppelpunkt außerhalb von Maskierungen, Labels,
Verweisen, Raw, Formeln, `#`-Ausdrücken und Inhaltsblöcken. **Insel bleiben:** geschachtelte Punkte mit anderem
Zeichen (`- a⏎  + b`), eine Spalte zwischen zwei Ebenen, Tabulatoren im Einzug, leere Punkte, springende Nummern
(`1.`, `3.`), geschachtelte Begriffslisten, ein Begriff ohne Doppelpunkt, mehr als neun Ebenen. Erzeugt (`emit`)
wird kanonisch: zwei Leerzeichen Einzug je Ebene, Folgezeilen eine Ebene tiefer, fortlaufende Nummern, im Begriff
jeder `:` maskiert (`\:`), sonst endete der Begriff dort.

**Grenze: geschachtelte Begriffslisten** (geprüft in Runde 21b, bleiben Insel). `terms` hat keine `depths` wie `list`
und `enum`: Eine Erklärung, die selbst eine Begriffsliste (oder Liste) enthält, wäre im Satzbild ein `<dd>` mit
eigenem Block darin – das Modell (`Term { term, desc }` mit Inlines), `emit`/`serialize`, `html.rs`, die Rücklesung in
`wysiwyg.ts` (`<dl>` → Blöcke) und der Eigenschaftstest müssten dafür gemeinsam um verschachtelte Blöcke wachsen. Die
Insel ist verlustfrei (Text byte-gleich, im Quelltext bearbeitbar, im Satz richtig gesetzt); darum kein Umbau in 21b.
In Runde 23 nicht umgesetzt, bleibt offen (in R24 nicht umgesetzt, Backlog „offen nach R24“; Backlog „Nach Runde 23 offen“; zusammen mit gemischten
Listenzeichen, wenn das Satzbild verschachtelte Blöcke in Listenpunkten lernt).

**Umbruch und Zeilenende (Runde 21b):** Ein `Soft` direkt hinter einem `Linebreak` (`\` am Zeilenende) erscheint als
leeres `<span class="ws-soft" data-after-break="1"></span>`: Typst verschluckt dieses Zeilenende, das Satzbild rückte
die nächste Zeile sonst um ein Leerzeichen ein. Das Element bleibt, damit das Widget es weiter als `soft` zurückliest
(der Text bleibt byte-gleich).

**Verweise und Umbrüche erzeugen.** Ein Verweis steht als `@ziel`, wenn das Ziel mit Buchstabe, Ziffer oder `_`
beginnt, nicht auf `.`/`:` endet und das Zeichen dahinter nicht zum Ziel gehören würde; sonst als `#ref(<ziel>)` (z. B.
vor `x` oder `_`). Hinter einem erzwungenen Umbruch steht ein Leerzeichen, außer am Blockende und vor einem
Zeilenende (`\]` bzw. `\_` wären Maskierungen). Ein `\` vor Leerraum ist auch beim Zerlegen in Zeilen ein Umbruch,
kein Maskieren des Zeilenendes: Das Zeilenende dahinter beendet die Zeile weiterhin (wie in Typst).

**Szenen und Arbeitsaufträge.** Eine Zeile, die genau ein `#szene…[…]` bzw. `#arbeit…[…]` ist, ist immer
ein eigener Block und verbindet sich nicht mit Nachbarzeilen. Ihr Rumpf besteht aus eigenen Blöcken
(Absatz, Liste, Zitat, Überschrift; was nicht abbildbar ist, bleibt Insel im Rumpf). Der gemeinsame Einzug
des Rumpfs zählt beim Lesen nicht (eingerückte Listen werden Listen); die `raw` der Rumpf-Blöcke sind
Stücke des Originals samt Einzug. Argumente vor dem Rumpf (`#szene(ort: [Hof])[…]`) stehen wörtlich in
`args` und lassen sich nur im Quelltext ändern.

**Labels.** Ein Label am Ende der Überschriftszeile steht in deren `inlines` (wie bisher). Ein Label allein
auf der Zeile direkt hinter der Überschrift gehört ebenfalls zur Überschrift (`label`, `raw` umfasst beide
Zeilen), nicht zu einem Absatz; hinter einem Absatz gehört es wie bisher zum Absatz.

## Inseln

Eine **Typst-Insel** ist Quelltext, den das Satzbild nicht sicher abbilden kann. Er erscheint gelb
hinterlegt mit `contenteditable="false"` und bleibt Zeichen für Zeichen erhalten, solange niemand ihn ändert. Im
Satzbild kann man eine Insel **als Ganzes löschen** und seit Runde 18b **an Ort und Stelle bearbeiten**
(Doppelklick, siehe „Inseln an Ort und Stelle“).

- **Block-Inseln:** Code-Zeilen (`#let`, `#set`, `#show`, `#import`, `#if` …), Absätze nur aus Inseln
  (`#pagebreak()`, Formel `$ … $` als Block, Raw-Block ```` ``` ````, Kommentarzeilen, ein Absatz nur aus `\`),
  Listen mit gemischten Zeichen und die anderen unsicheren Listen (siehe oben), eingerückte Überschriften,
  Leerzeilen am Anfang, und jeder Absatz, der etwas Unsicheres enthält: eckige Klammern ohne Maskierung, nicht
  geschlossenes `_`, `*`, `$`, `` ` ``, `/*`, ein Schlüsselwort mitten im Satz. Seit Runde 18b keine Inseln mehr:
  Aufzählungen `1.`, Begriffslisten `/ …`, geschachtelte Listen, der erzwungene Zeilenumbruch `\`.
- **Inseln im Satz:** Formeln `$…$`, Raw `` `…` ``, Kommentare `// …` und `/* … */`,
  eigene Funktionen und alle anderen `#`-Ausdrücke (`#box[…]`, `#zitat(…)`, ein `#link` mit mehr als einer
  Adresse als Argument; `#link("…")[…]` und Verweise `@name` sind seit Runde 18b eigene Konstrukte), außerdem
  Maskierungen, die als Text eine Kurzschreibweise würden: `\'` und `\"` (gerade statt typografischer
  Anführungszeichen), `\-` (sonst `--`, `---`, `-?`), `\.` neben einem Punkt (sonst `...`), `\?` und eine
  maskierte Ziffer hinter `-`. Andere Maskierungen (`\#`, `1\.` …) werden Text und beim Speichern wieder
  maskiert.
- **Zeilenenden:** Typst bricht auch bei U+2028, U+2029, U+0085, `\x0B` und `\x0C` um. Im Satzbild (Tippen,
  Einfügen) und beim Erzeugen werden alle diese Zeichen in Absätzen zu Leerzeichen
  (`baruch_typst::is_newline`); sonst stünde Text ungeschützt am Zeilenanfang (`= `, `- ` …). In einer
  **Überschrift** beendet ein Zeilenende sie dagegen (siehe „Überschriften“ unten), und ein Titel mit
  Zeilenende aus dem Baum oder einer Überschriftszeile wird abgelehnt (Runde 7, `baruch_store::check_title`;
  vorher wurde er still zu einem Leerzeichen).
- Eine Überschrift mit Kommentar (`== Titel // Notiz`) ist keine Insel: Der Kommentar gehört nicht zum Titel
  und bleibt beim Speichern erhalten.

### Inseln an Ort und Stelle (Runde 18b)

- **Bedienung:** Doppelklick auf eine Insel (auch auf ein Bild, eine Insel im Satz oder im Rumpf einer Szene) öffnet
  an ihrer Stelle ein kleines Quelltextfeld (`.ws-island-edit`, Textfeld mit „Übernehmen“/„Abbrechen“).
  Strg+Enter oder „Übernehmen“ (auch das Verlassen des Felds nach einer Änderung) übernimmt, Escape oder
  „Abbrechen“ stellt die Insel wieder her. Tasten im Feld gehen nicht an das Satzbild (Enter teilt keinen Absatz).
  Inseln in Vorschlägen (`.pv`) öffnen kein Feld.
- **Speichern:** Eine geänderte Block-Insel (`data-edited="1"`, neuer Rohtext in `data-raw`) geht als
  `{"replaces": i, "raw": "<Typst>"}` an `wysiwyg.serialize` (im Rumpf einer Szene in deren Rumpf-Anweisung); der
  Server übernimmt den Text **wörtlich** (Leerraum am Ende entfällt, der Trenner des alten Blocks bleibt; im Rumpf
  wird der Einzug des Rumpfs einheitlich gesetzt, auch vor jede Zeile eines Rohblocks ```` ``` ````; geschickt werden
  darf der Text mit dem Einzug – wie in `data-raw`, die erste Zeile mit oder ohne – oder ganz ohne ihn: Er gilt
  als mitgeschickt, wenn jede nicht leere Folgezeile mit ihm beginnt, sonst bleibt der Text, wie er ist, und der
  Einzug eingerückter Codezeilen ändert sich nicht), ein leerer Text löscht die Insel. Eine Insel im Satz gehört zu
  ihrem Absatz: Er wird mit der geänderten Insel neu erzeugt, die Insel steht darin wörtlich.
- **Grenzen:** `{"raw": …}` ohne `replaces` (eine neue Insel) bleibt verboten (`NewIsland`), ein unbekannter oder
  doppelter Index ist ein Fehler (-32602). Der Server prüft den neuen Rohtext nicht; was er bewirkt (auch eine neue
  Überschrift), prüft wie immer die Gliederung beim Speichern.

## Überschriften (Runde 7)

Ursache des Fehlers „Absatz wird Titel“ (Backlog, Nacht 05./06.11.2026): Das Widget schickte eine Überschrift,
hinter deren Titel ein einfacher Zeilenumbruch stand (Umschalt+Enter oder ein `<br>` des Browsers, danach der
getippte oder eingefügte Absatz), als **einen** Überschriftsblock `{kind: heading, inlines: [Titel, soft,
Absatz]}`. `emit` machte daraus eine einzige Überschriftszeile (`soft` in Überschriften → Leerzeichen), und
`outline.save` übernahm die ganze Zeile als Titel des Gliederungspunkts. Belegt durch
`crates/baruch-server/tests/round7_titles.rs` (scheitert ohne den Fix mit dem Titel „Laub Anna's Laube …“).

Seitdem:

- Eine Überschrift ist **immer eine Zeile**: `emit` und `serialize` beenden sie am ersten Zeilenumbruch der
  obersten Ebene (`Inline::Soft` oder ein Zeilenende im Text); was dahinter steht, wird ein eigener Absatz
  darunter (Umbrüche am Anfang zählen nicht). Label und Kommentar der Originalzeile bleiben an der
  Überschrift. Nur echte Überschriftszeilen (Spalte 0, 1–3 `=`, dann Leerzeichen/Tab, wie
  `baruch_typst::parse`) liefern Titel.
- Das Widget schickt eine solche Überschrift selbst schon geteilt (Überschrift + neuer Absatz); Umschalt+Enter
  und Einfügen mehrerer Zeilen beginnen in einer Überschrift einen neuen Absatz; Rückschritt am Anfang eines
  Absatzes hinter einer Überschrift bzw. Entf am Ende einer Überschrift ziehen den Absatz nicht in die
  Überschrift (ein leerer Absatz entfällt).
- **Titelregeln** (Server, `baruch_store::check_title`): leer, nur Leerraum, ein Zeilenumbruch oder mehr als
  200 Zeichen → abgelehnt (-32602), nichts gespeichert. Das Widget prüft vor dem Übertragen ins Textfeld
  dieselben Regeln mit derselben Meldung („Ein Gliederungspunkt braucht einen Titel.“, „Der Titel darf
  keinen Zeilenumbruch enthalten.“, „Der Titel ist zu lang (höchstens 200 Zeichen, hier N).“); maßgeblich
  bleibt der Server. Ein ganzer Absatz, der versehentlich in die Überschriftszeile geraten ist, scheitert
  so an der Länge.

## Markdown-Überschriften (Nutzer 2026-11-07)

Eine Zeile, die mit 1–6 `#` und einem Leerzeichen beginnt, ist eine Überschrift dieser Ebene. Maßgeblich ist
die Regel des Servers, `baruch_typst::markdown` (`markdown_headings`, Tabellentest mit allen Ausnahmen): Er
wendet sie beim Speichern des Menschen an (`outline.save`, `outline.check`, die Rückfrage vor dem Teilen,
`revision.save`, seit Runde 18b auch `revision.update` (Fassung ändern), auch ohne JavaScript) und beim Übernehmen eines Vorschlags (`revision.accept`); danach gleicht
`outline_save` die Gliederung ab wie bei einer getippten `=`-Zeile.

- `#` → `=`, `##` → `==`, `###` → `===`; `####` bis `######` werden auf `===` **begrenzt** (die Gliederung hat
  drei Überschriftsebenen; ob die Ebene an der Stelle passt, prüft wie immer die Gliederung beim Speichern).
- Titel wörtlich wie in einer `=`-Zeile, schließende Folge ` ##` und Leerraum am Ende entfallen; `# ` ohne Titel
  bleibt.
- **Nicht umgewandelt:** `\#` am Anfang, Raute in Anführungszeichen (`'# …'`, `"# …"`, „…“), Typst-Code
  (`#heading(…)`, `#set`, `#let` – nach der Raute kein Leerzeichen), eingerückte Zeilen, Zeilen in Raw-Blöcken,
  Kommentaren, Strings und offenen Klammern (Inhaltsblöcke wie `#quote[…]`) – dieselben Stellen, an denen auch
  eine `=`-Zeile keine Überschrift ist –, und die Werkvorlage (sie läuft nicht durch die Regel).
- **Live im Widget** (`web/src/markdown.ts`, dieselben Fälle, im Zweifel keine Umwandlung): im Quelltext beim
  Enter am Ende bzw. in der Zeile (der Cursor bleibt an seiner Stelle im Titel, dann der Zeilenumbruch) und beim
  Verlassen des Textfelds für alle Zeilen; im Satzbild wird ein Absatz, dessen Text mit `# ` … beginnt, beim
  Enter (und spätestens vor dem Übertragen ins Textfeld) zum Überschriftsblock der Ebene, die Rauten
  verschwinden, der Cursor bleibt im Titel. Im Satzbild schreibt `emit` eine getippte Raute sonst als `\#`
  (Text), darum wandelt hier nur das Widget um.
- **Vorschläge** von Assistenten bleiben, wie sie geschrieben sind (keine Umwandlung, keine Gliederung).
  Übernimmt der Mensch einen Vorschlag mit Markdown-Überschriften, bleibt der Vorschlag im Revisionsbaum
  unverändert; darauf folgt eine Fassung des Menschen mit den umgewandelten Überschriften (weitere Überschriften
  werden neue Punkte, die Übernahme ist die Bestätigung). Geht das nicht, bleibt es beim Übernommenen und
  `markdown` in der Antwort sagt, warum.

## Teile und „ohne Nummer“ (Runde 14)

- **Erkennen:** Eine Zeile ist ein `part`- bzw. `unnumbered`-Block genau dann, wenn `baruch_typst::parse` sie
  als Teil (`level` 0) bzw. als Überschrift mit `numbered: false` liest – also nur in der kanonischen Form
  (`docs/Architektur.md` „Teile im Typst-Text“), nicht eingerückt, nur auf oberster Ebene (im Rumpf einer
  Szene ist dieselbe Zeile eine Insel, wie sie auch für `baruch_typst` Text ist). Die Zeile ist immer ein
  eigener Block wie eine Überschrift; ein Kommentar dahinter gehört zu `raw`, nicht zum Titel.
- **Behalten:** `{keep: i}` übernimmt die Zeile wörtlich (Maskierungen, Kommentar).
- **Ändern:** `{replaces: i, block: {kind: "part", title}}` baut die Zeile mit `baruch_typst::part_line` neu
  (`heading_line(0, …)`: ein Kommentar der alten Zeile bleibt, auch wenn sie eine `=`-Zeile war);
  `{kind: "unnumbered", level, title}` mit `unnumbered_line` (ersetzt sie eine Markierung „ohne Nummer“, bleibt
  deren Kommentar). Ein neuer Block (`replaces: null`) ergibt die Markierung ohne Kommentar. Für jeden Titel gilt
  `parse(emit(k)) == k` (Eigenschaftstest `random_markers_read_back_unchanged`).
- **Teil im Kapiteltext (Runde 18b, festgelegt):** Das Satzbild bietet keinen Teil an. `wysiwyg.serialize` lehnt einen
  **neuen** Teil-Block (`replaces: null`) und das Umwandeln eines anderen Blocks in einen Teil ab
  (`WysiwygError::NewPart`, -32602 „Einen Teil legt man in der Gliederung an, nicht im Satzbild.“); einen vorhandenen
  Teil umbenennen geht weiter. Schreibt der Mensch die Teil-Markierung im Quelltext als weitere Überschrift in einen
  Kapiteltext, verlangt `outline.check` die Rückfrage zum Teilen; ohne Bestätigung speichert `outline.save` nichts,
  mit Bestätigung (`split`) entsteht der Teil (Aktion des Menschen). Test:
  `crates/baruch-server/tests/round18_konstrukte.rs`
  (`a_part_block_in_a_chapter_text_is_refused_in_the_satzbild_and_needs_confirmation_in_outline_save`).
- **HTML:** `<div class="ws-block ws-part" data-b="i" data-kind="part" data-level="0" role="heading"
  aria-level="1" title="Teil (Markierung im Typst-Text)">Titel</div>` bzw. `class="ws-block ws-unnumbered"
  data-kind="unnumbered" data-level="N" role="heading" aria-level="N"`; der Titel steht als Klartext (maskiert),
  „Teil“ bzw. „ohne Nummer“ setzt das Stylesheet über `::before`/`::after` (`static/wysiwyg.css`), damit es
  nicht in den Titel gerät.
- **Widget:** `kindOf` liest `data-kind=part|unnumbered` mit dem Textinhalt als Titel (`markerTitle`:
  Zeilenenden und geschützte Leerzeichen → Leerzeichen); Teil und „ohne Nummer“ verhalten sich wie
  Überschriften (`isTitleKind`: Enter und Einfügen mehrerer Zeilen beginnen einen Absatz dahinter,
  `guardHeadingMerge`, Titelregeln in `headingProblem`). Das Absatzformat lässt sich für sie nicht wählen
  (Meldung); Teile legt man in der Gliederung an, nicht im Satzbild.
- **Ohne JavaScript** bleibt das Textfeld mit der Markierung wörtlich; Speichern wie bisher.

## Rundreise

Laden und Speichern ohne Änderung ergibt **denselben Text, Byte für Byte**. Dafür baut
`baruch_wysiwyg::serialize(original, edits)` den Text aus Anweisungen:

- `{"keep": i}`: Block `i` des Originals wörtlich (samt seiner Leerzeilen). Das Widget schickt `keep` für
  **jeden Block ohne Eingabe**; ein unberührter Block wird also nie neu erzeugt.
- `{"replaces": i, "block": {…}}`: Block `i` neu erzeugen (kanonisches Typst, `baruch_wysiwyg::emit`), mit
  den Leerzeilen des alten Blocks dahinter. Eine bearbeitete Überschrift behält Label und Kommentar der
  Originalzeile (wie `baruch_typst::heading_line`; das Label nur, wenn die neue keins trägt).
- `{"replaces": null, "block": {…}}`: neuer Block, danach eine Leerzeile.
- `{"replaces": i, "body": [Anweisungen]}`: nur im Rumpf der Szene bzw. Arbeit `i` ändern, mit denselben
  Regeln für die Blöcke des Rumpfs (Indizes im Rumpf). Rahmen (`#szene(…)[`, `]`), Argumente und Leerraum
  bleiben wörtlich; neu erzeugte Blöcke bekommen den Einzug des Rumpfs. Der Einzug der ersten Zeile des
  Rumpfs gehört zum ersten Block und wandert mit ihm, wenn davor eingefügt oder umgestellt wird (sonst
  stünde seine erste Zeile in Spalte 0 und eine Liste würde geschachtelt). So ändert die Bearbeitung eines
  Absatzes in einer Szene nur ihn. Kein Rumpf (Absatz, Insel …) → Fehler.
- Eine bearbeitete Überschrift behält auch ihre Labelzeile wörtlich; trägt der neue Block ein anderes
  `label`, steht dort `<neu>`.
- Eine Überschrift mit Zeilenumbruch endet dort; der Rest wird ein Absatz dahinter (Runde 7, siehe oben).
- Nicht genannte Blöcke sind gelöscht. Ein Block, der leer herauskommt, entfällt. Folgen zwei Stücke nicht
  wie im Original aufeinander, steht dazwischen mindestens eine Leerzeile.
- `{"replaces": i, "raw": "…"}` (Runde 18b): Quelltext von Block `i` wörtlich ersetzen (Insel an Ort und Stelle,
  siehe oben), mit dem Trenner des alten Blocks; leer = gelöscht.
- Fehler (`WysiwygError`, deutsch): unbekannter Index, ein Index mehrfach (auch `keep` und `replaces`),
  eine Insel als neuer Block (auch `{"raw": …}` ohne `replaces`) oder eine als Block bearbeitete Insel, eine
  Rumpf-Anweisung für einen Block ohne Rumpf, ein neuer oder umgewandelter Teil (Runde 18b, `NewPart`).

Kanonisches Typst (`emit`): Sonderzeichen werden maskiert (`\#`, `\*`, `\_`, `\$`, `\@`, `\<`, `\[` …), am
Zeilenanfang auch `=`, `-`, `+`, `/` und `1.`; ein geschütztes Leerzeichen wird `~`; `_…_`/`*…*` nur an
Wortgrenzen und nicht in sich selbst geschachtelt, sonst `#emph[…]`/`#strong[…]`; hinter einem `#`-Ausdruck
wird `(` und `.name` maskiert; Listenpunkte mit zwei Leerzeichen Einzug in Folgezeilen; `#szene[…]` und
`#arbeit[…]` mehrzeilig, `#quote(…)[…]` mit einem einzeiligen Absatz in einer Zeile.

Belegt durch Tests in `crates/baruch-wysiwyg/tests/`:
- `roundtrip.rs`: alle Testdateien aller Crates (`crates/*/tests/data/*.typ`, nur gelesen) und die eigenen
  `tests/data/satzbild.typ` (alle Konstrukte und Inseln) und `tests/data/szenen.typ` (Überschriften mit
  Label, Szenen mit mehreren Absätzen und Fußnote, Arbeitsauftrag mit Liste, Bildinsel), beide frei
  erfunden, ganz und je Abschnitt;
- `blocks.rs`: Szenen, Arbeitsaufträge, Labels und Rumpf-Anweisungen (nur der bearbeitete Absatz ändert
  sich), Einfügen mehrerer Absätze mit den Anweisungen, die das Widget schickt;
- `serialize.rs` (Runde 7): eine Überschrift mit Zeilenumbruch endet dort, auch neu eingefügt und mit
  Label/Kommentar; `html.rs`: Bild-Inseln als `ws-figure` mit maskierten Attributen, byte-gleich bei `keep`;
- `emit.rs`: `parse(emit(k)) == k` für jede Art außer der Insel, dazu ein Eigenschaftstest mit fester Saat über
  2000 zufällige kanonische Blöcke (mehr mit `WS_CASES=…`, andere Saat mit `WS_SEED=…`), seit Runde 18b mit
  Aufzählungen, Begriffslisten, geschachtelten Listen, Umbrüchen, Links und Verweisen; die Lücke des Generators
  (Leerraum am Anfang einer Hervorhebung in einer Überschrift, `canonical_heading`) ist mit dem Fall aus dem Backlog
  als eigener Test geschlossen;
- `konstrukte.rs` (Runde 18b): Erkennen, byte-gleich unberührt, kanonisch bearbeitet, unsichere Listen als Insel,
  alte JSON-Formen, `Edit::Raw` (auch im Rumpf einer Szene), mehrere Bilder, HTML der neuen Konstrukte;
- `align.rs` (Runde 8): Ausrichtung zweier Fassungen nach Blöcken, siehe „Vorschläge an ihrer Stelle“;
- `serialize.rs`, `html.rs`, `style.rs`; dazu `crates/baruch-server/tests/wysiwyg.rs` über alle Fassungen des
  Beispielwerks (im Quelltext und gesät) und die Exporte, `crates/baruch-server/tests/satzbild.rs` für den
  Feinschliff (Bild-Adresse, Abbildung als Bild und byte-gleich gespeichert, Stil, Seite ohne JavaScript,
  Widget mit Titelprüfung), `crates/baruch-server/tests/round7_titles.rs` für „Absatz wird Titel“ und die
  Titelregeln auf allen Wegen (RPC, Formular, MCP).

## Vorschläge an ihrer Stelle (Runde 8)

Offene Vorschläge stehen nicht mehr als Liste unter dem Editor, sondern **an der Stelle, die sie ändern**: im
Satzbild, im Quelltext-Lesemodus und in der Vorschau, in der Farbe der Persona, einklappbar zu einer farbigen
Linie genau dort.

**Ausrichtung** (`baruch_wysiwyg::align(base, proposal) -> Vec<Hunk>`, rein, scheitert nie): Beide Texte werden
mit `parse` in oberste Blöcke zerlegt und nach der längsten gemeinsamen Teilfolge verglichen. Gleich sind zwei
Blöcke, wenn ihr `raw` ohne Leerraum am Ende byte-gleich ist (die Leerzeilen hinter einem Block gehören zu
seinem `raw`; sonst meldete jedes Anhängen den bisher letzten Block als geändert). Jede zusammenhängende Folge
ungleicher Blöcke ist ein `Hunk { kind, base, proposal }` mit Indexbereichen der Blöcke:

| `kind`     | `base`                | `proposal`         |
|------------|-----------------------|--------------------|
| `Changed`  | nicht leer            | nicht leer         |
| `Inserted` | leer; `base.start` = Block, vor dem eingefügt wird (am Ende die Blockzahl) | nicht leer |
| `Removed`  | nicht leer            | leer               |

Zwischen den Stücken stehen auf beiden Seiten gleich viele, paarweise gleiche Blöcke (auch null: Stücke dürfen
aneinanderstoßen). Über 4 Mio. Vergleiche (Blockzahl × Blockzahl) gilt alles als ein Stück.

**Ähnliche Blöcke (Runde 18c):** Die längste gemeinsame Teilfolge kennt nur gleiche Blöcke; ein umformulierter
Absatz neben einem eingefügten wurde so ein einziges „geändert“-Stück über beide. Ein „geändert“-Stück mit
**ungleich vielen** Blöcken beider Seiten wird darum nach Ähnlichkeit zerlegt: Wort-Dice (2 · gemeinsame Wörter /
alle Wörter, klein geschrieben, mit Vielfachheit) ≥ 0,5 gilt als „umformuliert“; die Paare werden monoton mit
der größten Summe der Ähnlichkeiten gewählt. Paare werden „geändert“ (aufeinander folgende Paare bleiben ein
Stück), was dazwischen nur auf einer Seite steht, „eingefügt“ bzw. „entfernt“, was auf beiden Seiten ungepaart
steht, bleibt „geändert“. Bei gleich vielen Blöcken (paarweise ersetzt), ohne ähnliches Paar oder über 10 000
Vergleichen im Stück bleibt es, wie es war. Beispiel: „Im Beet stehen Kohlköpfe.“ → „Im Beet stehen heute
Kohlköpfe.“ + neuer Absatz dahinter ergibt `Changed(2..3, 2..3)` und `Inserted(3..3, 3..4)`. `hunk_html_with(base, proposal, &hunk, &bilder)` zeigt ein
Stück im Satzbild: `<div class="ws-diff-doc" data-kind="changed|inserted|removed">`, die Blöcke der Basis in
`div.ws-del`, die des Vorschlags in `div.ws-ins` (wie `diff_html_with`, ohne `data-b`, Bilder wie dort).
Tests: `crates/baruch-wysiwyg/tests/align.rs` (geändert; eingefügt am Anfang, in der Mitte, am Ende; entfernt;
mehrere Stücke; Satzbild; Eigenschaftstest mit fester Saat über 1500 Fälle, `WS_SEED`/`WS_CASES`; Runde 18c:
umformuliert + eingefügt bzw. + entfernt als zwei Stücke).

**Basis** ist die **gezeigte** Fassung (im Editor offen, meist die aktuelle) – das ist, was „Übernehmen“
ersetzt. Beruht der Vorschlag auf einer anderen Fassung, nennt der Kopf sie („· auf Fassung m“). Ist die
gezeigte Fassung selbst ein offener Vorschlag, stehen seine Stücke gegen seine Basis (Elternteil, sonst die
aktuelle) an ihrer Stelle in ihm, mit dem Vermerk „Unterschied zu Fassung b (aktuell) · gezeigt“. Gleicht ein
Vorschlag der gezeigten Fassung, steht er als ein Stück am Ende („gleicht der gezeigten Fassung“), damit
Übernehmen/Verwerfen bleiben.

**HTML** (`templates/fragments/proposals.html`, Kontext `InlineView` aus `view.rs`): je Stück

```html
<details class="pv pc" data-pv="<key>-<rev>" data-pv-at="<b>" data-pv-gap="<g>"
         data-pv-kind="changed|inserted|removed" style="--pc-light:…;--pc-dark:…;--pc-line:…;--pc-line-dark:…" open>
  <summary class="pv-line"><span class="pv-label">Name (@handle) · Fassung n · Modell · auf Fassung m · Teil i/k</span></summary>
  <div class="pv-body"> Notiz · div.pv-satz (hunk_html_with) · pre.pv-quelltext (Zeilen-Diff des Stücks)
    · Knöpfe „Übernehmen“/„Verwerfen“ (form="pv-accept-<key>-<rev>" bzw. "pv-reject-…") </div>
</details>
```

`at` ist der erste betroffene Block der gezeigten Fassung (`data-b` aus `wysiwyg.parse`), bei eingefügten der
Block, vor dem eingefügt wird; `gap` die Lücke, in der das Stück steht (vor Block `gap`, also hinter einem
geänderten oder entfernten Block, am Ende die Blockzahl). Der **Kopf steht nur einmal**, in der Linie
(`summary`); „Teil i/k“ nur bei mehreren Stücken. Mehrere Vorschläge an derselben Stelle stehen untereinander
in Persona-Reihenfolge, darin nach Nummer; Autoren ohne Persona am Ende. Die Formulare (`revision.accept` bzw.
`revision.reject` an `/act` mit `work`, `node`, `key`, `rev`, `view`, `lq`, unverändert) stehen **einmal je
Vorschlag** außerhalb der Stücke (`div.pv-forms`); die Knöpfe zeigen über das Attribut `form` darauf. So bleiben
sie gültig, wenn das Widget die Stücke ins Satzbild setzt, das im Editor-Formular liegt (Formulare lassen sich
nicht schachteln, und `outline.ts` hielte ein Absenden darin für Speichern). Das Cookie `baruch_folded`
(`<key>-<rev>`) klappt alle Stücke eines Vorschlags ein; der gezeigte Vorschlag bleibt offen (`pv-shown`).

**Lesefluss im Editor:** `<section class="pv-flow" data-widget="proposals" data-key="K" data-rev="R">` unter dem
Editor-Formular (also unter dem Textfeld), nur wenn es offene Vorschläge gibt: die Formulare, die gezeigte Fassung
blockweise als Quelltext (`<pre class="pv-src" data-b="i">raw</pre>`) und die Stücke an ihrer Stelle. Die
**Leiste** (Anzahl, „Alle einklappen“) steht seit Runde 18c **vor** dem Lesefluss als eigenes Kind des Blatts
(`<div class="pv-bar pv-bar-editor" data-pv-bar="K">`); CSS (`proposals.css`) rückt sie mit `order: -1` ganz nach
oben und lässt sie beim Scrollen kleben. So bleibt „Alle einklappen“ auch bei offenem Satzbild sichtbar, wenn
`wysiwyg.ts` den Lesefluss verbirgt (früher stand die Leiste im verborgenen Lesefluss); `proposals.ts` findet den
Knopf über `data-pv-bar` beim Geschwister des Leseflusses, `wysiwyg.ts` bleibt unverändert. In der Vorschau steht
die Leiste weiter im Abschnitt. Der **Zeilen-Diff** eines Stücks vergleicht die Blöcke ohne Leerraum am Ende
(Runde 18c): Die Leerzeile hinter einem Block gehört zu seinem `raw` und stand sonst als leere „−“-Zeile am Ende. Ohne JavaScript und im Quelltext ist das die Ansicht der
Vorschläge (Zeilen-Diff). Mit JavaScript und offenem Satzbild setzt `wysiwyg.ts` jedes Stück als nicht
bearbeitbares Geschwister (`contenteditable="false"`) vor den Block `[data-b=gap]` des Satzbilds (am Ende
dahinter) und verbirgt den Lesefluss; im Satzbild zeigt das Stück sein Satzbild (`pv-satz`), nicht den
Zeilen-Diff (nur CSS). Beim Wechsel in den Quelltext und vor jedem neuen Satzbild kommen die Stücke an ihren
Platz im Lesefluss zurück. `editsOf`, `blocksIn` und `markSelection` übergehen `.pv`; Speichern ändert dadurch
nichts.

**Vorschau** (`view=preview`): je Abschnitt des Kapitels `div.pv-section` mit seinen Blöcken als Zeilen
(`h1`–`h3`, `p`) und den Stücken (nur Zeilen-Diff) zwischen den Blöcken an ihrer Stelle, Basis ist die dort
gezeigte (aktuelle oder probeweise gesetzte) Fassung.

**Farben:** Auf dem stets hellen Blatt (Satzbild, Vorschau) stehen die Stücke in den hellen Werten
(`--pc-line`, `--pc-light`), sonst je Thema `--pc-line` bzw. `--pc-line-dark`. Beide Linien erreichen
mindestens 3:1 gegen die Fläche ihres Themas (WCAG 1.4.11, `crate::personas::style`, `MIN_LINE_CONTRAST`),
`--pc-line` zugleich gegen das Blatt des dunklen Themas (`--sheet: #f4f1ea`, `DARK_SHEET`):
reicht die Farbe nicht, wird sie für hell abgedunkelt, für dunkel aufgehellt (in Schritten von 5 %).

## Satzbild als Vorgabe (Runde 8)

Fehler aus der Sichtprüfung: Nach dem Laden war auf allen Bildschirmfotos „Quelltext“ aktiv, obwohl die Vorgabe
„satz“ ist. **Ursache** (Code, Stand development f601260): Der Server lieferte immer `aria-pressed="true"` bei
„Satzbild“ und ein **leeres** Satzbild. `wysiwyg.ts` rief beim Einhängen zuerst `show(state, root)` mit
`open = false` – das drückt „Quelltext“, verbirgt Werkzeuge und Blatt und zeigt das Textfeld – und öffnete das
Satzbild erst in `openSheet` nach zwei RPC-Antworten (`wysiwyg.parse`, `wysiwyg.style`). Das Laden der Seite
(Ereignis `load`) wartet nicht auf diese `fetch`-Aufrufe; ein Bildschirmfoto direkt nach dem Laden
(`firefox --screenshot`) zeigt darum den Zwischenzustand „Quelltext“. Ein Fehler beim Öffnen hätte ihn
dauerhaft gemacht, ein Fehler war aber nicht nötig.

**Behebung:** Der Server zeichnet den Zustand selbst: `Settings.editor` (`satz` | `quelltext`, aus dem Cookie
`baruch_editor`, Vorgabe `satz`) setzt `aria-pressed`, `data-ws-start` und – bei „quelltext“ – `hidden` an
Werkzeugen, Hinweis und Blatt. Bei „satz“ steht das Satzbild der gezeigten Fassung fertig im Blatt
(`<div … data-ws-sheet … data-ws-ready>…</div>`, dasselbe HTML wie `wysiwyg.parse`, mit Bildern). Das Widget
übernimmt es beim Einhängen sofort (`adoptSheet`), ohne Zwischenzustand; der Stil der Werkvorlage kommt per
`wysiwyg.style` nach. Liefert das Textfeld nicht denselben Text wie die Fassung (ein Zeilenumbruch direkt am
Anfang, den der Browser hinter `<textarea>` streicht, oder `\r`), fehlt `data-ws-ready`, und das Widget öffnet
das Satzbild wie bisher über RPC – ohne vorher auf Quelltext umzuschalten (nur das Textfeld ist kurz
verborgen; scheitert das Öffnen, kommt der Quelltext). `#fundstelle` und das Cookie „quelltext“ öffnen den
Quelltext wie bisher. Test: `crates/baruch-server/tests/round8_inline.rs`
(`the_server_already_renders_the_satzbild_as_default_and_the_source_only_on_request`, scheitert ohne die
Behebung an `data-ws-start`/`data-ws-ready`).

**Lange Abschnitte (Runde 18c):** Über 64 KiB Text (`view::READY_SHEET_MAX`, Bytes) setzt der Server das Satzbild
nicht vorab ins HTML – es verdoppelte den Text (Textfeld, Blatt, mit Vorschlägen auch Lesefluss). Dann fehlt
`data-ws-ready`, und das Widget holt das Satzbild wie oben über `wysiwyg.parse`, ohne vorher den Quelltext zu zeigen.

**Stil schon im HTML (Runde 18c):** Der Editor bringt `<style id="ws-style" data-work="W">` mit (CSS aus
`sheet::sheet_css_for`: Werkvorlage, überschrieben mit den Satzbild-Einstellungen des Werks, siehe unten), damit
das Blatt nach dem Laden nicht Schrift und Größe wechselt; `wysiwyg.ts` übernimmt ein vorhandenes `#ws-style` mit
gleichem `data-work` (Runde 18b).

**Verhaltenstest (Runde 18c):** `crates/baruch-server/tests/round18_satzbild.rs`
(`satzbild_as_default_behaves_right_in_the_real_widgets_without_a_browser`) lässt die übersetzten Widgets
`wysiwyg.js`, `proposals.js` und `sheet.js` in Node 20 auf einem kleinen nachgebauten DOM gegen das Editor-HTML
von `ui.render` laufen – ohne Browser, ohne Netz (`fetch` ist nachgebaut und zählt die RPC-Aufrufe). Belegt wird
das Verhalten: ohne Cookie ist das Satzbild schon beim Einhängen offen (Textfeld verborgen, „Satzbild“ gedrückt,
kein `wysiwyg.parse`), die Stücke stehen im Satzbild, der Lesefluss ist verborgen und „Alle einklappen“ sichtbar;
mit Cookie „quelltext“ bleibt das Textfeld offen; ohne vorab gesetztes Satzbild holt das Widget es per
`wysiwyg.parse` und öffnet es danach. Nachgebaut ist nur, was die Widgets beim Einhängen brauchen (Selektoren mit
Tag, `#id`, `.klasse`, `[a]`, `[a=b]`, Nachfahre, Kind, `:scope`); was fehlt, meldet der Test als Fehler im
Nachbau. Ohne `node` wird er mit „ÜBERSPRUNGEN“ übersprungen.

## Darstellung nach der Werkvorlage

`baruch_wysiwyg::sheet_style(vorlage)` liest aus der Werkvorlage nur Regeln der **obersten Ebene** (Kommentare
und Raw-Text zählen nicht; die letzte Regel gewinnt, auch mehrere `#set` in einer Zeile). Regeln in
`#show …: it => […]`, in `#show: …`-Rümpfen, in Funktionsrümpfen (`#let f() = { set par(…) }`), in `#if`
oder in anderen Ausdrücken gelten nur für einen Teil des Dokuments und zählen nicht; ebenso wenig
`#show footnote: set text(size: …)` (nur `emph`, `strong` und `heading` mit ihren eigenen Feldern):

| Regel                                         | Feld                         | CSS-Variable              |
|-----------------------------------------------|------------------------------|---------------------------|
| `#set text(font:, size:, fill:)`              | `font`, `size`, `fill`       | `--ws-font`, `--ws-size`, `--ws-fill` |
| `#set par(first-line-indent:, leading:, justify:)` | `indent`, `leading`, `justify` | `--ws-indent`, `--ws-leading`, `--ws-justify` |
| `#set page(width:)` bzw. `paper:`             | `width`                      | `--ws-width`              |
| `#show emph: set text(fill:)`                 | `emph_fill`                  | `--ws-emph-fill`          |
| `#show strong: set text(fill:)`               | `strong_fill`                | `--ws-strong-fill`        |
| `#show heading: set text(font:, fill:)`       | `heading_font`, `heading_fill` | `--ws-heading-font`, `--ws-heading-fill` |
| `#let szene(…) = block/rect/box(fill:, inset:, stroke:)` | `scene`           | `--ws-scene-fill/-inset/-stroke` |
| `#let arbeit(…) = …`                          | `work`                       | `--ws-work-fill/-inset/-stroke` |

Farben: `rgb("#…")`, `rgb(r, g, b)` (auch Prozent), `luma(n)`, `cmyk(…)` und die Farbnamen von Typst;
Längen in pt, mm, cm, in, em. Was sich nicht übersetzen lässt (z. B. `red.lighten(20%)`), fehlt; dann gilt
die Vorgabe aus `wysiwyg.css`. `sheet_css` schreibt nur Werte aus sicheren Zeichen (Buchstaben, Ziffern,
` #%.,()-+_"'`, Anführungszeichen nur paarig – nie `<`, `;`, `{`, `}`, `\`), darum darf das Ergebnis unmaskiert in
ein `<style>`-Element.

**Satzbild-Einstellungen je Werk (Runde 18c):** Einstellungen → Werk → „Satzbild“ (RPC `work.sheet`,
`work.sheet_set`, Store `work_sheet`, Migration 0044) überschreiben feldweise, was die Vorlage hergibt:
`sheet_style_over(sheet_style(vorlage), einstellungen)` – jedes gesetzte Feld gewinnt, der Rest bleibt aus der
Vorlage. Felder: `font`, `heading_font`, `size`, `fill`, `heading_fill`, `width` und `background` (Blattfarbe,
`SheetStyle.background` → `--ws-background`; die Vorlage liefert sie nie, das Blatt bleibt sonst hell; `sheet.css`
liest sie). `wysiwyg.style`, das `<style id="ws-style">` im Editor und `work.sheet` rechnen über dieselbe Funktion
(`crate::sheet::sheet_style_for`) und liefern dasselbe CSS. Die Einstellungen gelten **nur im Satzbild des
Browsers**: Typst-Satz, PDF und Export lesen `work_sheet` nie (byte-gleich, Test). Nach dem Speichern in den
Einstellungen (`baruch:settings-changed`) holt `sheet.ts` `work.sheet` und ersetzt den Inhalt von `#ws-style`.

## RPC

Alle Methoden lesen nur (keine Schreibsperre), nur über `POST /rpc` – nicht über `/act`, nicht über MCP.
`work` fehlt → Vorgabewerk. Fehler über `INVALID_PARAMS` (-32602).

| Methode | Parameter | Ergebnis |
|---------|-----------|----------|
| `wysiwyg.parse` | `{work?, body}` | `{blocks, html}`: Blöcke (wie oben, mit `raw`) und HTML des Satzbilds (Bilder in Inseln, siehe unten) |
| `wysiwyg.serialize` | `{work?, body, edits}` | `{body}`; ungültige Anweisungen → -32602 mit deutscher Meldung; Runde 18b auch `{replaces, raw}` (Insel an Ort und Stelle) |
| `wysiwyg.style` | `{work?}` | `{css, style}` aus der Werkvorlage (`Store::work_template`) |
| `wysiwyg.diff` | `{work?, key\|node, rev, base?}` | `{html, base}`: Fassung `rev` gegen `base`; fehlt `base`, ihr Elternteil (`based_on`), sonst die aktuelle; Bilder wie bei `parse` |

HTML (`render_html`): je Block `<div class="ws-block" data-b="i" data-kind="…">` (Überschrift mit
`data-level`, Liste mit `data-ordered`, Zitat mit `data-block`; Rumpf in `div.ws-body`, Herkunft in
`div.ws-attribution`), Inseln mit `contenteditable="false"` und `data-raw`, Fließtext als `em`, `strong`,
`span.ws-footnote`, `q.ws-quote`, `span.ws-label`, `span.ws-soft`, `span.ws-nbsp`, `span.ws-island`. Jeder
Text ist maskiert. Das Label hinter einer Überschrift steht als
`span.ws-label.ws-heading-label[data-heading-label]` (nicht bearbeitbar) am Ende des Überschriftsblocks, die
Argumente einer Szene als `div.ws-args[data-raw]` (nicht bearbeitbar) vor ihrem Rumpf.

Runde 18b: Aufzählung `div.ws-block[data-kind=enum][data-start=N]` mit `<ol start="N">`, Begriffsliste
`div.ws-block[data-kind=terms]` mit `<dl><dt>Begriff</dt><dd>Erklärung</dd>…</dl>`, geschachtelte Punkte als Liste
im `<li>` des Punkts davor (gleiches Listenelement), Umbruch `<span class="ws-linebreak" contenteditable="false"
data-linebreak="1">↵<br></span>`, Link `<a class="ws-link" data-url="…" href="…">Text</a>` (Text bearbeitbar;
`href` nur bei `http://`, `https://`, `mailto:`; ohne Text `data-bare="1"` und nicht bearbeitbar), Verweis
`<button type="button" class="ws-ref" contenteditable="false" tabindex="-1" data-target="…" data-supplement="…">@ziel</button>`
(`type="button"`: schickt das Editor-Formular nicht ab). Der Titel einer Block-Insel lautet „Typst-Insel: Doppelklick
öffnet den Quelltext an Ort und Stelle“, der eines Bilds „Bild – Doppelklick öffnet den Quelltext an Ort und Stelle“.

**Bilder** (Schnittstelle zu „images“, Runde 7: als Bild statt als Typst-Insel):
`baruch_wysiwyg::image_path(raw)` liefert das String-Literal im ersten Argument des ersten `image(`-Aufrufs,
auch in `#figure(image("…"), …)`, außerhalb von Kommentaren, Raw-Text und Formeln (berechnete Pfade nicht).
Runde 18b: `image_paths(raw)` liefert die Pfade **aller** `image(`-Aufrufe; das Satzbild zeigt jedes Bild einer
Insel (vorher nur das erste), Unterschrift und Alternativtext-Zeile wie beim ersten Bild. Runde 21b: Mit mehreren
Bildern nennt die Unterschrift ohne `caption:` jedes Bild (Alternativtext, sonst Pfad, getrennt durch „ · “), und
darunter steht immer `<span class="ws-alt ws-paths">Bilder: Pfad · Pfad</span>` (vorher nur der erste Pfad).
`render_html_with(doc, &bilder)` bzw. `diff_html_with(alt, neu, &bilder)` zeigen jede Insel mit Bildpfad, für
die der Rückruf `Some(ImageRef { src, alt })` liefert, **als Bild** und ohne sichtbaren Rohtext:

- Block-Insel: `<div class="ws-block ws-figure" data-b="i" data-kind="island" data-image="1"
  contenteditable="false" data-raw="…" title="Bild – Quelltext nur im Quelltext bearbeitbar"><img
  class="ws-image" src="…" alt="…" loading="eager"><span class="ws-caption">…</span></div>`; die Unterschrift
  ist `caption: […]` bzw. `caption: "…"` wörtlich, sonst der Alternativtext, sonst der Pfad.
- Insel im Satz: `<span class="ws-island ws-inline-image" contenteditable="false" title="Bild – …"
  data-raw="…"><img …></span>`.

Alle Attribute sind maskiert. Die Insel bleibt eine Insel (`data-kind="island"`, nicht bearbeitbar, als
Ganzes löschbar); das Widget schickt sie als `keep`, beim Speichern bleibt sie byte-gleich. Ohne Bild vom
Rückruf bleibt die Insel wie bisher (Rohtext in `code.ws-raw`). `render_html(doc)` ist
`render_html_with(doc, &|_| None)` und zeigt keine Bilder. Der Server (`wysiwyg::image_ref`) nimmt `src =
/werke/{work}/bilder/pfad?p=<Pfad, prozentkodiert>` mit dem Pfad in Normalform
(`baruch_store::normalize_asset_path`: `./bilder/a.png` und `/bilder/a.png` → `bilder/a.png`; seit Runde 18b ohne
führendes `/`, vorher `p=%2Fbilder%2F…`) und den
Alternativtext der aktuellen Fassung (`wysiwyg::image_alts`, ebenfalls nach Normalform). Ausgeliefert wird
das Bild nur über diese vorhandene Route (`nosniff`, CSP `sandbox`); sie findet `/bilder/a.png`,
`bilder/a.png` und `./bilder/a.png` gleich, `..` nie. Diff (`diff_html`): Blöcke nach der längsten gemeinsamen Teilfolge über `raw` (ohne
Leerraum am Ende), gleiche in `div.ws-same`, entfernte in `div.ws-del`, neue in `div.ws-ins`.

## Menüband (Runde 18a)

Die Werkzeugleiste über dem Blatt ist das **Menüband für das Bearbeiten** (`templates/fragments/edit_toolbar.html`,
Widget `ribbon` in `web/src/ribbon.ts`, Stil `static/ribbon.css`). Es behält alle Haken, die `wysiwyg.ts` liest
(`data-edit-toolbar hidden`, `data-ws-mode`, `data-ws-tools`, `data-ws-format`, `data-ws-cmd="italic|bold"`;
`data-ws-hint` in der Statuszeile), und bekommt dahinter die Gruppe `span.ribbon-formats[data-widget=ribbon]`:

- je Format der Werkvorlage ein Knopf `button[data-ribbon-cmd=<WsCommandName>][data-ribbon-snippet]` aus
  `crate::ribbon::formats` (RPC `ribbon.formats`): `scene`/`work` nur, wenn die Vorlage `#let szene`/`#let arbeit`
  definiert, seit Runde 19b `scripture` („Bibelstelle“) nur mit `#let bibelstelle(…)` bzw. `#let bibel(…)` und
  `cite` („Literaturverweis“) nur mit `#bibliography(` in der Vorlage, dann immer `footnote`, `quote`, `list`,
  `enum`, `terms`, `break`, `link` (seit Runde 30a `xref`);
- `button[data-ribbon-image]` („Bild“): öffnet nur `details[data-image-insert]` (Formular von Runde 18c).

**Formate der Vorlage für Stellen und Literatur (Runde 19b):**

| Vorlage enthält | Knopf | Quelltext (Snippet) | Satzbild (`baruch:ws-command`) |
|---|---|---|---|
| `#let bibelstelle(…)` bzw. `#let bibel(…)` | Bibelstelle | `#bibelstelle[]` bzw. `#bibel[]`, Cursor in den Klammern | `scripture`, `arg` = `{"fn": "bibelstelle"}`; das Satzbild fragt nach der Stelle und setzt die Insel `span.ws-island[data-raw="#bibelstelle[Joh 3,16]"]` (nur als Ganzes änderbar) |
| `#bibliography(` (nicht in einer Kommentarzeile) | Literaturverweis | `@`, Cursor dahinter | `cite`, `arg` = Schlüssel (fehlt: das Satzbild fragt); setzt `button.ws-ref[data-target]` wie ein `@schlüssel` aus dem Quelltext |

Die reinen Bausteine stehen in `web/src/ws_command.ts` (importiert nichts): `scriptureFunction(snippet)` holt den
Namen aus `#name[]`, `scriptureSource(fn, stelle)` baut `#fn[stelle]` und maskiert Markup-Zeichen wie `needs_escape`
in `baruch-wysiwyg` (`\ # * _ $ @ < > [ ] ` ~`, Zeilenumbruch → Leerzeichen, höchstens 200 Zeichen), `citeKey(text)`
prüft den Schlüssel wie ein Typst-Label (Buchstaben, Ziffern, `_ - : .`; `@` vorn und Satzpunkt hinten entfallen).
Gespeichert wird wie immer über `wysiwyg.serialize`: die Insel wörtlich, der Verweis als `@schlüssel` (bzw.
`#ref(<schlüssel>)`, wenn ein Zeichen eines Labels folgt). Annahme: die Funktion der Vorlage nimmt die Stelle als
Inhalt (`[…]`); eine Vorlage, deren Funktion einen String erwartet, braucht den Quelltext.

**Satzbild offen** (`div.wysiwyg[data-ws-open]`): Der Knopf ruft `sendWsCommand(root, cmd)` aus
`web/src/ws_command.ts` (Ereignis `baruch:ws-command` an `div.wysiwyg`, ohne bubbles). Setzt `wysiwyg.ts`
`handled = true`, ist der Befehl ausgeführt; bleibt `handled` `false`, zeigt `ribbon.ts` einen Hinweis
(`baruch:toast`, „… geht im Satzbild noch nicht – bitte im Quelltext einfügen“). **Quelltext:** `ribbon.ts` fügt
`snippet` selbst an `selectionStart` ins Textfeld ein (`execCommand("insertText")`, sonst `setRangeText` + `input`);
Blockformate beginnen an einem Zeilenanfang, eine Markierung wandert in den Inhalt (`[…]`), der Cursor steht im
Inhalt. Kein Import zwischen `ribbon.ts` und `wysiwyg.ts`.

Dasselbe Fragment ist mit `satz_page` das Menüband der Satz-Seite (`/werke/{id}/satz`, `div.ribbon-satz
[data-ribbon-satz]`, ohne `hidden`, ohne JavaScript): Zurück, PDF, ‹ › mit `aria-label`, „Satz abbrechen“ als
Formular (`docs/Bedienung.md`, „Satz (Typst-Vorschau)“).

## Bedienung

- **Klick auf einen Abschnitt** öffnet ihn im Satzbild (Vorgabe), auch nach jedem Neuzeichnen (Gliederung,
  `baruch:navigate`, `ui.render`). Der Umschalter **Satzbild | Quelltext** über dem Text wechselt; nur diese
  ausdrückliche Wahl setzt das Cookie `baruch_editor` (`satz` bzw. `quelltext`), und nur `quelltext` öffnet
  den Editor im Quelltext. Kommt man beim Laden der Seite über einen Treffer der Literatursuche
  (`#fundstelle`), beginnt dieser erste Editor im Quelltext, weil die Markierung dort steht; spätere Klicks
  öffnen wieder das Satzbild.
- Im Quelltext füllt das Textfeld das Blatt (das Satzbild ist `hidden`); offen füllt das Satzbild das Blatt.
  Werkzeugleiste und Statuszeile scrollen nicht mit.
- Tippen wie in einer Textverarbeitung: **Enter** beginnt einen neuen Absatz (in Listen einen neuen Punkt),
  **Umschalt+Enter** einen einfachen Zeilenumbruch (in einer Überschrift einen neuen Absatz: eine Überschrift
  ist immer eine Zeile), **Strg+I** hebt hervor, **Strg+B** setzt fett (auch über
  die Knöpfe *K* und **F**), die Auswahl **Absatz / Überschrift 1–3** ändert das Format des Blocks am Cursor.
  Ein Absatz, der mit `# `, `## ` … beginnt, wird beim Enter eine Überschrift dieser Ebene (Markdown, siehe
  oben).
  Einfügen nimmt nur Text; Leerzeilen im eingefügten Text trennen Absätze (jedes weitere Stück wird ein
  neuer Absatz, in Listen bleibt alles ein Punkt).
- **Szenen und Arbeitsaufträge** stehen als Kasten da; jeder Absatz darin ist einzeln bearbeitbar (Enter
  teilt ihn, wie draußen). Gespeichert wird nur der bearbeitete Absatz neu, der Rest der Szene bleibt
  Zeichen für Zeichen. Argumente (`#szene(ort: …)`) und das Label einer Überschrift stehen grau bzw. gelb
  da und bleiben beim Bearbeiten erhalten.
- **Bilder** aus `#image(…)`/`#figure(image(…))` erscheinen als Bild mit Unterschrift (Bild von der Seite
  „Bilder“ des Werks, auch bei Pfaden mit `./`), nicht als Typst-Rohtext; ein feiner Rand beim Überfahren zeigt,
  dass sie sich nur im Quelltext ändern oder als Ganzes löschen lassen.
- **Strg+S** speichert (wie im Quelltext). Vor dem Speichern überträgt das Widget das Satzbild ins Textfeld;
  danach gelten dieselben Regeln wie im Quelltext (Überschriftszeile bestimmt Titel und Ebene, Teilen und
  Zusammenführen nach Rückfrage). Ein leerer, mehrzeiliger oder über 200 Zeichen langer Titel wird mit einer
  Meldung abgewiesen, nichts gespeichert. Ungespeicherter Text im Satzbild löst dieselbe Rückfrage aus wie im
  Quelltext.
- Gelb hinterlegt: **Typst-Inseln**; im Satzbild als Ganzes löschbar und seit Runde 18b per **Doppelklick** an Ort
  und Stelle als Quelltext bearbeitbar (Strg+Enter übernimmt, Escape bricht ab).
- **Runde 18b:** Aufzählungen (`1.`), Begriffslisten (Begriff fett, Erklärung eingerückt; Enter beginnt einen neuen
  Eintrag), geschachtelte Listen, erzwungene Umbrüche (↵), Links (Text bearbeitbar, Adresse im Titel) und Verweise
  (`@ziel` als Schaltfläche; ein Klick zeigt das Label im Abschnitt, ändern nur im Quelltext) stehen im Satzbild
  statt als Insel. Die Knöpfe des Menübands (Szene, Arbeitsauftrag, Fußnote, Zitat, Liste, Aufzählung,
  Begriffsliste, Umbruch, Link) wirken bei offenem Satzbild am Cursor: Fußnote, Zitat und Link umschließen die
  Auswahl (sonst ein markierter Platzhalter), Liste/Aufzählung/Begriffsliste machen aus dem Absatz am Cursor eine
  (Liste ↔ Aufzählung wechselt), Szene und Arbeitsauftrag nehmen den Block am Cursor in ihren Rumpf (an einer
  Überschrift oder Insel entsteht eine leere dahinter). Geht ein Befehl an der Stelle nicht, sagt eine Meldung warum.
- **Vorschläge** stehen an ihrer Stelle (Runde 8, siehe oben): bei offenem Satzbild direkt beim Block, den
  sie ändern, mit dem Unterschied im Satzbild (entfernt rot durchgestrichen, neu grün, fertig vom Server über
  `hunk_html_with`), im Quelltext und ohne JavaScript im Lesefluss unter dem Textfeld mit dem zeilenweisen
  Diff. Ein Klick auf die farbige Linie klappt einen Vorschlag auf oder zu (alle seine Stücke).
- **Fassung ändern** bleibt Quelltext; seit Runde 18b werden Markdown-Überschriften auch dort umgewandelt (eine
  weitere Überschrift lehnt „Fassung ändern“ ab wie eine getippte `=`-Zeile).
- **Ohne JavaScript** ist der Satzbild-Bereich verborgen; es gibt nur das Textfeld, das wie bisher an
  `/gliederung/act` geht.

## Technik im Widget

- `wysiwyg.ts` hängt sich an `div.wysiwyg[data-widget=wysiwyg]` im Editor-Formular, übernimmt das fertige
  Satzbild des Servers (`data-ws-ready`, Runde 8) oder holt `wysiwyg.parse`, dazu `wysiwyg.style` (eingefügt
  als `<style id="ws-style">`) und zeigt das Satzbild (`contenteditable`). Danach setzt `placeProposals` die
  Stücke der Vorschläge aus `section.pv-flow` an ihre Stelle (`returnProposals` holt sie zurück).
- Jede Eingabe markiert alle berührten Blöcke (`data-dirty="1"`, auch geschachtelte im Rumpf einer Szene),
  setzt `form.dataset.dirty = "1"` und gibt den Speichern-Knopf frei. Neue Blöcke (Enter) haben kein
  `data-b`; ein vom Browser kopierter Index gilt beim zweiten Mal als neu. Blöcke im Rumpf einer Szene
  bzw. Arbeit nummeriert das Widget beim Öffnen (`data-n`); für eine bearbeitete Szene schickt es eine
  Rumpf-Anweisung `{"replaces": i, "body": […]}`.
- Vor dem Speichern löst `outline.ts` am Formular `baruch:before-save` aus; das Widget legt dann ein Promise in
  `detail.wait`, das `wysiwyg.serialize` ruft, das Ergebnis ins Textfeld schreibt und `input` auslöst.
  Abgeschickt wird immer das Textfeld. Als Rückfall (solange niemand das Ereignis auslöst) fängt das Widget
  das Absenden im Fenster ab, überträgt und schickt erneut ab.
- **Runde 22 (Fortschreiben):** Das Editor-Formular trägt das versteckte Feld `mode=auto`; „Speichern“ (erster
  Knopf des Formulars – `submitButton` nimmt ihn für „Gespeichert“/„Speichern“) schreibt die eigene aktuelle Fassung
  fort, solange keine Regel eine neue Fassung verlangt, „Als neue Fassung speichern“ schickt `as_new=1` (der Server
  bzw. `workspace.ts` macht daraus `mode: "new"`). Beim erneuten Absenden nach dem Übertragen des Satzbilds geht der
  auslösende Knopf mit (`form.requestSubmit(submitter)` in `wysiwyg.ts` und `outline.ts`), damit `as_new` nicht
  verloren geht; Strg+S löst wie bisher ohne Knopf aus (also „Speichern“, Modus „auto“). Nach dem Speichern zeigt
  `workspace.ts` „Gespeichert: fortgeschrieben (Stand k).“ bzw. „Gespeichert: neue Fassung n (Grund).“; ohne
  JavaScript steht dieselbe Rückmeldung nach der Weiterleitung im Editor-Kopf (`saved=…` in der Adresse).
- `wysiwyg.ts` importiert nichts aus `outline.ts`.
- **Quelltext ohne Speichern (Runde 17, Vertrag §2.6):** Wer den vollständigen Quelltext braucht, ohne zu speichern
  (z. B. die Live-Zählung in `timing.ts`, die Prüfanzeige `scripture_check.ts`), löst am Editor-Formular
  `baruch:serialize-text` mit `detail: { wait: Promise<string>[] }` aus. Ist das Satzbild offen, legt `wysiwyg.ts`
  ein Promise in `wait`: ohne Eingabe seit dem letzten Übertragen das Textfeld, sonst `wysiwyg.serialize` mit
  denselben Anweisungen wie beim Speichern – aber ohne Markdown-Überschriften umzuwandeln, ohne das Textfeld zu
  schreiben und ohne `touched` zu ändern. Ist niemand zuständig (Quelltext-Ansicht), bleibt `wait` leer; der
  Auslöser nimmt dann das Textfeld.
- **Markierungen der Prüfanzeige (Runde 17c):** Das Widget `scripture-check` (Statuszeile, `web/src/scripture_check.ts`)
  markiert geprüfte Zitate im Satzbild **nur** über die CSS Custom Highlight API (`CSS.highlights`, Namen
  `bibel-stimmt` und `bibel-weicht`, Stil in `static/pruefung.css`): Es legt `Range`-Objekte über vorhandene
  Textknoten (gesucht wird der Zitattext, Leerraum zählt als ein Zeichen; Vorschläge `.pv` zählen nicht) und ändert
  das DOM des Satzbilds nicht – `editsOf` sieht also keine Änderung, Speichern bleibt byte-gleich. Jede Eingabe
  löscht die Markierung. Ohne Highlight-API gibt es nur die Liste.
- Überschriften (Runde 7): `inlinesOf(…, heading = true)` macht aus Zeilenenden im Text Umbrüche,
  `splitHeading` teilt am ersten (wie `split_heading` im Server), `headingEdits` schickt Überschrift + neuen
  Absatz; `headingProblem` prüft vor `wysiwyg.serialize` die Titelregeln (Ebene 1–3) und bricht das Speichern
  mit der Meldung ab; `guardHeadingMerge` (an `beforeinput`) fängt Rückschritt/Entf an der Grenze
  Überschrift ↔ Absatz ab. `ws-figure` und `ws-inline-image` sind Inseln wie alle anderen.
- **`guardHeadingMerge` über Vorschläge (Runde 18b):** Der Nachbar eines Blocks ist der nächste Block davor bzw.
  dahinter, Stücke der Vorschläge (`details.pv`) dazwischen zählen nicht (`blockNeighbour`). Vorher war der Nachbar
  das `<details>`, und Rückschritt am Anfang des Absatzes zog ihn doch in die Überschrift (Gutachten R8). Nachbau über
  RPC und Prüfung der Stelle im Widget: `round18_konstrukte.rs`
  (`a_proposal_between_heading_and_paragraph_does_not_disarm_the_merge_guard`); im Browser nur von Hand prüfbar.
- **Befehle des Menübands (Runde 18b, Vertrag R18-S1):** `baruch:ws-command` am Bereich `div.wysiwyg` (ohne
  bubbles, `web/src/ws_command.ts`, eingefroren): Bei offenem Satzbild setzt das Widget `detail.handled = true` und
  führt den Befehl aus (`runCommand`, alle `WsCommandName`: `scene`, `work`, `footnote`, `quote`, `list`, `enum`,
  `terms`, `break`, `link`; `arg` = Adresse beim Link bzw. Text des Platzhalters, ohne Adresse fragt ein Dialog);
  bei geschlossenem Satzbild bleibt `handled` `false`, und das Menüband fügt den Typst-Text selbst ins Textfeld ein.
  Die Auswahl im Satzbild merkt sich das Widget (`selectionchange`), damit ein Klick ins Menüband sie nicht verliert.
- **Cursorstelle (Runde 18b):** `baruch:ws-caret` (am Bereich oder am Editor-Formular): Bei offenem Satzbild legt das
  Widget genau ein Promise in `detail.wait` mit `{body, offset}` – `body` wie bei `baruch:serialize-text`, `offset` die
  UTF-16-Länge des Texts, den dieselben Anweisungen bis einschließlich des obersten Blocks am Cursor ergeben (ein
  Zeilenanfang hinter seinen Leerzeilen; ohne Cursor oder hinter dem letzten Block das Ende). Zwei Aufrufe
  `wysiwyg.serialize` (Text und Präfix); Ablauf über RPC belegt in `round18_konstrukte.rs`.
- **Stil vom Server (Runde 18b):** Steht schon ein `<style id="ws-style" data-work="W">` mit dem Werk des Editors im
  Dokument (Satzbild je Werk, `fragments/editor.html`), übernimmt `applyStyle` es ohne `wysiwyg.style`; andere
  `#ws-style` (anderer Werke) entfallen. Sonst holt es den Stil wie bisher per RPC (eine Anfrage je Werk zugleich).

## Querverweise (Runde 30a)

Ein **Querverweis** zeigt auf einen Gliederungspunkt über dessen feste Sprungmarke `<k{key}>`
(`baruch_typst::xref::key_label`). Die Sprungmarken stehen nur im Export, im Spiegel und im Satz, nicht in den
Fassungen (siehe `docs/Architektur.md`, „Querverweise“).

- **Form im Text** (`baruch_typst::xref::xref_snippet`): `#link(<k103>)[die Einleitung]`; mit Seitenzahl
  `#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]`. Kein `@k103`: Überschriften ohne
  Nummer lassen sich in Typst nicht mit `@` referenzieren.
- **Blockmodell:** `parse` erkennt beide Formen als `Inline::Xref {key, c, page}` (JSON `{"t": "xref", "key": 103,
  "c": [...], "page": true}`). Erkannt wird nur ein kanonisches Label `k` + Ziffern ohne führende Null und genau dieser
  Seitenzahl-Zusatz mit demselben Schlüssel. Ein leerer Linktext, ein anderes Label oder ein anderer Zusatz bleiben
  Insel (`#link(<hafen>)[…]` wie bisher). `emit` schreibt die Form von `xref_snippet`; `emit`/`parse` ergeben eine
  Rundreise (Eigenschaftstest `tests/xref.rs`, Saat `WS_SEED`, Fälle `WS_CASES`).
- **Satzbild:** `render_html` setzt einen `span.ws-xref` mit `data-key` und `data-page`, darin den bearbeitbaren
  Linktext, bei Seitenzahl einen nicht bearbeitbaren `span.ws-xref-page` („, S. …“).
  - `wysiwyg.ts` liest den Block ohne den Zusatz zurück.
  - Der Befehl `xref` (`ws_command.ts`, `arg` = JSON `{key, text, page}`) fügt ihn an der Cursorstelle ein.
  - Strg+Klick springt zum Ziel, ein Klick zeigt „Öffnen | Bearbeiten“ (`xref.ts`).
- **Sprungmarken im Export:** Eine `=`-Überschrift bekommt ` <kN>` vor einem Kommentar. Eine Überschrift mit eigenem
  Label (Teil `<baruch-part>`, „ohne Nummer“ `<baruch-unnumbered>`, Label des Menschen am Zeilenende oder allein auf
  der Zeile darunter) behält ihr Label; darunter steht die Ankerzeile `#metadata("baruch-key") <kN>` (hinter einer
  Label-Zeile). Typst kennt nur ein Label je Element.
- **Messung** (`crates/baruch-render/tests/xref.rs`): `#link(<kN>)` und `counter(page).at(<kN>)` lösen bei beiden
  Formen auf, und Typst prüft per `#context assert`, dass jede Sprungmarke auf der Seite ihrer Überschrift liegt.
  - **Ausnahme Teil im Satz:** Mit der Teil-Regel (`part_page_rule`, Umbruch nach dem Teil) läge die Ankerzeile
    hinter der Teil-Zeile auf der Folgeseite (belegt).
  - Der Satz setzt sie darum vor die Teil-Zeile, als `#pagebreak(weak: true, to: "odd")#metadata("baruch-key")
    <kN>` (`part_anchors_for_typesetting`). Der Umbruch der Regel entfällt danach, die Seitenzahl bleibt gleich.
  - Ein Verweis auf eine fehlende Sprungmarke ist in Typst ein Fehler mit dem Label in der Meldung. Darum macht der
    Satz kaputte Verweise zu bloßem Text, und die Druckprüfung nennt sie.
- **Einlesen:** Der Import entfernt Sprungmarke und Ankerzeile (`strip_section`) und setzt die Verweise auf die
  neuen Schlüssel um (`remap_keys`).

## EPUB-Abbildung (Runde 20a)

Das EPUB (`crates/baruch-epub`, `xhtml.rs`) nimmt dasselbe Blockmodell wie das Satzbild: `baruch_wysiwyg::parse` je
Abschnittstext (aktuelle Fassung wie im Export, mit Sprungmarke `<kN>`), dann eine eigene XHTML-Abbildung – nicht
`render_html` (das schreibt Editor-HTML mit `contenteditable`, `data-raw` und Widget-Klassen) und nicht typst-html.

| Blockmodell | XHTML im EPUB |
|---|---|
| Überschrift des Abschnitts (`=`, Teil, „ohne Nummer“) | `<h1>`–`<h3 id="k{key}">`, davor `<span class="baruch-number">` mit der Nummer des Gliederungsschemas; Teil `<h1 class="baruch-part-title">` |
| weitere Überschrift im Text | `<h{Ebene}>` (höchstens `<h6>`), mit Label als `id="lbl-…"` |
| Absatz, `Soft` | `<p>`, Zeilenumbruch im Quelltext bleibt Leerraum |
| `_…_`, `*…*`, `\` | `<em>`, `<strong>`, `<br/>` |
| Liste `-`/`+`, Aufzählung `1.` | `<ul>`/`<ol>`, geschachtelt wie im Satzbild, `start` ab der ersten Nummer |
| Begriffsliste | `<dl><dt>…</dt><dd>…</dd></dl>` |
| Blockzitat, Zitat im Satz | `<blockquote class="baruch-quote">` mit `<footer class="baruch-attribution">`, `<q>` |
| `#szene[…]`, `#arbeit[…]` | `<section class="baruch-scene">`, `<section class="baruch-work">`; Argumente (`ort: […]`) fallen weg, Warnung `ARGS_OMITTED` |
| `#link("…")[…]` | `<a href>` nur für `http`, `https`, `mailto`, sonst nur der Text |
| Querverweis `#link(<kN>)[…]` | `<a class="baruch-xref" href="k{Kapitel}.xhtml#kN">`; unbekannter Schlüssel → Text, Warnung `XREF_UNKNOWN` |
| Verweis `@name`, `@kN` | Link auf das Label bzw. die Überschrift (Text = Überschrift), sonst Text |
| Label `<name>` | `<span id="lbl-name"></span>` (Sprungmarken `<kN>` tragen die Überschriften) |
| `#footnote[…]` | Popup: `<sup><a epub:type="noteref" href="#fnN">` + `<aside epub:type="footnote" id="fnN">` in `ol.baruch-footnotes` am Dateiende; Endnoten: Verweis nach `notes.xhtml#fnN`, dort mit Rückverweis |
| Insel mit `image("…")` (Block) | `<figure class="baruch-figure"><img alt="…"/><figcaption>` aus `caption:` |
| Insel mit `image("…")` (im Satz) | `<img class="baruch-inline-image" alt="…"/>` |
| `` `Code` `` | `<code>` |
| Maskierung `\x` als Insel | das Zeichen |
| Kommentar, Ankerzeile `#metadata("baruch-key") <kN>` | nichts, ohne Warnung (Ankerzeile → `id`, falls noch frei) |
| jede andere Insel (`#set`, `#let`, Formeln, eigene Funktionen) | nichts, Warnung `ISLAND_OMITTED` mit Schlüssel und Auszug |

Alternativtext eines Bilds: `alt: "…"` im Typst-Text, sonst der Alternativtext der Bildangaben; leer → `alt=""` und
Warnung `IMAGE_NO_ALT`. Ein Pfad ohne Bild im Werk erscheint als „Bild fehlt: <Pfad>“ (Warnung `IMAGE_MISSING`).
Aller Text wird maskiert (`escape_text`); nichts aus dem Werk wird als HTML übernommen.

## Registermarken und Kontextmenü (Runde 31c)

Eine **Registermarke** steht im Typst-Text direkt hinter dem markierten Wort, kanonisch
`#metadata((register: "stichwort", term: "Gnade"[, sub: "…"][, see: "…"][, also: "…"]))<baruch-idx>`
(`baruch_typst::index::mark_source`, gelesen von `parse_mark`). Im Blockmodell ist sie `Inline::IndexMark { mark }`, in
JSON `{"t": "index_mark", "mark": {"register": "…", "term": "…", …}}` (leere Zusätze fehlen, unbekannte Felder lehnt der
Server ab). Eine Marke mit Leerzeichen vor dem Label oder anderer Schlüsselfolge ist keine Marke, sondern bleibt Insel.

Im Satzbild (`render_html`) erscheint sie als leeres, nicht tippbares Element

```html
<span class="ws-index-mark" contenteditable="false" role="img" tabindex="-1"
      title="Registereintrag im Stichwortregister: Gnade › im Alltag; siehe auch: Huld"
      aria-label="…" data-register="stichwort" data-term="Gnade" data-sub="im Alltag" data-also="Huld"
      data-raw="#metadata((…))&lt;baruch-idx&gt;"></span>
```

- **Dezent:** Das kleine Zeichen (⁘, im Personenregister blau) kommt nur aus dem Stil (`::before` in `wysiwyg.css`); das
  Element hat keinen Textknoten, darum bleiben Text, Suche, Markierung und Zitatprüfung des Absatzes unberührt.
- **Beschreibung:** `title`/`aria-label` = `baruch_wysiwyg::index_mark_label` („Registereintrag im {Register}: {Stichwort}
  › {Unterstichwort}; siehe: …; siehe auch: …“; eigene Register als „Register „slug““). `markLabel` in `ws_command.ts`
  folgt derselben Regel (Node-Vergleichstest).
- **Zurück:** `wysiwyg.ts` liest `span.ws-index-mark` aus den `data-*`-Feldern als `index_mark` (vor der Prüfung auf
  Inseln); der Server schreibt sie mit `emit` kanonisch. Ein unberührter Block bleibt `keep` und byte-gleich; ein Block,
  in dem eine Marke gesetzt, geändert oder entfernt wurde, wird neu erzeugt – bei kanonischem Text ändert sich nur die
  Marke (Test `baruch-wysiwyg/tests/index_mark.rs`).
- **Setzen, ändern, entfernen** nur über den Befehl `index_mark` (`baruch:ws-command`, `arg` = JSON
  `{op: "add" | "edit" | "remove", mark?}`), den das Kontextmenü schickt: `add` setzt die Marke hinter das Ende der
  Auswahl (Leerraum am Ende zählt nicht) bzw. hinter das Wort am Cursor; nicht in Überschrift, Teil, „ohne Nummer“,
  Insel, Verweis, Label oder einer anderen Marke (Meldung). `edit`/`remove` wirken auf die Marke, die das Kontextmenü
  vorher mit `data-ctx-target` bezeichnet. Danach ist der Editor ungespeichert; gespeichert wird mit „Speichern“ als
  neue Fassung des Menschen.
- **Doppelklick** auf eine Marke öffnet „Registereintrag bearbeiten“.

**Kontextmenü** (`web/src/context_menu.ts`, Widget `context-menu` auf der Menüband-Gruppe „Register und Glossar“):

- Öffnen im Satzbild und im Quelltext-Textfeld mit Rechtsklick, Kontextmenü-Taste oder Umschalt+F10 (`opensMenu`), oder
  über den Link „Markierung …“ der Statuszeile. Schickt der Browser nach der Taste zusätzlich `contextmenu`, öffnet es
  nicht doppelt.
- ARIA-Menümuster: `div.ctx-menu[role=menu]` mit `button[role=menuitem]` (roving `tabindex`), Pfeil ab/auf laufen um,
  Pos1/Ende, Enter/Leertaste lösen aus, Escape und Tab schließen; der Fokus (und im Textfeld die Auswahl) geht zurück
  (`menuKey`).
- Einträge: „In Glossar aufnehmen …“ (Dialog mit Begriff = markiertes Wort, kurze/ausführliche Erklärung, Varianten und
  Bibelstellen je Zeile, Quelle → RPC `glossary.add`, Meldungen des Servers im Dialog), „In Register aufnehmen …“
  (Stichwort, Unterstichwort, Register aus `index.registers` – ohne Antwort die eingebauten –, siehe, siehe auch → Marke
  wie oben bzw. im Quelltext `markSource` hinter das Wort, `markInSource` mit denselben Verboten wie oben),
  „Im Glossar nachschlagen“ (RPC `glossary.lookup`, Popup mit Begriffen, Varianten, Erklärungen und Link auf
  `/werke/{id}/glossar#begriff-{id}`); auf einer Marke (Rechtsklick auf sie, Cursor direkt daneben, im Textfeld Cursor
  in ihr) zusätzlich „Registereintrag bearbeiten …“ und „… entfernen“.
- Menüband: dieselben Aktionen als Knöpfe (`data-ctx-action`) und „Register“ (Knopf `[data-index-toggle]` mit `aria-expanded`, Escape schließt), das beim
  Aufklappen alle Registereinträge des Abschnitts listet – aus den Marken des Satzbilds bzw. des Textfelds (`findMarks`),
  also auch ungespeicherte; ein Klick springt zur Marke.
- Ohne JavaScript ist das Menüband verborgen; der Link „Markierung …“ führt auf `/werke/{id}/markierung?key=K&rev=R` mit
  denselben drei Aktionen als Formularen (`docs/Bedienung.md`).

## Zitatblock (Runde 23)

`baruch_wysiwyg::parse` liest eine logische Zeile, die genau ein `#quote(block: true, …)[…]` ist, als eigenes Stück
(wie `#szene[…]`/`#arbeit[…]`): Eine Zeile direkt dahinter ist ein eigener Absatz, eine Zeile direkt davor beendet
ihren Absatz – wie Typst setzt. Bis Runde 22 wurde ein Zitatblock mit der Stellenangabe in der Folgezeile zu einer
Code-Insel. `#quote[…]`, `block: false` und unlesbare Argumente bleiben Teil ihres Absatzes. Unberührte Blöcke gehen
byte-gleich zurück (`serialize`).

Der Knopf „Zitat“ (`ribbon::QUOTE_SNIPPET`) fügt `#quote(block: true)[]` mit der Folgezeile „– “ für die Stellenangabe
ein, mit genau einer Leerzeile davor und danach (vorhandene zählen mit), Cursor im Rumpf; mit einer Auswahl wandert die
Auswahl in den Rumpf. Nicht `attribution: []`: Eine leere `attribution` setzt in Typst 0.15.1 eine Zeile mit dem
Geviertstrich (gemessen in `crates/baruch-render/tests/quote_attribution.rs`). Im Satzbild entsteht ohne Auswahl
dasselbe als eigener Block hinter dem Absatz am Cursor; mit Auswahl bleibt es beim Zitat im Fließtext. Ohne JavaScript
gibt es das Menüband nicht; der von Hand geschriebene Block wird genauso gelesen. Tests: `baruch-wysiwyg/tests/zitat.rs`,
`round23_web::widgets::the_quote_button_inserts_the_block…`, Abnahme `round23_abnahme.rs`.
