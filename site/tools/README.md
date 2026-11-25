# Bildschirmfotos für die Webseite

Die Seite „Funktionen“ zeigt sieben Bildschirmfotos aus `site/static/bilder/`: sechs in 1400×860 und seit Runde 33
die Arbeitsseite zusätzlich auf Laptop-Breite (1366×768, `arbeitsplatz-laptop.png`). Sie entstehen aus einem
**frei erfundenen Werk** (`beispielwerk.typ`, „Ein Jahr auf Mövenoog“ – Ort, Personen und Text gibt es nicht) in
einem Wegwerf-Datenordner. Die Fotos macht der Integrator; Entwickler nutzen kein Firefox. Fehlen die Dateien,
lässt die Seite die Bilder aus und baut trotzdem ohne Warnung.

## Skript

`bildschirmfotos.py` (nur Python 3, Standardbibliothek), aus dem Hauptordner:

```sh
site/tools/bildschirmfotos.py --liste       # nur zeigen: welches Bild, welche Adresse, welche Fenstergröße
site/tools/bildschirmfotos.py --pruefen     # Server starten, jede Adresse abrufen (200?), Server beenden
site/tools/bildschirmfotos.py --halten      # Server starten und offen lassen (Eingabetaste beendet)
site/tools/bildschirmfotos.py --fotos       # mit Firefox fotografieren nach site/static/bilder/
```

Weitere Angaben: `--server PFAD` (Vorgabe `target/release/baruch-server`, sonst `target/debug/…`, auch
`BARUCH_SERVER`), `--ziel ORDNER`, `--temp ORDNER` (wo der Wegwerf-Datenordner entsteht; Vorgabe `$TMPDIR`).

Was das Skript tut:

1. Wegwerf-Datenordner anlegen, `baruch-server import site/tools/beispielwerk.typ --data <Temp>`.
2. In `<Temp>/baruch.sqlite` die Verbindungen `eliezer` und `multigpt` auf `http://127.0.0.1:9` stellen – erst
   danach startet der Server, sonst bettete er beim Start über die Vorgabe `127.0.0.1:1234` ein (SKILLS.md).
3. `baruch-server serve --listen 127.0.0.1:0 --data <Temp> --secrets memory`, ohne geerbte `BARUCH_*`-Variablen.
4. Über RPC (`revision.save`, wie der Mensch in der Oberfläche) bekommt der Abschnitt „Der erste Morgen“ eine
   zweite Fassung.
5. Adressen zeigen, prüfen oder fotografieren; danach den eigenen Server über seine PID beenden und den
   Wegwerf-Ordner löschen.

Es berührt weder den Dienst des Nutzers (`127.0.0.1:8765`) noch `~/.local/share/baruch` noch den Schlüsselbund
und spricht nur mit `127.0.0.1`.

## Bild → Adresse

`PORT` nennt der Server beim Start, `WERK` der Import (in einem neuen Datenordner 2, weil das mitgelieferte
Beispielwerk die 1 hat), `KEY` ist der Schlüssel des Abschnitts „Der erste Morgen“ (das Skript gibt alles aus).

| Bild | Adresse | Fenster | Zeigt |
|---|---|---|---|
| `arbeitsplatz.png` | `http://127.0.0.1:PORT/?work=WERK&key=KEY&laptop=off` | 1400×860 | Arbeitsplatz: volles Menüband (Reiter Start, Laptop-Modus „Aus“), Gliederung, Editor im Satzbild, Akkordeons |
| `gliederung.png` | `http://127.0.0.1:PORT/gliederung?work=WERK` | 1400×860 | Seite Gliederung |
| `fassungen.png` | `http://127.0.0.1:PORT/?work=WERK&key=KEY&rev=1` | 1400×860 | Abschnitt mit zwei Fassungen, gezeigt ist die ältere |
| `assistenten.png` | `http://127.0.0.1:PORT/einstellungen/assistenten?work=WERK` | 1400×860 | Einstellungen → Assistenten |
| `satz.png` | `http://127.0.0.1:PORT/werke/WERK/satz` | 1400×860 | Satz-Vorschau (Typst), Menüband im Reiter Buch |
| `einstellungen.png` | `http://127.0.0.1:PORT/einstellungen/werk?work=WERK` | 1400×860 | Einstellungen → Werk mit dem Block Satz |
| `arbeitsplatz-laptop.png` | `http://127.0.0.1:PORT/?work=WERK&key=KEY` | 1366×768 | Arbeitsplatz auf Laptop-Breite: Menüband kompakt (Laptop-Modus „automatisch“) |

