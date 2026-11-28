# Typeset view editor (WYSIWYG)

First step in Round 5, polish in Round 6 (scenes and work assignments as blocks of their own, labels on
headings, images in islands, display only from rules of the top level, typeset view as the default), bugs from the
work on the book in Round 7 (a heading is always one line, title rules, images as images), proposals in
their place and typeset view as the default already in the server HTML in Round 8, more constructs instead of islands, islands
in place and commands of the ribbon in Round 18b.

A section can be edited in the **typeset view**: the way it looks according to the work template (font,
size, colours, indent, text width, boxes of `#szene` and `#arbeit`), instead of in the Typst source. The source
stays Typst; saving works as before through the editor form (`outline.save`), every save is a
new revision. Without JavaScript there is only the source.

Parts: the pure crate `crates/baruch-wysiwyg` (blocks, round trip, HTML, style), the server module
`crates/baruch-server/src/wysiwyg.rs` (RPC `wysiwyg.*`), the widget `web/src/wysiwyg.ts`, the style
`static/wysiwyg.css` and the area in the editor (`templates/fragments/editor.html`, toolbar
`templates/fragments/edit_toolbar.html`). Since the wish of 2026-11-06 `div.wysiwyg` comprises the toolbar
(fixed at the top), the sheet `div.editor-body` (fills the editor and scrolls on its own; in it the editor form with
typeset view and text field and the reading flow of the proposals) and the status line (hints, „Speichern“ and – since
Round 22 – „Als neue Fassung speichern“, both with `form="editor-form"`). Without JavaScript the toolbar,
the typeset view and its hint are `hidden`.

## Block model

`baruch_wysiwyg::parse(text)` splits a section text into **blocks**. Every block carries its
source `raw` **literally**, including the empty lines behind it; the concatenation of all `raw` is the text, byte
for byte. `parse` never fails.

| Kind (`kind`) | Typst                                                         |
|---------------|---------------------------------------------------------------|
| `heading`     | `=` to `======` at the start of a line (one line), `level` 1–6; `label`: a label alone on the line behind it (`= Titel⏎<name>`) |
| `paragraph`   | running text up to the empty line                             |
| `list`        | `- …` (`ordered: false`) or `+ …` (`ordered: true`), continuation lines indented; Round 18b: nested with `depths` (see below) |
| `enum`        | Round 18b: enumeration `1. …`; `start` = number of the first item (default 1), `items`, `depths` as with `list` |
| `terms`       | Round 18b: term list `/ Begriff: Erklärung`; `items: [{term, desc}]` (inlines each), without nesting (limit, see below) |
| `quote`       | `#quote(block: …, attribution: […])[…]` as a whole paragraph  |
| `scene`       | `#szene[…]` or `#szene(…)[…]` as a whole paragraph (also multi-line, directly behind heading and label); `args` = arguments literally, `body` = blocks |
| `work`        | `#arbeit[…]` or `#arbeit(…)[…]`, like `scene`                 |
| `part`        | Round 14: part marker `#heading(level: 1, numbering: none)[T] <baruch-part>` (one line, top level only); `title` = resolved title |
| `unnumbered`  | Round 14: marker „ohne Nummer“ `#heading(level: N, numbering: none)[T] <baruch-unnumbered>`; `level` 1–3, `title` |
| `island`      | everything that cannot be mapped safely (see islands)         |

In running text (`inlines`, tag `t`):

| `t`        | Typst                                                                   |
|------------|-------------------------------------------------------------------------|
| `text`     | text, escapes resolved (`\#` → `#`, `\u{2014}` → —, `~` → non-breaking space) |
| `soft`     | simple line break in the source (a space in the typeset text)           |
| `emph`     | `_…_` or `#emph[…]`                                                     |
| `strong`   | `*…*` or `#strong[…]`                                                   |
| `footnote` | `#footnote[…]`                                                          |
| `quote`    | `#quote[…]` within a sentence                                           |
| `label`    | `<name>`                                                                |
| `island`   | Typst island within a sentence (`raw` literally)                        |
| `linebreak`| Round 18b: forced line break `\` (before whitespace, line end or at the end; spaces behind it belong to it) |
| `link`     | Round 18b: `#link("adresse")[Text]` → `{url, c}`; without a content block `#link("adresse")` → `{url}` (the address is shown) |
| `ref`      | Round 18b: reference `@ziel` or `@ziel[Zusatz]` (also `#ref(<ziel>)`, `#ref(<ziel>, supplement: […])`) → `{target, supplement?}`; `supplement` is Typst markup literally |
| `xref`     | Round 30a: cross-reference `#link(<kN>)[Text]` or with a page number `#link(<kN>)[Text, S.~#context counter(page).at(<kN>).first()]` → `{key, c, page}` (see "Cross-references") |

`_` and `*` between two word characters are text (as in Typst: `datei_name`).

**JSON form of the new constructs (Round 18b).** Old forms stay valid: a list without `depths` has all
items at the top, and without nesting the server does not write `depths` (answers as before); `enum` without `start`
begins at 1. Examples:

```json
{"kind": "list", "ordered": false, "items": [[{"t":"text","v":"Obst"}], [{"t":"text","v":"Äpfel"}], [{"t":"text","v":"Gemüse"}]], "depths": [0, 1, 0]}
{"kind": "enum", "start": 3, "items": [[{"t":"text","v":"drei"}], [{"t":"text","v":"vier"}]]}
{"kind": "terms", "items": [{"term": [{"t":"text","v":"Krug"}], "desc": [{"t":"text","v":"Gefäß aus Ton"}]}]}
{"t": "linebreak"}   {"t": "link", "url": "https://example.org", "c": [{"t":"text","v":"Beispiel"}]}   {"t": "ref", "target": "abb-1", "supplement": "S. 3"}
```

**Lists, enumerations, term lists (Round 18b).** Every line that, after its indent, begins with the character of the list
(`-`, `+`, `N.` or `/`) begins an item; the depth follows from the column of the character (further right than
the item before = one level deeper, otherwise the level with the same column). Other lines are continuation lines and
must be indented further than the character of their item. An enumeration counts continuously: at the top from `start`, every
nested group from 1. The term is the text up to the first colon outside of escapes, labels,
references, raw, formulas, `#` expressions and content blocks. **They stay islands:** nested items with a different
character (`- a⏎  + b`), a column between two levels, tabs in the indent, empty items, jumping numbers
(`1.`, `3.`), nested term lists, a term without a colon, more than nine levels. Generation (`emit`)
is canonical: two spaces of indent per level, continuation lines one level deeper, continuous numbers, in the term
every `:` escaped (`\:`), otherwise the term would end there.

**Limit: nested term lists** (checked in Round 21b, they stay islands). `terms` has no `depths` like `list`
and `enum`: an explanation that itself contains a term list (or list) would be a `<dd>` with
a block of its own inside it in the typeset view – the model (`Term { term, desc }` with inlines), `emit`/`serialize`, `html.rs`, the reading back in
`wysiwyg.ts` (`<dl>` → blocks) and the property test would have to grow nested blocks together for that. The
island is lossless (text byte-identical, editable in the source, typeset correctly); hence no rebuild in 21b.
Not implemented in Round 23, stays open (not implemented in R24, backlog „offen nach R24“; backlog „Nach Runde 23 offen“; together with mixed
list characters, once the typeset view learns nested blocks in list items).

**Break and line end (Round 21b):** a `Soft` directly behind a `Linebreak` (`\` at the end of a line) appears as an
empty `<span class="ws-soft" data-after-break="1"></span>`: Typst swallows this line end; the typeset view would otherwise indent
the next line by a space. The element stays so that the widget still reads it back as `soft`
(the text stays byte-identical).

**Generating references and breaks.** A reference stands as `@ziel` if the target begins with a letter, digit or `_`,
does not end in `.`/`:` and the character behind it would not belong to the target; otherwise as `#ref(<ziel>)` (e.g.
before `x` or `_`). Behind a forced break stands a space, except at the end of the block and before a
line end (`\]` or `\_` would be escapes). A `\` before whitespace is a break also when splitting into lines,
not an escape of the line end: the line end behind it still ends the line (as in Typst).

**Scenes and work assignments.** A line that is exactly one `#szene…[…]` or `#arbeit…[…]` is always
a block of its own and does not join neighbouring lines. Its body consists of blocks of its own
(paragraph, list, quotation, heading; what cannot be mapped stays an island in the body). The common indent
of the body does not count when reading (indented lists become lists); the `raw` of the body blocks are
pieces of the original including the indent. Arguments before the body (`#szene(ort: [Hof])[…]`) stand literally in
`args` and can only be changed in the source.

**Labels.** A label at the end of the heading line stands in its `inlines` (as before). A label alone
on the line directly behind the heading also belongs to the heading (`label`, `raw` comprises both
lines), not to a paragraph; behind a paragraph it belongs to the paragraph as before.

