# Bildschirmfotos für die Webseite

Die Webseite zeigt zwei Gruppen von Bildschirmfotos:

- **Bilder der Hilfe** – `site/static/bilder/hilfe/<name>.png`, dunkle Fassung daneben als `<name>-dunkel.png`. Die
  Liste steht in `site/data/hilfebilder.toml` (94 Namen); die Hilfeseiten binden sie mit dem Shortcode `hilfebild`
  ein. Fehlt eine Datei, lässt die Seite das Bild aus und baut trotzdem ohne Warnung.
- **Sieben Bilder der Seite „Funktionen“** – `site/static/bilder/*.png`. Namen und Adressen stehen im Skript
  (`FUNCTION_PICTURES`), unverändert seit Runde 24 bzw. 33.

Alle entstehen aus einem **frei erfundenen Werk** (`beispielwerk.typ`, „Ein Jahr auf Mövenoog“ von Mara Tiedemann –
Ort, Personen und Text gibt es nicht) in einem Wegwerf-Datenordner. Die Fotos macht der Integrator; Entwickler
arbeiten mit `--liste`, `--pruefen` und `--abgleich` und starten nie Firefox.

## Aufruf

`bildschirmfotos.py` braucht nur Python 3.11 oder neuer (Standardbibliothek, `tomllib`), aus dem Hauptordner:

```sh
site/tools/bildschirmfotos.py --liste                    # ohne Server: jedes Bild mit Adresse, Fenster, Thema, Schritten
site/tools/bildschirmfotos.py --abgleich                 # ohne Server: Namen der Seiten gegen die Liste, Dateien, Größe
site/tools/bildschirmfotos.py --pruefen --temp ~/tmp     # Server, Beispielwerk, jede Adresse über den Vermittler
site/tools/bildschirmfotos.py --halten  --temp ~/tmp     # wie --pruefen, hält je Stufe an (Eingabetaste geht weiter)
site/tools/bildschirmfotos.py --fotos   --temp ~/tmp     # mit Firefox fotografieren – nur der Integrator
```

| Angabe | Bedeutung |
|---|---|
| `--nur NAME[,NAME]` | nur diese Bilder (Namen wie in `--liste`); zum Nachfotografieren. Das Beispielwerk wird trotzdem ganz aufgebaut. |
| `--sword ORDNER` | Ordner mit Bibelmodulen (`mods.d`, `modules`) für Bilder mit `braucht = ["sword"]`. Ohne die Angabe nennt das Skript sie als übersprungen. |
| `--temp ORDNER` | wo Wegwerf-Datenordner und Firefox-Profile entstehen. **Immer einen Ordner auf einer Platte angeben** – `/tmp` ist hier Arbeitsspeicher. |
| `--server PFAD` | `baruch-server` (Vorgabe `target/release/…`, sonst `target/debug/…`, auch `BARUCH_SERVER`). |
| `--ziel ORDNER` | wohin die Bilder kommen (Vorgabe `site/static/bilder`; die Hilfe darunter in `hilfe/`). |
| `--breite N` | größte Breite der fertigen Bilder (Vorgabe 1400). |
| `--ohne-palette` | Bilder nicht auf 256 Farben verringern (siehe „Größe“). |

`--abgleich` endet mit Exit 1, wenn eine Seite einen Namen verwendet, der nicht in der Liste steht, oder wenn
`site/static/bilder` größer als 20 MB ist. Ungenutzte Namen, Bilder auf einer anderen Seite als in der Liste
vorgesehen, fehlende Dateien und Dateien ohne Eintrag sind nur Listen. `--pruefen` endet mit Exit 1, sobald eine
Aufnahme nicht stimmt.

## Ablauf

1. Wegwerf-Datenordner unter `--temp` anlegen, `baruch-server import site/tools/beispielwerk.typ --data <Ordner>`.
2. In `<Ordner>/baruch.sqlite` die Verbindungen `eliezer` und `multigpt` auf `http://127.0.0.1:9` stellen (dort
   antwortet niemand) – **erst danach** startet der Server, sonst bettete er beim Start über die Vorgabe-Adresse ein.
