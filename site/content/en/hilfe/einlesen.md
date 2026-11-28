---
title: "Reading in existing texts"
description: "Taking over an existing text as a work: from Typst, Markdown, a folder or ZIP, from PDF, EPUB or Word."
weight: 20
---

Whoever already has a text need not type it again. Baruch reads five kinds of files and creates a **new work**
from them. For PDF, EPUB and Word, Baruch first shows a preview of the outline; the work is only created after
confirmation. Everything works without JavaScript as well. Assistants cannot read anything in – a text read in
counts as the text of the human, and only the human creates it.

| File | Way in the menu Baruch ▾ |
|---|---|
| Typst (`.typ`), Markdown (`.md`, `.markdown`), ZIP, folder | New work / Import… |
| PDF, EPUB | Import PDF or EPUB… |
| Word (`.docx`) | Import Word (.docx)… |
| Exchange file of another Baruch | Exchange: Read in… – see [Exchange](/en/hilfe/austausch-sicherung/) |

## Reading in Typst, Markdown, ZIP or a folder {#typst-markdown}

{{< hilfebild name="einlesen-bericht" alt="The report after reading in a Markdown file: name of the new work, number of sections and pictures, below them the notices with line numbers and the link to the work." titel="The report names what was adjusted or left out while reading in." >}}

Typst is the typesetting format in which Baruch keeps the texts: ordinary text with a few signs for headings and
emphasis. Markdown is a similar, widespread format; Baruch converts it.

How it works:

1. Choose **Baruch ▾ → New work / Import…**.
2. Under **Import a Typst or Markdown file**, choose a *file* (`.typ`, `.md`, `.markdown` up to 16 MiB, `.zip` up
   to 80 MiB) **or** a *folder*.
3. Enter a title if you like. With JavaScript the field is filled in from the file; if it stays empty, Baruch
   takes the title from the file, otherwise the file name.
4. Press **Import**.

If there is nothing to remark, the new work opens. Otherwise a **report** shows which pictures were skipped (with
the reason) and what could not be taken over exactly (with the line), and links to the work. Nothing is
discarded silently.

What becomes of the file:

- Everything **before the first heading** becomes the work template (page, fonts, own formats).
- Headings of the first level become **chapters**, of the second **sections**, of the third **subsections**. The
  text from one heading up to the next becomes revision 1 – by the human, current at once.
- Title and author come from the details in the file, otherwise the file name applies.
- Deeper headings (fourth level and deeper) and indented headings stay ordinary text in a Typst file.
- The same file a second time is refused (also when renamed). If the title is already taken, the form appears
  again: enter another title and choose the file once more.
- The file must be text in the UTF-8 encoding.

