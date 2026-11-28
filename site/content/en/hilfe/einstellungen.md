---
title: "Settings"
description: "All pages of the settings: display, work, assistants, connections, sources, MCP access and data."
weight: 120
---

**Baruch ▾ → Settings** opens the settings. This page describes what is on each of their pages. The first setup
of Bibles, assistants and services is shown step by step in [Installation](/en/installation/#bibeln-einrichten)
and [MCP and assistants](/en/mcp/#assistenten-einrichten).

## Opening the settings {#oeffnen}

{{< hilfebild name="einstellungen-popup" alt="The settings as a window above the work page: on the left the list of pages – Display, Work, Assistants, Connections, Sources, MCP access, Data –, on the right the chosen page; at the top right the button Close." titel="With JavaScript the settings open as a window above the work page." >}}

How it works:

1. Choose **Baruch ▾ → ⚙ Settings** or press **Alt+,** (unchecked on the Mac, see
   [Keys](/en/hilfe/tasten/#ueberall)).
2. Choose a page on the left; it appears on the right.
3. Every form has its own button **Save** or **Apply**. **Esc** or **Close** closes the window.

At the top, every page says where its values are kept ("Saved: …") – only in this browser, for one work or for
all works. If Baruch refuses an input, the page stays with the message and everything entered. Without JavaScript
the settings are ordinary pages.

## Display {#darstellung}

{{< hilfebild name="einstellungen-darstellung" alt="Settings, page Display: the choice of the theme (like the system, light, dark), of the view when opening (typeset view or source text), whether proposals are unfolded at first, which area on the right is open, and the button Reset column widths." titel="The display applies to this browser." >}}

Applies only to **this browser**; another browser has its own values.

- **Theme:** like the system, light or dark. The small switch for it is also found in the tab **View**.
- **Editor when opening:** typeset view or source text.
- **Proposals in the reading flow:** unfolded or folded at first (folded, they appear as a coloured line; only the
  one shown at the moment is open).
- **Open area on the right:** Assistants, Revisions or Chat.
- **Reset column widths.**

## Work {#werk}

{{< hilfebild name="einstellungen-werk" alt="Settings, page Work: at the top the details of the work and the work template, below them the blocks outline scheme, book details, cover, typeset view, typesetting, revisions, sermon time, commentaries and lexicons, and sync with MultiGPT." titel="Everything that concerns only this one work." >}}

The page gathers everything that concerns **one work**. Each block is described in its place in the help:

| Block | Content | described under |
|---|---|---|
| Details and work template | title, subtitle, author, kind, audience, Bible translation, template with its revisions | [Changing a work](/en/hilfe/erste-schritte/#werk-bearbeiten) |
| Outline scheme | numbering of chapters and sections, with preview | [Outline scheme](/en/hilfe/gliederung/#schema) |
| Book details, cover | publisher, ISBN, blurb, front matter, cover picture | [Cover and front matter](/en/hilfe/buchsatz/#umschlag) |
| Typeset view | font, size, width and colours while writing on the screen | [Typesetting settings](/en/hilfe/buchsatz/#satz-einstellungen) |
| Typesetting | page format, margins, font, page numbers, index and glossary, draft | [Typesetting settings](/en/hilfe/buchsatz/#satz-einstellungen) |
| Revisions | amending, limits for new revisions, hiding older revisions | [Saving and revisions](/en/hilfe/schreiben/#fassungen) |
| Work language | German or English: the language the assistants answer in | [Work language](/en/hilfe/einstellungen/#werksprache) |
| Sermon time | words per minute, target time | [Time keeper](/en/hilfe/schreiben/#zeitwaechter) |
| Commentaries and lexicons | which reference works the work uses | [Choosing the Bible of the work](/en/hilfe/bibel-quellen/#bibeln-waehlen) |
| Sync with MultiGPT | switch "upload to MultiGPT" (off by default) | [Sync with MultiGPT](/en/hilfe/bibel-quellen/#abgleich) |

### Work language {#werksprache}

Every work has a **work language**: German or English. You choose it in the block "Work language" and confirm
with "Save". The assistants answer in this language – even when the question or an own prompt is written in
another language –, and they write their markers in it ("Kritisch:" or "Critical:", "Vorschlag:" or
"Suggestion:"). The language of the interface does not change through this, and Baruch changes nothing in the
text of the work. A new work is German; in the [exchange](/en/hilfe/austausch-sicherung/#einlesen) the work
language travels with the work. Only the human can change it, no assistant.

The page also shows the **identifier of the work** – a fixed mark that cannot be changed, by which another Baruch
recognises in the [exchange](/en/hilfe/austausch-sicherung/#einlesen) whether it already knows the work.

## Assistants {#assistenten}

{{< hilfebild name="einstellungen-assistenten" alt="Settings, page Assistants: at the top the choice of the lector, below it a coloured tab per role; in the chosen tab the fields name, handle, colour, role, model, context length, active and the large text of the instruction, below them the forms model connection and the switches for evidence." titel="One tab per assistant with all its settings." >}}

Applies to **all works**.

- **Lector:** "fixed" to a particular role or "automatically by context" (the active role with the largest
  capacity). If the fixed role is not active, the automatic rule applies; the page says so.
- **One tab per assistant** in its colour. In it, a form with one button **Save**:
  - **Name** and **Handle** (the handle, such as `@lektor`, is display only – you address the assistant with it in
    the chat),
  - **Colour** – it colours its posts, revisions and proposals; **Palette colour** resets it,
  - **Role** (short description) and **active**,
  - **Model** – an input field with the list of models the service reports,
  - **Context length** – how much text the model can take in at once; Baruch shortens accordingly,
  - the **Role prompt**: the detailed instruction to the model. The default can be replaced by an own text;
    **Restore default** asks first.
- **Model connection** (per assistant): which service drives it, the badge **local** or **external** and the check
  box **Send book text to an external connection** (default: off). Without this clearance Baruch refuses requests
  to an external service with an honest sentence; nothing goes out. Below it the **effective context** is shown:
  the smaller of the set length and what the service reports.
- **Pass along evidence** (only for @belege and the lector): hits of the semantic search and evidence from the
  Bible, commentaries and lexicons. Only for @belege: **research on the web** (off by default), see
  [Web search for evidence](/en/hilfe/assistenten/#websuche).
- **Chat: what the assistants get to see:** the whole chapter (default) or **Chat only the section**.
- If the list of models is still empty, it says "Model list not fetched yet" with the link **Fetch model list**.
  The page itself asks no service when it opens.

The button **Colours …** in the area "Assistants" of the work page leads here.

## Connections {#verbindungen}

{{< hilfebild name="einstellungen-verbindungen" alt="Settings, page Connections: per service a box with address, the note stored or missing for the credentials and the buttons Save and Check connection; the addresses in the picture are placeholders." titel="One form per service: address, credentials, check." >}}

Applies to **all works**. If you like, Baruch works together with four kinds of services; none of them is needed
for writing.

| Connection | what for |
|---|---|
| Eliezer and further model connections | services that provide language models – they drive the assistants |
| LanceDB (local) | the index for the semantic search in the own data folder |
| pgvector | an own database with texts for the semantic search |
| MultiGPT | an own document service: search, full texts, sync, web search |

For each connection:

- **Save** takes over the address and the fields.
- **Credentials** (key, token – a kind of password for programs –, password): the field is always empty; next to
  it only **stored** or **missing** is shown. An input replaces, leaving it empty changes nothing. Credentials lie
  in the keyring of the system and never appear on a page.
- **Credentials only apply to the address they were entered for.** If you change the address, Baruch does not send
  them to the new target until you enter them anew for it.
- **Check connection** asks the service once (at most 10 seconds) and shows the result: for model services the
  list of models, for MultiGPT and pgvector the collections. A failure appears as "✗ failed" with a message.
- **Quick test** (for model connections) sends the fixed request "Reply only with: OK" – without book text.
- **Remove secret** and **Delete connection**, each with a check box.

**New model connection:** name, kind (Eliezer, OpenAI-compatible or Ollama), address and the **location**: "by the
address" (default), "external" or "local". "By the address" treats only recognisably local addresses as local,
everything else as external – so a forgotten tick does not open the lock. If a connection is listed as local but
its address is not recognisably local, its box shows a red warning.

**Paths of the semantic search** (at the bottom of the page, per work):

- **RAG paths of the work:** check boxes for LanceDB, pgvector and MultiGPT. Without a tick there is only the
  search in the wording.
- **Collections:** only ticked collections of pgvector and MultiGPT are searched. First **Check connection**, then
  they appear for choosing.
- **Status of the paths:** per path "connected", "failed" or "off" with the reason, plus the state of the local
  index.
- **Rebuild index** (with a check box) rebuilds the local index of the work – needed after changing the embedding
  model.

The local index follows the work by itself: after every save Baruch updates it in the background. Without a
service that prepares texts for the semantic search, the search stays in the wording. The semantic search only
reads.

## Sources {#quellen}

{{< hilfebild name="einstellungen-quellen" alt="Settings, page Sources: at the top the list of preferred sources with title, address and note, below it the section Bibles (SWORD) with the module folders, the table of modules and the choice of the Bible of the work." titel="Preferred sources for the web search, and the Bibles." >}}

- **Preferred sources:** a list of web pages with title, address and note, in a fixed order. Per source ↑ and ↓ for
  moving, **Edit** and **Delete**; at the bottom **New source**. The web search of @belege asks the first three
  sources of this list first. Assistants can read the list; only the human can change it.
- **Bibles (SWORD):** folders of the Bible modules, list of modules, Bible of the work, keys of encrypted modules
  – see [Choosing the Bible of the work](/en/hilfe/bibel-quellen/#bibeln-waehlen).

## MCP access {#mcp-zugang}

{{< hilfebild name="einstellungen-mcp" alt="Settings, page MCP access: the address for programs, the ready command for an assistant program, the note that no access key (token) has been created yet, the form Create token and the list of programs connected since the start." titel="The access for programs – with access keys for other computers." >}}

**MCP** is the interface through which other programs – such as an assistant in the terminal – work with Baruch:
reading, writing proposals and posts, creating ideas. How to connect such a program is described under
[MCP clients](/en/mcp/#mcp-clients).

The page shows:

- the **address** for copying and the ready command for connecting;
- the **programs connected** since the start with name, version and the name their posts appear under;
- the **Tokens for other computers**. A token is an access key for a program.

From the own computer a program needs no token. From another computer it only works with one:

1. Enter a name (for example that of the computer) and press **Create token**.
2. The answer page shows the token **exactly once**. Copy it at once; whoever loses it creates a new one.
3. Enter the token in the other program.

- **Revoke** (with a check box) makes a token invalid at once.
- The list shows name, beginning of the token, author, created, last used and state – never the whole token.
- Posts and proposals of such an access appear under the name of the token.
- **Caution:** without a server in front with encryption, tokens and texts go through the network unencrypted.
  Only use it in the own, trustworthy network; everything else under [Operation](/en/betrieb/#reverse-proxy).

## Data {#daten}

{{< hilfebild name="einstellungen-daten" alt="Settings, page Data: data folder, size and version with the link About Baruch, the section This instance with identifier and name, the section EPUBCheck with the field for the path and the section Backup with the button Back up now." titel="Data folder, name of this Baruch, EPUBCheck and backup." >}}

- **Data folder, size, version** – with the link **About Baruch**. If the data folder is also accessible to other
  users of the computer, a warning appears here.
- **This instance:** identifier (display only) and **Instance name** with **Save name** – see
  [The name of this Baruch](/en/hilfe/austausch-sicherung/#instanz).
- **EPUBCheck:** whether the checking program for e-books has been found, and the field **Path to epubcheck.jar**
  with **Save path** – see [E-book](/en/hilfe/buchsatz/#epub).
- **Back up now** – see [Backup](/en/hilfe/austausch-sicherung/#sicherung).
- For each work its folder with the last entry in the history.

With [accounts](/en/hilfe/konten/) the page **Accounts** is added.
