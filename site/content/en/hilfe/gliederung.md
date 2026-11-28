---
title: "Outline"
description: "Create, arrange and number parts, chapters and sections – and decide on requests of the assistants."
weight: 40
---

The outline is the sequence of the headings of a work: **Part**, **Chapter** (in sermons **Sermon part**),
**Section** and **Subsection**. The outline and the headings in the text are always the same: whoever renames in
the outline changes the heading in the text – and the other way round.

## The outline on the left of the workspace {#gliederung}

{{< hilfebild name="gliederung-leiste" alt="The outline on the left of the workspace: parts, chapters and sections of the sample work as an indented list with numbers; a section is chosen, the head shows the numbering." titel="The outline on the left: this is where items are created, renamed, moved and deleted." >}}

With JavaScript everything happens directly in the list on the left:

- **Open:** a click on an entry opens the section.
- **Rename:** double click on the chosen entry or **F2**. Enter confirms, Esc cancels.
- **Context menu:** right mouse button, Shift+F10, the context menu key or the button **⋯** at the entry. In it:
  **Rename**, **New item before**, **after it** or **below it** (type the title, Enter creates it), at parts and
  chapters **New part before** or **after**, **Promote**, **Demote** and **Delete…**.
- **Delete:** context menu or **Del** (on the Mac the key ⌦ or Fn+Backspace). Baruch asks ("… delete with all
  revisions and posts?"). An item with subitems and the last chapter cannot be deleted. The deleted text stays
  readable in the history of the work – together with all other revisions and posts.
- **Moving and changing the level with the keyboard:** Alt+Shift+↑ and ↓ move, Alt+Shift+← and → promote or
  demote (Mac: ⌥⇧ with the arrow keys). Subitems move along. This works everywhere, except while you are typing in
  an input field.
- **Drag and drop:** dropping on the upper third of an entry means *before*, on the middle *below it*, on the lower
  third *after it*. While dragging, target and level are shown below the outline; invalid targets are marked red
  and refused.

F2, Del and the context menu key work only when the outline has the focus (click an entry first).

**Moving keeps the level:** a chapter stays a chapter. Before and after only work next to an item of the same
level, below only under an item exactly one level higher. Otherwise Baruch reports "Moving keeps the level …;
change the level first".

**Rules for titles:** a title must not be empty, must not contain a line break and may be at most 200 characters
long. This applies wherever a title is created or changed – also for proposals of the assistants. An old item
without a title or with an overlong title stays as it is until you rename it.

**Unsaved text:** if there is unsaved text in the editor, Baruch asks before every change of the outline "Discard
the unsaved change?". **Cancel** changes nothing.

### Headings in the text {#ueberschriften}

Saving reads the heading of the section:

- A new title or another level adjusts the outline (subitems move along).
- An **additional heading** splits the section, a **removed heading** attaches the text to the previous item
  (posts move along) – both only after a question (**Split and save** or **Merge and save**).
- Text before the first heading is refused.

Headings can also be typed as in Markdown: `# Title` at the start of a line becomes a chapter, `## Title` a
section, `### Title` a subsection. More hash signs also become a subsection, because the outline knows only three
levels. In the source text the editor converts the line on Enter, in the typeset view the paragraph becomes a
heading on Enter; without JavaScript it happens when saving.

**Revisions bring their heading along:** if you switch to another revision or accept a proposal, Baruch never
changes its text. Instead the outline follows the heading of this revision: another title renames the item;
another level changes its level, and all subitems move along. Switching back restores the old outline. If that is
not possible (for example because a subitem would end up deeper than the third level), Baruch refuses with a
message, and nothing changes.

**Several windows:** if another window has rearranged the outline in the meantime, saving, switching or deleting
still lands in the section shown. If it has been deleted, Baruch reports "This outline item no longer exists …".

### Parts {#teile}

A **part** is created before or after a chapter or part (context menu **New part before/after**), also the first
part before the first chapter. All following chapters up to the next part then stand under it; their texts and
numbers stay. Chapters keep counting across parts, parts count on their own (I, II, …).

- A chapter cannot be changed into a part and a part cannot be changed into a chapter.
- The text of a part carries exactly one part heading and no other; chapters are created in the outline.
- In the book every part gets a page of its own.

### Chapters without a number {#ohne-nummer}

A preface, an introduction or a closing prayer often should not be counted. On the page **Outline** (see below)
every chapter, section and subsection has the button **without number** or **with number**. An item without a
number is not counted; the others move up. Parts always count.

## The page Outline {#seite}

{{< hilfebild name="gliederung-seite" alt="The page Outline: all items of the sample work as a table with number, level and title, per row the buttons for moving, changing the level, for without number and for editing; above them the numbering." titel="The page Outline shows everything at once – and works without JavaScript." >}}

**Book → Outline** shows all items with number and level on one page. It is the way without JavaScript – and
convenient when you rearrange a lot.

- Per entry: **↑** (before the previous item), **↓** (after the next item of the same level), **←** and **→**
  (promote or demote), **without number / with number** and **Edit**.
- For sermons and for works with a target time there is the column **Duration**: per item the speaking time and
  the time mark "from m:ss"; above it total time, target time and – if exceeded – the warning as text. See
  [Time keeper](/en/hilfe/schreiben/#zeitwaechter).

{{< hilfebild name="gliederung-bearbeiten" alt="The page for editing an outline item with forms for renaming, for choosing the level, for moving, for creating a new item before, after or below it and for deleting with a checkbox." titel="Edit an item: rename, move, change the level, create, delete." >}}

**Edit** at an entry opens a page with forms: rename, choose the level, move, create a new item before, after or
below it (at parts and chapters with "Level: Part" also a new part) and delete (with a checkbox for confirmation).

Without JavaScript the same buttons ↑ ↓ ← → are also at every entry of the outline on the left, plus the links
**edit** above the outline and **Edit** at the entry.

## Requests of the assistants {#antraege}

{{< hilfebild name="gliederung-antraege" alt="The block Requests of the assistants on the page Outline: an open request with its number, the wish to rename a section, the requester, the reason and the buttons Confirm and Reject." titel="An assistant requests – the human decides." >}}

Assistants may **create** new items. Everything else – renaming, moving, changing the level, counting with or
without a number, deleting – they can only **request**. Until the decision nothing changes.

How it works:

1. The head of the outline reports open requests with **"N requests on the outline"**; the tab **Book** also shows their
   number. The link leads to the block **Requests of the assistants** on the page Outline.
2. Per request it shows the number, what it wants (for example "rename 'Summer' to 'High summer'"), who makes it
   and its reason.
3. Unfold the **Preview**: it shows the outline as it would be after confirming.
4. **Confirm** carries out the request, **Reject** closes it; the outline then stays unchanged.

{{< hilfebild name="gliederung-antrag-vorschau" alt="The preview of a request: the outline after confirming, with kind and number per row; the changed item carries the mark renamed and the note before and after." titel="The preview shows what the request would change – it changes nothing itself." >}}

- In the **preview** changed items are marked ("renamed", "moved", "new level", "with/without number"); a deleted
  one stands struck through in its old place ("dropped"). Every row names kind and number ("Part I", "Chapter 2",
  "Chapter (without number)"), so that a part and a chapter with the same number cannot be mixed up. If the kind or
  number of an item changes, "(before … → after …)" is added.
- If the item has changed since the request (deleted, another title, another level), nothing is carried out when
  confirming; the request counts as **outdated**, with a reason. The preview then says "No preview: …".
- Every decision is recorded in the history of the work – also a rejected or outdated request, with requester,
  reason and decider.
- **Decided recently** unfolds the last ten decisions.
- If there are very many open requests, the page calculates the preview at once only for the first 20; the others
  show the link **Show preview**.
- What would not work already at the time of the request (invalid title, deleting with subitems, no effect) Baruch
  does not even accept; an identical open request is not made twice.

### Consent to a new part {#zustimmung-teil}

If an assistant creates a **part** in such a way that existing chapters would move under it (for example "Part II"
before chapter 5), Baruch does not create it but makes a request that the human must **actively consent** to.
Without a click nothing happens – not even after a waiting time.

- **With JavaScript** the window **Consent to the outline** opens on every page: who wants the part, in which work,
  the reason and the outline afterwards. **Consent** creates the part (the chapters keep text and number),
  **Reject** leaves everything as it is. **Decide later** (or Esc) only closes the window; it appears again on the
  next page.
- **Always** (also without JavaScript) a yellow **notice bar** "Consent needed:" stands below the ribbon with the
  link **View and decide**.
- If the outline has changed in the meantime so that the part would take over other chapters, nothing is carried
  out; the request counts as outdated.
- A part that takes over no existing chapters (for example at the end of the work) is created at once, like every
  new item of an assistant.

The same applies if an [exchange file](/en/hilfe/austausch-sicherung/#einlesen) brings such a part along.

## Outline scheme and numbers {#schema}

{{< hilfebild name="gliederung-schema" alt="The page Outline scheme: the choice of schemes, each with an example, and on the right the preview on the first twelve items of your own outline; below them the buttons Preview and Apply." titel="Choose a scheme and look at it on your own outline." >}}

How the items are counted is determined by the **outline scheme**.

How it works:

1. Choose **Book → Outline scheme** (or **Choose scheme…** in the head of the outline).
2. Click a scheme. **Preview** shows it on the first twelve items of your own outline, without changing anything.
3. **Apply** sets it; all numbers are calculated anew.

The words in the numbers (Part, Chapter, Introduction, Conclusion, para., no.) follow the language of the work; a
German work keeps the German words. The names of the schemes are shown in German in this version; the table names
them in English.

| Scheme | Numbers |
|---|---|
| From the work template (default for new and imported works) | as the template sets it, otherwise 1 · 1.1 |
| Decimal (DIN 1421) | 1 · 1.1 · 1.1.1 |
| Decimal with final dot | 1. · 1.1. · 1.1.1. |
| Alphanumeric | I. · A. · 1. · a) · aa) |
| Chapters only, Roman | I · II · III, sections without number |
| Chapters only, Arabic | 1 · 2 · 3, sections without number |
| Parts and chapters | Part I · Chapter 1 · 1.1 · 1.1.1 |
| Chapters in words | parts and chapters as ordinal words (up to 99 in words, then with digits), sections without |
| Legal | § 1 · para. 1 · no. 1, parts without |
| Sermon | first chapter "Introduction", last "Conclusion", in between I. · II. · III.; sections and parts without |
| Book | chapters as in "Sermon", sections without; parts as ordinal words (first part, second part …) |
| No numbers | – |
| Own pattern | see below |

- Parts count in Roman numerals (I, II), with "Parts and chapters" as "Part I", with "Chapters in words" and "Book"
  as ordinal words, with "No numbers", "Legal" and "Sermon" not at all.
- With "Sermon" and "Book" only chapters with a number count; if there is only one, it is called "Introduction".
- **Sermon with parts:** if a sermon is to get parts, switch to "Book" – the chapters keep their numbers, the parts
  are called first part, second part. Switching back works at any time. Whether a work is a book or a sermon does
  not depend on the scheme.
- The scheme applies everywhere alike: in the outline, in the head of the editor, in the typesetting, in the PDF
  and in the export.
- The **head of the outline** shows "Numbers: …" with the name of the scheme in force. Unfolded, it can also be set
  there to "from the work template", "none" or a pattern of your own.

### Own pattern {#muster}

In the field "own pattern" you describe the counting yourself:

| Pattern | Meaning |
|---|---|
| `1.1`, `I.1`, `1.a)` | one pattern for all levels |
| `I\|1` | only the levels up to the given number get numbers (here: only chapters, Roman); `1.1\|2`: subsections without number |
| `I.;A.;1.;a);aa)` | a pattern of its own per level, separated by `;` – an empty one means "without number" |

Counting characters are `1`, `a`, `A`, `i`, `I` and `*`; `aa` or `AA` doubles the letter (aa, bb, cc …). Every
non-empty pattern needs a counting character. Baruch refuses an invalid pattern with a reason. A pattern of your
own stays remembered when you switch to another scheme.
