---
title: "Typesetting: PDF, e-book and cover"
description: "Typesetting and writing out the work: typeset preview, PDF, cover and front matter, e-book, note cards, Word file for the publisher."
weight: 100
---

The manuscript becomes a book: Baruch typesets the work with the built-in typesetting program **Typst** – pages,
margins, font, page numbers – and writes it out as a PDF, as an e-book or as a Word file. Everything is found in
the tab **Book** and works without JavaScript. The **current revisions** are always typeset.

## Typeset preview {#satz}

{{< hilfebild name="satz-vorschau" alt="The typeset preview of the sample work: a typeset book page large in the middle, above it the pages of the chapter as links; in the ribbon the groups Back, Download, Pages with Page n of N and Typesetting run." titel="The typeset preview shows the work page by page as it would look in print." >}}

How it works:

1. Choose **Book → Typeset preview**. The page on which the open section begins is shown.
2. Page with **‹** and **›** in the ribbon ("Page n of N") or click on a page of the chapter at the top.
3. **Download PDF** downloads the whole book.

- **Messages** of the typesetting program appear above the page: "Error" (red) or "Warning" (yellow), with the
  section as a link back into the editor. In case of an error there are no pages, only the messages.
- **Fonts:** built in are Libertinus Serif, New Computer Modern and DejaVu Sans Mono, plus all fonts installed on
  the computer. If a font of the template is missing, Baruch continues with an available one and warns.
- Baruch does not load **extension packages from the Internet**; a template that asks for one results in an error.
- **Duration:** typesetting a big book takes seconds. After 60 seconds the page reports that it is taking longer.
  **Cancel typesetting** (group "Typesetting run") cancels the typesetting of this work; **Typeset again** starts
  it again.
- **Parts** stand alone on a right-hand page of their own, with their number above the title.
- **Print check** (at the bottom of the page): missing pictures, pictures with less than 300 dpi at their printed
  size (with the number of pixels needed), broken cross references and places where the work template contradicts
  a typesetting setting. If everything is fine, it says so.

This page loads no JavaScript. Therefore the sign for the theme leads to Settings → Display here, and there are no
keyboard shortcuts on it.

## Typesetting settings {#satz-einstellungen}

{{< hilfebild name="satz-einstellungen" alt="Settings, page Work, block Typesetting: the groups Chapters and parts, Page and margins, Font and paragraph, Page numbers, running heads and footnotes, Cross references, Index and glossary, Draft; at the bottom the buttons Preview, Save and Reset." titel="The typesetting settings of a work – with a preview on a sample text." >}}

What the book looks like is decided first by the work template. What you often want to change can be set without
knowing the typesetting format: **Settings → Work → Typesetting**. Every field starts with **"from the work
template"** or is empty – then what the template says applies.

