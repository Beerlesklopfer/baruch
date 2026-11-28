<!-- translation-of: docs/Austauschformat.md source-commit: 831dd0e -->

# Austauschformat {#exchange-format}

> Dies ist die deutsche Übersetzung von `docs/Austauschformat.md` (englisches Original, maßgeblich), Stand: Commit `831dd0e`.

Ein Werk wandert mit allen seinen Fassungen, Einfällen, Beiträgen, Bildern, seiner Einfallskarte, dem Umschlag-Layout, den
Satzbild-Einstellungen, seinen Befunden, den Satzeinstellungen und seinem Glossar von einer Baruch-Instanz zu einer anderen
(Runde 26, Karte seit Runde 25, Umschlag-Layout, Anschriften und Satzbild seit Runde 27, Befunde und alte
Beitragsfassungen seit Runde 28, Querverweise und Varianten seit Runde 30, Satzeinstellungen seit Runde 29, Glossar und
eigene Register seit Runde 31, Fortschreibungen, Entwurf und Stand übernommener Befunde seit Runde 22, Baum der Mindmap
an den Einfällen seit Runde 34). Namensraum
`urn:baruch:werk:1`, Unterversion 9; Schema `docs/schema/baruch-werk-1-u9.xsd`. Die Schemata der Unterversionen 1 bis
8 (`docs/schema/baruch-werk-1.xsd`, `baruch-werk-1-u2.xsd` … `baruch-werk-1-u8.xsd`) bleiben eingefroren (Byte-identisch,
Tests in `crates/baruch-exchange/tests/xsd.rs`); jede gültige Datei der Unterversionen 1 bis 8 ist auch nach dem
Schema u9 gültig.

> **Runde 38:** Die aktuelle Unterversion ist **10** (Schema `docs/schema/baruch-werk-1-u10.xsd`, auch das Schema u9 ist
> eingefroren); siehe „Werksprache (Unterversion 10, Runde 38)“ am Ende. Ab Runde 38 wurden neue Abschnitte dieses
> Dokuments auf Englisch geschrieben; seit Runde 38c ist das ganze Dokument englisch.

## 1 Übersicht, Container und was nicht im Format steht {#1-overview-container-and-what-is-not-in-the-format}

Eine Austauschdatei trägt **ein ganzes Werk** von einer Baruch-Instanz zu einer anderen (Laptop ↔ Rechner zu
Hause, Sicherung für später, Weitergabe an jemanden mit eigenem Baruch). Jedes Stück darin hat eine weltweit
eindeutige Kennung (UUID, §3); so erkennt die empfangende Instanz beim Einlesen, was sie schon kennt (§4).

Ausgeben: Seite „Buch → Ausgabe → Austausch: Ausgeben“ (Menüband) (`/werke/{id}/austausch`), RPC `work.exchange_export`,
Kommandozeile `baruch-server export ID|SLUG --baruch DATEI`, MCP `exchange_export` (nur lesend). Einlesen nur
durch den Menschen (Seite, RPC, Kommandozeile), nie über MCP.

**Zwei Formen** mit demselben XML:

| Form | Datei | Medientyp | Bilder |
|---|---|---|---|
| Container (Voreinstellung) | `<slug>.baruch` | `application/vnd.baruch.werk+zip` | eigene Dateien im ZIP |
| nur XML | `<slug>.baruch.xml` | `application/vnd.baruch.werk+xml` | Base64 in `<daten>` |

**Der Container `.baruch`** ist ein ZIP wie bei EPUB und ODF; die Einträge stehen in fester Reihenfolge:

1. `mimetype` – unkomprimiert (STORED), Inhalt genau `application/vnd.baruch.werk+zip` ohne Zeilenende, ohne
   Extrafeld im Kopf. Der Typ ist so in der Datei ab Byte 38 lesbar, ohne auszupacken.
2. `werk.xml` – das Werk (DEFLATE); jede Bildfassung verweist mit `datei="bilder/…"` auf ihren Eintrag.
3. `bilder/<fassungs-uuid>.<ext>` – eine Datei je Bildfassung, `ext` ∈ `png|jpg|gif|webp|svg`; Rasterbilder
   STORED (schon komprimiert), SVG DEFLATE. Die Bytes sind genau die Bytes des Bildes.
4. `MANIFEST.xml` – zuletzt (DEFLATE): jeder Eintrag außer `mimetype` und dem Manifest selbst mit SHA-256
   (Kleinbuchstaben-Hex) und Bytezahl.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<manifest xmlns="urn:baruch:werk:1" version="3">
  <datei pfad="werk.xml" sha256="3f0c…e1" bytes="18234"/>
  <datei pfad="bilder/26000000-0000-4000-8000-00000000003d.png" sha256="0b2e…82" bytes="70"/>
</manifest>
```

Die Ausgabe ist **deterministisch**: dasselbe Werk im selben Zustand ergibt Byte-identisches XML und ZIP (jeder
ZIP-Eintrag trägt den festen Zeitstempel 1980-01-01 00:00:00 und die Rechte 0644; nur `<ausgabe zeit>`
ändert sich von Export zu Export).

**Nicht im Format** (absichtlich):

- Schlüssel, Tokens und Geheimnisse; Verbindungen zu Diensten (Eliezer, MultiGPT, pgvector, LanceDB) – nicht zu
  verwechseln mit den Verbindungen der Einfallskarte zwischen Einfällen, die im Format stehen (`<karte>`).
- Die **Nähe** der Einfallskarte (aus Einbettungen und Stichwörtern berechnet, nie gespeichert).
- Personas mit ihren Modellen und Rollenanweisungen – nur ihre Kürzel (`@lektor`) als Herkunft. Das Modell einer
  **Fassung** (`modell`) steht darin als Herkunft der Fassung, nicht die Einstellung der Persona.
- Offene Anträge (Gliederung und Bilder), Belege der Websuche (`web_evidence`), RAG-Zuordnungen und der
  Suchindex.
- Bibel-, Kommentar- und Literaturtexte: nur Modulnamen (`<bibelmodul>`, `<quellen>`) und Verweise im Text.
- Numerische Ids, `slug` und die berechnete Nummer eines Punkts (`node.number`); die Quelle eines Typst-Imports.
- Seit Runde 30 (Migration 0047) bleiben lokal: das Protokoll des werkweiten Ersetzens (`replace_batch`, keine uuid,
  wie die Abgleichstände) und der Auftrag eines Beitrags (`post.request`/`chat_thread_post.request`, nur für
  „wiederholen“). Was ab Unterversion 5 mitwandert (Lösungen, Varianten, Basis und Verknüpfungen von Beiträgen,
  Schlüssel der Punkte), regelt §6.
- Seit Unterversion 3 (Runde 27) wandern die Satzbild-Einstellungen mit (`<satzbild>`, §2); vorher
  hielt nach dem Einlesen die Werkvorlage.
- Seit Unterversion 6 (Runde 29) wandern die Satzeinstellungen mit (`<satz>`, Abschnitt „Satzeinstellungen
  (Unterversion 6)“); der erzeugte Regelblock „// Baruch: Satz“ selbst steht nicht in der Datei.
- Seit Runde 22: die Einstellung **„Fassungen“** („Einstellungen → Werk → Fassungen“, Tabelle `work_revision_policy`:
  Fortschreiben an/aus, Schwelle der großen Änderung, Zeitgrenze) – sie ist eine Arbeitsweise der Instanz wie die offenen
  Anträge und wandert nicht mit (Test `the_revision_policy_is_not_in_the_exchange_format`). Was davon im Werk sichtbar
  wird, wandert dagegen mit: Fortschreibungen und Stände (Abschnitt „Fortschreibungen, Entwurf und Stand
  (Unterversion 8)“). Ebenfalls nicht im Format: das Wasserzeichen als Typst-Regel (nur die Einstellung `<entwurf/>`).

**Nutzerfragen mit Voreinstellung** (2026-11-08, blockieren nichts):

1. *Soll das Format auch die Autorenbibliothek (Titel, Zitate, Markierungen) tragen?* – Voreinstellung: **nein**,
   später stattdessen ein eigenes Format.
2. *Soll die Synchronisation später direkt zwischen zwei Instanzen über das Netz gehen, ohne Datei?* – Voreinstellung:
   **später**, mit denselben Elementen wie hier.

## 2 Elemente {#2-elements}

Seit Runde 29 ist das maßgebliche XSD `docs/schema/baruch-werk-1-u6.xsd` (Satzeinstellungen `<satz>`, Grabstein-Art
`satz`, siehe „Satzeinstellungen (Unterversion 6)“ unten; die Schemata u1 bis u5 sind eingefroren). In Runde 30
galt `docs/schema/baruch-werk-1-u5.xsd` (globale Elemente `<werk>` und `<manifest>`); Ausgaben ohne `<satz>` werden in
älteren Tests weiterhin mit `xmllint --noout --nonet --schema docs/schema/baruch-werk-1-u5.xsd DATEI` geprüft (Test `the_u5_schema_accepts_a_u4_file_and_the_frozen_u4_schema_too`:
eine Datei der Unterversion 4 besteht u5 und u4). Bis Runde 28 galt `docs/schema/baruch-werk-1-u4.xsd`
(`baruch-exchange/tests/xsd.rs`, Server `tests/round26_export.rs`, `round26_roundtrip.rs`, `round28_befunde.rs`;
`round25_widget.rs`; `round27_daten.rs` prüft gegen das Schema u3, sein Werk hat keine Befunde). Das
Schema u4 akzeptiert auch jede gültige Datei der Unterversionen 1 bis 3 (Test
`the_u3_schema_accepts_every_valid_file_of_versions_one_and_two`, prüft seit Runde 28 gegen u4 und u3).
Die Beispiele stammen aus dem erfundenen Beispielwerk `baruch_exchange::sample` (mit `…` gekürzt).

**Allgemeine Regeln**

- UTF-8, XML 1.0, keine DTD, keine Entitäten außer `&amp; &lt; &gt; &quot; &apos;` und Zeichenverweisen.
- Namensraum `urn:baruch:werk:1` = Hauptversion; `version="3"` an `<werk>` und `<manifest>` = Unterversion
  (§6). Ein Leser weist einen anderen Namensraum ab.
- Die Reihenfolge der Kinder ist fest (`xs:sequence`); Elemente mit `?` dürfen fehlen, mit `*` beliebig oft vorkommen,
  mit `+` mindestens einmal.
- **Text ist exakt:** Text steht immer in einem Element ohne Kinder; der Leser kürzt nie. Leerraum *zwischen*
  Elementen (Zeilenumbruch, Einrückung um zwei Leerzeichen) hat keine Bedeutung. Der Schreiber setzt Escapes für `&`, `<`,
  `>` (so steht `]]>` nie im Text) und `\r` als `&#13;` (jeder Parser machte aus einem wörtlichen `\r\n` ein `\n`);
  in Attributen zusätzlich `"` und Tabulator, Zeilenumbruch, `\r` als `&#9;`, `&#10;`, `&#13;`.
- **Verbotene Steuerzeichen** (in XML 1.0 selbst als Verweis unzulässig, z. B. U+0001): Ein Textelement
  (XSD-Typ `Text`) steht dann ganz als Base64 der UTF-8-Bytes mit `kodierung="base64"`; in einem Attribut ist das
  ein Fehler beim Export, der das Feld nennt (z. B. `werk/gliederung/punkt/fassung/@autor`).

  ```xml
  <text kodierung="base64">PT0gRGVyIEVpbWVyCgpEZXIgRWltZXIgAWtsaXJydCBsZWlzZSBhbSBSYW5kIGRlcyBCcnVubmVucy4K</text>
  ```

- Kennungen: UUID kanonisch in Kleinbuchstaben `8-4-4-4-12` (§3). Zeiten, wie sie gespeichert sind
  (`2026-01-05T09:00:00.000Z`, UTC). Wahrheitswerte `true`/`false`.

**`<werk>`** – Wurzel. Attribute `version` (Unterversion, heute `6`), `uuid` (Werk), `art` (`buch` | `predigt`).
Kinder in dieser Reihenfolge: `ausgabe`, `angaben`, `buch?`, `vorlage`, `gliederung`, `einfaelle?`, `faeden?`,
`bilder?`, `geloescht?`, `karte?`. Baruch schreibt keine leeren optionalen Teile.

```xml
<werk xmlns="urn:baruch:werk:1" version="3" uuid="26000000-0000-4000-8000-000000000001" art="buch">
```

**`<ausgabe>`** – wer wann exportiert hat: `zeit`, `instanz` (UUID der Instanz), `instanzname` (leer = ohne
Namen, angezeigt als „ohne Namen (26000000)“), `programm` (Programm und Version).

```xml
<ausgabe zeit="2026-01-08T19:00:00.000Z" instanz="26000000-0000-4000-8000-000000000002" instanzname="Probeinstanz" programm="baruch 0.0.1"/>
```

**`<angaben>`** – Metadaten des Werks: `<titel>`, `<untertitel>`, `<autor>`, `<zielgruppe>`, `<bibel>` (Übersetzung als
Text; immer vorhanden, auch leer), `<angelegt>`, `<geaendert>?`, `<schema name>` (Gliederungsschema `template`,
`decimal`, `decimal_dot`, `alphanumeric`, `chapter_roman`, `chapter_arabic`, `parts_chapters`, `chapter_words`,
`legal`, `sermon`, `book`, `none`, `custom`) mit eigenem `<muster>?`, `<bibelmodul>?` (SWORD-Bibel des Werks),
`<quellen>?` mit `<quelle art="kommentar|lexikon">Modul</quelle>*`, `<sprechdauer woerter-je-minute
ziel-sekunden?/>?` (Zeitwächter) und ab Unterversion 3 `<satzbild/>?` (Satzbild-Einstellungen des Werks, Tabelle
`work_sheet`, Runde 18c; nur mit mindestens einer Einstellung). Attribute von `<satzbild>`, jeweils optional (fehlt =
aus der Werkvorlage), Werte wie gespeichert: `schrift`, `schrift-ueberschriften` (Schriftlisten), `groesse`,
`satzbreite` (Länge mit Einheit `pt|mm|cm|em`), `farbe`, `farbe-ueberschriften`, `blattfarbe` (`#rrggbb`). Beim
Einlesen prüft Baruch die Werte wie beim Speichern (`SheetSettings::normalized`); was nicht besteht, wird nicht
übernommen und steht unter „Nicht übernommen“.