3. `baruch-server serve --listen 127.0.0.1:0 --data <Ordner> --secrets memory`, ohne geerbte `BARUCH_*`-Variablen.
4. Vermittler starten (siehe unten).
5. **Stufe „offen“** – noch ohne Konten: Beispielwerk ausbauen, dann die Bilder dieser Stufe.
6. **Stufe „konten“** – Autorin und Lektor anlegen, Werk freigeben, der Lektor schreibt Befunde; dann die Bilder
   dieser Stufe.
7. Den eigenen Server über seine PID beenden, den Wegwerf-Ordner löschen (auch nach einem Fehler).

Das Skript berührt weder den Dienst des Nutzers (`127.0.0.1:8765`) noch `~/.local/share/baruch` noch den
Schlüsselbund, spricht nur mit `127.0.0.1` (jede Anfrage prüft das) und fragt kein Modell. Die Bibelmodule des
Nutzers (`~/.sword`, `/usr/share/sword`) liest es nicht: Gleich nach dem Start setzt es den Ordner der Module auf
einen leeren Ordner im Wegwerf-Ordner bzw. auf `--sword ORDNER`.

### Was der Aufbau anlegt

Alles über Schnittstellen des eigenen Servers, nichts direkt in der Datenbank (außer den zwei Verbindungen in
Schritt 2):

| Was | Weg | Für die Bilder |
|---|---|---|
| zweite Fassung von „Der erste Morgen“ | RPC `revision.save` (wie der Mensch) | Fassungen, ältere Fassung, Verlauf |
| Werkbild `leuchtturm.png`, Umschlag `umschlag.png` (im Skript mit `zlib`/`struct` gezeichnet) | RPC `asset.add`, `work.set_cover` | Bilder, Bild einfügen, Umschlag |
| Bild im Text, vier Registermarken, Querverweis | `revision.save`, `index.mark` | Satzbild, Register, Verweise |
| drei Glossarbegriffe | `glossary.add` | Glossar, Markierung |
| Buchangaben (erfundener Verlag, erfundene ISBN aus 978-3-99999), Satz-Einstellungen, Zielzeit | `work.meta.set`, `work.typeset_set`, `work.timing_set` | Buchangaben, Satz, Zeitwächter |
| Einfälle: zwei Hauptthemen mit Unterknoten, einer am Abschnitt, zwei in der Wolke, zwei Verbindungen | `idea.create`, `idea.link` | Einfälle, Mindmap |
| Beiträge, ein Vorschlag und ein Gliederungsantrag eines Assistenten | **MCP am eigenen Server** (`add_post`, `propose_revision` mit `persona: "lektor"`, `outline_rename`) | Chat, Vorschlag im Lesefluss, Anträge |
| ein Faden mit drei Beiträgen | RPC `thread.post`, MCP `chat` | Fäden |
| Quelle in der Bibliothek (ein erfundenes Dienstbuch als kleines EPUB, im Skript gepackt) | `source_import.preview`, `source_import.apply` mit `target: "library"` | Literatur, Quelle lesen |
| Wegwerf-Werk „Notizen vom Festland“ (Markdown, ein Bild fehlt absichtlich), danach gelöscht | Formular `POST /werke/import`, RPC `work.delete` | Bericht nach dem Einlesen, Archiv |
| Vorschauen des Einlesens aus **Ausgaben des eigenen Servers** | `GET /werke/W/word.docx`, `/pdf`, `/austausch.baruch`, zurück an die Formulare | Word-, PDF- und Austausch-Vorschau |
| Konten Mara Tiedemann (Autorin) und Jonte Harksen (Lektor), Freigabe | Formulare `/einrichten`, `/einstellungen/konten/act`, `/anmelden`, RPC `member.grant` | Konten |
| fünf Befunde (Hinweis, kritisch, Lob, einer ohne Abschnitt), ein Vorschlag und ein Beitrag des Lektors | RPC `finding.add`, `revision.propose`, `post.add` **mit der Sitzung des Lektors** | Befunde, Abzeichen „3 offen“, Arbeitsseite des Lektors |

