---
title: "Erste Schritte"
description: "Baruch starten, den Bildschirm kennenlernen, Werke anlegen, ändern und löschen – und Baruch wieder beenden."
weight: 10
---

Baruch läuft als kleines Programm auf dem eigenen Rechner und wird im Browser bedient. Alles geht auch dann, wenn
im Browser JavaScript abgeschaltet ist; mit JavaScript kommen Bequemlichkeiten dazu (Spalten ziehen, Tastenkürzel,
Seiten, die sich ohne Neuladen auffrischen).

## Baruch starten {#starten}

{{< hilfebild name="installation-erster-start" alt="Die Arbeitsseite von Baruch direkt nach dem ersten Start: oben das Menüband, links die Gliederung des mitgelieferten Beispielwerks, in der Mitte der erste Abschnitt, rechts die Spalte der Assistenten." titel="So sieht Baruch nach dem ersten Start aus." >}}

So geht es:

1. **Baruch** im Startmenü wählen. Der Starter startet das Programm, falls es noch nicht läuft, und öffnet ein
   Fenster.
2. Öffnet sich kein Fenster, im Browser die Adresse `http://127.0.0.1:8765/` eintippen.
3. Beim allerersten Start bringt Baruch ein kleines **Beispielwerk** mit. Darin lässt sich gefahrlos alles
   ausprobieren.

Wer lieber im Terminal arbeitet:

```sh
baruch                              # startet das Programm, falls nötig, und öffnet den Browser
baruch-server                       # nur das Programm, erreichbar unter http://127.0.0.1:8765/
baruch-server --data ~/baruch-probe # zum Ausprobieren mit einem eigenen Datenordner
baruch-server --version             # nennt die Version
```

Gut zu wissen:

- Das Beispielwerk kommt nur in einen wirklich neuen Datenordner, und nur einmal. Wer später alle Werke löscht,
  behält eine leere Liste – auch nach einem Neustart.
- Die Teile des Beispielwerks haben keinen eigenen Text; sie gliedern nur.
- Zum Ausprobieren einen eigenen Datenordner nehmen (dritte Zeile oben). Zugangsdaten zu anderen Diensten, die
  man in einem solchen Probelauf einträgt, merkt sich Baruch nur, bis es beendet wird; die Zugangsdaten des
  eigentlichen Baruch bleiben davon unberührt.
