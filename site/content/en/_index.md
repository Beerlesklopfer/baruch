---
title: "Baruch"
claim: "Free writing workshop for books and sermons"
lead: "Baruch is a writing environment for sermons, talks and books – from the first idea through Bible study to the print-ready book, without handing control to an AI. Every revision is kept, assistants read along and make suggestions, and the human keeps the last word."
description: "Free writing workshop for books and sermons: revisions, book typesetting with Typst and assistants that only suggest. AGPL-3.0, runs on your own computer."
---

> Named after Baruch, Jeremiah's scribe, who wrote the burnt scroll down again, and enlarged
> (Jer 36:32). Revisions are the core.

## Write, check, typeset {#schreiben-pruefen-setzen}

- **Write:** a sermon or a book as an outline of parts, chapters and sections. The text appears as it will be
  printed, and every revision is kept – going back to yesterday's takes one click.
- **Check:** Bible references and quotations are compared with the chosen Bible translation. An editor, an
  exegete or a critical listener read along on request and suggest improvements; for sermons a time keeper
  shows the speaking time.
- **Typeset:** the same text becomes the print-ready book PDF with cover, an e-book and note sheets for the
  pulpit – without any additional program.

More under [Features](/en/funktionen/).

## Free {#frei}

Baruch is licensed under the **GNU Affero General Public License 3.0** (or later) and is deliberately a free
counterpart to paid writing tools. The source code is open; whoever changes Baruch and offers it to others over a
network releases their changes as well. More under [Licence](/en/lizenz/).

## Local {#lokal}

Baruch runs on your own computer and is used in the browser; by default it can only be reached from there
(`http://127.0.0.1:8765/`). The data stays in your own user folder: one database file and, per work, a **Git
mirror** – an automatic backup of all drafts as ordinary text files, traceable with every change and readable and
typesettable even without Baruch. Keys and tokens live in the keyring of the system, never in files. Baruch never
asks the net for new versions on its own; models of third-party providers are used only after explicit
clearance.

## The human decides {#der-mensch-entscheidet}

Assistants write **proposals**, posts, ideas and requests. Accepting, changing and deleting book text stay with
the human alone – in the browser as well as through the interfaces for other programs. Every section has any
number of revisions, exactly one is the current one, and switching works at any time. Every change is recorded in
the mirror of the work, with its origin: human, assistant or a connected AI program. What does not work, Baruch
says openly; nothing is silently shortened, left out or made up in the background – not even when a text is too
long for an AI model.

## Usable without JavaScript {#ohne-javascript}

Every page of Baruch works with links and forms. JavaScript adds comfort – dragging, keyboard shortcuts, windows
that open, immediate updates – but is never required. This website, too, works without JavaScript, cookies,
counters and third-party fonts.

## Next {#weiter}

- [Features](/en/funktionen/): what Baruch can do, with pictures
- [Installation](/en/installation/): packages for Debian and Ubuntu, AppImage, macOS and Windows – and the
  [first steps afterwards](/en/installation/#nach-der-installation)
- [Help](/en/hilfe/): using Baruch, step by step and with pictures – from the
  [first start](/en/hilfe/erste-schritte/#starten) to the [finished book](/en/hilfe/buchsatz/#pdf)
- [Operation](/en/betrieb/): data folder, backup, permanent operation and the way into the network through a
  web server in front
- [MCP and assistants](/en/mcp/): set up assistants with an AI model, connect AI programs such as Claude Code
  with Baruch
