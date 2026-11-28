---
title: "Search, references, index and glossary"
description: "Searching and replacing in the section and in the whole work, setting and checking cross references, keeping the index and the glossary."
weight: 90
---

This page describes four aids for the finished manuscript: search and replace, references from one place in the
book to another, the index at the end of the book and the glossary with explained terms.

## Search {#suchen}

{{< hilfebild name="suchen-treffer" alt="The page Search and replace in the work with a search word: at the top the form with search text, replacement and the switches for case, whole word and regular expression, below it the hits as a tree by sections with line and context." titel="Search in the whole work: hits ordered by sections." >}}

**In the section** (with JavaScript):

1. **Ctrl+F (Mac: ⌘F)** opens the search bar at the top right of the editor. Selected text becomes the search
   text.
2. Enter or F3 jumps to the next hit, Shift+Enter or Shift+F3 (Mac: ⇧↩ or ⇧F3) to the previous one. "n of m"
   counts the hits.
3. Esc closes the bar.

The buttons **Aa**, **ab** and **.\*** switch case, whole word and regular expression (with the cursor in the bar
also Alt+C, Alt+W, Alt+R; Mac: ⌥C, ⌥W, ⌥R). The button **⌶** (Alt+L, Mac: ⌥L) searches **only in the
selection**; a selection over several lines switches it on by itself when the bar opens. The search covers the
text being edited, unsaved text as well. In the chat input field Ctrl+F (Mac: ⌘F) stays the search of the
browser.

**In the whole work:**

1. Press **Ctrl+Shift+F (Mac: ⌘⇧F)** or choose **Review → Search and replace in the work**. On the work page a bar
   with the hits as a tree by sections opens on the right.
2. A click on a hit jumps to the place without changing the page. **Alt+← (Mac: ⌥←)** leads back.
3. **Preview and replace …** leads to the page **Search and replace in the work**. Outside the work page and
   without JavaScript this page opens straight away.

On the page the search can be set more precisely:

- **Range:** section, chapter, part, "from the section to the end", a choice from the outline (list of check
  boxes), the whole work or all works (search only).
- **Revision:** the current one or – for a single section – the one shown at the moment.
- **Kinds of text:** text, headings, footnotes, quotations (Bible quotations as well), code of the template.
  Default when searching: everything except code.
- Every hit is a link to its place.

Limits: search text at most 500 characters; per search at most 2000 hits and 2 seconds – after that the page says
"cancelled". Then make the range smaller.

## Replace {#ersetzen}

{{< hilfebild name="suchen-ersetzen" alt="The preview of the replacement: for each section the changed lines, the old crossed out, the new highlighted, a check box in front of each place; at the bottom the check box Yes, replace and the button Replace all." titel="Before replacing, the preview shows every single place." >}}

**In the section:** **Ctrl+H** opens the line "Replace" below the search bar (so does the arrow ▸ on the left of
the bar; on the Mac ⌘H – if the system hides the window with it, Ctrl+H works). **Replace** replaces the current
hit (Ctrl+Shift+1, Mac: ⌘⇧1), **All** replaces all (Ctrl+Alt+Enter, Mac: ⌘⌥↩). This only changes the text in the
editor; saving is done as always with **Save**.

**In the whole work** (Ctrl+Shift+H, Mac: ⌘⇧H, or the page **Search and replace in the work**):

1. Enter the search text and the replacement, choose the range.
2. Look at the **preview**: for each section the changed lines are shown, the old crossed out, the new
   highlighted. There is a check box for each section and each place; what is unticked stays as it is.
3. Tick **Yes, replace** and press **Replace all**. One new revision is created per section.
4. Afterwards the result is shown – and the button **Undo**.

- **Undo** switches all affected sections back to their previous revision. Sections that have been changed again
  in the meantime stay and are named. The most recent replacement remains at the bottom of the page with "Undo"
  later on as well.
- **Quotations and code stay untouched**, unless quotations are chosen explicitly. Baruch never replaces code.
- After replacing, Baruch reminds you to check the Bible quotations of the changed sections.
- A changed title renames the outline item. A replacement that would create a heading or change its level is
  skipped for this section, with the reason.
