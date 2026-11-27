---
title: "Schreiben, Bilder und Fassungen"
description: "Im Satzbild schreiben und speichern, Bilder einfügen, zwischen den Fassungen eines Abschnitts umschalten und Vorschläge annehmen."
weight: 50
---

Geschrieben wird Abschnitt für Abschnitt. Ein Klick in der Gliederung öffnet einen Abschnitt in der Mitte der
Arbeitsseite; **Speichern** legt den Text sicher ab. Nichts geht verloren: Jeder gespeicherte Stand bleibt als
**Fassung** oder in deren Verlauf erhalten.

## Im Satzbild schreiben {#satzbild}

{{< hilfebild name="schreiben-satzbild" alt="Ein Abschnitt des Beispielwerks im Satzbild: Der Text steht auf einem hellen Blatt in der Schrift des Buchs, mit Überschrift, Absätzen und einem hervorgehobenen Kasten; im Menüband darüber die Knöpfe Satzbild, Quelltext, Absatzformat, K, F, Speichern und Als neue Fassung." titel="Das Satzbild zeigt den Abschnitt so, wie die Werkvorlage ihn setzt." >}}

Das **Satzbild** zeigt den Abschnitt so, wie er nach der Werkvorlage aussieht: Schrift, Farben, Einzüge, Kästen.
Man schreibt darin wie in einer Textverarbeitung.

So geht es:

1. In der Gliederung links einen Abschnitt anklicken.
2. In den Text klicken und schreiben.
3. **Strg+S (Mac: ⌘S)** oder der Knopf **Speichern** legt den Text ab.

| Taste | Wirkung |
|---|---|
| Enter | neuer Absatz (in Listen: neuer Punkt) |
| Umschalt+Enter (Mac: ⇧↩) | einfacher Zeilenumbruch |
| Strg+I (Mac: ⌘I) oder Knopf **K** | hervorheben (kursiv) |
| Strg+B (Mac: ⌘B) oder Knopf **F** | fett |
| Strg+S (Mac: ⌘S) | speichern |

- **Absatzformat** im Menüband macht aus dem Block am Cursor einen Absatz oder eine Überschrift 1 bis 3.
- **Einfügen aus der Zwischenablage** übernimmt nur Text. Leerzeilen trennen Absätze.
- **Überschriften sind immer eine Zeile.** Umschalt+Enter beginnt in einer Überschrift einen neuen Absatz;
  mehrzeiliger Text, in eine Überschrift eingefügt, wird zu Absätzen darunter.
- **Unberührte Absätze bleiben Zeichen für Zeichen**, wie sie sind; nur bearbeitete Blöcke schreibt Baruch neu.
  Laden und Speichern ohne Änderung ergibt denselben Text.
- **Teile** und Punkte **ohne Nummer** stehen als eigene Zeile da: ein Teil groß und zentriert mit „Teil“ davor,
  ein Punkt ohne Nummer mit dem Zusatz „ohne Nummer“. Den Titel ändert man direkt darin.
- **Kästen der Werkvorlage** (im Beispiel „Szene“ und „Arbeitsauftrag“) stehen als Kasten da; jeder Absatz darin
  ist einzeln bearbeitbar.
- **Listen, Aufzählungen, Begriffslisten und Verweise** erscheinen als das, was sie sind. Der Text eines
  Verweises lässt sich im Satzbild ändern.
- **Gelb hinterlegte Stellen („Inseln“)** sind Stellen, die das Satzbild nicht sicher abbilden kann: Formeln,
  eigene Befehle der Vorlage, Kommentare. Ein **Doppelklick** öffnet an Ort und Stelle ein Feld mit dem
  Quelltext: **Strg+Enter (Mac: ⌘↩)** oder „Übernehmen“ übernimmt, **Esc** oder „Abbrechen“ stellt die Insel
  wieder her. Eine neue Insel legt man im Quelltext an.
- Vor dem Wechsel zu einer anderen Stelle fragt Baruch nach, wenn ungespeicherter Text im Blatt steht.
- Sehr lange Abschnitte erscheinen einen Augenblick nach dem Laden.

### Satzbild, Quelltext und Vorschau {#ansichten}

