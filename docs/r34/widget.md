# Runde 34 – mindmap_widget: Bedienung und DOM-Vertrag der Mindmap

Fragment für `docs/Bedienung.md` (Abschnitt 1–4) und `docs/Architektur.md` (Abschnitt 5–7). Stand: Zweig
`team/mindmap_widget`, gegen die Schnittstelle R34-S1 (flacher Baum aus `idea_map::tree::tree`).

## 1. Die Seite „Mindmap“ (Bedienung)

Die Mindmap eines Werks steht unter **Start → Einfälle → Mindmap** (Adresse `/werke/{id}/karte`, alte Lesezeichen
der Gedankenkarte gelten weiter). In der Mitte steht das Werk, rechts und links davon die Hauptäste: die Teile bzw.
Kapitel in der Reihenfolge der Gliederung, die erste Hälfte rechts, die zweite links. Jeder Hauptast hat eine eigene
Farbe, die alle Unteräste erben. Einfälle hängen an ihrem Kapitel oder als Unterknoten an einem anderen Einfall,
beliebig tief.

Zeichen am Knoten:

| Zeichen | Bedeutung |
| --- | --- |
| „−“ / „+ n“ an der Außenkante | Ast zuklappen bzw. aufklappen; `n` = Zahl der verborgenen Knoten |
| Blatt-Symbol | Der Einfall hat Notiz, Zitat, Quelle oder Bibelstelle (Tooltip zeigt die Notiz) |
| „§“ | Der Einfall hat einen Bezug; der Link führt zum Abschnitt im Werk |
| „k offen“ | Ebene „Befunde“: offene Befunde am Teil/Kapitel (ab 3 dick umrandet), Link zu den Befunden |
| durchgestrichener Titel | Einfall ist verarbeitet |
| blasser Knoten | passt nicht zur Suche bzw. steht nur wegen sichtbarer Unterknoten da |

**Beziehungen** zwischen Einfällen sind gebogene Linien mit Beschriftung (Art, dazu die Notiz): „führt weiter“
durchgezogen, „gehört zu“ gestrichelt, „Beispiel für“ gepunktet (alle mit Pfeil), „widerspricht“ rot, fett,
Strich-Punkt und ohne Pfeil; Widersprüche stehen zusätzlich in der Liste „Widersprüche“ unter dem Bild. Ist ein
Ende eingeklappt, endet die Linie am sichtbaren Vorfahren. **Nähe** (zuschaltbar im Menüband) zeigt fein gepunktete
Linien zwischen ähnlichen Einfällen; sie ist nur Anzeige und wird nie gespeichert. Der Abschnitt „Gruppen“ (Namen
vom Assistenten als Vorschlag) steht wie bisher unter dem Bild.

Gespeicherte Lagen der alten Gedankenkarte bleiben in der Datenbank, haben in der Mindmap aber keine Wirkung: Der
Baum ordnet sich selbst.

## 2. Die Wolke (Ablage neben der Mindmap)

Unter dem Baum liegt die **Wolke**: die Ablage aller Einfälle ohne Platz – also ohne Elternknoten und ohne Bezug
(früher „Ohne Kapitel“ und freie Einfälle). Der Zähler „n in der Wolke“ steht in der Wolke und in der Leiste über
dem Bild.

- **Aus der Wolke in die Mindmap:** Kärtchen auf einen Knoten ziehen – auf einen Einfall wird es dessen
  Unterknoten, auf ein Kapitel oder einen Teil hängt es dort.
- **Aus der Mindmap in die Wolke:** Einfall in die Wolke (oder auf das Werk) ziehen oder im Menüband „In die Wolke“.
  **Entscheidung:** Der Einfall wandert **mit seinen Unterknoten**; sie bleiben an ihm hängen und werden in der
  Wolke nicht einzeln gezeichnet (am Kärtchen steht „+ n“). Grund: Es ist dieselbe Regel wie beim Umhängen im Baum
  (ein Ast bleibt ein Ast), nichts geht still verloren, und „Rückgängig“ stellt mit einem Schritt alles wieder her.
  Wer nur den einen Einfall lösen will, zieht vorher dessen Unterknoten an einen anderen Knoten.
- **Neue Einfälle** ohne Platz landen in der Wolke: Alt+E auf der Mindmap, Tab/Enter am Werk, die Schnellerfassung
  ohne Bezug und MCP `idea_create` ohne `parent` und `key`.