```xml
<angaben>
  <titel>Der Brunnen im Hof</titel>
  <untertitel>Ein erfundenes Buch für Prüfungen</untertitel>
  <autor>Hanna Probe</autor>
  <zielgruppe>Leser mit Geduld</zielgruppe>
  <bibel>Freie Probeübersetzung</bibel>
  <angelegt>2026-01-05T08:00:00.000Z</angelegt>
  <geaendert>2026-01-08T18:00:00.000Z</geaendert>
  <schema name="custom">
    <muster>I.;1.;a)</muster>
  </schema>
  <bibelmodul>ProbeBibel</bibelmodul>
  <quellen>
    <quelle art="kommentar">ProbeKommentar</quelle>
    <quelle art="lexikon">ProbeLexikon</quelle>
  </quellen>
  <sprechdauer woerter-je-minute="150" ziel-sekunden="1800"/>
  <satzbild schrift="&quot;Erfundene Antiqua&quot;, serif" groesse="11pt" blattfarbe="#fbf8f1"/>
</angaben>
```

**`<buch>?`** – Buchangaben, nur wenn das Werk welche hat. Attribute `titelbereich` (`oben` | `mitte` | `unten`),
`titelei`, `umschlagseiten` (Wahrheitswerte), `papier-mm`, `beschnitt-mm` (Zahlen), `umschlag?` und
`autorenfoto?` (UUID des **Bildes**, nicht der Fassung), `geaendert?`. Kinder (Text, immer vorhanden): `<verlag>`,
`<ort>`, `<jahr>`, `<auflage>`, `<isbn>`, `<preis>`, `<reihe>`, `<druckerei>`, `<bibelvermerk>`,
`<bildnachweis>`, `<widmung>`, `<klappentext>`, `<vita>`; ab Unterversion 3 gefolgt von `<verlag-anschrift>` und
`<druckerei-anschrift>` (mehrzeilig, höchstens 500 Zeichen; Baruch schreibt immer beide, fehlen sie, gelten sie als
leer) und `<umschlag>?` mit dem Umschlag-Layout (nur wenn es Blöcke gibt). Hat ein Werk Blöcke, aber keine
Buchangaben, steht `<buch>` mit den Voreinstellungen da (Titel oben, ohne Titelei, Umschlagseiten an, 0,1 mm Papier, 3 mm
Beschnitt).

**`<umschlag>`** (ab Unterversion 3) mit `<block>*` – höchstens ein Block je Art (Vertrag Runde 27). Attribute:
`uuid`, `art` (Bereich und Block, Tabelle unten), `x-mm`, `y-mm` (linke obere Ecke des Blockrahmens in mm vom
Beschnittrand seines eigenen Bereichs, ohne Anschnitt, −50 bis 1000; negativ = im Anschnitt), `breite-mm` (Breite in
Leserichtung, 1 bis 1000; am Rücken die senkrechte Ausdehnung), `ausrichtung` (`links` | `mitte` | `rechts` |
`blocksatz`), `groesse-pt` (4 bis 200), `zeilenabstand-em` (0 bis 3), `farbe` (`hell` = weiß | `dunkel` | `#rrggbb`
in Kleinbuchstaben), `schrift?` (Familienname aus Buchstaben, Ziffern, Leerzeichen, `- _ .`, höchstens 200 Zeichen; fehlt = Schrift
der Werkvorlage), `schnitt` (`normal` | `fett` | `kursiv`), `hintergrund` (`kein` | `schatten` | `verlauf`),
`sichtbar` (Wahrheitswert), `richtung?` (nur und immer am Rücken: `abwaerts` = von oben nach unten lesbar,
`aufwaerts` = von unten nach oben), `geaendert`. Kind `<text>?` (eigener Text, höchstens 1000 Zeichen) nur bei den
Arten mit eigenem Text (Freitexte und Motto); alle anderen nehmen ihren Text aus dem Werk und den Buchangaben.

| `art` | Datenbank (`cover_block.kind`) | Block |
|---|---|---|
| `vorderseite-titel`, `vorderseite-untertitel`, `vorderseite-autor`, `vorderseite-verlag`, `vorderseite-freitext` | `front_title`, `front_subtitle`, `front_author`, `front_publisher`, `front_free` | Vorderseite: Titel, Untertitel, Autor, Verlag, Freitext (mit `<text>`) |
| `ruecken-titel`, `ruecken-autor`, `ruecken-verlag`, `ruecken-freitext` | `spine_title`, `spine_author`, `spine_publisher`, `spine_free` | Rücken: Titel, Autor, Verlag, Freitext (mit `<text>`) |
| `rueckseite-motto`, `rueckseite-klappentext`, `rueckseite-vita`, `rueckseite-strichcode`, `rueckseite-verlag-preis`, `rueckseite-freitext` | `back_motto`, `back_blurb`, `back_bio`, `back_barcode`, `back_publisher_price`, `back_free` | Rückseite: Motto/Bibelvers (mit `<text>`), Klappentext, Kurzvita mit Foto, ISBN mit Strichcode, Verlag und Preis, Freitext (mit `<text>`) |

```xml
  <verlag-anschrift>Probeverlag
Am Brunnen 1
12345 Musterstadt</verlag-anschrift>
  <druckerei-anschrift>Probedruck
Druckgasse 2
12345 Musterstadt</druckerei-anschrift>
  <umschlag>
    <block uuid="26000000-0000-4000-8000-0000000000a0" art="vorderseite-titel" x-mm="15" y-mm="30.5" breite-mm="118" ausrichtung="mitte" groesse-pt="28" zeilenabstand-em="0.65" farbe="hell" schrift="Libertinus Serif" schnitt="fett" hintergrund="schatten" sichtbar="true" geaendert="2026-01-08T16:00:00.000Z"/>
    <block uuid="26000000-0000-4000-8000-0000000000a1" art="ruecken-titel" x-mm="2" y-mm="20" breite-mm="170" ausrichtung="links" groesse-pt="10" zeilenabstand-em="0.5" farbe="dunkel" schnitt="normal" hintergrund="kein" sichtbar="true" richtung="aufwaerts" geaendert="2026-01-08T16:00:00.000Z"/>
    <block uuid="26000000-0000-4000-8000-0000000000a2" art="rueckseite-motto" x-mm="-2.5" y-mm="15" breite-mm="100" ausrichtung="blocksatz" groesse-pt="11" zeilenabstand-em="0.8" farbe="#1f2a33" schnitt="kursiv" hintergrund="verlauf" sichtbar="false" geaendert="2026-01-08T16:00:00.000Z">
      <text>Erfundenes Motto &amp; &lt;Spruch&gt;
zweite Zeile</text>
    </block>
  </umschlag>
</buch>
```

Was das XSD nicht prüft, prüft der Leser in Rust (`read/check.rs`): höchstens ein Block je Art, `richtung` genau am
Rücken, `<text>` nur bei eigenem Text und höchstens 1000 Zeichen, Anschriften höchstens 500 Zeichen, keine Kennung
doppelt (auch kein Grabstein für einen vorhandenen Block).

```xml
<buch titelbereich="mitte" titelei="true" umschlagseiten="false" papier-mm="0.1" beschnitt-mm="3" umschlag="26000000-0000-4000-8000-00000000003c" geaendert="2026-01-08T17:00:00.000Z">
  <verlag>Probeverlag</verlag>
  <ort>Musterstadt</ort>
  <jahr>2026</jahr>
  <auflage>1. Auflage</auflage>
  <isbn/>
  <preis>12,00 €</preis>
  <reihe>Erfundene Reihe</reihe>
  <druckerei>Probedruck</druckerei>
  <bibelvermerk>Bibelzitate frei erfunden.</bibelvermerk>
  <bildnachweis>Bilder: Hanna Probe</bildnachweis>
  <widmung>Für alle, die Brunnen graben.</widmung>
  <klappentext>Ein Buch über Wasser &amp; Licht.</klappentext>
  <vita>Hanna Probe schreibt Prüftexte.</vita>
</buch>
```

**`<vorlage>`** – Werkvorlage: `<text>` (aktueller Text, `work.template`) und alle Fassungen `<fassung uuid
nummer autor angelegt>` mit `<notiz>?` (nur wenn nicht leer) und `<text>`.

```xml
<vorlage>
  <text>#set page(paper: "a5")
#set text(lang: "de")
</text>
  <fassung uuid="26000000-0000-4000-8000-000000000051" nummer="2" autor="Hanna Probe" angelegt="2026-01-06T08:00:00.000Z">
    <notiz>A5 gesetzt.</notiz>
    <text>#set page(paper: "a5")
#set text(lang: "de")
</text>
  </fassung>
</vorlage>
```

**`<gliederung>`** – die Wurzeln der Gliederung als `<punkt>*` der Reihe nach.

**`<punkt uuid art nummeriert>`** – Gliederungspunkt: `art` `teil` | `kapitel` | `abschnitt`, `nummeriert`
(`false` = „ohne Nummer“; die Nummer selbst wird beim Einlesen berechnet). Kinder: `<titel>`, alle Fassungen
`<fassung>+` nach Nummer, die Beiträge des Abschnittschats `<beitrag>*`, dann die Unterpunkte `<punkt>*`.

**`<fassung>` im Punkt** – Attribute `uuid`, `nummer` (Nummer in der exportierenden Instanz, mit Lücken nach
Löschungen), `status` (`aktuell` | `vorschlag` | `verworfen` | `abgeloest`; genau eine `aktuell` je Punkt),
`autor-art` (`mensch` | `assistent`), `autor` (Name oder Persona-Kürzel), `modell?` (nur beim Assistenten: Modell
der Fassung als Herkunft), `basis?` (UUID der Elternfassung im selben Punkt), `angelegt`, `geaendert?`. Kinder
`<notiz>?` und `<text>` (Typst-Text einschließlich der Überschrift, exakt).

```xml
<punkt uuid="26000000-0000-4000-8000-00000000000b" art="kapitel" nummeriert="true">
  <titel>Der Eimer</titel>
  <fassung uuid="26000000-0000-4000-8000-00000000006f" nummer="1" status="aktuell" autor-art="mensch" autor="Hanna Probe" angelegt="2026-01-05T09:00:00.000Z">
    <notiz>Erste Niederschrift.</notiz>
    <text>  == Der Eimer

	Der Eimer hält &lt;&amp;&gt; und schließt nie mit ]]&gt; – Grüße aus Übersee.&#13;
Ärger, Öl, Füße.  </text>
  </fassung>
  <fassung uuid="26000000-0000-4000-8000-000000000071" nummer="3" status="vorschlag" autor-art="assistent" autor="@lektor" modell="probe/modell-7b" basis="26000000-0000-4000-8000-00000000006f" angelegt="2026-01-07T09:00:00.000Z">
    <notiz>Klang ergänzt.</notiz>
    <text kodierung="base64">PT0gRGVyIEVpbWVy…</text>
  </fassung>
  …
</punkt>
```

**`<beitrag>` im Punkt** – Beitrag im Abschnittschat: `uuid`, `position` (Reihenfolge ab 1, ohne Lücken – nicht
die interne Position), `autor` (Name, `@handle` oder `@mcp:<client>`), `angelegt`, ab Unterversion 4 `geaendert?`
(Zeit der letzten Bearbeitung, fehlt = nie bearbeitet). Kinder `<text>`, `<quelle>*` und
`<bezug punkt fassung?>Zitat</bezug>*` (Verweis über die UUID des Punkts und, wenn die Fassung existiert, deren UUID),
ab Unterversion 4 gefolgt von `<alte-fassung uuid gueltig-ab ersetzt bearbeitet-von>` mit `<text>` je alter Fassung,
älteste zuerst (Tabelle `post_version`; `gueltig-ab` = seit wann diese Fassung galt, `ersetzt` = wann sie
ersetzt wurde, `bearbeitet-von` = wer bearbeitet hat).

```xml
<beitrag uuid="26000000-0000-4000-8000-00000000001f" position="2" autor="Hanna Probe" angelegt="2026-01-07T12:00:00.000Z" geaendert="2026-01-07T13:00:00.000Z">
  <text>Ja, und die Lampe später auch.</text>
  …
  <alte-fassung uuid="26000000-0000-4000-8000-00000000012c" gueltig-ab="2026-01-07T12:00:00.000Z" ersetzt="2026-01-07T13:00:00.000Z" bearbeitet-von="Hanna Probe">
    <text>Ja, und die Lampe &amp; &lt;das Seil&gt;.</text>
  </alte-fassung>
</beitrag>
```

```xml
<beitrag uuid="26000000-0000-4000-8000-00000000001e" position="1" autor="@lektor" angelegt="2026-01-07T11:00:00.000Z">
  <text>Der Eimer braucht ein Bild am Anfang.</text>
  <quelle>Probequelle, S. 3</quelle>
  <bezug punkt="26000000-0000-4000-8000-00000000000b" fassung="26000000-0000-4000-8000-00000000006f">Der Eimer hält</bezug>
</beitrag>
```

**`<einfaelle>?`** mit `<einfall>*` – alle Einfälle des Werks, auch gelöschte. Attribute `uuid`, `status`
(`offen` | `erledigt`), `erledigt-als?` (`gliederung` | `beitrag` | `persona` | `notiz`), `herkunft-art`
(`mensch` | `persona` | `mcp`), `herkunft` (Name, `@handle`, `@mcp:<client>`), `angelegt`, `geaendert`,
`geloescht?` (weich gelöscht seit), `bezug?` (UUID des Punkts), ab Unterversion 9 `titel?`, `eltern?`, `platz?`,
`hauptthema?` (Baum der Mindmap, siehe „Baum der Mindmap (Unterversion 9, Runde 34)“). Kinder `<schlagwort>*`, `<notiz>`, `<zitat>`,
`<quelle>`, `<bibelstelle>` (immer vorhanden, auch leer) und `<erledigt-ref>?` (freier Verweis; ein „Schlüssel N“ der
Datenbank steht darin als UUID des Punkts, der Rest wörtlich).

```xml
<einfall uuid="26000000-0000-4000-8000-000000000028" status="offen" herkunft-art="mensch" herkunft="Hanna Probe" angelegt="2026-01-06T09:00:00.000Z" geaendert="2026-01-06T09:00:00.000Z" bezug="26000000-0000-4000-8000-00000000000c">
  <schlagwort>Seil</schlagwort>
  <schlagwort>Knoten</schlagwort>
  <notiz>Das Seil reißt nie ganz.</notiz>
  <zitat/>
  <quelle/>
  <bibelstelle>Pred 4,12</bibelstelle>
</einfall>
```

**`<faeden>?`** mit `<faden uuid angelegt-von angelegt geaendert>*` – Fäden ohne Abschnitt: `<titel>` und
`<beitrag uuid position autor angelegt>*` mit `<text>` und `<quelle>*` (`position` ab 1); ab Unterversion 4 wie im
Punkt `geaendert?` und `<alte-fassung>*` am Ende.

