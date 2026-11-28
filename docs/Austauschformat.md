# Exchange format

A work moves with all its revisions, ideas, posts, images, its idea map, the cover layout, the
typeset view settings, its findings, the typesetting settings and its glossary from one Baruch instance to another
(Round 26, map since Round 25, cover layout, addresses and typeset view since Round 27, findings and old
post revisions since Round 28, cross-references and variants since Round 30, typesetting settings since Round 29, glossary and
own indexes since Round 31, amendments, draft and state of adopted findings since Round 22, tree of the mind map
on the ideas since Round 34). Namespace
`urn:baruch:werk:1`, minor version 9; schema `docs/schema/baruch-werk-1-u9.xsd`. The schemas of minor versions 1 to
8 (`docs/schema/baruch-werk-1.xsd`, `baruch-werk-1-u2.xsd` … `baruch-werk-1-u8.xsd`) stay frozen (byte-identical,
tests in `crates/baruch-exchange/tests/xsd.rs`); every valid file of minor versions 1 to 8 is also valid under the
u9 schema.

> **Round 38:** the current minor version is **10** (schema `docs/schema/baruch-werk-1-u10.xsd`, the u9 schema is
> frozen as well); see "Work language (minor version 10, Round 38)" at the end. From Round 38 on, new sections of
> this document were written in English; since Round 38c the whole document is English.

## 1 Overview, container and what is not in the format

An exchange file carries **a whole work** from one Baruch instance to another (laptop ↔ computer at
home, backup for later, handing on to someone with their own Baruch). Every piece in it has a globally
unique identifier (UUID, §3); so the receiving instance recognises on reading in what it already knows (§4).

Exporting: page „Buch → Ausgabe → Austausch: Ausgeben“ (ribbon) (`/werke/{id}/austausch`), RPC `work.exchange_export`,
command line `baruch-server export ID|SLUG --baruch DATEI`, MCP `exchange_export` (read-only). Reading in only
by the human (page, RPC, command line), never over MCP.

**Two forms** with the same XML:

| Form | File | Media type | Images |
|---|---|---|---|
| Container (default) | `<slug>.baruch` | `application/vnd.baruch.werk+zip` | own files in the ZIP |
| XML only | `<slug>.baruch.xml` | `application/vnd.baruch.werk+xml` | Base64 in `<daten>` |

**Container `.baruch`** is a ZIP as with EPUB and ODF; the entries stand in a fixed order:

1. `mimetype` – uncompressed (STORED), content exactly `application/vnd.baruch.werk+zip` without a line end, without
   an extra field in the header. The type is thus readable in the file from byte 38 on, without unpacking.
2. `werk.xml` – the work (DEFLATE); every image revision points to its entry with `datei="bilder/…"`.
3. `bilder/<fassungs-uuid>.<ext>` – one file per image revision, `ext` ∈ `png|jpg|gif|webp|svg`; raster images
   STORED (already compressed), SVG DEFLATE. The bytes are exactly the bytes of the image.
4. `MANIFEST.xml` – last (DEFLATE): every entry except `mimetype` and the manifest itself with SHA-256
   (lower-case hex) and byte count.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<manifest xmlns="urn:baruch:werk:1" version="3">
  <datei pfad="werk.xml" sha256="3f0c…e1" bytes="18234"/>
  <datei pfad="bilder/26000000-0000-4000-8000-00000000003d.png" sha256="0b2e…82" bytes="70"/>
</manifest>
```

The output is **deterministic**: the same work in the same state yields byte-identical XML and ZIP (every
ZIP entry carries the fixed timestamp 1980-01-01 00:00:00 and the permissions 0644; only `<ausgabe zeit>`
changes from export to export).

**Not in the format** (deliberately):

- Keys, tokens and secrets; connections to services (Eliezer, MultiGPT, pgvector, LanceDB) – not to be
  confused with the connections of the idea map between ideas, which are in the format (`<karte>`).
- The **proximity** of the idea map (computed from embeddings and keywords, never stored).
- Personas with their models and role prompts – only their handles (`@lektor`) as origin. The model of a
  **revision** (`modell`) stands in it as the origin of the revision, not the setting of the persona.
- Open requests (outline and images), evidence of the web search (`web_evidence`), RAG assignments and the
  search index.
- Bible, commentary and literature texts: only module names (`<bibelmodul>`, `<quellen>`) and references in the text.
- Numeric ids, `slug` and the computed number of an item (`node.number`); the source of a Typst import.
- Since Round 30 (migration 0047) these stay local: the log of the work-wide replace (`replace_batch`, no uuid,
  like the sync states) and the job of a post (`post.request`/`chat_thread_post.request`, only for
  „wiederholen“). What travels along from minor version 5 on (solutions, variants, basis and links of posts,
  keys of the items) is governed by §6.
- Since minor version 3 (Round 27) the typeset view settings travel along (`<satzbild>`, §2); before that the
  work template held after reading in.
- Since minor version 6 (Round 29) the typesetting settings travel along (`<satz>`, section "Typesetting settings
  (minor version 6)"); the generated rule block „// Baruch: Satz“ itself is not in the file.
- Since Round 22: the setting **„Fassungen“** („Einstellungen → Werk → Fassungen“, table `work_revision_policy`:
  amending on/off, threshold of the big change, time limit) – it is a way of working of the instance like the open
  requests and does not travel along (test `the_revision_policy_is_not_in_the_exchange_format`). What of it becomes visible
  in the work does travel along, though: amendments and states (section "Amendments, draft and state
  (minor version 8)"). Likewise not in the format: the watermark as a Typst rule (only the setting `<entwurf/>`).

**User questions with a default** (2026-11-08, block nothing):

1. *Should the format also carry the author library (titles, quotations, highlights)?* – default: **no**,
   a format of its own later instead.
2. *Should the synchronisation later go directly between two instances over the network, without a file?* – default:
   **later**, with the same elements as here.

## 2 Elements

Since Round 29 the authoritative XSD is `docs/schema/baruch-werk-1-u6.xsd` (typesetting settings `<satz>`, tombstone kind
`satz`, see "Typesetting settings (minor version 6)" below; the schemas u1 to u5 are frozen). In Round 30
`docs/schema/baruch-werk-1-u5.xsd` held (global elements `<werk>` and `<manifest>`); outputs without `<satz>` are still checked in
older tests with `xmllint --noout --nonet --schema docs/schema/baruch-werk-1-u5.xsd DATEI` (test `the_u5_schema_accepts_a_u4_file_and_the_frozen_u4_schema_too`:
a file of minor version 4 passes u5 and u4). Until Round 28 `docs/schema/baruch-werk-1-u4.xsd` held
(`baruch-exchange/tests/xsd.rs`, server `tests/round26_export.rs`, `round26_roundtrip.rs`, `round28_befunde.rs`;
`round25_widget.rs`; `round27_daten.rs` checks against the u3 schema, its work has no findings). The
u4 schema also accepts every valid file of minor versions 1 to 3 (test
`the_u3_schema_accepts_every_valid_file_of_versions_one_and_two`, checks against u4 and u3 since Round 28).
The examples come from the invented sample work `baruch_exchange::sample` (shortened with `…`).

**General rules**

- UTF-8, XML 1.0, no DTD, no entities except `&amp; &lt; &gt; &quot; &apos;` and character references.
- Namespace `urn:baruch:werk:1` = major version; `version="3"` at `<werk>` and `<manifest>` = minor version
  (§6). A reader rejects a different namespace.
- The order of the children is fixed (`xs:sequence`); elements with `?` may be missing, with `*` occur any number of times,
  with `+` at least once.
- **Text is exact:** text always stands in an element without children; the reader never trims. Whitespace *between*
  elements (line break, indentation by two spaces) has no meaning. The writer escapes `&`, `<`,
  `>` (so `]]>` never stands in the text) and `\r` as `&#13;` (every parser would turn a literal `\r\n` into `\n`);
  in attributes additionally `"` and tab, line break, `\r` as `&#9;`, `&#10;`, `&#13;`.
- **Forbidden control characters** (in XML 1.0 inadmissible even as a reference, e.g. U+0001): a text element
  (XSD type `Text`) then stands entirely as Base64 of the UTF-8 bytes with `kodierung="base64"`; in an attribute that is
  an error on export that names the field (e.g. `werk/gliederung/punkt/fassung/@autor`).

  ```xml
  <text kodierung="base64">PT0gRGVyIEVpbWVyCgpEZXIgRWltZXIgAWtsaXJydCBsZWlzZSBhbSBSYW5kIGRlcyBCcnVubmVucy4K</text>
  ```

- Identifiers: UUID canonical lower case `8-4-4-4-12` (§3). Times as they are stored
  (`2026-01-05T09:00:00.000Z`, UTC). Booleans `true`/`false`.

**`<werk>`** – root. Attributes `version` (minor version, today `6`), `uuid` (work), `art` (`buch` | `predigt`).
Children in this order: `ausgabe`, `angaben`, `buch?`, `vorlage`, `gliederung`, `einfaelle?`, `faeden?`,
`bilder?`, `geloescht?`, `karte?`. Baruch does not write empty optional parts.

```xml
<werk xmlns="urn:baruch:werk:1" version="3" uuid="26000000-0000-4000-8000-000000000001" art="buch">
```

**`<ausgabe>`** – who exported when: `zeit`, `instanz` (UUID of the instance), `instanzname` (empty = without a
name, shown as „ohne Namen (26000000)“), `programm` (program and version).

```xml
<ausgabe zeit="2026-01-08T19:00:00.000Z" instanz="26000000-0000-4000-8000-000000000002" instanzname="Probeinstanz" programm="baruch 0.0.1"/>
```

