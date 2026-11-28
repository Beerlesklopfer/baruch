<!-- translation-of: docs/WYSIWYG.md source-commit: 6880d80 -->

# Editor für das Satzbild (WYSIWYG) {#typeset-view-editor-wysiwyg}

> Dies ist die deutsche Übersetzung von `docs/WYSIWYG.md` (englisches Original, maßgeblich), Stand: Commit `6880d80`.

Erster Schritt in Runde 5, Feinschliff in Runde 6 (Szenen und Arbeitsaufträge als eigene Blöcke, Marken an
Überschriften, Bilder in Inseln, Anzeige nur nach Regeln der obersten Ebene, Satzbild als Voreinstellung), Fehler aus
der Arbeit am Buch in Runde 7 (eine Überschrift ist immer eine Zeile, Titelregeln, Bilder als Bilder), Vorschläge an
ihrer Stelle und Satzbild als Voreinstellung schon im HTML des Servers in Runde 8, mehr Konstrukte statt Inseln, Inseln
an Ort und Stelle und Befehle des Menübands in Runde 18b.

Ein Abschnitt lässt sich im **Satzbild** bearbeiten: so, wie er nach der Werkvorlage aussieht (Schrift,
Größe, Farben, Einzug, Textbreite, Kästen von `#szene` und `#arbeit`), statt im Typst-Quelltext. Der Quelltext
bleibt Typst; gespeichert wird wie bisher über das Formular des Editors (`outline.save`), jedes Speichern ist eine
neue Fassung. Ohne JavaScript gibt es nur den Quelltext.

Bestandteile: die reine Crate `crates/baruch-wysiwyg` (Blöcke, Rundlauf, HTML, Stil), das Servermodul
`crates/baruch-server/src/wysiwyg.rs` (RPC `wysiwyg.*`), das Widget `web/src/wysiwyg.ts`, der Stil
`static/wysiwyg.css` und der Bereich im Editor (`templates/fragments/editor.html`, Werkzeugleiste
`templates/fragments/edit_toolbar.html`). Seit dem Wunsch vom 2026-11-06 umfasst `div.wysiwyg` die Werkzeugleiste
(oben fixiert), das Blatt `div.editor-body` (füllt den Editor und scrollt für sich; darin das Editor-Formular mit
Satzbild und Textfeld und der Lesefluss der Vorschläge) und die Statuszeile (Hinweise, „Speichern“ und – seit
Runde 22 – „Als neue Fassung speichern“, beide mit `form="editor-form"`). Ohne JavaScript sind die Werkzeugleiste,
das Satzbild und sein Hinweis `hidden`.

## Blockmodell {#block-model}

`baruch_wysiwyg::parse(text)` zerlegt einen Abschnittstext in **Blöcke**. Jeder Block trägt seinen
Quelltext `raw` **wörtlich**, einschließlich der Leerzeilen dahinter; die Verkettung aller `raw` ist der Text, Byte
für Byte. `parse` schlägt nie fehl.

| Art (`kind`) | Typst                                                         |
|---------------|---------------------------------------------------------------|
| `heading`     | `=` bis `======` am Zeilenanfang (eine Zeile), `level` 1–6; `label`: eine Marke allein in der Zeile dahinter (`= Titel⏎<name>`) |
| `paragraph`   | Fließtext bis zur Leerzeile                                   |
| `list`        | `- …` (`ordered: false`) oder `+ …` (`ordered: true`), Fortsetzungszeilen eingerückt; Runde 18b: verschachtelt mit `depths` (siehe unten) |
| `enum`        | Runde 18b: Aufzählung `1. …`; `start` = Nummer des ersten Eintrags (Voreinstellung 1), `items`, `depths` wie bei `list` |
| `terms`       | Runde 18b: Begriffsliste `/ Begriff: Erklärung`; `items: [{term, desc}]` (je Inlines), ohne Verschachtelung (Grenze, siehe unten) |
| `quote`       | `#quote(block: …, attribution: […])[…]` als ganzer Absatz     |
| `scene`       | `#szene[…]` oder `#szene(…)[…]` als ganzer Absatz (auch mehrzeilig, direkt hinter Überschrift und Marke); `args` = Argumente wörtlich, `body` = Blöcke |
| `work`        | `#arbeit[…]` oder `#arbeit(…)[…]`, wie `scene`                |
| `part`        | Runde 14: Teilmarke `#heading(level: 1, numbering: none)[T] <baruch-part>` (eine Zeile, nur oberste Ebene); `title` = aufgelöster Titel |
| `unnumbered`  | Runde 14: Marke „ohne Nummer“ `#heading(level: N, numbering: none)[T] <baruch-unnumbered>`; `level` 1–3, `title` |
| `island`      | alles, was sich nicht sicher abbilden lässt (siehe Inseln)    |

Im Fließtext (`inlines`, Kennung `t`):

| `t`        | Typst                                                                   |
|------------|-------------------------------------------------------------------------|
| `text`     | Text, Escapes aufgelöst (`\#` → `#`, `\u{2014}` → —, `~` → geschützter Leerraum) |
| `soft`     | einfacher Zeilenumbruch im Quelltext (ein Leerzeichen im gesetzten Text) |
| `emph`     | `_…_` oder `#emph[…]`                                                   |
| `strong`   | `*…*` oder `#strong[…]`                                                 |
| `footnote` | `#footnote[…]`                                                          |
| `quote`    | `#quote[…]` innerhalb eines Satzes                                      |
| `label`    | `<name>`                                                                |
| `island`   | Typst-Insel innerhalb eines Satzes (`raw` wörtlich)                     |
| `linebreak`| Runde 18b: erzwungener Zeilenumbruch `\` (vor Leerraum, Zeilenende oder am Ende; Leerzeichen dahinter gehören dazu) |
| `link`     | Runde 18b: `#link("adresse")[Text]` → `{url, c}`; ohne Inhaltsblock `#link("adresse")` → `{url}` (die Adresse wird angezeigt) |
| `ref`      | Runde 18b: Verweis `@ziel` oder `@ziel[Zusatz]` (auch `#ref(<ziel>)`, `#ref(<ziel>, supplement: […])`) → `{target, supplement?}`; `supplement` ist Typst-Auszeichnung wörtlich |
| `xref`     | Runde 30a: Querverweis `#link(<kN>)[Text]` oder mit Seitenzahl `#link(<kN>)[Text, S.~#context counter(page).at(<kN>).first()]` → `{key, c, page}` (siehe „Querverweise“) |

`_` und `*` zwischen zwei Wortzeichen sind Text (wie in Typst: `datei_name`).

**JSON-Form der neuen Konstrukte (Runde 18b).** Alte Formen bleiben gültig: Eine Liste ohne `depths` hat alle
Einträge oben, und ohne Verschachtelung schreibt der Server kein `depths` (Antworten wie bisher); `enum` ohne `start`
beginnt bei 1. Beispiele:

```json
{"kind": "list", "ordered": false, "items": [[{"t":"text","v":"Obst"}], [{"t":"text","v":"Äpfel"}], [{"t":"text","v":"Gemüse"}]], "depths": [0, 1, 0]}
{"kind": "enum", "start": 3, "items": [[{"t":"text","v":"drei"}], [{"t":"text","v":"vier"}]]}
{"kind": "terms", "items": [{"term": [{"t":"text","v":"Krug"}], "desc": [{"t":"text","v":"Gefäß aus Ton"}]}]}
{"t": "linebreak"}   {"t": "link", "url": "https://example.org", "c": [{"t":"text","v":"Beispiel"}]}   {"t": "ref", "target": "abb-1", "supplement": "S. 3"}
```

**Listen, Aufzählungen, Begriffslisten (Runde 18b).** Jede Zeile, die nach ihrem Einzug mit dem Zeichen der Liste
(`-`, `+`, `N.` oder `/`) beginnt, eröffnet einen Eintrag; die Tiefe ergibt sich aus der Spalte des Zeichens (weiter rechts als
der Eintrag davor = eine Ebene tiefer, sonst die Ebene mit derselben Spalte). Andere Zeilen sind Fortsetzungszeilen und
müssen weiter eingerückt sein als das Zeichen ihres Eintrags. Eine Aufzählung zählt fortlaufend: oben ab `start`, jede
verschachtelte Gruppe ab 1. Der Begriff ist der Text bis zum ersten Doppelpunkt außerhalb von Escapes, Marken,
Verweisen, Rohtext, Formeln, `#`-Ausdrücken und Inhaltsblöcken. **Inseln bleiben:** verschachtelte Einträge mit einem anderen
Zeichen (`- a⏎  + b`), eine Spalte zwischen zwei Ebenen, Tabulatoren im Einzug, leere Einträge, springende Nummern
(`1.`, `3.`), verschachtelte Begriffslisten, ein Begriff ohne Doppelpunkt, mehr als neun Ebenen. Die Erzeugung (`emit`)
ist kanonisch: zwei Leerzeichen Einzug je Ebene, Fortsetzungszeilen eine Ebene tiefer, fortlaufende Nummern, im Begriff
jeder `:` mit Escape (`\:`), sonst würde der Begriff dort enden.

**Grenze: verschachtelte Begriffslisten** (in Runde 21b geprüft, sie bleiben Inseln). `terms` hat kein `depths` wie `list`
und `enum`: Eine Erklärung, die selbst eine Begriffsliste (oder Liste) enthält, wäre im Satzbild ein `<dd>` mit
einem eigenen Block darin – das Modell (`Term { term, desc }` mit Inlines), `emit`/`serialize`, `html.rs`, das Zurücklesen in
`wysiwyg.ts` (`<dl>` → Blöcke) und der Eigenschaftstest müssten dafür gemeinsam um verschachtelte Blöcke wachsen. Die
Insel ist verlustfrei (Text Byte für Byte gleich, im Quelltext bearbeitbar, richtig gesetzt); darum kein Umbau in 21b.
In Runde 23 nicht umgesetzt, bleibt offen (in R24 nicht umgesetzt, Backlog „offen nach R24“; Backlog „Nach Runde 23 offen“; zusammen mit gemischten
Listenzeichen, sobald das Satzbild verschachtelte Blöcke in Listeneinträgen lernt).

**Umbruch und Zeilenende (Runde 21b):** Ein `Soft` direkt hinter einem `Linebreak` (`\` am Zeilenende) erscheint als
leeres `<span class="ws-soft" data-after-break="1"></span>`: Typst schluckt dieses Zeilenende; das Satzbild würde sonst
die nächste Zeile um ein Leerzeichen einrücken. Das Element bleibt, damit das Widget es weiterhin als `soft` zurückliest
(der Text bleibt Byte für Byte gleich).

**Verweise und Umbrüche erzeugen.** Ein Verweis steht als `@ziel`, wenn das Ziel mit einem Buchstaben, einer Ziffer oder `_` beginnt,
nicht auf `.`/`:` endet und das Zeichen dahinter nicht zum Ziel gehören würde; sonst als `#ref(<ziel>)` (z. B.
vor `x` oder `_`). Hinter einem erzwungenen Umbruch steht ein Leerzeichen, außer am Ende des Blocks und vor einem
Zeilenende (`\]` oder `\_` wären Escapes). Ein `\` vor Leerraum ist auch beim Zerlegen in Zeilen ein Umbruch,
kein Escape des Zeilenendes: Das Zeilenende dahinter beendet die Zeile weiterhin (wie in Typst).

**Szenen und Arbeitsaufträge.** Eine Zeile, die genau ein `#szene…[…]` oder `#arbeit…[…]` ist, ist immer
ein eigener Block und schließt sich nicht an Nachbarzeilen an. Sein Rumpf besteht aus eigenen Blöcken
(Absatz, Liste, Zitat, Überschrift; was sich nicht abbilden lässt, bleibt im Rumpf eine Insel). Der gemeinsame Einzug
des Rumpfs zählt beim Lesen nicht (eingerückte Listen werden Listen); das `raw` der Rumpfblöcke sind
Stücke des Originals einschließlich des Einzugs. Argumente vor dem Rumpf (`#szene(ort: [Hof])[…]`) stehen wörtlich in
`args` und lassen sich nur im Quelltext ändern.

