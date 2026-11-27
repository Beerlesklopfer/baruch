---
title: "Suchen, Verweise, Register und Glossar"
description: "Im Abschnitt und im ganzen Werk suchen und ersetzen, Querverweise setzen und prüfen, Register und Glossar pflegen."
weight: 90
---

Diese Seite beschreibt vier Hilfen für das fertige Manuskript: das Suchen und Ersetzen, Verweise von einer
Stelle des Buchs auf eine andere, das Register am Buchende und das Glossar mit erklärten Begriffen.

## Suchen {#suchen}

{{< hilfebild name="suchen-treffer" alt="Die Seite „Im Werk suchen und ersetzen“ mit einem Suchwort: oben das Formular mit Suchtext, Ersatz und den Schaltern für Groß- und Kleinschreibung, ganzes Wort und regulären Ausdruck, darunter die Treffer als Baum nach Abschnitten mit Zeile und Umfeld." titel="Suche im ganzen Werk: Treffer nach Abschnitten geordnet." >}}

**Im Abschnitt** (mit JavaScript):

1. **Strg+F (Mac: ⌘F)** öffnet oben rechts im Editor die Suchleiste. Markierter Text wird Suchtext.
2. Enter oder F3 springt zum nächsten Treffer, Umschalt+Enter oder Umschalt+F3 (Mac: ⇧↩ oder ⇧F3) zum vorigen.
   „n von m“ zählt die Treffer.
3. Esc schließt die Leiste.

Die Knöpfe **Aa**, **ab** und **.\*** schalten Groß-/Kleinschreibung, ganzes Wort und regulären Ausdruck um (mit
dem Cursor in der Leiste auch Alt+C, Alt+W, Alt+R; Mac: ⌥C, ⌥W, ⌥R). Der Knopf **⌶** (Alt+L, Mac: ⌥L) sucht **nur
in der Markierung**; eine mehrzeilige Markierung schaltet ihn beim Öffnen von selbst ein. Gesucht wird im gerade
bearbeiteten, auch ungespeicherten Text. Im Chat-Eingabefeld bleibt Strg+F (Mac: ⌘F) die Suche des Browsers.

**Im ganzen Werk:**

1. **Strg+Umschalt+F (Mac: ⌘⇧F)** drücken oder **Prüfen → Im Werk suchen und ersetzen** wählen. Auf der
   Arbeitsseite öffnet sich rechts eine Leiste mit den Treffern als Baum nach Abschnitten.
2. Ein Klick auf einen Treffer springt ohne Seitenwechsel an die Stelle. **Alt+← (Mac: ⌥←)** führt zurück.
3. **Vorschau und Ersetzen …** führt zur Seite **Im Werk suchen und ersetzen**. Außerhalb der Arbeitsseite und
   ohne JavaScript öffnet sich gleich diese Seite.

Auf der Seite lässt sich die Suche genauer einstellen:

- **Bereich:** Abschnitt, Kapitel, Teil, „ab Abschnitt bis zum Ende“, eine Auswahl aus der Gliederung
  (Häkchenliste), das ganze Werk oder alle Werke (nur Suchen).
- **Fassung:** die aktuelle oder – bei einem einzelnen Abschnitt – die gerade gezeigte.
- **Textarten:** Text, Überschriften, Fußnoten, Zitate (auch Bibelzitate), Code der Vorlage. Vorgabe beim Suchen:
  alles außer Code.
- Jeder Treffer ist ein Verweis an seine Stelle.

Grenzen: Suchtext höchstens 500 Zeichen; je Suche höchstens 2000 Treffer und 2 Sekunden – danach steht
„abgebrochen“ da. Dann den Bereich verkleinern.

## Ersetzen {#ersetzen}

{{< hilfebild name="suchen-ersetzen" alt="Die Vorschau des Ersetzens: je Abschnitt die geänderten Zeilen, das Alte durchgestrichen, das Neue hervorgehoben, davor je Stelle ein Häkchen; unten das Kästchen „Ja, ersetzen“ und der Knopf „Alle ersetzen“." titel="Vor dem Ersetzen zeigt die Vorschau jede einzelne Stelle." >}}

**Im Abschnitt:** **Strg+H** öffnet zur Suchleiste die Zeile „Ersetzen“ (auch der Pfeil ▸ links in der Leiste; am
Mac ⌘H – blendet das System damit das Fenster aus, geht Ctrl+H). **Ersetzen** ersetzt den aktuellen Treffer
(Strg+Umschalt+1, Mac: ⌘⇧1), **Alle** alle (Strg+Alt+Enter, Mac: ⌘⌥↩). Das ändert nur den Text im Editor;
gespeichert wird wie immer mit **Speichern**.

