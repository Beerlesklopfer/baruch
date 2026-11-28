---
title: "Writing, images and revisions"
description: "Write and save in the typeset view, insert images, switch between the revisions of a section and accept proposals."
weight: 50
---

Writing happens section by section. A click in the outline opens a section in the middle of the workspace;
**Save** stores the text safely. Nothing is lost: every saved state is kept as a **revision** or in its history.

## Writing in the typeset view {#satzbild}

{{< hilfebild name="schreiben-satzbild" alt="A section of the sample work in the typeset view: the text stands on a light sheet in the font of the book, with heading, paragraphs and a highlighted box; in the ribbon above it the buttons Typeset view, Source text, Paragraph format, K, F, Save and As new revision." titel="The typeset view shows the section as the work template typesets it." >}}

The **typeset view** shows the section as it looks according to the work template: font, colours, indents, boxes.
You write in it as in a word processor.

How it works:

1. Click a section in the outline on the left.
2. Click into the text and write.
3. **Ctrl+S (Mac: ⌘S)** or the button **Save** stores the text.

| Key | Effect |
|---|---|
| Enter | new paragraph (in lists: new item) |
| Shift+Enter (Mac: ⇧↩) | simple line break |
| Ctrl+I (Mac: ⌘I) or the button **K** (Emphasise) | emphasise (italic) |
| Ctrl+B (Mac: ⌘B) or the button **F** (Bold) | bold |
| Ctrl+S (Mac: ⌘S) | save |

- **Paragraph format** in the ribbon turns the block at the cursor into a paragraph or a heading 1 to 3.
- **Pasting from the clipboard** takes over text only. Blank lines separate paragraphs.
- **Headings are always one line.** Shift+Enter in a heading starts a new paragraph; text with several lines pasted
  into a heading becomes paragraphs below it.
- **Untouched paragraphs stay character for character** as they are; Baruch rewrites only edited blocks. Loading
  and saving without a change gives the same text.
- **Parts** and items **without a number** stand as a line of their own: a part large and centred with "Part" in
  front, an item without a number with the addition "without number". You change the title directly in it.
- **Boxes of the work template** (in the example "Szene" (scene) and "Arbeitsauftrag" (work assignment)) appear as a box;
  every paragraph in them can be edited on its own.
- **Lists, enumerations, term lists and references** appear as what they are. The text of a reference can be
  changed in the typeset view.
- **Places with a yellow background ("islands")** are places the typeset view cannot show reliably: formulas,
  commands of the template, comments. A **double click** opens a field with the source text right there:
  **Ctrl+Enter (Mac: ⌘↩)** or **Apply** takes it over, **Esc** or **Cancel** restores the island. A new island is
  created in the source text.
- Before moving to another place Baruch asks if there is unsaved text on the sheet.
- Very long sections appear a moment after loading.

### Typeset view, source text and preview {#ansichten}

{{< hilfebild name="schreiben-quelltext" alt="The same section in the source text: a text field with the text in the typesetting format Typst, the heading begins with an equals sign, emphasis stands between underscores." titel="The source text shows what is really saved." >}}

- **Typeset view | Source text** (tab **Start**, group "Text") switches the view. The default is always the
  typeset view; only whoever explicitly chooses "Source text" stays there. The **source text** is the text in the
  typesetting format Typst – ordinary text with a few characters: `=` at the start of a line for a heading, `_so_`
  for emphasis, `*so*` for bold.
- **Editor/Preview** (tab **Start**, group "View of the text") shows the **preview** instead of the editor: the
  whole chapter in the reading flow, with the open proposals between the paragraphs.

{{< hilfebild name="schreiben-vorschau" alt="The preview of the workspace: several sections of the chapter one below the other in the reading flow; the switch Editor/Preview in the ribbon is set to Preview." titel="The preview shows the chapter in context." >}}

Without JavaScript there is only the source text in a text field, with **Save** below it.

### Buttons for inserting {#einfuegen}

The tab **Insert** inserts at the cursor – in the typeset view as in the source text:

- always: **Footnote**, **Quotation**, **List**, **Enumeration**, **Term** (term list), **Break**, **Link**,
  **Reference**, **Image**;