**Marken.** Eine Marke am Ende der Überschriftzeile steht in deren `inlines` (wie bisher). Eine Marke allein
in der Zeile direkt hinter der Überschrift gehört ebenfalls zur Überschrift (`label`, `raw` umfasst beide
Zeilen), nicht zu einem Absatz; hinter einem Absatz gehört sie wie bisher zum Absatz.

## Inseln {#islands}

Eine **Typst-Insel** ist Quelltext, den das Satzbild nicht sicher abbilden kann. Sie erscheint mit gelbem
Hintergrund und `contenteditable="false"` und bleibt Zeichen für Zeichen erhalten, solange niemand sie ändert. Im
Satzbild lässt sich eine Insel **als Ganzes löschen** und seit Runde 18b **an Ort und Stelle bearbeiten**
(Doppelklick, siehe „Inseln an Ort und Stelle“).

- **Blockinseln:** Codezeilen (`#let`, `#set`, `#show`, `#import`, `#if` …), Absätze, die nur aus Inseln bestehen
  (`#pagebreak()`, Formel `$ … $` als Block, Rohtextblock ```` ``` ````, Kommentarzeilen, ein Absatz, der nur aus `\` besteht),
  Listen mit gemischten Zeichen und die übrigen unsicheren Listen (siehe oben), eingerückte Überschriften,
  Leerzeilen am Anfang und jeder Absatz, der etwas Unsicheres enthält: eckige Klammern ohne Escape, nicht geschlossene
  `_`, `*`, `$`, `` ` ``, `/*`, ein Schlüsselwort mitten im Satz. Seit Runde 18b keine Inseln mehr:
  Aufzählungen `1.`, Begriffslisten `/ …`, verschachtelte Listen, der erzwungene Zeilenumbruch `\`.
- **Inseln innerhalb eines Satzes:** Formeln `$…$`, Rohtext `` `…` ``, Kommentare `// …` und `/* … */`,
  eigene Funktionen und alle übrigen `#`-Ausdrücke (`#box[…]`, `#zitat(…)`, ein `#link` mit mehr als einer
  Adresse als Argument; `#link("…")[…]` und Verweise `@name` sind seit Runde 18b eigene Konstrukte) sowie
  Escapes, die als Text zu einer Kurzform würden: `\'` und `\"` (gerade statt typografischer
  Anführungszeichen), `\-` (sonst `--`, `---`, `-?`), `\.` neben einem Punkt (sonst `...`), `\?` und eine
  mit Escape geschriebene Ziffer hinter `-`. Andere Escapes (`\#`, `1\.` …) werden Text und beim Speichern erneut mit Escape geschrieben.
- **Zeilenenden:** Typst bricht auch bei U+2028, U+2029, U+0085, `\x0B` und `\x0C` um. Im Satzbild (Tippen,
  Einfügen) und beim Erzeugen werden alle diese Zeichen in Absätzen zu Leerzeichen
  (`baruch_typst::is_newline`); sonst stünde Text ungeschützt am Zeilenanfang (`= `, `- ` …). In einer
  **Überschrift** dagegen beendet ein Zeilenende sie (siehe „Überschriften“ unten), und ein Titel mit einem
  Zeilenende aus dem Baum oder aus einer Überschriftzeile wird abgelehnt (Runde 7, `baruch_store::check_title`;
  vorher wurde er stillschweigend zu einem Leerzeichen).
- Eine Überschrift mit Kommentar (`== Titel // Notiz`) ist keine Insel: Der Kommentar gehört nicht zum Titel
  und bleibt beim Speichern erhalten.

### Inseln an Ort und Stelle (Runde 18b) {#islands-in-place-round-18b}

- **Bedienung:** Ein Doppelklick auf eine Insel (auch auf ein Bild, eine Insel innerhalb eines Satzes oder im Rumpf einer Szene) öffnet
  an ihrer Stelle ein kleines Quelltextfeld (`.ws-island-edit`, Textfeld mit „Übernehmen“/„Abbrechen“).
  Strg+Enter oder „Übernehmen“ (auch das Verlassen des Felds nach einer Änderung) übernimmt, Escape oder
  „Abbrechen“ stellt die Insel wieder her. Tasten im Feld gehen nicht an das Satzbild (Enter teilt keinen Absatz).
  Inseln in Vorschlägen (`.pv`) öffnen kein Feld.
- **Speichern:** Eine geänderte Blockinsel (`data-edited="1"`, neuer Quelltext in `data-raw`) geht als
  `{"replaces": i, "raw": "<Typst>"}` an `wysiwyg.serialize` (im Rumpf einer Szene in deren Rumpfanweisung); der
  Server übernimmt den Text **wörtlich** (nachfolgender Leerraum entfällt, der Trenner des alten Blocks bleibt; im Rumpf
  wird der Einzug des Rumpfs einheitlich gesetzt, auch vor jeder Zeile eines Rohtextblocks ```` ``` ````; der Text darf
  mit dem Einzug gesendet werden – wie in `data-raw`, die erste Zeile mit oder ohne – oder ganz ohne ihn: Er gilt
  als mitgesendet, wenn jede nicht leere Fortsetzungszeile mit ihm beginnt, sonst bleibt der Text, wie er ist, und der
  Einzug eingerückter Codezeilen ändert sich nicht), ein leerer Text löscht die Insel. Eine Insel innerhalb eines Satzes gehört zu
  ihrem Absatz: Der Absatz wird mit der geänderten Insel neu erzeugt, die Insel steht darin wörtlich.
- **Grenzen:** `{"raw": …}` ohne `replaces` (eine neue Insel) bleibt verboten (`NewIsland`), ein unbekannter oder
  doppelter Index ist ein Fehler (-32602). Der Server prüft den neuen Quelltext nicht; was er bewirkt (auch eine neue
  Überschrift), prüft wie immer die Gliederung beim Speichern.

## Überschriften (Runde 7) {#headings-round-7}

Ursache des Fehlers „Absatz wird Titel“ (Backlog, Nacht vom 5. auf den 6. Nov. 2026): Das Widget schickte eine Überschrift,
auf deren Titel ein einfacher Zeilenumbruch folgte (Umschalt+Enter oder ein `<br>` des Browsers, dann der
getippte oder eingefügte Absatz), als **einen** Überschriftblock `{kind: heading, inlines: [Titel, soft,
Absatz]}`. `emit` machte daraus eine einzige Überschriftzeile (`soft` in Überschriften → Leerzeichen), und
`outline.save` nahm die ganze Zeile als Titel des Gliederungspunkts. Bewiesen durch
`crates/baruch-server/tests/round7_titles.rs` (schlägt ohne die Korrektur mit dem Titel „Laub Anna's Laube …“ fehl).

Seitdem gilt:

- Eine Überschrift ist **immer eine Zeile**: `emit` und `serialize` beenden sie am ersten Zeilenumbruch der
  obersten Ebene (`Inline::Soft` oder ein Zeilenende im Text); was dahinter steht, wird ein eigener Absatz
  darunter (Umbrüche am Anfang zählen nicht). Marke und Kommentar der ursprünglichen Zeile bleiben bei der
  Überschrift. Nur echte Überschriftzeilen (Spalte 0, 1–3 `=`, dann Leerzeichen/Tabulator, wie
  `baruch_typst::parse`) liefern Titel.
- Das Widget schickt eine solche Überschrift schon selbst geteilt (Überschrift + neuer Absatz); Umschalt+Enter
  und das Einfügen mehrerer Zeilen beginnen in einer Überschrift einen neuen Absatz; Rücktaste am Anfang eines
  Absatzes hinter einer Überschrift oder Entf am Ende einer Überschrift ziehen den Absatz nicht in die
  Überschrift (ein leerer Absatz entfällt).
- **Titelregeln** (Server, `baruch_store::check_title`): leer, nur Leerraum, ein Zeilenumbruch oder mehr als
  200 Zeichen → abgelehnt (-32602), nichts gespeichert. Vor der Übernahme ins Textfeld prüft das Widget
  dieselben Regeln mit derselben Meldung („Ein Gliederungspunkt braucht einen Titel.“, „Der Titel darf
  keinen Zeilenumbruch enthalten.“, „Der Titel ist zu lang (höchstens 200 Zeichen, hier N).“); der Server
  bleibt maßgeblich. Ein ganzer Absatz, der versehentlich in die Überschriftzeile geraten ist, scheitert somit
  an der Länge.

## Markdown-Überschriften (Nutzer 2026-11-07) {#markdown-headings-user-2026-11-07}

Eine Zeile, die mit 1–6 `#` und einem Leerzeichen beginnt, ist eine Überschrift dieser Ebene. Maßgeblich ist
die Regel des Servers, `baruch_typst::markdown` (`markdown_headings`, Tabellentest mit allen Ausnahmen): Er
wendet sie an, wenn der Mensch speichert (`outline.save`, `outline.check`, die Rückfrage vor dem Teilen,
`revision.save`, seit Runde 18b auch `revision.update` (Fassung ändern), auch ohne JavaScript) und wenn ein Vorschlag übernommen wird (`revision.accept`); danach
gleicht `outline_save` die Gliederung ab wie bei einer getippten `=`-Zeile.

- `#` → `=`, `##` → `==`, `###` → `===`; `####` bis `######` werden bei `===` **gekappt** (die Gliederung hat
  drei Überschriftebenen; ob die Ebene an der Stelle passt, prüft wie immer die Gliederung beim Speichern).
- Titel wörtlich wie in einer `=`-Zeile, eine abschließende Folge ` ##` und nachfolgender Leerraum entfallen; `# ` ohne Titel
  bleibt.
- **Nicht umgewandelt:** `\#` am Anfang, ein Doppelkreuz in Anführungszeichen (`'# …'`, `"# …"`, „…“), Typst-Code
  (`#heading(…)`, `#set`, `#let` – kein Leerzeichen hinter dem Doppelkreuz), eingerückte Zeilen, Zeilen in Rohtextblöcken,
  Kommentare, Zeichenketten und offene Klammern (Inhaltsblöcke wie `#quote[…]`) – dieselben Stellen, an denen
  auch eine `=`-Zeile keine Überschrift ist –, und die Werkvorlage (sie durchläuft die Regel nicht).
- **Live im Widget** (`web/src/markdown.ts`, dieselben Fälle, im Zweifel keine Umwandlung): im Quelltext bei
  Enter am Ende oder innerhalb der Zeile (der Cursor bleibt an seiner Stelle im Titel, dann der Zeilenumbruch) und beim
  Verlassen des Textfelds für alle Zeilen; im Satzbild wird ein Absatz, dessen Text mit `# ` … beginnt, bei
  Enter (und spätestens vor der Übernahme ins Textfeld) ein Überschriftblock dieser Ebene, die Doppelkreuze
  verschwinden, der Cursor bleibt im Titel. Im Satzbild schreibt `emit` ein getipptes Doppelkreuz sonst als `\#`
  (Text), darum wandelt hier nur das Widget um.
- **Vorschläge** von Assistenten bleiben, wie sie geschrieben sind (keine Umwandlung, keine Gliederung).
  Übernimmt der Mensch einen Vorschlag mit Markdown-Überschriften, bleibt der Vorschlag im Fassungsbaum unverändert;
  es folgt eine Fassung des Menschen mit den umgewandelten Überschriften (weitere Überschriften
  werden neue Punkte, die Übernahme ist die Bestätigung). Gelingt das nicht, bleibt die Übernahme, wie sie ist, und
  `markdown` in der Antwort nennt den Grund.

## Teile und „ohne Nummer“ (Runde 14) {#parts-and-ohne-nummer-round-14}

- **Erkennen:** Eine Zeile ist genau dann ein Block `part` oder `unnumbered`, wenn `baruch_typst::parse` sie
  als Teil (`level` 0) oder als Überschrift mit `numbered: false` liest – also nur in der kanonischen Form
  (`docs/Architektur.md`, „Parts in the Typst text (Round 14)“), nicht eingerückt, nur auf der obersten Ebene (im Rumpf einer
  Szene ist dieselbe Zeile eine Insel, so wie sie für `baruch_typst` Text ist). Die Zeile ist immer ein
  eigener Block wie eine Überschrift; ein Kommentar dahinter gehört zu `raw`, nicht zum Titel.
- **Beibehalten:** `{keep: i}` übernimmt die Zeile wörtlich (Escapes, Kommentar).
- **Ändern:** `{replaces: i, block: {kind: "part", title}}` baut die Zeile mit `baruch_typst::part_line` neu
  (`heading_line(0, …)`: Ein Kommentar der alten Zeile bleibt, auch wenn sie eine `=`-Zeile war);
  `{kind: "unnumbered", level, title}` mit `unnumbered_line` (ersetzt es eine Marke „ohne Nummer“, bleibt deren
  Kommentar). Ein neuer Block (`replaces: null`) ergibt die Marke ohne Kommentar. Für jeden Titel
  gilt `parse(emit(k)) == k` (Eigenschaftstest `random_markers_read_back_unchanged`).
- **Teil im Kapiteltext (Runde 18b, entschieden):** Das Satzbild bietet keinen Teil an. `wysiwyg.serialize` weist einen
  **neuen** Teilblock (`replaces: null`) und das Umwandeln eines anderen Blocks in einen Teil ab
  (`WysiwygError::NewPart`, -32602 „Einen Teil legt man in der Gliederung an, nicht im Satzbild.“); das Umbenennen eines vorhandenen
  Teils funktioniert weiterhin. Schreibt der Mensch die Teilmarke im Quelltext als weitere Überschrift in einen
  Kapiteltext, verlangt `outline.check` die Rückfrage zum Teilen; ohne Bestätigung speichert `outline.save` nichts,
  mit Bestätigung (`split`) entsteht der Teil (eine Handlung des Menschen). Test:
  `crates/baruch-server/tests/round18_konstrukte.rs`
  (`a_part_block_in_a_chapter_text_is_refused_in_the_satzbild_and_needs_confirmation_in_outline_save`).
- **HTML:** `<div class="ws-block ws-part" data-b="i" data-kind="part" data-level="0" role="heading"
  aria-level="1" title="Teil (Markierung im Typst-Text)">Titel</div>` oder `class="ws-block ws-unnumbered"
  data-kind="unnumbered" data-level="N" role="heading" aria-level="N"`; der Titel steht als reiner Text (mit Escape),
  „Teil“ oder „ohne Nummer“ setzt das Stylesheet über `::before`/`::after` (`static/wysiwyg.css`), damit es
  nicht in den Titel gerät.
- **Widget:** `kindOf` liest `data-kind=part|unnumbered` mit dem Textinhalt als Titel (`markerTitle`:
  Zeilenenden und geschützte Leerzeichen → Leerzeichen); Teil und „ohne Nummer“ verhalten sich wie
  Überschriften (`isTitleKind`: Enter und das Einfügen mehrerer Zeilen beginnen dahinter einen Absatz,
  `guardHeadingMerge`, Titelregeln in `headingProblem`). Das Absatzformat lässt sich für sie nicht wählen
  (Meldung); Teile legt man in der Gliederung an, nicht im Satzbild.
- **Ohne JavaScript** behält das Textfeld die Marke wörtlich; Speichern wie bisher.

## Rundlauf {#round-trip}

Laden und Speichern ohne Änderung ergibt **denselben Text, Byte für Byte**. Dafür
baut `baruch_wysiwyg::serialize(original, edits)` den Text aus Anweisungen:

- `{"keep": i}`: Block `i` des Originals wörtlich (einschließlich seiner Leerzeilen). Das Widget sendet `keep` für
  **jeden Block ohne Eingabe**; ein unberührter Block wird also nie neu erzeugt.
- `{"replaces": i, "block": {…}}`: Block `i` neu erzeugen (kanonisches Typst, `baruch_wysiwyg::emit`), mit
  den Leerzeilen des alten Blocks dahinter. Eine bearbeitete Überschrift behält Marke und Kommentar der
  ursprünglichen Zeile (wie `baruch_typst::heading_line`; die Marke nur, wenn die neue keine trägt).
- `{"replaces": null, "block": {…}}`: neuer Block, gefolgt von einer Leerzeile.
- `{"replaces": i, "body": [Anweisungen]}`: Änderung nur im Rumpf der Szene oder des Arbeitsauftrags `i`, mit denselben
  Regeln für die Blöcke des Rumpfs (Indizes innerhalb des Rumpfs). Rahmen (`#szene(…)[`, `]`), Argumente und Leerraum
  bleiben wörtlich; neu erzeugte Blöcke erhalten den Einzug des Rumpfs. Der Einzug der ersten Zeile des
  Rumpfs gehört zum ersten Block und wandert mit ihm, wenn davor etwas eingefügt oder umgeordnet wird (sonst
  stünde seine erste Zeile in Spalte 0 und eine Liste würde verschachtelt). Das Bearbeiten eines
  Absatzes in einer Szene ändert also nur diesen Absatz. Kein Rumpf (Absatz, Insel …) → Fehler.
- Eine bearbeitete Überschrift behält auch ihre Markenzeile wörtlich; trägt der neue Block eine andere
  `label`, steht dort `<neu>`.
- Eine Überschrift mit Zeilenumbruch endet dort; der Rest wird ein Absatz dahinter (Runde 7, siehe oben).
- Nicht genannte Blöcke werden gelöscht. Ein Block, der leer herauskommt, entfällt. Folgen zwei Stücke nicht aufeinander
  wie im Original, steht mindestens eine Leerzeile dazwischen.
- `{"replaces": i, "raw": "…"}` (Runde 18b): Quelltext des Blocks `i` wörtlich ersetzen (Insel an Ort und Stelle,
  siehe oben), mit dem Trenner des alten Blocks; leer = gelöscht.
- Fehler (`WysiwygError`, deutsch): unbekannter Index, ein mehrfach genannter Index (auch `keep` und `replaces`),
  eine Insel als neuer Block (auch `{"raw": …}` ohne `replaces`) oder eine als Block bearbeitete Insel, eine
  Rumpfanweisung für einen Block ohne Rumpf, ein neuer oder umgewandelter Teil (Runde 18b, `NewPart`).

Kanonisches Typst (`emit`): Sonderzeichen erhalten ein Escape (`\#`, `\*`, `\_`, `\$`, `\@`, `\<`, `\[` …), am
Zeilenanfang auch `=`, `-`, `+`, `/` und `1.`; ein geschütztes Leerzeichen wird `~`; `_…_`/`*…*` nur an
Wortgrenzen und nicht in sich selbst verschachtelt, sonst `#emph[…]`/`#strong[…]`; hinter einem `#`-Ausdruck
erhalten `(` und `.name` ein Escape; Listeneinträge mit zwei Leerzeichen Einzug in Fortsetzungszeilen; `#szene[…]` und
`#arbeit[…]` mehrzeilig, `#quote(…)[…]` mit einem einzeiligen Absatz in einer Zeile.

Bewiesen durch Tests in `crates/baruch-wysiwyg/tests/`:
- `roundtrip.rs`: alle Testdateien aller Crates (`crates/*/tests/data/*.typ`, nur gelesen) und die eigenen
  `tests/data/satzbild.typ` (alle Konstrukte und Inseln) und `tests/data/szenen.typ` (Überschriften mit
  Marke, Szenen mit mehreren Absätzen und einer Fußnote, Arbeitsauftrag mit einer Liste, Bildinsel), beide frei
  erfunden, ganz und je Abschnitt;
- `blocks.rs`: Szenen, Arbeitsaufträge, Marken und Rumpfanweisungen (nur der bearbeitete Absatz ändert sich),
  Einfügen mehrerer Absätze mit den Anweisungen, die das Widget sendet;
- `serialize.rs` (Runde 7): eine Überschrift mit Zeilenumbruch endet dort, auch wenn sie neu eingefügt wird und mit
  Marke/Kommentar; `html.rs`: Bildinseln als `ws-figure` mit Attributen mit Escape, Byte-identisch mit `keep`;
- `emit.rs`: `parse(emit(k)) == k` für jede Art außer der Insel, dazu ein Eigenschaftstest mit festem Startwert über
  2000 zufällige kanonische Blöcke (mehr mit `WS_CASES=…`, anderer Startwert mit `WS_SEED=…`), seit Runde 18b mit
  Aufzählungen, Begriffslisten, verschachtelten Listen, Umbrüchen, Links und Verweisen; die Lücke des Erzeugers
  (Leerraum am Anfang einer Hervorhebung in einer Überschrift, `canonical_heading`) ist mit dem Fall aus dem Backlog
  als eigener Test geschlossen;
- `konstrukte.rs` (Runde 18b): Erkennen, Byte-identisch bei Nichtberühren, kanonisch beim Bearbeiten, unsichere Listen als Inseln,
  alte JSON-Formen, `Edit::Raw` (auch im Rumpf einer Szene), mehrere Bilder, HTML der neuen Konstrukte;
- `align.rs` (Runde 8): Abgleich zweier Fassungen nach Blöcken, siehe „Vorschläge an ihrer Stelle“;
- `serialize.rs`, `html.rs`, `style.rs`; dazu `crates/baruch-server/tests/wysiwyg.rs` über alle Fassungen des
  Beispielwerks (im Quelltext und als Ausgangsbestand) und die Exporte, `crates/baruch-server/tests/satzbild.rs` für den
  Feinschliff (Bildadresse, Abbildung als Bild und Byte-identisch gespeichert, Stil, Seite ohne JavaScript,
  Widget mit Titelprüfung), `crates/baruch-server/tests/round7_titles.rs` für „Absatz wird Titel“ und die
  Titelregeln auf allen Wegen (RPC, Formular, MCP).

## Vorschläge an ihrer Stelle (Runde 8) {#proposals-in-their-place-round-8}

Offene Vorschläge stehen nicht mehr als Liste unter dem Editor, sondern **an der Stelle, die sie ändern**: im
Satzbild, im Lesemodus des Quelltexts und in der Vorschau, in der Farbe der Persona, genau dort zu einer farbigen
Zeile einklappbar.

**Abgleich** (`baruch_wysiwyg::align(base, proposal) -> Vec<Hunk>`, rein, schlägt nie fehl): Beide Texte werden
mit `parse` in Blöcke der obersten Ebene zerlegt und über die längste gemeinsame Teilfolge verglichen. Zwei
Blöcke sind gleich, wenn ihr `raw` ohne nachfolgenden Leerraum Byte-identisch ist (die Leerzeilen hinter einem Block gehören zu
seinem `raw`; sonst würde jedes Anhängen den bisher letzten Block als geändert melden). Jede zusammenhängende Folge
ungleicher Blöcke ist ein `Hunk { kind, base, proposal }` mit Indexbereichen der Blöcke:

| `kind`     | `base`                | `proposal`         |
|------------|-----------------------|--------------------|
| `Changed`  | nicht leer            | nicht leer         |
| `Inserted` | leer; `base.start` = Block, vor dem eingefügt wird (am Ende die Anzahl der Blöcke) | nicht leer |
| `Removed`  | nicht leer            | leer               |

Zwischen den Stücken stehen auf beiden Seiten gleich viele, paarweise gleiche Blöcke (auch null: Stücke dürfen
aneinanderstoßen). Ab 4 Millionen Vergleichen (Blockzahl × Blockzahl) gilt alles als ein Stück.

**Ähnliche Blöcke (Runde 18c):** Die längste gemeinsame Teilfolge kennt nur gleiche Blöcke; ein umformulierter
Absatz neben einem eingefügten wurde daher zu einem einzigen „geänderten“ Stück über beide. Ein „geändertes“ Stück mit
**ungleicher Blockzahl** auf den beiden Seiten wird deshalb nach Ähnlichkeit geteilt: Wort-Dice (2 · gemeinsame Wörter /
alle Wörter, kleingeschrieben, mit Vielfachheit) ≥ 0,5 gilt als „umformuliert“; die Paare werden monoton mit
der größten Summe der Ähnlichkeiten gewählt. Paare werden „geändert“ (aufeinanderfolgende Paare bleiben ein
Stück), was nur auf einer Seite dazwischen steht, wird „eingefügt“ oder „entfernt“, was auf beiden Seiten ungepaart steht,
bleibt „geändert“. Bei gleich vielen Blöcken (paarweise ersetzt), ohne ähnliches Paar oder ab 10 000
Vergleichen im Stück bleibt es, wie es war. Beispiel: „Im Beet stehen Kohlköpfe.“ → „Im Beet stehen heute
Kohlköpfe.“ + ein neuer Absatz dahinter ergibt `Changed(2..3, 2..3)` und `Inserted(3..3, 3..4)`. `hunk_html_with(base, proposal, &hunk, &bilder)` zeigt ein
Stück im Satzbild: `<div class="ws-diff-doc" data-kind="changed|inserted|removed">`, die Blöcke der Grundlage in
`div.ws-del`, die des Vorschlags in `div.ws-ins` (wie `diff_html_with`, ohne `data-b`, Bilder wie dort).
Tests: `crates/baruch-wysiwyg/tests/align.rs` (geändert; eingefügt am Anfang, in der Mitte, am Ende; entfernt;
mehrere Stücke; Satzbild; Eigenschaftstest mit festem Startwert über 1500 Fälle, `WS_SEED`/`WS_CASES`; Runde 18c:
umformuliert + eingefügt oder + entfernt als zwei Stücke).

**Grundlage** ist die **gezeigte** Fassung (im Editor geöffnet, meist die aktuelle) – das ersetzt „Übernehmen“.
Beruht der Vorschlag auf einer anderen Fassung, nennt die Kopfzeile sie („· auf Fassung m“). Ist die
gezeigte Fassung selbst ein offener Vorschlag, stehen ihre Stücke gegen ihre Grundlage (Elternfassung, sonst die
aktuelle) an ihrer Stelle darin, mit dem Hinweis „Unterschied zu Fassung b (aktuell) · gezeigt“. Gleicht ein
Vorschlag der gezeigten Fassung, steht er als ein Stück am Ende („gleicht der gezeigten Fassung“), damit
„Übernehmen“/„Verwerfen“ bleiben.

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
geänderten oder entfernten Block, am Ende die Anzahl der Blöcke). Die **Kopfzeile steht nur einmal**, in der Zeile
(`summary`); „Teil i/k“ nur bei mehreren Stücken. Mehrere Vorschläge an derselben Stelle stehen untereinander
in der Reihenfolge der Personas, darin nach Nummer; Autoren ohne Persona am Ende. Die Formulare (`revision.accept` oder
`revision.reject` an `/act` mit `work`, `node`, `key`, `rev`, `view`, `lq`, unverändert) stehen **einmal je
Vorschlag** außerhalb der Stücke (`div.pv-forms`); die Knöpfe verweisen über das Attribut `form` auf sie. So bleiben sie
gültig, wenn das Widget die Stücke ins Satzbild setzt, das im Editor-Formular liegt (Formulare lassen sich nicht
verschachteln, und `outline.ts` würde ein Absenden darin für das Speichern halten). Das Cookie `baruch_folded`
(`<key>-<rev>`) klappt alle Stücke eines Vorschlags zu; der gezeigte Vorschlag bleibt offen (`pv-shown`).

**Lesefluss im Editor:** `<section class="pv-flow" data-widget="proposals" data-key="K" data-rev="R">` unter dem
Editor-Formular (also unter dem Textfeld), nur wenn es offene Vorschläge gibt: die Formulare, die gezeigte Fassung
Block für Block als Quelltext (`<pre class="pv-src" data-b="i">raw</pre>`) und die Stücke an ihrer Stelle. Seit Runde 18c steht die
**Leiste** (Anzahl, „Alle einklappen“) **vor** dem Lesefluss als eigenes Kind des Blatts
(`<div class="pv-bar pv-bar-editor" data-pv-bar="K">`); CSS (`proposals.css`) schiebt sie mit `order: -1` ganz nach oben
und lässt sie beim Scrollen haften. So bleibt „Alle einklappen“ auch bei geöffnetem Satzbild sichtbar, wenn
`wysiwyg.ts` den Lesefluss verbirgt (vorher stand die Leiste im verborgenen Lesefluss); `proposals.ts` findet den
Knopf über `data-pv-bar` beim Nachbarn des Leseflusses, `wysiwyg.ts` bleibt unverändert. In der Vorschau
steht die Leiste weiterhin im Abschnitt. Der **Zeilen-Diff** eines Stücks vergleicht die Blöcke ohne nachfolgenden Leerraum
(Runde 18c): Die Leerzeile hinter einem Block gehört zu seinem `raw` und stand sonst als leere „−“-Zeile am Ende. Ohne JavaScript und im Quelltext ist dies die Ansicht der
Vorschläge (Zeilen-Diff). Mit JavaScript und geöffnetem Satzbild setzt `wysiwyg.ts` jedes Stück als nicht bearbeitbaren
Nachbarn (`contenteditable="false"`) vor den Block `[data-b=gap]` des Satzbilds (am Ende
dahinter) und verbirgt den Lesefluss; im Satzbild zeigt das Stück sein Satzbild (`pv-satz`), nicht den
Zeilen-Diff (nur CSS). Beim Wechsel in den Quelltext und vor jedem neuen Satzbild kehren die Stücke an ihre
Stelle im Lesefluss zurück. `editsOf`, `blocksIn` und `markSelection` überspringen `.pv`; das Speichern ändert deshalb
nichts.

**Vorschau** (`view=preview`): je Abschnitt des Kapitels ein `div.pv-section` mit seinen Blöcken als Zeilen
(`h1`–`h3`, `p`) und den Stücken (nur Zeilen-Diff) zwischen den Blöcken an ihrer Stelle; die Grundlage ist die dort gezeigte
Fassung (aktuelle oder probeweise gesetzte).

**Farben:** Auf dem stets hellen Blatt (Satzbild, Vorschau) stehen die Stücke in den hellen Werten
(`--pc-line`, `--pc-light`), sonst je Thema `--pc-line` oder `--pc-line-dark`. Beide Linien erreichen
mindestens 3:1 gegen die Fläche ihres Themas (WCAG 1.4.11, `crate::personas::style`, `MIN_LINE_CONTRAST`),
`--pc-line` zugleich gegen das Blatt des dunklen Themas (`--sheet: #f4f1ea`, `DARK_SHEET`):
Reicht die Farbe nicht, wird sie für hell abgedunkelt und für dunkel aufgehellt (in Schritten von 5 %).

## Satzbild als Voreinstellung (Runde 8) {#typeset-view-as-the-default-round-8}

Fehler aus der Sichtprüfung: Nach dem Laden war auf allen Bildschirmfotos „Quelltext“ aktiv, obwohl die Voreinstellung
„satz“ ist. **Ursache** (Code, Stand development f601260): Der Server lieferte immer `aria-pressed="true"` bei
„Satzbild“ und ein **leeres** Satzbild. Beim Einhängen rief `wysiwyg.ts` zuerst `show(state, root)` mit
`open = false` auf – das drückt „Quelltext“, verbirgt Werkzeuge und Blatt und zeigt das Textfeld – und öffnete das
Satzbild erst in `openSheet` nach zwei RPC-Antworten (`wysiwyg.parse`, `wysiwyg.style`). Das Laden der Seite
(Ereignis `load`) wartet nicht auf diese `fetch`-Aufrufe; ein Bildschirmfoto direkt nach dem Laden
(`firefox --screenshot`) zeigt daher den Zwischenzustand „Quelltext“. Ein Fehler beim Öffnen hätte ihn
dauerhaft gemacht, aber es brauchte keinen Fehler.

**Korrektur:** Der Server zeichnet den Zustand selbst: `Settings.editor` (`satz` | `quelltext`, aus dem Cookie
`baruch_editor`, Voreinstellung `satz`) setzt `aria-pressed`, `data-ws-start` und – bei „quelltext“ – `hidden` an
Werkzeugen, Hinweis und Blatt. Bei „satz“ steht das Satzbild der gezeigten Fassung fertig im Blatt
(`<div … data-ws-sheet … data-ws-ready>…</div>`, dasselbe HTML wie `wysiwyg.parse`, mit Bildern). Das Widget
übernimmt es beim Einhängen sofort (`adoptSheet`), ohne Zwischenzustand; der Stil der Werkvorlage folgt über
`wysiwyg.style`. Liefert das Textfeld nicht denselben Text wie die Fassung (ein Zeilenumbruch direkt am
Anfang, den der Browser hinter `<textarea>` entfernt, oder `\r`), fehlt `data-ws-ready`, und das Widget öffnet
das Satzbild wie bisher über RPC – ohne vorher in den Quelltext zu wechseln (nur das Textfeld wird kurz
verborgen; scheitert das Öffnen, kommt der Quelltext). `#fundstelle` und das Cookie „quelltext“ öffnen den
Quelltext wie bisher. Test: `crates/baruch-server/tests/round8_inline.rs`
(`the_server_already_renders_the_satzbild_as_default_and_the_source_only_on_request`, schlägt ohne die
Korrektur bei `data-ws-start`/`data-ws-ready` fehl).

**Lange Abschnitte (Runde 18c):** Ab 64 KiB Text (`view::READY_SHEET_MAX`, Bytes) legt der Server das Satzbild nicht
vorab ins HTML – es verdoppelte den Text (Textfeld, Blatt, mit Vorschlägen auch der Lesefluss). Dann
fehlt `data-ws-ready`, und das Widget holt das Satzbild wie oben über `wysiwyg.parse`, ohne vorher den Quelltext zu zeigen.

**Stil schon im HTML (Runde 18c):** Der Editor bringt `<style id="ws-style" data-work="W">` mit (CSS aus
`sheet::sheet_css_for`: Werkvorlage, überschrieben mit den Satzbild-Einstellungen des Werks, siehe unten), damit
das Blatt nach dem Laden Schrift und Größe nicht ändert; `wysiwyg.ts` übernimmt ein vorhandenes `#ws-style` mit
demselben `data-work` (Runde 18b).

**Verhaltenstest (Runde 18c):** `crates/baruch-server/tests/round18_satzbild.rs`
(`satzbild_as_default_behaves_right_in_the_real_widgets_without_a_browser`) lässt die kompilierten Widgets
`wysiwyg.js`, `proposals.js` und `sheet.js` in Node 20 auf einem kleinen nachgebildeten DOM gegen das Editor-HTML
von `ui.render` laufen – ohne Browser, ohne Netz (`fetch` ist nachgebildet und zählt die RPC-Aufrufe). Der Test belegt
das Verhalten: Ohne Cookie ist das Satzbild beim Einhängen schon offen (Textfeld verborgen, „Satzbild“ gedrückt,
kein `wysiwyg.parse`), die Stücke stehen im Satzbild, der Lesefluss ist verborgen und „Alle einklappen“ sichtbar;
mit dem Cookie „quelltext“ bleibt das Textfeld offen; ohne vorab gesetztes Satzbild holt das Widget es über
`wysiwyg.parse` und öffnet es danach. Nachgebildet ist nur, was die Widgets beim Einhängen brauchen (Selektoren mit
Tag, `#id`, `.klasse`, `[a]`, `[a=b]`, Nachfahre, Kind, `:scope`); was fehlt, meldet der Test als Fehler in
der Nachbildung. Ohne `node` wird er mit „ÜBERSPRUNGEN“ übersprungen.

## Anzeige nach der Werkvorlage {#display-according-to-the-work-template}

`baruch_wysiwyg::sheet_style(vorlage)` liest aus der Werkvorlage nur Regeln der **obersten Ebene** (Kommentare
und Rohtext zählen nicht; die letzte Regel gewinnt, auch mehrere `#set` in einer Zeile). Regeln in
`#show …: it => […]`, in `#show: …`-Rümpfen, in Funktionsrümpfen (`#let f() = { set par(…) }`), in `#if`
oder in anderen Ausdrücken gelten nur für einen Teil des Dokuments und zählen nicht; ebenso wenig
`#show footnote: set text(size: …)` (nur `emph`, `strong` und `heading` mit eigenen Feldern):

| Regel                                         | Feld                         | CSS-Variable              |
|-----------------------------------------------|------------------------------|---------------------------|
| `#set text(font:, size:, fill:)`              | `font`, `size`, `fill`       | `--ws-font`, `--ws-size`, `--ws-fill` |
| `#set par(first-line-indent:, leading:, justify:)` | `indent`, `leading`, `justify` | `--ws-indent`, `--ws-leading`, `--ws-justify` |
| `#set page(width:)` oder `paper:`             | `width`                      | `--ws-width`              |
| `#show emph: set text(fill:)`                 | `emph_fill`                  | `--ws-emph-fill`          |
| `#show strong: set text(fill:)`               | `strong_fill`                | `--ws-strong-fill`        |
| `#show heading: set text(font:, fill:)`       | `heading_font`, `heading_fill` | `--ws-heading-font`, `--ws-heading-fill` |
| `#let szene(…) = block/rect/box(fill:, inset:, stroke:)` | `scene`           | `--ws-scene-fill/-inset/-stroke` |
| `#let arbeit(…) = …`                          | `work`                       | `--ws-work-fill/-inset/-stroke` |

Farben: `rgb("#…")`, `rgb(r, g, b)` (auch Prozent), `luma(n)`, `cmyk(…)` und die Farbnamen von Typst;
Längen in pt, mm, cm, in, em. Was sich nicht übersetzen lässt (z. B. `red.lighten(20%)`), fehlt; dann
gilt die Voreinstellung aus `wysiwyg.css`. `sheet_css` schreibt nur Werte aus sicheren Zeichen (Buchstaben, Ziffern,
` #%.,()-+_"'`, Anführungszeichen nur paarweise – nie `<`, `;`, `{`, `}`, `\`), darum darf das Ergebnis ohne Escape in
ein `<style>`-Element.

**Satzbild-Einstellungen je Werk (Runde 18c):** „Einstellungen → Werk → Satzbild“ (RPC `work.sheet`,
`work.sheet_set`, Speicher `work_sheet`, Migration 0044) überschreiben feldweise, was die Vorlage liefert:
`sheet_style_over(sheet_style(vorlage), einstellungen)` – jedes gesetzte Feld gewinnt, der Rest bleibt aus der
Vorlage. Felder: `font`, `heading_font`, `size`, `fill`, `heading_fill`, `width` und `background` (Blattfarbe,
`SheetStyle.background` → `--ws-background`; die Vorlage liefert sie nie, das Blatt bleibt sonst hell; `sheet.css`
liest sie). `wysiwyg.style`, das `<style id="ws-style">` im Editor und `work.sheet` rechnen über dieselbe Funktion
(`crate::sheet::sheet_style_for`) und liefern dasselbe CSS. Die Einstellungen gelten **nur im Satzbild des
Browsers**: Typst-Satz, PDF und Export lesen `work_sheet` nie (Byte-identisch, Test). Nach dem Speichern in den
Einstellungen (`baruch:settings-changed`) holt `sheet.ts` `work.sheet` und ersetzt den Inhalt von `#ws-style`.

## RPC {#rpc}

Alle Methoden lesen nur (keine Schreibsperre), nur über `POST /rpc` – nicht über `/act`, nicht über MCP.
Fehlt `work`, gilt das Standardwerk. Fehler über `INVALID_PARAMS` (-32602).

| Methode | Parameter | Ergebnis |
|---------|-----------|----------|
| `wysiwyg.parse` | `{work?, body}` | `{blocks, html}`: Blöcke (wie oben, mit `raw`) und HTML des Satzbilds (Bilder in Inseln, siehe unten) |
| `wysiwyg.serialize` | `{work?, body, edits}` | `{body}`; ungültige Anweisungen → -32602 mit deutscher Meldung; Runde 18b auch `{replaces, raw}` (Insel an Ort und Stelle) |
| `wysiwyg.style` | `{work?}` | `{css, style}` aus der Werkvorlage (`Store::work_template`) |
| `wysiwyg.diff` | `{work?, key\|node, rev, base?}` | `{html, base}`: Fassung `rev` gegen `base`; fehlt `base`, deren Elternfassung (`based_on`), sonst die aktuelle; Bilder wie bei `parse` |

HTML (`render_html`): je Block `<div class="ws-block" data-b="i" data-kind="…">` (Überschrift mit
`data-level`, Liste mit `data-ordered`, Zitat mit `data-block`; Rumpf in `div.ws-body`, Zuschreibung in
`div.ws-attribution`), Inseln mit `contenteditable="false"` und `data-raw`, Fließtext als `em`, `strong`,
`span.ws-footnote`, `q.ws-quote`, `span.ws-label`, `span.ws-soft`, `span.ws-nbsp`, `span.ws-island`. Jeder
Text erhält ein Escape. Die Marke hinter einer Überschrift steht als
`span.ws-label.ws-heading-label[data-heading-label]` (nicht bearbeitbar) am Ende des Überschriftblocks, die
Argumente einer Szene als `div.ws-args[data-raw]` (nicht bearbeitbar) vor ihrem Rumpf.

Runde 18b: Aufzählung `div.ws-block[data-kind=enum][data-start=N]` mit `<ol start="N">`, Begriffsliste
`div.ws-block[data-kind=terms]` mit `<dl><dt>Begriff</dt><dd>Erklärung</dd>…</dl>`, verschachtelte Einträge als Liste
im `<li>` des Eintrags davor (dasselbe Listenelement), Umbruch `<span class="ws-linebreak" contenteditable="false"
data-linebreak="1">↵<br></span>`, Link `<a class="ws-link" data-url="…" href="…">Text</a>` (Text bearbeitbar;
`href` nur mit `http://`, `https://`, `mailto:`; ohne Text `data-bare="1"` und nicht bearbeitbar), Verweis
`<button type="button" class="ws-ref" contenteditable="false" tabindex="-1" data-target="…" data-supplement="…">@ziel</button>`
(`type="button"`: sendet das Editor-Formular nicht ab). Der Titel einer Blockinsel lautet „Typst-Insel: Doppelklick
öffnet den Quelltext an Ort und Stelle“, der eines Bildes „Bild – Doppelklick öffnet den Quelltext an Ort und Stelle“.

**Bilder** (Schnittstelle zu „Bilder“, Runde 7: als Bild statt als Typst-Insel):
`baruch_wysiwyg::image_path(raw)` liefert das Zeichenketten-Literal im ersten Argument des ersten Aufrufs `image(`,
auch in `#figure(image("…"), …)`, außerhalb von Kommentaren, Rohtext und Formeln (berechnete Pfade nicht).
Runde 18b: `image_paths(raw)` liefert die Pfade **aller** Aufrufe `image(`; das Satzbild zeigt jedes Bild einer
Insel (vorher nur das erste), Beschriftung und Zeile mit dem Alternativtext wie beim ersten Bild. Runde 21b: Bei mehreren
Bildern nennt die Beschriftung ohne `caption:` jedes Bild (Alternativtext, sonst Pfad, getrennt durch " · "), und
darunter steht stets `<span class="ws-alt ws-paths">Bilder: Pfad · Pfad</span>` (vorher nur der erste Pfad).
`render_html_with(doc, &bilder)` oder `diff_html_with(alt, neu, &bilder)` zeigen jede Insel mit einem Bildpfad, für den
der Rückruf `Some(ImageRef { src, alt })` liefert, **als Bild** und ohne sichtbaren Quelltext:

- Blockinsel: `<div class="ws-block ws-figure" data-b="i" data-kind="island" data-image="1"
  contenteditable="false" data-raw="…" title="Bild – Quelltext nur im Quelltext bearbeitbar"><img
  class="ws-image" src="…" alt="…" loading="eager"><span class="ws-caption">…</span></div>`; die Beschriftung
  ist `caption: […]` oder `caption: "…"` wörtlich, sonst der Alternativtext, sonst der Pfad.
- Insel innerhalb eines Satzes: `<span class="ws-island ws-inline-image" contenteditable="false" title="Bild – …"
  data-raw="…"><img …></span>`.

Alle Attribute erhalten ein Escape. Die Insel bleibt eine Insel (`data-kind="island"`, nicht bearbeitbar, als
Ganzes löschbar); das Widget sendet sie als `keep`, beim Speichern bleibt sie Byte-identisch. Ohne Bild aus dem
Rückruf bleibt die Insel wie bisher (Quelltext in `code.ws-raw`). `render_html(doc)` ist
`render_html_with(doc, &|_| None)` und zeigt keine Bilder. Der Server (`wysiwyg::image_ref`) nimmt `src =
/werke/{work}/bilder/pfad?p=<Pfad, prozentkodiert>` mit dem Pfad in Normalform
(`baruch_store::normalize_asset_path`: `./bilder/a.png` und `/bilder/a.png` → `bilder/a.png`; seit Runde 18b ohne
führendes `/`, vorher `p=%2Fbilder%2F…`) und den
Alternativtext der aktuellen Fassung (`wysiwyg::image_alts`, ebenfalls nach Normalform). Das Bild wird
nur über diese vorhandene Route geliefert (`nosniff`, CSP `sandbox`); sie findet `/bilder/a.png`,
`bilder/a.png` und `./bilder/a.png` gleichermaßen, `..` nie. Diff (`diff_html`): Blöcke über die längste gemeinsame Teilfolge über `raw` (ohne
nachfolgenden Leerraum), gleiche in `div.ws-same`, entfernte in `div.ws-del`, neue in `div.ws-ins`.

## Menüband (Runde 18a, angedockt seit Runde 33) {#ribbon-round-18a-docked-since-round-33}

**Seit Runde 33 dockt die Werkzeugleiste in das Menüband der Seite an** (`docs/Architektur.md`, „Round 33 – Ribbon“):
Sie steht weiterhin im Fragment des Editors (`templates/fragments/edit_toolbar.html`, `div[data-edit-toolbar][hidden]`
in `div.wysiwyg`) und wird von `ui.render` mit neu gezeichnet; das Widget `band-dock` (`web/src/band.ts`, Marke am
Ende von `fragments/editor.html`) hängt sie nach jedem Einhängen in den Andockplatz `div[data-band-dock]` der Kopfzeile.
Sie hat zwei Teile: `data-band-part="start"` (Gruppen „Text“: „Satzbild“ | „Quelltext“; „Absatz“: Absatzformat, „K“, „F“;
„Fassung“: ‹ n/m ›, „Speichern“, „Als neue Fassung“; „Assistenten“: „Prüfen“, „Vorschlag“) für den Reiter „Start“ und
`data-band-part="einfuegen"` („Einfügen“: die Formate unten; „Bild“; „Register und Glossar“) für den Reiter
„Einfügen“. Ohne JavaScript bleibt sie am Blatt verborgen; es gibt dann wie bisher Textfeld und Statuszeile.

Folgen für die Widgets: Nach dem Andocken liegt die Werkzeugleiste nicht mehr unter `div.wysiwyg`. `wysiwyg.ts`, `ribbon.ts`
und `context_menu.ts` verbinden sich beim Einhängen (vor dem Andocken) mit ihrem Editor; was später in die Werkzeugleiste
greift (der Umschalter „Satzbild“/„Quelltext“ in `show`), nimmt `editToolbar(root)` aus `widgets.ts`. „Speichern“ und „Als
neue Fassung“ im Band gehören über `form="editor-form"` zum Formular im Blatt; maßgeblich (Beschriftung
„Gespeichert“/„Speichern“, ohne Änderung deaktiviert) bleibt der Knopf der Statuszeile (`submitButton` überspringt die
Knöpfe der Werkzeugleiste). Während des Speicherns sperrt `lockEditor` auch den Andockplatz. Im Quelltext
zeigt die Gruppe „Absatz“ statt der Werkzeuge den Hinweis „nur im Satzbild“ (`data-ws-source-note`).

Die Werkzeugleiste ist das **Menüband zum Bearbeiten** (`templates/fragments/edit_toolbar.html`,
Widget `ribbon` in `web/src/ribbon.ts`, Stil `static/ribbon.css` und `static/band.css`). Sie behält alle Haken, die
`wysiwyg.ts` liest (`data-edit-toolbar hidden`, `data-ws-mode`, `data-ws-tools`, `data-ws-format`,
`data-ws-cmd="italic|bold"`; `data-ws-hint` in der Statuszeile); im Teil „einfuegen“ steht
`div.ribbon-formats[data-widget=ribbon]`:

- je Format der Werkvorlage ein Knopf `button[data-ribbon-cmd=<WsCommandName>][data-ribbon-snippet]` aus
  `crate::ribbon::formats` (RPC `ribbon.formats`): `scene`/`work` nur, wenn die Vorlage `#let szene`/`#let arbeit` definiert,
  seit Runde 19b `scripture` („Bibelstelle“) nur mit `#let bibelstelle(…)` oder `#let bibel(…)` und
  `cite` („Literaturverweis“) nur mit `#bibliography(` in der Vorlage, dann immer `footnote`, `quote`, `list`,
  `enum`, `terms`, `break`, `link` (seit Runde 30a `xref`);
- `button[data-ribbon-image]` („Bild“): öffnet nur `details[data-image-insert]` (Formular aus Runde 18c).

**Formate der Vorlage für Verweise und Literatur (Runde 19b):**

| Vorlage enthält | Knopf | Quelltext (Schnipsel) | Satzbild (`baruch:ws-command`) |
|---|---|---|---|
| `#let bibelstelle(…)` oder `#let bibel(…)` | „Bibelstelle“ | `#bibelstelle[]` oder `#bibel[]`, Cursor in den Klammern | `scripture`, `arg` = `{"fn": "bibelstelle"}`; das Satzbild fragt nach der Stelle und setzt die Insel `span.ws-island[data-raw="#bibelstelle[Joh 3,16]"]` (nur als Ganzes änderbar) |
| `#bibliography(` (nicht in einer Kommentarzeile) | „Literaturverweis“ | `@`, Cursor dahinter | `cite`, `arg` = Schlüssel (fehlt er: das Satzbild fragt); setzt `button.ws-ref[data-target]` wie ein `@schlüssel` aus dem Quelltext |

Die reinen Bausteine stehen in `web/src/ws_command.ts` (importiert nichts): `scriptureFunction(snippet)` nimmt den
Namen aus `#name[]`, `scriptureSource(fn, stelle)` baut `#fn[stelle]` und setzt Escapes für Auszeichnungszeichen wie `needs_escape`
in `baruch-wysiwyg` (`\ # * _ $ @ < > [ ] ~` und der Backtick, Zeilenumbruch → Leerzeichen, höchstens 200 Zeichen), `citeKey(text)`
prüft den Schlüssel wie eine Typst-Marke (Buchstaben, Ziffern, `_ - : .`; ein `@` am Anfang und ein Punkt am Ende entfallen).
Das Speichern läuft wie immer über `wysiwyg.serialize`: die Insel wörtlich, der Verweis als `@schlüssel` (oder
`#ref(<schlüssel>)`, wenn ein Markenzeichen folgt). Annahme: Die Funktion der Vorlage nimmt den Verweis als
Inhalt (`[…]`) auf; eine Vorlage, deren Funktion eine Zeichenkette erwartet, braucht den Quelltext.

**Satzbild geöffnet** (`div.wysiwyg[data-ws-open]`): Der Knopf ruft `sendWsCommand(root, cmd)` aus
`web/src/ws_command.ts` auf (Ereignis `baruch:ws-command` an `div.wysiwyg`, ohne Bubbles). Setzt `wysiwyg.ts`
`handled = true`, wird der Befehl ausgeführt; bleibt `handled` auf `false`, zeigt `ribbon.ts` einen Hinweis
(`baruch:toast`, „… geht im Satzbild noch nicht – bitte im Quelltext einfügen“). **Quelltext:** `ribbon.ts` fügt
`snippet` selbst bei `selectionStart` in das Textfeld ein (`execCommand("insertText")`, sonst `setRangeText` + `input`);
Blockformate beginnen am Zeilenanfang, eine Auswahl wandert in den Inhalt (`[…]`), der Cursor steht im
Inhalt. Kein Import zwischen `ribbon.ts` und `wysiwyg.ts`.

Mit `satz_page` hat dasselbe Fragment noch den Zweig für die Satzseite (seit Runde 33 stehen ihre Knöpfe als
Gruppen im Reiter „Buch“; der Zweig entfällt nach dem Zusammenführen der Runde): das Menüband der Satzseite (`/werke/{id}/satz`, `div.ribbon-satz
[data-ribbon-satz]`, ohne `hidden`, ohne JavaScript): zurück, PDF, ‹ › mit `aria-label`, „Satz abbrechen“ als
Formular (`docs/Bedienung.md`, „Satz (Typst-Vorschau)“).

## Bedienung {#usage}

- **Ein Klick auf einen Abschnitt** öffnet ihn im Satzbild (Voreinstellung), auch nach jedem Neuzeichnen (Gliederung,
  `baruch:navigate`, `ui.render`). Der Umschalter „Satzbild | Quelltext“ über dem Text schaltet um; nur diese
  ausdrückliche Wahl setzt das Cookie `baruch_editor` (`satz` oder `quelltext`), und nur `quelltext` öffnet
  den Editor im Quelltext. Kommt man beim Laden der Seite über einen Treffer der Literatursuche
  (`#fundstelle`) an, beginnt dieser erste Editor im Quelltext, weil dort die Hervorhebung steht; spätere Klicks
  öffnen wieder das Satzbild.
- Im Quelltext füllt das Textfeld das Blatt (das Satzbild ist `hidden`); geöffnet füllt das Satzbild das Blatt.
  Werkzeugleiste und Statuszeile scrollen nicht mit.
- Tippen wie in einer Textverarbeitung: **Enter** beginnt einen neuen Absatz (in Listen einen neuen Eintrag),
  **Umschalt+Enter** einen einfachen Zeilenumbruch (in einer Überschrift einen neuen Absatz: eine Überschrift
  ist immer eine Zeile), **Strg+I** hebt hervor, **Strg+B** setzt fett (auch über
  die Knöpfe *K* und **F**), die Wahl „Absatz / Überschrift 1–3“ ändert das Format des Blocks am Cursor.
  Ein Absatz, der mit `# `, `## ` … beginnt, wird bei Enter eine Überschrift dieser Ebene (Markdown, siehe
  oben).
  Einfügen übernimmt nur Text; Leerzeilen im eingefügten Text trennen Absätze (jedes weitere Stück wird ein
  neuer Absatz, in Listen bleibt alles ein Eintrag).
- **Szenen und Arbeitsaufträge** stehen dort als Kasten; jeder Absatz darin lässt sich für sich bearbeiten (Enter
  teilt ihn, wie außerhalb). Gespeichert wird nur der bearbeitete Absatz neu, der Rest der Szene bleibt
  Zeichen für Zeichen. Argumente (`#szene(ort: …)`) und die Marke einer Überschrift stehen dort grau oder gelb
  und bleiben beim Bearbeiten erhalten.
- **Bilder** aus `#image(…)`/`#figure(image(…))` erscheinen als Bild mit Beschriftung (Bild von der Seite
  „Bilder“ des Werks, auch bei Pfaden mit `./`), nicht als roher Typst-Text; ein feiner Rand beim Überfahren zeigt,
  dass sie sich nur im Quelltext ändern oder als Ganzes löschen lassen.
- **Strg+S** speichert (wie im Quelltext). Vor dem Speichern übernimmt das Widget das Satzbild in das Textfeld;
  danach gelten dieselben Regeln wie im Quelltext (die Überschriftzeile bestimmt Titel und Ebene, Teilen und
  Zusammenführen nach einer Rückfrage). Ein leerer, mehrzeiliger oder mehr als 200 Zeichen langer Titel wird mit einer
  Meldung abgelehnt, nichts gespeichert. Ungespeicherter Text im Satzbild löst dieselbe Rückfrage aus wie im
  Quelltext.
- Gelber Hintergrund: **Typst-Inseln**; im Satzbild als Ganzes löschbar und seit Runde 18b per
  **Doppelklick** an Ort und Stelle als Quelltext bearbeitbar (Strg+Enter übernimmt, Escape bricht ab).
- **Runde 18b:** Aufzählungen (`1.`), Begriffslisten (Begriff fett, Erklärung eingerückt; Enter beginnt einen neuen
  Eintrag), verschachtelte Listen, erzwungene Umbrüche (↵), Links (Text bearbeitbar, Adresse im Titel) und Verweise
  (`@ziel` als Knopf; ein Klick zeigt die Marke im Abschnitt, Ändern nur im Quelltext) stehen im Satzbild
  statt als Inseln. Die Knöpfe des Menübands (Szene, Arbeitsauftrag, Fußnote, Zitat, Liste, Aufzählung,
  Begriffsliste, Umbruch, Link) wirken am Cursor, wenn das Satzbild geöffnet ist: Fußnote, Zitat und Link umschließen die
  Auswahl (sonst ein ausgewählter Platzhalter), Liste/Aufzählung/Begriffsliste machen den Absatz am Cursor zu einer
  solchen (Liste ↔ Aufzählung schaltet um), Szene und Arbeitsauftrag nehmen den Block am Cursor in ihren Rumpf (bei einer
  Überschrift oder Insel entsteht dahinter ein leerer). Geht ein Befehl an der Stelle nicht, sagt eine Meldung, warum.
- **Vorschläge** stehen an ihrer Stelle (Runde 8, siehe oben): bei geöffnetem Satzbild direkt am Block, den
  sie ändern, mit dem Unterschied im Satzbild (Entferntes rot durchgestrichen, Neues grün, vom Server fertig über
  `hunk_html_with`), im Quelltext und ohne JavaScript im Lesefluss unter dem Textfeld mit dem zeilenweisen
  Diff. Ein Klick auf die farbige Zeile klappt einen Vorschlag (alle seine Stücke) auf oder zu.
- **„Fassung ändern“** bleibt Quelltext; seit Runde 18b werden auch dort Markdown-Überschriften umgewandelt (eine
  weitere Überschrift weist „Fassung ändern“ wie eine getippte `=`-Zeile ab).
- **Ohne JavaScript** ist der Bereich des Satzbilds verborgen; es gibt nur das Textfeld, das wie bisher an
  `/gliederung/act` geht.

## Technik im Widget {#technology-in-the-widget}

- `wysiwyg.ts` hängt sich an `div.wysiwyg[data-widget=wysiwyg]` im Editor-Formular, übernimmt das fertige
  Satzbild des Servers (`data-ws-ready`, Runde 8) oder holt `wysiwyg.parse`, dazu `wysiwyg.style` (eingefügt
  als `<style id="ws-style">`), und zeigt das Satzbild (`contenteditable`). Danach setzt `placeProposals` die
  Stücke der Vorschläge aus `section.pv-flow` an ihre Stelle (`returnProposals` holt sie zurück).
- Jede Eingabe markiert alle berührten Blöcke (`data-dirty="1"`, auch verschachtelte im Rumpf einer Szene),
  setzt `form.dataset.dirty = "1"` und schaltet den Speicherknopf frei. Neue Blöcke (Enter) haben kein
  `data-b`; ein vom Browser kopierter Index zählt beim zweiten Mal als neu. Blöcke im Rumpf einer Szene
  oder eines Arbeitsauftrags nummeriert das Widget beim Öffnen (`data-n`); für eine bearbeitete Szene sendet es eine
  Rumpfanweisung `{"replaces": i, "body": […]}`.
- Vor dem Speichern löst `outline.ts` `baruch:before-save` am Formular aus; das Widget legt dann ein Promise in
  `detail.wait`, das `wysiwyg.serialize` aufruft, das Ergebnis ins Textfeld schreibt und `input` auslöst.
  Das Textfeld ist immer das, was abgesendet wird. Als Rückfall (solange niemand das Ereignis auslöst) fängt das Widget
  das Absenden im Fenster ab, übernimmt und sendet erneut ab.
- **Runde 22 (Fortschreiben):** Das Editor-Formular trägt das verborgene Feld `mode=auto`; „Speichern“ (erster
  Knopf des Formulars – `submitButton` nimmt ihn für „Gespeichert“/„Speichern“) schreibt die eigene aktuelle Fassung fort,
  solange keine Regel eine neue Fassung verlangt, „Als neue Fassung speichern“ sendet `as_new=1` (der Server
  oder `workspace.ts` macht daraus `mode: "new"`). Beim erneuten Absenden nach der Übernahme des Satzbilds geht der
  auslösende Knopf mit (`form.requestSubmit(submitter)` in `wysiwyg.ts` und `outline.ts`), damit `as_new` nicht
  verloren geht; Strg+S löst wie bisher ohne Knopf aus (also „Speichern“, Modus "auto"). Nach dem Speichern
  zeigt `workspace.ts` „Gespeichert: fortgeschrieben (Stand k).“ oder „Gespeichert: neue Fassung n (Grund).“; ohne
  JavaScript steht dieselbe Rückmeldung nach der Weiterleitung in der Kopfzeile des Editors (`saved=…` in der Adresse).
- `wysiwyg.ts` importiert nichts aus `outline.ts`.
- **Quelltext ohne Speichern (Runde 17, Vertrag §2.6):** Wer den vollständigen Quelltext ohne Speichern braucht
  (z. B. die Live-Zählung in `timing.ts`, die Prüfanzeige `scripture_check.ts`), löst am Editor-Formular
  `baruch:serialize-text` mit `detail: { wait: Promise<string>[] }` aus. Ist das Satzbild geöffnet, legt `wysiwyg.ts`
  ein Promise in `wait`: ohne Eingabe seit der letzten Übernahme das Textfeld, sonst `wysiwyg.serialize` mit
  denselben Anweisungen wie beim Speichern – aber ohne Markdown-Überschriften umzuwandeln, ohne das Textfeld zu beschreiben
  und ohne `touched` zu ändern. Ist niemand zuständig (Quelltext-Ansicht), bleibt `wait` leer; der
  Auslöser nimmt dann das Textfeld.
- **Hervorhebungen der Prüfanzeige (Runde 17c):** Das Widget `scripture-check` (Statuszeile, `web/src/scripture_check.ts`)
  hebt geprüfte Zitate im Satzbild **nur** über die CSS Custom Highlight API hervor (`CSS.highlights`, Namen
  `bibel-stimmt` und `bibel-weicht`, Stil in `static/pruefung.css`): Es legt `Range`-Objekte über vorhandene
  Textknoten (der Zitattext wird gesucht, Leerraum zählt als ein Zeichen; Vorschläge `.pv` zählen nicht) und ändert
  das DOM des Satzbilds nicht – `editsOf` sieht also keine Änderung, das Speichern bleibt Byte-identisch. Jede Eingabe
  löscht die Hervorhebung. Ohne die Highlight API gibt es nur die Liste.
- Überschriften (Runde 7): `inlinesOf(…, heading = true)` macht Zeilenenden im Text zu Umbrüchen,
  `splitHeading` teilt am ersten (wie `split_heading` im Server), `headingEdits` sendet Überschrift + neuen
  Absatz; `headingProblem` prüft die Titelregeln (Ebene 1–3) vor `wysiwyg.serialize` und bricht das Speichern
  mit der Meldung ab; `guardHeadingMerge` (bei `beforeinput`) fängt Rücktaste/Entf an der Grenze
  Überschrift ↔ Absatz ab. `ws-figure` und `ws-inline-image` sind Inseln wie alle anderen.
- **`guardHeadingMerge` über Vorschläge hinweg (Runde 18b):** Der Nachbar eines Blocks ist der nächste Block davor oder
  dahinter; Stücke von Vorschlägen (`details.pv`) dazwischen zählen nicht (`blockNeighbour`). Vorher war der Nachbar
  das `<details>`, und die Rücktaste am Anfang des Absatzes zog ihn doch in die Überschrift (Review R8). Nachbildung
  über RPC und Prüfung der Stelle im Widget: `round18_konstrukte.rs`
  (`a_proposal_between_heading_and_paragraph_does_not_disarm_the_merge_guard`); im Browser nur von Hand prüfbar.
- **Befehle des Menübands (Runde 18b, Vertrag R18-S1):** `baruch:ws-command` am Bereich `div.wysiwyg` (ohne
  Bubbles, `web/src/ws_command.ts`, eingefroren): Bei geöffnetem Satzbild setzt das Widget `detail.handled = true` und
  führt den Befehl aus (`runCommand`, alle `WsCommandName`: `scene`, `work`, `footnote`, `quote`, `list`, `enum`,
  `terms`, `break`, `link`; `arg` = Adresse beim Link oder Text des Platzhalters, ohne Adresse fragt ein Dialog);
  bei geschlossenem Satzbild bleibt `handled` auf `false`, und das Menüband fügt den Typst-Text selbst in das Textfeld ein.
  Das Widget merkt sich die Auswahl im Satzbild (`selectionchange`), damit ein Klick ins Menüband sie nicht verliert.
- **Cursorposition (Runde 18b):** `baruch:ws-caret` (am Bereich oder am Editor-Formular): Bei geöffnetem Satzbild legt das
  Widget genau ein Promise mit `{body, offset}` in `detail.wait` – `body` wie bei `baruch:serialize-text`, `offset` die
  UTF-16-Länge des Textes, den dieselben Anweisungen bis einschließlich des Blocks der obersten Ebene am Cursor ergeben (ein
  Zeilenanfang hinter seinen Leerzeilen; ohne Cursor oder hinter dem letzten Block das Ende). Zwei Aufrufe von
  `wysiwyg.serialize` (Text und Präfix); der Verlauf über RPC ist in `round18_konstrukte.rs` belegt.
- **Stil vom Server (Runde 18b):** Steht ein `<style id="ws-style" data-work="W">` mit dem Werk des Editors schon im
  Dokument (Satzbild je Werk, `fragments/editor.html`), übernimmt `applyStyle` es ohne `wysiwyg.style`; andere
  `#ws-style` (anderer Werke) werden verworfen. Sonst holt es den Stil wie bisher über RPC (je Werk eine Anfrage zur Zeit).

## Querverweise (Runde 30a) {#cross-references-round-30a}

Ein **Querverweis** zeigt über seinen festen Anker `<k{key}>` auf einen Gliederungspunkt
(`baruch_typst::xref::key_label`). Die Anker stehen nur im Export, im Spiegel und im Satz, nicht in den
Fassungen (siehe `docs/Architektur.md`, „Cross-references (Round 30a)“).

- **Form im Text** (`baruch_typst::xref::xref_snippet`): `#link(<k103>)[die Einleitung]`; mit Seitenzahl
  `#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]`. Kein `@k103`: Überschriften ohne
  Nummer lassen sich in Typst nicht mit `@` verweisen. In einem englischen Werk lautet der Zusatz `, p.~#context …`
  (`xref_snippet_in`, Runde 39b).
- **Blockmodell:** `parse` erkennt beide Formen als `Inline::Xref {key, c, page}` (JSON `{"t": "xref", "key": 103,
  "c": [...], "page": true}`). Erkannt werden nur eine kanonische Marke `k` + Ziffern ohne führende Null und genau dieser
  Seitenzahl-Zusatz mit demselben Schlüssel. Ein leerer Linktext, eine andere Marke oder ein anderer Zusatz bleiben
  Inseln (`#link(<hafen>)[…]` wie bisher). `emit` schreibt die Form von `xref_snippet`; `emit`/`parse` laufen im
  Rundlauf (Eigenschaftstest `tests/xref.rs`, Startwert `WS_SEED`, Fälle `WS_CASES`). Runde 39b: `parse` liest den
  Zusatz in jeder Werksprache (`, S.~` und `, p.~`); `emit_in`/`serialize_in` schreiben ihn in der Werksprache
  (der Server reicht sie bei `wysiwyg.serialize` weiter), `emit`/`serialize` bleiben der deutsche Fall.
- **Satzbild:** `render_html` setzt ein `span.ws-xref` mit `data-key` und `data-page`, darin der bearbeitbare
  Linktext, bei einer Seitenzahl ein nicht bearbeitbares `span.ws-xref-page` („, S. …“; `render_html_in` mit der Seiten-
  abkürzung der Werksprache, „, p. …“ in einem englischen Werk).
  - `wysiwyg.ts` liest den Block ohne den Zusatz zurück.
  - Der Befehl `xref` (`ws_command.ts`, `arg` = JSON `{key, text, page}`) fügt ihn an der Cursorposition ein.
  - Strg+Klick springt zum Ziel, ein Klick zeigt „Öffnen | Bearbeiten“ (`xref.ts`).
- **Anker im Export:** Eine `=`-Überschrift erhält ` <kN>` vor einem Kommentar. Eine Überschrift mit eigener
  Marke (Teil `<baruch-part>`, „ohne Nummer“ `<baruch-unnumbered>`, Marke des Menschen am Zeilenende oder allein in
  der Zeile darunter) behält ihre Marke; darunter steht die Ankerzeile `#metadata("baruch-key") <kN>` (hinter einer
  Markenzeile). Typst kennt nur eine Marke je Element.
- **Messung** (`crates/baruch-render/tests/xref.rs`): `#link(<kN>)` und `counter(page).at(<kN>)` lösen mit beiden
  Formen auf, und Typst prüft über `#context assert`, dass jeder Anker auf der Seite seiner Überschrift liegt.
  - **Ausnahme Teil im Satz:** Mit der Teilregel (`part_page_rule`, Umbruch nach dem Teil) läge die Ankerzeile
    hinter der Teilzeile auf der folgenden Seite (belegt).
  - Der Satz setzt sie deshalb vor die Teilzeile, als `#pagebreak(weak: true, to: "odd")#metadata("baruch-key")
    <kN>` (`part_anchors_for_typesetting`). Der Umbruch der Regel entfällt dann, die Seitenzahl bleibt gleich.
  - Ein Verweis auf einen fehlenden Anker ist in Typst ein Fehler mit der Marke in der Meldung. Deshalb macht der
    Satz kaputte Verweise zu einfachem Text, und die Druckprüfung nennt sie.
- **Einlesen:** Der Import entfernt Anker und Ankerzeile (`strip_section`) und schreibt die Verweise auf die
  neuen Schlüssel um (`remap_keys`).

## EPUB-Abbildung (Runde 20a) {#epub-mapping-round-20a}

Das EPUB (`crates/baruch-epub`, `xhtml.rs`) nimmt dasselbe Blockmodell wie das Satzbild: `baruch_wysiwyg::parse` je
Abschnittstext (aktuelle Fassung wie im Export, mit Anker `<kN>`), dann eine eigene XHTML-Abbildung – nicht
`render_html` (das schreibt Editor-HTML mit `contenteditable`, `data-raw` und Widget-Klassen) und nicht typst-html.

| Blockmodell | XHTML im EPUB |
|---|---|
| Überschrift des Abschnitts (`=`, Teil, „ohne Nummer“) | `<h1>`–`<h3 id="k{key}">`, davor `<span class="baruch-number">` mit der Nummer des Gliederungsschemas; Teil `<h1 class="baruch-part-title">` |
| weitere Überschrift im Text | `<h{level}>` (höchstens `<h6>`), mit einer Marke als `id="lbl-…"` |
| Absatz, `Soft` | `<p>`, ein Zeilenumbruch im Quelltext bleibt Leerraum |
| `_…_`, `*…*`, `\` | `<em>`, `<strong>`, `<br/>` |
| Liste `-`/`+`, Aufzählung `1.` | `<ul>`/`<ol>`, verschachtelt wie im Satzbild, `start` ab der ersten Nummer |
| Begriffsliste | `<dl><dt>…</dt><dd>…</dd></dl>` |
| Blockzitat, Zitat innerhalb eines Satzes | `<blockquote class="baruch-quote">` mit `<footer class="baruch-attribution">`, `<q>` |
| `#szene[…]`, `#arbeit[…]` | `<section class="baruch-scene">`, `<section class="baruch-work">`; Argumente (`ort: […]`) entfallen, Warnung `ARGS_OMITTED` |
| `#link("…")[…]` | `<a href>` nur für `http`, `https`, `mailto`, sonst nur der Text |
| Querverweis `#link(<kN>)[…]` | `<a class="baruch-xref" href="k{Kapitel}.xhtml#kN">`; unbekannter Schlüssel → Text, Warnung `XREF_UNKNOWN` |
| Verweis `@name`, `@kN` | Link auf die Marke oder die Überschrift (Text = Überschrift), sonst Text |
| Marke `<name>` | `<span id="lbl-name"></span>` (die Überschriften tragen die Anker `<kN>`) |
| `#footnote[…]` | Popup: `<sup><a epub:type="noteref" href="#fnN">` + `<aside epub:type="footnote" id="fnN">` in `ol.baruch-footnotes` am Dateiende; Endnoten: Verweis auf `notes.xhtml#fnN`, dort mit Rückverweis |
| Insel mit `image("…")` (Block) | `<figure class="baruch-figure"><img alt="…"/><figcaption>` aus `caption:` |
| Insel mit `image("…")` (innerhalb eines Satzes) | `<img class="baruch-inline-image" alt="…"/>` |
| `` `Code` `` | `<code>` |
| Escape `\x` als Insel | das Zeichen |
| Kommentar, Ankerzeile `#metadata("baruch-key") <kN>` | nichts, ohne Warnung (Ankerzeile → `id`, falls noch frei) |
| jede andere Insel (`#set`, `#let`, Formeln, eigene Funktionen) | nichts, Warnung `ISLAND_OMITTED` mit Schlüssel und Auszug |

Alternativtext eines Bildes: `alt: "…"` im Typst-Text, sonst der Alternativtext der Bild-Metadaten; leer → `alt=""` und
Warnung `IMAGE_NO_ALT`. Ein Pfad ohne Bild im Werk erscheint als „Bild fehlt: <Pfad>“ (Warnung `IMAGE_MISSING`).
Aller Text erhält ein Escape (`escape_text`); nichts aus dem Werk wird als HTML übernommen.

## Registermarken und Kontextmenü (Runde 31c) {#index-marks-and-context-menu-round-31c}

Eine **Registermarke** steht im Typst-Text direkt hinter dem markierten Wort, kanonisch
`#metadata((register: "stichwort", term: "Gnade"[, sub: "…"][, see: "…"][, also: "…"]))<baruch-idx>`
(`baruch_typst::index::mark_source`, gelesen von `parse_mark`). Im Blockmodell ist sie `Inline::IndexMark { mark }`, in
JSON `{"t": "index_mark", "mark": {"register": "…", "term": "…", …}}` (leere Zusätze fehlen, unbekannte Felder weist der
Server ab). Eine Marke mit einem Leerzeichen vor der Marke oder einer anderen Schlüsselreihenfolge ist keine Marke, sondern bleibt eine Insel.

Im Satzbild (`render_html`) erscheint sie als leeres Element, in das man nicht tippen kann

```html
<span class="ws-index-mark" contenteditable="false" role="img" tabindex="-1"
      title="Registereintrag im Stichwortregister: Gnade › im Alltag; siehe auch: Huld"
      aria-label="…" data-register="stichwort" data-term="Gnade" data-sub="im Alltag" data-also="Huld"
      data-raw="#metadata((…))&lt;baruch-idx&gt;"></span>
```

- **Dezent:** Das kleine Zeichen (⁘, im Personenregister blau) kommt nur aus dem Stil (`::before` in `wysiwyg.css`); das
  Element hat keinen Textknoten, darum bleiben Text, Suche, Hervorhebung und Zitatprüfung des Absatzes unberührt.
- **Beschreibung:** `title`/`aria-label` = `baruch_wysiwyg::index_mark_label` (`Registereintrag im {Register}: {Stichwort}
  › {Unterstichwort}; siehe: …; siehe auch: …`; eigene Register als `Register „slug“`). `markLabel` in `ws_command.ts`
  folgt derselben Regel (Vergleichstest mit Node).
- **Zurück:** `wysiwyg.ts` liest `span.ws-index-mark` aus den `data-*`-Feldern als `index_mark` (vor der Prüfung auf
  Inseln); der Server schreibt sie mit `emit` kanonisch. Ein unberührter Block bleibt `keep` und Byte-identisch; ein Block,
  in dem eine Marke gesetzt, geändert oder entfernt wurde, wird neu erzeugt – bei kanonischem Text ändert sich nur die
  Marke (Test `baruch-wysiwyg/tests/index_mark.rs`).
- **Setzen, Ändern, Entfernen** nur über den Befehl `index_mark` (`baruch:ws-command`, `arg` = JSON
  `{op: "add" | "edit" | "remove", mark?}`), den das Kontextmenü sendet: `add` setzt die Marke hinter das Ende der
  Auswahl (nachfolgender Leerraum zählt nicht) oder hinter das Wort am Cursor; nicht in einer Überschrift, einem Teil, „ohne Nummer“,
  einer Insel, einem Verweis, einer Marke oder einer anderen Registermarke (Meldung). `edit`/`remove` wirken auf die Marke, die das Kontextmenü
  vorher mit `data-ctx-target` bezeichnet hat. Danach ist der Editor ungespeichert; Speichern mit „Speichern“ erzeugt eine
  neue Fassung des Menschen.
- **Doppelklick** auf eine Marke öffnet „Registereintrag bearbeiten“.

**Kontextmenü** (`web/src/context_menu.ts`, Widget `context-menu` an der Menübandgruppe „Register und Glossar“):

- Öffnen im Satzbild und im Quelltext-Textfeld mit Rechtsklick, der Kontextmenütaste oder Umschalt+F10 (`opensMenu`) oder
  über den Link „Markierung …“ der Statuszeile. Sendet der Browser nach der Taste zusätzlich `contextmenu`, öffnet es sich nicht
  doppelt.
- ARIA-Menümuster: `div.ctx-menu[role=menu]` mit `button[role=menuitem]` (wandernder `tabindex`), Pfeil ab/auf springt am Rand um,
  Pos1/Ende, Enter/Leertaste lösen aus, Escape und Tab schließen; der Fokus (und im Textfeld die Auswahl) geht zurück
  (`menuKey`).
- Einträge: „In Glossar aufnehmen …“ (Dialog mit Begriff = markiertes Wort, kurzer/langer Erklärung, Varianten und
  Bibelstellen je Zeile, Quelle → RPC `glossary.add`, Meldungen des Servers im Dialog), „In Register aufnehmen …“
  (Stichwort, Unterstichwort, Register aus `index.registers` – ohne Antwort die eingebauten –, siehe, siehe auch → Marke
  wie oben oder im Quelltext `markSource` hinter dem Wort, `markInSource` mit denselben Verboten wie oben),
  „Im Glossar nachschlagen“ (RPC `glossary.lookup`, Popup mit Begriffen, Varianten, Erklärungen und einem Link auf
  `/werke/{id}/glossar#begriff-{id}`); auf einer Marke (Rechtsklick darauf, Cursor direkt daneben, im Textfeld Cursor
  darin) zusätzlich „Registereintrag bearbeiten …“ und „… entfernen“.
- Menüband: dieselben Aktionen als Knöpfe (`data-ctx-action`) und „Register“ (Knopf `[data-index-toggle]` mit `aria-expanded`, Escape schließt), der beim
  Aufklappen alle Registereinträge des Abschnitts auflistet – aus den Marken des Satzbilds oder des Textfelds (`findMarks`),
  also auch ungespeicherte; ein Klick springt zur Marke.
- Ohne JavaScript ist das Menüband verborgen; der Link „Markierung …“ führt zu `/werke/{id}/markierung?key=K&rev=R` mit
  denselben drei Aktionen als Formulare (`docs/Bedienung.md`).

## Zitatblock (Runde 23) {#quotation-block-round-23}

`baruch_wysiwyg::parse` liest eine logische Zeile, die genau ein `#quote(block: true, …)[…]` ist, als eigenes Stück
(wie `#szene[…]`/`#arbeit[…]`): Eine Zeile direkt dahinter ist ein eigener Absatz, eine Zeile direkt davor beendet
ihren Absatz – so, wie Typst es setzt. Bis Runde 22 wurde ein Zitatblock mit dem Nachweis in der folgenden Zeile zu einer
Codeinsel. `#quote[…]`, `block: false` und nicht lesbare Argumente bleiben Teil ihres Absatzes. Unberührte Blöcke gehen
Byte-identisch zurück (`serialize`).

Der Knopf „Zitat“ (`ribbon::QUOTE_SNIPPET`) fügt `#quote(block: true)[]` ein – seit Runde 36 ohne die folgende Zeile
„– “ für den Nachweis (unausgefüllt gelassen, blieb ein einzelner Gedankenstrich im Buch; den Nachweis schreibt der Autor
selbst in die Zeile unter dem Block) –, mit genau einer Leerzeile davor und danach (vorhandene zählen), Cursor im Rumpf; bei einer Auswahl wandert die
Auswahl in den Rumpf. Nicht `attribution: []`: Eine leere `attribution` setzt in Typst 0.15.1 eine Zeile mit dem
Geviertstrich (gemessen in `crates/baruch-render/tests/quote_attribution.rs`). Im Satzbild entsteht ohne Auswahl
dasselbe als eigener Block hinter dem Absatz am Cursor; mit einer Auswahl bleibt es ein Zitat im Fließtext. Ohne JavaScript
gibt es kein Menüband; ein von Hand geschriebener Block wird genauso gelesen. Tests: `baruch-wysiwyg/tests/zitat.rs`,
`round23_web::widgets::the_quote_button_inserts_the_block…`, Abnahmetest `round23_abnahme.rs`.