**`<angaben>`** – work metadata: `<titel>`, `<untertitel>`, `<autor>`, `<zielgruppe>`, `<bibel>` (translation as
text; always present, also empty), `<angelegt>`, `<geaendert>?`, `<schema name>` (outline scheme `template`,
`decimal`, `decimal_dot`, `alphanumeric`, `chapter_roman`, `chapter_arabic`, `parts_chapters`, `chapter_words`,
`legal`, `sermon`, `book`, `none`, `custom`) with its own `<muster>?`, `<bibelmodul>?` (SWORD Bible of the work),
`<quellen>?` with `<quelle art="kommentar|lexikon">Modul</quelle>*`, `<sprechdauer woerter-je-minute
ziel-sekunden?/>?` (time keeper) and from minor version 3 on `<satzbild/>?` (typeset view settings of the work, table
`work_sheet`, Round 18c; only with at least one setting). Attributes of `<satzbild>`, each optional (missing =
from the work template), values as stored: `schrift`, `schrift-ueberschriften` (font lists), `groesse`,
`satzbreite` (length with unit `pt|mm|cm|em`), `farbe`, `farbe-ueberschriften`, `blattfarbe` (`#rrggbb`). On
reading in, Baruch checks the values as on saving (`SheetSettings::normalized`); what does not pass is not
adopted and stands under „Nicht übernommen“.

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

**`<buch>?`** – book metadata, only if the work has any. Attributes `titelbereich` (`oben` | `mitte` | `unten`),
`titelei`, `umschlagseiten` (booleans), `papier-mm`, `beschnitt-mm` (numbers), `umschlag?` and
`autorenfoto?` (UUID of the **image**, not of the revision), `geaendert?`. Children (text, always present): `<verlag>`,
`<ort>`, `<jahr>`, `<auflage>`, `<isbn>`, `<preis>`, `<reihe>`, `<druckerei>`, `<bibelvermerk>`,
`<bildnachweis>`, `<widmung>`, `<klappentext>`, `<vita>`; from minor version 3 on followed by `<verlag-anschrift>` and
`<druckerei-anschrift>` (multi-line, at most 500 characters; Baruch always writes both, if they are missing they count as
empty) and `<umschlag>?` with the cover layout (only if there are blocks). If a work has blocks but no
book metadata, `<buch>` stands there with the defaults (title at the top, without front matter, cover pages on, 0.1 mm paper, 3 mm
bleed).

**`<umschlag>`** (from minor version 3 on) with `<block>*` – at most one block per kind (contract Round 27). Attributes:
`uuid`, `art` (area and block, table below), `x-mm`, `y-mm` (top left corner of the block frame in mm from the
trim edge of its own area, without bleed, −50 to 1000; negative = in the bleed), `breite-mm` (width in
reading direction, 1 to 1000; on the spine the vertical extent), `ausrichtung` (`links` | `mitte` | `rechts` |
`blocksatz`), `groesse-pt` (4 to 200), `zeilenabstand-em` (0 to 3), `farbe` (`hell` = white | `dunkel` | `#rrggbb`
lower case), `schrift?` (family name of letters, digits, spaces, `- _ .`, at most 200 characters; missing = font
of the work template), `schnitt` (`normal` | `fett` | `kursiv`), `hintergrund` (`kein` | `schatten` | `verlauf`),
`sichtbar` (boolean), `richtung?` (only and always on the spine: `abwaerts` = readable from top to bottom,
`aufwaerts` = from bottom to top), `geaendert`. Child `<text>?` (own text, at most 1000 characters) only with the
kinds with their own text (free texts and motto); all others take their text from the work and the book metadata.

| `art` | Database (`cover_block.kind`) | Block |
|---|---|---|
| `vorderseite-titel`, `vorderseite-untertitel`, `vorderseite-autor`, `vorderseite-verlag`, `vorderseite-freitext` | `front_title`, `front_subtitle`, `front_author`, `front_publisher`, `front_free` | front: title, subtitle, author, publisher, free text (with `<text>`) |
| `ruecken-titel`, `ruecken-autor`, `ruecken-verlag`, `ruecken-freitext` | `spine_title`, `spine_author`, `spine_publisher`, `spine_free` | spine: title, author, publisher, free text (with `<text>`) |
| `rueckseite-motto`, `rueckseite-klappentext`, `rueckseite-vita`, `rueckseite-strichcode`, `rueckseite-verlag-preis`, `rueckseite-freitext` | `back_motto`, `back_blurb`, `back_bio`, `back_barcode`, `back_publisher_price`, `back_free` | back: motto/Bible verse (with `<text>`), blurb, short biography with photo, ISBN with barcode, publisher and price, free text (with `<text>`) |

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

What the XSD does not check, the reader checks in Rust (`read/check.rs`): at most one block per kind, `richtung` exactly on
the spine, `<text>` only with own text and at most 1000 characters, addresses at most 500 characters, no identifier
twice (also no tombstone for an existing block).

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

**`<vorlage>`** – work template: `<text>` (current text, `work.template`) and all revisions `<fassung uuid
nummer autor angelegt>` with `<notiz>?` (only if not empty) and `<text>`.

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

**`<gliederung>`** – the roots of the outline as `<punkt>*` in order.

**`<punkt uuid art nummeriert>`** – outline item: `art` `teil` | `kapitel` | `abschnitt`, `nummeriert`
(`false` = „ohne Nummer“; the number itself is computed on reading in). Children: `<titel>`, all revisions
`<fassung>+` by number, the posts of the section chat `<beitrag>*`, then the sub-items `<punkt>*`.

**`<fassung>` in the item** – attributes `uuid`, `nummer` (number in the exporting instance, with gaps after
deletions), `status` (`aktuell` | `vorschlag` | `verworfen` | `abgeloest`; exactly one `aktuell` per item),
`autor-art` (`mensch` | `assistent`), `autor` (name or persona handle), `modell?` (only with the assistant: model
of the revision as origin), `basis?` (UUID of the parent revision in the same item), `angelegt`, `geaendert?`. Children
`<notiz>?` and `<text>` (Typst text including the heading, exact).

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

**`<beitrag>` in the item** – post in the section chat: `uuid`, `position` (order from 1, without gaps – not
the internal position), `autor` (name, `@handle` or `@mcp:<client>`), `angelegt`, from minor version 4 on `geaendert?`
(time of the last edit, missing = never edited). Children `<text>`, `<quelle>*` and
`<bezug punkt fassung?>Zitat</bezug>*` (reference through the UUID of the item and, if the revision exists, its UUID),
from minor version 4 on followed by `<alte-fassung uuid gueltig-ab ersetzt bearbeitet-von>` with `<text>` per old revision,
oldest first (table `post_version`; `gueltig-ab` = since when this revision held, `ersetzt` = when it was
replaced, `bearbeitet-von` = who edited).

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

**`<einfaelle>?`** with `<einfall>*` – all ideas of the work, also deleted ones. Attributes `uuid`, `status`
(`offen` | `erledigt`), `erledigt-als?` (`gliederung` | `beitrag` | `persona` | `notiz`), `herkunft-art`
(`mensch` | `persona` | `mcp`), `herkunft` (name, `@handle`, `@mcp:<client>`), `angelegt`, `geaendert`,
`geloescht?` (softly deleted since), `bezug?` (UUID of the item), from minor version 9 on `titel?`, `eltern?`, `platz?`,
`hauptthema?` (tree of the mind map, see "Tree of the mind map (minor version 9, Round 34)"). Children `<schlagwort>*`, `<notiz>`, `<zitat>`,
`<quelle>`, `<bibelstelle>` (always present, also empty) and `<erledigt-ref>?` (free reference; a "key N" of the
database stands in it as the UUID of the item, the rest literally).

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

**`<faeden>?`** with `<faden uuid angelegt-von angelegt geaendert>*` – threads without a section: `<titel>` and
`<beitrag uuid position autor angelegt>*` with `<text>` and `<quelle>*` (`position` from 1); from minor version 4 on, as in the
item, `geaendert?` and `<alte-fassung>*` at the end.

```xml
<faden uuid="26000000-0000-4000-8000-000000000032" angelegt-von="@lektor" angelegt="2026-01-07T14:00:00.000Z" geaendert="2026-01-07T15:00:00.000Z">
  <titel>Fragen zum Brunnen</titel>
  <beitrag uuid="26000000-0000-4000-8000-000000000034" position="2" autor="Hanna Probe" angelegt="2026-01-07T15:00:00.000Z">
    <text>Sieben Meter, erfunden.</text>
    <quelle>Notizbuch</quelle>
  </beitrag>
</faden>
```

**`<bilder>?`** with `<bild uuid name angelegt>*` – images of the work: `<pfad>*` (paths under which the template and
texts name the image, sorted) and all revisions `<fassung>+`. Attributes of the image revision: `uuid`, `nummer`,
`basis?` (UUID of the parent revision), `mime` (`image/png` | `image/jpeg` | `image/gif` | `image/webp` |
`image/svg+xml`), `bytes`, `sha256` (over the raw bytes), `status` (`aktuell` | `abgeloest`), `autor`,
`angelegt`, `geaendert?` and in the container `datei` (entry in the ZIP). Children `<unterschrift>`, `<alt>`,
`<notiz>?` and in the XML-only form `<daten>` (raw bytes as Base64, without line breaks). Exactly one of `datei` and
`<daten>` is present.

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

In the XML-only form, instead of `datei=…`, at the end of the revision:
`<daten>iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mM4YKn9HwAFBQIkh/EstQAAAABJRU5ErkJggg==</daten>`.

**`<geloescht>?`** with `<eintrag uuid art zeit/>*` – tombstones: pieces of the work deleted here (§3), `art`
`werk` | `punkt` | `fassung` | `vorlage` | `beitrag` | `einfall` | `faden` | `faden-beitrag` | `bild` |
`bildfassung`, from minor version 2 on `verbindung` | `lage` (in a file with `version="1"` these two are an
error), from minor version 3 on `umschlag-block` (in a file with `version="1"` or `"2"` an error) and from
minor version 4 on `befund` | `befund-status` | `beitrag-fassung` (in a file with `version` 1 to 3 an error), from
minor version 6 on `satz` (in a file with `version` 1 to 5 an error), from minor version 7 on `glossar-begriff` |
`register` and from minor version 8 on `fortschreibung` (in a file with `version` 1 to 7 an error).

```xml
<geloescht>
  <eintrag uuid="26000000-0000-4000-8000-000000000046" art="punkt" zeit="2026-01-07T16:00:00.000Z"/>
</geloescht>
```

