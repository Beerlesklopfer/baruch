---
title: "Ideas and mind map"
description: "Capturing ideas quickly, finding and processing them again – and ordering, linking and exchanging them in the mind map."
weight: 70
---

An **idea** captures what comes to mind while writing, reading or praying, without interrupting the text. The
**mind map** shows the ideas of a work as a tree around the work. Ideas never change the book text.

## Capturing an idea {#festhalten}

{{< hilfebild name="einfaelle-festhalten" alt="The small window Capture idea above the work page: fields for keywords, thought, marked text and source – still empty here –, below them the reference to the open section and the button Capture (Enter)." titel="An idea is captured in a few seconds." >}}

How it works:

1. Press **Alt+E (Mac: ⌥E)** or choose **Start → Capture idea**. A small window opens above everything.
2. Enter one or more **keywords** (required), separated by comma or semicolon. Add a **thought** if you like.
3. **Enter** saves, **Esc** closes. In the field "Thought" Shift+Enter (Mac: ⇧↩) makes a new line.

- If text was selected before – on the sheet or in the literature –, it appears in the field **Marked**; from the
  literature the source comes along.
- The idea refers to the section open at the moment.
- After closing, cursor and selection are back where they were.
- Without JavaScript the button leads to the page of the ideas with the form **New idea**.

An idea can also carry a **source** and a **Bible reference**. It belongs to a work (with or without reference to
a section) or stands **without a work**.

Limits: 1 to 20 keywords, each at most 60 characters; thought and marked text at most 4000, source and Bible
reference at most 200 characters.

## Finding ideas again {#liste}

{{< hilfebild name="einfaelle-liste" alt="The list of ideas of the sample work: at the top the filters for work, status and keyword and the search field, next to them the keyword cloud, below them the ideas with keywords, thought, section, origin and status." titel="All ideas with filter, search and keyword cloud." >}}

**Start → Ideas** opens the list: the ideas, newest first, with keywords, thought, marked text, work, section,
origin (human, assistant, program) and status (open or processed).