```xml
<faden uuid="26000000-0000-4000-8000-000000000032" angelegt-von="@lektor" angelegt="2026-01-07T14:00:00.000Z" geaendert="2026-01-07T15:00:00.000Z">
  <titel>Fragen zum Brunnen</titel>
  <beitrag uuid="26000000-0000-4000-8000-000000000034" position="2" autor="Hanna Probe" angelegt="2026-01-07T15:00:00.000Z">
    <text>Sieben Meter, erfunden.</text>
    <quelle>Notizbuch</quelle>
  </beitrag>
</faden>
```

**`<bilder>?`** mit `<bild uuid name angelegt>*` – Bilder des Werks: `<pfad>*` (Pfade, unter denen Vorlage und
Texte das Bild nennen, sortiert) und alle Fassungen `<fassung>+`. Attribute der Bildfassung: `uuid`, `nummer`,
`basis?` (UUID der Elternfassung), `mime` (`image/png` | `image/jpeg` | `image/gif` | `image/webp` |
`image/svg+xml`), `bytes`, `sha256` (über die rohen Bytes), `status` (`aktuell` | `abgeloest`), `autor`,
`angelegt`, `geaendert?` und im Container `datei` (Eintrag im ZIP). Kinder `<unterschrift>`, `<alt>`,
`<notiz>?` und in der Form nur XML `<daten>` (rohe Bytes als Base64, ohne Zeilenumbrüche). Genau eines von `datei` und
`<daten>` ist vorhanden.

```xml
<bild uuid="26000000-0000-4000-8000-00000000003c" name="brunnen.png" angelegt="2026-01-06T10:00:00.000Z">
  <pfad>bilder/brunnen.png</pfad>
  <fassung uuid="26000000-0000-4000-8000-00000000003e" nummer="2" basis="26000000-0000-4000-8000-00000000003d" mime="image/png" bytes="70" sha256="0828e85a…1efc" status="aktuell" autor="Hanna Probe" angelegt="2026-01-06T12:00:00.000Z" datei="bilder/26000000-0000-4000-8000-00000000003e.png">
    <unterschrift>Der Brunnen im Abendlicht</unterschrift>
    <alt>Ein roter Punkt</alt>
    <notiz>Farbe gewechselt.</notiz>
  </fassung>
</bild>
```

In der Form nur XML steht statt `datei=…` am Ende der Fassung:
`<daten>iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mM4YKn9HwAFBQIkh/EstQAAAABJRU5ErkJggg==</daten>`.

**`<geloescht>?`** mit `<eintrag uuid art zeit/>*` – Grabsteine: hier gelöschte Stücke des Werks (§3), `art`
`werk` | `punkt` | `fassung` | `vorlage` | `beitrag` | `einfall` | `faden` | `faden-beitrag` | `bild` |
`bildfassung`, ab Unterversion 2 `verbindung` | `lage` (in einer Datei mit `version="1"` sind diese beiden ein
Fehler), ab Unterversion 3 `umschlag-block` (in einer Datei mit `version="1"` oder `"2"` ein Fehler) und ab
Unterversion 4 `befund` | `befund-status` | `beitrag-fassung` (in einer Datei mit `version` 1 bis 3 ein Fehler), ab
Unterversion 6 `satz` (in einer Datei mit `version` 1 bis 5 ein Fehler), ab Unterversion 7 `glossar-begriff` |
`register` und ab Unterversion 8 `fortschreibung` (in einer Datei mit `version` 1 bis 7 ein Fehler).

```xml
<geloescht>
  <eintrag uuid="26000000-0000-4000-8000-000000000046" art="punkt" zeit="2026-01-07T16:00:00.000Z"/>
</geloescht>
```

**`<karte>?`** – die Einfallskarte des Werks (ab Unterversion 2, Runde 25), am Ende von `<werk>` und nur, wenn es
mindestens eine Verbindung oder Lage gibt: zuerst alle `<verbindung>`, dann alle `<lage>`. Verweise laufen über die
UUIDs der Einfälle dieses Werks (`<einfaelle>`); Verbindungen gibt es **nur zwischen Einfällen** (nicht zu Punkten
oder Beiträgen).

- **`<verbindung>`** – Attribute `uuid`, `von`, `nach` (UUIDs der Einfälle), `art` (`fuehrt-weiter` |
  `gehoert-zu` | `beispiel-fuer` | `widerspricht`; in der Oberfläche „führt weiter“, „gehört zu“, „Beispiel für“,
  „widerspricht“), `herkunft-art` (`mensch` | `persona` | `mcp`), `herkunft` (Name, `@handle`, `@mcp:<client>`),
  `angelegt`, `geaendert`, `geloescht?` (weich gelöscht seit; wiederherstellbar, die Verbindung bleibt im
  Format). Kind `<notiz>` (höchstens 500 Zeichen, ohne Steuerzeichen außer Zeilenumbruch; leer als `<notiz/>`).
  `widerspricht` ist ungerichtet: Die Reihenfolge von `von`/`nach` hat keine Bedeutung (Baruch speichert und
  schreibt das Paar in der Reihenfolge seiner Einfälle); die anderen Arten sind gerichtet. Je Paar und Art steht höchstens
  eine Verbindung im Dokument (bei `widerspricht` auch nicht umgekehrt).

  ```xml
  <verbindung uuid="26000000-0000-4000-8000-000000000096" von="26000000-0000-4000-8000-000000000028" nach="26000000-0000-4000-8000-000000000029" art="fuehrt-weiter" herkunft-art="mensch" herkunft="Hanna Probe" angelegt="2026-01-07T19:00:00.000Z" geaendert="2026-01-07T19:00:00.000Z">
    <notiz>Vom Seil zur Lampe.</notiz>
  </verbindung>
  <verbindung uuid="26000000-0000-4000-8000-000000000097" von="26000000-0000-4000-8000-000000000029" nach="26000000-0000-4000-8000-00000000002a" art="widerspricht" herkunft-art="persona" herkunft="@lektor" angelegt="2026-01-07T20:00:00.000Z" geaendert="2026-01-07T21:00:00.000Z" geloescht="2026-01-07T21:00:00.000Z">
    <notiz>Eimer &amp; &lt;Lampe&gt;
  streiten sich.</notiz>
  </verbindung>
  ```

- **`<lage/>`** – gespeicherte Lage eines Einfalls auf der Karte dieses Werks (der Mensch hat ihn dorthin
  gezogen): Attribute `uuid`, `einfall` (UUID des Einfalls), `x`, `y` (SVG-Nutzereinheiten der Karte, das Werk
  steht bei 0,0; kürzeste Darstellung, die beim Einlesen genau dieselbe Zahl ergibt, endlich und höchstens
  ±1 000 000), `geaendert`. Höchstens eine Lage je Einfall. Lagen sind reine Anzeige (kein Spiegel-Commit).

  ```xml
  <lage uuid="26000000-0000-4000-8000-000000000098" einfall="26000000-0000-4000-8000-000000000028" x="120.5" y="0.30000000000000004" geaendert="2026-01-07T22:00:00.000Z"/>
  <lage uuid="26000000-0000-4000-8000-000000000099" einfall="26000000-0000-4000-8000-000000000029" x="-1000000" y="-80.25" geaendert="2026-01-07T23:00:00.000Z"/>
  ```

- Grabsteine der Karte stehen wie alle in `<geloescht>`: `<eintrag uuid="…" art="verbindung" zeit="…"/>` oder
  `art="lage"`.

**`<befunde>?`** – die Befunde des Werks (ab Unterversion 4, Runde 28), ganz am Ende von `<werk>` (nach `<karte>`)
und nur, wenn es welche gibt: `<befund>*` in der Reihenfolge ihrer Entstehung. Ein Befund ist ein Punkt aus der Antwort
einer Persona (`docs/Bedienung.md`, „Befunde“).

- **`<befund>`** – Attribute `uuid`, `art` (`lob` | `hinweis` | `kritisch`), `status` (`offen` | `uebernommen` |
  `verworfen` | `verschoben` | `erledigt`; in der Oberfläche „offen“, „übernommen“, „verworfen“, „verschoben“,
  „erledigt ohne Änderung“), `persona` (Autor der Antwort, 1 bis 200 Zeichen), `stelle` (Stelle in der Antwort, ab
  0), `punkt?` (UUID des Abschnitts; fehlt bei Fäden und gelöschten Abschnitten), `beitrag?` oder `faden-beitrag?`
  (UUID der Herkunft, nie beide; fehlt, wenn der Beitrag gelöscht ist), `fassung?` (UUID der Fassung bei
  „übernommen“, Pflicht), `ziel-punkt?` oder `ziel-einfall?` (Ziel bei „verschoben“, genau eines), `gleicht?` (UUID
  des Befunds, dem dieser gleicht – „angehängt“, zählt nicht als offen), `angelegt`, `geaendert`. Kinder `<text>`
  (nicht leer, höchstens 4000 Zeichen), `<zitat>?` (wörtliches Zitat, höchstens 2000), `<pruefvermerk>?` (Vermerk
  der Zitat- und Stellenprüfung, z. B. „(nicht im Text belegt)“, höchstens 200), `<begruendung>?` (Pflicht bei
  „verworfen“, höchstens 2000), `<sammelt befund="…"/>*` (Sammelvorschlag des Lektors: UUIDs der zusammengefassten
  Befunde) und `<status-eintrag>*` (Verlauf, ältester zuerst). Fassung und Ziel stehen dort als Kennungen, auch wenn sie in
  der Datei nicht mehr vorhanden sind (wie in der Datenbank ohne Fremdschlüssel).
- **`<status-eintrag>`** – eine Statusänderung durch den Menschen oder ein Vorschlag eines Assistenten: `uuid`, `status`,
  `herkunft-art` (`mensch` | `persona` | `mcp`), `herkunft`, `vorschlag` (`true` = Vorschlag; ohne Vorschlag setzt es nur
  der Mensch), `fassung?`, `ziel-punkt?`, `ziel-einfall?`, `entscheidung?` (`bestaetigt` | `abgelehnt` |
  `veraltet`, nur bei Vorschlägen, mit `entschieden` und `entschieden-von`), `angelegt`; Kinder `<begruendung>?` und
  `<notiz>?`. Die erste Entstehung eines Befunds hat keinen Eintrag.

```xml
<befunde>
  <befund uuid="26000000-0000-4000-8000-000000000136" art="kritisch" status="verworfen" persona="@lektor" stelle="0" punkt="26000000-0000-4000-8000-00000000000b" beitrag="26000000-0000-4000-8000-00000000001e" angelegt="2026-01-07T11:00:00.000Z" geaendert="2026-01-08T02:00:00.000Z">
    <text>„Der Eimer hält“ wirkt zu glatt &amp; &lt;knapp&gt;.</text>
    <zitat>Der Eimer hält</zitat>
    <pruefvermerk>(nicht im Text belegt)</pruefvermerk>
    <begruendung>Absicht: der Eimer bleibt glatt.
Zweite Zeile.</begruendung>
    <status-eintrag uuid="26000000-0000-4000-8000-000000000141" status="erledigt" herkunft-art="persona" herkunft="@lektor" vorschlag="true" entscheidung="veraltet" entschieden="2026-01-08T03:00:00.000Z" entschieden-von="Hanna Probe" angelegt="2026-01-08T01:00:00.000Z">
      <notiz>Erledigt sich mit Fassung 3.</notiz>
    </status-eintrag>
    <status-eintrag uuid="26000000-0000-4000-8000-000000000140" status="verworfen" herkunft-art="mensch" herkunft="Hanna Probe" vorschlag="false" angelegt="2026-01-08T02:00:00.000Z">
      <begruendung>Absicht: der Eimer bleibt glatt.
Zweite Zeile.</begruendung>
    </status-eintrag>
  </befund>
</befunde>
```

**`<manifest version>`** mit `<datei pfad sha256 bytes/>+` – nur im Container (`MANIFEST.xml`, Beispiel in §1).

## 3 Kennungen und Grabsteine {#3-identifiers-and-tombstones}

**Kennungen.** Jedes Stück, das zwischen Instanzen wandert, trägt eine weltweit eindeutige Kennung (UUID) in
kanonischen Kleinbuchstaben `8-4-4-4-12`, z. B. `3f2b8c1e-9a47-4d2e-b5c3-0e1f2a3b4c5d`:

| Element im XML | Tabelle | Art im Grabstein (`<eintrag art>` / Datenbank) |
| --- | --- | --- |
| `<werk uuid>` | `work` | `werk` / `work` |
| `<punkt uuid>` | `node` | `punkt` / `node` |
| `<punkt>/<fassung uuid>` | `revision` | `fassung` / `revision` |
| `<vorlage>/<fassung uuid>` | `work_template_revision` | `vorlage` / `template_revision` |
| `<punkt>/<beitrag uuid>` | `post` | `beitrag` / `post` |
| `<einfall uuid>` | `idea` | `einfall` / `idea` |
| `<faden uuid>` | `chat_thread` | `faden` / `thread` |
| `<faden>/<beitrag uuid>` | `chat_thread_post` | `faden-beitrag` / `thread_post` |
| `<bild uuid>` | `asset` | `bild` / `asset` |
| `<bild>/<fassung uuid>` | `asset_revision` | `bildfassung` / `asset_revision` |
| `<karte>/<verbindung uuid>` (ab Unterversion 2) | `idea_link` | `verbindung` / `idea_link` |
| `<karte>/<lage uuid>` (ab Unterversion 2) | `idea_map_pos` | `lage` / `idea_map_pos` |
| `<buch>/<umschlag>/<block uuid>` (ab Unterversion 3) | `cover_block` | `umschlag-block` / `cover_block` |
| `<befunde>/<befund uuid>` (ab Unterversion 4) | `finding` | `befund` / `finding` |
| `<befund>/<status-eintrag uuid>` (ab Unterversion 4) | `finding_status` | `befund-status` / `finding_status` |
| `<beitrag>/<alte-fassung uuid>` (ab Unterversion 4, Abschnitt und Faden) | `post_version` | `beitrag-fassung` / `post_version` |
| `<angaben>/<satz uuid>` (ab Unterversion 6) | `work_typeset` | `satz` / `work_typeset` |
| `<glossar>/<begriff uuid>`, `<eigene-register>/<register uuid>` (ab Unterversion 7) | `glossary_term`, `index_register` | `glossar-begriff` / `glossary_term`, `register` / `index_register` |
| `<fassung>/<fortschreibung uuid>` (ab Unterversion 8) | `revision_amend` | `fortschreibung` / `revision_amend` |

- **Vergabe:** Neue Zeilen erhalten ihre UUID in der Datenbank (Trigger `<Tabelle>_uuid_fill`, Version 4,
  zufällig, Variante 10). Das gilt für jeden Weg: Anlegen, Typst-Import (auch mit Bildern), Beispielwerk,
  Gliederung (auch durch Assistenten), Speichern, Vorschläge, Beiträge, Einfälle, Fäden, Bilder und neue
  Fassungen der Werkvorlage. Beim Einlesen behält ein Stück die UUID aus der Datei; abweichende Fassungen
  erhalten eine abgeleitete UUID Version 8 (siehe §4).
