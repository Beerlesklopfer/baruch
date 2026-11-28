---
title: "Assistants, chat and findings"
description: "Having assistants review and propose, working in the chat at the section and deciding about their findings and proposals."
weight: 60
---

An **assistant** is a role that a language model takes on: the lector, the exegete, the critical auditor "Nathan",
a listener from the audience and others. In some places of the interface these roles are called "persona". Each
has a name, a handle for addressing it (`@lektor`), a colour and a task.

These principles apply everywhere:

- **Assistants never change the valid text.** Answers become posts in the chat, new texts only proposals. Only the
  human can accept them.
- **Assistants answer in the language of the work.** German or English is set per work:
  [Work language](/en/hilfe/einstellungen/#werksprache). Baruch recognises the markers of the answers
  ("Critical:", "Kritisch:" …) in both languages.
- **Baruch brings no language model along.** Which service drives the assistants is set once:
  [Setting up assistants](/en/mcp/#assistenten-einrichten). As long as nothing is set up, Baruch says so honestly;
  writing, revisions and typesetting work anyway.

## Review and proposal {#pruefen}

{{< hilfebild name="assistenten-leiste" alt="The right column with the area Assistants unfolded: the list of roles in their colours, the lector is selected, for each role the note local or external; below it a field for a hint, the choice section or whole chapter, the basis of the check and the buttons Review and Proposal." titel="The area Assistants: choose a role, then Review or Proposal." >}}

How it works:

1. Open the section and unfold the area **Assistants** on the right.
2. Choose a role (default: the lector). Add a hint if you like (at most 4000 characters).
3. Choose the **scope**: **Section** (default) or **whole chapter**.
4. Choose the **Basis of the check**: the **shown revision** (default), the **current revision** or one of the open
   proposals of the section.
5. Press **Review** or **Proposal** (the same buttons are found in the ribbon, tab **Start**, group "Assistants").

- **Review:** the assistant reads the section and writes his findings as a post in the chat of the section.
- **Proposal:** the assistant writes a new revision of the whole section as a proposal. The heading always stays.
  If the proposal equals the current revision, nothing is created, and the page says so.
- **Reviewing the whole chapter:** the assistant reviews all parts of the chapter (for a sermon the whole sermon)
  and names the part for every finding.
- **Proposal for the whole chapter:** the assistant writes a new revision for each part of the chapter one after
  the other. Every changed part gets a proposal of its own; acceptance happens part by part. This takes
  correspondingly longer.

**While the assistant is writing** (with JavaScript) the answer appears word by word in a provisional post with a
dashed border and the button **Cancel**. After cancelling, nothing is saved for review and proposal. Without
JavaScript the page reloads after the finished answer; this takes as long as the model needs, and no progress is
visible then.

**What an assistant gets to see:** the whole chapter word for word, the findings of the other assistants on this
chapter, and the lector in addition the [ideas](/en/hilfe/einfaelle/) about the section. The head of every post
names what was checked – for example the current revision 1, or revision 2 as a proposal by @lektor. Baruch writes
this, not the model.

**If the chapter is too long,** Baruch never shortens silently. First the additions become shorter (the post then
ends with a note that the context was shortened). If that is not enough, Baruch has more distant parts summarised
and splits a section that is too long into overlapping pieces; the post then ends with a paragraph on the
compression that names what the assistant only saw as a summary. If even that is not enough, Baruch refuses
honestly and saves nothing. Remedy: have a single section checked.

**Rules that Baruch enforces itself:**

- Answers are in the work language and do not start with phrases such as "Gladly" or "As an AI". If an answer
  starts like that after all, Baruch requests it once more.
- A critical finding needs a literal quotation from the text. If the quotation is not there, Baruch visibly
  appends **"(not backed by the text)"** (in a German work `(nicht im Text belegt)`).
- Every critical point needs a **solution**: "instead of: … – better: …" or a clear next step. If it is still
  missing after asking once more, "(no solution proposed)" follows it.
- The **lector** is a companion: he first says what works, then at most two proposals with a next step. The
  auditor **Nathan** must name at least one critical finding and never approves.
- What the human has already decided is passed to the assistants as decisions of the author that are not to be
  reported again.
- What other assistants have written is shown to an assistant as data, not as instruction.
- If an answer is cut off at the length limit, this is stated at the end of the post; a cut-off proposal is not
  created.
- Only one request runs at a time; up to eight wait, beyond that Baruch reports that the queue is full.
- The page reports errors (service not reachable, timeout, unknown model) honestly; nothing is saved then.

**Local or external:** for each role it says whether its service runs on the own computer ("local") or outside
("external"). A role with an external service **without clearance** is marked as blocked and cannot be chosen –
book text does not go out without clearance. The clearance is given under
[Settings → Assistants](/en/hilfe/einstellungen/#assistenten). There you also set which role is the **lector**:
a fixed one or automatically the one with the largest capacity.

**Evidence from the Bible, commentaries and lexicons:** if a Bible, commentaries or lexicons are chosen for the work
and the switch of the role is on, the assistant gets numbered pieces of evidence [S1], [S2] … along. Every piece of
evidence quoted appears below the post as a source. If the answer names a piece of evidence that did not exist, a
note follows it that the source is unknown.

{{< hilfebild name="assistenten-vorschlaege" alt="Open proposals at a section: two proposals of different assistants marked in colour with a note, the difference from the shown revision and the buttons Accept and Reject." titel="Open proposals wait at the section for the decision." >}}

How to accept or reject proposals is described under
[Writing, pictures and revisions](/en/hilfe/schreiben/#vorschlaege).

## The chat at the section {#chat}

{{< hilfebild name="assistenten-chat" alt="The chat at the section in the right column: posts of the author and two assistants in their colours, for each post the small buttons Copy, Quote, Repeat and Delete; at the bottom the input field with the placeholder ask @persona …" titel="The chat belongs to the section shown." >}}

On the right, in the area **Chat**, are the posts on the section shown. The head shows their number, also when
folded; with JavaScript "· k new" is added, and new posts by others have a coloured line on the left.

How it works:

1. Write into the field **"ask @persona …"** at the bottom.
2. Address an assistant: with `@lektor` anywhere in the message or with the name at the start of the sentence,
   followed by a comma or colon ("Lektor, what do you think?"). In running text the name calls nobody.
3. Send. The message appears in the history at once; every active assistant addressed answers with a post of his
   own in his colour, one after the other (at most three).

Without an address the message stays an ordinary post – a note at the section.

- **List of suggestions:** after "@" the list of the active assistants appears; further letters filter. ↑ and ↓
  choose, Tab or Enter accepts, Esc closes.
- **RFC and @alle:** if the message starts with "RFC" or names "@alle", every active assistant speaks up exactly
  once. The lector answers last: he weighs the findings of the others under "Important now:" and "Can wait:" and
  closes with "My proposal:" – from this Baruch makes a combined proposal.
- **Writing task:** if you ask for text ("@lektor write the ending shorter") or tick **as a task**, the assistant
  writes a **proposal** of the section and reports briefly in the chat. With "as a task" without an address the
  lector writes. A writing task deletes no sections and creates none. If the task names findings such as "[B12]",
  Baruch proposes "adopted" for them; only the human can confirm. Questions ("How does the beginning come
  across?") stay questions.
- **Assistants address one another:** if a line of an answer starts with the address of another assistant, that
  one answers once; his post shows which post it answers. This goes only one level deep and at most three times
  per message.
- **Cancel** ends the running answer; the own message and answers already finished stay.
- **Live:** as long as the chat is open, new posts appear without reloading; above the input field it says
  "@lektor is writing …" as long as a task is running.

**For each post:**

- **Copy** puts the text on the clipboard; **Quote** puts it as a quotation at the start of the input field.
  Quoted text calls nobody; only what you write below it yourself counts.
- **↻ Repeat** asks the same request once more, namely about the revision shown now. The old answer stays; the new
  one says which post it repeats and after which revision. Findings of the old answer that no longer occur in the
  new one are proposed by the assistant as "done".
- **✕ Delete** removes the post after a confirmation. The deleted text remains readable in the history of the work.
- **Manage:** **edit** own posts (the old version is kept, the post carries "edited"), **Move…** into the chat of
  another section, **Convert into an idea** (keywords are required; afterwards the post is an idea) and, for
  answers of the assistants, **Convert into findings**.

**Structured posts:** Baruch shows paragraphs, lists, bold text, quotations and subheadings of the answer in order.
Only Baruch builds links in the post, and only checked ones: to places in the text (literal quotations of 12
characters or more, outline items named unambiguously), to Bible references (`Ps 23,1` opens the Bible of the
work), to "idea 7" and "finding 12". A click jumps to the place and marks it; **Alt+← (Mac: ⌥←)** or the button
"Back to the chat" leads back.

{{< hilfebild name="assistenten-beitraege" alt="The page Whole chat history: all posts of a section one below the other, the newest at the bottom, for each post author, time and the buttons for managing." titel="The whole chat history on one page." >}}

**Scrolling in the chat:** the chat has a limited height and scrolls within itself; when it opens it is at the
newest post. The newest 50 posts are shown; "Older posts (n) …" loads earlier ones. Without JavaScript the same
link leads to the page **Whole chat history**; "Open post" shows a single post there for selecting, quoting,
editing and deleting.

**What the assistants get to see in the chat** is set under
[Settings → Assistants](/en/hilfe/einstellungen/#assistenten): the whole chapter (default) or only the section
shown ("Chat only the section" – saves time with long chapters).

## Findings {#befunde}

{{< hilfebild name="assistenten-befunde-liste" alt="The page Findings of the sample work: at the top the filters, below them the table chapter by assistant with signs and numbers per status, below that the list of open findings with kind, quotation, solution and the disclosure Set status." titel="All findings of a work: table, filters and lists." >}}

From every answer of an assistant Baruch makes **findings**: one per point, with a **kind** (praise, hint,
critical) and a **status** (open, adopted, rejected, moved, done without a change). The post itself stays unchanged
in the chat. **Only the human sets the status;** assistants can only propose it.

How it works:

1. Open **Review → Findings**. The tab shows "N open".
2. Click on a cell in the table; below it exactly its findings are listed.
3. At a finding, unfold **Set status …** and decide: **Adopted** (choose the revision), **Reject** (with a reason),
   **Move** (into another section or an idea) or **Done**. A decided finding can be opened again.

- **The table** has one row per chapter and one column per assistant. Every cell names sign and number per status
  – ○ open, ✓ adopted, ✗ rejected, → moved, = done without a change – and is coloured by the number of open
  findings; a chapter with three or more open ones carries "many open" as text. With the keyboard the arrow keys
  move from cell to cell, Enter opens the cell.
- **Filters:** status, kind, assistant, period and "older than … days", plus "Show praise". Praise is saved but
  hidden by default and never counts as open.
- **Lists:** "open, older than 14 days" (number adjustable), the combined proposals of the lector and the history
  per chapter (what went into which revision).
- **A finding** (link "[B12]") shows kind, status in words, assistant, section, quotation, reason, its
  **solutions** and the history of all changes. The quotation and the link **to the place** open the section and
  mark the quotation.
- **What becomes a finding:** every bullet point of the answer, every line starting with "Critical:",
  "Suggestion:", "Note:" or "Praise:" (or the German markers), and every paragraph under "What works", "What is
  missing", "What disturbs". Chatting without bullet points ("Thanks, that sounds good!") becomes no finding.
- **Duplicates are attached:** if an assistant reports something that equals an already decided finding of the
  same section, it is attached there and does not count as open.

{{< hilfebild name="assistenten-befunde-abschnitt" alt="The right column of the work page with the area Findings unfolded: the open findings of the section shown with kind, quotation, solution, the check box Adopt as a proposal and the disclosure Set status." titel="The findings of the section shown also appear on the right next to the text." >}}

**On the work page:**

- The outline shows "k open" for each item; chapters count their sections along. The same number appears in the
  head of the section.
- The area **Findings** on the right shows the open findings of the section with the same forms; decided ones and
  praise are folded.
- In the tree view of the revisions a revision shows "✓ implements finding [B4], [B7]".

### Adopting solutions and variants {#loesungen}

- **Adopt as a proposal:** a solution with a new wording whose quotation is in the shown revision has a check box;
  several can be chosen at once. The button inserts the wordings and creates a **new revision as a proposal** from
  them. Accepting the revision and confirming the findings stay with the human.
- **Variants:** **Apply solutions** (choice 2 or 3) or a chat message with the word "variants" has the lector write
  two or three variants – each with the label "Variant A/B/C" and one line on what it does differently.
  **compare** places them side by side. **Reject the others** (with a check box) rejects the other variants with
  one click.

### Asking back: finding done? {#rueckfrage}

If an assistant reviews a section again (Review, ↻, RFC or a review task in the chat), he gets his own findings
that are still open along. If he considers one implemented, he does not repeat it but reports "Done: finding 12".
From this an **inquiry** at the finding arises: whether finding 12 seems done and should be marked as done, with
the buttons **Yes, done** and **No, still open**. It appears in the area "Findings", on the page "Findings" and at
the post in the chat.

A plain question ("@nathan how wide is the river?") is no review task; the assistant then does not get his
findings along.

**How a round of critique runs:** an auditor (such as Nathan) reviews the revision. The lector writes a proposal
from the findings. The auditor reviews **this proposal** (basis of the check "proposal n by @lektor"): what is done
he reports as done, the rest stays open. This goes on until the human accepts a revision.

## Web search for evidence {#websuche}

{{< hilfebild name="assistenten-websuche" alt="The page Web search evidence: as long as no web search has run, it honestly says that there is no evidence yet; otherwise for each piece of evidence title, address, date of retrieval, quotation and the result of the check." titel="The evidence of the web search – or its honest emptiness." >}}

If you like, the assistant **@belege** can research on the Internet – through MultiGPT, the own document service
(see [Setting up MultiGPT](/en/mcp/#multigpt-einrichten)). The switch is found under **Settings → Assistants** at
@belege ("research on the web"); **default: off**. If it is off, not a single request goes out. Other assistants
never search the web.

- **Caution:** the search query is the question or the beginning of the section (at most 200 characters). This
  unpublished text goes to the search engine.
- Baruch first searches the preferred sources (Settings → Sources), then generally. At most eight hits, at most
  four pages are fetched.
- **Baruch checks, not the model:** a quotation only counts if the wording appears like that on the fetched page;
  otherwise **"(not supported)"** follows it. If there is no evidence, the answer ends with "check reference: not
  supported". The source line at the end is always set by Baruch.
- Baruch does not fetch addresses in the own network.
- If the web search fails, @belege answers anyway; the reason appears below the answer.
- The link **↪ Web search evidence** at the post leads to the evidence page. For a **supported** quotation, **Into
  the library** takes the page including the quotation into the [literature](/en/hilfe/bibel-quellen/#literatur).

## Threads without a section {#faeden}

{{< hilfebild name="assistenten-faeden" alt="The list of threads of a work: for each thread the beginning of the first message as the title, who started it and the number of posts; below it the form New thread." titel="Threads are conversations that hang on no section." >}}

A **thread** is a conversation with assistants that hangs on no section – for example about the whole book.
Threads change no book text.

How it works:

1. Below the chat choose **Threads without a section**.
2. Write the first message under **New thread**.
3. In the thread, continue writing under **Reply**. The assistants addressed and the one chosen under "Let a persona
   answer" answer, at most three. **Ctrl+Enter (Mac: ⌘↩)** sends.

{{< hilfebild name="assistenten-faden" alt="A single thread: the posts one below the other in the colours of the assistants, below them the form Reply with the choice of which assistant should answer." titel="A thread with its posts." >}}

Copying, quoting, editing own posts, ↻ and ✕ work as in the chat at the section.
