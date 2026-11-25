# Änderungen

Alle nennenswerten Änderungen an Baruch, die neueste Version zuerst. Die Versionsnummer legt der Autor fest
(`[workspace.package] version` in `Cargo.toml`). Einzelheiten je Runde – Commits, Dauer, Befunde – stehen in
[docs/Fahrplan.md](docs/Fahrplan.md), offene Punkte in [docs/Backlog.md](docs/Backlog.md).

## Unveröffentlicht

### Mindmap wie XMind (Runde 34)

- **Mindmap statt Gedankenkarte:** Die Seite `/werke/<id>/karte` (Start → Einfälle → Mindmap) zeigt das Werk in der
  Mitte, Teile und Kapitel als Äste rechts und links und die Einfälle als Unteräste, beliebig tief. Jeder Hauptast
  hat seine Farbe; Äste lassen sich einklappen („−“/„+ n“). Alte Lesezeichen gelten weiter.
- **Bedienung wie gewohnt:** Tab legt einen Unterknoten an, Enter ein Geschwister, F2 oder Doppelklick benennt um,
  Entf löscht samt Unterknoten (wiederherstellbar), Ziehen hängt um oder ordnet, Pfeiltasten wandern, Leertaste
  klappt, Strg+Z nimmt die letzte Aktion zurück, Alt+L verbindet zwei Einfälle mit einer beschrifteten Linie.
- **Einfälle als Baum:** Ein Einfall hat jetzt einen Titel, kann unter einem anderen Einfall hängen und hat eine
  Reihenfolge unter seinen Geschwistern. Titel oder Schlagwort genügt.
- **Wolke und Mindmap verbunden:** Einfälle ohne Platz liegen in der Wolke unter dem Baum („n in der Wolke“). Von
  dort zieht man sie auf einen Knoten, auf ein Kapitel oder aufs Werk (freies Hauptthema) – und zurück, samt
  Unterknoten. Wo die Nähe einen passenden Einfall kennt, steht am Kärtchen „passt zu Ast …“. Bestehende Einfälle
  ohne Bezug liegen nach der Umstellung in der Wolke; Verbindungen und gespeicherte Lagen bleiben erhalten (Lagen
  haben in der Mindmap keine Wirkung mehr).
- **Ohne JavaScript:** Die Gliederungsansicht (`/werke/<id>/karte/gliederung`) zeigt dieselbe Mindmap als
  verschachtelte Liste mit Formularen für alle Aktionen.
- **Austausch mit XMind, FreeMind und Freeplane:** Ausgabe als `.xmind`, `.mm`, SVG, PDF und Markdown; Einlesen von
  `.xmind` und `.mm` mit Vorschau (Titel, Notizen, Schlagwörter, Reihenfolge, Beziehungen). In echtem XMind und
  Freeplane ist nichts geprüft – der Aufbau der XMind-Datei folgt der Formatbeschreibung aus dem Gedächtnis.
- **RPC und MCP:** `idea.move`/`idea_move` und `idea.tree`/`idea_tree` neu; `idea.create`/`idea_create` mit `title`,
  `parent`, `before`, `cloud`; Löschen nimmt die Unterknoten mit. Jede Aktion ist ein Commit mit Herkunft.
- **Austauschformat Unterversion 9** (`titel`, `eltern`, `platz`, `hauptthema` am Einfall); ältere Dateien bleiben
  lesbar. Migration 0052 (nur neue Spalten).

### Word und Beenden (Runde 32)

- **Word-Import:** „Baruch ▾ → Word (.docx) importieren…“ liest eine Word-Datei, zeigt Gliederung, Bearbeiter und
  Hinweise und legt nach Bestätigung ein Werk an. Fassung 1 ist der Stand vor allen verfolgten Änderungen; die
  Änderungen jedes Bearbeiters werden offene Vorschläge unter `@word:<name>`, Kommentare werden Beiträge an der
  kommentierten Stelle – nie eine aktuelle Fassung. Auch über RPC (`docx_import.preview/get/apply/discard`) und die
  Kommandozeile (`baruch-server import DATEI.docx [--uebernehmen]`); nicht über MCP.