- **Unveränderlich:** Verschieben, Umbenennen, Ebene, Nummer an/aus, Umschalten, Übernehmen, Ändern einer
  Fassung und Umschalten eines Bildes behalten jede UUID. Ein umbenannter Punkt bleibt derselbe Punkt.
- **Intern** bleiben die Zahlen (`id`, `key`, `seq`, Fassungsnummer): Im XML verweist alles über UUIDs, z. B.
  `<bezug punkt="…" fassung="…">` oder `basis="…"`.
- **Instanz:** Jede Baruch-Instanz hat eine eigene Kennung (`store_meta.instance_uuid`, beim ersten Start vergeben)
  und einen frei gewählten Namen (`store_meta.instance_name`, Voreinstellung leer). Beide stehen in
  `<ausgabe instanz="…" instanzname="…">`. Ohne Namen zeigt Baruch „ohne Namen (<erste 8 Zeichen>)“, z. B.
  „ohne Namen (3f2b8c1e)“. Setzen: „Einstellungen → Daten“ oder RPC `instance.set_name`.

**Grabsteine.** Wer eine Zeile mit UUID löscht (Fassung, Beitrag, Bildfassung, Bild, Gliederungspunkt mit seinen
Fassungen und Beiträgen, ganzes Werk mit seinen Punkten, Fassungen, Vorlagenfassungen und Fäden), hinterlässt einen Eintrag in
`uuid_tombstone` mit UUID, Art, Werk-UUID und Zeit. Der Export schreibt die Grabsteine
des Werks als

```xml
<geloescht>
  <eintrag uuid="0c1d2e3f-4a5b-4c6d-8e7f-8091a2b3c4d5" art="fassung" zeit="2026-11-08T17:20:31.512Z"/>
</geloescht>
```

- Beim **Abgleichen** legt die Instanz nichts neu an, wofür sie einen Grabstein hat, und zeigt es in der
  Vorschau als „hier gelöscht“. Beim **Einlesen als neues Werk** gelten Grabsteine nicht.
- Ein Einfall wird nur weich gelöscht (`geloescht="…"` am `<einfall>`, wiederherstellbar) und hat darum keinen
  Grabstein. Ebenso eine Verbindung der Karte (`geloescht="…"` an `<verbindung>`); ihr Grabstein (`verbindung`)
  entsteht praktisch nur, wenn ein Einfall hart gelöscht wird (Kaskade). Eine Lage hinterlässt einen Grabstein
  (`lage`), wenn der Mensch die Anordnung zurücksetzt (die Lagen werden dann gelöscht) oder das Werk gelöscht wird.
  Ein Umschlag-Block hinterlässt einen Grabstein (`umschlag-block`), wenn der Mensch das Umschlag-Layout ganz oder
  für einen Bereich zurücksetzt (auch über die Schnellwahl) oder das Werk gelöscht wird; das Ändern eines Blocks behält
  seine Zeile und Kennung. Befunde, ihre Verlaufseinträge und alte Beitragsfassungen hinterlassen Grabsteine
  (`befund`, `befund-status`, `beitrag-fassung`), wenn sie gelöscht werden – auch in der Kaskade, wenn ihr Werk,
  ihr Befund oder ihr Beitrag gelöscht wird (Migration 0046); ein gelöschter Beitrag lässt seine Befunde stehen
  (ohne Herkunft). Wird ein Werk gelöscht, bleiben seine Einfälle als Einfälle ohne Werk (ebenfalls ohne
  Grabstein, die Kennung bleibt). Wird das Werk später aus seiner Datei erneut eingelesen (neu oder abgleichend),
  erhält es diese Einfälle zurück: Werk und Bezug werden gesetzt, ihr Inhalt bleibt, wie er hier ist; die
  Vorschau nennt sie „Einfall ohne Werk, wieder zugeordnet“ (Test
  `a_deleted_work_comes_back_from_its_own_file_and_gets_its_ideas_back`).
- Der Speicher selbst schaut nie in die Grabsteine; sie zählen nur beim Abgleich.

## 4 Einlesen und Abgleich {#4-reading-in-and-synchronisation}

Das Einlesen ist eine Handlung des Menschen (Formular, RPC, Kommandozeile; **nicht** über MCP). Es schaltet nie um,
überschreibt nie und löscht nie; Änderungen der Gliederung werden zu Anträgen. Code: Einlesen
`baruch-exchange/src/read.rs` (Untermodule `read/{dom,map,archive,check}.rs`), Planen und Schreiben in
`baruch-store/src/exchange_apply.rs`, Wege in `baruch-server/src/exchange/import.rs`.

**Ablauf.** Zuerst die **Vorschau**: Die Datei wird gelesen und geprüft (§5), dann plant der Speicher das Einlesen in
einer Transaktion, die am Ende zurückgerollt wird – die Vorschau ist also genau das, was „Übernehmen“ täte, und
schreibt nichts. „Übernehmen“ verlangt die Prüfsumme (SHA-256) der gezeigten Datei, liest sie neu, plant neu (der
Vorschau wird nicht vertraut) und schreibt unter der Schreibsperre in **einer** Transaktion (`BEGIN IMMEDIATE`);
am Ende prüft `load::work` das Werk in derselben Transaktion. Danach genau **ein** Commit im Spiegel mit dem
Betreff „Austausch aus <Instanz>: …“ (Autor „Austausch aus <Instanz>“, Trailer `Baruch-Action: exchange-import`,
`Baruch-Exchange-Instance`, `Baruch-Exchange-File`); die semantische Suche wird wie beim Typst-Import benachrichtigt.
Ein Abgleich, der nichts hinzufügt und keinen Antrag stellt, macht keinen Commit. `<Instanz>` ist der
Instanzname aus `<ausgabe>`, sonst die ersten 8 Zeichen ihrer Kennung. Der Leser bereinigt den Namen
(`baruch_exchange::clean_instance_name`): Steuerzeichen und Leerraum werden zu einem Leerzeichen, `<` und `>` werden zu
`‹` und `›` (Git weist sie im Autor ab), auf 80 Zeichen gekürzt wie lokal.

**Modus.** Die Werk-UUID entscheidet: hier unbekannt → **neu** (Grabsteine zählen dafür nicht), bekannt →
**abgleichen**. Wer den falschen Modus verlangt, erhält eine Abweisung (RPC -32000, Formular 409, CLI Exit 1).

### Neu {#new}

Das Werk entsteht mit allen Kennungen, Nummern, Status, Zeiten und der Marke „aktuell“ aus der Datei:
Werkangaben mit Schema und eigenem Muster, Vorlage (aktueller Text und alle Fassungen), Gliederung (die Nummern
`node.number` berechnet die Gliederung wie beim Laden), Fassungen mit Basis, Beiträge mit Quellen und Verweisen,
Einfälle (gelöschte bleiben gelöscht, mit ihrer Zeit), Fäden, Bilder mit Pfaden und allen Fassungen, Umschlag und
Autorenfoto, Anschriften, das Umschlag-Layout (alle Blöcke mit ihren Kennungen und Zeiten), die
Satzbild-Einstellungen, Sprechdauer, Bibelmodul, SWORD-Quellen, die Einfallskarte (alle Verbindungen, auch gelöschte, und
alle Lagen mit ihren Kennungen; `widerspricht` wird so gespeichert, dass der kleinere Einfall zuerst steht), alte
Fassungen bearbeiteter Beiträge und Faden-Beiträge mit `geaendert`, die Befunde mit Status, Begründung, Fassung,
Ziel, Sammlung und Verlauf (`gleicht` über die Kennung, nachdem alle Befunde stehen) und die Grabsteinliste der
Datei. Titelregeln wie beim
Typst-Import: Ein schon vergebener Titel wird abgewiesen (in der Vorschau unter „Nicht übernommen“), mit `title` geht es
unter einem eigenen Titel. Ein Bild, dessen Bytes die Prüfung wie beim Hochladen nicht bestehen (höchstens 8 MiB,
Typ aus den ersten Bytes erkannt und gleich dem angegebenen), kommt nicht mit und steht unter „Nicht
übernommen“. Ein Verweis ohne Fassung wird zu `rev = 0`; ein `erledigt-ref`, der mit der Kennung eines Punkts beginnt,
wird wieder zu „key <Schlüssel hier>…“ (Gegenstück zum Export).

### Abgleichen (Regeln F6) {#synchronising-rules-f6}

Es wird nur hinzugefügt, was fehlt. Jede vorhandene Zeile bleibt in allen Spalten, wie sie war (Ausnahme: Stelle,
Elternstelle, Reihenfolge und Nummer der Punkte sowie `last_rev`, die das Anlegen eines Punkts oder einer
Fassung neu berechnet; geprüft im Test `merging_never_overwrites_an_existing_row`).

- **(a) Unbekannter Punkt:** an seiner Stelle angelegt – hinter dem vorherigen Geschwister aus der Datei, das hier
  unter demselben Eltern existiert, sonst als erstes Kind der Eltern –, mit **genau** seinen Fassungen aus der
  Datei (keine zusätzliche Fassung 1). Bekäme er dort andere Eltern oder übernähme er vorhandene Punkte,
  wird er nicht angelegt (Hinweis). Ein **neuer Teil**, unter den vorhandene Kapitel wandern würden, wird ein
  Antrag `Add`, den der Mensch bestätigt (AGENTS.md). Seit Runde 22 trägt der Antrag die **Kennung des Teils und
  aller seiner Fassungen** (Nummer, Text, Herkunft, Status, Basis, Stand, Fortschreibungen; die Vorschau sagt „Antrag mit
  Kennung und n Fassung(en)“, der Antrag „… (mit Kennung und n Fassungen) …“). Bei der Bestätigung entsteht der Teil mit
  genau dieser Kennung und genau diesen Fassungen (die Kapitel wandern wie bisher darunter, ein Spiegel-Commit); ein
  späterer Abgleich mit derselben Datei erkennt ihn wieder und meldet nichts. Existiert die Kennung inzwischen hier,
  ist der Antrag veraltet; Ablehnen legt nichts an und hinterlässt keinen Grabstein. Unterpunkte eines Punkts, der nicht angelegt wurde,
  kommen nicht (sie wandern bei der Bestätigung nur unter den Teil, wenn die Kapitel hier schon existieren).
- **(b) Bekannter Punkt:** anderer Titel → Antrag `Rename`, es sei denn, eine hier vorhandene oder gerade angelegte Fassung
  trägt schon den Titel der Datei (dann ein Hinweis mit Link zu dieser Fassung: Umschalten genügt). Andere Stelle
  (anderes Elternteil oder andere Reihenfolge unter den Punkten, die beide Seiten kennen) → Antrag `Move` hinter
  dem vorherigen Geschwister, sonst vor dem nächsten, sonst unter die Eltern. Andere Ebene 1–3 → `SetLevel`,
  anders nummeriert → `SetNumbered`. Gelingt der Probelauf des Antrags nicht oder ist derselbe Antrag schon offen,
  wird nur der Unterschied aufgeführt. Ein Punkt, der in der Datei fehlt (oder dort gelöscht ist), ergibt nur einen
  Hinweis – nie einen Antrag zum Löschen.
- **(c) Unbekannte Fassung:** mit neuer lokaler Nummer angehängt (`last_rev + 1`), Basis über die Kennung
  (fehlt die Elternfassung, wird sie eine Wurzel). Status: `vorschlag` → Vorschlag, `verworfen` → verworfen,
  `aktuell` oder `abgeloest` → abgelöst. Eine eingelesene Fassung wird **nie** aktuell. Hat der Punkt hier schon
  eine Fassung mit genau demselben Text, wird nichts angelegt (Inhaltsgleichheit, siehe „Hin und zurück“ unten).
- **(d0) Vorspulen (seit Runde 22, Unterversion 8):** Sind die Fortschreibungen einer hier bekannten Fassung ein echtes
  Präfix der eingelesenen (dieselben Kennungen in derselben Reihenfolge, Stand hier = Zahl der Schritte hier) und
  ist der Text hier genau der Text der Datei an dieser Stelle (nach dem letzten gemeinsamen Schritt oder vor dem ersten),
  wird die Fassung **vorgespult**: Die fehlenden Schritte werden mit ihren Kennungen ergänzt, Text und Stand folgen der Datei;
  Nummer, Status und Herkunft bleiben. Die Vorschau zählt sie als „Fortschreibungen“ (`AddCounts.amends`) und meldet
  „Fassung n: vorgespult von Stand k auf Stand m“ (Hinweis); der Commit des Einlesens enthält es. Vorgespult wird nur, was auch lokal
  fortgeschrieben werden darf (Nachtrag R22) – die **aktuelle Fassung des Menschen** (hier
  aktuell und vom Menschen, in der Datei vom Menschen); Fassungen von Personas oder MCP-Clients, Vorschläge,
  übernommene, verworfene und abgelöste Fassungen werden nie an Ort und Stelle umgeschrieben. Und nur, wenn die
  fehlenden Schritte die Änderung **lückenlos** nachbilden: erster fehlender Schritt `<vorher>` = Text hier, jedes
  `<vorher>` = `<nachher>` des vorigen, letztes `<nachher>` = Text der Fassung in der Datei. Das Lesen prüft das nicht
  (Ändern über den Baum ändert einen Text ohne Schritt, eine gültige Datei kann Lücken haben); passt es nicht,
  gilt (d).
- **(d) Bekannte Fassung mit anderem Text:** Es entsteht eine weitere Fassung mit der abgeleiteten Kennung
  `derive(original, text)`, Basis = die Fassung hier, Status nach (c), Notiz mit dem Zusatz
  „(Austausch: abweichende Fassung aus <Instanz>)“. Existiert die abgeleitete Kennung schon oder hat der Punkt hier
  schon eine Fassung mit genau diesem Text, geschieht nichts (ein wiederholtes Einlesen legt nichts doppelt an). Eine andere Notiz oder ein anderer Stand „aktuell“: nur ein Hinweis
  mit dem Link `/?work=W&key=K&rev=R`.
- **(e) Vorlage:** Fassungen mit unbekannter Kennung werden angehängt (nächste Nummer); der aktuelle Vorlagentext
  `work.template` bleibt, ein Unterschied ist ein Hinweis.
- **(f) Einfälle, Beiträge (mit Verweisen), Fäden und Faden-Beiträge, Bilder und Bildfassungen** mit unbekannter
  Kennung werden hinzugefügt (Beiträge und Faden-Beiträge am Ende, Bilder über die Prüfung wie beim Hochladen,
  Namenskollision → freier Name `-2`, `-3` … wie beim Import; eingelesene Bildfassungen eines bekannten Bildes
  werden abgelöst, nie aktuell). Bekannte mit anderem Inhalt: nur ein Hinweis.
