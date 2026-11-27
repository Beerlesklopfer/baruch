---
title: "Einstellungen"
description: "Alle Seiten der Einstellungen: Darstellung, Werk, Assistenten, Verbindungen, Quellen, MCP-Zugang und Daten."
weight: 120
---

**Baruch ▾ → Einstellungen** öffnet die Einstellungen. Diese Seite beschreibt, was auf jeder Seite steht. Die
erste Einrichtung von Bibeln, Assistenten und Diensten führen [Installation](/installation/#bibeln-einrichten)
und [MCP und Assistenten](/mcp/#assistenten-einrichten) Schritt für Schritt vor.

## Die Einstellungen öffnen {#oeffnen}

{{< hilfebild name="einstellungen-popup" alt="Die Einstellungen als Fenster über der Arbeitsseite: links die Liste der Seiten – Darstellung, Werk, Assistenten, Verbindungen, Quellen, MCP-Zugang, Daten –, rechts die gewählte Seite; oben rechts der Knopf Schließen." titel="Mit JavaScript öffnen sich die Einstellungen als Fenster über der Arbeitsseite." >}}

So geht es:

1. **Baruch ▾ → ⚙ Einstellungen** wählen oder **Alt+,** drücken (am Mac ungeprüft, siehe
   [Tasten](/hilfe/tasten/#ueberall)).
2. Links eine Seite wählen; rechts erscheint sie.
3. Jedes Formular hat seinen eigenen Knopf **Speichern** bzw. **Übernehmen**. **Esc** oder **Schließen**
   schließt das Fenster.

Jede Seite sagt oben, wo ihre Werte liegen („Gespeichert: …“) – nur in diesem Browser, für ein Werk oder für alle
Werke. Lehnt Baruch eine Eingabe ab, bleibt die Seite mit der Meldung und allem Eingegebenen stehen. Ohne
JavaScript sind die Einstellungen gewöhnliche Seiten.

## Darstellung {#darstellung}

{{< hilfebild name="einstellungen-darstellung" alt="Einstellungen, Seite Darstellung: die Wahl des Themas (wie das System, hell, dunkel), der Ansicht beim Öffnen (Satzbild oder Quelltext), ob Vorschläge anfangs aufgeklappt sind, welcher Bereich rechts offen ist, und der Knopf „Spaltenbreiten zurücksetzen“." titel="Die Darstellung gilt für diesen Browser." >}}

Gilt nur für **diesen Browser**; ein anderer Browser hat seine eigenen Werte.

- **Thema:** wie das System, hell oder dunkel. Der kleine Umschalter dafür steht auch im Reiter **Ansicht**.
- **Editor beim Öffnen:** Satzbild oder Quelltext.
- **Vorschläge im Lesefluss:** anfangs aufgeklappt oder eingeklappt (eingeklappt erscheinen sie als farbige
  Linie; nur der gerade gezeigte steht offen).
- **Offener Bereich rechts:** Assistenten, Fassungen oder Chat.
- **Spaltenbreiten zurücksetzen.**

## Werk {#werk}

{{< hilfebild name="einstellungen-werk" alt="Einstellungen, Seite Werk: oben die Angaben des Werks und die Werkvorlage, darunter die Blöcke Gliederungsschema, Buchangaben, Umschlag, Satzbild, Satz, Fassungen, Predigtzeit, Kommentare und Lexika sowie Abgleich mit MultiGPT." titel="Alles, was nur dieses eine Werk betrifft." >}}

Die Seite sammelt alles, was **ein Werk** betrifft. Jeder Block ist an seiner Stelle der Hilfe beschrieben:

| Block | Inhalt | beschrieben unter |
|---|---|---|
| Angaben und Werkvorlage | Titel, Untertitel, Autor, Art, Zielgruppe, Bibelübersetzung, Vorlage mit ihren Fassungen | [Ein Werk ändern](/hilfe/erste-schritte/#werk-bearbeiten) |
| Gliederungsschema | Zählung der Kapitel und Abschnitte, mit Vorschau | [Gliederungsschema](/hilfe/gliederung/#schema) |
| Buchangaben, Umschlag | Verlag, ISBN, Klappentext, Titelei, Umschlagbild | [Umschlag und Titelei](/hilfe/buchsatz/#umschlag) |
| Satzbild | Schrift, Größe, Breite und Farben beim Schreiben am Bildschirm | [Satz-Einstellungen](/hilfe/buchsatz/#satz-einstellungen) |
| Satz | Seitenformat, Ränder, Schrift, Seitenzahlen, Register und Glossar, Entwurf | [Satz-Einstellungen](/hilfe/buchsatz/#satz-einstellungen) |
| Fassungen | Fortschreiben, Grenzen für neue Fassungen, ältere Fassungen ausblenden | [Speichern und Fassungen](/hilfe/schreiben/#fassungen) |
| Werksprache | Deutsch oder Englisch: die Sprache, in der die Assistenten antworten | [Werksprache](/hilfe/einstellungen/#werksprache) |
| Predigtzeit | Wörter je Minute, Zielzeit | [Zeitwächter](/hilfe/schreiben/#zeitwaechter) |
| Kommentare und Lexika | welche Nachschlagewerke das Werk nutzt | [Die Bibel des Werks wählen](/hilfe/bibel-quellen/#bibeln-waehlen) |
| Abgleich mit MultiGPT | Schalter „nach MultiGPT hochladen“ (Vorgabe aus) | [Abgleich mit MultiGPT](/hilfe/bibel-quellen/#abgleich) |

### Werksprache {#werksprache}

Jedes Werk hat eine **Werksprache**: Deutsch oder Englisch. Im Block „Werksprache“ wählt man sie aus und
bestätigt mit „Speichern“. In dieser Sprache antworten die Assistenten – auch dann, wenn die Frage oder ein
eigener Prompt in einer anderen Sprache geschrieben ist –, und in ihr schreiben sie ihre Marken („Kritisch:“
bzw. „Critical:“, „Vorschlag:“ bzw. „Suggestion:“). Die Sprache der Oberfläche ändert sich dadurch nicht, und
am Text des Werks ändert Baruch nichts. Ein neues Werk ist deutsch; beim
[Austausch](/hilfe/austausch-sicherung/#einlesen) reist die Werksprache mit dem Werk. Umstellen kann sie nur
der Mensch, kein Assistent.

Auf der Seite steht auch die **Kennung des Werks** – eine feste, nicht änderbare Kennzeichnung, an der ein
anderes Baruch beim [Austausch](/hilfe/austausch-sicherung/#einlesen) erkennt, ob es das Werk schon kennt.

## Assistenten {#assistenten}

{{< hilfebild name="einstellungen-assistenten" alt="Einstellungen, Seite Assistenten: oben die Wahl des Lektors, darunter je Rolle ein farbiger Reiter; im gewählten Reiter die Felder Name, Kürzel, Farbe, Rolle, Modell, Kontextlänge, aktiv und der große Text der Anweisung, darunter die Formulare Modell-Verbindung und die Schalter für Belege." titel="Je Assistent ein Reiter mit allen seinen Einstellungen." >}}

Gilt für **alle Werke**.

- **Lektor:** „fest“ eine bestimmte Rolle oder „automatisch nach Kontext“ (die aktive Rolle mit dem größten
  Fassungsvermögen). Ist die feste Rolle nicht aktiv, gilt die automatische Regel; die Seite sagt das.
- **Je Assistent ein Reiter** in seiner Farbe. Darin ein Formular mit einem Knopf **Speichern**:
  - **Name** und **Kürzel** (das Kürzel, etwa `@lektor`, ist nur Anzeige – darüber spricht man ihn im Chat an),
  - **Farbe** – sie färbt seine Beiträge, Fassungen und Vorschläge; **Farbe der Palette** setzt sie zurück,
  - **Rolle** (kurze Beschreibung) und **aktiv**,
  - **Modell** – ein Eingabefeld mit der Liste der Modelle, die der Dienst meldet,
  - **Kontextlänge** – wie viel Text das Modell auf einmal fassen kann; Baruch kürzt danach,
  - der **Rollen-Prompt**: die ausführliche Anweisung an das Modell. Die Vorgabe lässt sich durch einen eigenen
    Text ersetzen; **Vorgabe wiederherstellen** fragt erst nach.
- **Modell-Verbindung** (je Assistent): welcher Dienst ihn antreibt, das Abzeichen **lokal** oder **extern** und
  das Kästchen **Buchtext an eine externe Verbindung schicken** (Vorgabe: aus). Ohne diese Freigabe lehnt Baruch
  Aufträge an einen externen Dienst mit einem ehrlichen Satz ab; es geht nichts hinaus. Darunter steht der
  **wirksame Kontext**: das Kleinere aus der eingestellten Länge und dem, was der Dienst meldet.
- **Belege mitgeben** (nur bei @belege und beim Lektor): Treffer der Bedeutungssuche und Belege aus Bibel,
  Kommentaren und Lexika. Nur bei @belege: **im Web recherchieren** (Vorgabe aus), siehe
  [Websuche für Belege](/hilfe/assistenten/#websuche).
- **Chat: was die Assistenten mitbekommen:** das ganze Kapitel (Vorgabe) oder **Chat nur Abschnitt**.
- Ist die Liste der Modelle noch leer, steht dort „Modellliste noch nicht abgefragt“ mit dem Verweis
  **Modellliste abfragen**. Die Seite selbst fragt beim Öffnen keinen Dienst.

Der Knopf **Farben …** im Bereich „Assistenten“ der Arbeitsseite führt hierher.

## Verbindungen {#verbindungen}

{{< hilfebild name="einstellungen-verbindungen" alt="Einstellungen, Seite Verbindungen: je Dienst ein Kasten mit Adresse, dem Vermerk „hinterlegt“ oder „fehlt“ für die Zugangsdaten und den Knöpfen Speichern und „Verbindung prüfen“; die Adressen im Bild sind Platzhalter." titel="Je Dienst ein Formular: Adresse, Zugangsdaten, Prüfung." >}}

Gilt für **alle Werke**. Baruch arbeitet auf Wunsch mit vier Arten von Diensten zusammen; keiner ist nötig, um zu
schreiben.

| Verbindung | wozu |
|---|---|
| Eliezer und weitere Modell-Verbindungen | Dienste, die Sprachmodelle bereitstellen – sie treiben die Assistenten an |
| LanceDB (lokal) | der Index für die Bedeutungssuche im eigenen Datenordner |
| pgvector | eine eigene Datenbank mit Texten für die Bedeutungssuche |
| MultiGPT | ein eigener Dokumentendienst: Suche, Volltexte, Abgleich, Websuche |

Je Verbindung:

- **Speichern** übernimmt Adresse und Felder.
- **Zugangsdaten** (Schlüssel, Token – eine Art Passwort für Programme –, Passwort): Das Feld ist immer leer;
  daneben steht nur **hinterlegt** oder **fehlt**. Eine Eingabe ersetzt, leer lassen ändert nichts. Zugangsdaten
  liegen im Schlüsselbund des Systems und erscheinen nie auf einer Seite.
- **Zugangsdaten gelten nur für die Adresse, für die sie eingetragen wurden.** Ändert man die Adresse, schickt
  Baruch sie nicht an das neue Ziel, bis man sie dafür neu einträgt.
- **Verbindung prüfen** fragt den Dienst einmal (höchstens 10 Sekunden) und zeigt das Ergebnis: bei
  Modell-Diensten die Liste der Modelle, bei MultiGPT und pgvector die Sammlungen. Ein Fehlschlag steht als
  „✗ fehlgeschlagen“ mit Meldung da.
- **Kurztest** (bei Modell-Verbindungen) schickt den festen Auftrag „Antworte nur mit: OK“ – ohne Buchtext.
- **Geheimnis entfernen** und **Verbindung löschen**, je mit Kästchen.

**Neue Modell-Verbindung:** Name, Art (Eliezer, OpenAI-kompatibel oder Ollama), Adresse und die **Lage**: „nach
der Adresse“ (Vorgabe), „extern“ oder „lokal“. „Nach der Adresse“ behandelt nur erkennbar lokale Adressen als
lokal, alles andere als extern – ein vergessener Haken öffnet die Sperre also nicht. Ist eine Verbindung als
lokal geführt, ihre Adresse aber nicht erkennbar lokal, zeigt ihr Kasten eine rote Warnung.

**Wege der Bedeutungssuche** (unten auf der Seite, je Werk):

- **Wege des Werks:** Kästchen für LanceDB, pgvector und MultiGPT. Ohne Haken gibt es nur die Suche im Wortlaut.
- **Sammlungen:** Nur angehakte Sammlungen von pgvector und MultiGPT werden durchsucht. Erst **Verbindung
  prüfen**, dann erscheinen sie zur Auswahl.
- **Status der Wege:** je Weg „verbunden“, „ausgefallen“ oder „aus“ mit Grund, dazu der Stand des lokalen Index.
- **Index neu aufbauen** (mit Häkchen) baut den lokalen Index des Werks neu auf – nötig nach einem Wechsel des
  Einbettungsmodells.

Der lokale Index folgt der Arbeit von selbst: Nach jedem Speichern zieht Baruch ihn im Hintergrund nach. Ohne
einen Dienst, der Texte für die Bedeutungssuche aufbereitet, bleibt die Suche im Wortlaut. Die
Bedeutungssuche liest nur.

## Quellen {#quellen}

{{< hilfebild name="einstellungen-quellen" alt="Einstellungen, Seite Quellen: oben die Liste bevorzugter Quellen mit Titel, Adresse und Notiz, darunter der Abschnitt „Bibeln (SWORD)“ mit den Modulordnern, der Tabelle der Module und der Wahl der Bibel des Werks." titel="Bevorzugte Quellen für die Websuche und die Bibeln." >}}

- **Bevorzugte Quellen:** eine Liste von Webseiten mit Titel, Adresse und Notiz, in fester Reihenfolge. Je Quelle
  ↑ und ↓ zum Verschieben, **Bearbeiten** und **Löschen**; unten **Neue Quelle**. Die Websuche von @belege fragt
  zuerst die ersten drei Quellen dieser Liste. Assistenten können die Liste lesen, ändern kann sie nur der
  Mensch.
- **Bibeln (SWORD):** Ordner der Bibelmodule, Liste der Module, Bibel des Werks, Schlüssel verschlüsselter Module
  – siehe [Die Bibel des Werks wählen](/hilfe/bibel-quellen/#bibeln-waehlen).

## MCP-Zugang {#mcp-zugang}

{{< hilfebild name="einstellungen-mcp" alt="Einstellungen, Seite MCP-Zugang: die Adresse für Programme, der fertige Befehl für ein Assistenten-Programm, der Vermerk, dass noch kein Zugangsschlüssel (Token) erzeugt ist, das Formular „Token erzeugen“ und die Liste der seit dem Start verbundenen Programme." titel="Der Zugang für Programme – mit Zugangsschlüsseln für andere Rechner." >}}

**MCP** ist die Schnittstelle, über die andere Programme – etwa ein Assistent im Terminal – mit Baruch arbeiten:
lesen, Vorschläge und Beiträge schreiben, Einfälle anlegen. Wie man ein solches Programm anschließt, steht unter
[MCP-Clients](/mcp/#mcp-clients).

Die Seite zeigt:

- die **Adresse** zum Kopieren und den fertigen Befehl zum Anschließen;
- die seit dem Start **verbundenen Programme** mit Name, Version und dem Namen, unter dem ihre Beiträge
  erscheinen;
- die **Tokens für andere Rechner**. Ein Token ist ein Zugangsschlüssel für ein Programm.

Vom eigenen Rechner braucht ein Programm keinen Token. Von einem anderen Rechner aus geht es nur mit:

1. Einen Namen eintragen (zum Beispiel den des Rechners) und **Token erzeugen** drücken.
2. Die Antwortseite zeigt den Token **genau einmal**. Sofort kopieren; wer ihn verliert, erzeugt einen neuen.
3. Den Token im anderen Programm eintragen.

- **Widerrufen** (mit Kästchen) macht einen Token sofort ungültig.
- Die Liste zeigt Name, Anfang des Tokens, Autor, erzeugt, zuletzt benutzt und Stand – nie den ganzen Token.
- Beiträge und Vorschläge eines solchen Zugangs stehen unter dem Namen des Tokens.
- **Achtung:** Ohne vorgeschalteten Server mit Verschlüsselung gehen Token und Texte unverschlüsselt durchs Netz.
  Nur im eigenen, vertrauenswürdigen Netz verwenden; alles Weitere unter [Betrieb](/betrieb/#reverse-proxy).

## Daten {#daten}

{{< hilfebild name="einstellungen-daten" alt="Einstellungen, Seite Daten: Datenordner, Größe und Version mit dem Verweis „Über Baruch“, der Abschnitt „Diese Instanz“ mit Kennung und Name, der Abschnitt EPUBCheck mit dem Feld für den Pfad und der Abschnitt Sicherung mit dem Knopf „Sicherung jetzt“." titel="Datenordner, Name dieses Baruch, EPUBCheck und Sicherung." >}}

- **Datenordner, Größe, Version** – mit dem Verweis **Über Baruch**. Ist der Datenordner auch für andere Benutzer
  des Rechners zugänglich, steht hier eine Warnung.
- **Diese Instanz:** Kennung (nur Anzeige) und **Name der Instanz** mit **Namen speichern** – siehe
  [Der Name dieses Baruch](/hilfe/austausch-sicherung/#instanz).
- **EPUBCheck:** ob das Prüfprogramm für E-Books gefunden ist, und das Feld **Pfad zum epubcheck.jar** mit **Pfad
  speichern** – siehe [E-Book](/hilfe/buchsatz/#epub).
- **Sicherung jetzt** – siehe [Sicherung](/hilfe/austausch-sicherung/#sicherung).
- Je Werk sein Ordner mit dem letzten Eintrag im Verlauf.

Mit [Konten](/hilfe/konten/) kommt die Seite **Konten** dazu.