**`<karte>?`** – the idea map of the work (from minor version 2 on, Round 25), at the end of `<werk>` and only if there is
at least one connection or position: first all `<verbindung>`, then all `<lage>`. References run through the
UUIDs of the ideas of this work (`<einfaelle>`); connections exist **only between ideas** (not to items
or posts).

- **`<verbindung>`** – attributes `uuid`, `von`, `nach` (UUIDs of the ideas), `art` (`fuehrt-weiter` |
  `gehoert-zu` | `beispiel-fuer` | `widerspricht`; in the user interface „führt weiter“, „gehört zu“, „Beispiel für“,
  „widerspricht“), `herkunft-art` (`mensch` | `persona` | `mcp`), `herkunft` (name, `@handle`, `@mcp:<client>`),
  `angelegt`, `geaendert`, `geloescht?` (softly deleted since; restorable, the connection stays in the
  format). Child `<notiz>` (at most 500 characters, without control characters except line break; empty as `<notiz/>`).
  `widerspricht` is undirected: the order of `von`/`nach` carries no meaning (Baruch stores and
  writes the pair in the order of its ideas); the other kinds are directed. Per pair and kind at most
  one connection stands in the document (with `widerspricht` not reversed either).

  ```xml
  <verbindung uuid="26000000-0000-4000-8000-000000000096" von="26000000-0000-4000-8000-000000000028" nach="26000000-0000-4000-8000-000000000029" art="fuehrt-weiter" herkunft-art="mensch" herkunft="Hanna Probe" angelegt="2026-01-07T19:00:00.000Z" geaendert="2026-01-07T19:00:00.000Z">
    <notiz>Vom Seil zur Lampe.</notiz>
  </verbindung>
  <verbindung uuid="26000000-0000-4000-8000-000000000097" von="26000000-0000-4000-8000-000000000029" nach="26000000-0000-4000-8000-00000000002a" art="widerspricht" herkunft-art="persona" herkunft="@lektor" angelegt="2026-01-07T20:00:00.000Z" geaendert="2026-01-07T21:00:00.000Z" geloescht="2026-01-07T21:00:00.000Z">
    <notiz>Eimer &amp; &lt;Lampe&gt;
  streiten sich.</notiz>
  </verbindung>
  ```

- **`<lage/>`** – stored position of an idea on the map of this work (the human dragged it
  there): attributes `uuid`, `einfall` (UUID of the idea), `x`, `y` (SVG user units of the map, the work
  stands at 0,0; shortest representation that yields exactly the same number on reading, finite and at most
  ±1 000 000), `geaendert`. At most one position per idea. Positions are pure display (no mirror commit).

  ```xml
  <lage uuid="26000000-0000-4000-8000-000000000098" einfall="26000000-0000-4000-8000-000000000028" x="120.5" y="0.30000000000000004" geaendert="2026-01-07T22:00:00.000Z"/>
  <lage uuid="26000000-0000-4000-8000-000000000099" einfall="26000000-0000-4000-8000-000000000029" x="-1000000" y="-80.25" geaendert="2026-01-07T23:00:00.000Z"/>
  ```

- Tombstones of the map stand, like all of them, in `<geloescht>`: `<eintrag uuid="…" art="verbindung" zeit="…"/>` or
  `art="lage"`.

**`<befunde>?`** – the findings of the work (from minor version 4 on, Round 28), at the very end of `<werk>` (after `<karte>`)
and only if there are any: `<befund>*` in the order of their creation. A finding is an item from the answer
of a persona (`docs/Bedienung.md`, „Befunde“).

- **`<befund>`** – attributes `uuid`, `art` (`lob` | `hinweis` | `kritisch`), `status` (`offen` | `uebernommen` |
  `verworfen` | `verschoben` | `erledigt`; in the user interface „offen“, „übernommen“, „verworfen“, „verschoben“,
  „erledigt ohne Änderung“), `persona` (author of the answer, 1 to 200 characters), `stelle` (position in the answer, from
  0), `punkt?` (UUID of the section; missing for threads and deleted sections), `beitrag?` or `faden-beitrag?`
  (UUID of the origin, never both; missing if the post is deleted), `fassung?` (UUID of the revision with
  „übernommen“, mandatory), `ziel-punkt?` or `ziel-einfall?` (target with „verschoben“, exactly one), `gleicht?` (UUID
  of the finding this one equals – „angehängt“, does not count as open), `angelegt`, `geaendert`. Children `<text>`
  (not empty, at most 4000 characters), `<zitat>?` (literal quotation, at most 2000), `<pruefvermerk>?` (note
  of the quotation and reference check, e.g. „(nicht im Text belegt)“, at most 200), `<begruendung>?` (mandatory with
  „verworfen“, at most 2000), `<sammelt befund="…"/>*` (collective proposal of the lector: UUIDs of the summarised
  findings) and `<status-eintrag>*` (history, oldest first). Revision and target stand there as identifiers, even if they
  no longer exist in the file (as in the database without a foreign key).
- **`<status-eintrag>`** – a status change by the human or a proposal by an assistant: `uuid`, `status`,
  `herkunft-art` (`mensch` | `persona` | `mcp`), `herkunft`, `vorschlag` (`true` = proposal; without a proposal only
  the human sets it), `fassung?`, `ziel-punkt?`, `ziel-einfall?`, `entscheidung?` (`bestaetigt` | `abgelehnt` |
  `veraltet`, only with proposals, with `entschieden` and `entschieden-von`), `angelegt`; children `<begruendung>?` and
  `<notiz>?`. The first creation of a finding has no entry.

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

**`<manifest version>`** with `<datei pfad sha256 bytes/>+` – only in the container (`MANIFEST.xml`, example in §1).

## 3 Identifiers and tombstones

**Identifiers.** Every piece that moves between instances carries a globally unique identifier (UUID) in
canonical lower case `8-4-4-4-12`, e.g. `3f2b8c1e-9a47-4d2e-b5c3-0e1f2a3b4c5d`:

| Element in the XML | Table | Kind in the tombstone (`<eintrag art>` / database) |
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
| `<karte>/<verbindung uuid>` (from minor version 2) | `idea_link` | `verbindung` / `idea_link` |
| `<karte>/<lage uuid>` (from minor version 2) | `idea_map_pos` | `lage` / `idea_map_pos` |
| `<buch>/<umschlag>/<block uuid>` (from minor version 3) | `cover_block` | `umschlag-block` / `cover_block` |
| `<befunde>/<befund uuid>` (from minor version 4) | `finding` | `befund` / `finding` |
| `<befund>/<status-eintrag uuid>` (from minor version 4) | `finding_status` | `befund-status` / `finding_status` |
| `<beitrag>/<alte-fassung uuid>` (from minor version 4, section and thread) | `post_version` | `beitrag-fassung` / `post_version` |
| `<angaben>/<satz uuid>` (from minor version 6) | `work_typeset` | `satz` / `work_typeset` |
| `<glossar>/<begriff uuid>`, `<eigene-register>/<register uuid>` (from minor version 7) | `glossary_term`, `index_register` | `glossar-begriff` / `glossary_term`, `register` / `index_register` |
| `<fassung>/<fortschreibung uuid>` (from minor version 8) | `revision_amend` | `fortschreibung` / `revision_amend` |

- **Assignment:** new rows get their UUID in the database (trigger `<Tabelle>_uuid_fill`, version 4,
  random, variant 10). That holds for every path: creating, Typst import (also with images), sample work,
  outline (also by assistants), saving, proposals, posts, ideas, threads, images and new
  revisions of the work template. On reading in, a piece keeps the UUID from the file; differing revisions
  get a derived UUID version 8 (see §4).
- **Immutable:** moving, renaming, level, number on/off, switching, accepting, changing a
  revision and switching an image keep every UUID. A renamed item stays the same item.
- **Internal** stay the numbers (`id`, `key`, `seq`, revision number): in the XML everything refers through UUIDs, e.g.
  `<bezug punkt="…" fassung="…">` or `basis="…"`.
- **Instance:** every Baruch instance has its own identifier (`store_meta.instance_uuid`, assigned on the first start)
  and a freely chosen name (`store_meta.instance_name`, default empty). Both stand in
  `<ausgabe instanz="…" instanzname="…">`. Without a name Baruch shows „ohne Namen (<erste 8 Zeichen>)“, e.g.
  „ohne Namen (3f2b8c1e)“. Setting it: „Einstellungen → Daten“ or RPC `instance.set_name`.

**Tombstones.** Whoever deletes a row with a UUID (revision, post, image revision, image, outline item with its
revisions and posts, whole work with its items, revisions, template revisions and threads) leaves an entry in
`uuid_tombstone` with UUID, kind, work UUID and time. Exporting writes the tombstones
of the work as

```xml
<geloescht>
  <eintrag uuid="0c1d2e3f-4a5b-4c6d-8e7f-8091a2b3c4d5" art="fassung" zeit="2026-11-08T17:20:31.512Z"/>
</geloescht>
```

- When **synchronising**, the instance creates nothing again for which it has a tombstone, and shows it in the
  preview as „hier gelöscht“. When **reading in as a new work**, tombstones do not hold.
- An idea is only deleted softly (`geloescht="…"` at the `<einfall>`, restorable) and therefore has no
  tombstone. Likewise a connection of the map (`geloescht="…"` at `<verbindung>`); its tombstone (`verbindung`)
  practically only arises when an idea is deleted hard (cascade). A position leaves a tombstone
  (`lage`) when the human resets the arrangement (the positions are then deleted) or the work is deleted.
  A cover block leaves a tombstone (`umschlag-block`) when the human resets the cover layout entirely or
  for one area (also through the quick choice) or the work is deleted; changing a block keeps
  its row and identifier. Findings, their history entries and old post revisions leave tombstones
  (`befund`, `befund-status`, `beitrag-fassung`) when they are deleted – also in the cascade when their work,
  their finding or their post is deleted (migration 0046); a deleted post leaves its findings standing
  (without origin). If a work is deleted, its ideas stay as ideas without a work (likewise without a
  tombstone, the identifier stays). If the work is later read in again from its file (new or synchronising),
  it gets these ideas back: work and reference are set, their content stays as it is here; the
  preview calls them „Einfall ohne Werk, wieder zugeordnet“ (test
  `a_deleted_work_comes_back_from_its_own_file_and_gets_its_ideas_back`).
