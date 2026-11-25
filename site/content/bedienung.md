---
title: "Bedienung"
description: "Der erste Rundgang: starten, ein Werk anlegen, schreiben, Fassungen umschalten – und wo die ausführliche Dokumentation steht."
---

Diese Seite ist ein kurzer Einstieg. Jede Seite und jedes Formular beschreibt die
{{< doku "docs/Bedienung.md" "ausführliche Bedienung" >}} im Repository.

## Starten

„Baruch“ im Startmenü wählen oder im Terminal:

```sh
baruch                              # startet den Server, falls nötig, und öffnet den Browser
baruch-server                       # nur der Server auf http://127.0.0.1:8765/
baruch-server --data /tmp/probe     # zum Ausprobieren mit eigenem Datenordner
```

Danach `http://127.0.0.1:8765/` im Browser öffnen. Eine neue Datenbank bringt ein Beispielwerk mit.

## Beenden

**Baruch ▾ → Baruch beenden…** fragt nach und beendet Baruch dann auf diesem Rechner. Das Fenster zeigt „Baruch
ist beendet – dieses Fenster kann geschlossen werden“ und schließt sich, wenn der Browser das zulässt: im
eigenen Fenster, das der Starter mit Chromium, Chrome, Brave oder Edge öffnet, in der Regel ja; in Firefox und
in einem gewöhnlichen Tab bleibt der Hinweis stehen. Es geht auch ohne JavaScript.

- Gespeicherte Texte stehen schon in der Datenbank und im Git-Spiegel; beim Beenden geht nichts verloren.
  Ungespeicherte Eingaben in anderen Fenstern vorher speichern.
- Beenden lässt sich Baruch nur am Rechner, auf dem es läuft – nicht von einem anderen Rechner und nicht über
  einen vorgeschalteten Server (Reverse-Proxy).
- Läuft Baruch als Dienst, bleibt es danach aus, bis der Dienst wieder gestartet wird
  (`systemctl --user start baruch.service`).
- Im Terminal beendet Strg+C; `systemctl --user stop baruch.service` und `kill` fahren ebenso sauber herunter.

## Das erste eigene Werk

**Baruch ▾ → Neues Werk / Importieren…** (oben links im Menüband) legt ein Buch oder eine Predigt an (Titel, Autor, Grundgerüst,
Zielgruppe, Bibelübersetzung) – oder liest eine vorhandene Datei ein: Typst, Markdown (auch als ZIP mit
Bildern), PDF, EPUB oder eine `.baruch`-Austauschdatei. Beim Einlesen zeigt Baruch zuerst eine Vorschau der
Gliederung; angelegt wird erst nach Bestätigung.

### Erste Einrichtung: Bibeln und Assistenten

Baruch bringt weder Bibeltexte noch KI-Modelle mit. Beim ersten Start sind darum die Auswahl der
Bibelübersetzung und die Assistenten noch leer – das ist kein Fehler. Schreiben, Fassungen und Buchsatz gehen
trotzdem sofort.

- **Bibeln:** Module des freien SWORD-Projekts installieren, am einfachsten mit BibleTime, Xiphos oder dem
  Paket `sword-utils`. Baruch findet sie in `~/.sword` von selbst; weitere Ordner lassen sich unter
  **Einstellungen → Quellen** eintragen.
- **Assistenten:** unter **Einstellungen → Assistenten** eine Verbindung zu einem Modell anlegen (lokal etwa mit
  Ollama) und die Personas einrichten. Mehr dazu unter [MCP und Assistenten](/mcp/).

## Der Arbeitsplatz

- **Oben das Menüband:** das Menü **Baruch ▾** (Werke, Import, Einstellungen, Über Baruch) und die Reiter
  **Start** (Editor, Fassung, Einfälle), **Einfügen** (Fußnote, Zitat, Bild, Formate der Vorlage), **Prüfen**
  (Befunde, Bibelstellen, Suchen, Literatur), **Buch** (Gliederung, Satz, Ausgabe, Umschlag) und **Ansicht**
  (Seitenleisten, Laptop-Modus, Thema). Auf kleinen Bildschirmen fasst der **Laptop-Modus** die Gruppen zu
  Knöpfen zusammen.
