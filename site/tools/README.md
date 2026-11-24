# Bildschirmfotos für die Webseite

Die Seite „Funktionen“ zeigt sechs Bildschirmfotos (1400×860) aus `site/static/bilder/`. Sie entstehen aus einem
**frei erfundenen Werk** (`beispielwerk.typ`, „Ein Jahr auf Mövenoog“ – Ort, Personen und Text gibt es nicht) in
einem Wegwerf-Datenordner. Die Fotos macht der Integrator; Entwickler nutzen kein Firefox. Fehlen die Dateien,
lässt die Seite die Bilder aus und baut trotzdem ohne Warnung.

## Skript

`bildschirmfotos.py` (nur Python 3, Standardbibliothek), aus dem Hauptordner:

```sh
site/tools/bildschirmfotos.py --liste       # nur zeigen: welches Bild, welche Adresse
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

| Bild | Adresse | Zeigt |
|---|---|---|
| `arbeitsplatz.png` | `http://127.0.0.1:PORT/?work=WERK&key=KEY` | Arbeitsplatz: Gliederung, Editor im Satzbild, Akkordeons |
| `gliederung.png` | `http://127.0.0.1:PORT/gliederung?work=WERK` | Seite Gliederung |
| `fassungen.png` | `http://127.0.0.1:PORT/?work=WERK&key=KEY&rev=1` | Abschnitt mit zwei Fassungen, gezeigt ist die ältere |
| `assistenten.png` | `http://127.0.0.1:PORT/einstellungen/assistenten?work=WERK` | Einstellungen → Assistenten |
| `satz.png` | `http://127.0.0.1:PORT/werke/WERK/satz` | Satz-Vorschau (Typst) |
| `einstellungen.png` | `http://127.0.0.1:PORT/einstellungen/werk?work=WERK` | Einstellungen → Werk mit dem Block Satz |

## Stand

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
  - Nach dem Fotografieren `hugo --minify --panicOnWarning -s site` wiederholen und die sechs PNG-Dateien
    einchecken.