- The store itself never looks into the tombstones; they only count during synchronisation.

## 4 Reading in and synchronisation

Reading in is an action of the human (form, RPC, command line; **not** over MCP). It never switches,
never overwrites and never deletes; outline changes become requests. Code: reading in
`baruch-exchange/src/read.rs` (submodules `read/{dom,map,archive,check}.rs`), planning and writing in
`baruch-store/src/exchange_apply.rs`, paths in `baruch-server/src/exchange/import.rs`.

**Course.** First the **preview**: the file is read and checked (§5), then the store plans the reading in within
a transaction that is rolled back at the end – the preview is therefore exactly what „Übernehmen“ would do, and
writes nothing. „Übernehmen“ requires the checksum (SHA-256) of the shown file, reads it anew, plans anew (the
preview is not trusted) and writes under the write lock in **one** transaction (`BEGIN IMMEDIATE`);
at the end `load::work` checks the work in the same transaction. Afterwards exactly **one** commit in the mirror with the
subject „Austausch aus <Instanz>: …“ (author „Austausch aus <Instanz>“, trailers `Baruch-Action: exchange-import`,
`Baruch-Exchange-Instance`, `Baruch-Exchange-File`); the semantic search is notified as with the Typst import.
A synchronisation that adds nothing and files no request makes no commit. `<Instanz>` is the
instance name from `<ausgabe>`, otherwise the first 8 characters of its identifier. The reader cleans the name
(`baruch_exchange::clean_instance_name`): control characters and whitespace become a space, `<` and `>` become
`‹` and `›` (Git rejects them in the author), shortened to 80 characters as locally.

**Mode.** The work UUID decides: unknown here → **new** (tombstones do not count for this), known →
**synchronise**. Whoever asks for the wrong mode gets a rejection (RPC -32000, form 409, CLI exit 1).

### New

The work comes into being with all identifiers, numbers, statuses, times and the mark „aktuell“ from the file:
work metadata with scheme and own pattern, template (current text and all revisions), outline (the numbers
`node.number` are computed by the outline as when loading), revisions with basis, posts with sources and references,
ideas (deleted ones stay deleted, with their time), threads, images with paths and all revisions, cover and
author photo, addresses, the cover layout (all blocks with their identifiers and times), the
typeset view settings, speaking time, Bible module, SWORD sources, the idea map (all connections, also deleted ones, and
all positions with their identifiers; `widerspricht` is stored so that the smaller idea stands first), old
revisions of edited posts and thread posts with `geaendert`, the findings with status, reasoning, revision,
target, collection and history (`gleicht` through the identifier, after all findings stand) and the tombstone list of the
file. Title rules as with the
Typst import: a title already taken is rejected (in the preview under „Nicht übernommen“), with `title` it
works under a title of its own. An image whose bytes do not pass the check as on uploading (at most 8 MiB,
type recognised from the first bytes and equal to the stated one) does not come along and stands under „Nicht
übernommen“. A reference without a revision becomes `rev = 0`; an `erledigt-ref` that begins with the identifier of an item
becomes „key <Schlüssel hier>…“ again (counterpart to the export).

### Synchronising (rules F6)

Only what is missing is added. Every existing row stays as it was in all columns (exception: position,
parent position, order and number of the items as well as `last_rev`, which creating an item or a
revision recomputes; checked in the test `merging_never_overwrites_an_existing_row`).

- **(a) Unknown item:** created at its place – behind the previous sibling from the file that exists here
  under the same parent, otherwise as the first child of the parent –, with **exactly** its revisions from the
  file (no additional revision 1). If it would get a different parent there or take over existing items,
  it is not created (hint). A **new part** under which existing chapters would move becomes a
  request `Add` that the human approves (AGENTS.md). Since Round 22 the request carries the **identifier of the part and
  all its revisions** (number, text, origin, status, basis, state, amendments; the preview says „Antrag mit
  Kennung und n Fassung(en)“, the request „… (mit Kennung und n Fassungen) …“). On approval the part comes into being with
  exactly this identifier and exactly these revisions (the chapters move under it as before, one mirror commit); a
  later synchronisation with the same file recognises it again and reports nothing. If the identifier exists here in the meantime,
  the request is outdated; rejecting creates nothing and leaves no tombstone. Sub-items of an item that was not created
  do not come (they only move under the part with the approval if the chapters already exist here).
- **(b) Known item:** another title → request `Rename`, unless a revision present here or just created
  already carries the title of the file (then a hint with a link to this revision: switching suffices). Another place
  (another parent or another order among the items both sides know) → request `Move` behind
  the previous sibling, otherwise before the next one, otherwise under the parent. Another level 1–3 → `SetLevel`,
  numbered differently → `SetNumbered`. If the trial run of the request does not work or the same request is already open,
  only the difference is listed. An item that is missing in the file (or deleted there) yields only a
  hint – never a request to delete.
- **(c) Unknown revision:** appended with a new local number (`last_rev + 1`), basis through the identifier
  (if the parent revision is missing, it becomes a root). Status: `vorschlag` → proposal, `verworfen` → discarded,
  `aktuell` or `abgeloest` → superseded. A revision read in **never** becomes current. If the item here already has
  a revision with exactly the same text, nothing is created (equality of content, see "Back and forth" below).
- **(d0) Fast-forward (since Round 22, minor version 8):** if the amendments of a known revision here are a true
  prefix of those read in (the same identifiers in the same order, state here = number of steps here) and the
  text here is exactly the text of the file at this point (after the last common step or before the first one),
  the revision is **fast-forwarded**: the missing steps are added with their identifiers, text and state follow the file;
  number, status and origin stay. The preview counts them as „Fortschreibungen“ (`AddCounts.amends`) and reports
  „Fassung n: vorgespult von Stand k auf Stand m“ (hint); the commit of the reading in contains it. Only what may also be amended
  locally is fast-forwarded (follow-up R22) – the **current revision of the human** (here
  current and by the human, in the file by the human); revisions by personas or MCP clients, proposals,
  accepted, discarded and superseded revisions are never rewritten in place. And only if the
  missing steps reproduce the change **without gaps**: first missing step `<vorher>` = text here, every
  `<vorher>` = `<nachher>` of the previous one, last `<nachher>` = text of the revision in the file. Reading does not check that
  (changing through the tree changes a text without a step, a valid file can have gaps); if it does not fit,
  (d) holds.
- **(d) Known revision with a different text:** a further revision arises with the derived identifier
  `derive(original, text)`, basis = the revision here, status according to (c), note with the addition
  „(Austausch: abweichende Fassung aus <Instanz>)“. If the derived identifier exists already or the item here
  already has a revision with exactly this text, nothing happens (a repeated reading in creates nothing twice). Another note or another state „aktuell“: only a hint
  with the link `/?work=W&key=K&rev=R`.
- **(e) Template:** revisions with an unknown identifier are appended (next number); the current template text
  `work.template` stays, a difference is a hint.
- **(f) Ideas, posts (with references), threads and thread posts, images and image revisions** with an unknown
  identifier are added (posts and thread posts at the end, images through the check as on uploading,
  name collision → free name `-2`, `-3` … as with the import; image revisions read in for a known image
  are superseded, never current). Known ones with different content: only a hint.
- **(g) Work metadata, book metadata (with addresses), typeset view settings, speaking time, Bible module,
  SWORD sources:** if the row is missing here entirely, it is created from the file (hint); otherwise, on a difference, only a
  hint.
- **(i) Cover layout** (Round 27): a block with an unknown identifier is added if there is no block here for (work, kind)
  yet – **local wins**, otherwise a hint „hier gibt es schon einen Block dieser Art“. A
  known identifier is never overwritten (other values: hint „weicht ab“). If the block has a tombstone here
  (reset), it does not come back („Hier gelöscht“). The preview counts „Blöcke der Umschlag-Anordnung“
  (`AddCounts.cover_blocks`, also in `total()`).
- **(j) Findings and old post revisions** (Round 28): a finding with an unknown identifier is added if its
  origin (post or thread post) belongs to the work here, no tombstone exists and this position of the answer here
  is still free – otherwise a hint (such as „diese Stelle der Antwort ist hier schon Befund 12 (unter anderer
  Kennung)“ if both instances split the same post themselves). It brings along status, reasoning, revision,
  target and its history. A **known** finding is never overwritten: if its status differs, there is only
  a hint, and history entries of the file are not added – the human sets the status here. Old revisions
  with an unknown identifier are added, also to a known post (they are only history). The preview counts
  „Befunde“, „Einträge im Verlauf der Befunde“ and „alte Fassungen von Beiträgen“ (`AddCounts.findings`,
  `finding_entries`, `post_versions`, also in `total()`).
- **(h) Idea map** (Round 25): a connection with an unknown identifier is added if both ideas belong to the work here
  (also ones just read in); if one is missing, that is only a hint. If the same pair
  with the same kind already exists here under a different identifier (active or deleted; `widerspricht` in both directions), it does
  not come twice (hint). A known identifier is never overwritten (another note, another state „gelöscht“:
  hint). A position only comes if there is none here for (work, idea) – **local wins**; a known
  position stays as it is here, without a hint (pure display). The preview counts „Verbindungen der Gedankenkarte“
  and „Lagen auf der Gedankenkarte“ (`AddCounts.links`, `AddCounts.positions`, also in `total()`).
- **Tombstones:** what has a tombstone here (`uuid_tombstone`) is not created again and stands in the
  preview under „Hier gelöscht“ – also connections, positions, cover blocks, findings, their history entries and old
  post revisions (a reset layout thus does not come
  back through the file of the other side). What the file reports as deleted and lives here stays (hint).
- **Origin of the requests:** human „Austausch: <Instanz>“, note „Austausch aus Instanz <uuid>, Datei <sha256>“.

