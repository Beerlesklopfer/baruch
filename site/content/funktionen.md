---
title: "Funktionen"
description: "Was Baruch kann: Gliederung und Fassungen, Schreiben im Satzbild, Assistenten, Bibel, Einfälle, Literatur, Buchsatz und E-Book."
---

Die Bilder zeigen ein frei erfundenes Beispielwerk.

## Der Arbeitsplatz

Links die Gliederung, in der Mitte der Editor, rechts Assistenten, Fassungen, Befunde und der Chat zum
Abschnitt. Die Breite der drei Bereiche lässt sich ziehen; oben stehen die Menüs **Werk**, **Ansicht** und
**Werkzeuge**, der Hell/Dunkel-Schalter und die Einstellungen.

{{< bild datei="arbeitsplatz.png" alt="Arbeitsplatz von Baruch: links die Gliederung des Beispielwerks, in der Mitte ein Abschnitt im Satzbild, rechts die Akkordeons für Assistenten, Fassungen und Chat." titel="Der Arbeitsplatz mit Gliederung, Editor und Akkordeons." >}}

## Gliederung

- **Teile, Kapitel und Abschnitte** als Baum, jeder Punkt mit oder ohne Nummer. Markdown-Überschriften werden
  beim Tippen zu Gliederungspunkten.
- **Dreizehn Gliederungsschemata**, jederzeit umschaltbar: dezimal, römisch, „Teil I · Kapitel 1“, juristisch,
  **Predigt** (Einleitung · I. · II. · Schluss), **Buch** oder ein eigenes Muster. Typst setzt dieselben Nummern.
- **Zeitwächter:** Sprechdauer je Punkt und für das Werk, mit Zielzeit und Warnung – vor allem für Predigten.
  Dazu Stichwortzettel als A5-Blatt oder Karteikarten.
- **Anträge:** Umbenennen, Verschieben, Umstufen und Löschen stellen Assistenten nur als Antrag. Der Mensch
  bestätigt oder lehnt ab; beides wird protokolliert.

{{< bild datei="gliederung.png" alt="Seite Gliederung: Teile, Kapitel und Abschnitte des Beispielwerks mit Nummern und Formularen zum Umbenennen und Verschieben." titel="Die Gliederung als eigene Seite – bedienbar ohne JavaScript." >}}

## Fassungen

Jeder Abschnitt hat beliebig viele Fassungen als Baum. Genau eine ist die aktuelle; umschalten geht jederzeit,
nichts wird still verdrängt. Vorschläge der Assistenten sind eigene Fassungen, auf Wunsch in mehreren Varianten
nebeneinander. Jede Änderung wird ein Commit im **Git-Spiegel** des Werks – einem gewöhnlichen Typst-Ordner.

{{< bild datei="fassungen.png" alt="Ein Abschnitt mit zwei Fassungen: gezeigt ist die ältere Fassung 1, daneben der Umschalter 1/2 und der Verweis auf die aktuelle Fassung 2; rechts das Akkordeon Fassungen." titel="Mehrere Fassungen eines Abschnitts; genau eine ist die aktuelle." >}}

## Schreiben

- **Satzbild:** Der Abschnitt erscheint so, wie er gesetzt wird, und wird dort bearbeitet – Absätze,
  Überschriften, Listen, Zitate, Fußnoten, Links, Bilder. Was sich nicht als Block bearbeiten lässt, bleibt eine
  Typst-Insel an Ort und Stelle. Die Rundreise Typst ↔ Satzbild ist verlustfrei.
- **Quelltext:** dieselbe Fassung als Typst-Text, umschaltbar.
- **Suchen und Ersetzen** im Abschnitt und im ganzen Werk, mit ganzem Wort, Groß-/Kleinschreibung und regulären
  Ausdrücken. Ersetzen legt je Abschnitt eine neue Fassung an und lässt sich zurücknehmen.
- **Querverweise** mit festen Sprungmarken; im PDF und EPUB sind sie Links.

## Assistenten