**Warum Beiträge und Vorschlag über MCP?** RPC `post.add` schreibt immer als „Autor“, `revision.propose` und
`finding.add` sind den Lektoren vorbehalten. Der MCP-Zugang des eigenen Servers nimmt von `127.0.0.1` ohne Token
Beiträge und Vorschläge an und schreibt sie mit `persona: "lektor"` der Rolle Lektor zu – ohne Modell, ohne Eingriff
in die Datenbank. Die Fäden antwortet der Client unter seinem eigenen Namen (`@mcp:schreibhilfe`). Kein Beitrag
erwähnt einen Assistenten; `thread.post` ruft so kein Modell (das Skript bricht ab, falls doch eine Antwort käme).

**Warum Befunde erst mit Konten?** `finding.add` gibt es nur für Lektoren. Darum liegen die Bilder mit Befunden
(`band-pruefen`, `assistenten-befunde-liste`, `assistenten-befunde-abschnitt`) in der Stufe „konten“ und zeigen die
Seite, wie die angemeldete Autorin sie sieht.

## Die Bildliste und ihre Felder

`site/data/hilfebilder.toml`, je Bild ein Block `[[bild]]`. Die Felder `name`, `seite`, `zeigt`, `dunkel`, `braucht`
stammen aus der Schnittstelle der Runde 37; niemand benennt um oder streicht, neue Namen trägt nur der Integrator
nach. Die Felder des Werkzeugs:

| Feld | Bedeutung |
|---|---|
| `adresse` | Pfad am Wegwerf-Server, beginnt mit `/`. Platzhalter siehe unten. `/_antwort/<name>` ist die gespeicherte Antwort eines Formulars. |
| `fenster` | `"BREITExHÖHE"`, Vorgabe `1400x860`. |
| `thema` | `"hell"` (Vorgabe) oder `"dunkel"` für die Hauptaufnahme. `dunkel = true` ergibt **zusätzlich** `<name>-dunkel.png`. |
| `ausschnitt` | `"BxH+X+Y"` (fest) oder `"css:SELEKTOR"` (das Element wird im Browser gemessen). Fehlt = ganzes Fenster. |
| `schritt` | Liste: `"cookie:NAME=WERT"` (leerer Wert = Cookie weglassen), `"offen:TEXT"` (jedes `<details>`, dessen öffnendes Tag TEXT enthält, bekommt `open`), `"klick:SELEKTOR"`, `"sitzung:autorin"` / `"sitzung:lektor"`. |
| `erwartet` | Texte, die `--pruefen` in der Antwort verlangt (roh oder HTML-maskiert). |
| `stufe` | `"offen"` (Vorgabe) oder `"konten"`. |

Platzhalter in `adresse`, `erwartet` und Cookie-Werten: `{work}` (Beispielwerk), `{sample}` (mitgeliefertes
Beispielwerk), `{key}` („Der erste Morgen“: zwei Fassungen, Beiträge), `{key_logbook}` („Das Logbuch“: Zitat, Bild,
Registermarken), `{key_proposal}` und `{rev_proposal}` („Besuch vom Festland“ mit dem Vorschlag eines Assistenten),
`{key_xref}` und `{rev_xref}` („Was das Licht nicht zeigt“: Bibelstelle, Querverweis), `{key_night}` („Die Nacht ohne
Lampe“: Befund, Vorschlag des Lektors), `{key_farewell}` („Abschied“), `{idea}`, `{branch}`, `{request}`, `{thread}`,
`{post}`, `{title}`. `--pruefen` gibt alle Werte aus. Ein unbekannter Platzhalter oder ein falsches Feld bricht
schon `--liste` ab.

Was `braucht` aus der Schnittstelle im Skript bedeutet: `ohne-js` – Firefox-Profil mit
`javascript.enabled = false` (dann kein `klick:` und kein `css:`-Ausschnitt); `sword` – nur mit `--sword`;
`konten` – die Stufe legt allein `stufe` fest („Konten einrichten“ gibt es nur, **bevor** Konten bestehen, darum
`stufe = "offen"`); `klick` – ein Hinweis, das Skript löst ihn je Bild über `offen:`, ein Cookie oder `klick:`.

Vorgaben für jedes Bild: Cookie `baruch_work={work}` und `baruch_laptop=off` (1400 px liegen unter der Schwelle für
das kompakte Menüband; ohne das Cookie zeichnete der Laptop-Modus „automatisch“ das Band kompakt). Die beiden
Laptop-Bilder nehmen das Cookie mit `"cookie:baruch_laptop="` wieder weg.

