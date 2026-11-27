---
title: "Gliederung"
description: "Teile, Kapitel und Abschnitte anlegen, ordnen und nummerieren – und über Anträge der Assistenten entscheiden."
weight: 40
---

Die Gliederung ist die Folge der Überschriften eines Werks: **Teil**, **Kapitel** (bei Predigten
**Predigtteil**), **Abschnitt** und **Unterabschnitt**. Gliederung und Überschriften im Text sind immer gleich:
Wer in der Gliederung umbenennt, ändert die Überschrift im Text – und umgekehrt.

## Die Gliederung links auf der Arbeitsseite {#gliederung}

{{< hilfebild name="gliederung-leiste" alt="Die Gliederung links auf der Arbeitsseite: Teile, Kapitel und Abschnitte des Beispielwerks als eingerückte Liste mit Nummern; ein Abschnitt ist gewählt, im Kopf steht die Nummerierung." titel="Die Gliederung links: Hier wird angelegt, umbenannt, verschoben und gelöscht." >}}

Mit JavaScript geschieht alles direkt in der Liste links:

- **Öffnen:** Ein Klick auf einen Eintrag öffnet den Abschnitt.
- **Umbenennen:** Doppelklick auf den gewählten Eintrag oder **F2**. Enter bestätigt, Esc bricht ab.
- **Kontextmenü:** rechte Maustaste, Umschalt+F10, die Kontextmenü-Taste oder der Knopf **⋯** am Eintrag. Darin:
  **Umbenennen**, **Neuer Punkt davor**, **danach** oder **darunter** (Titel eintippen, Enter legt an), an Teilen
  und Kapiteln **Neuer Teil davor** bzw. **danach**, **Höher stufen**, **Tiefer stufen** und **Löschen…**.
- **Löschen:** Kontextmenü oder **Entf** (am Mac die Taste ⌦ bzw. Fn+Rückschritt). Baruch fragt nach („… mit
  allen Fassungen und Beiträgen löschen?“). Ein Punkt mit Unterpunkten und das letzte Kapitel lassen sich nicht
  löschen. Der gelöschte Text bleibt im Verlauf des Werks nachlesbar – samt allen übrigen Fassungen und
  Beiträgen.
- **Verschieben und Umstufen mit der Tastatur:** Alt+Umschalt+↑ und ↓ verschieben, Alt+Umschalt+← und → stufen
  höher bzw. tiefer (Mac: ⌥⇧ mit den Pfeiltasten). Unterpunkte wandern mit. Das wirkt überall, außer wenn man
  gerade in einem Eingabefeld schreibt.
- **Ziehen und Ablegen:** Auf das obere Drittel eines Eintrags ablegen heißt *davor*, auf die Mitte *darunter*,
  auf das untere Drittel *danach*. Beim Ziehen stehen Ziel und Ebene unter der Gliederung; ungültige Ziele werden
  rot markiert und abgewiesen.

F2, Entf und die Kontextmenü-Taste wirken nur, wenn die Gliederung den Fokus hat (vorher einen Eintrag
anklicken).

**Verschieben behält die Ebene:** Ein Kapitel bleibt Kapitel. Davor und danach geht nur neben einen Punkt
derselben Ebene, darunter nur unter einen Punkt genau eine Ebene höher. Sonst meldet Baruch „Verschieben behält
die Ebene …; erst umstufen“.

**Regeln für Titel:** Ein Titel darf nicht leer sein, keinen Zeilenumbruch enthalten und höchstens 200 Zeichen
lang sein. Das gilt überall, wo ein Titel entsteht oder sich ändert – auch bei Vorschlägen der Assistenten. Ein
alter Punkt ohne Titel oder mit überlangem Titel bleibt, wie er ist, bis man ihn umbenennt.

**Ungespeicherter Text:** Steht im Editor ungespeicherter Text, fragt Baruch vor jeder Änderung der Gliederung
„Ungespeicherte Änderung verwerfen?“. „Abbrechen“ ändert nichts.

### Überschriften im Text {#ueberschriften}

Speichern liest die Überschrift des Abschnitts:

- Ein neuer Titel oder eine andere Ebene passt die Gliederung an (Unterpunkte wandern mit).
- Eine **zusätzliche Überschrift** teilt den Abschnitt, eine **entfernte Überschrift** hängt den Text an den
  vorigen Punkt (Beiträge wandern mit) – beides erst nach einer Rückfrage („Teilen und speichern“ bzw.
  „Zusammenführen und speichern“).
- Text vor der ersten Überschrift wird abgelehnt.

