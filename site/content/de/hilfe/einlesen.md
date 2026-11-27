---
title: "Vorhandene Texte einlesen"
description: "Einen vorhandenen Text als Werk übernehmen: aus Typst, Markdown, einem Ordner oder ZIP, aus PDF, EPUB oder Word."
weight: 20
---

Wer schon einen Text hat, muss ihn nicht abtippen. Baruch liest fünf Arten von Dateien und legt daraus ein
**neues Werk** an. Bei PDF, EPUB und Word zeigt Baruch zuerst eine Vorschau der Gliederung; angelegt wird erst
nach der Bestätigung. Alles geht auch ohne JavaScript. Assistenten können nichts einlesen – ein eingelesener Text
gilt als Text des Menschen, und den legt nur der Mensch an.

| Datei | Weg im Menü Baruch ▾ |
|---|---|
| Typst (`.typ`), Markdown (`.md`, `.markdown`), ZIP, Ordner | Neues Werk / Importieren… |
| PDF, EPUB | PDF oder EPUB importieren… |
| Word (`.docx`) | Word (.docx) importieren… |
| Austauschdatei eines anderen Baruch | Austausch: Einlesen… – siehe [Austausch](/hilfe/austausch-sicherung/) |

## Typst, Markdown, ZIP oder Ordner einlesen {#typst-markdown}

{{< hilfebild name="einlesen-bericht" alt="Der Bericht nach dem Einlesen einer Markdown-Datei: Name des neuen Werks, Zahl der Abschnitte und Bilder, darunter die Hinweise mit Zeilennummer und der Verweis zum Werk." titel="Der Bericht nennt, was beim Einlesen angepasst oder ausgelassen wurde." >}}

Typst ist das Satzformat, in dem Baruch die Texte ablegt: gewöhnlicher Text mit wenigen Zeichen für Überschriften
und Hervorhebungen. Markdown ist ein ähnliches, weit verbreitetes Format; Baruch wandelt es um.

So geht es:

1. **Baruch ▾ → Neues Werk / Importieren…** wählen.
2. Unter **Typst- oder Markdown-Datei importieren** eine *Datei* wählen (`.typ`, `.md`, `.markdown` bis 16 MiB,
   `.zip` bis 80 MiB) **oder** einen *Ordner*.
3. Auf Wunsch einen Titel eintragen. Mit JavaScript füllt sich das Feld aus der Datei vor; bleibt es leer, nimmt
   Baruch den Titel aus der Datei, sonst den Dateinamen.
4. **Importieren** drücken.

Gibt es nichts anzumerken, öffnet sich das neue Werk. Sonst zeigt ein **Bericht**, welche Bilder übersprungen
wurden (mit Grund) und was sich nicht genau übernehmen ließ (mit Zeile), und verlinkt das Werk. Nichts wird still
verworfen.

Was aus der Datei wird:

- Alles **vor der ersten Überschrift** wird die Werkvorlage (Seite, Schriften, eigene Formate).
- Überschriften der ersten Ebene werden **Kapitel**, der zweiten **Abschnitte**, der dritten **Unterabschnitte**.
  Der Text von einer Überschrift bis vor die nächste wird Fassung 1 – vom Menschen, sofort aktuell.
- Titel und Autor kommen aus den Angaben der Datei, sonst gilt der Dateiname.
- Tiefere Überschriften (vierte Ebene und tiefer) und eingerückte Überschriften bleiben in einer Typst-Datei
  gewöhnlicher Text.
- Dieselbe Datei ein zweites Mal wird abgelehnt (auch umbenannt). Ist der Titel schon vergeben, erscheint das
  Formular wieder: anderen Titel eintragen und die Datei noch einmal wählen.
- Die Datei muss Text in der Kodierung UTF-8 sein.

