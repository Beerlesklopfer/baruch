# Bedienung von Baruch

**Die Bedienung steht in der Hilfe auf der Webseite:** <https://baruch.bernau.family/hilfe/> – im Repository die
Seiten unter [`site/content/hilfe/`](../site/content/hilfe/). Sie ist die einzige Beschreibung für Anwender, in
Alltagssprache, Schritt für Schritt und mit Bildern. Für Betreiber gilt entsprechend
[`site/content/betrieb.md`](../site/content/betrieb.md) (Einstieg: [`docs/Betrieb.md`](Betrieb.md)).

Diese Datei enthält nur noch:

1. die Tabelle **„Wo steht was“** – jede frühere Überschrift dieser Datei mit ihrem neuen Ort,
2. die **Regel für Entwickler**,
3. **Technische Anmerkungen** je Hilfeseite: was kein Anwender braucht (Adressen und Routen, Cookie-Namen,
   Parameter, Commits im Git-Spiegel, RPC- und MCP-Namen, Grenzen der Prüfung, Annahmen zu Ungeprüftem).

Bedienschritte stehen hier nicht mehr – nie in beiden Dateien.

## Regel für Entwickler

„Bedienung nachziehen“ (AGENTS.md: Doku wächst mit dem Code) heißt seit Runde 37: **die zuständige Hilfeseite unter
`site/content/hilfe/` im selben Commit ändern.** Technisches zur selben Änderung kommt in die „Technischen
Anmerkungen“ unten. Für die Hilfeseiten gilt:

- Alltagssprache: Fassung statt Revision, Assistent (Rolle) statt Persona; Fachwörter (MCP, Token, SWORD, Typst,
  Reverse-Proxy …) beim ersten Vorkommen in einem Halbsatz erklären. Keine Rundennummern, keine Dateinamen des
  Quelltexts, keine Routen (nur Adressen, die man wirklich eintippt).
- Tasten immer für beide Welten („Strg+S (Mac: ⌘S)“), die Mac-Taste aus dem Code belegt; `tasten.md` sammelt alle.
- Jede Überschrift trägt einen festen Anker `{#…}`; jede `##`-Überschrift hat ein Bild
  (`{{< hilfebild name="…" alt="…" titel="…" >}}`, Namen nur aus `site/data/hilfebilder.toml`) oder steht im Kopf
  der Seite unter `ohne_bild`. Kein rohes HTML, auch keine HTML-Kommentare (der Bau bricht ab).
- Verweise nur als gewöhnliche Links ab `/`; auf `/installation/`, `/mcp/`, `/betrieb/` nur auf die Seite oder
  ihre festen Anker. Kein `doku`-Shortcode und kein Link zu GitHub in der Hilfe.
- Beispiele frei erfunden; das Beispielwerk der Hilfe heißt „Ein Jahr auf Mövenoog“ von Mara Tiedemann.
- Prüfen: `hugo --minify --panicOnWarning -s site` und `cargo test -p baruch-server --test server round37_hilfe`.

Kommentare im Rust-Code, die noch `docs/Bedienung.md „…“` nennen, meinen den Abschnitt aus der Tabelle unten.

## Wo steht was

Jede `##`-Überschrift der früheren Fassung dieser Datei (Stand `00537dd`, `git show 00537dd:docs/Bedienung.md`):

