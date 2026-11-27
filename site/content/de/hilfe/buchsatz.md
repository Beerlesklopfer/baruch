---
title: "Buchsatz: PDF, E-Book und Umschlag"
description: "Das Werk setzen und ausgeben: Satz-Vorschau, PDF, Umschlag und Titelei, E-Book, Stichwortzettel, Word-Datei für den Verlag."
weight: 100
---

Aus dem Manuskript wird ein Buch: Baruch setzt das Werk mit dem eingebauten Satzprogramm **Typst** – Seiten,
Ränder, Schrift, Seitenzahlen – und gibt es als PDF, als E-Book oder als Word-Datei aus. Alles steht im Reiter
**Buch** und geht ohne JavaScript. Gesetzt werden immer die **aktuellen Fassungen**.

## Satz-Vorschau {#satz}

{{< hilfebild name="satz-vorschau" alt="Die Satz-Vorschau des Beispielwerks: eine gesetzte Buchseite groß in der Mitte, darüber die Seiten des Kapitels als Verweise; im Menüband die Gruppen Zurück, Herunterladen, Seiten mit „Seite n von N“ und Satzlauf." titel="Die Satz-Vorschau zeigt das Werk Seite für Seite, wie es gedruckt aussähe." >}}

So geht es:

1. **Buch → Satz-Vorschau** wählen. Gezeigt wird die Seite, auf der der geöffnete Abschnitt beginnt.
2. Mit **‹** und **›** im Menüband blättern („Seite n von N“) oder oben eine Seite des Kapitels anklicken.
3. **PDF herunterladen** lädt das ganze Buch.

- **Meldungen** des Satzprogramms stehen über der Seite: „Fehler“ (rot) oder „Warnung“ (gelb), mit dem Abschnitt
  als Verweis zurück in den Editor. Bei einem Fehler gibt es keine Seiten, nur die Meldungen.
- **Schriften:** Eingebaut sind Libertinus Serif, New Computer Modern und DejaVu Sans Mono, dazu alle auf dem
  Rechner installierten. Fehlt eine Schrift der Vorlage, setzt Baruch mit einer vorhandenen weiter und warnt.
- **Erweiterungspakete aus dem Internet** lädt Baruch nicht; eine Vorlage, die eines verlangt, ergibt einen
  Fehler.
- **Dauer:** Ein Satz dauert bei großen Büchern Sekunden. Nach 60 Sekunden meldet die Seite, dass es länger
  dauert. **Satz abbrechen** (Gruppe „Satzlauf“) bricht den Satz dieses Werks ab; **Neu setzen** startet ihn
  wieder.
- **Teile** stehen allein auf einer eigenen rechten Seite, mit ihrer Nummer über dem Titel.
- **Druckprüfung** (unten auf der Seite): fehlende Bilder, Bilder mit weniger als 300 dpi in ihrer gedruckten
  Größe (mit der nötigen Pixelzahl), kaputte Querverweise und Stellen, an denen die Werkvorlage einer
  Satz-Einstellung widerspricht. Ist alles in Ordnung, sagt sie das.

Diese Seite lädt kein JavaScript. Darum führt hier das Zeichen für das Thema zu Einstellungen → Darstellung, und
Tastenkürzel gibt es auf ihr nicht.

## Satz-Einstellungen {#satz-einstellungen}

{{< hilfebild name="satz-einstellungen" alt="Einstellungen, Seite Werk, Block Satz: die Gruppen Kapitel und Teile, Seite und Ränder, Schrift und Absatz, Seitenzahlen, Kolumnentitel und Fußnoten, Querverweise, Register und Glossar, Entwurf; unten die Knöpfe Vorschau, Speichern und Zurücksetzen." titel="Die Satz-Einstellungen eines Werks – mit Vorschau an einem Mustertext." >}}

Wie das Buch aussieht, bestimmt zuerst die Werkvorlage. Was man häufig ändern will, lässt sich ohne Kenntnis des
Satzformats einstellen: **Einstellungen → Werk → Satz**. Jedes Feld beginnt mit **„aus der Werkvorlage“** bzw.
ist leer – dann gilt, was die Vorlage vorgibt.