- **Word-Export für den Verlag:** Buch → Export → „Word …“ (`/werke/<id>/word`), RPC `work.docx_export` und
  `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]` schreiben das Werk als .docx: Titelei aus den
  Werkangaben, Formatvorlagen „Überschrift 1–3“, „Baruch Teil“ und „Baruch ohne Nummer“, echte Fußnoten, eingebettete
  Bilder mit Alternativtext, Listen, Blockzitate und einfache Tabellen. Was Word nicht abbilden kann, steht als
  Typst-Insel mit wörtlichem Quelltext in der Datei und wird als Warnung genannt. Auf Wunsch liegt je Abschnitt der
  jüngste offene Vorschlag als Änderungsverfolgung über der aktuellen Fassung, mit der Persona als Bearbeiter. Der
  Export ändert nichts am Werk. Das Öffnen in Word prüft nur der Nutzer; mit LibreOffice ist nur eine Umwandlung
  ohne Fenster geprüft.
- **Baruch beenden:** Menü Baruch ▾ → „Baruch beenden…“ mit Rückfrage (`/beenden`, auch ohne JavaScript). Nur am
  eigenen Rechner, nie unter einem Proxy-Namen, nicht über RPC oder MCP. Danach „Baruch ist beendet – dieses
  Fenster kann geschlossen werden“; das Fenster schließt sich, wenn der Browser es zulässt.
- **Sauberes Herunterfahren** auch auf `SIGTERM` (`systemctl stop`, `kill`) und unter Windows auf Strg+Pause,
  das Schließen des Fensters und das Herunterfahren (Windows ungeprüft): laufende Antworten gehen zu Ende
  (höchstens 10 s), die Datenbank wird geschlossen, letzte Zeile „Baruch beendet.“, Status 0. Bisher nur Strg+C.
- **Starter:** öffnen, wenn vorhanden, ein eigenes Fenster eines Chromium-artigen Browsers (`--app`), unter
  Windows Edge; `BARUCH_BROWSER=standard` nimmt den Standardbrowser. Der Starter wartet nach einem Signal auf
  das saubere Ende des Servers.
- **CI:** Webseite wird in zwei Jobs gebaut (`pages-build`) und ausgeliefert (`pages-deploy`).
- **Rundreise:** Eine von Baruch geschriebene Word-Datei liest der Word-Import mit derselben Gliederung und
  demselben Text wieder ein; eine Datei mit Änderungsverfolgung ergibt wieder aktuelle Fassung und offenen
  Vorschlag. Listen, die nicht bei 1 beginnen, behalten beim Import ihre Nummern („3.“, „4.“ …).

## 0.9.0

Erste Version mit fertigen Paketen. Sie fasst die gesamte bisherige Arbeit zusammen (Runden 1 bis 24, begonnen
am 2026-11-05); davor gab es nur den Stand 0.0.1 aus dem Quelltext.

### Verteilung (Runde 24)

- **Pakete:** `.deb` für Debian und Ubuntu (amd64), AppImage (x86_64), `.dmg` für macOS (Apple Silicon und
  Intel) und `.msi` für Windows (x64). Die Pakete für macOS und Windows sind unsigniert und entstehen nur in der
  CI. Jedes Paket enthält die Lizenz, die Lizenzen aller eingebundenen Bibliotheken
  (`THIRD-PARTY-LICENSES.txt`) und den Hinweis auf den Quelltext.
- **APT-Repository** unter `https://baruch.bernau.family/apt/` (Suite `stable`, Komponente `main`); neue
  Versionen kommen dort mit `sudo apt upgrade`.
- **Seite „Über Baruch“** (Baruch ▾ → Über Baruch, auch von Einstellungen → Daten aus): Version, Lizenz mit
  Lizenztext, Quellcode, Webseite, Ort der Drittlizenzen je Paket, Datenordner.
- **Keine Aktualisierungsprüfung:** Baruch fragt nie von sich aus im Netz nach neuen Versionen. Die Über-Seite
  verlinkt die Release-Seite; aktualisiert wird über APT oder von Hand.
- **Daten bleiben beim Benutzer:** Kein Paket legt etwas im Datenordner an, und das Entfernen löscht dort
  nichts. Die systemd-Unit liegt dem `.deb` nur als Beispiel bei; ein Systemdienst wird nicht eingerichtet.
- **Schlüsselbund je System:** unter Linux der Secret Service, unter macOS der Schlüsselbund, unter Windows die
  Anmeldeinformationsverwaltung (die beiden letzten gebaut, aber noch nicht auf einem echten Gerät erprobt).
- **Kommandozeile:** `baruch-server --version`; für den Betrieb hinter einem Reverse-Proxy `--allow-host NAME`
  bzw. `BARUCH_ALLOWED_HOSTS`. Unter einem zusätzlichen Namen gilt eine Anfrage nie als lokal (MCP nur mit
  Token).
