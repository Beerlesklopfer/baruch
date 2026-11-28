---
title: "First steps"
description: "Start Baruch, get to know the screen, create, change and delete works – and quit Baruch again."
weight: 10
---

Baruch runs as a small program on your own computer and is used in the browser. Everything works even when
JavaScript is switched off in the browser; with JavaScript, conveniences are added (dragging columns, keyboard
shortcuts, pages that refresh without reloading).

## Starting Baruch {#starten}

{{< hilfebild name="installation-erster-start" alt="The workspace of Baruch right after the first start: at the top the ribbon, on the left the outline of the bundled sample work, in the middle the first section, on the right the column of the assistants." titel="This is what Baruch looks like after the first start." >}}

How it works:

1. Choose **Baruch** in the start menu. The launcher starts the program if it is not running yet and opens a
   window.
2. If no window opens, type the address `http://127.0.0.1:8765/` into the browser.
3. On the very first start Baruch brings a small **sample work**. You can safely try everything in it.

If you prefer the terminal:

```sh
baruch                              # starts the program if needed and opens the browser
baruch-server                       # only the program, reachable at http://127.0.0.1:8765/
baruch-server --data ~/baruch-probe # for trying things out with a data folder of your own
baruch-server --version             # prints the version
```

Good to know:

- The sample work only goes into a really new data folder, and only once. Whoever later deletes all works keeps an
  empty list – even after a restart.
- The parts of the sample work have no text of their own; they only structure.
- For trying things out, take a data folder of your own (third line above). Access data for other services that
  you enter in such a trial run is remembered by Baruch only until it quits; the access data of your actual Baruch
  stays untouched.