**Im ganzen Werk** (Strg+Umschalt+H, Mac: ⌘⇧H, oder die Seite **Im Werk suchen und ersetzen**):

1. Suchtext und Ersatz eintragen, Bereich wählen.
2. **Vorschau** ansehen: Je Abschnitt stehen die geänderten Zeilen da, alt durchgestrichen, neu hervorgehoben. Je
   Abschnitt und je Stelle gibt es ein Häkchen; Abgewähltes bleibt, wie es ist.
3. **Ja, ersetzen** anhaken und **Alle ersetzen** drücken. Je Abschnitt entsteht eine neue Fassung.
4. Danach steht das Ergebnis da – und der Knopf **Rückgängig**.

- **Rückgängig** schaltet alle betroffenen Abschnitte auf ihre vorige Fassung zurück. Abschnitte, die danach
  schon wieder geändert wurden, bleiben und werden genannt. Das jüngste Ersetzen steht auch später noch unten auf
  der Seite mit „Rückgängig“.
- **Zitate und Code bleiben unberührt**, außer man wählt Zitate ausdrücklich. Code ersetzt Baruch nie.
- Nach dem Ersetzen erinnert Baruch daran, die Bibelzitate der geänderten Abschnitte zu prüfen.
- Ein geänderter Titel benennt den Gliederungspunkt um. Eine Ersetzung, die eine Überschrift anlegen oder ihre
  Ebene ändern würde, wird für diesen Abschnitt mit Grund übersprungen.