{{< hilfebild name="schreiben-quelltext" alt="Derselbe Abschnitt im Quelltext: ein Textfeld mit dem Text im Satzformat Typst, die Überschrift beginnt mit einem Gleichheitszeichen, Hervorhebungen stehen zwischen Unterstrichen." titel="Der Quelltext zeigt, was wirklich gespeichert wird." >}}

- **Satzbild | Quelltext** (Reiter **Start**, Gruppe „Text“) wechselt die Ansicht. Vorgabe ist immer das
  Satzbild; nur wer ausdrücklich „Quelltext“ wählt, bleibt dort. Der **Quelltext** ist der Text im Satzformat
  Typst – gewöhnlicher Text mit wenigen Zeichen: `=` am Zeilenanfang für eine Überschrift, `_so_` für
  Hervorhebung, `*so*` für fett.
- **Editor/Vorschau** (Reiter **Start**, Gruppe „Ansicht des Textes“) zeigt statt des Editors die **Vorschau**:
  das ganze Kapitel im Lesefluss, mit den offenen Vorschlägen zwischen den Absätzen.

{{< hilfebild name="schreiben-vorschau" alt="Die Vorschau der Arbeitsseite: mehrere Abschnitte des Kapitels untereinander im Lesefluss; der Umschalter Editor/Vorschau im Menüband steht auf Vorschau." titel="Die Vorschau zeigt das Kapitel im Zusammenhang." >}}

Ohne JavaScript gibt es nur den Quelltext in einem Textfeld, mit **Speichern** darunter.

### Knöpfe zum Einfügen {#einfuegen}

Der Reiter **Einfügen** fügt an der Cursorstelle ein – im Satzbild wie im Quelltext:

- immer: **Fußnote**, **Zitat**, **Liste**, **Aufzählung**, **Begriff** (Begriffsliste), **Umbruch**, **Link**,
  **Verweis**, **Bild**;
- nur wenn die Werkvorlage sie kennt: ihre eigenen Formate (im Beispiel **Szene** und **Arbeit**),
  **Bibelstelle** und **Literaturverweis**. Fehlt der Vorlage das Format, fehlt der Knopf.

Eine Markierung wandert in das Eingefügte (zum Beispiel wird markierter Text zur Fußnote). Im Satzbild fragt
Baruch bei „Bibelstelle“ nach der Stelle („Joh 3,16“) und bei „Literaturverweis“ nach dem Kurznamen der Quelle.
Kann das Satzbild ein Format noch nicht einfügen, sagt ein Hinweis unten rechts „… bitte im Quelltext einfügen“;
es geschieht dann nichts stillschweigend. Rückgängig mit Strg+Z (Mac: ⌘Z) geht wie nach dem Tippen.

**Zitat:** Der Knopf fügt einen Zitatblock mit je einer Leerzeile davor und danach ein; der Cursor steht im
Zitat. Die Stellenangabe schreibt man selbst als eigenen Absatz direkt darunter („– Stelle“). Ist im Satzbild
Text markiert, wird die Markierung ein Zitat im Fließtext.

## Speichern und Fassungen {#fassungen}

{{< hilfebild name="schreiben-fassungen" alt="Die rechte Spalte mit dem aufgeklappten Bereich „Fassungen“: für die gezeigte Fassung die Knöpfe „Diese Fassung verwenden“, „Fassung ändern“ und „Löschen“, darunter die Fassungen des Abschnitts als eingerückte Liste mit Nummer, Status, Autor und Zeit; die aktuelle ist markiert." titel="Der Bereich „Fassungen“ rechts: umschalten, ändern, löschen." >}}

Ein Abschnitt hat beliebig viele **Fassungen**. Jede hat eine Nummer und beruht auf einer anderen; zusammen
bilden sie einen Baum. **Genau eine Fassung je Abschnitt ist „aktuell“** – sie steht im Buch, im PDF und im
Export.

**Speichern** arbeitet wie in einer Textverarbeitung:

- **Speichern** (Strg+S, Mac: ⌘S – gleich, wo auf der Arbeitsseite der Cursor steht) schreibt die eigene aktuelle
  Fassung fort. Sie behält ihre Nummer, ihr **Stand** zählt hoch („Fassung 3 · Stand 2“ im Kopf). Der Cursor
  steht danach an derselben Stelle. Die Rückmeldung lautet zum Beispiel „Gespeichert: fortgeschrieben (Stand 2).“
- **Als neue Fassung speichern** legt immer eine neue Fassung an – sinnvoll vor einem größeren Umbau.
- Eine neue Fassung entsteht von selbst, wenn die aktuelle Fassung von einem Assistenten stammt (auch ein
  angenommener Vorschlag), wenn man eine andere als die aktuelle Fassung bearbeitet hat oder wenn sich die
  Überschrift oder ihre Ebene ändert. Die Rückmeldung nennt den Grund.
- Wer will, dass große Änderungen oder lange Pausen von selbst eine neue Fassung ergeben, schaltet das unter
  **Einstellungen → Werk → Fassungen** ein („ab einer Änderung von mehr als … Prozent“, „wenn seit dem letzten
  Speichern … Minuten vergangen sind“). In der Vorgabe gibt es keine solche Grenze.
- Gleicher Text wie zuvor speichert nichts.
- Sind zwei Fenster auf demselben Abschnitt offen, überschreibt keines das andere still: Das spätere Speichern
  wird eine eigene Fassung; beide Texte bleiben erhalten.
- Während des Speicherns ist der Editor kurz gesperrt, damit kein getipptes Zeichen verloren geht.

**Mit Fassungen arbeiten** (Bereich **Fassungen** in der rechten Spalte):

- **‹ n/m ›** im Kopf des Abschnitts blättert durch die Fassungen (Alt+← und Alt+→, Mac: ⌥← und ⌥→). Was gezeigt
  wird, wird auch bearbeitet.
- **Diese Fassung verwenden** macht die gezeigte Fassung zur aktuellen. Kein Text ändert sich dabei; die bisher
  aktuelle heißt danach „ersetzt“ und lässt sich mit einem Klick zurückholen.
- **Fassung ändern** ändert Text und Notiz der gezeigten Fassung selbst (im Quelltext); sie trägt danach den
  Vermerk „geändert“ mit Zeitpunkt. Leerer Text wird abgelehnt.
- **Löschen** entfernt die gezeigte Fassung – nur mit dem Häkchen „Fassung n wirklich löschen“. Die aktuelle
  Fassung lässt sich nicht löschen: erst auf eine andere umschalten. Die Nummer einer gelöschten Fassung wird
  nie neu vergeben. Der gelöschte Text bleibt im Verlauf des Werks nachlesbar.
- Die **Baumansicht** zeigt die Fassungen eingerückt nach ihrer Herkunft, mit Nummer, Status, Autor (Assistenten
  in ihrer Farbe), Zeit und „geändert“. Aufgeklappt zeigt sie das ganze Kapitel. Je Fassung gibt es **zeigen**
  und **in der Vorschau probieren** – die Vorschau setzt die Fassung dann probeweise ein, ohne etwas umzuschalten.
- Hat ein Abschnitt weitere nicht aktuelle Fassungen, zeigt der Kopf **weitere Fassung(en): n** zum Aufklappen,
  je mit **aktuell machen**. In der Gliederung trägt der Abschnitt dann das Abzeichen „+n“.

Ändern, Löschen und Umschalten kann nur der Mensch. Assistenten legen nur Vorschläge an.

{{< hilfebild name="schreiben-aeltere-fassung" alt="Die Arbeitsseite zeigt eine ältere Fassung: Im Kopf steht „Fassung 1 · ersetzt“, der Umschalter zeigt 1/2, daneben der Verweis „aktuell: Fassung 2“." titel="Eine ältere Fassung ansehen – und mit einem Klick wieder verwenden." >}}

### Verlauf und Rückgängig {#verlauf}

{{< hilfebild name="schreiben-fassungsverlauf" alt="Der Verlauf einer Fassung: die Speicherschritte, neueste zuerst, je mit Autor, Zeit und dem Unterschied Wort für Wort – Entferntes rot, Neues grün – und dem Knopf „Bis hier zurück“." titel="Der Verlauf zeigt jeden gespeicherten Stand einer Fassung." >}}