| Gruppe | Felder |
|---|---|
| Kapitel und Teile | Kapitelanfang (fortlaufend, neue Seite, neue rechte Seite); Teil-Seite (rechte oder beliebige Seite, „Rückseite der Teil-Seite leer“) |
| Seite und Ränder | Seitenformat (A5, 120 × 190 mm, 135 × 215 mm, A4, eigenes Format); Ränder innen, außen, oben, unten und Bundsteg in mm – alle vier Ränder oder keiner |
| Schrift und Absatz | Grundschrift, Schriftgröße, Zeilenabstand, Absatz (Einzug oder Abstand), Blocksatz, Silbentrennung, Sprache, Schusterjungen und Hurenkinder vermeiden |
| Seitenzahlen, Kolumnentitel, Fußnoten | Seitenzahlen (unten mittig, unten außen, oben außen, keine; Zählung ab der ersten Seite oder ab dem ersten Kapitel); Kolumnentitel (aus, Werktitel, Kapitel); Fußnoten (1, 2, 3 · *, †, ‡ · a, b, c; fortlaufend oder je Kapitel neu) |
| Querverweise | Verweis mit Seitenzahl: Vorgabe für das Häkchen im Fenster „Verweis“ |
| Register und Glossar | Glossarseite, Registerseiten, erste Erwähnung auszeichnen – siehe [Register und Glossar](/hilfe/suchen/#register) |
| Entwurf | Wasserzeichen „Entwurf“ – siehe unten |

Die Fachwörter kurz erklärt: Der **Bundsteg** ist die Zugabe für die Bindung – was in Falz und Leim verschwindet,
fehlte sonst am Innenrand. Der **Kolumnentitel** ist die Kopfzeile über dem Text. Ein **Schusterjunge** ist die
erste Zeile eines Absatzes, die allein unten auf einer Seite steht, ein **Hurenkind** die letzte Zeile eines
Absatzes allein oben auf der nächsten Seite.

- **Vorschau** setzt einen erfundenen Mustertext mit den Werten im Formular, **ohne zu speichern**, und zeigt die
  Seiten als kleine Bilder. Mit JavaScript erscheint sie kurz nach jeder Änderung von selbst.
- **Speichern** übernimmt alle Werte. Ungültige Werte lehnt Baruch ab; die Meldung steht beim Feld (zum Beispiel
  „Schriftgröße: 6 bis 24 pt“).
- **Zurücksetzen** (mit Häkchen) löscht alle Satz-Einstellungen; danach gilt wieder nur die Werkvorlage.
- **Ganzes Werk im Satz ansehen** öffnet die Satz-Vorschau.
- **Hinweise zur Werkvorlage:** Legt die Vorlage dieselbe Sache selbst fest, steht beim Feld ein Hinweis mit der
  Zeile der Vorlage. Meist gilt die Einstellung trotzdem; wirkt sie nicht, ist der Hinweis gelb umrandet.
- **Kapitelanfang aus der Vorlage übernehmen:** Erkennt Baruch in der Vorlage eine Regel für den Kapitelanfang,
  bietet der Block an, sie als Einstellung zu übernehmen und die Zeile aus der Vorlage zu entfernen (mit
  Häkchen).
- Kapitelanfangs-, Teil- und Leerseiten tragen keinen Kolumnentitel; Teil- und Leerseiten keine Seitenzahl.
- Der Stichwortzettel bleibt von den Satz-Einstellungen unberührt.

**Satzbild im Browser:** Der Block **Satzbild** darüber stellt nur ein, wie der Text beim Schreiben am
Bildschirm aussieht (Schrift, Größe, Satzbreite, Farben) – nicht das gedruckte Buch.

## Buch als PDF {#pdf}

{{< hilfebild name="satz-vorschau" alt="Die Satz-Vorschau mit dem Knopf „PDF herunterladen“ in der Gruppe Herunterladen des Menübands." titel="Das PDF hat immer genau die Seiten der Satz-Vorschau." >}}

So geht es:

1. **Buch → Ausgabe → PDF** wählen (oder **PDF herunterladen** in der Satz-Vorschau, oder „PDF“ in der Liste der
   Werke).
2. Der Browser lädt das Buch als PDF-Datei.

Im PDF stehen die Werkvorlage, die aktuellen Fassungen und die Bilder – und, wenn eingeschaltet, Umschlag-Vorder-
und Rückseite und die Titelei. PDF und Satz-Vorschau haben immer gleich viele Seiten.

- **Nie eine halbe Datei:** Scheitert der Satz, erscheint statt des PDFs eine Seite mit allen Meldungen und dem
  Verweis zum Abschnitt.
- Entsteht für dasselbe Werk gerade schon ein PDF, bittet Baruch, kurz zu warten.
- **Entwurf:** **Buch → Ausgabe → Entwurf** lädt dasselbe PDF mit dem Wasserzeichen „ENTWURF“ – gut zum
  Weitergeben vor der Fertigstellung. Unter **Einstellungen → Werk → Satz → Entwurf** lässt sich das Wasserzeichen
  dauerhaft einschalten und anpassen: Text, Zusatz, Datum, Deckkraft, Lage (diagonal, Kopfzeile, Fußzeile) und
  „auch auf dem Umschlag-PDF“. Es ändert den Umbruch nicht. In der Satz-Vorschau stehen dazu **Als Entwurf
  herunterladen** und **Ohne Wasserzeichen herunterladen**.

Von der Kommandozeile:

```sh
baruch-server export mein-buch --pdf mein-buch.pdf
```

Eine vorhandene Datei wird nie überschrieben.

## Umschlag und Titelei {#umschlag}

{{< hilfebild name="umschlag-editor" alt="Der Umschlag-Editor: der ganze Umschlag als Vorschau – links die Rückseite, in der Mitte der Rücken, rechts die Vorderseite mit einem Umschlagbild –, darüber die Rahmen der Textblöcke und die Hilfslinien für Beschnitt und Sicherheitsabstand; daneben die Werte des gewählten Blocks." titel="Der Umschlag-Editor: Blöcke auf Vorderseite, Rücken und Rückseite frei setzen." >}}

**Umschlagbild wählen:** auf der Seite **Bilder** (Knopf **Als Umschlag**) oder unter **Einstellungen → Werk →
Umschlag**. Assistenten können das nur beantragen.

**Der Umschlag-Editor** (**Buch → Umschlag**) zeigt den ganzen Umschlag für die Druckerei: links die Rückseite,
in der Mitte der Rücken, rechts die Vorderseite, außen der Beschnitt. Darüber liegen die **Rahmen der Blöcke**
(Titel, Untertitel, Autor, Verlag, Klappentext, Kurzbiografie mit Foto, ISBN mit Strichcode, Motto, freier Text)
und die **Hilfslinien** (Beschnitt, Schnittkante, Sicherheitsabstand 5 mm, Rückenkanten, Mitten).

So geht es:

1. Einen Rahmen mit der Maus ziehen; die Griffe an den Ecken ändern die Breite. Der Rahmen rastet an
   Schnittkante, Sicherheitsabstand und Mitte ein.
2. Daneben je Block die Werte aufklappen und ändern: Lage, Breite, Schriftgröße, Zeilenabstand, Ausrichtung,
   Farbe, Schrift, Hintergrund (keiner, Schatten, Verlauf), sichtbar ja oder nein.
3. **Speichern** (Strg+S, Mac: ⌘S).

| Taste | Wirkung |
|---|---|
| Tab | wählt den nächsten Rahmen |
| Pfeiltasten | verschieben um 0,5 mm, mit Umschalt um 5 mm |
| Strg+← und → (Mac: ⌘← und ⌘→) | Breite ändern (am Rücken Strg+↑ und ↓, Mac: ⌘↑ und ⌘↓) |
| Strg+Z (Mac: ⌘Z) | letzten Schritt zurücknehmen |
| Strg+S (Mac: ⌘S) | speichern |
| Esc | Auswahl aufheben |

- **Schnellwahl oben/Mitte/unten** setzt die Blöcke der Vorderseite auf die Vorgaben; **Anordnung zurücksetzen**
  löscht die gespeicherte Anordnung der angekreuzten Flächen.
- **Ohne JavaScript** sind dieselben Werte ein Formular mit Zahlenfeldern: **Vorschau ohne Speichern** und
  **Anordnung übernehmen**.
- **Rücken:** Seine Breite rechnet Baruch aus der Seitenzahl und der **Papierdicke je Blatt** (ein Blatt sind
  zwei Seiten; Vorgabe 0,2 mm, übliches Werkdruckpapier hat etwa 0,1 mm). Die Rechnung steht bei den Maßen.
- **Umschlag-PDF** (**Buch → Umschlag → Umschlag-PDF**) ist dieser Umschlag als eine Seite für die Druckerei.
- **Druckprüfung** unter der Vorschau: Block ragt über die Schnittkante, liegt zu nah an der Kante, überdeckt den
  Strichcode, ist breiter als der Rücken, hat zu wenig Kontrast zum Bild oder eine fehlende Schrift.

{{< hilfebild name="satz-buchangaben" alt="Der Block Buchangaben mit erfundenen Angaben: Verlag, Verlagsort, Jahr, Auflage, ISBN, Preis, Reihe, Druck, Papierdicke, Beschnitt, die Häkchen „Titelei setzen“ und „Umschlag-Vorder- und Rückseite im Satz zeigen“, darunter Widmung, Klappentext und Kurzbiografie." titel="Die Buchangaben füllen Titelei, Impressum und Umschlag." >}}

**Buchangaben** (**Einstellungen → Werk → Buchangaben**, auch auf der Seite des Umschlags): Verlag, Verlagsort,
Jahr, Auflage, ISBN (die Prüfziffer wird geprüft), Preis, Reihe, Druck, Anschrift von Verlag und Druckerei,
Autorenfoto, Papierdicke, Beschnitt (Vorgabe 3 mm), Widmung, Klappentext, Kurzbiografie, Rechtehinweis der
Bibelübersetzung und Bildnachweis.

- **Umschlag im Buch:** Mit dem Häkchen „Umschlag-Vorder- und Rückseite im Satz zeigen“ (Vorgabe an) ist die
  erste Seite des PDFs die Vorderseite, die zweite die leere Innenseite, die letzte die Rückseite (Klappentext,
  Kurzbiografie mit Foto, ISBN mit Strichcode, Verlag und Preis).
- **Titelei setzen** fügt vorn Schmutztitel, Haupttitel und Impressum ein, mit Widmung auch eine Widmungsseite.
  Diese Seiten tragen keine Seitenzahl.
- **Eigene Titelei:** Wer die Werkvorlage selbst schreibt, kann die vier Seiten dort selbst gestalten; der Knopf
  **Impressum-Vorlage in die Werkvorlage übernehmen** hängt die Bausteine dafür an.
- **Impressum:** Bei eingeschalteter Titelei prüft Baruch, ob die üblichen Angaben da sind (Verlag bzw.
  Herausgeber mit Anschrift, Druckerei mit Anschrift, Erscheinungsjahr, Rechtehinweis der Bibelübersetzung). Das
  ist **keine Rechtsberatung**: Was im Einzelfall vorgeschrieben ist, prüft der Verlag bzw. die Druckerei.

## E-Book (EPUB) {#epub}

{{< hilfebild name="epub-vorschau" alt="Die EPUB-Vorschau: derselbe Buchanfang nebeneinander in mehreren Rahmen in der Größe eines Telefons, eines Tablets und eines Lesegeräts; darüber die Auswahl der Lesegeräte, der Datei, der Fußnoten und des Nachtmodus." titel="Die EPUB-Vorschau zeigt das E-Book in mehreren Lesegeräten nebeneinander." >}}

**EPUB** ist das übliche Format für E-Books; fast jedes Lesegerät und jede Lese-App öffnet es.

So geht es:

1. **Buch → Ausgabe → EPUB** lädt das E-Book herunter.
2. Vorher lohnt **Buch → Satz → EPUB-Vorschau**: Sie zeigt das E-Book in mehreren Lesegeräten nebeneinander.
   Lesegeräte per Häkchen wählen, dazu die Datei (Teil, Kapitel, Titelei), die Art der Fußnoten und den
   Nachtmodus, dann **Anzeigen**.

- **Aufteilung:** Jeder Teil und jedes Kapitel beginnt im Lesegerät auf einer neuen Seite.
- **Fußnoten:** Vorgabe „Popup“ – Lesegeräte, die das können, zeigen die Fußnote beim Antippen; alle anderen
  finden sie am Kapitelende. Wahlweise stehen alle Fußnoten als „Anmerkungen“ am Ende.
- **Bilder** kommen mit Alternativtext und Bildunterschrift mit.
- **Titelbild** ist die gesetzte Vorderseite des Umschlags; ohne Umschlagbild steht der Titel auf einer dunklen
  Fläche. Mit eingeschalteter Titelei folgen Titelseite, Impressum und Widmung.
- **Was sich im E-Book nicht abbilden lässt** (Formeln, eigene Befehle der Vorlage), lässt Baruch nie still weg,
  sondern meldet es.
- **Kindle:** Ein eigenes Kindle-Format gibt es nicht; „Send to Kindle“ nimmt das EPUB an und wandelt es um.
- **Die Vorschau ist eine Annäherung:** Baruch bildet Schrift, Größe, Ränder und Nachtmodus der Lesegeräte nach,
  ohne echtes Gerät. Verbindlich bleibt das Gerät selbst.

{{< hilfebild name="epub-pruefung" alt="Der Prüfbericht des EPUB: oben die Zusammenfassung „Keine Fehler“ mit der Zahl der Warnungen und Hinweise, darunter die Liste der Befunde mit Art, Meldung, Ort und Code und der Knopf „EPUBCheck ausführen“." titel="Der Prüfbericht nennt Fehler, Warnungen und Hinweise zum E-Book." >}}

**Prüfen:** Unter der Vorschau steht der Prüfbericht kurz; **Ganzer Prüfbericht, EPUBCheck ausführen …** öffnet
ihn ganz. Geprüft werden der Aufbau der Datei, die Barrierefreiheit (Sprache, Titel, Alternativtexte der Bilder),
kaputte Verweise und – je Lesegerät – was es nicht kann. **EPUBCheck ausführen** prüft zusätzlich mit
EPUBCheck, dem Prüfprogramm der W3C-Gemeinschaft, wenn es auf dem Rechner liegt (siehe
[Einstellungen → Daten](/hilfe/einstellungen/#daten)). Baruch lädt es nie selbst herunter; fehlt es, steht dort
ehrlich „EPUBCheck nicht gefunden“.

## Stichwortzettel {#stichwortzettel}

{{< hilfebild name="satz-stichwortzettel" alt="Die Seite Stichwortzettel: oben die Wahl von Format und Schriftgröße, darunter die Vorschau der Karten – je Gliederungspunkt Nummer und Titel, Zeitmarke, Dauer, Stichpunkte und Bibelstellen – und der Knopf „PDF herunterladen“." titel="Der Stichwortzettel: je Gliederungspunkt eine Karte für das freie Sprechen." >}}

Der **Stichwortzettel** ist für das freie Sprechen gedacht, vor allem bei Predigten.

So geht es:

1. **Buch → Ausgabe → Stichwortzettel …** wählen.
2. Format wählen – **A5 hoch** oder **Karten (A6 quer)** – und die Schriftgröße (14 bis 28 Punkt),
   **Übernehmen**.
3. Die Vorschau prüfen und **PDF herunterladen**.

Je Gliederungspunkt entsteht eine Karte: Nummer und Titel, **Zeitmarke** und **Dauer** (aus dem
[Zeitwächter](/hilfe/schreiben/#zeitwaechter)), die Stichpunkte und die Bibelstellen.

**Woher die Stichpunkte kommen** – je Abschnitt gilt die erste Regel, die etwas findet:

1. **Eigene Stichwörter:** Zeilen im Quelltext der Form `// Stichwort: …`. Sie erscheinen in keinem Buch.
2. **Hervorhebungen:** sonst die fett gesetzten Stellen.
3. **Erster Satz je Absatz:** sonst der erste Satz jedes Absatzes.

Je Abschnitt stehen höchstens 6 Punkte, jeder höchstens 160 Zeichen. Welche Regel galt, steht in der Vorschau
unter jeder Karte.

## Word-Datei für den Verlag {#word-export}

{{< hilfebild name="satz-word-export" alt="Die Seite „Word-Export“: eine kurze Erklärung, was in der Datei steht, das Häkchen „offene Vorschläge als Änderungsverfolgung“ mit der Zahl der offenen Vorschläge und der Knopf „Word-Datei herunterladen“." titel="Das Werk als Word-Datei, auf Wunsch mit den offenen Vorschlägen als Änderungen." >}}

Viele Verlage arbeiten mit Word. **Buch → Ausgabe → Word …** öffnet eine Seite mit Erklärung; **Word-Datei
herunterladen** liefert das Werk als `.docx`.

- In der Datei stehen die Titelei aus den Buchangaben und alle Abschnitte mit ihrer aktuellen Fassung:
  Überschriften mit den Word-Formatvorlagen „Überschrift 1–3“, fett und kursiv, Verweise, **Fußnoten als echte
  Fußnoten**, Zitate, Listen, einfache Tabellen und eingebettete Bilder.
- Die Nummern der Gliederung stehen nicht in der Datei; die Zählung richtet der Verlag an den Formatvorlagen ein.
- Was Word nicht abbilden kann (Formeln, eigene Befehle der Vorlage, Kästen), steht als grau hinterlegtes Feld
  mit dem lesbaren Inhalt in der Datei. Nichts geht still verloren.
- **Offene Vorschläge als Änderungsverfolgung:** Mit dem Häkchen steht je Abschnitt der jüngste offene Vorschlag
  als Einfügung und Löschung in der Datei („Überarbeiten“ in Word), mit dem Kürzel des Assistenten als
  Bearbeiter. „Alle ablehnen“ ergibt die aktuelle Fassung, „Alle annehmen“ den Vorschlag.
- Der Export ändert nichts am Werk.
- Zitate im Text tragen in Word keine Anführungszeichen; Beiträge aus dem Chat gehen nicht mit.

**Nicht geprüft:** Bisher wurde keine dieser Dateien in Microsoft Word geöffnet. Geprüft ist, dass Baruch die
eigene Datei wieder einlesen kann und dass LibreOffice eine Probedatei in ein PDF umwandelt.

Den umgekehrten Weg beschreibt [Word-Datei einlesen](/hilfe/einlesen/#word).

## Typst-Export und eigenes Setzen {#typst-export}

{{< hilfebild name="band-buch" alt="Das Menüband mit dem Reiter Buch; in der Gruppe Ausgabe stehen PDF, Entwurf, EPUB, Stichwortzettel, Word, Exportieren (Typst) und Austausch: Ausgeben." titel="Alle Ausgaben stehen im Reiter Buch, Gruppe Ausgabe." >}}

**Buch → Ausgabe → Exportieren (Typst)** lädt das Werk als eine Textdatei im Satzformat Typst: die Werkvorlage,
alle aktuellen Texte, die Satz-Einstellungen und – wenn eingeschaltet – Register und Glossar. Direkt nach dem
Einlesen einer Typst-Datei ist der Export Zeichen für Zeichen die Eingabe.

Die Datei lässt sich ohne ein eigenes Typst setzen, mit denselben Schriften wie in Baruch:

```sh
baruch-server export mein-buch --out ordner     # Typst-Datei und Bilder in einen Ordner
baruch-typst compile ordner/mein-buch.typ mein-buch.pdf
```

Weitere Ausgaben von der Kommandozeile:

```sh
baruch-server export mein-buch --epub mein-buch.epub
baruch-server export mein-buch --docx mein-buch.docx --vorschlaege
baruch-server export mein-buch --stichwort zettel.pdf --format karten
```

Die Austauschdatei für ein anderes Baruch beschreibt [Austausch](/hilfe/austausch-sicherung/#ausgeben).
