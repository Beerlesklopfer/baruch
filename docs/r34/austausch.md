# Runde 34 – mindmap_austausch: Gliederungsansicht, Ein- und Ausgabe der Mindmap

Doku-Fragment des Entwicklers `mindmap_austausch` (Vertrag Runde 34, §5 und §8, samt Nachtrag des Nutzers vom
2026-11-25 „Verbindung zwischen Wolke und Mindmap“). Der Integrator setzt die Abschnitte in `docs/Bedienung.md`,
`docs/Architektur.md` und `docs/RPC.md` ein.

## Für `docs/Bedienung.md`

### Wolke und Mindmap

Die Mindmap eines Werks hat zwei Bereiche auf einer Seite: die **Äste** (Teile, Kapitel und die Einfälle, die
dort hängen) und daneben die **Wolke**. In der Wolke liegen die Einfälle ohne Platz – Einfälle, die weder unter
einem anderen Einfall hängen noch einen Bezug zu einem Abschnitt haben. Neue Einfälle ohne Platz (Alt+E, MCP
`idea_create` ohne `parent` und `key`) landen dort; die Überschrift der Wolke zählt sie („n in der Wolke“).

- **In die Mindmap:** Einen Einfall der Wolke unter einen Knoten hängen – unter einen anderen Einfall oder an ein
  Kapitel. Wo die Nähe (gemeinsame Schlagwörter, Bedeutung) einen Einfall kennt, der schon in der Mindmap hängt,
  steht am Wolken-Einfall der Vorschlag „passt zu Ast …“ mit dem Knopf „Dort einhängen“.
- **In die Wolke:** Ein Knoten der Mindmap wandert zurück in die Wolke. Entscheidung: Er wandert **mit seinen
  Unterknoten** – sie bleiben seine Unterknoten und liegen mit ihm in der Wolke. Wer nur den einen Knoten lösen
  will, hängt vorher seine Unterknoten um. Elternknoten **und** Bezug werden dabei gelöst (ein Einfall mit Bezug
  hinge sonst gleich wieder an seinem Kapitel).

### Gliederungsansicht (ohne JavaScript)

`/werke/{id}/karte/gliederung` – im Menüband der Mindmap unter „Gliederungsansicht“. Die Seite zeigt dieselbe
Mindmap als verschachtelte Liste und bedient sie ganz ohne JavaScript:

- Links die Äste; jeder Knoten mit Unterknoten lässt sich auf- und zuklappen. Rechts die Wolke mit dem Zähler,
  dem Formular „Neuer Einfall … In die Wolke legen“ und den Vorschlägen „passt zu Ast …“.
- „Bearbeiten“ (bei Kapiteln „Wählen“) wählt einen Knoten; oben erscheinen seine Formulare:
  **Umbenennen**, **Unterknoten anlegen**, **Geschwister anlegen** (direkt hinter dem Knoten), **Umhängen unter …**
  (bei Wolken-Einfällen „In die Mindmap unter …“), **In die Wolke**, **Reihenfolge** (nach oben, nach unten),
  **Bezug setzen** (Link zu einem Kapitel; ein genauerer Bezug auf einen Unterabschnitt lässt sich auf der Seite
  des Einfalls setzen), **Verbinden** (Art und Beschriftung) samt **Lösen**, **Löschen**.
- Löschen entfernt den Einfall samt Unterknoten; die Meldung bietet „Rückgängig“ an, und unten steht die Liste
  „Gelöschte Einfälle“ mit „Wiederherstellen“.
- Teile und Kapitel gehören zur Gliederung des Werks: An sie lässt sich ein Einfall hängen, umbenannt, verschoben
  und gelöscht werden sie nur in der Gliederung (Antragsregel).
- Unter dem Baum: „Beziehungen“ (mit „Lösen“) und „Widersprüche“.
- Der Filter (Status, Schlagwort, Herkunft, Suche) gilt wie auf der Mindmap und bleibt nach jedem Formular
  erhalten.

### Ausgabe

Im Menüband (Gruppe „Ausgabe“), jeweils mit dem Filter der Seite:

| Datei | Inhalt |
|---|---|
| `.xmind` | XMind (ZIP mit `content.json`, `metadata.json`, `manifest.json`) |
| `.mm` | FreeMind/Freeplane |
| `.svg` | das Bild der Mindmap als eigenständige Datei |
| `.pdf` | dasselbe Bild auf A4 quer, mit Titel und Filter |
| `.md` | Markdown: der Baum als verschachtelte Liste (Notizen eingerückt), danach „Wolke“, „Beziehungen“, „Widersprüche“ |

Die Einfälle der Wolke stehen in `.xmind` und `.mm` als eigener, letzter Ast **„Wolke“**, in `.md` als eigener
Abschnitt. In `.svg`/`.pdf` gilt, was die Mindmap zeichnet; eingeklappte Knoten (`zu=`) bleiben dort eingeklappt,
in `.mm` und `.xmind` ist nie etwas eingeklappt.

**Was wird übertragen (`.xmind`, `.mm`)**

| Angabe | übertragen | wie |
|---|---|---|
| Titel | ja | Kapitel mit Nummer („1 Helling“) |
| Notiz | ja | `.xmind`: `notes.plain.content`; `.mm`: `richcontent TYPE="NOTE"`, je Zeile ein Absatz |
| Schlagwörter | ja | `.xmind`: `labels`; `.mm`: Attribut „Schlagwort“ je Wort |
| Beziehungen | ja | `.xmind`: `relationships` (`end1Id`, `end2Id`, `title`); `.mm`: `arrowlink` mit `MIDDLE_LABEL` (Freeplane; FreeMind zeigt die Beschriftung nicht). Beschriftung = Art und Notiz („führt weiter: …“) |
| Reihenfolge | ja | Reihenfolge der Knoten in der Datei |
| Wolke | ja | als Ast „Wolke“ |
| Status (offen/verarbeitet) | nein | nur in `.md` als Text |
| Herkunft | nein | nur in `.md` als Text |
| Bezug (Abschnitt) | nein | nur in `.md` als Text; in den Dateien hängt der Einfall an seinem Kapitel |
| Zitat, Quelle, Bibelstelle | nein | – |
| Art der Beziehung | nur als Text | steckt in der Beschriftung; `.mm` färbt zusätzlich je Art |
| Lagen | nein | bleiben in Baruch gespeichert |
| Bilder, Stile, Marker, Eingeklapptes | nein | – |

### Einlesen

`/werke/{id}/karte/einlesen` – im Menüband unter „Einlesen“. Baruch liest eine Mindmap aus XMind (`.xmind`),
FreeMind oder Freeplane (`.mm`):

1. Datei wählen, wahlweise „unter Knoten …“ (ein Einfall des Werks). Ohne Zielknoten kommen die Knoten **in die
   Wolke**.
2. Die **Vorschau** zeigt den gelesenen Baum, die Zahl der Knoten und Hinweise (Gekürztes, Übergangenes). Es ist
   noch nichts angelegt.
3. „Anlegen“ legt die Knoten als Einfälle an (ein Commit im Spiegel, Herkunft Mensch) und führt zur Mindmap.

Übernommen werden Titel, Notizen (HTML wird reiner Text), Schlagwörter (XMind: Etiketten; `.mm`: Attribut
„Schlagwort“), Reihenfolge und Beziehungen (als „führt weiter“ mit der Beschriftung als Notiz). Das **Hauptthema**
der Datei wird nicht selbst zum Einfall: Seine Kinder werden Einfälle der Wolke bzw. Unterknoten des gewählten
Knotens. Eine von Baruch selbst ausgegebene Datei lässt sich wieder einlesen; der Ast „Wolke“ wird dabei aufgelöst.
Kapitel aus einer eigenen Ausgabe kommen dabei als gewöhnliche Einfälle zurück – das Einlesen legt nie
Gliederungspunkte an.

Grenzen (fremde Dateien sind fremde Eingabe): Datei höchstens 10 MiB; höchstens 2000 Knoten und 32 Ebenen; Titel
werden auf 200, Notizen auf 4000 Zeichen gekürzt (mit Hinweis in der Vorschau); höchstens 20 Schlagwörter je Knoten.
Ein `.xmind`-Archiv darf höchstens 50 Einträge haben und entpackt höchstens 20 MiB groß sein. Dateien im alten
**XMind-8-Format** (nur `content.xml`) werden abgelehnt: bitte in einem aktuellen XMind öffnen und neu speichern
oder als `.mm` ausgeben. `.mm`-Dateien müssen UTF-8 oder ASCII sein (FreeMind und Freeplane schreiben so).