## Vermittler

`firefox --headless --screenshot` kann nichts anklicken, keine Cookies setzen und keine Antwort eines Formulars
zeigen. Darum läuft **jede** Aufnahme über einen Vermittler im Skript (`http.server` auf `127.0.0.1:0` vor dem
Wegwerf-Server). Die Adresse der Seite trägt `_bild=<Aufnahme>`; daran erkennt er das Rezept, alle weiteren Anfragen
der Seite (Stile, Skripte, RPC der Widgets) laufen mit denselben Cookies durch.

- **Cookies:** an den Server (`Cookie`) **und** an den Browser (`Set-Cookie`) – die Widgets lesen `document.cookie`.
  Verwaltete Cookies, die das Rezept nicht nennt, löscht er im Browser.
- **Sitzung:** `baruch_session` ergänzt er nur zum Server hin. Das Token gelangt nie in den Browser, nie in eine
  Ausgabe; `--pruefen` verlangt, dass keine Seite es enthält. Die Passwörter entstehen je Lauf zufällig.
- **`offen:`** setzt `open` im HTML – das geht auch ohne JavaScript.
- **`klick:`** und **`css:`-Ausschnitt:** ein eingefügtes Skript klickt 0,5 s nach `DOMContentLoaded` und misst das
  Element nach 2,5 s (meldet über `/_mass`); ein nachgeladenes Bild (`/_warte?ms=3500`) hält das `load`-Ereignis
  auf, auf das `--screenshot` wartet. Findet das Skript sein Ziel nicht, meldet es das und `--fotos` nennt es.
- **`/_antwort/<name>`:** liefert die gespeicherte Antwort eines Formulars aus (Bericht, Vorschauen).
- **Host, Origin, Referer** tragen zum Server hin dessen Adresse – sonst lehnt er Formulare und RPC mit 403 ab
  (`--pruefen` belegt beides: über den Vermittler 200, ohne ihn 403).
- Weiterleitungen behalten `_bild`; `Set-Cookie` der Sitzung wird nicht weitergereicht.

Je Aufnahme entsteht ein **eigenes Firefox-Profil** (kein geerbtes Cookie, keine gemerkte Fensterbreite); für die
dunkle Fassung mit `layout.css.prefers-color-scheme.content-override = 0` und `ui.systemUsesDarkTheme = 1`, dazu das
Cookie `baruch_theme=dark`.

## Größe

Alle Bilder zusammen (`site/static/bilder`) bleiben unter **20 MB**; `--abgleich` und `--fotos` nennen die Summe.
Nach jedem Foto bearbeitet das Skript das Bild mit `magick` (bzw. `convert`): Ausschnitt, Breite höchstens
`--breite`, Metadaten entfernt, 256 Farben ohne Rasterung, stärkste PNG-Kompression. An einem vorhandenen Foto
gemessen: 192 KB → 53 KB. Ohne `magick` meldet das Skript das und lässt die Bilder unbearbeitet – 104 Aufnahmen
lägen dann nahe an der Grenze. Wirkt ein Bild mit Palette fleckig (Verläufe, das Umschlagbild), dieses eine mit
`--nur NAME --ohne-palette` wiederholen.

## Stand der Prüfung

**Belegt durch `--pruefen`** (Runde 37, Aufgabe bilder, Server aus `target/debug`): Import, Start, Aufbau beider
Stufen; alle 103 Aufnahmen der 100 Bilder ohne Bibelmodul (mit `--sword <leerer Ordner>` alle 104 der 101)
antworten über den Vermittler mit 200 und Inhalt, jeder Text aus `erwartet` steht in der Antwort, jedes `offen:` hat
ein `<details>` getroffen, die dunklen Fassungen tragen `data-theme="dark"`, jedes Ziel von `klick:` und `css:`
kommt im HTML vor; dazu die Probe des Vermittlers (Stylesheet, RPC mit umgeschriebener Herkunft samt Gegenprobe 403,
Warte-Bild, Messung, Cookies, unbekannte Antwort). Danach sind Server und Wegwerf-Ordner weg.