Überschriften lassen sich auch wie in Markdown tippen: `# Titel` am Zeilenanfang wird ein Kapitel, `## Titel`
ein Abschnitt, `### Titel` ein Unterabschnitt. Mehr Rauten werden ebenfalls ein Unterabschnitt, weil die
Gliederung nur drei Ebenen kennt. Im Quelltext wandelt der Editor die Zeile beim Enter um, im Satzbild wird der
Absatz beim Enter zur Überschrift; ohne JavaScript geschieht es beim Speichern.

**Fassungen bringen ihre Überschrift mit:** Schaltet man auf eine andere Fassung um oder nimmt man einen
Vorschlag an, ändert Baruch deren Text nie. Die Gliederung folgt stattdessen der Überschrift dieser Fassung: Ein
anderer Titel benennt den Punkt um; eine andere Ebene stuft ihn um, und alle Unterpunkte wandern mit.
Zurückschalten stellt die alte Gliederung wieder her. Geht es nicht (zum Beispiel, weil ein Unterpunkt tiefer als
die dritte Ebene geriete), lehnt Baruch mit einer Meldung ab, und nichts ändert sich.

**Mehrere Fenster:** Hat ein anderes Fenster die Gliederung inzwischen umgestellt, landet Speichern, Umschalten
oder Löschen trotzdem im gezeigten Abschnitt. Ist er gelöscht, meldet Baruch „Diesen Gliederungspunkt gibt es
nicht mehr …“.

### Teile {#teile}

Einen **Teil** legt man vor oder hinter einem Kapitel oder Teil an (Kontextmenü **Neuer Teil davor/danach**),
auch den ersten Teil vor dem ersten Kapitel. Alle folgenden Kapitel bis zum nächsten Teil stehen danach unter
ihm; ihre Texte und Nummern bleiben. Kapitel zählen über Teile hinweg weiter, Teile zählen für sich (I, II, …).

- Ein Kapitel lässt sich nicht zum Teil umstufen und ein Teil nicht zum Kapitel.
- Der Text eines Teils trägt genau eine Teilüberschrift und keine weitere; Kapitel legt man in der Gliederung
  an.
- Im Buch bekommt jeder Teil seine eigene Seite.

### Kapitel ohne Nummer {#ohne-nummer}

Vorwort, Einleitung oder Schlussgebet sollen oft nicht mitzählen. Auf der Seite **Gliederung** (siehe unten) hat
jedes Kapitel, jeder Abschnitt und Unterabschnitt den Knopf **ohne Nummer** bzw. **mit Nummer**. Ein Punkt ohne
Nummer zählt nicht mit; die übrigen rücken nach. Teile zählen immer.

## Die Seite Gliederung {#seite}

{{< hilfebild name="gliederung-seite" alt="Die Seite Gliederung: alle Punkte des Beispielwerks als Tabelle mit Nummer, Ebene und Titel, je Zeile die Knöpfe zum Verschieben, Umstufen, für „ohne Nummer“ und zum Bearbeiten; darüber die Nummerierung." titel="Die Seite Gliederung zeigt alles auf einmal – und geht ohne JavaScript." >}}

**Buch → Gliederung** zeigt alle Punkte mit Nummer und Ebene auf einer Seite. Sie ist der Weg ohne JavaScript –
und bequem, wenn man viel umstellt.

- Je Eintrag: **↑** (vor den vorigen Punkt), **↓** (hinter den nächsten Punkt derselben Ebene), **←** und **→**
  (höher bzw. tiefer stufen), **ohne Nummer / mit Nummer** und **Bearbeiten**.