- **Regular expression:** groups in the replacement as `$1`.
- Limits: replacement at most 2000 characters, at most 500 sections per replacement.
- **Only the human can replace.** Assistants can only search. For [lectors](/en/hilfe/konten/#lektoren) the
  button is therefore called "Search in the work".

## Cross references {#verweise}

{{< hilfebild name="verweise-seite" alt="The page Check cross references: at the top the box Broken references, here without a find, below it Possible cross references; no mention is found in the sample work. If there are some, they are listed there with the detected target, a check box and the button Convert into cross references." titel="Finding broken references and converting mentions into references." >}}

A **cross reference** points from one place to another outline item ("see the introduction, p. 12"). In the PDF
and in the e-book it can be clicked. It stays right even when the outline is rearranged.

**Inserting a reference:**

1. Put the cursor at the place and press **Insert → Reference**.
2. In the window, search for the target (title or number) and choose it.
3. Adjust the text of the reference (default: the title) and tick **with page number** if you like.
4. Insert.

In the typeset view the reference is a block with a blue dashed line; its text can be changed there. A click on it
shows "Open | Edit".

**Jumping:**

- **Ctrl+click (Mac: ⌘-click)** on a reference jumps to the target.
- **Alt+← (Mac: ⌥←)** returns to the starting place. Without a previous jump Alt+← pages back to the previous
  revision as usual.
- At the bottom in the middle a small bar shows "← Back" and, after going back, "Forward →".

**Checking references** (**Review → Check cross references**):

- **Broken references** point to an item that no longer exists. In the typeset book they appear only as text, and
  the print check names them.
- **Possible cross references** are mentions in the running text: "the introduction", "in the third chapter",
  "chapter III", "section 2.3", a title in quotation marks. They only count if the target is unambiguous.
- The list is only a proposal. Only **Yes, convert** and **Convert into cross references** insert the ticked
  references – as a new revision.

## Glossary {#glossar}

{{< hilfebild name="glossar-seite" alt="The page Glossary with two invented terms: for each term the short explanation, variants and the link Edit or delete; below it, separately, the proposals of the assistants with Accept and Reject and the form New term." titel="The glossary: terms, explanations and proposals of the assistants." >}}

The **glossary** explains terms of the book: term, short and detailed explanation, variants (inflections, other
forms), source and Bible references.

How it works:

1. Select a word in the text (or put the cursor into it).
2. Right click, the context menu key or Shift+F10 – or **Insert → Index and glossary**.
3. Choose **Add to glossary …**, enter the explanation, **Add**. The text of the section does not change.

- **Look up in the glossary** in the same menu shows a term even if the text only contains a variant.
- **Book → Glossary** shows all terms with **Edit or delete** and **New term** at the end.
- **Proposals of the assistants** are listed separately and marked; **Accept** or **Reject** decides. **Have terms
  proposed** asks the lector for terms that should be explained.
- A term exists only once (case does not count). Bible references are checked when saving; an unknown reference
  is refused.
- **In the book:** under **Settings → Work → Typesetting → Index and glossary** you choose whether a **glossary
  page** stands at the end of the book (before or after the indexes) and whether the **first mention** of a term
  per chapter is set in italics – with a glossary page, as a link to the entry.

## Index {#register}

{{< hilfebild name="register-seite" alt="The page Index: for each index – subjects, persons, Bible references – the entries with subentries and the sections they occur in; below them the index proposals of the assistants and Manage indexes." titel="The indexes of a work with their entries." >}}

An **index** stands at the end of the book and names the pages for every keyword. Built in are the **subject
index**, the **index of persons** and the **index of Bible references**; up to 13 own indexes can be added.

How it works:

1. Select a word in the text.
2. Right click (or **Insert → Index and glossary**) and choose **Add to index …**.
3. Enter the keyword (filled in already), a subentry if you like, the index, "see" and "see also".
4. The entry appears as a small sign ⁘ behind the word. **Save** creates the new revision.

- A right click on the sign offers **Edit index entry …** and **Remove index entry**.
- **Insert → Index** unfolds the list of all entries of the section; a click jumps there.
- The **index of Bible references** fills itself from all Bible references detected in the running text.
- **Book → Index** shows the entries of each index, the open **index proposals** of the assistants (Accept sets
  the entry as a new revision) and **Manage indexes** (add own ones, rename, delete).
- **In the book:** under **Settings → Work → Typesetting → Index and glossary** you tick which indexes appear at
  the end of the book (in this order, in one to three columns). The pages are sorted the German way (umlauts
  with their base letter, the sharp s like ss), following pages appear as "12–14".
- Entries never stand in headings. The same entry never stands twice behind the same word.

{{< hilfebild name="glossar-markierung" alt="The page At the selection for a section: a field for the word, below it the buttons Add to glossary, Add to index and Look up in the glossary as well as the index entries of the section." titel="Without JavaScript: glossary and index through the page At the selection." >}}

**Without JavaScript**, **Marking …** in the status line below the sheet leads to the page **At the selection**:
enter the word, then "Add to glossary", "Add to index" or "Look up in the glossary". Below that the index
entries of the section are listed with "Remove index entry".