- **MultiGPT ohne Vorgabe-Adresse:** Eine neue Instanz hat keine voreingestellte MultiGPT-Adresse mehr; eine
  gespeicherte Verbindung bleibt unverändert.
- **CI und Webseite:** Tests, `clippy` und Lizenzprüfung (`cargo deny`) in GitHub Actions; ein Release-Ablauf
  baut die Pakete, das APT-Repository und die Webseite `https://baruch.bernau.family/`. Signiert wird nur in
  einer geschützten Umgebung und nur, wenn der veröffentlichte Fingerabdruck zum Archivschlüssel passt.
- **Beispielwerk:** Eine neue Instanz bekommt das frei erfundene „Beispielwerk: Leise Abhängigkeit (Auszug)“
  von „Erika Mustermann“. Vorhandene Werke ändern sich nicht.

- **Abhängigkeiten (Sicherheitsmeldungen):** quick-xml 0.38 → 0.42, zip 2.4 → 9.8, url 2.5.8 → 2.5.9; verbleibende
  RustSec-Meldungen ohne verfügbaren Fix (quick-xml über citationberg, libsqlite3-sys über sqlx 0.8, chrono)
  sind in `deny.toml` begründet ausgenommen.

### Werke, Gliederung und Fassungen

- Vor Runde 1 (Umbau): lokaler Server (axum) mit Seiten, die auch ohne JavaScript bedienbar sind, und
  TypeScript-Widgets statt einer Oberfläche in Iced.
- Runde 1: SQLite als Quelle aller Daten, Git-Spiegel je Werk, Autorenbibliothek mit Volltextsuche (FTS5).
- Runde 2: MCP-Server mit echten Werkzeugen, Import und Export von Typst, mehrere Werke nebeneinander.
- Runde 3: Fassungen als Baum – genau eine ist aktuell, jederzeit umschaltbar; Literatur als Popup,
  Hervorhebungen.
- Runde 4: Werke anlegen, ändern und löschen, Import per Hochladen, eindeutige Titel, Gliederungsmanager.
- Runde 5: eine Gliederung als Baum, ein Klick öffnet den Editor im Satzbild.
- Runde 6: Gliederungsschemata (Dezimal, römisch, Predigt, Buch, eigenes Muster …), Bilder.
- Runde 8 (Nachgang): Gliederung über MCP – Anlegen sofort, Umbenennen, Verschieben, Umstufen und Löschen nur
  als Antrag, den der Mensch bestätigt.
- Runde 14 (mit Nachgang): Teile als eigene Ebene, „ohne Nummer“, Markdown-Überschriften beim Tippen als
  Gliederung; Zeitangaben in Ortszeit.
- Runde 22: Fortschreiben der eigenen Fassung bei kleinen Änderungen (statt jedes Mal einer neuen Fassung) mit
  Verlauf und Zurücknehmen; Archiv gelöschter Werke; Entwurf als Wasserzeichen.
- Ältere Fassungen ausblenden: an jeder Fassung „bis hierhin ausblenden“ (Akkordeon „Fassungen“, Editor-Kopf); ersetzte
  und verworfene Fassungen darunter stehen hinter dem Schalter „n ältere Fassungen zeigen ▸ / verbergen ▾“, der Zähler
  zeigt „2/3 (+ 24 ausgeblendet)“ und ‹ › überspringt sie. Die aktuelle Fassung und offene Vorschläge bleiben immer
  sichtbar, nichts wird gelöscht; „Wieder alle zeigen“ hebt die Marke auf. Werkweit unter Einstellungen → Werk →
  Fassungen „Ältere Fassungen ausblenden bis …“ (Datum). Nur der Mensch, nicht über MCP; Migration 0051.

### Schreiben

- Runde 5 bis 8: Satzbild-Editor (Absätze, Überschriften, Szenen, Bilder, Fußnoten) mit verlustfreier
  Rundreise nach Typst; Vorschläge der Assistenten an ihrer Stelle im Lesefluss, in der Farbe der Persona.
- Runde 7: Einfälle mit Schlagwörtern, frei oder an einem Abschnitt, auch über MCP.
- Runde 9: eine Kopfzeile mit den Menüs Werk, Ansicht und Werkzeuge; Einstellungen als Popup und als Seiten
  ohne JavaScript; Verbindungen mit Schlüsselbund.
