---
title: "Features"
description: "Baruch accompanies sermons, talks and books from the first idea through Bible study to the print-ready book – with revisions, Bible checks, assistants that only suggest, and book typesetting."
---

Baruch is a writing environment for sermons, talks and books – from the first idea through Bible study to the
print-ready book, without handing control to an AI. This page shows what belongs to it; at the end there are
[frequently asked questions](#haeufige-fragen). The pictures show a freely invented sample work.

## The workspace {#arbeitsplatz}

On the left the outline, in the middle the editor, on the right panels that open for assistants, revisions,
findings and the chat about the section. The width of the three areas can be dragged. At the top is the
**Ribbon**: the menu **Baruch**, the tabs **Start**, **Insert**, **Review**, **Book** and **View** and below them
the groups of the chosen tab – from saving through the formats to the book as a PDF.

{{< bild datei="arbeitsplatz.png" alt="Workspace of Baruch: on the left the outline of the sample work, in the middle a section in the typeset view, on the right the panels for assistants, revisions and chat." titel="The workspace with outline, editor and the panels on the right." >}}

On a laptop space gets tight. The **Laptop mode** (tab **View**: Automatic, On, Off) gathers the groups of the
ribbon into buttons with a drop-down list below a window width of 1600 pixels, and below 1280 pixels it folds the
outline and the right column into narrow bars that lie over the text when needed. That way the space stays with
the text. More in the [help on the laptop mode](/en/hilfe/menueband/#laptop-modus).

{{< bild datei="arbeitsplatz-laptop.png" breite="1366" hoehe="768" alt="Workspace of Baruch in a window 1366 pixels wide: the ribbon in its compact form, below it the outline, the editor and the panels on the right." titel="The same workspace at laptop width: the ribbon compact." >}}

## Outline {#gliederung}

- **Parts, chapters and sections** as a tree, each item with or without a number. Markdown headings become
  outline items while you type (`# Title` becomes a chapter).
- **Thirteen numbering schemes**, switchable at any time: decimal, Roman, "Part I · Chapter 1", legal,
  **sermon** (Introduction · I. · II. · Conclusion), **book** or a pattern of your own. The book typesetting uses
  the same numbers.
- **Time keeper:** speaking time per item and for the work, with a target time and a warning – above all for
  sermons. In addition, note sheets as an A5 page or index cards.
- **Requests:** assistants can only request renaming, moving, changing the level and deleting. The human
  confirms or rejects; both are logged.

Step by step: [help on the outline](/en/hilfe/gliederung/).

{{< bild datei="gliederung.png" alt="Outline page: parts, chapters and sections of the sample work with numbers and forms for renaming and moving." titel="The outline as a page of its own – usable without JavaScript." >}}

## Revisions {#fassungen}

Every section has any number of revisions as a tree. Exactly one is the current one; switching works at any time,
nothing is silently pushed aside. Proposals of the assistants are revisions of their own, on request in several
variants side by side. In addition, every change lands in the **Git mirror** of the work – an automatic backup
of all drafts as ordinary text files, in which every step can be traced.

{{< bild datei="fassungen.png" alt="A section with two revisions: shown is the older revision 1, next to it the switch 1/2 and the reference to the current revision 2; on the right the open panel Revisions." titel="Several revisions of a section; exactly one is the current one." >}}

## Writing {#schreiben}

- **Typeset view:** the section appears as it will be typeset and is edited there – paragraphs, headings,
  lists, quotations, footnotes, links, images. Complex elements such as tables or formulas stay in the text as
  editable code blocks; switching back and forth loses nothing.
- **Source text:** the same revision as text in **Typst**, the markup language from which Baruch typesets
  print-ready PDFs (similar to Markdown, but made for books) – switchable.
- **Search and replace** in the section and in the whole work, with whole word, match case and search patterns
  (regular expressions). Replacing creates a new revision per section and can be undone.
- **Cross-references** with fixed anchors; in the PDF and the EPUB they are links.

## Assistants {#assistenten}

- **Assistants with a role of their own** (in Baruch also called "personas"), their own AI model and their own
  connection – editor, exegete, sources, listener or your own. The models run best locally (for example with
  Ollama or another service with an OpenAI-compatible interface); third-party providers only after explicit
  clearance.
- **The editor as a companion:** first it says what holds up, then it names the most important points with a
  concrete next step – for a section or a whole chapter.
- **Solutions instead of mere criticism:** every point of criticism comes with a solution and the button
  **Accept as proposal**.
- **List of findings:** Baruch splits the feedback of the assistants into single points that can be ticked off
  (findings). The human marks them as adopted, rejected, deferred or done; what was rejected is not reported
  again.

{{< bild datei="assistenten.png" alt="Settings, page Assistants: the assistants with role, model, context length and colour." titel="Setting up assistants: role, model, connection and colour." >}}

Setting up: [Set up assistants](/en/mcp/#assistenten-einrichten). Working: help on
[chat](/en/hilfe/assistenten/#chat) and [findings](/en/hilfe/assistenten/#befunde).

## Bible {#bibel}

- **Recognise and check references**, also lists and ranges; times of day and number words are not taken for
  references.
- **Bibles, commentaries and lexicons** from the free **SWORD project** (the library behind programs such as
  BibleTime and Xiphos): once installed, they are on your own computer and can be read, searched and quoted,
  with their licence note. Baruch itself ships no Bible texts.
- **Versifications:** conversion between German and English verse numbering, for example in the Psalms.
- **Check quotations:** in the editor, Bible quotations are marked as **matches** or **differs**.

Setting up: [Set up Bibles](/en/installation/#bibeln-einrichten).

## Ideas, literature and search {#einfaelle}

- **Ideas** with title, keywords, source and Bible reference; the **mind map** shows them as branches on parts
  and chapters, with child nodes, links and contradictions. Ideas without a place lie in the cloud next to it.
  Exchange with XMind, FreeMind and Freeplane.
- **Author's library** with citation keys, quotations and full-text search across the work and the literature.
- **Semantic search:** finds passages by meaning, not only by wording ("comfort in suffering" also finds "God
  wipes away the tears"). The search index is on your own computer; further sources are optional.

## Typesetting, book and e-book {#buchsatz}

- **Book typesetting in the settings:** chapter opening, page format, margins and gutter, font, hyphenation,
  running heads, footnotes – with a preview and explanations of the terms.
- **Book PDF** with front matter, **cover editor** with guides for bleed and spine, print check.
- **Index and glossary**, **e-book** in the standard format EPUB 3 with a preview for several reading devices
  and a check for errors and accessibility.

Step by step in the help: [Book as a PDF](/en/hilfe/buchsatz/#pdf), [Cover](/en/hilfe/buchsatz/#umschlag),
[E-book](/en/hilfe/buchsatz/#epub).

{{< bild datei="satz.png" alt="Typeset preview: pages of the sample work, typeset with Typst according to the typesetting settings of the work." titel="The typeset preview shows the book as Typst typesets it." >}}

{{< bild datei="einstellungen.png" alt="Settings, page Work: details of the work and the block Typesetting with page format, margins and font." titel="Settings of the work, among them the book typesetting." >}}

## Import, export and exchange {#austausch}

- **Import:** Typst, Markdown (also as a ZIP with images), PDF, EPUB and Word (.docx) – always with a preview of
  the outline, created only after confirmation (help: [Importing existing texts](/en/hilfe/einlesen/),
  [Word](/en/hilfe/einlesen/#word)).
- **Export:** Typst file or folder with images, book PDF, cover PDF, EPUB, Word (.docx), note sheets.
- **Typst command line included:** `baruch-typst compile buch.typ` typesets the exported Typst folder – the same
  program with the built-in typesetting engine, without a second Typst and without the net (see
  [Installation](/en/installation/#typst-kommandozeile)).
- **Word export for the publisher:** Baruch writes the work as a Word file (.docx) with styles for headings, real
  footnotes and embedded images – on request with the open proposals of the assistants as tracked changes, which
  the publisher accepts or rejects in Word.
- **Exchange between two installations:** the `.baruch` file carries a work completely, but no keys. When
  comparing, only what is missing is added; nothing is deleted or overwritten.

## Editing: accounts and roles {#konten}

- **Alone on your own computer** Baruch needs no account and no sign-in.
- **With editors:** the author sets up an account, creates editors and shares single works with them. Editors
  sign in in the browser – intended through a web server in front with encryption (reverse proxy, see
  [Operation](/en/betrieb/#reverse-proxy)).
- **An editor reads and suggests:** proposals for revisions, posts in the section chat and findings, all under
  their own name (`@lektorat:<name>`) and traceable in the Git mirror. What counts is decided by the author:
  accepting, changing and deleting stay reserved to them, as do assistants, export and settings.
- **Strictly locked:** for editors everything is forbidden that is not explicitly allowed – including new
  functions of future versions.

Setting up and working: help on [accounts](/en/hilfe/konten/#einrichten) and
[editors](/en/hilfe/konten/#lektoren).

## Frequently asked questions {#haeufige-fragen}

**Which systems does Baruch run on?**
Baruch is a small program that runs on your own computer and is used in the browser – no cloud, no account with a
provider. There are packages for Linux (Debian and Ubuntu, AppImage), macOS and Windows; the packages for macOS
are signed and notarised by Apple, the package for Windows is not signed yet. See [Installation](/en/installation/).

**How quickly can I start?**
Writing, revisions, outline and book typesetting work right after the installation; the typesetting system is
built in, print-ready PDFs need no additional program. Two things you set up yourself if you need them:
**Bibles** from the SWORD project (easiest with BibleTime, Xiphos or the package `sword-utils`; Baruch then finds
them by itself) and **assistants**, that is an AI model (locally, for example, with Ollama). There is no guided
setup wizard yet; the steps are under [After the installation](/en/installation/#nach-der-installation).

**Do I need an AI?**
No. Without a model Baruch is a complete writing and typesetting environment; the assistants are an offer.

**What is MCP?**
An open interface (Model Context Protocol) through which AI programs such as Claude Code can work with Baruch –
reading and with proposals, never with the last word. See
[Connecting AI programs with Baruch](/en/mcp/#mcp-clients).

The complete description is in the [help](/en/hilfe/) – step by step, with pictures. For developers: the
{{< doku "README.md" "README" >}} of the project.