**Derived identifier.** `derive(original, text)` = the first 16 bytes of
`SHA-256("baruch-fassung:" ‖ original ‖ ":" ‖ sha256_hex(text))`, in it version 8 (byte 6 = `0x8_`) and variant
10 (byte 8 = `0b10xx_xxxx`), canonical lower case as 8-4-4-4-12 (`baruch_store::exchange_apply::derive_uuid`).
Example: original `26000000-0000-4000-8000-00000000006f`, text `Ein Text` → the hex digits 1–12, 14–16 and
18–32 (without dashes) are those of the hash, digit 13 is `8`, digit 17 one of `8 9 a b`.

**Example.** Both computers have the work from the same file; „Das Rad“ has the revisions 1 (current)
and 2 there. On computer B the human changes the text of revision 1 and counts „Die Speichen“ without a number; B exports, A
reads in with „abgleichen“: A gets revision 3 (superseded, basis 1, identifier `derive(<Kennung von Fassung 1>,
<Text von B>)`, note with the addition), revision 1 stays current, plus a request `„Die Speichen“ ohne Nummer zählen`
from „Austausch: B“. A second pass with the same file adds nothing and files no request.
If the human on B renames an item in the outline, a new revision with the new heading arises there;
on A it arrives as a superseded revision, and instead of a request a hint „Umschalten
genügt“ stands there (rule b).

**Back and forth.** If B edits a shared revision X in place (changing the text instead of a new revision),
A gets the derived revision D(X, new) on synchronisation, B on the way back D(X, old) – so each side has both
texts, nothing gets lost. No further copies arise: D(X, new) has the same text on B as X and is
skipped there, likewise D(X, old) on A. After that nothing more is added (test
`merging_back_and_forth_after_an_in_place_edit_creates_no_duplicate_texts`). The promise "nothing twice" thus holds
per identifier **and** per text of an item.

**Older file.** The synchronisation knows no common basis: every difference of the outline counts as if the
other side had changed it. If one reads in an **older** file (say one's own export from yesterday), requests therefore arise
to undo one's own changes to number, level, place or title (title only if no revision carries the
old title). They are only requests – nothing changes without confirmation; when in doubt, reject. Revisions,
ideas, posts and images are not affected (they are only added).

Open (after Round 26): synchronisation directly over the network (user question 2); „als Kopie einlesen“ (new identifiers) does not exist
yet. (Done in Round 22: a new part that would take over chapters travels in the request with its identifier and revisions.)

## 5 Security and limits

On **every** error nothing is written (no row, no request, no commit, no incoming file). Code:
`baruch-exchange/src/read/` (tests `crates/baruch-exchange/tests/read.rs`).

**XML** (`read/dom.rs`):
- No DTD: `<!DOCTYPE …>` → „Unsicheres XML“ (so neither "Billion Laughs" nor external entities take effect;
  nothing is resolved or loaded). No processing instruction except the XML declaration. Only the five
  predefined entities (`&lt; &gt; &amp; &apos; &quot;`) and character references; every other reference →
  „Unsicheres XML“. Characters forbidden in XML 1.0 (also as `&#1;`) → XML error. UTF-8 only.
- Text exact: never trimmed; line ends as in XML 1.0 (`\r\n`, `\r` → `\n`; a real `\r` stands as `&#13;`).
  In attribute values tab, line break and carriage return become spaces; as character references they stay.
  `kodierung="base64"` at a text element is decoded (UTF-8), `<daten>` is Base64 (whitespace allowed).
- The namespace `urn:baruch:werk:1` is mandatory; another `urn:baruch:…` → „Version … wird nicht unterstützt“,
  a foreign or missing one → „Keine Austauschdatei“. A higher minor version (`version="3"`) is read;
  the preview says so, and unknown things stand as a path in `unknown` (`werk/angaben/farbe`, attributes as
  `…/@name`, text as `…/#text`, foreign namespaces as `{ns}name`, an unknown child of the map as
  `werk/karte/<name>`). Nothing of it is adopted. The reader reads `<karte>` only from `version="2"` on; in a
  file with `version="1"` it stands as `werk/karte` in `unknown`. Likewise `<satzbild>`, `<verlag-anschrift>`,
  `<druckerei-anschrift>` and `<umschlag>` only from `version="3"` on (before that `werk/angaben/satzbild`,
  `werk/buch/verlag-anschrift` … in `unknown`), `<befunde>`, `geaendert` and `<alte-fassung>` at posts only from
  `version="4"` on (before that `werk/befunde`, `…/beitrag/@geaendert`, `…/beitrag/alte-fassung` in `unknown`).
- Mandatory elements and attributes missing → XML error with a path (e.g. „Pflichtelement fehlt: werk/angaben/titel“).
- Afterwards `validate`: canonical UUIDs, no identifier twice (also no tombstone for an existing piece),
  references (basis in the same item or image with a smaller number, references, cover), per item and image exactly one
  current revision, valid levels, checksum of the image bytes; map: connections and positions only to ideas of the
  document, no self-reference, no pair with the same kind twice, note at most 500 characters without control characters
  except line break, per idea at most one position, coordinates finite (otherwise XML error) and at most
  ±1 000 000; cover (from minor version 3 on): at most one block per kind, limits as above, `richtung` exactly on the
  spine, `<text>` only with own text, font names by the rule of the typeset view fonts, addresses at most 500
  characters; findings (from minor version 4 on): item, post and thread post from the document (never post and thread post
  at once), `gleicht` to another finding of the document, limits like the CHECKs of migration 0046 (text not
  empty and at most 4000, quotation 2000, check note 200, reasoning and note 2000, persona 1–200 characters),
  „verworfen“ only with a reasoning, „übernommen“ only with a revision, „verschoben“ with exactly one target – for the finding and
  every history entry; without a proposal only the human sets, a decision only on a proposal and exactly with
  `entschieden`; old revisions with their own identifier.

**ZIP** (`read/archive.rs`):
- First entry `mimetype` at the start of the file, uncompressed, exactly `application/vnd.baruch.werk+zip`.
- Names only `mimetype`, `werk.xml`, `MANIFEST.xml`, `bilder/<uuid>.<png|jpg|gif|webp|svg>`; rejected are
  absolute paths, `..`, backslash, drive, control characters, duplicates (the central directory is counted itself),
  links, folders, encrypted entries and every compression other than STORED and DEFLATE.
- Unpacking uses `take(Grenze + 1)`; the sizes in the header are not trusted (ZIP bomb, lying header size).
  Limits per entry, for `werk.xml` and in total.
- Every entry except `mimetype` and the manifest stands in the manifest and vice versa; SHA-256 and byte count must
  match („Prüfsumme stimmt nicht: <Eintrag>“). Every image revision with `datei` names exactly its entry
  `bilder/<fassungs-uuid>.<ext>`; an image in the archive without a revision is rejected.

**Limits** (`baruch_exchange::Limits::DEFAULT`):

| Limit | Default |
|---|---|
| whole file | 128 MiB |
| unpacked in total | 256 MiB |
| one entry unpacked | 64 MiB |
| entries in the ZIP | 4096 |
| `werk.xml` or `.baruch.xml` | 64 MiB |
| depth of the elements | 32 |
| elements | 500 000 |
| attributes per element | 32 |
| bytes per text or attribute value | 16 MiB |

Paths: form up to 128 MiB + 1 MiB (`UPLOAD_LIMIT`, above it 413), RPC up to `works::RPC_BODY_LIMIT` (32 MiB +
64 KiB as Base64, so about a 24 MiB file), command line up to 128 MiB. The incoming file between preview and
„Übernehmen“ lies under `<data>/austausch-eingang/<sha256>` (only names of 64 hex characters), is deleted after
accepting – also after a rejection – and cleared away after 24 hours.

## 6 Minor versions and extension

- **Minor version instead of a new namespace:** compatible extensions (new optional elements or attributes)
  count `version` up, the namespace `urn:baruch:werk:1` stays. Only an incompatible change would get a
  new namespace (`urn:baruch:werk:2`), which a reader of today rejects.
- **Minor version 2 (Round 25, idea map)** fills the `<karte>` foreseen in minor version 1 at the end of
  `<werk>` with connections **between ideas** and the positions of ideas (§2). Correction compared with the
  announcement from Round 26: connections to items or posts do not exist; an idea hangs on its item through `bezug`.
  Plus the tombstone kinds `verbindung` and `lage`. Schema: `docs/schema/baruch-werk-1-u2.xsd`
  (`<karte>` fixed instead of lax, `GrabsteinArt` extended); `docs/schema/baruch-werk-1.xsd` stays unchanged for
  minor version 1. Because of the new tombstone kinds an output with a map does not always pass the
  frozen v1 schema, but it does pass the u2 schema.
- **Minor version 3 (Round 27, cover editor and imprint)** brings into `<buch>` after `<vita>` the addresses
  `<verlag-anschrift>` and `<druckerei-anschrift>` (mandatory statements in the imprint) and `<umschlag>` with the blocks of the
  cover layout (§2), into `<angaben>` after `<sprechdauer>` `<satzbild/>` with the typeset view settings (item
  from „Nach Runde 18 offen“) and the tombstone kind `umschlag-block`. All of these are new optional elements at the end of their
  sequence. Schema: `docs/schema/baruch-werk-1-u3.xsd`; `baruch-werk-1-u2.xsd` and `baruch-werk-1.xsd` stay
  unchanged. An output of minor version 3 does not pass the older schemas (new elements), a file of
  minor version 1 or 2 passes the u3 schema.
- **Minor version 4 (Round 28, findings)** brings `<befunde>` at the end of `<werk>` (findings with status history), at
  `<beitrag>` (section and thread) `geaendert` and `<alte-fassung>*` at the end, and the tombstone kinds `befund`,
  `befund-status`, `beitrag-fassung`. All of these are new optional elements or attributes at the end of their sequence. Schema:
  `docs/schema/baruch-werk-1-u4.xsd`; the schemas of minor versions 1 to 3 stay unchanged. An output with
  findings or edited posts does not pass the older schemas, a file of minor versions 1 to 3
  passes the u4 schema.