- Runde 18: Menüband mit den Formaten der Werkvorlage, Listen, Begriffslisten, Links, „Satz abbrechen“.
- Runde 25: Gedankenkarte der Einfälle mit Verbindungen, Nähe und Ausgabe als SVG, PDF, Markdown und FreeMind.
- Runde 30: Suchen und Ersetzen im Abschnitt und im ganzen Werk (auch mit regulären Ausdrücken, mit
  Zurücknehmen), Querverweise mit festen Sprungmarken.
- Runde 31: Stichwort-, Personen- und Bibelstellenregister, Glossar, Kontextmenü im Text.

### Assistenten

- Runde 10: Personas rufen wirklich ein Modell auf (OpenAI-kompatibel, gestreamt, mit Abbrechen); Prüfen,
  Vorschlagen und Fragen im Abschnitts-Chat; MCP `ask_persona` und `chat`.
- Runde 15: der Lektor liest ganze Kapitel (bei Überlänge überlappend gestückelt, nie still gekürzt), eigene
  Verbindung je Persona (auch Ollama; externe Anbieter nur nach Freigabe), Fäden ohne Abschnitt.
- Runde 28: Befunde als eigene Datensätze mit Status (übernommen, verworfen, verschoben, erledigt), Übersicht
  übers Werk, Aufträge im Chat (`@alle`, RFC), Beiträge kopieren, zitieren und wiederholen.
- Runde 30: Lösungen statt nur Kritik („statt … / besser …“), Varianten nebeneinander vergleichen.
- Runde 22: Personas fragen nach, ob ein Befund erledigt ist.
- Fehler behoben (Kritik-Zyklus): Ein Kritiker kann den Vorschlag des Lektors gegenprüfen und der Lektor darauf
  weiterschreiben – die Prüfgrundlage bietet die offenen Vorschläge an, „@persona …“ im Chat prüft die eigenen offenen
  Befunde nach, und ein Vorschlag beruht auf der gezeigten bzw. gewählten Fassung statt stets auf der aktuellen.

### Bibel und Predigt

- SWORD (neben Runde 11): lokale SWORD-Bibeln lesen, Bibel je Werk, Wortlaut von Zitaten prüfen.
- Runde 16: Predigtvorlage, Sprechdauer und Zielzeit, Stichwortzettel als A5-Blatt oder Karteikarten.
- Runde 15: SWORD-Kommentare und -Lexika zum Nachschlagen.
- Runde 17: alle 18 Verszählungen des SWORD-Projekts mit Umrechnung, Stellen in Listen und Sätzen erkennen
  („Psalm 139 … Vers 23“), komprimierte und verschlüsselte Module, Prüfanzeige der Zitate im Editor.
- Runde 19: Wortlautprüfung auch bei Trennresten im Modul, Bibelseite in anderer Zählung.

### Literatur und Suche

- Runde 12: Bedeutungssuche (RAG) mit lokalem Vektorindex (LanceDB), wahlweise pgvector und MultiGPT als
  weitere Wege.
- Runde 13: Dokumente aus MultiGPT lesen und übernehmen, Websuche mit Belegen, Abgleich nur auf Knopfdruck.
- Runde 19: Pflege des Index ohne lange Sperre, ehrlicher Status, schwache Treffer gekennzeichnet.

### Satz, Buch und E-Book

- Runde 8: Typst-Satz mit der Werkvorlage als Vorschau.
- Runde 11: Umschlag (Vorderseite, Rücken aus der Seitenzahl, Rückseite mit EAN-13), Titelei, Buch als PDF.
- Runde 20: EPUB 3 mit Vorschau in neun Reader-Profilen, Strukturprüfung und EPUBCheck (wenn installiert).
- Runde 27: Umschlag-Editor mit frei verschiebbaren Blöcken, Hilfslinien und Druckprüfung; Impressum in der
  Werkvorlage.
- Runde 29: Buchsatz in den Einstellungen (Kapitelanfang, Seitenformat, Ränder, Schrift, Kolumnentitel,
  Schusterjungen und Hurenkinder) mit Vorschau.

### Import, Export und Austausch

- Runde 21: Import von Markdown (auch als ZIP mit Bildern), PDF und EPUB – immer mit Vorschau der Gliederung,
  angelegt erst nach Bestätigung.
- Runde 26: Austauschformat `.baruch` (XML im ZIP) zwischen Instanzen; Einlesen als neues Werk oder als
  Abgleich, bei dem nichts gelöscht oder überschrieben wird.

### Sicherheit und Härtung

