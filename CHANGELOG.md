# Änderungen

Alle nennenswerten Änderungen an Baruch, die neueste Version zuerst. Die Versionsnummer legt der Autor fest
(`[workspace.package] version` in `Cargo.toml`). Einzelheiten je Runde – Commits, Dauer, Befunde – stehen in
[docs/Fahrplan.md](docs/Fahrplan.md), offene Punkte in [docs/Backlog.md](docs/Backlog.md).

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
- **Seite „Über Baruch“** (Werkzeuge ▾ → Über Baruch, auch von Einstellungen → Daten aus): Version, Lizenz mit
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

### Bekannte Grenzen

- Die Pakete für macOS und Windows sind unsigniert und wurden noch auf keinem echten Gerät erprobt.
- Kein Paket für arm64 unter Linux; das `.deb` braucht glibc 2.39 oder neuer (Debian 13, Ubuntu 24.04).
- Baruch hat keine Anmeldung: Der Betrieb hinter einem Reverse-Proxy braucht dort einen Zugangsschutz.
- Bibeltexte, Kommentare und Literatur bringt Baruch nicht mit.