Jedes Fortschreiben bleibt als Schritt erhalten. Ein Klick auf **Stand k** (im Kopf, im Bereich „Fassungen“ oder
in der Baumansicht) öffnet den **Verlauf**: alle Schritte, neueste zuerst, je mit Art, Autor, Zeit und dem
Unterschied Wort für Wort (Entferntes rot, Neues grün).

**Bis hier zurück** stellt den Stand vor diesem Schritt wieder her – als neuer Schritt; der Verlauf bleibt
vollständig. Das geht nur auf der aktuellen Fassung des Menschen.

### Ältere Fassungen ausblenden {#ausblenden}

{{< hilfebild name="schreiben-fassungen-ausblenden" alt="Einstellungen, Seite Werk, Block Fassungen: die Schalter zum Fortschreiben, darunter das Datumsfeld „Ältere Fassungen ausblenden bis“ und der Knopf „Alle Marken aufheben“." titel="Ältere Fassungen im ganzen Werk ausblenden." >}}

Ein Buch sammelt schnell Dutzende Fassungen je Abschnitt. Ältere lassen sich **ausblenden** – gelöscht wird
nichts.

- **bis hierhin ausblenden** steht an jeder Fassung, unter der es etwas auszublenden gibt (im Bereich
  „Fassungen“, in der Baumansicht und im Kopf neben „‹ n/m ›“). Danach stehen alle ersetzten und verworfenen
  Fassungen mit kleinerer Nummer hinter dem Schalter **„n ältere Fassungen zeigen ▸“**.
- **Immer sichtbar** bleiben die aktuelle Fassung und offene Vorschläge.
- Der Zähler zeigt dann zum Beispiel „2/3 (+ 24 ausgeblendet)“; ‹ › überspringt die ausgeblendeten. Ein Klick auf
  „(+ 24 ausgeblendet)“ zeigt wieder alle, „24 ältere verbergen“ blendet sie wieder aus.
- **Wieder alle zeigen** im Bereich „Fassungen“ hebt die Marke des Abschnitts auf.
- **Für das ganze Werk:** **Einstellungen → Werk → Fassungen → Ältere Fassungen ausblenden bis** (Datum) blendet
  in jedem Abschnitt alles vor diesem Tag aus; **Alle Marken aufheben** zeigt wieder alles.

Ausblenden kann nur der Mensch.

## Vorschläge der Assistenten {#vorschlaege}

{{< hilfebild name="schreiben-vorschlag-lesefluss" alt="Ein Vorschlag eines Assistenten im Lesefluss: unter dem betroffenen Absatz ein farbig umrandeter Kasten mit dem Namen des Assistenten, der Notiz und dem Unterschied – Gestrichenes rot durchgestrichen, Neues grün – sowie den Knöpfen Übernehmen und Verwerfen." titel="Ein Vorschlag steht an seiner Stelle; der Mensch entscheidet." >}}

Ein Assistent ändert nie den gültigen Text. Er legt einen **Vorschlag** an – eine eigene Fassung, die wartet.
Offene Vorschläge stehen **an ihrer Stelle**: direkt bei dem Absatz, den sie ändern, in der Farbe ihres
Assistenten.

So geht es:

1. Den Vorschlag lesen. Der Kopf nennt Assistent, Fassung und Modell, darunter die Notiz. Entferntes ist rot
   durchgestrichen, Neues grün (im Quelltext und ohne JavaScript Zeile für Zeile mit + und −).
2. **Übernehmen** macht den Vorschlag zur aktuellen Fassung. **Verwerfen** lässt den Text, wie er ist.

- Verglichen wird mit der **gezeigten** Fassung – das ist, was „Übernehmen“ ersetzt. Beruht der Vorschlag auf
  einer anderen Fassung, steht „auf Fassung m“ im Kopf. Auf einer fortgeschriebenen Fassung nennt er auch den
  Stand („beruht auf Fassung 3, Stand 2“).