- **Minor version 5 (Round 30, solutions, variants, chat)** brings (no new tombstone kind):
  - at `<befund>` after the status history `<loesung zitat="…" ersatz="…" schritt="…"/>*` (each attribute optional; a
    solution needs a quotation with a replacement or a step, which the reader checks);
  - at `<fassung>` of a variant `variante-gruppe` (UUID), `variante` (A, B, C) and `variante-zeile` (at most 300
    characters) and after `<text>` per implemented finding `<setzt-um befund="uuid"/>` (only findings of the document; deleted ones
    are dropped on writing);
  - at `<punkt>` `schluessel` – the fixed key of the item in the exporting instance. It is needed because
    cross-references in the text `#link(<kN>)` point to this key (Round 30a); on reading in, Baruch rewrites them through
    `baruch_typst::xref::remap_keys` to the keys here (mapping `schluessel` → identifier → key here,
    only in revisions that the reading in creates). Assumption compared with the contract R30 §4.3, which does not name `schluessel`:
    without it §3.3 „Austausch u5 bildet die Schlüssel um“ could not be fulfilled. Consequence: two outputs of the same
    work from two instances differ, besides in `<ausgabe>`, also in `schluessel`;
  - at `<beitrag>` (section and thread) `grundlage-fassung` (UUID of the reviewed revision of the same item; in the thread
    never), `grundlage` (header line „geprüft: …“, at most 300 characters), `antwort-auf` and `wiederholt` (UUIDs of
    another post at the same place; a target at another place, say after moving, is dropped on writing)
    and `relay-von` (persona, 1 to 200 characters).
  The job of a post (`request`) and the log `replace_batch` stay local. On reading in, a
  post-pass sets these parts only on the rows that the reading in itself created (`exchange_apply::apply_u5`):
  known revisions, posts and findings stay unchanged when synchronising, even if the file carries other values;
  a differing revision that gets a derived identifier stays without a variant. Schema:
  `docs/schema/baruch-werk-1-u5.xsd`; the schemas of minor versions 1 to 4 stay unchanged (SHA-256 in the test).
- **This reader** (minor version 5) reads the parts of minor version 5 only from 5 on (in 1 to 4 they are reported as
  `werk/befunde/befund/loesung`, `…/punkt/@schluessel`, `…/fassung/@variante…`, `…/fassung/setzt-um`,
  `…/beitrag/@grundlage…`, `@antwort-auf`, `@wiederholt`, `@relay-von` and not adopted; test
  `files_of_version_four_leave_the_u5_parts_unknown_and_still_read`); it reads an output of minor version 4 without a
  hint. A reader of minor version 4 (Round 28) reads a file of minor version 5 like a higher minor version:
  solutions, variants, keys and post metadata are lost in the process, nothing else.
- **The reader of minor version 4** read `version="1"` to `"3"` as before and the parts of minor version 4 only from
  4 on (in 1 to 3 reported as `werk/befunde`, `…/beitrag/@geaendert`, `…/beitrag/alte-fassung` and not adopted;
  the tombstones `befund`, `befund-status`, `beitrag-fassung` are an error there; test
  `files_of_version_three_leave_findings_unknown_and_refuse_their_tombstones`). For the older parts: `<karte>` only from 2 on (in 1 reported as
  `werk/karte`), the parts of minor version 3 only from 3 on (in 1 and 2 reported as `werk/angaben/satzbild`,
  `werk/buch/verlag-anschrift`, `werk/buch/druckerei-anschrift`, `werk/buch/umschlag` and not adopted);
  a tombstone of a kind that the minor version of the file does not know yet (`verbindung`/`lage` in 1,
  `umschlag-block` in 1 or 2) is an error. It reads an output of minor version 2 without the new parts without a
  hint (test `files_of_version_two_leave_the_new_parts_unknown_and_refuse_the_new_tombstone`). An even higher
  minor version (`version="5"`) it reads like 4, reports unknown things (e.g. `werk/karte/kante`) as a hint and
  adopts nothing of them (§4).
- **New enumeration values** are no compatible extension – with one exception: a new kind of
  tombstone (`<eintrag art="…">`). This reader reports a kind unknown to it with `version` > 4 as unknown
  (`werk/geloescht/eintrag/@art=„wolke“`) and skips only this tombstone; in `version="1"` to `"4"`
  an unknown value stays an error (test
  `an_unknown_tombstone_kind_is_skipped_in_a_newer_minor_version_but_refused_in_version_one`). Every other new
  value (e.g. another kind of connection) needs a new element or attribute that an older reader reports as
  unknown, instead of extending an existing value.
- **A reader of minor version 1** (Round 26) reads a file of minor version 2: it reports `werk/karte` as
  unknown and skips the tombstones `verbindung` and `lage` (§6 there); it adopts work, revisions, ideas and images
  as before. The map is lost in the process, nothing else.
- **A reader of minor version 2** (Round 25/26) reads a file of minor version 3 like a higher minor version: it
  reports `werk/angaben/satzbild`, `werk/buch/verlag-anschrift`, `werk/buch/druckerei-anschrift` and
  `werk/buch/umschlag` as unknown and skips the tombstones `umschlag-block`; everything else it adopts as
  before. Cover layout, addresses and typeset view are lost in the process, nothing else.
- **A reader of minor version 3** (Round 27) reads a file of minor version 4 like a higher minor version: it
  reports `werk/befunde`, `…/beitrag/@geaendert` and `…/beitrag/alte-fassung` as unknown and skips the
  tombstones `befund`, `befund-status`, `beitrag-fassung`; findings and old post revisions are lost in the process,
  nothing else (the text of an edited post is the present one).
- **The writer** always writes its own minor version (today `version="6"`, also in the manifest; until Round 30 `5`) – also for a
  document that was read from a higher minor version; what it does not know is not in the model and is
  not written along.
- New elements come at the end of their sequence or as optional elements, so that older readers keep the order of the
  known children.

## Typesetting settings (minor version 6, Round 29a)

Owner: satz-daten. Since Round 29 the typesetting settings of a work travel along (table `work_typeset`, migration
0048; data model `baruch_typst::typeset::TypesetSettings`). Minor version **6**: the writer writes `version="6"`
(also in the manifest; this replaces "today `version="5"`" in §6), XSD `docs/schema/baruch-werk-1-u6.xsd`. The schemas
of minor versions 1 to 5 stay byte-unchanged (SHA-256 in the test `the_frozen_v1_and_u2_schemas_are_byte_unchanged`);
a file of minor version 5 passes u6 and u5, an output with `<satz>` does not pass u5.

**Element.** In `<angaben>` as the last child behind `<satzbild/>`, only if the work has typesetting settings:

```xml
<satz uuid="…" geaendert="2026-11-09T10:00:00.000Z" geaendert-von="Autor"
      kapitelanfang="rechte-seite" teil-seite="rechts" teil-rueckseite-leer="true" seitenformat="a5"
      rand-innen-mm="20" rand-aussen-mm="15" rand-oben-mm="18" rand-unten-mm="22" bundsteg-mm="5"
      grundschrift="Libertinus Serif" schriftgroesse-pt="10.5" zeilenabstand-em="0.65" absatz="einzug" absatz-em="1"
      blocksatz="true" silbentrennung="true" sprache="de" seitenzahlen="unten-aussen"
      seitenzahlen-beginn="erstes-kapitel" kolumnentitel="kapitel" fussnoten="arabisch" fussnoten-zaehlung="je-kapitel"
      schusterjungen-hurenkinder="true" verweis-seitenzahl="true"/>
```

Mandatory are `uuid` (identifier of the row; stays the same with every change), `geaendert` (time of the last
change, as stored) and `geaendert-von` (name of the human, may be empty). Every field is an optional
attribute; **if it is missing, the work template holds** (like `None` in the model). Vocabulary (words from
`baruch_exchange::typeset`, enumerations in the XSD):

| Field (JSON) | Attribute(s) | Values |
| --- | --- | --- |
| `chapter_start` | `kapitelanfang` | `fortlaufend`, `neue-seite`, `rechte-seite` |
| `part_page` | `teil-seite` + `teil-rueckseite-leer` | `rechts`, `beliebig` + `true`/`false` |
| `page_format` | `seitenformat` (+ `seite-breite-mm`, `seite-hoehe-mm` only with `eigenes`) | `a5`, `120x190`, `135x215`, `a4`, `eigenes` |
| `margins` | `rand-innen-mm`, `rand-aussen-mm`, `rand-oben-mm`, `rand-unten-mm`, `bundsteg-mm` | numbers (mm) |
| `font` | `grundschrift` | family name |
| `font_size_pt` | `schriftgroesse-pt` | number (pt) |
| `leading_em` | `zeilenabstand-em` | number (em) |
| `paragraph` | `absatz` + `absatz-em` | `einzug`, `abstand` + number (em) |
| `justify` | `blocksatz` | `true`/`false` |
| `hyphenate` | `silbentrennung` | `true`/`false` |
| `lang` | `sprache` | `de`, `en`, `de-CH` … (`[a-z]{2,3}(-[A-Z]{2})?`) |
| `page_numbers` | `seitenzahlen` + `seitenzahlen-beginn` | `unten-mitte`, `unten-aussen`, `oben-aussen`, `keine` + `erste-seite`, `erstes-kapitel` |
| `running_head` | `kolumnentitel` | `aus`, `werktitel`, `kapitel` |
| `footnotes` | `fussnoten` + `fussnoten-zaehlung` | `arabisch`, `zeichen`, `buchstaben` + `fortlaufend`, `je-kapitel` |
| `widows_orphans` | `schusterjungen-hurenkinder` | `true`/`false` |
| `xref_with_page` | `verweis-seitenzahl` | `true`/`false` |

Numbers stand in the shortest exact form (`f64::to_string`: `20`, `10.5`). Composite fields stand there entirely
or not at all; the reader rejects half a statement (say four of the five margins, `teil-seite` without `teil-rueckseite-leer`, dimensions without
`seitenformat="eigenes"`) and a foreign word with the path (`werk/angaben/satz/@bundsteg-mm: …`,
`ExchangeError::Xml`), so that no field gets lost silently. The limits (margins 5–80 mm, font size 6–24 pt …)
are not checked by the reader but by the reading in with `TypesetSettings::validate` (after `normalized`).