- **Nähe als Hilfe:** Ist die Nähe zugeschaltet und ein Kärtchen einem Einfall im Baum nah, steht darunter der
  Vorschlag „passt zu Ast ‚…‘“ in der Farbe dieses Hauptasts (bestes Paar der Nähe). Es ist nur ein Hinweis;
  einhängen muss der Mensch.

Annahme gegenüber mindmap_daten: „in der Wolke“ heißt im Baum `MindMap.root.children` der Art `Idea` (Einfälle
direkt am Werk). Das „freie Hauptthema“ aus dem Vertrag §0 gibt es nach dem Nachtrag vom 2026-11-25 nicht mehr als
Hauptast; solche Einfälle liegen in der Wolke.

## 3. Maus und Tasten (mit JavaScript)

| Eingabe | Wirkung |
| --- | --- |
| Klick | wählt den Knoten (Strg/Umschalt+Klick auf den Titel öffnet den Einfall bzw. Abschnitt als Link) |
| Doppelklick, F2 | benennt den gewählten Einfall um (Enter übernimmt, Esc verwirft); Werk, Teile, Kapitel nicht |
| Tab | neuer Unterknoten des gewählten Knotens, sofort im Umbenennen (an Kapitel/Teil: Einfall mit diesem Bezug; am Werk: Einfall in der Wolke) |
| Enter | neues Geschwister hinter dem gewählten Einfall (vor dem nächsten Geschwister); an Kapitel, Teil und Werk wie Tab |
| Entf, Rücktaste | löscht den gewählten Einfall samt Unterknoten nach Rückfrage (wiederherstellbar); an einer gewählten Beziehung: löst sie |
| Pfeil links/rechts | zum Eltern- bzw. ersten Kindknoten, je nach Seite des Asts; am Werk zum ersten Hauptast der Seite |
| Pfeil hoch/runter | zum vorigen/nächsten Geschwister; vom Werk mit „runter“ in die Wolke, in der Wolke von Kärtchen zu Kärtchen |
| Leertaste | klappt den gewählten Ast zu bzw. auf |
| Strg+Mausrad, `+`, `-`, `0` | vergrößern, verkleinern, einpassen (auch Knöpfe „−“, „+“, „Einpassen“ über dem Bild) |
| Fläche ziehen | verschiebt den Ausschnitt |
| Einfall ziehen | auf einen Einfall: wird Unterknoten; auf dessen oberen/unteren Rand: davor/dahinter einordnen; auf Kapitel/Teil: hängt dort; aufs Werk oder in die Wolke: in die Wolke |
| Strg+Z | nimmt die letzte Aktion auf der Mindmap zurück (eine Stufe; auch Knopf „Rückgängig“ in der Meldung) |
| Alt+L, „Verbinden“ | Beziehung vom gewählten Einfall zum nächsten gewählten (Klick oder Pfeile und Enter); Dialog mit Art und Notiz; Esc bricht ab |
| Alt+E | neuer Einfall in der Wolke, sofort im Umbenennen |
| Esc | bricht „Verbinden“ ab bzw. hebt die Auswahl auf (danach wandert Tab wieder normal durch die Seite) |

Umschalt+Tab und Tab ohne gewählten Knoten bleiben beim Browser (kein Tastaturkäfig). Eingeklappte Äste merkt sich
der Browser je Werk (`localStorage`); gespeichert wird davon nichts. Der Ausschnitt (Zoom, Lage) gilt bis zum
Schließen des Tabs. Jede Aktion ist ein eigener Commit im Spiegel mit Herkunft Mensch, auch jede Umkehr.

## 4. Ohne JavaScript

Die Seite ist dann ein Bild mit Links: „−“/„+ n“ klappen über die Adresse (`zu=<Kennungen>`), „§“ führt zum
Abschnitt, der Titel eines Einfalls zu seiner Seite, der eines Kapitels ins Werk. „Alles aufklappen/zuklappen“,
„Nähe“ und „Ebene Befunde“ im Menüband sind Links, der Filter ein GET-Formular. Die Knöpfe der Gruppe „Knoten“
(Unterknoten, Geschwister, Umbenennen, Löschen, Verbinden, In die Wolke, Rückgängig) führen ohne JavaScript in die
**Gliederungsansicht** (`/werke/{id}/karte/gliederung`); dort stehen die Formulare, auch „in die Mindmap unter …“
und „in die Wolke“. Das Bild rollt in seinem Rahmen in voller Größe.

Menüband der Seite (Reiter Start, Gruppen als `details.band-group[data-band-group]` wie in Runde 33, also auch im
Laptop-Modus): **Knoten**, **Ansicht** (Alles auf-/zuklappen, Nähe, Ebene Befunde), **Ausgabe** (XMind, FreeMind,
SVG, PDF, Markdown – mit dem Filter der Seite), **Einlesen**, **Gliederungsansicht**, **Zurück**.

