---
title: "Bibel, Literatur und Quellen"
description: "Bibelstellen und Zitate prüfen, in der Bibel nachschlagen, Literatur führen und eigene Quellen nach Wortlaut oder Bedeutung durchsuchen."
weight: 80
---

Baruch prüft Bibelstellen selbst – nie ein Sprachmodell. Ob es Buch, Kapitel und Vers gibt, weiß Baruch immer.
Den **Wortlaut** eines Zitats kann es nur prüfen, wenn eine Bibel eingerichtet und für das Werk gewählt ist.
Bibeltexte bringt Baruch nicht mit; wie man sie einrichtet, steht unter
[Bibeln einrichten](/installation/#bibeln-einrichten).

## Die Bibel des Werks wählen {#bibeln-waehlen}

{{< hilfebild name="bibel-einstellungen" alt="Einstellungen, Abschnitt „Bibeln (SWORD)“: das Feld mit den Ordnern der Bibelmodule, darunter die Tabelle der gefundenen Module mit Name, Art, Verszählung, Lizenzvermerk und Lesbarkeit, darunter die Auswahl „Bibel des Werks“." titel="Ordner der Bibelmodule und die Bibel des Werks." >}}

Baruch liest Bibeln im freien Format **SWORD** (so heißen die Bibelmodule des SWORD-Projekts; auch Programme wie
BibleTime oder Xiphos nutzen sie). Ein anderes Programm braucht Baruch dafür nicht.

So geht es:

1. **Einstellungen → Quellen** öffnen und zum Abschnitt **Bibeln (SWORD)** gehen.
2. Unter **Modulordner** je Zeile einen Ordner eintragen, in dem Module liegen, und **Ordner speichern**. Ohne
   eigene Liste sucht Baruch an den üblichen Orten (unter Linux etwa `~/.sword`). **Vorgabe** stellt das wieder
   her.
3. Die Tabelle prüfen: Sie zeigt je Modul Name, Beschreibung, Art (Bibel, Kommentar, Lexikon), Verszählung, den
   **Lizenzvermerk** des Herausgebers, die Verschlüsselung und ob Baruch das Modul lesen kann – sonst mit Grund.
4. Unter **Bibel des Werks** eine Bibel wählen und **Bibel wählen** drücken. „keine“ schaltet die Prüfung des
   Wortlauts für das Werk ab.

- **Verschlüsselte Module:** Manche Herausgeber geben Module nur verschlüsselt heraus; den Schlüssel bekommt man
  von ihnen. Unter der Tabelle steht dann **Schlüssel verschlüsselter Module** mit einem Feld je Modul und dem
  Knopf **Schlüssel prüfen und hinterlegen**. Baruch liest zur Probe; passt der Schlüssel nicht, wird nichts
  gespeichert. Ein passender Schlüssel kommt nur in den Schlüsselbund des Systems; angezeigt wird nur „hinterlegt“
  oder „fehlt“.
- **Kommentare und Lexika:** Unter **Einstellungen → Werk → Kommentare und Lexika** kreuzt man an, welche das
  Werk nutzt. Sie erscheinen beim Aufschlagen einer Stelle und können den Assistenten als Belege mitgegeben
  werden.
- **Lizenz beachten:** Der Lizenzvermerk des Moduls steht überall, wo Bibeltext erscheint. Was man mit dem Text
  tun darf, bestimmt der Herausgeber.

## Bibelstellen und Zitate prüfen {#zitate-pruefen}

{{< hilfebild name="bibel-zitate-pruefen" alt="Die Seite „Zitate prüfen“ für einen Abschnitt: eine Tabelle mit Stelle, Zitat, Ergebnis und Unterschieden; darüber das benutzte Modul, die Verszählung und der Lizenzvermerk." titel="„Zitate prüfen“ zeigt je Stelle, ob der Wortlaut stimmt." >}}

Baruch erkennt Stellenangaben wie „1. Kön 3,16–28“, „Ps 139,23-24“ oder „Joh 3,16“ im Text und prüft sie.

So geht es:

1. Den Abschnitt öffnen.
2. **Prüfen → Zitate prüfen** wählen (oder **Zitate prüfen** in der Statuszeile unter dem Blatt).
3. Mit JavaScript öffnet sich über der Statuszeile die Liste; im Satzbild sind die Zitate zugleich markiert:
   **grün** unterstrichen heißt „stimmt“, **rot** gewellt „weicht ab“. Ohne JavaScript erscheint eine eigene
   Seite mit derselben Tabelle.

Geprüft wird die gezeigte Fassung – mit JavaScript auch der noch ungespeicherte Text. Die Markierung ändert den
Text nicht; jede Eingabe nimmt sie weg, ein neuer Klick prüft neu. Esc schließt die Liste.

| Ergebnis | Bedeutung |
|---|---|
| Wortlaut stimmt | auch „(Teilzitat)“, wenn nur ein Teil des Verses zitiert ist |
| Wortlaut weicht ab | mit den Unterschieden: `[−Wort]` steht in der Bibel und fehlt im Zitat, `[+Wort]` steht nur im Zitat |
| kein Zitat gefunden | die Stelle steht ohne Zitat da |
| Vers fehlt im Modul | die Bibel hat an dieser Stelle keinen Text |
| Versifikation passt nicht | die Bibel zählt anders, und die Stelle lässt sich nicht sicher zuordnen |
| Stelle ungültig | Buch, Kapitel oder Vers gibt es nicht („Ps 139 hat nur 24 Verse“) |
| ungeprüft | für das Werk ist keine Bibel gewählt |

**Was als Zitat zur Stelle gilt:** Text in Anführungszeichen direkt vor der Stelle („…“ (Joh 3,16)) oder direkt
danach (Joh 3,16: „…“), auch mit kurzen Einleitungswörtern dazwischen („wie Lk 15,6 erzählt: „…““), und
Zitatblöcke mit der Stelle als Quellenangabe. Verglichen wird großzügig: Satzzeichen, Groß- und
Kleinschreibung, Bindestriche und Leerzeichen zählen nicht; „…“ oder „[…]“ im Zitat ist eine Auslassung.

**Schreibweisen von Stellen:**

- Bücher mit Nummer als „1. Kön“, „1 Kön“, „1.Kön“ oder „1. Könige“; „Joh“ ist das Evangelium, „1. Joh“ der
  Brief.
- Mehrere Stellen mit Strichpunkt („Joh 3,16; 4,1“), Bereiche („Joh 3,16–4,2“), Verslisten („Joh 3,16.18“ oder
  „Joh 3,16, 18“), „f“ (ein Vers mehr), „ff“ (bis Kapitelende), ganze Kapitel („Ps 23“).
- „Psalm 139, besonders Vers 23“ im selben Satz versteht Baruch als Ps 139,23.
- Abkürzungen, die auch gewöhnliche Wörter sind („Am“, „Mal“, „Mi“, „Ob“), zählen im Fließtext nur mit Vers –
  „Am 3. Tag“ ist keine Stelle, „Mi 19:30“ nach einem Wochentag eine Uhrzeit.

**Verszählung:** Grundlage ist die deutsche Zählung (wie in der Schlachter-Bibel von 1951): Psalmüberschriften
zählen als Vers, Joel hat vier Kapitel, Maleachi drei. Stellen aus englischen Büchern zählen manchmal anders –
„Mal 4,1“ meldet Baruch darum als ungültig. Eine Bibel in englischer Zählung liest Baruch trotzdem richtig: Es
rechnet die Stelle um und zeigt, was es gelesen hat („gelesen: Mal 4,1–6“). Was sich nicht sicher umrechnen
lässt, bleibt „Versifikation passt nicht“ – Baruch rät nie.

**Assistenten und Bibelstellen:** Kein Assistent bestätigt selbst einen Wortlaut. Unter einer Antwort, die eine
Stelle nennt, steht entweder „Wortlaut geprüft von Baruch (nicht vom Modell) …“ oder, ohne Bibel des Werks, ein
Hinweis, dass der Wortlaut ungeprüft ist. Behauptet ein Befund, eine Stelle fehle, obwohl sie im Text steht,
hängt Baruch „(Stelle steht im Text: …)“ an; nennt er eine Stelle, die es nicht gibt, „(Stelle ungültig: …)“.

## In der Bibel nachschlagen {#nachschlagen}

{{< hilfebild name="bibel-nachschlagen" alt="Die Bibelseite der Literatur: oben die Wahl der Bibel und das Feld „Stelle“ mit Psalm 119,105, darunter der Vers mit seiner Nummer, das Formular zum Zitieren und zum Weitergeben an einen Assistenten und unten die Wahl von Buch und Kapitel. Das Bild zeigt eine Beispielbibel mit einem einzigen Vers." titel="Eine Stelle aufschlagen, blättern, suchen – und Verse verwenden." >}}

So geht es:

1. **Prüfen → Literatur** öffnen. Unter den Treffern steht der Kasten **Bibeln** mit den eingerichteten Bibeln;
   die Bibel des Werks steht zuerst.
2. Eine Bibel anklicken.
3. Im Feld **Stelle** eine Stelle eintragen, zum Beispiel „Ps 139,23-24“ – oder Buch und Kapitel wählen und mit
   „voriges Kapitel“ und „nächstes Kapitel“ blättern. Das Feld **Wortlaut** sucht in der Bibel.

**Verse verwenden:** Das Feld „Stelle“ im Formular trägt die aufgeschlagene Stelle. Mit JavaScript setzt ein Klick
auf eine Versnummer die Stelle, Umschalt-Klick erweitert sie bis dorthin.

- **Zitieren** legt im Abschnitt einen **Vorschlag** an: der bisherige Text und darunter das Zitat mit
  Quellenangabe. Gültig wird es erst, wenn man den Vorschlag annimmt.
- **An Persona geben** schickt die Verse mit einer Frage an den gewählten Assistenten; seine Antwort steht im
  Chat des Abschnitts.

Beides braucht einen Abschnitt – die Literatur also aus einem Abschnitt heraus öffnen. Den Bibeltext nimmt
Baruch immer aus dem Modul. Neben der Stelle stehen die gewählten **Kommentare** zum Aufklappen. Bei einer Bibel
in anderer Verszählung zeigt jeder Vers daneben die deutsche Stelle.

Auch das Suchfeld der Literatur findet Verse (Gruppe „Bibel · Modul“) und Kommentare.

## Literatur {#literatur}

{{< hilfebild name="literatur-seite" alt="Die Literatur des Beispielwerks: oben das Suchfeld mit der Wahl Wortlaut oder Bedeutung, darunter die Bibel des Werks und die Titel der Bibliothek mit Kurznamen; ein Klick auf einen Titel klappt Angaben, Zitate und Hervorhebungen auf." titel="Die Literatur: Titel, Zitate, Hervorhebungen – und eine Suche über alles." >}}

Die **Literatur** ist die Bibliothek des Autors: Titel mit Angaben, Zitaten und Hervorhebungen.

So geht es:

1. **Prüfen → Literatur** wählen oder **Alt+L** drücken (am Mac ungeprüft, siehe
   [Tasten](/hilfe/tasten/#ueberall)). Mit JavaScript öffnet sich ein Fenster über der Arbeitsseite; **Schließen**,
   Esc oder noch einmal Alt+L schließt es. Ohne JavaScript ist es eine eigene Seite.
2. Einen Titel anklicken: Angaben, Zitate und Hervorhebungen klappen auf. **Dokumentansicht** öffnet die Quelle.
3. Im **Suchfeld** suchen. Es durchsucht **Werk und Literatur** zugleich: die aktuellen Texte, die Beiträge im
   Chat, Titel, Zitate und Hervorhebungen. Alle Wörter müssen vorkommen; Groß- und Kleinschreibung und Akzente
   sind egal.

Ein Treffer öffnet sein Ziel an der Fundstelle: im Werk den Abschnitt mit der richtigen Fassung, in der Literatur
die Dokumentansicht. Mit einer Suche steht unter den Treffern auch der Kasten **Bilder des Werks** (höchstens
24 Bilder, die zur Suche passen).

### Eine Quelle einlesen {#quelle-einlesen}

Ein Formular, um einen Titel von Hand einzutragen, hat die Literatur nicht. Neue Quellen kommen auf drei Wegen
hinein: aus einer **Datei** (PDF oder EPUB), aus einem Treffer von **MultiGPT** (siehe
[Abgleich mit MultiGPT](/hilfe/bibel-quellen/#abgleich)) und aus einem belegten Zitat der
[Websuche für Belege](/hilfe/assistenten/#websuche). Der übliche Weg ist die Datei:

1. **Baruch ▾ → PDF oder EPUB importieren…** wählen. Der Eintrag steht auch dann im Menü, wenn kein Werk offen
   ist.
2. Unter **Datei** die PDF oder das EPUB wählen (höchstens 64 MiB). Bei einer PDF unter **Weg** am besten
   **lokal** wählen: Dann liest Baruch die Datei auf diesem Rechner, und sie verlässt ihn nicht. Ein EPUB liest
   Baruch immer selbst.
3. Das Feld **Titel** leer lassen – dann kommt der Titel aus der Datei – und **Lesen und Gliederung zeigen**
   drücken.
4. Die **Vorschau** zeigt die gefundene Gliederung mit Textproben und Hinweisen. Für eine Quelle muss man daran
   meist nichts ändern.
5. Unter **Übernehmen als** den Punkt **Quelle der Bibliothek** wählen – oder **beides**, wenn aus derselben
   Datei zugleich ein Werk entstehen soll.
6. **Titel** und **Autor** prüfen (aus der Datei vorbelegt) und **Übernehmen** drücken.
7. Baruch öffnet die **Dokumentansicht** der neuen Quelle. In der Literatur steht sie ab jetzt bei den Titeln.

- Lokal gelesen merkt sich Baruch den **ganzen Text** der Quelle. Er steht dann als **Volltext** in der
  Dokumentansicht, und Hervorhebungen lassen sich gegen ihn prüfen (siehe
  [Quellen lesen und hervorheben](/hilfe/bibel-quellen/#quellen)).
- Dieselbe Datei ein zweites Mal als Quelle lehnt Baruch ab. **Verwerfen** in der Vorschau löscht den Entwurf;
  angelegt wird dann nichts.
- Eine Quelle einlesen kann nur der Mensch, kein Assistent.
- Alles Weitere zur Vorschau und zu den Wegen steht unter [PDF oder EPUB einlesen](/hilfe/einlesen/#pdf-epub).
  Von der Kommandozeile geht es mit `baruch-server import buch.epub --bibliothek`.

### Wortlaut oder Bedeutung {#bedeutungssuche}

Unter dem Suchfeld wählt „Suche nach“:

- **Wortlaut** (Vorgabe): findet die eingegebenen Wörter.
- **Bedeutung:** findet auch Stellen, die dasselbe meinen, ohne dasselbe Wort zu enthalten. Dafür muss ein
  Dienst eingerichtet sein, der Texte nach ihrer Bedeutung vergleicht – siehe
  [Einstellungen → Verbindungen](/hilfe/einstellungen/#verbindungen).

Treffer der Bedeutungssuche zeigen Titel, Herkunft, Ähnlichkeit in Prozent und den Ausschnitt. Treffer mit
geringer Ähnlichkeit (unter 45 %) heißen „schwach“ und stehen zugeklappt am Ende. Antwortet kein Dienst, steht
dort „Bedeutungssuche nicht verfügbar: …“ mit dem Verweis auf dieselbe Suche im Wortlaut – Baruch schaltet nie
still um. Die Bedeutungssuche liest nur; sie ändert nie einen Text.

## Quellen lesen und hervorheben {#quellen}

{{< hilfebild name="literatur-quelle" alt="Die Dokumentansicht einer Quelle: oben die Angaben, darunter die Zitate mit farbigen Hervorhebungen, die Liste der Hervorhebungen mit Art und Status und unten das Formular „Markierten Text verwenden“." titel="Eine Quelle lesen, Stellen hervorheben und zitieren." >}}

Die **Dokumentansicht** zeigt eine Quelle: ihre Angaben, alle Zitate (Hervorhebungen darin farbig) und alle
Hervorhebungen. Hat Baruch den ganzen Text der Quelle (etwa nach dem
[Einlesen einer PDF oder eines EPUB](/hilfe/bibel-quellen/#quelle-einlesen) oder über MultiGPT), steht darunter der
**Volltext**, Teil für Teil, mit „← vorheriger Teil“ und „nächster Teil →“.

So hebt man hervor:

1. Text in einem Zitat oder im Volltext markieren.
2. Mit der rechten Maustaste das Kontextmenü öffnen:
   - **Hervorheben** – mit Art: Zitat, Kernaussage, Widerspruch, zu prüfen.
   - **Zitieren** – hebt hervor und legt im Abschnitt einen **Vorschlag** an: der bisherige Text und darunter das
     Zitat mit Quellenangabe.
   - **An Persona geben** – gibt den markierten Text mit Quellenangabe und einer Frage an einen Assistenten
     (Vorgabe: der Lektor); Frage und Antwort stehen im Chat des Abschnitts. Höchstens 4000 Zeichen.
   - **Ähnliches suchen** – sucht mit den längsten Wörtern der Markierung in der Literatur.

Ohne JavaScript geht dasselbe über das Formular **Markierten Text verwenden** unter der Dokumentansicht. Dort
lässt sich jede Hervorhebung auch ändern (Art, Notiz, Status), als Zitat übernehmen oder löschen.

- Zitiert wird immer in den Abschnitt, aus dem die Literatur geöffnet wurde.
- **Hervorhebungen der Assistenten** sind immer Vorschläge. Man kann sie ablehnen oder mit **Als eigene
  übernehmen** zu einer eigenen machen. Abgelehnte findet die Suche nicht mehr.
- **Prüfung gegen den Volltext:** Eine Hervorhebung ohne Zitat prüft Baruch gegen den Wortlaut der Quelle – „im
  Volltext gefunden“, „nicht gefunden“ oder „nicht prüfbar“ (mit Grund). Nichts wird als gefunden ausgegeben, was
  nicht geprüft ist.
- Für Bibeltexte gibt es keine Hervorhebungen; die Seite sagt das.

{{< hilfebild name="einstellungen-quellen" alt="Einstellungen, Seite Quellen: die Liste bevorzugter Quellen mit Titel, Adresse und Notiz, je Eintrag Pfeile zum Verschieben und die Knöpfe Bearbeiten und Löschen; darunter das Formular „Neue Quelle“." titel="Bevorzugte Quellen für die Websuche." >}}

**Bevorzugte Quellen:** Unter **Einstellungen → Quellen** steht eine Liste von Webseiten, bei denen der
Assistent @belege zuerst sucht – siehe [Einstellungen](/hilfe/einstellungen/#quellen) und
[Websuche für Belege](/hilfe/assistenten/#websuche).

## Abgleich mit MultiGPT {#abgleich}

{{< hilfebild name="abgleich-seite" alt="Die Seite „Abgleich mit MultiGPT“, solange nichts verbunden ist: der Knopf „Abgleichen“ mit dem Vermerk „Noch nie abgeglichen“, darunter je Werk der Schalter zum Hochladen (ausgeschaltet) und die Liste der Sammlungen." titel="Ohne eingerichtetes MultiGPT sagt die Seite das ehrlich." >}}

**MultiGPT** ist ein eigener Dokumentendienst, den man selbst betreibt; Baruch kann sich mit ihm abgleichen.
Wer keinen hat, braucht diesen Abschnitt nicht. Die Einrichtung steht unter
[MultiGPT einrichten](/mcp/#multigpt-einrichten).

So geht es:

1. **Prüfen → Abgleichen** öffnen.
2. **Abgleichen** drücken. Der Abgleich läuft im Hintergrund, immer nur einer zur Zeit.
3. Nach dem Neuladen steht dort „zuletzt abgeglichen …“ mit den Zahlen (hochgeladen, ersetzt, entfernt, geholt,
   unverändert, Fehler) und den Hinweisen des Laufs.

Baruch gleicht sich **nur auf diesen Knopfdruck** ab – nie beim Start, nie beim Speichern, nie auf Wunsch eines
Assistenten.

- **Von Baruch nach MultiGPT** gehen nur Werke mit dem Schalter **nach MultiGPT hochladen** (je Werk auf dieser
  Seite und unter Einstellungen → Werk; Vorgabe aus, Einschalten nur mit angehaktem Kästchen). Je Kapitel
  entsteht ein Dokument mit den aktuellen Fassungen, in der Sammlung „Baruch: Titel des Werks“. Geänderte Kapitel
  werden ersetzt, gelöschte entfernt. **Nur der Buchtext** geht hinaus, keine Beiträge und Befunde. Ausschalten
  lädt nichts mehr hoch, lässt aber stehen, was schon dort liegt.
- **Von MultiGPT nach Baruch** kommen die unter „Sammlungen von MultiGPT holen“ gewählten Sammlungen als
  **lokale Kopie**. Sie sind danach auch ohne Netz in der Bedeutungssuche zu finden. Grenzen: 500 Dokumente je
  Sammlung, 2 MiB je Dokument, 64 MiB je Abgleich; der Rest folgt beim nächsten Abgleich.
- **Fehler sind Hinweise:** Ist MultiGPT nicht erreichbar oder fehlt ein Recht, endet der Abgleich „mit
  Hinweisen“ und sagt warum. Lokal bleibt alles unverändert.
- In der Bedeutungssuche trägt jeder Treffer aus MultiGPT den Verweis **Volltext**. **In die Bibliothek
  übernehmen** legt daraus einen Titel der Literatur an; das kann nur der Mensch.

**Stand der Prüfung:** Die automatischen Prüfungen von Baruch laufen gegen einen nachgebauten Dienst. Ob der
Abgleich mit dem eigenen MultiGPT klappt, zeigt erst der eigene Versuch – am besten zuerst mit einem Probewerk.