## Annahmen zu XMind und Freeplane (Handprüfung Nutzer)

Der Aufbau der `.xmind`-Datei folgt der offenen Beschreibung des Formats **aus dem Gedächtnis**. Geprüft ist nur
die Rundreise mit dem eigenen Leser (Tests), **nicht** mit einem echten XMind oder Freeplane. Bitte von Hand
prüfen (Werk 3: `.xmind` in XMind, `.mm` in Freeplane öffnen; eine dort gespeicherte Datei wieder einlesen):

| Annahme | Risiko, falls sie nicht stimmt |
|---|---|
| XMind (2020 und neuer) öffnet ein ZIP mit `content.json`, `metadata.json`, `manifest.json` ohne `content.xml` und ohne Vorschaubild | Datei öffnet nicht; dann fehlt ein Pflichteintrag |
| `content.json` ist ein Feld von Blättern; ein Blatt hat `id`, `class: "sheet"`, `title`, `rootTopic`, `relationships` | Blatt wird nicht erkannt |
| Themen: `id`, `class: "topic"`, `title`, `children.attached`, `notes.plain.content`, `labels` | Notizen oder Etiketten fehlen |
| `structureClass: "org.xmind.ui.map.unbalanced"` am Hauptthema verteilt die Äste rechts und links | andere Anordnung (nur Darstellung) |
| Themen-ids dürfen beliebige Texte sein (`i:12`, `n:7`, `baruch-wolke`); XMind verlangt keine UUID | Beziehungen oder Themen fehlen |
| `relationships`: `id`, `end1Id`, `end2Id`, `title` genügen (keine Kontrollpunkte, kein Stil) | Beziehungslinien fehlen |
| `manifest.json` mit `file-entries` für `content.json` und `metadata.json` | Warnung beim Öffnen |
| Beim Einlesen: frei schwebende Themen stehen unter `children.detached`; HTML-Notizen unter `notes.realHTML.content` | diese Inhalte fehlen nach dem Einlesen (Hinweis in der Vorschau nur für die freien Themen) |
| Freeplane liest `MIDDLE_LABEL` an `arrowlink` und zeigt Attribute („Schlagwort“) an; FreeMind 1.0.1 übergeht `MIDDLE_LABEL` | Beschriftung der Beziehung fehlt in der Anzeige |
| Freeplane schreibt Beschriftungen als `MIDDLE_LABEL` (sonst `SOURCE_LABEL`/`TARGET_LABEL`) und Notizen als `richcontent TYPE="NOTE"` | Beschriftung oder Notiz fehlt nach dem Einlesen |

Nicht gelesen werden: weitere Blätter einer XMind-Datei (Hinweis), Zusammenfassungen und Sprechblasen, Bilder,
Marker, Stile, andere Attribute als „Schlagwort“, Freeplane-Details (`richcontent TYPE="DETAILS"`).

## Für `docs/Architektur.md`

`crates/baruch-server/src/idea_map/files/` (Ein- und Ausgabe der Mindmap, Bedienung ohne JavaScript):

- `mod.rs` – Routen (`pages()`, `writes()`), die Ausgaben, `POST /werke/{id}/karte/act`.
- `export.rs` – die Regel der **Wolke** (`cloud`, `placed`: Kinder der Wurzel von der Art Einfall), das Blatt
  (`Sheet`) als Vorstufe von `.mm`/`.xmind`, Markdown.
- `freemind.rs`, `xmind.rs` – je Schreiber und Leser. `import.rs` – Grenzen, Reinigung, `Raw` → `ImportTree`
  (`finish`, `check`). `upload.rs` – Formular, Vorschau, Bestätigung. `outline.rs` – Gliederungsansicht.
- Vorlagen `karte_gliederung.html`, `karte_einlesen.html`; Stil `static/karte_gliederung.css` (nur Variablen aus
  `app.css`).