- Wurde die Fassung seit dem Vorschlag weiter fortgeschrieben, steht „veraltet: Fassung 3 ist inzwischen bei
  Stand 4“. Wer ihn dennoch übernimmt, ersetzt die späteren eigenen Änderungen (sie bleiben in der ersetzten
  Fassung erhalten).
- Auch ein übernommener Vorschlag verdrängt nichts endgültig: Alle Fassungen bleiben; man kann jederzeit
  zurückschalten.
- Ein Vorschlag mit mehreren Stellen heißt „Teil 1/2“, „Teil 2/2“. Mehrere Vorschläge an derselben Stelle stehen
  untereinander. Vorschläge von Programmen ohne eigene Rolle stehen am Ende in Grau.
- **Einklappen:** Ein Klick auf die Kopfzeile klappt den Vorschlag zu einer farbigen Linie zusammen und wieder
  auf. Oben im Blatt steht „n offene Vorschläge · **Alle einklappen**“.
- Am Reiter **Start** steht die Zahl der offenen Vorschläge im ganzen Werk.

Wie man einen Vorschlag bestellt, steht unter [Assistenten, Chat und Befunde](/hilfe/assistenten/).

## Bilder des Werks {#bilder}

{{< hilfebild name="schreiben-bilder" alt="Die Seite „Bilder“ des Beispielwerks: oben der Block Umschlag, darunter die Liste mit Vorschaubild, Name, Typ, Größe, Pfaden, Alternativtext und Fassung eines Beispielbilds, unten das Formular „Bild hinzufügen“." titel="Alle Bilder eines Werks an einem Ort." >}}

Jedes Werk hat eine Seite **Bilder** (Reiter **Buch → Bilder**). Sie geht ohne JavaScript.

So kommt ein Bild ins Werk:

1. **Buch → Bilder** öffnen.
2. Unter **Bild hinzufügen** eine Datei wählen (PNG, JPEG, GIF, WebP oder SVG, höchstens 8 MiB).
3. Auf Wunsch Name, Bildunterschrift, **Alternativtext** (die Beschreibung für Menschen, die das Bild nicht
   sehen) und eine Notiz eintragen.
4. Hochladen.

- **Liste:** Vorschaubild, Name, Typ, Größe, Pfade, Alternativtext, die aktuelle Fassung („2 von 3“) und die
  Spalte **Umschlag** mit dem Knopf **Als Umschlag**.
- **Suche** über der Liste findet nach Name, Bildunterschrift und Alternativtext.
- **Ein Bild** anklicken zeigt Vorschau, Typ, Größe und alle **Fassungen** des Bilds mit **Aktuell machen**.
  **Ersetzen** lädt eine neue Fassung hoch, die sofort aktuell wird; die alten bleiben. Ein vorhandener Name beim
  Hinzufügen tut dasselbe.
- **Anderes Format:** Eine Datei in einem anderen Format (etwa PNG statt SVG) ersetzt ein Bild nur mit dem
  Häkchen **Endung anpassen**. Das Bild heißt dann mit der neuen Endung; der Text muss nicht geändert werden.