- Der Server antwortet nur unter `127.0.0.1`, `[::1]` und `localhost` (seit Runde 24 zusätzlich unter
  ausdrücklich erlaubten Namen); schreibende Aufrufe nimmt er nur von der eigenen Seite an.
- Runde 9: Schlüssel und Token nur im Schlüsselbund des Systems, nie in Dateien.
- Runde 23: Härtung nach Gutachten – Zitat-Werkzeug, Autorennamen und Kontingente der MCP-Clients, Schutz der
  Geheimnisse, strengere Grenzen beim Abruf fremder Seiten.
- Eingelesene Dateien werden mit Grenzen für Größe, Tiefe und entpackte Bytes geprüft (Runden 17, 21, 26).

### Menüband und Laptop-Modus (Runde 33)

- **Menüband:** Die Menüs „Werk“, „Ansicht“, „Werkzeuge“ und die Knopfreihe über dem Blatt sind ein Menüband mit dem
  Menü **Baruch ▾** und den Reitern **Start, Einfügen, Prüfen, Buch, Ansicht**. Jede Funktion bleibt ohne JavaScript
  erreichbar (Reiter als Links `?tab=`, Gruppen als aufklappbare Bereiche).
- **Band ein- und ausklappen:** Ein Klick auf den gewählten Reiter oder auf ▴ klappt das Band zu (gemerkt im Cookie
  `baruch_band`); im zugeklappten Band zeigt ein Reiter seine Gruppen vorübergehend.
- **Abzeichen an den Reitern:** Start zeigt die offenen Vorschläge, Prüfen „N offen“ (Befunde), Buch die offenen
  Anträge zur Gliederung.
- **Seiten im Menüband:** Jede Seite öffnet mit ihrem Reiter (Satz, Umschlag, EPUB, Austausch, Glossar, Register,
  Bilder, Gliederungsschema, Werk bearbeiten: Buch; Befunde, Zitate prüfen, Suche, Querverweise, Literatur, Abgleich:
  Prüfen; Einstellungen: Ansicht; sonst Start). Die Links der Seite stehen dort als benannte Gruppen.
- **Satz-Seite:** „Zurück zum Werk“, „PDF herunterladen“, ‹ Seite n von m › und „Satz abbrechen“ stehen im Reiter
  Buch statt in einem eigenen Band unter der Kopfzeile.
- **Laptop-Modus:** Ansicht → Laptop-Modus „Automatisch / An / Aus“. Automatisch wird das Band unter 1600 px
  Fensterbreite kompakt (Gruppen als Knöpfe mit Popup), unter 1280 px klappen Gliederung und Assistenten zu schmalen
  Leisten ein; ein Klick auf die Leiste holt die Spalte als Schublade über den Text (Esc, Klick daneben oder
  „Schließen“ legt sie weg).
- **Seitenspalten einzeln einklappen** („Einklappen“ in der Spalte oder Ansicht → Seitenleisten); die Wahl wird
  gemerkt. Ansicht → **Zoom zurücksetzen** setzt alle Bereiche auf 100 %.
- **Ohne JavaScript:** Schalter als Links, Schublade über ein Sprungziel. Die Fensterbreite kennt der Server dann
  nicht; den Laptop-Modus stellt man von Hand „An“.
- **Umbenannt:** „Satz“ heißt „Satz-Vorschau“; „Als PDF/Entwurf/EPUB herunterladen“ heißen in der Gruppe Ausgabe
  „PDF“, „Entwurf“, „EPUB“; „Editor“ außerhalb der Arbeitsseite heißt „Zum Text“.
- **Geändert:** Unter 900 px Fensterbreite stehen die Spalten nicht mehr untereinander, sondern werden Leisten.
  Alle Seiten unter dem Menüband haben denselben Seitenrand.
- **Webseite:** Das Bildschirmfoto-Skript kennt eine Fenstergröße je Bild und zeigt die Arbeitsseite zusätzlich auf
  Laptop-Breite (1366×768).

### Bekannte Grenzen

- Die Pakete für macOS und Windows sind unsigniert und wurden noch auf keinem echten Gerät erprobt.
- Kein Paket für arm64 unter Linux; das `.deb` braucht glibc 2.39 oder neuer (Debian 13, Ubuntu 24.04).
- Baruch hat keine Anmeldung: Der Betrieb hinter einem Reverse-Proxy braucht dort einen Zugangsschutz.
- Bibeltexte, Kommentare und Literatur bringt Baruch nicht mit.