## 5. Routen und Abfrage (Architektur)

| Route | Inhalt |
| --- | --- |
| `GET /werke/{id}/karte` | Seite „Mindmap“ (`idea_map/bild/mod.rs`, Vorlage `karte.html`) |
| `GET /werke/{id}/karte/bild` | nur das SVG (`image/svg+xml`, `Cache-Control: no-store`), gleiche Abfrage; das Fragment bekommt hier `xmlns` |

Abfrage: Filter wie bisher (`status`, `word`, `origin`, `q`, `near`, `near_limit`, `near_threshold`, `befunde`),
dazu `zu=<Kennungen, Komma>` (eingeklappte Knoten) und `wahl=<Kennung>` (gewählter Knoten, im Bild
`aria-current="true"`). Kennungen sind `w`, `n:<key>`, `i:<id>`; alles andere fällt weg (`bild::node_id`,
höchstens 2000). Beide stehen in `bild::Extra` (`closed`, `wahl`), das `parse_filter` füllt; `filter_query` trägt
sie nicht (Ausgaben klappen nie ein).

Die Seite baut zwei Sichten: den Baum (`tree::tree`) für das Bild und – für den Abschnitt „Gruppen“ – die flache
Karte (`idea_map::build`). Das ist ein zweiter Durchlauf mit demselben Filter; fällt er aus, fehlen nur die Gruppen.

## 6. DOM-Vertrag des Bildes (`idea_map/bild/mindmap.rs`, Anordnung `bild/tree_layout.rs`)

- Bühne: `<div class="mm-stage" data-widget="mindmap" data-work data-bild data-back data-zu data-wahl
  data-gliederung>`, darin `[data-mm-toolbar]`, `[data-mm-canvas]` mit `<svg class="mm-svg" data-cloud-count
  data-zu>` und `<g class="mm-viewport">`, darunter `[data-mm-status]` (Live-Region). Zähler:
  `[data-mm-cloud-count]`.
- Reihenfolge im Viewport: Äste, Nähe, Beziehungen, Wolke, Knoten (Knoten liegen oben).
- Knoten: `<g class="mm-node mm-<work|part|chapter|idea>" data-node data-parent data-depth data-side="right|left"
  data-branch="0..7" data-editable="1|0" data-title data-own tabindex="0">`; Einfälle `data-idea`, mit Bezug
  `data-key`; mit Nachfahren `data-children="n"`; mit offenen Befunden `data-open-findings="k"`. Zusatzklassen: `mm-b<n>` (Astfarbe), `mm-main` (Hauptast),
  `mm-done`, `mm-hit`, `mm-dim`, `mm-ghost`, `mm-closed`, `mm-many-findings`. Das Werk hat weder `data-side` noch
  `data-branch`. Kinder: `<title>`, `rect.mm-box`, `text.mm-label` (in `a.mm-label-link`), `g.mm-note`,
  `a.mm-section`, `a.mm-findings[data-open-findings]`, `a.mm-toggle[data-count]`.
- Äste: `<path class="mm-edge mm-b<n>" data-from data-to>` (kubische Kurve, `stroke-width` 3.6 / 2.4 / 1.8 / 1.4).
- Beziehungen: `<g class="mm-rel">` mit `<path class="mm-link mm-link-<kind>" data-link data-from="i:<a>"
  data-to="i:<b>" data-end-from data-end-to>` und `<text class="mm-link-label">`; Widerspruch zusätzlich
  `mm-contradiction` an Gruppe, Pfad und Text. `data-end-*` nennt den Knoten, an dem die Linie sichtbar endet.
- Nähe: `<path class="mm-near" data-from data-to>` (nur mit `near`, nur zwischen Knoten des Baums).
- Wolke: `<g class="mm-cloud" data-count>` mit `rect.mm-cloud-area` und `text.mm-cloud-head`; Kärtchen sind
  `g.mm-node.mm-idea.mm-cloud-item[data-cloud="1"][data-parent="w"]`, mit Vorschlag `data-fits="<Hauptast>"` und
  `text.mm-cloud-hint.mm-b<n>`, mit Unterknoten `text.mm-cloud-below` („+ n“).
- Eingeklappte Knoten zeichnen keine Nachfahren; `a.mm-toggle` zeigt „+ n“, sein `href` ist `?<Filter>&zu=<neue
  Liste>`. Die Zahl zählt alle verborgenen Knoten (jede Art), nicht nur Einfälle.