**Tombstone.** Reset typesetting settings leave a tombstone `<eintrag art="satz">` (database
`uuid_tombstone.kind = 'work_typeset'`, with the work uuid, also when the work is deleted, through the cascade trigger).
The kind exists only from minor version 6 on: in a file with `version="1"` to `"5"` it is an error (like
`umschlag-block` before 3). A reader of minor version 6 reports `<satz>` in a file of minor versions 1 to 5 as
`werk/angaben/satz` and does not adopt it; a reader of minor version 5 (Round 30) reads a file of
minor version 6 like a higher minor version: it reports `werk/angaben/satz`, skips the tombstone `satz`, and only
the typesetting settings are lost in the process.

**Reading in (rule „ohne stillen Verlust“).**

- *New:* the typesetting settings come into being with identifier, `geaendert` and `geaendert-von` from the file; the tombstones
  `satz` go into the tombstone list like all others. In the report they count as `added.typeset` (piece `satz`).
- *Synchronising:* if typesetting settings are missing here and there is no tombstone here for their identifier, they come from the
  file (setting them thus reaches the other instance). Otherwise **never overwritten**: if they differ (same or
  different identifier), a hint (`kind: "satz"`, action „Hinweis“) names the differing fields with their
  labels and carries both states completely as JSON (`here`, `there`) – so the values of the file are not
  silently lost; the human adopts them if needed in „Einstellungen → Werk → Satz“. If their identifier was
  reset here (tombstone), they stand under „hier gelöscht“ and do not come back. A tombstone `satz` of the file
  leaves the settings here standing (hint „In … gelöscht, hier vorhanden – bleibt“, as with all tombstones).
- Invalid values (`TypesetSettings::validate`) are not adopted and stand under „Nicht übernommen“
  (`refusals`, „Satz-Einstellungen: Schriftgröße: 6 bis 24 pt – nicht übernommen“); the rest of the work comes nevertheless.
- Proven in `crates/baruch-store/tests/typeset.rs` (`exchange_carries_the_settings_and_merging_never_overwrites_silently`)
  and `crates/baruch-server/tests/round29_daten.rs` (`two_instances_exchange_set_change_and_reset_without_silent_loss`:
  setting, changing, resetting across two or three instances).

The rule block „// Baruch: Satz“ in the export (`snapshot::head`) is not part of the exchange format: the file carries the
settings, not the generated Typst rules; the work template in `<vorlage>` stays without the block.

## Glossary and own indexes (minor version 7, Round 31a)

Owner: glossar-daten. Since Round 31 the glossary (tables `glossary_term`, `glossary_variant`, migration 0049)
and the own indexes (`index_register`) travel along, plus the index/glossary fields of the typesetting settings. Minor version
**7**: the writer writes `version="7"` (also in the manifest; this replaces "today `version="6"`" above), XSD
`docs/schema/baruch-werk-1-u7.xsd`. The schemas of minor versions 1 to 6 stay byte-unchanged (SHA-256 in the test,
u6 newly added); a file of minor version 6 passes u7 and u6, an output with `<glossar>` does not pass u6.

**Elements.** At the end of `<werk>` behind `<befunde>`, only if present:

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

- `<register>`: identifier `[a-z][a-z0-9-]{0,31}`, never a built-in one (`stichwort`, `personen`, `bibel` – they never stand
  in the file), unique per work, at most 13; title 1–80 characters. The order of the elements is the order
  of the indexes.
- `<begriff>`: `status` `angenommen` or `vorgeschlagen`, `herkunft` (name of the human or handle of the persona or
  of the MCP client), `angelegt`, optionally `geaendert` and `geaendert-von`. `<wort>` is mandatory, `<kurz>`,
  `<ausfuehrlich>`, `<quelle>` only if set; one `<variante>` per variant, one `<bibelstelle>` per Bible reference
  (at most 32 per term; text as stored, i.e. normalised). All text elements are of type `Text` (Base64 with
  forbidden characters). Among the accepted terms every key (`term_key`: NFC, whitespace, lower case) is allowed only
  once (the reader checks this); a proposal may have the same key.
- `<satz>` additionally carries (missing = off):

| Field (JSON) | Attribute(s) | Values |
| --- | --- | --- |
| `glossary_page` | `glossarseite` | `vor-registern`, `nach-registern` |
| `index_pages` | `registerseiten` + `registerseiten-spalten` | identifiers separated by spaces (1–16) + `1`–`3` |
| `glossary_first_mention` | `glossar-erste-erwaehnung` | `true`/`false` |

  `registerseiten` stands only together with `registerseiten-spalten` (otherwise an error with the path); the reading in checks the values
  with `TypesetSettings::validate`.
- **Not in the format:** index proposals (`index_proposal`) – they are a workflow of the instance (like open
  requests) and carry no identifier. Index marks in the text (`#metadata((…))<baruch-idx>`) travel as part of the revisions.

**Tombstones.** Deleted or discarded terms leave `<eintrag art="glossar-begriff">` (database
`glossary_term`), deleted own indexes `<eintrag art="register">` (`index_register`), both with the work uuid
(also when the work is deleted). The kinds exist only from minor version 7 on: in a file with `version="1"` to `"6"` they
are an error. A reader of minor version 7 reports in a file of minor versions 1 to 6 `werk/eigene-register`,
`werk/glossar` and the four new attributes of `<satz>` (`werk/angaben/satz/@glossarseite` …) as unknown and
does not adopt them (test `files_of_version_six_leave_the_u7_parts_unknown_and_refuse_their_tombstones`). A reader of
minor version 6 (Round 29) reads a file of minor version 7 like a higher minor version: it reports these parts,
skips the new tombstones, and only the glossary, own indexes and the new typesetting fields are lost in the process.

**Reading in.**

- *New:* indexes and terms (accepted and proposed) come into being with identifier, origin, status and times from the
  file; in the report `added.registers` and `added.glossary_terms` (pieces `register`, `glossar-begriff`).
- *Synchronising (matching instead of doubling):* a known identifier is **never overwritten** (difference: hint with
  both states). An unknown identifier is added – unless there is already an index with the same identifier here
  (`kennung`) or a term with the same key (an accepted one blocks every other, a proposal blocks another
  proposal): then a hint, no duplicate. If the identifier has a tombstone here, it stands under „hier
  gelöscht“ and does not come back; with 13 own indexes already, a new one stands under „Nicht übernommen“. Terms
  with fields outside the length limits (`MAX_GLOSSARY_*` per term) stand under „Nicht übernommen“, the rest
  comes. Checked since Round 23 (see "Round 23 – Hardening" at the end; open before, review R31): the number of terms per work (`MAX_GLOSSARY_TERMS`) and the length
  of an index title (`MAX_REGISTER_TITLE_CHARS`) – a file can exceed both.
- Proven in `crates/baruch-exchange/tests/{read,write,xsd}.rs`, `crates/baruch-store/tests/exchange_apply.rs`
  (`a_new_import_carries_registers_and_glossary_and_dumps_them_back`,
  `merging_the_glossary_matches_instead_of_doubling_and_never_overwrites`) and
  `crates/baruch-server/tests/round31_glossar.rs` (`two_instances_exchange_glossary_and_registers_without_doubling`).

## Amendments, draft and state (minor version 8, Round 22)

Owner: spiegel-entwurf. Since Round 22 the human **amends** their own current revision on saving in the editor
instead of creating a new revision every time (contract R22 §2.1; table `revision_amend`, columns
`revision.amend_seq`/`based_on_amend`, migration 0050). Added are the draft as a typesetting setting (watermark
of the outputs) and the state in which a finding was adopted. Minor version **8**: the writer writes
`version="8"` (also in the manifest; this replaces "today `version="7"`" above), XSD `docs/schema/baruch-werk-1-u8.xsd`. The
schemas of minor versions 1 to 7 stay byte-unchanged (SHA-256 in the test, u7 newly added); a file of
minor version 7 passes u8 and u7, an output with amendments or a draft does not pass u7.

**Revision.** At `<fassung>` in the item two optional attributes and at the end (behind `<setzt-um>`) the steps:

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

- `stand` = `revision.amend_seq` (number of steps; missing = 0, never amended), `beruht-stand` =
  `revision.based_on_amend` (state of the basis on which a proposal rests; missing = without). Baruch does not write state 0 (basis never
  amended) – it means the same as "missing"; so files without amendments stay valid under
  the older schemas as well (integrator R22).
- `<fortschreibung>`: `uuid` (identifier of the step), `seq` (from 1, without gaps), `art` (`amend` = amendment by the
  human, `undo` = undoing through the history; the words are named by contract R22 §2.11, equal to the database values),
  `undo-von` (only and always with `undo`: the step whose previous state was restored, smaller than `seq`),
  `autor` (human), `zeit`; children `<vorher>` and `<nachher>` (text before and after the step, type `Text`, i.e. Base64 with
  forbidden characters).
- The reader checks (`read/check.rs`): identifiers unique in the document, steps without gaps from 1, `undo-von` exactly with
  `undo` and smaller than `seq`, `stand` equal to the number of steps.

**Finding.** `<befund>` and `<status-eintrag>` optionally carry `stand` (`finding.adopted_amend` or
`finding_status.adopted_amend`): state of the revision when the finding was adopted (display „Fassung n · Stand k“);
written only if greater than 0.

**Draft.** `<satz>` optionally has the child `<entwurf/>` (typesetting setting `draft`, contract R22 §2.9):

```xml
<satz uuid="…" geaendert="…" geaendert-von="Hanna Probe" kapitelanfang="rechte-seite" …>
  <entwurf text="Probe &amp; &lt;Satz&gt;" zusatz="Stand &quot;heute&quot; · erfunden" datum="true" deckkraft="20" lage="fusszeile" umschlag="true"/>
</satz>
```

| Field (JSON `draft`) | Attribute | Values |
| --- | --- | --- |
| `text` | `text` (mandatory) | 1–40 characters, without control characters |
| `note` | `zusatz` (missing = empty) | up to 120 characters |
| `date` | `datum` (missing = `false`) | `true`/`false` – the date itself never stands in the file (day of the output) |
| `opacity_percent` | `deckkraft` (mandatory) | 5–50 (percent) |
| `placement` | `lage` (mandatory) | `diagonal`, `kopfzeile`, `fusszeile` |
| `cover` | `umschlag` (missing = `false`) | `true`/`false` (watermark also on the cover PDF) |