| Group | Fields |
|---|---|
| Chapters and parts | chapter start (continuous, new page, new right-hand page); part page (right-hand or any page, "Back of the part page blank") |
| Page and margins | page format (A5, 120 × 190 mm, 135 × 215 mm, A4, own format); margins inside, outside, top, bottom and gutter in mm – all four margins or none |
| Font and paragraph | base font, font size, line spacing, paragraph (indent or spacing), justification, hyphenation, language, avoid orphans and widows |
| Page numbers, running heads, footnotes | page numbers (bottom centre, bottom outside, top outside, none; counting from the first page or from the first chapter); running heads (off, work title, chapter); footnotes (1, 2, 3 · *, †, ‡ · a, b, c; continuous or anew per chapter) |
| Cross references | reference with page number: default for the check box in the window "Reference" |
| Index and glossary | glossary page, index pages, mark the first mention – see [Index and glossary](/en/hilfe/suchen/#register) |
| Draft | watermark "Draft" – see below |

The technical terms briefly explained: the **gutter** is the allowance for the binding – what disappears in the
fold and the glue would otherwise be missing at the inner margin. The **running head** is the header line above
the text. An **orphan** is the first line of a paragraph standing alone at the bottom of a page, a **widow** the
last line of a paragraph alone at the top of the next page.

- **Preview** typesets an invented sample text with the values in the form, **without saving**, and shows the pages
  as small pictures. With JavaScript it appears by itself shortly after every change.
- **Save** takes over all values. Baruch refuses invalid values; the message appears at the field (for example
  "Font size: 6 to 24 pt").
- **Reset** (with a check box) deletes all typesetting settings; afterwards only the work template applies again.
- **View the whole work typeset** opens the typeset preview.
- **Notes on the work template:** if the template sets the same thing itself, a note with the line of the template
  appears at the field. Usually the setting applies anyway; if it has no effect, the note has a yellow border.
- **Taking over the chapter start from the template:** if Baruch detects a rule for the chapter start in the
  template, the block offers to take it over as a setting and to remove the line from the template (with a check
  box).
- Chapter start, part and blank pages carry no running head; part and blank pages no page number.
- The note cards stay untouched by the typesetting settings.

**Typeset view in the browser:** the block **Typeset view** above only sets what the text looks like while writing
on the screen (font, size, text width, colours) – not the printed book.

## Book as a PDF {#pdf}

{{< hilfebild name="satz-vorschau" alt="The typeset preview with the button Download PDF in the group Download of the ribbon." titel="The PDF always has exactly the pages of the typeset preview." >}}

How it works:

1. Choose **Book → Output → PDF** (or **Download PDF** in the typeset preview, or "PDF" in the list of works).
2. The browser downloads the book as a PDF file.

The PDF contains the work template, the current revisions and the pictures – and, if switched on, cover front and
back and the front matter. PDF and typeset preview always have the same number of pages.

- **Never half a file:** if the typesetting fails, a page with all messages and the link to the section appears
  instead of the PDF.
- If a PDF is already being created for the same work, Baruch asks you to wait a moment.
- **Draft:** **Book → Output → Draft** downloads the same PDF with a watermark ("DRAFT" in an English work,
  "ENTWURF" in a German one) – good for passing on before completion. Under **Settings → Work → Typesetting →
  Draft** the watermark can be switched on permanently and adjusted: text, addition, date, opacity, position
  (diagonal, header, footer) and "also on the cover PDF". It does not change the page breaks. In the typeset
  preview there are also **Download as a draft** and **Download without a watermark**.

From the command line:

```sh
baruch-server export mein-buch --pdf mein-buch.pdf
```

An existing file is never overwritten.

## Cover and front matter {#umschlag}

{{< hilfebild name="umschlag-editor" alt="The cover editor: the whole cover as a preview – on the left the back, in the middle the spine, on the right the front with a cover picture –, above it the frames of the text blocks and the guide lines for bleed and safety margin; next to it the values of the chosen block." titel="The cover editor: placing blocks freely on front, spine and back." >}}

**Choosing the cover picture:** on the page **Images** (button **As cover**) or under **Settings → Work → Cover**.
Assistants can only request this.

**The cover editor** (**Book → Cover**) shows the whole cover for the printer: on the left the back, in the middle
the spine, on the right the front, outside the bleed. Above it lie the **frames of the blocks** (title, subtitle,
author, publisher, blurb, short biography with photo, ISBN with barcode, motto, free text) and the **guide lines**
(bleed, trim edge, safety margin 5 mm, spine edges, centres).

How it works:

1. Drag a frame with the mouse; the handles at the corners change the width. The frame snaps to the trim edge,
   the safety margin and the centre.
2. Next to it, unfold and change the values of each block: position, width, font size, line spacing, alignment,
   colour, font, background (none, shadow, gradient), visible yes or no.
3. **Save** (Ctrl+S, Mac: ⌘S).

| Key | Effect |
|---|---|
| Tab | selects the next frame |
| Arrow keys | move by 0.5 mm, with Shift by 5 mm |
| Ctrl+← and → (Mac: ⌘← and ⌘→) | change the width (at the spine Ctrl+↑ and ↓, Mac: ⌘↑ and ⌘↓) |
| Ctrl+Z (Mac: ⌘Z) | take back the last step |
| Ctrl+S (Mac: ⌘S) | save |
| Esc | clear the selection |

- **Quick choice top/middle/bottom** sets the blocks of the front to the defaults; **Reset arrangement** deletes
  the saved arrangement of the ticked areas.
- **Without JavaScript** the same values are a form with number fields: **Preview without saving** and **Apply
  arrangement**.
- **Spine:** Baruch calculates its width from the number of pages and the **paper thickness per sheet** (one sheet
  is two pages; default 0.2 mm, usual book paper has about 0.1 mm). The calculation is shown with the dimensions.
- **Cover PDF** (**Book → Cover → Cover PDF**) is this cover as one page for the printer.
- **Print check** below the preview: a block extends beyond the trim edge, lies too close to the edge, covers the
  barcode, is wider than the spine, has too little contrast to the picture or a missing font.

{{< hilfebild name="satz-buchangaben" alt="The block Book details with invented details: publisher, place of publication, year, edition, ISBN, price, series, printing, paper thickness, bleed, the check boxes Set front matter and Show the cover front and back in the typesetting, below them dedication, blurb and short biography." titel="The book details fill front matter, imprint and cover." >}}

**Book details** (**Settings → Work → Book details**, also on the page of the cover): publisher, place of
publication, year, edition, ISBN (the check digit is checked), price, series, printing, address of publisher and
printer, photo of the author, paper thickness, bleed (default 3 mm), dedication, blurb, short biography, rights
notice of the Bible translation and picture credits.

- **Cover in the book:** with the check box "Show the cover front and back in the typesetting" (on by default) the
  first page of the PDF is the front, the second the empty inside, the last the back (blurb, short biography with
  photo, ISBN with barcode, publisher and price).
- **Set front matter** adds half title, main title and imprint at the front, with a dedication also a dedication
  page. These pages carry no page number.
- **Own front matter:** whoever writes the work template himself can design the four pages there; the button
  **Adopt the imprint template into the work template** appends the building blocks for it.
- **Imprint:** with the front matter switched on, Baruch checks whether the usual details are there (publisher or
  editor with address, printer with address, year of publication, rights notice of the Bible translation). This is
  **no legal advice**: what is required in the individual case is checked by the publisher or the printer.

## E-book (EPUB) {#epub}

{{< hilfebild name="epub-vorschau" alt="The EPUB preview: the same beginning of the book side by side in several frames the size of a phone, a tablet and an e-reader; above them the choice of the readers, the file, the footnotes and the night mode." titel="The EPUB preview shows the e-book in several readers side by side." >}}

**EPUB** is the usual format for e-books; almost every e-reader and every reading app opens it.

How it works:

1. **Book → Output → EPUB** downloads the e-book.
2. Beforehand, **Book → Typesetting → EPUB preview** is worth a look: it shows the e-book in several readers side
   by side. Choose readers with check boxes, plus the file (part, chapter, front matter), the kind of footnotes and
   the night mode, then **Show**.

- **Division:** every part and every chapter starts on a new page in the reader.
- **Footnotes:** default "popup" – readers that can do it show the footnote when tapped; all others find it at the
  end of the chapter. Alternatively all footnotes appear as "Notes" at the end.
- **Pictures** come along with alternative text and caption.
- The **cover picture** is the typeset front of the cover; without a cover picture the title appears on a dark
  area. With the front matter switched on, title page, imprint and dedication follow.
- **What cannot be represented in the e-book** (formulas, own commands of the template) Baruch never leaves out
  silently, but reports it.
- **Kindle:** there is no separate Kindle format; "Send to Kindle" accepts the EPUB and converts it.
- **The preview is an approximation:** Baruch imitates font, size, margins and night mode of the readers without a
  real device. The device itself remains authoritative.

{{< hilfebild name="epub-pruefung" alt="The check report of the EPUB: at the top the summary No errors with the number of warnings and notes, below it the list of findings with kind, message, place and code and the button Run EPUBCheck." titel="The check report names errors, warnings and notes on the e-book." >}}

**Checking:** below the preview the check report appears in short; **Full check report, run EPUBCheck …** opens it
in full. Checked are the structure of the file, accessibility (language, title, alternative texts of the
pictures), broken links and – per reader – what it cannot do. **Run EPUBCheck** additionally checks with
EPUBCheck, the checking program of the W3C community, if it lies on the computer (see
[Settings → Data](/en/hilfe/einstellungen/#daten)). Baruch never downloads it itself; if it is missing, it says
honestly "EPUBCheck not found".

## Note cards {#stichwortzettel}

{{< hilfebild name="satz-stichwortzettel" alt="The page Note cards: at the top the choice of format and font size, below it the preview of the cards – per outline item number and title, time mark, duration, key points and Bible references – and the button Download PDF." titel="The note cards: one card per outline item for speaking freely." >}}

The **note cards** are meant for speaking freely, above all for sermons.

How it works:

1. Choose **Book → Output → Note cards …**.
2. Choose the format – **A5 portrait** or **cards (A6 landscape)** – and the font size (14 to 28 points),
   **Apply**.
3. Check the preview and **Download PDF**.

One card is created per outline item: number and title, **time mark** and **duration** (from the
[time keeper](/en/hilfe/schreiben/#zeitwaechter)), the key points and the Bible references.

**Where the key points come from** – per section the first rule that finds something applies:

1. **Own keywords:** lines in the source text of the form `// Stichwort: …`. They appear in no book.
2. **Emphasis:** otherwise the places set in bold.
3. **First sentence per paragraph:** otherwise the first sentence of every paragraph.

Per section there are at most 6 points, each at most 160 characters. Which rule applied is shown in the preview
below every card.

## Word file for the publisher {#word-export}

{{< hilfebild name="satz-word-export" alt="The page Word export: a short explanation of what is in the file, the check box for open proposals as tracked changes with the number of open proposals and the button Download Word file." titel="The work as a Word file, if you like with the open proposals as changes." >}}

Many publishers work with Word. **Book → Output → Word …** opens a page with an explanation; **Download Word file**
delivers the work as `.docx`.

- The file contains the front matter from the book details and all sections with their current revision: headings
  with the Word styles "Heading 1–3", bold and italic, links, **footnotes as real footnotes**, quotations, lists,
  simple tables and embedded pictures.
- The numbers of the outline are not in the file; the publisher sets up the numbering with the styles.
- What Word cannot represent (formulas, own commands of the template, boxes) appears in the file as a field with a
  grey background and the readable content. Nothing is lost silently.
- **Open proposals as tracked changes:** with the check box, for each section the most recent open proposal appears
  in the file as insertion and deletion (tracked changes in Word), with the handle of the assistant as the editor.
  "Reject all" gives the current revision, "Accept all" the proposal.
- The export changes nothing in the work.
- Quotations in the text carry no quotation marks in Word; posts from the chat do not go along.

**Not checked:** so far none of these files has been opened in Microsoft Word. Checked is that Baruch can read its
own file in again and that LibreOffice converts a test file into a PDF.

The reverse way is described in [Reading in a Word file](/en/hilfe/einlesen/#word).

## Typst export and own typesetting {#typst-export}

{{< hilfebild name="band-buch" alt="The ribbon with the tab Book; in the group Output there are PDF, Draft, EPUB, Note cards, Word, Export (Typst) and Exchange: Write out." titel="All outputs are found in the tab Book, group Output." >}}

**Book → Output → Export (Typst)** downloads the work as one text file in the typesetting format Typst: the work
template, all current texts, the typesetting settings and – if switched on – index and glossary. Directly after
reading in a Typst file, the export is the input character for character.

The file can be typeset without an own Typst, with the same fonts as in Baruch:

```sh
baruch-server export mein-buch --out ordner     # Typst file and pictures into a folder
baruch-typst compile ordner/mein-buch.typ mein-buch.pdf
```

Further outputs from the command line:

```sh
baruch-server export mein-buch --epub mein-buch.epub
baruch-server export mein-buch --docx mein-buch.docx --vorschlaege
baruch-server export mein-buch --stichwort zettel.pdf --format karten
```

The exchange file for another Baruch is described in [Exchange](/en/hilfe/austausch-sicherung/#ausgeben).
