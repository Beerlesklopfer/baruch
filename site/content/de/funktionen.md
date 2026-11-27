---
title: "Funktionen"
description: "Baruch begleitet Predigten, Vorträge und Bücher vom ersten Einfall über die Bibelarbeit bis zum druckfertigen Buch – mit Fassungen, Bibelprüfung, Assistenten, die nur vorschlagen, und Buchsatz."
---

Baruch ist eine Schreibumgebung für Predigten, Vorträge und Bücher – vom ersten Einfall über die Bibelarbeit
bis zum druckfertigen Buch, ohne die Kontrolle an eine KI abzugeben. Diese Seite zeigt, was dazugehört; am Ende
stehen [häufige Fragen](#haeufige-fragen). Die Bilder zeigen ein frei erfundenes Beispielwerk.

## Der Arbeitsplatz {#arbeitsplatz}

Links die Gliederung, in der Mitte der Editor, rechts aufklappbare Bereiche für Assistenten, Fassungen, Befunde
und den Chat zum Abschnitt. Die Breite der drei Bereiche lässt sich ziehen. Oben steht das **Menüband**: das Menü **Baruch**, die
Reiter **Start**, **Einfügen**, **Prüfen**, **Buch** und **Ansicht** und darunter die Gruppen des gewählten Reiters –
vom Speichern über die Formate bis zum Buch als PDF.

{{< bild datei="arbeitsplatz.png" alt="Arbeitsplatz von Baruch: links die Gliederung des Beispielwerks, in der Mitte ein Abschnitt im Satzbild, rechts die aufklappbaren Bereiche für Assistenten, Fassungen und Chat." titel="Der Arbeitsplatz mit Gliederung, Editor und den Bereichen rechts." >}}

Auf einem Laptop wird es eng. Der **Laptop-Modus** (Reiter **Ansicht**: Automatisch, An, Aus) fasst die Gruppen des
Menübands unter 1600 Pixeln Fensterbreite zu Knöpfen mit Aufklappliste zusammen und klappt unter 1280 Pixeln die
Gliederung und die rechte Spalte zu schmalen Leisten ein, die sich bei Bedarf über den Text legen. So bleibt der
Platz beim Text. Mehr dazu in der [Hilfe zum Laptop-Modus](/hilfe/menueband/#laptop-modus).

{{< bild datei="arbeitsplatz-laptop.png" breite="1366" hoehe="768" alt="Arbeitsplatz von Baruch in einem 1366 Pixel breiten Fenster: das Menüband in der kompakten Form, darunter Gliederung, Editor und die Bereiche rechts." titel="Derselbe Arbeitsplatz auf Laptop-Breite: das Menüband kompakt." >}}

## Gliederung {#gliederung}

- **Teile, Kapitel und Abschnitte** als Baum, jeder Punkt mit oder ohne Nummer. Markdown-Überschriften werden
  beim Tippen zu Gliederungspunkten (`# Titel` wird ein Kapitel).
- **Dreizehn Gliederungsschemata**, jederzeit umschaltbar: dezimal, römisch, „Teil I · Kapitel 1“, juristisch,
  **Predigt** (Einleitung · I. · II. · Schluss), **Buch** oder ein eigenes Muster. Der Buchsatz übernimmt dieselben Nummern.
- **Zeitwächter:** Sprechdauer je Punkt und für das Werk, mit Zielzeit und Warnung – vor allem für Predigten.
  Dazu Stichwortzettel als A5-Blatt oder Karteikarten.
- **Anträge:** Umbenennen, Verschieben, Umstufen und Löschen stellen Assistenten nur als Antrag. Der Mensch
  bestätigt oder lehnt ab; beides wird protokolliert.

Schritt für Schritt: [Hilfe zur Gliederung](/hilfe/gliederung/).

{{< bild datei="gliederung.png" alt="Seite Gliederung: Teile, Kapitel und Abschnitte des Beispielwerks mit Nummern und Formularen zum Umbenennen und Verschieben." titel="Die Gliederung als eigene Seite – bedienbar ohne JavaScript." >}}

## Fassungen {#fassungen}

Jeder Abschnitt hat beliebig viele Fassungen als Baum. Genau eine ist die aktuelle; umschalten geht jederzeit,
nichts wird still verdrängt. Vorschläge der Assistenten sind eigene Fassungen, auf Wunsch in mehreren Varianten
nebeneinander. Jede Änderung landet zusätzlich im **Git-Spiegel** des Werks – einer automatischen Sicherung aller Entwürfe als
gewöhnliche Textdateien, in der sich jeder Schritt nachvollziehen lässt.

{{< bild datei="fassungen.png" alt="Ein Abschnitt mit zwei Fassungen: gezeigt ist die ältere Fassung 1, daneben der Umschalter 1/2 und der Verweis auf die aktuelle Fassung 2; rechts der aufgeklappte Bereich Fassungen." titel="Mehrere Fassungen eines Abschnitts; genau eine ist die aktuelle." >}}

## Schreiben {#schreiben}

- **Satzbild:** Der Abschnitt erscheint so, wie er gesetzt wird, und wird dort bearbeitet – Absätze,
  Überschriften, Listen, Zitate, Fußnoten, Links, Bilder. Komplexe Elemente wie Tabellen oder Formeln bleiben als
  bearbeitbare Code-Blöcke direkt im Text stehen; beim Hin- und Herschalten geht nichts verloren.
- **Quelltext:** dieselbe Fassung als Text in **Typst**, der Auszeichnungssprache, aus der Baruch druckreife PDFs
  setzt (ähnlich wie Markdown, aber für Bücher gemacht) – umschaltbar.
- **Suchen und Ersetzen** im Abschnitt und im ganzen Werk, mit ganzem Wort, Groß-/Kleinschreibung und Suchmustern
  (reguläre Ausdrücke). Ersetzen legt je Abschnitt eine neue Fassung an und lässt sich zurücknehmen.
- **Querverweise** mit festen Sprungmarken; im PDF und EPUB sind sie Links.

## Assistenten {#assistenten}

- **Assistenten mit eigener Rolle** (in Baruch auch „Personas“ genannt), eigenem KI-Modell und eigener
  Verbindung – Lektor, Exeget, Belege, Hörer oder eigene. Die Modelle laufen am besten lokal (etwa mit Ollama oder einem anderen Dienst mit OpenAI-kompatibler
  Schnittstelle); fremde Anbieter nur nach ausdrücklicher Freigabe.
- **Der Lektor als Begleiter:** Er sagt zuerst, was trägt, und nennt dann die wichtigsten Punkte mit einem
  konkreten nächsten Schritt – für einen Abschnitt oder ein ganzes Kapitel.
- **Lösungen statt nur Kritik:** Jeder Kritikpunkt kommt mit einer Lösung und dem Knopf „Als Vorschlag
  übernehmen“.
- **Befundliste:** Baruch zerlegt die Rückmeldungen der Assistenten in einzelne, abhakbare Punkte (Befunde).
  Der Mensch markiert sie als übernommen, verworfen, verschoben oder erledigt; Verworfenes wird nicht erneut
  gemeldet.

{{< bild datei="assistenten.png" alt="Einstellungen, Seite Assistenten: die Assistenten mit Rolle, Modell, Kontextlänge und Farbe." titel="Assistenten einrichten: Rolle, Modell, Verbindung und Farbe." >}}

Einrichten: [Assistenten einrichten](/mcp/#assistenten-einrichten). Arbeiten: Hilfe zu
[Chat](/hilfe/assistenten/#chat) und [Befunden](/hilfe/assistenten/#befunde).

## Bibel {#bibel}

- **Stellen erkennen und prüfen**, auch Listen und Bereiche; Uhrzeiten und Zahlwörter werden nicht für Stellen
  gehalten.
- **Bibeln, Kommentare und Lexika** aus dem freien **SWORD-Projekt** (der Bibliothek hinter Programmen wie
  BibleTime und Xiphos): einmal installiert, liegen sie auf dem eigenen Rechner und lassen sich lesen, durchsuchen
  und zitieren, mit Lizenzvermerk. Baruch bringt selbst keine Bibeltexte mit.
- **Verszählungen:** Umrechnung zwischen deutscher und englischer Zählung, etwa bei den Psalmen.
- **Zitate prüfen:** Im Editor sind Bibelzitate als „stimmt“ oder „weicht ab“ markiert.

Einrichten: [Bibeln einrichten](/installation/#bibeln-einrichten).

## Einfälle, Literatur und Suche {#einfaelle}

- **Einfälle** mit Titel, Schlagwörtern, Quelle und Bibelstelle; die **Mindmap** zeigt sie als Äste an Teilen und
  Kapiteln, mit Unterknoten, Verbindungen und Widersprüchen. Einfälle ohne Platz liegen in der Wolke daneben.
  Austausch mit XMind, FreeMind und Freeplane.
- **Autorenbibliothek** mit Zitierschlüsseln, Zitaten und Volltextsuche über Werk und Literatur.
- **Bedeutungssuche:** findet Stellen nach dem Sinn, nicht nur nach dem Wortlaut („Trost im Leid“ findet auch
  „Gott wischt die Tränen ab“). Der Suchindex liegt auf dem eigenen Rechner; weitere Quellen wahlweise.

## Satz, Buch und E-Book {#buchsatz}

- **Buchsatz in den Einstellungen:** Kapitelanfang, Seitenformat, Ränder und Bundsteg, Schrift, Silbentrennung,
  Kolumnentitel, Fußnoten – mit Vorschau und Erklärung der Begriffe.
- **Buch-PDF** mit Titelei, **Umschlag-Editor** mit Hilfslinien für Beschnitt und Rücken, Druckprüfung.
- **Register und Glossar**, **E-Book** im Standardformat EPUB 3 mit Vorschau für mehrere Lesegeräte und
  Prüfung auf Fehler und Barrierefreiheit.

Schritt für Schritt in der Hilfe: [Buch als PDF](/hilfe/buchsatz/#pdf), [Umschlag](/hilfe/buchsatz/#umschlag),
[E-Book](/hilfe/buchsatz/#epub).

{{< bild datei="satz.png" alt="Satz-Vorschau: Seiten des Beispielwerks, gesetzt mit Typst nach den Satz-Einstellungen des Werks." titel="Die Satz-Vorschau zeigt das Buch, wie Typst es setzt." >}}

{{< bild datei="einstellungen.png" alt="Einstellungen, Seite Werk: Angaben zum Werk und der Block Satz mit Seitenformat, Rändern und Schrift." titel="Einstellungen zum Werk, darunter der Buchsatz." >}}

## Import, Export und Austausch {#austausch}

- **Import:** Typst, Markdown (auch als ZIP mit Bildern), PDF, EPUB und Word (.docx) – immer mit Vorschau der Gliederung,
  angelegt erst nach Bestätigung (Hilfe: [Vorhandene Texte einlesen](/hilfe/einlesen/),
  [Word](/hilfe/einlesen/#word)).
- **Export:** Typst-Datei oder -Ordner mit Bildern, Buch-PDF, Umschlag-PDF, EPUB, Word (.docx), Stichwortzettel.
- **Typst-Kommandozeile inklusive:** Den exportierten Typst-Ordner setzt `baruch-typst compile buch.typ` – dasselbe
  Programm mit dem eingebauten Satzprogramm, ohne ein zweites Typst und ohne Netz (siehe
  [Installation](/installation/#typst-kommandozeile)).
- **Word-Export für den Verlag:** Baruch schreibt das Werk als Word-Datei (.docx) mit Formatvorlagen für
  Überschriften, echten Fußnoten und eingebetteten Bildern – auf Wunsch mit den offenen Vorschlägen der Assistenten
  als Änderungsverfolgung, die der Verlag in Word annimmt oder ablehnt.
- **Austausch zwischen zwei Installationen:** Die `.baruch`-Datei trägt ein Werk vollständig, aber keine Schlüssel. Beim
  Abgleich kommt nur dazu, was fehlt; nichts wird gelöscht oder überschrieben.

## Lektorat: Konten und Rollen {#konten}

- **Allein am eigenen Rechner** braucht Baruch kein Konto und keine Anmeldung.
- **Mit Lektoren:** Der Autor richtet ein Konto ein, legt Lektoren an und gibt ihnen einzelne Werke frei. Lektoren
  melden sich im Browser an – gedacht über einen vorgeschalteten Webserver mit Verschlüsselung
  (Reverse-Proxy, siehe [Betrieb](/betrieb/#reverse-proxy)).
- **Ein Lektor liest und schlägt vor:** Vorschläge für Fassungen, Beiträge im Abschnitts-Chat und Befunde, alles
  unter seinem Namen (`@lektorat:<name>`) und im Git-Spiegel nachvollziehbar. Was gilt, entscheidet der Autor:
  Übernehmen, Ändern und Löschen bleiben ihm vorbehalten, ebenso Assistenten, Export und Einstellungen.
- **Streng gesperrt:** Für Lektoren ist alles verboten, was nicht ausdrücklich erlaubt ist – auch neue Funktionen
  künftiger Versionen.

Einrichten und Arbeiten: Hilfe zu [Konten](/hilfe/konten/#einrichten) und [Lektoren](/hilfe/konten/#lektoren).

## Häufige Fragen {#haeufige-fragen}

**Auf welchen Systemen läuft Baruch?**
Baruch ist ein kleines Programm, das auf dem eigenen Rechner läuft und im Browser bedient wird – keine Cloud,
kein Konto bei einem Anbieter. Pakete gibt es für Linux (Debian und Ubuntu, AppImage), macOS und Windows; die Pakete für macOS sind
signiert und von Apple notarisiert, das Paket für Windows ist noch nicht signiert. Siehe [Installation](/installation/).

**Wie schnell ist man startklar?**
Schreiben, Fassungen, Gliederung und Buchsatz funktionieren sofort nach der Installation; das Satzsystem ist
eingebaut, druckreife PDFs brauchen kein Zusatzprogramm. Zwei Dinge richtet man bei Bedarf selbst ein:
**Bibeln** aus dem SWORD-Projekt (am einfachsten mit BibleTime, Xiphos oder dem Paket `sword-utils`; Baruch
findet sie dann von selbst) und **Assistenten**, also ein KI-Modell (lokal etwa mit Ollama). Einen geführten
Einrichtungsassistenten gibt es noch nicht; die Schritte stehen unter
[Nach der Installation](/installation/#nach-der-installation).

**Brauche ich eine KI?**
Nein. Ohne Modell ist Baruch eine vollständige Schreib- und Satzumgebung; die Assistenten sind ein Angebot.

**Was ist MCP?**
Eine offene Schnittstelle (Model Context Protocol), über die KI-Programme wie Claude Code mit Baruch arbeiten
können – lesend und mit Vorschlägen, nie mit dem letzten Wort. Siehe
[KI-Programme mit Baruch verbinden](/mcp/#mcp-clients).

Die vollständige Beschreibung steht in der [Hilfe](/hilfe/) – Schritt für Schritt, mit Bildern. Für Entwickler:
das {{< doku "README.md" "README" >}} des Projekts.