- Where Baruch stores the data and how to back it up is described under [Operation](/en/betrieb/#datenordner).
  The installation and installing new versions are described under [Installation](/en/installation/#aktualisieren).
- Baruch ships no Bibles and no assistants; both lists are empty at first. That is not an error. Writing,
  revisions and book typesetting work right away. Setting them up is described under
  [Set up Bibles](/en/installation/#bibeln-einrichten) and [Set up assistants](/en/mcp/#assistenten-einrichten).

## The screen {#bildschirm}

{{< hilfebild name="start-arbeitsplatz" alt="The workspace with the sample work: at the top the ribbon with the tabs Start, Insert, Review, Book and View, on the left the outline, in the middle the sheet with the text, on the right the column with assistants, revisions and chat." titel="The workspace: ribbon, outline, sheet and right column." >}}

The workspace has four areas:

- **At the top the ribbon.** The first line holds the menu **Baruch ▾**, the choice of work, title and author of
  the work and the tabs **Start**, **Insert**, **Review**, **Book** and **View**; below them the buttons of the
  chosen tab. More under [Ribbon and laptop mode](/en/hilfe/menueband/).
- **On the left the outline** – the only one in the interface – with parts, chapters and sections. A click on an
  entry opens the section, also from the preview. Chapters can be folded and unfolded; small marks show open
  proposals. Rearranging, renaming, creating and deleting happen directly in this list, see
  [Outline](/en/hilfe/gliederung/). The literature is not here but in the tab **Review**.
- **In the middle the chosen section:**
  - The **head** stays fixed at the top: the title, below it "Revision n · status · author" in the colour of the
    assistant the revision comes from, and "‹ n/m ›" for paging through the revisions (Alt+← and Alt+→, Mac: ⌥←
    and ⌥→). If the revision shown is the current one, "n/m" is highlighted in green; otherwise the reference
    **current: revision k** stands next to it.
  - The **sheet** shows the text – as the typeset view (as it will be typeset later) or as source text, in the
    preview the preview. It fills the remaining height and scrolls on its own; head and status line stay in
    place. The open proposals of the assistants are on the sheet as well, each in the colour of its assistant.
  - The **status line** at the bottom shows notes (the full text appears when you move the mouse over it) and
    the buttons **Save** and **Save as new revision**.
- **On the right a column with three panels that open**, of which one is always open (Baruch remembers which):
  **Assistants**, **Revisions** and the **Chat** about the section. Even folded, the panel "Revisions" shows how
  many revisions the section has and how many proposals are open.

The borders between the columns can be dragged with the mouse. With the keyboard: reach the border with Tab and
move it with ← or →.

How to write and save is described under [Writing, images and revisions](/en/hilfe/schreiben/).

## The list of works {#werke}

{{< hilfebild name="start-werkliste" alt="The list of all works as a table with title, author, kind, number of sections, open proposals and last change; per work the buttons Open, Edit, Export and Delete." titel="All works at a glance." >}}

A **work** is a book or a sermon. Only the human manages works; an assistant can neither create nor delete works.

- **Switch work:** on the workspace, the first line of the ribbon holds a selection with all works ("title ·
  author · n sections"). With JavaScript the selection opens the work at once, without JavaScript with the button
  **Open** next to it. Outline, editor, assistants and literature search always refer to the work shown.
- **See all works:** **Baruch ▾ → All works**. The list shows per work title, subtitle, author, kind, sections,
  open proposals and last change, plus **Open**, **Edit**, **Export** and **Delete**.
- Baruch remembers the work shown last for one year in this browser and opens it again on the next visit. If this
  work no longer exists, the oldest one opens; if there is none at all, the list of works appears.
- **Export** downloads the work as one text file in the typesetting format Typst (the template of the work and all
  current texts). More under [Book typesetting](/en/hilfe/buchsatz/#typst-export).

## Creating a new work {#neues-werk}

{{< hilfebild name="start-neues-werk" alt="The form New work / Import: at the top importing an existing file or folder, below it the fields for a new work – title, subtitle, author, kind, framework, audience and Bible translation." titel="Create a new work or import an existing text." >}}

How it works:

1. Choose **Baruch ▾ → New work / Import…**. On the workspace a window opens above the text, otherwise (and
   without JavaScript) a page of its own.
2. Under **Create a new work** enter title, subtitle and author.
3. Choose the **Kind**: Book or Sermon. In addition the framework, audience and Bible translation.
4. Create. The new work opens.

A book begins with one chapter (its title is created in German in this version: `Erstes Kapitel`, "first
chapter") and a plain template that numbers chapters in the form "1.1". Whoever already has a text imports it instead: [Importing existing texts](/en/hilfe/einlesen/).

**Titles are unique.** Upper and lower case and spaces do not count ("The Book" is the same as "the  book"). If
the title is already taken when creating, importing or renaming, Baruch reports "There already is a work …" and
creates nothing.

### Creating a sermon {#predigt}

The kind **Sermon** (framework "matching the kind" or "sermon") creates a work with a sermon outline:

| Number | Title | Meaning |
|---|---|---|
| (none) | Sermon text | the Bible passage the sermon expounds |
| Introduction | Lead-in | way into the text |
| I. | Observation on the text | first main point |
| II. | Interpretation | second main point |
| III. | Encouragement | third main point |
| (none) | Application | what follows from it |
| Conclusion | Outlook | ending |

The titles do not repeat the number (otherwise "Introduction Introduction" would appear). Renaming, moving,
adding further main points or "number on/off" work as in any outline. Sermon text and application are not
counted; that is why the last counted item is always called "Conclusion". The table shows the items in English;
in this version Baruch creates the titles in German (`Predigttext`, `Hinführung`, `Beobachtung am Text`,
`Deutung`, `Zuspruch`, `Anwendung`, `Ausblick`), and you rename them as you like.

**Sermon text with wording:** in the field **Sermon text** enter exactly one Bible reference, for example
"John 3:16–18", and under **SWORD Bible for the wording** choose one of the Bibles set up (SWORD is the free format
in which Baruch reads Bibles; in the window on the workspace the field is a text field for the name of the Bible).
If the Bible stays empty, the one of the work currently shown applies. Baruch puts the wording as a quotation with
its source into the item "Sermon text"; the chosen Bible becomes the Bible of the new sermon at the same time.

- If verses are missing in this Bible, the rest is there, and Baruch says how many are missing.
- **Without a Bible set up**, honestly only the reference is there – Baruch writes no wording from memory.
- A reference that does not exist ("Ps 151:1") creates nothing: the form appears again, with a message and all
  entries.
- If the field stays empty, the sermon text is a mere heading to be filled in later.

## Changing a work {#werk-bearbeiten}

{{< hilfebild name="start-werk-bearbeiten" alt="The page Edit work with the fields title, subtitle, author, kind, audience and Bible translation, below them the large field for the template of the work and the list of its revisions." titel="Change the details and the template of a work." >}}

How it works:

1. Choose the tab **Book → Edit work** (the same fields are under **Settings → Work**).
2. Change the title, subtitle, author, kind, audience or Bible translation.
3. Save.

The same page holds the **work template**: everything that stands before the first heading and determines the look
of the whole book (page, fonts, formats of your own). A heading does not belong in it. Every change of the work
template becomes a new **revision** with a note; the list below shows all revisions, older ones can be read. The
folder of the work in the data folder keeps its name when the work is renamed.

## Deleting a work and getting it back {#loeschen-archiv}

{{< hilfebild name="start-archiv" alt="The archive of the deleted works with a deleted trial work, its archive folder, the time of deletion and the button to restore it." titel="Deleted works lie in the archive and can be brought back." >}}

How it works:

1. In the list of works choose **Delete**.
2. To be safe, type the title of the work and confirm.

What is deleted: the outline, all revisions, proposals, posts and the revisions of the work template. The ideas of
the work are kept as ideas without a work. The page points to the export and the archive beforehand. If the deleted
work was the oldest, Baruch opens the next one from then on.

**Getting it back:**

1. In the list of works choose **Archive**. It lists the deleted works with title, archive folder and time of
   deletion.
2. Tick the checkbox for confirmation and press **Restore**.

What comes back: the title, the outline, the texts that were current last (each as revision 1 with the note
"restored from the archive"), the work template and the typesetting settings. What does **not** come back: older
revisions, posts and findings (they stay readable in the history of the work folder), the images (they can be
added again on the page **Images**) and the kind of the work (afterwards it is a "Book"). The ideas can be assigned
to the work again on the page **Ideas**. If another work now carries the same title, rename that one first.

A deleted work does not free its place: whoever creates a work with the same title again or imports the same file
once more gets a new work with its own number and its own folder. That way the archive stays unambiguous; the
delete page says so beforehand.

## Address and bookmarks {#adresse}

{{< hilfebild name="gliederung-leiste" alt="Part of the workspace: on the left the outline of the sample work with a chosen section." titel="Every section has its own permanent address." >}}

Every section has an address of its own; it is in the address bar of the browser and can be kept as a bookmark or
passed on. The address stays valid when the outline is rearranged – a remembered reference or one named by an
assistant always opens the same section. If the section no longer exists (deleted or merged with another), Baruch
shows another section and at the top the note "A section with key … does not exist (any more)."

The address also remembers which revision is shown. A deleted or unknown revision shows the current one. An
unknown work gives the message "unknown work".

## Times {#zeitangaben}

{{< hilfebild name="schreiben-zeitwaechter" alt="The workspace of the sample work: under the head of the section the line with the speaking time of the section, the total and the target; in the chat on the right every post carries date and time." titel="All times are in the local time of the computer." >}}

All points in time on the pages – for revisions, ideas, images, works, connection checks, comparisons, accesses for
programs and the retrieval date of the web search – are given in the **local time** of the computer Baruch runs
on, with daylight saving time.

## About Baruch {#ueber}

{{< hilfebild name="start-ueber" alt="The page About Baruch with name and version, licence, source code, website and help, data folder, the reference to new versions and the third-party libraries." titel="About Baruch shows what is running and where it comes from." >}}

**Baruch ▾ → About Baruch** shows what is running and where it comes from. The page needs neither JavaScript nor a
work.

| Item | Content |
|---|---|
| Name, version | "Baruch" and the version of the program (the same is shown under Settings → Data) |
| Licence | the free licence AGPL (version 3 or later); the link opens the complete text |
| Source code | where the source code of exactly this version is |
| Website, help | the website of Baruch and this help |
| Data folder | the folder Baruch is working with right now (and the usual one, if it differs) |
| New versions | link to the page with the packages of this version |
| Third-party libraries | where the licences of the bundled libraries are |

**Baruch never asks the Internet for new versions on its own.** The page fetches nothing; only the click on the
link opens the page with the packages in the browser. How to update is described under
[Installation](/en/installation/#aktualisieren). The same page is linked under **Settings → Data** next to the
version.

## Quitting Baruch {#beenden}

{{< hilfebild name="start-beenden" alt="The question Quit Baruch with a short explanation and the buttons Quit Baruch and Cancel." titel="Baruch asks before quitting." >}}

How it works:

1. Choose **Baruch ▾ → Quit Baruch…**.
2. In the question press **Quit Baruch**. **Cancel** leads back to the work opened last.
3. The window shows "Baruch has quit – this window can be closed".

Good to know:

- The window closes by itself if the browser allows it: in the window of its own without an address bar that the
  launcher opens with Chromium, Chrome, Brave or Edge, usually yes. In Firefox and in an ordinary tab the sentence
  stays.
- Saved texts are stored safely; nothing is lost when quitting. Save unsaved entries in other windows first.
  Running answers of the assistants are finished (at most 10 seconds); an open live chat is disconnected
  afterwards.
- Baruch can be quit **only on the computer it runs on** – not from another computer and not through a server in
  front. There the page shows an explanation instead of the button. Assistants cannot quit Baruch either.
- If Baruch runs as a service in the background, it stays off after quitting until the service is started again
  (`systemctl --user start baruch.service`).
- In the terminal, Ctrl+C quits (on the Mac also Ctrl+C). The last line of the output then reads `Baruch beendet.`
  (Baruch quit).