- **Fragment** (`standalone: false`): kein `xmlns`, kein `<style>`, keine Farbattribute – Farben über
  `--mm-b0`…`--mm-b7` in `karte.css` (hell, dunkel nach System, dunkel nach Schalter). **Eigenständig**
  (`standalone: true`, für `.svg`/`.pdf`): `xmlns`, eigener `<style>`, Farben als Attribute
  (`mindmap::BRANCH_COLORS`), weißer Grund, keine Links (statt `<a>` ein `<g>` derselben Klasse).
- Die Anordnung ist deterministisch; Textbreiten sind geschätzt (der Server kennt die Schrift des Browsers nicht),
  Titel über 48 Zeichen werden im Bild gekürzt (voller Titel in `data-title` und im Tooltip).

## 7. Widget `web/src/mindmap.ts`

Ersetzt `idea_map.ts` (ausgeräumt und nicht mehr in `main.ts` eingebunden; die Datei steht nur noch als leere Hülle
`export {};` da, siehe „Für den Integrator“). Es ordnet nichts selbst an: Nach jeder Aktion holt es `data-bild` mit Filter und
`zu=` neu, setzt das SVG ein und behält Auswahl und Ausschnitt (`transform` auf `.mm-viewport`). RPC (nur diese,
über `rpc.ts`): `idea.create`, `idea.update`, `idea.move`, `idea.delete`, `idea.restore`, `idea.link`,
`idea.unlink`, `idea.link_restore`. Rückgängig ist die Umkehr-RPC der letzten Aktion (anlegen ↔ `idea.delete`,
löschen ↔ `idea.restore`, umbenennen ↔ `idea.update` mit dem alten gespeicherten Titel aus `data-own`, umhängen ↔
`idea.move` zurück mit altem `parent`, `key`, `before`, verbinden ↔ `idea.unlink`, lösen ↔ `idea.link_restore`).
Knöpfe im Menüband tragen `data-mm-action` (`child`, `sibling`, `rename`, `delete`, `link`, `cloud`, `undo`), Links
mit `data-mm-zu` setzen die eingeklappten Knoten aus ihrem `href`.

Stand gegen S1: `idea.move`, `title`/`parent`/`before` an `idea.create` und `title` an `idea.update` liefert
mindmap_daten. Bis dahin meldet das Widget die Ablehnung des Servers als Fehlertext in der Statuszeile; die
Zeichnung von Unterknoten ist am von Hand gebauten Baum geprüft (`tests/round34_widget.rs`).

## Für den Integrator

- `docs/Bedienung.md`, `docs/Architektur.md`: Abschnitte 1–7 einsetzen; überall „Mindmap“ statt „Gedankenkarte“.
- **`web/src/idea_map.ts` löschen:** `tests/round18_menueband.rs` (Zeile 563, Test
  `the_map_side_panel_starts_below_the_toolbar`) liest die Datei per `include_str!` ein; ohne sie übersetzt die
  ganze Testdatei nicht. Darum steht sie noch als leere Hülle da. Den Test streichen (die Seitenleiste gibt es
  nicht mehr), dann die Datei löschen und in `tests/round34_widget.rs` die drei Zeilen zur Hülle durch „404 für
  `/static/js/idea_map.js`“ ersetzen.
- Fremde Tests, die am alten Bild hängen (`.im-stage`, `im-edge`, `im-many-findings`, Seitenleiste) und jetzt
  ausfallen, stehen im Bericht des Entwicklers mit Ersatztest in `round34_widget.rs`.
- `bild/layout.rs`, `bild/svg.rs` (eingefroren) benutzt die Seite nicht mehr; `bild::load` (für `files/`) noch.
  Sobald `files/` auf `mindmap::render` umgestellt ist, können `load`, `layout.rs`, `svg.rs` und die alten
  `.im-*`-Regeln für das Bild in `karte.css` weg (die Regeln für Filter, Gruppen, Widersprüche bleiben).
- `files/mod.rs` leitet nach einem Formular auf `/karte?…&ok=<wort>` weiter; neue Wörter mit Text in
  `bild::notice_of`: `angelegt`, `umgehaengt`, `geloescht`, `eingelesen` (sonst „Gespeichert.“).
- Abnahme Ende-zu-Ende (Unterknoten über `idea.move`, Wolke ↔ Baum) erst nach dem Zusammenführen mit mindmap_daten
  möglich; Handprüfung im Browser: Ziehen, Tasten, Zoom, Dunkel.