- **(g) Werkangaben, Buchangaben (mit Anschriften), Satzbild-Einstellungen, Sprechdauer, Bibelmodul,
  SWORD-Quellen:** Fehlt die Zeile hier ganz, wird sie aus der Datei angelegt (Hinweis); sonst bei einem Unterschied nur ein
  Hinweis.
- **(i) Umschlag-Layout** (Runde 27): Ein Block mit unbekannter Kennung wird hinzugefügt, wenn es hier für (Werk, Art)
  noch keinen Block gibt – **lokal gewinnt**, sonst ein Hinweis „hier gibt es schon einen Block dieser Art“. Eine
  bekannte Kennung wird nie überschrieben (andere Werte: Hinweis „weicht ab“). Hat der Block hier einen Grabstein
  (Zurücksetzen), kommt er nicht zurück („Hier gelöscht“). Die Vorschau zählt „Blöcke der Umschlag-Anordnung“
  (`AddCounts.cover_blocks`, auch in `total()`).
- **(j) Befunde und alte Beitragsfassungen** (Runde 28): Ein Befund mit unbekannter Kennung wird hinzugefügt, wenn seine
  Herkunft (Beitrag oder Faden-Beitrag) zum Werk hier gehört, kein Grabstein existiert und diese Stelle der Antwort hier
  noch frei ist – sonst ein Hinweis (etwa „diese Stelle der Antwort ist hier schon Befund 12 (unter anderer
  Kennung)“, wenn beide Instanzen denselben Beitrag selbst zerlegt haben). Er bringt Status, Begründung, Fassung,
  Ziel und seinen Verlauf mit. Ein **bekannter** Befund wird nie überschrieben: Weicht sein Status ab, gibt es nur
  einen Hinweis, und Verlaufseinträge der Datei werden nicht ergänzt – den Status setzt hier der Mensch. Alte Fassungen
  mit unbekannter Kennung werden hinzugefügt, auch zu einem bekannten Beitrag (sie sind nur Verlauf). Die Vorschau zählt
  „Befunde“, „Einträge im Verlauf der Befunde“ und „alte Fassungen von Beiträgen“ (`AddCounts.findings`,
  `finding_entries`, `post_versions`, auch in `total()`).
- **(h) Einfallskarte** (Runde 25): Eine Verbindung mit unbekannter Kennung wird hinzugefügt, wenn beide Einfälle zum Werk hier gehören
  (auch gerade eingelesene); fehlt einer, ist das nur ein Hinweis. Gibt es dasselbe Paar
  mit derselben Art hier schon unter anderer Kennung (aktiv oder gelöscht; `widerspricht` in beiden Richtungen), kommt es
  nicht doppelt (Hinweis). Eine bekannte Kennung wird nie überschrieben (andere Notiz, anderer Stand „gelöscht“:
  Hinweis). Eine Lage kommt nur, wenn es hier für (Werk, Einfall) keine gibt – **lokal gewinnt**; eine bekannte
  Lage bleibt, wie sie hier ist, ohne Hinweis (reine Anzeige). Die Vorschau zählt „Verbindungen der Gedankenkarte“
  und „Lagen auf der Gedankenkarte“ (`AddCounts.links`, `AddCounts.positions`, auch in `total()`).
- **Grabsteine:** Was hier einen Grabstein hat (`uuid_tombstone`), wird nicht neu angelegt und steht in der
  Vorschau unter „Hier gelöscht“ – auch Verbindungen, Lagen, Umschlag-Blöcke, Befunde, ihre Verlaufseinträge und alte
  Beitragsfassungen (ein zurückgesetztes Layout kommt so nicht
  durch die Datei der Gegenseite zurück). Was die Datei als gelöscht meldet und hier lebt, bleibt (Hinweis).
- **Herkunft der Anträge:** Mensch „Austausch: <Instanz>“, Notiz „Austausch aus Instanz <uuid>, Datei <sha256>“.

**Abgeleitete Kennung.** `derive(original, text)` = die ersten 16 Bytes von
`SHA-256("baruch-fassung:" ‖ original ‖ ":" ‖ sha256_hex(text))`, darin Version 8 (Byte 6 = `0x8_`) und Variante
10 (Byte 8 = `0b10xx_xxxx`), kanonisch in Kleinbuchstaben als 8-4-4-4-12 (`baruch_store::exchange_apply::derive_uuid`).
Beispiel: Original `26000000-0000-4000-8000-00000000006f`, Text `Ein Text` → die Hexziffern 1–12, 14–16 und
18–32 (ohne Bindestriche) sind die des Hashs, Ziffer 13 ist `8`, Ziffer 17 eine von `8 9 a b`.

**Beispiel.** Beide Rechner haben das Werk aus derselben Datei; „Das Rad“ hat dort die Fassungen 1 (aktuell)
und 2. Auf Rechner B ändert der Mensch den Text der Fassung 1 und zählt „Die Speichen“ ohne Nummer; B exportiert, A
liest mit „abgleichen“ ein: A erhält Fassung 3 (abgelöst, Basis 1, Kennung `derive(<Kennung von Fassung 1>,
<Text von B>)`, Notiz mit dem Zusatz), Fassung 1 bleibt aktuell, dazu ein Antrag `„Die Speichen“ ohne Nummer zählen`
von „Austausch: B“. Ein zweiter Durchgang mit derselben Datei fügt nichts hinzu und stellt keinen Antrag.
Benennt der Mensch auf B einen Punkt in der Gliederung um, entsteht dort eine neue Fassung mit der neuen Überschrift;
auf A kommt sie als abgelöste Fassung an, und statt eines Antrags steht dort ein Hinweis „Umschalten
genügt“ (Regel b).

**Hin und zurück.** Bearbeitet B eine gemeinsame Fassung X an Ort und Stelle (Text ändern statt neuer Fassung),
erhält A beim Abgleich die abgeleitete Fassung D(X, neu), B auf dem Rückweg D(X, alt) – so hat jede Seite beide
Texte, nichts geht verloren. Weitere Kopien entstehen nicht: D(X, neu) hat auf B denselben Text wie X und wird dort
übersprungen, ebenso D(X, alt) auf A. Danach wird nichts mehr hinzugefügt (Test
`merging_back_and_forth_after_an_in_place_edit_creates_no_duplicate_texts`). Das Versprechen „nichts doppelt“ gilt somit
je Kennung **und** je Text eines Punkts.

**Ältere Datei.** Der Abgleich kennt keine gemeinsame Basis: Jeder Unterschied der Gliederung zählt, als hätte
die Gegenseite ihn geändert. Liest man eine **ältere** Datei ein (etwa den eigenen Export von gestern), entstehen daher Anträge,
die eigene Änderungen an Nummer, Ebene, Stelle oder Titel rückgängig machen (Titel nur, wenn keine Fassung den
alten Titel trägt). Es sind nur Anträge – ohne Bestätigung ändert sich nichts; im Zweifel ablehnen. Fassungen,
Einfälle, Beiträge und Bilder sind nicht betroffen (sie werden nur hinzugefügt).

Offen (nach Runde 26): Abgleich direkt über das Netz (Nutzerfrage 2); „als Kopie einlesen“ (neue Kennungen) gibt es noch
nicht. (Erledigt in Runde 22: Ein neuer Teil, der Kapitel übernähme, wandert im Antrag mit seiner Kennung und seinen Fassungen.)

## 5 Sicherheit und Grenzen {#5-security-and-limits}

Bei **jedem** Fehler wird nichts geschrieben (keine Zeile, kein Antrag, kein Commit, keine eingehende Datei). Code:
`baruch-exchange/src/read/` (Tests `crates/baruch-exchange/tests/read.rs`).

**XML** (`read/dom.rs`):
- Keine DTD: `<!DOCTYPE …>` → „Unsicheres XML“ (so wirken weder „Billion Laughs“ noch externe Entitäten;
  nichts wird aufgelöst oder geladen). Keine Verarbeitungsanweisung außer der XML-Deklaration. Nur die fünf
  vordefinierten Entitäten (`&lt; &gt; &amp; &apos; &quot;`) und Zeichenverweise; jeder andere Verweis →
  „Unsicheres XML“. In XML 1.0 verbotene Zeichen (auch als `&#1;`) → XML-Fehler. Nur UTF-8.
- Text exakt: nie gekürzt; Zeilenenden wie in XML 1.0 (`\r\n`, `\r` → `\n`; ein echtes `\r` steht als `&#13;`).
  In Attributwerten werden Tabulator, Zeilenumbruch und Wagenrücklauf zu Leerzeichen; als Zeichenverweise bleiben sie.
  `kodierung="base64"` an einem Textelement wird dekodiert (UTF-8), `<daten>` ist Base64 (Leerraum erlaubt).
- Der Namensraum `urn:baruch:werk:1` ist Pflicht; ein anderer `urn:baruch:…` → „Version … wird nicht unterstützt“,
  ein fremder oder fehlender → „Keine Austauschdatei“. Eine höhere Unterversion (`version="3"`) wird gelesen;
  die Vorschau sagt es, und Unbekanntes steht als Pfad in `unknown` (`werk/angaben/farbe`, Attribute als
  `…/@name`, Text als `…/#text`, fremde Namensräume als `{ns}name`, ein unbekanntes Kind der Karte als
  `werk/karte/<name>`). Nichts davon wird übernommen. Der Leser liest `<karte>` erst ab `version="2"`; in einer
  Datei mit `version="1"` steht sie als `werk/karte` in `unknown`. Ebenso `<satzbild>`, `<verlag-anschrift>`,
  `<druckerei-anschrift>` und `<umschlag>` erst ab `version="3"` (davor `werk/angaben/satzbild`,
  `werk/buch/verlag-anschrift` … in `unknown`), `<befunde>`, `geaendert` und `<alte-fassung>` an Beiträgen erst ab
  `version="4"` (davor `werk/befunde`, `…/beitrag/@geaendert`, `…/beitrag/alte-fassung` in `unknown`).
- Fehlende Pflichtelemente und -attribute → XML-Fehler mit Pfad (z. B. „Pflichtelement fehlt: werk/angaben/titel“).
- Danach `validate`: kanonische UUIDs, keine Kennung doppelt (auch kein Grabstein für ein vorhandenes Stück),
  Verweise (Basis im selben Punkt oder Bild mit kleinerer Nummer, Verweise, Umschlag), je Punkt und Bild genau eine
  aktuelle Fassung, gültige Ebenen, Prüfsumme der Bildbytes; Karte: Verbindungen und Lagen nur zu Einfällen des
  Dokuments, kein Selbstbezug, kein Paar mit derselben Art doppelt, Notiz höchstens 500 Zeichen ohne Steuerzeichen
  außer Zeilenumbruch, je Einfall höchstens eine Lage, Koordinaten endlich (sonst XML-Fehler) und höchstens
  ±1 000 000; Umschlag (ab Unterversion 3): höchstens ein Block je Art, Grenzen wie oben, `richtung` genau am
  Rücken, `<text>` nur bei eigenem Text, Schriftnamen nach der Regel der Satzbild-Schriften, Anschriften höchstens 500
  Zeichen; Befunde (ab Unterversion 4): Punkt, Beitrag und Faden-Beitrag aus dem Dokument (nie Beitrag und Faden-Beitrag
  zugleich), `gleicht` auf einen anderen Befund des Dokuments, Grenzen wie die CHECKs der Migration 0046 (Text nicht
  leer und höchstens 4000, Zitat 2000, Prüfvermerk 200, Begründung und Notiz 2000, Persona 1–200 Zeichen),
  „verworfen“ nur mit Begründung, „übernommen“ nur mit Fassung, „verschoben“ mit genau einem Ziel – für den Befund und
  jeden Verlaufseintrag; ohne Vorschlag setzt nur der Mensch, eine Entscheidung nur bei einem Vorschlag und genau mit
  `entschieden`; alte Fassungen mit eigener Kennung.

**ZIP** (`read/archive.rs`):
- Erster Eintrag `mimetype` am Dateianfang, unkomprimiert, genau `application/vnd.baruch.werk+zip`.
- Namen nur `mimetype`, `werk.xml`, `MANIFEST.xml`, `bilder/<uuid>.<png|jpg|gif|webp|svg>`; abgewiesen werden
  absolute Pfade, `..`, Rückwärtsstrich, Laufwerk, Steuerzeichen, Duplikate (das zentrale Verzeichnis wird selbst gezählt),
  Links, Ordner, verschlüsselte Einträge und jede andere Kompression als STORED und DEFLATE.
- Das Entpacken nutzt `take(Grenze + 1)`; den Größen im Kopf wird nicht vertraut (ZIP-Bombe, lügende Kopfgröße).
  Grenzen je Eintrag, für `werk.xml` und insgesamt.
- Jeder Eintrag außer `mimetype` und dem Manifest steht im Manifest und umgekehrt; SHA-256 und Bytezahl müssen
  stimmen („Prüfsumme stimmt nicht: <Eintrag>“). Jede Bildfassung mit `datei` nennt genau ihren Eintrag
  `bilder/<fassungs-uuid>.<ext>`; ein Bild im Archiv ohne Fassung wird abgewiesen.

**Grenzen** (`baruch_exchange::Limits::DEFAULT`):

| Grenze | Voreinstellung |
|---|---|
| ganze Datei | 128 MiB |
| entpackt insgesamt | 256 MiB |
| ein Eintrag entpackt | 64 MiB |
| Einträge im ZIP | 4096 |
| `werk.xml` oder `.baruch.xml` | 64 MiB |
| Tiefe der Elemente | 32 |
| Elemente | 500 000 |
| Attribute je Element | 32 |
| Bytes je Text- oder Attributwert | 16 MiB |

Wege: Formular bis 128 MiB + 1 MiB (`UPLOAD_LIMIT`, darüber 413), RPC bis `works::RPC_BODY_LIMIT` (32 MiB +
64 KiB als Base64, also etwa eine 24-MiB-Datei), Kommandozeile bis 128 MiB. Die eingehende Datei zwischen Vorschau und
„Übernehmen“ liegt unter `<data>/austausch-eingang/<sha256>` (nur Namen aus 64 Hexzeichen), wird nach dem
Übernehmen – auch nach einer Abweisung – gelöscht und nach 24 Stunden weggeräumt.

## 6 Unterversionen und Erweiterung {#6-minor-versions-and-extension}

- **Unterversion statt neuem Namensraum:** Kompatible Erweiterungen (neue optionale Elemente oder Attribute)
  zählen `version` hoch, der Namensraum `urn:baruch:werk:1` bleibt. Nur eine inkompatible Änderung bekäme einen
  neuen Namensraum (`urn:baruch:werk:2`), den ein Leser von heute abweist.