- **Personas** mit eigener Rolle, eigenem Modell und eigener Verbindung – Lektor, Exeget, Belege, Hörer oder
  eigene. Lokal über einen OpenAI-kompatiblen Dienst oder Ollama; fremde Anbieter nur nach Freigabe.
- **Der Lektor als Begleiter:** Er sagt zuerst, was trägt, und nennt dann die wichtigsten Punkte mit einem
  konkreten nächsten Schritt – für einen Abschnitt oder ein ganzes Kapitel.
- **Lösungen statt nur Kritik:** Jeder Kritikpunkt kommt mit einer Lösung und dem Knopf „Als Vorschlag
  übernehmen“.
- **Befundliste:** Baruch zerlegt Antworten in einzelne Befunde. Der Mensch markiert sie als übernommen,
  verworfen, verschoben oder erledigt; Verworfenes wird nicht erneut gemeldet.

{{< bild datei="assistenten.png" alt="Einstellungen, Seite Assistenten: die Personas mit Rolle, Modell, Kontextlänge und Farbe." titel="Personas einrichten: Rolle, Modell, Verbindung und Farbe." >}}

Mehr dazu unter [MCP und Assistenten](/mcp/).

## Bibel

- **Stellen erkennen und prüfen**, auch Listen und Bereiche; Uhrzeiten und Zahlwörter werden nicht für Stellen
  gehalten.
- **Verszählungen:** die Zählungen des SWORD-Projekts mit Umrechnung zwischen deutscher und englischer Zählung.
- **SWORD-Module:** lokale Bibeln, Kommentare und Lexika zum Lesen, Suchen und Zitieren, mit Lizenzvermerk.
  Baruch bringt selbst keine Bibeltexte mit.
- **Zitate prüfen:** Im Editor sind Bibelzitate als „stimmt“ oder „weicht ab“ markiert.

## Einfälle, Literatur und Suche

- **Einfälle** mit Schlagwörtern, Quelle und Bibelstelle; die **Gedankenkarte** zeigt sie um Teile und Kapitel,
  mit Verbindungen und Widersprüchen.
- **Autorenbibliothek** mit Zitierschlüsseln, Zitaten und Volltextsuche über Werk und Literatur.
- **Bedeutungssuche** mit einem lokalen Vektorindex; wahlweise weitere Quellen.

## Satz, Buch und E-Book

- **Buchsatz in den Einstellungen:** Kapitelanfang, Seitenformat, Ränder und Bundsteg, Schrift, Silbentrennung,
  Kolumnentitel, Fußnoten – mit Vorschau und Erklärung der Begriffe.
- **Buch-PDF** mit Titelei, **Umschlag-Editor** mit Hilfslinien für Beschnitt und Rücken, Druckprüfung.
- **Register und Glossar**, **EPUB 3** mit Vorschau für mehrere Reader und Strukturprüfung.

{{< bild datei="satz.png" alt="Satz-Vorschau: Seiten des Beispielwerks, gesetzt mit Typst nach den Satz-Einstellungen des Werks." titel="Die Satz-Vorschau zeigt das Buch, wie Typst es setzt." >}}

{{< bild datei="einstellungen.png" alt="Einstellungen, Seite Werk: Angaben zum Werk und der Block Satz mit Seitenformat, Rändern und Schrift." titel="Einstellungen zum Werk, darunter der Buchsatz." >}}

## Import, Export und Austausch

- **Import:** Typst, Markdown (auch als ZIP mit Bildern), PDF und EPUB – immer mit Vorschau der Gliederung,
  angelegt erst nach Bestätigung.
- **Export:** Typst-Datei oder -Ordner mit Bildern, Buch-PDF, Umschlag-PDF, EPUB, Stichwortzettel.
- **Austausch zwischen Instanzen:** Die `.baruch`-Datei trägt ein Werk vollständig, aber keine Schlüssel. Beim
  Abgleich kommt nur dazu, was fehlt; nichts wird gelöscht oder überschrieben.

Die vollständige Beschreibung steht in der {{< doku "docs/Bedienung.md" "Bedienung" >}} und im
{{< doku "README.md" "README" >}} des Projekts.