- **Links die Gliederung:** Ein Klick öffnet den Abschnitt. Umbenennen, Verschieben und Umstufen gehen über
  das Kontextmenü oder über Formulare.
- **In der Mitte der Editor:** das Satzbild – der Abschnitt, wie er gesetzt wird – oder der Typst-Quelltext.
  **Strg+S** speichert; daraus wird eine Fassung.
- **Rechts die Akkordeons:** Assistenten, Fassungen, Befunde und der Chat zum Abschnitt.

## Fassungen

**Speichern** schreibt die eigene aktuelle Fassung fort, solange die Änderung klein bleibt: höchstens etwa ein
Fünftel des Textes geändert (einstellbar unter **Einstellungen → Werk → Fassungen**), Überschrift und Ebene gleich, und seit dem letzten Speichern nicht
mehr als 30 Minuten vergangen. Sonst entsteht von selbst eine neue Fassung, und die Rückmeldung nennt den
Grund, etwa „neue Fassung 4 (große Änderung)“. **Als neue Fassung speichern** legt immer eine neue an. Im Akkordeon
**Fassungen** steht der Baum aller Fassungen des Abschnitts; „aktuell“ ist genau eine. Ein Vorschlag eines
Assistenten erscheint dort als eigene Fassung – **Übernehmen** macht ihn zur aktuellen, **Verwerfen** lässt
den Text, wie er ist.

## Wichtige Tasten

Auf dem Mac gilt **⌘ (Befehl)** statt Strg und **⌥ (Wahl)** statt Alt.

| Taste | Wirkung |
|---|---|
| Strg+S | Speichern |
| Alt+E | Einfall festhalten |
| Alt+L | Literatur öffnen und schließen |
| Alt+, | Einstellungen |
| Strg+F · Strg+H | Suchen · Ersetzen im Abschnitt |
| Strg+Umschalt+F · Strg+Umschalt+H | Suchen · Ersetzen im Werk |
| Alt+→ | nächste Fassung |

Alles geht auch ohne Tastenkürzel und ohne JavaScript über Links und Formulare.

## Buch setzen und ausgeben

Der Reiter **Buch** im Menüband führt zur Satz-Vorschau, zum Buch als PDF, zum Umschlag und zum EPUB. Wie das Buch
aussieht, stellt **Einstellungen → Werk → Satz** ein (Seitenformat, Ränder, Schrift, Kapitelanfang). Auf der
Kommandozeile:

```sh
baruch-server export ID --pdf buch.pdf
baruch-server export ID --epub buch.epub
baruch-server export ID --out ordner        # Typst-Datei und Bilder
```

`ID` ist die Nummer des Werks – sie steht in der Adresszeile des Browsers (`…/?work=3`) – oder sein Ordnername,
den **Einstellungen → Daten** zeigt. Ein Export überschreibt nie eine vorhandene Datei. `baruch-server --help` nennt alle Angaben.

## Über Baruch

**Baruch ▾ → Über Baruch** zeigt Version, Lizenz und den Link zum Quellcode. Baruch fragt nie von sich aus im Netz
nach neuen Versionen; die Seite verlinkt die Release-Seite, auf der man selbst nachsieht.

## Dokumentation

- {{< doku "docs/Bedienung.md" "Bedienung" >}}: jede Seite, jedes Formular
- {{< doku "docs/Betrieb.md" "Betrieb" >}}: Datenordner, Sicherung, Reverse-Proxy
- {{< doku "docs/MCP.md" "MCP" >}} und {{< doku "docs/RPC.md" "RPC" >}}: die Schnittstellen
- {{< doku "docs/Architektur.md" "Architektur" >}}: Aufbau und Entscheidungen
- {{< doku "docs/Verteilung.md" "Verteilung" >}}: Pakete und Release