- **Unterversion 2 (Runde 25, Einfallskarte)** füllt die in Unterversion 1 vorgesehene `<karte>` am Ende von
  `<werk>` mit Verbindungen **zwischen Einfällen** und den Lagen von Einfällen (§2). Berichtigung gegenüber der
  Ankündigung aus Runde 26: Verbindungen zu Punkten oder Beiträgen gibt es nicht; ein Einfall hängt über `bezug` an seinem Punkt.
  Dazu die Grabstein-Arten `verbindung` und `lage`. Schema: `docs/schema/baruch-werk-1-u2.xsd`
  (`<karte>` fest statt locker, `GrabsteinArt` erweitert); `docs/schema/baruch-werk-1.xsd` bleibt für
  Unterversion 1 unverändert. Wegen der neuen Grabstein-Arten besteht eine Ausgabe mit Karte nicht immer das
  eingefrorene Schema v1, wohl aber das Schema u2.
- **Unterversion 3 (Runde 27, Umschlag-Editor und Impressum)** bringt in `<buch>` nach `<vita>` die Anschriften
  `<verlag-anschrift>` und `<druckerei-anschrift>` (Pflichtangaben im Impressum) und `<umschlag>` mit den Blöcken des
  Umschlag-Layouts (§2), in `<angaben>` nach `<sprechdauer>` `<satzbild/>` mit den Satzbild-Einstellungen (Punkt
  aus „Nach Runde 18 offen“) und die Grabstein-Art `umschlag-block`. Das alles sind neue optionale Elemente am Ende ihrer
  Folge. Schema: `docs/schema/baruch-werk-1-u3.xsd`; `baruch-werk-1-u2.xsd` und `baruch-werk-1.xsd` bleiben
  unverändert. Eine Ausgabe der Unterversion 3 besteht die älteren Schemata nicht (neue Elemente), eine Datei der
  Unterversion 1 oder 2 besteht das Schema u3.
- **Unterversion 4 (Runde 28, Befunde)** bringt `<befunde>` am Ende von `<werk>` (Befunde mit Statusverlauf), an
  `<beitrag>` (Abschnitt und Faden) `geaendert` und `<alte-fassung>*` am Ende sowie die Grabstein-Arten `befund`,
  `befund-status`, `beitrag-fassung`. Das alles sind neue optionale Elemente oder Attribute am Ende ihrer Folge. Schema:
  `docs/schema/baruch-werk-1-u4.xsd`; die Schemata der Unterversionen 1 bis 3 bleiben unverändert. Eine Ausgabe mit
  Befunden oder bearbeiteten Beiträgen besteht die älteren Schemata nicht, eine Datei der Unterversionen 1 bis 3
  besteht das Schema u4.
- **Unterversion 5 (Runde 30, Lösungen, Varianten, Chat)** bringt (keine neue Grabstein-Art):
  - an `<befund>` nach dem Statusverlauf `<loesung zitat="…" ersatz="…" schritt="…"/>*` (jedes Attribut optional; eine
    Lösung braucht ein Zitat mit Ersatz oder einen Schritt, was der Leser prüft);
  - an `<fassung>` einer Variante `variante-gruppe` (UUID), `variante` (A, B, C) und `variante-zeile` (höchstens 300
    Zeichen) und nach `<text>` je umgesetztem Befund `<setzt-um befund="uuid"/>` (nur Befunde des Dokuments; gelöschte
    entfallen beim Schreiben);
  - an `<punkt>` `schluessel` – der feste Schlüssel des Punkts in der exportierenden Instanz. Er wird gebraucht, weil
    Querverweise im Text `#link(<kN>)` auf diesen Schlüssel zeigen (Runde 30a); beim Einlesen schreibt Baruch sie über
    `baruch_typst::xref::remap_keys` auf die Schlüssel hier um (Zuordnung `schluessel` → Kennung → Schlüssel hier,
    nur in Fassungen, die das Einlesen anlegt). Annahme gegenüber dem Vertrag R30 §4.3, der `schluessel` nicht nennt:
    Ohne ihn ließe sich §3.3 „Austausch u5 bildet die Schlüssel um“ nicht erfüllen. Folge: Zwei Ausgaben desselben
    Werks aus zwei Instanzen unterscheiden sich außer in `<ausgabe>` auch in `schluessel`;
  - an `<beitrag>` (Abschnitt und Faden) `grundlage-fassung` (UUID der geprüften Fassung desselben Punkts; im Faden
    nie), `grundlage` (Kopfzeile „geprüft: …“, höchstens 300 Zeichen), `antwort-auf` und `wiederholt` (UUIDs eines
    anderen Beitrags an derselben Stelle; ein Ziel an anderer Stelle, etwa nach dem Verschieben, entfällt beim Schreiben)
    und `relay-von` (Persona, 1 bis 200 Zeichen).
  Der Auftrag eines Beitrags (`request`) und das Protokoll `replace_batch` bleiben lokal. Beim Einlesen setzt ein
  Nachlauf diese Teile nur an den Zeilen, die das Einlesen selbst angelegt hat (`exchange_apply::apply_u5`):
  Bekannte Fassungen, Beiträge und Befunde bleiben beim Abgleich unverändert, auch wenn die Datei andere Werte trägt;
  eine abweichende Fassung, die eine abgeleitete Kennung erhält, bleibt ohne Variante. Schema:
  `docs/schema/baruch-werk-1-u5.xsd`; die Schemata der Unterversionen 1 bis 4 bleiben unverändert (SHA-256 im Test).
- **Dieser Leser** (Unterversion 5) liest die Teile der Unterversion 5 erst ab 5 (in 1 bis 4 werden sie als
  `werk/befunde/befund/loesung`, `…/punkt/@schluessel`, `…/fassung/@variante…`, `…/fassung/setzt-um`,
  `…/beitrag/@grundlage…`, `@antwort-auf`, `@wiederholt`, `@relay-von` gemeldet und nicht übernommen; Test
  `files_of_version_four_leave_the_u5_parts_unknown_and_still_read`); eine Ausgabe der Unterversion 4 liest er ohne
  Hinweis. Ein Leser der Unterversion 4 (Runde 28) liest eine Datei der Unterversion 5 wie eine höhere Unterversion:
  Lösungen, Varianten, Schlüssel und Beitragsmetadaten gehen dabei verloren, sonst nichts.
- **Der Leser der Unterversion 4** las `version="1"` bis `"3"` wie bisher und die Teile der Unterversion 4 erst ab
  4 (in 1 bis 3 als `werk/befunde`, `…/beitrag/@geaendert`, `…/beitrag/alte-fassung` gemeldet und nicht übernommen;
  die Grabsteine `befund`, `befund-status`, `beitrag-fassung` sind dort ein Fehler; Test
  `files_of_version_three_leave_findings_unknown_and_refuse_their_tombstones`). Für die älteren Teile: `<karte>` erst ab 2 (in 1 als
  `werk/karte` gemeldet), die Teile der Unterversion 3 erst ab 3 (in 1 und 2 als `werk/angaben/satzbild`,
  `werk/buch/verlag-anschrift`, `werk/buch/druckerei-anschrift`, `werk/buch/umschlag` gemeldet und nicht übernommen);
  ein Grabstein einer Art, die die Unterversion der Datei noch nicht kennt (`verbindung`/`lage` in 1,
  `umschlag-block` in 1 oder 2), ist ein Fehler. Eine Ausgabe der Unterversion 2 ohne die neuen Teile liest er ohne
  Hinweis (Test `files_of_version_two_leave_the_new_parts_unknown_and_refuse_the_new_tombstone`). Eine noch höhere
  Unterversion (`version="5"`) liest er wie 4, meldet Unbekanntes (z. B. `werk/karte/kante`) als Hinweis und
  übernimmt nichts davon (§4).
- **Neue Aufzählungswerte** sind keine kompatible Erweiterung – mit einer Ausnahme: eine neue Art von
  Grabstein (`<eintrag art="…">`). Dieser Leser meldet eine ihm unbekannte Art bei `version` > 4 als unbekannt
  (`werk/geloescht/eintrag/@art=„wolke“`) und überspringt nur diesen Grabstein; in `version="1"` bis `"4"`
  bleibt ein unbekannter Wert ein Fehler (Test
  `an_unknown_tombstone_kind_is_skipped_in_a_newer_minor_version_but_refused_in_version_one`). Jeder andere neue
  Wert (z. B. eine andere Art von Verbindung) braucht ein neues Element oder Attribut, das ein älterer Leser als
  unbekannt meldet, statt einen vorhandenen Wert zu erweitern.
- **Ein Leser der Unterversion 1** (Runde 26) liest eine Datei der Unterversion 2: Er meldet `werk/karte` als
  unbekannt und überspringt die Grabsteine `verbindung` und `lage` (§6 dort); Werk, Fassungen, Einfälle und Bilder übernimmt er
  wie bisher. Die Karte geht dabei verloren, sonst nichts.
- **Ein Leser der Unterversion 2** (Runde 25/26) liest eine Datei der Unterversion 3 wie eine höhere Unterversion: Er
  meldet `werk/angaben/satzbild`, `werk/buch/verlag-anschrift`, `werk/buch/druckerei-anschrift` und
  `werk/buch/umschlag` als unbekannt und überspringt die Grabsteine `umschlag-block`; alles andere übernimmt er wie
  bisher. Umschlag-Layout, Anschriften und Satzbild gehen dabei verloren, sonst nichts.
- **Ein Leser der Unterversion 3** (Runde 27) liest eine Datei der Unterversion 4 wie eine höhere Unterversion: Er
  meldet `werk/befunde`, `…/beitrag/@geaendert` und `…/beitrag/alte-fassung` als unbekannt und überspringt die
  Grabsteine `befund`, `befund-status`, `beitrag-fassung`; Befunde und alte Beitragsfassungen gehen dabei verloren,
  sonst nichts (der Text eines bearbeiteten Beitrags ist der gegenwärtige).
- **Der Schreiber** schreibt immer seine eigene Unterversion (heute `version="6"`, auch im Manifest; bis Runde 30 `5`) – auch für ein
  Dokument, das aus einer höheren Unterversion gelesen wurde; was er nicht kennt, steht nicht im Modell und wird
  nicht mitgeschrieben.
- Neue Elemente stehen am Ende ihrer Folge oder als optionale Elemente, damit ältere Leser die Reihenfolge der
  bekannten Kinder behalten.

## Satzeinstellungen (Unterversion 6, Runde 29a) {#typesetting-settings-minor-version-6-round-29a}

Eigner: satz-daten. Seit Runde 29 wandern die Satzeinstellungen eines Werks mit (Tabelle `work_typeset`, Migration
0048; Datenmodell `baruch_typst::typeset::TypesetSettings`). Unterversion **6**: Der Schreiber schreibt `version="6"`
(auch im Manifest; das ersetzt „heute `version="5"`“ in §6), XSD `docs/schema/baruch-werk-1-u6.xsd`. Die Schemata
der Unterversionen 1 bis 5 bleiben Byte-unverändert (SHA-256 im Test `the_frozen_v1_and_u2_schemas_are_byte_unchanged`);
eine Datei der Unterversion 5 besteht u6 und u5, eine Ausgabe mit `<satz>` besteht u5 nicht.

**Element.** In `<angaben>` als letztes Kind hinter `<satzbild/>`, nur wenn das Werk Satzeinstellungen hat:

```xml
<satz uuid="…" geaendert="2026-11-09T10:00:00.000Z" geaendert-von="Autor"
      kapitelanfang="rechte-seite" teil-seite="rechts" teil-rueckseite-leer="true" seitenformat="a5"
      rand-innen-mm="20" rand-aussen-mm="15" rand-oben-mm="18" rand-unten-mm="22" bundsteg-mm="5"
      grundschrift="Libertinus Serif" schriftgroesse-pt="10.5" zeilenabstand-em="0.65" absatz="einzug" absatz-em="1"
      blocksatz="true" silbentrennung="true" sprache="de" seitenzahlen="unten-aussen"
      seitenzahlen-beginn="erstes-kapitel" kolumnentitel="kapitel" fussnoten="arabisch" fussnoten-zaehlung="je-kapitel"
      schusterjungen-hurenkinder="true" verweis-seitenzahl="true"/>
```

Pflicht sind `uuid` (Kennung der Zeile; bleibt bei jeder Änderung gleich), `geaendert` (Zeit der letzten
Änderung, wie gespeichert) und `geaendert-von` (Name des Menschen, darf leer sein). Jedes Feld ist ein optionales
Attribut; **fehlt es, gilt die Werkvorlage** (wie `None` im Modell). Vokabular (Wörter aus
`baruch_exchange::typeset`, Aufzählungen im XSD):

| Feld (JSON) | Attribut(e) | Werte |
| --- | --- | --- |
| `chapter_start` | `kapitelanfang` | `fortlaufend`, `neue-seite`, `rechte-seite` |
| `part_page` | `teil-seite` + `teil-rueckseite-leer` | `rechts`, `beliebig` + `true`/`false` |
| `page_format` | `seitenformat` (+ `seite-breite-mm`, `seite-hoehe-mm` nur bei `eigenes`) | `a5`, `120x190`, `135x215`, `a4`, `eigenes` |
| `margins` | `rand-innen-mm`, `rand-aussen-mm`, `rand-oben-mm`, `rand-unten-mm`, `bundsteg-mm` | Zahlen (mm) |
| `font` | `grundschrift` | Familienname |
| `font_size_pt` | `schriftgroesse-pt` | Zahl (pt) |
| `leading_em` | `zeilenabstand-em` | Zahl (em) |
| `paragraph` | `absatz` + `absatz-em` | `einzug`, `abstand` + Zahl (em) |
| `justify` | `blocksatz` | `true`/`false` |
| `hyphenate` | `silbentrennung` | `true`/`false` |
| `lang` | `sprache` | `de`, `en`, `de-CH` … (`[a-z]{2,3}(-[A-Z]{2})?`) |
| `page_numbers` | `seitenzahlen` + `seitenzahlen-beginn` | `unten-mitte`, `unten-aussen`, `oben-aussen`, `keine` + `erste-seite`, `erstes-kapitel` |
| `running_head` | `kolumnentitel` | `aus`, `werktitel`, `kapitel` |
| `footnotes` | `fussnoten` + `fussnoten-zaehlung` | `arabisch`, `zeichen`, `buchstaben` + `fortlaufend`, `je-kapitel` |
| `widows_orphans` | `schusterjungen-hurenkinder` | `true`/`false` |
| `xref_with_page` | `verweis-seitenzahl` | `true`/`false` |

Zahlen stehen in der kürzesten exakten Form (`f64::to_string`: `20`, `10.5`). Zusammengesetzte Felder stehen ganz
oder gar nicht da; der Leser weist eine halbe Angabe ab (etwa vier der fünf Ränder, `teil-seite` ohne `teil-rueckseite-leer`, Maße ohne
`seitenformat="eigenes"`) und ein fremdes Wort mit dem Pfad (`werk/angaben/satz/@bundsteg-mm: …`,
`ExchangeError::Xml`), damit kein Feld stillschweigend verloren geht. Die Grenzen (Ränder 5–80 mm, Schriftgröße 6–24 pt …)
prüft nicht der Leser, sondern das Einlesen mit `TypesetSettings::validate` (nach `normalized`).