Sicherheit beim Einlesen: Upload höchstens 10 MiB (`DefaultBodyLimit` an der Route). Aus dem ZIP wird allein der
Eintrag `content.json` gelesen (Namen als Bytes verglichen, nie als Pfad benutzt, nichts auf Platte entpackt); beim
Entpacken wird mitgezählt und bei 20 MiB abgebrochen; Archive mit mehr als 50 Einträgen oder mit Namen, die aus dem
Archiv herausführten (`..`, absoluter Pfad, Laufwerk, `\`), werden ganz abgelehnt. XML: eine DOCTYPE wird
abgelehnt, Entitäten werden nicht aufgelöst (nur die fünf von XML, `nbsp`, Zeichenreferenzen); der Leser arbeitet
mit eigenem Stapel und begrenzt Knotenzahl und Tiefe beim Lesen. Zwischen Vorschau und Bestätigung steht der Baum
als JSON in einem verborgenen Formularfeld (keine Datei im Datenordner) und wird bei der Bestätigung erneut
geprüft (`import::check`). Beide POST-Routen liegen hinter `guard::same_origin`.

## Für `docs/RPC.md` (Routen, kein RPC)

| Route | Zweck |
|---|---|
| `GET /werke/{id}/karte.svg` \| `.pdf` \| `.md` \| `.mm` \| `.xmind` | Ausgaben; Abfrage wie die Seite (`status`, `word`, `origin`, `q`, `near`, `befunde`), `.svg`/`.pdf` zusätzlich `zu=<Kennungen>` |
| `GET /werke/{id}/karte/gliederung` | Gliederungsansicht; dazu `wahl=<Kennung>` (gewählter Knoten), `zu=<Kennungen>`, nach Formularen `ok`, `undo`, `wieder` |
| `POST /werke/{id}/karte/act` | Formulare (urlencoded), siehe unten |
| `GET /werke/{id}/karte/einlesen` | Formular; `unter=i:<id>` wählt den Zielknoten vor |
| `POST /werke/{id}/karte/einlesen` | multipart `file`, wahlweise `parent`; zeigt die Vorschau, legt nichts an (413 über 10 MiB, 422 nicht lesbar, 409 Zielknoten aus anderem Werk) |
| `POST /werke/{id}/karte/einlesen/anlegen` | urlencoded `tree` (JSON aus der Vorschau), `parent`; legt über `tree::import` an, 303 auf `/werke/{id}/karte?ok=eingelesen` |

`POST /werke/{id}/karte/act`: Feld `method` ∈ `idea.create`, `idea.update`, `idea.move`, `idea.delete`,
`idea.restore`, `idea.link`, `idea.unlink`, `idea.link_restore`, `idea.map_pos_reset`, `idea.map_groups_name`,
`idea.map_group_accept`, `idea.map_group_dismiss`. Dazu die Felder der Aktion:

- `idea.create`: `title`, `words`, `note`, `parent`, `key`, `before` (ohne `parent` und `key` → Wolke).
- `idea.update`: `id`, wahlweise `title`, `key`, `words`, `note` – nur Felder, die das Formular schickt, ändern sich.
- `idea.move`: `id`, entweder `ziel` ∈ `wolke` \| `i:<Einfall>` \| `n:<Kapitel-key>` oder die Felder der RPC
  `parent`, `key`, `before` (vorhanden und leer = `null`).
- `idea.delete`, `idea.restore`: `id`. `idea.link`: `from`, `to`, `kind`, `note`. `idea.unlink`,
  `idea.link_restore`: `id` (Verbindung).
- `back` (Abfrageteil des Filters), `seite` ∈ `karte` \| `gliederung` (Ziel der Weiterleitung, Vorgabe `karte`),
  `wahl` (gewählter Knoten, nur mit `seite=gliederung`).

Antwort: 303 auf `/werke/{id}/karte[/gliederung]?<back>&ok=<wort>` (`angelegt`, `umbenannt`, `bezug`, `umgehaengt`,
`in-wolke`, `ort-unveraendert`, `geloescht` mit `wieder=<id>`, `einfall-wieder`, `verbunden`, `unveraendert`,
`geloest` mit `undo=<id>`, `wiederhergestellt`, …). Fehler als Seite mit Meldung: 400 unbekannte Aktion, 409 abgelehnt
(auch: ein genannter Einfall gehört nicht zum Werk der Adresse – bisher 422), 422 ungültige Eingabe, 404 unbekanntes
Werk, 403 fremde Herkunft.