- **Beschreiben:** Bildunterschrift, Alternativtext und Notiz lassen sich je Fassung ändern.
- **Löschen:** eine nicht aktuelle Fassung oder das ganze Bild, jeweils mit Häkchen.
- **Umschlag** (Block oben): das Umschlagbild wählen oder entfernen und den Titelbereich festlegen – siehe
  [Buchsatz](/hilfe/buchsatz/#umschlag).
- **Pfade mit zwei Bildern:** In älteren Werken kann derselbe Pfad zu zwei Bildern gehören. Die Seite zeigt das
  oben an; man wählt je Pfad das Bild, das ihn behält, und drückt **Zuordnen**.

**Anträge der Assistenten** (Block unten): Assistenten dürfen Bilder **hinzufügen**. Ersetzen, Umschlag setzen
oder entfernen und Löschen können sie nur **beantragen**. Je Antrag stehen dort, was beantragt ist, wer ihn
stellt, die Begründung und beim Ersetzen eine kleine Vorschau; **Bestätigen** führt ihn aus, **Ablehnen** ändert
nichts. Hat sich das Bild inzwischen geändert, gilt der Antrag als veraltet. Ein Assistent darf je Werk in
24 Stunden höchstens 30-mal Bilder hinzufügen oder das Ersetzen beantragen.

### Ein Bild in den Text einfügen {#bild-einfuegen}

{{< hilfebild name="schreiben-bild-einfuegen" alt="Das aufgeklappte Formular „Bild einfügen“ in der Statuszeile unter dem Blatt: Auswahl eines Bilds des Werks, die Wahl der Stelle (an der Cursorstelle, am Ende, am Anfang, vor einem Absatz), ein Feld für die Bildunterschrift und der Knopf „Einfügen als neue Fassung“." titel="Ein Bild des Werks an der Cursorstelle einfügen." >}}

So geht es:

1. Den Cursor an die gewünschte Stelle setzen.
2. **Einfügen → Bild** drücken oder unten in der Statuszeile **Bild einfügen …** aufklappen.
3. Ein Bild des Werks wählen. Unter **Stelle** steht „an der Cursorstelle“ (Vorgabe mit JavaScript), „am Ende“,
   „am Anfang (unter der Überschrift)“ oder „vor …“ einem bestimmten Absatz.
4. Auf Wunsch eine Bildunterschrift eintragen (leer: die des Bilds) und **Einfügen als neue Fassung** drücken.

Das Bild steht danach als eigener Block im Text – nie vor oder in der Überschrift. Mit JavaScript geht der noch
nicht gespeicherte Text mit; nur die Überschrift darf sich nicht geändert haben (sonst: erst speichern). Ohne
JavaScript vorher speichern.

- Im Satzbild erscheint das Bild mit Unterschrift und Alternativtext. Ändern lässt es sich per Doppelklick,
  löschen als Ganzes.
- **Fehlende Bilder** meldet Baruch deutlich („Bild fehlt: …“ mit Verweis zur Seite Bilder) – in der Vorschau,
  über dem Text und im Satzbild an der Stelle des Bilds.
- **Kapitelbild:** Legt die Werkvorlage für jedes Kapitel ein Bild fest, zeigt der Kopf jedes Abschnitts das Bild
  seines Kapitels klein an. Lässt es sich nicht sicher ermitteln, steht dort „Bild aus der Vorlage“ – Baruch rät
  nicht.

## Zeitwächter {#zeitwaechter}

{{< hilfebild name="schreiben-zeitwaechter" alt="Der Kopf eines Abschnitts einer Predigt mit der Zeile „Abschnitt ≈ 0:45 · gesamt 12:30 / Ziel 15:00“; in der Gliederung links steht hinter jedem Punkt seine Sprechdauer." titel="Der Zeitwächter schätzt, wie lange das Vorlesen dauert." >}}

Wie lang dauert die Predigt? Baruch schätzt die **Sprechdauer** je Gliederungspunkt und für das ganze Werk aus
den gesprochenen Wörtern der aktuellen Fassung und einer Sprechgeschwindigkeit (Vorgabe **110 Wörter je
Minute**).

Die Zeiten stehen bei einer **Predigt** immer, bei einem **Buch** nur, wenn eine Zielzeit gesetzt ist:

- **In der Gliederung:** im Kopf „Gesamt ≈ m:ss / Ziel m:ss“, hinter jedem Punkt seine Dauer. Ist die Zielzeit
  überschritten, steht dort als Text „Zielzeit überschritten um m:ss“ – nicht nur als Farbe.
- **Im Kopf des Abschnitts:** „Abschnitt ≈ m:ss · gesamt m:ss / Ziel m:ss“. Mit JavaScript zählt Baruch beim
  Tippen nach einer kurzen Pause neu.
- **Einstellen:** **Einstellungen → Werk → Predigtzeit** – Wörter je Minute (60 bis 250) und Zielzeit in Minuten
  (1 bis 240; leer heißt: keine Zielzeit).

Gezählt wird nur, was gesprochen wird: Überschriften, Fußnoten, Arbeitsaufträge, Kommentare und Formeln zählen
nicht. „Joh 3,16“ sind zwei Wörter, ein Wort mit Bindestrich ist eines. Assistenten können die Zeiten lesen,
einstellen kann sie nur der Mensch.