- Bei Predigten und bei Werken mit Zielzeit steht die Spalte **Dauer**: je Punkt die Sprechdauer und die
  Zeitmarke „ab m:ss“; darüber Gesamtzeit, Zielzeit und – wenn überschritten – die Warnung als Text. Siehe
  [Zeitwächter](/hilfe/schreiben/#zeitwaechter).

{{< hilfebild name="gliederung-bearbeiten" alt="Die Seite „Gliederungspunkt bearbeiten“ mit Formularen zum Umbenennen, zur Wahl der Ebene, zum Verschieben, zum Anlegen eines neuen Punkts davor, danach oder darunter und zum Löschen mit Kästchen." titel="Einen Punkt bearbeiten: umbenennen, verschieben, umstufen, anlegen, löschen." >}}

**Bearbeiten** an einem Eintrag öffnet eine Seite mit Formularen: umbenennen, Ebene wählen, verschieben, einen
neuen Punkt davor, danach oder darunter anlegen (an Teilen und Kapiteln mit „Ebene: Teil“ auch einen neuen Teil)
und löschen (mit Kästchen zur Bestätigung).

Ohne JavaScript stehen dieselben Knöpfe ↑ ↓ ← → auch an jedem Eintrag der Gliederung links, dazu die Verweise
**bearbeiten** über der Gliederung und **Bearbeiten** am Eintrag.

## Anträge der Assistenten {#antraege}

{{< hilfebild name="gliederung-antraege" alt="Der Abschnitt „Anträge der Assistenten“ auf der Seite Gliederung: ein offener Antrag mit Nummer, dem Wunsch, einen Abschnitt umzubenennen, dem Antragsteller, der Begründung und den Knöpfen Bestätigen und Ablehnen." titel="Ein Assistent beantragt – der Mensch entscheidet." >}}

Assistenten dürfen neue Punkte **anlegen**. Alles andere – umbenennen, verschieben, umstufen, mit oder ohne
Nummer zählen, löschen – können sie nur **beantragen**. Bis zur Entscheidung ändert sich nichts.

So geht es:

1. Offene Anträge meldet der Kopf der Gliederung mit **„N Anträge zur Gliederung“**; auch am Reiter **Buch**
   steht ihre Zahl. Der Verweis führt zum Abschnitt **Anträge der Assistenten** auf der Seite Gliederung.
2. Je Antrag stehen dort Nummer, was er will (zum Beispiel „„Sommer“ in „Hochsommer“ umbenennen“), wer ihn
   stellt und seine Begründung.
3. **Vorschau** aufklappen: Sie zeigt die Gliederung, wie sie nach dem Bestätigen wäre.
4. **Bestätigen** führt den Antrag aus, **Ablehnen** schließt ihn; die Gliederung bleibt dann unverändert.

{{< hilfebild name="gliederung-antrag-vorschau" alt="Die Vorschau eines Antrags: die Gliederung nach dem Bestätigen, mit Art und Nummer je Zeile; der geänderte Punkt trägt die Marke „umbenannt“ und den Vermerk vorher und nachher." titel="Die Vorschau zeigt, was der Antrag ändern würde – sie ändert selbst nichts." >}}

- In der **Vorschau** sind geänderte Punkte markiert („umbenannt“, „verschoben“, „neue Ebene“, „mit/ohne
  Nummer“); ein gelöschter steht durchgestrichen an seiner alten Stelle („entfällt“). Jede Zeile nennt Art und
  Nummer („Teil I“, „Kapitel 2“, „Kapitel (ohne Nummer)“), damit ein Teil und ein Kapitel mit derselben Zahl
  nicht zu verwechseln sind. Ändert sich Art oder Nummer eines Punkts, steht „(vorher … → nachher …)“ dabei.
- Hat sich der Punkt seit dem Antrag geändert (gelöscht, anderer Titel, andere Ebene), wird beim Bestätigen
  nichts ausgeführt; der Antrag gilt als **veraltet**, mit Grund. Die Vorschau sagt dann „Keine Vorschau: …“.
- Jede Entscheidung wird im Verlauf des Werks festgehalten – auch ein abgelehnter oder veralteter Antrag, mit
  Antragsteller, Begründung und Entscheider.
- **Zuletzt entschieden** klappt die zehn letzten Entscheidungen auf.
- Gibt es sehr viele offene Anträge, rechnet die Seite die Vorschau sofort nur für die ersten 20; bei den übrigen
  steht der Verweis „Vorschau anzeigen“.
- Was schon beim Antrag nicht ginge (ungültiger Titel, Löschen mit Unterpunkten, keine Wirkung), nimmt Baruch gar
  nicht erst an; ein gleicher offener Antrag wird nicht doppelt gestellt.

### Zustimmung zu einem neuen Teil {#zustimmung-teil}

Legt ein Assistent einen **Teil** so an, dass vorhandene Kapitel unter ihn wandern würden (zum Beispiel
„Teil II“ vor Kapitel 5), legt Baruch ihn nicht an, sondern stellt einen Antrag, dem der Mensch **aktiv
zustimmen** muss. Ohne Klick geschieht nichts – auch nicht nach einer Wartezeit.

- **Mit JavaScript** öffnet sich auf jeder Seite das Fenster **„Zustimmung zur Gliederung“**: wer den Teil
  möchte, in welchem Werk, die Begründung und die Gliederung danach. **Zustimmen** legt den Teil an (die Kapitel
  behalten Text und Nummer), **Ablehnen** lässt alles, wie es ist. **Später entscheiden** (oder Esc) schließt nur
  das Fenster; es erscheint auf der nächsten Seite wieder.
- **Immer** (auch ohne JavaScript) steht unter dem Menüband eine gelbe **Hinweisleiste** „Zustimmung nötig“ mit
  dem Verweis **Ansehen und entscheiden**.
- Hat sich die Gliederung inzwischen so geändert, dass der Teil andere Kapitel übernähme, wird nichts ausgeführt;
  der Antrag gilt als veraltet.
- Ein Teil, der keine vorhandenen Kapitel übernimmt (zum Beispiel am Ende des Werks), entsteht wie jeder neue
  Punkt eines Assistenten sofort.

Dasselbe gilt, wenn eine [Austauschdatei](/hilfe/austausch-sicherung/#einlesen) einen solchen Teil mitbringt.

## Gliederungsschema und Nummern {#schema}

{{< hilfebild name="gliederung-schema" alt="Die Seite Gliederungsschema: die Auswahl der Schemata, je mit Beispiel, und rechts die Vorschau an den ersten zwölf Punkten der eigenen Gliederung; darunter die Knöpfe Vorschau und Übernehmen." titel="Ein Schema wählen und an der eigenen Gliederung ansehen." >}}

Wie die Punkte gezählt werden, bestimmt das **Gliederungsschema**.

So geht es:

1. **Buch → Gliederungsschema** wählen (oder **Schema wählen…** im Kopf der Gliederung).
2. Ein Schema anklicken. **Vorschau** zeigt es an den ersten zwölf Punkten der eigenen Gliederung, ohne etwas zu
   ändern.
3. **Übernehmen** stellt es ein; alle Nummern werden neu berechnet.

| Schema | Nummern |
|---|---|
| Aus der Werkvorlage (Vorgabe für neue und eingelesene Werke) | wie die Vorlage es festlegt, sonst 1 · 1.1 |
| Dezimal (DIN 1421) | 1 · 1.1 · 1.1.1 |
| Dezimal mit Schlusspunkt | 1. · 1.1. · 1.1.1. |
| Alphanumerisch | I. · A. · 1. · a) · aa) |
| Nur Kapitel römisch | I · II · III, Abschnitte ohne Nummer |
| Nur Kapitel arabisch | 1 · 2 · 3, Abschnitte ohne Nummer |
| Teile und Kapitel | Teil I · Kapitel 1 · 1.1 · 1.1.1 |
| Kapitel ausgeschrieben | Erster Teil · Erstes Kapitel · Zweites Kapitel (bis 99 in Worten, dann „100. Kapitel“), Abschnitte ohne |
| Juristisch | § 1 · Abs. 1 · Nr. 1, Teile ohne |
| Predigt | erstes Kapitel „Einleitung“, letztes „Schluss“, dazwischen I. · II. · III.; Abschnitte und Teile ohne |
| Buch | Kapitel wie „Predigt“, Abschnitte ohne; Teile „Erster Teil“, „Zweiter Teil“ … |
| Ohne Nummern | – |
| Eigenes Muster | siehe unten |

