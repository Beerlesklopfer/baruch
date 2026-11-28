---
title: "Bible, literature and sources"
description: "Checking Bible references and quotations, looking up the Bible, keeping the literature and searching own sources by wording or meaning."
weight: 80
---

Baruch checks Bible references itself – never a language model. Whether book, chapter and verse exist, Baruch
always knows. It can only check the **wording** of a quotation if a Bible is set up and chosen for the work.
Baruch does not bring Bible texts along; how to set them up is described under
[Setting up Bibles](/en/installation/#bibeln-einrichten).

## Choosing the Bible of the work {#bibeln-waehlen}

{{< hilfebild name="bibel-einstellungen" alt="Settings, section Bibles (SWORD): the field with the folders of the Bible modules, below it the table of the modules found with name, kind, versification, licence note and readability, below that the choice Bible of the work." titel="Folders of the Bible modules and the Bible of the work." >}}

Baruch reads Bibles in the free format **SWORD** (this is what the Bible modules of the SWORD project are called;
programs such as BibleTime or Xiphos use them as well). Baruch needs no other program for this.

How it works:

1. Open **Settings → Sources** and go to the section **Bibles (SWORD)**.
2. Under the module folders enter one folder per line in which modules lie, and press **Save folders**. Without an
   own list Baruch searches the usual places (under Linux for example `~/.sword`). **Default** restores that.
3. Check the table: for each module it shows name, description, kind (Bible, commentary, lexicon), versification,
   the **licence note** of the publisher, the encryption and whether Baruch can read the module – otherwise with
   the reason.
4. Under **Bible of the work** choose a Bible and press **Choose Bible**. "none" switches off the check of the
   wording for the work.

- **Encrypted modules:** some publishers only release modules encrypted; you get the key from them. Below the
  table there is then **Keys of encrypted modules** with one field per module and the button **Check and store
  key**. Baruch reads as a test; if the key does not fit, nothing is saved. A fitting key goes only into the
  keyring of the system; only "stored" or "missing" is shown.
- **Commentaries and lexicons:** under **Settings → Work → Commentaries and lexicons** you tick which ones the work
  uses. They appear when a passage is opened and can be passed to the assistants as evidence.
- **Mind the licence:** the licence note of the module appears wherever Bible text appears. What you may do with
  the text is decided by the publisher.

## Checking Bible references and quotations {#zitate-pruefen}

{{< hilfebild name="bibel-zitate-pruefen" alt="The page Check quotations for a section: a table with reference, quotation, result and differences; above it the module used, the versification and the licence note." titel="Check quotations shows for each reference whether the wording is right." >}}

Baruch detects references such as `1. Kön 3,16–28`, `Ps 139,23-24` or `Joh 3,16` (German notation) in the text
and checks them.

How it works:

1. Open the section.
2. Choose **Review → Check quotations** (or **Check quotations** in the status line below the sheet).
3. With JavaScript the list opens above the status line; in the typeset view the quotations are marked at the
   same time: underlined in **green** means "matches", a **red** wavy line "differs". Without JavaScript an own page
   appears with the same table.

The revision shown is checked – with JavaScript, unsaved text as well. The marking does not change the text; every
input removes it, a new click checks again. Esc closes the list.

| Result | Meaning |
|---|---|
| wording matches | also "(partial quotation)" if only part of the verse is quoted |
| wording differs | with the differences: `[−Wort]` is in the Bible and missing in the quotation, `[+Wort]` is only in the quotation |
| no quotation found | the reference stands without a quotation |
| verse missing in the module | the Bible has no text at this place |
| versification does not fit | the Bible counts differently, and the reference cannot be assigned reliably |
| reference invalid | book, chapter or verse does not exist ("Ps 139 has only 24 verses") |
| unchecked | no Bible is chosen for the work |

**What counts as a quotation belonging to the reference:** text in quotation marks directly before the reference
(`„…“ (Joh 3,16)`) or directly after it (`Joh 3,16: „…“`), also with short introductory words in between
(`wie Lk 15,6 erzählt: „…“`), and quotation blocks with the reference as the source. The comparison is generous:
punctuation, case, hyphens and spaces do not count; "…" or "[…]" in the quotation is an omission.

**Ways of writing references:**

- Books with a number as `1. Kön`, `1 Kön`, `1.Kön` or `1. Könige`; `Joh` is the Gospel, `1. Joh` the letter.
- Several references with a semicolon (`Joh 3,16; 4,1`), ranges (`Joh 3,16–4,2`), lists of verses (`Joh 3,16.18`
  or `Joh 3,16, 18`), `f` (one verse more), `ff` (to the end of the chapter), whole chapters (`Ps 23`).
- `Psalm 139, besonders Vers 23` ("Psalm 139, especially verse 23") in the same sentence is understood by Baruch
  as Ps 139,23.
- Abbreviations that are also ordinary German words (`Am`, `Mal`, `Mi`, `Ob`) only count with a verse in running
  text – `Am 3. Tag` ("on the 3rd day") is no reference, `Mi 19:30` after a weekday is a time of day.

**Versification:** the basis is the German numbering (as in the Schlachter Bible of 1951): psalm titles count as a
verse, Joel has four chapters, Malachi three. References from English books sometimes count differently – Baruch
therefore reports "Mal 4,1" as invalid. Baruch nevertheless reads a Bible in English numbering correctly: it
converts the reference and shows what it has read ("read: Mal 4,1–6"). What cannot be converted reliably stays
"versification does not fit" – Baruch never guesses.

**Assistants and Bible references:** no assistant confirms a wording itself. Below an answer that names a reference
there is either a line saying that Baruch (not the model) checked the wording or, without a Bible of the work, a
notice that the wording is unchecked. If a finding claims that a reference is missing although it is in the text,
Baruch appends a remark that the reference is in the text; if it names a reference that does not exist, a remark
that the reference is invalid.

## Looking up the Bible {#nachschlagen}

{{< hilfebild name="bibel-nachschlagen" alt="The Bible page of the literature: at the top the choice of the Bible and the field Reference with Psalm 119,105, below it the verse with its number, the form for quoting and for passing it to an assistant and at the bottom the choice of book and chapter. The picture shows a sample Bible with a single verse." titel="Opening a passage, paging, searching – and using verses." >}}

How it works:

1. Open **Review → Literature**. Below the hits there is the box **Bibles** with the Bibles set up; the Bible of the
   work comes first.
2. Click on a Bible.
3. Enter a reference in the field **Reference**, for example `Ps 139,23-24` – or choose book and chapter and page
   with "previous chapter" and "next chapter". The field **Wording** searches in the Bible.

**Using verses:** the field "Reference" in the form carries the opened passage. With JavaScript a click on a verse
number sets the reference, Shift-click extends it up to there.

- **Quote** creates a **proposal** in the section: the previous text and below it the quotation with the source.
  It only becomes valid when you accept the proposal.
- **Give to persona** sends the verses with a question to the chosen assistant; his answer appears in the chat of
  the section.

Both need a section – so open the literature from within a section. Baruch always takes the Bible text from the
module. Next to the passage, the chosen **commentaries** can be unfolded. With a Bible in another versification
every verse shows the German reference next to it.

The search field of the literature also finds verses (group "Bible · module") and commentaries.

## Literature {#literatur}

{{< hilfebild name="literatur-seite" alt="The literature of the sample work: at the top the search field with the choice wording or meaning, below it the Bible of the work and the titles of the library with short names; a click on a title unfolds details, quotations and highlights." titel="The literature: titles, quotations, highlights – and a search across everything." >}}

The **literature** is the library of the author: titles with details, quotations and highlights.

How it works:

1. Choose **Review → Literature** or press **Alt+L** (unchecked on the Mac, see
   [Keys](/en/hilfe/tasten/#ueberall)). With JavaScript a window opens above the work page; **Close**, Esc or
   Alt+L once more closes it. Without JavaScript it is a page of its own.
2. Click on a title: details, quotations and highlights unfold. **Document view** opens the source.
3. Search in the **search field**. It searches **work and literature** at the same time: the current texts, the
   posts in the chat, titles, quotations and highlights. All words must occur; case and accents do not matter.

A hit opens its target at the place found: in the work the section with the right revision, in the literature the
document view. With a search, the box **Images of the work** also appears below the hits (at most 24 pictures that
match the search).

### Wording or meaning {#bedeutungssuche}

Below the search field, "Search by" chooses:

- **Wording** (default): finds the words entered.
- **Meaning:** also finds places that mean the same without containing the same word. For this a service must be
  set up that compares texts by their meaning – see
  [Settings → Connections](/en/hilfe/einstellungen/#verbindungen).

Hits of the semantic search show title, origin, similarity in per cent and the excerpt. Hits with low similarity
(below 45 %) are called "weak" and are folded at the end. If no service answers, it says "Semantic search not
available: …" with the link to the same search in the wording – Baruch never switches silently. The semantic
search only reads; it never changes a text.

## Reading and highlighting sources {#quellen}

{{< hilfebild name="literatur-quelle" alt="The document view of a source: at the top the details, below them the quotations with coloured highlights, the list of highlights with kind and status and at the bottom the form Use the selected text." titel="Reading a source, highlighting places and quoting them." >}}

The **document view** shows a source: its details, all quotations (with highlights in colour) and all highlights.
If Baruch has the whole text of the source (for example after
[reading in a PDF or an EPUB](/en/hilfe/einlesen/#pdf-epub) or through MultiGPT), the **full text** follows below,
part by part, with "← previous part" and "next part →".

How to highlight:

1. Select text in a quotation or in the full text.
2. Open the context menu with the right mouse button:
   - **Highlight** – with a kind: quotation, key statement, contradiction, to check.
   - **Quote** – highlights and creates a **proposal** in the section: the previous text and below it the
     quotation with the source.
   - **Give to a persona** – gives the selected text with the source and a question to an assistant (default: the
     lector); question and answer appear in the chat of the section. At most 4000 characters.
   - **Find similar** – searches the literature with the longest words of the selection.

Without JavaScript the same works through the form **Use the selected text** below the document view. There every
highlight can also be changed (kind, note, status), taken over as a quotation or deleted.

- Quotations always go into the section from which the literature was opened.
- **Highlights of the assistants** are always proposals. You can reject them or make them your own with **Adopt as
  own**. The search no longer finds rejected ones.
- **Check against the full text:** Baruch checks a highlight without a quotation against the wording of the source
  – "found in the full text", "not found" or "cannot be checked" (with the reason). Nothing is reported as found
  that has not been checked.
- There are no highlights for Bible texts; the page says so.

{{< hilfebild name="einstellungen-quellen" alt="Settings, page Sources: the list of preferred sources with title, address and note, arrows for moving and the buttons Edit and Delete for each entry; below it the form New source." titel="Preferred sources for the web search." >}}

**Preferred sources:** under **Settings → Sources** there is a list of web pages where the assistant @belege
searches first – see [Settings](/en/hilfe/einstellungen/#quellen) and
[Web search for evidence](/en/hilfe/assistenten/#websuche).

## Sync with MultiGPT {#abgleich}

{{< hilfebild name="abgleich-seite" alt="The page Sync with MultiGPT while nothing is connected: the button Sync with the note Never synced, below it the switch for uploading per work (switched off) and the list of collections." titel="Without MultiGPT set up, the page says so honestly." >}}

**MultiGPT** is an own document service that you run yourself; Baruch can sync with it. Whoever has none does not
need this section. The setup is described under [Setting up MultiGPT](/en/mcp/#multigpt-einrichten).

How it works:

1. Open **Review → Sync**.
2. Press **Sync**. The sync runs in the background, only one at a time.
3. After reloading, it says "last synced …" with the numbers (uploaded, replaced, removed, fetched, unchanged,
   errors) and the notices of the run.

Baruch syncs **only when this button is pressed** – never at start, never when saving, never at the request of an
assistant.

- **From Baruch to MultiGPT** go only works with the switch **upload to MultiGPT** (per work on this page and under
  Settings → Work; off by default, switching on only with a ticked check box). Per chapter a document with the
  current revisions is created, in the collection `Baruch: <title of the work>`. Changed chapters are replaced,
  deleted ones removed. **Only the book text** goes out, no posts and findings. Switching off uploads nothing more
  but leaves what already lies there.
- **From MultiGPT to Baruch** come the collections chosen under "Fetch collections from MultiGPT" as a **local
  copy**. Afterwards they can be found in the semantic search even without a network. Limits: 500 documents per
  collection, 2 MiB per document, 64 MiB per sync; the rest follows with the next sync.
- **Errors are notices:** if MultiGPT cannot be reached or a permission is missing, the sync ends "with notes" and
  says why. Locally everything stays unchanged.
- In the semantic search every hit from MultiGPT carries the link **Full text**. **Take into the library** creates
  a title of the literature from it; only the human can do that.

**State of testing:** the automatic tests of Baruch run against a simulated service. Whether the sync with your own
MultiGPT works is only shown by your own attempt – best first with a test work.