**Grabstein.** Zurückgesetzte Satzeinstellungen hinterlassen einen Grabstein `<eintrag art="satz">` (Datenbank
`uuid_tombstone.kind = 'work_typeset'`, mit der Werk-UUID, auch wenn das Werk gelöscht wird, über den Kaskaden-Trigger).
Die Art gibt es erst ab Unterversion 6: In einer Datei mit `version="1"` bis `"5"` ist sie ein Fehler (wie
`umschlag-block` vor 3). Ein Leser der Unterversion 6 meldet `<satz>` in einer Datei der Unterversionen 1 bis 5 als
`werk/angaben/satz` und übernimmt es nicht; ein Leser der Unterversion 5 (Runde 30) liest eine Datei der
Unterversion 6 wie eine höhere Unterversion: Er meldet `werk/angaben/satz`, überspringt den Grabstein `satz`, und nur
die Satzeinstellungen gehen dabei verloren.

**Einlesen (Regel „ohne stillen Verlust“).**

- *Neu:* Die Satzeinstellungen entstehen mit Kennung, `geaendert` und `geaendert-von` aus der Datei; die Grabsteine
  `satz` gehen wie alle anderen in die Grabsteinliste. Im Bericht zählen sie als `added.typeset` (Stück `satz`).
- *Abgleichen:* Fehlen hier Satzeinstellungen und gibt es hier keinen Grabstein für ihre Kennung, kommen sie aus der
  Datei (das Setzen erreicht so die andere Instanz). Sonst **nie überschrieben**: Weichen sie ab (gleiche oder
  andere Kennung), nennt ein Hinweis (`kind: "satz"`, Aktion „Hinweis“) die abweichenden Felder mit ihren
  Bezeichnungen und trägt beide Stände vollständig als JSON (`here`, `there`) – die Werte der Datei gehen so nicht
  stillschweigend verloren; der Mensch übernimmt sie bei Bedarf in „Einstellungen → Werk → Satz“. Wurde ihre Kennung
  hier zurückgesetzt (Grabstein), stehen sie unter „hier gelöscht“ und kommen nicht zurück. Ein Grabstein `satz` der Datei
  lässt die Einstellungen hier stehen (Hinweis „In … gelöscht, hier vorhanden – bleibt“, wie bei allen Grabsteinen).
- Ungültige Werte (`TypesetSettings::validate`) werden nicht übernommen und stehen unter „Nicht übernommen“
  (`refusals`, „Satz-Einstellungen: Schriftgröße: 6 bis 24 pt – nicht übernommen“); der Rest des Werks kommt trotzdem.
- Belegt in `crates/baruch-store/tests/typeset.rs` (`exchange_carries_the_settings_and_merging_never_overwrites_silently`)
  und `crates/baruch-server/tests/round29_daten.rs` (`two_instances_exchange_set_change_and_reset_without_silent_loss`:
  Setzen, Ändern, Zurücksetzen über zwei oder drei Instanzen).

Der Regelblock „// Baruch: Satz“ im Export (`snapshot::head`) gehört nicht zum Austauschformat: Die Datei trägt die
Einstellungen, nicht die erzeugten Typst-Regeln; die Werkvorlage in `<vorlage>` bleibt ohne den Block.

## Glossar und eigene Register (Unterversion 7, Runde 31a) {#glossary-and-own-indexes-minor-version-7-round-31a}

Eigner: glossar-daten. Seit Runde 31 wandern das Glossar (Tabellen `glossary_term`, `glossary_variant`, Migration 0049)
und die eigenen Register (`index_register`) mit, dazu die Register-/Glossarfelder der Satzeinstellungen. Unterversion
**7**: Der Schreiber schreibt `version="7"` (auch im Manifest; das ersetzt „heute `version="6"`“ oben), XSD
`docs/schema/baruch-werk-1-u7.xsd`. Die Schemata der Unterversionen 1 bis 6 bleiben Byte-unverändert (SHA-256 im Test,
u6 neu aufgenommen); eine Datei der Unterversion 6 besteht u7 und u6, eine Ausgabe mit `<glossar>` besteht u6 nicht.

**Elemente.** Am Ende von `<werk>` hinter `<befunde>`, nur wenn vorhanden:

```xml
<eigene-register>
  <register uuid="…" kennung="orte" titel="Orte im Hof" angelegt="2026-01-08T10:00:00.000Z"/>
</eigene-register>
<glossar>
  <begriff uuid="…" status="angenommen" herkunft="Hanna Probe" angelegt="…" geaendert="…" geaendert-von="Hanna Probe">
    <wort>Brunnenstube</wort>
    <kurz>Gemauerter Raum, in dem das Quellwasser gesammelt wird.</kurz>
    <ausfuehrlich>Im Hof liegt sie unter der Linde.…</ausfuehrlich>
    <quelle>frei erfunden</quelle>
    <variante>Brunnenstuben</variante>
    <bibelstelle>Gen 26,19</bibelstelle>
  </begriff>
  <begriff uuid="…" status="vorgeschlagen" herkunft="@lektor" angelegt="…"><wort>Zisterne</wort>…</begriff>
</glossar>
```

- `<register>`: Kennung `[a-z][a-z0-9-]{0,31}`, nie ein eingebautes (`stichwort`, `personen`, `bibel` – sie stehen nie
  in der Datei), je Werk eindeutig, höchstens 13; Titel 1–80 Zeichen. Die Reihenfolge der Elemente ist die Reihenfolge
  der Register.
- `<begriff>`: `status` `angenommen` oder `vorgeschlagen`, `herkunft` (Name des Menschen oder Kürzel der Persona oder
  des MCP-Clients), `angelegt`, optional `geaendert` und `geaendert-von`. `<wort>` ist Pflicht, `<kurz>`,
  `<ausfuehrlich>`, `<quelle>` nur wenn gesetzt; je Variante ein `<variante>`, je Bibelstelle ein `<bibelstelle>`
  (höchstens 32 je Begriff; Text wie gespeichert, also normalisiert). Alle Textelemente haben den Typ `Text` (Base64 bei
  verbotenen Zeichen). Unter den angenommenen Begriffen ist jeder Schlüssel (`term_key`: NFC, Leerraum, Kleinbuchstaben) nur
  einmal erlaubt (der Leser prüft das); ein Vorschlag darf denselben Schlüssel haben.
- `<satz>` trägt zusätzlich (fehlt = aus):

| Feld (JSON) | Attribut(e) | Werte |
| --- | --- | --- |
| `glossary_page` | `glossarseite` | `vor-registern`, `nach-registern` |
| `index_pages` | `registerseiten` + `registerseiten-spalten` | durch Leerzeichen getrennte Kennungen (1–16) + `1`–`3` |
| `glossary_first_mention` | `glossar-erste-erwaehnung` | `true`/`false` |

  `registerseiten` steht nur zusammen mit `registerseiten-spalten` (sonst Fehler mit Pfad); das Einlesen prüft die Werte
  mit `TypesetSettings::validate`.
- **Nicht im Format:** Registervorschläge (`index_proposal`) – sie sind ein Arbeitsablauf der Instanz (wie offene
  Anträge) und tragen keine Kennung. Registermarken im Text (`#metadata((…))<baruch-idx>`) wandern als Teil der Fassungen mit.

**Grabsteine.** Gelöschte oder verworfene Begriffe hinterlassen `<eintrag art="glossar-begriff">` (Datenbank
`glossary_term`), gelöschte eigene Register `<eintrag art="register">` (`index_register`), beide mit der Werk-UUID
(auch wenn das Werk gelöscht wird). Die Arten gibt es erst ab Unterversion 7: In einer Datei mit `version="1"` bis `"6"` sind sie
ein Fehler. Ein Leser der Unterversion 7 meldet in einer Datei der Unterversionen 1 bis 6 `werk/eigene-register`,
`werk/glossar` und die vier neuen Attribute von `<satz>` (`werk/angaben/satz/@glossarseite` …) als unbekannt und
übernimmt sie nicht (Test `files_of_version_six_leave_the_u7_parts_unknown_and_refuse_their_tombstones`). Ein Leser der
Unterversion 6 (Runde 29) liest eine Datei der Unterversion 7 wie eine höhere Unterversion: Er meldet diese Teile,
überspringt die neuen Grabsteine, und nur das Glossar, die eigenen Register und die neuen Satzfelder gehen dabei verloren.

**Einlesen.**

- *Neu:* Register und Begriffe (angenommene und vorgeschlagene) entstehen mit Kennung, Herkunft, Status und Zeiten aus der
  Datei; im Bericht `added.registers` und `added.glossary_terms` (Stücke `register`, `glossar-begriff`).
- *Abgleichen (zuordnen statt verdoppeln):* Eine bekannte Kennung wird **nie überschrieben** (Unterschied: Hinweis mit
  beiden Ständen). Eine unbekannte Kennung wird hinzugefügt – es sei denn, hier gibt es schon ein Register mit derselben Kennung
  (`kennung`) oder einen Begriff mit demselben Schlüssel (ein angenommener blockiert jeden anderen, ein Vorschlag blockiert einen anderen
  Vorschlag): dann ein Hinweis, kein Duplikat. Hat die Kennung hier einen Grabstein, steht sie unter „hier
  gelöscht“ und kommt nicht zurück; bei schon 13 eigenen Registern steht ein neues unter „Nicht übernommen“. Begriffe
  mit Feldern außerhalb der Längengrenzen (`MAX_GLOSSARY_*` je Begriff) stehen unter „Nicht übernommen“, der Rest
  kommt. Seit Runde 23 geprüft (siehe „Round 23 – Hardening“ am Ende; vorher offen, Review R31): die Zahl der Begriffe je Werk (`MAX_GLOSSARY_TERMS`) und die Länge
  eines Registertitels (`MAX_REGISTER_TITLE_CHARS`) – eine Datei kann beide überschreiten.
- Belegt in `crates/baruch-exchange/tests/{read,write,xsd}.rs`, `crates/baruch-store/tests/exchange_apply.rs`
  (`a_new_import_carries_registers_and_glossary_and_dumps_them_back`,
  `merging_the_glossary_matches_instead_of_doubling_and_never_overwrites`) und
  `crates/baruch-server/tests/round31_glossar.rs` (`two_instances_exchange_glossary_and_registers_without_doubling`).

## Fortschreibungen, Entwurf und Stand (Unterversion 8, Runde 22) {#amendments-draft-and-state-minor-version-8-round-22}

Eigner: spiegel-entwurf. Seit Runde 22 **schreibt** der Mensch seine eigene aktuelle Fassung beim Speichern im Editor **fort**,
statt jedes Mal eine neue Fassung anzulegen (Vertrag R22 §2.1; Tabelle `revision_amend`, Spalten
`revision.amend_seq`/`based_on_amend`, Migration 0050). Hinzu kommen der Entwurf als Satzeinstellung (Wasserzeichen
der Ausgaben) und der Stand, in dem ein Befund übernommen wurde. Unterversion **8**: Der Schreiber schreibt
`version="8"` (auch im Manifest; das ersetzt „heute `version="7"`“ oben), XSD `docs/schema/baruch-werk-1-u8.xsd`. Die
Schemata der Unterversionen 1 bis 7 bleiben Byte-unverändert (SHA-256 im Test, u7 neu aufgenommen); eine Datei der
Unterversion 7 besteht u8 und u7, eine Ausgabe mit Fortschreibungen oder einem Entwurf besteht u7 nicht.

**Fassung.** An `<fassung>` im Punkt zwei optionale Attribute und am Ende (hinter `<setzt-um>`) die Schritte:

```xml
<fassung uuid="…6f" nummer="1" status="aktuell" autor-art="mensch" autor="Hanna Probe" angelegt="…" stand="3">
  <text>…</text>
  <fortschreibung uuid="…898" seq="1" art="amend" autor="Hanna Probe" zeit="2026-01-05T10:00:00.000Z">
    <vorher>== Der Eimer

Erster Entwurf.
</vorher>
    <nachher>== Der Eimer

Zweiter Entwurf &amp; &lt;mehr&gt;.
</nachher>
  </fortschreibung>
  <fortschreibung uuid="…899" seq="2" art="undo" undo-von="1" autor="Hanna Probe" zeit="…">…</fortschreibung>
</fassung>
<fassung uuid="…71" nummer="3" status="vorschlag" autor-art="assistent" autor="@lektor" modell="…" basis="…6f" angelegt="…" beruht-stand="2">…</fassung>
```

- `stand` = `revision.amend_seq` (Zahl der Schritte; fehlt = 0, nie fortgeschrieben), `beruht-stand` =
  `revision.based_on_amend` (Stand der Basis, auf der ein Vorschlag beruht; fehlt = ohne). Baruch schreibt den Stand 0 (Basis nie
  fortgeschrieben) nicht – er bedeutet dasselbe wie „fehlt“; so bleiben Dateien ohne Fortschreibungen auch
  unter den älteren Schemata gültig (Integrator R22).
- `<fortschreibung>`: `uuid` (Kennung des Schritts), `seq` (ab 1, ohne Lücken), `art` (`amend` = Fortschreibung durch den
  Menschen, `undo` = Rückgängigmachen über den Verlauf; die Wörter nennt Vertrag R22 §2.11, gleich den Datenbankwerten),
  `undo-von` (nur und immer bei `undo`: der Schritt, dessen früherer Stand wiederhergestellt wurde, kleiner als `seq`),
  `autor` (Mensch), `zeit`; Kinder `<vorher>` und `<nachher>` (Text vor und nach dem Schritt, Typ `Text`, also Base64 bei
  verbotenen Zeichen).
- Der Leser prüft (`read/check.rs`): Kennungen im Dokument eindeutig, Schritte lückenlos ab 1, `undo-von` genau bei
  `undo` und kleiner als `seq`, `stand` gleich der Zahl der Schritte.

**Befund.** `<befund>` und `<status-eintrag>` tragen optional `stand` (`finding.adopted_amend` bzw.
`finding_status.adopted_amend`): Stand der Fassung, als der Befund übernommen wurde (Anzeige „Fassung n · Stand k“);
nur geschrieben, wenn größer als 0.

**Entwurf.** `<satz>` hat optional das Kind `<entwurf/>` (Satzeinstellung `draft`, Vertrag R22 §2.9):

```xml
<satz uuid="…" geaendert="…" geaendert-von="Hanna Probe" kapitelanfang="rechte-seite" …>
  <entwurf text="Probe &amp; &lt;Satz&gt;" zusatz="Stand &quot;heute&quot; · erfunden" datum="true" deckkraft="20" lage="fusszeile" umschlag="true"/>
</satz>
```