- Teile zählen römisch (I, II), bei „Teile und Kapitel“ als „Teil I“, bei „Kapitel ausgeschrieben“ und „Buch“
  als „Erster Teil“, bei „Ohne Nummern“, „Juristisch“ und „Predigt“ gar nicht.
- Bei „Predigt“ und „Buch“ zählen nur Kapitel mit Nummer; ist es nur eines, heißt es „Einleitung“.
- **Predigt mit Teilen:** Soll eine Predigt Teile bekommen, auf „Buch“ umstellen – die Kapitel behalten ihre
  Nummern, die Teile heißen „Erster Teil“, „Zweiter Teil“. Zurück geht jederzeit. Ob ein Werk ein Buch oder eine
  Predigt ist, hängt nicht am Schema.
- Das Schema gilt überall gleich: in der Gliederung, im Kopf des Editors, im Satz, im PDF und im Export.
- Im **Kopf der Gliederung** steht „Nummern: …“ mit dem Namen des geltenden Schemas. Aufgeklappt lässt sich dort
  auch „aus der Vorlage“, „keine“ oder ein eigenes Muster einstellen.

### Eigenes Muster {#muster}

Im Feld „eigenes Muster“ beschreibt man die Zählung selbst:

| Muster | Bedeutung |
|---|---|
| `1.1`, `I.1`, `1.a)` | ein Muster für alle Ebenen |
| `I\|1` | nur die Ebenen bis zur genannten Zahl bekommen Nummern (hier: nur Kapitel, römisch); `1.1\|2`: Unterabschnitte ohne Nummer |
| `I.;A.;1.;a);aa)` | je Ebene ein eigenes Muster, getrennt durch `;` – ein leeres heißt „ohne Nummer“ |

Zählzeichen sind `1`, `a`, `A`, `i`, `I` und `*`; `aa` bzw. `AA` verdoppelt den Buchstaben (aa, bb, cc …). Jedes
nichtleere Muster braucht ein Zählzeichen. Ein ungültiges Muster lehnt Baruch mit Grund ab. Ein eigenes Muster
bleibt gemerkt, wenn man auf ein anderes Schema wechselt.