**Pictures:** a single file brings only the text along. If the pictures are to come along, upload the whole
**folder** or a **ZIP**: in it lies exactly one main file (with several, `index.md`, `main.typ` or `README.md` on
the top level counts) and next to it the pictures, as the text names them. Missing pictures, pictures over 8 MiB
and files that are no pictures are listed in the report with the reason; files the text does not name as a
notice. A ZIP may contain at most 2000 files, each at most 16 MiB. Baruch refuses an unsafe archive (for example
with links or paths that lead out of the archive) as a whole; then nothing is created. Pictures can also be added
later on the page "Images", see [Writing, pictures and revisions](/en/hilfe/schreiben/#bilder).

### What becomes of Markdown {#markdown}

| Markdown | becomes in the work |
|---|---|
| `#`, `##`, `###` | chapter, section, subsection; deeper levels become subsections (notice) |
| text before the first heading | an own first chapter with the title of the work (notice) |
| header with `title:` and `author:` | title and author of the work; other details: notice |
| `*kursiv*`, `**fett**`, `~~durch~~` | emphasised, bold, struck through |
| lists and enumerations, nested ones as well | lists and enumerations |
| `> Zitat` | quotation block |
| code | code |
| `[Text](https://…)` | link; links to other files or anchors: only the text (notice) |
| footnote `[^1]` | footnote at the place of the reference |
| picture `![Alt](bilder/a.png "Titel")` | picture with alternative text, with a title as a figure with caption; pictures from the network become a link (notice) |
| table | table with header and alignment |
| definition list | definition list |
| HTML | a line break stays; other markup is dropped, the text stays (notice) |

Characters that mean something in the typesetting format (for example `#`, `*`, `_`, `$`, `@`) stay literal text.

### Parts and chapters without a number in Typst files {#teile}

Baruch understands two special lines in a Typst file in exactly this form:

```typst
#heading(level: 1, numbering: none)[Erster Teil] <baruch-part>
#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>
```

The first becomes a **part** (the following chapters stand below it and are counted on across parts), the second
a chapter **without a number** (with `level: 2` or `3` a section or subsection without a number). On export Baruch
writes the same lines out again.

### Reading in an exported work again {#export-import}

What Baruch writes from the settings into the file on export comes back to its place when reading in; every
intervention is listed in the report:

- The **typesetting settings** become typesetting settings of the new work again. If they were changed by hand in
  the file, they stay part of the work template.
- **Headings without a number** keep their "number off".
- The generated pages for **index and glossary** at the end of the book and the automatic additions (marks at Bible
  references, markup of the first mention of a glossary term) are dropped – Baruch generates them anew.
- Index, glossary and the setting "cross reference with page" are not in the file and do not come along. That is
  what the [exchange file](/en/hilfe/austausch-sicherung/) is for.

### From the command line {#kommandozeile}

```sh
baruch-server import buch.typ                 # also heft.md or buch.zip
baruch-server import buch.typ --title "Other title"
```

From the command line Baruch reads the pictures from the folder of the file as well. The output names work,
sections, pictures, skipped pictures and every notice.

## Reading in PDF or EPUB {#pdf-epub}

{{< hilfebild name="einlesen-pdf-epub" alt="The form Import PDF or EPUB: file choice, the choice of the way (automatic, local, through MultiGPT), a field for the title and the button Read and show outline." titel="Step 1: choose the file and have it read." >}}

From a finished book – PDF or EPUB, at most 64 MiB – Baruch proposes an outline. From it comes a work, a source for
the literature or both.

How it works:

1. Choose **Baruch ▾ → Import PDF or EPUB…** (the link is also found below the form "New work").
2. Choose the file, enter a title if you like, press **Read and show outline**.
3. Check the **preview** and change the outline if needed (see below).
4. Choose **Take over as** work, source of the library or both; title and author are filled in from the file.
   **Take over** leads to the new work or to the source. **Discard** deletes the draft; then nothing is created.

{{< hilfebild name="einlesen-pdf-vorschau" alt="The preview after reading a PDF: at the top the chosen way and the notices, below them a text field with the title for each outline item, the number of characters and a text sample; at the bottom the choice Take over as." titel="Step 2: check and take over the outline." >}}

**Changing the outline in the preview:** every item is one line.

- `= Titel` is a chapter, `== Titel` a section, `=== Titel` a subsection. You change the title in the field, the
  level with more or fewer `=`.
- An emptied line removes the item; its text goes to the previous item. Nothing is lost.
- `= Teil: Titel` makes a part – only this way, even when Baruch suspects a "possible part". (The prefix `Teil:`
  is German for now.)
- The order stays as in the file. Text before the first item is found in the item `Vorspann` (front text).

**Which way?**

- Baruch always reads an **EPUB** itself. Paragraphs, emphasis, footnotes, lists, quotations, code and pictures
  are taken over; the outline comes from the table of contents of the EPUB.
- **PDF locally:** Baruch reads the file on this computer with the free program Poppler; nothing leaves the
  computer. If Poppler is missing, Baruch says so. Text and headings are taken over (detected by the font size or
  by numbers such as "1.2 Title" – the number is dropped, Baruch counts by itself); page numbers are dropped,
  emphasis, footnotes and pictures do not come along, tables only as lines of text. At most the first 2000 pages
  are read.
- **PDF through MultiGPT** (an own document service, if set up under Settings → Connections): the file is uploaded
  there – only if the check box "upload to MultiGPT" is ticked explicitly. The outline then comes from the table
  of contents that MultiGPT detects.
- **automatic:** MultiGPT if it is set up, a collection is entered and the upload is ticked, otherwise locally.
  Which way applies and why is shown at the top of the preview.
- If MultiGPT fails, the reason is shown together with a button **Read locally**; the file need not be chosen
  again. Baruch never changes the way by itself.

**As a source of the library:** title and author come from the file. When read locally, Baruch keeps the whole
text; this way the source can be read later, and quotations can be checked against it (see
[Bible, literature and sources](/en/hilfe/bibel-quellen/#quellen)). The same file a second time as a source is
refused. The steps for a source alone are described under
[Reading in a source](/en/hilfe/bibel-quellen/#quelle-einlesen).

If the title is already taken, the preview stays with a message. Baruch keeps drafts at most 24 hours, at most 20
at a time.

From the command line (always locally):

```sh
baruch-server import buch.pdf                             # only show outline and notices
baruch-server import buch.pdf --uebernehmen               # create a work with this outline
baruch-server import buch.epub --bibliothek               # only as a source in the library
baruch-server import buch.epub --uebernehmen --bibliothek # both
```

## Reading in a Word file {#word}

{{< hilfebild name="einlesen-word" alt="The form Import Word (.docx) with file choice, a field for the title and the button Read and show preview." titel="Step 1: choose the Word file." >}}

**Baruch ▾ → Import Word (.docx)…** reads a Word file (`.docx`, at most 64 MiB) and creates a new work from it.
What is special: the **tracked changes** and the **comments** from Word are not lost – they become proposals and
posts.

How it works:

1. Choose the file, enter a title if you like, press **Read and show preview**. Only a draft is created; the file
   stays on this computer.
2. Check the preview: the **outline** (parts and headings of levels 1 to 3, each with length and text sample), the
   **Editors** from the tracked changes (name in Word, name in Baruch, number of insertions and deletions), the
   number of comments, footnotes and pictures and all **notices** about what is not taken over or only in a
   simplified form.
3. Press **Adopt as a work**. The title can still be changed; two check boxes switch off adopting the changes or
   the comments. **Discard** deletes the draft. Drafts expire after 24 hours.

{{< hilfebild name="einlesen-word-vorschau" alt="The preview of a Word file: the outline with text samples, the table of editors with insertions and deletions, the number of comments, footnotes and pictures and the list of notices." titel="Step 2: check outline, editors and notices." >}}

**Changes and comments never become the valid text by themselves:**

- **Revision 1** of every section is the state **before** all tracked changes. It is the current revision.
- The changes of **each editor** become **one open proposal** per changed section, with a note naming the changes
  from Word and the editor. If two editors changed the same section, there are two proposals. Only the human can
  accept, reject or merge – as with every proposal.
- **Headings** never change the outline: a heading changed in Word becomes a post at the section. A heading
  inserted by the editor creates no section (its text appears in bold in the proposal), a deleted one stays; both
  are listed as a notice.
- **Comments** become posts at the section of the commented place: name and date, the place as a quotation (at
  most 200 characters), then the comment; replies follow directly. A comment without a place that can be found is
  put at the first section.

In Baruch the editor is named after his name in Word, in lower case and without umlauts, with "@word:" in front –
`Jörg Müller` becomes `@word:joerg-mueller`.

**What is taken over:** headings and title, bold, italic, underlined, struck through, superscript and subscript,
small capitals, links, footnotes, block quotations, lists (nested ones as well), tables, embedded pictures with
alternative text and caption, page breaks. Text before the first heading gets an own first chapter; a document
without any heading becomes one chapter.

**What does not arrive or arrives simplified** (each with a notice): headings of levels 4 to 6 (they appear bold in
the text), text boxes, drawings and embedded objects, formulas (only as plain text), tables with merged cells,
pictures in unsupported formats, pictures that are only linked (Baruch never fetches them), hidden text. Endnotes
become footnotes. Moved text appears as a deletion at the old and an insertion at the new place.

**Not read** are the old format `.doc` and files with a password.

**Not checked:** so far no file written by Microsoft Word or LibreOffice has been read in; checked are files built
by hand according to the standard and files that Baruch wrote itself.

From the command line:

```sh
baruch-server import manuskript.docx                # only the preview, nothing is created
baruch-server import manuskript.docx --uebernehmen  # create the work with its proposals and posts
```

The reverse way – the work as a Word file for the publisher – is described in
[Typesetting](/en/hilfe/buchsatz/#word-export).