- only if the work template knows them: its own formats (in the example **Scene** and **Task**), **Bible passage**
  and **Literature reference**. If the template lacks the format, the button is missing.

A selection moves into what is inserted (for example, selected text becomes the footnote). In the typeset view
Baruch asks for the reference with "Bible passage" ("John 3:16") and for the short name of the source with
"Literature reference". If the typeset view cannot insert a format yet, a note at the bottom right says "… please
insert it in the source"; nothing then happens silently. Undo with Ctrl+Z (Mac: ⌘Z) works as after typing.

**Quotation:** the button inserts a quotation block with one blank line before and after it; the cursor is in the
quotation. You write the reference yourself as a paragraph of its own directly below ("– reference"). If text is
selected in the typeset view, the selection becomes a quotation in the running text.

## Saving and revisions {#fassungen}

{{< hilfebild name="schreiben-fassungen" alt="The right column with the open panel Revisions: for the revision shown the buttons Use this revision, Change revision and Delete, below them the revisions of the section as an indented list with number, status, author and time; the current one is marked." titel="The panel Revisions on the right: switch, change, delete." >}}

A section has any number of **revisions**. Each has a number and is based on another; together they form a tree.
**Exactly one revision per section is "current"** – it is the one in the book, in the PDF and in the export.

**Saving** works as in a word processor:

- **Save** (Ctrl+S, Mac: ⌘S – wherever the cursor is on the workspace) continues your own current revision. It
  keeps its number, its **state** counts up ("Revision 3 · state 2" in the head). Afterwards the cursor is in the
  same place. The feedback reads, for example, "Saved: continued (state 2)."
- **Save as new revision** always creates a new revision – sensible before a larger rebuild.
- A new revision is created by itself if the current revision comes from an assistant (also an accepted proposal),
  if you edited a revision other than the current one, or if the heading or its level changes. The feedback names
  the reason.
