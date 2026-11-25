# Baruch

**Freie Schreibwerkstatt für Bücher und Predigten.** Baruch hält ein Werk als Gliederung aus Teilen,
Kapiteln und Abschnitten, bewahrt jede Fassung jedes Abschnitts auf, setzt das Buch mit
[Typst](https://typst.app) und lässt KI-Assistenten mitlesen, prüfen und vorschlagen – ohne dem Menschen
das letzte Wort zu nehmen.

> Benannt nach Baruch, dem Schreiber Jeremias, der die verbrannte Rolle neu und erweitert niederschrieb
> (Jer 36,32). Fassungen sind der Kern.

Baruch steht unter der [AGPL-3.0](LICENSE) und ist bewusst ein freier Gegenentwurf zu bezahlten
Schreibwerkzeugen. Es läuft lokal auf dem eigenen Rechner, die Daten bleiben dort.

Webseite: <https://baruch.bernau.family>

**Stand:** Version 0.9.0. Baruch wird täglich für ein echtes Buch und für Predigten benutzt. Es gibt Pakete
für Debian und Ubuntu (auch über ein APT-Repository), ein AppImage sowie unsignierte Pakete für macOS und
Windows; Schnittstellen können sich noch ändern (siehe [Stand und Grenzen](#stand-und-grenzen), Änderungen in
[CHANGELOG.md](CHANGELOG.md)).

## Inhalt

- [Grundsätze](#grundsätze)
- [Schnellstart](#schnellstart) (Installieren, aus dem Quelltext bauen, Starten)
- [Ein Rundgang](#ein-rundgang)
- [Was Baruch kann](#was-baruch-kann)
- [Tastenkürzel](#tastenkürzel)
- [Assistenten und Modelle einrichten](#assistenten-und-modelle-einrichten)
- [Baruch mit Claude Code und anderen MCP-Clients](#baruch-mit-claude-code-und-anderen-mcp-clients)
- [Kommandozeile](#kommandozeile)
- [Daten, Sicherung und Sicherheit](#daten-sicherung-und-sicherheit)
- [Aufbau](#aufbau)
- [Dokumentation](#dokumentation)
- [Mitarbeiten](#mitarbeiten)
- [Stand und Grenzen](#stand-und-grenzen)
- [Lizenz](#lizenz)

## Grundsätze

- **Der Mensch entscheidet.** Assistenten schreiben Vorschläge, Beiträge, Einfälle und Anträge. Übernehmen,
  Ändern und Löschen am Buchtext bleiben dem Menschen. Was die Gliederung umbaut, stellen Assistenten nur als
  Antrag, dem der Mensch zustimmt.
- **Nichts geht verloren.** Jeder Abschnitt hat beliebig viele Fassungen; genau eine ist „aktuell“. Jede
  Änderung wird ein Commit in einem Git-Spiegel des Werks – mit Herkunft (Mensch, Persona, MCP-Client).
- **Typst ist die Quelle.** Der Spiegel ist ein gewöhnlicher Typst-Ordner, der sich mit `typst compile main.typ`
  auch ohne Baruch setzen lässt.
- **Ohne JavaScript bedienbar.** Jede Seite funktioniert mit Links und Formularen. JavaScript bringt Komfort:
  Ziehen, Tastatur, Popups, Live-Aktualisierung.
- **Lokal und ehrlich.** Baruch antwortet nur auf dem eigenen Rechner. Schlüssel liegen im Schlüsselbund.
  Externe Modelle gibt es nur nach ausdrücklicher Freigabe. Was nicht geht, sagt Baruch – nichts wird still
  gekürzt, verworfen oder geraten.

## Schnellstart

### Installieren

Die Pakete liegen auf der [Release-Seite](https://github.com/Beerlesklopfer/baruch/releases/tag/v0.9.0), dazu
`SHA256SUMS` zum Vergleichen. Die Webseite <https://baruch.bernau.family/> führt Schritt für Schritt durch die
Installation; alle Einzelheiten stehen in [docs/Verteilung.md](docs/Verteilung.md).

**Debian 13 und Ubuntu 24.04 oder neuer (amd64) über das APT-Repository** – so kommen auch neue Versionen
mit `sudo apt upgrade`:

```sh
sudo curl -fsSL -o /usr/share/keyrings/baruch-archive-keyring.gpg \
  https://baruch.bernau.family/apt/baruch-archive-keyring.gpg

sudo tee /etc/apt/sources.list.d/baruch.sources >/dev/null <<'EOF'
Types: deb
URIs: https://baruch.bernau.family/apt
Suites: stable
Components: main
Architectures: amd64
Signed-By: /usr/share/keyrings/baruch-archive-keyring.gpg
EOF

sudo apt update
sudo apt install baruch
```

Fingerabdruck des Archivschlüssels (vor `apt update` mit
`gpg --show-keys --with-fingerprint /usr/share/keyrings/baruch-archive-keyring.gpg` vergleichen):
`BEBD FEF8 F980 2EC5 8A90  B818 9A9C 9199 6BB8 8DB4`

Steht hier noch dieser Platzhalter, ist der Fingerabdruck nicht veröffentlicht und die Paketquelle noch nicht
für Endnutzer gedacht: Die Schlüsseldatei kommt von derselben Webseite wie die Pakete und lässt sich nur mit
einem Fingerabdruck prüfen, den man auf einem zweiten Weg kennt (dieses README im Repository). Ein signiertes
Release entsteht erst, wenn der Wert hier, auf der Webseite und in `docs/Verteilung.md` zum Schlüssel passt
(`scripts/ci-fingerprint.sh`).

| System | Datei | Installieren |
|---|---|---|
| Debian, Ubuntu (amd64) | `baruch_0.9.0-1_amd64.deb` | `sudo apt install ./baruch_0.9.0-1_amd64.deb` |
| Linux allgemein (x86_64) | `Baruch-0.9.0-x86_64.AppImage` | `chmod +x Baruch-0.9.0-x86_64.AppImage`, dann starten |
| macOS, Apple Silicon | `Baruch-0.9.0-macos-arm64.dmg` | öffnen, Baruch nach „Programme“ ziehen (unsigniert) |
| macOS, Intel | `Baruch-0.9.0-macos-x86_64.dmg` | wie oben (unsigniert) |
| Windows (x64) | `Baruch-0.9.0-x64.msi` | ausführen (unsigniert, SmartScreen warnt) |

Die Pakete für macOS und Windows sind **nicht signiert** und noch auf keinem echten Gerät erprobt; wie man sie
trotzdem startet, steht in [docs/Verteilung.md](docs/Verteilung.md). Jedes Paket enthält die Lizenz, die
Lizenzen der eingebundenen Bibliotheken (`THIRD-PARTY-LICENSES.txt`) und den Hinweis auf den Quelltext. Kein
Paket legt etwas im Datenordner an, und das Entfernen löscht dort nichts.

Nach der Installation startet „Baruch“ im Startmenü bzw. `baruch` im Terminal den lokalen Server und öffnet die
Oberfläche im Browser.

**Aktualisieren:** Baruch fragt nie von sich aus im Netz nach neuen Versionen. Mit dem APT-Repository genügt
`sudo apt upgrade`; sonst lädt man das neue Paket von der Release-Seite. Welche Version läuft, zeigt
**Baruch ▾ → Über Baruch** (oder `baruch-server --version`).

### Aus dem Quelltext bauen

Du brauchst:

- Rust (Edition 2024, also eine aktuelle stabile Toolchain);
- Node.js mit npm (nur für den TypeScript-Compiler der Oberfläche);
- `protoc` (Protocol Buffers, für LanceDB) im `PATH`;
- wahlweise `xmllint`, `pdftotext` (Poppler) und Java mit EPUBCheck für zusätzliche Prüfungen.

```sh
git clone https://github.com/Beerlesklopfer/baruch.git
cd baruch
(cd crates/baruch-server/web && npm ci)
nice -n 10 ionice -c3 cargo install --locked --path crates/baruch-server -j 4
```

Das Programm landet in `~/.cargo/bin/baruch-server`. Ein eigenes `.deb` baut `scripts/build-deb.sh`
(braucht `cargo-deb`), siehe [docs/Verteilung.md](docs/Verteilung.md).

### Starten

```sh
baruch                              # aus einem Paket: Server starten und Browser öffnen
baruch-server                       # nur der Server auf http://127.0.0.1:8765/
baruch-server --data /tmp/probe     # zum Ausprobieren mit eigenem Datenordner
```

Danach `http://127.0.0.1:8765/` im Browser öffnen. Eine neue Datenbank bringt ein Beispielwerk mit.

Beenden: Menü **Baruch ▾ → „Baruch beenden…“** (nur am eigenen Rechner), Strg+C im Terminal oder
`systemctl --user stop baruch.service`. Baruch fährt dabei sauber herunter.

### Das erste eigene Werk

- **Baruch ▾ → Neues Werk / Importieren…:** ein Buch oder eine Predigt anlegen (Titel, Autor, Grundgerüst,
  Zielgruppe, Bibelübersetzung) – oder eine vorhandene Datei einlesen: Typst, Markdown (auch als ZIP mit
  Bildern), PDF, EPUB oder eine `.baruch`-Austauschdatei. Beim Einlesen zeigt Baruch zuerst eine Vorschau der
  Gliederung; angelegt wird erst nach Bestätigung.
- Für den Dauerbetrieb als Benutzerdienst siehe [packaging/systemd/README.md](packaging/systemd/README.md);
  das `.deb` bringt dafür ein Beispiel unter `/usr/share/doc/baruch/examples/baruch.service` mit, richtet aber
  selbst keinen Dienst ein.

## Ein Rundgang

Der Arbeitsplatz hat drei Bereiche, deren Breite sich ziehen lässt:

- **Links die Gliederung:** Teile, Kapitel und Abschnitte als Baum, mit Nummern nach dem gewählten Schema,
  Sprechdauer, Abzeichen für offene Vorschläge und Befunde. Ein Klick öffnet den Abschnitt; Umbenennen,
  Verschieben, Umstufen und „Nummer an/aus“ gehen über das Kontextmenü oder Formulare.
- **In der Mitte der Editor:** das Satzbild (so, wie der Abschnitt nach der Werkvorlage aussieht) oder der
  Typst-Quelltext, darüber das Menüband.
- **Rechts die Akkordeons:** Assistenten, Fassungen, Befunde und der Abschnitts-Chat.

Oben eine Kopfzeile mit den Menüs **Werk**, **Ansicht** und **Werkzeuge**, dem Hell/Dunkel-Schalter und den
**Einstellungen** (als Popup, ohne JavaScript als eigene Seite).

## Was Baruch kann

### Werke, Gliederung und Fassungen

- **Werke:** Bücher und Predigten, beliebig viele nebeneinander, jedes mit Titel, Untertitel, Autor,
  Zielgruppe, Bibelübersetzung und Buchangaben.
- **Gliederung:** Teile, Kapitel und Abschnitte, jeder Punkt mit oder ohne Nummer. Markdown-Überschriften
  (`# `, `## ` …) werden beim Tippen zu Gliederungspunkten. Ein neuer Teil, unter den vorhandene Kapitel
  wandern würden, braucht die aktive Zustimmung des Menschen.
- **Gliederungsschemata:** dreizehn Arten zu nummerieren, jederzeit umschaltbar und wieder zurück – Dezimal
  (1.1), römisch, „Teil I · Kapitel 1“, „Erstes Kapitel“, juristisch (§ 1 · Abs. 1), **Predigt**
  (Einleitung · I. · II. · Schluss), **Buch** (wie Predigt, Teile als „Erster Teil“) oder ein eigenes Muster.
  Typst setzt dieselben Nummern wie die Gliederung.
- **Fassungen:** Jeder Abschnitt hat beliebig viele Fassungen als Baum. Genau eine ist aktuell; umschalten
  geht jederzeit, nichts wird still verdrängt. Vorschläge der Assistenten sind eigene Fassungen, auf Wunsch
  in mehreren Varianten nebeneinander.
- **Git-Spiegel:** Jede Änderung wird ein Commit im Spiegel des Werks, mit Herkunft im Commit.
- **Anträge:** Umbenennen, Verschieben, Umstufen, Löschen, Bilder ersetzen, Umschlag setzen – Assistenten
  stellen das nur als Antrag. Der Mensch bestätigt oder lehnt ab, beides wird protokolliert.

### Schreiben

- **Satzbild:** Der Abschnitt erscheint so, wie er gesetzt wird, und wird dort bearbeitet – Absätze,
  Überschriften, Hervorhebung, Listen, Begriffslisten, Zitate, Fußnoten, Links, Szenen, Bilder. Was sich nicht
  als Block bearbeiten lässt, bleibt eine Typst-Insel an Ort und Stelle. Die Rundreise Typst ↔ Satzbild ist
  verlustfrei.
- **Quelltext:** dieselbe Fassung als Typst-Text, umschaltbar.
- **Menüband:** Knöpfe für die Formate der Werkvorlage (Szene, Arbeitsauftrag, Fußnote, Zitat, Liste, Begriff,
  Umbruch, Link, Bibelstelle, Literaturverweis, Verweis) – ein Knopf erscheint nur, wenn die Vorlage das Format
  kennt – und „Bild an der Cursorstelle“.
- **Suchen und Ersetzen** wie in VS Code: Strg+F und Strg+H im Abschnitt, Strg+Umschalt+F und Strg+Umschalt+H
  im ganzen Werk, mit einstellbarem Bereich (Markierung, Abschnitt, Kapitel, Teil, Auswahl, Werk), ganzem Wort,
  Groß-/Kleinschreibung und regulären Ausdrücken. Ersetzen legt je Abschnitt eine neue Fassung an und lässt
  sich mit einem Klick zurücknehmen; Bibelzitate sind standardmäßig ausgenommen.
- **Querverweise:** Jeder Gliederungspunkt hat eine feste Sprungmarke. „Verweis“ fügt einen Verweis ein,
  Strg+Klick springt hin, Alt+← zurück; im PDF und EPUB sind Verweise Links. Baruch findet Formulierungen wie
  „im dritten Kapitel“ und bietet sie zum Umwandeln an.
- **Kontextmenü im Text:** „In Glossar aufnehmen“, „In Register aufnehmen“, „Im Glossar nachschlagen“
  (Rechtsklick, Menütaste oder Umschalt+F10).
- **Zeitwächter:** Sprechdauer je Gliederungspunkt und für das Werk, Zielzeit mit Warnung – vor allem für
  Predigten. Dazu **Stichwortzettel** als A5-Blatt oder Karteikarten.

### Assistenten

- **Personas** mit eigener Rolle, eigenem Modell und eigener Verbindung – Lektor, Exeget, Schrift, Belege,
  Nathan (Auditor), Hörer oder eigene. Lokal über einen OpenAI-kompatiblen Dienst oder Ollama; externe
  Anbieter nur nach Freigabe.
- **Der Lektor als Begleiter:** Er sagt zuerst, was trägt, und nennt dann die wichtigsten Punkte mit einem
  konkreten nächsten Schritt. Er prüft einen Abschnitt oder ein ganzes Kapitel; ist der Text zu lang, teilt
  Baruch ihn überlappend auf – still gekürzt wird nie.
- **Lösungen statt nur Kritik:** Jeder Kritikpunkt kommt mit einer Lösung („statt … / besser …“) und dem
  Knopf „Als Vorschlag übernehmen“. Auf Wunsch setzt der Lektor die Lösungen in zwei bis drei Varianten um,
  die man nebeneinander vergleicht.
- **Chat am Abschnitt und Fäden:** `@lektor` oder „Lektor, …“ ruft eine Persona, „RFC“ bzw. `@alle` ruft alle
  (der Lektor fasst zuletzt zusammen). Antworten erscheinen live, gegliedert, mit geprüften Sprungmarken in den
  Text und der Angabe, welche Fassung geprüft wurde. Beiträge lassen sich kopieren, zitieren, wiederholen und
  löschen; eigene Beiträge auch bearbeiten.
- **Befundliste:** Baruch zerlegt Antworten in einzelne Befunde. Der Mensch markiert sie als übernommen,
  verworfen (mit Begründung), verschoben oder erledigt. Verworfene Befunde gehen als Entscheidung an die
  Personas zurück und werden nicht erneut gemeldet. **Reiter Prüfen → Befunde** zeigt eine Übersicht übers ganze Werk
  (Kapitel × Persona), die Gliederung zeigt „k offen“.
- **Belege:** Eine Persona sucht in der eigenen Bibliothek, in SWORD-Kommentaren und auf Wunsch im Web und
  nennt ihre Quellen.

### Bibel

- **Stellen erkennen und prüfen:** „1. Kön 3,16–28“, „Ps 139,23“, Listen wie „Joh 3,16, 18“, „Psalm 139 …
  Vers 23“; Uhrzeiten und Zahlwörter werden nicht für Stellen gehalten.
- **Verszählungen:** alle 18 Zählungen des SWORD-Projekts mit Umrechnung zwischen deutscher und englischer
  Zählung (z. B. Mal 3,19–24 ↔ Mal 4,1–6). Wo keine Regel greift, sagt Baruch das.
- **SWORD-Module:** lokale Bibeln, Kommentare und Lexika zum Lesen, Suchen und Zitieren – auch komprimiert
  (ZIP, BZIP2, XZ) und mit dem Schlüssel des Herausgebers verschlüsselt. Der Lizenzvermerk wird angezeigt.
- **Zitate prüfen:** Im Editor sind Bibelzitate als „stimmt“ oder „weicht ab“ markiert. Weicht ein Zitat nur
  durch Leerraumreste im Modul ab, heißt es „stimmt (Leerraum im Modul)“.

### Einfälle und Gedankenkarte

- **Einfälle:** Notizen mit Schlagwörtern, Gedanke, markiertem Text, Quelle und Bibelstelle, frei oder an
  einem Abschnitt. Schnellerfassung mit Alt+E. Ein Einfall wird mit einem Klick zum Gliederungspunkt,
  Beitrag, zur Notiz oder zum Auftrag an eine Persona. Volltext- und Bedeutungssuche.
- **Gedankenkarte:** alle Einfälle eines Werks als Karte um Teile und Kapitel, mit Verbindungen („führt
  weiter“, „gehört zu“, „Beispiel für“, „widerspricht“), Nähe aus der Bedeutungssuche, leeren Kapiteln und
  Widersprüchen hervorgehoben. Einfälle per Ziehen einem Kapitel zuordnen oder verbinden; Assistenten schlagen
  Gruppennamen vor. Ausgabe als SVG, PDF, Markdown und FreeMind.

### Literatur und Suche

- **Autorenbibliothek:** Titel mit Zitierschlüssel, Zitate und Hervorhebungen, Volltextsuche (SQLite FTS5)
  über Werk und Literatur; Literatur als Popup mit Alt+L.
- **Bedeutungssuche (RAG):** lokaler Vektorindex (LanceDB) mit Einbettungen über das eigene Modell-Backend,
  ehrlicher Status, Neuaufbau ohne lange Sperre, schwache Treffer gekennzeichnet; wahlweise pgvector und
  MultiGPT als weitere Quellen. Abgleich mit MultiGPT nur auf Knopfdruck.

### Satz, Buch und E-Book

- **Buchsatz in den Einstellungen** (Einstellungen → Werk → Satz): Kapitelanfang (fortlaufend, neue Seite,
  neue rechte Seite), Teil-Seiten, Seitenformat, Ränder und Bundsteg, Schrift, Absatz, Silbentrennung,
  Seitenzahlen, Kolumnentitel, Fußnoten, Schusterjungen und Hurenkinder – mit Vorschau und Erklärung der
  Begriffe. Die Werkvorlage bleibt für Sonderfälle; Widersprüche zeigt Baruch an.
- **Satz-Vorschau und Buch-PDF** mit Titelei (Schmutztitel, Haupttitel, Impressum, Widmung). Impressum und
  Titelseiten lassen sich als Funktionen in der Werkvorlage gestalten; die Druckprüfung warnt bei fehlenden
  Pflichtangaben und bei Bildern unter 300 dpi.
- **Umschlag-Editor:** alle Textblöcke von Vorder-, Rückseite und Rücken frei verschieben, mit Hilfslinien
  (Beschnitt, Sicherheitsabstand, Rücken), Kontrastprüfung und optionaler Schrift. Der Umschlag für die
  Druckerei berechnet die Rückenbreite aus der Seitenzahl.
- **Register und Glossar:** Stichwort-, Personen- und Bibelstellenregister (in biblischer Reihenfolge) mit
  Seitenzahlen, Bereichen, Unterstichwörtern und „siehe auch“; Glossar mit Varianten und Erklärungen, am
  Buchende und im EPUB.
- **EPUB 3:** jedes Kapitel und jede Teil-Seite als eigene Datei, Inhaltsverzeichnis, Fußnoten als Popup oder
  Endnoten, Querverweise, Bilder mit Alternativtext, Umschlag als Cover, Metadaten aus den Buchangaben. Eine
  **EPUB-Vorschau** zeigt neun Reader-Profile nebeneinander (auch „Kindle über EPUB“); eine Strukturprüfung
  läuft immer, EPUBCheck, wenn er auf dem Rechner liegt.

### Import, Export und Austausch

- **Import:** Typst, Markdown (auch als ZIP oder Ordner mit Bildern), PDF (lokal oder über MultiGPT), EPUB und
  Word (.docx) – immer mit Vorschau der Gliederung und Warnungen, angelegt erst nach Bestätigung.
- **Word (.docx) importieren:** Gliederung, Fußnoten, Zitate, Listen, Tabellen und Bilder werden ein Werk; verfolgte
  Änderungen kommen je Bearbeiter als offene Vorschläge, Kommentare als Beiträge – entschieden wird in Baruch.
- **Export:** Typst-Datei oder -Ordner mit Bildern, Buch-PDF, Umschlag-PDF, EPUB, Word (.docx), Stichwortzettel.
- **Word (.docx) für den Verlag:** Export des ganzen Werks mit Formatvorlagen, echten Fußnoten und eingebetteten
  Bildern; offene Vorschläge der Assistenten auf Wunsch als Änderungsverfolgung (`/werke/<id>/word`,
  `baruch-server export ID --docx DATEI [--vorschlaege]`). In Microsoft Word selbst ist das noch nicht geprüft.
- **Austausch zwischen Instanzen:** Die `.baruch`-Datei (ZIP mit XML nach eigenem
  [Schema](docs/schema/)) trägt ein Werk vollständig – alle Fassungen, Einfälle, Verbindungen, Beiträge,
  Befunde, Bilder, Umschlag, Satz-Einstellungen –, aber keine Schlüssel. Einlesen geht als neues Werk oder
  als **Abgleich**: Es kommt nur dazu, was fehlt; nichts wird gelöscht oder überschrieben, abweichende
  Fassungen werden weitere Fassungen, Gliederungsänderungen werden Anträge.

## Tastenkürzel

| Taste | Wirkung |
|---|---|
| Alt+E | Einfall festhalten |
| Alt+L | Literatur öffnen/schließen |
| Alt+, | Einstellungen |
| Strg+S | Speichern |
| Strg+I · Strg+B | hervorheben · fett |
| Umschalt+Enter | Zeilenumbruch im Satzbild |
| Strg+F · Strg+H | Suchen · Ersetzen im Abschnitt |
| Strg+Umschalt+F · Strg+Umschalt+H | Suchen · Ersetzen im Werk |
| Enter/F3 · Umschalt+Enter/Umschalt+F3 | nächster · voriger Treffer |
| Strg+Umschalt+1 · Strg+Alt+Enter | Treffer ersetzen · alle ersetzen |
| Alt+C · Alt+W · Alt+R | Groß/klein · ganzes Wort · regulärer Ausdruck |
| Strg+Klick · Alt+← | Verweis öffnen · zurück |
| Alt+→ | nächste Fassung |
| Menütaste · Umschalt+F10 | Kontextmenü im Text |
| Strg+Enter | Nachricht im Faden absenden |
| Strg+Mausrad · Strg+0 | Bereich vergrößern · zurücksetzen |

## Assistenten und Modelle einrichten

**Einstellungen → Verbindungen** legt die Modell-Verbindungen an (OpenAI-kompatibel, Ollama, wahlweise
pgvector und MultiGPT), **Einstellungen → Assistenten** die Personas mit Rolle, Modell, Kontextlänge, Farbe
und eigenem Prompt. Schlüssel und Token liegen im Schlüsselbund des Systems (Dienst `baruch`), nie in
Dateien. Ein externer Anbieter muss ausdrücklich freigegeben werden. Bibeln, Kommentare und Lexika wählst du
unter **Einstellungen → Quellen**.

## Baruch mit Claude Code und anderen MCP-Clients

Baruch ist ein MCP-Server unter `POST /mcp`. Mit Claude Code:

```sh
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

Über MCP lesen Assistenten Werke, Gliederung, Fassungen, Einfälle, Befunde und Bibeltexte, schreiben
Vorschläge, Beiträge und Einfälle, stellen Anträge und schlagen Glossarbegriffe vor. Was den Buchtext ändert
oder löscht, geht über MCP nicht. Von anderen Rechnern geht MCP nur mit Token. Alle Werkzeuge stehen in
[docs/MCP.md](docs/MCP.md); dieselben Funktionen gibt es als JSON-RPC unter `POST /rpc`
([docs/RPC.md](docs/RPC.md)).

## Kommandozeile

```sh
baruch-server --version
baruch-server [serve] [--listen ADRESSE:PORT] [--data ORDNER] [--secrets keyring|memory] [--allow-host NAME]…
baruch-server import DATEI.typ [--title "TITEL"]
baruch-server import DATEI.md|DATEI.zip [--title "TITEL"]
baruch-server import DATEI.pdf|DATEI.epub [--uebernehmen] [--bibliothek] [--title "TITEL"]
baruch-server import DATEI.docx [--uebernehmen] [--title "TITEL"]
baruch-server import DATEI.baruch [--abgleichen] [--vorschau]
baruch-server export ID|SLUG [--out ORDNER | --pdf DATEI | --epub DATEI | --baruch DATEI]
baruch-server export ID|SLUG --docx DATEI [--vorschlaege]
baruch-server export ID|SLUG --stichwort DATEI [--format a5|karten] [--schrift PT]
```

Export überschreibt nie eine vorhandene Datei. Beim Import von PDF, EPUB und Word zeigt die Kommandozeile ohne
`--uebernehmen` nur die erkannte Gliederung (bei Word auch Bearbeiter und Hinweise). `baruch-server --help` nennt alle Einzelheiten.

## Daten, Sicherung und Sicherheit

- **Daten:** `~/.local/share/baruch` (macOS `~/Library/Application Support/baruch`, Windows
  `%LOCALAPPDATA%\baruch`). Dort liegen `baruch.sqlite` als Quelle aller Daten und unter `works/<slug>/` der
  Git-Spiegel jedes Werks. Die Daten liegen nie im Installationsverzeichnis; kein Paket legt dort etwas an,
  und das Entfernen von Baruch löscht keine Daten.
- **Sicherung:** Einstellungen → Daten → „Sicherung jetzt“. Vor dem Einspielen einer neuen Fassung von Baruch
  empfiehlt sich eine Kopie des Datenordners; Migrationen laufen beim ersten Start automatisch.
- **Sicherheit:** Der Server antwortet nur unter `127.0.0.1`, `[::1]` oder `localhost` (und unter Namen, die
  beim Start mit `--allow-host` erlaubt wurden, siehe [docs/Betrieb.md](docs/Betrieb.md)). Schreibende Aufrufe
  nimmt er nur von der eigenen Seite an. Eingelesene Dateien werden mit Grenzen für Größe, Tiefe und
  entpackte Bytes geprüft. Einzelheiten stehen in [docs/Bedienung.md](docs/Bedienung.md) im Abschnitt
  „Sicherheit“.

## Aufbau

Ein Rust-Server ([axum](https://github.com/tokio-rs/axum) und
[minijinja](https://github.com/mitsuhiko/minijinja)) liefert Seiten und Fragmente aus. Die Widgets sind in
TypeScript geschrieben (`crates/baruch-server/web/src`) und werden beim Bauen von `build.rs` übersetzt.

| Crate | Aufgabe |
|---|---|
| `baruch-core` | Werke, Kapitelbaum, Fassungen je Abschnitt, Personas |
| `baruch-store` | Datenhaltung in SQLite (Migrationen, Anträge, Befunde, Glossar …) |
| `baruch-gitmirror` | Git-Spiegel der Typst-Ordner (reines Rust über gix) |
| `baruch-typst` | Werkvorlage und Gliederung, Schemata, Satz-Einstellungen, Umschlag, Register |
| `baruch-render` | Typst-Satz für Vorschau und PDF |
| `baruch-wysiwyg` | Satzbild-Editor: Typst in bearbeitbare Blöcke und zurück |
| `baruch-epub` | EPUB 3 erzeugen und prüfen |
| `baruch-exchange` | Austauschformat `.baruch` (XML im ZIP-Container) schreiben und sicher lesen |
| `baruch-library` | Autorenbibliothek und Volltextsuche (FTS5) |
| `baruch-rag`, `baruch-lancedb` | Schnittstelle der Suchwege und lokaler Vektorindex |
| `baruch-llm` | OpenAI-kompatibler Modell-Client |
| `baruch-scripture` | Bibelstellen erkennen, Verszählungen, Wortlaut prüfen |
| `baruch-sword` | Lokale SWORD-Module lesen (auch komprimiert und verschlüsselt) |
| `baruch-server` | Server, Oberfläche, RPC und MCP |

## Dokumentation

- [docs/Bedienung.md](docs/Bedienung.md): Bedienung, Seite für Seite
- [docs/Verteilung.md](docs/Verteilung.md): Pakete, Installation, APT-Repository, Release, Auslieferung über
  GitHub Pages
- [docs/Betrieb.md](docs/Betrieb.md): Datenordner, Sicherung, Dauerbetrieb, Betrieb hinter einem Reverse-Proxy
- [CHANGELOG.md](CHANGELOG.md): Änderungen je Version
- [docs/Architektur.md](docs/Architektur.md): Aufbau und Entscheidungen
- [docs/RPC.md](docs/RPC.md): JSON-RPC-Schnittstelle
- [docs/MCP.md](docs/MCP.md): MCP-Werkzeuge
- [docs/WYSIWYG.md](docs/WYSIWYG.md): Satzbild-Editor
- [docs/Austauschformat.md](docs/Austauschformat.md): Austauschformat `.baruch`, Schema unter
  [docs/schema/](docs/schema/)
- [docs/Plan.md](docs/Plan.md), [docs/Fahrplan.md](docs/Fahrplan.md), [docs/Backlog.md](docs/Backlog.md):
  Plan, Runden und offene Punkte

## Mitarbeiten

Oberfläche und Dokumentation sind Deutsch, Bezeichner im Code Englisch. Gearbeitet wird auf dem Zweig
`development`; `main` folgt nach Freigabe. Jede Änderung an Oberfläche, RPC oder MCP zieht die Doku im selben
Commit nach. Die übrigen Regeln stehen in [AGENTS.md](AGENTS.md).

Tests:

```sh
nice -n 10 ionice -c3 cargo test --workspace -j 4
```

Die Tests brauchen kein Netz und keine Zugangsdaten; Modelle, MultiGPT und Webseiten werden nachgebaut.
Bibeltests laufen nur mit freien bzw. erfundenen Testmodulen, Testtexte sind frei erfunden.

Vor einem Commit außerdem:

```sh
nice -n 10 ionice -c3 cargo clippy --workspace --all-targets -j 4 -- -D warnings
cargo deny check licenses sources bans
scripts/ci-selftest.sh
```

Clippy-Warnungen sind Fehler. Ein `#[allow(clippy::…)]` braucht eine Begründung als Kommentar.
`scripts/ci-selftest.sh` prüft ohne Netz die Hilfsskripte für Release und Webseite und die Regeln der
Arbeitsabläufe (Aktionen nur mit Commit-SHA, minimale Rechte, keine Geheimnisse in Befehlszeilen).

Die CI läuft bei jedem Push auf `main` (`.github/workflows/ci.yml`): Tests, clippy, `cargo deny check`,
der Satz eines exportierten Werks mit der Typst-Kommandozeile und der Bau der Webseite. Ein Tag `v<Version>`
baut die Pakete, führt das APT-Repository fort und liefert Webseite und Repository über GitHub Pages aus
(`.github/workflows/release.yml`). Ablauf, einmalige Einrichtung und Stand der Prüfung:
[docs/Verteilung.md](docs/Verteilung.md). Die Quelle der Webseite liegt unter [site/](site/README.md).

## Stand und Grenzen

- **Version 0.9.0** – die erste mit Paketen. Fertig sind alle oben beschriebenen Funktionen; was sich seit dem
  Anfang getan hat, steht in [CHANGELOG.md](CHANGELOG.md), was offen ist, in
  [docs/Backlog.md](docs/Backlog.md), der zeitliche Verlauf in [docs/Fahrplan.md](docs/Fahrplan.md).
- **Pakete:** Das `.deb` und das AppImage sind auf Debian 13 gebaut und gestartet worden. Die Pakete für macOS
  und Windows entstehen nur in der CI, sind unsigniert und noch auf keinem echten Gerät erprobt – auch der
  Schlüsselbund dort (macOS-Schlüsselbund, Windows-Anmeldeinformationsverwaltung) ist gebaut, aber ungeprüft.
  Für Linux gibt es nur amd64/x86_64; das `.deb` braucht glibc 2.39 oder neuer (Debian 13, Ubuntu 24.04).
- **Aktualisierung:** nur über das APT-Repository oder von Hand. Baruch prüft nie selbst auf neue Versionen.
- **Betrieb im Netz:** Baruch hat keine Anmeldung und antwortet von sich aus nur auf dem eigenen Rechner. Ohne
  Zugangsschutz im Proxy gehört es nicht ins Netz. Unter einem Rechnernamen antwortet Baruch nur, wenn er beim
  Start erlaubt wurde (`--allow-host NAME`); Datenordner, Sicherung, Dauerbetrieb und der Betrieb hinter einem
  Reverse-Proxy mit TLS stehen in [docs/Betrieb.md](docs/Betrieb.md).
- **Webseite:** <https://baruch.bernau.family/> – Quelle unter [site/](site/README.md), ausgeliefert nur über
  die CI. Kein Arbeitsablauf ist bisher gelaufen (siehe [docs/Verteilung.md](docs/Verteilung.md), „Stand der
  Prüfung“).
- **Schnittstellen** (RPC, MCP, Austauschformat) können sich bis zur Version 1.0 noch ändern.
- Bibeltexte, Kommentare und Literatur bringt Baruch nicht mit; es liest nur, was auf dem eigenen Rechner
  liegt, und zeigt dessen Lizenz an.
- Rechtliche Angaben im Impressum prüft Baruch auf Vollständigkeit, ersetzt aber keine Rechtsberatung.

## Lizenz

GNU Affero General Public License v3.0 oder später (`AGPL-3.0-or-later`), siehe [LICENSE](LICENSE); im
laufenden Programm unter **Baruch ▾ → Über Baruch**. Der Quelltext jeder Version steht unter
<https://github.com/Beerlesklopfer/baruch> beim Tag `v<Version>`. Die Pakete enthalten Bibliotheken Dritter;
ihre Lizenzen stehen in `THIRD-PARTY-LICENSES.txt` im jeweiligen Paket.
© Jörg Bernau
