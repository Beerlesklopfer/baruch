---
title: "Austausch, Sicherung und Sicherheit"
description: "Ein Werk von einem Baruch zum anderen tragen, die Daten sichern und wissen, wer auf Baruch zugreifen kann."
weight: 110
---

Diese Seite beantwortet drei Fragen: Wie kommt ein Werk vom Arbeitszimmer auf den Laptop? Wie sichere ich meine
Arbeit? Und wer kann auf mein Baruch zugreifen?

## Ein Werk ausgeben {#ausgeben}

{{< hilfebild name="austausch-ausgeben" alt="Die Seite „Austausch: Ausgeben“ des Beispielwerks: eine Erklärung, was in der Datei steht, der Umfang in Zahlen – Gliederungspunkte, Fassungen, Beiträge, Einfälle, Bilder –, der Name dieser Instanz und zwei Knöpfe zum Herunterladen." titel="Die Austauschdatei enthält das ganze Werk mit allen Fassungen." >}}

Die **Austauschdatei** trägt ein ganzes Werk von einem Baruch zu einem anderen – anders als der Typst-Export mit
**allen** Fassungen, Beiträgen, Einfällen und Bildern.

So geht es:

1. Das Werk öffnen und **Buch → Ausgabe → Austausch: Ausgeben** wählen.
2. Die Seite zeigt, was in der Datei steht, und den Umfang in Zahlen.
3. **Austauschdatei herunterladen (.baruch)** drücken. Für kleine Werke oder zum Hineinschauen gibt es daneben
   **Nur XML herunterladen** – eine einzige lesbare Datei.

**In der Datei steht:** die Angaben des Werks, die Buchangaben samt Umschlag, die Werkvorlage und die Gliederung
mit allen Fassungen (auch Vorschläge und verworfene) und der Markierung „aktuell“, die Beiträge, Einfälle (auch
gelöschte), die Mindmap mit ihren Verbindungen, Fäden, Bilder mit allen Fassungen, Befunde, Glossar und Register.

**Nicht in der Datei steht:** Zugangsdaten, die Verbindungen zu Diensten, die Assistenten und ihre Modelle,
offene Anträge, Konten sowie Bibel- und Literaturtexte.

Ausgeben ändert nichts am Werk. Von der Kommandozeile:

```sh
baruch-server export mein-buch --baruch mein-buch.baruch
```

## Ein Werk einlesen und abgleichen {#einlesen}

{{< hilfebild name="austausch-einlesen" alt="Die Seite „Austausch: Einlesen“: Dateiauswahl für eine Austauschdatei, ein Feld für einen eigenen Titel bei einem neuen Werk und der Knopf „Vorschau zeigen“." titel="Schritt 1: Austauschdatei wählen." >}}

So geht es:

1. **Baruch ▾ → Austausch: Einlesen…** wählen.
2. Die Datei wählen (höchstens 128 MiB). Bei einem neuen Werk auf Wunsch einen eigenen Titel eintragen.
3. **Vorschau zeigen** drücken und die Vorschau lesen.
4. **Übernehmen** drücken.

{{< hilfebild name="austausch-vorschau" alt="Die Vorschau vor dem Übernehmen: von welcher Instanz die Datei stammt, ob das Werk neu entsteht oder abgeglichen wird, was hinzukäme, die Abweichungen mit Verweis an die Stelle und der Knopf „Übernehmen“." titel="Schritt 2: Die Vorschau sagt genau, was geschehen würde." >}}

Kennt dieses Baruch das Werk noch nicht, entsteht es **neu**. Kennt es das Werk schon, wird **abgeglichen**. Die
Vorschau nennt:

- von welchem Baruch die Datei stammt;
- **was hinzukäme** (Gliederungspunkte, Fassungen, Einfälle, Beiträge, Bilder …);
- die **Abweichungen**, je mit Verweis an die Stelle: Ein dort geänderter Text kommt als *weitere Fassung*, eine
  geänderte Gliederung als *Antrag*, anderes als *Hinweis*;