**Nicht belegt – das zeigen erst die Fotos:**

- Ob ein **Klick wirkt** (`einfaelle-festhalten`, `einstellungen-popup`). Findet das Skript sein Ziel nicht, sagt
  `--fotos` es; ob danach das Richtige offen ist, zeigt nur das Bild.
- Die **Messung** für `css:`-Ausschnitte bei `#pane-assistants` und im kompakten Band. Kommt keine, bleibt das Bild
  ganz und `--fotos` sagt es.
- Der **feste Ausschnitt** `band-eingeklappt` (1400×320) ist geschätzt; nach dem ersten Foto ansehen und in der
  Liste anpassen.
- Ob **Anker** (`#satz`, `#buchangaben`, `#fassungen-ausblenden`, `#kommentare`, `#antraege`, `#bild-einfuegen`) den
  Block ins Bild rücken.
- Ob das Bild das Motiv **gut** zeigt (Fenstergröße, was aufgeklappt ist), ob Palette und Verkleinern sauber wirken,
  die Gesamtgröße.
- `band-ohne-javascript` und `mindmap-ohne-javascript` (Profil ohne JavaScript).
- `bibel-nachschlagen` mit **echten Modulen**: geprüft ist nur, dass die Seite mit einem leeren Ordner 200 liefert.
  `bibel-zitate-pruefen` zeigt ohne Module ehrlich „ungeprüft“; `bibel-einstellungen` zeigt dann keine Module.

**Am Rande gesehen, nicht geplant:** Bei einer Probe von `--fotos`, die mit einer Attrappe statt Firefox laufen
sollte, lief versehentlich der echte Firefox für sieben Aufnahmen (nichts davon ist eingecheckt; siehe Bericht der
Aufgabe). Dabei kam die Messung an (`header.band` 1400×132, `#pane-tree` 278×716), das Menü „Baruch ▾“ stand offen
mit dem Eintrag „Hilfe“ vor „Über Baruch“ (danach der Ausschnitt 560×400), die dunkle Fassung der Arbeitsseite war
dunkel, und das Bild ohne JavaScript entstand. Das ersetzt die Durchsicht aller Fotos nicht.

Bewusst so:

- `verweise-seite` zeigt „Keine kaputten Verweise“ – ein kaputter Verweis im Beispielwerk bräche den Satz (PDF).
- `assistenten-websuche` zeigt die Belege-Seite eines Beitrags ohne Websuche (ehrlicher Leerstand), `abgleich-seite`
  den Zustand ohne erreichbares MultiGPT, `einstellungen-mcp` die Seite **ohne** angelegtes Token (auch der Anfang
  eines Tokens soll auf keinem Bild stehen).
- `installation-erster-start` zeigt das mitgelieferte Beispielwerk; in der Werkauswahl steht zusätzlich das
  Mövenoog-Werk (zugeklappt nicht zu sehen).
- Die Werkliste enthält neben „Ein Jahr auf Mövenoog“ das mitgelieferte Beispielwerk (erfundene Autorin).

## Hinweise für den Integrator

1. Erst der volle Testlauf, **danach** und nie parallel: `--pruefen`, dann `--fotos` (beides mit `--temp` auf einer
   Platte, Server am besten aus `target/release`).
2. `--fotos` nennt je Aufnahme Größe und Auffälligkeiten („ACHTUNG: …“). Einzelne Bilder mit `--nur NAME[,NAME]`
   wiederholen; `--halten` gibt die Adressen am Vermittler aus und hält je Stufe an – so lässt sich eine Seite im
   eigenen Browser ansehen (nur solange das Skript läuft).
3. Danach `--abgleich` (Namen, fehlende Dateien, Größe) und `hugo --minify --panicOnWarning -s site`.
4. Die sieben Bilder der Seite „Funktionen“ entstehen mit; ihr Werk hat jetzt mehr Inhalt (Bild, Zitat, Beiträge).
   Wer sie nicht erneuern will, fotografiert nur die Hilfe: `--ziel` auf einen anderen Ordner und `hilfe/` kopieren.
5. Vorhandene Dateien löscht das Skript nie; ein Foto überschreibt nur seine eigene Datei.