## Islands

A **Typst island** is source that the typeset view cannot map safely. It appears with a yellow
background and `contenteditable="false"` and is preserved character for character as long as nobody changes it. In the
typeset view an island can be **deleted as a whole** and, since Round 18b, **edited in place**
(double click, see "Islands in place").

- **Block islands:** code lines (`#let`, `#set`, `#show`, `#import`, `#if` …), paragraphs consisting only of islands
  (`#pagebreak()`, formula `$ … $` as a block, raw block ```` ``` ````, comment lines, a paragraph consisting only of `\`),
  lists with mixed characters and the other unsafe lists (see above), indented headings,
  empty lines at the start, and every paragraph that contains something unsafe: square brackets without escaping, unclosed
  `_`, `*`, `$`, `` ` ``, `/*`, a keyword in the middle of a sentence. No longer islands since Round 18b:
  enumerations `1.`, term lists `/ …`, nested lists, the forced line break `\`.
- **Islands within a sentence:** formulas `$…$`, raw `` `…` ``, comments `// …` and `/* … */`,
  own functions and all other `#` expressions (`#box[…]`, `#zitat(…)`, a `#link` with more than one
  address as argument; `#link("…")[…]` and references `@name` are constructs of their own since Round 18b), as well as
  escapes that as text would become a shorthand: `\'` and `\"` (straight instead of typographic
  quotation marks), `\-` (otherwise `--`, `---`, `-?`), `\.` next to a full stop (otherwise `...`), `\?` and an
  escaped digit behind `-`. Other escapes (`\#`, `1\.` …) become text and are escaped again on saving.
- **Line ends:** Typst also breaks at U+2028, U+2029, U+0085, `\x0B` and `\x0C`. In the typeset view (typing,
  pasting) and when generating, all these characters in paragraphs become spaces
  (`baruch_typst::is_newline`); otherwise text would stand unprotected at the start of a line (`= `, `- ` …). In a
  **heading**, on the other hand, a line end ends it (see "Headings" below), and a title with a
  line end from the tree or from a heading line is rejected (Round 7, `baruch_store::check_title`;
  before, it silently became a space).
- A heading with a comment (`== Titel // Notiz`) is not an island: the comment does not belong to the title
  and is preserved on saving.

### Islands in place (Round 18b)

- **Usage:** a double click on an island (also on an image, an island within a sentence or in the body of a scene) opens
  a small source field in its place (`.ws-island-edit`, text field with „Übernehmen“/„Abbrechen“).
  Ctrl+Enter or „Übernehmen“ (also leaving the field after a change) accepts, Escape or
  „Abbrechen“ restores the island. Keys in the field do not go to the typeset view (Enter splits no paragraph).
  Islands in proposals (`.pv`) open no field.
- **Saving:** a changed block island (`data-edited="1"`, new raw text in `data-raw`) goes as
  `{"replaces": i, "raw": "<Typst>"}` to `wysiwyg.serialize` (in the body of a scene in its body instruction); the
  server takes the text **literally** (trailing whitespace is dropped, the separator of the old block stays; in the body
  the indent of the body is set uniformly, also before every line of a raw block ```` ``` ````; the text may be sent
  with the indent – as in `data-raw`, the first line with or without it – or entirely without it: it counts
  as sent along if every non-empty continuation line begins with it, otherwise the text stays as it is, and the
  indent of indented code lines does not change), an empty text deletes the island. An island within a sentence belongs to
  its paragraph: the paragraph is generated anew with the changed island, the island stands in it literally.
- **Limits:** `{"raw": …}` without `replaces` (a new island) stays forbidden (`NewIsland`), an unknown or
  duplicate index is an error (-32602). The server does not check the new raw text; what it causes (also a new
  heading) is checked, as always, by the outline on saving.

## Headings (Round 7)

Cause of the bug „Absatz wird Titel“ (backlog, night of 05/06 Nov 2026): the widget sent a heading
whose title was followed by a simple line break (Shift+Enter or a `<br>` of the browser, then the
typed or pasted paragraph) as **one** heading block `{kind: heading, inlines: [Titel, soft,
Absatz]}`. `emit` made a single heading line out of it (`soft` in headings → space), and
`outline.save` took the whole line as the title of the outline item. Proven by
`crates/baruch-server/tests/round7_titles.rs` (fails without the fix with the title „Laub Anna's Laube …“).

Since then:

- A heading is **always one line**: `emit` and `serialize` end it at the first line break of the
  top level (`Inline::Soft` or a line end in the text); what stands behind it becomes a paragraph of its own
  below it (breaks at the start do not count). Label and comment of the original line stay with the
  heading. Only real heading lines (column 0, 1–3 `=`, then space/tab, like
  `baruch_typst::parse`) deliver titles.
- The widget already sends such a heading split itself (heading + new paragraph); Shift+Enter
  and pasting several lines begin a new paragraph in a heading; Backspace at the start of a
  paragraph behind a heading or Delete at the end of a heading do not pull the paragraph into the
  heading (an empty paragraph is dropped).
- **Title rules** (server, `baruch_store::check_title`): empty, only whitespace, a line break or more than
  200 characters → rejected (-32602), nothing stored. Before transferring into the text field the widget checks
  the same rules with the same message („Ein Gliederungspunkt braucht einen Titel.“, „Der Titel darf
  keinen Zeilenumbruch enthalten.“, „Der Titel ist zu lang (höchstens 200 Zeichen, hier N).“); the server
  stays authoritative. A whole paragraph that got into the heading line by mistake thus fails
  on the length.

## Markdown headings (user 2026-11-07)

A line that begins with 1–6 `#` and a space is a heading of that level. Authoritative is
the rule of the server, `baruch_typst::markdown` (`markdown_headings`, table test with all exceptions): it
applies it when the human saves (`outline.save`, `outline.check`, the query before splitting,
`revision.save`, since Round 18b also `revision.update` (change revision), also without JavaScript) and when a proposal is accepted (`revision.accept`); afterwards
`outline_save` aligns the outline as with a typed `=` line.

- `#` → `=`, `##` → `==`, `###` → `===`; `####` to `######` are **capped** at `===` (the outline has
  three heading levels; whether the level fits at the place is checked, as always, by the outline on saving).
- Title literally as in a `=` line, a closing sequence ` ##` and trailing whitespace are dropped; `# ` without a title
  stays.
- **Not converted:** `\#` at the start, a hash in quotation marks (`'# …'`, `"# …"`, „…“), Typst code
  (`#heading(…)`, `#set`, `#let` – no space after the hash), indented lines, lines in raw blocks,
  comments, strings and open brackets (content blocks such as `#quote[…]`) – the same places where
  a `=` line is not a heading either –, and the work template (it does not run through the rule).
- **Live in the widget** (`web/src/markdown.ts`, the same cases, when in doubt no conversion): in the source on
  Enter at the end or within the line (the cursor stays at its place in the title, then the line break) and on
  leaving the text field for all lines; in the typeset view a paragraph whose text begins with `# ` … becomes, on
  Enter (and at the latest before transferring into the text field), a heading block of that level, the hashes
  disappear, the cursor stays in the title. In the typeset view `emit` otherwise writes a typed hash as `\#`
  (text), which is why only the widget converts here.
- **Proposals** of assistants stay as they are written (no conversion, no outline).
  If the human accepts a proposal with Markdown headings, the proposal stays unchanged in the revision tree;
  it is followed by a revision of the human with the converted headings (further headings
  become new items, the acceptance is the confirmation). If that does not work, the acceptance stays as it is and
  `markdown` in the answer says why.

## Parts and „ohne Nummer“ (Round 14)

- **Recognising:** a line is a `part` or `unnumbered` block exactly when `baruch_typst::parse` reads it
  as a part (`level` 0) or as a heading with `numbered: false` – so only in the canonical form
  (`docs/Architektur.md`, "Parts in the Typst text (Round 14)"), not indented, only at the top level (in the body of a
  scene the same line is an island, just as it is text for `baruch_typst`). The line is always a
  block of its own like a heading; a comment behind it belongs to `raw`, not to the title.
- **Keeping:** `{keep: i}` takes the line literally (escapes, comment).
- **Changing:** `{replaces: i, block: {kind: "part", title}}` rebuilds the line with `baruch_typst::part_line`
  (`heading_line(0, …)`: a comment of the old line stays, even if it was a `=` line);
  `{kind: "unnumbered", level, title}` with `unnumbered_line` (if it replaces a marker „ohne Nummer“, its
  comment stays). A new block (`replaces: null`) yields the marker without a comment. For every title
  `parse(emit(k)) == k` holds (property test `random_markers_read_back_unchanged`).
- **Part in the chapter text (Round 18b, decided):** the typeset view offers no part. `wysiwyg.serialize` rejects a
  **new** part block (`replaces: null`) and converting another block into a part
  (`WysiwygError::NewPart`, -32602 „Einen Teil legt man in der Gliederung an, nicht im Satzbild.“); renaming an existing
  part still works. If the human writes the part marker in the source as a further heading into a
  chapter text, `outline.check` demands the query about splitting; without confirmation `outline.save` stores nothing,
  with confirmation (`split`) the part comes into being (an action of the human). Test:
  `crates/baruch-server/tests/round18_konstrukte.rs`
  (`a_part_block_in_a_chapter_text_is_refused_in_the_satzbild_and_needs_confirmation_in_outline_save`).
- **HTML:** `<div class="ws-block ws-part" data-b="i" data-kind="part" data-level="0" role="heading"
  aria-level="1" title="Teil (Markierung im Typst-Text)">Titel</div>` or `class="ws-block ws-unnumbered"
  data-kind="unnumbered" data-level="N" role="heading" aria-level="N"`; the title stands as plain text (escaped),
  „Teil“ or „ohne Nummer“ is set by the stylesheet through `::before`/`::after` (`static/wysiwyg.css`) so that it
  does not get into the title.
- **Widget:** `kindOf` reads `data-kind=part|unnumbered` with the text content as the title (`markerTitle`:
  line ends and non-breaking spaces → spaces); part and „ohne Nummer“ behave like
  headings (`isTitleKind`: Enter and pasting several lines begin a paragraph behind them,
  `guardHeadingMerge`, title rules in `headingProblem`). The paragraph format cannot be chosen for them
  (message); parts are created in the outline, not in the typeset view.
- **Without JavaScript** the text field keeps the marker literally; saving as before.

## Round trip

Loading and saving without a change yields **the same text, byte for byte**. For that
`baruch_wysiwyg::serialize(original, edits)` builds the text from instructions:

- `{"keep": i}`: block `i` of the original literally (including its empty lines). The widget sends `keep` for
  **every block without input**; an untouched block is thus never generated anew.
- `{"replaces": i, "block": {…}}`: generate block `i` anew (canonical Typst, `baruch_wysiwyg::emit`), with
  the empty lines of the old block behind it. An edited heading keeps label and comment of the
  original line (like `baruch_typst::heading_line`; the label only if the new one carries none).
- `{"replaces": null, "block": {…}}`: new block, followed by an empty line.
- `{"replaces": i, "body": [instructions]}`: change only within the body of scene or work assignment `i`, with the same
  rules for the blocks of the body (indices within the body). Frame (`#szene(…)[`, `]`), arguments and whitespace
  stay literal; newly generated blocks get the indent of the body. The indent of the first line of the
  body belongs to the first block and moves with it when something is inserted before it or rearranged (otherwise
  its first line would stand in column 0 and a list would become nested). So editing a
  paragraph in a scene changes only that paragraph. No body (paragraph, island …) → error.
- An edited heading also keeps its label line literally; if the new block carries a different
  `label`, `<neu>` stands there.
- A heading with a line break ends there; the rest becomes a paragraph behind it (Round 7, see above).
- Blocks not named are deleted. A block that comes out empty is dropped. If two pieces do not follow each other
  as in the original, at least one empty line stands between them.
- `{"replaces": i, "raw": "…"}` (Round 18b): replace the source of block `i` literally (island in place,
  see above), with the separator of the old block; empty = deleted.
- Errors (`WysiwygError`, German): unknown index, an index more than once (also `keep` and `replaces`),
  an island as a new block (also `{"raw": …}` without `replaces`) or an island edited as a block, a
  body instruction for a block without a body, a new or converted part (Round 18b, `NewPart`).

Canonical Typst (`emit`): special characters are escaped (`\#`, `\*`, `\_`, `\$`, `\@`, `\<`, `\[` …), at the
start of a line also `=`, `-`, `+`, `/` and `1.`; a non-breaking space becomes `~`; `_…_`/`*…*` only at
word boundaries and not nested in themselves, otherwise `#emph[…]`/`#strong[…]`; behind a `#` expression
`(` and `.name` are escaped; list items with two spaces of indent in continuation lines; `#szene[…]` and
`#arbeit[…]` multi-line, `#quote(…)[…]` with a single-line paragraph on one line.

Proven by tests in `crates/baruch-wysiwyg/tests/`:
- `roundtrip.rs`: all test files of all crates (`crates/*/tests/data/*.typ`, only read) and the own
  `tests/data/satzbild.typ` (all constructs and islands) and `tests/data/szenen.typ` (headings with
  label, scenes with several paragraphs and a footnote, work assignment with a list, image island), both freely
  invented, whole and per section;
- `blocks.rs`: scenes, work assignments, labels and body instructions (only the edited paragraph changes),
  pasting several paragraphs with the instructions that the widget sends;
- `serialize.rs` (Round 7): a heading with a line break ends there, also when newly inserted and with
  label/comment; `html.rs`: image islands as `ws-figure` with escaped attributes, byte-identical with `keep`;
- `emit.rs`: `parse(emit(k)) == k` for every kind except the island, plus a property test with a fixed seed over
  2000 random canonical blocks (more with `WS_CASES=…`, another seed with `WS_SEED=…`), since Round 18b with
  enumerations, term lists, nested lists, breaks, links and references; the gap of the generator
  (whitespace at the start of an emphasis in a heading, `canonical_heading`) is closed with the case from the backlog
  as a test of its own;
- `konstrukte.rs` (Round 18b): recognising, byte-identical when untouched, canonical when edited, unsafe lists as islands,
  old JSON forms, `Edit::Raw` (also in the body of a scene), several images, HTML of the new constructs;
- `align.rs` (Round 8): alignment of two revisions by blocks, see "Proposals in their place";
- `serialize.rs`, `html.rs`, `style.rs`; plus `crates/baruch-server/tests/wysiwyg.rs` over all revisions of the
  sample work (in the source and seeded) and the exports, `crates/baruch-server/tests/satzbild.rs` for the
  polish (image address, figure as an image and stored byte-identically, style, page without JavaScript,
  widget with title check), `crates/baruch-server/tests/round7_titles.rs` for „Absatz wird Titel“ and the
  title rules on all paths (RPC, form, MCP).

## Proposals in their place (Round 8)

Open proposals no longer stand as a list under the editor but **at the place they change**: in the
typeset view, in the source reading mode and in the preview, in the colour of the persona, collapsible to a coloured
line exactly there.

**Alignment** (`baruch_wysiwyg::align(base, proposal) -> Vec<Hunk>`, pure, never fails): both texts are
split into top-level blocks with `parse` and compared by the longest common subsequence. Two
blocks are equal if their `raw` without trailing whitespace is byte-identical (the empty lines behind a block belong to
its `raw`; otherwise every appending would report the previously last block as changed). Every contiguous sequence
of unequal blocks is a `Hunk { kind, base, proposal }` with index ranges of the blocks:

| `kind`     | `base`                | `proposal`         |
|------------|-----------------------|--------------------|
| `Changed`  | not empty             | not empty          |
| `Inserted` | empty; `base.start` = block before which it is inserted (at the end the number of blocks) | not empty |
| `Removed`  | not empty             | empty              |

Between the pieces stand equally many, pairwise equal blocks on both sides (also zero: pieces may
abut). Above 4 million comparisons (number of blocks × number of blocks) everything counts as one piece.

**Similar blocks (Round 18c):** the longest common subsequence knows only equal blocks; a reworded
paragraph next to an inserted one thus became a single "changed" piece over both. A "changed" piece with
**unequal numbers** of blocks on the two sides is therefore split by similarity: word Dice (2 · common words /
all words, lower case, with multiplicity) ≥ 0.5 counts as "reworded"; the pairs are chosen monotonically with
the largest sum of similarities. Pairs become "changed" (consecutive pairs stay one
piece), what stands between them on one side only becomes "inserted" or "removed", what stands unpaired on both sides
stays "changed". With equally many blocks (replaced pairwise), without a similar pair or above 10 000
comparisons in the piece it stays as it was. Example: „Im Beet stehen Kohlköpfe.“ → „Im Beet stehen heute
Kohlköpfe.“ + a new paragraph behind it yields `Changed(2..3, 2..3)` and `Inserted(3..3, 3..4)`. `hunk_html_with(base, proposal, &hunk, &bilder)` shows a
piece in the typeset view: `<div class="ws-diff-doc" data-kind="changed|inserted|removed">`, the blocks of the basis in
`div.ws-del`, those of the proposal in `div.ws-ins` (like `diff_html_with`, without `data-b`, images as there).
Tests: `crates/baruch-wysiwyg/tests/align.rs` (changed; inserted at the start, in the middle, at the end; removed;
several pieces; typeset view; property test with a fixed seed over 1500 cases, `WS_SEED`/`WS_CASES`; Round 18c:
reworded + inserted or + removed as two pieces).

**Basis** is the **shown** revision (open in the editor, mostly the current one) – that is what „Übernehmen“
replaces. If the proposal rests on another revision, the header names it („· auf Fassung m“). If the
shown revision is itself an open proposal, its pieces against its basis (parent, otherwise the
current one) stand at their place within it, with the note „Unterschied zu Fassung b (aktuell) · gezeigt“. If a
proposal equals the shown revision, it stands as one piece at the end („gleicht der gezeigten Fassung“) so that
„Übernehmen“/„Verwerfen“ remain.

**HTML** (`templates/fragments/proposals.html`, context `InlineView` from `view.rs`): per piece

```html
<details class="pv pc" data-pv="<key>-<rev>" data-pv-at="<b>" data-pv-gap="<g>"
         data-pv-kind="changed|inserted|removed" style="--pc-light:…;--pc-dark:…;--pc-line:…;--pc-line-dark:…" open>
  <summary class="pv-line"><span class="pv-label">Name (@handle) · Fassung n · Modell · auf Fassung m · Teil i/k</span></summary>
  <div class="pv-body"> Notiz · div.pv-satz (hunk_html_with) · pre.pv-quelltext (Zeilen-Diff des Stücks)
    · Knöpfe „Übernehmen“/„Verwerfen“ (form="pv-accept-<key>-<rev>" bzw. "pv-reject-…") </div>
</details>
```

`at` is the first affected block of the shown revision (`data-b` from `wysiwyg.parse`), for inserted ones the
block before which it is inserted; `gap` the gap in which the piece stands (before block `gap`, i.e. behind a
changed or removed block, at the end the number of blocks). The **header stands only once**, in the line
(`summary`); „Teil i/k“ only with several pieces. Several proposals at the same place stand one below the other
in persona order, within that by number; authors without a persona at the end. The forms (`revision.accept` or
`revision.reject` to `/act` with `work`, `node`, `key`, `rev`, `view`, `lq`, unchanged) stand **once per
proposal** outside the pieces (`div.pv-forms`); the buttons point to them through the attribute `form`. So they stay
valid when the widget puts the pieces into the typeset view, which lies in the editor form (forms cannot be
nested, and `outline.ts` would take a submission in it for saving). The cookie `baruch_folded`
(`<key>-<rev>`) collapses all pieces of a proposal; the shown proposal stays open (`pv-shown`).

**Reading flow in the editor:** `<section class="pv-flow" data-widget="proposals" data-key="K" data-rev="R">` below the
editor form (i.e. below the text field), only if there are open proposals: the forms, the shown revision
block by block as source (`<pre class="pv-src" data-b="i">raw</pre>`) and the pieces at their place. Since Round 18c the
**bar** (count, „Alle einklappen“) stands **before** the reading flow as a child of the sheet of its own
(`<div class="pv-bar pv-bar-editor" data-pv-bar="K">`); CSS (`proposals.css`) moves it to the very top with `order: -1`
and makes it stick while scrolling. So „Alle einklappen“ stays visible even with an open typeset view when
`wysiwyg.ts` hides the reading flow (before, the bar stood in the hidden reading flow); `proposals.ts` finds the
button through `data-pv-bar` at the sibling of the reading flow, `wysiwyg.ts` stays unchanged. In the preview
the bar still stands in the section. The **line diff** of a piece compares the blocks without trailing whitespace
(Round 18c): the empty line behind a block belongs to its `raw` and otherwise stood as an empty "−" line at the end. Without JavaScript and in the source this is the view of the
proposals (line diff). With JavaScript and an open typeset view, `wysiwyg.ts` puts every piece as a non-editable
sibling (`contenteditable="false"`) before the block `[data-b=gap]` of the typeset view (at the end
behind it) and hides the reading flow; in the typeset view the piece shows its typeset view (`pv-satz`), not the
line diff (CSS only). When switching to the source and before every new typeset view the pieces return to their
place in the reading flow. `editsOf`, `blocksIn` and `markSelection` skip `.pv`; saving therefore changes
nothing.

**Preview** (`view=preview`): per section of the chapter a `div.pv-section` with its blocks as lines
(`h1`–`h3`, `p`) and the pieces (line diff only) between the blocks at their place; the basis is the revision shown
there (current or set on trial).

**Colours:** on the always light sheet (typeset view, preview) the pieces stand in the light values
(`--pc-line`, `--pc-light`), otherwise per theme `--pc-line` or `--pc-line-dark`. Both lines reach
at least 3:1 against the surface of their theme (WCAG 1.4.11, `crate::personas::style`, `MIN_LINE_CONTRAST`),
`--pc-line` at the same time against the sheet of the dark theme (`--sheet: #f4f1ea`, `DARK_SHEET`):
if the colour does not suffice, it is darkened for light and lightened for dark (in steps of 5 %).

## Typeset view as the default (Round 8)

Bug from the visual check: after loading, „Quelltext“ was active on all screenshots although the default
is „satz“. **Cause** (code, state development f601260): the server always delivered `aria-pressed="true"` on
„Satzbild“ and an **empty** typeset view. On mounting, `wysiwyg.ts` first called `show(state, root)` with
`open = false` – that presses „Quelltext“, hides tools and sheet and shows the text field – and opened the
typeset view only in `openSheet` after two RPC answers (`wysiwyg.parse`, `wysiwyg.style`). Loading the page
(event `load`) does not wait for these `fetch` calls; a screenshot directly after loading
(`firefox --screenshot`) therefore shows the intermediate state „Quelltext“. An error on opening would have made it
permanent, but no error was needed.

**Fix:** the server draws the state itself: `Settings.editor` (`satz` | `quelltext`, from the cookie
`baruch_editor`, default `satz`) sets `aria-pressed`, `data-ws-start` and – with „quelltext“ – `hidden` on
tools, hint and sheet. With „satz“ the typeset view of the shown revision stands finished in the sheet
(`<div … data-ws-sheet … data-ws-ready>…</div>`, the same HTML as `wysiwyg.parse`, with images). The widget
adopts it immediately on mounting (`adoptSheet`), without an intermediate state; the style of the work template follows through
`wysiwyg.style`. If the text field does not deliver the same text as the revision (a line break directly at the
start, which the browser strips behind `<textarea>`, or `\r`), `data-ws-ready` is missing, and the widget opens
the typeset view as before through RPC – without switching to the source first (only the text field is briefly
hidden; if opening fails, the source comes). `#fundstelle` and the cookie „quelltext“ open the
source as before. Test: `crates/baruch-server/tests/round8_inline.rs`
(`the_server_already_renders_the_satzbild_as_default_and_the_source_only_on_request`, fails without the
fix on `data-ws-start`/`data-ws-ready`).

**Long sections (Round 18c):** above 64 KiB of text (`view::READY_SHEET_MAX`, bytes) the server does not put the typeset view
into the HTML in advance – it doubled the text (text field, sheet, with proposals also the reading flow). Then
`data-ws-ready` is missing, and the widget fetches the typeset view as above through `wysiwyg.parse`, without showing the source first.

**Style already in the HTML (Round 18c):** the editor brings along `<style id="ws-style" data-work="W">` (CSS from
`sheet::sheet_css_for`: work template, overridden with the typeset view settings of the work, see below) so that
the sheet does not change font and size after loading; `wysiwyg.ts` adopts an existing `#ws-style` with
the same `data-work` (Round 18b).

**Behaviour test (Round 18c):** `crates/baruch-server/tests/round18_satzbild.rs`
(`satzbild_as_default_behaves_right_in_the_real_widgets_without_a_browser`) runs the compiled widgets
`wysiwyg.js`, `proposals.js` and `sheet.js` in Node 20 on a small mocked DOM against the editor HTML
of `ui.render` – without a browser, without network (`fetch` is mocked and counts the RPC calls). The test proves
the behaviour: without a cookie the typeset view is already open on mounting (text field hidden, „Satzbild“ pressed,
no `wysiwyg.parse`), the pieces stand in the typeset view, the reading flow is hidden and „Alle einklappen“ visible;
with the cookie „quelltext“ the text field stays open; without a typeset view set in advance the widget fetches it through
`wysiwyg.parse` and opens it afterwards. Only what the widgets need on mounting is mocked (selectors with
tag, `#id`, `.klasse`, `[a]`, `[a=b]`, descendant, child, `:scope`); what is missing, the test reports as an error in
the mock. Without `node` it is skipped with „ÜBERSPRUNGEN“.

## Display according to the work template

`baruch_wysiwyg::sheet_style(vorlage)` reads from the work template only rules of the **top level** (comments
and raw text do not count; the last rule wins, also several `#set` on one line). Rules in
`#show …: it => […]`, in `#show: …` bodies, in function bodies (`#let f() = { set par(…) }`), in `#if`
or in other expressions hold only for part of the document and do not count; nor does
`#show footnote: set text(size: …)` (only `emph`, `strong` and `heading` with their own fields):

| Rule                                          | Field                        | CSS variable              |
|-----------------------------------------------|------------------------------|---------------------------|
| `#set text(font:, size:, fill:)`              | `font`, `size`, `fill`       | `--ws-font`, `--ws-size`, `--ws-fill` |
| `#set par(first-line-indent:, leading:, justify:)` | `indent`, `leading`, `justify` | `--ws-indent`, `--ws-leading`, `--ws-justify` |
| `#set page(width:)` or `paper:`               | `width`                      | `--ws-width`              |
| `#show emph: set text(fill:)`                 | `emph_fill`                  | `--ws-emph-fill`          |
| `#show strong: set text(fill:)`               | `strong_fill`                | `--ws-strong-fill`        |
| `#show heading: set text(font:, fill:)`       | `heading_font`, `heading_fill` | `--ws-heading-font`, `--ws-heading-fill` |
| `#let szene(…) = block/rect/box(fill:, inset:, stroke:)` | `scene`           | `--ws-scene-fill/-inset/-stroke` |
| `#let arbeit(…) = …`                          | `work`                       | `--ws-work-fill/-inset/-stroke` |

Colours: `rgb("#…")`, `rgb(r, g, b)` (also percent), `luma(n)`, `cmyk(…)` and the colour names of Typst;
lengths in pt, mm, cm, in, em. What cannot be translated (e.g. `red.lighten(20%)`) is missing; then
the default from `wysiwyg.css` holds. `sheet_css` writes only values of safe characters (letters, digits,
` #%.,()-+_"'`, quotation marks only in pairs – never `<`, `;`, `{`, `}`, `\`), therefore the result may go unescaped into
a `<style>` element.

**Typeset view settings per work (Round 18c):** „Einstellungen → Werk → Satzbild“ (RPC `work.sheet`,
`work.sheet_set`, store `work_sheet`, migration 0044) override field by field what the template provides:
`sheet_style_over(sheet_style(vorlage), einstellungen)` – every set field wins, the rest stays from the
template. Fields: `font`, `heading_font`, `size`, `fill`, `heading_fill`, `width` and `background` (sheet colour,
`SheetStyle.background` → `--ws-background`; the template never delivers it, the sheet otherwise stays light; `sheet.css`
reads it). `wysiwyg.style`, the `<style id="ws-style">` in the editor and `work.sheet` compute through the same function
(`crate::sheet::sheet_style_for`) and deliver the same CSS. The settings hold **only in the typeset view of the
browser**: Typst typesetting, PDF and export never read `work_sheet` (byte-identical, test). After saving in the
settings (`baruch:settings-changed`) `sheet.ts` fetches `work.sheet` and replaces the content of `#ws-style`.

## RPC

All methods only read (no write lock), only through `POST /rpc` – not through `/act`, not over MCP.
`work` missing → default work. Errors through `INVALID_PARAMS` (-32602).

| Method | Parameters | Result |
|---------|-----------|----------|
| `wysiwyg.parse` | `{work?, body}` | `{blocks, html}`: blocks (as above, with `raw`) and HTML of the typeset view (images in islands, see below) |
| `wysiwyg.serialize` | `{work?, body, edits}` | `{body}`; invalid instructions → -32602 with a German message; Round 18b also `{replaces, raw}` (island in place) |
| `wysiwyg.style` | `{work?}` | `{css, style}` from the work template (`Store::work_template`) |
| `wysiwyg.diff` | `{work?, key\|node, rev, base?}` | `{html, base}`: revision `rev` against `base`; if `base` is missing, its parent (`based_on`), otherwise the current one; images as with `parse` |

HTML (`render_html`): per block `<div class="ws-block" data-b="i" data-kind="…">` (heading with
`data-level`, list with `data-ordered`, quotation with `data-block`; body in `div.ws-body`, attribution in
`div.ws-attribution`), islands with `contenteditable="false"` and `data-raw`, running text as `em`, `strong`,
`span.ws-footnote`, `q.ws-quote`, `span.ws-label`, `span.ws-soft`, `span.ws-nbsp`, `span.ws-island`. Every
text is escaped. The label behind a heading stands as
`span.ws-label.ws-heading-label[data-heading-label]` (not editable) at the end of the heading block, the
arguments of a scene as `div.ws-args[data-raw]` (not editable) before its body.

Round 18b: enumeration `div.ws-block[data-kind=enum][data-start=N]` with `<ol start="N">`, term list
`div.ws-block[data-kind=terms]` with `<dl><dt>Begriff</dt><dd>Erklärung</dd>…</dl>`, nested items as a list
in the `<li>` of the item before (same list element), break `<span class="ws-linebreak" contenteditable="false"
data-linebreak="1">↵<br></span>`, link `<a class="ws-link" data-url="…" href="…">Text</a>` (text editable;
`href` only with `http://`, `https://`, `mailto:`; without text `data-bare="1"` and not editable), reference
`<button type="button" class="ws-ref" contenteditable="false" tabindex="-1" data-target="…" data-supplement="…">@ziel</button>`
(`type="button"`: does not submit the editor form). The title of a block island reads „Typst-Insel: Doppelklick
öffnet den Quelltext an Ort und Stelle“, that of an image „Bild – Doppelklick öffnet den Quelltext an Ort und Stelle“.

**Images** (interface to "images", Round 7: as an image instead of a Typst island):
`baruch_wysiwyg::image_path(raw)` delivers the string literal in the first argument of the first `image(` call,
also in `#figure(image("…"), …)`, outside of comments, raw text and formulas (computed paths not).
Round 18b: `image_paths(raw)` delivers the paths of **all** `image(` calls; the typeset view shows every image of an
island (before only the first), caption and alternative text line as with the first image. Round 21b: with several
images the caption without `caption:` names every image (alternative text, otherwise path, separated by " · "), and
below it there is always `<span class="ws-alt ws-paths">Bilder: Pfad · Pfad</span>` (before only the first path).
`render_html_with(doc, &bilder)` or `diff_html_with(alt, neu, &bilder)` show every island with an image path for
which the callback delivers `Some(ImageRef { src, alt })` **as an image** and without visible raw text:

- Block island: `<div class="ws-block ws-figure" data-b="i" data-kind="island" data-image="1"
  contenteditable="false" data-raw="…" title="Bild – Quelltext nur im Quelltext bearbeitbar"><img
  class="ws-image" src="…" alt="…" loading="eager"><span class="ws-caption">…</span></div>`; the caption
  is `caption: […]` or `caption: "…"` literally, otherwise the alternative text, otherwise the path.
- Island within a sentence: `<span class="ws-island ws-inline-image" contenteditable="false" title="Bild – …"
  data-raw="…"><img …></span>`.

All attributes are escaped. The island stays an island (`data-kind="island"`, not editable, deletable as a
whole); the widget sends it as `keep`, on saving it stays byte-identical. Without an image from the
callback the island stays as before (raw text in `code.ws-raw`). `render_html(doc)` is
`render_html_with(doc, &|_| None)` and shows no images. The server (`wysiwyg::image_ref`) takes `src =
/werke/{work}/bilder/pfad?p=<Pfad, prozentkodiert>` with the path in normal form
(`baruch_store::normalize_asset_path`: `./bilder/a.png` and `/bilder/a.png` → `bilder/a.png`; since Round 18b without
a leading `/`, before `p=%2Fbilder%2F…`) and the
alternative text of the current revision (`wysiwyg::image_alts`, likewise by normal form). The image is delivered
only through this existing route (`nosniff`, CSP `sandbox`); it finds `/bilder/a.png`,
`bilder/a.png` and `./bilder/a.png` alike, `..` never. Diff (`diff_html`): blocks by the longest common subsequence over `raw` (without
trailing whitespace), equal ones in `div.ws-same`, removed ones in `div.ws-del`, new ones in `div.ws-ins`.

## Ribbon (Round 18a, docked since Round 33)

**Since Round 33 the toolbar docks into the ribbon of the page** (`docs/Architektur.md`, "Round 33 – Ribbon"):
it still stands in the fragment of the editor (`templates/fragments/edit_toolbar.html`, `div[data-edit-toolbar][hidden]`
in `div.wysiwyg`) and is redrawn along by `ui.render`; the widget `band-dock` (`web/src/band.ts`, mark at the
end of `fragments/editor.html`) hangs it, after every mounting, into the docking place `div[data-band-dock]` of the header.
It has two parts: `data-band-part="start"` (groups „Text“: „Satzbild“ | „Quelltext“; „Absatz“: paragraph format, „K“, „F“;
„Fassung“: ‹ n/m ›, „Speichern“, „Als neue Fassung“; „Assistenten“: „Prüfen“, „Vorschlag“) for the tab „Start“ and
`data-band-part="einfuegen"` („Einfügen“: the formats below; „Bild“; „Register und Glossar“) for the tab
„Einfügen“. Without JavaScript it stays hidden at the sheet; there are then, as before, text field and status line.

Consequences for the widgets: after docking, the toolbar no longer lies under `div.wysiwyg`. `wysiwyg.ts`, `ribbon.ts`
and `context_menu.ts` connect to their editor on mounting (before docking); what reaches into the toolbar later
(the switch „Satzbild“/„Quelltext“ in `show`) takes `editToolbar(root)` from `widgets.ts`. „Speichern“ and „Als
neue Fassung“ in the band belong to the form in the sheet through `form="editor-form"`; authoritative (label
„Gespeichert“/„Speichern“, disabled without a change) stays the button of the status line (`submitButton` skips the
buttons of the toolbar). While saving, `lockEditor` also locks the docking place. In the source the group
„Absatz“ shows the hint „nur im Satzbild“ (`data-ws-source-note`) instead of the tools.

The toolbar is the **ribbon for editing** (`templates/fragments/edit_toolbar.html`,
widget `ribbon` in `web/src/ribbon.ts`, style `static/ribbon.css` and `static/band.css`). It keeps all hooks that
`wysiwyg.ts` reads (`data-edit-toolbar hidden`, `data-ws-mode`, `data-ws-tools`, `data-ws-format`,
`data-ws-cmd="italic|bold"`; `data-ws-hint` in the status line); in the part "einfuegen" stands
`div.ribbon-formats[data-widget=ribbon]`:

- per format of the work template a button `button[data-ribbon-cmd=<WsCommandName>][data-ribbon-snippet]` from
  `crate::ribbon::formats` (RPC `ribbon.formats`): `scene`/`work` only if the template defines `#let szene`/`#let arbeit`,
  since Round 19b `scripture` („Bibelstelle“) only with `#let bibelstelle(…)` or `#let bibel(…)` and
  `cite` („Literaturverweis“) only with `#bibliography(` in the template, then always `footnote`, `quote`, `list`,
  `enum`, `terms`, `break`, `link` (since Round 30a `xref`);
- `button[data-ribbon-image]` („Bild“): only opens `details[data-image-insert]` (form of Round 18c).

**Formats of the template for references and literature (Round 19b):**

| Template contains | Button | Source (snippet) | Typeset view (`baruch:ws-command`) |
|---|---|---|---|
| `#let bibelstelle(…)` or `#let bibel(…)` | „Bibelstelle“ | `#bibelstelle[]` or `#bibel[]`, cursor in the brackets | `scripture`, `arg` = `{"fn": "bibelstelle"}`; the typeset view asks for the reference and sets the island `span.ws-island[data-raw="#bibelstelle[Joh 3,16]"]` (changeable only as a whole) |
| `#bibliography(` (not in a comment line) | „Literaturverweis“ | `@`, cursor behind it | `cite`, `arg` = key (missing: the typeset view asks); sets `button.ws-ref[data-target]` like an `@schlüssel` from the source |

The pure building blocks stand in `web/src/ws_command.ts` (imports nothing): `scriptureFunction(snippet)` takes the
name from `#name[]`, `scriptureSource(fn, stelle)` builds `#fn[stelle]` and escapes markup characters like `needs_escape`
in `baruch-wysiwyg` (`\ # * _ $ @ < > [ ] ~` and the backtick, line break → space, at most 200 characters), `citeKey(text)`
checks the key like a Typst label (letters, digits, `_ - : .`; `@` at the front and a full stop at the end are dropped).
Saving works as always through `wysiwyg.serialize`: the island literally, the reference as `@schlüssel` (or
`#ref(<schlüssel>)` if a label character follows). Assumption: the function of the template takes the reference as
content (`[…]`); a template whose function expects a string needs the source.

**Typeset view open** (`div.wysiwyg[data-ws-open]`): the button calls `sendWsCommand(root, cmd)` from
`web/src/ws_command.ts` (event `baruch:ws-command` at `div.wysiwyg`, without bubbles). If `wysiwyg.ts` sets
`handled = true`, the command is executed; if `handled` stays `false`, `ribbon.ts` shows a hint
(`baruch:toast`, „… geht im Satzbild noch nicht – bitte im Quelltext einfügen“). **Source:** `ribbon.ts` inserts
`snippet` itself at `selectionStart` into the text field (`execCommand("insertText")`, otherwise `setRangeText` + `input`);
block formats begin at the start of a line, a selection moves into the content (`[…]`), the cursor stands in the
content. No import between `ribbon.ts` and `wysiwyg.ts`.

With `satz_page` the same fragment still has the branch for the typesetting page (since Round 33 its buttons stand as
groups in the tab „Buch“; the branch is dropped after the merge of the round): the ribbon of the typesetting page (`/werke/{id}/satz`, `div.ribbon-satz
[data-ribbon-satz]`, without `hidden`, without JavaScript): back, PDF, ‹ › with `aria-label`, „Satz abbrechen“ as a
form (`docs/Bedienung.md`, „Satz (Typst-Vorschau)“).

## Usage

- **A click on a section** opens it in the typeset view (default), also after every redraw (outline,
  `baruch:navigate`, `ui.render`). The switch „Satzbild | Quelltext“ above the text switches; only this
  explicit choice sets the cookie `baruch_editor` (`satz` or `quelltext`), and only `quelltext` opens
  the editor in the source. If one arrives on loading the page through a hit of the literature search
  (`#fundstelle`), this first editor begins in the source because the highlight stands there; later clicks
  open the typeset view again.
- In the source the text field fills the sheet (the typeset view is `hidden`); when open, the typeset view fills the sheet.
  Toolbar and status line do not scroll along.
- Typing as in a word processor: **Enter** begins a new paragraph (in lists a new item),
  **Shift+Enter** a simple line break (in a heading a new paragraph: a heading
  is always one line), **Ctrl+I** emphasises, **Ctrl+B** sets bold (also through
  the buttons *K* and **F**), the choice „Absatz / Überschrift 1–3“ changes the format of the block at the cursor.
  A paragraph that begins with `# `, `## ` … becomes a heading of that level on Enter (Markdown, see
  above).
  Pasting takes text only; empty lines in the pasted text separate paragraphs (every further piece becomes a
  new paragraph, in lists everything stays one item).
- **Scenes and work assignments** stand there as a box; every paragraph in it can be edited on its own (Enter
  splits it, as outside). Only the edited paragraph is saved anew, the rest of the scene stays
  character for character. Arguments (`#szene(ort: …)`) and the label of a heading stand there in grey or yellow
  and are preserved while editing.
- **Images** from `#image(…)`/`#figure(image(…))` appear as an image with a caption (image from the page
  „Bilder“ of the work, also with paths with `./`), not as raw Typst text; a fine border on hovering shows
  that they can only be changed in the source or deleted as a whole.
- **Ctrl+S** saves (as in the source). Before saving, the widget transfers the typeset view into the text field;
  afterwards the same rules hold as in the source (the heading line determines title and level, splitting and
  merging after a query). An empty, multi-line or longer than 200 characters title is rejected with a
  message, nothing stored. Unsaved text in the typeset view triggers the same query as in the
  source.
- Yellow background: **Typst islands**; deletable as a whole in the typeset view and, since Round 18b, editable as source in place by
  **double click** (Ctrl+Enter accepts, Escape cancels).
- **Round 18b:** enumerations (`1.`), term lists (term bold, explanation indented; Enter begins a new
  entry), nested lists, forced breaks (↵), links (text editable, address in the title) and references
  (`@ziel` as a button; a click shows the label in the section, changing only in the source) stand in the typeset view
  instead of as islands. The buttons of the ribbon (scene, work assignment, footnote, quotation, list, enumeration,
  term list, break, link) act at the cursor when the typeset view is open: footnote, quotation and link enclose the
  selection (otherwise a selected placeholder), list/enumeration/term list turn the paragraph at the cursor into one
  (list ↔ enumeration switches), scene and work assignment take the block at the cursor into their body (at a
  heading or island an empty one arises behind it). If a command does not work at that place, a message says why.
- **Proposals** stand at their place (Round 8, see above): with an open typeset view directly at the block that
  they change, with the difference in the typeset view (removed struck through in red, new in green, ready-made by the server through
  `hunk_html_with`), in the source and without JavaScript in the reading flow below the text field with the line-by-line
  diff. A click on the coloured line expands or collapses a proposal (all its pieces).
- **„Fassung ändern“** stays source; since Round 18b Markdown headings are converted there as well (a
  further heading is rejected by „Fassung ändern“ like a typed `=` line).
- **Without JavaScript** the typeset view area is hidden; there is only the text field, which goes to
  `/gliederung/act` as before.

## Technology in the widget

- `wysiwyg.ts` attaches to `div.wysiwyg[data-widget=wysiwyg]` in the editor form, adopts the finished
  typeset view of the server (`data-ws-ready`, Round 8) or fetches `wysiwyg.parse`, plus `wysiwyg.style` (inserted
  as `<style id="ws-style">`), and shows the typeset view (`contenteditable`). Afterwards `placeProposals` puts the
  pieces of the proposals from `section.pv-flow` at their place (`returnProposals` fetches them back).
- Every input marks all touched blocks (`data-dirty="1"`, also nested ones in the body of a scene),
  sets `form.dataset.dirty = "1"` and enables the save button. New blocks (Enter) have no
  `data-b`; an index copied by the browser counts as new the second time. Blocks in the body of a scene
  or work assignment are numbered by the widget on opening (`data-n`); for an edited scene it sends a
  body instruction `{"replaces": i, "body": […]}`.
- Before saving, `outline.ts` fires `baruch:before-save` at the form; the widget then puts a promise into
  `detail.wait` that calls `wysiwyg.serialize`, writes the result into the text field and fires `input`.
  The text field is always what is submitted. As a fallback (as long as nobody fires the event) the widget intercepts
  the submission in the window, transfers and submits again.
- **Round 22 (amending):** the editor form carries the hidden field `mode=auto`; „Speichern“ (first
  button of the form – `submitButton` takes it for „Gespeichert“/„Speichern“) amends the own current revision
  as long as no rule demands a new revision, „Als neue Fassung speichern“ sends `as_new=1` (the server
  or `workspace.ts` turns that into `mode: "new"`). On submitting again after transferring the typeset view, the
  triggering button goes along (`form.requestSubmit(submitter)` in `wysiwyg.ts` and `outline.ts`) so that `as_new` does not
  get lost; Ctrl+S triggers without a button as before (i.e. „Speichern“, mode "auto"). After saving,
  `workspace.ts` shows „Gespeichert: fortgeschrieben (Stand k).“ or „Gespeichert: neue Fassung n (Grund).“; without
  JavaScript the same feedback stands in the editor header after the redirect (`saved=…` in the address).
- `wysiwyg.ts` imports nothing from `outline.ts`.
- **Source without saving (Round 17, contract §2.6):** whoever needs the complete source without saving
  (e.g. the live count in `timing.ts`, the check display `scripture_check.ts`) fires at the editor form
  `baruch:serialize-text` with `detail: { wait: Promise<string>[] }`. If the typeset view is open, `wysiwyg.ts` puts
  a promise into `wait`: without input since the last transfer the text field, otherwise `wysiwyg.serialize` with
  the same instructions as on saving – but without converting Markdown headings, without writing the text field
  and without changing `touched`. If nobody is responsible (source view), `wait` stays empty; the
  trigger then takes the text field.
- **Highlights of the check display (Round 17c):** the widget `scripture-check` (status line, `web/src/scripture_check.ts`)
  highlights checked quotations in the typeset view **only** through the CSS Custom Highlight API (`CSS.highlights`, names
  `bibel-stimmt` and `bibel-weicht`, style in `static/pruefung.css`): it lays `Range` objects over existing
  text nodes (the quotation text is searched, whitespace counts as one character; proposals `.pv` do not count) and does not change
  the DOM of the typeset view – `editsOf` thus sees no change, saving stays byte-identical. Every input
  clears the highlight. Without the Highlight API there is only the list.
- Headings (Round 7): `inlinesOf(…, heading = true)` turns line ends in the text into breaks,
  `splitHeading` splits at the first one (like `split_heading` in the server), `headingEdits` sends heading + new
  paragraph; `headingProblem` checks the title rules (level 1–3) before `wysiwyg.serialize` and aborts saving
  with the message; `guardHeadingMerge` (on `beforeinput`) intercepts Backspace/Delete at the boundary
  heading ↔ paragraph. `ws-figure` and `ws-inline-image` are islands like all others.
- **`guardHeadingMerge` across proposals (Round 18b):** the neighbour of a block is the next block before or
  behind it; pieces of proposals (`details.pv`) in between do not count (`blockNeighbour`). Before, the neighbour was
  the `<details>`, and Backspace at the start of the paragraph did pull it into the heading (review R8). Mock through
  RPC and check of the place in the widget: `round18_konstrukte.rs`
  (`a_proposal_between_heading_and_paragraph_does_not_disarm_the_merge_guard`); in the browser only checkable by hand.
- **Commands of the ribbon (Round 18b, contract R18-S1):** `baruch:ws-command` at the area `div.wysiwyg` (without
  bubbles, `web/src/ws_command.ts`, frozen): with an open typeset view the widget sets `detail.handled = true` and
  executes the command (`runCommand`, all `WsCommandName`: `scene`, `work`, `footnote`, `quote`, `list`, `enum`,
  `terms`, `break`, `link`; `arg` = address with the link or text of the placeholder, without an address a dialog asks);
  with a closed typeset view `handled` stays `false`, and the ribbon inserts the Typst text into the text field itself.
  The widget remembers the selection in the typeset view (`selectionchange`) so that a click into the ribbon does not lose it.
- **Cursor position (Round 18b):** `baruch:ws-caret` (at the area or at the editor form): with an open typeset view the
  widget puts exactly one promise into `detail.wait` with `{body, offset}` – `body` as with `baruch:serialize-text`, `offset` the
  UTF-16 length of the text that the same instructions yield up to and including the top-level block at the cursor (a
  line start behind its empty lines; without a cursor or behind the last block the end). Two calls of
  `wysiwyg.serialize` (text and prefix); the course through RPC is proven in `round18_konstrukte.rs`.
- **Style from the server (Round 18b):** if a `<style id="ws-style" data-work="W">` with the work of the editor already stands in the
  document (typeset view per work, `fragments/editor.html`), `applyStyle` adopts it without `wysiwyg.style`; other
  `#ws-style` (of other works) are dropped. Otherwise it fetches the style through RPC as before (one request per work at a time).

## Cross-references (Round 30a)

A **cross-reference** points to an outline item through its fixed anchor `<k{key}>`
(`baruch_typst::xref::key_label`). The anchors stand only in the export, in the mirror and in the typesetting, not in the
revisions (see `docs/Architektur.md`, "Cross-references (Round 30a)").

- **Form in the text** (`baruch_typst::xref::xref_snippet`): `#link(<k103>)[die Einleitung]`; with a page number
  `#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]`. No `@k103`: headings without a
  number cannot be referenced with `@` in Typst. In an English work the addition reads `, p.~#context …`
  (`xref_snippet_in`, Round 39b).
- **Block model:** `parse` recognises both forms as `Inline::Xref {key, c, page}` (JSON `{"t": "xref", "key": 103,
  "c": [...], "page": true}`). Only a canonical label `k` + digits without a leading zero and exactly this
  page-number addition with the same key are recognised. An empty link text, another label or another addition stay
  islands (`#link(<hafen>)[…]` as before). `emit` writes the form of `xref_snippet`; `emit`/`parse` make a
  round trip (property test `tests/xref.rs`, seed `WS_SEED`, cases `WS_CASES`). Round 39b: `parse` reads the
  addition in every work language (`, S.~` and `, p.~`); `emit_in`/`serialize_in` write it in the work language
  (the server passes it in `wysiwyg.serialize`), `emit`/`serialize` stay the German case.
- **Typeset view:** `render_html` sets a `span.ws-xref` with `data-key` and `data-page`, in it the editable
  link text, with a page number a non-editable `span.ws-xref-page` („, S. …“; `render_html_in` with the page
  abbreviation of the work language, „, p. …“ in an English work).
  - `wysiwyg.ts` reads the block back without the addition.
  - The command `xref` (`ws_command.ts`, `arg` = JSON `{key, text, page}`) inserts it at the cursor position.
  - Ctrl+click jumps to the target, a click shows „Öffnen | Bearbeiten“ (`xref.ts`).
- **Anchors in the export:** a `=` heading gets ` <kN>` before a comment. A heading with its own
  label (part `<baruch-part>`, „ohne Nummer“ `<baruch-unnumbered>`, label of the human at the end of the line or alone on
  the line below) keeps its label; below it stands the anchor line `#metadata("baruch-key") <kN>` (behind a
  label line). Typst knows only one label per element.
- **Measurement** (`crates/baruch-render/tests/xref.rs`): `#link(<kN>)` and `counter(page).at(<kN>)` resolve with both
  forms, and Typst checks through `#context assert` that every anchor lies on the page of its heading.
  - **Exception part in the typesetting:** with the part rule (`part_page_rule`, break after the part) the anchor line
    behind the part line would lie on the following page (proven).
  - The typesetting therefore puts it before the part line, as `#pagebreak(weak: true, to: "odd")#metadata("baruch-key")
    <kN>` (`part_anchors_for_typesetting`). The break of the rule is then dropped, the page number stays the same.
  - A reference to a missing anchor is an error in Typst with the label in the message. Therefore the
    typesetting turns broken references into plain text, and the print check names them.
- **Reading in:** the import removes anchor and anchor line (`strip_section`) and rewrites the references to the
  new keys (`remap_keys`).

## EPUB mapping (Round 20a)

The EPUB (`crates/baruch-epub`, `xhtml.rs`) takes the same block model as the typeset view: `baruch_wysiwyg::parse` per
section text (current revision as in the export, with anchor `<kN>`), then an XHTML mapping of its own – not
`render_html` (that writes editor HTML with `contenteditable`, `data-raw` and widget classes) and not typst-html.

| Block model | XHTML in the EPUB |
|---|---|
| heading of the section (`=`, part, „ohne Nummer“) | `<h1>`–`<h3 id="k{key}">`, before it `<span class="baruch-number">` with the number of the outline scheme; part `<h1 class="baruch-part-title">` |
| further heading in the text | `<h{level}>` (at most `<h6>`), with a label as `id="lbl-…"` |
| paragraph, `Soft` | `<p>`, a line break in the source stays whitespace |
| `_…_`, `*…*`, `\` | `<em>`, `<strong>`, `<br/>` |
| list `-`/`+`, enumeration `1.` | `<ul>`/`<ol>`, nested as in the typeset view, `start` from the first number |
| term list | `<dl><dt>…</dt><dd>…</dd></dl>` |
| block quotation, quotation within a sentence | `<blockquote class="baruch-quote">` with `<footer class="baruch-attribution">`, `<q>` |
| `#szene[…]`, `#arbeit[…]` | `<section class="baruch-scene">`, `<section class="baruch-work">`; arguments (`ort: […]`) are dropped, warning `ARGS_OMITTED` |
| `#link("…")[…]` | `<a href>` only for `http`, `https`, `mailto`, otherwise only the text |
| cross-reference `#link(<kN>)[…]` | `<a class="baruch-xref" href="k{Kapitel}.xhtml#kN">`; unknown key → text, warning `XREF_UNKNOWN` |
| reference `@name`, `@kN` | link to the label or the heading (text = heading), otherwise text |
| label `<name>` | `<span id="lbl-name"></span>` (the headings carry the anchors `<kN>`) |
| `#footnote[…]` | popup: `<sup><a epub:type="noteref" href="#fnN">` + `<aside epub:type="footnote" id="fnN">` in `ol.baruch-footnotes` at the end of the file; endnotes: reference to `notes.xhtml#fnN`, there with a back reference |
| island with `image("…")` (block) | `<figure class="baruch-figure"><img alt="…"/><figcaption>` from `caption:` |
| island with `image("…")` (within a sentence) | `<img class="baruch-inline-image" alt="…"/>` |
| `` `Code` `` | `<code>` |
| escape `\x` as an island | the character |
| comment, anchor line `#metadata("baruch-key") <kN>` | nothing, without a warning (anchor line → `id`, if still free) |
| every other island (`#set`, `#let`, formulas, own functions) | nothing, warning `ISLAND_OMITTED` with key and excerpt |

Alternative text of an image: `alt: "…"` in the Typst text, otherwise the alternative text of the image metadata; empty → `alt=""` and
warning `IMAGE_NO_ALT`. A path without an image in the work appears as „Bild fehlt: <Pfad>“ (warning `IMAGE_MISSING`).
All text is escaped (`escape_text`); nothing from the work is taken over as HTML.

## Index marks and context menu (Round 31c)

An **index mark** stands in the Typst text directly behind the marked word, canonically
`#metadata((register: "stichwort", term: "Gnade"[, sub: "…"][, see: "…"][, also: "…"]))<baruch-idx>`
(`baruch_typst::index::mark_source`, read by `parse_mark`). In the block model it is `Inline::IndexMark { mark }`, in
JSON `{"t": "index_mark", "mark": {"register": "…", "term": "…", …}}` (empty additions are missing, unknown fields are rejected by the
server). A mark with a space before the label or another key order is not a mark but stays an island.

In the typeset view (`render_html`) it appears as an empty element that cannot be typed into

```html
<span class="ws-index-mark" contenteditable="false" role="img" tabindex="-1"
      title="Registereintrag im Stichwortregister: Gnade › im Alltag; siehe auch: Huld"
      aria-label="…" data-register="stichwort" data-term="Gnade" data-sub="im Alltag" data-also="Huld"
      data-raw="#metadata((…))&lt;baruch-idx&gt;"></span>
```

- **Discreet:** the small sign (⁘, blue in the index of persons) comes only from the style (`::before` in `wysiwyg.css`); the
  element has no text node, therefore text, search, highlighting and quotation check of the paragraph stay untouched.
- **Description:** `title`/`aria-label` = `baruch_wysiwyg::index_mark_label` (`Registereintrag im {Register}: {Stichwort}
  › {Unterstichwort}; siehe: …; siehe auch: …`; own indexes as `Register „slug“`). `markLabel` in `ws_command.ts`
  follows the same rule (Node comparison test).
- **Back:** `wysiwyg.ts` reads `span.ws-index-mark` from the `data-*` fields as `index_mark` (before the check for
  islands); the server writes it canonically with `emit`. An untouched block stays `keep` and byte-identical; a block
  in which a mark was set, changed or removed is generated anew – with canonical text only the
  mark changes (test `baruch-wysiwyg/tests/index_mark.rs`).
- **Setting, changing, removing** only through the command `index_mark` (`baruch:ws-command`, `arg` = JSON
  `{op: "add" | "edit" | "remove", mark?}`), which the context menu sends: `add` sets the mark behind the end of the
  selection (trailing whitespace does not count) or behind the word at the cursor; not in a heading, part, „ohne Nummer“,
  island, reference, label or another mark (message). `edit`/`remove` act on the mark that the context menu
  designated before with `data-ctx-target`. Afterwards the editor is unsaved; saving with „Speichern“ creates a
  new revision of the human.
- **Double click** on a mark opens „Registereintrag bearbeiten“.

**Context menu** (`web/src/context_menu.ts`, widget `context-menu` on the ribbon group „Register und Glossar“):

- Opening in the typeset view and in the source text field with a right click, the context menu key or Shift+F10 (`opensMenu`), or
  through the link „Markierung …“ of the status line. If the browser additionally sends `contextmenu` after the key, it does not
  open twice.
- ARIA menu pattern: `div.ctx-menu[role=menu]` with `button[role=menuitem]` (roving `tabindex`), arrow down/up wrap around,
  Home/End, Enter/Space trigger, Escape and Tab close; the focus (and in the text field the selection) goes back
  (`menuKey`).
- Entries: „In Glossar aufnehmen …“ (dialog with term = marked word, short/long explanation, variants and
  Bible references one per line, source → RPC `glossary.add`, messages of the server in the dialog), „In Register aufnehmen …“
  (headword, sub-headword, index from `index.registers` – without an answer the built-in ones –, see, see also → mark
  as above or in the source `markSource` behind the word, `markInSource` with the same prohibitions as above),
  „Im Glossar nachschlagen“ (RPC `glossary.lookup`, popup with terms, variants, explanations and a link to
  `/werke/{id}/glossar#begriff-{id}`); on a mark (right click on it, cursor directly next to it, in the text field cursor
  in it) additionally „Registereintrag bearbeiten …“ and „… entfernen“.
- Ribbon: the same actions as buttons (`data-ctx-action`) and „Register“ (button `[data-index-toggle]` with `aria-expanded`, Escape closes), which on
  expanding lists all index entries of the section – from the marks of the typeset view or of the text field (`findMarks`),
  so also unsaved ones; a click jumps to the mark.
- Without JavaScript the ribbon is hidden; the link „Markierung …“ leads to `/werke/{id}/markierung?key=K&rev=R` with
  the same three actions as forms (`docs/Bedienung.md`).

## Quotation block (Round 23)

`baruch_wysiwyg::parse` reads a logical line that is exactly one `#quote(block: true, …)[…]` as a piece of its own
(like `#szene[…]`/`#arbeit[…]`): a line directly behind it is a paragraph of its own, a line directly before it ends
its paragraph – as Typst typesets it. Until Round 22 a quotation block with the reference in the following line became a
code island. `#quote[…]`, `block: false` and unreadable arguments stay part of their paragraph. Untouched blocks go back
byte-identically (`serialize`).

The button „Zitat“ (`ribbon::QUOTE_SNIPPET`) inserts `#quote(block: true)[]` – since Round 36 without the following line
"– " for the reference (left unfilled, a single dash remained in the book; the author writes the reference
themselves in the line below the block) –, with exactly one empty line before and after it (existing ones count), cursor in the body; with a selection the
selection moves into the body. Not `attribution: []`: an empty `attribution` sets, in Typst 0.15.1, a line with the
em dash (measured in `crates/baruch-render/tests/quote_attribution.rs`). In the typeset view, without a selection,
the same arises as a block of its own behind the paragraph at the cursor; with a selection it stays a quotation in running text. Without JavaScript
there is no ribbon; a block written by hand is read the same way. Tests: `baruch-wysiwyg/tests/zitat.rs`,
`round23_web::widgets::the_quote_button_inserts_the_block…`, acceptance test `round23_abnahme.rs`.