- Wo Baruch die Daten ablegt und wie man sie sichert, steht unter [Betrieb](/betrieb/#datenordner). Die
  Installation und das Einspielen neuer Versionen beschreibt [Installation](/installation/#aktualisieren).
- Bibeln und Assistenten bringt Baruch nicht mit; beide Listen sind anfangs leer. Das ist kein Fehler. Schreiben,
  Fassungen und Buchsatz gehen sofort. Die Einrichtung steht unter
  [Bibeln einrichten](/installation/#bibeln-einrichten) und [Assistenten einrichten](/mcp/#assistenten-einrichten).

## Der Bildschirm {#bildschirm}

{{< hilfebild name="start-arbeitsplatz" alt="Die Arbeitsseite mit dem Beispielwerk „Ein Jahr auf Mövenoog“: oben das Menüband mit den Reitern Start, Einfügen, Prüfen, Buch und Ansicht, links die Gliederung, in der Mitte das Blatt mit dem Text, rechts die Spalte mit Assistenten, Fassungen und Chat." titel="Die Arbeitsseite: Menüband, Gliederung, Blatt und rechte Spalte." >}}

Die Arbeitsseite hat vier Bereiche:

- **Oben das Menüband.** In der ersten Zeile stehen das Menü **Baruch ▾**, die Werkwahl, Titel und Autor des Werks
  und die Reiter **Start**, **Einfügen**, **Prüfen**, **Buch** und **Ansicht**; darunter die Knöpfe des gewählten
  Reiters. Mehr dazu unter [Menüband und Laptop-Modus](/hilfe/menueband/).
- **Links die Gliederung** – die einzige der Oberfläche – mit Teilen, Kapiteln und Abschnitten. Ein Klick auf
  einen Eintrag öffnet den Abschnitt, auch aus der Vorschau heraus. Kapitel lassen sich auf- und zuklappen; kleine
  Marken zeigen offene Vorschläge. Umstellen, Umbenennen, Anlegen und Löschen geschehen direkt in dieser Liste,
  siehe [Gliederung](/hilfe/gliederung/). Die Literatur steht nicht hier, sondern im Reiter **Prüfen**.
- **In der Mitte der gewählte Abschnitt:**
  - Der **Kopf** steht fest oben: Titel, darunter „Fassung n · Status · Autor“ in der Farbe des Assistenten, von
    dem die Fassung stammt, und „‹ n/m ›“ zum Blättern durch die Fassungen (Alt+← und Alt+→, Mac: ⌥← und ⌥→).
    Ist die gezeigte Fassung die aktuelle, ist „n/m“ grün hervorgehoben; sonst steht daneben der Verweis
    **aktuell: Fassung k**.
  - Das **Blatt** zeigt den Text – als Satzbild (so, wie er später gesetzt wird) oder als Quelltext, in der
    Vorschau die Vorschau. Es füllt die restliche Höhe und rollt allein; Kopf und Statuszeile bleiben stehen. Im
    Blatt stehen auch die offenen Vorschläge der Assistenten, jeder in der Farbe seines Assistenten.
  - Die **Statuszeile** unten zeigt Hinweise (der volle Text erscheint, wenn man mit der Maus darüberfährt) und
    die Knöpfe **Speichern** und **Als neue Fassung speichern**.
- **Rechts eine Spalte mit drei aufklappbaren Bereichen**, von denen immer einer offen ist (Baruch merkt sich,
  welcher): **Assistenten**, **Fassungen** und der **Chat** zum Abschnitt. Am Bereich „Fassungen“ steht auch
  zugeklappt, wie viele Fassungen der Abschnitt hat und wie viele Vorschläge offen sind.

Die Grenzen zwischen den Spalten lassen sich mit der Maus ziehen. Mit der Tastatur: die Grenze mit Tab ansteuern
und mit ← oder → verschieben.

Wie man schreibt und speichert, steht unter [Schreiben, Bilder und Fassungen](/hilfe/schreiben/).

## Die Liste der Werke {#werke}

{{< hilfebild name="start-werkliste" alt="Die Liste aller Werke als Tabelle mit Titel, Autor, Art, Zahl der Abschnitte, offenen Vorschlägen und letzter Änderung; je Werk die Knöpfe Öffnen, Ändern, Export und Löschen." titel="Alle Werke auf einen Blick." >}}

Ein **Werk** ist ein Buch oder eine Predigt. Werke verwaltet nur der Mensch; ein Assistent kann weder Werke
anlegen noch löschen.

- **Werk wechseln:** In der ersten Zeile des Menübands steht auf der Arbeitsseite ein Auswahlfeld mit allen Werken
  („Titel · Autor · n Abschnitte“). Mit JavaScript öffnet die Auswahl das Werk sofort, ohne JavaScript mit dem
  Knopf **Öffnen** daneben. Gliederung, Editor, Assistenten und Literatursuche gelten immer dem gezeigten Werk.
- **Alle Werke ansehen:** **Baruch ▾ → Alle Werke**. Die Liste zeigt je Werk Titel, Untertitel, Autor, Art,
  Abschnitte, offene Vorschläge und letzte Änderung, dazu **Öffnen**, **Ändern**, **Export** und **Löschen**.
- Baruch merkt sich das zuletzt gezeigte Werk ein Jahr lang in diesem Browser und öffnet es beim nächsten Besuch
  wieder. Gibt es dieses Werk nicht mehr, öffnet sich das älteste; gibt es gar keines, erscheint die Liste der
  Werke.
- **Export** lädt das Werk als eine Textdatei im Satzformat Typst herunter (die Vorlage des Werks und alle
  aktuellen Texte). Mehr unter [Buchsatz](/hilfe/buchsatz/#typst-export).

## Ein neues Werk anlegen {#neues-werk}

{{< hilfebild name="start-neues-werk" alt="Das Formular „Neues Werk / Importieren“: oben das Einlesen einer vorhandenen Datei oder eines Ordners, darunter die Felder für ein neues Werk – Titel, Untertitel, Autor, Art, Grundgerüst, Zielgruppe und Bibelübersetzung." titel="Neues Werk anlegen oder einen vorhandenen Text einlesen." >}}

So geht es:

1. **Baruch ▾ → Neues Werk / Importieren…** wählen. Auf der Arbeitsseite öffnet sich ein Fenster über dem Text,
   sonst (und ohne JavaScript) eine eigene Seite.
2. Unter **Neues Werk anlegen** Titel, Untertitel und Autor eintragen.
3. **Art** wählen: Buch oder Predigt. Dazu Grundgerüst, Zielgruppe und Bibelübersetzung.
4. Anlegen. Das neue Werk öffnet sich.

Ein Buch beginnt mit dem Kapitel „Erstes Kapitel“ und einer schlichten Vorlage, die Kapitel in der Form „1.1“
zählt. Wer schon einen Text hat, liest ihn stattdessen ein: [Vorhandene Texte einlesen](/hilfe/einlesen/).

**Titel sind eindeutig.** Groß- und Kleinschreibung und Leerzeichen zählen dabei nicht („Das Buch“ ist dasselbe
wie „das  buch“). Ist der Titel beim Anlegen, Einlesen oder Umbenennen vergeben, meldet Baruch „Es gibt schon ein
Werk …“ und legt nichts an.

### Eine Predigt anlegen {#predigt}

Die Art **Predigt** (Grundgerüst „passend zur Art“ oder „Predigt“) legt ein Werk mit einer Predigtgliederung an:

| Nummer | Titel | Bedeutung |
|---|---|---|
| (ohne) | Predigttext | das Bibelwort, das die Predigt auslegt |
| Einleitung | Hinführung | Einstieg zum Text |
| I. | Beobachtung am Text | erster Hauptpunkt |
| II. | Deutung | zweiter Hauptpunkt |
| III. | Zuspruch | dritter Hauptpunkt |
| (ohne) | Anwendung | was daraus folgt |
| Schluss | Ausblick | Abschluss |

Die Titel wiederholen die Nummer nicht (sonst stünde „Einleitung Einleitung“ da). Umbenennen, verschieben,
weitere Hauptpunkte anlegen oder „Nummer an/aus“ geht wie bei jeder Gliederung. Predigttext und Anwendung zählen
nicht mit; darum heißt der letzte gezählte Punkt immer „Schluss“.

**Predigttext mit Wortlaut:** Im Feld **Predigttext** genau eine Bibelstelle eintragen, zum Beispiel
„Joh 3,16–18“, und unter **SWORD-Bibel für den Wortlaut** eine der eingerichteten Bibeln wählen (SWORD ist das
freie Format, in dem Baruch Bibeln liest; im Fenster auf der Arbeitsseite ist das Feld ein Textfeld für den Namen
der Bibel). Bleibt die Bibel leer, gilt die des gerade gezeigten Werks. Baruch setzt den Wortlaut als Zitat mit
Quellenangabe in den Punkt „Predigttext“; die gewählte Bibel wird zugleich die Bibel der neuen Predigt.

- Fehlen Verse in dieser Bibel, steht der Rest da, und Baruch sagt, wie viele fehlen.
- **Ohne eingerichtete Bibel** steht ehrlich nur die Stellenangabe da – Baruch schreibt keinen Wortlaut aus dem
  Gedächtnis.
- Eine Stelle, die es nicht gibt („Ps 151,1“), legt nichts an: Das Formular erscheint wieder, mit Meldung und
  allen Eingaben.
- Bleibt das Feld leer, ist der Predigttext eine bloße Überschrift zum späteren Ausfüllen.

## Ein Werk ändern {#werk-bearbeiten}

{{< hilfebild name="start-werk-bearbeiten" alt="Die Seite „Werk bearbeiten“ mit den Feldern Titel, Untertitel, Autor, Art, Zielgruppe und Bibelübersetzung, darunter das große Feld für die Vorlage des Werks und die Liste ihrer Fassungen." titel="Angaben und Vorlage eines Werks ändern." >}}

So geht es:

1. Reiter **Buch → Werk bearbeiten** wählen (dieselben Felder stehen unter **Einstellungen → Werk**).
2. Titel, Untertitel, Autor, Art, Zielgruppe oder Bibelübersetzung ändern.
3. Speichern.

Auf derselben Seite steht die **Werkvorlage**: alles, was vor der ersten Überschrift steht und das Aussehen des
ganzen Buchs bestimmt (Seite, Schriften, eigene Formate). Eine Überschrift gehört nicht hinein. Jede Änderung der
Werkvorlage wird eine neue **Fassung** mit Notiz; die Liste darunter zeigt alle Fassungen, ältere lassen sich
nachlesen. Der Ordner des Werks im Datenordner behält beim Umbenennen seinen Namen.

## Ein Werk löschen und zurückholen {#loeschen-archiv}

{{< hilfebild name="start-archiv" alt="Das Archiv der gelöschten Werke mit einem gelöschten Probewerk, seinem Archivordner, dem Zeitpunkt des Löschens und dem Knopf zum Wiederherstellen." titel="Gelöschte Werke liegen im Archiv und lassen sich zurückholen." >}}

So geht es:

1. In der Liste der Werke **Löschen** wählen.
2. Zur Sicherheit den Titel des Werks eintippen und bestätigen.

Gelöscht werden die Gliederung, alle Fassungen, Vorschläge, Beiträge und die Fassungen der Werkvorlage. Die
Einfälle des Werks bleiben als Einfälle ohne Werk erhalten. Die Seite verweist vorher auf den Export und das
Archiv. War das gelöschte Werk das älteste, öffnet Baruch künftig das nächste.

**Zurückholen:**

1. In der Liste der Werke **Archiv** wählen. Dort stehen die gelöschten Werke mit Titel, Archivordner und
   Zeitpunkt des Löschens.
2. Das Kästchen zur Bestätigung ankreuzen und **Wiederherstellen** drücken.

Zurück kommen der Titel, die Gliederung, die zuletzt aktuellen Texte (jeder als Fassung 1 mit dem Vermerk „aus
dem Archiv wiederhergestellt“), die Werkvorlage und die Satz-Einstellungen. **Nicht** zurück kommen ältere
Fassungen, Beiträge und Befunde (sie bleiben im Verlauf des Werkordners nachlesbar), die Bilder (sie lassen sich
auf der Seite „Bilder“ wieder hinzufügen) und die Art des Werks (es ist danach ein „Buch“). Die Einfälle lassen
sich auf der Seite „Einfälle“ wieder dem Werk zuordnen. Trägt inzwischen ein anderes Werk denselben Titel, erst
jenes umbenennen.

Ein gelöschtes Werk gibt seinen Platz nicht frei: Wer ein Werk mit demselben Titel neu anlegt oder dieselbe Datei
noch einmal einliest, bekommt ein neues Werk mit eigener Nummer und eigenem Ordner. So bleibt das Archiv
eindeutig; die Seite zum Löschen sagt das vorher.

## Adresse und Lesezeichen {#adresse}

{{< hilfebild name="gliederung-leiste" alt="Ausschnitt der Arbeitsseite: links die Gliederung des Beispielwerks mit einem gewählten Abschnitt." titel="Jeder Abschnitt hat seine eigene, dauerhafte Adresse." >}}

Jeder Abschnitt hat eine eigene Adresse; sie steht in der Adresszeile des Browsers und lässt sich als Lesezeichen
merken oder weitergeben. Die Adresse bleibt gültig, wenn die Gliederung umgestellt wird – ein gemerkter oder von
einem Assistenten genannter Verweis öffnet immer denselben Abschnitt. Gibt es den Abschnitt nicht mehr (gelöscht
oder mit einem anderen zusammengeführt), zeigt Baruch einen anderen Abschnitt und oben den Hinweis „Abschnitt …
gibt es nicht (mehr).“

Die Adresse merkt sich auch, welche Fassung gezeigt wird. Eine gelöschte oder unbekannte Fassung zeigt die
aktuelle. Ein unbekanntes Werk ergibt die Meldung „unbekanntes Werk“.

## Zeitangaben {#zeitangaben}

{{< hilfebild name="schreiben-zeitwaechter" alt="Ausschnitt der Arbeitsseite mit Zeitangaben: Datum und Uhrzeit einer Fassung und die Anzeige des Zeitwächters." titel="Alle Zeiten stehen in der Ortszeit des Rechners." >}}

Alle Zeitpunkte auf den Seiten – bei Fassungen, Einfällen, Bildern, Werken, Verbindungsprüfungen, beim Abgleich,
bei Zugängen für Programme und beim Abrufdatum der Websuche – stehen in der **Ortszeit** des Rechners, auf dem
Baruch läuft, mit Sommerzeit.

## Über Baruch {#ueber}

{{< hilfebild name="start-ueber" alt="Die Seite „Über Baruch“ mit Name und Version, Lizenz, Quellcode, Webseite und Hilfe, Datenordner, dem Verweis auf neue Versionen und den Bibliotheken Dritter." titel="„Über Baruch“ zeigt, was läuft und woher es kommt." >}}

**Baruch ▾ → Über Baruch** zeigt, was läuft und woher es kommt. Die Seite braucht weder JavaScript noch ein Werk.

| Angabe | Inhalt |
|---|---|
| Name, Version | „Baruch“ und die Version des Programms (dieselbe steht unter Einstellungen → Daten) |
| Lizenz | die freie Lizenz AGPL (Version 3 oder später); der Verweis öffnet den vollständigen Text |
| Quellcode | wo der Quelltext genau dieser Version liegt |
| Webseite, Hilfe | die Webseite von Baruch und diese Hilfe |
| Datenordner | der Ordner, mit dem Baruch gerade arbeitet (und der übliche, wenn er abweicht) |
| Neue Versionen | Verweis auf die Seite mit den Paketen dieser Version |
| Bibliotheken Dritter | wo die Lizenzen der mitgelieferten Bibliotheken stehen |

**Baruch fragt nie von sich aus im Internet nach neuen Versionen.** Die Seite ruft nichts ab; erst der Klick auf
den Verweis öffnet die Seite mit den Paketen im Browser. Wie man aktualisiert, steht unter
[Installation](/installation/#aktualisieren). Dieselbe Seite ist unter **Einstellungen → Daten** neben der
Version verlinkt.

## Baruch beenden {#beenden}

{{< hilfebild name="start-beenden" alt="Die Rückfrage „Baruch beenden“ mit einer kurzen Erklärung und den Knöpfen „Baruch beenden“ und „Abbrechen“." titel="Vor dem Beenden fragt Baruch nach." >}}

So geht es:

1. **Baruch ▾ → Baruch beenden…** wählen.
2. In der Rückfrage **Baruch beenden** drücken. **Abbrechen** führt zurück zum zuletzt geöffneten Werk.
3. Das Fenster zeigt „Baruch ist beendet – dieses Fenster kann geschlossen werden“.

Gut zu wissen:

- Das Fenster schließt sich von selbst, wenn der Browser das zulässt: im eigenen Fenster ohne Adresszeile, das der
  Starter mit Chromium, Chrome, Brave oder Edge öffnet, in der Regel ja. In Firefox und in einem gewöhnlichen Tab
  bleibt der Satz stehen.
- Gespeicherte Texte sind sicher abgelegt; beim Beenden geht nichts verloren. Ungespeicherte Eingaben in anderen
  Fenstern vorher speichern. Laufende Antworten der Assistenten gehen noch zu Ende (höchstens 10 Sekunden); ein
  offener Live-Chat wird danach getrennt.
- Beenden lässt sich Baruch **nur am Rechner, auf dem es läuft** – nicht von einem anderen Rechner aus und nicht
  über einen vorgeschalteten Server. Dort zeigt die Seite eine Erklärung statt des Knopfs. Auch Assistenten können
  Baruch nicht beenden.
- Läuft Baruch als Dienst im Hintergrund, bleibt es nach dem Beenden aus, bis der Dienst wieder gestartet wird
  (`systemctl --user start baruch.service`).
- Im Terminal beendet Strg+C (am Mac ebenfalls Ctrl+C). Die letzte Zeile der Ausgabe heißt dann „Baruch beendet.“