- die **Anträge zur Gliederung**, die entstünden (ein neuer Teil, der Kapitel übernähme, braucht die
  [Zustimmung](/hilfe/gliederung/#zustimmung-teil));
- was **hier gelöscht** ist und darum nicht wiederkommt;
- was **nicht übernommen** wird, mit Grund.

**Nichts wird gelöscht oder überschrieben, und keine eingelesene Fassung wird von selbst aktuell.** Welche
Fassung gilt, entscheidet der Mensch durch Umschalten. Einzige Ausnahme: Hat das andere Baruch die eigene
aktuelle Fassung nur weitergeschrieben und passt ihr Verlauf lückenlos, wird sie hier **vorgespult** („vorgespult
von Stand k auf Stand m“). Ein zweites Einlesen derselben Datei fügt nichts doppelt hinzu.

Von der Kommandozeile:

```sh
baruch-server import mein-buch.baruch                # neues Werk
baruch-server import mein-buch.baruch --vorschau     # nur zeigen, was geschähe
baruch-server import mein-buch.baruch --abgleichen   # mit dem vorhandenen Werk abgleichen
```

### Der Name dieses Baruch {#instanz}

Jedes Baruch (genauer: jeder Datenordner) hat einen **Namen**, den man selbst wählt – etwa „Arbeitszimmer“ oder
„Laptop“. Die andere Seite zeigt ihn in der Vorschau; Anträge aus einem Abgleich tragen die Herkunft „Austausch:
Arbeitszimmer“. Einstellen: **Einstellungen → Daten → Diese Instanz → Name der Instanz** (höchstens 80 Zeichen).
Ohne Namen heißt es „ohne Namen“ mit einer kurzen Kennung dahinter.

## Sicherung {#sicherung}

{{< hilfebild name="einstellungen-daten" alt="Einstellungen, Seite Daten: Datenordner, Größe und Version, der Abschnitt „Diese Instanz“ mit Kennung und Name, darunter der Abschnitt Sicherung mit dem Kästchen und dem Knopf „Sicherung jetzt“." titel="Die Sicherung legt eine vollständige Kopie neben den Datenordner." >}}

So geht es:

1. **Einstellungen → Daten** öffnen.
2. Das Kästchen bei **Sicherung jetzt** ankreuzen und den Knopf drücken.
3. Die Seite nennt den Ordner der neuen Sicherung. Er liegt **neben** dem Datenordner und trägt Datum und Uhrzeit
   im Namen.

- Die Sicherung enthält alles: alle Werke mit allen Fassungen, ihren Verlauf, Einfälle, Bilder.
- **Nie wird etwas überschrieben.** Liegen schon zehn Sicherungen dort, lehnt Baruch ab, bis alte entfernt oder
  verschoben sind – Baruch löscht keine selbst. Auch wenn danach zu wenig Platz frei bliebe, legt es nichts an.
- **Vor jeder Aktualisierung** eine Sicherung anlegen.
- **Sicherungen auch woanders hinlegen.** Sie liegen auf derselben Platte wie die Daten; gegen deren Ausfall
  hilft nur eine Kopie an anderer Stelle.
- **Zugangsdaten gehören nicht zur Sicherung.** Nach einem Umzug auf einen anderen Rechner trägt man sie unter
  Einstellungen → Verbindungen neu ein.
- Sichern kann nur der Mensch, kein Assistent.

Wie man eine Sicherung zurückspielt, steht unter [Betrieb](/betrieb/#sicherung). Ein einzelnes gelöschtes Werk
holt das [Archiv](/hilfe/erste-schritte/#loeschen-archiv) zurück.

## Sicherheit {#sicherheit}

{{< hilfebild name="einstellungen-mcp" alt="Einstellungen, Seite MCP-Zugang: die Adresse für Programme, der fertige Befehl für ein Assistenten-Programm, der Vermerk, dass noch kein Zugangsschlüssel (Token) erzeugt ist, das Formular „Token erzeugen“ und die Liste der seit dem Start verbundenen Programme." titel="Wer von außen zugreifen darf, steht unter Einstellungen → MCP-Zugang." >}}

Das Wichtigste in Kürze:

- **Baruch ist in der Vorgabe nur vom eigenen Rechner erreichbar.** Von einem anderen Rechner im Netz kommt man
  nicht auf die Oberfläche.
- **Ohne Konten gibt es keine Anmeldung:** Wer die Oberfläche erreicht, ist der Autor und kann alles lesen und
  ändern. Wer Baruch für andere Rechner oder Menschen öffnet, richtet [Konten](/hilfe/konten/) ein.
- **Fremde Webseiten können nichts auslösen.** Ruft eine andere Webseite im selben Browser Baruch auf, lehnt
  Baruch ab – das Speichern ebenso wie das Starten eines Satzes.
- **Programme** (etwa ein Assistent im Terminal) greifen über einen eigenen Zugang zu. Vom eigenen Rechner geht
  das ohne Weiteres, von anderen Rechnern nur mit einem Zugangsschlüssel; siehe
  [Einstellungen → MCP-Zugang](/hilfe/einstellungen/#mcp-zugang). Programme können nur Vorschläge, Beiträge und
  Einfälle schreiben – Übernehmen, Ändern, Löschen und Einlesen bleiben dem Menschen.
- **Zugangsdaten zu Diensten** liegen im Schlüsselbund des Systems, nie in einer Datei des Datenordners, und
  erscheinen auf keiner Seite.
- **Buchtext verlässt den Rechner nur mit ausdrücklicher Freigabe:** je Assistent für einen externen Dienst, je
  Werk für das Hochladen nach MultiGPT, und für die Websuche von @belege.

Wer Baruch über das Internet oder für mehrere Rechner erreichbar machen will, braucht einen vorgeschalteten
Server mit Verschlüsselung (einen sogenannten Reverse-Proxy); das beschreibt
[Betrieb](/betrieb/#reverse-proxy).

## Wo die Daten liegen {#datenordner}

{{< hilfebild name="einstellungen-daten" alt="Einstellungen, Seite Daten: oben stehen der Pfad des Datenordners, seine Größe und die Version des Programms; darunter je Werk sein Ordner mit dem letzten Eintrag im Verlauf." titel="Einstellungen → Daten nennt den Datenordner und seine Größe." >}}

Alles, was Baruch speichert, liegt in einem einzigen **Datenordner**. Welcher das ist, steht unter
**Einstellungen → Daten** und auf der Seite **Über Baruch**.

- Darin liegt eine Datenbank und je Werk ein Ordner mit dem **Verlauf**: Jede Änderung – Speichern, Umschalten,
  Löschen, jede Entscheidung über einen Antrag – wird dort mit Zeitpunkt und Herkunft festgehalten. Technisch ist
  das ein Git-Archiv; wer Git kennt, kann darin lesen. Auch Gelöschtes bleibt dort nachlesbar. Was genau
  festgehalten wird und wo man es sieht, steht unter [Was Baruch festhält](/hilfe/austausch-sicherung/#protokoll).
- Ist der Datenordner auch für andere Benutzer des Rechners zugänglich, meldet **Einstellungen → Daten** das.
  Baruch ändert die Rechte nie selbst.
- Den Datenordner von Hand nur kopieren, wenn Baruch nicht läuft. Im Betrieb ist **Sicherung jetzt** der richtige
  Weg.

Mehr dazu, auch die üblichen Orte je Betriebssystem: [Betrieb](/betrieb/#datenordner).

### Was Baruch festhält {#protokoll}

{{< hilfebild name="schreiben-fassungsverlauf" alt="Der Verlauf einer Fassung: die Speicherschritte, neueste zuerst, je mit Autor, Zeit und dem Unterschied Wort für Wort – Entferntes rot, Neues grün – und dem Knopf „Bis hier zurück“." titel="Den Verlauf einer Fassung zeigt Baruch selbst an." >}}

Ein Protokoll unter diesem Namen gibt es in Baruch nicht – festgehalten wird trotzdem jede Änderung. Wo man was
sieht:

| Was | Wo man es sieht |
|---|---|
| jeder gespeicherte Stand einer Fassung | im **Verlauf** der Fassung: ein Klick auf **Stand k** im Kopf des Abschnitts; dort auch **Bis hier zurück** – siehe [Verlauf und Rückgängig](/hilfe/schreiben/#verlauf) |
| alle Fassungen eines Abschnitts mit Autor, Zeit und Status | im Bereich **Fassungen** rechts und in der Baumansicht – siehe [Speichern und Fassungen](/hilfe/schreiben/#fassungen) |
| alle Beiträge eines Abschnitts | auf der Seite **Ganze Chat-Historie** – siehe [Chat](/hilfe/assistenten/#chat) |
| jede Änderung am ganzen Werk, auch Gelöschtes | im **Verlauf des Werks** im Datenordner – siehe [Den Verlauf eines Werks lesen](/hilfe/austausch-sicherung/#spiegel) |
| der letzte Eintrag je Werk | unter **Einstellungen → Daten** |

In den **Verlauf des Werks** kommt ein Eintrag für:

- jedes Speichern und Fortschreiben, das Übernehmen eines Vorschlags, das Umschalten auf eine andere Fassung,
  das Ändern und das Löschen einer Fassung – beim Löschen samt dem gelöschten Text;
- jede Änderung der Gliederung, jeden Antrag und die Entscheidung darüber;
- Einfälle, das Löschen, Verschieben und Umwandeln von Beiträgen, Befunde, Bilder, Glossar und
  Satz-Einstellungen;
- das Einlesen und Abgleichen über den [Austausch](/hilfe/austausch-sicherung/#einlesen).

Jeder Eintrag nennt **Zeitpunkt**, **Autor** – „Autor“ für den Menschen, sonst den Assistenten oder das Programm,
das die Änderung angelegt hat – und die **Herkunft**. Bestätigt der Mensch einen Antrag, nennt der Eintrag beide:
wer beantragt und wer bestätigt hat. Nur lesen und suchen hinterlässt keinen Eintrag, ebenso wenig die
Darstellung im Browser und die Zugangsdaten.

### Den Verlauf eines Werks lesen {#spiegel}

Der Verlauf liegt im **Datenordner** im Unterordner `works`: je Werk ein Ordner mit dem Kurznamen des Werks,
etwa `works/ein-jahr-auf-moevenoog`. Welcher Ordner zu welchem Werk gehört, zeigt **Einstellungen → Daten** in
der Tabelle unten auf der Seite (Spalten **Werk** und **Ordner**, dazu die Kennung des letzten Eintrags). Den
Datenordner selbst nennt dieselbe Seite oben. Einfälle ohne Werk haben ihren eigenen Ordner `einfaelle`.

Im Ordner eines Werks liegt sein aktueller Stand als Typst-Dateien: `main.typ`, je Abschnitt eine Datei im
Unterordner `abschnitte`, die Einfälle in `einfaelle.md` und, wenn das Werk eine Werkvorlage hat, `vorlage.typ`.

Lesen kann man den Verlauf mit dem freien Programm Git (Baruch selbst braucht es nicht):

```sh
cd DATENORDNER/works/ein-jahr-auf-moevenoog
git log --stat                                 # alle Einträge, neueste zuerst, mit den geänderten Dateien
git log -p -- abschnitte/001-am-morgen.typ     # alle Änderungen an einem Abschnitt
git show HEAD~3                                # ein älterer Eintrag mit allem, was er geändert hat
```

Die erste Zeile eines Eintrags sagt, was geschah, etwa „1.1 Am Morgen: Fassung 1 fortgeschrieben (Stand 1)“ oder
„Einfall 1: angelegt“. Darunter stehen Kennzeichen wie `Baruch-Node` (der Abschnitt) und `Baruch-Action` (die
Art der Änderung). Die Einträge sind auf Deutsch geschrieben, unabhängig von der Sprache der Oberfläche.

**Nur lesen:**

- Im Ordner nichts ändern, keine eigenen Einträge anlegen und nichts zurücksetzen. Maßgeblich ist die Datenbank;
  Baruch schreibt den Verlauf nach ihr fort, und was man im Ordner von Hand ändert, gilt nicht.
- Am sichersten liest man in einer Kopie, etwa in einer [Sicherung](/hilfe/austausch-sicherung/#sicherung).
- Einen gelöschten Text holt man zurück, indem man ihn aus dem Eintrag des Löschens kopiert und im Abschnitt als
  neue Fassung speichert. Ein ganzes gelöschtes Werk holt das
  [Archiv](/hilfe/erste-schritte/#loeschen-archiv) zurück.