| Frühere Überschrift | Hilfeseite und Anker |
|---|---|
| Starten | [`erste-schritte.md`](../site/content/hilfe/erste-schritte.md) `#starten`; Datenordner: `/betrieb/#datenordner` |
| Typst- oder Markdown-Datei importieren | [`einlesen.md`](../site/content/hilfe/einlesen.md) `#typst-markdown`, `#markdown`, `#teile`, `#export-import`, `#kommandozeile` |
| PDF oder EPUB importieren (Runde 21a) | [`einlesen.md`](../site/content/hilfe/einlesen.md) `#pdf-epub` |
| Werke | [`erste-schritte.md`](../site/content/hilfe/erste-schritte.md) `#werke`, `#neues-werk`, `#predigt`, `#werk-bearbeiten`, `#loeschen-archiv` |
| Bildschirm | [`erste-schritte.md`](../site/content/hilfe/erste-schritte.md) `#bildschirm` |
| Menüband | [`menueband.md`](../site/content/hilfe/menueband.md) `#menueband`, `#menue-baruch`, `#reiter`, `#band-einklappen`, `#kompaktes-band`, `#laptop-modus`, `#ohne-javascript`; Knöpfe des Editors: [`schreiben.md`](../site/content/hilfe/schreiben.md) `#einfuegen`; Über Baruch: [`erste-schritte.md`](../site/content/hilfe/erste-schritte.md) `#ueber` |
| Zeitangaben | [`erste-schritte.md`](../site/content/hilfe/erste-schritte.md) `#zeitangaben` |
| Adresse | [`erste-schritte.md`](../site/content/hilfe/erste-schritte.md) `#adresse` |
| Gliederung | [`gliederung.md`](../site/content/hilfe/gliederung.md) `#gliederung`, `#ueberschriften`, `#teile`, `#ohne-nummer`, `#seite`, `#antraege`, `#zustimmung-teil`, `#schema`, `#muster` |
| Zeitwächter | [`schreiben.md`](../site/content/hilfe/schreiben.md) `#zeitwaechter` |
| Satzbild | [`schreiben.md`](../site/content/hilfe/schreiben.md) `#satzbild`, `#ansichten`, `#einfuegen`, `#bild-einfuegen` |
| Bilder | [`schreiben.md`](../site/content/hilfe/schreiben.md) `#bilder`, `#bild-einfuegen` |
| Revisionen | [`schreiben.md`](../site/content/hilfe/schreiben.md) `#fassungen`, `#ausblenden` |
| Vorschläge im Lesefluss | [`schreiben.md`](../site/content/hilfe/schreiben.md) `#vorschlaege` |
| Abschnitts-Chat | [`assistenten.md`](../site/content/hilfe/assistenten.md) `#chat` |
| Assistenten fragen | [`assistenten.md`](../site/content/hilfe/assistenten.md) `#pruefen`, `#websuche` |
| Befunde (Runde 28) | [`assistenten.md`](../site/content/hilfe/assistenten.md) `#befunde` |
| Chat bedienen und Aufträge (Runde 28, chat-auftrag) | [`assistenten.md`](../site/content/hilfe/assistenten.md) `#chat`, `#faeden` |
| Suchen und Ersetzen (Runde 30a) | [`suchen.md`](../site/content/hilfe/suchen.md) `#suchen`, `#ersetzen` |
| Querverweise (Runde 30a) | [`suchen.md`](../site/content/hilfe/suchen.md) `#verweise` |
| Lösungsvorschläge und Varianten (Runde 30b) | [`assistenten.md`](../site/content/hilfe/assistenten.md) `#pruefen`, `#loesungen`, `#rueckfrage` |
| Chat: Wiederholen, Löschen, Anrede, Live, gegliederte Beiträge, Sprungmarken (Runde 30c) | [`assistenten.md`](../site/content/hilfe/assistenten.md) `#chat` |
| Bibelstellen | [`bibel-quellen.md`](../site/content/hilfe/bibel-quellen.md) `#zitate-pruefen`, `#nachschlagen` |
| Satz (Typst-Vorschau) | [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#satz`, `#satz-einstellungen` |
| Buch als PDF, Umschlag und Titelei | [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#pdf`, `#umschlag`, `#epub` |
| Stichwortzettel | [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#stichwortzettel` |
| Literatur | [`bibel-quellen.md`](../site/content/hilfe/bibel-quellen.md) `#literatur`, `#bedeutungssuche`, `#quellen`, `#nachschlagen` |
| Einfälle | [`einfaelle.md`](../site/content/hilfe/einfaelle.md) `#festhalten`, `#liste`, `#einfall` |
| Mindmap | [`einfaelle.md`](../site/content/hilfe/einfaelle.md) `#mindmap`, `#wolke`, `#mindmap-tasten`, `#mindmap-gliederung`, `#mindmap-austausch` |
| Fäden | [`assistenten.md`](../site/content/hilfe/assistenten.md) `#faeden` |
| Abgleich mit MultiGPT | [`bibel-quellen.md`](../site/content/hilfe/bibel-quellen.md) `#abgleich` |
| Vergrößern mit Strg+Mausrad | [`menueband.md`](../site/content/hilfe/menueband.md) `#vergroessern` |
| Einstellungen | [`einstellungen.md`](../site/content/hilfe/einstellungen.md) `#oeffnen`, `#darstellung`, `#werk`, `#assistenten`, `#verbindungen`, `#quellen`, `#mcp-zugang`, `#daten`; Satz: [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#satz-einstellungen`; Bibeln (SWORD), Kommentare und Lexika: [`bibel-quellen.md`](../site/content/hilfe/bibel-quellen.md) `#bibeln-waehlen` |
| Sicherheit | [`austausch-sicherung.md`](../site/content/hilfe/austausch-sicherung.md) `#sicherheit`; Betrieb hinter einem Proxy: `/betrieb/#reverse-proxy` |
| Konten und Rollen (Runde 35) | [`konten.md`](../site/content/hilfe/konten.md) `#einrichten`, `#anmelden`, `#lektoren`, `#lektor-rechte`, `#passwort` |
| Austausch zwischen Instanzen | [`austausch-sicherung.md`](../site/content/hilfe/austausch-sicherung.md) `#ausgeben`, `#einlesen`, `#instanz` |
| Umschlag-Editor (Runde 27) | [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#umschlag` |
| Runde 19 – RAG-Pflege, Reste der Stellen | [`bibel-quellen.md`](../site/content/hilfe/bibel-quellen.md) `#bedeutungssuche`, `#zitate-pruefen`, `#nachschlagen`; [`einstellungen.md`](../site/content/hilfe/einstellungen.md) `#verbindungen` |
| EPUB (Runde 20) | [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#epub` |
| Runde 31 – Register und Glossar | [`suchen.md`](../site/content/hilfe/suchen.md) `#glossar`, `#register` |
| Runde 22 – Revisionen und Spiegel | [`schreiben.md`](../site/content/hilfe/schreiben.md) `#fassungen`, `#verlauf`; Rückfrage: [`assistenten.md`](../site/content/hilfe/assistenten.md) `#rueckfrage`; Archiv: [`erste-schritte.md`](../site/content/hilfe/erste-schritte.md) `#loeschen-archiv`; Entwurf: [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#pdf`; Austausch: [`austausch-sicherung.md`](../site/content/hilfe/austausch-sicherung.md) `#einlesen` |
| Runde 23 – Härtung | nach Inhalt verteilt: [`schreiben.md`](../site/content/hilfe/schreiben.md) `#einfuegen`, `#fassungen`; [`gliederung.md`](../site/content/hilfe/gliederung.md) `#antraege`; [`assistenten.md`](../site/content/hilfe/assistenten.md) `#chat`, `#befunde`, `#websuche`; [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#umschlag`, `#epub`; [`einstellungen.md`](../site/content/hilfe/einstellungen.md) `#verbindungen`, `#mcp-zugang`; [`austausch-sicherung.md`](../site/content/hilfe/austausch-sicherung.md) `#sicherung`, `#datenordner` |
| Runde 32 – Word und Beenden | [`einlesen.md`](../site/content/hilfe/einlesen.md) `#word`; [`buchsatz.md`](../site/content/hilfe/buchsatz.md) `#word-export`; [`erste-schritte.md`](../site/content/hilfe/erste-schritte.md) `#beenden` |
| Runde 36 – Reste | nach Inhalt verteilt: [`konten.md`](../site/content/hilfe/konten.md) `#lektor-rechte`, `#passwort`, `#anmelden`; [`einfaelle.md`](../site/content/hilfe/einfaelle.md) `#mindmap-austausch`; [`schreiben.md`](../site/content/hilfe/schreiben.md) `#ausblenden`, `#einfuegen`; [`assistenten.md`](../site/content/hilfe/assistenten.md) `#rueckfrage`, `#pruefen`; [`menueband.md`](../site/content/hilfe/menueband.md) `#laptop-modus`; [`austausch-sicherung.md`](../site/content/hilfe/austausch-sicherung.md) `#sicherung`, `#datenordner`; [`gliederung.md`](../site/content/hilfe/gliederung.md) `#gliederung` |

Alle Tastenkürzel und Mausgesten stehen zusätzlich gesammelt in
[`tasten.md`](../site/content/hilfe/tasten.md); der Einstieg ist [`_index.md`](../site/content/hilfe/_index.md).

## Technische Anmerkungen

Je Hilfeseite, was dort bewusst fehlt. Schnittstellen im Einzelnen: `docs/RPC.md`, `docs/MCP.md`; Aufbau:
`docs/Architektur.md`; Betrieb: `docs/Betrieb.md`; Verteilung und Starter: `docs/Verteilung.md`.

### Erste Schritte (`erste-schritte.md`)

- **Aufruf:**

  ```
  baruch-server [serve] [--listen 127.0.0.1:8765] [--data ORDNER] [--secrets keyring|memory] [--allow-host NAME]…
  baruch-server --version
  baruch-server import DATEI.typ [--title "TITEL"] [--data ORDNER]
  baruch-server export ID|SLUG [--data ORDNER]
  ```

  Der Befehl (`serve`, `import`, `export`) steht vor den Angaben. `--version` gibt `baruch-server <Version>` aus.
  `--listen` auch als `BARUCH_LISTEN` (Vorgabe `127.0.0.1:8765`, derselbe Port wie beim Benutzerdienst aus
  `packaging/systemd/`), `--data` auch als `BARUCH_DATA`. `--allow-host NAME` (mehrfach, auch
  `BARUCH_ALLOWED_HOSTS`, Komma-getrennt) erlaubt für den Betrieb hinter einem Reverse-Proxy zusätzlich einen
  Rechnernamen (`docs/Betrieb.md`).
- **Datenordner:** `ORDNER/baruch.sqlite`, Git-Spiegel jedes Werks in `ORDNER/works/<slug>/`. Vorgabe ohne `--data`:
  Linux `~/.local/share/baruch` (bzw. `$XDG_DATA_HOME/baruch`), macOS `~/Library/Application Support/baruch`,
  Windows `C:\Users\<name>\AppData\Local\baruch` – bewusst **Local**, nicht Roaming: Servergespeicherte Profile einer
  Domäne würden sonst SQLite-Datenbank (samt WAL) und Git-Spiegel bei jeder An- und Abmeldung hin- und herkopieren.
  Legt Baruch den Datenordner selbst neu an, bekommt er unter Linux und macOS die Rechte 0700; einen vorhandenen
  Ordner ändert Baruch nicht (wer `--data` außerhalb des eigenen Home legt, prüft die Rechte selbst – Datenbank und
  Spiegel enthalten das ganze Manuskript).
- **Geheimnisse** der Verbindungen (Schlüssel, Token, Passwort) liegen nur mit dem Vorgabe-Datenordner im
  Schlüsselbund des Systems (Dienst `baruch`). Mit jedem anderen `--data` bleiben sie im Speicher des Prozesses und
  sind beim Beenden weg – so kann ein Wegwerf- oder Prüfserver die Einträge `baruch/…` des eigentlichen Dienstes
  weder lesen noch überschreiben oder löschen. `--secrets keyring` (`BARUCH_SECRETS=keyring`) nimmt ausdrücklich den
  Schlüsselbund, `--secrets memory` den Speicher; die Startausgabe nennt, was gilt („Geheimnisse: …“).
- **Beispielwerk:** Eine wirklich neue Datenbank bekommt „Beispielwerk: Leise Abhängigkeit (Auszug)“; ein
  vorhandener Datenordner wird nicht verändert. Die Hilfe und ihre Bilder zeigen stattdessen das erfundene Werk
  „Ein Jahr auf Mövenoog“ (Bildliste `site/data/hilfebilder.toml`).
- **Werkwahl:** Adresse `/?work=<id>`; Cookie `baruch_work` (ein Jahr). `/` ohne `?work=` öffnet das gemerkte
  Werk, sonst das älteste, sonst die Werkliste. Links und Formulare tragen das Werk weiter.
- **Seiten:** Werkliste `/werke`; Anlegen/Importieren `/werke/neu` (mit JavaScript der Dialog „Importieren…“);
  Ändern `/werke/<id>/bearbeiten`; Löschen `/werke/<id>/loeschen`; Archiv `/werke/archiv`; Export
  `/werke/<id>/export` (lädt `<slug>.typ`, wie `baruch-server export`); Über Baruch `/ueber`, Lizenztext
  `/ueber/lizenz` (reiner Text).
- **Neues Werk:** Die Werkvorlage eines neuen Buchs enthält `#set document(…)` und
  `#set heading(numbering: "1.1")`; Commit „Werk angelegt: <Titel>“. Predigttext aus SWORD steht als
  `#quote(block: true, attribution: [Joh 3,16 (Modul)])[…]`; fehlende Verse meldet `sermon_text.missing`; eine
  ungültige Stelle antwortet ohne JavaScript mit Status 422. Auf `/werke/neu` gilt bei leerer Bibel das Vorgabewerk,
  im Dialog das gezeigte Werk.
- **Eindeutige Titel:** Meldung „Es gibt schon ein Werk „…“ (slug).“ Hatten vor dem Update zwei Werke denselben
  Titel, bekommt beim ersten Start eines einen neuen: das gesäte Beispielwerk wieder seinen Beispieltitel, sonst das
  jüngere „<Titel> (2)“. Ordnernamen, Spiegel und Texte bleiben.
- **Ändern:** Commit „Werk geändert: <Felder>“, wenn sich eine Datei ändert (z. B. `vorlage.typ`). In die
  Werkvorlage gehören `#set`, `#show`, `#let` …
- **Löschen:** letzter Commit „Werk gelöscht: <Titel>“, der Spiegel wandert nach
  `ORDNER/works/.geloescht/<slug>-<JJJJ-MM-TT>/`. Der Ordnername (`slug`) bleibt dauerhaft reserviert (neues Werk:
  `<slug>-2`); Werk-Nummern werden nie wieder vergeben. Die Einfälle stehen sofort im Spiegel der Einfälle ohne
  Werk (`<data>/einfaelle/einfaelle.md`); beim Start holt Baruch diesen Spiegel auf (ohne Änderung kein Commit).
- **Archiv wiederherstellen:** Der Ordner wandert zurück nach `works/<slug>`, Commit „Werk wiederhergestellt“.
  Über MCP gibt es das nicht.
- **Adresse:** `/?work=<id>&key=<schlüssel>&rev=<nummer>[&view=preview][&pick=…][&lq=<suche>]`. `key` ist der feste
  Schlüssel des Abschnitts (MCP-Feld `url` nennt dieselbe Adresse). Ältere Adressen mit `node=<stelle>` gehen
  weiter; fehlt der Schlüssel, zeigt Baruch den Abschnitt der Stelle `node` bzw. den Vorgabe-Abschnitt und
  „Abschnitt mit Schlüssel K gibt es nicht (mehr).“ `rev` ist die Nummer der Fassung (nach dem Löschen mit
  Lücken), nicht ihre Stelle. `pick=<abschnitt>:<nummer>[,…]` setzt in der Vorschau probeweise andere Fassungen
  ein; ungültige Einträge werden übergangen. Ohne `work` das Vorgabewerk (das älteste), unbekanntes Werk 404.
  Mit `?lq=` steht der Kasten „Bilder des Werks“ unter den Treffern, auch ohne JavaScript.
- **Zeitangaben:** Ortszeit seit der Nutzerentscheidung 2026-11-07 (vorher UTC); gespeichert und über RPC/MCP
  geliefert wird weiter UTC.
- **Über Baruch:** Lizenz `AGPL-3.0-or-later`; Quelltext beim Tag `v<Version>`; Webseite
  `https://baruch.bernau.family/`; `THIRD-PARTY-LICENSES.txt` je Paket (liegt die Datei neben dem Programm, steht
  ihr Pfad dabei). Ein aus dem Quelltext gebautes Programm hat die Datei nicht; die Seite nennt den Befehl
  (`scripts/third-party-licenses.py`). Im AppImage liegt der Pfad im eingehängten Abbild und gilt nur, solange
  Baruch läuft. Nur über das APT-Repository (Debian, Ubuntu) kommen neue Versionen mit `sudo apt upgrade`.
- **Beenden:**

  | Route | Zweck |
  |---|---|
  | `GET /beenden` | Rückfrage mit Formular; nicht vom eigenen Rechner 403 mit Erklärung |
  | `POST /beenden` | hinter `guard::same_origin`; nur vom eigenen Rechner (auch nicht unter einem `--allow-host`-Namen), sonst 403; Antwort 200 mit der Abschlussseite, 500 ms später fährt Baruch herunter |

  Mit JavaScript ruft die Abschlussseite `window.close()`; Browser lassen das nur bei Fenstern zu, die ein Skript
  oder der Starter als App-Fenster geöffnet hat. Die Starter (`baruch`, AppImage, macOS-Programm,
  Windows-Startmenü) öffnen Baruch deshalb mit einem Chromium-artigen Browser als `--app`-Fenster;
  `BARUCH_BROWSER=standard` nimmt immer den Standardbrowser, `BARUCH_NO_BROWSER=1` öffnet keinen (Stand der Prüfung
  je Plattform: `docs/Verteilung.md`, „Baruch beenden“). Als Dienst (`systemd --user`, `Restart=on-failure`) bleibt
  Baruch nach dem Beenden aus. `systemctl --user stop`, `kill` (SIGTERM) und unter Windows Strg+Pause bzw. das
  Schließen des Konsolenfensters fahren genauso sauber herunter; letzte Zeile „Baruch beendet.“, Status 0. Über RPC
  und MCP lässt sich Baruch nicht beenden.

### Vorhandene Texte einlesen (`einlesen.md`)

- **Typst-Import, Regeln:** `=` Kapitel, `==` Abschnitt, `===` Unterabschnitt; der Text samt Überschriftszeile wird
  Revision 1. Titel und Autor aus `#set document(title: …, author: …)`; auskommentierte `#set document(…)` (`//`,
  `/* … */`) zählen nicht. Titel vergeben: Exit 1 mit „Es gibt schon ein Werk „…“ (slug). Mit --title "…" einen
  anderen Titel wählen.“ Ordnername (`slug`) aus dem Titel, höchstens 40 Zeichen, an der letzten Wortgrenze gekürzt
  („Mövenoog – ein ganzes Jahr auf der kleinen Insel“ → `moevenoog-ein-ganzes-jahr-auf-der`), bei Gleichheit `-2`,
  `-3` …; vorhandene Werke behalten ihren Ordnernamen.
- **Spiegel nach dem Import:** Commit „Import aus <datei>“ mit `vorlage.typ`, `main.typ` (Werkvorlage + `#include`
  aller Abschnitte) und `abschnitte/NNN-titel.typ`, jede mit der ersten Zeile `#import "../vorlage.typ": *` (nur im
  Spiegel, nicht in der Revision; so gelten eigene Funktionen wie `#szene` auch dort). Werke ohne Vorlage haben
  weder `vorlage.typ` noch die Zeile. Ausgabe: Werk, Ordnername, Anzahl der Abschnitte, Spiegelordner, Commit.
- **Abgelehnt:** dieselbe Datei ein zweites Mal (auch umbenannt oder mit CRLF; stderr, Exit 1); eine FIFO, ein
  Gerät oder ein Ordner („… ist keine Datei“, ohne zu warten); nicht UTF-8; über 16 MiB. Ein BOM bleibt in der
  Werkvorlage, die erste Überschrift wird trotzdem Kapitel.
- **Keine Gliederung** sind Überschriftszeilen in Raw-Blöcken, in mehrzeiligem Inline-Raw, in Blockkommentaren, in
  mehrzeiligen Strings im Code (`#let x = "…"`) und in mehrzeiligen Inhaltsblöcken aus Code (`#block[…]`,
  `#figure(caption: […])`, `#let x = […]`, `#show …: it => […]`). Ein Kommentar hinter einer Überschrift
  (`== Titel // Notiz`) gehört nicht zum Titel. Bekannte Grenze: eingerückte Überschriften, `====` und tiefer und
  `#heading(...)` werden nicht erkannt – außer den Markierungen `<baruch-part>` und `<baruch-unnumbered>` (genau in
  der dokumentierten Form; dahinter darf `// …` stehen; andere Abstände, ein anderes Label oder ein Einzug machen
  daraus Text bzw. Werkvorlage). Ein anderes `#heading(...)` vor der ersten Überschrift bleibt Werkvorlage.
  Vorhandene Werke werden für die Markierungen nicht umgeschrieben.
- **Export auf der Kommandozeile:** `baruch-server export <id|slug>` schreibt Werkvorlage + aktuelle Texte auf
  stdout (ohne die `#import`-Zeilen des Spiegels); direkt nach dem Import Byte für Byte die Eingabe.
  `--out ORDNER` schreibt die Typst-Datei (unter ihrem Importnamen, sonst `<slug>.typ`) und jedes Bild an seinen
  Pfad (ohne `./` und führendes `/`), byte-gleich. Der Export prüft vor der ersten Datei: vorhandene Zieldatei,
  Zwischenordner als Verknüpfung oder Datei, zwei Bildpfade zugleich Datei und Ordner (`a.png` und `a.png/b.svg`)
  → Exit 1 mit Grund, nichts geschrieben.
- **Bilder beim Import (CLI):** gelesen wird, was Vorlage und Text mit `image("…")` nennen (auch in
  `#figure(image("…"))`), relativ zum Ordner der Datei; `./bilder/a.png`, `bilder//a.png` und `/bilder/a.png` sind
  dasselbe Bild `bilder/a.png` (der Pfad im Text bleibt, wie er geschrieben ist). Übersprungen: außerhalb des
  Ordners (über `..` oder Symlink), Pfade mit `..`, fehlend, über 8 MiB, kein Bild – je eine Zeile „Bild
  übersprungen: <pfad> (<grund>)“ auf stderr; Ausgabe „Bilder: n“; zusammen höchstens 64 MiB je Datei. Das
  Hochladen einer einzelnen Datei (`/werke/import`) und `work.import` bringen nur den Text.
- **Export → Import, Hinweiscodes** (CLI: `Hinweis (<code>): …` auf stderr): `import-typeset-adopted` (Block
  `// Baruch: Satz` wird Satz-Einstellung), `import-typeset-kept` (von Hand geändert, bleibt Werkvorlage; der
  Bundsteg kommt als Teil des Innenrands zurück), Show-Regeln `// Baruch: Überschriften ohne Nummer` → Merker
  „Nummer aus“ (auch bei Überschrift mit eigenem Label und unter „Ohne Nummern“), `import-back-matter-cut` (ab
  `// Baruch: Register und Glossar`, nur wenn dahinter der erzeugte Block steht, mit Zeile),
  `import-back-matter-kept`, `import-projection-removed` (ohne Glossarseite trägt die erste Erwähnung den Vermerk
  `/*baruch-erste-erwaehnung*/#emph[Wort]`).
- **Markdown → Typst:** `#`/`##`/`###` → `=`/`==`/`===`, `####`–`######` auf `===` begrenzt; YAML-Kopf → `#set
  document(…)`; `#emph[…]`, `#strong[…]`, `#strike[…]`; `#quote(block: true)[…]`; `#raw("…")`; `#link("…")[Text]`;
  `#footnote[…]`; `#image("…", alt: "…")` bzw. `#figure(…, caption: […])`; `#table(…)`; `/ Begriff: Erklärung`;
  `<br>` wird Umbruch, HTML-Blöcke stehen als Kommentar. Wörtlich bleiben `#`, `*`, `_`, `$`, `@`, `<`, `[`, `]`,
  `` ` ``, `\`, `~` und `=`/`-`/`+`/`/` am Zeilenanfang. Dateien, die die Haupt-Datei nicht nennt: `archive-unused`.
  Abgelehnte Archive: Verknüpfungen, `..`, absolute Pfade, doppelte Namen, Zip-Bomben. CLI-Ausgabe:
  `Hinweis (<code>, Zeile n): …`.
- **EPUB auf der Kommandozeile:** `baruch-server export <id|slug> --epub DATEI [--data ORDNER]`; vorhandene Datei
  Exit 1; `--epub` zusammen mit `--out`, `--pdf`, `--stichwort` oder `--baruch` Exit 2.
- **PDF/EPUB:** Seite `/werke/import/quelle`; lokal `pdftotext`/`pdfinfo` (Paket `poppler-utils`); EPUB-Fußnoten
  werden `#footnote[…]`, die Zahl am Anfang entfällt nur als Nummer des Fußnotenzeichens (Hinweis „Fußnotennummer
  entfernt“; „1 Kor 13,4“ bleibt). MultiGPT nimmt laut seiner Werkzeugbeschreibung kein EPUB an (Annahme aus der
  Beschreibung, nicht gegen einen echten Dienst geprüft). Entwürfe in `import-drafts/`. Kommandozeile mit
  `--data DIR`; ohne `--uebernehmen` und `--bibliothek` schreibt sie nichts, ein vermuteter Teil wird dort ein
  Kapitel; beide Schalter bei `.typ`/`.md`/`.zip` → Exit 2.
- **Word:** Seite `/werke/import/word`; Vorschläge unter `@word:<name>` (Modell „word“); Name: klein, ä→ae, ö→oe,
  ü→ue, ß→ss, alles andere Bindestrich, höchstens 40 Zeichen, ohne Namen `@word:unbekannt`. Überschriften aus den
  Vorlagen „Überschrift 1–3“/„Heading 1–3“ (auch abgeleitete Vorlagen und Gliederungsebene des Absatzes);
  Tabellen als Typst-Tabelle (im Satzbild eine Insel); Bilder PNG, JPEG, GIF, WebP, SVG. Aus einer von Baruch
  geschriebenen Datei wird `@lektor` zu `@word:lektor`; Typst-Inseln kommen wörtlich zurück (Test
  `round32_abnahme`). Hinweiscodes:

  | Code | Bedeutung |
  |---|---|
  | `docx_ueberschrift_tief` | Überschrift 4–6 steht fett im Abschnitt |
  | `docx_text_vor_ueberschrift` | Text vor der ersten Überschrift in eigenem ersten Kapitel |
  | `docx_ueberschrift_eingefuegt`, `docx_ueberschrift_geloescht` | vom Bearbeiter eingefügte bzw. gelöschte Überschrift |
  | `docx_vorspann_eingefuegt` | eingefügter Text vor der ersten Überschrift – nicht übernommen |
  | `docx_formataenderung` | verfolgte Formatänderungen: Endstand gilt |
  | `docx_verschoben` | verschobener Text als Löschung und Einfügung |
  | `docx_textfeld`, `docx_objekt` | Textfeld, Zeichnung, eingebettetes Objekt – nicht übernommen |
  | `docx_formel` | Formel nur als einfacher Text |
  | `docx_tabelle_vereinfacht` | verbundene Zellen, ungleich lange Zeilen, eingefügte/gelöschte Tabellenzeilen |
  | `docx_bild_format`, `docx_bild_extern`, `docx_bild_uebersprungen` | Format nicht unterstützt (EMF/WMF), nur verknüpft, zu groß |
  | `docx_endnote`, `docx_fussnote_absaetze` | Endnoten als Fußnoten; mehrere Absätze einer Fußnote als Zeilen |
  | `docx_verborgener_text`, `docx_symbol`, `docx_unbekannt` | verborgener Text, Symbolschriften, unbekanntes Element |
  | `docx_kommentar_ohne_stelle` | Kommentar am ersten Abschnitt |
  | `docx_gliederung_abweichend`, `docx_vorschlag_abgelehnt`, `docx_beitrag_abgelehnt` | Vorschlag oder Beitrag ließ sich nicht anlegen (das Werk besteht trotzdem) |

  Kommandozeile: `baruch-server import DATEI.docx [--uebernehmen] [--title "TITEL"] [--data ORDNER]`; unlesbare
  Datei Exit 1.
- Über MCP gibt es keinen Import (kein Typst, PDF, EPUB, Word).

### Menüband und Laptop-Modus (`menueband.md`)

- Das Menüband ersetzt die frühere Kopfzeile mit den Menüs „Werk“, „Ansicht“ und „Werkzeuge“ und die frühere
  Werkzeugleiste über dem Blatt; kein Eintrag ist entfallen. Umbenannt: „Satz“ → Satz-Vorschau, „Als
  PDF/Entwurf/EPUB herunterladen“ → PDF, Entwurf, EPUB; „Editor“ außerhalb der Arbeitsseite → Zum Text.
- **Adressen:** Reiter `?tab=start|einfuegen|pruefen|buch|ansicht`, Band `?band=…` (Cookie `baruch_band`),
  Laptop-Modus `?laptop=on|off|auto` (Cookie `baruch_laptop`, ein Jahr), Leisten `?lrail=1`, `?rrail=1` (Cookies
  `baruch_lrail`, `baruch_rrail`); Schubladen ohne JavaScript über die Sprungziele `#pane-tree` und
  `#pane-assistants`. Ohne JavaScript sind Menü und Gruppen `<details>`.
- **Ziele:** Neues Werk `/werke/neu`; Einstellungen ohne JavaScript `/einstellungen/darstellung`; Suche `/suche`;
  Querverweise `/verweise`; Literatur `/literatur` (auf der Arbeitsseite das Popup); Abgleich `/abgleich`;
  „Literatur“ als Zurück-Gruppe auf `/quelle`. Die Satz-Seite trägt im Reiter Buch die Gruppen Zurück,
  Herunterladen, Seiten (‹ Seite n von N ›) und Satzlauf (Satz abbrechen).
- **Abzeichen** stehen im ausgelieferten HTML jeder Seite; das Skript frischt sie nur auf.
- **Zoom:** Stufen im Cookie `baruch_zoom`; Strg+Mausrad wertet nur `ctrlKey` aus (am Mac also Ctrl, nicht ⌘),
  Strg+0 auch `metaKey`.
- **Hilfe-Eintrag:** gewöhnlicher Link auf `about::HELP_URL` (`https://baruch.bernau.family/hilfe/`), neues
  Fenster, ohne `nur-autor` (auch Lektoren sehen ihn); keine Route, kein Eintrag in `roles`.
- **K** und **F** heißen für Bildschirmleser „Hervorheben“ und „Fett“. In der Auswahlliste „Absatzformat“ gehören
  die Pfeiltasten der Liste.
- Früher stellte Baruch die drei Spalten unter 900 px untereinander; das ist entfallen.

### Gliederung (`gliederung.md`)

- **Schreibweise im Text:** Teil = `#heading(level: 1, numbering: none)[Titel] <baruch-part>`, Kapitel `= `,
  Abschnitt `== `, Unterabschnitt `=== `; ein Punkt ohne Nummer kann als
  `#heading(level: N, numbering: none)[Titel] <baruch-unnumbered>` stehen. Ändern sich Ebene oder Titel, bekommt
  der Abschnitt eine neue Fassung vom Menschen, die sich nur in der Überschriftszeile unterscheidet (Label `<…>`
  und Kommentar `// …` bleiben). Jede Gliederungsänderung ist genau ein Commit („Gliederung: …“).
- **Assistenten:** anlegen über MCP `outline_add` (Commit mit ihrem Namen; Teil mit `level: 0`); umbenennen,
  verschieben, umstufen, Nummer an/aus, löschen nur als Antrag. In einer Predigt sagt der Antrag „n vorhandene
  Predigtteile wandern unter den neuen Teil“. Legt ein Assistent einen Teil vor ein Kapitel mitten in einem Teil
  (ohne vorhandene Kapitel zu übernehmen gilt das als Anlegen), stehen die folgenden Kapitel bis zum nächsten Teil
  unter dem neuen Teil.
- **Markdown-Überschriften** (seit 2026-11-07): 1–6 Rauten und ein Leerzeichen am Zeilenanfang. Nicht umgewandelt:
  `\# …`, Raute in Anführungszeichen (`'# …'`, `"# …"`), Typst-Code (`#heading(…)`, `#set`, `#let`), eingerückte
  Zeilen, Zeilen in Raw-Blöcken, Kommentaren und Inhaltsblöcken, die Werkvorlage. Vorschläge der Assistenten bleiben,
  wie sie sind; beim Übernehmen gilt dieselbe Regel (der Vorschlag bleibt im Fassungsbaum, darauf folgt die
  umgewandelte Fassung). „Fassung ändern“ wandelt um wie „Speichern“.
- **Titelregeln, Meldungen:** „Ein Gliederungspunkt braucht einen Titel.“, „Der Titel darf keinen Zeilenumbruch
  enthalten.“, „Der Titel ist zu lang (höchstens 200 Zeichen, hier N).“ Geprüft wird nur ein geänderter Titel
  (auch bei `revision.save`, Umschalten, Übernehmen, jeder Überschrift, die den Abschnitt teilen würde); ein Punkt
  `== ` ohne Titel bleibt. Import und Export bleiben byte-gleich.
- **Löschen:** Der Commit nennt den Text der aktuellen Fassung und, was sonst verloren geht – jede übrige Fassung
  (offene Vorschläge eigens gezählt) und jeden Beitrag im Wortlaut.
- **Ohne JavaScript:** Gliederungsmanager `/gliederung`, Bearbeiten `/gliederung/bearbeiten`; mit JavaScript sind
  die Rückfall-Links ausgeblendet. Die Rückfrage zum Teilen/Zusammenführen ist ohne JavaScript eine eigene Seite
  mit „Zurück“.
- **Teile, Einzelheiten:** Umbenennen schreibt die Markierungszeile neu (Kommentar dahinter bleibt; ein Teil mit
  leerem Text bekommt die Markierung). Ein Teil ohne Markierung (selbst geschriebener Text) behält seinen Text;
  nur der Titel ändert sich, die Meldung sagt es. Ein anderer Titel in der Markierung benennt den Teil um, auch
  beim Umschalten auf eine ältere Fassung; eine Fassung ohne Markierung schaltet ohne Gliederungsabgleich um.
- **Ältere Teile (Nutzerentscheidung 2026-11-07):** Beim ersten Start stellt Baruch einmal alle Teile, deren
  aktuelle Fassung leer ist oder nur aus der alten Zeile „= Teil …“ besteht (genau eine `=`-Zeile ohne Label und
  Kommentar, Titel der des Teils oder mit „Teil“ beginnend), auf die Markierung um: neue aktuelle Fassung des
  Menschen mit der Notiz „Umwandlung Runde 14 (Nutzerentscheidung)“, je Werk ein Commit „Umwandlung Runde 14
  (Nutzerentscheidung): n Teile …“; die alte Fassung bleibt umschaltbar. Teile mit anderem Text bleiben; stderr
  nennt Anzahl und Knoten-ids. Merker `parts_marked_r14` in `store_meta`; eine neue Datenbank setzt ihn gleich
  (ihr Beispielwerk behält seine leeren Teile).
- **Ohne Nummer:** Commit „Gliederung: „…“ ohne Nummer“ (`Baruch-Action: outline-numbered`), RPC
  `outline.set_numbered`. Ändert man die Ebene über die Gliederung, bleibt die Markierung (nur `level:` ändert
  sich). Im Export, Spiegel und Satz steht ein Punkt mit „Nummer aus“ als
  `#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>` – in allen Schemata mit Nummern; der
  gespeicherte Text bleibt (`= Vorwort`). Eine Überschrift mit eigenem Label (`= Vorwort <vorwort>`) bekommt
  stattdessen `#show heading.where(…): set heading(numbering: none)` hinter der Vorlage. Bei „Ohne Nummern“ bleibt
  alles, wie es gespeichert ist.
- **Anträge, Commits:** Bestätigen = ein Commit mit dem Menschen als Autor und Committer, Trailer `Baruch-Origin`,
  `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`, Betreff „… (Antrag N)“. Abgelehnt oder veraltet
  (Nutzerentscheidung 2026-11-08, jede Antragsart): ein leerer Commit „Gliederung: Antrag N abgelehnt“ bzw.
  „… veraltet“, Trailer `Baruch-Decision: rejected|stale`, `Baruch-Requested-By`, `Baruch-Origin`,
  `Baruch-Request`, `Baruch-Decided-By`, im Rumpf Antragsteller, Zusammenfassung, Begründung, Grund. Gleich, ob
  über Formular, Zustimmungsdialog oder RPC. Zustimmung zu einem Teil: Revision 1 des Teils trägt den Assistenten.
  Hinweisleiste verlinkt `/gliederung#antraege`; Vorschau `/gliederung/antraege/ID/vorschau`. Predigten nennen in
  der Vorschau „Predigtteil II.“.
- **Umschalten mit anderer Ebene:** ein Commit („… auf Fassung n umgeschaltet · Ebene 1 → 2“, im Rumpf die
  mitgestuften Punkte); jeder mitgestufte Unterpunkt bekommt eine neue Fassung vom Menschen. Abgelehnt: Fassung
  ohne Überschrift, mit weiteren Überschriften, mit Text vor der Überschrift, mit einer Ebene, die an der Stelle
  nicht geht (z. B. der erste Punkt als Abschnitt). „Fassung ändern“ und `revision.save` der aktuellen Fassung
  verlangen genau eine Überschrift der Ebene des Punkts; eine nicht aktuelle Fassung lässt sich frei ändern.
  Assistenten können Vorschläge mit anderer Ebene nicht anlegen.
- **Maskierungen** in Überschriften (`\'`, `\#`, `\*` …) zeigt die Gliederung aufgelöst („Anna's Laube“);
  Dateinamen im Spiegel folgen dem aufgelösten Titel und der Stelle. Der Export (`work.export`) ist nach
  Umstellungen gültiges Typst in neuer Reihenfolge, ohne Änderung byte-gleich zur Import-Datei.
- **Gliederungsschema:** Seite `/werke/<id>/nummerierung`; Vorschau per GET; Commit „Gliederung:
  Gliederungsschema „Juristisch““; ungültiges Muster 422, unverändertes Schema 409. Im Export folgt der Vorlage
  eine Zeile `// Baruch: Gliederungsschema „…“` und eine `#set heading(numbering: …)`-Regel (bei „Aus der
  Werkvorlage“ nichts); geprüft für alle dreizehn Schemata. Die gespeicherte Vorlage (`vorlage.typ`) bleibt. Im
  Kopf der Gliederung: „I|1“ → „Nur Kapitel römisch“, „1.1“ → „Dezimal (DIN 1421)“, sonst das Muster, das rohe
  Muster im Tooltip; Werte `vorlage`, `keine`.
- **Mustersprache:** ein Muster mit genau einem Zählzeichen zeigt nur die eigene Zahl der Ebene, eins mit mehreren
  zählt hierarchisch; Ebenen hinter der Liste haben keine Nummer; `;` und `|` sind reserviert.
- **Nummerierung aus der Werkvorlage:** Es zählt die letzte `#set heading(numbering: …)`-Regel der obersten Ebene.
  Ein Textmuster gilt wörtlich wie in Typst (`"1;"` ergibt „2;“, `aa` sind zwei Zählzeichen). Eine Funktion wird
  je Ebene ausgerechnet, wenn sie nur aus Bedingungen über die Tiefe (`n.pos().len()` mit Vergleichen, auch über
  `let p = n.pos()`), `if`/`else`, Text, `numbering("…", ..n)`, `numbering("…", p.at(i))` (`p.first()`,
  `p.last()`), `str(p.at(i))`, `* 2` und `+` besteht – direkt oder über eine `#let`-Funktion. Beispiel:
  `(..n) => if n.pos().len() == 1 { "Kapitel " + str(n.pos().at(0)) } else { numbering("1.1", ..n) }` ergibt
  „Kapitel 3“ · „3.2“ · „3.2.1“. Die Regel, die Baruch für „Teile und Kapitel“, „Kapitel ausgeschrieben“,
  „Juristisch“, „Predigt“, „Buch“ exportiert, wird beim Einlesen wieder dieses Schema (als Schema gilt danach „Aus
  der Werkvorlage“). Nicht Ausrechenbares: das erste `numbering("…")` der Funktion für alle Ebenen.
- **Nennungen im Text** („im dreizehnten Kapitel“, „Kapitel XLII“) erkennt Baruch bis zur hundertsten
  Ordnungszahl und für römische Zahlen in üblicher Schreibweise, nur wenn genau ein Punkt gemeint sein kann.

### Schreiben, Bilder und Fassungen (`schreiben.md`)

- **Satzbild:** Die Quelle bleibt Typst; vor dem Speichern wird das Satzbild in Typst übersetzt, danach gelten die
  Regeln des Quelltexts. Der Server liefert das Satzbild fertig mit (kein Umweg über den Quelltext); Abschnitte über
  64 KiB kommen ohne fertiges Satzbild. Gemerkte Ansicht: Cookie `baruch_editor`. Kommt man über einen Treffer der
  Literatursuche (`#fundstelle`), beginnt der Editor im Quelltext, bei offenem Satzbild ist die Fundstelle dort gelb
  markiert (ändert den Text nicht, verschwindet beim Tippen). Beschreibung von Blöcken, Inseln und RPC:
  `docs/WYSIWYG.md`.
- **Überschrift im Satzbild:** Rückschritt am Anfang eines Absatzes direkt hinter einer Überschrift und Entf am
  Ende einer Überschrift ziehen den Absatz nicht in die Überschrift (ein leerer Absatz entfällt). Das Satzbild
  prüft die Titelregeln mit derselben Meldung wie der Server. Teile bietet das Satzbild nicht an (der Server lehnt
  einen neuen Teil aus dem Satzbild ab); für Teile und Punkte ohne Nummer lässt sich kein Absatzformat wählen,
  gespeichert wird wieder die Markierung (Kommentar dahinter bleibt).
- **Blöcke:** `#szene[…]`, `#arbeit[…]` als Kasten; Argumente (`#szene(ort: […])`) und ein Label `<name>` auf der
  nächsten Zeile stehen klein daneben und bleiben erhalten. `#image("…")` und `#figure(image("…"), caption: […])`
  erscheinen als Bild, sobald das Bild unter diesem Pfad angelegt ist (auch mit `./`); ein Block mit mehreren
  `image(…)` zeigt alle. `1. …` nummerierte Liste, `/ Begriff: Erklärung` Begriffsliste, `\` erzwungener Umbruch
  (↵), `#link("…")[Text]`, `@ziel` als Schaltfläche (Klick zeigt das Label; ändern nur im Quelltext). Inseln:
  `#let`, `#set`, `#show`, eigene Funktionen, Formeln, Raw-Text, Kommentare, Listen mit gemischten Zeichen; das
  Verlassen des Inselfelds nach einer Änderung übernimmt ebenfalls. Bibelstelle als Insel `#bibelstelle[…]` (nur
  wenn die Vorlage `#let bibelstelle(…)` bzw. `#let bibel(…)` definiert, mit dem Namen der Vorlage),
  Literaturverweis `@schlüssel` (nur mit `#bibliography(…)`); Szene nur mit `#let szene`, Arbeit nur mit
  `#let arbeit`; Begriff `/ Begriff: …`, Umbruch `\` am Zeilenende. Im Quelltext beginnen Blöcke an einem
  Zeilenanfang.
- **Zitat-Knopf:** fügte bis Runde 35 eine Zeile „– “ ein, die ungefüllt als einzelner Gedankenstrich im Buch
  stand; seit Runde 36 entfällt sie. Stellenangabe direkt unter einem Zitatblock zeigt das
  Satzbild als Zitat und Absatz (keine Code-Insel).
- **Speichern:** seit 2026-11-27 normales Speichern ohne Größen- und Zeitgrenze (vorher: neue Fassung nach
  30 Minuten Pause oder ab 20 % Änderung). Der Dialog „Seite speichern“ des Browsers erscheint auf der
  Arbeitsseite nicht. Eine eingeschaltete Zeitgrenze zählt ab dem letzten Schreiben der Fassung: Die erste kleine
  Korrektur an einem älteren, nie fortgeschriebenen Abschnitt ergibt eine neue Fassung („Zeitgrenze
  überschritten“). Der Editor schickt den geladenen Stand mit; bei anderem Stand entsteht eine neue Fassung („nicht
  auf der aktuellen Fassung bearbeitet“). Jede Fortschreibung ist ein Commit. Rückmeldung mit JavaScript als
  Hinweis unten, ohne im Editor-Kopf. Alle anderen Schreibwege (Bild einfügen, Registermarke, Verweis, Ersetzen,
  Gliederung) legen neue Fassungen an. Lehnt das Speichern ohne JavaScript ab (z. B. ungültige Überschriftenebene),
  zeigt Baruch Meldung und Text in einem Textfeld („Erneut speichern“). Die Rückfrage „Teilen/Zusammenführen und
  speichern“ behält den Speichermodus; der Kopf nennt danach Fassung und Grund („Überschrift geändert“).
- **Einstellungen → Werk → Fassungen** (Anker `#fassungen`): Fortschreiben an/aus, Prozent 1–100 (Vorgabe 20),
  Minuten 0–1440 (0 = keine Zeitgrenze, Vorgabe 30); nur in der Datenbank (nicht im Austauschformat), jede
  Änderung ein Protokoll-Commit.
- **Verlauf:** `/werke/<id>/fassungen/verlauf`; jede Rücknahme ist ein Commit. „Fassung ändern“ an der aktuellen
  oder einer fortgeschriebenen Fassung ist ein Schritt im Verlauf; ist sie inzwischen in einem anderen Reiter
  fortgeschrieben, lehnt Baruch ab („Veraltet: …“, das Formular an `/act` reicht `based_on_amend` durch).
- **Weitere Fassungen:** Link `?rev=`, „aktuell machen“ ohne JavaScript als Formular an `/act`.
- **Fassungen:** Neue Fassung hat die gezeigte Fassung als Elternteil. Löschen prüft der Server selbst (ohne Haken
  „Löschen bitte bestätigen.“); Kinder hängen danach am Elternteil; unbekannte Nummer: „Fassung 3 gibt es nicht;
  vorhanden: 1, 2, 4“. „In der Vorschau probieren“ = `pick` in der Adresse. Jede Übernahme, Umschaltung,
  Speicherung, Änderung und Löschung ist genau ein Commit (`docs/Architektur.md`); Ändern und Löschen schreiben
  vorher/nachher bzw. den gelöschten Text in die Commit-Nachricht. Scheitert der Spiegel, bleibt die Änderung
  gespeichert; der nächste Commit holt nach.
- **Ausblenden:** Adresse `&fassungen=alle`; die Marke gilt je Abschnitt, steht nur in der Datenbank (nicht im
  Austauschformat), kein Commit; die Aktion gibt es nur an der aktuellen Fassung und darunter; nach Umschalten auf
  eine ältere Fassung gilt eine höhere Marke nur bis unter die neue aktuelle. Werkweit: Marke auf die letzte
  Fassung vor dem Tag (höchstens die aktuelle), ersetzt vorhandene Marken. Über MCP nicht möglich. Der Zähler im
  Menüband zeigt dasselbe wie der im Editor-Kopf.
- **Vorschläge im Lesefluss:** Linien mindestens 3 : 1 Kontrast (WCAG 1.4.11); auf dem Blatt, das auch im dunklen
  Thema hell bleibt (#f4f1ea), gilt die helle Linie. Einklappen: Cookie `baruch_folded`; ohne JavaScript klappt der
  Browser selbst (nicht über das Neuladen hinweg). Autoren ohne Persona-Eintrag: `@mcp:<name>`. Ein umformulierter
  Absatz neben einem neuen erscheint als zwei Stücke („geändert“, „eingefügt“).
- **Bilder:** Seite `/werke/<id>/bilder`, Suche `?q=…` (ohne Groß/Klein, `%` und `_` wörtlich), Vorschau
  `…/vorschau?b=320` (lädt beim Hinscrollen), ein Bild `/werke/<id>/bilder/<name>[?fassung=n]` mit SHA-256, Bytes
  `/werke/<id>/bilder/<name>/datei[?fassung=n]`, nach Pfad `/werke/<id>/bilder/pfad?p=<Pfad>` (jede Schreibweise,
  nie `..`), Anträge `#antraege`, vorgeschlagener Inhalt `/werke/<id>/bilder/antraege/<nr>/datei` (30 Tage nach
  der Entscheidung entfernt, dann HTTP 410). Name leer = aus dem Dateinamen (klein, unzulässige Zeichen `-`, bei
  Kollision `-2` vor der Endung); „SVG als Base64 speichern“ („(Base64)“ in der Liste). Spiegel: `bilder/<name>`
  bzw. `bilder/<name>.b64` (76 Zeichen je Zeile), Commit „Bild <name>: …“. Endung anpassen: `haus.svg` →
  `haus.png` (vergeben: `haus-2.png`), Pfade bleiben, im EPUB heißt die Datei z. B. `haus.svg.png`. Pfad-Kollision:
  Commit „Pfad-Kollision … aufgelöst“. Grenzen für Assistenten je Werk in 24 Stunden: 30 Anlagen/Ersetzungsanträge,
  zusammen 64 MiB, höchstens 10 offene Ersetzungsanträge.
- **Bild einfügen:** eingefügt wird `#figure(image("<Pfad>", alt: "…"), caption: [...])` als Mensch (ein Commit;
  der Text der Unterschrift wird maskiert). Ohne Pfad bekommt das Bild `bilder/<name>` zusammen mit der Fassung.
  Cursorstelle im Quelltext: nächste Zeilengrenze, nie in ein mehrzeiliges Konstrukt (`#footnote[…]`,
  `#link(…)[…]`). Kapitelbild: erkannt aus einer literalen Liste wie `#let kapitelbilder = ("bilder/a.png", …)` und
  `image(kapitelbilder.at(counter(heading).get().first() - 1))`, wenn Kapitel die erste Ebene sind; sonst
  „Kapitelbild fehlt: <Pfad>“ bzw. „Bild aus der Vorlage“.
- **Zeitwächter:** Sekunden = Wörter × 60 ÷ Wörter je Minute, aufgerundet; Gesamtzeit = Summe der Punkte. Nur in
  der Datenbank, kein Commit. RPC `work.timing`, `work.timing_set`, `timing.count`; MCP `work_timing` (nur lesend).
  Zählregeln (`baruch_typst::timing::spoken_words`): (1) CRLF → LF; (2) Kommentare `/* … */` und `//` am
  Zeilenanfang oder nach Leerraum zählen nicht; (3) Überschriftszeilen und Teil-Markierungen nicht; (4) Raw-Text
  und Mathe `$…$` nicht; (5) `#name`/`#name.feld` samt Argumenten fällt weg, folgende Inhaltsblöcke `[…]` werden
  gesprochen – außer `#footnote` und `#arbeit`; Zeilen mit `#set`, `#show`, `#let`, `#import`, `#include` fallen
  ganz weg; (6) Labels `<…>` und Verweise `@name` fallen weg; (7) `*`, `_` fallen weg, `~` wird Leerzeichen,
  Listenzeichen am Zeilenanfang fallen weg, `\x` wird `x`; (8) ein Wort ist eine Folge ohne Leerraum mit
  mindestens einem Buchstaben oder einer Ziffer. Beispiel: `#quote(block: true)[Im Anfang war das Wort.] <zitat>`
  = 5 Wörter = 3 Sekunden bei 110.

### Assistenten, Chat und Befunde (`assistenten.md`)

- **Begriffe:** In Code, RPC und MCP heißen die Assistenten „Persona“, das Kürzel „Handle“. Baruch ruft das Modell
  der Persona über das Modell-Backend Eliezer auf (Einstellungen → Verbindungen).
- **Aufträge:** Formulare ohne JavaScript an `/assistenten/act`; Fehler zeigt eine Hinweisseite (statt einer
  Fehlerseite) mit „Zurück zum Abschnitt“, im Chat je Persona, wer nicht geantwortet hat; mit „komprimiert“ der
  Hinweis und „Weiter zum Abschnitt“. Mit JavaScript der schwebende Hinweis (Toast) unten rechts, der stehen
  bleibt. Mit Gemma dauert ein Aufruf etwa 25 s. Fordert Baruch neu an, verschwindet das bisher Gezeigte („Antwort
  wird neu angefordert (…)“).
- **Länge der Antworten:** höchstens 700 (Frage), 900 (Prüfen), 500 (Chat) Token, beim Vorschlag nach der Länge des
  Abschnitts (höchstens 4096); `BARUCH_MAX_TOKENS` setzt eine kleinere Obergrenze. Abgeschnitten: „… (Antwort
  abgeschnitten: Längengrenze erreicht)“.
- **Warteschlange:** Eliezer bearbeitet einen Auftrag zur Zeit (sonst lädt es ein Modell zweimal; die Grafikkarte
  des Entwicklungsrechners hat 22 GB), acht warten. Die Kontextlänge der Persona muss zu der passen, mit der
  Eliezer das Modell lädt (Gemma: 32768); Baruch fordert selbst keine an.
- **Lektor:** für den Versuch seit 2026-11-06 fest `@lektor` (alle Personas auf einem Modell); das Abzeichen sagt
  im Titel „fest gewählt“ bzw. „nach größtem Kontext“. Gedächtnis: Einfälle zum Abschnitt und bis zu zehn offene
  des Werks ohne Bezug. Befunde der anderen: je Abschnitt die letzten drei.
- **Prompt-Regeln:** verbotene Einleitungen „Als Assistent“, „Als KI“, „Als Sprachmodell“, „Gerne“, „Natürlich“
  (beginnt auch die zweite Antwort so, fällt ihr erster Satz weg); Bibelverse im Wortlaut nur von `@schrift`;
  der Hörer liest in der Ich-Perspektive der Zielgruppe. Im Vorschlag entfernt Baruch Zeilen, die mit „Kritisch:“,
  „Was trägt:“, „Vorschlag:“, „Jetzt wichtig:“, „Kann warten:“ beginnen, einen Codeblock um den Text, und prüft
  dort keine verbotenen Einleitungen. Der Vermerk „Komprimiert“ steht beim Vorschlag in der Notiz, nie im Buchtext;
  Notiz beim Kapitel „Vorschlag von @lektor (ganzes Kapitel)“, der Kopf nennt „beruht auf Fassung n“.
- **Lektor-Vorgabe:** verlangt „Was trägt:“ nur bei einer Prüfung; im Chat geht der Lektor zuerst auf die Nachricht
  ein (Regel 8 des Prompts); ein eigener Rollen-Prompt bleibt unberührt (bei der Handprüfung auf den Ton achten).
  „Chat nur Abschnitt“ gilt für alle Werke und steht nur in der Datenbank; die Persona bekommt dann den Abschnitt,
  Titel und Gliederung (nur Nummern und Titel). Prüfen und Fragen im Bereich „Assistenten“ sehen weiter das Kapitel.
- **Arbeitsseite ohne JavaScript:** In der Gliederung stehen die Pfeile und „Bearbeiten“ in einer eigenen Zeile
  unter dem Eintrag; in schmaler Spalte rutscht das Abzeichen „k offen“ unter den Titel. Die Liste „Zitate prüfen“
  hängt an der ganzen Statuszeile.
- **„Kritisch:“-Erkennung** (nur noch Nathan als Auditor Pflicht): Zeile, die nach Listenzeichen mit „Kritisch:“
  beginnt, oder „Kritisch:“ direkt hinter der Angabe des Teils („- III. Zuspruch: Kritisch: …“, fett, mit Zusatz
  „(Schluss)“ oder nur „II. Kritisch: …“); „Unkritisch:“ oder „… ist kritisch: …“ zählen nicht. Fehlt er nach
  einmaligem Nachfordern, steht ein Hinweis von Baruch unter dem Beitrag. Lösung in fester Form:
  `- statt: „Zitat“`, `- besser: „…“` oder `- Schritt: …`; Fortschritt „fehlender Lösungsvorschlag“; bei
  abgeschnittener Antwort kein Nachfordern.
- **Prüfauftrag erkannt** an einem Wort, das mit „prüf“, „überprüf“, „nachprüf“, „gegenprüf“, „kontrollier“,
  „check“ oder „review“ beginnt, oder an einer Befundnummer („[B12]“). Nur eigene, offene Befunde; fremde,
  unbekannte oder entschiedene Nummern übergeht Baruch; keine doppelte Rückfrage. Sieht die Persona alles
  umgesetzt, sagt sie „Alle meine Befunde zu diesem Abschnitt scheinen erledigt.“ (dann kein Pflichtbefund).
  „Ja, erledigt“ setzt „erledigt ohne Änderung“ (Begründung „laut @persona umgesetzt …“); beides ein Commit.
  Übernommene Befunde zeigen „übernommen in Fassung 3 · Stand 2“.
- **Kritik-Zyklus:** auch über MCP `ask_persona` mit `rev`. Zeigt die aktuelle Fassung nur die Überschrift und
  liegt ein Vorschlag mit Text vor, prüfen alle den Vorschlag (Hinweis „aktuelle Fassung enthält nur die
  Überschrift – Vorschlag n prüfen?“ bei ausdrücklicher Wahl). Nur der Lektor kommentiert fremde Befunde; die
  übrigen sehen sie als „schon gemeldet“.
- **Chat:** neu-Marke im LocalStorage `baruch_chat_seen` (eigene Beiträge nie neu; erster Besuch: fremde Beiträge
  seit dem letzten Besuch irgendeines Abschnitts des Werks; leerer Chat: Merker „war leer“; verglichen werden nur
  Serverzeiten). Ganze Historie `/beitraege` (Anker `#chat-neueste`), Beitrag `/beitrag`, Fäden `/faeden?work=…`,
  Faden `/faden?…`. Vervollständigung ohne JavaScript über `datalist`; am Satzanfang nach zwei Buchstaben die
  Anrede („Le“ → „Lektor, “). Erwähnungen: höchstens drei, jede nur einmal, Groß/Klein egal; inaktive oder
  unbekannte werden übergangen; RFC hebt die Grenze auf. Live: Abfrage alle drei Sekunden.
- **Commits:** Löschen eines Beitrags (trägt den ganzen Text; im Faden „Fadenbeitrag n: gelöscht“), Bearbeiten
  („Beitrag n: bearbeitet“ mit vorher/nachher), Umwandeln in Einfall. Fäden stehen nie im Spiegel: das Bearbeiten
  eines Fadenbeitrags ergibt keinen Commit (alte Fassung nur in Datenbank und Austauschformat). Nach dem
  Verschieben stehen die offenen Befunde beim Zielabschnitt, übernommene bleiben beim alten.
- **MCP:** nur eigene Beiträge (`post_edit`, Verschieben, Löschen, Umwandeln; `docs/MCP.md`). `chat` ohne
  Abschnitt legt einen Faden an; eine Relay-Stufe, höchstens drei Antworten je Nachricht, jede Persona einmal; je
  Client höchstens zwei Aufträge zugleich. Alle Clients ohne Token teilen sich je Werk 300 Beiträge in 24 Stunden,
  100 offene Vorschläge und vier gleichzeitige Modellaufträge (je Name weiter zwei). Relay-Antworten werden bei ↻
  mit derselben Anrede wiederholt. Ältere Antworten ohne gespeicherten Auftrag: ↻ wiederholt die letzte
  Nachricht des Menschen davor als Chat.
- **Sprungmarken im Prompt:** `[key:105] 2.3 Titel`; Personas nennen Stellen als `[[key:105]]` bzw.
  `[[key:105|„Zitat“]]`; ungeprüfte bleiben Text mit „Sprungmarke ungeprüft: …“. HTML aus einer Antwort wird nie
  ausgeführt. Der Schlusshinweis („Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft“) steht als Fußzeile.
- **Befunde:** Seite `/werke/<id>/befunde`; Datei `befunde.md` im Spiegel (nach Teil, Kapitel, Abschnitt, dann
  Fäden); jede Statusänderung, jeder Vorschlag, jede Entscheidung ein Commit. „Angehängt“: Inhaltswörter ab drei
  Buchstaben ohne Füllwörter, dazu jede Zahl, Dice-Maß ab 0,8. Matrix zählt höchstens die neuesten 2000 („Gekappt:
  …“); Liste „Offen, älter als n Tage“ gekappt bei 200. Sammelvorschläge zählen nicht in „k offen“; sind alle
  Ursprünge entschieden: „veraltet (alle Ursprünge entschieden)“. Anspringen über `find` in der Adresse. Beim
  ersten Start mit dieser Funktion zerlegt Baruch einmal alle vorhandenen Persona-Antworten. Austauschdatei
  (`.baruch`, Unterversion 4) trägt Befunde samt Verlauf (`docs/Austauschformat.md`). Lösungen ohne JavaScript an
  `/loesungen/act`; Varianten `/varianten` (Geschwister im Fassungsbaum; lange Abschnitte als Teilvorschlag).
  Mindmap: `?befunde=1`.
- **Websuche:** MultiGPT-Werkzeuge `web_search` (dahinter SearXNG) und `fetch_url`; die ersten drei Rechner der
  Quellenliste je mit „site:<Rechner> <Anfrage>“; [W1]…[W4], je Seite höchstens 1 500 Zeichen; Seiten über 1 MiB
  oder ohne Antwort nach 10 s übersprungen; die ganze Websuche höchstens 30 s. Zitatprüfung: ganze Wörter; mit
  Auslassung „…“ je Stück mindestens drei Wörter, übersprungen höchstens 400 Zeichen; „(unbekannte Quelle)“,
  „(nicht abgerufen)“; letzte Zeile „Quellen: [Wn] Titel – URL (abgerufen am TT.MM.JJJJ)“. Nicht abgerufen werden
  `localhost`, 127.x, 10.x, 192.168.x, 172.16–31.x, 169.254.x, 100.64–127.x, `fc00::/7`, `fe80::/10`, Namen auf
  `.local`, `.internal`, `.home.arpa`, `.localhost`, Namen ohne Punkt, Adressen mit Benutzerangaben, andere
  Schemata; Baruch fragt kein DNS (ein öffentlicher Name auf eine private Adresse wird nicht erkannt). Belegseite
  `/websuche/belege`; übernommener Beleg = Titel der Art „web“ mit Notiz „Abgerufen am … (Websuche @belege)“.
  Kein MCP-Werkzeug; über `ask_persona`/`chat` sucht @belege nicht im Web. Ein Vorschlag bekommt keine Websuche.
  Annahme: Namen und Parameter der MultiGPT-Werkzeuge wie in `connections/multigpt.rs`; gegen einen echten
  MultiGPT-Dienst und eine echte Suchmaschine prüft nur der Nutzer.

### Bibel, Literatur und Quellen (`bibel-quellen.md`)

- **Verszählung:** Zählung „German“ des SWORD-Projekts (Schlachter 1951, `docs/Architektur.md`): „Ps 51,21“,
  „Neh 3,38“, „1. Kön 5,32“, „Dan 3,33“, „3. Joh 15“. Baruch kennt alle 18 Verszählungen des SWORD-Projekts
  (Luther, NRSV, Vulg, MT …). Umrechnen über RPC `scripture.map` bzw. MCP `scripture_map`, Vorgabe KJV → German
  („Joel 2,28–32“ → „Joel 3,1–5“, „Mal 4,1–6“ → „Mal 3,19–24“, „Ps 51,1“ → „Ps 51,3“), nur über eine geprüfte
  Regeltabelle. Nicht abgebildet: die Psalmen der griechisch-lateinischen Zählungen (LXX, Vulgata, Synodal); für
  Catholic, Catholic2, Segond und Calvin gibt es keine geprüften Regeln (anders zählende Bücher bleiben
  „Versifikation passt nicht“).
- **Module:** Baruch liest SWORD-Module selbst (Treiber zText, RawText; Kompression ZIP, LZSS, BZIP2, XZ;
  Kommentare RawCom, RawCom4, zCom, zCom4; Lexika RawLD, RawLD4, zLD), ohne libsword. Nicht lesbar: HREFCom,
  RawFiles, unbekannte Verszählung. Ein Block, der sich zu mehr als 64 MiB entpacken würde, wird abgelehnt.
  Modulordner = Ordner mit `mods.d/` und `modules/`; Vorgabe `$SWORD_PATH`, dann `~/.sword`, dann
  `/usr/share/sword`; ein Modulname gilt nur einmal (früherer Ordner gewinnt). `DataPath` relativ zum Modulordner,
  `..` abgelehnt. Freie Module: CrossWire (`https://www.crosswire.org/sword/modules/`), Installer `installmgr`
  (Paket `sword-utils`), BibleTime, Xiphos; oder Modul-ZIP
  (`https://www.crosswire.org/ftpmirror/pub/sword/packages/rawzip/`) entpacken; eigene Module mit `osis2mod`.
  Lizenzvermerk aus `DistributionLicense`, `Copyright` der conf. Verschlüsselt = `CipherKey=` in der conf;
  Schlüssel im Schlüsselbund unter `sword-cipher:<modul>`, nie in conf, Datenbank oder Seite. Gespeichert:
  Modulordner für alle Werke, Bibel je Werk (Migration 0030), Kommentare/Lexika je Werk (Migration 0037, Formular
  an `POST /einstellungen/werk/bibeln`). RPC `bible.set_dirs`, `bible.set_work`, `bible.cipher_key_set`,
  `bible.cipher_key_delete`, `bible.modules`, `bible.set_work_sources`, `bible.work_sources`, `bible.check`,
  `bible.lookup`, `bible.search`, `bible.marks`, `scripture.check`, `commentary.lookup/search`,
  `lexicon.lookup/keys/search`; MCP nur lesend `bible_modules`, `bible_lookup`, `bible_search`, `scripture_check`,
  `commentary_lookup`, `lexicon_lookup`, `lexicon_search`.
- **Wortlautprüfung:** gelesen wird Klartext ohne Fußnoten, Querverweise und Überschriften (kanonische
  Psalmüberschriften bleiben). Zitatformen: „…“, “…”, »…«, "…", `#quote[…]`, Blockzitat
  `#quote(block: true, attribution: [Joh 3,16])[…]` (Stelle in der Zuschreibung, am Ende oder direkt dahinter;
  auch in `#szene[…]`). Zwischen Stelle und Zitat nur Leerraum, Klammern, Satzzeichen, weitere Stellen derselben
  Liste oder kurze Einleitungswörter mit Doppelpunkt. Innere Anführungszeichen und mehrere Absätze (wiederholtes
  Anführungszeichen oder bis zum Schlusszeichen) gelten; Grenzen: gleichartiges Zitat im Zitat, vergessenes
  Schlusszeichen vor einem Absatz mit demselben Zeichen. Normalisiert: Satzzeichen, Groß/Klein, Bindestriche,
  Trennungen, Apostrophe, Leerraum, typografische Zeichen, Typst-Auszeichnung (`_…_`, `*…*`, Maskierungen,
  Fußnoten im Zitat), zerlegte Umlaute. Steht die Stelle am Absatzanfang und das Zitat dahinter, gehört dieses zur
  Stelle. Trennreste im Modul („die ses“): Vergleich zusätzlich ohne Leerraum → „Wortlaut stimmt (Leerraum im
  Modul)“, duldet nur zusätzliche Wortgrenzen des Moduls; Hinweis bei auffällig vielen Wortfragmenten.
- **Stellenerkennung, Einzelheiten:** Bücher mit einem Kapitel („Jud 3“ = Jud 1,3); „4. Mo 25,19“, „5. Mose 6,4“,
  „4Mo 1,1“; „Joh 3, 1. Joh 4“ sind zwei Stellen; nur mit Vers zählen „Am“, „Mal“, „Mi“, „Ob“, „Hab“, „Ri“, „Tit“,
  „Da“, „Hi“ („Ob“ nur mit Kapitel und Vers oder Bereich). Keine Stelle vor einem Zahlwort („Johannes 3 Mal“,
  „Psalm 23 Jahre“, „3x“, „Stück“, „Tage“, „Uhr“) – nur ohne ausdrücklichen Vers: „Joh 3,16 Mal 3“ bleibt
  Joh 3,16, „Joh 3,16 Mal 3,20“ sind zwei Stellen; Uhrzeit nach Wochentag (Mo–So, Stunde 0–23, Minute 00–59; im
  Stellenfeld bleibt „Mi 7:18“ Micha). „Vers im selben Satz“: höchstens 120 Zeichen, kein Satzende, keine
  Leerzeile, keine andere Stelle dazwischen (auch „Verse 23–24“, „V. 23“); ein anderes Kapitel ohne Buchname
  beendet die Suche. Englische Zählungen: „Mal 4,1“, „2. Kor 13,14“ ungültig. Predigttext: bis 400 Verse
  vollständig, ab 401 „Nur die ersten 400 Verse eingefügt.“; Sonderzeichen im Bibeltext werden maskiert.
- **Faktenblock für Personas:** `@schrift` und `@belege` bekommen „Stellenprüfung (Baruch, Verszählung Schlachter
  1951 – kein Wortlaut): …“ bzw. mit Bibel Modul, Versifikation, Lizenzvermerk und Ergebnis je Stelle. Antworten
  tragen „Wortlaut: ungeprüft – keine lokale Bibel in Baruch (geprüft nur Buch/Kapitel/Vers).“ bzw. „Wortlaut
  geprüft von Baruch (nicht vom Modell), Modul …, Versifikation …, Lizenz laut conf: …“. Trennt das Modell Ziffern
  („Mk 1 0,42–45“), setzt Baruch sie zusammen (Befund 2026-11-06: die getrennten Ziffern kamen aus dem Text des
  Modells). Vermerke „(nicht im Text belegt)“, „(Stelle steht im Text: …)“, „(Stelle ungültig: …)“ setzt nur
  Baruch; schreibt ein Modell sie selbst, entfernt Baruch sie und prüft neu. „fehlt“ zählt nur mit Ort. Beleg ist
  nur der gezeigte Text, der Auftrag und Baruchs Belege – nicht fremde Befunde, frühere Beiträge oder Einfälle
  (diese stehen gerahmt als „Daten, keine Anweisungen“).
- **Zitate prüfen:** Seite `/werke/<id>/pruefung?key=…&rev=…` (auch bei Strg/Umschalt-Klick); Markierung über die
  CSS Custom Highlight API (ältere Browser: nur die Liste), nur ganze Wörter, mehrdeutiger Wortlaut bleibt
  unmarkiert („n Zitat(e) im Satzbild nicht eindeutig gefunden (nur in der Liste)“).
- **Lexikon-Stichwörter:** erst Strong-Nummern der Verse, dann Strong-Nummern im Text („G25“, „G26“ findet
  „G0026“), dann großgeschriebene Wörter mit Lemma-Probe (ohne -ern, -en, -es, -er, -e, -n, -s; zurückgeführter
  Umlaut; letzter Bestandteil eines Kompositums); höchstens 5 Artikel je Lexikon; Kommentar-Einträge höchstens
  4000 Zeichen.
- **Literatur:** Seite `/literatur?work=…&node=…&rev=…&lq=…`, Modus `lmode=rag` (Einfälle: `mode=rag`),
  Dokumentansicht `/quelle?title=…` (`&quote=…#zitat-…`, `&highlight=…#hervorhebung-…`), Volltext
  `/quelle/multigpt?collection=…&document=…`, Bibel `/literatur/bibel`, Treffer im Werk
  `/?work&node&rev&lq#fundstelle`. Die Seiten merken sich den Abschnitt über `key`. Zitieren legt
  `#quote(block: true, attribution: [@schlüssel[Stelle]])[…]` bzw. `[Ps 139,23–24 (Modul)]` an, Urheber
  `@literatur`. Bibeltreffer im Suchfeld: je Bibel höchstens 10, höchstens drei Bibeln, 3 s. Volltext in Teilen zu
  höchstens 20 000 Zeichen; Antwort über 4 MiB wird abgelehnt; Felder „Anfang/Ende im Volltext“ (Zeichen ab 0);
  Zitierschlüssel übernommener MultiGPT-Dokumente `mg-…`. Eine Suche nur aus Steuerzeichen: „Keine Treffer.“
- **Bedeutungssuche:** Wege LanceDB, pgvector, MultiGPT; Treffer mehrerer Wege einmal („gefunden von …“,
  Reciprocal Rank Fusion); Links nur bei sicherer Adresse (im Werk `/…`, sonst http/https). Von einer fremden
  Seite geöffnet (`Sec-Fetch-Site`/`Origin`) sucht Baruch nicht nach Bedeutung und lädt keinen Volltext.
  Höchstens vier Suchen fragen gleichzeitig die Remote-Wege (danach nach 20 s „Zu viele gleichzeitige Suchen …“).
  `weak` in `rag.search` und MCP `rag_search`. Weitere Einzelheiten unter „Einstellungen“.
- **Abgleich:** Seite `/abgleich`. Dokumenttitel „<Nummer> <Titel>“; ein Teil und Abschnitte vor dem ersten
  Kapitel je ein eigenes Dokument; die Sammlung wird beim Einschalten festgehalten. Scheitert das Löschen des
  alten Dokuments, löscht es der nächste Abgleich. Beiträge lädt der Abgleich nicht hoch (anders als im Plan;
  vorgemerkt im Backlog). Geholte Dokumente: Art `remote`; 30 s je Anfrage; unvollständige Listen löschen keine
  lokale Kopie; Rechte `docs.write`/`docs.read`; ohne Dokument-ID Hinweis statt erneutem Hochladen; verlorene
  Antworten meldet der nächste Abgleich; „Verbindung prüfen“ beendet ihre Sitzung bei MultiGPT. Annahme: Werkzeuge
  `search_documents`, `read_document`, `list_collections`, `document_info`, `list_documents`, `upload_document`,
  `delete_document` mit den Parametern aus `connections/multigpt.rs`; geprüft gegen einen Nachbau.

### Einfälle und Mindmap (`einfaelle.md`)

- **Seiten:** Liste `/einfaelle` (`?work=<id>|none|all&word=&status=open|done|all&q=&deleted=1`, Bedeutung
  `&mode=rag`), ein Einfall `/einfaelle/{id}`, Mindmap `/werke/{id}/karte` (alte Lesezeichen der Gedankenkarte
  gelten weiter; `?befunde=1`, eingeklappt `zu=<Kennungen>`), Gliederungsansicht `/werke/{id}/karte/gliederung`,
  Einlesen `/werke/{id}/karte/einlesen`.
- **Einfall:** Doppelte Schlagwörter (auch in anderer Groß-/Kleinschreibung) fallen zusammen; kein Zeilenumbruch
  im Schlagwort. Quelle aus der Literatur: „Literatur: <Titel>“. Löschen prüft der Server. Verarbeiten: Titel aus
  den Schlagwörtern („Laube, Rosen“, höchstens 200 Zeichen, Titelregeln der Gliederung); Punkt und „verarbeitet“
  entstehen in einem Schritt; Beitrag mit Autor „Autor“ bzw. „Notiz“, Markierung als `> …`, Quelle „Einfall <id>“
  bzw. „Einfall 3 von @belege“ / „… von @mcp:claude-code“ (Autor des Beitrags bleibt, wer verarbeitet).
- **Spiegel:** Jede Aktion (anlegen, ändern, löschen, wiederherstellen, verarbeiten, jede Aktion und Umkehr auf
  der Mindmap) ist ein Commit mit Herkunft: `works/<slug>/einfaelle.md` bzw. ohne Werk `<data>/einfaelle/`
  (`einfaelle.md`); der Lösch-Commit trägt den ganzen Eintrag.
- **Assistenten:** voller Zugriff auf Einfälle (Entscheidung IDEA_CRUD; MCP `idea_create`, `idea_update`,
  `idea_delete`, `idea_move`, `idea_link` …). `idea_create` ohne `parent` und `key` legt in die Wolke. Noch nicht
  umgesetzt: Der Lektor schlägt von sich aus passende Stellen vor.
- **Mindmap:** die erste Hälfte der Hauptäste steht rechts, die zweite links. Gespeicherte Lagen der alten
  Gedankenkarte bleiben in der Datenbank, wirken aber nicht. Ist ein Ende einer Beziehung eingeklappt, endet die
  Linie am sichtbaren Vorfahren; Beschriftungen suchen sich einen freien Platz auf deckendem Grund (in einer
  vollen Karte ein Stück neben der Linie). „widerspricht“ ohne Pfeil, Strich-Punkt. Nähe wird nie gespeichert
  (gespeicherte Einbettungen oder gleiche Schlagwörter, bestes Paar). „Gruppen benennen (Assistent)“: Vorschläge
  mit Übernehmen/Verwerfen, nur bis zum Neustart. Eingeklapptes in `localStorage` je Werk; Ausschnitt bis zum
  Schließen des Tabs. Umschalt+Tab und Tab ohne gewählten Knoten bleiben beim Browser. Strg/Umschalt+Klick auf den
  Titel öffnet den Einfall bzw. Abschnitt als Link. An Kapitel, Teil und Werk wirkt Enter wie Tab (am Werk: freies
  Hauptthema). Einfall auf den oberen/unteren Rand eines Einfalls ziehen ordnet davor/dahinter ein. Dunkles Thema:
  Bildlaufleiste am Fensterrand in den Farben der Seite. Menüband der Seite: Gruppen als
  `details.band-group[data-band-group]` – Knoten, Ansicht, Ausgabe, Einlesen, Gliederungsansicht, Zurück.
- **Tasten am Mac (aus `mindmap.ts`):** Alt+E und Alt+L werten auch `event.code` aus (KeyE, KeyL) und gehen darum
  mit ⌥; Strg+Z und das Mausrad werten `ctrlKey || metaKey` aus.
- **Stand der Prüfung:** Die Tests prüfen die Aufrufe, die das Widget absetzt, und das Bild, das der Server danach
  liefert; in der Sichtprüfung lief es in Firefox mit künstlichen Ereignissen. Echte Maus und Tastatur, Zoom und
  Pfeiltasten sind Handprüfung (`HANDOFF.md`).
- **Ausgabe:** `.xmind` = ZIP mit `content.json`, `metadata.json`, `manifest.json`; `.mm` FreeMind/Freeplane;
  `.svg`; `.pdf` (A4 quer, Titel und Filter); `.md`. In `.svg`/`.pdf` bleibt Eingeklapptes eingeklappt, in `.mm`
  und `.xmind` nie. Übertragung: Titel (Kapitel mit Nummer, „1 Helling“); Notiz (`notes.plain.content` bzw.
  `richcontent TYPE="NOTE"`, je Zeile ein Absatz); Schlagwörter (`labels` bzw. Attribut „Schlagwort“);
  Beziehungen (`relationships` mit `end1Id`, `end2Id`, `title` bzw. `arrowlink` mit `MIDDLE_LABEL`; Beschriftung =
  Art und Notiz; `.mm` färbt je Art); nicht: Status, Herkunft, Bezug (nur in `.md`), Zitat, Quelle, Bibelstelle,
  Lagen, Bilder, Stile, Marker.
- **Einlesen:** HTML-Notizen werden reiner Text; Beziehungen kommen als „führt weiter“ mit der Beschriftung als
  Notiz; das Hauptthema der Datei wird nicht selbst zum Einfall; der eigene Ast „Wolke“ wird an seiner Kennung
  (`baruch-wolke`) erkannt, nicht am Titel; „unter Knoten …“ kennt keine Wolke. Ein Commit, Herkunft Mensch.
  Grenzen: `.xmind`-Archiv höchstens 50 Einträge und 20 MiB entpackt; höchstens 20 Schlagwörter je Knoten; `.mm`
  nur UTF-8 oder ASCII; XMind-8 (nur `content.xml`) abgelehnt. Nicht gelesen: weitere Blätter (Hinweis),
  Zusammenfassungen, Sprechblasen, Bilder, Marker, Stile, andere Attribute, `richcontent TYPE="DETAILS"`.
- **Annahmen zu XMind und Freeplane (nicht geprüft, Handprüfung des Nutzers):** Der Aufbau der `.xmind`-Datei
  folgt der offenen Beschreibung des Formats aus dem Gedächtnis; geprüft ist nur die Rundreise mit dem eigenen
  Leser.

  | Annahme | Risiko, falls sie nicht stimmt |
  |---|---|
  | XMind (2020 und neuer) öffnet ein ZIP mit `content.json`, `metadata.json`, `manifest.json` ohne `content.xml` und ohne Vorschaubild | Datei öffnet nicht |
  | `content.json` ist ein Feld von Blättern mit `id`, `class: "sheet"`, `title`, `rootTopic`, `relationships` | Blatt wird nicht erkannt |
  | Themen: `id`, `class: "topic"`, `title`, `children.attached`, `notes.plain.content`, `labels` | Notizen oder Etiketten fehlen |
  | `structureClass: "org.xmind.ui.map.unbalanced"` verteilt die Äste rechts und links | andere Anordnung |
  | Themen-ids dürfen beliebige Texte sein (`i:12`, `n:7`, `baruch-wolke`) | Beziehungen oder Themen fehlen |
  | `relationships`: `id`, `end1Id`, `end2Id`, `title` genügen | Beziehungslinien fehlen |
  | `manifest.json` mit `file-entries` für beide Dateien | Warnung beim Öffnen |
  | Beim Einlesen: freie Themen unter `children.detached`, HTML-Notizen unter `notes.realHTML.content` | Inhalte fehlen nach dem Einlesen |
  | Freeplane liest `MIDDLE_LABEL` und zeigt Attribute an; FreeMind 1.0.1 übergeht `MIDDLE_LABEL` | Beschriftung fehlt in der Anzeige |
  | Freeplane schreibt Beschriftungen als `MIDDLE_LABEL` (sonst `SOURCE_LABEL`/`TARGET_LABEL`), Notizen als `richcontent TYPE="NOTE"` | Beschriftung oder Notiz fehlt nach dem Einlesen |

### Suchen, Verweise, Register und Glossar (`suchen.md`)

- **Suche:** Seite `/suche`
  (`/suche?work=W&q=…&range=…&key=…&sel=1,2,3&case=1&word=1&re=1&kinds=text,heading`); Markierung aus dem Editor
  `range=selection&start=…&end=…` (UTF-16, Bereich „Markierung im Editor (Zeichen a–b)“); gezeigte Fassung
  `shown_rev=n` bzw. `rev=shown` (nur für Abschnitt und Markierung; Vorschau und Ersetzen gelten immer der aktuellen
  Fassung). Treffer-Links `/?work=W&key=K&find=…&at=…`. Ohne Teile ist der Bereich „Teil“ das ganze Werk.
- **Regeln:** Gesucht wird im Typst-Quelltext. Im Wortlaut-Modus findet ein Typst-Sonderzeichen auch seine
  maskierte Form („C#“ findet „C\#“), der Ersatz wird maskiert. Regulärer Ausdruck: Syntax des Rust-Crates `regex`,
  Gruppen `$1` bzw. `${name}`, Ersatz wörtlich im Quelltext. Treffer: Vorgabe 500 gezeigt, höchstens 2000 (mehr
  werden gezählt); nach „abgebrochen“ geht Ersetzen erst mit kleinerem Bereich. Die Vorschau zeigt nur ganze
  Abschnitte mit zusammen höchstens 2000 Stellen; über 8 MiB eingesetzter Text: „Zu viele Ersetzungen auf einmal“.
  Die Vorschau startet nicht auf Anfrage einer fremden Seite (403). Ein Abschnitt, dessen aktuelle Fassung seit
  der Vorschau eine andere ist, wird mit Grund übersprungen.
- **Commits:** „Ersetzt „x“ → „y“ in n Abschnitten“ (ein Commit für alle), „Ersetzen rückgängig: …“. MCP sucht nur
  (`text_search`); Vorschläge dort über `propose_revision`.
- **Tasten (aus `search_keys.ts`):** Strg gilt auch als ⌘ (`ctrlKey || metaKey`); Buchstaben über `code`, darum
  gehen Alt+C/W/R/L am Mac mit ⌥. ⌘H fängt macOS in der Regel selbst ab (Fenster ausblenden) – am Mac nicht
  geprüft; Ctrl+H erreicht den Browser. Nach „Alle“ geht „⌶“ wieder aus. Bei offenem Satzbild schaltet der Editor
  nach dem Ersetzen auf den Quelltext. Auf `/suche` schalten Alt+C/W/R die Kästchen, wenn der Fokus im Such- oder
  Ersatzfeld steht.
- **Sprungmarken:** Jeder Gliederungspunkt hat `<k{Schlüssel}>` (etwa `<k103>`), nur in Export, Spiegel,
  Satz-Ansicht und PDF: am Zeilenende der Überschrift (`= Einleitung <k103>`); Teile, Punkte ohne Nummer und
  Überschriften mit eigenem Label behalten ihr Label und bekommen darunter `#metadata("baruch-key") <k103>`. Beim
  ersten Start mit dieser Funktion ändert sich jeder Spiegel einmal (ein Commit je Werk). Eingefügt wird
  `#link(<k103>)[die Einleitung]` bzw. mit Seitenzahl
  `#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]`. Ohne JavaScript schreibt man die
  Form selbst; `xref.targets` bzw. `/verweise` (`?key=K`) nennen die Marken. Strg+Klick wirkt auch auf
  `<kN>`/`#link(<kN>)` im Quelltext. Sprungverlauf je Browser-Tab (`sessionStorage`); „Vorwärts“ nur als Knopf.
  Beim Wiedereinlesen eines Exports verschwinden die Marken aus den Überschriften, die Verweise zeigen auf die
  neuen Schlüssel. Im EPUB interne Links auf die Überschrift in der Kapiteldatei. Gezählte Nennungen („das dritte
  Kapitel“) nur für Kapitel mit Nummer. Ist die Werk-Einstellung „Verweis mit Seitenzahl“ ungesetzt, merkt sich
  der Browser die letzte Wahl; gesetzt gilt eine Änderung im Dialog nur für diesen Verweis.
- **Glossar und Register:** Seiten `/werke/{id}/glossar`, `/werke/{id}/register` (`?key=K`: Abschnitt mit „Marke
  setzen“), `/werke/{id}/markierung?key=K&rev=R`; ohne Werk `/werke`. Registermarke im Text:
  `#metadata((register: "stichwort", term: "Gnade"))<baruch-idx>`; nie in Überschriften, Code, Kommentaren, Raw
  oder einer anderen Marke; Meldung „… trägt an der gewählten Fundstelle schon diese Registermarke; nichts
  geändert“; „alle“ setzt nur fehlende. Eigene Register: Kennung aus Kleinbuchstaben, Ziffern, Bindestrich
  (z. B. `orte`), Titel bis 80 Zeichen, höchstens 13. Registervorschläge werden „veraltet“, wenn das Wort fehlt
  oder die Marke schon trägt; sie bleiben auf ihrer Instanz. Verworfene Glossarvorschläge hinterlassen einen
  Grabstein (kommen beim Austausch nicht wieder). RPC `glossary.suggest`; MCP `glossary_list`,
  `glossary_propose`, `index_propose`. Spiegel: Datei `glossar.md`, jede Änderung ein Commit. Austauschdatei
  (Unterversion 7) trägt Glossar samt Vorschlägen, eigene Register und die Satz-Einstellungen dafür; höchstens
  5000 Glossarbegriffe, Registertitel höchstens 80 Zeichen (sonst wird die Datei ganz abgelehnt).
- **Im Satz:** Register- und Glossarseiten entstehen nur beim Herausschreiben (Typst-Export, Spiegel,
  Satz-Vorschau, PDF, EPUB) ab der Zeile `// Baruch: Register und Glossar`; der gespeicherte Text bleibt. Der
  Export lässt sich mit `typst compile` ohne Paket und ohne Netz setzen (geprüft mit dem eingebetteten Compiler).
  Im EPUB `Glossar` und je Register eine eigene Datei am Ende. Bibelstellenregister: nicht aus Kommentaren, Code
  oder Überschriften; Reihenfolge der biblischen Bücher. Beim Wiedereinlesen bleiben die Registermarken des
  Menschen; die Hervorhebung der ersten Erwähnung ohne Glossarseite bleibt als gewöhnliche Hervorhebung.
  Ende-zu-Ende geprüft in `tests/round31_abnahme.rs`.

### Buchsatz: PDF, E-Book und Umschlag (`buchsatz.md`)

- **Adressen:** Satz `/werke/<id>/satz?key=<schlüssel>` (Abschnitt `#druckpruefung`), PDF `/werke/<id>/pdf`
  (`<slug>.pdf`), Umschlag `/werke/<id>/umschlag`, Umschlag-PDF `/werke/<id>/umschlag.pdf`
  (`<slug>-umschlag.pdf`), EPUB `/werke/{id}/epub` (`?fussnoten=endnoten`), Vorschau `/werke/{id}/epub/vorschau`,
  Prüfung `/werke/<id>/epub/pruefung`, EPUBCheck `POST /werke/epub/pruefen`, Stichwortzettel
  `/werke/<id>/stichwortzettel` und `…/stichwortzettel.pdf?format=…&schrift=…` (`<slug>-stichwortzettel.pdf`),
  Word `/werke/<id>/word` (`<ordnername>.docx`). Entwurf: jede Ausgabe mit `?entwurf=1` bzw. `?entwurf=0`
  (`/pdf`, `/stichwortzettel.pdf`, `/epub`, `/umschlag.pdf`, `/satz`).
- **Satz-Seite:** lädt kein JavaScript (Thema-Zeichen führt zu Einstellungen → Darstellung, „Einstellungen“ öffnet
  die Seite, kein Alt+, und keine Pfeiltasten). Eine Seite, die zwei Kapitel teilen, steht in der Liste beider;
  ein lebender Kolumnentitel zählt nicht als Text des Kapitels. Meldungen mit Zeile und Spalte im Export.
  Fehlende Schrift: „Schrift „x“ fehlt; ersetzt durch eingebettete Schrift“. Pakete (`#import "@preview/…"`):
  „Typst-Pakete werden nicht geladen (offline)“. `image("/x")`, `image("./x")`, `image("x")` finden dasselbe Bild.
  Nach 60 s „Typst brauchte länger als 60 s“ (503); Zwischenspeicher höchstens 8 Stände und 192 MiB Seiten.
  Abbrechen: „Der Satz wurde abgebrochen. Es wurde nicht neu gesetzt.“ / „Es lief kein Satz, der abzubrechen
  war.“; wartende Anfragen 503. Typst lässt sich nicht mitten im Rechnen anhalten: Es hört nach dem Umbruch auf;
  eine lange Schleife der Vorlage läuft zu Ende. Von einer fremden Website starten Satz-Seite, Seitenbilder und
  PDF keinen Satz (403). Druckprüfung, Beispiel: „Bild „umschlag.png“: 142 dpi bei 14,8 × 21,6 cm – für 300 dpi
  braucht es 1748 × 2552 Pixel“; SVG erscheint nie; sie zählt nicht zu den Fehlern von Typst.
- **Satz-Einstellungen:** Tabelle `work_typeset`, jede Änderung ein Commit; Regelblock `// Baruch: Satz` hinter
  der Werkvorlage in `main.typ`, Export, Satz-Vorschau, Buch-PDF und Umschlag – nur wenn mindestens eine
  Einstellung gesetzt ist. Ändern nur der Mensch (MCP `typeset_settings` liest). Anker `#satz`. Grenzen und
  Einzelheiten: Bundsteg leer = 0; Absatzmaß leer = 1 bzw. 0,5 em; Sprache z. B. `de`, `en`, `de-CH`; Zählung „ab
  dem ersten Kapitel“: die erste Kapitelseite trägt die 1, ein Querverweis „S. n“ nennt die gedruckte Zahl; der
  Werktitel für den Kolumnentitel kommt aus `#set document(title: …)`; der Umbruch des Kapitelanfangs gilt im
  Export auch vor einem Teil; bei Absätzen mit Einzug bleibt der erste Absatz nach einer Überschrift ohne Einzug.
  Übernehmbare Regel: `#show heading.where(level: 1): it => { pagebreak(weak: true, to: "odd"); it }` (auch
  abgewandelt, mehrzeilig oder `it => [#pagebreak() #it]`); Übernehmen = neue Fassung der Werkvorlage ohne die
  Zeile, ein Commit. Vorschaubilder liegen nur im Zwischenspeicher. „Erzeugte Typst-Regeln“ zeigt den Block.
  Seitenmaß des Umschlags folgt der ersten Buchseite; der Rücken rechnet mit der Seitenzahl samt Leerseiten.
- **Satzbild-Einstellungen** (Anker `#satzbild`, nur SQLite, nicht im Spiegel und nicht im Austauschformat):
  Schrift als Liste wie in CSS (höchstens 200 Zeichen, ohne `; < > { }`), Größe 6–48 pt, Satzbreite 6–60 cm mit
  `pt`, `mm`, `cm`, `em`, Farben `#rrggbb`; „Alles aus der Vorlage“ nimmt alle zurück.
- **Entwurf:** Text höchstens 40, Zusatz 120 Zeichen; Datum = Tag der Ausgabe in Ortszeit; Deckkraft 5–50 %
  (Vorgabe 12 %); auf dem Umschlag-PDF nur mit Häkchen (dann Warnung der Druckprüfung); EPUB: Hinweis auf der
  Titelseite und „(Entwurf)“ im Titel; Export, Spiegel und erzeugte Regeln bleiben ohne Wasserzeichen.
- **PDF:** Fehler 422 (Seite mit Meldungen), 409 („Für dieses Werk entsteht gerade ein PDF …“), 503 nach 120 s.
  Kommandozeile `baruch-server export <id|slug> --pdf DATEI [--data ORDNER]`: vorhandene Datei Exit 1 („… gibt es
  schon; nichts überschrieben“), Satzfehler Exit 1, `--pdf` mit `--out` Exit 2; Warnungen der Druckprüfung auf
  stderr. `pdfinfo` (poppler 25.03) meldet bei allen PDFs „Syntax Error: Suspects object is wrong type
  (boolean)“ – harmlos: Typst schreibt `/MarkInfo << /Marked true /Suspects false >>` (PDF/UA); gemessen im Test
  `round21_bilder`.
- **Umschlag:** Blöcke je Fläche in mm ab der Schnittkante oben links (negativ = im Beschnitt; y zwischen −50 und
  1000); Farbe `light`, `dark` oder `#rrggbb`; Freitext/Motto höchstens 1000 Zeichen; Einrasten bis 1,5 mm. Ohne
  JavaScript trägt die Adresse die Werte als `b.<Art>.<Feld>=…` (z. B. `?b.front_title.y_mm=40`). Schnellwahl und
  Zurücksetzen sind je ein Commit. Sobald ein Block gespeichert ist, setzt Baruch den Umschlag aus den Blöcken
  (Vorschau, Umschlag-PDF, Vorder-/Rückseite im Buch-PDF). Schrift ohne Auswahl: die erste aus dem letzten
  `#set text(font: …)` auf oberster Ebene. Höhe der Blöcke misst Typst mit den Text- und Absatzregeln der
  Werkvorlage (`#set text(…)`/`#set par(…)` am Zeilenanfang; nicht Schrift, Größe, Farbe). Eigene Schriftdateien
  hochladen geht noch nicht. Namensschilder der Rückenblöcke nur am gewählten Block. Kontrast: WCAG unter 3 : 1,
  gemessen im Rasterbild samt Schatten/Verlauf; SVG nicht gemessen; Bilder über 20 000 Pixel Kantenlänge „nicht
  gemessen“. Rücken-Rechnung: „Rücken 1,5 mm = 30 Buchseiten (15 Blatt) × 0,1 mm je Blatt (die 3 Umschlagseiten
  im Satz zählen nicht)“. Löschen des Umschlagbilds entfernt zuerst den Umschlag (Commit „Umschlag entfernt“); ein
  bestätigter Umschlag-Antrag ist ein Commit „Bildantrag n bestätigt: …“. RPC `cover.layout`; MCP `cover_layout`
  liest, `cover_layout_set` stellt nur einen Antrag.
- **Buchangaben:** Jahr vierstellig; ISBN-13 mit oder ohne Bindestriche, Präfix 978/979, gespeichert wie
  eingegeben; einzeilige Felder 200 Zeichen, Klappentext 4000, Kurzbiografie 2000, Widmung, Rechtehinweis,
  Bildnachweis 1000, Anschriften 500; ungültig 422; Commit „Buchangaben geändert: …“.
- **Titelei aus der Werkvorlage:** Funktionen `#let schmutztitel(buch) = …`, `haupttitel`, `impressum`, `widmung`;
  `buch` ist ein Wörterbuch aus Zeichenketten: `titel`, `untertitel`, `autor`, `verlag`, `verlag_anschrift`,
  `ort`, `jahr`, `auflage`, `isbn`, `preis`, `reihe`, `druck`, `druck_anschrift`, `bibel`, `bibel_hinweis`,
  `bildnachweis`, `widmung`, `klappentext`, `vita`, `copyright`, `dnb_titel`, `dnb_text`, `gesetzt`,
  `impressum_zeilen`. Reihenfolge: Schmutztitel, Seite 2 (Reihe bzw. leer), Haupttitel, Impressum (Hinweis der
  Deutschen Nationalbibliothek, ©-Zeile, Auflage, ISBN, Bibelübersetzung, Bildnachweis, „Gesetzt mit Baruch“,
  Druck), Widmung samt Leerseite. Neue Bücher bekommen die Impressum-Vorlage angehängt. Anschriften: Verlag nach
  der ©-Zeile, Druckerei hinter „Druck:“, Zeilen mit Komma verbunden. Rückseite: Strichcode EAN-13 bzw.
  Platzhalter.
- **EPUB:** EPUB 3 mit `toc.ncx` für EPUB-2-Leser; ohne Typst-Satz, kein Commit; `break-before` im Stil.
  Titelbild: gesetzte Vorderseite als PNG bis 1600 × 2560 Pixel; scheitert das, die Bilddatei mit Warnung
  `COVER_RASTER_FAILED`. Sprache: Satz-Einstellung, sonst `#set text(lang: …)`, sonst Deutsch. Hinweiscodes:
  `ISLAND_OMITTED`, `IMAGE_NO_ALT`, `IMAGE_MISSING`, `ARGS_OMITTED`, `TEMPLATE_PAGEBREAK_MAPPED`,
  `TEMPLATE_PAGEBREAK_UNMAPPED` (RPC `work.epub`, MCP `epub_export`, CLI stderr). Gleicher Stand, gleiche Bytes;
  ohne Abschnitt 422; fremde Seite 403. Ein Verweis `@name` auf eine Abbildung zeigt „Abbildung n“; eine Fußnote
  in einer Fußnote steht hinter ihrer äußeren. Profile der Vorschau: Apple Books (iPhone, iPad), Google Play
  Books, „Kindle (über EPUB)“, „Kindle-App (über EPUB)“, wählbar tolino, Kobo, Thorium Reader, calibre – Annahmen
  ohne Gerät; Popup-Fußnoten erscheinen als Kasten; unbekannte Reader/Dateien 404 mit Hinweis; Pfeiltasten in
  einer Auswahlliste laden nicht neu (Enter bestätigt). Prüfung: `mimetype` als erster unkomprimierter Eintrag,
  `META-INF/container.xml`, Paketdokument, Manifest, Spine, nav, NCX; Wohlgeformtheit mit `xmllint --noout
  --nonet`, sonst quick-xml. EPUBCheck: `epubcheck` im PATH, sonst das eingestellte `epubcheck.jar` mit
  `java -jar`; höchstens 3 Minuten, einer zur Zeit; MCP `epub_check` bekommt nur die eigene Prüfung.
- **Stichwortzettel:** Markierung `// Stichwort: …` (Groß/Klein egal), sonst `*…*` und `#strong[…]`, sonst erster
  Satz je Absatz. Kein Satzende nach Ziffer („1. Kor“) und nach Abkürzungen („z. B.“, „d. h.“, „u. a.“, „bzw.“,
  „vgl.“, „usw.“, „ca.“, „Nr.“, „S.“, „Kap.“, „V.“, „etc.“, „ggf.“, „evtl.“ …). Nicht gezählt: Überschriften,
  Kommentare, `#set`/`#show`/`#let`/`#import`/`#include`, Raw, Mathe, `#footnote[…]`, `#arbeit[…]`; tiefer als
  32 Ebenen fällt weg; `@name` fällt weg (ein `@` mitten im Wort bleibt). Bibelstellen einheitlich, jede einmal.
  A5 hoch Vorgabe 16 pt, Karten 14 pt. Fehler 409/422/503 wie beim PDF. Kommandozeile
  `baruch-server export <id|slug> --stichwort DATEI [--format a5|karten] [--schrift PT] [--data ORDNER]`
  („Stichwortzettel geschrieben: DATEI (n Seiten, m Bytes)“; falsche Kombinationen Exit 2).
- **Word-Export:** Formatvorlagen „Überschrift 1–3“ (tiefere „Überschrift 4–6“), „Baruch Teil“, „Baruch ohne
  Nummer 1–3“, „Zitat“, „Baruch Zitatquelle“, Zeichenvorlage „Baruch Zitat“; Tabellen aus
  `#table(columns: …, […], …)` (auch `table.header`); Bilder PNG, JPEG, GIF eingebettet; Typst-Inseln mit
  verborgenem Quelltext. Warnungen (RPC, CLI): `docx_typst_insel`, `docx_bild_fehlt`,
  `docx_tabelle_vereinfacht`, `docx_weitere_vorschlaege`; die Seite im Browser liefert ohne Bericht.
  Änderungsverfolgung: wortweise; neue/gestrichene Abbildung als Absatz; bei Tabellen bleibt das leere Gitter;
  Seitenumbrüche nicht verfolgt; Ebenenwechsel einer Überschrift als gestrichen und neu. Ausgeblendete Fassungen
  zählen nicht. Kommandozeile
  `baruch-server export ID|SLUG --docx DATEI [--vorschlaege] [--data ORDNER]`. Über MCP gibt es ihn nicht.
- **Typst-Kommandozeile:** `baruch-typst compile buch.typ buch.pdf` bzw. `baruch-server typst compile …`
  (`docs/Verteilung.md`, „Typst-Kommandozeile“).

### Austausch, Sicherung und Sicherheit (`austausch-sicherung.md`)

- **Austausch:** Format in `docs/Austauschformat.md`. Ausgeben `/werke/{id}/austausch` (ohne Werk zur Werkliste):
  `.baruch` = ZIP-Container mit dem Werk als XML, den Bildern als eigene Dateien und einem Manifest mit Prüfsummen;
  `.baruch.xml` = eine XML-Datei mit Bildern als Base64. Umfang auch: Grabsteine gelöschter Teile, Mindmap mit
  Verbindungen (auch gelöschte) und gemerkten Lagen (Unterversion 2), Befunde samt Verlauf (Unterversion 4),
  Glossar und Register (Unterversion 7), Verlauf der Fortschreibungen. Nicht darin: Schlüssel und Token,
  Verbindungen (Modelle, MultiGPT, pgvector), Personas samt Modellen, offene Anträge, die berechnete Nähe, Bibel-
  und Literaturtexte, Konten, Sitzungen, Freigaben, die Einstellungen „Fassungen“ und „Satzbild“, Marken zum
  Ausblenden. Kommandozeile `baruch-server export ID|SLUG --baruch DATEI [--data ORDNER]` (endet `DATEI` auf
  `.xml`, nur XML; vorhandene Datei Exit 1; mit `--out`, `--pdf`, `--stichwort` Exit 2; Ausgabe nennt Werk,
  Kennung, Größe, SHA-256). RPC `work.exchange_export`, MCP `exchange_export` (nur lesend, höchstens 4 MiB).
- **Einlesen:** `/austausch/einlesen`; Abweichungs-Links `/?work=W&key=K&rev=R`; Commit „Austausch aus
  <Instanz>“. Eine Verbindung der Mindmap, die es schon gibt (gleiche Einfälle, gleiche Art), kommt nicht doppelt;
  eine Lage nur für Einfälle ohne eigene. Wurde ein Punkt drüben über die Gliederung umbenannt, reist die Fassung
  mit der neuen Überschrift mit: Hinweis „Umschalten genügt“ statt eines Antrags. Unbekanntes aus einer neueren
  Datei wird nicht übernommen. Vorspulen nur für die eigene aktuelle Fassung und nur, wenn die Schritte der Datei
  den neuen Text lückenlos ergeben; Fassungen von Personas oder MCP-Clients, Vorschläge und abgelöste Fassungen
  werden nie überschrieben („abweichende Fassung“). Ein neuer Teil aus der Datei entsteht nach Zustimmung „mit
  Kennung und n Fassungen“ genau wie drüben (Text, Herkunft, Status, Verlauf). Befunde, die es hier schon gibt,
  behalten Status, Begründung und Verlauf („… 2 Einträge im Verlauf der Datei nicht übernommen“). Umschlag-Blöcke
  kommen in Normalform an (Schriftname ohne Leerraum am Rand, Farbe klein, Zeilenenden `\n`); ein Block, den die
  Prüfung des Editors ablehnt, wird übernommen und steht unter den Hinweisen. Passt die Überschrift einer Fassung
  nicht zur Art ihres Punkts (`===` an einem Kapitel), steht ein Hinweis da. Kommandozeile
  `baruch-server import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title TITEL] [--data ORDNER]`
  (Werk schon da ohne `--abgleichen`: Exit 1; `--abgleichen` bei unbekanntem Werk: Exit 1; mit `.typ`: Exit 2).
  Über MCP gibt es das Einlesen nicht.
- **Instanz:** Kennung = UUID, beim ersten Start vergeben; Name ohne Zeilenumbrüche oder Tabs, Leerraum am Rand
  fällt weg; ohne Namen „ohne Namen (3f2b8c1e)“ (erste 8 Zeichen der Kennung). Kennung des Werks (UUID) unter
  „Werk bearbeiten“.
- **Sicherung:** Ordner `<Name>.sicherung-<JJJJ-MM-TT-HHMMSS>` (Ortszeit), Datenbank über `VACUUM INTO`, dazu
  Spiegel, Einfälle und alles andere; symbolische Verknüpfungen werden übersprungen; eine zweite Sicherung in
  derselben Sekunde scheitert; Grenze 10 Sicherungen; nach der Sicherung müssen 512 MiB frei bleiben. Adresse
  nach dem Anlegen `?ok=sicherung`; „Sicherung angelegt“ steht nur einmal da. Nicht über MCP. Zurückspielen:
  `docs/Betrieb.md`.
- **Sicherheit:** Schreibende Aufrufe (`/rpc`, `/act`, `/mcp`) nur von der eigenen Seite; andere `Origin` bzw.
  `Sec-Fetch-Site` → 403 (Skripte ohne Browser schicken diese Kopfzeilen nicht). Ebenso 403 für Satz, Satz-Seiten
  als Bild, Umschlag (Seite, Bild, PDF), Buch-PDF, Stichwortzettel-PDF und Mindmap als PDF, wenn eine fremde Seite
  sie anstößt. Alle Seiten antworten nur unter IP-Adresse oder `localhost` (`http://127.0.0.1:8765/`,
  `http://[::1]:8765/`, `http://localhost:8765/`); fremder Name (DNS-Rebinding) → 421. Unter einem Rechnernamen
  nur mit `--allow-host NAME` / `BARUCH_ALLOWED_HOSTS` (genau der Name, beliebiger Port in `Host`; kein
  Platzhalter, kein Port, keine IP – ungültig: Exit 2). Der Proxy muss `Host` unverändert durchreichen (sonst 403
  „Origin passt nicht zu Host“); `X-Forwarded-*` wertet Baruch nicht aus. Unter einem erlaubten Namen gilt eine
  Anfrage nie als „vom eigenen Rechner“ (`/mcp` dort immer mit Token, 401 ohne). Ohne Konten muss der Proxy den
  Zugang schützen. Beispiele (nginx, Caddy), Probe und TLS zu pgvector (`PGSSLROOTCERT`): `docs/Betrieb.md`.
  `/mcp` ohne Token nur von 127.0.0.1 bzw. ::1 (sonst 401); mit Token auch von anderen Rechnern, alles andere dort
  403. Über MCP nur Vorschläge, Beiträge und Einfälle; Abschnitte nur über `key` (ein Aufruf mit `node` wird
  abgelehnt, `docs/MCP.md`). Der Import liest nur gewöhnliche Dateien (FIFO oder Gerät werden vor dem Öffnen
  abgelehnt).
- **Datenordner:** Meldung zu Rechten mit Zahl (z. B. 0755); `chmod 700 <Ordner>` lässt nur den Besitzer hinein.

### Einstellungen (`einstellungen.md`)

- **Adressen:** `/einstellungen/<seite>` (`darstellung`, `werk`, `assistenten`, `verbindungen`, `quellen`, `mcp`,
  `daten`, mit Konten `konten`); die alte Adresse `/einstellungen/personas` leitet auf `assistenten`. Das Popup ist
  bis 1480 px breit; ein abgelehntes Formular gilt nicht als gespeichert (Thema, Satzbild und Abzeichen der
  Arbeitsseite werden dann nicht neu geholt). Abzeichen und Popup brauchen JavaScript.
- **Cookies (`baruch_*`):** `baruch_work`, `baruch_editor`, `baruch_pv` (Vorschläge anfangs auf/zu),
  `baruch_folded` (höchstens 200 Einträge `<key>-<rev>`), `baruch_band`, `baruch_laptop`, `baruch_lrail`,
  `baruch_rrail`, `baruch_zoom`, dazu Spaltenbreiten, offener Bereich rechts, zugeklappte Kapitel und das Thema.
- **Werk:** gespeichert in SQLite und im Git-Spiegel. Abgleich-Schalter: Kästchen „Ja, die aktuellen Fassungen
  dieses Werks dürfen nach MultiGPT“; das Schalten selbst schickt nichts.
- **Assistenten** (SQLite, alle Werke): Tabs ohne JavaScript als `/einstellungen/assistenten?persona=<handle ohne
  @>` (Anker `#persona-<handle>`; unbekanntes `?persona=` zeigt den ersten Tab mit Hinweis). Grenzen: Name 1–80,
  Rolle und Modell 1–200 Zeichen, Kontext 1–10 000 000 Tokens, Prompt 1–8000 Zeichen, Farbe `#rrggbb` oder `#rgb`
  (ein ausgefülltes Textfeld geht vor); ungültig 422. Palette: acht Farben nach der Reihenfolge der Personas;
  Hintergründe werden auf mindestens 4,5 : 1 Kontrast gebracht (WCAG AA); Farben stehen nicht im Spiegel und
  lassen sich über MCP nicht ändern. Bei Gleichstand des Kontexts gilt die frühere Persona. Warnungen „Modell …
  ist unbekannt bei Eliezer“, „Alter Name: Alias → …“, Hinweis auf den Grafikspeicher bei verschiedenen Modellen,
  „Modellliste nicht verfügbar: …“. Die Kontextlänge dient nur dem Kürzen; Baruch fordert nie eine an. Formulare:
  `action=rag` (bis zu fünf Treffer `[R1]`…`[R5]`, der geprüfte Abschnitt ist nie darunter), `POST /websuche/act`,
  `action=backend`, `action=bible` (Vorgabe: @belege an, Lektor aus). Wirksamer Kontext: Eliezer und Ollama
  melden die Länge, OpenAI-kompatible Dienste nicht; bei Ollama zählt `num_ctx` aus dem Modelfile, sonst die Länge
  des geladenen Modells, sonst 4096 Tokens. Beim Öffnen fragt Baruch keine Gegenstelle (nur der gezeigte Tab auf
  Wunsch, höchstens 10 s, 60 s gemerkt); Beispiel „wirksam 4096 Tokens (Einstellung 32768), Gegenstelle meldet
  4096“.
- **Verbindungen** (SQLite; Geheimnisse im Schlüsselbund, Dienst `baruch`):

  | Verbindung | Felder | Geheimnis | Vorgabe |
  |---|---|---|---|
  | Eliezer (Modell-Backend) | Adresse | Schlüssel `baruch/eliezer` | `http://127.0.0.1:1234` |
  | MultiGPT (MCP) | Adresse | Token `baruch/multigpt` | leer (= nicht eingerichtet); Platzhalter `https://multigpt.example/mcp/` |
  | pgvector (PostgreSQL) | Rechner, Port, Datenbank, Benutzer, TLS (`disable`/`prefer`/`require`/`verify-full`); Tabelle (`tabelle` oder `schema.tabelle`), Spalten Text, Vektor, Sammlung, Dokument (Pflicht), Stelle, Titel, Adresse, Modell; Einbettungsmodell; Präfix der Anfrage | Passwort `baruch/pgvector` | Rechner leer, Port 5432, TLS `prefer`; `chunks`, `content`, `embedding`, `collection`, `document`, `nomic-embed-text` |
  | LanceDB (lokal) | Ordner (relativ zum Datenordner oder absolut), Einbettungsmodell, Präfix der Anfrage und der Dokumente | – | `lancedb`, `nomic-embed-text` |

  MultiGPT hatte früher eine Intranet-Adresse als Vorgabe; seitdem ist sie leer. Der Platzhalter wird nie
  gespeichert und nie angefragt; eine gespeicherte Adresse bleibt (keine Migration); wer nur die Vorgabe benutzt
  hat, trägt die Adresse einmal ein. Ohne Adresse: Weg „aus“, Volltext „Keine Adresse für MultiGPT eingetragen
  (Einstellungen → Verbindungen)“. Aufgaben-Präfixe (für nomic-embed-text `search_query: ` und
  `search_document: `, das Leerzeichen gehört dazu; höchstens 64 Zeichen); ein geändertes Dokument-Präfix verlangt
  „Index neu aufbauen“. TLS-Hinweis bei Passwort mit `prefer`/`disable`, milder bei `require`; nur `verify-full`
  ohne Hinweis. Spaltennamen: Buchstaben, Ziffern, `_`, höchstens 63 Zeichen, bestätigt gegen
  `information_schema`. Geheimnis: nicht leer, eine Zeile, höchstens 4096 Zeichen; gebunden an Schema, Rechner
  und Port (pgvector: Rechner, Port, TLS) – sonst „Nicht geprüft: …“ bzw. „Schlüssel gilt für ein anderes Ziel;
  nicht gesendet“. Adressen nur `http://`/`https://`, ohne Benutzerangaben und ohne Token in der Abfrage.
  **Verbindung prüfen** (10 s gemeinsam, Antwort höchstens 1 MiB, keine Weiterleitung – eine Weiterleitung ist
  ein Fehler, Geheimnisse gehen nie an ihr Ziel): Eliezer `GET <Adresse>/v1/models` mit `Authorization: Bearer`,
  danach Abgleich der Modelle aller Personas (Liste 60 s gültig); MultiGPT MCP `initialize` (Protokoll
  `2025-06-18`, Client `baruch`), `notifications/initialized`, `tools/list` mit `Mcp-Session-Id` über höchstens
  10 Seiten (`nextCursor`), `list_collections`, JSON oder `text/event-stream`; pgvector Anmeldung (nie aus
  `PGPASSWORD`/`~/.pgpass`), Version der Erweiterung `vector`, Tabelle, Sammlungen, Dimension, Modelle (Werte
  gekürzt: Sammlungsnamen 200 Zeichen, höchstens 1000 Sammlungen); LanceDB Ordner, Tabellen, Einträge, Modell,
  Dimension und Probe-Einbettung „Baruch prüft die Einbettung.“. HTTP 401: „Zugang verweigert … Schlüssel
  baruch/… prüfen“. Kurztest: höchstens 16 Token, vorgewählt `google/gemma-4-26b-a4b`. Modell-Verbindungen
  (Abschnitt `#modelle`): Arten Eliezer, OpenAI-kompatibel (`/v1/models`, `/v1/chat/completions`), Ollama;
  Schlüssel `baruch/model-<id>`; „nach der Adresse“ lokal sind Loopback (`127.…`, `localhost`, `::1`), private
  Adressen (`10.…`, `172.16–31.…`, `192.168.…`, `fd…`), Namen ohne Punkt und unter `.local`, `.lan`, `.internal`,
  `.home.arpa`; je Verbindung eine Anfrage zur Zeit; ist Eliezer selbst extern, bettet Baruch nichts ein.
  Formulare an `POST /einstellungen/verbindungen/act` bzw. `POST /einstellungen/modelle/act`, Antwort 303 mit
  `?ok=<code>` / `?fehler=<code>[&kind=…]` / `?modellfehler=…`; in der Adresse stehen nie Eingaben.
- **Bedeutungssuche (RAG):** Abschnitt `#rag`; Vorgabe ohne Wahl: LanceDB; eine Anfrage (RPC, MCP, Persona) kann
  Wege nur weglassen. Sammlungen höchstens 100, je 1–200 Zeichen. Der lokale Index (Vorgabe
  `<Datenordner>/lancedb`) bettet nur geänderte Absätze neu ein (über Eliezer, eigene Warteschlange, höchstens 32
  Texte je Anfrage), holt beim Start alle Werke nach, merkt sich Modell und Dimension („Der Index ist mit dem
  Einbettungsmodell … aufgebaut, eingestellt ist …“). Scheitert das Einbetten dreimal, bleiben Aufträge liegen
  („n Aufträge liegengeblieben – erneuter Versuch nach einer Änderung oder „Index neu aufbauen““); eine Suche
  versucht es nicht erneut. Mit eigenem `--data` ohne gespeichertes Eliezer gibt es beim Start keine
  Einbettungen. „Index neu aufbauen“ antwortet sofort und bricht einen hängenden Auftrag ab; nur der Mensch. Je
  Suche alle Wege gleichzeitig (je 20 s). pgvector: Kosinus-Abstand, nur lesend. MultiGPT: `search_documents`,
  `read_document`.
- **Quellen:** Titel 1–200 Zeichen, Adresse http/https; Links mit `rel="noopener noreferrer"`; MCP
  `sources_list`; Formulare an `POST /einstellungen/quellen/act`. Eine Lesezeichenliste lässt sich nicht einlesen.
- **MCP-Zugang:** gespeichert nur der SHA-256-Hash; Adresse `http://<Rechner>:<Port>/mcp`; Token `baruch_…`, Name
  1–64 Zeichen; Autor `@mcp:<tokenname>` (aus `clientInfo` zählt nur die Version; die Tabelle nennt „früher
  `@mcp:…`“, wenn die Namensregel früher einen anderen Autor ergab; abgelehnt werden Namen, die denselben Autor
  ergäben wie ein vorhandener oder widerrufener Token oder mit „lokal-“ beginnen). Befehl:
  `claude mcp add -s local --transport http baruch http://<IP>:8765/mcp --header "Authorization: Bearer <Token>"`.
  Voraussetzung: `--listen 192.168.…:8765`; Ansprache über die IP-Adresse (sonst 421). Von anderen Rechnern ist
  nur `/mcp` erreichbar (sonst 403 „Von anderen Rechnern nur /mcp mit Token.“); ein falscher Token wird auch lokal
  abgelehnt (401). Formulare an `POST /einstellungen/mcp/act` (Erzeugen 200, Widerrufen 303 `?ok=widerrufen`).
  Hinter einem Reverse-Proxy zeigt die Seite `https://<Name>/mcp` (`https` ist eine Annahme); Basic-Auth des Proxys
  muss `/mcp` ausnehmen (`docs/Betrieb.md`).
- **Daten:** EPUBCheck-Pfad absolut, Endung `.jar`, vorhandene Datei; `epubcheck` im PATH geht vor.

### Konten und Lektoren (`konten.md`)

- **Adressen:** `/einrichten` (nur am eigenen Rechner, sonst 403), `/anmelden`, `/konto`,
  `/werke/<id>/mitwirkende`, Einstellungen → Konten. Mit Konten verlangen Seiten, Formulare und `/rpc` eine
  Anmeldung; `/mcp` bleibt bei Tokens.
- **Rechte:** einzige Stelle der Prüfung ist `roles` (`LEKTOR_ROUTES`, `LEKTOR_RPC`); alles nicht Erlaubte
  antwortet 403 „Abgelehnt: …“. Herkunft `@lektorat:<name>`; jeder Vorschlag, Beitrag und Befund ist ein Commit.
- **Gespeichert:** Passwort als Argon2id-Prüfwert, Sitzung als Hash; nicht im Austauschformat und nicht im
  Spiegel. Das Sitzungs-Cookie schickt der Browser bei einem Sprung von außen nicht mit („Schon angemeldet?
  Weiter“). Anmelden und Einrichten folgen dem Thema-Cookie ohne JavaScript.
- **Kommandozeile:** `baruch-server passwort NAME [--data ORDNER]` liest das Passwort als eine Zeile von der
  Eingabe, beendet alle Sitzungen und hebt die Sperre nach Fehlversuchen auf.
- **„Am eigenen Rechner ohne Anmeldung“:** Steht ein Reverse-Proxy auf demselben Rechner, der `Host` umschreibt,
  sähe jeder Besucher aus wie der eigene Rechner (`docs/Betrieb.md`).
- Für Lektoren ist das Umschlagbild in der Werkliste kein Link; Einstellungen → Darstellung erreichen sie auch
  ohne freigegebenes Werk. Offen: ob Lektoren das Buch als PDF lesen dürfen (zurzeit nicht).

### Tasten und Maus (`tasten.md`)

- Die Mac-Spalte ist aus `crates/baruch-server/web/src` abgeleitet, nicht am Mac geprüft: `ctrlKey || metaKey` in
  `panes.ts` (Strg+S), `wysiwyg.ts` (Strg+I, Strg+B, Strg+Enter), `search_keys.ts`, `threads.ts`, `xref.ts`,
  `cover_editor.ts`, `mindmap.ts`, `zoom.ts` (nur Strg+0). Nur `ctrlKey`: das Mausrad in `zoom.ts`. Alt-Kürzel
  mit `event.code`: Alt+E (`ideas.ts`, `mindmap.ts`), Alt+L in der Mindmap, Alt+C/W/R/L der Suche. Nur über
  `event.key`: Alt+L für die Literatur (`literature.ts`) und Alt+, für die Einstellungen (`settings.ts`,
  `aria-keyshortcuts="Alt+Comma"` in `base.html`) – am Mac erzeugt ⌥ dort ein anderes Zeichen, die Kürzel greifen
  dann vermutlich nicht (Kandidat für eine Korrektur).