- Whoever wants large changes or long pauses to give a new revision by themselves switches that on under
  **Settings → Work → Revisions** ("for a change of more than … percent", "when … minutes have passed since the
  last save"). By default there is no such limit.
- The same text as before saves nothing.
- If two windows are open on the same section, neither silently overwrites the other: the later save becomes a
  revision of its own; both texts are kept.
- While saving, the editor is locked briefly so that no typed character is lost.

**Working with revisions** (panel **Revisions** in the right column):

- **‹ n/m ›** in the head of the section pages through the revisions (Alt+← and Alt+→, Mac: ⌥← and ⌥→). What is
  shown is also what is edited.
- **Use this revision** makes the revision shown the current one. No text changes; the formerly current one is
  then "replaced" and can be brought back with one click.
- **Change revision** changes the text and note of the revision shown itself (in the source text); afterwards it
  carries the note "changed" with the time. Empty text is refused.
- **Delete** removes the revision shown – only with the tick "Really delete revision n". The current revision
  cannot be deleted: switch to another one first. The number of a deleted revision is never given again. The
  deleted text stays readable in the
  [history of the work](/en/hilfe/austausch-sicherung/#spiegel).
- The **tree view** shows the revisions indented by their origin, with number, status, author (assistants in their
  colour), time and "changed". Unfolded, it shows the whole chapter. Per revision there are **show** and
  **try in the preview** – the preview then puts the revision in on trial, without switching anything.
- If a section has further revisions that are not current, the head shows **further revisions: n** to unfold,
  each with **make current**. In the outline the section then carries the badge "+n".

Only the human can change, delete and switch. Assistants only create proposals.

{{< hilfebild name="schreiben-aeltere-fassung" alt="The workspace shows an older revision: the head reads revision 1, replaced, the switch shows 1/2, next to it the reference to the current revision 2." titel="Look at an older revision – and use it again with one click." >}}

### History and undo {#verlauf}

{{< hilfebild name="schreiben-fassungsverlauf" alt="The history of a revision: the saving steps, newest first, each with author, time and the difference word by word – removed text red, new text green – and the button Back to here." titel="The history shows every saved state of a revision." >}}

Every continuation is kept as a step. A click on **state k** (in the head, in the panel "Revisions" or in the tree
view) opens the **history**: all steps, newest first, each with kind, author, time and the difference word by word
(removed text red, new text green).

**Back to here** restores the state before this step – as a new step; the history stays complete. This works only
on the human's current revision.

### Hiding older revisions {#ausblenden}

{{< hilfebild name="schreiben-fassungen-ausblenden" alt="Settings, page Work, block Revisions: the switches for continuing, below them the date field for hiding older revisions with the button Hide." titel="Hide older revisions in the whole work." >}}

A book quickly collects dozens of revisions per section. Older ones can be **hidden** – nothing is deleted.

- **hide up to here** is shown at every revision below which there is something to hide (in the panel
  "Revisions", in the tree view and in the head next to "‹ n/m ›"). Afterwards all replaced and rejected revisions
  with a lower number are behind the switch **"Show n older revisions ▸"**.
- **Always visible** are the current revision and open proposals.
- The counter then shows, for example, "2/3 (+ 24 hidden)"; ‹ › skips the hidden ones. A click on "(+ 24 hidden)"
  shows all again, "Hide 24 older ▾" hides them again.
- **Show all again** in the panel "Revisions" removes the mark of the section.
- **For the whole work:** **Settings → Work → Revisions → Hide older revisions up to** (date) hides everything
  before this day in every section; **Remove all marks (show all revisions again)** shows everything again.

Only the human can hide.

## Proposals of the assistants {#vorschlaege}

{{< hilfebild name="schreiben-vorschlag-lesefluss" alt="A proposal of an assistant in the reading flow: below the paragraph concerned a box with a coloured border with the name of the assistant, the note and the difference – deleted text red and struck through, new text green – and the buttons Accept and Reject." titel="A proposal stands in its place; the human decides." >}}

An assistant never changes the valid text. It creates a **proposal** – a revision of its own that waits. Open
proposals stand **in their place**: right at the paragraph they change, in the colour of their assistant.

How it works:

1. Read the proposal. The head names assistant, revision and model, below it the note. Removed text is red and
   struck through, new text green (in the source text and without JavaScript line by line with + and −).
2. **Accept** makes the proposal the current revision. **Reject** leaves the text as it is.

- The comparison is with the revision **shown** – that is what "Accept" replaces. If the proposal is based on
  another revision, "based on revision m" is shown in the head. On a continued revision it also names the state
  ("based on revision 3, state 2").
- If the revision has been continued since the proposal, it says "outdated: revision 3 is now at state 4". Whoever
  accepts it anyway replaces the later own changes (they are kept in the replaced revision).
- Even an accepted proposal pushes nothing aside for good: all revisions are kept; you can switch back at any time.
- A proposal with several places is called "part 1/2", "part 2/2". Several proposals at the same place stand one
  below the other. Proposals from programs without a role of their own stand at the end in grey.
- **Collapse:** a click on the head line folds the proposal into a coloured line and opens it again. At the top of
  the sheet it says "n open proposals · **Collapse all**".
- The tab **Start** shows the number of open proposals in the whole work.

How to ask for a proposal is described under [Assistants, chat and findings](/en/hilfe/assistenten/).

## Images of the work {#bilder}

{{< hilfebild name="schreiben-bilder" alt="The page Images of the sample work: at the top the block Cover, below it the list with thumbnail, name, type, size, paths, alternative text and revision of a sample image, at the bottom the form Add image." titel="All images of a work in one place." >}}

Every work has a page **Images** (tab **Book → Images**). It works without JavaScript.

How an image gets into the work:

1. Open **Book → Images**.
2. Under **Add image** choose a file (PNG, JPEG, GIF, WebP or SVG, at most 8 MiB).
3. If you like, enter a name, caption, **alternative text** (the description for people who cannot see the image)
   and a note.
4. Upload.

- **List:** thumbnail, name, type, size, paths, alternative text, the current revision ("2 of 3") and the column
  **Cover** with the button **As cover**.
- **Search** above the list finds by name, caption and alternative text.
- Clicking **an image** shows preview, type, size and all **revisions** of the image with **Make current**.
  **Replace** uploads a new revision that becomes current at once; the old ones stay. An existing name when adding
  does the same.
- **Another format:** a file in another format (for example PNG instead of SVG) replaces an image only with the
  tick **Adjust extension**. The image is then named with the new extension; the text does not need to change.
- **Describing:** caption, alternative text and note can be changed per revision.
- **Delete:** a revision that is not current or the whole image, each with a tick.
- **Cover** (block at the top): choose or remove the cover image and set the title area – see
  [Book typesetting](/en/hilfe/buchsatz/#umschlag).
- **Paths with two images:** in older works the same path can belong to two images. The page shows this at the
  top; you choose per path the image that keeps it and press **Assign**.

**Requests of the assistants** (block at the bottom): assistants may **add** images. Replacing, setting or
removing the cover and deleting they can only **request**. Per request it shows what is requested, who makes it,
the reason and, for replacing, a small preview; **Confirm** carries it out, **Reject** changes nothing. If the
image has changed in the meantime, the request counts as outdated. An assistant may add images or request
replacing at most 30 times per work in 24 hours.

### Inserting an image into the text {#bild-einfuegen}

{{< hilfebild name="schreiben-bild-einfuegen" alt="The open form Insert image in the status line under the sheet: choice of an image of the work, the choice of the place (at the cursor, at the end, at the beginning, before a paragraph), a field for the caption and the button Insert as new revision." titel="Insert an image of the work at the cursor." >}}

How it works:

1. Put the cursor at the desired place.
2. Press **Insert → Image** or unfold **Insert image …** at the bottom in the status line.
3. Choose an image of the work. Under **Place** you find "at the cursor" (default with JavaScript), "at the end",
   "at the beginning (below the heading)" or "before …" a particular paragraph.
4. If you like, enter a caption (empty: the image's own) and press **Insert as new revision**.

The image then stands as a block of its own in the text – never before or in the heading. With JavaScript the text
that has not been saved yet goes along; only the heading must not have changed (otherwise: save first). Without
JavaScript, save first.

- In the typeset view the image appears with caption and alternative text. It can be changed by a double click and
  deleted as a whole.
- **Missing images** are reported clearly by Baruch ("Image missing: …" with a link to the page Images) – in the
  preview, above the text and in the typeset view at the place of the image.
- **Chapter image:** if the work template sets an image for every chapter, the head of every section shows the
  image of its chapter small. If it cannot be determined reliably, it says "Image from the template" – Baruch does
  not guess.

## Time keeper {#zeitwaechter}

{{< hilfebild name="schreiben-zeitwaechter" alt="The workspace of the sample work: under the head of the section the line with the speaking time of the section, the total and the target; in the outline on the left each item shows its speaking time." titel="The time keeper estimates how long reading aloud takes." >}}

How long does the sermon take? Baruch estimates the **speaking time** per outline item and for the whole work from
the spoken words of the current revision and a speaking rate (default **110 words per minute**).

The times are always shown for a **sermon**, for a **book** only when a target time is set:

- **In the outline:** in the head "Total ≈ m:ss / target m:ss", after every item its duration. If the target time
  is exceeded, it says as text "Target time exceeded by m:ss" – not only as a colour.
- **In the head of the section:** "Section ≈ m:ss · total m:ss / target m:ss". With JavaScript Baruch counts again
  after a short pause while you type.
- **Setting:** **Settings → Work → Sermon time** – words per minute (60 to 250) and target time in minutes (1 to
  240; empty means: no target time).

Only what is spoken is counted: headings, footnotes, tasks, comments and formulas do not count. "John 3:16" is two
words, a word with a hyphen is one. Assistants can read the times; only the human can set them.

### Counting words {#statistik}

Baruch has no page of its own with statistics – such as characters, pages or writing progress per day. The
**number of words** is shown by the time keeper:

- On the page **Outline** (**Book → Outline**) the number of words of the whole work and the speaking rate follow
  the **Total time** at the top, for example "(1840 words, 110 words per minute)".
- When you move the mouse over the **Duration** of an item there, the hint names its words, for example
  "230 words". Only the item's own text is counted, without its sub-items.
- The spoken words of the **current** revisions are counted by the same rules as above: without headings,
  footnotes, tasks, comments and formulas. The number is therefore lower than what a word processor would count.

For a sermon the number is always shown. For a book times and words appear only when a **target time** is entered
under **Settings → Work → Sermon time**. Whoever only wants to see the words of a book enters a target time (at
most 240 minutes); this changes nothing in the book. If the target time is exceeded, the warning "target time
exceeded by …" appears as well – it can then be ignored.
