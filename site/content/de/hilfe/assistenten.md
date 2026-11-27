---
title: "Assistenten, Chat und Befunde"
description: "Assistenten prüfen und vorschlagen lassen, im Chat am Abschnitt arbeiten und über ihre Befunde und Vorschläge entscheiden."
weight: 60
---

Ein **Assistent** ist eine Rolle, die ein Sprachmodell übernimmt: der Lektor, der Exeget, der kritische Prüfer
„Nathan“, ein Hörer aus der Zielgruppe und andere. In der Oberfläche heißen diese Rollen an manchen Stellen
„Persona“. Jede hat einen Namen, ein Kürzel zum Ansprechen (`@lektor`), eine Farbe und eine Aufgabe.

Zwei Grundsätze gelten überall:

- **Assistenten ändern nie den gültigen Text.** Antworten werden Beiträge im Chat, neue Texte nur Vorschläge.
  Annehmen kann nur der Mensch.
- **Baruch bringt kein Sprachmodell mit.** Welcher Dienst die Assistenten antreibt, stellt man einmal ein:
  [Assistenten einrichten](/mcp/#assistenten-einrichten). Solange nichts eingerichtet ist, meldet Baruch das
  ehrlich; Schreiben, Fassungen und Buchsatz gehen trotzdem.

## Prüfen und Vorschlag {#pruefen}

{{< hilfebild name="assistenten-leiste" alt="Die rechte Spalte mit dem aufgeklappten Bereich „Assistenten“: die Liste der Rollen in ihren Farben, der Lektor ist markiert, je Rolle der Vermerk lokal oder extern; darunter ein Feld für einen Hinweis, die Wahl Abschnitt oder ganzes Kapitel, die Prüfgrundlage und die Knöpfe Prüfen und Vorschlag." titel="Der Bereich „Assistenten“: Rolle wählen, dann Prüfen oder Vorschlag." >}}

So geht es:

1. Den Abschnitt öffnen und rechts den Bereich **Assistenten** aufklappen.
2. Eine Rolle wählen (Vorgabe: der Lektor). Auf Wunsch einen Hinweis dazuschreiben (höchstens 4000 Zeichen).
3. **Umfang** wählen: **Abschnitt** (Vorgabe) oder **ganzes Kapitel**.
4. **Prüfgrundlage** wählen: die **gezeigte Fassung** (Vorgabe), die **aktuelle Fassung** oder einen der offenen
   Vorschläge des Abschnitts.
5. **Prüfen** oder **Vorschlag** drücken (dieselben Knöpfe stehen im Menüband, Reiter **Start**, Gruppe
   „Assistenten“).

- **Prüfen:** Der Assistent liest den Abschnitt und schreibt seine Befunde als Beitrag in den Chat des Abschnitts.
- **Vorschlag:** Der Assistent schreibt eine neue Fassung des ganzen Abschnitts als Vorschlag. Die Überschrift
  bleibt immer erhalten. Gleicht der Vorschlag der aktuellen Fassung, wird nichts angelegt, und die Seite sagt es.
- **Ganzes Kapitel prüfen:** Der Assistent prüft alle Teile des Kapitels (bei einer Predigt die ganze Predigt) und
  nennt zu jedem Befund den Teil.
- **Vorschlag für das ganze Kapitel:** Der Assistent schreibt nacheinander für jeden Teil des Kapitels eine neue
  Fassung. Jeder geänderte Teil bekommt einen eigenen Vorschlag; angenommen wird Teil für Teil. Das dauert
  entsprechend länger.

**Während der Assistent schreibt** (mit JavaScript) erscheint die Antwort Wort für Wort in einem vorläufigen
Beitrag mit gestricheltem Rand und dem Knopf **Abbrechen**. Nach dem Abbrechen ist bei Prüfen und Vorschlag
nichts gespeichert. Ohne JavaScript lädt die Seite nach der fertigen Antwort neu; das dauert so lange, wie das
Modell braucht, und ein Fortschritt ist dann nicht sichtbar.

**Was ein Assistent zu sehen bekommt:** das ganze Kapitel im Wortlaut, die Befunde der anderen Assistenten zu
diesem Kapitel, der Lektor zusätzlich die [Einfälle](/hilfe/einfaelle/) zum Abschnitt. Der Kopf jedes Beitrags
nennt, was geprüft wurde – etwa „geprüft: aktuelle Fassung 1“ oder „geprüft: Fassung 2 (Vorschlag von @lektor)“.
Das schreibt Baruch, nicht das Modell.

**Wenn das Kapitel zu lang ist,** kürzt Baruch nie still. Erst werden Zusätze knapper (der Beitrag endet dann
mit „(Kontext gekürzt)“). Reicht das nicht, lässt Baruch entferntere Teile zusammenfassen und teilt einen zu
langen Abschnitt in überlappende Stücke; der Beitrag endet dann mit einem Absatz „Komprimiert: …“, der nennt, was
der Assistent nur als Zusammenfassung gesehen hat. Reicht auch das nicht, lehnt Baruch ehrlich ab und speichert
nichts. Abhilfe: einen einzelnen Abschnitt prüfen lassen.

**Regeln, die Baruch selbst durchsetzt:**

- Antworten sind deutsch und beginnen nicht mit Floskeln wie „Gerne“ oder „Als KI“. Beginnt eine Antwort doch
  so, fordert Baruch einmal neu an.
- Ein kritischer Befund braucht ein wörtliches Zitat aus dem Text. Steht das Zitat dort nicht, hängt Baruch
  sichtbar **„(nicht im Text belegt)“** an.
- Zu jedem kritischen Punkt gehört eine **Lösung**: „statt: … – besser: …“ oder ein klarer nächster Schritt.
  Fehlt sie auch nach einmaligem Nachfordern, steht „(ohne Lösungsvorschlag)“ dahinter.
- Der **Lektor** ist Begleiter: Er sagt zuerst, was trägt, dann höchstens zwei Vorschläge mit einem nächsten
  Schritt. Der Prüfer **Nathan** muss mindestens einen kritischen Befund nennen und gibt nie frei.
- Worüber der Mensch schon entschieden hat, wird den Assistenten als „Entscheidungen des Autors – nicht erneut
  melden“ mitgegeben.
- Was andere Assistenten geschrieben haben, bekommt ein Assistent als Daten zu sehen, nicht als Anweisung.
- Wird eine Antwort an der Längengrenze abgeschnitten, steht das am Ende des Beitrags; ein abgeschnittener
  Vorschlag wird nicht angelegt.
- Es läuft immer nur eine Anfrage zur Zeit; bis zu acht warten, darüber hinaus meldet Baruch „Warteschlange
  voll“.
- Fehler (Dienst nicht erreichbar, Zeitüberschreitung, unbekanntes Modell) meldet die Seite ehrlich; gespeichert
  ist dann nichts.

**Lokal oder extern:** Je Rolle steht dabei, ob ihr Dienst auf dem eigenen Rechner läuft („lokal“) oder
außerhalb („extern“). Eine Rolle mit externem Dienst **ohne Freigabe** ist als „Gesperrt“ gekennzeichnet und
steht nicht zur Wahl – Buchtext geht ohne Freigabe nicht hinaus. Die Freigabe erteilt man unter
[Einstellungen → Assistenten](/hilfe/einstellungen/#assistenten). Dort legt man auch fest, welche Rolle der
**Lektor** ist: fest eine bestimmte oder automatisch die mit dem größten Fassungsvermögen.

**Belege aus Bibel, Kommentaren und Lexika:** Sind für das Werk Bibel, Kommentare oder Lexika gewählt und ist der
Schalter der Rolle an, bekommt der Assistent nummerierte Belege [S1], [S2] … mit. Jeder zitierte Beleg steht
unter dem Beitrag als Quelle. Nennt die Antwort einen Beleg, den es nicht gab, steht „(unbekannte Quelle)“
dahinter.

{{< hilfebild name="assistenten-vorschlaege" alt="Offene Vorschläge an einem Abschnitt: zwei farbig markierte Vorschläge verschiedener Assistenten mit Notiz, Unterschied zur gezeigten Fassung und den Knöpfen Übernehmen und Verwerfen." titel="Offene Vorschläge warten am Abschnitt auf die Entscheidung." >}}

Wie man Vorschläge annimmt oder ablehnt, steht unter
[Schreiben, Bilder und Fassungen](/hilfe/schreiben/#vorschlaege).

## Der Chat am Abschnitt {#chat}

{{< hilfebild name="assistenten-chat" alt="Der Chat am Abschnitt in der rechten Spalte: Beiträge der Autorin und zweier Assistenten in ihren Farben, je Beitrag die kleinen Knöpfe Kopieren, Zitieren, Wiederholen und Löschen; unten das Eingabefeld mit dem Platzhalter „@persona fragen …“." titel="Der Chat gehört zum gezeigten Abschnitt." >}}

Rechts im Bereich **Chat** stehen die Beiträge zum gezeigten Abschnitt. Der Kopf zeigt ihre Zahl, auch
zugeklappt; mit JavaScript kommt „· k neu“ dazu, und neue fremde Beiträge haben links einen farbigen Strich.

So geht es:

1. Unten in das Feld **„@persona fragen …“** schreiben.
2. Einen Assistenten ansprechen: mit `@lektor` irgendwo in der Nachricht oder mit dem Namen am Satzanfang, gefolgt
   von Komma oder Doppelpunkt („Lektor, was meinst du?“). Im Fließtext ruft der Name niemanden.
3. Absenden. Die Nachricht steht sofort im Verlauf; jeder angesprochene aktive Assistent antwortet mit einem
   eigenen Beitrag in seiner Farbe, einer nach dem anderen (höchstens drei).

Ohne Anrede bleibt die Nachricht ein gewöhnlicher Beitrag – eine Notiz am Abschnitt.

- **Vorschlagsliste:** Nach „@“ erscheint die Liste der aktiven Assistenten; weitere Buchstaben filtern. ↑ und ↓
  wählen, Tab oder Enter übernimmt, Esc schließt.
- **RFC und @alle:** Beginnt die Nachricht mit „RFC“ oder nennt sie „@alle“, meldet sich jeder aktive Assistent
  genau einmal. Der Lektor antwortet zuletzt: Er gewichtet die Befunde der anderen in „Jetzt wichtig:“ und „Kann
  warten:“ und schließt mit „Mein Vorschlag:“ – daraus macht Baruch einen Sammel-Vorschlag.
- **Schreibauftrag:** Bittet man um Text („@lektor schreibe den Schluss kürzer“) oder kreuzt man **als Auftrag**
  an, schreibt der Assistent einen **Vorschlag** des Abschnitts und meldet sich kurz im Chat. Mit „als Auftrag“
  ohne Anrede schreibt der Lektor. Ein Schreibauftrag löscht keine Abschnitte und legt keine an. Nennt der Auftrag
  Befunde wie „[B12]“, schlägt Baruch für sie „übernommen“ vor; bestätigen kann nur der Mensch. Fragen („Wie
  wirkt der Anfang?“) bleiben Fragen.
- **Assistenten sprechen einander an:** Beginnt eine Zeile einer Antwort mit der Anrede eines anderen
  Assistenten, antwortet dieser einmal darauf; sein Beitrag zeigt „antwortet auf Beitrag n von …“. Das geht nur
  eine Stufe weit und höchstens dreimal je Nachricht.
- **Abbrechen** beendet die laufende Antwort; die eigene Nachricht und schon fertige Antworten bleiben.
- **Live:** Solange der Chat offen ist, erscheinen neue Beiträge ohne Neuladen; über dem Eingabefeld steht
  „@lektor schreibt …“, solange ein Auftrag läuft.

**Je Beitrag:**

- **Kopieren** legt den Text in die Zwischenablage; **Zitieren** setzt ihn als Zitat an den Anfang des
  Eingabefelds. Zitierter Text ruft niemanden; es zählt nur, was man darunter selbst schreibt.
- **↻ Wiederholen** stellt dieselbe Anfrage noch einmal, und zwar an die jetzt gezeigte Fassung. Die alte Antwort
  bleibt stehen; die neue trägt „Wiederholung von Beitrag n nach Fassung m“. Befunde der alten Antwort, die in der
  neuen nicht mehr vorkommen, schlägt der Assistent als „erledigt“ vor.
- **✕ Löschen** entfernt den Beitrag nach Rückfrage. Der gelöschte Text bleibt im Verlauf des Werks nachlesbar.
- **Verwalten:** eigene Beiträge **bearbeiten** (die alte Fassung bleibt erhalten, der Beitrag trägt
  „bearbeitet“), **Verschieben…** in den Chat eines anderen Abschnitts, **In Einfall umwandeln** (Schlagwörter
  sind Pflicht; danach ist der Beitrag ein Einfall) und, bei Antworten der Assistenten, **In Befunde umwandeln**.

**Gegliederte Beiträge:** Baruch zeigt Absätze, Listen, Fettes, Zitate und Zwischenüberschriften der Antwort
geordnet an. Verweise im Beitrag baut nur Baruch, und nur geprüfte: auf Stellen im Text (wörtliche Zitate ab
12 Zeichen, eindeutig genannte Gliederungspunkte), auf Bibelstellen („Ps 23,1“ öffnet die Bibel des Werks), auf
„Einfall 7“ und „Befund 12“. Ein Klick springt an die Stelle und markiert sie; **Alt+← (Mac: ⌥←)** oder der
Knopf „Zurück zum Chat“ führt zurück.

{{< hilfebild name="assistenten-beitraege" alt="Die Seite „Ganze Chat-Historie“: alle Beiträge eines Abschnitts untereinander, der neueste unten, je Beitrag Autor, Zeit und die Knöpfe zum Verwalten." titel="Die ganze Chat-Historie auf einer Seite." >}}

**Blättern im Chat:** Der Chat hat eine begrenzte Höhe und rollt in sich; beim Öffnen steht er am neuesten Beitrag.
Gezeigt werden die neuesten 50 Beiträge; „Ältere Beiträge (n) …“ lädt frühere nach. Ohne JavaScript führt
derselbe Verweis auf die Seite **Ganze Chat-Historie**; „Beitrag öffnen“ zeigt dort einen einzelnen Beitrag zum
Markieren, Zitieren, Bearbeiten und Löschen.

**Was die Assistenten im Chat mitbekommen** stellt man unter
[Einstellungen → Assistenten](/hilfe/einstellungen/#assistenten) ein: das ganze Kapitel (Vorgabe) oder nur den
gezeigten Abschnitt („Chat nur Abschnitt“ – spart Zeit bei langen Kapiteln).

## Befunde {#befunde}

{{< hilfebild name="assistenten-befunde-liste" alt="Die Seite „Befunde“ des Beispielwerks: oben die Filter, darunter die Tabelle Kapitel mal Assistent mit Zeichen und Zahlen je Status, darunter die Liste der offenen Befunde mit Art, Zitat, Lösung und dem Aufklapper „Status setzen“." titel="Alle Befunde eines Werks: Tabelle, Filter und Listen." >}}

Aus jeder Antwort eines Assistenten macht Baruch **Befunde**: je Punkt einen, mit **Art** (Lob, Hinweis,
kritisch) und **Status** (offen, übernommen, verworfen, verschoben, erledigt ohne Änderung). Der Beitrag selbst
bleibt unverändert im Chat stehen. **Den Status setzt nur der Mensch;** Assistenten können ihn nur vorschlagen.

So geht es:

1. **Prüfen → Befunde** öffnen. Am Reiter steht „N offen“.
2. In der Tabelle eine Zelle anklicken; darunter stehen genau deren Befunde.
3. Bei einem Befund **Status setzen …** aufklappen und entscheiden: **Übernommen** (Fassung wählen),
   **Verwerfen** (mit Begründung), **Verschieben** (in einen anderen Abschnitt oder einen Einfall) oder
   **Erledigt**. Ein entschiedener Befund lässt sich wieder öffnen.

- **Die Tabelle** hat je Kapitel eine Zeile und je Assistent eine Spalte. Jede Zelle nennt Zeichen und Zahl je
  Status – ○ offen, ✓ übernommen, ✗ verworfen, → verschoben, = erledigt ohne Änderung – und ist nach der Zahl
  offener Befunde eingefärbt; ein Kapitel mit drei oder mehr offenen trägt „viele offen“ als Text. Mit der
  Tastatur wandern die Pfeiltasten von Zelle zu Zelle, Enter öffnet die Zelle.
- **Filter:** Status, Art, Assistent, Zeitraum und „älter als … Tage“, dazu „Lob zeigen“. Lob wird gespeichert,
  ist aber in der Vorgabe ausgeblendet und zählt nie als offen.
- **Listen:** „Offen, älter als 14 Tage“ (Zahl einstellbar), die Sammelvorschläge des Lektors und der Verlauf je
  Kapitel (was in welche Fassung eingeflossen ist).
- **Ein Befund** (Verweis „[B12]“) zeigt Art, Status in Worten, Assistent, Abschnitt, Zitat, Begründung, seine
  **Lösungen** und den Verlauf aller Änderungen. Das Zitat und der Verweis **zur Stelle** öffnen den Abschnitt
  und markieren das Zitat.
- **Was ein Befund wird:** jeder Aufzählungspunkt der Antwort, jede Zeile, die mit „Kritisch:“, „Vorschlag:“,
  „Hinweis:“ oder „Lob:“ beginnt, und jeder Absatz unter „Was trägt“, „Was fehlt“, „Was stört“. Plaudern ohne
  Aufzählung („Danke, das klingt gut!“) wird kein Befund.
- **Doppeltes wird angehängt:** Meldet ein Assistent etwas, das einem schon entschiedenen Befund desselben
  Abschnitts gleicht, wird es dort angehängt und zählt nicht als offen.

{{< hilfebild name="assistenten-befunde-abschnitt" alt="Die rechte Spalte der Arbeitsseite mit dem aufgeklappten Bereich „Befunde“: die offenen Befunde des gezeigten Abschnitts mit Art, Zitat, Lösung, dem Häkchen „Als Vorschlag übernehmen“ und dem Aufklapper „Status setzen“." titel="Die Befunde des gezeigten Abschnitts stehen auch rechts neben dem Text." >}}

**Auf der Arbeitsseite:**

- Die Gliederung zeigt je Punkt „k offen“; Kapitel zählen ihre Abschnitte mit. Dieselbe Zahl steht im Kopf des
  Abschnitts.
- Der Bereich **Befunde** rechts zeigt die offenen Befunde des Abschnitts mit denselben Formularen; entschiedene
  und Lob sind eingeklappt.
- In der Baumansicht der Fassungen steht an einer Fassung „✓ setzt Befund [B4], [B7] um“.

### Lösungen übernehmen und Varianten {#loesungen}

- **Als Vorschlag übernehmen:** Eine Lösung mit neuer Formulierung, deren Zitat in der gezeigten Fassung steht,
  hat ein Häkchen; mehrere sind zugleich wählbar. Der Knopf setzt die Formulierungen ein und legt daraus eine
  **neue Fassung als Vorschlag** an. Annehmen der Fassung und Bestätigen der Befunde bleiben beim Menschen.
- **Varianten:** **Lösungen umsetzen** (Auswahl 2 oder 3) oder eine Chat-Nachricht mit dem Wort „Varianten“ lässt
  den Lektor zwei oder drei Varianten schreiben – jede mit dem Etikett „Variante A/B/C“ und einer Zeile, was sie
  anders macht. **vergleichen** stellt sie nebeneinander. **Übrige verwerfen** (mit Häkchen) verwirft die anderen
  Varianten mit einem Klick.

### Rückfrage: Befund erledigt? {#rueckfrage}

Prüft ein Assistent einen Abschnitt erneut (Prüfen, ↻, RFC oder ein Prüfauftrag im Chat), bekommt er seine
eigenen noch offenen Befunde mit. Hält er einen für umgesetzt, wiederholt er ihn nicht, sondern meldet „Erledigt:
Befund 12“. Daraus wird eine **Rückfrage** am Befund: „Befund 12 scheint erledigt – als erledigt markieren?“ mit
den Knöpfen **Ja, erledigt** und **Nein, noch offen**. Sie steht im Bereich „Befunde“, auf der Seite „Befunde“
und am Beitrag im Chat.

Eine schlichte Frage („@nathan wie breit ist der Fluss?“) ist kein Prüfauftrag; der Assistent bekommt seine
Befunde dann nicht mit.

**So läuft ein Durchgang Kritik:** Ein Prüfer (etwa Nathan) prüft die Fassung. Der Lektor schreibt aus den Befunden
einen Vorschlag. Der Prüfer prüft **diesen Vorschlag** (Prüfgrundlage „Vorschlag n von @lektor“): Erledigtes
meldet er als erledigt, anderes bleibt offen. Das geht so weiter, bis der Mensch eine Fassung annimmt.

## Websuche für Belege {#websuche}

{{< hilfebild name="assistenten-websuche" alt="Die Seite „Belege der Websuche“: Solange keine Websuche gelaufen ist, steht dort ehrlich der Hinweis, dass es noch keine Belege gibt; sonst je Beleg Titel, Adresse, Abrufdatum, Zitat und das Ergebnis der Prüfung." titel="Die Belege der Websuche – oder ihr ehrlicher Leerstand." >}}

Der Assistent **@belege** kann auf Wunsch im Internet recherchieren – über MultiGPT, den eigenen Dokumentendienst
(siehe [MultiGPT einrichten](/mcp/#multigpt-einrichten)). Der Schalter steht unter **Einstellungen → Assistenten**
bei @belege („im Web recherchieren“); **Vorgabe: aus**. Ist er aus, geht keine einzige Anfrage hinaus. Andere
Assistenten suchen nie im Web.

- **Achtung:** Die Suchanfrage ist die Frage bzw. der Anfang des Abschnitts (höchstens 200 Zeichen). Dieser
  unveröffentlichte Text geht an die Suchmaschine.
- Zuerst sucht Baruch bei den bevorzugten Quellen (Einstellungen → Quellen), dann allgemein. Höchstens acht
  Treffer, höchstens vier Seiten werden abgerufen.
- **Baruch prüft, nicht das Modell:** Ein Zitat gilt nur, wenn der Wortlaut so auf der abgerufenen Seite steht;
  sonst steht **„(nicht belegt)“** dahinter. Gibt es keinen Beleg, endet die Antwort mit „Angabe prüfen: nicht
  belegt“. Die Quellenzeile am Ende setzt immer Baruch.
- Adressen im eigenen Netz ruft Baruch nicht ab.
- Fällt die Websuche aus, antwortet @belege trotzdem; unter der Antwort steht der Grund.
- Der Verweis **↪ Belege der Websuche** am Beitrag führt zur Belegseite. Bei einem **belegten** Zitat übernimmt
  **In die Bibliothek** die Seite samt Zitat in die [Literatur](/hilfe/bibel-quellen/#literatur).

## Fäden ohne Abschnitt {#faeden}

{{< hilfebild name="assistenten-faeden" alt="Die Liste der Fäden eines Werks: je Faden der Anfang der ersten Nachricht als Titel, wer ihn begonnen hat und die Zahl der Beiträge; darunter das Formular „Neuer Faden“." titel="Fäden sind Gespräche, die an keinem Abschnitt hängen." >}}

Ein **Faden** ist ein Gespräch mit Assistenten, das an keinem Abschnitt hängt – etwa über das ganze Buch. Fäden
ändern keinen Buchtext.

So geht es:

1. Unter dem Chat **Fäden ohne Abschnitt** wählen.
2. Unter **Neuer Faden** die erste Nachricht schreiben.
3. Im Faden unter **Antworten** weiterschreiben. Angesprochene Assistenten und der unter „Persona antworten
   lassen“ gewählte antworten, höchstens drei. **Strg+Enter (Mac: ⌘↩)** schickt ab.

{{< hilfebild name="assistenten-faden" alt="Ein einzelner Faden: die Beiträge untereinander in den Farben der Assistenten, darunter das Formular „Antworten“ mit der Auswahl, welcher Assistent antworten soll." titel="Ein Faden mit seinen Beiträgen." >}}

Kopieren, Zitieren, Bearbeiten eigener Beiträge, ↻ und ✕ gehen wie im Chat am Abschnitt.