- **Keyword cloud:** every keyword is a link that narrows the list down to this word; frequent words are bigger.
- **Filter:** work (all, without a work, a particular one), status, keyword.
- **Search** across keywords, thought, marked text, source and Bible reference, regardless of accents
  ("gartnerin" finds the same word written with an umlaut). "Search by" chooses **Wording** or **Meaning**; the
  search by meaning only works within one work (see [Wording or meaning](/en/hilfe/bibel-quellen/#bedeutungssuche)).
- **Show deleted ones** lists deleted ideas for restoring.

## Editing and processing an idea {#einfall}

{{< hilfebild name="einfaelle-einfall" alt="The page of an idea: keywords, thought, marked text, source and reference; below them the forms Edit, Process – as an outline item, post, note or task – and Delete." titel="An idea with everything you can do with it." >}}

A click on an idea shows all its details, and in addition:

- **Edit:** keywords, texts, work, reference, open or processed. If the work changes, the old reference is
  dropped.
- **Delete** (with the check box "Yes, delete this idea"). Deleted ideas can be **restored**.
- **Process** – with one click the idea becomes:
  - a **new outline item** after, before or below the chosen section; the title is suggested from the keywords
    and can be changed;
  - a **post in the chat** of the section;
  - a **note on the section**;
  - a **task for an assistant** (default: the lector): the idea goes to him as a question about the section; his
    answer appears in the chat.

Afterwards the idea is "processed" (not deleted). If something goes wrong, it stays open. Ideas without a work can
only be processed once they are assigned to a work. If the idea comes from an assistant, the post says so in its
source.

**Assistants and ideas:** because ideas change no book text, assistants may create, change and delete ideas as
well. The origin is shown at the idea. The lector knows the ideas about the section as a memory. If a work is
deleted, its ideas are kept as ideas without a work.

## The mind map {#mindmap}

{{< hilfebild name="mindmap-karte" alt="The mind map of the sample work: the work in the middle, to the right and left the chapters as coloured main branches with ideas as sub-branches, two ideas are linked by a labelled curved line; below the tree the cloud with ideas without a place." titel="The mind map: the work in the middle, branches to the right and left, the cloud below." >}}

**Start → Ideas → Mind map** (or the button **Mind map** on the list of ideas) shows the work in the middle and,
to the right and left, the main branches: the parts or chapters in the order of the outline, followed by the
**free main topics** (ideas hanging directly on the work). Every main branch has a colour that its sub-branches
inherit. The nodes are the ideas of the work – there is no second store of data.

An idea hangs on its chapter, as a sub-node on another idea (as deep as you like) or as a main topic on the work.
**Parts and chapters belong to the outline:** they cannot be renamed, moved or deleted in the mind map.

| Sign at the node | Meaning |
|---|---|
| "−" / "+ n" at the outer edge | fold or unfold the branch; n is the number of hidden nodes |
| leaf symbol | the idea has a note, quotation, source or Bible reference |
| "§" | the idea refers to a section; the link leads there |
| "k open" | layer "Findings": open findings at the part or chapter (with a thick border from three on) |
| crossed-out title | the idea is processed |
| pale node | does not match the search |

**Relations** between ideas are curved lines with a label: "leads on to" solid, "belongs to" dashed, "example of"
dotted, "contradicts" red and bold. Contradictions are also listed under "Contradictions" below the picture.
**Nearness** (can be switched on in the ribbon) shows finely dotted lines between similar ideas; it is display
only.

{{< hilfebild name="mindmap-eingeklappt" alt="The mind map with a folded branch: at the outer edge of the chapter there is a plus sign with the number of hidden nodes." titel="A folded branch shows how many nodes it hides." >}}

### The cloud {#wolke}

Below the tree lies the **cloud**: the tray of all ideas that have no place yet.

- **From the cloud into the mind map:** drag the card onto a node. On an idea it becomes its sub-node, on a
  chapter or part it hangs there, on the work it becomes a free main topic.
- **From the mind map into the cloud:** drag the idea into the cloud or choose **Into the cloud** in the ribbon.
  It moves with its sub-nodes (the card shows "+ n").
- **New ideas without a place** land in the cloud, for example with Alt+E on the mind map.
- **Proposal "fits branch …":** if a card is similar to an idea in the tree, a proposal appears below it in the
  colour of that branch. It is only a hint; the human has to hang it in.

### Mouse and keys in the mind map {#mindmap-tasten}

| Input | Effect |
|---|---|
| Click | selects the node |
| Double click, F2 | renames the selected idea (Enter accepts, Esc discards) |
| Tab | new sub-node of the selected node |
| Enter | new sibling after the selected idea |
| Delete, Backspace | deletes the selected idea including its sub-nodes after a confirmation (restorable) |
| ← → | to the parent or the first child node |
| ↑ ↓ | to the previous or next sibling; from the work with ↓ into the cloud |
| Space | folds or unfolds the selected branch |
| Ctrl+mouse wheel (Mac: ⌘ or Ctrl with the mouse wheel), `+`, `-`, `0` | zoom in, zoom out, fit |
| Dragging the area | moves the view |
| Dragging an idea | move it below another node, put it in order or lay it into the cloud |
| Ctrl+Z (Mac: ⌘Z) | takes back the last action (one step) |
| Alt+L (Mac: ⌥L), "Link" | relation from the selected idea to the next one selected |
| Alt+E (Mac: ⌥E) | new idea in the cloud |
| Esc | cancels "Link" or clears the selection |

The browser remembers folded branches per work. **Not checked:** using the mind map with a real mouse and keyboard
(zoom and arrow keys as well) has not been checked thoroughly so far; the Mac keys are derived from the program but
have not been tried on a Mac.

### Mind map as a list and without JavaScript {#mindmap-gliederung}

{{< hilfebild name="mindmap-gliederung" alt="The outline view of the mind map: on the left the branches as a nested list, on the right the cloud; at the top the forms for the selected node – Rename, Create sub-node, Move, Into the cloud, Link, Delete." titel="The same mind map as a list – fully usable without JavaScript." >}}

The **outline view** (in the ribbon of the mind map) shows the same mind map as a nested list. There everything
works through forms: **Edit** selects a node; then **Rename**, **Create sub-node**, **Create sibling**, **Move
below …**, **As a main topic onto the work**, **Into the cloud**, **Order**, **Set reference**, **Link** and
**Delete** appear. On the right is the cloud with the form "New idea" and the proposals "fits branch …" together
with **Hang in there**.

{{< hilfebild name="mindmap-ohne-javascript" alt="The mind map without JavaScript: the picture of the tree with clickable plus and minus signs; the buttons of the group Node in the ribbon lead into the outline view." titel="Without JavaScript the mind map is a picture with links." >}}

Without JavaScript the mind map is a picture with links: "−" and "+ n" fold branches, "§" leads to the section,
the title of an idea to its page. For changes the buttons lead into the outline view.

### Writing out and reading in the mind map {#mindmap-austausch}

{{< hilfebild name="mindmap-einlesen" alt="The page Read in a mind map: file choice for an XMind or Freeplane file, the choice of the node to hang it below and, below that, the preview of the tree read with the number of nodes and the notices." titel="Reading in a mind map from XMind or Freeplane – preview first, then create." >}}

**Writing out** (ribbon of the mind map, group "Output"): as a file for **XMind**, for **FreeMind/Freeplane**, as
a picture (**SVG**), as a **PDF** (A4 landscape) or as a **Markdown** list. Titles, notes, keywords, relations and
order are carried over; the cloud appears as its own last branch "Wolke" (the file labels are German for now).
Status, origin, reference, quotation, source and Bible reference appear only in the Markdown output or not at all
in the file.

**Reading in:**

1. In the ribbon of the mind map choose **Read in** and choose a file (`.xmind` or `.mm`, at most 10 MiB). If you
   like, choose an idea to hang the file below; without one the top nodes become free main topics on the work.
2. The **preview** shows the tree read, the number of nodes and notices. Nothing has been created yet.
3. **Create** creates the nodes as ideas.

Titles, notes, keywords, order and relations are adopted. Reading in never creates outline items. A file written
out by Baruch itself comes back with its cloud. Limits: at most 2000 nodes and 32 levels; titles are shortened to
200, notes to 4000 characters (with a notice). Files in the old format of XMind 8 are refused: please save them
again in a current XMind.

**Not checked:** the files have so far been opened neither in a real XMind nor in Freeplane, and no file saved
there has been read in. Only the way from Baruch to Baruch is checked.