| Feld (JSON `draft`) | Attribut | Werte |
| --- | --- | --- |
| `text` | `text` (Pflicht) | 1–40 Zeichen, ohne Steuerzeichen |
| `note` | `zusatz` (fehlt = leer) | bis zu 120 Zeichen |
| `date` | `datum` (fehlt = `false`) | `true`/`false` – das Datum selbst steht nie in der Datei (Tag der Ausgabe) |
| `opacity_percent` | `deckkraft` (Pflicht) | 5–50 (Prozent) |
| `placement` | `lage` (Pflicht) | `diagonal`, `kopfzeile`, `fusszeile` |
| `cover` | `umschlag` (fehlt = `false`) | `true`/`false` (Wasserzeichen auch auf dem Umschlag-PDF) |

Ohne Entwurf bleibt `<satz/>` leer wie in Unterversion 7. Wie bei allen Satzeinstellungen prüft die Grenzen nicht der
Leser, sondern das Einlesen (`TypesetSettings::validate`, sonst „Nicht übernommen“). Die Typst-Regel des Wasserzeichens
steht nie in der Datei.

**Grabstein.** `<eintrag art="fortschreibung">` (Datenbank `uuid_tombstone.kind = 'revision_amend'`): Schritte werden
nie einzeln gelöscht, nur mit ihrer Fassung oder ihrem Werk (Kaskaden-Trigger der Migration 0050); dann
hinterlässt jeder Schritt einen Grabstein mit der Werk-UUID. Die Art gibt es erst ab Unterversion 8: In einer Datei mit `version="1"`
bis `"7"` ist sie ein Fehler.

**Ältere Leser.** Ein Leser der Unterversion 8 meldet in einer Datei der Unterversionen 1 bis 7
`…/fassung/fortschreibung`, `…/fassung/@stand`, `…/fassung/@beruht-stand`, `werk/angaben/satz/entwurf`,
`werk/befunde/befund/@stand` und `…/status-eintrag/@stand` als unbekannt und übernimmt sie nicht (Test
`files_of_version_seven_leave_the_u8_parts_unknown_and_refuse_the_amend_tombstone`). Ein Leser der Unterversion 7
(Runde 31) liest eine Datei der Unterversion 8 wie eine höhere Unterversion: Er meldet diese Teile, überspringt den
Grabstein `fortschreibung`, und nur Verlauf, Stände und Entwurf gehen dabei verloren – der Text jeder Fassung ist der
fortgeschriebene.

**Einlesen.**

- *Neu:* Fassungen kommen mit Stand, Stand der Basis und allen Schritten (Kennungen aus der Datei; Querverweise in
  `<vorher>`/`<nachher>` werden wie der Text der Fassung auf die Schlüssel hier umgeschrieben), Befunde und
  Verlaufseinträge mit ihrem Stand, die Satzeinstellungen mit dem Entwurf. Im Bericht zählen die Schritte als
  `added.amends` (Stück `fortschreibung`).
- *Abgleichen:* Eine neu eingelesene Fassung (Regel a, c) bringt ihren Verlauf mit; eine abweichende Fassung mit
  abgeleiteter Kennung (Regel d) bleibt ohne Verlauf. Eine bekannte Fassung wird nach Regel (d0) vorgespult oder bleibt,
  wie sie ist. Ist eine Kennung eines Schritts hier schon vergeben oder begraben, kommt der Verlauf dieser Fassung nicht
  mit (Hinweis), die Fassung selbst schon (Stand 0).
- Ein neuer Teil, der vorhandene Kapitel übernähme, wird ein Antrag mit Kennung und Fassungen (Regel a).
- Belegt in `crates/baruch-exchange/tests/{read,write,xsd}.rs` (`version_eight_carries_amends_stands_and_the_draft_exactly`,
  `validate_refuses_amends_with_gaps_wrong_undo_or_a_wrong_stand`, `a_u8_output_with_amends_draft_and_stands_passes_the_u8_schema_but_not_the_frozen_u7`),
  `crates/baruch-store/tests/exchange_u8.rs` (neu einlesen und zurück, Vorspulen, Abweichendes, Bestätigen und Ablehnen eines Antrags
  mit Kennung, „Fassungen“ nicht im Format) und `crates/baruch-server/tests/round22_abgleich.rs` (zwei
  Instanzen Byte-identisch bis auf `<ausgabe>` und Schlüssel, Bestätigung mit Spiegel-Commit und ein weiterer Abgleich).

## Runde 23 – Härtung (keine neue Unterversion) {#round-23--hardening-no-new-minor-version}

Das Format bleibt bei Unterversion 8; nur Regeln des Einlesens haben sich geändert.

- Zu Unterversion 7: „**Grenzen.** Der Titel eines eigenen Registers hat – in einer Zeile,
  Leerraum zusammengezogen – höchstens 80 Zeichen (`baruch_typst::index::MAX_REGISTER_TITLE_CHARS`); der
  Leser (`check_u7`) und das Einlesen (`exchange_apply::check_u7_limits`, `insert_register`) prüfen das. Ein Werk hat höchstens
  5000 Glossarbegriffe, Vorschläge eingerechnet (`baruch_store::MAX_GLOSSARY_TERMS`): Beim Einlesen als neues Werk zählt die
  Datei, beim Abgleich der Bestand hier zusammen mit den Begriffen, die hinzukämen (`insert_term`). Eine Überschreitung
  weist das ganze Einlesen mit einer deutschen Meldung ab; nichts bleibt geschrieben (eine Transaktion).“
- Regel (j), Satz zu bekannten Befunden: „Ein **bekannter** Befund wird nie überschrieben: Status, Begründung und
  Verlauf bleiben, wie sie hier sind – der Verlauf ist das Protokoll der Entscheidungen dieser Instanz, und ein offener
  Vorschlag aus der Datei würde hier sonst zu einer Entscheidung, die niemand getroffen hat. Weicht der Status ab oder
  hat die Datei Einträge im Verlauf, die hier fehlen, steht dort ein Hinweis, der ihre Zahl nennt.“
- Unterversion 5, letzter Satz zu Varianten: „… eine abweichende Fassung, die eine abgeleitete Kennung erhält, trägt
  die Variante der Datei (Gruppe, Bezeichnung, Zeile, Befunde) – seit Runde 23.“
- Umschlag (Regel zu Blöcken): „Blöcke werden in Normalform gespeichert und verglichen (`check_cover_block`); weist
  die Prüfung einen Block ab, wird er unverändert übernommen und als Hinweis gemeldet.“
- Unterversion 8, Vorspulen: „Vorgespult wird nur, wenn alle Überschriftzeilen des Textes (Zeilen mit `=` oder `#heading(` am Anfang) gleich bleiben;
  sonst kommt die Fassung der Datei wie vor Runde 22 als abweichende Fassung.“
- Neu unter „Hinweise beim Einlesen“: „Passt die Überschrift einer gerade eingelesenen aktuellen Fassung nicht zur Art
  ihres Punkts (Kapitel Ebene 1, Abschnitt 2, unter einem Abschnitt 3; Teile ungeprüft), meldet das Einlesen einen
  Hinweis; der Text bleibt exakt. `validate` selbst kennt keine Hinweise und bleibt unverändert.“

## Baum der Mindmap (Unterversion 9, Runde 34) {#tree-of-the-mind-map-minor-version-9-round-34}

Eigner: mindmap_daten. Seit Runde 34 bilden die Einfälle eines Werks einen Baum (die Mindmap; Migration 0052, Spalten
`idea.title`, `idea.parent_id`, `idea.position`, `idea.main_topic`). Unterversion **9**: Der Schreiber schreibt
`version="9"` (auch im Manifest; das ersetzt „`version="8"`“ oben), XSD `docs/schema/baruch-werk-1-u9.xsd`. Die
Schemata der Unterversionen 1 bis 8 bleiben Byte-unverändert (SHA-256 im Test, u8 neu aufgenommen); eine Datei der
Unterversion 8 besteht u9 und u8, eine Ausgabe mit einem Baum besteht u8 nicht.

**Einfall.** `<einfall>` trägt vier weitere optionale Attribute:

| Attribut | Bedeutung | fehlt = |
|---|---|---|
| `titel` | Titel des Knotens in der Mindmap, höchstens 200 Zeichen (`idea.title`) | leer |
| `eltern` | UUID des Elterneinfalls im selben Werk (`idea.parent_id`) | kein Elternknoten |
| `platz` | Reihenfolge unter Geschwistern, ganze Zahl (`idea.position`); bei Gleichstand nach Entstehung | 0 |
| `hauptthema` | `true`: freies Hauptthema direkt am Werk (`idea.main_topic`); nur ohne `eltern` und `bezug` | nein |

Der Schreiber gibt jedes der vier nur aus, wenn es von der Voreinstellung abweicht – ein Werk ohne Baum sieht in
Unterversion 9 aus wie in 8 (und besteht auch das Schema u8). Der Ort eines Einfalls folgt einer Regel: mit `eltern`
ein Unterknoten dieses Einfalls (`bezug` ist dann nur die Verknüpfung zum Abschnitt); sonst mit `bezug` am Kapitel des Punkts;
sonst mit `hauptthema` am Werk; sonst liegt er **in der Wolke** neben der Mindmap (Ablage für Einfälle ohne Platz).

```xml
<einfall uuid="26000000-0000-4000-8000-000000000029" status="erledigt" erledigt-als="beitrag" herkunft-art="persona" herkunft="@lektor" angelegt="2026-01-06T10:00:00.000Z" geaendert="2026-01-07T12:00:00.000Z" titel="Lampe am Seil" eltern="26000000-0000-4000-8000-000000000028" platz="2">
```

**Leser.** `read` liest die Attribute ab `version="9"`. In einer Datei der Unterversionen 1 bis 8 bleiben sie
ungelesen und stehen in `unknown` (`werk/einfaelle/einfall/@titel` …); alles andere der Datei wird gelesen. `validate`
prüft: Titel höchstens 200 Zeichen, `eltern` eine UUID und nicht der Einfall selbst. Ein `eltern`, das auf keinen
Einfall des Dokuments zeigt, ist **kein** Formatfehler.

**Einlesen** (neu und abgleichend) in zwei Schritten: zuerst alle Einfälle (mit Titel, Platz und Hauptthema), dann die
Elternknoten (`exchange_apply::link_idea_parents`) – der Elternknoten darf in der Datei hinter dem Kind stehen. Ein
Einfall wird **ohne Elternknoten** übernommen (er bleibt an seinem Bezug oder liegt in der Wolke), mit einem Hinweis
im Bericht, wenn

- der Elternknoten hier im Werk nicht existiert (unbekannte Kennung oder Einfall eines anderen Werks),
- der Elternknoten gelöscht ist, der Einfall selbst aber nicht (ein mit seinem Elternknoten gelöschter Unterknoten
  behält die Bindung, damit das Wiederherstellen ihn mitnimmt),
- die Kette der Elternknoten einen Kreis bildet oder tiefer als 32 Einfälle würde.

Beim **Abgleichen** werden neue Einfälle samt ihrem Baum hinzugefügt; ein hier schon bekannter Einfall bleibt, wie er ist
(lokal gewinnt) – weicht sein Titel ab, steht „Titel weicht ab (bleibt)“ als Hinweis im Bericht; Elternknoten und
Reihenfolge bekannter Einfälle zählen als Anordnung dieser Instanz und werden nicht verglichen. Der Titel steht im
Volltextindex der Einfälle (Spalte `words` von `idea_fts`), auch nach dem Einlesen.

**Ältere Leser.** Ein Leser der Unterversion 8 (0.9.0) liest eine Datei der Unterversion 9 wie jede höhere
Unterversion: Er meldet die vier Attribute als unbekannt und übernimmt die Einfälle ohne Baum.

Tests: `crates/baruch-exchange/tests/read.rs` (Abschnitt „Sub-version 9 (Round 34)“), `xsd.rs` (u9 besteht, u8 eingefroren),
`crates/baruch-store/tests/exchange_u9.rs` (Einlesen, Ausgeben, Lösen mit Hinweis, Datei der Unterversion 8),
`crates/baruch-server/tests/round34_daten.rs` (Rundlauf zwischen zwei Instanzen).

## Werksprache (Unterversion 10, Runde 38) {#work-language-minor-version-10-round-38}

Seit Runde 38 hat jedes Werk eine Werksprache (`de` oder `en`; Migration 0055, Spalte `work.language`). Personas
antworten darin; die Buchausgabe folgt ihr ab Runde 39. Unterversion **10**: Der Schreiber schreibt `version="10"`
(auch im Manifest; das ersetzt „`version="9"`“ oben), XSD `docs/schema/baruch-werk-1-u10.xsd`. Die Schemata
der Unterversionen 1 bis 9 bleiben Byte-identisch (`-u9.xsd` ist ab jetzt eingefroren).

**Attribut.** `<angaben>` trägt ein weiteres Attribut:

| Attribut | Bedeutung | fehlt = |
|---|---|---|
| `sprache` | Werksprache, `de` oder `en` (`work.language`) | `de` |

Der Schreiber schreibt `sprache` **immer**. Eine Datei ohne es – jede Datei bis Unterversion 9 – wird als `de` gelesen.
Ein ungültiger Wert ist ein Fehler des Lesers, wie ein ungültiger Wert der übrigen Aufzählungsattribute des
Formats (z. B. der Werkart): `…/@sprache: unbekannter Wert „fr“`; nichts wird eingelesen. Modell: `ExchangeLanguage { De, En }` (Voreinstellung `De`), Feld `ExchangeWork::language`.

Die Werksprache ist unabhängig von `<satz sprache>` (`TypesetSettings.lang`, die Sprache der Silbentrennung); beide
werden frühestens in Runde 39 gekoppelt.

**Einlesen.** Ein **neues** Werk übernimmt die Sprache der Datei. Beim **Abgleichen** in ein vorhandenes Werk wird die
Sprache wie das Feld `bible` (Bibelübersetzung) der Werkangaben behandelt: Der lokale Wert bleibt, und
weicht die Datei ab, steht im Bericht die Zeile „Werkangabe „Sprache“ weicht ab (bleibt)“.

**Ältere Leser.** Ein Leser der Unterversion 9 oder älter liest eine Datei der Unterversion 10 wie jede höhere Unterversion
(siehe die Regel zu höheren Unterversionen oben); er kennt das Attribut nicht, das Werk, das er anlegt, ist also
deutsch.

Tests: `crates/baruch-store/tests/exchange_u10.rs` (Datei der Unterversion 9 → `de`, Unterversion 10 hin und
zurück), `crates/baruch-exchange/tests/xsd.rs` (eingefrorene Schemata).