Without a draft `<satz/>` stays empty as in minor version 7. As with all typesetting settings, the limits are checked not by the
reader but by the reading in (`TypesetSettings::validate`, otherwise „Nicht übernommen“). The Typst rule of the watermark
never stands in the file.

**Tombstone.** `<eintrag art="fortschreibung">` (database `uuid_tombstone.kind = 'revision_amend'`): steps are
never deleted individually, only with their revision or their work (cascade trigger of migration 0050); then
every step leaves a tombstone with the work uuid. The kind exists only from minor version 8 on: in a file with `version="1"`
to `"7"` it is an error.

**Older readers.** A reader of minor version 8 reports in a file of minor versions 1 to 7
`…/fassung/fortschreibung`, `…/fassung/@stand`, `…/fassung/@beruht-stand`, `werk/angaben/satz/entwurf`,
`werk/befunde/befund/@stand` and `…/status-eintrag/@stand` as unknown and does not adopt them (test
`files_of_version_seven_leave_the_u8_parts_unknown_and_refuse_the_amend_tombstone`). A reader of minor version 7
(Round 31) reads a file of minor version 8 like a higher minor version: it reports these parts, skips the
tombstone `fortschreibung`, and only history, states and draft are lost in the process – the text of every revision is the
amended one.

**Reading in.**

- *New:* revisions come with state, state of the basis and all steps (identifiers from the file; cross-references in
  `<vorher>`/`<nachher>` are rewritten to the keys here like the text of the revision), findings and
  history entries with their state, the typesetting settings with the draft. In the report the steps count as
  `added.amends` (piece `fortschreibung`).
- *Synchronising:* a newly read-in revision (rule a, c) brings its history along; a differing revision with a
  derived identifier (rule d) stays without history. A known revision is fast-forwarded according to rule (d0) or stays
  as it is. If an identifier of a step is already taken or buried here, the history of this revision does not come
  along (hint), the revision itself does (state 0).
- A new part that would take over existing chapters becomes a request with identifier and revisions (rule a).
- Proven in `crates/baruch-exchange/tests/{read,write,xsd}.rs` (`version_eight_carries_amends_stands_and_the_draft_exactly`,
  `validate_refuses_amends_with_gaps_wrong_undo_or_a_wrong_stand`, `a_u8_output_with_amends_draft_and_stands_passes_the_u8_schema_but_not_the_frozen_u7`),
  `crates/baruch-store/tests/exchange_u8.rs` (reading in new and back, fast-forward, differing, approving and rejecting a request
  with identifier, „Fassungen“ not in the format) and `crates/baruch-server/tests/round22_abgleich.rs` (two
  instances byte-identical except for `<ausgabe>` and keys, approval with a mirror commit and another synchronisation).

## Round 23 – Hardening (no new minor version)

The format stays at minor version 8; only rules of the reading in have changed.

- On minor version 7: "**Limits.** The title of an own index has – on one line,
  whitespace collapsed – at most 80 characters (`baruch_typst::index::MAX_REGISTER_TITLE_CHARS`); the
  reader (`check_u7`) and the reading in (`exchange_apply::check_u7_limits`, `insert_register`) check this. A work has at most
  5000 glossary terms, proposals included (`baruch_store::MAX_GLOSSARY_TERMS`): when reading in as a new work the
  file counts, when synchronising the stock here together with the terms that would be added (`insert_term`). Exceeding it
  rejects the whole reading in with a German message; nothing stays written (one transaction)."
- Rule (j), sentence on known findings: "A **known** finding is never overwritten: status, reasoning and
  history stay as they are here – the history is the log of the decisions of this instance, and an open
  proposal from the file would otherwise become a decision here that nobody made. If the status differs or
  the file has entries in the history that are missing here, a hint stands there that names their number."
- Minor version 5, last sentence on variants: "… a differing revision that gets a derived identifier carries
  the variant of the file (group, label, line, findings) – since Round 23."
- Cover (rule on blocks): "Blocks are stored and compared in normal form (`check_cover_block`); if
  the check rejects a block, it is adopted unchanged and reported as a hint."
- Minor version 8, fast-forward: "Fast-forwarding happens only if all heading lines of the text (lines with `=` or `#heading(` at the start) stay the same;
  otherwise the revision of the file comes, as before Round 22, as a differing revision."
- New under "Hints on reading in": "If the heading of a current revision just read in does not match the kind
  of its item (chapter level 1, section 2, under a section 3; parts unchecked), the reading in reports a
  hint; the text stays exact. `validate` itself knows no hints and stays unchanged."

## Tree of the mind map (minor version 9, Round 34)

Owner: mindmap_daten. Since Round 34 the ideas of a work form a tree (the mind map; migration 0052, columns
`idea.title`, `idea.parent_id`, `idea.position`, `idea.main_topic`). Minor version **9**: the writer writes
`version="9"` (also in the manifest; this replaces "`version="8"`" above), XSD `docs/schema/baruch-werk-1-u9.xsd`. The
schemas of minor versions 1 to 8 stay byte-unchanged (SHA-256 in the test, u8 newly added); a file of
minor version 8 passes u9 and u8, an output with a tree does not pass u8.

**Idea.** `<einfall>` carries four further optional attributes:

| Attribute | Meaning | missing = |
|---|---|---|
| `titel` | title of the node in the mind map, at most 200 characters (`idea.title`) | empty |
| `eltern` | UUID of the parent idea in the same work (`idea.parent_id`) | no parent node |
| `platz` | order among siblings, integer (`idea.position`); on a tie by creation | 0 |
| `hauptthema` | `true`: free main topic directly at the work (`idea.main_topic`); only without `eltern` and `bezug` | no |

The writer outputs each of the four only if it differs from the default – a work without a tree looks in
minor version 9 as in 8 (and passes the u8 schema as well). The place of an idea follows a rule: with `eltern`
a sub-node of this idea (`bezug` is then only the link to the section); otherwise with `bezug` at the chapter of the item;
otherwise with `hauptthema` at the work; otherwise it lies **in the cloud** beside the mind map (storage for ideas without a place).

```xml
<einfall uuid="26000000-0000-4000-8000-000000000029" status="erledigt" erledigt-als="beitrag" herkunft-art="persona" herkunft="@lektor" angelegt="2026-01-06T10:00:00.000Z" geaendert="2026-01-07T12:00:00.000Z" titel="Lampe am Seil" eltern="26000000-0000-4000-8000-000000000028" platz="2">
```

**Reader.** `read` reads the attributes from `version="9"` on. In a file of minor versions 1 to 8 they stay
unread and stand in `unknown` (`werk/einfaelle/einfall/@titel` …); everything else of the file is read. `validate`
checks: title at most 200 characters, `eltern` a UUID and not the idea itself. An `eltern` that points to no
idea of the document is **not** a format error.

**Reading in** (new and synchronising) in two steps: first all ideas (with title, place and main topic), then the
parent nodes (`exchange_apply::link_idea_parents`) – the parent node may stand behind the child in the file. An
idea is adopted **without a parent node** (it stays at its reference or lies in the cloud), with a hint
in the report, if

- the parent node does not exist here in the work (unknown identifier or idea of another work),
- the parent node is deleted but the idea itself is not (a sub-node deleted with its parent node
  keeps the binding so that restoring takes it along),
- the chain of parent nodes forms a cycle or would be deeper than 32 ideas.

When **synchronising**, new ideas are added together with their tree; an idea already known here stays as it is
(local wins) – if its title differs, „Titel weicht ab (bleibt)“ stands as a hint in the report; parent nodes and
order of known ideas count as the arrangement of this instance and are not compared. The title is in the
full-text index of the ideas (column `words` of `idea_fts`), also after reading in.

**Older readers.** A reader of minor version 8 (0.9.0) reads a file of minor version 9 like every higher
minor version: it reports the four attributes as unknown and adopts the ideas without a tree.

Tests: `crates/baruch-exchange/tests/read.rs` (section "Sub-version 9 (Round 34)"), `xsd.rs` (u9 passes, u8 frozen),
`crates/baruch-store/tests/exchange_u9.rs` (reading in, exporting, detaching with a hint, file of minor version 8),
`crates/baruch-server/tests/round34_daten.rs` (round trip between two instances).

## Work language (minor version 10, Round 38)

Since Round 38 each work has a work language (`de` or `en`; migration 0055, column `work.language`). Personas
answer in it; book output follows it from Round 39 on. Minor version **10**: the writer writes `version="10"`
(also in the manifest; this replaces „`version="9"`“ above), XSD `docs/schema/baruch-werk-1-u10.xsd`. The schemas
of minor versions 1 to 9 stay byte-identical (`-u9.xsd` is frozen from now on).

**Attribute.** `<angaben>` carries one more attribute:

| Attribute | Meaning | missing = |
|---|---|---|
| `sprache` | work language, `de` or `en` (`work.language`) | `de` |

The writer **always** writes `sprache`. A file without it – every file up to minor version 9 – is read as `de`.
An invalid value is an error of the reader, like an invalid value of the other enumeration attributes of the
format (e.g. the kind of work): `…/@sprache: unbekannter Wert „fr“`; nothing is read in. Model: `ExchangeLanguage { De, En }` (default `De`), field `ExchangeWork::language`.

The work language is independent of `<satz sprache>` (`TypesetSettings.lang`, the language of hyphenation); the
two are coupled in Round 39 at the earliest.

**Reading in.** A **new** work takes over the language of the file. When **merging** into an existing work the
language is treated like the field `bible` (Bible translation) of the work details: the local value stays, and
if the file differs the report has the line „Werkangabe „Sprache“ weicht ab (bleibt)“.

**Older readers.** A reader of minor version 9 or older reads a file of minor version 10 like any higher minor
version (see the rule on higher minor versions above); it does not know the attribute, so the work it creates is
German.

Tests: `crates/baruch-store/tests/exchange_u10.rs` (file of minor version 9 → `de`, minor version 10 there and
back), `crates/baruch-exchange/tests/xsd.rs` (frozen schemas).