**Bilder:** Eine einzelne Datei bringt nur den Text mit. Sollen die Bilder mitkommen, den ganzen **Ordner** oder
ein **ZIP** hochladen: Darin liegt genau eine Hauptdatei (bei mehreren gilt `index.md`, `main.typ` oder
`README.md` auf oberster Ebene) und daneben die Bilder, so wie der Text sie nennt. Fehlende Bilder, Bilder über
8 MiB und Dateien, die keine Bilder sind, stehen mit Grund im Bericht; Dateien, die der Text nicht nennt, als
Hinweis. Ein ZIP darf höchstens 2000 Dateien enthalten, jede höchstens 16 MiB. Ein unsicheres Archiv (etwa mit
Verknüpfungen oder Pfaden, die aus dem Archiv hinausführen) lehnt Baruch ganz ab; angelegt wird dann nichts.
Bilder lassen sich auch später auf der Seite „Bilder“ hinzufügen, siehe
[Schreiben, Bilder und Fassungen](/hilfe/schreiben/#bilder).

### Was aus Markdown wird {#markdown}

| Markdown | wird im Werk |
|---|---|
| `#`, `##`, `###` | Kapitel, Abschnitt, Unterabschnitt; tiefere Ebenen werden Unterabschnitte (Hinweis) |
| Text vor der ersten Überschrift | ein eigenes erstes Kapitel mit dem Titel des Werks (Hinweis) |
| Kopf mit `title:` und `author:` | Titel und Autor des Werks; andere Angaben: Hinweis |
| `*kursiv*`, `**fett**`, `~~durch~~` | hervorgehoben, fett, durchgestrichen |
| Listen und Aufzählungen, auch verschachtelt | Listen und Aufzählungen |
| `> Zitat` | Zitatblock |
| Code | Code |
| `[Text](https://…)` | Verweis; Verweise auf andere Dateien oder Sprungmarken: nur der Text (Hinweis) |
| Fußnote `[^1]` | Fußnote an der Stelle des Verweises |
| Bild `![Alt](bilder/a.png "Titel")` | Bild mit Alternativtext, mit Titel als Abbildung mit Unterschrift; Bilder aus dem Netz werden ein Verweis (Hinweis) |
| Tabelle | Tabelle mit Kopf und Ausrichtung |
| Begriffsliste | Begriffsliste |
| HTML | ein Zeilenumbruch bleibt; andere Auszeichnung fällt weg, der Text bleibt (Hinweis) |

Zeichen, die im Satzformat etwas bedeuten (zum Beispiel `#`, `*`, `_`, `$`, `@`), bleiben wörtlich Text.

### Teile und Kapitel ohne Nummer in Typst-Dateien {#teile}

Zwei besondere Zeilen versteht Baruch in einer Typst-Datei genau in dieser Form:

```typst
#heading(level: 1, numbering: none)[Erster Teil] <baruch-part>
#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>
```

Die erste wird ein **Teil** (die folgenden Kapitel stehen darunter und zählen über Teile hinweg weiter), die
zweite ein Kapitel **ohne Nummer** (mit `level: 2` oder `3` ein Abschnitt bzw. Unterabschnitt ohne Nummer). Beim
Export schreibt Baruch dieselben Zeilen wieder heraus.

### Ein exportiertes Werk wieder einlesen {#export-import}

Was Baruch beim Export aus den Einstellungen in die Datei schreibt, kommt beim Einlesen an seinen Platz zurück;
jeder Eingriff steht im Bericht:

- Die **Satz-Einstellungen** werden wieder Satz-Einstellungen des neuen Werks. Wurden sie in der Datei von Hand
  geändert, bleiben sie Teil der Werkvorlage.
- **Überschriften ohne Nummer** behalten ihr „Nummer aus“.
- Die erzeugten Seiten für **Register und Glossar** am Buchende und die automatischen Zusätze (Marken an
  Bibelstellen, Auszeichnung der ersten Erwähnung eines Glossarbegriffs) fallen weg – Baruch erzeugt sie neu.
- Register, Glossar und die Einstellung „Querverweis mit Seite“ stehen nicht in der Datei und kommen nicht mit.
  Dafür ist die [Austauschdatei](/hilfe/austausch-sicherung/) da.

### Von der Kommandozeile {#kommandozeile}

```sh
baruch-server import buch.typ                 # auch heft.md oder buch.zip
baruch-server import buch.typ --title "Anderer Titel"
```

Von der Kommandozeile liest Baruch die Bilder aus dem Ordner der Datei mit. Die Ausgabe nennt Werk, Abschnitte,
Bilder, übersprungene Bilder und jeden Hinweis.

## PDF oder EPUB einlesen {#pdf-epub}

{{< hilfebild name="einlesen-pdf-epub" alt="Das Formular „PDF oder EPUB importieren“: Dateiauswahl, die Wahl des Wegs (automatisch, lokal, über MultiGPT), ein Feld für den Titel und der Knopf „Lesen und Gliederung zeigen“." titel="Schritt 1: Datei wählen und lesen lassen." >}}

Aus einem fertigen Buch – PDF oder EPUB, höchstens 64 MiB – schlägt Baruch eine Gliederung vor. Daraus wird ein
Werk, eine Quelle für die Literatur oder beides.

So geht es:

1. **Baruch ▾ → PDF oder EPUB importieren…** wählen (der Verweis steht auch unter dem Formular „Neues Werk“).
2. Datei wählen, auf Wunsch einen Titel eintragen, **Lesen und Gliederung zeigen** drücken.
3. Die **Vorschau** prüfen und die Gliederung bei Bedarf ändern (siehe unten).
4. **Übernehmen als** Werk, Quelle der Bibliothek oder beides wählen; Titel und Autor sind aus der Datei
   vorbelegt. **Übernehmen** führt zum neuen Werk bzw. zur Quelle. **Verwerfen** löscht den Entwurf; angelegt
   wird dann nichts.

{{< hilfebild name="einlesen-pdf-vorschau" alt="Die Vorschau nach dem Lesen einer PDF: oben der gewählte Weg und die Hinweise, darunter je Gliederungspunkt ein Textfeld mit dem Titel, die Zahl der Zeichen und eine Textprobe; unten die Auswahl „Übernehmen als“." titel="Schritt 2: Gliederung prüfen und übernehmen." >}}

**Die Gliederung in der Vorschau ändern:** Jeder Punkt ist eine Zeile.

- `= Titel` ist ein Kapitel, `== Titel` ein Abschnitt, `=== Titel` ein Unterabschnitt. Den Titel ändert man im
  Feld, die Ebene durch mehr oder weniger `=`.
- Eine geleerte Zeile streicht den Punkt; sein Text geht an den vorigen Punkt. Nichts geht verloren.
- `= Teil: Titel` macht einen Teil – nur so, auch wenn Baruch „möglicher Teil“ vermutet.
- Die Reihenfolge bleibt wie in der Datei. Text vor dem ersten Punkt steht im Punkt „Vorspann“.

**Welcher Weg?**

- **EPUB** liest Baruch immer selbst. Übernommen werden Absätze, Hervorhebungen, Fußnoten, Listen, Zitate, Code
  und Bilder; die Gliederung kommt aus dem Inhaltsverzeichnis des EPUB.
- **PDF lokal:** Baruch liest die Datei auf diesem Rechner mit dem freien Programm Poppler; nichts verlässt den
  Rechner. Fehlt Poppler, sagt Baruch das. Übernommen werden Text und Überschriften (erkannt an der Schriftgröße
  oder an Nummern wie „1.2 Titel“ – die Nummer fällt weg, Baruch zählt selbst); Seitenzahlen fallen weg,
  Hervorhebungen, Fußnoten und Bilder kommen nicht mit, Tabellen nur als Textzeilen. Gelesen werden höchstens die
  ersten 2000 Seiten.
- **PDF über MultiGPT** (ein eigener Dokumentendienst, falls unter Einstellungen → Verbindungen eingerichtet):
  Die Datei wird dorthin hochgeladen – nur, wenn das Kästchen „in MultiGPT hochladen“ ausdrücklich angekreuzt
  ist. Die Gliederung kommt dann aus dem Inhaltsverzeichnis, das MultiGPT erkennt.
- **automatisch:** MultiGPT, wenn es eingerichtet, eine Sammlung eingetragen und das Hochladen angekreuzt ist,
  sonst lokal. Welcher Weg gilt und warum, steht oben in der Vorschau.
- Scheitert MultiGPT, steht der Grund da und ein Knopf **Lokal lesen**; die Datei muss nicht neu gewählt werden.
  Baruch wechselt den Weg nie von selbst.

**Als Quelle der Bibliothek:** Titel und Autor kommen aus der Datei. Lokal gelesen merkt sich Baruch den ganzen
Text; so lässt sich die Quelle später lesen, und Zitate lassen sich gegen sie prüfen (siehe
[Bibel, Literatur und Quellen](/hilfe/bibel-quellen/#quellen)). Dieselbe Datei ein zweites Mal als Quelle wird
abgelehnt.

Ist der Titel schon vergeben, bleibt die Vorschau mit einer Meldung stehen. Entwürfe bewahrt Baruch höchstens
24 Stunden auf, höchstens 20 zugleich.

Von der Kommandozeile (immer lokal):

```sh
baruch-server import buch.pdf                             # nur Gliederung und Hinweise zeigen
baruch-server import buch.pdf --uebernehmen               # Werk mit dieser Gliederung anlegen
baruch-server import buch.epub --bibliothek               # nur als Quelle in die Bibliothek
baruch-server import buch.epub --uebernehmen --bibliothek # beides
```

## Word-Datei einlesen {#word}

{{< hilfebild name="einlesen-word" alt="Das Formular „Word (.docx) importieren“ mit Dateiauswahl, einem Feld für den Titel und dem Knopf „Lesen und Vorschau zeigen“." titel="Schritt 1: Word-Datei wählen." >}}

**Baruch ▾ → Word (.docx) importieren…** liest eine Word-Datei (`.docx`, höchstens 64 MiB) und legt daraus ein
neues Werk an. Das Besondere: Die **Änderungsverfolgung** („Überarbeiten“) und die **Kommentare** aus Word gehen
nicht verloren – aus ihnen werden Vorschläge und Beiträge.

So geht es:

1. Datei wählen, auf Wunsch einen Titel eintragen, **Lesen und Vorschau zeigen** drücken. Dabei wird nur ein
   Entwurf angelegt; die Datei bleibt auf diesem Rechner.
2. Die Vorschau prüfen: die **Gliederung** (Teile und Überschriften der Ebenen 1 bis 3, je mit Länge und
   Textprobe), die **Bearbeiter** aus der Änderungsverfolgung (Name in Word, Name in Baruch, Zahl der Einfügungen
   und Löschungen), die Zahl der Kommentare, Fußnoten und Bilder und alle **Hinweise** auf das, was nicht oder nur
   vereinfacht übernommen wird.
3. **Als Werk übernehmen** drücken. Der Titel lässt sich noch ändern; zwei Häkchen schalten das Übernehmen der
   Änderungen bzw. der Kommentare ab. **Verwerfen** löscht den Entwurf. Entwürfe verfallen nach 24 Stunden.

{{< hilfebild name="einlesen-word-vorschau" alt="Die Vorschau einer Word-Datei: die Gliederung mit Textproben, die Tabelle der Bearbeiter mit Einfügungen und Löschungen, die Zahl der Kommentare, Fußnoten und Bilder und die Liste der Hinweise." titel="Schritt 2: Gliederung, Bearbeiter und Hinweise prüfen." >}}

**Änderungen und Kommentare werden nie von selbst zum gültigen Text:**

- **Fassung 1** jedes Abschnitts ist der Stand **vor** allen verfolgten Änderungen. Sie ist die aktuelle Fassung.
- Die Änderungen **jedes Bearbeiters** werden je geändertem Abschnitt **ein offener Vorschlag** mit der Notiz
  „Änderungen aus Word von …“. Haben zwei Bearbeiter denselben Abschnitt geändert, gibt es zwei Vorschläge.
  Annehmen, ablehnen oder mischen kann nur der Mensch – wie bei jedem Vorschlag.
- **Überschriften** ändern nie die Gliederung: Eine in Word geänderte Überschrift wird ein Beitrag am Abschnitt.
  Eine vom Bearbeiter eingefügte Überschrift legt keinen Abschnitt an (ihr Text steht fett im Vorschlag), eine
  gelöschte bleibt bestehen; beides steht als Hinweis da.
- **Kommentare** werden Beiträge am Abschnitt der kommentierten Stelle: Name und Datum, die Stelle als Zitat
  (höchstens 200 Zeichen), dann der Kommentar; Antworten stehen direkt dahinter. Ein Kommentar ohne auffindbare
  Stelle steht am ersten Abschnitt.

Der Bearbeiter heißt in Baruch nach seinem Namen in Word, klein und ohne Umlaute, mit „@word:“ davor – aus
„Jörg Müller“ wird „@word:joerg-mueller“.

**Was übernommen wird:** Überschriften und Titel, fett, kursiv, unterstrichen, durchgestrichen, hoch- und
tiefgestellt, Kapitälchen, Verweise, Fußnoten, Blockzitate, Listen (auch verschachtelt), Tabellen, eingebettete
Bilder mit Alternativtext und Bildunterschrift, Seitenumbrüche. Text vor der ersten Überschrift bekommt ein
eigenes erstes Kapitel; ein Dokument ganz ohne Überschrift wird ein Kapitel.

**Was nicht oder vereinfacht ankommt** (jeweils mit Hinweis): Überschriften der Ebenen 4 bis 6 (stehen fett im
Text), Textfelder, Zeichnungen und eingebettete Objekte, Formeln (nur als einfacher Text), Tabellen mit
verbundenen Zellen, Bilder in nicht unterstützten Formaten, nur verknüpfte Bilder (Baruch ruft sie nie ab),
verborgener Text. Endnoten werden Fußnoten. Verschobener Text steht als Löschung an der alten und Einfügung an der
neuen Stelle.

**Nicht gelesen** werden das alte Format `.doc` und Dateien mit Kennwort.

**Nicht geprüft:** Bisher wurde keine von Microsoft Word oder LibreOffice geschriebene Datei eingelesen; geprüft
sind selbst gebaute Dateien nach dem Standard und Dateien, die Baruch selbst geschrieben hat.

Von der Kommandozeile:

```sh
baruch-server import manuskript.docx                # nur die Vorschau, nichts wird angelegt
baruch-server import manuskript.docx --uebernehmen  # Werk samt Vorschlägen und Beiträgen anlegen
```

Den umgekehrten Weg – das Werk als Word-Datei für den Verlag – beschreibt
[Buchsatz](/hilfe/buchsatz/#word-export).