- **Regulärer Ausdruck:** Gruppen im Ersatz als `$1`.
- Grenzen: Ersatz höchstens 2000 Zeichen, höchstens 500 Abschnitte je Ersetzen.
- **Ersetzen kann nur der Mensch.** Assistenten können nur suchen. Für [Lektoren](/hilfe/konten/#lektoren) heißt
  der Knopf darum „Im Werk suchen“.

## Querverweise {#verweise}

{{< hilfebild name="verweise-seite" alt="Die Seite „Querverweise prüfen“: oben der Kasten „Kaputte Verweise“, hier ohne Fund, darunter „Mögliche Querverweise“; im Beispielwerk ist keine Nennung gefunden. Gibt es welche, stehen sie dort mit dem erkannten Ziel, einem Häkchen und dem Knopf „In Querverweise umwandeln“." titel="Kaputte Verweise finden und Nennungen in Verweise umwandeln." >}}

Ein **Querverweis** zeigt von einer Stelle auf einen anderen Gliederungspunkt („siehe die Einleitung, S. 12“). Im
PDF und im E-Book ist er anklickbar. Er bleibt richtig, auch wenn die Gliederung umgestellt wird.

**Verweis einfügen:**

1. Den Cursor an die Stelle setzen und **Einfügen → Verweis** drücken.
2. Im Fenster das Ziel suchen (Titel oder Nummer) und wählen.
3. Den Text des Verweises anpassen (Vorgabe: der Titel) und auf Wunsch **mit Seitenzahl** anhaken.
4. Einfügen.

Im Satzbild ist der Verweis ein blau gestrichelter Block; sein Text lässt sich dort ändern. Ein Klick darauf
zeigt „Öffnen | Bearbeiten“.

**Springen:**

- **Strg+Klick (Mac: ⌘-Klick)** auf einen Verweis springt zum Ziel.
- **Alt+← (Mac: ⌥←)** kehrt an die Ausgangsstelle zurück. Ohne vorherigen Sprung blättert Alt+← wie sonst zur
  vorigen Fassung.
- Unten in der Mitte zeigt eine kleine Leiste „← Zurück“ und, nach einem Zurück, „Vorwärts →“.

**Verweise prüfen** (**Prüfen → Querverweise prüfen**):

- **Kaputte Verweise** zeigen auf einen Punkt, den es nicht mehr gibt. Im Satz stehen sie nur als Text, und die
  Druckprüfung nennt sie.
- **Mögliche Querverweise** sind Nennungen im Fließtext: „die Einleitung“, „im dritten Kapitel“, „Kapitel III“,
  „Abschnitt 2.3“, ein Titel in Anführungszeichen. Sie zählen nur, wenn das Ziel eindeutig ist.
- Die Liste ist nur ein Vorschlag. Erst **Ja, umwandeln** und **In Querverweise umwandeln** setzen die
  angehakten Verweise ein – als neue Fassung.

## Glossar {#glossar}

{{< hilfebild name="glossar-seite" alt="Die Seite Glossar mit zwei erfundenen Begriffen: je Begriff die kurze Erklärung, Varianten und der Verweis „Bearbeiten oder löschen“; darunter gesondert die Vorschläge der Assistenten mit Übernehmen und Verwerfen und das Formular „Neuer Begriff“." titel="Das Glossar: Begriffe, Erklärungen und Vorschläge der Assistenten." >}}

Das **Glossar** erklärt Begriffe des Buchs: Begriff, kurze und ausführliche Erklärung, Varianten (Beugungen,
andere Formen), Quelle und Bibelstellen.

So geht es:

1. Im Text ein Wort markieren (oder den Cursor hineinsetzen).
2. Rechtsklick, die Kontextmenü-Taste oder Umschalt+F10 – oder **Einfügen → Register und Glossar**.
3. **In Glossar aufnehmen …** wählen, Erklärung eintragen, **Aufnehmen**. Der Text des Abschnitts ändert sich
   dabei nicht.

- **Im Glossar nachschlagen** im selben Menü zeigt einen Begriff, auch wenn im Text nur eine Variante steht.
- **Buch → Glossar** zeigt alle Begriffe mit **Bearbeiten oder löschen** und am Ende **Neuer Begriff**.
- **Vorschläge der Assistenten** stehen gesondert und gekennzeichnet; **Übernehmen** oder **Verwerfen**
  entscheidet. **Begriffe vorschlagen lassen** fragt den Lektor nach Begriffen, die erklärt werden sollten.
- Einen Begriff gibt es nur einmal (Groß- und Kleinschreibung zählen nicht). Bibelstellen werden beim Speichern
  geprüft; eine unbekannte Stelle wird abgelehnt.
- **Im Buch:** Unter **Einstellungen → Werk → Satz → Register und Glossar** wählt man, ob am Buchende eine
  **Glossarseite** steht (vor oder nach den Registern) und ob die **erste Erwähnung** eines Begriffs je Kapitel
  kursiv gesetzt wird – mit Glossarseite als Verweis auf den Eintrag.

## Register {#register}

{{< hilfebild name="register-seite" alt="Die Seite Register: je Register – Stichwort, Personen, Bibelstellen – die Einträge mit Unterstichwörtern und den Abschnitten, in denen sie stehen; darunter die Registervorschläge der Assistenten und „Register verwalten“." titel="Die Register eines Werks mit ihren Einträgen." >}}

Ein **Register** steht am Buchende und nennt zu jedem Stichwort die Seiten. Eingebaut sind
**Stichwortregister**, **Personenregister** und **Bibelstellenregister**; bis zu 13 eigene lassen sich anlegen.

So geht es:

1. Im Text ein Wort markieren.
2. Rechtsklick (oder **Einfügen → Register und Glossar**) und **In Register aufnehmen …** wählen.
3. Stichwort (vorbelegt), auf Wunsch Unterstichwort, Register, „siehe“ und „siehe auch“ eintragen.
4. Der Eintrag steht als kleines Zeichen ⁘ hinter dem Wort. **Speichern** legt die neue Fassung an.

- Ein Rechtsklick auf das Zeichen bietet **Registereintrag bearbeiten …** und **Registereintrag entfernen**.
- **Einfügen → Register** klappt die Liste aller Einträge des Abschnitts auf; ein Klick springt hin.
- Das **Bibelstellenregister** füllt sich von selbst aus allen erkannten Bibelstellen im Fließtext.
- **Buch → Register** zeigt je Register die Einträge, die offenen **Registervorschläge** der Assistenten
  (Übernehmen setzt den Eintrag als neue Fassung) und **Register verwalten** (eigene anlegen, umbenennen, löschen).
- **Im Buch:** Unter **Einstellungen → Werk → Satz → Register und Glossar** hakt man an, welche Register am
  Buchende erscheinen (in dieser Reihenfolge, in ein bis drei Spalten). Die Seiten sind deutsch sortiert (Ä bei
  A, ß wie ss), Folgeseiten stehen als „12–14“.
- Einträge stehen nie in Überschriften. Derselbe Eintrag steht nie zweimal hinter demselben Wort.

{{< hilfebild name="glossar-markierung" alt="Die Seite „An der Markierung“ für einen Abschnitt: ein Feld für das Wort, darunter die Knöpfe „In Glossar aufnehmen“, „In Register aufnehmen“ und „Im Glossar nachschlagen“ sowie die Registereinträge des Abschnitts." titel="Ohne JavaScript: Glossar und Register über die Seite „An der Markierung“." >}}

**Ohne JavaScript** führt **Markierung …** in der Statuszeile unter dem Blatt auf die Seite **An der
Markierung**: Wort eintragen, dann „In Glossar aufnehmen“, „In Register aufnehmen“ oder „Im Glossar nachschlagen“.
Darunter stehen die Registereinträge des Abschnitts mit „Registereintrag entfernen“.