Unter 1600 px Breite zeichnet der Laptop-Modus („automatisch“) das Menüband kompakt; die Breite misst das Skript der
Seite (`laptop.ts`). Auch 1400 px liegen darunter: Damit die übrigen Bilder das volle Band zeigen, stellt
`arbeitsplatz.png` mit `&laptop=off` den Laptop-Modus „Aus“; das Cookie gilt im Firefox-Profil dieser Fenstergröße
für alle 1400er-Bilder. Das Laptop-Bild fotografiert `--fotos` mit einem eigenen Profil (kein geerbtes Cookie), dort
gilt „automatisch“. Ohne JavaScript kennt der Server die Breite nicht; dann zeigt erst `?laptop=on` das kompakte Band.

## Stand

- **Geprüft** (Aufgabe seiten, Runde 33): `--liste` zeigt sieben Bilder mit Fenstergröße; `--pruefen` – Import,
  Start, zweite Fassung, alle sieben Adressen antworten mit 200, Server und Ordner wieder weg. **Offen** (Integrator,
  nach dem Zusammenführen der Runde 33): `--fotos` – alle sieben Bilder neu, weil jede Seite jetzt das Menüband
  trägt; `arbeitsplatz-laptop.png` gibt es bis dahin nicht (die Seite „Funktionen“ lässt ein fehlendes Bild aus).
  Der Shortcode `bild` schreibt bisher fest `width="1400" height="860"`; für das Laptop-Bild übergibt die Seite
  `breite="1366" hoehe="768"` (der Shortcode muss sie noch auswerten, siehe Bericht der Aufgabe).
- **Geprüft** (Aufgabe webseite, Runde 24): `--pruefen` – Import, Start, zweite Fassung, alle sechs Adressen
  antworten mit 200, Server und Ordner wieder weg.
- **Geprüft** (Integrator, Runde 24): `--fotos` – die sechs Bilder unter `site/static/bilder/` sind so entstanden
  (Server aus `target/release`, `--temp` auf einen Ordner unter `/home`). Es ruft je Bild
  `firefox --headless --no-remote --profile <eigener Temp-Ordner> --window-size=1400,860 --screenshot DATEI ADRESSE`
  auf. Nie parallel zu Testläufen. `fassungen.png` zeigt die ältere Fassung mit dem Umschalter 1/2; das Akkordeon
  „Fassungen“ ist zugeklappt.
- Hinweise für den Integrator:
  - Ob im Foto das Akkordeon „Fassungen“ offen ist, hängt an Cookies der Oberfläche (`baruch_*`); für
    `fassungen.png` gegebenenfalls nach SKILLS.md („Cookies … für ein Foto“) nachhelfen oder mit `--halten`
    von Hand fotografieren.
  - Die Seite Assistenten zeigt die mitgelieferten Personas; ihre Modell-Verbindung zeigt auf `127.0.0.1:9`
    (niemand antwortet) – auf dem Foto steht also keine echte Adresse.
  - Die Werkliste des Wegwerf-Ordners enthält zusätzlich das mitgelieferte Beispielwerk.
  - Nach dem Fotografieren `hugo --minify --panicOnWarning -s site` wiederholen und die sieben PNG-Dateien
    einchecken.
